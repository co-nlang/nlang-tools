use indexmap::IndexMap;
use nlang_interpreter::{ComboVal, EffectTag, EvalContext, Ouroboros, Value};
use nlang_parser::ast::AtomKind;
use nlang_parser::parse_program;
use num_bigint::BigInt;

fn setup() -> Ouroboros {
    Ouroboros::new_in_memory()
}

fn empty_ctx() -> EvalContext {
    EvalContext::new(ComboVal::new(
        IndexMap::new(),
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

// Stage 2 (call-by-observation): eval_observed is the observation API
// (eval + force_recursive); eval returns pre-observation structure.
fn eval_observed(oo: &Ouroboros, src: &str) -> Value {
    let program = parse_program(src).unwrap();
    let mut ctx = empty_ctx();
    oo.eval_observed(&program.fields[0].value, &mut ctx)
}

#[test]
fn test_dispatch_anonymous_morphism_basic() {
    let input = "res: (x -> x + 1) 5";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("{} = {:?}", input, val);
    match val {
        Value::Atom(AtomKind::Int(i), _, _) if i == BigInt::from(6) => {}
        other => panic!("FAIL: expected 6, got {:?}", other),
    }
}

#[test]
fn test_dispatch_no_match_returns_bottom() {
    let val = eval_observed(&setup(), r#"res: { 1: "one", 2: "two" } 3"#);
    println!("no match = {:?}", val);
    match val {
        Value::Bottom(_) => {}
        other => panic!("FAIL: expected _|_ (no matching branch), got {:?}", other),
    }
}

#[test]
fn test_dispatch_exact_key_lookup() {
    let val = eval_observed(&setup(), r#"res: { 1: "one", 2: "two" } 1"#);
    println!("exact key lookup = {:?}", val);
    match val {
        Value::Atom(AtomKind::Str(s), _, _) if s == "one" => {}
        other => panic!("FAIL: expected \"one\", got {:?}", other),
    }
}

#[test]
fn test_dispatch_it_fallback() {
    let val = eval_observed(&setup(), r#"res: { it: "default" } 42"#);
    println!("it fallback = {:?}", val);
    match val {
        Value::Atom(AtomKind::Str(s), _, _) if s == "default" => {}
        other => panic!("FAIL: expected \"default\" (it fallback), got {:?}", other),
    }
}

#[test]
fn test_dispatch_wildcard_fallback() {
    let val = eval_observed(&setup(), r#"res: { _: "any" } 42"#);
    println!("wildcard fallback = {:?}", val);
    match val {
        Value::Atom(AtomKind::Str(s), _, _) if s == "any" => {}
        other => panic!("FAIL: expected \"any\" (_ fallback), got {:?}", other),
    }
}
