//! Local store garbage collection (local_gc arc).
//!
//! Roots: HEAD → commit chain (`parent`) + each commit's `root` tree, then
//! every digest referenced by those value objects. **`CommitMeta.abandoned`
//! is not a root** (R-b). Forgetting never runs automatically (R-c).

use crate::storage::ObjectStore;
use crate::value::ContentHash;
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
    /// Undecodable but *reachable* objects — reported, never swept.
    pub integrity: Vec<String>,
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
                if k == "digest" {
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

fn is_hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F'))
}

/// Mark phase: reachable digests from HEAD (R-a). Reports undecodable
/// reachable objects instead of treating them as garbage (R-10).
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
        if !store.object_exists_digest(&d) {
            continue;
        }
        seen.insert(d.clone());
        let Ok(bytes) = store.read_raw_digest(&d) else {
            integrity.push(format!(
                "integrity #object_undecodable: reachable digest {d} unreadable"
            ));
            continue;
        };
        let Ok(json) = serde_json::from_slice::<JsonValue>(&bytes) else {
            integrity.push(format!(
                "integrity #object_undecodable: reachable digest {d} cannot be decoded"
            ));
            continue;
        };
        let mut refs = Vec::new();
        refs_of(&json, follow_abandoned, &mut refs);
        for r in refs {
            if !seen.contains(&r) {
                stack.push_back(r);
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

    Ok(GcReport {
        total_objects,
        reachable: live.len(),
        collectable,
        collectable_bytes,
        abandoned_content,
        removed: 0,
        freed_bytes: 0,
        integrity,
    })
}

/// Sweep unreachable objects under `.oo/objects/` only.
pub fn run_gc(store: &ObjectStore, base_dir: &Path, dry_run: bool) -> Result<GcReport, String> {
    let mut report = plan_gc(store, base_dir)?;
    if dry_run || report.collectable == 0 {
        return Ok(report);
    }

    let all = store
        .list_digests()
        .map_err(|e| format!("list objects: {e}"))?;
    let (live, _) = mark(store, base_dir, false);

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
