use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn int_val(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
fn make_combo2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn as_str(v: &Value) -> String {
    match v { Value::Atom(AtomKind::Str(s), _, _) => s.clone(), other => panic!("expected Str, got {:?}", other) }
}

#[test]
fn test_str_char_at_first() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.char_at", make_combo2(int_val(0), str_val("hello")));
    assert_eq!(as_str(&r), "h");
}

#[test]
fn test_str_char_at_last() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.char_at", make_combo2(int_val(4), str_val("hello")));
    assert_eq!(as_str(&r), "o");
}

#[test]
fn test_str_char_at_oob_returns_top() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.char_at", make_combo2(int_val(5), str_val("hello")));
    assert!(matches!(r, Value::Top));
}

#[test]
fn test_str_chars_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.chars", str_val("hi!"));
    if let Value::Combo(c) = &r {
        assert_eq!(c.get_field("0").unwrap().to_string_plain(), "h");
        assert_eq!(c.get_field("1").unwrap().to_string_plain(), "i");
        assert_eq!(c.get_field("2").unwrap().to_string_plain(), "!");
    } else { panic!("expected list"); }
}

#[test]
fn test_str_chars_empty() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.chars", str_val(""));
    if let Value::Combo(c) = &r {
        assert_eq!(c.fields().keys().filter(|k| k.parse::<usize>().is_ok()).count(), 0);
    } else { panic!("expected empty list"); }
}
