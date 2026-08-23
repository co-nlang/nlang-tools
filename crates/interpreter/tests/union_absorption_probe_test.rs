// Union absorption probes (2026-07-20, pre-committed by work order —
// docs/union_absorption_handover.md).
//
// RULING A (2026-07-20): the lattice absorption axiom a ∨ (a ∧ b) = a
// becomes a normalization duty of the union VALUE itself (SPEC_01
// §2.4.2): cull ⊥ (G4) → idempotent dedupe → ABSORB — any branch
// b <= a (the W3 meet reduction, same G1 full relation: six axes,
// closed, effect, provenance) is absorbed by a; only MAXIMAL branches
// (an antichain) survive. Display, `=`, `<=` and CAID stay one voice
// (the engine-wide unique equality clause survives). Top family:
// unions with a Top branch collapse to a single Top (caused Top beats
// bare `_`; multiple distinct caused Tops → leftmost, same beat as the
// all-⊥ collapse REAL_04 §4). #blur branches are EXEMPT BOTH WAYS
// (never absorb, never absorbed — horizon ⊆ is undecidable; the
// survival law SPEC_08 §3.2.2 #5 outranks absorption). BREAKING #2:
// affected unions' CAIDs shift once.
//
// MEASURED (v0.2.31): no absorption anywhere — `(@int | 1) = @int` →
// #false, `({a:1} | {a:1,b:2}) = {a:1}` → #false, `9 | _` displays
// `9 | _`, coverage `<=` on open-combo unions #false (the W3
// under-approximation this arc cures). Healthy: dedupe (`1 | 1` → 1),
// ⊥ cull, canonical display order, incomparable branches persist.
//
// NOT in scope: dispatch tables (morphism combos — rules axis is
// spelling-sensitive, meets don't equate distinct rules, so absorption
// never fires there by construction), stored-universe migration (dev
// stores are disposable; CAID shift documented not migrated), W4.

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("uabsorb")
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
                let _ = universe.evolve(&engine, f);
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

fn flat_chain(n: usize) -> String {
    vec!["1"; n].join(" + ")
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — absorption normalization (SPEC_01 §2.4.2)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_absorb_type_covers_atom() {
    // L2-87 twin: the singleton lives inside the type space.
    assert_obs("out: (@int | 1) = @int", "#true");
    // Multi-branch: every singleton folds into the type space.
    assert_obs("out: (1 | 3 | @int) = @int", "#true");
}

#[test]
fn red_absorb_combo_refinement() {
    // a ∨ (a ∧ b) = a — the refined branch folds back in.
    assert_obs("out: ({a: 1} | {a: 1, b: 2}) = {a: 1}", "#true");
    assert_obs("out: ((1 & @int) | @int) = @int", "#true");
}

#[test]
fn red_absorb_top_collapses() {
    // L2-89 twin: everything is a subset of Top — both spellings.
    assert_obs("out: 9 | _", "_");
    assert_obs("out: _ | 9", "_");
}

#[test]
fn red_absorb_cures_coverage_order() {
    // The W3 under-approximation faces turn mathematically true:
    // after absorption the meet normalizes back to A itself.
    assert_obs(
        "out: ({a: 1} | {b: 2}) <= ({a: 1} | {b: 2} | {c: 3})",
        "#true",
    );
    assert_obs("out: {a: 1} <= ({a: 1} | {b: 2})", "#true");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — what absorption must NOT touch
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_dedupe_and_incomparable_branches() {
    assert_obs("out: 1 | 1", "1");
    assert_obs("out: 1 | 2", "1 | 2");
    assert_obs("out: ({a: 1} | {b: 2}) = ({b: 2} | {a: 1})", "#true");
    assert_obs("out: ({a: 1} | {b: 2}) = {a: 1}", "#false");
}

#[test]
fn pin_bottom_cull_unchanged() {
    assert_obs("out: (1 & 2) | 9", "9");
}

#[test]
fn pin_blur_branch_exempt_both_ways() {
    // Horizon branches survive absorption in BOTH directions — even
    // against Top (survival law outranks absorption).
    let src = format!("big: {}\n", flat_chain(4000));
    let got = observe_nlang(&format!("{src}out: {{v: 1}} | big"), "out");
    assert!(
        got.contains(" | ") && got.contains("#blur"),
        "blur branch never absorbed by a combo: {got:?}"
    );
    let got = observe_nlang(&format!("{src}out: big | _"), "out");
    assert!(
        got.contains("#blur") && got.contains("_"),
        "blur branch survives even a Top branch: {got:?}"
    );
}
