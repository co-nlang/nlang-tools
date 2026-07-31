use indexmap::IndexMap;
use nlang_interpreter::value::{ComboVal, EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;

fn make_oo() -> Ouroboros {
    Ouroboros::new_in_memory()
}

fn some(v: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("%val".to_string(), v);
    Value::Combo(ComboVal::new(
        m,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}
fn none() -> Value {
    Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None)
}
fn ok(v: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("%val".to_string(), v);
    Value::Combo(ComboVal::new(
        m,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}
fn err(cause: &str) -> Value {
    let mut m = IndexMap::new();
    m.insert(
        "%cause".to_string(),
        Value::Atom(AtomKind::Str(cause.to_string()), EffectTag::Pure, None),
    );
    Value::Combo(ComboVal::new(
        m,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}
fn int(n: i64) -> Value {
    Value::Atom(AtomKind::Int(n.into()), EffectTag::Pure, None)
}
fn make_combo2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a);
    m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(
        m,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn is_none(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none")
}
fn is_some(v: &Value) -> bool {
    matches!(v, Value::Combo(ref cv) if cv.get_field("%val").is_some())
}
fn is_ok(v: &Value) -> bool {
    matches!(v, Value::Combo(ref cv) if cv.get_field("%val").is_some() && cv.get_field("%cause").is_none())
}
fn is_err(v: &Value) -> bool {
    matches!(v, Value::Combo(ref cv) if cv.get_field("%cause").is_some())
}
fn unwrap_val(v: &Value) -> &Value {
    match v {
        Value::Combo(ref cv) => cv.get_field("%val").unwrap(),
        _ => panic!("not Some/Ok"),
    }
}

// ── option.zip ─────────────────────────────────────────────────────

#[test]
fn test_option_zip_both_some() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "option.zip",
        make_combo2(some(int(1)), some(int(2))),
    );
    assert!(is_some(&r));
    if let Value::Combo(ref pair) = *unwrap_val(&r) {
        assert_eq!(pair.get_field("0").unwrap().to_string_plain(), "1");
        assert_eq!(pair.get_field("1").unwrap().to_string_plain(), "2");
    } else {
        panic!("inner should be Combo pair");
    }
}

#[test]
fn test_option_zip_first_none() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "option.zip",
        make_combo2(none(), some(int(2))),
    );
    assert!(is_none(&r));
}

#[test]
fn test_option_zip_second_none() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "option.zip",
        make_combo2(some(int(1)), none()),
    );
    assert!(is_none(&r));
}

// ── option.flatten ─────────────────────────────────────────────────

#[test]
fn test_option_flatten_nested_some() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let nested = some(some(int(42)));
    let r = call(&oo, &mut ctx, "option.flatten", nested);
    assert!(is_some(&r));
    assert_eq!(unwrap_val(&r).to_string_plain(), "42");
}

#[test]
fn test_option_flatten_outer_none() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "option.flatten", none());
    assert!(is_none(&r));
}

#[test]
fn test_option_flatten_inner_none() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "option.flatten", some(none()));
    assert!(is_none(&r));
}

// ── result.and ─────────────────────────────────────────────────────

#[test]
fn test_result_and_both_ok_returns_second() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "result.and",
        make_combo2(ok(int(2)), ok(int(1))),
    );
    assert!(is_ok(&r));
    assert_eq!(unwrap_val(&r).to_string_plain(), "2");
}

#[test]
fn test_result_and_first_err_propagates() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "result.and",
        make_combo2(ok(int(2)), err("boom")),
    );
    assert!(is_err(&r));
}

// ── result.or ──────────────────────────────────────────────────────

#[test]
fn test_result_or_ok_returns_self() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "result.or",
        make_combo2(err("fallback"), ok(int(1))),
    );
    assert!(is_ok(&r));
    assert_eq!(unwrap_val(&r).to_string_plain(), "1");
}

#[test]
fn test_result_or_err_uses_fallback() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "result.or",
        make_combo2(ok(int(99)), err("boom")),
    );
    assert!(is_ok(&r));
    assert_eq!(unwrap_val(&r).to_string_plain(), "99");
}

// ── result.flatten ─────────────────────────────────────────────────

#[test]
fn test_result_flatten_ok_ok() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "result.flatten", ok(ok(int(42))));
    assert!(is_ok(&r));
    assert_eq!(unwrap_val(&r).to_string_plain(), "42");
}

#[test]
fn test_result_flatten_outer_err() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "result.flatten", err("outer"));
    assert!(is_err(&r));
}

#[test]
fn test_result_flatten_ok_err() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "result.flatten", ok(err("inner")));
    assert!(is_err(&r));
}
