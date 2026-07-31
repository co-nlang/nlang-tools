use indexmap::IndexMap;
use nlang_interpreter::value::ComboVal;
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
fn combo1(a: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a);
    Value::Combo(ComboVal::new(
        m,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}
fn combo2(a: Value, b: Value) -> Value {
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
fn combo3(a: Value, b: Value, c: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a);
    m.insert("1".to_string(), b);
    m.insert("2".to_string(), c);
    Value::Combo(ComboVal::new(
        m,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn as_int(v: &Value) -> i64 {
    match v {
        Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(),
        o => panic!("{:?}", o),
    }
}
fn is_bottom(v: &Value) -> bool {
    matches!(v, Value::Bottom(_))
}
fn is_true(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "true")
}
fn is_false(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "false")
}

#[test]
fn test_math_factorial_basic() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    assert_eq!(
        as_int(&call(&oo, &mut ctx, "math.factorial", combo1(int(5)))),
        120
    );
    assert_eq!(
        as_int(&call(&oo, &mut ctx, "math.factorial", combo1(int(0)))),
        1
    );
    assert_eq!(
        as_int(&call(&oo, &mut ctx, "math.factorial", combo1(int(1)))),
        1
    );
}

#[test]
fn test_math_factorial_negative_is_bottom() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    assert!(is_bottom(&call(
        &oo,
        &mut ctx,
        "math.factorial",
        combo1(int(-1))
    )));
}

#[test]
fn test_math_choose_basic() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    assert_eq!(
        as_int(&call(&oo, &mut ctx, "math.choose", combo2(int(5), int(2)))),
        10
    );
    assert_eq!(
        as_int(&call(&oo, &mut ctx, "math.choose", combo2(int(5), int(0)))),
        1
    );
    assert_eq!(
        as_int(&call(&oo, &mut ctx, "math.choose", combo2(int(5), int(5)))),
        1
    );
    assert_eq!(
        as_int(&call(&oo, &mut ctx, "math.choose", combo2(int(3), int(5)))),
        0
    );
}

#[test]
fn test_math_is_prime() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    assert!(is_true(&call(
        &oo,
        &mut ctx,
        "math.is_prime",
        combo1(int(2))
    )));
    assert!(is_true(&call(
        &oo,
        &mut ctx,
        "math.is_prime",
        combo1(int(7))
    )));
    assert!(is_true(&call(
        &oo,
        &mut ctx,
        "math.is_prime",
        combo1(int(97))
    )));
    assert!(is_false(&call(
        &oo,
        &mut ctx,
        "math.is_prime",
        combo1(int(1))
    )));
    assert!(is_false(&call(
        &oo,
        &mut ctx,
        "math.is_prime",
        combo1(int(9))
    )));
    assert!(is_false(&call(
        &oo,
        &mut ctx,
        "math.is_prime",
        combo1(int(100))
    )));
}

#[test]
fn test_math_pow_mod_basic() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    assert_eq!(
        as_int(&call(
            &oo,
            &mut ctx,
            "math.pow_mod",
            combo3(int(2), int(10), int(1000))
        )),
        24
    );
    assert_eq!(
        as_int(&call(
            &oo,
            &mut ctx,
            "math.pow_mod",
            combo3(int(3), int(0), int(7))
        )),
        1
    );
}

#[test]
fn test_math_pow_mod_invalid_is_bottom() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    assert!(is_bottom(&call(
        &oo,
        &mut ctx,
        "math.pow_mod",
        combo3(int(2), int(-1), int(7))
    )));
    assert!(is_bottom(&call(
        &oo,
        &mut ctx,
        "math.pow_mod",
        combo3(int(2), int(3), int(0))
    )));
}
