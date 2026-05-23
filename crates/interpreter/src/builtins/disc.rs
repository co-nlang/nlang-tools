use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn, Peer};
use crate::value::{Value, EffectTag, BottomCause, BottomDetail, ContentHash};
use crate::storage::ObjectStore;
use nlang_parser::ast::AtomKind;

fn base64_decode_sketch(s: &str) -> Vec<u8> {
    use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
    STANDARD_NO_PAD.decode(s).unwrap_or_default()
}

fn bottom_not_found() -> Value {
    Value::Bottom(Box::new(BottomDetail {
        cause: BottomCause::Conflict,
        message: Some("#not_found: no compatible peer".to_string()),
        ..Default::default()
    }))
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
        // Phase 5 mass: field count (closer to Tr(P) semantics)
        let mass = if let Value::Combo(ref cv) = arg {
            (cv.system.len() + cv.meta.len() + cv.types.len()
             + cv.rules.len() + cv.data.len() + cv.local.len()) as f64
        } else { 1.0 };
        // Phase 5 mass capped to avoid runaway weights
        let mass = mass.min(100.0);
        let sketch_bytes = base64_decode_sketch(&hash.lattice_sketch);
        let masa_ref = hash.masa_ref.clone();
        // Phase 5 nerve_structure from refine_map (approximation)
        let nerve_structure: Vec<crate::ladd::NerveEntry> = {
            oo.refine_map.read().map_or_else(|_| vec![], |m| {
                m.iter().map(|(src, targets)| crate::ladd::NerveEntry {
                    masa_caid: src.clone(),
                    overlapping_masa_caids: targets.clone(),
                }).collect()
            })
        };
        let gbb = crate::ladd::GBB { node_caid: hash.clone(), mass, sketch_bytes, masa_ref, nerve_structure };
        if let Ok(mut reg) = oo.gbb_registry.write() {
            reg.insert(hash.to_string(), gbb);
        }
        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::IO, None)
    }) as Arc<BuiltinFn>);

    // Phase 4 / Phase 5: LADD find
    m.insert("disc.find".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        // 1. Build query GBB
        let query_hash = arg.content_hash();
        let query_mass = if let Value::Combo(ref cv) = arg {
            (cv.system.len() + cv.meta.len() + cv.types.len()
             + cv.rules.len() + cv.data.len() + cv.local.len()) as f64
        } else { 1.0 };
        let query_sketch = base64_decode_sketch(&query_hash.lattice_sketch);
        let query_gbb = crate::ladd::GBB {
            node_caid: query_hash.clone(), mass: query_mass.min(100.0),
            sketch_bytes: query_sketch, masa_ref: query_hash.masa_ref.clone(),
            nerve_structure: vec![],
        };

        // 2. MASA filter + gravitational weighting
        const EPSILON: f64 = 1e-6;
        let mut candidates: Vec<(f64, String)> = {
            let reg = match oo.gbb_registry.read() { Ok(r) => r, Err(_) => return BottomCause::Conflict.into() };
            reg.values()
                .filter(|peer_gbb| crate::ladd::masa_compatible(&query_gbb, peer_gbb))
                .filter(|peer_gbb| crate::ladd::nerve_overlap(&query_gbb, peer_gbb))
                .map(|peer_gbb| {
                    let w = crate::ladd::gravitational_weight(&query_gbb, peer_gbb, EPSILON);
                    (w, peer_gbb.node_caid.to_string())
                })
                .collect()
        };

        if candidates.is_empty() { return bottom_not_found(); }
        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // 3. Horizon oscillation: 10% random jump
        let chosen_caid_str = if ctx.horizon_salt.digest.first() == Some(&0) {
            let idx = (ctx.horizon_salt.digest.get(1).copied().unwrap_or(0) as usize) % candidates.len();
            candidates[idx].1.clone()
        } else {
            candidates[0].1.clone()
        };

        // 4. Fetch target
        let target_caid_str = if let Value::Combo(ref c) = arg {
            if let Some(v) = c.get_field("target") { oo.force(v.clone(), ctx).to_string_plain() }
            else { chosen_caid_str.clone() }
        } else { chosen_caid_str.clone() };

        if let Ok(hash) = crate::value::ContentHash::parse(&target_caid_str) {
            if let Ok(val) = oo.store.get_value(&hash) { return val; }
            let peers_copy: Vec<_> = oo.peers.read().map(|p| p.values().cloned().collect()).unwrap_or_default();
            for peer in peers_copy {
                match peer {
                    crate::Peer::Local(store) => { if let Ok(val) = store.get_value(&hash) { return val; } }
                    crate::Peer::Remote(addr) => { if let Ok(val) = oo.remote_fetch(&addr, &hash) { return val; } }
                }
            }
        }
        bottom_not_found()
    }) as Arc<BuiltinFn>);
}