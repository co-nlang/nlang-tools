use indexmap::IndexMap;
use nlang_interpreter::value::{ComboVal, EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

fn make_oo() -> Ouroboros {
    Ouroboros::new_in_memory()
}

fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}
fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}

fn make_combo(fields: Vec<(&str, Value)>) -> Value {
    let mut cv = ComboVal::default();
    for (k, v) in fields {
        cv.insert_field(k, v);
    }
    Value::Combo(cv)
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

fn is_top(v: &Value) -> bool {
    matches!(v, Value::Top)
}

#[test]
fn test_refl_get_existing() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let obj = make_combo(vec![("name", str_val("Alice")), ("age", int_val(30))]);
    let arg = make_combo_2(str_val("name"), obj);
    let r = call(&oo, &mut ctx, "refl.get", arg);
    assert_eq!(r.to_string_plain(), "Alice");
}

#[test]
fn test_refl_get_missing() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let obj = make_combo(vec![("name", str_val("Alice"))]);
    let arg = make_combo_2(str_val("nonexistent"), obj);
    let r = call(&oo, &mut ctx, "refl.get", arg);
    assert!(is_top(&r), "missing key should return Top, got {:?}", r);
}

#[test]
fn test_refl_set_new_field() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let obj = make_combo(vec![("name", str_val("Alice"))]);
    let arg = make_combo_3(str_val("city"), str_val("Taipei"), obj.clone());
    let r = call(&oo, &mut ctx, "refl.set", arg);
    if let Value::Combo(ref cv) = r {
        assert_eq!(cv.get_field("name").unwrap().to_string_plain(), "Alice");
        assert_eq!(cv.get_field("city").unwrap().to_string_plain(), "Taipei");
    } else {
        panic!("expected Combo, got {:?}", r);
    }
    if let Value::Combo(ref cv) = obj {
        assert!(
            cv.get_field("city").is_none(),
            "original combo should be unchanged"
        );
    }
}

#[test]
fn test_refl_set_update_field() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let obj = make_combo(vec![("name", str_val("Alice")), ("age", int_val(30))]);
    let arg = make_combo_3(str_val("age"), int_val(31), obj);
    let r = call(&oo, &mut ctx, "refl.set", arg);
    if let Value::Combo(ref cv) = r {
        assert_eq!(cv.get_field("age").unwrap().to_string_plain(), "31");
    } else {
        panic!("expected Combo");
    }
}

#[test]
fn test_refl_delete_existing() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let obj = make_combo(vec![("name", str_val("Alice")), ("age", int_val(30))]);
    let arg = make_combo_2(str_val("age"), obj);
    let r = call(&oo, &mut ctx, "refl.delete", arg);
    if let Value::Combo(ref cv) = r {
        assert!(cv.get_field("age").is_none(), "age should be removed");
        assert_eq!(cv.get_field("name").unwrap().to_string_plain(), "Alice");
    } else {
        panic!("expected Combo");
    }
}

#[test]
fn test_refl_delete_missing_is_noop() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let obj = make_combo(vec![("name", str_val("Alice"))]);
    let arg = make_combo_2(str_val("city"), obj);
    let r = call(&oo, &mut ctx, "refl.delete", arg);
    if let Value::Combo(ref cv) = r {
        assert_eq!(cv.get_field("name").unwrap().to_string_plain(), "Alice");
    } else {
        panic!("expected Combo");
    }
}

#[test]
fn test_refl_values_parallel_to_keys() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let obj = make_combo(vec![
        ("c", int_val(3)),
        ("a", int_val(1)),
        ("b", int_val(2)),
    ]);
    let keys_r = call(&oo, &mut ctx, "refl.keys", obj.clone());
    let vals_r = call(&oo, &mut ctx, "refl.values", obj);
    if let (Value::Combo(ref kc), Value::Combo(ref vc)) = (&keys_r, &vals_r) {
        let k0 = kc.get_field("0").unwrap().to_string_plain();
        let v0 = vc.get_field("0").unwrap().to_string_plain();
        assert_eq!(k0, "a");
        assert_eq!(v0, "1");
        assert!(kc.get_field("2").is_some());
        assert!(vc.get_field("2").is_some());
    } else {
        panic!("expected list combos");
    }
}

#[test]
fn test_refl_entries_format() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let obj = make_combo(vec![("x", int_val(10))]);
    let r = call(&oo, &mut ctx, "refl.entries", obj);
    if let Value::Combo(ref lc) = r {
        let entry = lc.get_field("0").expect("should have entry at index 0");
        if let Value::Combo(ref ec) = entry {
            assert_eq!(ec.get_field("key").unwrap().to_string_plain(), "x");
            assert_eq!(ec.get_field("val").unwrap().to_string_plain(), "10");
        } else {
            panic!("entry should be Combo, got {:?}", entry);
        }
    } else {
        panic!("expected list Combo");
    }
}
