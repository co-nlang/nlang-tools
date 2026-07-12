// G1 combo-equality probes (2026-07-13, pre-committed by work order —
// docs/g1_combo_equality_handover.md).
//
// MEASURED DEFECT: cmp compares UNSOLIDIFIED combos — Thunk PartialEq is
// AST equality including span and symbol spelling. Even inline
// `{a:1} = {a:1}` → #false; `x == x` → #true only because the spans
// coincide. The lazy implementation's residue leaks into semantics.
//
// RULING (SYNTAX_06 §4 #11–13, approved 2026-07-13):
//   #11  `=` on Combo = extensional structural equality AFTER
//        solidification: six axes + closed + relations, nested recursion
//        with the SAME relation, field order blind; effect participates —
//        one engine-wide equality shared with union dedupe (L1-28).
//   #12  `==`/`!=` on a non-collapsible Combo (no %val) = family misuse →
//        ⊥ #conflict — never a silent #false. Hybrid nodes (%val present)
//        collapse to the atom and compare in the atomic family.
//        NOTE the boundary moves BOTH ways: `x == x` and `(x & z) == x`
//        on combos are #true today and become ⊥ #conflict — red-gated
//        here so the flip is deliberate, not collateral.
//   #13  Solidification firewall: span and symbol spelling must not
//        affect any semantic judgment.

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::parse_program;
use nlang_parser::ast::{Path, PathAnchor, Span};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-g1probe-{}-{}",
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&d).ok();
    fs::create_dir_all(&d).unwrap();
    d
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
            let mut universe = Universe::new(None, engine.root_with_system());
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

/// Family-misuse verdict: ⊥ with #conflict cause (exact %cause print may
/// carry detail lines; prefix + cause tag are the normative part).
fn assert_bottom_conflict(src: &str) {
    let got = observe_nlang(src, "out");
    assert!(
        got.starts_with("_|_"),
        "{src:?} :: out — expected ⊥, got {got:?}"
    );
    assert!(
        got.contains("#conflict"),
        "{src:?} :: out — expected #conflict cause, got {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — #11: `=` = extensional structural equality after solidify
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "G1 red gate: awaiting combo-equality fix"]
fn red_lattice_eq_combo_literal() {
    // L1-33. Today: #false (thunk AST+span equality).
    assert_obs("out: { a: 1 } = { a: 1 }", "#true");
}

#[test]
#[ignore = "G1 red gate: awaiting combo-equality fix"]
fn red_lattice_eq_combo_bound_span_blind() {
    // L1-34. Bound names, different definition lines — span must not matter.
    assert_obs("x: { a: 1 }\ny: { a: 1 }\nout: x = y", "#true");
}

#[test]
#[ignore = "G1 red gate: awaiting combo-equality fix"]
fn red_lattice_eq_combo_nested_same_relation() {
    // Nested fields recurse with the same equality relation.
    assert_obs("x: { a: { b: 1 } }\ny: { a: { b: 1 } }\nout: x = y", "#true");
}

#[test]
#[ignore = "G1 red gate: awaiting combo-equality fix"]
fn red_lattice_eq_field_order_blind() {
    // Set view: field spelling order does not participate.
    assert_obs("out: { a: 1, b: 2 } = { b: 2, a: 1 }", "#true");
}

#[test]
#[ignore = "G1 red gate: awaiting combo-equality fix"]
fn red_lattice_eq_symbol_spelling_blind() {
    // Fields referencing different symbols with equal values solidify equal.
    assert_obs(
        "p: 1\nq: 1\nw: { b: p }\nv: { b: q }\nout: w = v",
        "#true",
    );
}

#[test]
#[ignore = "G1 red gate: awaiting combo-equality fix"]
fn red_lattice_eq_rules_axis_span_blind() {
    // Same-spelling morphism rule on different lines: span-blind (#13).
    // (Different SPELLINGS stay unequal — pinned below; no alpha-equivalence.)
    assert_obs(
        "x: { f: (q -> q) }\ny: { f: (q -> q) }\nout: x = y",
        "#true",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — #12: `==`/`!=` family misuse → ⊥ #conflict; hybrid collapses
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "G1 red gate: awaiting combo-equality fix"]
fn red_eqeq_combo_literal_is_conflict() {
    // L1-35. Today: silent #false — that is a lie ("compared, unequal").
    assert_bottom_conflict("out: { a: 1 } == { a: 1 }");
}

#[test]
#[ignore = "G1 red gate: awaiting combo-equality fix"]
fn red_neq_combo_literal_is_conflict() {
    // `!=` mirrors: family misuse is ⊥, NOT #true.
    assert_bottom_conflict("out: { a: 1 } != { a: 1 }");
}

#[test]
#[ignore = "G1 red gate: awaiting combo-equality fix"]
fn red_eqeq_same_binding_flips_to_conflict() {
    // BOUNDARY MOVES: today #true (same-span thunks). Ruled ⊥ #conflict —
    // combo operands are family misuse regardless of instance identity.
    assert_bottom_conflict("x: { a: 1 }\nout: x == x");
}

#[test]
#[ignore = "G1 red gate: awaiting combo-equality fix"]
fn red_eqeq_meet_result_flips_to_conflict() {
    // BOUNDARY MOVES: today #true (meet solidified the fields). Still a
    // non-collapsible combo → ruled ⊥ #conflict.
    assert_bottom_conflict("x: { a: 1 }\nz: { a: 1 }\nout: (x & z) == x");
}

#[test]
#[ignore = "G1 red gate: awaiting combo-equality fix"]
fn red_eqeq_hybrid_collapses_to_atom() {
    // L1-36. Hybrid node (%val present) collapses, then atomic family.
    assert_obs("h: 3 & { note: \"n\" }\nout: h == 3", "#true");
}

#[test]
fn red_neq_hybrid_collapses_to_atom_demoted_to_pin() {
    // DEMOTED at calibration (protocol): green today — the combo≠atom
    // fallthrough coincidentally yields #true. After the fix the verdict
    // must be IDENTICAL via the ruled route (collapse to 3, then 3 != 4).
    assert_obs("h: 3 & { note: \"n\" }\nout: h != 4", "#true");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — must stay green through the fix
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_atom_eqeq_true() {
    assert_obs("out: 1 == 1", "#true");
}

#[test]
fn pin_atom_lattice_eq_true() {
    assert_obs("out: 1 = 1", "#true");
}

#[test]
fn pin_atom_lattice_eq_false() {
    assert_obs("out: 1 = 2", "#false");
}

#[test]
fn pin_lattice_eq_combo_unequal_values_false() {
    // Clean #false for genuinely unequal combos — misuse ⊥ is `==`-only.
    assert_obs("out: { a: 1 } = { a: 2 }", "#false");
}

#[test]
fn pin_lattice_eq_combo_unequal_keys_false() {
    assert_obs("out: { a: 1 } = { b: 1 }", "#false");
}

#[test]
fn pin_lattice_eq_type_atoms() {
    // Superposition/type set-equality unaffected (SYNTAX_06 §2 rule 1).
    assert_obs("out: @int = @int", "#true");
}

#[test]
fn pin_lattice_eq_morphism_spelling_sensitive() {
    // No alpha-equivalence granted: different body spelling stays unequal.
    assert_obs(
        "x: { f: (q -> q) }\ny: { f: (w -> w) }\nout: x = y",
        "#false",
    );
}

#[test]
fn pin_eqeq_bottom_absorbs() {
    // Atomic family absorption (SYNTAX_06 §4 #1) unchanged.
    let got = observe_nlang("out: _|_ == _|_", "out");
    assert!(got.starts_with("_|_"), "absorption lost: {got:?}");
}

#[test]
fn pin_lattice_eq_bottom_clean_booleans() {
    // Set family does not absorb: ⊥ is the empty set.
    assert_obs("out: _|_ = 3", "#false");
    assert_obs("out: _|_ = _|_", "#true");
}

#[test]
fn pin_combo_lte_stays_conflict() {
    // Adjacent, NOT in this ruling (§4.10 case): `<=` on combos is
    // ⊥ #conflict today — a drive-by change here would be scope creep.
    assert_bottom_conflict("out: { a: 1 } <= { a: 1 }");
}

#[test]
fn pin_tag_vs_unit_eqeq_false() {
    assert_obs("out: #true == ()", "#false");
}

#[test]
fn pin_hybrid_observe_current_full_print() {
    // MEASURED at calibration: observing a hybrid prints the FULL combo
    // (%val visible) — SYNTAX_06 §4 #6 says observation reads %val → `3`.
    // That spec-engine discrepancy is ledgered as G6 (separate case; NOT
    // in the G1 order). This pin freezes today's display so the G1 fix
    // doesn't drive-by change observation; unpin when G6 is adjudicated.
    assert_obs(
        "h: 3 & { note: \"n\" }\nout: h",
        "{\n  %val: 3\n  note: \"n\"\n}",
    );
}

#[test]
fn pin_union_dedupe_combo_then_navigate() {
    // Dedupe relation (L1-28) and union navigation (L1-32) untouched.
    assert_obs("w: ({ a: 1 } | { a: 1 })\nout: w.a", "1");
}
