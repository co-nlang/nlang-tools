use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, BottomCause, ComboVal};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use num_bigint::BigInt;

fn oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}
fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}
fn tag(t: &str) -> Value {
    Value::Atom(AtomKind::Tag(t.to_string()), EffectTag::Pure, None)
}

fn combo(pairs: &[(&str, Value)]) -> Value {
    let mut m = IndexMap::new();
    for (k, v) in pairs { m.insert(k.to_string(), v.clone()); }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn list_of(items: &[Value]) -> Value {
    let mut m = IndexMap::new();
    m.insert("%kind".to_string(), tag("list"));
    for (i, v) in items.iter().enumerate() { m.insert(i.to_string(), v.clone()); }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}

fn args2(a: Value, b: Value) -> Value {
    combo(&[("0", a), ("1", b)])
}

// ─── query.select ─────────────────────────────────────────────────────────────

#[test]
fn test_select_top_level_field() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let val = combo(&[("name", str_val("Alice")), ("age", int_val(30))]);
    let result = call(&oo, &mut ctx, "query.select", args2(val, str_val("name")));
    assert!(matches!(&result, Value::Atom(AtomKind::Str(s), _, _) if s == "Alice"));
}

#[test]
fn test_select_nested_path() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let inner = combo(&[("city", str_val("Taipei"))]);
    let outer = combo(&[("address", inner)]);
    let result = call(&oo, &mut ctx, "query.select", args2(outer, str_val("address.city")));
    assert!(matches!(&result, Value::Atom(AtomKind::Str(s), _, _) if s == "Taipei"));
}

#[test]
fn test_select_missing_path_returns_missing_key() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let val = combo(&[("x", int_val(1))]);
    let result = call(&oo, &mut ctx, "query.select", args2(val, str_val("y.z")));
    assert!(matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::MissingKey)));
}

#[test]
fn test_select_list_index() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let lst = list_of(&[int_val(10), int_val(20), int_val(30)]);
    let container = combo(&[("items", lst)]);
    let result = call(&oo, &mut ctx, "query.select", args2(container, str_val("items.1")));
    assert!(matches!(&result, Value::Atom(AtomKind::Int(n), _, _) if n == &BigInt::from(20i64)));
}

// ─── query.pluck ──────────────────────────────────────────────────────────────

#[test]
fn test_pluck_extracts_specified_fields() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let val = combo(&[("a", int_val(1)), ("b", int_val(2)), ("c", int_val(3))]);
    let keys = list_of(&[str_val("a"), str_val("c")]);
    let result = call(&oo, &mut ctx, "query.pluck", args2(val, keys));
    if let Value::Combo(ref cv) = result {
        assert!(cv.get_field("a").is_some(), "should have field a");
        assert!(cv.get_field("b").is_none(), "should not have field b");
        assert!(cv.get_field("c").is_some(), "should have field c");
    } else { panic!("expected Combo, got {:?}", result); }
}

// ─── query.deep_merge ─────────────────────────────────────────────────────────

#[test]
fn test_deep_merge_combines_disjoint_fields() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let a = combo(&[("x", int_val(1))]);
    let b = combo(&[("y", int_val(2))]);
    let result = call(&oo, &mut ctx, "query.deep_merge", args2(a, b));
    if let Value::Combo(ref cv) = result {
        assert!(cv.get_field("x").is_some());
        assert!(cv.get_field("y").is_some());
    } else { panic!("expected Combo"); }
}

#[test]
fn test_deep_merge_recurses_nested_combos() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let a = combo(&[("nested", combo(&[("x", int_val(1))]))]);
    let b = combo(&[("nested", combo(&[("y", int_val(2))]))]);
    let result = call(&oo, &mut ctx, "query.deep_merge", args2(a, b));
    let nested = if let Value::Combo(ref cv) = result {
        cv.get_field("nested").cloned().expect("nested field")
    } else { panic!("expected Combo"); };
    if let Value::Combo(ref nc) = nested {
        assert!(nc.get_field("x").is_some(), "x from a");
        assert!(nc.get_field("y").is_some(), "y from b");
    } else { panic!("nested should be Combo"); }
}

// ─── query.where ──────────────────────────────────────────────────────────────

#[test]
fn test_where_empty_list_returns_empty() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let empty_list = list_of(&[]);
    let result = call(&oo, &mut ctx, "query.where", args2(empty_list, Value::Top));
    if let Value::Combo(ref cv) = result {
        assert!(cv.get_field("0").is_none(), "empty list should have no items");
    } else { panic!("expected Combo list, got {:?}", result); }
}
