use indexmap::IndexMap;
use nlang_interpreter::value::ComboVal;
use nlang_interpreter::value::{EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;
use tempfile::tempdir;

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
fn is_true(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "true")
}
fn is_false(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "false")
}
fn is_none(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none")
}
fn as_str_content(v: &Value) -> &str {
    match v {
        Value::Atom(AtomKind::Str(s), _, _) => s,
        o => panic!("expected Str: {:?}", o),
    }
}

#[test]
fn test_io_write_and_read_roundtrip() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.txt").to_string_lossy().into_owned();

    let wrote = call(
        &oo,
        &mut ctx,
        "io.write_file",
        combo2(str_val(&path), str_val("hello nlang")),
    );
    assert!(is_true(&wrote));

    let content = call(&oo, &mut ctx, "io.read_file", combo1(str_val(&path)));
    assert_eq!(as_str_content(&content), "hello nlang");
    assert!(matches!(content, Value::Atom(_, EffectTag::IO, _)));
}

#[test]
fn test_io_read_nonexistent_returns_none() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "io.read_file",
        combo1(str_val("/nonexistent/path/that/cannot/exist/file.txt")),
    );
    assert!(is_none(&r));
}

#[test]
fn test_io_exists_true_and_false() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let dir = tempdir().unwrap();
    let path = dir
        .path()
        .join("exists_test.txt")
        .to_string_lossy()
        .into_owned();

    assert!(is_false(&call(
        &oo,
        &mut ctx,
        "io.exists",
        combo1(str_val(&path))
    )));

    call(
        &oo,
        &mut ctx,
        "io.write_file",
        combo2(str_val(&path), str_val("x")),
    );
    assert!(is_true(&call(
        &oo,
        &mut ctx,
        "io.exists",
        combo1(str_val(&path))
    )));
}

#[test]
fn test_io_write_truncates_existing() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let dir = tempdir().unwrap();
    let path = dir.path().join("trunc.txt").to_string_lossy().into_owned();

    call(
        &oo,
        &mut ctx,
        "io.write_file",
        combo2(str_val(&path), str_val("long content here")),
    );
    call(
        &oo,
        &mut ctx,
        "io.write_file",
        combo2(str_val(&path), str_val("short")),
    );

    let r = call(&oo, &mut ctx, "io.read_file", combo1(str_val(&path)));
    assert_eq!(as_str_content(&r), "short");
}

#[test]
fn test_io_append_file() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let dir = tempdir().unwrap();
    let path = dir.path().join("append.txt").to_string_lossy().into_owned();

    call(
        &oo,
        &mut ctx,
        "io.write_file",
        combo2(str_val(&path), str_val("hello ")),
    );
    let appended = call(
        &oo,
        &mut ctx,
        "io.append_file",
        combo2(str_val(&path), str_val("world")),
    );
    assert!(is_true(&appended));

    let r = call(&oo, &mut ctx, "io.read_file", combo1(str_val(&path)));
    assert_eq!(as_str_content(&r), "hello world");
}

#[test]
fn test_io_append_creates_if_absent() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let dir = tempdir().unwrap();
    let path = dir
        .path()
        .join("new_append.txt")
        .to_string_lossy()
        .into_owned();

    let r = call(
        &oo,
        &mut ctx,
        "io.append_file",
        combo2(str_val(&path), str_val("created")),
    );
    assert!(is_true(&r));

    let content = call(&oo, &mut ctx, "io.read_file", combo1(str_val(&path)));
    assert_eq!(as_str_content(&content), "created");
}
