// Lexical-chain COMPLETION probes (2026-07-16, pre-committed by work
// order — docs/lexical_completion_handover.md). Successor to
// lexical_scope_probe_test.rs (that file stays live — red line).
//
// LAW (SPEC_04 §2.1, EXISTING): resolve_bare_name is FULLY recursive —
// any chain depth, inside Combos AND Cocoons; first match wins.
// MEASURED after the lexical-scope arc: the two-step snap/frame seal is a
// DEPTH-2 wall — `g2: e + 1` (needs e, needs d, needs k) → `_`; display
// shows it plainly (d/e resolved, g2/h2 `_`). Cocoons never seal: closed
// construction forces fields BEFORE seal_defining_scope runs, so cocoon
// siblings are `_` and cocoon shadowing tells the wrong-value lie #2
// (`k:5; {{k:7, d:k+1}}.d` → 6, law 8).
// DESIGN GUARDRAIL: whatever depth-independent mechanism is chosen, the
// cycle guard must keep mutual-reference semantics EXACTLY as pinned
// (cycle_test Top; frozen pins below) — the previous arc abandoned an
// ambient-scopes route precisely because it flipped that pin. Re-entry
// must fall through as unresolved (chain continues outward), never mint
// #divergent here (adjudication candidate, separate case).
// NOT in scope: cocoon eigenstate default (frozen pin MIGRATED
// 2026-07-16 to cocoon_eigenstate_probe_test.rs — see note below),
// mutual/self sibling reference semantics (frozen), effect isolation.

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
        "nlang-lexfull-{}-{}",
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

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — chain depth (§2.1 full recursion)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_three_hop_chain() {
    // L2-47. Today: `_` (depth-2 wall).
    assert_obs(
        "c: { k: 5, d: k + 1, e: d + k, g2: e + 1 }\nout: c.g2",
        "12",
    );
}

#[test]
fn red_four_hop_chain() {
    assert_obs(
        "c: { k: 5, d: k + 1, e: d + k, g2: e + 1, h2: g2 + e }\nout: c.h2",
        "23",
    );
}

#[test]
fn red_morphism_on_deep_sibling() {
    // Morphism body reads a 2-hop sibling — same wall. Today: `_`.
    assert_obs(
        "c: { k: 5, d: k + 1, e: d + k, f: (x -> x + e) }\nout: 1 |> c.f",
        "12",
    );
}

#[test]
fn red_private_combo_deep_chain() {
    // The wall is combo-universal (private combos included). Today: `_`.
    assert_obs(
        "c: { ~z: 9, k: 5, d: k + 1, e: d + k, g2: e + 1 }\nout: c.g2",
        "12",
    );
}

#[test]
fn red_display_deep_chain_resolved() {
    // Display shows the wall today: d/e resolved, g2/h2 `_`.
    let got = observe_nlang(
        "c: { k: 5, d: k + 1, e: d + k, g2: e + 1, h2: g2 + e }\nout: c",
        "out",
    );
    assert!(
        got.contains("g2: 12") && got.contains("h2: 23"),
        "display must resolve full chains: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — cocoon lexical chain (§2.1 inside {{}})
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_cocoon_sibling() {
    // L2-48. Today: `_` (closed force runs before seal).
    assert_obs("cc: {{ k: 5, d: k + 1 }}\nout: cc.d", "6");
}

#[test]
fn red_cocoon_morphism() {
    assert_obs("cc: {{ k: 5, f: (x -> x + k) }}\nout: 1 |> cc.f", "6");
}

#[test]
fn red_cocoon_shadowing_inner_first() {
    // L2-49. Today: 6 — wrong-value lie #2 (outer k substituted).
    assert_obs("k: 5\ncc: {{ k: 7, d: k + 1 }}\nout: cc.d", "8");
}

#[test]
fn red_nested_cocoon_sibling() {
    assert_obs("w: { cc: {{ k: 5, d: k + 1 }} }\nout: w.cc.d", "6");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — living faces + frozen separate-case boundaries
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_two_hop_chain_still() {
    // Previous arc's gate — must stay through the mechanism change.
    assert_obs("c: { k: 5, d: k + 1, e: d + k }\nout: c.e", "11");
}

#[test]
fn pin_chain_x_lift() {
    // Already green: frame hop + grandparent lift compose.
    assert_obs("w: { k: 5, c: { d: k + 1, e: d + 2 } }\nout: w.c.e", "8");
}

#[test]
fn pin_cocoon_plain_force() {
    assert_obs("cc: {{ a: 1 + 1 }}\nout: cc.a", "2");
}

#[test]
fn pin_cocoon_twin_eq() {
    // Anti-pollution tripwire, cocoon flavor.
    assert_obs(
        "cc1: {{ k: 5, d: k + 1 }}\ncc2: {{ k: 5, d: k + 1 }}\nout: cc1 = cc2",
        "#true",
    );
}

#[test]
fn pin_cross_depth_id_still() {
    // Repair pin territory of the previous arc — re-guarded here since the
    // mechanism this order replaces is exactly what that repair patched.
    assert_obs(
        "a1: { k: 5, d: k + 1 }\nb1: { q2: { k: 5, d: k + 1 } }\nout: a1.%id == (b1.q2).%id",
        "#true",
    );
}

#[test]
fn pin_mutual_sibling_frozen() {
    // FROZEN — separate case (adjudication candidate: ⊥ #divergent vs
    // open). Cycle-guard semantics must keep this exact value; the
    // previous arc's abandoned route flipped the workspace cycle_test.
    assert_obs("c: { a2: b2, b2: a2 }\nout: c.a2", "_");
}

// MIGRATED (2026-07-16, by the acceptor): `pin_self_ref_sibling_frozen`
// froze `c: {d: d+1}` → `_` pending the mutual/self adjudication. That
// ruling (SPEC_12 §1.1 as amended) classifies it a TRANSFORM cycle
// (arithmetic hop) → ⊥ #divergent — the L2-17 canon family. Successor
// gate: red_combo_transform_self_divergent in static_cycle_probe_test.rs.
// Second same-type acceptor oversight (three sibling pins migrated at
// order time, this one missed); implementer halted correctly again.

// MIGRATED (2026-07-16, by the acceptor): `pin_cocoon_closed_miss_frozen`
// froze `{{a:1}}.b` → `_` as an EXPOSED VIOLATION pending its own arc.
// That arc is now docs/cocoon_eigenstate_handover.md — successor gate
// `red_cocoon_access_bottom` in cocoon_eigenstate_probe_test.rs demands
// the lawful ⊥ #missing_key (SPEC_03 §1.2 #1/§1.3). Migration performed
// by the acceptor after the implementer HALTED on the pin conflict per
// the red-line rule (correct protocol behavior; the pin should have been
// migrated in the order commit — acceptor oversight, co-owned).
