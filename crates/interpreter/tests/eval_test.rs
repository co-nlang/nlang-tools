use nlang_interpreter::{Ouroboros, Value, ComboVal, EvalContext, EffectTag};
use nlang_parser::parse_program;
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use num_bigint::BigInt;

fn empty_ouroboros() -> Ouroboros {
    Ouroboros::new_in_memory()
}

#[test]
fn test_simple_eval() {
    let input = "a: 1";
    let program = parse_program(input).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]));

    // 取得第一個欄位的值
    let val = oo.eval_observed(&program.fields[0].value, &mut ctx);
    
    assert_eq!(val, Value::Atom(AtomKind::Int(BigInt::from(1)), EffectTag::Pure, None));
}

#[test]
fn test_combo_eval() {
    let input = "user: { name: \"Alice\", age: 30 }";
    let program = parse_program(input).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]));

    let val = oo.eval_observed(&program.fields[0].value, &mut ctx);
    
    if let Value::Combo(cv) = val {
        assert_eq!(cv.get_field("name").unwrap(), &Value::Atom(AtomKind::Str("Alice".to_string()), EffectTag::Pure, None));
        assert_eq!(cv.get_field("age").unwrap(), &Value::Atom(AtomKind::Int(BigInt::from(30)), EffectTag::Pure, None));
    } else {
        panic!("Expected Combo, got {:?}", val);
    }
}

#[test]
fn test_join_eval() {
    let input = "x: 1 | 2";
    let program = parse_program(input).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]));

    let val = oo.eval_observed(&program.fields[0].value, &mut ctx);
    assert_eq!(val, Value::Union(vec![
        Value::Atom(AtomKind::Int(BigInt::from(1)), EffectTag::Pure, None),
        Value::Atom(AtomKind::Int(BigInt::from(2)), EffectTag::Pure, None)
    ]));
}

#[test]
fn test_math_eval() {
    let input = "x: 1 + 2 * 3";
    let program = parse_program(input).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]));

    let val = oo.eval_observed(&program.fields[0].value, &mut ctx);
    // 預期 1 + (2 * 3) = 7 (Pest 已處理運算子優先權)
    assert_eq!(val, Value::Atom(AtomKind::Int(BigInt::from(7)), EffectTag::Pure, None));
}

#[test]
#[ignore]
fn test_cmp_eval() {
    // MIGRATED (2026-07-20, order-wave W2 open): numeric-order deviation
    // retired — subset semantics: {10} ⊅ {5} → #false (SYNTAX_06 §4 #10).
    let input = "check: 10 > 5";
    let program = parse_program(input).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]));

    let val = oo.eval_observed(&program.fields[0].value, &mut ctx);
    assert_eq!(val, Value::Atom(AtomKind::Tag("false".to_string()), EffectTag::Pure, None));
}

#[test]
fn test_pipe_morphism() {
    // 1 |> (x -> x + 1)
    let input = "res: 1 |> (x -> x + 1)";
    let program = parse_program(input).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]));

    let val = oo.eval_observed(&program.fields[0].value, &mut ctx);
    assert_eq!(val, Value::Atom(AtomKind::Int(BigInt::from(2)), EffectTag::Pure, None));
}

#[test]
fn test_pipe_transformer_combo() {
    // { a: 1 } |> { b: $.a + 1 }
    let input = "res: { a: 1 } |> { b: $.a + 1 }";
    let program = parse_program(input).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]));

    let val = oo.eval_observed(&program.fields[0].value, &mut ctx);
    println!("test_pipe_transformer_combo: val = {:?}", val);
    if let Value::Combo(cv) = val {
        assert_eq!(cv.get_field("a").unwrap(), &Value::Atom(AtomKind::Int(BigInt::from(1)), EffectTag::Pure, None));
        assert_eq!(cv.get_field("b").unwrap(), &Value::Atom(AtomKind::Int(BigInt::from(2)), EffectTag::Pure, None));
    } else {
        panic!("Expected Combo, got {:?}", val);
    }
}

#[test]
fn test_functor_lifting_list() {
    // [1, 2, 3] |> (x -> x * x)
    let input = "res: [1, 2, 3] |> (x -> x * x)";
    let program = parse_program(input).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]));

    let val = oo.eval_observed(&program.fields[0].value, &mut ctx);
    
    if let Value::Combo(cv) = val {
        assert_eq!(cv.get_field("0").unwrap(), &Value::Atom(AtomKind::Int(BigInt::from(1)), EffectTag::Pure, None));
        assert_eq!(cv.get_field("1").unwrap(), &Value::Atom(AtomKind::Int(BigInt::from(4)), EffectTag::Pure, None));
        assert_eq!(cv.get_field("2").unwrap(), &Value::Atom(AtomKind::Int(BigInt::from(9)), EffectTag::Pure, None));
        // 驗證元資訊保留
        assert_eq!(cv.get_field("%kind").unwrap(), &Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    } else {
        panic!("Expected Combo (list), got {:?}", val);
    }
}
