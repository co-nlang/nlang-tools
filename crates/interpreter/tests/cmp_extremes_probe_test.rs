// SYNTAX_06 §4.2 set-family (`< <= >= >`) ⊥/⊤ extremes probes
// (2026-07-10, pre-committed by work order — docs/cmp_extremes_handover.md).
//
// The set family is clean-boolean and NON-absorbing: `_|_` is the empty set
// (subset of everything), `_` is Top (superset of everything). Never
// implemented: `<`/`<=`/`>`/`>=` share eval_binary_cmp's absorbing ⊥/⊤
// early-returns with the atomic `==`/`!=` family (measured identical at
// baseline e7d2fcb AND after the Atom(Bottom) fix 9727f1a — pre-existing,
// not a regression).
//
// Baseline 2026-07-10 (all red lines):
//     _|_ <= 5    → ⊥Conflict   (spec #true)     5 <= _|_   → ⊥ (spec #false)
//     _|_ <= _|_  → ⊥           (spec #true)     5 <= _     → _ (spec #true)
//     _ <= _|_    → ⊥           (spec #false)    5 >= _|_   → ⊥ (spec #true)
//     _ >= 5      → _           (spec #true)     _|_ < 5    → ⊥ (spec #true)
//     5 < _       → _           (spec #true)     _ < _      → _ (spec #false)
//     _|_ > _|_   → ⊥           (spec #false)    _ >= _     → _ (spec #true)
//
// Active guards pin the finite side (the boundary's OTHER side — see the
// Atom(Bottom) acceptance lesson in ENGINE_SYNC): numeric compares, the
// type-constraint subtype path, and the atomic family's absorption must not
// move. NOTE the numeric semantics itself (`3 <= 5` → #true) is a DOCUMENTED
// deliberate deviation from SYNTAX_06 §4.10 (spec: subset semantics → #false;
// engine: numeric order, ENGINE_SYNC 求值層最小語義) — the guard pins
// no-silent-change, not spec truth. That ruling is out of scope here.
//
// Acceptance = remove the #[ignore]s, everything green (incl. active guards
// and every other suite).

use nlang_interpreter::{Ouroboros, EvalContext, Value};
use nlang_interpreter::value::{ComboVal, EffectTag};
use nlang_parser::parse_program;
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;

fn eval_one(src: &str) -> Value {
    let program = parse_program(src).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]));
    oo.eval_observed(&program.fields[0].value, &mut ctx)
}

fn assert_is_tag(src: &str, tag: &str) {
    let v = eval_one(src);
    match &v {
        Value::Atom(AtomKind::Tag(t), _, _) if t == tag => {}
        other => panic!("{src:?} must be #{tag} (SYNTAX_06 §4.2: 集合家族乾淨布林), got {other:?}"),
    }
}

// --- red lines: Lte extremes (moved here from bottom_spelling_probe_test) ---

#[test]
#[ignore = "SYNTAX_06 §4.2 not implemented: _|_ <= x must be #true (baseline 2026-07-10: ⊥Conflict)"]
fn bottom_is_subtype_of_everything() {
    assert_is_tag("r: _|_ <= 5", "true");
    assert_is_tag("r: _|_ <= _|_", "true");
    assert_is_tag("r: _|_ <= _", "true");
}

#[test]
#[ignore = "SYNTAX_06 §4.2 not implemented: x <= _ must be #true (baseline 2026-07-10: _)"]
fn everything_is_subtype_of_top() {
    assert_is_tag("r: 5 <= _", "true");
    assert_is_tag("r: _ <= _", "true");
}

#[test]
#[ignore = "SYNTAX_06 §4.2 not implemented: _ <= _|_ must be #false, 5 <= _|_ must be #false (baseline: _/⊥)"]
fn only_bottom_is_subtype_of_bottom() {
    assert_is_tag("r: _ <= _|_", "false");
    assert_is_tag("r: 5 <= _|_", "false");
}

// --- red lines: Gte mirror (x >= y ≡ y <= x) --------------------------------

#[test]
#[ignore = "SYNTAX_06 §4.2 not implemented: Gte must mirror Lte at extremes (baseline: ⊥/_)"]
fn gte_mirrors_lte_at_extremes() {
    assert_is_tag("r: 5 >= _|_", "true");   // ≡ _|_ <= 5
    assert_is_tag("r: _ >= 5", "true");     // ≡ 5 <= _
    assert_is_tag("r: _|_ >= _|_", "true"); // ≡ _|_ <= _|_
    assert_is_tag("r: _ >= _", "true");     // ≡ _ <= _
    assert_is_tag("r: _|_ >= 5", "false");  // ≡ 5 <= _|_
}

// --- red lines: strict `<`/`>` = subset ∧ not-equal at extremes -------------
// Pre-ruling (work order §修法): the set family's `<` is strict subset;
// mechanically at extremes: ⊥ < y ⟺ y ≠ ⊥; x < ⊤ ⟺ x ≠ ⊤; never ⊤ < y, x < ⊥.

#[test]
#[ignore = "SYNTAX_06 §4.2 not implemented: strict subset at extremes (baseline: ⊥/_)"]
fn strict_subset_at_extremes() {
    assert_is_tag("r: _|_ < 5", "true");
    assert_is_tag("r: 5 < _", "true");
    assert_is_tag("r: _|_ < _|_", "false");
    assert_is_tag("r: _ < _", "false");
    assert_is_tag("r: _|_ > _|_", "false");
    assert_is_tag("r: _ > 5", "true");      // ≡ 5 < _
}

// --- active guards: the finite side must not move ---------------------------

#[test]
fn finite_numeric_compare_unchanged() {
    assert_is_tag("r: 3 < 5", "true");
    assert_is_tag("r: 5 < 3", "false");
    assert_is_tag("r: 3 <= 5", "true"); // documented §4.10 deviation pin (numeric, not subset)
    assert_is_tag("r: 5 >= 3", "true");
    assert_is_tag("r: 5.5 <= 6", "true");
}

#[test]
fn type_constraint_subtype_path_unchanged() {
    assert_is_tag("r: @int <= @num", "true");
}

#[test]
fn atomic_family_absorption_unchanged() {
    // ==/!= keep absorbing (SYNTAX_06 §4.1) — the fix must split families,
    // not move this one. Full coverage in bottom_spelling_probe_test.rs.
    let v = eval_one("r: _|_ == _|_");
    assert!(matches!(v, Value::Bottom(_)), "==/!= must keep absorbing, got {v:?}");
    assert_is_tag("r: 1 == 1", "true");
    assert_is_tag("r: #a != #b", "true");
}
