use indexmap::IndexMap;
use nlang_interpreter::{
    BottomCause, ComboVal, EffectTag, EvalContext, ObservationStrategy, Ouroboros, Value,
};
use nlang_parser::parse_program;

#[test]
fn test_static_cycle() {
    let input = "x: { a: b, b: a }";
    let program = parse_program(input).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(
        IndexMap::new(),
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));

    let res = oo.eval_observed(&program.fields[0].value, &mut ctx);
    if let Value::Combo(cv) = res {
        let a_val = cv.get_field("a").unwrap().clone();
        let forced_a = oo.force(a_val, &mut ctx);
        assert_eq!(forced_a, Value::Top);
    } else {
        panic!("Expected Combo");
    }
}

#[test]
fn test_fuel_exhausted_strict_mode() {
    let input = "x: 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1";
    let program = parse_program(input).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(
        IndexMap::new(),
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
    .with_fuel(2)
    .with_strategy(ObservationStrategy::Strict);

    let res = oo.eval_observed(&program.fields[0].value, &mut ctx);
    match res {
        Value::Bottom(d) => {
            assert_eq!(d.cause, BottomCause::FuelExhausted);
        }
        other => panic!(
            "Expected Bottom(FuelExhausted) in strict mode, got {:?}",
            other
        ),
    }
}

#[test]
fn test_fuel_exhausted_blur_mode() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(
        IndexMap::new(),
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
    .with_fuel(1)
    .with_strategy(ObservationStrategy::Blur);

    let input = "x: 1 + 2";
    let program = parse_program(input).unwrap();

    let res = oo.eval_observed(&program.fields[0].value, &mut ctx);
    match res {
        // First-class Value::Blur (Phase 9 / G3 horizon value).
        Value::Blur(bd) => {
            assert!(
                matches!(bd.cause, nlang_interpreter::BlurCause::FuelExhausted),
                "expected fuel_exhausted blur, got {:?}",
                bd.cause
            );
        }
        Value::Combo(cv) => {
            // Legacy combo-shaped #blur (if any residual path).
            let kind = cv.get_field("%kind");
            assert!(kind
                .map(|k| k.to_string_plain().trim_start_matches('#') == "blur")
                .unwrap_or(false));
        }
        Value::Bottom(_) => {
            // Acceptable if computation was very cheap relative to fuel=1.
        }
        other => panic!("Expected Blur or Bottom, got {:?}", other),
    }
}
