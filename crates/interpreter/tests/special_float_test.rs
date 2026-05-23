use nlang_interpreter::{Ouroboros, Value, ComboVal, EvalContext, EffectTag, BottomCause};
use nlang_parser::parse_program;
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use num_bigint::BigInt;
use num_traits::Zero;

fn setup() -> Ouroboros {
    Ouroboros::new_in_memory()
}

fn empty_ctx() -> EvalContext {
    EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]))
}

#[test]
fn test_division_by_zero_int() {
    let input = "res: 1 / 0";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("1 / 0 = {:?}", val);
    match val {
        Value::Atom(AtomKind::Int(i), _, _) if i == BigInt::zero() => {}
        other => panic!("FAIL: expected 0 (div by zero protection), got {:?}", other),
    }
}

#[test]
fn test_division_by_zero_float() {
    let input = "res: 1.0 / 0.0";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("1.0 / 0.0 = {:?}", val);
    match val {
        Value::Atom(AtomKind::TagEnd, _, _) => {}
        other => panic!("FAIL: expected #_ (+Inf), got {:?}", other),
    }
}

#[test]
fn test_negative_division_by_zero() {
    let input = "res: -1.0 / 0.0";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("-1.0 / 0.0 = {:?}", val);
    match val {
        Value::Atom(AtomKind::TagStart, _, _) => {}
        other => panic!("FAIL: expected #_|_ (-Inf), got {:?}", other),
    }
}

#[test]
fn test_zero_divided_by_zero() {
    let input = "res: 0.0 / 0.0";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("0.0 / 0.0 = {:?}", val);
    match val {
        Value::Bottom(d) => {
            assert_eq!(d.cause, BottomCause::NumericalError);
        }
        other => panic!("FAIL: expected _|_ (NaN), got {:?}", other),
    }
}

#[test]
fn test_sqrt_negative() {
    let input = "res: -1.0 |> ~%Math./sqrt";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("sqrt(-1.0) = {:?}", val);
}

#[test]
fn test_overflow_to_infinity() {
    let oo = setup();
    let mut ctx = empty_ctx();
    
    let large_val = Value::Atom(AtomKind::Float(1e308), EffectTag::Pure, None);
    let result = large_val.clone();
    
    let mul_result = 1e308_f64 * 1e308_f64;
    println!("Rust: 1e308 * 1e308 = {} (is_inf={}, is_nan={})", mul_result, mul_result.is_infinite(), mul_result.is_nan());
}

#[test]
fn test_inf_arithmetic_prohibited() {
    let input = "res: #_ + 1";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("#_ + 1 = {:?}", val);
    match val {
        Value::Bottom(_) => {}
        other => panic!("FAIL: expected _|_ (arithmetic on order anchor), got {:?}", other),
    }
}