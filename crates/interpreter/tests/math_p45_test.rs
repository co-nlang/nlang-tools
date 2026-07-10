use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use indexmap::IndexMap;
use nlang_interpreter::value::ComboVal;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn int(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
fn float(n: f64) -> Value { Value::Atom(AtomKind::Float(n), EffectTag::Pure, None) }
fn combo1(a: Value) -> Value {
    let mut m = IndexMap::new(); m.insert("0".to_string(), a);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn combo2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn as_float(v: &Value) -> f64 {
    match v { Value::Atom(AtomKind::Float(f), _, _) => *f, o => panic!("expected Float: {:?}", o) }
}
fn as_int(v: &Value) -> i64 {
    match v { Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(), o => panic!("expected Int: {:?}", o) }
}

#[test]
fn test_math_atan2_hypot() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.atan2", combo2(int(0), int(1)));
    assert!((as_float(&r) - 0.0).abs() < 1e-10);
    let r = call(&oo, &mut ctx, "math.hypot", combo2(int(3), int(4)));
    assert!((as_float(&r) - 5.0).abs() < 1e-10);
}

#[test]
fn test_math_sinh_cosh_tanh() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.sinh", combo1(float(0.0)));
    assert!((as_float(&r) - 0.0).abs() < 1e-10);
    let r = call(&oo, &mut ctx, "math.cosh", combo1(float(0.0)));
    assert!((as_float(&r) - 1.0).abs() < 1e-10);
    let r = call(&oo, &mut ctx, "math.tanh", combo1(float(0.0)));
    assert!((as_float(&r) - 0.0).abs() < 1e-10);
}

#[test]
fn test_math_trunc_fract() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.trunc", combo1(float(3.7)));
    assert!((as_float(&r) - 3.0).abs() < 1e-10);
    let r = call(&oo, &mut ctx, "math.trunc", combo1(float(-3.7)));
    assert!((as_float(&r) - (-3.0)).abs() < 1e-10);
    let r = call(&oo, &mut ctx, "math.trunc", combo1(int(42)));
    assert_eq!(as_int(&r), 42);
    let r = call(&oo, &mut ctx, "math.fract", combo1(float(3.7)));
    assert!((as_float(&r) - 0.7).abs() < 1e-10);
}

#[test]
fn test_math_to_float() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.to_float", combo1(int(42)));
    assert!((as_float(&r) - 42.0).abs() < 1e-10);
    let r = call(&oo, &mut ctx, "math.to_float", combo1(float(3.14)));
    assert!((as_float(&r) - 3.14).abs() < 1e-10);
}

#[test]
fn test_math_hypot_with_negatives() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.hypot", combo2(int(-3), int(-4)));
    assert!((as_float(&r) - 5.0).abs() < 1e-10);
}
