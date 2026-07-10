// Range / @{expr} eval probes (2026-07-10, pre-committed by work order —
// docs/range_eval_handover.md; ruling: SPEC_02 §3 + SYNTAX_04 §4.5/§4.7,
// nlang-spec c3c7cdd).
//
// Ruling: `a..b` = CLOSED interval SET [a,b] — a symbolic lattice value, not
// a sequence, not a loop. Observation neither materializes nor collapses it.
// Meet = membership (atom & range) and intersection (stepless range & range;
// empty → ⊥, singleton → collapses to the atom). `@{ e } ≡ e` (transparent).
//
// Landed: Value::Range + eval/unify arms (range_eval_handover.md).
// Canonical print: `a..b` / `a..b..s`, no spaces.
// No-regression side: list_p25 (`~%List./range` half-open), absorption/cmp suites.

use nlang_interpreter::{Ouroboros, Universe, EvalContext, Value};
use nlang_interpreter::value::{ComboVal, EffectTag};
use nlang_parser::parse_program;
use nlang_parser::ast::{AtomKind, Path, PathAnchor, Span};
use indexmap::IndexMap;
use num_bigint::BigInt;

fn eval_one(src: &str) -> Value {
    let program = parse_program(src).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]));
    oo.eval_observed(&program.fields[0].value, &mut ctx)
}

fn assert_int(src: &str, expect: i64) {
    let v = eval_one(src);
    match &v {
        Value::Atom(AtomKind::Int(n), _, _) => assert_eq!(n, &BigInt::from(expect), "{src:?}"),
        other => panic!("{src:?} must be {expect}, got {other:?}"),
    }
}

fn assert_bottom(src: &str) {
    let v = eval_one(src);
    assert!(matches!(v, Value::Bottom(_)), "{src:?} must be _|_, got {v:?}");
}

fn assert_prints(src: &str, expect: &str) {
    let v = eval_one(src);
    assert!(!matches!(v, Value::Bottom(_)), "{src:?} must not be _|_ (got {v:?})");
    assert_eq!(v.to_nlang(0), expect, "{src:?} canonical print");
}

// --- Range is a value (observe = itself, no collapse) ------------------------

#[test]
fn range_literal_observes_as_itself() {
    assert_prints("x: 1..10", "1..10");
    assert_prints("x: 0..10..2", "0..10..2");
    assert_prints("x: 1..#_", "1..#_");
}

// --- membership meet (closed ends, steps, anchors) ---------------------------

#[test]
fn range_membership_meet() {
    assert_int("r: 5 & 1..10", 5);
    assert_bottom("r: 12 & 1..10");
    assert_int("r: 150 & 0..150", 150); // closed-closed: endpoint belongs
    assert_int("r: 5 & 1..#_", 5);      // anchor upper = +∞
    assert_bottom("r: 0 & 1..#_");
}

#[test]
fn range_step_membership() {
    assert_int("r: 4 & 0..10..2", 4);
    assert_int("r: 10 & 0..10..2", 10); // closed end on-step
    assert_bottom("r: 3 & 0..10..2");
}

// --- stepless intersection ---------------------------------------------------

#[test]
fn range_intersection() {
    assert_prints("r: 1..10 & 5..20", "5..10");
    assert_int("r: 3..5 & 5..8", 5);    // singleton collapses to the atom
    assert_bottom("r: 1..3 & 7..9");    // empty intersection
}

// --- declare-range-then-refine (monotone evolution) --------------------------

#[test]
fn evolve_can_refine_from_range() {
    let engine = Ouroboros::new_in_memory();
    let mut universe = Universe::new(None, ComboVal::default());
    let p1 = parse_program("t: { n: 1..10 }").unwrap();
    universe.evolve(&engine, &p1.fields[0]).unwrap();
    let p2 = parse_program("t: { n: 5 }").unwrap();
    universe.evolve(&engine, &p2.fields[0])
        .unwrap_or_else(|e| panic!("refining n from 1..10 to 5 is monotone and must succeed, got {:?}", e));
    let path = Path { anchor: PathAnchor::Bare, segments: vec!["t".into(), "n".into()], span: Span::default() };
    let obs = universe.observe(&engine, &path);
    match &obs {
        Value::Atom(AtomKind::Int(n), _, _) => assert_eq!(n, &BigInt::from(5)),
        other => panic!("t.n after refine must be 5, got {:?}", other),
    }
}

// --- variable bound resolves at observation ----------------------------------

#[test]
fn range_variable_bound_resolves_at_observation() {
    let engine = Ouroboros::new_in_memory();
    let mut universe = Universe::new(None, ComboVal::default());
    for src in ["y: 10", "x: 1..y"] {
        let p = parse_program(src).unwrap();
        universe.evolve(&engine, &p.fields[0]).unwrap();
    }
    let path = Path { anchor: PathAnchor::Bare, segments: vec!["x".into()], span: Span::default() };
    let obs = universe.observe(&engine, &path);
    assert!(!matches!(obs, Value::Bottom(_)), "x: 1..y with y=10 must not be _|_, got {obs:?}");
    assert_eq!(obs.to_nlang(0), "1..10", "bounds resolve against the observed universe");
}

// --- @{ e } transparency -----------------------------------------------------

#[test]
fn anon_set_is_transparent() {
    assert_int("r: @{ 5 }", 5);
    assert_prints("r: @{ 1..10 }", "1..10");
    let a = eval_one("r: @{ 1 | 2 }");
    let b = eval_one("r: 1 | 2");
    assert_eq!(a.content_hash(), b.content_hash(),
        "@{{ 1 | 2 }} must be the same object as 1 | 2, got {a:?} vs {b:?}");
}
