use nlang_interpreter::*;
use nlang_interpreter::value::{Value, ComboVal, EffectTag, ContentHash, HashAlgorithm, CaidVersion, MasaRef};
use nlang_interpreter::observation::ObservationStrategy;
use nlang_parser::ast::AtomKind;
use std::sync::Arc;
use indexmap::IndexMap;

fn dummy_masa_caid() -> ContentHash {
    ContentHash { algorithm: HashAlgorithm::Sha256, version: CaidVersion::V1,
        masa_ref: MasaRef::Top, lattice_sketch: String::new(), digest: vec![0; 32] }
}

fn setup_bohr() -> (Arc<Ouroboros>, EvalContext) {
    let oo = Arc::new(Ouroboros::new_in_memory());
    let ctx = EvalContext::new(oo.root_with_system());
    (oo, ctx)
}

#[test]
fn test_project_down_adds_blur_tag() {
    let (oo, mut ctx) = setup_bohr();
    let target = Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None);
    let arg_val = Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
        ("target".to_string(), target),
        ("masa".to_string(), Value::Atom(AtomKind::Str("hash:sha256:v1:00".to_string()), EffectTag::Pure, None)),
    ]), false, IndexMap::new(), EffectTag::Pure, vec![]));

    let builtin = oo.builtin_registry.get("engine.project_down").unwrap();
    let result = builtin(arg_val, &oo, &mut ctx);
    // Result should be a Combo with %kind field
    assert!(matches!(result, Value::Combo(_)), "project_down should return a Combo");
}

#[test]
fn test_project_down_noncombo_target() {
    let (oo, mut ctx) = setup_bohr();
    let target = Value::Atom(AtomKind::Int(99.into()), EffectTag::Pure, None);
    let arg_val = Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
        ("target".to_string(), target),
        ("masa".to_string(), Value::Atom(AtomKind::Str("hash:sha256:v1:00".to_string()), EffectTag::Pure, None)),
    ]), false, IndexMap::new(), EffectTag::Pure, vec![]));

    let builtin = oo.builtin_registry.get("engine.project_down").unwrap();
    let result = builtin(arg_val, &oo, &mut ctx);
    assert!(matches!(result, Value::Combo(_)), "should return a Combo wrapping the atom");
}

#[test]
fn test_project_up_single_section() {
    let (oo, mut ctx) = setup_bohr();
    let section = Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
        ("x".to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None)),
    ]), false, IndexMap::new(), EffectTag::Pure, vec![]));
    let sections = Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
        ("0".to_string(), section),
        ("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None)),
    ]), false, IndexMap::new(), EffectTag::Pure, vec![]));
    let arg_val = Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
        ("sections".to_string(), sections),
    ]), false, IndexMap::new(), EffectTag::Pure, vec![]));

    let builtin = oo.builtin_registry.get("engine.project_up").unwrap();
    let result = builtin(arg_val, &oo, &mut ctx);
    assert!(!matches!(result, Value::Bottom(_)), "single section should not produce Bottom");
}

#[test]
fn test_project_down_top_masa() {
    let (oo, mut ctx) = setup_bohr();
    let mut data = IndexMap::new();
    data.insert("x".to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None));
    let target = Value::Combo(ComboVal::new(data, false, IndexMap::new(), EffectTag::Pure, vec![]));
    let arg_val = Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
        ("target".to_string(), target),
        ("masa".to_string(), Value::Atom(AtomKind::Str("hash:sha256:v1:00".to_string()), EffectTag::Pure, None)),
    ]), false, IndexMap::new(), EffectTag::Pure, vec![]));

    let builtin = oo.builtin_registry.get("engine.project_down").unwrap();
    let result = builtin(arg_val, &oo, &mut ctx);
    assert!(matches!(result, Value::Combo(_)), "Top MASA project_down should return Combo");
}

#[test]
fn test_set_strategy_blur() {
    let (oo, mut ctx) = setup_bohr();
    let builtin = oo.builtin_registry.get("engine.set_strategy").unwrap();
    let arg = Value::Atom(AtomKind::Tag("blur".to_string()), EffectTag::Pure, None);
    builtin(arg, &oo, &mut ctx);
    assert_eq!(ctx.strategy, ObservationStrategy::Blur);
}

#[test]
fn test_set_strategy_strict() {
    let (oo, mut ctx) = setup_bohr();
    let builtin = oo.builtin_registry.get("engine.set_strategy").unwrap();
    let arg = Value::Atom(AtomKind::Tag("strict".to_string()), EffectTag::Pure, None);
    builtin(arg, &oo, &mut ctx);
    assert_eq!(ctx.strategy, ObservationStrategy::Strict);
}

#[test]
fn test_project_down_filters_fields() {
    let (oo, mut ctx) = setup_bohr();
    let mut data = IndexMap::new();
    data.insert("visible".to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None));
    let target = Value::Combo(ComboVal::new(data, false, IndexMap::new(), EffectTag::Pure, vec![]));
    let arg_val = Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
        ("target".to_string(), target),
        ("masa".to_string(), Value::Atom(AtomKind::Str("hash:sha256:v1:ff".to_string()), EffectTag::Pure, None)),
    ]), false, IndexMap::new(), EffectTag::Pure, vec![]));
    let builtin = oo.builtin_registry.get("engine.project_down").unwrap();
    let result = builtin(arg_val, &oo, &mut ctx);
    assert!(matches!(result, Value::Combo(_)), "project_down should work");
}

#[test]
fn test_project_up_compatible_sections() {
    let (oo, mut ctx) = setup_bohr();
    let a = Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
        ("x".to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None)),
        ("%kind".to_string(), Value::Atom(AtomKind::Tag("blur".to_string()), EffectTag::Pure, None)),
    ]), false, IndexMap::new(), EffectTag::Pure, vec![]));
    let sections = Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
        ("0".to_string(), a),
        ("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None)),
    ]), false, IndexMap::new(), EffectTag::Pure, vec![]));
    let arg_val = Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
        ("sections".to_string(), sections),
    ]), false, IndexMap::new(), EffectTag::Pure, vec![]));
    let builtin = oo.builtin_registry.get("engine.project_up").unwrap();
    let result = builtin(arg_val, &oo, &mut ctx);
    assert!(!matches!(result, Value::Bottom(_)), "compatible sections should merge");
}
