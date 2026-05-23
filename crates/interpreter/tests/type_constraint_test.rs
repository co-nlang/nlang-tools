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
fn test_int_value_meet_int_type() {
    let input = "result: 123 & @int";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("123 & @int = {:?}", val);
    match val {
        Value::Atom(AtomKind::Int(i), _, _) if i == BigInt::from(123) => {}
        Value::Bottom(_) => panic!("FAIL: 123 & @int should be 123, got _|_"),
        other => panic!("FAIL: expected 123, got {:?}", other),
    }
}

#[test]
fn test_int_value_meet_str_type() {
    let input = "result: 123 & @str";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("123 & @str = {:?}", val);
    match val {
        Value::Bottom(_) => {}
        other => panic!("FAIL: expected _|_, got {:?}", other),
    }
}

#[test]
fn test_str_value_meet_str_type() {
    let input = "result: \"hello\" & @str";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("\"hello\" & @str = {:?}", val);
    match val {
        Value::Atom(AtomKind::Str(s), _, _) if s == "hello" => {}
        other => panic!("FAIL: expected \"hello\", got {:?}", other),
    }
}

#[test]
fn test_float_value_meet_float_type() {
    let input = "result: 3.14 & @float";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("3.14 & @float = {:?}", val);
    match val {
        Value::Atom(AtomKind::Float(_), _, _) => {}
        other => panic!("FAIL: expected float, got {:?}", other),
    }
}

#[test]
fn test_int_value_meet_num_type() {
    let input = "result: 42 & @num";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("42 & @num = {:?}", val);
    match val {
        Value::Atom(AtomKind::Int(i), _, _) if i == BigInt::from(42) => {}
        other => panic!("FAIL: expected 42, got {:?}", other),
    }
}

#[test]
fn test_bool_value_meet_bool_type() {
    let input = "result: #true & @bool";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("#true & @bool = {:?}", val);
    match val {
        Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "true" => {}
        other => panic!("FAIL: expected #true, got {:?}", other),
    }
}

#[test]
fn test_int_value_meet_float_type_projects() {
    let input = "result: 123 & @float";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("123 & @float = {:?}", val);
    match val {
        Value::Atom(AtomKind::Float(f), _, _) if f == 123.0 => {}
        Value::Atom(AtomKind::Int(i), _, _) if i == BigInt::from(123) => {}
        other => panic!("FAIL: expected 123 (as float projection), got {:?}", other),
    }
}

#[test]
fn test_float_value_meet_complex_type() {
    let input = "result: 3.14 & @complex";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("3.14 & @complex = {:?}", val);
    match val {
        Value::Atom(AtomKind::Float(_), _, _) => {}
        other => panic!("FAIL: expected float (in @complex), got {:?}", other),
    }
}

#[test]
fn test_int_value_meet_complex_type() {
    let input = "result: 42 & @complex";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("42 & @complex = {:?}", val);
    match val {
        Value::Atom(AtomKind::Int(_), _, _) => {}
        other => panic!("FAIL: expected int (in @complex), got {:?}", other),
    }
}

#[test]
fn test_subtype_int_not_le_float() {
    let input = "result: @int <= @float";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("@int <= @float = {:?}", val);
    match val {
        Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "false" => {}
        Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "true" => {
            panic!("FAIL: @int <= @float should be #false, got #true")
        }
        other => panic!("FAIL: expected #false, got {:?}", other),
    }
}

#[test]
fn test_subtype_float_le_complex() {
    let input = "result: @float <= @complex";
    let program = parse_program(input).unwrap();
    let oo = setup();
    let mut ctx = empty_ctx();

    let val = oo.eval(&program.fields[0].value, &mut ctx);
    println!("@float <= @complex = {:?}", val);
    match val {
        Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "true" => {}
        other => panic!("FAIL: expected #true (@float ≤ @complex), got {:?}", other),
    }
}
