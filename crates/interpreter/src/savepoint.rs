//! Local savepoint (○) store.
//!
//! Identity is a locally minted random id, never a CAID (`commit.md`
//! §1.5.3) and never `ids.len()+1`. The covering relation is `parents:`
//! on the frame (D50). A commit is an annotation on a circle (D52/D54),
//! not a node and not a CAID. The directory is not CAS — putting ○ in
//! `objects/` would move the object-count of an all-solid universe
//! (`x: 0` has 3 objects).
//!
//! These files survive commit: D43 requires every ○ to already be durable.

use crate::store_codec::{
    decode_staged, encode_savepoint, parse_savepoint_commit, parse_savepoint_parents,
    savepoint_combo_text,
};
use crate::value::{ComboVal, ContentHash};
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

pub const DIR: &str = "savepoints";

fn dir(base: &Path) -> PathBuf {
    base.join(".oo").join(DIR)
}

fn paths(base: &Path) -> Result<Vec<PathBuf>> {
    let d = dir(base);
    if !d.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for e in fs::read_dir(&d)? {
        let p = e?.path();
        if !p.is_file() {
            continue;
        }
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name == "LOG" || name.starts_with('.') {
            continue;
        }
        out.push(p);
    }
    out.sort();
    Ok(out)
}

pub struct Circle {
    pub parents: Vec<String>,
    pub combo: String,
    /// 64-hex digest of the commit this circle became, if any (D52).
    pub commit_digest: Option<String>,
}

fn is_legacy_counter_id(id: &str) -> bool {
    id.len() == 16 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Load every circle. Files with no `parents:` line are v0.38/v0.39
/// counter-named bodies: reconstruct a chain along sorted 16-hex ids so
/// the first new ○ does not claim an N-way merge that never happened.
pub fn load_circles(base: &Path) -> Result<BTreeMap<String, Circle>> {
    let files = paths(base)?;
    let mut parsed: BTreeMap<String, (Option<Vec<String>>, String, Option<String>)> =
        BTreeMap::new();
    for p in &files {
        let id = p
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("savepoint: unreadable name"))?
            .to_string();
        let text = fs::read_to_string(p)?;
        let parents = parse_savepoint_parents(&text);
        let combo = savepoint_combo_text(&text).to_string();
        let commit = parse_savepoint_commit(&text);
        parsed.insert(id, (parents, combo, commit));
    }

    let legacy: Vec<String> = parsed
        .iter()
        .filter(|(id, (p, _, _))| p.is_none() && is_legacy_counter_id(id))
        .map(|(id, _)| id.clone())
        .collect();

    let mut nodes = BTreeMap::new();
    for (id, (parents, combo, commit_digest)) in parsed {
        let parents = match parents {
            Some(p) => p,
            None if is_legacy_counter_id(&id) => {
                let i = legacy.iter().position(|x| x == &id).unwrap();
                if i == 0 {
                    Vec::new()
                } else {
                    vec![legacy[i - 1].clone()]
                }
            }
            None => Vec::new(),
        };
        nodes.insert(
            id,
            Circle {
                parents,
                combo,
                commit_digest,
            },
        );
    }
    Ok(nodes)
}

fn tips_of(nodes: &BTreeMap<String, Circle>) -> Vec<String> {
    let mentioned: BTreeSet<&str> = nodes
        .values()
        .flat_map(|n| n.parents.iter().map(|s| s.as_str()))
        .collect();
    nodes
        .keys()
        .filter(|id| !mentioned.contains(id.as_str()))
        .cloned()
        .collect()
}

fn write_circle(base: &Path, body: &str) -> Result<String> {
    let d = dir(base);
    fs::create_dir_all(&d)?;
    let leftover = d.join("LOG");
    if leftover.exists() {
        let _ = fs::remove_file(&leftover);
    }
    for _ in 0..8 {
        let id = crate::injections::mint_id()?;
        let dest = d.join(&id);
        if dest.exists() {
            continue;
        }
        crate::storage::atomic_write(&dest, body.as_bytes())?;
        return Ok(id);
    }
    anyhow::bail!("savepoint id: exhausted unique names")
}

/// Append a savepoint of `combo` unless D51 says the covering relation
/// did not change: skip iff `parents` is exactly one tip T and the
/// candidate combo equals T's combo.
pub fn record(base: &Path, combo: &ComboVal) -> Result<Option<String>> {
    let nodes = load_circles(base)?;
    let mut tips = tips_of(&nodes);
    if !nodes.is_empty() && tips.is_empty() {
        anyhow::bail!("savepoint cycle: ids nonempty and tips empty");
    }
    tips.sort();
    let candidate_combo = encode_savepoint(combo, &[] as &[String], None);
    let candidate_combo = savepoint_combo_text(&candidate_combo).to_string();
    if tips.len() == 1 {
        if let Some(t) = nodes.get(&tips[0]) {
            if t.combo == candidate_combo {
                return Ok(None);
            }
        }
    }
    let body = encode_savepoint(combo, &tips, None);
    Ok(Some(write_circle(base, &body)?))
}

/// Mint the commit event's own circle (D51/D52). Always mints. Combo is
/// empty — the working set already lives on the parent; an empty combo
/// cannot equal a workset snapshot, so D51 will not skip a later evolve.
///
/// Parent: `parent` if given; otherwise the unique (or lexicographically
/// first) tip. No tip means this is the first history event on an empty
/// directory (unit-test refine / first commit without a disk evolve) —
/// mint a root circle with empty `parents:`. G3 forbids *two* parents,
/// not zero.
pub fn record_commit(
    base: &Path,
    commit: &ContentHash,
    parent: Option<&str>,
) -> Result<Option<String>> {
    let nodes = load_circles(base)?;
    if !nodes.is_empty() && tips_of(&nodes).is_empty() {
        anyhow::bail!("savepoint cycle: ids nonempty and tips empty");
    }
    let parents: Vec<String> = if let Some(p) = parent {
        vec![p.to_string()]
    } else {
        let mut tips = tips_of(&nodes);
        tips.sort();
        match tips.len() {
            0 => Vec::new(),
            _ => vec![tips.into_iter().next().unwrap()],
        }
    };
    let digest = hex::encode(&commit.digest);
    let body = encode_savepoint(&ComboVal::default(), &parents, Some(&digest));
    Ok(Some(write_circle(base, &body)?))
}

/// Directory is truth. Built per call; not a durable cache.
pub fn circle_id_for_commit(base: &Path, digest: &str) -> Result<Option<String>> {
    let nodes = load_circles(base)?;
    Ok(nodes
        .into_iter()
        .find(|(_, n)| n.commit_digest.as_deref() == Some(digest))
        .map(|(id, _)| id))
}

/// Previous commit along D52's edge, or `Commit.parent` when still set.
/// Cycle-safe: visited set on circle ids. Returns `None` when this commit
/// has no predecessor (first commit, or HEAD with no commit circle).
///
/// A digest listed in this commit's `abandoned` meta is a record, not a
/// chain member (history_ops R1 / R-b). Linear sessions have no such list,
/// so G5 is unaffected; after a rollback resume the covering may still
/// pass through the abandoned tip, and skipping it is what keeps the
/// record from re-entering `oo log`.
pub fn previous_commit(
    base: &Path,
    commit: &crate::value::Commit,
    digest: &str,
) -> Result<Option<ContentHash>> {
    if let Some(p) = &commit.parent {
        return Ok(Some(p.clone()));
    }
    let abandoned: HashSet<String> = commit
        .meta
        .abandoned
        .iter()
        .flatten()
        .filter_map(|s| {
            let hex = s.rsplit(':').next()?;
            (hex.len() == 64).then(|| hex.to_lowercase())
        })
        .collect();
    let nodes = load_circles(base)?;
    let Some(start) = nodes
        .iter()
        .find(|(_, n)| n.commit_digest.as_deref() == Some(digest))
        .map(|(id, _)| id.clone())
    else {
        return Ok(None);
    };
    let mut seen: HashSet<String> = HashSet::new();
    seen.insert(start.clone());
    let mut q: VecDeque<String> = nodes
        .get(&start)
        .map(|n| n.parents.clone().into())
        .unwrap_or_default();
    while let Some(pid) = q.pop_front() {
        if !seen.insert(pid.clone()) {
            continue;
        }
        let Some(n) = nodes.get(&pid) else {
            continue;
        };
        if let Some(d) = &n.commit_digest {
            if d != digest && !abandoned.contains(d) {
                if let Ok(bytes) = hex::decode(d) {
                    if bytes.len() == 32 {
                        return Ok(Some(ContentHash::v1(bytes)));
                    }
                }
            }
        }
        q.extend(n.parents.iter().cloned());
    }
    Ok(None)
}

/// Whether `base` is reachable from `head` as a commit ancestor (D50 DAG).
/// Dual walk: `parent` if set, else the commit circle's covering predecessors.
/// Cycles are cut by a visited set on commit digests.
pub fn commit_is_ancestor(
    base_dir: &Path,
    store: &crate::storage::ObjectStore,
    head: &ContentHash,
    base: &ContentHash,
) -> Result<bool> {
    if head == base {
        return Ok(false);
    }
    let mut seen: HashSet<String> = HashSet::new();
    let mut stack = vec![head.clone()];
    while let Some(h) = stack.pop() {
        let d = hex::encode(&h.digest);
        if !seen.insert(d.clone()) {
            continue;
        }
        if &h == base {
            return Ok(true);
        }
        let commit = store.get_commit(&h)?;
        if let Some(p) = previous_commit(base_dir, &commit, &d)? {
            stack.push(p);
        }
    }
    Ok(false)
}

#[allow(dead_code)]
pub fn load(base: &Path, id: &str) -> Result<ComboVal> {
    let text = fs::read_to_string(dir(base).join(id))?;
    decode_staged(&text)
}
