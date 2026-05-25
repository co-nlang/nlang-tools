use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use num_bigint::BigInt;

fn oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn int_val(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
fn list_of(items: &[Value]) -> Value {
    let mut m = IndexMap::new();
    m.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    for (i, v) in items.iter().enumerate() { m.insert(i.to_string(), v.clone()); }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn args2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn list_len(v: &Value) -> usize {
    if let Value::Combo(c) = v {
        (0u32..).take_while(|i| c.get_field(&i.to_string()).is_some()).count()
    } else { 0 }
}

#[test] fn test_set_from_list_dedup() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let list = list_of(&[int_val(1), int_val(2), int_val(1), int_val(3)]);
    let r = call(&oo, &mut ctx, "set.from_list", list);
    assert_eq!(list_len(&r), 3);
}

#[test] fn test_set_union() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let a = list_of(&[int_val(1), int_val(2)]);
    let b = list_of(&[int_val(2), int_val(3)]);
    let r = call(&oo, &mut ctx, "set.union", args2(a, b));
    assert_eq!(list_len(&r), 3);
}

#[test] fn test_set_intersection() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let a = list_of(&[int_val(1), int_val(2), int_val(3)]);
    let b = list_of(&[int_val(2), int_val(3), int_val(4)]);
    let r = call(&oo, &mut ctx, "set.intersection", args2(a, b));
    assert_eq!(list_len(&r), 2);
}

#[test] fn test_set_difference() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let a = list_of(&[int_val(1), int_val(2), int_val(3)]);
    let b = list_of(&[int_val(2)]);
    let r = call(&oo, &mut ctx, "set.difference", args2(a, b));
    assert_eq!(list_len(&r), 2);
}

#[test] fn test_set_is_subset() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let a = list_of(&[int_val(1), int_val(2)]);
    let b = list_of(&[int_val(1), int_val(2), int_val(3)]);
    let r = call(&oo, &mut ctx, "set.is_subset", args2(a.clone(), b.clone()));
    assert!(matches!(&r, Value::Atom(AtomKind::Tag(t), _, _) if t == "true"));
    let r2 = call(&oo, &mut ctx, "set.is_subset", args2(b, a));
    assert!(matches!(&r2, Value::Atom(AtomKind::Tag(t), _, _) if t == "false"));
}

#[test] fn test_set_contains() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let s = list_of(&[int_val(1), int_val(2), int_val(3)]);
    let r = call(&oo, &mut ctx, "set.contains", args2(s.clone(), int_val(2)));
    assert!(matches!(&r, Value::Atom(AtomKind::Tag(t), _, _) if t == "true"));
    let r2 = call(&oo, &mut ctx, "set.contains", args2(s, int_val(5)));
    assert!(matches!(&r2, Value::Atom(AtomKind::Tag(t), _, _) if t == "false"));
}
