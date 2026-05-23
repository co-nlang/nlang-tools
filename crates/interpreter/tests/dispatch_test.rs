use nlang_interpreter::{Ouroboros, Value, ComboVal, EvalContext, EffectTag};
use nlang_parser::parse_program;
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use num_bigint::BigInt;

fn setup() -> Ouroboros {
    Ouroboros::new_in_memory()
}

fn empty_ctx() -> EvalContext {
    EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]))
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
    let input = "res: { 1: \"one\", 2: \"two\" } 3";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("no match = {:?}", val);
    match val {
        Value::Bottom(_) => {}
        other => panic!("FAIL: expected _|_ (no matching branch), got {:?}", other),
    }
}

#[test]
fn test_dispatch_exact_key_lookup() {
    let input = "res: { 1: \"one\", 2: \"two\" } 1";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("exact key lookup = {:?}", val);
    match val {
        Value::Atom(AtomKind::Str(s), _, _) if s == "one" => {}
        other => panic!("FAIL: expected \"one\", got {:?}", other),
    }
}

#[test]
fn test_dispatch_it_fallback() {
    let input = "res: { it: \"default\" } 42";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("it fallback = {:?}", val);
    match val {
        Value::Atom(AtomKind::Str(s), _, _) if s == "default" => {}
        other => panic!("FAIL: expected \"default\" (it fallback), got {:?}", other),
    }
}

#[test]
fn test_dispatch_wildcard_fallback() {
    let input = "res: { _: \"any\" } 42";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("wildcard fallback = {:?}", val);
    match val {
        Value::Atom(AtomKind::Str(s), _, _) if s == "any" => {}
        other => panic!("FAIL: expected \"any\" (_ fallback), got {:?}", other),
    }
}