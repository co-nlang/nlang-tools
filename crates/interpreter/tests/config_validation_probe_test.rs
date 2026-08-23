// ~%Config field-name/type validation probes (2026-07-20, pre-committed
// by work order — docs/config_validation_handover.md).
//
// RULING A (2026-07-20): the knob family is CLOSED. The root
// `~%Config.<bare>` exemption (system-axis arc) validated SHAPE only —
// name membership and value type are now law (SPEC_09 §6 knob table):
// unknown names (incl. typos), wrong types, ⊥ and Top all die LOUDLY at
// the evolve boundary (same machinery as G2-S / system-axis root
// writes; CLI exit 1; TAG_REGISTRY #invalid_config — a named error
// class, never a node-level ⊥). Future knobs go through the spec
// evolution process; third-party engine designs go through ~%Engine —
// nobody privately extends ~%Config.
//
// MEASURED (v0.2.27): all three violation faces are accepted in
// silence — `~%Config.fool: 50` rc=0; the LIE face `~%Config.feul:
// 99999` leaves fuel at 10000 so the user's "raised" horizon still
// blurs; `fuel: "lots"` / `strategy: 5` accepted then ignored at the
// consumption sites (pattern-match miss → default). Display face:
// `out: ~%Config` shows the staged FRAGMENT (`{ fuel: 50 }`) instead
// of the effective 7-knob config; per-knob reads and the parenthesized
// lens spelling read through correctly (healthy, pinned).
//
// Knob table (genesis): fuel 10000 / timeout 1000 / max_branches 64 /
// max_unification_depth 256 / max_lifting_depth 32 /
// max_pattern_nodes 1024 — non-negative Int; strategy #blur (also
// #strict / #approximate) — Tag in that set.
//
// NOT in scope: combo-level ~%Config non-exemption and whole-group
// `~%Config: {...}` loud reject (system_axis pins), %fuel node-level
// hints, the future whole-group-replacement legislation (separate
// ledgered case).

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("cfgval")
}

/// 64 MiB thread; returns (all evolves ok, observed display).
fn run_program(src: &str, path: &str) -> (bool, String) {
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
            let mut all_ok = true;
            for f in &program.fields {
                if universe.evolve(&engine, f).is_err() {
                    all_ok = false;
                }
            }
            let p = Path {
                anchor: PathAnchor::Bare,
                segments: path.split('.').map(|s| s.to_string()).collect(),
                span: Span::default(),
            };
            (all_ok, universe.observe(&engine, &p).to_nlang(0))
        })
        .unwrap()
        .join()
        .unwrap()
}

fn assert_obs(src: &str, expect: &str) {
    let (_, got) = run_program(src, "out");
    assert_eq!(got, expect, "{src:?} :: out");
}

fn assert_loud(src: &str) {
    let (all_ok, _) = run_program(src, "out");
    assert!(!all_ok, "must die loudly at the evolve boundary: {src:?}");
}

fn assert_clean(src: &str) {
    let (all_ok, _) = run_program(src, "out");
    assert!(all_ok, "valid config write must evolve cleanly: {src:?}");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — closed knob family (SPEC_09 §6), loud at evolve boundary
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_config_unknown_name_loud() {
    assert_loud("~%Config.fool: 50\nout: 1 + 1");
}

#[test]
fn red_config_typo_lie_face_loud() {
    // THE lie face: user believes fuel is raised; the horizon still
    // blurs at 10000. Silence is forbidden — this must be loud.
    assert_loud("~%Config.feul: 99999\nout: 1 + 1");
}

#[test]
fn red_config_wrong_type_int_loud() {
    assert_loud("~%Config.fuel: \"lots\"\nout: 1 + 1");
    // Negative after evaluation is not a non-negative Int.
    assert_loud("~%Config.fuel: 0 - 5\nout: 1 + 1");
}

#[test]
fn red_config_wrong_type_strategy_loud() {
    assert_loud("~%Config.strategy: 5\nout: 1 + 1");
    // Tag, but outside the lawful set {#blur, #strict, #approximate}.
    assert_loud("~%Config.strategy: #eager\nout: 1 + 1");
}

#[test]
fn red_config_bottom_top_loud() {
    // ⊥ and Top are not knob values — no silent no-op, no node ⊥.
    assert_loud("~%Config.fuel: 1 & 2\nout: 1 + 1");
    assert_loud("~%Config.fuel: _\nout: 1 + 1");
}

#[test]
fn red_config_effective_display() {
    // SPEC_09 §6: observing ~%Config shows the EFFECTIVE config —
    // genesis ∧ overrides, all seven knobs — not the staged fragment.
    let (_, got) = run_program("~%Config.fuel: 50\nout: ~%Config", "out");
    // O41: genesis timeout is `#_` (unbound), not 1000.
    for needle in [
        "fuel: 50",
        "timeout: #_",
        "max_branches: 64",
        "max_unification_depth: 256",
        "max_lifting_depth: 32",
        "max_pattern_nodes: 1024",
        "strategy: #blur",
    ] {
        assert!(
            got.contains(needle),
            "effective config must show {needle:?}, got: {got}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — the lawful family stays writable and readable
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_seven_knobs_writable() {
    assert_clean("~%Config.fuel: 9999\nout: 1 + 1");
    assert_clean("~%Config.timeout: 500\nout: 1 + 1");
    assert_clean("~%Config.max_branches: 32\nout: 1 + 1");
    assert_clean("~%Config.max_unification_depth: 128\nout: 1 + 1");
    assert_clean("~%Config.max_lifting_depth: 16\nout: 1 + 1");
    assert_clean("~%Config.max_pattern_nodes: 512\nout: 1 + 1");
    assert_clean("~%Config.strategy: #strict\nout: 1 + 1");
    assert_clean("~%Config.strategy: #approximate\nout: 1 + 1");
}

#[test]
fn pin_override_readthrough() {
    // Per-knob reads see the override AND genesis through the overlay.
    assert_obs("~%Config.fuel: 50\nout: ~%Config.fuel", "50");
    // O41: genesis timeout is `#_`.
    assert_obs("~%Config.fuel: 50\nout: ~%Config.timeout", "#_");
    assert_obs("~%Config.fuel: 50\nout: (~%Config).max_branches", "64");
}

#[test]
fn pin_expr_rhs_lawful() {
    // The RHS is an expression; validation applies to the EVALUATED
    // value (40 + 10 = 50, a lawful non-negative Int).
    assert_obs("~%Config.fuel: 40 + 10\nout: ~%Config.fuel", "50");
}

#[test]
fn pin_fuel_override_real_effect() {
    // The override must keep steering the horizon (system_axis trap
    // pin twin, kept local so this file stands alone).
    let chain = vec!["1"; 4000].join(" + ");
    let (_, got) = run_program(
        &format!("~%Config.fuel: 50\nbig: {}\nout: big", chain),
        "out",
    );
    assert!(
        got.starts_with("#blur"),
        "fuel 50 must blur the long chain: {got:?}"
    );
}
