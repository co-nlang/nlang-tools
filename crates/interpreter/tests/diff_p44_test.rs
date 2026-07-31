use indexmap::IndexMap;
use nlang_interpreter::value::{BottomCause, ComboVal, EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

fn oo() -> Ouroboros {
    Ouroboros::new_in_memory()
}

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
    for (k, v) in pairs {
        m.insert(k.to_string(), v.clone());
    }
    Value::Combo(ComboVal::new(
        m,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

fn list_of(items: &[Value]) -> Value {
    let mut m = IndexMap::new();
    m.insert("%kind".to_string(), tag("list"));
    for (i, v) in items.iter().enumerate() {
        m.insert(i.to_string(), v.clone());
    }
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

fn args2(a: Value, b: Value) -> Value {
    combo(&[("0", a), ("1", b)])
}

fn list_len(v: &Value) -> usize {
    if let Value::Combo(c) = v {
        (0u32..)
            .take_while(|i| c.get_field(&i.to_string()).is_some())
            .count()
    } else {
        0
    }
}

// ─── diff.diff ────────────────────────────────────────────────────────────────

#[test]
fn test_diff_identical_returns_empty() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let val = combo(&[("x", int_val(1)), ("y", str_val("hello"))]);
    let result = call(&oo, &mut ctx, "diff.diff", args2(val.clone(), val));
    assert_eq!(list_len(&result), 0, "identical values → empty diff");
}

#[test]
fn test_diff_changed_leaf() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let a = combo(&[("x", int_val(1))]);
    let b = combo(&[("x", int_val(2))]);
    let result = call(&oo, &mut ctx, "diff.diff", args2(a, b));
    assert_eq!(list_len(&result), 1, "one changed field → one diff entry");
    if let Value::Combo(rc) = &result {
        if let Some(entry) = rc.get_field("0") {
            if let Value::Combo(ec) = entry {
                let path = ec.get_field("path").expect("diff entry has path");
                assert!(matches!(path, Value::Atom(AtomKind::Str(s), _, _) if s == "x"));
            }
        }
    }
}

#[test]
fn test_diff_added_field() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let a = combo(&[("x", int_val(1))]);
    let b = combo(&[("x", int_val(1)), ("y", int_val(2))]);
    let result = call(&oo, &mut ctx, "diff.diff", args2(a, b));
    assert_eq!(list_len(&result), 1);
    if let Value::Combo(rc) = &result {
        if let Some(entry) = rc.get_field("0") {
            if let Value::Combo(ec) = entry {
                let from = ec.get_field("from").expect("has from");
                assert!(
                    matches!(from, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::MissingKey))
                );
            }
        }
    }
}

#[test]
fn test_diff_nested_change() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let a = combo(&[("nested", combo(&[("val", int_val(10))]))]);
    let b = combo(&[("nested", combo(&[("val", int_val(99))]))]);
    let result = call(&oo, &mut ctx, "diff.diff", args2(a, b));
    assert_eq!(list_len(&result), 1);
    if let Value::Combo(rc) = &result {
        if let Some(entry) = rc.get_field("0") {
            if let Value::Combo(ec) = entry {
                let path_val = ec.get_field("path").expect("has path");
                assert!(
                    matches!(path_val, Value::Atom(AtomKind::Str(s), _, _) if s == "nested.val"),
                    "nested path should be 'nested.val', got {:?}",
                    path_val
                );
            }
        }
    }
}

// ─── diff.patch ───────────────────────────────────────────────────────────────

#[test]
fn test_patch_empty_diff_returns_original() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let val = combo(&[("x", int_val(42))]);
    let empty_diff = list_of(&[]);
    let result = call(&oo, &mut ctx, "diff.patch", args2(val, empty_diff));
    if let Value::Combo(rc) = &result {
        let x = rc.get_field("x").expect("x preserved");
        assert!(matches!(x, Value::Atom(AtomKind::Int(n), _, _) if n == &BigInt::from(42i64)));
    } else {
        panic!("expected Combo");
    }
}

#[test]
fn test_patch_applies_single_change() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let val = combo(&[("score", int_val(0))]);
    let entry = combo(&[("path", str_val("score")), ("to", int_val(100))]);
    let diff_list = list_of(&[entry]);
    let result = call(&oo, &mut ctx, "diff.patch", args2(val, diff_list));
    if let Value::Combo(rc) = &result {
        let score = rc.get_field("score").expect("score field");
        assert!(matches!(score, Value::Atom(AtomKind::Int(n), _, _) if n == &BigInt::from(100i64)));
    } else {
        panic!("expected Combo");
    }
}

// ─── diff.is_compatible ───────────────────────────────────────────────────────

#[test]
fn test_is_compatible_disjoint_fields() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let a = combo(&[("x", int_val(1))]);
    let b = combo(&[("y", int_val(2))]);
    let result = call(&oo, &mut ctx, "diff.is_compatible", args2(a, b));
    assert!(
        matches!(&result, Value::Atom(AtomKind::Tag(t), _, _) if t == "true"),
        "disjoint fields are compatible"
    );
}

#[test]
fn test_is_compatible_conflicting_atoms() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let a = combo(&[("x", int_val(1))]);
    let b = combo(&[("x", int_val(2))]);
    let result = call(&oo, &mut ctx, "diff.is_compatible", args2(a, b));
    assert!(
        matches!(&result, Value::Atom(AtomKind::Tag(t), _, _) if t == "false"),
        "conflicting same field → not compatible"
    );
}
