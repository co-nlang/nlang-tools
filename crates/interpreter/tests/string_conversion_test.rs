use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn int_val(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
fn float_val(f: f64) -> Value { Value::Atom(AtomKind::Float(f), EffectTag::Pure, None) }

fn make_combo_2(a: Value, b: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), a); f.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}

#[test]
fn test_str_parse_int_ok() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.parse_int", str_val("42"));
    match r { Value::Atom(AtomKind::Int(n), _, _) => assert_eq!(n, BigInt::from(42)), _ => panic!("expected Int") }
}

#[test]
fn test_str_parse_int_negative() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.parse_int", str_val("-7"));
    match r { Value::Atom(AtomKind::Int(n), _, _) => assert_eq!(n, BigInt::from(-7)), _ => panic!("expected Int") }
}

#[test]
fn test_str_parse_int_invalid() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.parse_int", str_val("not_a_number"));
    assert!(matches!(r, Value::Bottom(_)), "expected Bottom on invalid parse");
}

#[test]
fn test_str_from_int() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.from_int", int_val(99));
    match r { Value::Atom(AtomKind::Str(s), _, _) => assert_eq!(s, "99"), _ => panic!("expected Str") }
}

#[test]
fn test_str_from_int_float() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.from_int", float_val(3.14));
    match r { Value::Atom(AtomKind::Str(s), _, _) => assert!(s.contains("3.14"), "got: {}", s), _ => panic!("expected Str") }
}

#[test]
fn test_str_repeat() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.repeat", make_combo_2(int_val(3), str_val("ab")));
    match r { Value::Atom(AtomKind::Str(s), _, _) => assert_eq!(s, "ababab"), _ => panic!("expected Str") }
}
