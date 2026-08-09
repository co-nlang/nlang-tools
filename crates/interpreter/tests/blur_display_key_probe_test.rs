// Blur display-order key probes.
//
// Original arc 2026-07-18 (docs/blur_display_key_handover.md): blur branches
// sorted by their display string, which embedded the salted %caid, so two
// equal-cause blurs flipped order across processes. SPEC_01 §2.4.1 item 5 was
// amended to key on (cause lex, fuel_remaining asc, strategy) and to FORBID
// CAID/digest keys — the salt was the reason for that prohibition.
//
// Rewritten under O42 (2026-08-09 delivery). O42 removes the salt, so a digest
// key is now a function of the value and the 2026-07-18 hazard is gone. The
// delivery's new key is (cause, strategy, CHS digest).
//
// ACCEPTOR'S NOTE (2026-08-10). Two things about that rewrite:
//
//   * §2.4.1's "禁止以 CAID/digest 作顯示排序鍵" is still on the books. The
//     new key is defensible post-O42 but the MUST NOT is amended by the
//     acceptor at spec closure, not routed around by a delivery.
//   * The rewrite dropped seven pins to four. Two died honestly with the salt
//     (`pin_blur_caid_still_salted`, `red_blur_fuel_orders_adversarial_salts`).
//     Two did NOT — family rank and display text are not about the salt — and
//     are restored below. Recorded because deleting a pin is the one edit that
//     leaves no failing test behind to notice it.
//
// No literal digest is pinned here: the O42 repair (partial enters the
// identity by CAID, not inlined) moves every blur CAID a second time. Literals
// are re-pinned at final acceptance.

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

// ── restored 2026-08-10 (acceptor) — dropped by the O42 delivery, but neither
//    of these is about the salt ────────────────────────────────────────────

/// Family rank is unchanged: solid value < blur < Top.
///
/// SPEC_01 §2.4.1's family order (item 4 structural, item 5 blur, item 6 Top
/// last). Nothing in O42 touches it, and the intra-family key rewrite is
/// exactly the kind of edit that can move a branch across family boundaries
/// without any other test noticing.
#[test]
fn pin_blur_after_solid_before_top() {
    let two = Value::Atom(AtomKind::Int(2.into()), EffectTag::Pure, None);
    let b = blur(BlurCause::FuelExhausted, 0);
    let branches = [b, two, Value::Top];
    let ordered = canonical_display_order(&branches);
    assert!(matches!(ordered[0], Value::Atom(..)), "solid value first");
    assert!(matches!(ordered[1], Value::Blur(_)), "blur second");
    assert!(matches!(ordered[2], Value::Top), "Top last");
}

/// Only the KEY changes — blur display text still prints its `%caid`.
///
/// The 2026-07-18 arc drew this line explicitly ("NOT in scope: blur DISPLAY
/// text"), and O42 does not move it either: what a blur's identity is made of
/// changed, whether it is shown did not.
#[test]
fn pin_blur_display_text_untouched() {
    let b = blur(BlurCause::FuelExhausted, 0);
    let s = b.to_nlang(0);
    assert!(
        s.contains("#blur") && s.contains("%caid"),
        "blur display text must keep printing %caid: {s:?}"
    );
}
