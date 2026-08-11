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
    // ACCEPTOR EDIT (the_meter_reads_two, 2026-08-11). This was a ten-term
    // arithmetic chain. Under the semantic MBU schedule, work whose extent is
    // already fixed by the supplied AST is not billed — `%fuel`'s job, per O41,
    // is that observation is guaranteed to TERMINATE, and a finite expression
    // evaluated once always does. A literal `1 + 1 + …` therefore no longer
    // exhausts anything, and the old fixture stopped testing exhaustion.
    //
    // Morphism application is billed (REAL_01 §9.1: 算子應用 = 10 MBU), so this
    // fixture crosses the horizon the way the test always meant to.
    let input = "x: (y -> y + 1) ((y -> y + 1) 1)";
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

    // ACCEPTOR EDIT (the_meter_reads_two, 2026-08-11) — same reason as the
    // strict-mode test above: `1 + 2` is AST-bounded and is no longer billed,
    // so it can no longer reach the horizon. Application is.
    let input = "x: (y -> y + 1) 1";
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
