use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;

fn oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
#[allow(dead_code)]
fn args2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

#[test] fn test_url_parse_components() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "url.parse", str_val("https://example.com/path?key=val#frag"));
    if let Value::Combo(c) = &r {
        assert!(matches!(c.get_field("scheme"), Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "https"));
        assert!(matches!(c.get_field("host"),   Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "example.com"));
        assert!(matches!(c.get_field("path"),   Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "/path"));
    } else { panic!("expected Combo"); }
}

#[test] fn test_url_encode_decode_roundtrip() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let encoded = call(&oo, &mut ctx, "url.encode", str_val("hello world!"));
    let decoded = call(&oo, &mut ctx, "url.decode", encoded);
    assert!(matches!(&decoded, Value::Atom(AtomKind::Str(s), _, _) if s == "hello world!"));
}

#[test] fn test_url_query_params() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "url.query_params", str_val("https://x.com/?foo=1&bar=2"));
    if let Value::Combo(c) = &r {
        assert!(matches!(c.get_field("foo"), Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "1"));
        assert!(matches!(c.get_field("bar"), Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "2"));
    } else { panic!("expected Combo"); }
}
