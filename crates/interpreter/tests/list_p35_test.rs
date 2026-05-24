use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn int(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn combo1(a: Value) -> Value {
    let mut m = IndexMap::new(); m.insert("0".to_string(), a);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn combo2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn list(items: Vec<Value>) -> Value {
    let mut m = IndexMap::new();
    for (i, v) in items.into_iter().enumerate() { m.insert(i.to_string(), v); }
    m.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn list_len(v: &Value) -> usize {
    match v { Value::Combo(c) => c.fields_iter().filter(|(k,_)| k.parse::<usize>().is_ok()).count(), _ => panic!() }
}
fn list_at(v: &Value, i: usize) -> &Value {
    match v { Value::Combo(c) => c.get_field(&i.to_string()).unwrap(), _ => panic!() }
}
fn as_int(v: &Value) -> i64 {
    match v { Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(), o => panic!("{:?}", o) }
}

#[test]
fn test_list_enumerate_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.enumerate", combo1(list(vec![int(10), int(20), int(30)])));
    assert_eq!(list_len(&r), 3);
    let pair0 = list_at(&r, 0);
    if let Value::Combo(c) = pair0 {
        assert_eq!(as_int(c.get_field("0").unwrap()), 0);
        assert_eq!(as_int(c.get_field("1").unwrap()), 10);
    } else { panic!(); }
}

#[test]
fn test_list_enumerate_empty() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.enumerate", combo1(list(vec![])));
    assert_eq!(list_len(&r), 0);
}

#[test]
fn test_list_sort_by_ascending() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let cmp_fn = oo.builtin_registry.get("math.sub").unwrap().clone();
    let cmp_val = Value::Combo(ComboVal::new(
        IndexMap::from_iter(vec![
            ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
            ("%builtin".to_string(),  Value::Atom(AtomKind::Str("math.sub".to_string()), EffectTag::Pure, None)),
        ]),
        true, IndexMap::new(), EffectTag::Pure, vec![],
    ));
    let r = call(&oo, &mut ctx, "list.sort_by", combo2(cmp_val, list(vec![int(3), int(1), int(2)])));
    assert_eq!(as_int(list_at(&r, 0)), 1);
    assert_eq!(as_int(list_at(&r, 1)), 2);
    assert_eq!(as_int(list_at(&r, 2)), 3);
}

#[test]
fn test_list_dedup_consecutive() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.dedup",
        combo1(list(vec![int(1), int(1), int(2), int(3), int(3), int(1)])));
    assert_eq!(list_len(&r), 4);
    assert_eq!(as_int(list_at(&r, 0)), 1);
    assert_eq!(as_int(list_at(&r, 1)), 2);
    assert_eq!(as_int(list_at(&r, 2)), 3);
    assert_eq!(as_int(list_at(&r, 3)), 1);
}

#[test]
fn test_list_dedup_no_consecutive() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.dedup",
        combo1(list(vec![int(1), int(2), int(3)])));
    assert_eq!(list_len(&r), 3);
}

#[test]
fn test_list_intersperse_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.intersperse",
        combo2(int(0), list(vec![int(1), int(2), int(3)])));
    assert_eq!(list_len(&r), 5);
    assert_eq!(as_int(list_at(&r, 0)), 1);
    assert_eq!(as_int(list_at(&r, 1)), 0);
    assert_eq!(as_int(list_at(&r, 2)), 2);
    assert_eq!(as_int(list_at(&r, 3)), 0);
    assert_eq!(as_int(list_at(&r, 4)), 3);
}

#[test]
fn test_list_intersperse_single_element() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.intersperse",
        combo2(int(0), list(vec![int(42)])));
    assert_eq!(list_len(&r), 1);
    assert_eq!(as_int(list_at(&r, 0)), 42);
}

#[test]
fn test_list_intersperse_empty() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.intersperse",
        combo2(int(0), list(vec![])));
    assert_eq!(list_len(&r), 0);
}
