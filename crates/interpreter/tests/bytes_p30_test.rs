use indexmap::IndexMap;
use nlang_interpreter::value::{ComboVal, EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

fn make_oo() -> Ouroboros {
    Ouroboros::new_in_memory()
}

fn bytes_val(v: Vec<u8>) -> Value {
    Value::Atom(AtomKind::Bytes(v), EffectTag::Pure, None)
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
fn as_bytes(v: &Value) -> &[u8] {
    match v {
        Value::Atom(AtomKind::Bytes(b), _, _) => b,
        o => panic!("expected Bytes: {:?}", o),
    }
}
fn as_str(v: &Value) -> &str {
    match v {
        Value::Atom(AtomKind::Str(s), _, _) => s,
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
fn test_bytes_from_str_and_len() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let b = call(&oo, &mut ctx, "bytes.from_str", str_val("hello"));
    assert_eq!(as_bytes(&b), b"hello");
    let l = call(&oo, &mut ctx, "bytes.len", b);
    assert_eq!(as_int(&l), 5);
}

#[test]
fn test_bytes_to_str_roundtrip() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let b = call(&oo, &mut ctx, "bytes.from_str", str_val("nlang"));
    let s = call(&oo, &mut ctx, "bytes.to_str", b);
    assert_eq!(as_str(&s), "nlang");
}

#[test]
fn test_bytes_to_str_invalid_utf8_returns_none() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let b = bytes_val(vec![0xFF, 0xFE]);
    let r = call(&oo, &mut ctx, "bytes.to_str", b);
    assert!(is_none(&r), "invalid UTF-8 should return #none");
}

#[test]
fn test_bytes_at_basic() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let b = bytes_val(vec![10, 20, 30]);
    let r = call(&oo, &mut ctx, "bytes.at", combo2(int(1), b));
    assert_eq!(as_int(&r), 20);
}

#[test]
fn test_bytes_at_out_of_range_returns_top() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let b = bytes_val(vec![1, 2, 3]);
    let r = call(&oo, &mut ctx, "bytes.at", combo2(int(5), b));
    assert!(matches!(r, Value::Top));
}

#[test]
fn test_bytes_concat() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let a = bytes_val(vec![1, 2]);
    let b = bytes_val(vec![3, 4]);
    let r = call(&oo, &mut ctx, "bytes.concat", combo2(a, b));
    assert_eq!(as_bytes(&r), &[1u8, 2, 3, 4]);
}

#[test]
fn test_bytes_slice() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let b = bytes_val(vec![10, 20, 30, 40, 50]);
    let r = call(&oo, &mut ctx, "bytes.slice", combo3(int(1), int(4), b));
    assert_eq!(as_bytes(&r), &[20u8, 30, 40]);
}

#[test]
fn test_bytes_to_hex() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let b = bytes_val(vec![0xDE, 0xAD, 0xBE, 0xEF]);
    let r = call(&oo, &mut ctx, "bytes.to_hex", b);
    assert_eq!(as_str(&r), "deadbeef");
}

#[test]
fn test_bytes_from_hex_valid() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "bytes.from_hex", str_val("deadbeef"));
    assert_eq!(as_bytes(&r), &[0xDEu8, 0xAD, 0xBE, 0xEF]);
}

#[test]
fn test_bytes_from_hex_invalid_returns_none() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "bytes.from_hex", str_val("xyz!"));
    assert!(is_none(&r));
}
