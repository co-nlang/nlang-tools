use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn int(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
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
fn as_str(v: &Value) -> &str {
    match v { Value::Atom(AtomKind::Str(s), _, _) => s, o => panic!("expected Str: {:?}", o) }
}
fn as_int(v: &Value) -> i64 {
    match v { Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(), o => panic!("expected Int: {:?}", o) }
}

#[test]
fn test_str_encode_decode_uri() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.encode_uri", combo1(str_val("hello world")));
    assert_eq!(as_str(&r), "hello%20world");
    let r = call(&oo, &mut ctx, "str.decode_uri", combo1(str_val("hello%20world")));
    assert_eq!(as_str(&r), "hello world");
}

#[test]
fn test_str_encode_uri_special_chars() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.encode_uri", combo1(str_val("a/b?c=d&e=f")));
    assert_eq!(as_str(&r), "a%2Fb%3Fc%3Dd%26e%3Df");
    let r = call(&oo, &mut ctx, "str.decode_uri", combo1(str_val("a%2Fb%3Fc%3Dd%26e%3Df")));
    assert_eq!(as_str(&r), "a/b?c=d&e=f");
}

#[test]
fn test_str_levenshtein() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.levenshtein", combo2(str_val("kitten"), str_val("sitting")));
    assert_eq!(as_int(&r), 3);
    let r = call(&oo, &mut ctx, "str.levenshtein", combo2(str_val("hello"), str_val("hello")));
    assert_eq!(as_int(&r), 0);
    let r = call(&oo, &mut ctx, "str.levenshtein", combo2(str_val(""), str_val("abc")));
    assert_eq!(as_int(&r), 3);
}

#[test]
fn test_str_word_count() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.word_count", combo1(str_val("hello world")));
    assert_eq!(as_int(&r), 2);
    let r = call(&oo, &mut ctx, "str.word_count", combo1(str_val("")));
    assert_eq!(as_int(&r), 0);
    let r = call(&oo, &mut ctx, "str.word_count", combo1(str_val("  leading and trailing  ")));
    assert_eq!(as_int(&r), 3);
}

#[test]
fn test_str_title_case() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.title_case", combo1(str_val("hello world")));
    assert_eq!(as_str(&r), "Hello World");
    let r = call(&oo, &mut ctx, "str.title_case", combo1(str_val("already Title")));
    assert_eq!(as_str(&r), "Already Title");
    let r = call(&oo, &mut ctx, "str.title_case", combo1(str_val("")));
    assert_eq!(as_str(&r), "");
}
