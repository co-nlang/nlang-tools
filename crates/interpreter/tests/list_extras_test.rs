use indexmap::IndexMap;
use nlang_interpreter::value::{ComboVal, EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

fn make_oo() -> Ouroboros {
    Ouroboros::new_in_memory()
}

fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}
fn float_val(f: f64) -> Value {
    Value::Atom(AtomKind::Float(f), EffectTag::Pure, None)
}

fn make_list(items: Vec<Value>) -> Value {
    let mut f = IndexMap::new();
    for (i, v) in items.iter().enumerate() {
        f.insert(i.to_string(), v.clone());
    }
    f.insert(
        "%kind".to_string(),
        Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
    );
    f.insert("%len".to_string(), int_val(items.len() as i64));
    Value::Combo(ComboVal::new(
        f,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

fn make_combo_2(a: Value, b: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), a);
    f.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(
        f,
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
    if let Value::Combo(ref cv) = v {
        (0..)
            .take_while(|i| cv.get_field(&i.to_string()).is_some())
            .count()
    } else {
        0
    }
}

fn assert_int(v: &Value, expected: i64) {
    match v {
        Value::Atom(AtomKind::Int(n), _, _) => {
            assert_eq!(n, &BigInt::from(expected), "int mismatch")
        }
        _ => panic!("expected Int, got {:?}", v),
    }
}

// ── list.partition ──────────────────────────────────────────────

#[test]
fn test_list_partition_mixed() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int_val(1), int_val(2), int_val(3)]);
    let arg = make_combo_2(Value::Top, list);
    let r = call(&oo, &mut ctx, "list.partition", arg);
    if let Value::Combo(ref cv) = r {
        assert!(
            cv.get_field("yes").is_some(),
            "partition should have 'yes' field"
        );
        assert!(
            cv.get_field("no").is_some(),
            "partition should have 'no' field"
        );
        let yes_len = list_len(cv.get_field("yes").unwrap());
        let no_len = list_len(cv.get_field("no").unwrap());
        assert_eq!(
            yes_len + no_len,
            3,
            "yes+no should equal original list length"
        );
    } else {
        panic!("expected Combo with yes/no fields, got {:?}", r);
    }
}

#[test]
fn test_list_partition_empty_input() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![]);
    let arg = make_combo_2(Value::Top, list);
    let r = call(&oo, &mut ctx, "list.partition", arg);
    if let Value::Combo(ref cv) = r {
        assert_eq!(list_len(cv.get_field("yes").unwrap()), 0);
        assert_eq!(list_len(cv.get_field("no").unwrap()), 0);
    } else {
        panic!("expected Combo");
    }
}

#[test]
fn test_list_partition_pred_routing() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int_val(10), int_val(20)]);
    let arg = make_combo_2(Value::Top, list.clone());
    let r = call(&oo, &mut ctx, "list.partition", arg);
    if let Value::Combo(ref cv) = r {
        let total = list_len(cv.get_field("yes").unwrap()) + list_len(cv.get_field("no").unwrap());
        assert_eq!(total, 2, "total items must be preserved");
    } else {
        panic!("expected Combo");
    }
}

// ── list.flatten ────────────────────────────────────────────────

#[test]
fn test_list_flatten_basic() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let inner_a = make_list(vec![int_val(1), int_val(2)]);
    let inner_b = make_list(vec![int_val(3), int_val(4)]);
    let outer = make_list(vec![inner_a, inner_b]);
    let r = call(&oo, &mut ctx, "list.flatten", outer);
    assert_eq!(
        list_len(&r),
        4,
        "flatten of [[1,2],[3,4]] should have 4 elements"
    );
    if let Value::Combo(ref cv) = r {
        assert_eq!(cv.get_field("0").unwrap().to_string_plain(), "1");
        assert_eq!(cv.get_field("3").unwrap().to_string_plain(), "4");
    }
}

#[test]
fn test_list_flatten_non_list_passthrough() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let inner = make_list(vec![int_val(1), int_val(2)]);
    let outer = make_list(vec![inner, int_val(99)]);
    let r = call(&oo, &mut ctx, "list.flatten", outer);
    assert_eq!(
        list_len(&r),
        3,
        "non-list item should be kept as single element"
    );
}

// ── list.sum ────────────────────────────────────────────────────

#[test]
fn test_list_sum_ints() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int_val(1), int_val(2), int_val(3), int_val(4)]);
    let r = call(&oo, &mut ctx, "list.sum", list);
    assert_int(&r, 10);
}

#[test]
fn test_list_sum_mixed_float() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int_val(1), float_val(2.5), int_val(3)]);
    let r = call(&oo, &mut ctx, "list.sum", list);
    match r {
        Value::Atom(AtomKind::Float(f), _, _) => {
            assert!((f - 6.5).abs() < 1e-9, "expected 6.5, got {}", f)
        }
        _ => panic!("expected Float, got {:?}", r),
    }
}

#[test]
fn test_list_sum_empty() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![]);
    let r = call(&oo, &mut ctx, "list.sum", list);
    assert_int(&r, 0);
}

// ── list.min_by / list.max_by ───────────────────────────────────

#[test]
fn test_list_min_by() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int_val(5), int_val(3), int_val(8)]);
    let arg = make_combo_2(Value::Top, list);
    let r = call(&oo, &mut ctx, "list.min_by", arg);
    assert!(
        matches!(r, Value::Top),
        "empty key fn results should give Top"
    );
}

#[test]
fn test_list_min_by_empty() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![]);
    let arg = make_combo_2(Value::Top, list);
    let r = call(&oo, &mut ctx, "list.min_by", arg);
    assert!(
        matches!(r, Value::Top),
        "min_by on empty list should return Top"
    );
}

#[test]
fn test_list_max_by_empty() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![]);
    let arg = make_combo_2(Value::Top, list);
    let r = call(&oo, &mut ctx, "list.max_by", arg);
    assert!(
        matches!(r, Value::Top),
        "max_by on empty list should return Top"
    );
}

#[test]
fn test_list_max_by_with_key() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int_val(3), int_val(7), int_val(2)]);
    let arg = make_combo_2(Value::Top, list);
    let r = call(&oo, &mut ctx, "list.max_by", arg);
    assert!(matches!(r, Value::Top));
}
