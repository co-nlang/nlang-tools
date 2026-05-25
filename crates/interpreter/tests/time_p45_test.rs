use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn int_val(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
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
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn as_str(v: &Value) -> String {
    match v { Value::Atom(AtomKind::Str(s), _, _) => s.clone(), o => panic!("expected Str: {:?}", o) }
}
fn as_i64(v: &Value) -> i64 {
    match v { Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(), o => panic!("expected Int: {:?}", o) }
}

#[test]
fn test_time_parse_and_to_iso8601() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "time.parse", combo2(str_val("2023-01-15 10:30:00"), str_val("%Y-%m-%d %H:%M:%S")));
    let ts = as_i64(&r);
    assert!(ts > 0, "parsed timestamp should be positive");
    let r2 = call(&oo, &mut ctx, "time.to_iso8601", combo1(int_val(ts)));
    let s = as_str(&r2);
    assert!(s.starts_with("2023-01-15T10:30:00"), "to_iso8601 should start with 2023-01-15T10:30:00, got {}", s);
}

#[test]
fn test_time_add_days_hours() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "time.add_days", combo2(int_val(0), int_val(1)));
    assert_eq!(as_i64(&r), 86_400_000);
    let r = call(&oo, &mut ctx, "time.add_hours", combo2(int_val(0), int_val(1)));
    assert_eq!(as_i64(&r), 3_600_000);
}

#[test]
fn test_time_weekday_epoch() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "time.weekday", combo1(int_val(0)));
    assert!(matches!(&r, Value::Atom(AtomKind::Tag(t), _, _) if t == "thursday"));
}
