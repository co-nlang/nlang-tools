// Blur display-order key probes (2026-07-18, pre-committed by work
// order — docs/blur_display_key_handover.md). Case: two-blur union
// display order is cross-process NONDETERMINISTIC — exposed at the
// display-order arc acceptance (drafting hole in the original §2.4.1
// blur clause, acceptor's own).
//
// MEASURED disease (post-7a515bb): blur branches sort by their
// to_nlang display string, which embeds the salted %caid — the salt
// leaks back into the sort key. Two equal-cause blurs flip order
// across CLI runs (measured twice, order swapped). Exactly what the
// "no CAID/digest keys" clause was written to prevent.
//
// LAW (SPEC_01 §2.4.1 item 5, amended 2026-07-18):
//   #blur intra-family key = (%cause name lex, fuel_remaining asc,
//   strategy) — %caid/salt EXPLICITLY EXCLUDED. Ties are stable
//   (encounter order kept). Display layer only, as before.
//
// PROBE SHAPE — salt-proof both-permutation gates: feed
// canonical_display_order both input orders; under the amended law
// each must come out in the LAW order regardless of what the salted
// caids happen to be. Today the string sort imposes caid order on
// exactly one of the two permutations → deterministically red.
//
// NOT in scope: blur DISPLAY text (still prints %caid — only the
// sort key changes); blur absorption/unify laws (SPEC_08 §3.2.2);
// bn_serial (salt stays IN identity, by design); non-blur families'
// keys (delivered arc, pinned there).

use nlang_interpreter::value::{
    canonical_display_order, BlurCause, BlurDetail, ContentHash,
    EffectTag, HorizonParams, ObservationStrategy, Value,
};
use nlang_parser::ast::AtomKind;

fn blur(cause: BlurCause, fuel: u64, salt_byte: u8) -> Value {
    Value::Blur(BlurDetail {
        cause,
        horizon: HorizonParams {
            fuel_remaining: fuel,
            strategy: ObservationStrategy::Blur,
            salt: ContentHash::v1(vec![salt_byte; 32]),
        },
        partial: None,
        effect: EffectTag::Pure,
    })
}

/// Render an ordered branch list to its display-order salt signature:
/// the sequence of salts' first bytes, so tests can check WHICH blur
/// came out where without depending on display text internals.
fn order_sig(ordered: &[&Value]) -> Vec<u64> {
    ordered
        .iter()
        .map(|v| match v {
            Value::Blur(bd) => bd.horizon.fuel_remaining,
            _ => panic!("blur-only probe"),
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — blur sort key must be (cause, fuel, strategy), salt-blind
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_blur_tie_is_stable_both_permutations() {
    // Same cause, same fuel, different salts → key ties → STABLE:
    // encounter order preserved for BOTH input permutations. Today the
    // caid-string sort forces caid order onto exactly one permutation.
    let a = blur(BlurCause::FuelExhausted, 0, 0x11);
    let b = blur(BlurCause::FuelExhausted, 0, 0xEE);
    let ab = [a.clone(), b.clone()];
    let ba = [b, a];
    let oab = canonical_display_order(&ab);
    let oba = canonical_display_order(&ba);
    assert!(
        std::ptr::eq(oab[0], &ab[0]) && std::ptr::eq(oab[1], &ab[1]),
        "tied blurs must keep encounter order (a,b input)"
    );
    assert!(
        std::ptr::eq(oba[0], &ba[0]) && std::ptr::eq(oba[1], &ba[1]),
        "tied blurs must keep encounter order (b,a input)"
    );
}

#[test]
fn pin_blur_fuel_orders_lucky_salts() {
    // CALIBRATION FINDING: with these salts the sha-derived caid order
    // coincidentally matches fuel order → green today for the WRONG
    // reason. Kept as an ACTIVE pin (E1-E3 lesson (i)); the salt-proof
    // red gate is red_blur_fuel_orders_adversarial_salts below.
    let f3 = blur(BlurCause::FuelExhausted, 3, 0x22);
    let f5 = blur(BlurCause::FuelExhausted, 5, 0xDD);
    for pair in [[f3.clone(), f5.clone()], [f5, f3]] {
        let ordered = canonical_display_order(&pair);
        assert_eq!(
            order_sig(&ordered),
            vec![3, 5],
            "blur fuel must order ascending regardless of input order/salt"
        );
    }
}

#[test]
fn red_blur_fuel_orders_adversarial_salts() {
    // Salt bytes chosen OPPOSITE to fuel order — a salt-leaking key
    // sorts these wrong in at least one permutation.
    let f3 = blur(BlurCause::FuelExhausted, 3, 0xFE);
    let f5 = blur(BlurCause::FuelExhausted, 5, 0x01);
    for pair in [[f3.clone(), f5.clone()], [f5, f3]] {
        let ordered = canonical_display_order(&pair);
        assert_eq!(order_sig(&ordered), vec![3, 5]);
    }
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — adjacent law that must not move
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_blur_cause_orders_lex() {
    // Different causes order by cause name — green today (display
    // string prefix reaches %cause before %caid) and stays law.
    let fe = blur(BlurCause::FuelExhausted, 0, 0x99);
    let to = blur(BlurCause::Timeout, 0, 0x02);
    for pair in [[fe.clone(), to.clone()], [to, fe]] {
        let ordered = canonical_display_order(&pair);
        match ordered[0] {
            Value::Blur(bd) => assert_eq!(bd.cause, BlurCause::FuelExhausted),
            _ => unreachable!(),
        }
    }
}

#[test]
fn pin_blur_after_solid_before_top() {
    // Family rank unchanged: value < blur < Top.
    let two = Value::Atom(AtomKind::Int(2.into()), EffectTag::Pure, None);
    let b = blur(BlurCause::FuelExhausted, 0, 0x33);
    let branches = [b, two, Value::Top];
    let ordered = canonical_display_order(&branches);
    assert!(matches!(ordered[0], Value::Atom(..)));
    assert!(matches!(ordered[1], Value::Blur(_)));
    assert!(matches!(ordered[2], Value::Top));
}

#[test]
fn pin_blur_display_text_untouched() {
    // Only the KEY changes — blur display still prints its %caid.
    let b = blur(BlurCause::FuelExhausted, 0, 0x44);
    let s = b.to_nlang(0);
    assert!(
        s.contains("#blur") && s.contains("%caid"),
        "blur display text must keep printing %caid: {s:?}"
    );
}

#[test]
fn pin_blur_caid_still_salted() {
    // Identity is NOT display: blur CAID keeps its salt (bn_serial /
    // blur_caid untouched by this arc).
    let b1 = blur(BlurCause::FuelExhausted, 0, 0x55);
    let b2 = blur(BlurCause::FuelExhausted, 0, 0x66);
    match (&b1, &b2) {
        (Value::Blur(d1), Value::Blur(d2)) => {
            assert_ne!(d1.blur_caid(), d2.blur_caid());
        }
        _ => unreachable!(),
    }
}
