use indexmap::IndexMap;
use nlang_interpreter::value::{ComboVal, EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

fn make_oo() -> Ouroboros {
    Ouroboros::new_in_memory()
}
fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}
fn int(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
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
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn as_str(v: &Value) -> &str {
    match v {
        Value::Atom(AtomKind::Str(s), _, _) => s.as_str(),
        o => panic!("expected Str: {:?}", o),
    }
}
fn as_int(v: &Value) -> i64 {
    match v {
        Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(),
        o => panic!("expected Int: {:?}", o),
    }
}
fn is_none(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none")
}

#[test]
fn test_str_index_of_found() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "str.index_of",
        combo2(str_val("lo"), str_val("hello world")),
    );
    assert_eq!(as_int(&r), 3);
}

#[test]
fn test_str_index_of_not_found() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "str.index_of",
        combo2(str_val("xyz"), str_val("hello")),
    );
    assert!(is_none(&r));
}

#[test]
fn test_str_index_of_at_start() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "str.index_of",
        combo2(str_val("he"), str_val("hello")),
    );
    assert_eq!(as_int(&r), 0);
}

#[test]
fn test_str_pad_left_shorter() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.pad_left", combo2(int(6), str_val("hi")));
    assert_eq!(as_str(&r), "    hi");
}

#[test]
fn test_str_pad_left_already_wide() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "str.pad_left",
        combo2(int(2), str_val("hello")),
    );
    assert_eq!(as_str(&r), "hello");
}

#[test]
fn test_str_pad_right_basic() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "str.pad_right",
        combo2(int(5), str_val("hi")),
    );
    assert_eq!(as_str(&r), "hi   ");
}

#[test]
fn test_str_trim_start_basic() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.trim_start", str_val("   hello"));
    assert_eq!(as_str(&r), "hello");
}

#[test]
fn test_str_trim_end_basic() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.trim_end", str_val("hello   "));
    assert_eq!(as_str(&r), "hello");
}
