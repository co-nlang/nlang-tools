//! Local savepoint (○) store.
//!
//! Identity is a locally minted random id, never a CAID (`commit.md`
//! §1.5.3) and never `ids.len()+1`. The covering relation is `parents:`
//! on the frame (D50), derived from the directory (a name is a tip iff
//! no parent list contains it). The directory is not CAS — putting ○ in
//! `objects/` would move the object-count of an all-solid universe
//! (`x: 0` has 3 objects).
//!
//! These files survive commit: D43 requires every ○ to already be durable.
//! There is no `LOG`: that was a second truth and a crash window.

use crate::store_codec::{
    decode_staged, encode_savepoint, parse_savepoint_parents, savepoint_combo_text,
};
use crate::value::ComboVal;
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};
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
        // LOG is a leftover of the previous shape; `.partial-*` is atomic_write.
        if name == "LOG" || name.starts_with('.') {
            continue;
        }
        out.push(p);
    }
    out.sort();
    Ok(out)
}

struct Node {
    parents: Vec<String>,
    combo: String,
}

fn is_legacy_counter_id(id: &str) -> bool {
    id.len() == 16 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Load every circle. Files with no `parents:` line are v0.38/v0.39
/// counter-named bodies: reconstruct a chain along sorted 16-hex ids so
/// the first new ○ does not claim an N-way merge that never happened.
fn load_nodes(base: &Path) -> Result<BTreeMap<String, Node>> {
    let files = paths(base)?;
    let mut parsed: BTreeMap<String, (Option<Vec<String>>, String)> = BTreeMap::new();
    for p in &files {
        let id = p
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("savepoint: unreadable name"))?
            .to_string();
        let text = fs::read_to_string(p)?;
        let parents = parse_savepoint_parents(&text);
        let combo = savepoint_combo_text(&text).to_string();
        parsed.insert(id, (parents, combo));
    }

    let legacy: Vec<String> = parsed
        .iter()
        .filter(|(id, (p, _))| p.is_none() && is_legacy_counter_id(id))
        .map(|(id, _)| id.clone())
        .collect();

    let mut nodes = BTreeMap::new();
    for (id, (parents, combo)) in parsed {
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
        nodes.insert(id, Node { parents, combo });
    }
    Ok(nodes)
}

/// X is a tip iff no circle's `parents` contains X.
fn tips_of(nodes: &BTreeMap<String, Node>) -> Vec<String> {
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

/// Append a savepoint of `combo` unless D51 says the covering relation
/// did not change: skip iff `parents` is exactly one tip T and the
/// candidate combo equals T's combo.
pub fn record(base: &Path, combo: &ComboVal) -> Result<Option<String>> {
    let nodes = load_nodes(base)?;
    let mut tips = tips_of(&nodes);
    if !nodes.is_empty() && tips.is_empty() {
        anyhow::bail!("savepoint cycle: ids nonempty and tips empty");
    }
    tips.sort();
    let candidate_combo = encode_savepoint(combo, &[] as &[String]);
    let candidate_combo = savepoint_combo_text(&candidate_combo).to_string();
    if tips.len() == 1 {
        if let Some(t) = nodes.get(&tips[0]) {
            if t.combo == candidate_combo {
                return Ok(None);
            }
        }
    }
    let d = dir(base);
    fs::create_dir_all(&d)?;
    // S6: LOG is no longer truth. Drop a leftover so it cannot disagree
    // with the directory after this write.
    let leftover = d.join("LOG");
    if leftover.exists() {
        let _ = fs::remove_file(&leftover);
    }
    let body = encode_savepoint(combo, &tips);
    for _ in 0..8 {
        let id = crate::injections::mint_id()?;
        let dest = d.join(&id);
        if dest.exists() {
            continue;
        }
        crate::storage::atomic_write(&dest, body.as_bytes())?;
        return Ok(Some(id));
    }
    anyhow::bail!("savepoint id: exhausted unique names")
}

#[allow(dead_code)]
pub fn load(base: &Path, id: &str) -> Result<ComboVal> {
    let text = fs::read_to_string(dir(base).join(id))?;
    decode_staged(&text)
}
