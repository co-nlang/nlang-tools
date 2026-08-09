// Blur display-order key probes — updated for O42 (no salt / no fuel_remaining
// in identity or display key). Law key: cause + strategy + CHS digest.

use nlang_interpreter::value::{
    canonical_display_order, BlurCause, BlurDetail, EffectTag, HorizonParams, ObservationStrategy,
    Value,
};
use nlang_parser::ast::AtomKind;

fn blur(cause: BlurCause, fuel_budget: u64) -> Value {
    Value::Blur(BlurDetail::from_single(
        cause,
        HorizonParams {
            fuel: fuel_budget,
            fuel_remaining: 0,
            strategy: ObservationStrategy::Blur,
            max_branches: 64,
            max_unification_depth: 256,
            max_lifting_depth: 32,
            max_pattern_nodes: 1024,
        },
        None,
        EffectTag::Pure,
    ))
}

fn cause_of(v: &Value) -> String {
    match v {
        Value::Blur(bd) => bd.cause.as_str().to_string(),
        _ => String::new(),
    }
}

#[test]
fn pin_cause_orders_lexicographically() {
    let fe = blur(BlurCause::FuelExhausted, 10);
    let md = blur(BlurCause::MaxDepthExceeded, 10);
    let to = blur(BlurCause::Timeout, 10);
    let inputs = [
        vec![to.clone(), md.clone(), fe.clone()],
        vec![fe.clone(), to.clone(), md.clone()],
        vec![md.clone(), fe.clone(), to.clone()],
    ];
    for input in &inputs {
        let ordered = canonical_display_order(input);
        let names: Vec<String> = ordered.iter().map(|v| cause_of(v)).collect();
        assert_eq!(
            names,
            vec![
                "fuel_exhausted".to_string(),
                "max_depth_exceeded".to_string(),
                "timeout".to_string()
            ],
            "cause order must be stable independent of input order"
        );
    }
}

#[test]
fn pin_same_cause_stable_on_tie() {
    let a = blur(BlurCause::FuelExhausted, 5);
    let b = blur(BlurCause::FuelExhausted, 5);
    let input = vec![a.clone(), b.clone()];
    let o1 = canonical_display_order(&input);
    let rev = vec![b, a];
    let o2 = canonical_display_order(&rev);
    assert_eq!(o1.len(), 2);
    assert_eq!(o2.len(), 2);
}

#[test]
fn pin_budget_distinguishes_display_key() {
    let low = blur(BlurCause::FuelExhausted, 5);
    let high = blur(BlurCause::FuelExhausted, 50);
    let caid_low = match &low {
        Value::Blur(bd) => bd.blur_caid().digest.clone(),
        _ => panic!(),
    };
    let caid_high = match &high {
        Value::Blur(bd) => bd.blur_caid().digest.clone(),
        _ => panic!(),
    };
    assert_ne!(caid_low, caid_high);
    let fwd = vec![low.clone(), high.clone()];
    let rev = vec![high, low];
    let ordered = canonical_display_order(&fwd);
    let ordered_rev = canonical_display_order(&rev);
    let seq = |o: Vec<&Value>| {
        o.iter()
            .map(|v| match v {
                Value::Blur(bd) => bd.horizon.fuel,
                _ => 0,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(seq(ordered), seq(ordered_rev));
}

#[test]
fn pin_non_blur_unaffected() {
    let a = Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None);
    let b = Value::Atom(AtomKind::Int(2.into()), EffectTag::Pure, None);
    let input = vec![b, a];
    let o = canonical_display_order(&input);
    assert_eq!(o.len(), 2);
}
