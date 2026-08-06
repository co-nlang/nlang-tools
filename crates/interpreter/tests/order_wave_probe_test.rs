// Order-wave W1+W2 probes (2026-07-20, pre-committed by work order —
// docs/order_wave_handover.md).
//
// RULING (2026-07-20, wave plan approved): W1 = numeric order
// PREDICATES land in the standard library (~%Math./lt /lte /gt /gte,
// boolean; SPEC_09); W2 = the documented §4.10 numeric deviation on the
// `<` family is RETIRED — SYNTAX_06 §2.5/§4 #10 subset semantics are
// law: `A <= B ⟺ (A & B) = A`, so distinct atom singletons never
// contain each other (`3 <= 5` → #false, clean boolean, not ⊥), and
// numeric magnitude questions belong to ~%Math. BREAKING (Layer 1
// changelog; first breaking entry after v0.2.0).
//
// MEASURED (v0.2.29): atoms compare NUMERICALLY (`3 <= 5`/`3 < 5`/
// `5 > 3` → #true — deviation, pinned as documented until today);
// `1 <= @int` → ⊥ #conflict (subtype face unimplemented);
// ~%Math./lt → ⊥ #missing_key. Healthy and flip-stable: poset chains
// (#h1 < #h2 #true — declared lattice order, the real thing),
// reflexive `2 <= 2`, `3 >= 5` #false, extremes (⊥ <= x, x <= _),
// `3 <= 3.0` (numbers by value: same singleton).
//
// Open migrations (acceptor): cmp_extremes finite_numeric_compare pin,
// eval_test test_cmp_eval, conformance L1-20 expect → #false, L2-10
// ternary condition respelled via ~%Math./gt. Delivery migration
// (ordered, §3): corpus test_comparison.n `10 > 5` → `~%Math./gt 10 5`.
//
// NOT in scope: combo/union order (W3 — frozen #conflict pins in
// blur_boundary/combo_equality stand), blur×order two-stage (W3),
// %super/%predicate (W4), `=`/`==` families (untouched), poset track.

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("ordwave")
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

// ─────────────────────────────────────────────────────────────────────────
// RED GATES W1 — ~%Math numeric predicates (SPEC_09)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_math_lt_lte() {
    assert_obs("out: ~%Math./lt 3 5", "#true");
    assert_obs("out: ~%Math./lt 5 3", "#false");
    assert_obs("out: ~%Math./lte 5 5", "#true");
    assert_obs("out: ~%Math./lte 6 5", "#false");
}

#[test]
fn red_math_gt_gte() {
    assert_obs("out: ~%Math./gt 7 2", "#true");
    assert_obs("out: ~%Math./gt 2 7", "#false");
    assert_obs("out: ~%Math./gte 7 7", "#true");
    assert_obs("out: ~%Math./gte 2 7", "#false");
}

#[test]
fn red_math_predicates_mixed_numeric() {
    // Int/float cross-compare by numeric value (SYNTAX_02).
    assert_obs("out: ~%Math./lt 3 3.5", "#true");
    assert_obs("out: ~%Math./gte 3.0 3", "#true");
}

#[test]
fn red_math_predicate_curry_pipe() {
    // Same morphism machinery as the rest of ~%Math: currying + pipe.
    assert_obs("out: 3 |> ~%Math./lt 5", "#false");
    assert_obs("gt5: ~%Math./lt 5\nout: gt5 7", "#true");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES W2 — subset semantics on the `<` family (SYNTAX_06 §4 #10)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_atom_order_flip() {
    // L1-20 twin: distinct singletons never contain each other —
    // clean #false, NOT ⊥ (`=` family does not absorb).
    assert_obs("out: 3 <= 5", "#false");
    assert_obs("out: 3 < 5", "#false");
    assert_obs("out: 5 > 3", "#false");
    assert_obs("out: \"a\" < \"b\"", "#false");
}

#[test]
fn red_subtype_atom_vs_type() {
    // A <= B ⟺ (A & B) = A: 1 & @int = 1 → the singleton is a proper
    // subset of the int space.
    assert_obs("out: 1 <= @int", "#true");
    assert_obs("out: 1 < @int", "#true");
    assert_obs("out: @int <= 1", "#false");
    assert_obs("out: @int >= 1", "#true");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — flip-stable boundaries that must not move
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_reflexive_and_false_mirrors() {
    // Same verdicts under numeric AND subset readings — must not move.
    assert_obs("out: 2 <= 2", "#true");
    assert_obs("out: 3 < 3", "#false");
    assert_obs("out: 3 >= 5", "#false");
    assert_obs("out: 3 <= 3.0", "#true"); // numbers by value: one singleton
}

#[test]
fn pin_poset_track_untouched() {
    // Declared poset chains ARE lattice order — the real ≤, not the
    // numeric deviation. The flip must not touch them.
    let src = "~H: #{ #_|_ < #h1 < #h2 < #_ }\n";
    assert_obs(&format!("{src}out: ~H.#h1 <= ~H.#h2"), "#true");
    assert_obs(&format!("{src}out: ~H.#h1 < ~H.#h2"), "#true");
    assert_obs(&format!("{src}out: ~H.#h2 <= ~H.#h1"), "#false");
}

#[test]
fn pin_extremes_unchanged() {
    // cmp_extremes law pins, kept local: ⊥ ⊆ x, x ⊆ Top, Top ⊄ ⊥.
    assert_obs("out: (1 & 2) <= 5", "#true");
    assert_obs("out: 5 <= _", "#true");
    assert_obs("out: _ <= (1 & 2)", "#false");
}

#[test]
fn pin_atomic_eq_family_untouched() {
    assert_obs("out: 10 == 10", "#true");
    assert_obs("out: \"a\" == \"a\"", "#true");
    assert_obs("out: 1 = 1", "#true");
    assert_obs("out: (1 | 2) = (2 | 1)", "#true");
}

#[test]
fn pin_combo_union_order_still_frozen() {
    // MIGRATED (2026-07-20, order-wave W3 open): the W3 fence comes
    // down — non-atom order lands via the subset reduction.
    assert_obs("out: {a: 1} <= {a: @int}", "#true");
    assert_obs("out: (1 | 2) <= (1 | 2 | 3)", "#true");
}
