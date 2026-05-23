use nlang_interpreter::{Value, ComboVal, EffectTag};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use num_bigint::BigInt;

fn int_atom(v: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(v)), EffectTag::Pure, None)
}

#[test]
fn test_canonical_caid_sorting() {
    // c1 = {a:1, b:2}
    let mut f1 = IndexMap::new();
    f1.insert("a".to_string(), int_atom(1));
    f1.insert("b".to_string(), int_atom(2));
    let c1 = Value::Combo(ComboVal::new(f1, false, IndexMap::new(), EffectTag::Pure, vec![]));

    // c2 = {b:2, a:1}
    let mut f2 = IndexMap::new();
    f2.insert("b".to_string(), int_atom(2));
    f2.insert("a".to_string(), int_atom(1));
    let c2 = Value::Combo(ComboVal::new(f2, false, IndexMap::new(), EffectTag::Pure, vec![]));

    assert_eq!(c1.content_hash(), c2.content_hash());
}

#[test]
fn test_cocoon_caid_distinct() {
    let mut f1 = IndexMap::new();
    f1.insert("a".to_string(), int_atom(1));
    let c1 = Value::Combo(ComboVal::new(f1.clone(), false, IndexMap::new(), EffectTag::Pure, vec![]));

    // {{a:1}}
    let c2 = Value::Combo(ComboVal::new(f1, true, IndexMap::new(), EffectTag::Pure, vec![]));

    assert_ne!(c1.content_hash(), c2.content_hash());
}

#[test]
fn test_private_field_caid() {
    // { public: 1, ~secret: 2 }
    let mut f1 = IndexMap::new(); f1.insert("public".to_string(), int_atom(1));
    let mut l1 = IndexMap::new(); l1.insert("secret".to_string(), int_atom(2));
    let c1 = Value::Combo(ComboVal::new(f1, false, l1, EffectTag::Pure, vec![]));

    // { public: 1 } (無私有欄位)
    let mut f2 = IndexMap::new(); f2.insert("public".to_string(), int_atom(1));
    let c2 = Value::Combo(ComboVal::new(f2, false, IndexMap::new(), EffectTag::Pure, vec![]));

    assert_ne!(c1.content_hash(), c2.content_hash());
}
