use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}
fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}

fn make_combo_2(a: Value, b: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), a); f.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}

fn as_i64(v: &Value) -> i64 {
    match v {
        Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(),
        other => panic!("expected Int, got {:?}", other),
    }
}

fn as_str(v: &Value) -> String {
    match v {
        Value::Atom(AtomKind::Str(s), _, _) => s.clone(),
        other => panic!("expected Str, got {:?}", other),
    }
}

#[test]
fn test_time_now_is_positive_int() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "time.now", Value::Top);
    let ms = as_i64(&r);
    assert!(ms > 1_577_836_800_000i64, "time.now should return a recent timestamp, got {}", ms);
}

#[test]
fn test_time_diff_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let arg = make_combo_2(int_val(1000), int_val(0));
    let r = call(&oo, &mut ctx, "time.diff", arg);
    assert_eq!(as_i64(&r), 1000);
}

#[test]
fn test_time_diff_negative() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let arg = make_combo_2(int_val(0), int_val(1000));
    let r = call(&oo, &mut ctx, "time.diff", arg);
    assert_eq!(as_i64(&r), -1000);
}

#[test]
fn test_time_add_ms() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let arg = make_combo_2(int_val(500), int_val(1000));
    let r = call(&oo, &mut ctx, "time.add_ms", arg);
    assert_eq!(as_i64(&r), 1500);
}

#[test]
fn test_time_format_epoch_date() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let arg = make_combo_2(str_val("%Y-%m-%d"), int_val(0));
    let r = call(&oo, &mut ctx, "time.format", arg);
    assert_eq!(as_str(&r), "1970-01-01");
}

#[test]
fn test_time_format_epoch_time() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let arg = make_combo_2(str_val("%H:%M:%S"), int_val(0));
    let r = call(&oo, &mut ctx, "time.format", arg);
    assert_eq!(as_str(&r), "00:00:00");
}
