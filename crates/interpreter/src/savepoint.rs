//! Local savepoint (○) store.
//!
//! Identity is a locally minted sequential id, never a CAID (`commit.md`
//! §1.5.3). The body is the same combo `staged` carries. Order is the
//! `LOG` line order. The directory is not CAS — putting ○ in `objects/`
//! would move the object-count of an all-solid universe (G5 / work order
//! §7: `x: 0` has 3 objects).
//!
//! These files survive commit: D43 requires every ○ to already be durable.

use crate::store_codec::{decode_staged, encode_savepoint};
use crate::value::ComboVal;
use anyhow::Result;
use std::fs;
use std::path::Path;

pub const DIR: &str = "savepoints";
pub const LOG: &str = "LOG";

fn dir(base: &Path) -> std::path::PathBuf {
    base.join(".oo").join(DIR)
}

fn log_path(base: &Path) -> std::path::PathBuf {
    dir(base).join(LOG)
}

fn parse_ids(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}

/// Next local id: 16 hex digits of a monotonic counter. Not a content hash.
fn mint_id(prev: &[String]) -> String {
    let n = prev.len() as u64 + 1;
    format!("{n:016x}")
}

pub fn recorded_ids(base: &Path) -> Result<Vec<String>> {
    let p = log_path(base);
    if !p.exists() {
        return Ok(Vec::new());
    }
    Ok(parse_ids(&fs::read_to_string(p)?))
}

/// Append a savepoint of `combo` unless it is byte-identical to the last one
/// (D47 injection clause: no ○ when the lattice position did not move).
pub fn record(base: &Path, combo: &ComboVal) -> Result<Option<String>> {
    let body = encode_savepoint(combo);
    let ids = recorded_ids(base)?;
    if let Some(last) = ids.last() {
        let last_path = dir(base).join(last);
        if last_path.exists() {
            let prev = fs::read_to_string(&last_path)?;
            if prev == body {
                return Ok(None);
            }
        }
    }
    let id = mint_id(&ids);
    let d = dir(base);
    fs::create_dir_all(&d)?;
    crate::storage::atomic_write(&d.join(&id), body)?;
    let mut log = String::new();
    for existing in &ids {
        log.push_str(existing);
        log.push('\n');
    }
    log.push_str(&id);
    log.push('\n');
    crate::storage::atomic_write(&log_path(base), log)?;
    Ok(Some(id))
}

#[allow(dead_code)]
pub fn load(base: &Path, id: &str) -> Result<ComboVal> {
    let text = fs::read_to_string(dir(base).join(id))?;
    decode_staged(&text)
}
