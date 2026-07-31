use nlang_interpreter::ladd::{
    d_l_approx, gravitational_weight, masa_compatible, nerve_overlap, NerveEntry, GBB,
};
use nlang_interpreter::value::{CaidVersion, ContentHash, HashAlgorithm, MasaRef};

fn dummy_caid() -> ContentHash {
    ContentHash {
        algorithm: HashAlgorithm::Sha256,
        version: CaidVersion::V2,
        masa_ref: MasaRef::Top,
        lattice_sketch: String::new(),
        digest: vec![0; 32],
    }
}

fn gbb(masa: MasaRef, nerve: Vec<NerveEntry>) -> GBB {
    GBB {
        node_caid: dummy_caid(),
        mass: 1.0,
        sketch_bytes: vec![0x42],
        masa_ref: masa,
        nerve_structure: nerve,
    }
}

#[test]
fn test_nerve_overlap_both_empty() {
    let a = gbb(MasaRef::Top, vec![]);
    let b = gbb(MasaRef::Top, vec![]);
    assert!(nerve_overlap(&a, &b), "both empty → true (no pruning info)");
}

#[test]
fn test_nerve_overlap_no_common() {
    let a = gbb(
        MasaRef::Top,
        vec![NerveEntry {
            masa_caid: "m1".into(),
            overlapping_masa_caids: vec![],
            field_keys: vec![],
        }],
    );
    let b = gbb(
        MasaRef::Top,
        vec![NerveEntry {
            masa_caid: "m2".into(),
            overlapping_masa_caids: vec![],
            field_keys: vec![],
        }],
    );
    assert!(!nerve_overlap(&a, &b), "different MASA → false");
}

#[test]
fn test_nerve_overlap_direct_match() {
    let a = gbb(
        MasaRef::Top,
        vec![NerveEntry {
            masa_caid: "m1".into(),
            overlapping_masa_caids: vec![],
            field_keys: vec![],
        }],
    );
    let b = gbb(
        MasaRef::Top,
        vec![NerveEntry {
            masa_caid: "m1".into(),
            overlapping_masa_caids: vec![],
            field_keys: vec![],
        }],
    );
    assert!(nerve_overlap(&a, &b), "same masa_caid → true");
}

#[test]
fn test_nerve_overlap_via_overlapping() {
    let a = gbb(
        MasaRef::Top,
        vec![NerveEntry {
            masa_caid: "m1".into(),
            overlapping_masa_caids: vec![],
            field_keys: vec![],
        }],
    );
    let b = gbb(
        MasaRef::Top,
        vec![NerveEntry {
            masa_caid: "m2".into(),
            overlapping_masa_caids: vec!["m1".into()],
            field_keys: vec![],
        }],
    );
    assert!(
        nerve_overlap(&a, &b),
        "overlapping includes query's masa → true"
    );
}

#[test]
fn test_d_l_approx_after_phase5() {
    let a = gbb(MasaRef::Top, vec![]);
    let mut b = a.clone();
    b.sketch_bytes = vec![0xFF];
    let d = d_l_approx(&a, &b);
    assert!(d > 0.0, "different sketch → d_L > 0");
}

#[test]
fn test_masa_and_nerve_combined() {
    let q = gbb(
        MasaRef::Top,
        vec![NerveEntry {
            masa_caid: "m1".into(),
            overlapping_masa_caids: vec![],
            field_keys: vec![],
        }],
    );
    let p = gbb(
        MasaRef::Digest(vec![1]),
        vec![NerveEntry {
            masa_caid: "m2".into(),
            overlapping_masa_caids: vec![],
            field_keys: vec![],
        }],
    );
    assert!(masa_compatible(&q, &p), "Top × Digest → compatible");
    assert!(!nerve_overlap(&q, &p), "different nerve → incompatible");
}

#[test]
fn test_gravitational_weight_with_nerve_gbb() {
    let q = gbb(MasaRef::Top, vec![]);
    let p = GBB {
        mass: 2.0,
        ..gbb(MasaRef::Top, vec![])
    };
    let w = gravitational_weight(&q, &p, 1e-6);
    assert!(w > 0.0, "weight should be positive");
}

// ── Phase 11: nerve_structure field-key MASA tests ──

#[test]
fn nerve_overlap_same_field_structure() {
    use indexmap::IndexMap;
    use nlang_interpreter::value::{ComboVal, EffectTag};
    use nlang_interpreter::*;
    use nlang_parser::ast::AtomKind;
    use std::sync::Arc;

    let oo = Arc::new(Ouroboros::new_in_memory());
    let mut ctx = EvalContext::new(oo.root_with_system());

    let mut fields1 = IndexMap::new();
    fields1.insert("x".to_string(), Value::Top);
    fields1.insert("y".to_string(), Value::Top);
    let cv1 = ComboVal::new(
        fields1.clone(),
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    );

    let mut fields2 = IndexMap::new();
    fields2.insert(
        "x".to_string(),
        Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None),
    );
    fields2.insert(
        "y".to_string(),
        Value::Atom(AtomKind::Int(7.into()), EffectTag::Pure, None),
    );
    let cv2 = ComboVal::new(fields2, false, IndexMap::new(), EffectTag::Pure, vec![]);

    let advertise_fn = oo.builtin_registry.get("disc.advertise").unwrap();
    advertise_fn(Value::Combo(cv1), &oo, &mut ctx);
    advertise_fn(Value::Combo(cv2), &oo, &mut ctx);

    let reg = oo.gbb_registry.read().unwrap();
    let masa_ids: Vec<_> = reg
        .values()
        .filter(|gbb| !gbb.nerve_structure.is_empty())
        .map(|gbb| gbb.nerve_structure[0].masa_caid.clone())
        .collect();

    if masa_ids.len() >= 2 {
        assert_eq!(
            masa_ids[0], masa_ids[1],
            "same field structure → same MASA id"
        );
    }
}

#[test]
fn nerve_different_field_structure_different_masa() {
    use indexmap::IndexMap;
    use nlang_interpreter::value::{ComboVal, EffectTag};
    use nlang_interpreter::*;
    use std::sync::Arc;

    let mut fields1 = IndexMap::new();
    fields1.insert("x".to_string(), Value::Top);
    fields1.insert("y".to_string(), Value::Top);
    let cv1 = ComboVal::new(fields1, false, IndexMap::new(), EffectTag::Pure, vec![]);

    let mut fields2 = IndexMap::new();
    fields2.insert("a".to_string(), Value::Top);
    fields2.insert("b".to_string(), Value::Top);
    let cv2 = ComboVal::new(fields2, false, IndexMap::new(), EffectTag::Pure, vec![]);

    let oo = Arc::new(Ouroboros::new_in_memory());
    let mut ctx = EvalContext::new(oo.root_with_system());
    let advertise_fn = oo.builtin_registry.get("disc.advertise").unwrap();
    advertise_fn(Value::Combo(cv1), &oo, &mut ctx);
    advertise_fn(Value::Combo(cv2), &oo, &mut ctx);

    let reg = oo.gbb_registry.read().unwrap();
    let masa_ids: std::collections::HashSet<_> = reg
        .values()
        .filter(|gbb| !gbb.nerve_structure.is_empty())
        .map(|gbb| gbb.nerve_structure[0].masa_caid.clone())
        .collect();

    assert_eq!(
        masa_ids.len(),
        2,
        "different field structures → different MASA ids"
    );
}

#[test]
fn nerve_non_combo_empty_structure() {
    use nlang_interpreter::value::EffectTag;
    use nlang_interpreter::*;
    use nlang_parser::ast::AtomKind;
    use std::sync::Arc;

    let oo = Arc::new(Ouroboros::new_in_memory());
    let mut ctx = EvalContext::new(oo.root_with_system());
    let atom = Value::Atom(AtomKind::Int(99.into()), EffectTag::Pure, None);
    let advertise_fn = oo.builtin_registry.get("disc.advertise").unwrap();
    advertise_fn(atom, &oo, &mut ctx);

    let reg = oo.gbb_registry.read().unwrap();
    let nerve_lens: Vec<_> = reg.values().map(|gbb| gbb.nerve_structure.len()).collect();
    assert!(
        nerve_lens.iter().all(|&l| l == 0),
        "non-Combo → empty nerve_structure"
    );
}

fn make_gbb_with_nerve(nerve: Vec<NerveEntry>) -> GBB {
    GBB {
        node_caid: dummy_caid(),
        mass: 1.0,
        sketch_bytes: vec![0x42],
        masa_ref: MasaRef::Top,
        nerve_structure: nerve,
    }
}

// ── Phase 17: NerveEntry.field_keys tests ──

#[test]
fn test_nerve_overlap_same_masa() {
    let ne_a = NerveEntry {
        masa_caid: "masa:fk:abc".into(),
        overlapping_masa_caids: vec![],
        field_keys: vec!["x".into(), "y".into()],
    };
    let ne_b = NerveEntry {
        masa_caid: "masa:fk:abc".into(),
        overlapping_masa_caids: vec![],
        field_keys: vec!["x".into(), "y".into()],
    };
    let gbb_a = make_gbb_with_nerve(vec![ne_a]);
    let gbb_b = make_gbb_with_nerve(vec![ne_b]);
    assert!(nerve_overlap(&gbb_a, &gbb_b));
}

#[test]
fn test_nerve_overlap_partial_field_keys() {
    let ne_a = NerveEntry {
        masa_caid: "masa:fk:aaa".into(),
        overlapping_masa_caids: vec![],
        field_keys: vec!["x".into(), "y".into()],
    };
    let ne_b = NerveEntry {
        masa_caid: "masa:fk:bbb".into(),
        overlapping_masa_caids: vec![],
        field_keys: vec!["x".into(), "z".into()],
    };
    let gbb_a = make_gbb_with_nerve(vec![ne_a]);
    let gbb_b = make_gbb_with_nerve(vec![ne_b]);
    assert!(nerve_overlap(&gbb_a, &gbb_b));
}

#[test]
fn test_nerve_overlap_disjoint_field_keys() {
    let ne_a = NerveEntry {
        masa_caid: "masa:fk:aaa".into(),
        overlapping_masa_caids: vec![],
        field_keys: vec!["x".into(), "y".into()],
    };
    let ne_b = NerveEntry {
        masa_caid: "masa:fk:bbb".into(),
        overlapping_masa_caids: vec![],
        field_keys: vec!["z".into(), "w".into()],
    };
    let gbb_a = make_gbb_with_nerve(vec![ne_a]);
    let gbb_b = make_gbb_with_nerve(vec![ne_b]);
    assert!(!nerve_overlap(&gbb_a, &gbb_b));
}

#[test]
fn test_nerve_overlap_precomputed_overlapping() {
    let ne_a = NerveEntry {
        masa_caid: "masa:fk:aaa".into(),
        overlapping_masa_caids: vec![],
        field_keys: vec![],
    };
    let ne_b = NerveEntry {
        masa_caid: "masa:fk:bbb".into(),
        overlapping_masa_caids: vec!["masa:fk:aaa".into()],
        field_keys: vec![],
    };
    let gbb_a = make_gbb_with_nerve(vec![ne_a]);
    let gbb_b = make_gbb_with_nerve(vec![ne_b]);
    assert!(nerve_overlap(&gbb_a, &gbb_b));
}

// ── Phase 38: 精確交集 field_keys 過濾 ──

#[test]
fn test_morphism_node_gets_empty_nerve() {
    use indexmap::IndexMap;
    use nlang_interpreter::value::{ComboVal, EffectTag};
    use nlang_interpreter::*;
    use nlang_parser::ast::AtomKind;
    use std::sync::Arc;

    let oo = Arc::new(Ouroboros::new_in_memory());
    let mut ctx = EvalContext::new(oo.root_with_system());

    let mut fields = IndexMap::new();
    fields.insert(
        "%morphism".to_string(),
        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
    );
    fields.insert(
        "%builtin".to_string(),
        Value::Atom(AtomKind::Str("str.len".to_string()), EffectTag::Pure, None),
    );
    let cv = ComboVal::new(fields, true, IndexMap::new(), EffectTag::Pure, vec![]);

    let advertise_fn = oo.builtin_registry.get("disc.advertise").unwrap();
    advertise_fn(Value::Combo(cv), &oo, &mut ctx);

    let reg = oo.gbb_registry.read().unwrap();
    let all_empty = reg.values().all(|gbb| gbb.nerve_structure.is_empty());
    assert!(
        all_empty,
        "morphism node (only %-keys) -> empty nerve_structure"
    );
}

#[test]
fn test_data_node_nerve_excludes_percent_keys() {
    use indexmap::IndexMap;
    use nlang_interpreter::value::{ComboVal, EffectTag};
    use nlang_interpreter::*;
    use nlang_parser::ast::AtomKind;
    use std::sync::Arc;

    let oo = Arc::new(Ouroboros::new_in_memory());
    let mut ctx = EvalContext::new(oo.root_with_system());

    let mut fields = IndexMap::new();
    fields.insert(
        "name".to_string(),
        Value::Atom(AtomKind::Str("Alice".to_string()), EffectTag::Pure, None),
    );
    fields.insert(
        "age".to_string(),
        Value::Atom(AtomKind::Int(30.into()), EffectTag::Pure, None),
    );
    fields.insert(
        "%kind".to_string(),
        Value::Atom(AtomKind::Tag("person".to_string()), EffectTag::Pure, None),
    );
    let cv = ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]);

    let advertise_fn = oo.builtin_registry.get("disc.advertise").unwrap();
    advertise_fn(Value::Combo(cv), &oo, &mut ctx);

    let reg = oo.gbb_registry.read().unwrap();
    let all_nerves: Vec<_> = reg
        .values()
        .filter(|gbb| !gbb.nerve_structure.is_empty())
        .flat_map(|gbb| gbb.nerve_structure[0].field_keys.iter().cloned())
        .collect();

    assert!(
        !all_nerves.is_empty(),
        "data+%kind node should have non-empty nerve"
    );
    assert!(
        all_nerves
            .iter()
            .all(|k| !k.starts_with('%') && !k.starts_with("~%")),
        "nerve field_keys must not contain %-prefixed keys, got: {:?}",
        all_nerves
    );
    assert!(
        all_nerves.contains(&"name".to_string()),
        "nerve should contain 'name'"
    );
    assert!(
        all_nerves.contains(&"age".to_string()),
        "nerve should contain 'age'"
    );
}

#[test]
fn test_same_structure_diff_percent_same_masa() {
    use indexmap::IndexMap;
    use nlang_interpreter::value::{ComboVal, EffectTag};
    use nlang_interpreter::*;
    use nlang_parser::ast::AtomKind;
    use std::sync::Arc;

    let oo = Arc::new(Ouroboros::new_in_memory());
    let mut ctx = EvalContext::new(oo.root_with_system());

    let mut f1 = IndexMap::new();
    f1.insert(
        "x".to_string(),
        Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None),
    );
    f1.insert(
        "%kind".to_string(),
        Value::Atom(AtomKind::Tag("foo".to_string()), EffectTag::Pure, None),
    );
    let cv1 = ComboVal::new(f1, false, IndexMap::new(), EffectTag::Pure, vec![]);

    let mut f2 = IndexMap::new();
    f2.insert(
        "x".to_string(),
        Value::Atom(AtomKind::Int(2.into()), EffectTag::Pure, None),
    );
    f2.insert(
        "%kind".to_string(),
        Value::Atom(AtomKind::Tag("bar".to_string()), EffectTag::Pure, None),
    );
    let cv2 = ComboVal::new(f2, false, IndexMap::new(), EffectTag::Pure, vec![]);

    let advertise_fn = oo.builtin_registry.get("disc.advertise").unwrap();
    advertise_fn(Value::Combo(cv1), &oo, &mut ctx);
    advertise_fn(Value::Combo(cv2), &oo, &mut ctx);

    let reg = oo.gbb_registry.read().unwrap();
    let masas: Vec<_> = reg
        .values()
        .filter(|gbb| !gbb.nerve_structure.is_empty())
        .map(|gbb| gbb.nerve_structure[0].masa_caid.clone())
        .collect();

    assert!(
        masas.len() >= 2,
        "should have at least 2 advertised nodes with nerve"
    );
    let first = &masas[0];
    assert!(
        masas.iter().all(|m| m == first),
        "same data-field structure -> same MASA regardless of %kind: {:?}",
        masas
    );
}

#[test]
fn test_list_node_nerve_uses_index_keys() {
    use indexmap::IndexMap;
    use nlang_interpreter::value::{ComboVal, EffectTag};
    use nlang_interpreter::*;
    use nlang_parser::ast::AtomKind;
    use std::sync::Arc;

    let oo = Arc::new(Ouroboros::new_in_memory());
    let mut ctx = EvalContext::new(oo.root_with_system());

    let mut fields = IndexMap::new();
    fields.insert(
        "%kind".to_string(),
        Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
    );
    fields.insert(
        "0".to_string(),
        Value::Atom(AtomKind::Int(10.into()), EffectTag::Pure, None),
    );
    fields.insert(
        "1".to_string(),
        Value::Atom(AtomKind::Int(20.into()), EffectTag::Pure, None),
    );
    let cv = ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]);

    let advertise_fn = oo.builtin_registry.get("disc.advertise").unwrap();
    advertise_fn(Value::Combo(cv), &oo, &mut ctx);

    let reg = oo.gbb_registry.read().unwrap();
    let nerve_keys: Vec<_> = reg
        .values()
        .filter(|gbb| !gbb.nerve_structure.is_empty())
        .flat_map(|gbb| gbb.nerve_structure[0].field_keys.iter().cloned())
        .collect();

    assert!(
        nerve_keys.contains(&"0".to_string()),
        "list nerve should have key '0'"
    );
    assert!(
        nerve_keys.contains(&"1".to_string()),
        "list nerve should have key '1'"
    );
    assert!(
        !nerve_keys.iter().any(|k| k.starts_with('%')),
        "list nerve must not contain %kind: {:?}",
        nerve_keys
    );
}

#[test]
fn test_empty_after_filter_is_transparent() {
    use nlang_interpreter::ladd::{nerve_overlap, NerveEntry, GBB};
    use nlang_interpreter::value::{CaidVersion, ContentHash, HashAlgorithm, MasaRef};

    let dummy = ContentHash {
        algorithm: HashAlgorithm::Sha256,
        version: CaidVersion::V2,
        masa_ref: MasaRef::Top,
        lattice_sketch: String::new(),
        digest: vec![0; 32],
    };

    let empty_nerve = GBB {
        node_caid: dummy.clone(),
        mass: 1.0,
        sketch_bytes: vec![],
        masa_ref: MasaRef::Top,
        nerve_structure: vec![],
    };
    let has_nerve = GBB {
        node_caid: dummy,
        mass: 1.0,
        sketch_bytes: vec![],
        masa_ref: MasaRef::Top,
        nerve_structure: vec![NerveEntry {
            masa_caid: "masa:fk:abc".into(),
            overlapping_masa_caids: vec![],
            field_keys: vec!["x".into()],
        }],
    };

    assert!(
        nerve_overlap(&empty_nerve, &has_nerve),
        "empty nerve passes through"
    );
    assert!(
        nerve_overlap(&has_nerve, &empty_nerve),
        "symmetric: empty nerve passes through"
    );
}
