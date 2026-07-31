use indexmap::IndexMap;
use nlang_interpreter::value::{ComboVal, EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

fn make_oo() -> Ouroboros {
    Ouroboros::new_in_memory()
}

fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}
fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}

fn make_list(items: Vec<Value>) -> Value {
    let mut f = IndexMap::new();
    for (i, v) in items.iter().enumerate() {
        f.insert(i.to_string(), v.clone());
    }
    f.insert(
        "%kind".to_string(),
        Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
    );
    f.insert("%len".to_string(), int_val(items.len() as i64));
    Value::Combo(ComboVal::new(
        f,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

fn make_fmt_arg(fmt: &str, args: Vec<Value>) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), str_val(fmt));
    f.insert("1".to_string(), make_list(args));
    Value::Combo(ComboVal::new(
        f,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

fn call_format(oo: &Ouroboros, ctx: &mut EvalContext, fmt: &str, args: Vec<Value>) -> String {
    let f = oo.builtin_registry.get("str.format").unwrap().clone();
    let arg = make_fmt_arg(fmt, args);
    match f(arg, oo, ctx) {
        Value::Atom(AtomKind::Str(s), _, _) => s,
        other => panic!("expected Str, got {:?}", other),
    }
}

#[test]
fn test_str_format_single_placeholder() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call_format(&oo, &mut ctx, "Hello, {}!", vec![str_val("Alice")]);
    assert_eq!(r, "Hello, Alice!");
}

#[test]
fn test_str_format_multiple_placeholders() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call_format(
        &oo,
        &mut ctx,
        "{} + {} = {}",
        vec![int_val(1), int_val(2), int_val(3)],
    );
    assert_eq!(r, "1 + 2 = 3");
}

#[test]
fn test_str_format_explicit_index() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call_format(
        &oo,
        &mut ctx,
        "{1} then {0}",
        vec![str_val("first"), str_val("second")],
    );
    assert_eq!(r, "second then first");
}

#[test]
fn test_str_format_escape_braces() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call_format(&oo, &mut ctx, "{{literal}}", vec![]);
    assert_eq!(r, "{literal}");
}

#[test]
fn test_str_format_mixed_types() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call_format(&oo, &mut ctx, "val: {}", vec![int_val(42)]);
    assert_eq!(r, "val: 42");
}

#[test]
fn test_str_format_out_of_range() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call_format(&oo, &mut ctx, "{} {}", vec![str_val("only")]);
    assert_eq!(r, "only ");
}
