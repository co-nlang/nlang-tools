use indexmap::IndexMap;
use nlang_interpreter::value::ComboVal;
use nlang_interpreter::value::{EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;

fn make_oo() -> Ouroboros {
    Ouroboros::new_in_memory()
}
fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}
fn combo1(a: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a);
    Value::Combo(ComboVal::new(
        m,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}
fn combo2(a: Value, b: Value) -> Value {
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
fn as_str(v: &Value) -> &str {
    match v {
        Value::Atom(AtomKind::Str(s), _, _) => s,
        o => panic!("expected Str: {:?}", o),
    }
}
fn is_none(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none")
}
fn is_true(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "true")
}
fn is_false(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "false")
}
fn is_pure(v: &Value) -> bool {
    matches!(v, Value::Atom(_, EffectTag::Pure, _))
}

#[test]
fn test_path_join_basic() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "path.join",
        combo2(str_val("/foo"), str_val("bar")),
    );
    assert_eq!(as_str(&r), "/foo/bar");
    assert!(is_pure(&r));
}

#[test]
fn test_path_join_nested() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "path.join",
        combo2(str_val("/a/b"), str_val("c/d")),
    );
    assert_eq!(as_str(&r), "/a/b/c/d");
}

#[test]
fn test_path_dirname_basic() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "path.dirname",
        combo1(str_val("/foo/bar.txt")),
    );
    assert_eq!(as_str(&r), "/foo");
    assert!(is_pure(&r));
}

#[test]
fn test_path_dirname_root_returns_none() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "path.dirname", combo1(str_val("/")));
    assert!(is_none(&r));
}

#[test]
fn test_path_basename_basic() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "path.basename",
        combo1(str_val("/foo/bar.txt")),
    );
    assert_eq!(as_str(&r), "bar.txt");
    assert!(is_pure(&r));
}

#[test]
fn test_path_basename_root_returns_none() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "path.basename", combo1(str_val("/")));
    assert!(is_none(&r));
}

#[test]
fn test_path_extension_with_ext() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "path.extension",
        combo1(str_val("/foo/bar.txt")),
    );
    assert_eq!(as_str(&r), "txt");
    assert!(is_pure(&r));
}

#[test]
fn test_path_extension_no_ext() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "path.extension", combo1(str_val("/foo/bar")));
    assert!(is_none(&r));
}

#[test]
fn test_path_extension_dotfile_has_no_ext() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "path.extension", combo1(str_val(".hidden")));
    assert!(is_none(&r));
}

#[test]
fn test_path_is_absolute_true() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "path.is_absolute",
        combo1(str_val("/foo/bar")),
    );
    assert!(is_true(&r));
    assert!(is_pure(&r));
}

#[test]
fn test_path_is_absolute_false() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "path.is_absolute",
        combo1(str_val("foo/bar")),
    );
    assert!(is_false(&r));
}
