use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}

#[test]
fn test_process_pid_returns_positive_int() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "process.pid", Value::Top);
    assert!(matches!(r, Value::Atom(AtomKind::Int(_), EffectTag::IO, _)));
    if let Value::Atom(AtomKind::Int(n), _, _) = r {
        assert!(n > BigInt::from(0i64), "PID must be positive");
    }
}

#[test]
fn test_process_pid_effect_is_io() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "process.pid", Value::Top);
    assert!(matches!(r, Value::Atom(_, EffectTag::IO, _)));
}

#[test]
fn test_process_exit_registered() {
    let oo = make_oo();
    assert!(oo.builtin_registry.get("process.exit").is_some());
}

#[test]
fn test_process_pid_consistent() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r1 = call(&oo, &mut ctx, "process.pid", Value::Top);
    let r2 = call(&oo, &mut ctx, "process.pid", Value::Top);
    match (r1, r2) {
        (Value::Atom(AtomKind::Int(n1), _, _), Value::Atom(AtomKind::Int(n2), _, _)) => assert_eq!(n1, n2),
        _ => panic!("expected two Int values"),
    }
}
