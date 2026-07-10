use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use num_bigint::BigInt;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}

fn make_list(items: Vec<Value>) -> Value {
    let len = items.len();
    let mut fields = IndexMap::new();
    for (i, v) in items.into_iter().enumerate() {
        fields.insert(i.to_string(), v);
    }
    fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    fields.insert("%len".to_string(), int_val(len as i64));
    Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    let f = oo.builtin_registry.get(name).expect("builtin not found").clone();
    f(arg, oo, ctx)
}

fn make_combo_2(a: Value, b: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), a);
    f.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn is_tag(v: &Value, t: &str) -> bool {
    if let Value::Atom(AtomKind::Tag(s), _, _) = v.collapse() { s.trim_start_matches('#') == t } else { false }
}

#[test]
fn test_list_head_some() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int_val(10), int_val(20), int_val(30)]);
    let result = call(&oo, &mut ctx, "list.head", list);
    if let Value::Combo(ref cv) = result {
        let inner = cv.get_field("%val").expect("should have %val");
        assert_eq!(inner.to_string_plain(), "10");
    } else { panic!("expected Some, got {:?}", result); }
}

#[test]
fn test_list_head_empty() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![]);
    let result = call(&oo, &mut ctx, "list.head", list);
    assert!(is_tag(&result, "none"), "expected #none, got {:?}", result);
}

#[test]
fn test_list_tail_normal() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int_val(1), int_val(2), int_val(3)]);
    let result = call(&oo, &mut ctx, "list.tail", list);
    if let Value::Combo(ref cv) = result {
        assert_eq!(cv.get_field("0").unwrap().to_string_plain(), "2");
        assert_eq!(cv.get_field("1").unwrap().to_string_plain(), "3");
        assert!(cv.get_field("2").is_none());
    } else { panic!("expected list combo"); }
}

#[test]
fn test_list_take_n() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int_val(10), int_val(20), int_val(30), int_val(40)]);
    let arg = make_combo_2(int_val(2), list);
    let result = call(&oo, &mut ctx, "list.take", arg);
    if let Value::Combo(ref cv) = result {
        assert!(cv.get_field("0").is_some());
        assert!(cv.get_field("1").is_some());
        assert!(cv.get_field("2").is_none());
    } else { panic!("expected list"); }
}

#[test]
fn test_list_drop_n() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![int_val(10), int_val(20), int_val(30), int_val(40)]);
    let arg = make_combo_2(int_val(2), list);
    let result = call(&oo, &mut ctx, "list.drop", arg);
    if let Value::Combo(ref cv) = result {
        assert_eq!(cv.get_field("0").unwrap().to_string_plain(), "30");
        assert_eq!(cv.get_field("1").unwrap().to_string_plain(), "40");
        assert!(cv.get_field("2").is_none());
    } else { panic!("expected list"); }
}

#[test]
fn test_list_tail_empty() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(vec![]);
    let result = call(&oo, &mut ctx, "list.tail", list);
    if let Value::Combo(ref cv) = result {
        assert!(cv.get_field("0").is_none(), "tail of empty should be empty");
    } else { panic!("expected empty list combo"); }
}
