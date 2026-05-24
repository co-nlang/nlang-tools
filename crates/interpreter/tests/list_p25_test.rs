use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}
fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}
fn make_list(items: Vec<Value>) -> Value {
    let mut m = IndexMap::new();
    for (i, v) in items.into_iter().enumerate() { m.insert(i.to_string(), v); }
    m.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn morph(builtin: &str) -> Value {
    let mut m = IndexMap::new();
    m.insert("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
    m.insert("%builtin".to_string(), Value::Atom(AtomKind::Str(builtin.to_string()), EffectTag::Pure, None));
    Value::Combo(ComboVal::new(m, true, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn make_combo2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a);
    m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn list_len(v: &Value) -> usize {
    match v {
        Value::Combo(c) => c.fields().keys().filter(|k| k.parse::<usize>().is_ok()).count(),
        _ => panic!("expected list"),
    }
}

#[test]
fn test_list_unique_dedup() {
    // [1, 2, 1, 3, 2] → [1, 2, 3]
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let list = make_list(vec![int_val(1), int_val(2), int_val(1), int_val(3), int_val(2)]);
    let r = call(&oo, &mut ctx, "list.unique", list);
    assert_eq!(list_len(&r), 3);
}

#[test]
fn test_list_unique_empty() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.unique", make_list(vec![]));
    assert_eq!(list_len(&r), 0);
}

#[test]
fn test_list_range_basic() {
    // range(2, 5) → [2, 3, 4]
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let arg = make_combo2(int_val(2), int_val(5));
    let r = call(&oo, &mut ctx, "list.range", arg);
    assert_eq!(list_len(&r), 3);
    if let Value::Combo(c) = &r {
        assert_eq!(c.get_field("0").unwrap().to_string_plain(), "2");
        assert_eq!(c.get_field("2").unwrap().to_string_plain(), "4");
    }
}

#[test]
fn test_list_range_empty_when_start_ge_end() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let arg = make_combo2(int_val(5), int_val(3));
    let r = call(&oo, &mut ctx, "list.range", arg);
    assert_eq!(list_len(&r), 0);
}

#[test]
fn test_list_reduce_sum() {
    // reduce(math.add, [1, 2, 3, 4]) → 10
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let list = make_list(vec![int_val(1), int_val(2), int_val(3), int_val(4)]);
    let arg = make_combo2(morph("math.add"), list);
    let r = call(&oo, &mut ctx, "list.reduce", arg);
    match r {
        Value::Atom(AtomKind::Int(n), _, _) => assert_eq!(n, BigInt::from(10)),
        other => panic!("expected Int(10), got {:?}", other),
    }
}

#[test]
fn test_list_reduce_empty_returns_top() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let arg = make_combo2(morph("math.add"), make_list(vec![]));
    let r = call(&oo, &mut ctx, "list.reduce", arg);
    assert!(matches!(r, Value::Top));
}
