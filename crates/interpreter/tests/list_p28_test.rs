use indexmap::IndexMap;
use nlang_interpreter::value::{ComboVal, EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

fn make_oo() -> Ouroboros {
    Ouroboros::new_in_memory()
}

fn int(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}
fn make_list(items: Vec<Value>) -> Value {
    let mut m = IndexMap::new();
    for (i, v) in items.into_iter().enumerate() {
        m.insert(i.to_string(), v);
    }
    m.insert(
        "%kind".to_string(),
        Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
    );
    Value::Combo(ComboVal::new(
        m,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}
fn morph(builtin: &str) -> Value {
    let mut m = IndexMap::new();
    m.insert(
        "%morphism".to_string(),
        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
    );
    m.insert(
        "%builtin".to_string(),
        Value::Atom(AtomKind::Str(builtin.to_string()), EffectTag::Pure, None),
    );
    Value::Combo(ComboVal::new(
        m,
        true,
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
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn list_len(v: &Value) -> usize {
    match v {
        Value::Combo(c) => c
            .fields()
            .keys()
            .filter(|k| k.parse::<usize>().is_ok())
            .count(),
        _ => panic!("expected list, got {:?}", v),
    }
}
fn list_at(v: &Value, i: usize) -> &Value {
    match v {
        Value::Combo(c) => c.get_field(&i.to_string()).expect("index out of bounds"),
        _ => panic!("expected list"),
    }
}

// ── list.group_by ─────────────────────────────────────────────────

#[test]
fn test_list_group_by_sign() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int(-2), int(0), int(3)]);
    let r = call(
        &oo,
        &mut ctx,
        "list.group_by",
        combo2(morph("math.sign"), list),
    );
    if let Value::Combo(c) = &r {
        assert!(c.get_field("-1").is_some(), "missing '-1' group");
        assert!(c.get_field("0").is_some(), "missing '0' group");
        assert!(c.get_field("1").is_some(), "missing '1' group");
        assert_eq!(list_len(c.get_field("-1").unwrap()), 1);
        assert_eq!(list_len(c.get_field("0").unwrap()), 1);
        assert_eq!(list_len(c.get_field("1").unwrap()), 1);
    } else {
        panic!("expected Combo, got {:?}", r);
    }
}

#[test]
fn test_list_group_by_all_same_key() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int(1), int(2), int(3)]);
    let r = call(
        &oo,
        &mut ctx,
        "list.group_by",
        combo2(morph("math.sign"), list),
    );
    if let Value::Combo(c) = &r {
        assert_eq!(list_len(c.get_field("1").unwrap()), 3);
        assert!(c.get_field("-1").is_none());
        assert!(c.get_field("0").is_none());
    } else {
        panic!("expected Combo");
    }
}

#[test]
fn test_list_group_by_empty() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "list.group_by",
        combo2(morph("math.sign"), make_list(vec![])),
    );
    if let Value::Combo(c) = &r {
        assert!(c.fields().is_empty() || c.fields().keys().all(|k| k.starts_with('%')));
    } else {
        panic!("expected Combo");
    }
}

// ── list.chunk ────────────────────────────────────────────────────

#[test]
fn test_list_chunk_even() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int(1), int(2), int(3), int(4)]);
    let r = call(&oo, &mut ctx, "list.chunk", combo2(int(2), list));
    assert_eq!(list_len(&r), 2, "expected 2 chunks");
    assert_eq!(list_len(list_at(&r, 0)), 2);
    assert_eq!(list_len(list_at(&r, 1)), 2);
}

#[test]
fn test_list_chunk_with_remainder() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int(1), int(2), int(3), int(4), int(5)]);
    let r = call(&oo, &mut ctx, "list.chunk", combo2(int(2), list));
    assert_eq!(list_len(&r), 3, "expected 3 chunks");
    assert_eq!(list_len(list_at(&r, 0)), 2);
    assert_eq!(list_len(list_at(&r, 1)), 2);
    assert_eq!(list_len(list_at(&r, 2)), 1);
}

#[test]
fn test_list_chunk_larger_than_list() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int(1), int(2), int(3)]);
    let r = call(&oo, &mut ctx, "list.chunk", combo2(int(10), list));
    assert_eq!(list_len(&r), 1);
    assert_eq!(list_len(list_at(&r, 0)), 3);
}

#[test]
fn test_list_chunk_empty() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "list.chunk",
        combo2(int(3), make_list(vec![])),
    );
    assert_eq!(list_len(&r), 0);
}

// ── list.window ───────────────────────────────────────────────────

#[test]
fn test_list_window_basic() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int(1), int(2), int(3), int(4)]);
    let r = call(&oo, &mut ctx, "list.window", combo2(int(2), list));
    assert_eq!(list_len(&r), 3, "expected 3 windows");
    let w0 = list_at(&r, 0);
    assert_eq!(list_at(w0, 0).to_string_plain(), "1");
    assert_eq!(list_at(w0, 1).to_string_plain(), "2");
    let w2 = list_at(&r, 2);
    assert_eq!(list_at(w2, 0).to_string_plain(), "3");
    assert_eq!(list_at(w2, 1).to_string_plain(), "4");
}

#[test]
fn test_list_window_size_equals_list_len() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int(1), int(2), int(3)]);
    let r = call(&oo, &mut ctx, "list.window", combo2(int(3), list));
    assert_eq!(list_len(&r), 1);
    assert_eq!(list_len(list_at(&r, 0)), 3);
}

#[test]
fn test_list_window_larger_than_list() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int(1), int(2), int(3)]);
    let r = call(&oo, &mut ctx, "list.window", combo2(int(5), list));
    assert_eq!(list_len(&r), 0);
}
