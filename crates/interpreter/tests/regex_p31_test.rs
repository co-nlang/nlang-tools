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
fn is_true(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "true")
}
fn is_false(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "false")
}
fn is_none(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none")
}
fn list_len(v: &Value) -> usize {
    match v {
        Value::Combo(c) => c
            .fields()
            .keys()
            .filter(|k| k.parse::<usize>().is_ok())
            .count(),
        _ => panic!("expected list"),
    }
}
fn list_str_at(v: &Value, i: usize) -> &str {
    match v {
        Value::Combo(c) => as_str(c.get_field(&i.to_string()).expect("index")),
        _ => panic!("expected list"),
    }
}

#[test]
fn test_regex_match_true() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "regex.match",
        combo2(str_val(r"\d+"), str_val("hello123")),
    );
    assert!(is_true(&r));
}

#[test]
fn test_regex_match_false() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "regex.match",
        combo2(str_val(r"^\d+$"), str_val("hello")),
    );
    assert!(is_false(&r));
}

#[test]
fn test_regex_match_invalid_pattern_returns_top() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "regex.match",
        combo2(str_val("[invalid"), str_val("test")),
    );
    assert!(matches!(r, Value::Top));
}

#[test]
fn test_regex_find_found() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "regex.find",
        combo2(str_val(r"\d+"), str_val("abc123def")),
    );
    if let Value::Combo(ref c) = r {
        assert_eq!(as_str(c.get_field("match").unwrap()), "123");
        assert_eq!(as_int(c.get_field("start").unwrap()), 3);
        assert_eq!(as_int(c.get_field("end").unwrap()), 6);
    } else {
        panic!("expected Combo, got {:?}", r);
    }
}

#[test]
fn test_regex_find_not_found() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "regex.find",
        combo2(str_val(r"\d+"), str_val("hello")),
    );
    assert!(is_none(&r));
}

#[test]
fn test_regex_replace_all() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "regex.replace",
        combo3(str_val(r"\d+"), str_val("N"), str_val("a1 b22 c3")),
    );
    assert_eq!(as_str(&r), "aN bN cN");
}

#[test]
fn test_regex_replace_no_match_unchanged() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "regex.replace",
        combo3(str_val(r"\d+"), str_val("N"), str_val("hello")),
    );
    assert_eq!(as_str(&r), "hello");
}

#[test]
fn test_regex_split_whitespace() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "regex.split",
        combo2(str_val(r"\s+"), str_val("hello world")),
    );
    assert_eq!(list_len(&r), 2);
    assert_eq!(list_str_at(&r, 0), "hello");
    assert_eq!(list_str_at(&r, 1), "world");
}

#[test]
fn test_regex_split_comma() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "regex.split",
        combo2(str_val(r",\s*"), str_val("a, b, c")),
    );
    assert_eq!(list_len(&r), 3);
    assert_eq!(list_str_at(&r, 0), "a");
    assert_eq!(list_str_at(&r, 1), "b");
    assert_eq!(list_str_at(&r, 2), "c");
}
