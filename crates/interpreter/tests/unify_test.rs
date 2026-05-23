use nlang_interpreter::{Ouroboros, Value, ComboVal, BottomCause, EffectTag};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use num_bigint::BigInt;

fn int_atom(v: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(v)), EffectTag::Pure, None)
}

fn empty_ouroboros() -> Ouroboros {
    Ouroboros::new_in_memory()
}

#[test]
fn test_basic_lattice_properties() {
    let oo = empty_ouroboros();
    let v1 = int_atom(1);
    let v2 = int_atom(2);
    let top = Value::Top;

    // Top 是恆等元
    assert_eq!(oo.unify(top.clone(), v1.clone()), v1);
    assert_eq!(oo.unify(v1.clone(), top), v1);

    // Bottom 傳染
    let bottom: Value = BottomCause::Conflict.into();
    assert!(matches!(oo.unify(bottom.clone(), v1.clone()), Value::Bottom(_)));
    assert!(matches!(oo.unify(v1.clone(), bottom), Value::Bottom(_)));

    // 等值合併
    assert_eq!(oo.unify(v1.clone(), v1.clone()), v1);

    // 原子衝突
    let res = oo.unify(v1, v2);
    if let Value::Bottom(detail) = res {
        assert_eq!(detail.cause, BottomCause::Conflict);
    } else {
        panic!("Expected Bottom, got {:?}", res);
    }
}

#[test]
fn test_atomic_isomorphic_expansion() {
    let oo = empty_ouroboros();
    let v1 = int_atom(42);
    
    // { %unit: "kg" }
    let c1 = Value::Combo(ComboVal::new(
        IndexMap::from_iter(vec![("%unit".to_string(), Value::Atom(AtomKind::Str("kg".to_string()), EffectTag::Pure, None))]),
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));

    // unify(42, { %unit: "kg" }) == { %val: 42, %unit: "kg" }
    let res = oo.unify(v1, c1);
    if let Value::Combo(cv) = res {
        assert_eq!(cv.get_field("%val").unwrap(), &int_atom(42));
        assert_eq!(cv.get_field("%unit").unwrap(), &Value::Atom(AtomKind::Str("kg".to_string()), EffectTag::Pure, None));
    } else {
        panic!("Expected Combo, got {:?}", res);
    }
}

#[test]
fn test_combo_open_world() {
    let oo = empty_ouroboros();
    
    // {a:1}
    let c1 = Value::Combo(ComboVal::new(
        IndexMap::from_iter(vec![("a".to_string(), int_atom(1))]),
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));

    // {b:2}
    let c2 = Value::Combo(ComboVal::new(
        IndexMap::from_iter(vec![("b".to_string(), int_atom(2))]),
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));

    // unify({a:1}, {b:2}) == {a:1, b:2}
    let res = oo.unify(c1.clone(), c2);
    if let Value::Combo(cv) = res {
        assert_eq!(cv.get_field("a").unwrap(), &int_atom(1));
        assert_eq!(cv.get_field("b").unwrap(), &int_atom(2));
        assert!(!cv.closed);
    } else {
        panic!("Expected Combo, got {:?}", res);
    }
}

#[test]
fn test_cocoon_isolation() {
    let oo = empty_ouroboros();

    // r1 = {{a:1}} (Cocoon)
    let r1 = Value::Combo(ComboVal::new(
        IndexMap::from_iter(vec![("a".to_string(), int_atom(1))]),
        true,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));

    // c1 = {b:2} (Open)
    let c1 = Value::Combo(ComboVal::new(
        IndexMap::from_iter(vec![("b".to_string(), int_atom(2))]),
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));

    // unify({{a:1}}, {b:2}) == Bottom(MissingKey)
    // 根據 SPEC_03，Cocoon 拒絕任何未定義欄位的合併
    let res = oo.unify(r1, c1);
    if let Value::Bottom(detail) = res {
        assert_eq!(detail.cause, BottomCause::MissingKey);
    } else {
        panic!("Expected Bottom(MissingKey), got {:?}", res);
    }
}

#[test]
fn test_trinity_logic_morphism() {
    // 測試三位一體同構下的態射合併
    let _oo = empty_ouroboros();
    
    // f1 = { %morphism: #true, %rules: { x: _ } }
    let f1 = Value::Combo(ComboVal::new(
        IndexMap::from_iter(vec![
            ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
            ("%rules".to_string(), Value::Combo(ComboVal::new(
                IndexMap::from_iter(vec![("x".to_string(), Value::Top)]),
                false,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            ))),
        ]),
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));

    assert!(f1.is_morphism());
}
