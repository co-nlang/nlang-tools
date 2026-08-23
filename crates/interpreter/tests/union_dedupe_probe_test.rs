// Union idempotence (dedupe) probes (2026-07-12, pre-committed by work
// order — docs/union_dedupe_lint_handover.md).
//
// Lattice law: x ∨ x = x (SPEC_01 join idempotence). Today Union
// construction previously kept structural duplicates (`1 | 1`, `(1|2)|(1|2)`,
// Top-branch distribution, same-marker distribution).
// Ruling: dedupe = STRUCTURAL equality, first occurrence kept, single
// survivor collapses the Union wrapper. Existing orderings are NOT
// touched (eval `|` preserves writing order; unify's tropical-weight
// sort stays). Range coalescing (1..3 | 2..5) is a NON-goal.

use nlang_interpreter::{Ouroboros, Universe, Value};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("udedupe")
}

/// 64 MiB thread — parser/eval recursion headroom (established pattern).
fn observe_nlang(src: &str, path: &str) -> String {
    let src = src.to_string();
    let path = path.to_string();
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let dir = tmp_dir();
            let engine = Ouroboros::init(&dir).unwrap();
            let mut universe = Universe::new_with_standard(
                None,
                engine.root_with_system(),
                engine.root_with_system(),
            );
            let program = parse_program(&src).unwrap();
            for f in &program.fields {
                universe.evolve(&engine, f).unwrap();
            }
            let p = Path {
                anchor: PathAnchor::Bare,
                segments: path.split('.').map(|s| s.to_string()).collect(),
                span: Span::default(),
            };
            universe.observe(&engine, &p).to_nlang(0)
        })
        .unwrap()
        .join()
        .unwrap()
}

fn assert_obs(src: &str, expect: &str) {
    let got = observe_nlang(src, "out");
    assert_eq!(got, expect, "{src:?} :: out");
}

// ─────────────────────────────────────────────────────────────────────────
// RED LINES — structural duplicates survive today
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn union_literal_self_dedupes() {
    assert_obs("out: 1 | 1", "1");
}

#[test]
fn union_inner_duplicate_dedupes() {
    assert_obs("out: 1 | 2 | 1", "1 | 2");
}

#[test]
fn union_join_same_union_dedupes() {
    assert_obs("out: (1 | 2) | (1 | 2)", "1 | 2");
}

#[test]
fn union_top_branch_meet_dedupes() {
    assert_obs("out: (1 | _) & (1 | 2)", "1 | 2");
}

#[test]
fn union_same_marker_meet_dedupes() {
    assert_obs("out: 10 & (@int | @int)", "10");
}

#[test]
fn union_string_self_dedupes() {
    assert_obs("out: \"a\" | \"a\"", "\"a\"");
}

#[test]
// (today `1..5 | 1..5`; NOT range coalescing — see pin below)
fn union_identical_range_dedupes() {
    assert_obs("out: (1..5) | (1..5)", "1..5");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE pins — orderings, distinct branches, and adjacent semantics
// ─────────────────────────────────────────────────────────────────────────

#[test] // ACTIVE pin: distinct branches untouched, writing order preserved
fn pin_union_distinct_order_12() {
    assert_obs("out: 1 | 2", "1 | 2");
}

#[test] // ACTIVE pin: display is SPEC_01 §2.4.1 canonical (sorted), not
        // writing order — internal vector still encounter-order.
fn pin_union_distinct_order_21() {
    assert_obs("out: 2 | 1", "1 | 2");
}

#[test] // ACTIVE pin (TRAP): Int(1) and Float(1.0) are structurally
        // DIFFERENT atoms but PRINT identically ("1" — pre-existing float
        // display quirk, queued separately). Structural dedupe keeps both
        // → "1 | 1"; a print-string-based dedupe would wrongly collapse
        // to "1" and fail this pin.
fn pin_union_int_float_kept() {
    // MIGRATED (2026-07-20, union_absorption + W2 numeric-by-value):
    // 1 and 1.0 are one singleton under subset law — absorption keeps one.
    assert_obs("out: 1 | 1.0", "1");
}

#[test] // ACTIVE pin: overlapping but non-identical ranges are kept
        // (range coalescing is a NON-goal)
fn pin_union_overlapping_ranges_kept() {
    assert_obs("out: (1..3) | (2..5)", "1..3 | 2..5");
}

#[test] // ACTIVE pin: unify-side distribution result (already dedup-free)
fn pin_union_meet_distribute() {
    assert_obs("out: (1 | 2) & (2 | 1)", "1 | 2");
}

#[test] // ACTIVE pin: union × union evolution converges (unify arm)
fn pin_union_evolve_union() {
    assert_obs("a: 1 | 2\na: 2 | 1\nout: a", "1 | 2");
}

#[test] // ACTIVE pin: marker×union distribution keeps distinct survivors
        // (E4 guard neighborhood: (10|20) & @int)
fn pin_union_marker_distinct_survivors() {
    assert_obs("out: (10 | 20) & @int", "10 | 20");
}
