use indexmap::IndexMap;
use nlang_interpreter::value::{ComboVal, EffectTag, Value};
use nlang_interpreter::*;
use nlang_parser::ast::AtomKind;

#[test]
fn eml_branch_0_principal() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let builtins = &oo.builtin_registry;
    let eml_fn = builtins.get("math.eml").unwrap();
    let mut fields = IndexMap::new();
    fields.insert(
        "0".to_string(),
        Value::Atom(AtomKind::Int(0.into()), EffectTag::Pure, None),
    );
    fields.insert(
        "1".to_string(),
        Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None),
    );
    fields.insert(
        "%branch".to_string(),
        Value::Atom(AtomKind::Int(0.into()), EffectTag::Pure, None),
    );
    let arg = Value::Combo(ComboVal::new(
        fields,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));
    let result = eml_fn(arg, &oo, &mut ctx);
    if let Value::Atom(AtomKind::Complex(r, i), _, _) = result {
        assert!((r - 1.0).abs() < 1e-10);
        assert!(i.abs() < 1e-10);
    } else {
        panic!("Expected Complex, got {:?}", result);
    }
}

#[test]
fn eml_branch_1_shifts_imag() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let builtins = &oo.builtin_registry;
    let eml_fn = builtins.get("math.eml").unwrap();
    let mut fields = IndexMap::new();
    fields.insert(
        "0".to_string(),
        Value::Atom(AtomKind::Int(0.into()), EffectTag::Pure, None),
    );
    fields.insert(
        "1".to_string(),
        Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None),
    );
    fields.insert(
        "%branch".to_string(),
        Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None),
    );
    let arg = Value::Combo(ComboVal::new(
        fields,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));
    let result = eml_fn(arg, &oo, &mut ctx);
    let two_pi = 2.0 * std::f64::consts::PI;
    if let Value::Atom(AtomKind::Complex(r, i), _, _) = result {
        assert!((r - 1.0).abs() < 1e-10);
        assert!((i + two_pi).abs() < 1e-10, "Expected -2π, got {}", i);
    } else {
        panic!("Expected Complex, got {:?}", result);
    }
}
