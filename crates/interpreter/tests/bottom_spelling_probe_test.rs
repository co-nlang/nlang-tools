// Atom(Bottom) absorption probes (2026-07-10, pre-committed during acceptance
// of the fuzz/golden-AST delivery). The exact dual of the Atom(Top) bug fixed
// in 04df5c4: literal `_|_` evaluates to Value::Atom(AtomKind::Bottom) and the
// atomic-comparison path only recognizes the Value::Bottom variant, so the
// SYNTAX_06 §4.1 absorption law is violated (measured 2026-07-10):
//
//     _ == _        → _        ✓ (spec: _)
//     _ == _|_      → _        ✗ (spec: _|_ — 與「無」比較皆衝突)
//     _|_ == _|_    → #true    ✗ (spec: _|_)
//     _|_ != _|_    → #false   ✗ (spec: _|_ — 黑洞:任一側 _|_ → 整式 _|_)
//
// Discovery path: `@{}` (parses since 2885ed7; SYNTAX_02 §8 rules it the SAME
// object as `_|_`) evaluates through eval's wildcard arm straight to
// Value::Bottom — and thereby shows the spec-CORRECT absorption in `==` that
// the literal `_|_` spelling lacks. The two active equivalence guards below
// pin the positions where the spellings already agree; the #[ignore] red
// lines are the absorption law itself.
//
// Note for the fixer: 04df5c4's non-goal explicitly deferred the Bottom half
// of dual-spelling normalization; this is now the measured justification for
// it. Whether normalized Bottom should carry BottomCause::Conflict or a
// dedicated "declared-empty" cause is the fixer's call — the probes only pin
// the absorption behavior, not the %cause taxonomy.
//
// Acceptance = remove the #[ignore]s, everything green (incl. the two active
// guards and every other suite).

use nlang_interpreter::{Ouroboros, EvalContext, Value};
use nlang_interpreter::value::{ComboVal, EffectTag};
use nlang_parser::parse_program;
use indexmap::IndexMap;

fn eval_one(src: &str) -> Value {
    let program = parse_program(src).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]));
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

// --- red lines: SYNTAX_06 §4.1 absorption law (un-ignore = acceptance) ------

#[test]
#[ignore = "Atom(Bottom) not normalized on cmp path: _|_ == _|_ → #true (baseline 2026-07-10; spec: _|_)"]
fn bottom_eq_bottom_absorbs() {
    assert_is_bottom("r: _|_ == _|_");
}

#[test]
#[ignore = "Atom(Bottom) not normalized on cmp path: _ == _|_ → _ (baseline 2026-07-10; spec: _|_)"]
fn top_eq_bottom_absorbs() {
    assert_is_bottom("r: _ == _|_");
}

#[test]
#[ignore = "Atom(Bottom) not normalized on cmp path: _|_ != _|_ → #false (baseline 2026-07-10; spec: _|_)"]
fn bottom_ne_bottom_absorbs() {
    assert_is_bottom("r: _|_ != _|_");
}

#[test]
#[ignore = "follows from the absorption fix: both spellings must then agree in =="]
fn anon_empty_set_eq_bottom_matches_literal() {
    assert_same_as_bottom_spelling("r: HOLE == _|_");
}
