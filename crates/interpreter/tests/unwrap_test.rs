use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal, BottomCause};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use num_bigint::BigInt;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}

fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}

fn make_ok(v: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("%val".to_string(), v);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_err(cause: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("%cause".to_string(), cause);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_some(v: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("%val".to_string(), v);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_none() -> Value {
    Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None)
}

fn make_combo_2(a: Value, b: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), a);
    f.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    let f = oo.builtin_registry.get(name).expect("builtin not found").clone();
    f(arg, oo, ctx)
}

#[test]
fn test_result_unwrap_ok() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let ok = make_ok(int_val(42));
    let result = call(&oo, &mut ctx, "result.unwrap", ok);
    assert_eq!(result.collapse().to_string_plain(), "42");
}

#[test]
fn test_result_unwrap_err() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let err = make_err(str_val("bad_input"));
    let result = call(&oo, &mut ctx, "result.unwrap", err);
    match result {
        Value::Bottom(ref detail) => {
            assert!(matches!(detail.cause, BottomCause::Conflict));
            assert!(detail.message.as_deref().unwrap_or("").contains("unwrap"));
        }
        _ => panic!("expected Bottom, got {:?}", result),
    }
}

#[test]
fn test_result_expect_ok() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let ok = make_ok(int_val(99));
    let arg = make_combo_2(str_val("parse error"), ok);
    let result = call(&oo, &mut ctx, "result.expect", arg);
    assert_eq!(result.collapse().to_string_plain(), "99");
}

#[test]
fn test_result_expect_err() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let err = make_err(str_val("timeout"));
    let arg = make_combo_2(str_val("fetch failed"), err);
    let result = call(&oo, &mut ctx, "result.expect", arg);
    match result {
        Value::Bottom(ref detail) => {
            let msg = detail.message.as_deref().unwrap_or("");
            assert!(msg.contains("fetch failed"), "msg was: {}", msg);
            assert!(msg.contains("timeout"), "msg was: {}", msg);
        }
        _ => panic!("expected Bottom"),
    }
}

#[test]
fn test_option_expect_some() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let some = make_some(int_val(7));
    let arg = make_combo_2(str_val("should be present"), some);
    let result = call(&oo, &mut ctx, "option.expect", arg);
    assert_eq!(result.collapse().to_string_plain(), "7");
}

#[test]
fn test_option_expect_none() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let none = make_none();
    let arg = make_combo_2(str_val("expected a value"), none);
    let result = call(&oo, &mut ctx, "option.expect", arg);
    match result {
        Value::Bottom(ref detail) => {
            assert!(detail.message.as_deref().unwrap_or("").contains("expected a value"));
        }
        _ => panic!("expected Bottom"),
    }
}
