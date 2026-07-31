use indexmap::IndexMap;
use nlang_interpreter::value::{ComboVal, EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

fn make_oo() -> Ouroboros {
    Ouroboros::new_in_memory()
}
fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}
fn float_val(f: f64) -> Value {
    Value::Atom(AtomKind::Float(f), EffectTag::Pure, None)
}

fn make_combo_2(a: Value, b: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), a);
    f.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(
        f,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}
fn make_combo_3(a: Value, b: Value, c: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), a);
    f.insert("1".to_string(), b);
    f.insert("2".to_string(), c);
    Value::Combo(ComboVal::new(
        f,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}

fn assert_float(v: &Value, expected: f64) {
    match v {
        Value::Atom(AtomKind::Float(f), _, _) => assert!(
            (f - expected).abs() < 1e-9,
            "expected {}, got {}",
            expected,
            f
        ),
        _ => panic!("expected Float, got {:?}", v),
    }
}
fn assert_int(v: &Value, expected: i64) {
    match v {
        Value::Atom(AtomKind::Int(n), _, _) => assert_eq!(n, &BigInt::from(expected)),
        _ => panic!("expected Int, got {:?}", v),
    }
}

#[test]
fn test_math_min_ints() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "math.min",
        make_combo_2(int_val(3), int_val(7)),
    );
    assert_int(&r, 3);
}

#[test]
fn test_math_max_floats() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "math.max",
        make_combo_2(float_val(1.5), float_val(2.5)),
    );
    assert_float(&r, 2.5);
}

#[test]
fn test_math_floor() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.floor", float_val(3.7));
    assert_float(&r, 3.0);
    let r2 = call(&oo, &mut ctx, "math.floor", float_val(-1.2));
    assert_float(&r2, -2.0);
}

#[test]
fn test_math_ceil() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.ceil", float_val(3.2));
    assert_float(&r, 4.0);
}

#[test]
fn test_math_round() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.round", float_val(3.5));
    assert_float(&r, 4.0);
    let r2 = call(&oo, &mut ctx, "math.round", float_val(3.4));
    assert_float(&r2, 3.0);
}

#[test]
fn test_math_clamp_in_range() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "math.clamp",
        make_combo_3(int_val(0), int_val(10), int_val(5)),
    );
    assert_int(&r, 5);
}

#[test]
fn test_math_clamp_below() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "math.clamp",
        make_combo_3(float_val(0.0), float_val(10.0), float_val(-3.0)),
    );
    assert_float(&r, 0.0);
}

#[test]
fn test_math_clamp_above() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "math.clamp",
        make_combo_3(float_val(0.0), float_val(10.0), float_val(15.0)),
    );
    assert_float(&r, 10.0);
}
