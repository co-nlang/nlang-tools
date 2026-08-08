// Blur spread-source probes (2026-07-16, pre-committed by work order —
// docs/blur_spread_handover.md).
//
// RULING (SPEC_03 §3.1 Blur row, approved 2026-07-16): spread is a
// FULL-coordinate read; behind a horizon the source's field set is
// unknowable → the target container becomes THAT #blur verbatim
// (cause/CAID/horizon params preserved — Q2; never mint a new cause,
// never silently no-op). Derivation from two existing laws:
//   {b:1, ...big} ≡ {b:1} & unbox(big); unboxing behind the horizon is
//   the #blur itself; existing merge absorption finishes the job.
// Isomorphic to the Bottom spread clause (⊥ propagates cause / blur
// propagates the snapshot) and to SPEC_08 §3.2.2 #5 coordinate
// absorption ("the horizon must not silently vanish under narrowing").
// Order-blind (merge commutativity), per-node (nesting), target kind
// irrelevant ({} and {{}} both absorb). Top spread no-op UNCHANGED —
// the borderline is exactly "no constraint vs unknowable".
// MEASURED on v0.2.16: engine routes #blur through the Top arm
// ("无效操作") — {b:1, ...big} → {b:1}, %cause `_`, both orders, nested,
// cocoon target: total horizon erasure.
// NOT in scope: `<`/`<=` × blur (§4.10); forward-ref × spread (frozen
// in spread_collision_probe_test.rs); circular spread (pinned there).
// CORRECTION (2026-07-17, cause-canon audit): the "`&` × blur snapshot
// NON-preservation" suspicion once noted here is WITHDRAWN — it was a
// cross-process measurement artifact (per-instance horizon salt). The
// engine honors §3.2.2 #1; pinned in cause_canon_probe_test.rs
// (pin_blur_merge_caid_verbatim, L2-65).

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("blurspread")
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

/// Horizon verdict: #blur form with fuel-exhaustion cause. CAID is salted
/// per engine instance — only form and cause are normative here.
fn assert_blur_horizon(src: &str) {
    let got = observe_nlang(src, "out");
    assert!(
        got.starts_with("#blur"),
        "{src:?} :: out — expected #blur horizon, got {got:?}"
    );
    assert!(
        got.contains("max_depth_exceeded"),
        "{src:?} :: out — expected max_depth_exceeded cause, got {got:?}"
    );
}

fn flat_chain(n: usize) -> String {
    vec!["1"; n].join(" + ")
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — Blur spread source absorbs (SPEC_03 §3.1 Blur row)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_blur_spread_absorbs_cause() {
    // L2-57.
    assert_obs(
        &format!(
            "big: {}\np: {{ b: 1, ...big }}\nout: p.%cause",
            flat_chain(4000)
        ),
        "#max_depth_exceeded",
    );
}

#[test]
fn red_blur_spread_form() {
    assert_blur_horizon(&format!(
        "big: {}\nout: {{ b: 1, ...big }}",
        flat_chain(4000)
    ));
}

#[test]
fn red_blur_spread_order_blind() {
    assert_blur_horizon(&format!(
        "big: {}\nout: {{ ...big, b: 1 }}",
        flat_chain(4000)
    ));
}

#[test]
fn red_blur_spread_empty_target() {
    assert_blur_horizon(&format!("big: {}\nout: {{ ...big }}", flat_chain(4000)));
}

#[test]
fn red_blur_spread_caid_preserved() {
    // Q2 snapshot verbatimness: absorption carries the SOURCE snapshot —
    // same CAID, not a re-mint at the spread site.
    assert_obs(
        &format!(
            "big: {}\np: {{ b: 1, ...big }}\nout: p.%caid == big.%caid",
            flat_chain(4000)
        ),
        "#true",
    );
}

#[test]
fn red_blur_spread_nested_per_node() {
    // L2-58. Per-node: the inner combo absorbs, the OUTER stays a combo.
    let src = format!(
        "big: {}\nw: {{ a: {{ ...big }} }}\nout: (w.a).%cause",
        flat_chain(4000)
    );
    assert_obs(&src, "#max_depth_exceeded");
    let outer = observe_nlang(
        &format!(
            "big: {}\nw: {{ a: {{ ...big }} }}\nout: w.%cause",
            flat_chain(4000)
        ),
        "out",
    );
    assert_eq!(outer, "_", "outer combo must NOT absorb (per-node)");
}

#[test]
fn red_blur_spread_cocoon_target() {
    // Target kind irrelevant: absorption precedes any target attribute.
    assert_blur_horizon(&format!("big: {}\nout: {{{{ ...big }}}}", flat_chain(4000)));
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — the tri-border (Top/⊥/blur) and neighbor laws
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_top_spread_noop() {
    // L2-59 (green law pin). Top = no constraint → no-op; the borderline
    // the fix must NOT cross ("no constraint" vs "unknowable").
    assert_obs("p: { x: 1, ..._ }\nout: p.x", "1");
    let cause = observe_nlang("p: { x: 1, ..._ }\nout: p.%cause", "out");
    assert_eq!(cause, "_", "Top spread must stay a no-op, no cause");
}

#[test]
fn pin_bottom_spread_collapse() {
    // Neighbor lattice level (v0.2.13 law): ⊥ spread collapses the target
    // PROPAGATING the source cause.
    assert_obs("bot: 1 & 2\nout: ({ b: 1, ...bot }).%cause", "#conflict");
}

#[test]
fn pin_blur_merge_absorbs() {
    // Neighbor law (& absorption) — the derivation's second premise.
    assert_blur_horizon(&format!("big: {}\nout: {{ b: 1 }} & big", flat_chain(4000)));
}

#[test]
fn pin_blur_nav_absorbs() {
    // SPEC_08 §3.2.2 #5 — coordinate absorption must survive untouched.
    assert_blur_horizon(&format!("big: {}\nout: big.name", flat_chain(4000)));
}

#[test]
fn pin_blur_cause_meta() {
    // Recipe sanity: flat_chain(4000) is a fuel-exhaustion #blur.
    assert_obs(
        &format!("big: {}\nout: big.%cause", flat_chain(4000)),
        "#max_depth_exceeded",
    );
}

#[test]
fn pin_atom_spread_val() {
    // Heterogeneous neighbor row (Atom): {%val: v} shell unchanged.
    assert_obs("out: ({ x: 1, ...5 }).%val", "5");
}
