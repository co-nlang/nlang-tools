//! Immutable working-set injections (D48 / Q-014).
//!
//! `SPEC_10` §3 clause 2: staged is the **set** of definitions injected since
//! the last commit. Each successful evolve mints one durable file; the
//! working set is the fold of those files. Local ids are random — never
//! `ids.len()+1` (`savepoint.rs::mint_id`'s disease).

use crate::store_codec::{decode_staged, encode_injection};
use crate::value::{BottomCause, BottomDetail, ComboVal, Value};
use crate::Ouroboros;
use anyhow::Result;
use ring::rand::{SecureRandom, SystemRandom};
use std::fs;
use std::path::{Path, PathBuf};

pub const DIR: &str = "injections";

pub fn dir(base: &Path) -> PathBuf {
    base.join(".oo").join(DIR)
}

/// 16 bytes of OS entropy as 32 lowercase hex digits. Does not read the
/// directory, so two concurrent mints cannot collide by sharing a count.
pub fn mint_id() -> Result<String> {
    let rng = SystemRandom::new();
    let mut bytes = [0u8; 16];
    rng.fill(&mut bytes)
        .map_err(|_| anyhow::anyhow!("injection id: no entropy"))?;
    Ok(hex::encode(bytes))
}

pub fn paths(base: &Path) -> Result<Vec<PathBuf>> {
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
        // atomic_write temps live here as `.partial-*`; they are not injections.
        if name.starts_with('.') {
            continue;
        }
        out.push(p);
    }
    out.sort();
    Ok(out)
}

pub fn load_all(base: &Path) -> Result<Vec<ComboVal>> {
    let mut out = Vec::new();
    for p in paths(base)? {
        let text = fs::read_to_string(&p)?;
        out.push(if crate::store_codec::is_framed(&text) {
            decode_staged(&text)?
        } else {
            serde_json::from_str(&text)?
        });
    }
    Ok(out)
}

/// Engine-internal unify loop. Must not be an n/ `&` chain (recon Q16:
/// N=100 is already `#fuel_exhausted`).
pub fn fold(
    engine: &Ouroboros,
    combos: impl IntoIterator<Item = ComboVal>,
) -> std::result::Result<ComboVal, BottomDetail> {
    let mut acc = ComboVal::default();
    for c in combos {
        match engine.unify(Value::Combo(acc), Value::Combo(c)) {
            Value::Combo(m) => acc = m,
            Value::Bottom(d) => return Err(*d),
            _ => {
                return Err(BottomDetail {
                    cause: BottomCause::Conflict,
                    ..Default::default()
                });
            }
        }
    }
    Ok(acc)
}

pub fn write(base: &Path, combo: &ComboVal) -> Result<String> {
    let d = dir(base);
    fs::create_dir_all(&d)?;
    let body = encode_injection(combo);
    for _ in 0..8 {
        let id = mint_id()?;
        let dest = d.join(&id);
        if dest.exists() {
            continue;
        }
        crate::storage::atomic_write(&dest, body.as_bytes())?;
        return Ok(id);
    }
    anyhow::bail!("injection id: exhausted unique names")
}

pub fn clear(base: &Path) -> Result<()> {
    let d = dir(base);
    if !d.exists() {
        return Ok(());
    }
    for p in paths(base)? {
        let _ = fs::remove_file(p);
    }
    let _ = fs::remove_dir(&d);
    Ok(())
}
