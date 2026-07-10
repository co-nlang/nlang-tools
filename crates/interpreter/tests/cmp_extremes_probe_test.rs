// SYNTAX_06 §4.2 set-family (`< <= >= >`) ⊥/⊤ extremes probes
// (2026-07-10, work order docs/cmp_extremes_handover.md).
//
// Set family is clean-boolean and NON-absorbing: `_|_` = empty set (⊆ all),
// `_` = Top (⊇ all). Fixed by splitting eval_binary_cmp into atomic (absorbing
// ==/!=) vs set (extreme table + finite path) families.
//
// Active guards pin the finite side: numeric compares, type-constraint
// subtype, and atomic absorption must not move. `3 <= 5` → #true is a
// documented §4.10 deviation (ENGINE_SYNC 求值層最小語義) — guard pins
// no-silent-change, not full subset semantics.

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

// --- Lte extremes -----------------------------------------------------------

#[test]
fn bottom_is_subtype_of_everything() {
    assert_is_tag("r: _|_ <= 5", "true");
    assert_is_tag("r: _|_ <= _|_", "true");
    assert_is_tag("r: _|_ <= _", "true");
}

#[test]
fn everything_is_subtype_of_top() {
    assert_is_tag("r: 5 <= _", "true");
    assert_is_tag("r: _ <= _", "true");
}

#[test]
fn only_bottom_is_subtype_of_bottom() {
    assert_is_tag("r: _ <= _|_", "false");
    assert_is_tag("r: 5 <= _|_", "false");
}

// --- Gte mirror (x >= y ≡ y <= x) -------------------------------------------

#[test]
fn gte_mirrors_lte_at_extremes() {
    assert_is_tag("r: 5 >= _|_", "true");   // ≡ _|_ <= 5
    assert_is_tag("r: _ >= 5", "true");     // ≡ 5 <= _
    assert_is_tag("r: _|_ >= _|_", "true"); // ≡ _|_ <= _|_
    assert_is_tag("r: _ >= _", "true");     // ≡ _ <= _
    assert_is_tag("r: _|_ >= 5", "false");  // ≡ 5 <= _|_
}

// --- strict `<`/`>` at extremes ---------------------------------------------

#[test]
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
