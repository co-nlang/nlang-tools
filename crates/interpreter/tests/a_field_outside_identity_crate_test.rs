//! Q-037 crate-internal localisation.
//!
//! `expand_combo_pending` is `pub(crate)`, so the acceptor could only
//! locate the loss to "one of the two expand calls at the top of
//! `unify_combo`". This file is the last step: the operands never reach
//! those lines. They share a CAID because `pending_spreads` is not in
//! the digest, and `unify_internal` early-out kept the left one.
//!
//! Order: nlang-tools/docs/a_field_outside_identity_handover.md

use nlang_interpreter::{ComboVal, EvalContext, Ouroboros, Value};
use nlang_parser::parse_program;

fn eval_value(oo: &Ouroboros, src: &str) -> Value {
    let program = parse_program(&format!("r: {src}")).expect("parse");
    let mut ctx = EvalContext::new(ComboVal::default()).with_standard_root(oo.root_with_system());
    oo.eval(&program.fields[0].value, &mut ctx)
}

fn eval_observed_value(oo: &Ouroboros, src: &str) -> Value {
    let program = parse_program(&format!("r: {src}")).expect("parse");
    let mut ctx = EvalContext::new(ComboVal::default()).with_standard_root(oo.root_with_system());
    oo.eval_observed(&program.fields[0].value, &mut ctx)
}

fn as_combo(v: &Value) -> &ComboVal {
    match v {
        Value::Combo(c) => c,
        other => panic!("REACH: expected a combo, got {}", other.to_nlang(0)),
    }
}

#[test]
fn two_unexpanded_spread_results_share_a_digest_the_hash_cannot_see() {
    let oo = Ouroboros::new_in_memory();
    let a = eval_value(&oo, "{ ...{ a: 1 } }");
    let b = eval_value(&oo, "{ ...{ b: 2 } }");
    let ca = as_combo(&a);
    let cb = as_combo(&b);
    assert!(
        ca.data.is_empty() && cb.data.is_empty(),
        "construction defers expansion; data must still be empty"
    );
    assert_eq!(
        ca.pending_spreads.len(),
        1,
        "left operand arrives with one deferred spread"
    );
    assert_eq!(
        cb.pending_spreads.len(),
        1,
        "right operand arrives with one deferred spread"
    );
    assert_eq!(
        a.content_hash(),
        b.content_hash(),
        "pending_spreads does not enter the CAID — this is why a digest \
         early-out cannot be trusted while the field is live"
    );
}

#[test]
fn unify_keeps_both_deferred_sides_and_a_disagreement() {
    let oo = Ouroboros::new_in_memory();
    let merged = eval_observed_value(&oo, "{ ...{ a: 1 } } & { ...{ b: 2 } }");
    let keys: Vec<String> = as_combo(&merged).data.keys().cloned().collect();
    assert!(
        keys.contains(&"a".to_string()) && keys.contains(&"b".to_string()),
        "meet of two spread results must keep both sides; got {keys:?} in {}",
        merged.to_nlang(0)
    );

    let clash = eval_observed_value(&oo, "{ ...{ a: 1 } } & { ...{ a: 2 } }");
    let printed = clash.to_nlang(0);
    assert!(
        printed.contains("_|_") && printed.contains("conflict"),
        "1 and 2 disagree — dropping the right operand swallowed a collapse. \
         got {printed}"
    );
}

#[test]
fn identify_of_a_spread_into_a_fresh_combo_does_not_move() {
    let oo = Ouroboros::new_in_memory();
    let direct = eval_value(&oo, "{ /f: 1 }");
    let spread = eval_value(&oo, "{ ...{ /f: 1 } }");
    // Observation expands pending, so the two values the hash *can* see
    // are the same value. Putting pending into the digest would split them.
    let d = oo.force(
        direct,
        &mut EvalContext::new(ComboVal::default()).with_standard_root(oo.root_with_system()),
    );
    let s = oo.force(
        spread,
        &mut EvalContext::new(ComboVal::default()).with_standard_root(oo.root_with_system()),
    );
    assert_eq!(
        d.content_hash(),
        s.content_hash(),
        "G4: spreading a combo into a fresh combo is the same value"
    );
}
