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

fn make_combo_3(a: Value, b: Value, c: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), a);
    f.insert("1".to_string(), b);
    f.insert("2".to_string(), c);
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

fn assert_int(v: &Value, expected: i64) {
    match v {
        Value::Atom(AtomKind::Int(n), _, _) => assert_eq!(n, &BigInt::from(expected)),
        _ => panic!("expected Int, got {:?}", v),
    }
}

fn closed_combo() -> Value {
    Value::Combo(ComboVal::new(
        IndexMap::new(),
        true,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}
fn open_combo() -> Value {
    Value::Combo(ComboVal::new(
        IndexMap::new(),
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

fn make_pred_builtin(name: &str) -> Value {
    let mut f = IndexMap::new();
    f.insert(
        "%morphism".to_string(),
        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
    );
    f.insert(
        "%builtin".to_string(),
        Value::Atom(AtomKind::Str(name.to_string()), EffectTag::Pure, None),
    );
    Value::Combo(ComboVal::new(
        f,
        true,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

#[test]
fn test_list_count_empty() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let pred = make_pred_builtin("refl.is_cocoon");
    let mut f = IndexMap::new();
    f.insert("0".to_string(), pred);
    f.insert("1".to_string(), make_list(vec![]));
    let arg = Value::Combo(ComboVal::new(
        f,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));
    let r = call(&oo, &mut ctx, "list.count", arg);
    assert_int(&r, 0);
}

#[test]
fn test_list_count_some() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let pred = make_pred_builtin("refl.is_cocoon");
    let list = make_list(vec![closed_combo(), open_combo(), closed_combo()]);
    let mut f = IndexMap::new();
    f.insert("0".to_string(), pred);
    f.insert("1".to_string(), list);
    let arg = Value::Combo(ComboVal::new(
        f,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));
    let r = call(&oo, &mut ctx, "list.count", arg);
    assert_int(&r, 2);
}

#[test]
fn test_list_zip_with_add() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let la = make_list(vec![int_val(1), int_val(2), int_val(3)]);
    let lb = make_list(vec![int_val(10), int_val(20), int_val(30)]);
    // Use Top as f (identity) → each pair is returned as-is
    // apply_morphism(Top, pair) → Top. So result = [Top, Top, Top]
    let arg = make_combo_3(Value::Top, la, lb);
    let r = call(&oo, &mut ctx, "list.zip_with", arg);
    if let Value::Combo(ref cv) = r {
        assert!(cv.get_field("0").is_some());
        assert!(cv.get_field("1").is_some());
        assert!(cv.get_field("2").is_some());
    } else {
        panic!("expected list");
    }
}

#[test]
fn test_list_zip_with_truncates() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let la = make_list(vec![int_val(1), int_val(2), int_val(3)]);
    let lb = make_list(vec![int_val(10), int_val(20)]);
    let arg = make_combo_3(Value::Top, la, lb);
    let r = call(&oo, &mut ctx, "list.zip_with", arg);
    if let Value::Combo(ref cv) = r {
        assert!(cv.get_field("0").is_some());
        assert!(cv.get_field("1").is_some());
        assert!(cv.get_field("2").is_none(), "should truncate to min length");
    } else {
        panic!("expected list combo");
    }
}
