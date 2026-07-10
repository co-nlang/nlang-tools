use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn, Peer};
use crate::value::{Value, EffectTag, BottomCause, BottomDetail, ContentHash};
use crate::storage::ObjectStore;
use nlang_parser::ast::AtomKind;

const MAX_ROUTING_HOPS: u32 = 16;

fn base64_decode_sketch(s: &str) -> Vec<u8> {
    use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
    STANDARD_NO_PAD.decode(s).unwrap_or_default()
}

fn bottom_not_found() -> Value {
    Value::Bottom(Box::new(BottomDetail {
        cause: BottomCause::MissingKey, path: Some("disc.find".to_string()),
        message: Some("No matching peers found".to_string()),
        ..Default::default()
    }))
}

/// Perturb gravitational weight with a deterministic session salt.
/// Adds ±0.5% noise: enough to break ties, not enough to override strong gravity.
fn perturb_weight(weight: f64, caid: &str, horizon_salt: &crate::value::ContentHash) -> f64 {
    use sha2::{Sha256, Digest as Sha2Digest};
    let mut h = Sha256::new();
    h.update(&horizon_salt.digest);
    h.update(caid.as_bytes());
    let hash = h.finalize();
    let salt_f = u64::from_be_bytes(hash[0..8].try_into().unwrap()) as f64 / u64::MAX as f64;
    weight * (1.0 + (salt_f - 0.5) * 0.01)
}

/// Compute a MASA identifier from a Combo's field key set.
/// Two Combos with the same field keys form the same MASA (classical sub-algebra).
fn field_key_masa_id(cv: &crate::value::ComboVal) -> String {
    use sha2::{Sha256, Digest};
    let mut keys: Vec<String> = cv.all_fields_iter()
        .map(|(k, _)| k)
        .filter(|k| !k.starts_with('%') && !k.starts_with("~%"))
        .collect();
    keys.sort();
    let joined = keys.join("\x00");
    let digest = Sha256::digest(joined.as_bytes());
    format!("masa:fk:{}", hex::encode(&digest[..8]))
}

/// Field count as GBB mass (capped at 100).
fn compute_mass(val: &Value) -> f64 {
    if let Value::Combo(ref cv) = val {
        (cv.system.len() + cv.meta.len() + cv.types.len()
         + cv.rules.len() + cv.data.len() + cv.local.len()) as f64
    } else { 1.0 }.min(100.0)
}

/// Build the initial query nerve for disc.find (no overlapping MASA lookup).
fn build_query_nerve(val: &Value) -> Vec<crate::ladd::NerveEntry> {
    if let Value::Combo(ref cv) = val {
        let keys: Vec<String> = cv.all_fields_iter()
            .map(|(k, _)| k)
            .filter(|k| !k.starts_with('%') && !k.starts_with("~%"))
            .collect();
        if keys.is_empty() { vec![] }
        else {
            vec![crate::ladd::NerveEntry {
                masa_caid: field_key_masa_id(cv),
                overlapping_masa_caids: vec![],
                field_keys: keys,
            }]
        }
    } else { vec![] }
}

pub fn register_disc_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert("disc.connect".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg { 
            if let (Some(vname), Some(vpath)) = (c.get_field("0"), c.get_field("1")) {
                let name = oo.force(vname.clone(), ctx).to_string_plain();
                let path_str = oo.force(vpath.clone(), ctx).to_string_plain();
                if path_str.starts_with("tcp://") {
                    if let Ok(mut peers) = oo.peers.write() {
                        peers.insert(name, Peer::Remote(path_str[6..].to_string()));
                        return Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::IO, None);
                    }
                } else {
                    let path = std::path::PathBuf::from(path_str);
                    if let Ok(store) = ObjectStore::init(&path) {
                        if let Ok(mut peers) = oo.peers.write() {
                            peers.insert(name, Peer::Local(Arc::new(store)));
                            return Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::IO, None);
                        }
                    }
                }
            }
        }
        Value::Atom(AtomKind::Tag("false".to_string()), EffectTag::IO, None)
    }) as Arc<BuiltinFn>);
    
    m.insert("disc.fetch".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let (node_name, caid_str) = if let Value::Combo(ref c) = arg {
            if let (Some(v0), Some(v1)) = (c.get_field("0"), c.get_field("1")) {
                (Some(oo.force(v0.clone(), ctx).to_string_plain()), oo.force(v1.clone(), ctx).to_string_plain())
            } else if let Some(v0) = c.get_field("0") {
                (None, oo.force(v0.clone(), ctx).to_string_plain())
            } else { return BottomCause::Conflict.into(); }
        } else { (None, arg.collapse().to_string_plain()) };

        if let Ok(hash) = ContentHash::parse(&caid_str) {
            if let Some(name) = node_name {
                let peer_opt = if let Ok(peers) = oo.peers.read() { peers.get(&name).cloned() } else { None };
                if let Some(peer) = peer_opt {
                    match peer {
                        Peer::Local(store) => { if let Ok(val) = store.get_value(&hash) { return val; } }
                        Peer::Remote(addr) => { if let Ok(val) = oo.remote_fetch(&addr, &hash) { return val; } }
                    }
                }
            } else {
                let mut results = Vec::new();
                if let Ok(val) = oo.store.get_value(&hash) { results.push(val); }
                
                let peers_copy = if let Ok(peers) = oo.peers.read() { peers.values().cloned().collect::<Vec<_>>() } else { vec![] };
                for peer in peers_copy {
                    match peer {
                        Peer::Local(store) => { if let Ok(val) = store.get_value(&hash) { results.push(val); } }
                        Peer::Remote(addr) => { if let Ok(val) = oo.remote_fetch(&addr, &hash) { return val; } }
                    }
                }
                
                if results.is_empty() { return BottomCause::Conflict.into(); }
                
                let mut final_val = results.remove(0);
                for v in results {
                    let merged = oo.unify_internal(final_val.clone(), v.clone(), ctx);
                    if let Value::Bottom(_) = merged {
                        if v.bits() > final_val.bits() { final_val = v; }
                    } else {
                        final_val = merged;
                    }
                }
                return final_val;
            }
        }
        BottomCause::Conflict.into()
    }) as Arc<BuiltinFn>);
    
    m.insert("disc.identify".to_string(), Arc::new(|arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| {
        Value::Atom(AtomKind::Str(arg.content_hash().to_string()), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);

    // Phase 4 / Phase 5: LADD advertise
    m.insert("disc.advertise".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, _ctx: &mut EvalContext| {
        let hash = arg.content_hash();
        let mass = compute_mass(&arg);
        let sketch_bytes = base64_decode_sketch(&hash.lattice_sketch);
        let masa_ref = hash.masa_ref.clone();
        // Phase 11: nerve_structure from field key MASA computation
        // Phase 17: also store field_keys for dynamic intersection + compute overlapping
        let nerve_structure: Vec<crate::ladd::NerveEntry> = if let Value::Combo(ref cv) = arg {
            let keys: Vec<String> = cv.all_fields_iter()
                .map(|(k, _)| k)
                .filter(|k| !k.starts_with('%') && !k.starts_with("~%"))
                .collect();
            if keys.is_empty() {
                vec![]
            } else {
                let my_masa = field_key_masa_id(cv);
                let my_key_set: std::collections::HashSet<&str> = keys.iter().map(|s| s.as_str()).collect();
                let overlapping: Vec<String> = if let Ok(reg) = oo.gbb_registry.read() {
                    reg.values()
                        .flat_map(|g| g.nerve_structure.iter())
                        .filter(|ne| ne.masa_caid != my_masa)
                        .filter(|ne| ne.field_keys.iter().any(|k| my_key_set.contains(k.as_str())))
                        .map(|ne| ne.masa_caid.clone())
                        .collect::<std::collections::HashSet<_>>()
                        .into_iter()
                        .collect()
                } else { vec![] };
                vec![crate::ladd::NerveEntry {
                    masa_caid: my_masa,
                    overlapping_masa_caids: overlapping,
                    field_keys: keys,
                }]
            }
        } else {
            vec![]
        };
        let gbb = crate::ladd::GBB { node_caid: hash.clone(), mass, sketch_bytes, masa_ref, nerve_structure };
        if let Ok(mut reg) = oo.gbb_registry.write() {
            reg.insert(hash.to_string(), gbb);
        }
        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::IO, None)
    }) as Arc<BuiltinFn>);

    // Phase 4 / Phase 5: LADD find
    m.insert("disc.find".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        // 1. Build initial query GBB
        let query_hash = arg.content_hash();
        let mut current_query = crate::ladd::GBB {
            node_caid: query_hash.clone(),
            mass: compute_mass(&arg),
            sketch_bytes: base64_decode_sketch(&query_hash.lattice_sketch),
            masa_ref: query_hash.masa_ref.clone(),
            nerve_structure: build_query_nerve(&arg),
        };

        // 2. Extract explicit target CAID (optional direct-lookup mode)
        let explicit_target: Option<String> = if let Value::Combo(ref c) = arg {
            c.get_field("target").map(|v| oo.force(v.clone(), ctx).to_string_plain())
        } else { None };

        const EPSILON: f64 = 1e-6;

        // 3. Multi-hop routing loop
        loop {
            // Safety: hard hop budget (Phase 41)
            if ctx.disc_routing_hops >= MAX_ROUTING_HOPS {
                return Value::Bottom(Box::new(BottomDetail {
                    cause: BottomCause::SemanticEclipse,
                    path: Some("disc.find".to_string()),
                    message: Some(format!(
                        "Routing budget exceeded after {} hops", MAX_ROUTING_HOPS
                    )),
                    ..Default::default()
                }));
            }

            // Gravitational candidate scoring
            let candidates: Vec<(f64, String)> = {
                let reg = match oo.gbb_registry.read() {
                    Ok(r) => r, Err(_) => return BottomCause::Conflict.into(),
                };
                reg.values()
                    .filter(|g| crate::ladd::masa_compatible(&current_query, g))
                    .filter(|g| crate::ladd::nerve_overlap(&current_query, g))
                    .map(|g| {
                        let w = crate::ladd::gravitational_weight(&current_query, g, EPSILON);
                        (w, g.node_caid.to_string())
                    })
                    .collect()
            };

            if candidates.is_empty() { return bottom_not_found(); }

            // Blacklist + horizon_salt tiebreaker (Phase 41)
            let mut perturbed: Vec<(f64, String)> = candidates.iter()
                .map(|(w, caid)| (perturb_weight(*w, caid, &ctx.horizon_salt), caid.clone()))
                .collect();
            perturbed.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

            let chosen = if let Some((_, caid)) = perturbed.iter()
                .find(|(_, c)| !ctx.disc_routing_visited.contains(c))
            {
                caid.clone()
            } else {
                perturbed[0].1.clone()
            };

            ctx.disc_routing_visited.insert(chosen.clone());
            ctx.disc_routing_hops += 1;

            // Determine which CAID to fetch at this hop
            let fetch_target = explicit_target.as_deref().unwrap_or(chosen.as_str());

            // Try local store, then connected peers
            if let Ok(hash) = crate::value::ContentHash::parse(fetch_target) {
                if let Ok(val) = oo.store.get_value(&hash) { return val; }
                let peers_copy: Vec<_> = oo.peers.read()
                    .map(|p| p.values().cloned().collect()).unwrap_or_default();
                for peer in peers_copy {
                    match peer {
                        crate::Peer::Local(store) => {
                            if let Ok(val) = store.get_value(&hash) { return val; }
                        }
                        crate::Peer::Remote(addr) => {
                            if let Ok(val) = oo.remote_fetch(&addr, &hash) { return val; }
                        }
                    }
                }
            }

            // Value not found at this hop — advance query to chosen GBB for next hop
            let next_gbb = {
                let reg = match oo.gbb_registry.read() {
                    Ok(r) => r, Err(_) => return BottomCause::Conflict.into(),
                };
                reg.get(&chosen).cloned()
            };

            match next_gbb {
                Some(gbb) => { current_query = gbb; }
                None => { return bottom_not_found(); }
            }
        }
    }) as Arc<BuiltinFn>);
}