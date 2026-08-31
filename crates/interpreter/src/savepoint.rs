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
    decode_staged, encode_savepoint, parse_savepoint_ancestor, parse_savepoint_commit,
    parse_savepoint_parents, savepoint_combo_text,
};
use crate::value::{ComboVal, ContentHash};
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet, HashSet};
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
    /// Predecessor commit: 64-hex digest (A3), or a Repair-2 circle
    /// local id. Annotation, not a covering edge.
    pub ancestor: Option<String>,
}

fn is_legacy_counter_id(id: &str) -> bool {
    id.len() == 16 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Load every circle. Files with no `parents:` line are v0.38/v0.39
/// counter-named bodies: reconstruct a chain along sorted 16-hex ids so
/// the first new ○ does not claim an N-way merge that never happened.
pub fn load_circles(base: &Path) -> Result<BTreeMap<String, Circle>> {
    let files = paths(base)?;
    let mut parsed: BTreeMap<
        String,
        (Option<Vec<String>>, String, Option<String>, Option<String>),
    > = BTreeMap::new();
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
        let ancestor = parse_savepoint_ancestor(&text);
        parsed.insert(id, (parents, combo, commit, ancestor));
    }

    let legacy: Vec<String> = parsed
        .iter()
        .filter(|(id, (p, _, _, _))| p.is_none() && is_legacy_counter_id(id))
        .map(|(id, _)| id.clone())
        .collect();

    let mut nodes = BTreeMap::new();
    for (id, (parents, combo, commit_digest, ancestor)) in parsed {
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
                ancestor,
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
    let candidate_combo = encode_savepoint(combo, &[] as &[String], None, None);
    let candidate_combo = savepoint_combo_text(&candidate_combo).to_string();
    if tips.len() == 1 {
        if let Some(t) = nodes.get(&tips[0]) {
            if t.combo == candidate_combo {
                return Ok(None);
            }
        }
    }
    let body = encode_savepoint(combo, &tips, None, None);
    Ok(Some(write_circle(base, &body)?))
}

/// Mint the commit event's own circle (D51/D52/D55). Always mints. Combo
/// is empty — the working set already lives on the covering parent; an
/// empty combo cannot equal a workset snapshot, so D51 will not skip a
/// later evolve.
///
/// Covering (`parents:`): `covering` if given; otherwise the unique (or
/// lexicographically first) tip. No tip means a root circle with empty
/// `parents:` (unit-test refine / first commit without a disk evolve).
/// G3 forbids *two* covering parents, not zero.
///
/// Ancestor (annotation, not an H1 edge): the predecessor commit's
/// 64-hex digest. Omitted on the first commit. Pre-arc HEADs have no
/// circle, so naming a circle id would dangle and `gc` would sweep
/// history that was still on disk.
pub fn record_commit(
    base: &Path,
    commit: &ContentHash,
    covering: Option<&str>,
    ancestor: Option<&str>,
) -> Result<Option<String>> {
    let nodes = load_circles(base)?;
    if !nodes.is_empty() && tips_of(&nodes).is_empty() {
        anyhow::bail!("savepoint cycle: ids nonempty and tips empty");
    }
    let parents: Vec<String> = if let Some(p) = covering {
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
    let body = encode_savepoint(&ComboVal::default(), &parents, Some(&digest), ancestor);
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

/// Previous commit: `Commit.parent` if still set, else the D55 ancestor
/// annotation on this commit's circle. Does not walk `parents:` — that is
/// the time covering, and rollback does not leave a mark on it.
pub fn previous_commit(
    base: &Path,
    commit: &crate::value::Commit,
    digest: &str,
) -> Result<Option<ContentHash>> {
    if let Some(p) = &commit.parent {
        return Ok(Some(p.clone()));
    }
    let nodes = load_circles(base)?;
    let Some(start) = nodes
        .iter()
        .find(|(_, n)| n.commit_digest.as_deref() == Some(digest))
        .map(|(id, _)| id.clone())
    else {
        return Ok(None);
    };
    let Some(aid) = nodes.get(&start).and_then(|n| n.ancestor.clone()) else {
        return Ok(None);
    };
    Ok(hash_from_ancestor(&nodes, &aid, digest))
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn hash_from_hex(d: &str) -> Option<ContentHash> {
    if !is_hex64(d) {
        return None;
    }
    let bytes = hex::decode(d).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    Some(ContentHash::v1(bytes))
}

/// A3 spelling: `ancestor:` is the predecessor commit's 64-hex digest.
/// Repair-2 spelling: a circle local id; look up that circle's `commit:`.
fn hash_from_ancestor(
    nodes: &BTreeMap<String, Circle>,
    aid: &str,
    current: &str,
) -> Option<ContentHash> {
    if let Some(h) = hash_from_hex(aid) {
        if hex::encode(&h.digest) == current {
            return None;
        }
        return Some(h);
    }
    let d = nodes.get(aid).and_then(|n| n.commit_digest.as_deref())?;
    if d == current {
        return None;
    }
    hash_from_hex(d)
}

/// Whether `base` is reachable from `head` as a commit ancestor (D55).
/// Dual walk: `parent` if set, else the ancestor annotation.
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
