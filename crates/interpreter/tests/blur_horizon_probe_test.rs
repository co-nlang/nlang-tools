// G3 blur-horizon propagation probes (2026-07-13, pre-committed by work
// order — docs/g3_blur_erasure_handover.md).
//
// RE-DIAGNOSIS (5th ledger-scope correction): G3 is NOT "runaway cause
// refinement". Default strategy is Blur: fuel exhaustion correctly mints
// a first-class horizon value (#blur snapshot, SPEC_08 §3.2 — display
// and deterministic CAID already implemented). The defect is that
// VALUE-CONTEXT CONSUMERS have no Blur arm: eval_math's catch-all mints
// a fresh ⊥ #conflict (horizon → conflict, identity erased) and atomic
// cmp falls through to structural comparison (silent #false — same lie
// class as G1). Generic to ALL fuel exhaustion; a flat 4000-term
// addition dies identically. Runaway morphisms are just the easiest
// entry.
//
// RULING (SPEC_08 §3.2.2, approved 2026-07-13):
//   R1 value contexts ABSORB #blur (pass it out unchanged — never mint
//      #conflict, never a silent boolean; ontological status of #blur
//      vs ⊥ must not be rewritten).
//   R2 arguments CARRY #blur, they don't consume it; absorption happens
//      at the first value context inside the body. Binding/force
//      boundaries must not re-mint.
//   R3 fuel-exhaustion cause is #fuel_exhausted, honestly — #divergent
//      is reserved for DETECTED cycles (coordinate self-reference,
//      L2-17). Undecidable runaways must not claim divergence.
//   R4 %cause/%type on #blur reads its BlurCause tag.

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-g3probe-{}-{}",
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

/// Horizon verdict: a #blur snapshot with fuel-exhaustion cause. The CAID
/// is salted per engine instance — only the form and cause are normative.
fn assert_blur_fuel(src: &str) {
    let got = observe_nlang(src, "out");
    assert!(
        got.starts_with("#blur"),
        "{src:?} :: out — expected #blur horizon, got {got:?}"
    );
    assert!(
        got.contains("#fuel_exhausted") || got.contains("fuel_exhausted"),
        "{src:?} :: out — expected fuel_exhausted cause, got {got:?}"
    );
}

fn flat_chain(n: usize) -> String {
    vec!["1"; n].join(" + ")
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — R1: value contexts absorb #blur, never re-mint
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_flat_math_exhaustion_is_blur() {
    // Generic case, no recursion anywhere. Today: ⊥ #conflict.
    assert_blur_fuel(&format!("out: {}", flat_chain(4000)));
}

#[test]
fn red_runaway_slash_morphism_is_blur() {
    assert_blur_fuel("/recursive: x -> /recursive (x + 1)\nout: /recursive 1");
}

#[test]
fn red_runaway_bare_morphism_is_blur() {
    // `/` is not a variable (G2 lesson): bare-name control.
    assert_blur_fuel("rec: (x -> rec (x + 1))\nout: rec 1");
}

#[test]
fn red_same_arg_runaway_is_blur_not_divergent() {
    // R3: same-argument self-call is theoretically detectable but the
    // detector is a SEPARATE future case — until then the honest cause
    // is fuel exhaustion, not a claimed divergence.
    assert_blur_fuel("same: (x -> same x)\nout: same 1");
}

#[test]
fn red_eqeq_blur_operand_absorbs() {
    // Today: silent #false (blur falls through to structural compare) —
    // same lie class as G1's combo ==.
    assert_blur_fuel(&format!("big: {}\nout: big == 1", flat_chain(4000)));
}

#[test]
fn red_neq_blur_operand_absorbs() {
    assert_blur_fuel(&format!("big: {}\nout: big != 1", flat_chain(4000)));
}

#[test]
fn red_pipe_blur_arg_carries_body_absorbs() {
    // R2: the argument carries the blur; the body's math absorbs it.
    assert_blur_fuel(&format!(
        "inc: (n -> n + 1)\nbig: {}\nout: big |> inc",
        flat_chain(4000)
    ));
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — R4: meta observation reads BlurCause (L2-21/22)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_runaway_type_meta_fuel_exhausted() {
    // L2-21. cocoon_shape: blur meta whitelist is %cause/%caid only —
    // read the BlurCause tag via %cause (former %type alias retired).
    assert_obs(
        "/recursive: x -> /recursive (x + 1)\nout: (/recursive 1).%cause",
        "#fuel_exhausted",
    );
}

#[test]
fn red_flat_cause_meta_fuel_exhausted() {
    // L2-22.
    assert_obs(
        &format!("big: {}\nout: big.%cause", flat_chain(4000)),
        "#fuel_exhausted",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — detected divergence, ⊥ short-circuits, small paths
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_coordinate_selfref_stays_divergent() {
    // L2-17: DETECTED cycle keeps #divergent — R3's reserved case.
    let got = observe_nlang("a: a + 1\nout: a", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#divergent"),
        "L2-17 regressed: {got:?}"
    );
}

#[test]
fn pin_bottom_conflict_math_short_circuit() {
    // ⊥ absorption in math is cause-preserving today — must stay.
    let got = observe_nlang("out: (1 & 2) + 1", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#conflict"),
        "bottom short-circuit lost: {got:?}"
    );
}

#[test]
fn pin_bottom_arg_cause_preserved_through_apply() {
    let got = observe_nlang("a: a + 1\nf: (n -> n + 1)\nout: f a", "out");
    assert!(
        got.contains("#divergent"),
        "bottom-arg cause erased: {got:?}"
    );
}

#[test]
fn pin_small_math_unaffected() {
    assert_obs("out: 1 + 1", "2");
}

#[test]
fn pin_moderate_chain_converges() {
    // Well under the default budget: stays exact.
    assert_obs(&format!("out: {}", flat_chain(100)), "100");
}

// pin_lattice_eq_blur_current_behavior REMOVED 2026-07-14 by the ACCEPTOR:
// its freeze clause read "until the separate case" — that case is the Blur
// boundary ruling (SPEC_08 §3.2.2 #6). Superseded by the red gate
// blur_boundary_probe_test::red_eq_blur_vs_value_absorbs.
