use crate::storage::ObjectStore;
use crate::value::{whole_argument, BottomCause, BottomDetail, ContentHash, EffectTag, Value};
use crate::{BuiltinFn, EvalContext, Ouroboros, Peer};
use nlang_parser::ast::AtomKind;
use std::collections::HashMap;
use std::sync::Arc;

const MAX_ROUTING_HOPS: u32 = 16;

fn base64_decode_sketch(s: &str) -> Vec<u8> {
    use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
    STANDARD_NO_PAD.decode(s).unwrap_or_default()
}

fn bottom_not_found() -> Value {
    Value::Bottom(Box::new(BottomDetail {
        cause: BottomCause::MissingKey,
        path: Some("disc.find".to_string()),
        message: Some("No matching peers found".to_string()),
        ..Default::default()
    }))
}

/// Perturb gravitational weight with a deterministic session salt.
/// Adds ±0.5% noise: enough to break ties, not enough to override strong gravity.
fn perturb_weight(weight: f64, caid: &str, horizon_salt: &crate::value::ContentHash) -> f64 {
    use sha2::{Digest as Sha2Digest, Sha256};
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
    use sha2::{Digest, Sha256};
    let mut keys: Vec<String> = cv
        .all_fields_iter()
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
        (cv.system.len()
            + cv.meta.len()
            + cv.types.len()
            + cv.rules.len()
            + cv.data.len()
            + cv.local.len()) as f64
    } else {
        1.0
    }
    .min(100.0)
}

/// Build the initial query nerve for disc.find (no overlapping MASA lookup).
fn build_query_nerve(val: &Value) -> Vec<crate::ladd::NerveEntry> {
    if let Value::Combo(ref cv) = val {
        let keys: Vec<String> = cv
            .all_fields_iter()
            .map(|(k, _)| k)
            .filter(|k| !k.starts_with('%') && !k.starts_with("~%"))
            .collect();
        if keys.is_empty() {
            vec![]
        } else {
            vec![crate::ladd::NerveEntry {
                masa_caid: field_key_masa_id(cv),
                overlapping_masa_caids: vec![],
                field_keys: keys,
            }]
        }
    } else {
        vec![]
    }
}

pub fn register_disc_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert(
        "disc.connect".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vname), Some(vpath)) = (c.get_field("0"), c.get_field("1")) {
                    let name = oo.force(vname.clone(), ctx).to_string_plain();
                    let path_str = oo.force(vpath.clone(), ctx).to_string_plain();
                    // Non-filesystem peer address (leave alone).
                    //
                    // ACCEPTANCE REVERT: the delivery also began accepting a
                    // `remote:` prefix here. No such scheme exists — the work
                    // order named it by mistake, and the delivery accommodated
                    // the acceptor's error instead of reporting it. Adding a peer
                    // address scheme is a language-surface change with no spec
                    // clause, no vector and no test; it is not this arc's. Root
                    // cause is the work order, so the note lives here rather than
                    // as a finding against the delivery.
                    if path_str.starts_with("tcp://") {
                        // connect_consent: remote dial is privileged. Gate before
                        // any peer table write (and thus before any later dial).
                        if !ctx.privilege.connect {
                            return Value::Bottom(Box::new(BottomDetail {
                            cause: BottomCause::PrivilegedRequired,
                            path: Some("Discovery./connect".to_string()),
                            message: Some(
                                "connect requires --grant connect (privilege.connect capability)"
                                    .to_string(),
                            ),
                            expected: None,
                            found: None,
                            involved: vec![],
                            ..Default::default()
                        }));
                        }
                        if let Ok(mut peers) = oo.peers.write() {
                            peers.insert(name, Peer::Remote(path_str[6..].to_string()));
                            return Value::Atom(
                                AtomKind::Tag("true".to_string()),
                                EffectTag::IO,
                                None,
                            );
                        }
                    } else {
                        // Judge the path as handed in (SPEC_08 §6.3). Connecting to
                        // a store directory is the same boundary crossing.
                        if crate::builtins::fs_guard::crosses_store_boundary(&path_str) {
                            return crate::builtins::fs_guard::store_boundary_refusal(&path_str);
                        }
                        let path = std::path::PathBuf::from(path_str);
                        if let Ok(store) = ObjectStore::init(&path) {
                            if let Ok(mut peers) = oo.peers.write() {
                                peers.insert(name, Peer::Local(Arc::new(store)));
                                return Value::Atom(
                                    AtomKind::Tag("true".to_string()),
                                    EffectTag::IO,
                                    None,
                                );
                            }
                        }
                    }
                }
            }
            Value::Atom(AtomKind::Tag("false".to_string()), EffectTag::IO, None)
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "disc.fetch".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let (node_name, caid_str) = if let Value::Combo(ref c) = arg {
                if let (Some(v0), Some(v1)) = (c.get_field("0"), c.get_field("1")) {
                    (
                        Some(oo.force(v0.clone(), ctx).to_string_plain()),
                        oo.force(v1.clone(), ctx).to_string_plain(),
                    )
                } else if let Some(v0) = c.get_field("0") {
                    (None, oo.force(v0.clone(), ctx).to_string_plain())
                } else {
                    return BottomCause::Conflict.into();
                }
            } else {
                (None, arg.collapse().to_string_plain())
            };

            if let Ok(hash) = ContentHash::parse(&caid_str) {
                // SPEC_08 §4.2.4: user-facing fetch solidifies active → #cached
                // (observation projection). Store remains raw (get_value).
                let observe = |val: Value| val.solidify_effects();

                /// Classify a store/peer read: verified value, mismatch (record), or absence.
                fn try_local(
                    oo: &Ouroboros,
                    store: &ObjectStore,
                    hash: &ContentHash,
                    source: &str,
                ) -> Result<Value, bool /* saw_mismatch */> {
                    match store.get_value(hash) {
                        Ok(v) => Ok(v),
                        Err(e) => match e.downcast_ref::<crate::storage::StoreReadError>() {
                            Some(crate::storage::StoreReadError::CaidMismatch { .. }) => {
                                oo.record_integrity(hash, source, crate::IntegrityKind::Mismatch);
                                Err(true)
                            }
                            Some(crate::storage::StoreReadError::ObjectUndecodable { .. }) => {
                                oo.record_integrity(
                                    hash,
                                    source,
                                    crate::IntegrityKind::Undecodable,
                                );
                                Err(true)
                            }
                            Some(crate::storage::StoreReadError::NotFound { .. }) | None => {
                                Err(false)
                            }
                        },
                    }
                }

                if let Some(name) = node_name {
                    // Named peer: single source. Mismatch → immediate #caid_mismatch.
                    let peer_opt = if let Ok(peers) = oo.peers.read() {
                        peers.get(&name).cloned()
                    } else {
                        None
                    };
                    if let Some(peer) = peer_opt {
                        match peer {
                            Peer::Local(store) => {
                                match try_local(oo, &store, &hash, &format!("peer:{name}")) {
                                    Ok(val) => return observe(val),
                                    Err(true) => return BottomCause::CaidMismatch.into(),
                                    Err(false) => {}
                                }
                            }
                            Peer::Remote(addr) => {
                                // Named peer is a single source: surface OODP's
                                // four-way discriminator (success / not_found /
                                // conflict / timeout) rather than collapsing to
                                // #conflict (REAL_02 §3.2 / REAL_03 §6.6 條款三).
                                match oo.remote_fetch(&addr, &hash) {
                                    Ok(val) => return observe(val),
                                    Err(e) => return e.into(),
                                }
                            }
                        }
                    }
                } else {
                    // Sweep: continue past lying sources (Q1); only verified bytes win.
                    let mut results = Vec::new();
                    let mut saw_mismatch = false;
                    // Peer said #not_found (honest absence). Distinct from a peer
                    // that refused / does not implement / spoke an unknown dialect.
                    let mut saw_peer_not_found = false;
                    let mut peer_protocol: Option<BottomCause> = None;

                    match try_local(oo, &oo.store, &hash, "local") {
                        Ok(val) => results.push(observe(val)),
                        Err(true) => saw_mismatch = true,
                        Err(false) => {}
                    }

                    // Drop automatic slots that are no longer exact-ad eligible
                    // (expired claim, replaced by relayed ad, copy-cleared, …).
                    oo.revalidate_automatic_remotes();

                    let peers_copy = if let Ok(peers) = oo.peers.read() {
                        peers
                            .iter()
                            .map(|(n, p)| (n.clone(), p.clone()))
                            .collect::<Vec<_>>()
                    } else {
                        vec![]
                    };
                    for (pname, peer) in peers_copy {
                        match peer {
                            Peer::Local(store) => {
                                match try_local(oo, &store, &hash, &format!("peer:{pname}")) {
                                    Ok(val) => results.push(observe(val)),
                                    Err(true) => saw_mismatch = true,
                                    Err(false) => {}
                                }
                            }
                            Peer::Remote(addr) => {
                                match oo.remote_fetch(&addr, &hash) {
                                    Ok(val) => {
                                        // First verified remote answer is definitive
                                        // (unordered peer set; degree-0 identity is unique).
                                        return observe(val);
                                    }
                                    // wire_says_why: only substantiated integrity
                                    // sets saw_mismatch; protocol answers do not.
                                    Err(BottomCause::CaidMismatch) => saw_mismatch = true,
                                    Err(BottomCause::MissingKey) => saw_peer_not_found = true,
                                    Err(
                                        e @ (BottomCause::PeerNotImplemented
                                        | BottomCause::PeerUnknownStatus
                                        | BottomCause::PeerRefused
                                        | BottomCause::PeerTimeout),
                                    ) => {
                                        peer_protocol = Some(e);
                                    }
                                    Err(_) => {}
                                }
                            }
                        }
                    }
                    // Automatic admission class (separate cap domain from manual peers).
                    let auto_copy: Vec<(String, String)> =
                        if let Ok(auto) = oo.automatic_remotes.read() {
                            auto.iter()
                                .map(|(nid, ar)| (nid.clone(), ar.addr.clone()))
                                .collect()
                        } else {
                            vec![]
                        };
                    for (node_id, addr) in auto_copy {
                        match oo.remote_fetch(&addr, &hash) {
                            Ok(val) => return observe(val),
                            Err(BottomCause::CaidMismatch) => saw_mismatch = true,
                            Err(BottomCause::MissingKey) => saw_peer_not_found = true,
                            Err(
                                e @ (BottomCause::PeerNotImplemented
                                | BottomCause::PeerUnknownStatus
                                | BottomCause::PeerRefused
                                | BottomCause::PeerTimeout),
                            ) => {
                                peer_protocol = Some(e);
                            }
                            Err(_) => {}
                        }
                        let _ = node_id; // source label reserved for future diagnostics
                    }

                    if results.is_empty() {
                        // Integrity first; then pure absence (any honest #not_found);
                        // then a lone protocol answer from the other end; else miss.
                        return if saw_mismatch {
                            BottomCause::CaidMismatch.into()
                        } else if saw_peer_not_found {
                            BottomCause::MissingKey.into()
                        } else if let Some(c) = peer_protocol {
                            c.into()
                        } else {
                            BottomCause::MissingKey.into()
                        };
                    }

                    let mut final_val = results.remove(0);
                    for v in results {
                        let merged = oo.unify_internal(final_val.clone(), v.clone(), ctx);
                        if let Value::Bottom(_) = merged {
                            if v.bits() > final_val.bits() {
                                final_val = v;
                            }
                        } else {
                            final_val = merged;
                        }
                    }
                    return final_val;
                }
            }
            BottomCause::Conflict.into()
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "disc.identify".to_string(),
        Arc::new(|arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| {
            // caid_of_the_argument: hash the applied value, not the pack.
            let v = whole_argument(arg);
            Value::Atom(
                AtomKind::Str(v.content_hash().to_string()),
                EffectTag::Pure,
                None,
            )
        }) as Arc<BuiltinFn>,
    );

    // Phase 4 / Phase 5: LADD advertise
    m.insert(
        "disc.advertise".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, _ctx: &mut EvalContext| {
            let arg = whole_argument(arg);
            let hash = arg.content_hash();
            let mass = compute_mass(&arg);
            let sketch_bytes = base64_decode_sketch(&hash.lattice_sketch);
            let masa_ref = hash.masa_ref.clone();
            // Phase 11: nerve_structure from field key MASA computation
            // Phase 17: also store field_keys for dynamic intersection + compute overlapping
            let nerve_structure: Vec<crate::ladd::NerveEntry> = if let Value::Combo(ref cv) = arg {
                let keys: Vec<String> = cv
                    .all_fields_iter()
                    .map(|(k, _)| k)
                    .filter(|k| !k.starts_with('%') && !k.starts_with("~%"))
                    .collect();
                if keys.is_empty() {
                    vec![]
                } else {
                    let my_masa = field_key_masa_id(cv);
                    let my_key_set: std::collections::HashSet<&str> =
                        keys.iter().map(|s| s.as_str()).collect();
                    let overlapping: Vec<String> = if let Ok(reg) = oo.gbb_registry.read() {
                        reg.values()
                            .flat_map(|g| g.nerve_structure.iter())
                            .filter(|ne| ne.masa_caid != my_masa)
                            .filter(|ne| {
                                ne.field_keys
                                    .iter()
                                    .any(|k| my_key_set.contains(k.as_str()))
                            })
                            .map(|ne| ne.masa_caid.clone())
                            .collect::<std::collections::HashSet<_>>()
                            .into_iter()
                            .collect()
                    } else {
                        vec![]
                    };
                    vec![crate::ladd::NerveEntry {
                        masa_caid: my_masa,
                        overlapping_masa_caids: overlapping,
                        field_keys: keys,
                    }]
                }
            } else {
                vec![]
            };
            let gbb = crate::ladd::GBB {
                node_caid: hash.clone(),
                mass,
                sketch_bytes,
                masa_ref,
                nerve_structure,
            };
            if let Ok(mut reg) = oo.gbb_registry.write() {
                reg.insert(hash.to_string(), gbb);
            }
            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::IO, None)
        }) as Arc<BuiltinFn>,
    );

    // Phase 4 / Phase 5: LADD find
    m.insert(
        "disc.find".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let arg = whole_argument(arg);
            // 1. Build initial query GBB.
            // When the argument is a CAID string (R4: find by store address), use
            // that CAID as the query node id so it meets the advertised key — not
            // the content-hash of the string atom.
            let query_hash = match &arg {
                Value::Atom(AtomKind::Str(s), _, _) => {
                    ContentHash::parse(s).unwrap_or_else(|_| arg.content_hash())
                }
                _ => arg.content_hash(),
            };
            let mut current_query = crate::ladd::GBB {
                node_caid: query_hash.clone(),
                mass: compute_mass(&arg),
                sketch_bytes: base64_decode_sketch(&query_hash.lattice_sketch),
                masa_ref: query_hash.masa_ref.clone(),
                nerve_structure: build_query_nerve(&arg),
            };

            // 2. Extract explicit target CAID (optional direct-lookup mode)
            let explicit_target: Option<String> = if let Value::Combo(ref c) = arg {
                c.get_field("target")
                    .map(|v| oo.force(v.clone(), ctx).to_string_plain())
            } else if let Value::Atom(AtomKind::Str(s), _, _) = &arg {
                // find "hash:…" — treat the string as a direct fetch target as well
                if ContentHash::parse(s).is_ok() {
                    Some(s.clone())
                } else {
                    None
                }
            } else {
                None
            };

            const EPSILON: f64 = 1e-6;

            // 3. Multi-hop routing loop
            loop {
                // Safety: hard hop budget (Phase 41)
                if ctx.disc_routing_hops >= MAX_ROUTING_HOPS {
                    // ERROR_CODES §2.7.1: a spent hop budget is not an attack.
                    // Mint #routing_budget_exceeded; keep SemanticEclipse
                    // readable for stored universes only (no longer minted).
                    return Value::Bottom(Box::new(BottomDetail {
                        cause: BottomCause::RoutingBudgetExceeded,
                        path: Some("disc.find".to_string()),
                        message: Some(format!(
                            "Routing budget exceeded after {} hops",
                            MAX_ROUTING_HOPS
                        )),
                        ..Default::default()
                    }));
                }

                // Gravitational candidate scoring
                let candidates: Vec<(f64, String)> = {
                    let reg = match oo.gbb_registry.read() {
                        Ok(r) => r,
                        Err(_) => return BottomCause::Conflict.into(),
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

                if candidates.is_empty() {
                    return bottom_not_found();
                }

                // Blacklist + horizon_salt tiebreaker (Phase 41)
                let mut perturbed: Vec<(f64, String)> = candidates
                    .iter()
                    .map(|(w, caid)| (perturb_weight(*w, caid, &ctx.horizon_salt), caid.clone()))
                    .collect();
                perturbed
                    .sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

                let chosen = if let Some((_, caid)) = perturbed
                    .iter()
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

                // Try local store, then connected peers (skip liars, continue sweep).
                // SPEC_08 §4.2.4: user-facing find solidifies active → #cached.
                if let Ok(hash) = crate::value::ContentHash::parse(fetch_target) {
                    let mut saw_mismatch = false;
                    match oo.store.get_value(&hash) {
                        Ok(val) => return val.solidify_effects(),
                        Err(e) => match e.downcast_ref::<crate::storage::StoreReadError>() {
                            Some(crate::storage::StoreReadError::CaidMismatch { .. }) => {
                                oo.record_integrity(&hash, "local", crate::IntegrityKind::Mismatch);
                                saw_mismatch = true;
                            }
                            Some(crate::storage::StoreReadError::ObjectUndecodable { .. }) => {
                                oo.record_integrity(
                                    &hash,
                                    "local",
                                    crate::IntegrityKind::Undecodable,
                                );
                                saw_mismatch = true;
                            }
                            _ => {}
                        },
                    }
                    oo.revalidate_automatic_remotes();
                    let peers_copy: Vec<_> = oo
                        .peers
                        .read()
                        .map(|p| p.iter().map(|(n, pe)| (n.clone(), pe.clone())).collect())
                        .unwrap_or_default();
                    for (pname, peer) in peers_copy {
                        match peer {
                            crate::Peer::Local(store) => match store.get_value(&hash) {
                                Ok(val) => return val.solidify_effects(),
                                Err(e) => {
                                    match e.downcast_ref::<crate::storage::StoreReadError>() {
                                        Some(crate::storage::StoreReadError::CaidMismatch {
                                            ..
                                        }) => {
                                            oo.record_integrity(
                                                &hash,
                                                &format!("peer:{pname}"),
                                                crate::IntegrityKind::Mismatch,
                                            );
                                            saw_mismatch = true;
                                        }
                                        Some(
                                            crate::storage::StoreReadError::ObjectUndecodable {
                                                ..
                                            },
                                        ) => {
                                            oo.record_integrity(
                                                &hash,
                                                &format!("peer:{pname}"),
                                                crate::IntegrityKind::Undecodable,
                                            );
                                            saw_mismatch = true;
                                        }
                                        _ => {}
                                    }
                                }
                            },
                            crate::Peer::Remote(addr) => match oo.remote_fetch(&addr, &hash) {
                                Ok(val) => return val.solidify_effects(),
                                Err(BottomCause::CaidMismatch) => saw_mismatch = true,
                                Err(_) => {}
                            },
                        }
                    }
                    let auto_copy: Vec<String> = oo
                        .automatic_remotes
                        .read()
                        .map(|a| a.values().map(|ar| ar.addr.clone()).collect())
                        .unwrap_or_default();
                    for addr in auto_copy {
                        match oo.remote_fetch(&addr, &hash) {
                            Ok(val) => return val.solidify_effects(),
                            Err(BottomCause::CaidMismatch) => saw_mismatch = true,
                            Err(_) => {}
                        }
                    }
                    // If every source lied and none verified, surface #caid_mismatch
                    // rather than advancing the hop as if the CAID were merely absent.
                    if saw_mismatch {
                        return BottomCause::CaidMismatch.into();
                    }
                }

                // Value not found at this hop — advance query to chosen GBB for next hop
                let next_gbb = {
                    let reg = match oo.gbb_registry.read() {
                        Ok(r) => r,
                        Err(_) => return BottomCause::Conflict.into(),
                    };
                    reg.get(&chosen).cloned()
                };

                match next_gbb {
                    Some(gbb) => {
                        current_query = gbb;
                    }
                    None => {
                        return bottom_not_found();
                    }
                }
            }
        }) as Arc<BuiltinFn>,
    );
}
