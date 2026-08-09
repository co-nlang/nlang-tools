// ~%Config convergence probes (2026-07-13, pre-committed by work order —
// docs/config_home_handover.md).
//
// RULING (SPEC_08 §3.1, approved 2026-07-13): the normative home of the
// horizon parameters is the system-axis module ~%Config with BARE field
// names (fuel/timeout/strategy/max_branches/max_unification_depth/
// max_lifting_depth/max_pattern_nodes), per-observation scope, engine
// MUST implement. The `%` spelling is reserved for node metadata — the
// engine's current `~%Config.%fuel` spelling is a category error.
// Node-level %fuel etc. are advisory hints (non-binding; R5 lint).
//
// MEASURED TODAY: ~%Config exists but %-spelled (`~%Config.%fuel` →
// 10000, `~%Config.fuel` → `_`); max_lifting_depth is not configurable;
// strategy has three homes (~%Config.%strategy genesis read,
// /set_strategy ctx override, ~%Engine.state.strategy dead display).

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("cfgprobe")
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
// RED GATES — bare-name ~%Config fields observable at genesis defaults
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_config_fuel_bare() {
    // L2-23.
    assert_obs("out: ~%Config.fuel", "10000");
}

#[test]
fn red_config_strategy_bare() {
    assert_obs("out: ~%Config.strategy", "#blur");
}

#[test]
fn red_config_timeout_bare() {
    assert_obs("out: ~%Config.timeout", "1000");
}

#[test]
fn red_config_max_branches_bare() {
    assert_obs("out: ~%Config.max_branches", "64");
}

#[test]
fn red_config_max_unification_depth_bare() {
    // Renames the engine's `%max_depth` to the SPEC_09 §6 dictionary name.
    assert_obs("out: ~%Config.max_unification_depth", "256");
}

#[test]
fn red_config_max_pattern_nodes_bare() {
    assert_obs("out: ~%Config.max_pattern_nodes", "1024");
}

#[test]
fn red_config_max_lifting_depth_bare() {
    // New field: EvalContext.max_lifting_depth (32) was never configurable.
    assert_obs("out: ~%Config.max_lifting_depth", "32");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — wiring must stay live through the rename
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_fuel_wiring_alive_flat_exhaustion_blurs() {
    // The genesis fuel value must still reach EvalContext after the rename:
    // a 4000-term chain exhausts the 10000 default → #blur (G3 law).
    let chain = vec!["1"; 4000].join(" + ");
    let got = observe_nlang(&format!("out: {chain}"), "out");
    assert!(
        got.starts_with("#blur") && got.contains("max_depth_exceeded"),
        "fuel wiring broken: {got:?}"
    );
}

#[test]
fn pin_moderate_chain_still_converges() {
    let chain = vec!["1"; 100].join(" + ");
    assert_obs(&format!("out: {chain}"), "100");
}

#[test]
fn pin_small_math_unaffected() {
    assert_obs("out: 1 + 1", "2");
}
