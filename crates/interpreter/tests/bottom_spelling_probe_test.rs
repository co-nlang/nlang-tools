// Atom(Bottom) absorption probes (2026-07-10, pre-committed during acceptance
// of the fuzz/golden-AST delivery). Dual of Atom(Top) (04df5c4): literal `_|_`
// is normalized to Value::Bottom(Conflict) at eval sources so SYNTAX_06 §4.1
// absorption holds on the atomic-comparison path.
//
//     _ == _        → _
//     _ == _|_      → _|_   (與「無」比較皆衝突)
//     _|_ == _|_    → _|_
//     _|_ != _|_    → _|_   (黑洞:任一側 _|_ → 整式 _|_)
//
// `@{}` and `_|_` are the same object (SYNTAX_02 §8). Fix: eval + resolve_path
// normalize Atom(Bottom) → Value::Bottom(Conflict); unify re-enters Atom(Bottom)
// as a faithful alias (same pattern as Atom(Top) in 5b501e5).

use indexmap::IndexMap;
use nlang_interpreter::value::{ComboVal, EffectTag};
use nlang_interpreter::{EvalContext, Ouroboros, Value};
use nlang_parser::parse_program;

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

fn assert_is_bottom(src: &str) {
    let v = eval_one(src);
    assert!(
        matches!(v, Value::Bottom(_)),
        "{src:?} must absorb to _|_ (SYNTAX_06 §4.1), got {v:?}"
    );
}

/// `@{}` and `_|_` in the same source position must produce the same value
/// (SYNTAX_02 §8: 「`@{}` 與 `_|_` 為同一物」).
fn assert_same_as_bottom_spelling(template: &str) {
    let with_anon = eval_one(&template.replace("HOLE", "@{}"));
    let with_bottom = eval_one(&template.replace("HOLE", "_|_"));
    assert_eq!(
        with_anon.content_hash(), with_bottom.content_hash(),
        "`@{{}}` must be the same object as `_|_` in {template:?}:\n  @{{}}  → {with_anon:?}\n  _|_ → {with_bottom:?}"
    );
}

// --- active no-regression guards (green at baseline 2026-07-10) -------------

#[test]
fn anon_empty_set_is_bottom() {
    assert_same_as_bottom_spelling("x: HOLE");
}

#[test]
fn anon_empty_set_absorbs_in_meet() {
    assert_same_as_bottom_spelling("m: HOLE & 5");
}

// --- SYNTAX_06 §4.1 absorption law ------------------------------------------

#[test]
fn bottom_eq_bottom_absorbs() {
    assert_is_bottom("r: _|_ == _|_");
}

#[test]
fn top_eq_bottom_absorbs() {
    assert_is_bottom("r: _ == _|_");
}

#[test]
fn bottom_ne_bottom_absorbs() {
    assert_is_bottom("r: _|_ != _|_");
}

#[test]
fn anon_empty_set_eq_bottom_matches_literal() {
    assert_same_as_bottom_spelling("r: HOLE == _|_");
}

// --- SYNTAX_06 §4.1: the NON-absorbing set family (`=`) ---------------------
// Added at acceptance of 9727f1a: the Bottom normalization regressed these —
// LatticeEq's early-return treated the (now-reachable) Value::Bottom as a
// black hole, but `=` is the clean-boolean family (`_|_ = 3` was #false at
// baseline e7d2fcb, became ⊥). Active guards after the acceptance repair.

fn assert_is_tag(src: &str, tag: &str) {
    let v = eval_one(src);
    match &v {
        Value::Atom(nlang_parser::ast::AtomKind::Tag(t), _, _) if t == tag => {}
        other => panic!("{src:?} must be #{tag} (SYNTAX_06 §4.1: `=` 不塌縮不吸收), got {other:?}"),
    }
}

#[test]
fn lattice_eq_bottom_vs_atom_is_clean_false() {
    assert_is_tag("r: _|_ = 3", "false");
}

#[test]
fn lattice_eq_bottom_vs_bottom_is_clean_true() {
    assert_is_tag("r: _|_ = _|_", "true");
    assert_is_tag("r: @{} = _|_", "true");
}

// SYNTAX_06 §4.2 subtype-extreme red lines moved to
// cmp_extremes_probe_test.rs (2026-07-10, work order
// docs/cmp_extremes_handover.md) — expanded to Gte/Lt/Gt + finite guards.
