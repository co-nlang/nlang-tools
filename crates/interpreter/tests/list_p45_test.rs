use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn int(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
fn float(n: f64) -> Value { Value::Atom(AtomKind::Float(n), EffectTag::Pure, None) }
fn combo1(a: Value) -> Value {
    let mut m = IndexMap::new(); m.insert("0".to_string(), a);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn combo2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn combo3(a: Value, b: Value, c: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b); m.insert("2".to_string(), c);
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
    match v { Value::Combo(c) => c.fields().keys().filter(|k| k.parse::<usize>().is_ok()).count(), _ => panic!() }
}
fn list_at(v: &Value, i: usize) -> &Value {
    match v { Value::Combo(c) => c.get_field(&i.to_string()).unwrap(), _ => panic!() }
}
fn as_int(v: &Value) -> i64 {
    match v { Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(), o => panic!("{:?}", o) }
}
fn as_float(v: &Value) -> f64 {
    match v { Value::Atom(AtomKind::Float(f), _, _) => *f, o => panic!("{:?}", o) }
}

fn is_prime_morph() -> Value {
    Value::Combo(ComboVal::new(
        IndexMap::from_iter(vec![
            ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
            ("%builtin".to_string(),  Value::Atom(AtomKind::Str("math.is_prime".to_string()), EffectTag::Pure, None)),
        ]),
        true, IndexMap::new(), EffectTag::Pure, vec![],
    ))
}

fn add_morph() -> Value {
    Value::Combo(ComboVal::new(
        IndexMap::from_iter(vec![
            ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
            ("%builtin".to_string(),  Value::Atom(AtomKind::Str("math.add".to_string()), EffectTag::Pure, None)),
        ]),
        true, IndexMap::new(), EffectTag::Pure, vec![],
    ))
}

#[test]
fn test_list_scan_add() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.scan", combo3(list(vec![int(1), int(2), int(3)]), add_morph(), int(0)));
    assert_eq!(list_len(&r), 3);
    assert_eq!(as_int(list_at(&r, 0)), 1);
    assert_eq!(as_int(list_at(&r, 1)), 3);
    assert_eq!(as_int(list_at(&r, 2)), 6);
}

#[test]
fn test_list_product() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.product", combo1(list(vec![int(2), int(3), int(4)])));
    assert_eq!(as_int(&r), 24);
    let r = call(&oo, &mut ctx, "list.product", combo1(list(vec![])));
    assert_eq!(as_int(&r), 1);
    let r = call(&oo, &mut ctx, "list.product", combo1(list(vec![float(2.5), int(2)])));
    assert!((as_float(&r) - 5.0).abs() < 1e-10);
}

#[test]
fn test_list_transpose_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let inner1 = list(vec![int(1), int(2)]);
    let inner2 = list(vec![int(3), int(4)]);
    let outer = list(vec![inner1, inner2]);
    let r = call(&oo, &mut ctx, "list.transpose", combo1(outer));
    assert_eq!(list_len(&r), 2);
    let row0 = list_at(&r, 0);
    assert_eq!(list_len(row0), 2);
    assert_eq!(as_int(list_at(row0, 0)), 1);
    assert_eq!(as_int(list_at(row0, 1)), 3);
}

#[test]
fn test_list_take_while_and_drop_while() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.take_while",
        combo2(list(vec![int(2), int(3), int(4), int(5)]), is_prime_morph()));
    assert_eq!(list_len(&r), 2);
    assert_eq!(as_int(list_at(&r, 0)), 2);
    assert_eq!(as_int(list_at(&r, 1)), 3);
    let r = call(&oo, &mut ctx, "list.drop_while",
        combo2(list(vec![int(2), int(3), int(4), int(5)]), is_prime_morph()));
    assert_eq!(list_len(&r), 2);
    assert_eq!(as_int(list_at(&r, 0)), 4);
    assert_eq!(as_int(list_at(&r, 1)), 5);
}
