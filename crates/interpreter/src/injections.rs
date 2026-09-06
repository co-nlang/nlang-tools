//! Immutable working-set injections (D48 / Q-014).
//!
//! `SPEC_10` §3 clause 2: staged is the **set** of definitions injected since
//! the last commit. Each successful evolve mints one durable file; the
//! working set is the fold of those files. Local ids are random — never
//! `ids.len()+1` (`savepoint.rs::mint_id`'s disease).

use crate::store_codec::{decode_staged, encode_injection, FRAME};
use crate::value::{BottomCause, BottomDetail, ComboVal, EffectTag, Value};
use crate::Ouroboros;
use anyhow::Result;
use ring::rand::{SecureRandom, SystemRandom};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const DIR: &str = "injections";

/// One immutable member of the working-set set. `id` is carried inside the
/// member: the filename is only a local storage key and cannot be allowed to
/// give a set an accidental order (D58).
#[derive(Clone, Debug)]
pub struct Injection {
    pub id: String,
    pub combo: ComboVal,
    pub pin_coords: BTreeSet<String>,
    /// Per coordinate, the members this pin observed and replaced at evolve
    /// time. Two concurrent pins name the same past but not each other, so both
    /// remain and the ordinary meet reports their conflict (D49).
    pub absorbs: BTreeMap<String, BTreeSet<String>>,
    /// Active tags a `runPure` in this member actually discharged. Layout 5
    /// persists the set on the member; layout ≤ 4 used `.oo/effect_pending`.
    /// Commit takes the union over members (D58 / Q-040).
    pub effect_tags: EffectTag,
}

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

pub fn load_all(base: &Path) -> Result<Vec<Injection>> {
    let mut out = Vec::new();
    for p in paths(base)? {
        let text = fs::read_to_string(&p)?;
        let filename_id = p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        out.push(
            if let Some(rest) = text
                .trim_start()
                .strip_prefix(&format!("{FRAME} injection\n"))
                .and_then(|s| s.strip_prefix("id: "))
            {
                let (meta, body) = rest.split_once("\n\n").ok_or_else(|| {
                    anyhow::anyhow!("injection {filename_id}: incomplete metadata frame")
                })?;
                let mut lines = meta.lines();
                let id: String = serde_json::from_str(lines.next().unwrap_or_default())?;
                let pin_coords: BTreeSet<String> = serde_json::from_str(
                    lines
                        .next()
                        .and_then(|l| l.strip_prefix("pin_coords: "))
                        .ok_or_else(|| {
                            anyhow::anyhow!("injection {filename_id}: pin_coords absent")
                        })?,
                )?;
                let absorbs: BTreeMap<String, BTreeSet<String>> = serde_json::from_str(
                    lines
                        .next()
                        .and_then(|l| l.strip_prefix("absorbs: "))
                        .ok_or_else(|| {
                            anyhow::anyhow!("injection {filename_id}: absorbs absent")
                        })?,
                )?;
                // Layout 4 ends after absorbs. Layout 5 adds `effect_tags:`.
                // Any other leftover line is still unknown metadata — that
                // strictness is what makes "the declaration is still true"
                // enforceable (Q-040 Q1). An unreadable tags value is a
                // refusal, never "no discharge" (S2).
                let effect_tags = match lines.next() {
                    None => EffectTag::Pure,
                    Some(line) => {
                        let rest = line.strip_prefix("effect_tags: ").ok_or_else(|| {
                            anyhow::anyhow!("injection {filename_id}: unknown metadata")
                        })?;
                        let bits: u8 = serde_json::from_str(rest).map_err(|_| {
                            anyhow::anyhow!("injection {filename_id}: effect_tags unreadable")
                        })?;
                        if lines.next().is_some() {
                            anyhow::bail!("injection {filename_id}: unknown metadata");
                        }
                        // B1: bits this engine cannot fully represent must
                        // refuse, not shrink to the empty set. `from_bits`
                        // still masks — that is correct where the question
                        // is "may this grant something". Here the question
                        // is "was something discharged".
                        let representable = EffectTag::IO.to_bits()
                            | EffectTag::NonDet.to_bits()
                            | EffectTag::State.to_bits();
                        if bits & !representable != 0 {
                            anyhow::bail!(
                                "injection {filename_id}: this engine cannot fully \
                                 read the persisted tag set"
                            );
                        }
                        EffectTag::from_bits(bits)
                    }
                };
                let framed_body = format!("{FRAME} injection\n{body}");
                Injection {
                    id,
                    combo: decode_staged(&framed_body)?,
                    pin_coords,
                    absorbs,
                    effect_tags,
                }
            } else {
                // Layout <= 3 had no in-body id and pin intent lived in
                // `.oo/pin_pending`. Derive a stable compatibility id from the
                // immutable bytes: using the filename would reintroduce the
                // exact rename/order dependency D58 removes.
                let combo = if crate::store_codec::is_framed(&text) {
                    decode_staged(&text)?
                } else {
                    serde_json::from_str(&text)?
                };
                Injection {
                    id: format!(
                        "legacy-sha256:{}",
                        hex::encode(
                            ring::digest::digest(&ring::digest::SHA256, text.as_bytes()).as_ref()
                        )
                    ),
                    combo,
                    pin_coords: BTreeSet::new(),
                    absorbs: BTreeMap::new(),
                    effect_tags: EffectTag::Pure,
                }
            },
        );
    }
    Ok(out)
}

/// Engine-internal unify loop. Must not be an n/ `&` chain (recon Q16:
/// N=100 is already `#fuel_exhausted`).
pub fn fold(
    engine: &Ouroboros,
    injections: impl IntoIterator<Item = Injection>,
) -> std::result::Result<ComboVal, BottomDetail> {
    let injections: Vec<Injection> = injections.into_iter().collect();
    let mut absorbed: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for injection in &injections {
        for (coord, ids) in &injection.absorbs {
            absorbed
                .entry(coord.clone())
                .or_default()
                .extend(ids.iter().cloned());
        }
    }
    let mut acc = ComboVal::default();
    for injection in injections {
        let mut c = injection.combo;
        for (coord, ids) in &absorbed {
            if ids.contains(&injection.id) {
                c.remove_field(coord);
                c.local.shift_remove(coord.as_str());
            }
        }
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

pub fn write(
    base: &Path,
    combo: &ComboVal,
    pin_coords: &BTreeSet<String>,
    absorbs: &BTreeMap<String, BTreeSet<String>>,
    effect_tags: EffectTag,
) -> Result<String> {
    let declaration = crate::storage::read_layout_declaration(base)?;
    let current = crate::storage::layout_declaration_is_current(&declaration);
    let pin_frame = crate::storage::layout_writes_pin_frame(&declaration);
    // D60 / REAL_02 §5.1.1: a past layout must not receive a field it cannot
    // declare. Refuse before minting a member. Layout 4 already declares the
    // pin frame; it does not declare `effect_tags`.
    if !effect_tags.is_pure() && !current {
        anyhow::bail!(
            "this store declares {declaration}; a discharged injection cannot \
             land until the layout is current. Run `oo migrate --grant migrate`"
        );
    }
    let d = dir(base);
    fs::create_dir_all(&d)?;
    for _ in 0..8 {
        let id = mint_id()?;
        let dest = d.join(&id);
        if dest.exists() {
            continue;
        }
        let legacy_body = encode_injection(combo);
        let body = if current {
            let combo_body = legacy_body
                .strip_prefix(&format!("{FRAME} injection\n"))
                .expect("encode_injection frame");
            format!(
                "{FRAME} injection\nid: {}\npin_coords: {}\nabsorbs: {}\neffect_tags: {}\n\n{}",
                serde_json::to_string(&id)?,
                serde_json::to_string(pin_coords)?,
                serde_json::to_string(absorbs)?,
                serde_json::to_string(&effect_tags.to_bits())?,
                combo_body,
            )
        } else if pin_frame {
            // Layout 4 form: v0.43.0 can still parse this. Never add
            // `effect_tags:` — that line is unknown metadata to it.
            let combo_body = legacy_body
                .strip_prefix(&format!("{FRAME} injection\n"))
                .expect("encode_injection frame");
            format!(
                "{FRAME} injection\nid: {}\npin_coords: {}\nabsorbs: {}\n\n{}",
                serde_json::to_string(&id)?,
                serde_json::to_string(pin_coords)?,
                serde_json::to_string(absorbs)?,
                combo_body,
            )
        } else {
            // A past layout may still receive writes whose representation it
            // already declares. It must never receive the new metadata frame:
            // an old engine would parse that as a corrupt injection.
            legacy_body
        };
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
