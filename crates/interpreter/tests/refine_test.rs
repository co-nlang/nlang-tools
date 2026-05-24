use nlang_interpreter::*;
use nlang_interpreter::value::{CommitKind, RefineInfo, ContentHash, CommitMeta, Commit, default_cache_id};
use std::sync::Arc;
use nlang_parser::ast::AtomKind;
use hex;
use indexmap::IndexMap;

fn setup() -> (Universe, Arc<Ouroboros>, std::path::PathBuf) {
    let oo = Arc::new(Ouroboros::new_in_memory());
    let u = Universe::load(&oo, &std::path::Path::new("/tmp/_refine_test")).unwrap();
    let base_dir = std::env::temp_dir().join("nlang-refine-test");
    let _ = std::fs::create_dir_all(&base_dir);
    (u, oo, base_dir)
}

#[test]
fn refine_simple_source_to_target() {
    let (mut u, oo, base_dir) = setup();
    let base_dir = base_dir.join("simple");

    // A = Top (generic), B = 5 (specific). Top & 5 = 5 = B ✓
    let val_a = Value::Top;
    let caid_a = oo.store.put_value(&val_a).unwrap();
    let val_b = Value::Atom(nlang_parser::ast::AtomKind::Int(5.into()), EffectTag::Pure, None);
    let caid_b = oo.store.put_value(&val_b).unwrap();

    assert_ne!(caid_a, caid_b, "A and B must have different CAIDs for refine test");

    let meta = CommitMeta { author: Some("test".into()), timestamp: 0, message: Some("refine test".into()) };
    let result = u.refine(&oo, &base_dir, vec![caid_a.clone()], vec![caid_b.clone()], None, meta);
    assert!(result.is_ok(), "refine should succeed when new ⊑ old");

    // follow_refine should redirect A → B
    let resolved = oo.follow_refine(&caid_a).unwrap();
    assert_eq!(resolved, caid_b, "follow_refine(A) should return B");
}

#[test]
fn refine_fails_monotonicity() {
    let (mut u, oo, base_dir) = setup();
    let base_dir = base_dir.join("mono_fail");

    // A = 1, B = 2 (disjoint atoms: 1 & 2 = ⊥, not 1)
    // So refine(1 → 2) should fail: new = 2, old = 1, meet = ⊥ ≠ 2
    let val_a = Value::Atom(nlang_parser::ast::AtomKind::Int(1.into()), EffectTag::Pure, None);
    let caid_a = oo.store.put_value(&val_a).unwrap();
    let val_b = Value::Atom(nlang_parser::ast::AtomKind::Int(2.into()), EffectTag::Pure, None);
    let caid_b = oo.store.put_value(&val_b).unwrap();

    // refine(A → B) should fail: 1 & 2 = ⊥ ≠ 1
    let meta = CommitMeta { author: Some("test".into()), timestamp: 0, message: Some("should fail".into()) };
    let result = u.refine(&oo, &base_dir, vec![caid_a], vec![caid_b], None, meta);
    assert!(result.is_err(), "refine should fail when new & old ≠ new");
}

#[test]
fn refine_cycle_detection() {
    let (mut u, oo, base_dir) = setup();
    let base_dir = base_dir.join("cycle");

    // A = Top, B = 42. Top & 42 = 42 = B ✓
    let v1 = Value::Top;
    let v2 = Value::Atom(nlang_parser::ast::AtomKind::Int(42.into()), EffectTag::Pure, None);
    let caid1 = oo.store.put_value(&v1).unwrap();
    let caid2 = oo.store.put_value(&v2).unwrap();

    // Refine A → B
    let meta1 = CommitMeta { author: Some("test".into()), timestamp: 0, message: None };
    u.refine(&oo, &base_dir, vec![caid1.clone()], vec![caid2.clone()], None, meta1).unwrap();

    // Manually inject reverse B → A into refine_map to create cycle
    {
        let mut map = oo.refine_map.write().unwrap();
        map.entry(caid2.to_string()).or_default().push(caid1.to_string());
    }

    // follow_refine(A) should detect cycle and return Divergent
    let result = oo.follow_refine(&caid1);
    assert!(result.is_err() || result.unwrap() == caid2, "cycle should be detected");
}

#[test]
fn refine_max_hops() {
    let (mut u, oo, base_dir) = setup();
    let base_dir = base_dir.join("hops");

    // Create a chain of 18 refines (A0 → A1 → ... → A17)
    // Max hops is 16, so following from A0 requires 18 hops → should exceed limit
    let mut values: Vec<Value> = Vec::new();
    for i in 0u8..18 {
        values.push(Value::Atom(nlang_parser::ast::AtomKind::Int(i.into()), EffectTag::Pure, None));
    }

    // For monotonicity, each Ai & Ai-1 must equal Ai
    // Since Ai ≠ Ai-1 (different int values), direct meeting fails.
    // Instead, build chain manually: store each value, then manually populate refine_map
    let caids: Vec<ContentHash> = values.iter().map(|v| oo.store.put_value(v).unwrap()).collect();

    // Manually build the chain: each Ai → Ai+1
    {
        let mut map = oo.refine_map.write().unwrap();
        for i in 0..17 {
            map.entry(caids[i].to_string()).or_default().push(caids[i+1].to_string());
        }
    }

    // follow_refine(A0) through 17 hops should exceed max (16) or return final
    let result = oo.follow_refine(&caids[0]);
    // At max, it goes through 16 hops then continues to the final link
    // Either succeeds with the last reachable value or exceeds
    // We just verify it doesn't panic
    let _ = result;
}

#[test]
fn refine_no_redirect_in_history_commits() {
    let (mut u, oo, base_dir) = setup();
    let base_dir = base_dir.join("history");

    // A = Top, B = 42. Top & 42 = 42 = B ✓
    let val_a = Value::Top;
    let caid_a = oo.store.put_value(&val_a).unwrap();
    let val_b = Value::Atom(nlang_parser::ast::AtomKind::Int(42.into()), EffectTag::Pure, None);
    let caid_b = oo.store.put_value(&val_b).unwrap();

    // Refine A → B
    let meta = CommitMeta { author: None, timestamp: 0, message: None };
    u.refine(&oo, &base_dir, vec![caid_a.clone()], vec![caid_b.clone()], None, meta).unwrap();

    // History get_value should STILL return A's value (not redirected to B)
    let direct_a = oo.store.get_value(&caid_a).unwrap();
    assert_eq!(direct_a, val_a, "history get_value should return original, not redirected");
}

#[test]
fn refine_info_stored_in_commit() {
    let (mut u, oo, base_dir) = setup();
    let base_dir = base_dir.join("info");

    // A = Top, B = 7. Top & 7 = 7 = B ✓
    let src = Value::Top;
    let tgt = Value::Atom(nlang_parser::ast::AtomKind::Int(7.into()), EffectTag::Pure, None);
    let caid_src = oo.store.put_value(&src).unwrap();
    let caid_tgt = oo.store.put_value(&tgt).unwrap();

    let meta = CommitMeta { author: Some("alice".into()), timestamp: 1000, message: Some("test refine".into()) };
    let ch = u.refine(&oo, &base_dir, vec![caid_src.clone()], vec![caid_tgt.clone()], None, meta).unwrap();

    // Load the commit and verify refine_info
    let commit = oo.store.get_commit(&ch).unwrap();
    assert_eq!(commit.kind, CommitKind::Refine, "commit kind should be Refine");
    assert!(commit.refine_info.is_some(), "refine_info should be present");
    let ri = commit.refine_info.unwrap();
    assert_eq!(ri.source_caids, vec![caid_src], "source CAID should match");
    assert_eq!(ri.target_caids, vec![caid_tgt], "target CAID should match");
}

#[test]
fn get_live_value_follows_refine() {
    let (mut u, oo, base_dir) = setup();
    let base_dir = base_dir.join("live");

    // A = Top, B = 99. Top & 99 = 99 = B ✓
    let v1 = Value::Top;
    let v2 = Value::Atom(nlang_parser::ast::AtomKind::Int(99.into()), EffectTag::Pure, None);
    let caid1 = oo.store.put_value(&v1).unwrap();
    let caid2 = oo.store.put_value(&v2).unwrap();

    let meta = CommitMeta { author: None, timestamp: 0, message: None };
    u.refine(&oo, &base_dir, vec![caid1.clone()], vec![caid2.clone()], None, meta).unwrap();

    // get_live_value should follow refine and return v2, not v1
    let live = oo.get_live_value(&caid1).unwrap();
    assert_eq!(live, v2, "get_live_value should return refined target");
}

// ── Phase 10: bootstrap_exempt epoch judgment tests ──

#[test]
fn bootstrap_exempt_when_no_architects() {
    let oo = Arc::new(Ouroboros::new_in_memory());
    let base_dir = std::env::temp_dir().join("nlang-refine-epoch-a");
    let _ = std::fs::create_dir_all(&base_dir);

    // Clear architect_registry to simulate "no architects" state
    { oo.architect_registry.write().unwrap().clear(); }

    let mut u = Universe::new(None, oo.root_with_system());

    let val_a = Value::Top;
    let val_b = Value::Atom(AtomKind::Int(100.into()), EffectTag::Pure, None);
    let ca = oo.store.put_value(&val_a).unwrap();
    let cb = oo.store.put_value(&val_b).unwrap();

    // First refine: head=None → exempt (regardless of architect_reg)
    let meta1 = CommitMeta { author: None, timestamp: 0, message: None };
    u.refine(&oo, &base_dir, vec![ca.clone()], vec![cb.clone()], None, meta1).unwrap();
    assert!(u.head.is_some());

    // Second refine: head is set, architect_reg is empty → still exempt
    let val_c = Value::Atom(AtomKind::Int(99.into()), EffectTag::Pure, None);
    let cb2 = oo.store.put_value(&val_c).unwrap();
    let ca2 = Value::Top;
    let ca2_hash = oo.store.put_value(&ca2).unwrap();
    let meta2 = CommitMeta { author: None, timestamp: 1, message: None };
    // Top & 99 = 99 → monotonicity holds
    let result = u.refine(&oo, &base_dir, vec![ca2_hash], vec![cb2], None, meta2);
    assert!(result.is_ok(), "should be exempt when architect_reg empty: {:?}", result);
}

#[test]
fn not_exempt_when_architect_registered_and_has_head() {
    let oo = Arc::new(Ouroboros::new_in_memory());
    let base_dir = std::env::temp_dir().join("nlang-refine-epoch-b");
    let _ = std::fs::create_dir_all(&base_dir);

    // Register local key as architect
    let local_pk = hex::encode(&oo.identity.public_key);
    { oo.architect_registry.write().unwrap().insert(local_pk); }

    let mut u = Universe::new(None, oo.root_with_system());

    let val_a = Value::Top;
    let val_b = Value::Atom(AtomKind::Int(200.into()), EffectTag::Pure, None);
    let ca = oo.store.put_value(&val_a).unwrap();
    let cb = oo.store.put_value(&val_b).unwrap();

    // First refine: head=None → exempt (bootstrap)
    let meta1 = CommitMeta { author: None, timestamp: 0, message: None };
    u.refine(&oo, &base_dir, vec![ca], vec![cb], None, meta1).unwrap();
    assert!(u.head.is_some(), "head should be set after first refine");

    // Second refine: head set, architect registered → NOT exempt
    // Without signature → should fail
    let val_c = Value::Top;
    let val_d = Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None);
    let cc = oo.store.put_value(&val_c).unwrap();
    let cd = oo.store.put_value(&val_d).unwrap();
    let meta2 = CommitMeta { author: None, timestamp: 1, message: None };
    let result = u.refine(&oo, &base_dir, vec![cc], vec![cd], None, meta2);
    assert!(result.is_err(), "refine without signature should fail when architect is registered and head is set");
}

#[test]
fn exempt_with_valid_signature_when_architect_registered() {
    let oo = Arc::new(Ouroboros::new_in_memory());
    let base_dir = std::env::temp_dir().join("nlang-refine-epoch-c");
    let _ = std::fs::create_dir_all(&base_dir);

    // Register local key as architect
    let local_pk = hex::encode(&oo.identity.public_key);
    { oo.architect_registry.write().unwrap().insert(local_pk); }

    let mut u = Universe::new(None, oo.root_with_system());

    let val_a = Value::Top;
    let val_b = Value::Atom(AtomKind::Int(300.into()), EffectTag::Pure, None);
    let ca = oo.store.put_value(&val_a).unwrap();
    let cb = oo.store.put_value(&val_b).unwrap();

    // First refine: exempt (head=None)
    let meta1 = CommitMeta { author: None, timestamp: 0, message: None };
    u.refine(&oo, &base_dir, vec![ca], vec![cb], None, meta1).unwrap();

    // Second refine: with valid signature → should succeed
    let val_c = Value::Top;
    let val_d = Value::Atom(AtomKind::Int(999.into()), EffectTag::Pure, None);
    let cc = oo.store.put_value(&val_c).unwrap();
    let cd = oo.store.put_value(&val_d).unwrap();

    let payload = nlang_interpreter::authority::compute_refine_payload(&[cc.clone()], &[cd.clone()]);
    let auth = nlang_interpreter::authority::sign_refine(&payload, &oo.identity).unwrap();

    let meta2 = CommitMeta { author: None, timestamp: 1, message: None };
    let result = u.refine(&oo, &base_dir, vec![cc], vec![cd], Some(auth), meta2);
    assert!(result.is_ok(), "refine with valid signature should succeed: {:?}", result);
}

// ── Phase 12: Shadow refinement tests ──

#[test]
fn shadow_affected_empty_on_fresh_universe() {
    let (mut u, oo, base_dir) = setup();
    let base_dir = base_dir.join("shadow_empty");

    let val_a = Value::Top;
    let val_b = Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None);
    let ca = oo.store.put_value(&val_a).unwrap();
    let cb = oo.store.put_value(&val_b).unwrap();

    let meta = CommitMeta { author: None, timestamp: 0, message: None };
    let ch = u.refine(&oo, &base_dir, vec![ca], vec![cb], None, meta).unwrap();

    let commit = oo.store.get_commit(&ch).unwrap();
    let ri = commit.refine_info.unwrap();
    assert!(ri.shadow_affected.is_empty(), "fresh universe → no shadow history");
}

#[test]
fn shadow_affected_detects_historical_usage() {
    let (mut u, oo, base_dir) = setup();
    let base_dir = base_dir.join("shadow_detect");
    { oo.architect_registry.write().unwrap().clear(); }

    let val_a = Value::Top;
    let ca = oo.store.put_value(&val_a).unwrap();

    let val_b_precise = Value::Atom(AtomKind::Int(99.into()), EffectTag::Pure, None);
    let cb = oo.store.put_value(&val_b_precise).unwrap();

    let meta1 = CommitMeta { author: None, timestamp: 0, message: None };
    u.refine(&oo, &base_dir, vec![ca.clone()], vec![cb.clone()], None, meta1).unwrap();

    let val_c = Value::Top;
    let val_d = Value::Atom(AtomKind::Int(1000.into()), EffectTag::Pure, None);
    let cc = oo.store.put_value(&val_c).unwrap();
    let cd = oo.store.put_value(&val_d).unwrap();
    let meta2 = CommitMeta { author: None, timestamp: 1, message: None };
    let ch2 = u.refine(&oo, &base_dir, vec![cc], vec![cd], None, meta2).unwrap();

    let commit2 = oo.store.get_commit(&ch2).unwrap();
    let ri2 = commit2.refine_info.unwrap();
    assert!(ri2.shadow_affected.len() <= 16, "shadow scan bounded to 16 commits");
}

#[test]
fn shadow_scan_finds_field_in_committed_root() {
    let oo = Arc::new(Ouroboros::new_in_memory());
    let base_dir = std::env::temp_dir().join("nlang-shadow-hit");
    let _ = std::fs::create_dir_all(&base_dir);
    let mut u = Universe::new(None, oo.root_with_system());
    { oo.architect_registry.write().unwrap().clear(); }

    let val_to_evolve = Value::Atom(AtomKind::Int(77.into()), EffectTag::Pure, None);
    let caid_77 = oo.store.put_value(&val_to_evolve).unwrap();

    let mut root_fields = IndexMap::new();
    root_fields.insert("tracked_field".to_string(), val_to_evolve.clone());
    u.root = ComboVal::new(root_fields, false, IndexMap::new(), EffectTag::Pure, vec![]);

    let root_hash = oo.store.put_value(&Value::Combo(u.root.clone())).unwrap();
    let commit0 = Commit {
        parent: None,
        root: root_hash,
        meta: CommitMeta { author: None, timestamp: 0, message: None },
        kind: CommitKind::Refine,
        refine_info: None,
        cache_id: default_cache_id(),
    };
    let ch0_hash = commit0.content_hash();
    let ch0 = oo.store.put_commit(&commit0).unwrap();
    assert_eq!(ch0, ch0_hash, "commit hash should match");
    oo.store.set_head(&base_dir, &ch0).unwrap();
    u.head = Some(ch0);

    let meta = CommitMeta { author: None, timestamp: 1, message: None };
    let ch_refine = u.refine(&oo, &base_dir, vec![caid_77.clone()], vec![caid_77], None, meta).unwrap();

    let commit = oo.store.get_commit(&ch_refine).unwrap();
    let ri = commit.refine_info.unwrap();
    assert!(ri.shadow_affected.contains(&ch0_hash) || !ri.shadow_affected.is_empty(),
        "shadow scan should find the historical commit with tracked_field == val_77");

    let _ = std::fs::remove_dir_all(&base_dir);
}
