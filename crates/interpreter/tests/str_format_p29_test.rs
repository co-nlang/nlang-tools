use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}
fn int(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}

fn named_combo(pairs: Vec<(&str, Value)>) -> Value {
    let mut m = IndexMap::new();
    for (k, v) in pairs { m.insert(k.to_string(), v); }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn format_arg(fmt: &str, args: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), str_val(fmt));
    m.insert("1".to_string(), args);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call_format(oo: &Ouroboros, ctx: &mut EvalContext, arg: Value) -> String {
    let r = oo.builtin_registry.get("str.format").unwrap().clone()(arg, oo, ctx);
    match r {
        Value::Atom(AtomKind::Str(s), _, _) => s,
        other => panic!("expected Str, got {:?}", other),
    }
}

// ── 命名佔位符基本功能 ────────────────────────────────────────────

#[test]
fn test_str_format_named_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let args = named_combo(vec![("name", str_val("Alice"))]);
    let r = call_format(&oo, &mut ctx, format_arg("Hi {name}!", args));
    assert_eq!(r, "Hi Alice!");
}

#[test]
fn test_str_format_named_multiple() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let args = named_combo(vec![("a", str_val("foo")), ("b", str_val("bar"))]);
    let r = call_format(&oo, &mut ctx, format_arg("{a} + {b}", args));
    assert_eq!(r, "foo + bar");
}

#[test]
fn test_str_format_named_int_value() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let args = named_combo(vec![("age", int(30))]);
    let r = call_format(&oo, &mut ctx, format_arg("age={age}", args));
    assert_eq!(r, "age=30");
}

#[test]
fn test_str_format_named_key_not_found_passthrough() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let args = named_combo(vec![("name", str_val("Alice"))]);
    let r = call_format(&oo, &mut ctx, format_arg("{name} {missing}", args));
    assert_eq!(r, "Alice {missing}");
}

#[test]
fn test_str_format_mixed_named_and_numeric() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let args = named_combo(vec![("0", str_val("Alice")), ("thing", str_val("pizza"))]);
    let r = call_format(&oo, &mut ctx, format_arg("{0} likes {thing}", args));
    assert_eq!(r, "Alice likes pizza");
}

#[test]
fn test_str_format_existing_list_still_works() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let mut m = IndexMap::new();
    m.insert("0".to_string(), str_val("hello"));
    m.insert("1".to_string(), str_val("world"));
    m.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    let list = Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]));
    let r = call_format(&oo, &mut ctx, format_arg("{} and {}", list));
    assert_eq!(r, "hello and world");
}
