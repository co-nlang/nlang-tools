use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use num_bigint::BigInt;

fn oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn float_val(f: f64) -> Value { Value::Atom(AtomKind::Float(f), EffectTag::Pure, None) }
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
fn float_list(nums: &[f64]) -> Value {
    let items: Vec<Value> = nums.iter().map(|&f| float_val(f)).collect();
    list_of(&items)
}

#[test] fn test_stat_mean() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let list = float_list(&[1.0, 2.0, 3.0, 4.0, 5.0]);
    let r = call(&oo, &mut ctx, "stat.mean", list);
    assert!(matches!(&r, Value::Atom(AtomKind::Float(f), _, _) if (f - 3.0).abs() < 1e-10));
}

#[test] fn test_stat_median_odd() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let list = float_list(&[3.0, 1.0, 5.0, 2.0, 4.0]);
    let r = call(&oo, &mut ctx, "stat.median", list);
    assert!(matches!(&r, Value::Atom(AtomKind::Float(f), _, _) if (f - 3.0).abs() < 1e-10));
}

#[test] fn test_stat_std_dev() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let list = float_list(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
    let r = call(&oo, &mut ctx, "stat.std_dev", list);
    assert!(matches!(&r, Value::Atom(AtomKind::Float(f), _, _) if (f - 2.0).abs() < 1e-10));
}

#[test] fn test_stat_percentile_50() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let list = float_list(&[10.0, 20.0, 30.0, 40.0, 50.0]);
    let p = float_val(50.0);
    let r = call(&oo, &mut ctx, "stat.percentile", args2(list, p));
    assert!(matches!(&r, Value::Atom(AtomKind::Float(f), _, _) if (f - 30.0).abs() < 1e-10));
}

#[test] fn test_stat_histogram_bins() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let list = float_list(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    let bins = Value::Atom(AtomKind::Int(BigInt::from(3)), EffectTag::Pure, None);
    let r = call(&oo, &mut ctx, "stat.histogram", args2(list, bins));
    assert_eq!(list_len(&r), 3);
}

#[test] fn test_stat_variance() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let list = float_list(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
    let r = call(&oo, &mut ctx, "stat.variance", list);
    assert!(matches!(&r, Value::Atom(AtomKind::Float(f), _, _) if (f - 4.0).abs() < 1e-10));
}
