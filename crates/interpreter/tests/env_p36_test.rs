use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use nlang_interpreter::value::ComboVal;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn combo1(a: Value) -> Value {
    let mut m = IndexMap::new(); m.insert("0".to_string(), a);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn is_none(v: &Value) -> bool { matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none") }
fn as_str(v: &Value) -> &str { match v { Value::Atom(AtomKind::Str(s), _, _) => s, o => panic!("expected Str: {:?}", o) } }
fn is_list(v: &Value) -> bool {
    if let Value::Combo(c) = v {
        matches!(c.get_field("%kind"), Some(Value::Atom(AtomKind::Tag(t), _, _)) if t == "list")
    } else { false }
}

#[test]
fn test_env_get_existing() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    std::env::set_var("NLANG_TEST_VAR_P36", "nlang_value_xyz");
    let r = call(&oo, &mut ctx, "env.get", combo1(str_val("NLANG_TEST_VAR_P36")));
    assert!(matches!(&r, Value::Atom(AtomKind::Str(_), EffectTag::IO, _)));
    assert_eq!(as_str(&r), "nlang_value_xyz");
}

#[test]
fn test_env_get_nonexistent_returns_none() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "env.get", combo1(str_val("NLANG_DEFINITELY_NOT_SET_ABCXYZ123")));
    assert!(is_none(&r));
    assert!(matches!(r, Value::Atom(_, EffectTag::IO, _)));
}

#[test]
fn test_env_args_returns_list() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "env.args", Value::Top);
    assert!(is_list(&r));
    assert!(matches!(r, Value::Combo(_)));
    if let Value::Combo(ref c) = r {
        assert!(c.get_field("0").is_some(), "env.args must return at least argv[0]");
        assert!(matches!(c.get_field("0").unwrap(), Value::Atom(AtomKind::Str(_), EffectTag::IO, _)));
    }
}

#[test]
fn test_env_args_effect_is_io() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "env.args", Value::Top);
    assert!(matches!(r, Value::Combo(ref c) if c.effect == EffectTag::IO));
}

#[test]
fn test_env_cwd_returns_str() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "env.cwd", Value::Top);
    assert!(matches!(r, Value::Atom(AtomKind::Str(_), EffectTag::IO, _)));
    let s = as_str(&r);
    assert!(!s.is_empty());
}

#[test]
fn test_env_cwd_effect_is_io() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "env.cwd", Value::Top);
    assert!(matches!(r, Value::Atom(_, EffectTag::IO, _)));
}
