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

#[test]
fn test_toml_parse_basic() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let toml_str = "name = \"Alice\"\nage = 30\n";
    let r = call(&oo, &mut ctx, "toml.parse", str_val(toml_str));
    if let Value::Combo(c) = &r {
        assert!(
            matches!(c.get_field("name"), Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "Alice")
        );
    } else {
        panic!("expected Combo");
    }
}

#[test]
fn test_toml_parse_nested_table() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let toml_str = "[server]\nhost = \"localhost\"\nport = 8080\n";
    let r = call(&oo, &mut ctx, "toml.parse", str_val(toml_str));
    if let Value::Combo(c) = &r {
        let server = c.get_field("server").expect("server table");
        if let Value::Combo(sc) = server {
            assert!(
                matches!(sc.get_field("host"), Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "localhost")
            );
        } else {
            panic!("server not Combo");
        }
    } else {
        panic!("expected Combo");
    }
}

#[test]
fn test_toml_parse_invalid_returns_bottom() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "toml.parse", str_val("invalid = ==="));
    assert!(matches!(&r, Value::Bottom(_)));
}
