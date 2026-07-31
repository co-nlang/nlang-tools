use nlang_interpreter::value::{EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;

fn oo() -> Ouroboros {
    Ouroboros::new_in_memory()
}
fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn list_len(v: &Value) -> usize {
    if let Value::Combo(c) = v {
        (0u32..)
            .take_while(|i| c.get_field(&i.to_string()).is_some())
            .count()
    } else {
        0
    }
}

#[test]
fn test_csv_parse_basic() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "csv.parse", str_val("a,b,c\n1,2,3"));
    assert_eq!(list_len(&r), 2);
    if let Value::Combo(c) = &r {
        let row0 = c.get_field("0").expect("row 0");
        assert_eq!(list_len(row0), 3);
    }
}

#[test]
fn test_csv_parse_with_headers() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let r = call(
        &oo,
        &mut ctx,
        "csv.parse_with_headers",
        str_val("name,age\nAlice,30\nBob,25"),
    );
    assert_eq!(list_len(&r), 2);
    if let Value::Combo(c) = &r {
        let rec0 = c.get_field("0").expect("record 0");
        if let Value::Combo(rc) = rec0 {
            assert!(
                matches!(rc.get_field("name"), Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "Alice")
            );
        }
    }
}

#[test]
fn test_csv_stringify_roundtrip() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let original = "a,b\n1,2";
    let parsed = call(&oo, &mut ctx, "csv.parse", str_val(original));
    let stringified = call(&oo, &mut ctx, "csv.stringify", parsed);
    assert!(matches!(&stringified, Value::Atom(AtomKind::Str(s), _, _) if s == original));
}

#[test]
fn test_csv_quoted_field() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "csv.parse", str_val("\"hello, world\",two"));
    if let Value::Combo(c) = &r {
        let row0 = c.get_field("0").expect("row");
        if let Value::Combo(rc) = row0 {
            assert!(
                matches!(rc.get_field("0"), Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "hello, world")
            );
        }
    }
}
