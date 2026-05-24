use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn int(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
fn tag(t: &str) -> Value { Value::Atom(AtomKind::Tag(t.to_string()), EffectTag::Pure, None) }
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
fn as_str(v: &Value) -> &str {
    match v { Value::Atom(AtomKind::Str(s), _, _) => s, o => panic!("expected Str: {:?}", o) }
}
fn as_int(v: &Value) -> i64 {
    match v { Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(), o => panic!("expected Int: {:?}", o) }
}
fn is_none(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none")
}
fn list_len(v: &Value) -> usize {
    match v { Value::Combo(c) => c.fields_iter().filter(|(k,_)| k.parse::<usize>().is_ok()).count(), _ => panic!("not a list") }
}
fn list_str_at(v: &Value, i: usize) -> String {
    match v { Value::Combo(c) => as_str(c.get_field(&i.to_string()).unwrap()).to_string(), _ => panic!() }
}

// ── json.parse ─────────────────────────────────────────────────────

#[test]
fn test_json_parse_object() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "json.parse", combo1(str_val(r#"{"name":"Alice","age":30}"#)));
    if let Value::Combo(ref c) = r {
        assert_eq!(as_str(c.get_field("name").unwrap()), "Alice");
        assert_eq!(as_int(c.get_field("age").unwrap()), 30);
    } else { panic!("expected Combo, got {:?}", r); }
}

#[test]
fn test_json_parse_array() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "json.parse", combo1(str_val("[1,2,3]")));
    assert_eq!(list_len(&r), 3);
    if let Value::Combo(ref c) = r {
        assert_eq!(as_int(c.get_field("1").unwrap()), 2);
    }
}

#[test]
fn test_json_parse_primitives() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r_null  = call(&oo, &mut ctx, "json.parse", combo1(str_val("null")));
    let r_true  = call(&oo, &mut ctx, "json.parse", combo1(str_val("true")));
    let r_false = call(&oo, &mut ctx, "json.parse", combo1(str_val("false")));
    assert!(is_none(&r_null));
    assert!(matches!(r_true,  Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "true"));
    assert!(matches!(r_false, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "false"));
}

#[test]
fn test_json_parse_invalid_returns_none() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "json.parse", combo1(str_val("{bad json}")));
    assert!(is_none(&r));
}

// ── json.stringify ─────────────────────────────────────────────────

#[test]
fn test_json_stringify_int() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "json.stringify", combo1(int(42)));
    assert_eq!(as_str(&r), "42");
}

#[test]
fn test_json_stringify_list() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let mut m = IndexMap::new();
    m.insert("0".to_string(), int(1));
    m.insert("1".to_string(), int(2));
    m.insert("%kind".to_string(), tag("list"));
    let list = Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]));
    let r = call(&oo, &mut ctx, "json.stringify", combo1(list));
    assert_eq!(as_str(&r), "[1,2]");
}

// ── json.get ───────────────────────────────────────────────────────

#[test]
fn test_json_get_found() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let parsed = call(&oo, &mut ctx, "json.parse", combo1(str_val(r#"{"x":99}"#)));
    let r = call(&oo, &mut ctx, "json.get", combo2(str_val("x"), parsed));
    assert_eq!(as_int(&r), 99);
}

#[test]
fn test_json_get_not_found_returns_none() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let parsed = call(&oo, &mut ctx, "json.parse", combo1(str_val(r#"{"x":1}"#)));
    let r = call(&oo, &mut ctx, "json.get", combo2(str_val("missing"), parsed));
    assert!(is_none(&r));
}

// ── json.keys ──────────────────────────────────────────────────────

#[test]
fn test_json_keys() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let parsed = call(&oo, &mut ctx, "json.parse", combo1(str_val(r#"{"a":1,"b":2}"#)));
    let keys = call(&oo, &mut ctx, "json.keys", combo1(parsed));
    assert_eq!(list_len(&keys), 2);
    let k0 = list_str_at(&keys, 0);
    let k1 = list_str_at(&keys, 1);
    assert!(k0 == "a" || k0 == "b");
    assert!(k1 == "a" || k1 == "b");
    assert_ne!(k0, k1);
}
