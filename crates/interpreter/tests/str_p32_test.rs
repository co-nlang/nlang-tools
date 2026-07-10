use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn int(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
fn combo1(a: Value) -> Value {
    let mut m = IndexMap::new(); m.insert("0".to_string(), a);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn combo2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn combo3(a: Value, b: Value, c: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b); m.insert("2".to_string(), c);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn as_str(v: &Value) -> &str {
    match v { Value::Atom(AtomKind::Str(s), _, _) => s, o => panic!("expected Str: {:?}", o) }
}
fn as_int(v: &Value) -> i64 {
    match v { Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(), o => panic!("expected Int: {:?}", o) }
}
fn list_len(v: &Value) -> usize {
    match v { Value::Combo(c) => c.fields().keys().filter(|k| k.parse::<usize>().is_ok()).count(), _ => panic!("expected list") }
}
fn list_str_at(v: &Value, i: usize) -> &str {
    match v { Value::Combo(c) => as_str(c.get_field(&i.to_string()).expect("index")), _ => panic!() }
}

#[test]
fn test_str_reverse_ascii() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.reverse", combo1(str_val("hello")));
    assert_eq!(as_str(&r), "olleh");
}

#[test]
fn test_str_reverse_empty() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.reverse", combo1(str_val("")));
    assert_eq!(as_str(&r), "");
}

#[test]
fn test_str_count_occurrences() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.count", combo2(str_val("ab"), str_val("ababab")));
    assert_eq!(as_int(&r), 3);
}

#[test]
fn test_str_count_zero() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.count", combo2(str_val("xyz"), str_val("hello")));
    assert_eq!(as_int(&r), 0);
}

#[test]
fn test_str_slice_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.slice", combo3(int(1), int(4), str_val("hello")));
    assert_eq!(as_str(&r), "ell");
}

#[test]
fn test_str_slice_clamped() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.slice", combo3(int(2), int(100), str_val("hi")));
    assert_eq!(as_str(&r), "");
}

#[test]
fn test_str_is_empty_true() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.is_empty", combo1(str_val("")));
    assert!(matches!(r, Value::Atom(AtomKind::Tag(t), _, _) if t == "true"));
}

#[test]
fn test_str_lines_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.lines", combo1(str_val("a\nb\nc")));
    assert_eq!(list_len(&r), 3);
    assert_eq!(list_str_at(&r, 0), "a");
    assert_eq!(list_str_at(&r, 2), "c");
}
