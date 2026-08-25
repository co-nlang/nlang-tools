//! Local store garbage collection (local_gc arc).
//!
//! Roots: HEAD → commit chain (`parent`) + each commit's `root` tree, then
//! every digest referenced by those value objects. **`CommitMeta.abandoned`
//! is not a root** (R-b). Forgetting never runs automatically (R-c).
//!
//! REAL_03 §6.6 (verdict_must_gate): reachable `#object_undecodable` /
//! `#caid_mismatch` make the walk **incomplete** — `run_gc` must not sweep.

use crate::storage::ObjectStore;
use crate::value::{Commit, ContentHash, Value};
use serde_json::Value as JsonValue;
use std::collections::{BTreeSet, VecDeque};
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct GcReport {
    pub total_objects: usize,
    pub reachable: usize,
    pub collectable: usize,
    pub collectable_bytes: u64,
    /// Collectable digests that would be reachable if abandoned heads were roots.
    pub abandoned_content: usize,
    pub removed: usize,
    pub freed_bytes: u64,
    /// Reachable integrity findings — reported; **block the sweep** when non-empty.
    pub integrity: Vec<String>,
    /// True when any reachable object failed decode or address recompute.
    pub walk_incomplete: bool,
}

/// Collect digests a JSON object refers to (64-hex strings and byte arrays).
/// `follow_abandoned`: when false, skip the `abandoned` field (R-b).
pub fn refs_of(v: &JsonValue, follow_abandoned: bool, out: &mut Vec<String>) {
    match v {
        JsonValue::Object(m) => {
            for (k, x) in m {
                if k == "abandoned" && !follow_abandoned {
                    continue;
                }
                if k == "__nlang_system_digest" {
                    // O58: this is a real CAS edge to the packed standard
                    // root object.  It is intentionally a plain string inside
                    // a typed Atom, not a ContentHash-shaped `digest` field.
                    refs_of_standard_digest(x, out);
                } else if k == "digest" {
                    match x {
                        JsonValue::String(s) if s.len() == 64 && is_hex64(s) => {
                            out.push(s.to_lowercase());
                        }
                        JsonValue::Array(a) => {
                            let hex: String = a
                                .iter()
                                .map(|b| format!("{:02x}", b.as_u64().unwrap_or(0)))
                                .collect();
                            if hex.len() == 64 {
                                out.push(hex);
                            }
                        }
                        other => refs_of(other, follow_abandoned, out),
                    }
                } else {
                    refs_of(x, follow_abandoned, out);
                }
            }
        }
        JsonValue::Array(a) => {
            for x in a {
                refs_of(x, follow_abandoned, out);
            }
        }
        JsonValue::String(s) if s.starts_with("hash:sha256:") => {
            if let Some(d) = s.rsplit(':').next() {
                if d.len() == 64 && is_hex64(d) {
                    out.push(d.to_lowercase());
                }
            }
        }
        _ => {}
    }
}

fn refs_of_standard_digest(v: &JsonValue, out: &mut Vec<String>) {
    match v {
        JsonValue::String(s) if is_hex64(s) => out.push(s.to_lowercase()),
        JsonValue::Object(m) => {
            for value in m.values() {
                refs_of_standard_digest(value, out);
            }
        }
        JsonValue::Array(values) => {
            for value in values {
                refs_of_standard_digest(value, out);
            }
        }
        _ => {}
    }
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F'))
}

/// Result of verifying one on-disk object at a requested digest.
enum VerifiedObject {
    /// Address recomputed and matches; refs ready for the walk.
    Ok(Vec<String>),
    /// Bytes present but do not hash/decode to the requested address.
    CaidMismatch,
    /// Present but neither a Value nor a Commit (or unreadable).
    Undecodable,
}

fn json_refs(json: &JsonValue, follow_abandoned: bool) -> Vec<String> {
    let mut refs = Vec::new();
    refs_of(json, follow_abandoned, &mut refs);
    refs
}

/// Read + recompute (REAL_03 §6.6). Try Value then Commit so a genuine
/// engine Commit is not mis-reported as undecodable (v0.2.52 trap).
fn verify_reachable_object(
    store: &ObjectStore,
    digest_hex: &str,
    follow_abandoned: bool,
) -> VerifiedObject {
    let Ok(bytes) = store.read_raw_digest(digest_hex) else {
        return VerifiedObject::Undecodable;
    };
    let want = digest_hex.to_lowercase();
    let text = String::from_utf8_lossy(&bytes);

    if crate::store_codec::is_framed(&text) {
        let refs = crate::store_codec::refs_of_document_ex(&text, follow_abandoned);
        let Ok(bytes_digest) = hex::decode(digest_hex) else {
            return VerifiedObject::Undecodable;
        };
        let h = ContentHash::v1(bytes_digest);
        let is_mismatch = |e: &anyhow::Error| {
            matches!(
                e.downcast_ref::<crate::storage::StoreReadError>(),
                Some(crate::storage::StoreReadError::CaidMismatch { .. })
            )
        };
        match store.get_value(&h) {
            Ok(_) => return VerifiedObject::Ok(refs),
            Err(e) if is_mismatch(&e) => return VerifiedObject::CaidMismatch,
            Err(_) => {}
        }
        match store.get_commit(&h) {
            Ok(_) => return VerifiedObject::Ok(refs),
            Err(e) if is_mismatch(&e) => return VerifiedObject::CaidMismatch,
            Err(_) => {}
        }
        return VerifiedObject::Undecodable;
    }

    // Format-3 roots are decoded through the store's single value decoder;
    // raw serde alone sees their compact dependency marker, not the logical
    // root, and would falsely condemn a healthy reachable object.
    if let Ok(bytes_digest) = hex::decode(digest_hex) {
        if store.get_value(&ContentHash::v1(bytes_digest)).is_ok() {
            if let Ok(json) = serde_json::from_slice::<JsonValue>(&bytes) {
                return VerifiedObject::Ok(json_refs(&json, follow_abandoned));
            }
        }
    }

    if let Ok(val) = serde_json::from_slice::<Value>(&bytes) {
        let recomputed = val.content_hash();
        if hex::encode(&recomputed.digest) == want {
            if let Ok(json) = serde_json::from_slice::<JsonValue>(&bytes) {
                return VerifiedObject::Ok(json_refs(&json, follow_abandoned));
            }
        } else {
            return VerifiedObject::CaidMismatch;
        }
    }

    if let Ok(commit) = serde_json::from_slice::<Commit>(&bytes) {
        let recomputed = commit.content_hash();
        if hex::encode(&recomputed.digest) == want {
            if let Ok(json) = serde_json::from_slice::<JsonValue>(&bytes) {
                return VerifiedObject::Ok(json_refs(&json, follow_abandoned));
            }
        } else {
            return VerifiedObject::CaidMismatch;
        }
    }

    // Neither Value nor Commit verified — including "JSON that is neither".
    VerifiedObject::Undecodable
}

/// Mark phase: reachable digests from HEAD. Reports integrity findings;
/// incomplete walks still list what was seen, but must not drive a sweep.
pub fn mark(
    store: &ObjectStore,
    base_dir: &Path,
    follow_abandoned: bool,
) -> (BTreeSet<String>, Vec<String> /* integrity */) {
    let mut integrity = Vec::new();
    let mut seen = BTreeSet::new();
    let Ok(Some(head)) = store.get_head(base_dir) else {
        return (seen, integrity);
    };
    let mut stack = VecDeque::new();
    stack.push_back(hex::encode(&head.digest));

    while let Some(d) = stack.pop_front() {
        if seen.contains(&d) {
            continue;
        }
        // Absent: continue without a verdict (REAL_03 §6.6 / ruling R3).
        if !store.object_exists_digest(&d) {
            continue;
        }
        seen.insert(d.clone());
        match verify_reachable_object(store, &d, follow_abandoned) {
            VerifiedObject::Ok(refs) => {
                for r in refs {
                    if !seen.contains(&r) {
                        stack.push_back(r);
                    }
                }
            }
            VerifiedObject::CaidMismatch => {
                integrity.push(format!(
                    "integrity #caid_mismatch: reachable digest {d} does not match its bytes"
                ));
                // Do not follow refs — they are the forger's graph.
            }
            VerifiedObject::Undecodable => {
                integrity.push(format!(
                    "integrity #object_undecodable: reachable digest {d} cannot be decoded"
                ));
            }
        }
    }
    (seen, integrity)
}

/// Plan a collection without deleting.
pub fn plan_gc(store: &ObjectStore, base_dir: &Path) -> Result<GcReport, String> {
    let all = store
        .list_digests()
        .map_err(|e| format!("list objects: {e}"))?;
    let total_objects = all.len();
    let (live, integrity) = mark(store, base_dir, false);
    let (live_abs, _) = mark(store, base_dir, true);

    let mut collectable = 0usize;
    let mut collectable_bytes = 0u64;
    let mut abandoned_content = 0usize;
    for (d, len) in &all {
        if live.contains(d) {
            continue;
        }
        collectable += 1;
        collectable_bytes += len;
        if live_abs.contains(d) {
            abandoned_content += 1;
        }
    }

    let walk_incomplete = !integrity.is_empty();
    Ok(GcReport {
        total_objects,
        reachable: live.len(),
        collectable,
        collectable_bytes,
        abandoned_content,
        removed: 0,
        freed_bytes: 0,
        integrity,
        walk_incomplete,
    })
}

/// Sweep unreachable objects under `.oo/objects/` only.
///
/// If the reachability walk is incomplete (any `#object_undecodable` /
/// `#caid_mismatch` on a reachable object), **nothing is deleted** and the
/// call fails — REAL_03 §6.6 / verdict_must_gate. Dry-run still reports.
pub fn run_gc(store: &ObjectStore, base_dir: &Path, dry_run: bool) -> Result<GcReport, String> {
    let mut report = plan_gc(store, base_dir)?;
    if dry_run {
        return Ok(report);
    }
    if report.walk_incomplete {
        return Err(format!(
            "gc refused: reachability walk incomplete ({} integrity finding(s)); nothing removed",
            report.integrity.len()
        ));
    }
    if report.collectable == 0 {
        return Ok(report);
    }

    let all = store
        .list_digests()
        .map_err(|e| format!("list objects: {e}"))?;
    let (live, integrity) = mark(store, base_dir, false);
    // Re-check after a second walk (store could change under us).
    if !integrity.is_empty() {
        report.integrity = integrity;
        report.walk_incomplete = true;
        return Err(format!(
            "gc refused: reachability walk incomplete ({} integrity finding(s)); nothing removed",
            report.integrity.len()
        ));
    }

    let mut removed = 0usize;
    let mut freed = 0u64;
    for (d, len) in all {
        if live.contains(&d) {
            continue;
        }
        store
            .remove_digest(&d)
            .map_err(|e| format!("remove {d}: {e}"))?;
        removed += 1;
        freed += len;
    }
    report.removed = removed;
    report.freed_bytes = freed;
    Ok(report)
}

/// Whether a CAID's object still exists (for `oo log` abandoned lines).
pub fn content_present(store: &ObjectStore, caid: &ContentHash) -> bool {
    store.object_exists_digest(&hex::encode(&caid.digest))
}

pub fn format_plan_report(r: &GcReport) -> String {
    let mut s = format!(
        "oo gc: {} objects, {} reachable, {} collectable ({} bytes)\n",
        r.total_objects, r.reachable, r.collectable, r.collectable_bytes
    );
    if r.abandoned_content > 0 {
        s.push_str(&format!(
            "        {} of them are content of heads abandoned by #rollback — after this,\n\
                     `oo log` can name them but not resolve them, and rolling forward is\n\
                     no longer possible\n",
            r.abandoned_content
        ));
    }
    s.push_str(
        "        expects exclusive use of the workspace (concurrent writers are not locked)\n",
    );
    for msg in &r.integrity {
        s.push_str(msg);
        s.push('\n');
    }
    s
}

pub fn format_done_report(r: &GcReport) -> String {
    format!(
        "oo gc: removed {} objects, freed {} bytes",
        r.removed, r.freed_bytes
    )
}
