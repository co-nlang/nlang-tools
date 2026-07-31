// Range 語義補完 E1–E3 probes (2026-07-11, pre-committed by work order —
// docs/range_gaps_handover.md; gaps first exposed by the comparison-section
// migration, nlang-spec ENGINE_SYNC「Range 語義補完缺口」).
//
// E1 type-marker × Range: `@int & 6..` must be the Range (refinement), ⊥ today.
//    Ruling: pass iff every non-anchor bound validates under the constraint;
//    bounds are NEVER rewritten (fmt v2 freeze — no CAID drift).
// E2 dispatch keys with Range: three sub-defects (constant rules lack %code
//    path; resolve_pattern doesn't recognize canonical range keys → silent
//    Top match-all; filter_minimal compares unified values not patterns).
// E3 Range orthocomplement: NO new Value kind (fmt v2 frozen). Meet-context
//    membership negation only: `x & !(a..b)` ⟺ x if x∉[a,b] else ⊥.
//    Standalone `!(range)` stays ⊥ (honest message; fmt v3 residual 另案).
//
// Guards are active and pin known-green behavior — MUST stay green.

use indexmap::IndexMap;
use nlang_interpreter::value::{ComboVal, EffectTag};
use nlang_interpreter::{EvalContext, Ouroboros, Value};
use nlang_parser::ast::AtomKind;
use nlang_parser::parse_program;
use num_bigint::BigInt;

fn eval_one(src: &str) -> Value {
    let program = parse_program(src).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(
        IndexMap::new(),
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));
    oo.eval_observed(&program.fields[0].value, &mut ctx)
}

fn assert_int(src: &str, expect: i64) {
    let v = eval_one(src);
    match &v {
        Value::Atom(AtomKind::Int(n), _, _) => assert_eq!(n, &BigInt::from(expect), "{src:?}"),
        other => panic!("{src:?} must be {expect}, got {other:?}"),
    }
}

fn assert_float(src: &str, expect: f64) {
    let v = eval_one(src);
    match &v {
        Value::Atom(AtomKind::Float(f), _, _) => assert!((f - expect).abs() < 1e-12, "{src:?}"),
        other => panic!("{src:?} must be {expect}, got {other:?}"),
    }
}

fn assert_str(src: &str, expect: &str) {
    let v = eval_one(src);
    match &v {
        Value::Atom(AtomKind::Str(s), _, _) => assert_eq!(s, expect, "{src:?}"),
        other => panic!("{src:?} must be {expect:?}, got {other:?}"),
    }
}

fn assert_bottom(src: &str) {
    let v = eval_one(src);
    assert!(
        matches!(v, Value::Bottom(_)),
        "{src:?} must be _|_, got {v:?}"
    );
}

/// Range value with the given canonical plain print (e.g. "6..#_").
fn assert_range_plain(src: &str, expect: &str) {
    let v = eval_one(src);
    match &v {
        Value::Range { .. } => assert_eq!(v.to_string_plain(), expect, "{src:?}"),
        other => panic!("{src:?} must be Range `{expect}`, got {other:?}"),
    }
}

/// Union whose branches' canonical `to_nlang` equal `expect` (order-insensitive).
/// Expects source-literal form (e.g. `"A"` with quotes) — same identity axis as
/// to_nlang, not unquoted to_string_plain.
fn assert_union_plain(src: &str, expect: &[&str]) {
    let v = eval_one(src);
    match &v {
        Value::Union(branches) => {
            let mut got: Vec<String> = branches.iter().map(|b| b.to_nlang(0)).collect();
            let mut want: Vec<String> = expect.iter().map(|s| s.to_string()).collect();
            got.sort();
            want.sort();
            assert_eq!(got, want, "{src:?}");
        }
        other => panic!("{src:?} must be Union {expect:?}, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// E1 — type-marker × Range (RED LINES)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn e1_int_marker_meets_range_is_range() {
    assert_range_plain("a: @int & 6..", "6..#_");
    assert_range_plain("a: @int & 1..9", "1..9");
}

#[test]
fn e1_range_meets_int_marker_mirror() {
    assert_range_plain("a: 6.. & @int", "6..#_");
}

#[test]
fn e1_real05_l1_05_membership_through_marker() {
    assert_int("a: @{ @int & 6.. } & 10", 10);
    assert_int("a: 10 & (@int & 6..)", 10);
}

#[test] // ACTIVE both-sides pin (E1): green today (⊥ via wrong reason —
        // @int&Range is ⊥ pre-fix), must stay ⊥ post-fix for the RIGHT reason
fn e1_marker_refined_range_rejects_nonmember() {
    assert_bottom("a: 5 & (@int & 6..)");
}

#[test] // ACTIVE both-sides pin (E1): green today (wrong reason), must stay ⊥
fn e1_marker_kind_mismatch_is_bottom() {
    assert_bottom("a: @str & 6..");
    assert_bottom("a: @int & (1.5..9)");
}

// (standing adversarial vector — 5b501e5 arm-order bug class)
#[test]
fn e1_union_distributes_over_marker_range() {
    assert_int("a: (10 | 5) & (@int & 6..)", 10);
}

// ─────────────────────────────────────────────────────────────────────────
// E2 — dispatch keys with Range (RED LINES)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn e2_range_key_constant_rule() {
    assert_str(r#"r: { @{ 4.. }: "A" } 5"#, "A");
}

#[test] // ACTIVE both-sides pin (E2): green today ("no %code" ⊥ for EVERY arg —
        // wrong reason), must stay ⊥ post-fix because 3 ∉ [4,⊤] (right reason)
fn e2_range_key_rejects_nonmember() {
    assert_bottom(r#"r: { @{ 4.. }: "A" } 3"#);
}

#[test]
fn e2_range_key_morphism_rule() {
    assert_int(r#"r: { @{ 4.. }: (x -> x + 1) } 5"#, 6);
}

// BOTH-SIDES pin (a): patterns 4.. and ..6 are incomparable → "A"|"B".
#[test]
fn e2_incomparable_patterns_stay_multiple() {
    assert_union_plain(
        r#"r: { @{ @int & 4.. }: "A", @{ @int & ..6 }: "B" } 5"#,
        &[r#""A""#, r#""B""#],
    );
}

#[test]
fn e2_overlap_edges_single_arm() {
    assert_str(
        r#"r: { @{ @int & 4.. }: "A", @{ @int & ..6 }: "B" } 9"#,
        "A",
    );
    assert_str(
        r#"r: { @{ @int & 4.. }: "A", @{ @int & ..6 }: "B" } 1"#,
        "B",
    );
}

// BOTH-SIDES pin (b): 4..6 ⊂ 4.. and 4..6 ⊂ ..6 → "C" alone, not A|B|C.
#[test]
fn e2_subset_pattern_is_unique_minimal() {
    assert_str(
        r#"r: { @{ @int & 4.. }: "A", @{ @int & ..6 }: "B", @{ @int & 4..6 }: "C" } 5"#,
        "C",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// E3 — Range orthocomplement, meet-context membership negation (RED LINES)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn e3_meet_not_range_membership_negation() {
    assert_int("a: 5 & !(..0)", 5);
    assert_bottom("a: -1 & !(..0)");
}

#[test] // ACTIVE both-sides pin (E3): green today (!(range) is absorbing ⊥ —
        // wrong reason), must stay ⊥ post-fix because 0 ∈ [⊥,0] closed (right reason)
fn e3_closed_end_excluded() {
    assert_bottom("a: 0 & !(..0)");
}

// exists at all; int-materialization `1..` would wrongly kill this)
#[test]
fn e3_dense_member_passes() {
    assert_float("a: 0.5 & !(..0)", 0.5);
}

#[test]
fn e3_mirror_not_range_on_left() {
    assert_int("a: !(..0) & 5", 5);
}

#[test]
fn e3_bounded_range_complement() {
    assert_int("a: 0 & !(1..3)", 0);
    assert_bottom("a: 2 & !(1..3)");
}

#[test]
fn e3_union_distributes_over_not_range() {
    assert_int("a: (1 | -1) & !(..0)", 1);
}

#[test]
fn e3_check_pos_morphism() {
    assert_int("r: (x -> @{ $ & !(..0) }) 5", 5);
    assert_bottom("r: (x -> @{ $ & !(..0) }) (-3)");
}

// ─────────────────────────────────────────────────────────────────────────
// Active guards — pin known-green behavior (MUST stay green through E1–E3)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn guard_range_membership_core() {
    assert_int("a: 10 & 6..", 10);
    assert_bottom("a: 15 & 18..");
}

#[test]
fn guard_marker_passes_atoms() {
    assert_int("a: @int & 10", 10);
    assert_bottom("a: @int & \"x\"");
}

#[test]
fn guard_anonset_transparent_range_meet() {
    assert_int("a: @{ 6.. } & 10", 10);
}

#[test]
fn guard_union_distribution_over_plain_range() {
    // permanent vector from the range_eval acceptance (b3f9316)
    assert_int("a: (1 | 7) & 1..3", 1);
}

#[test]
fn guard_standalone_not_range_stays_bottom_not_silent() {
    // E3 scope cut: standalone `!(range)` has NO closed materialization and
    // NO residual value form under the fmt v2 freeze — it must stay ⊥
    // (message may improve; silence or a wrong concrete set is a violation).
    assert_bottom("a: !(..0)");
}

#[test]
fn guard_dispatch_atom_keys_unchanged() {
    // dispatch on plain atom keys must be untouched by the E2 rework
    let v = eval_one(r#"r: { 1: "one", 2: "two" } 3"#);
    assert!(
        matches!(v, Value::Bottom(_)),
        "no-match must stay ⊥, got {v:?}"
    );
}

#[test]
fn guard_union_x_type_marker_distributes() {
    // Acceptance-added permanent guard (2026-07-11): the delivered E1 arm was
    // hoisted as marker×ANY and preempted Union distribution — `(10|20) & @int`
    // regressed to ⊥ (5b501e5 arm-order class, 4th occurrence). The early arm
    // owns ONLY marker×Range; everything else declines downstream.
    assert_union_plain("a: (10 | 20) & @int", &["10", "20"]);
    assert_int("a: (10 | \"x\") & @int", 10);
    assert_int("a: @int & (10 | \"x\")", 10);
}
