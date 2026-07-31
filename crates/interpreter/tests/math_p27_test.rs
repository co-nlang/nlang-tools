use nlang_interpreter::value::{EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

fn make_oo() -> Ouroboros {
    Ouroboros::new_in_memory()
}
fn int(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}
fn float(f: f64) -> Value {
    Value::Atom(AtomKind::Float(f), EffectTag::Pure, None)
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn combo2(a: Value, b: Value) -> Value {
    use indexmap::IndexMap;
    use nlang_interpreter::value::ComboVal;
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a);
    m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(
        m,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}
fn as_int(v: &Value) -> i64 {
    match v {
        Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(),
        o => panic!("expected Int: {:?}", o),
    }
}
fn as_float(v: &Value) -> f64 {
    match v {
        Value::Atom(AtomKind::Float(f), _, _) => *f,
        o => panic!("expected Float: {:?}", o),
    }
}

#[test]
fn test_math_gcd_basic() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.gcd", combo2(int(12), int(8)));
    assert_eq!(as_int(&r), 4);
}

#[test]
fn test_math_gcd_zero() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.gcd", combo2(int(0), int(5)));
    assert_eq!(as_int(&r), 5);
}

#[test]
fn test_math_lcm_basic() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.lcm", combo2(int(4), int(6)));
    assert_eq!(as_int(&r), 12);
}

#[test]
fn test_math_lcm_with_zero() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.lcm", combo2(int(0), int(7)));
    assert_eq!(as_int(&r), 0);
}

#[test]
fn test_math_sign_positive() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    assert_eq!(as_int(&call(&oo, &mut ctx, "math.sign", int(42))), 1);
}

#[test]
fn test_math_sign_negative() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    assert_eq!(as_int(&call(&oo, &mut ctx, "math.sign", int(-7))), -1);
}

#[test]
fn test_math_sign_zero() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    assert_eq!(as_int(&call(&oo, &mut ctx, "math.sign", int(0))), 0);
}

#[test]
fn test_math_log2_basic() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.log2", float(8.0));
    let f = as_float(&r);
    assert!((f - 3.0).abs() < 1e-9, "log2(8) should be 3.0, got {}", f);
}

#[test]
fn test_math_log2_zero_is_blur() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.log2", float(0.0));
    assert!(matches!(r, Value::Blur(_)), "log2(0) should be Blur");
}

#[test]
fn test_math_log10_basic() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.log10", float(1000.0));
    let f = as_float(&r);
    assert!(
        (f - 3.0).abs() < 1e-9,
        "log10(1000) should be 3.0, got {}",
        f
    );
}

#[test]
fn test_math_log10_zero_is_blur() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.log10", float(0.0));
    assert!(matches!(r, Value::Blur(_)), "log10(0) should be Blur");
}
