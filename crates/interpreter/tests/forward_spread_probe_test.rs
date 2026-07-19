// Forward-reference × spread probes (2026-07-19, pre-committed by
// work order — docs/forward_spread_handover.md). UNFREEZES the
// spread-collision arc's Q3 case (pin_forward_ref_spread_frozen —
// migrated in the open commit).
//
// MEASURED (v0.2.25): spread is EAGER at construction — a forward-
// referenced source contributes NOTHING, silently, in every face:
//   basic field   q: {...later, b:1}; later: {a:7} → q.a = _   (law 7)
//   collision     w: {a:1..5, ...src}; src: {a:1}  → w.a = 1..5 (law 1)
//   ⊥ source      w: {b:1, ...bot}; bot: 1&2 → %cause _ (law #conflict;
//                 backward control = #conflict today)
//   blur source   forward → no-op (law: absorption verbatim)
//   alias chain   q: {...al, b:1}; al: src; src: {a:7} → _ (law 7)
// All violate SPEC_03 field simultaneity / commutativity (L1-26/27
// forward refs are law) — the source's textual position changes the
// result. Zero new adjudication: SPEC_03 §3.1 timing clause (2026-07-19)
// = spread expands at OBSERVATION CONVERGENCE; collision-intersect /
// ⊥-propagation / blur-absorption apply at expansion identically.
//
// Healthy faces (pinned): never-defined source = open _ → Top no-op
// (both orders); cyclic spread → #divergent (C4 machinery).
// RECORD-DUTY (not a gate): cross-dep `q: {...src, b:1}; src: {a: q.b}`
// today `_` — ideal 1 via per-coordinate laziness, but cycle-guard
// granularity may lawfully differ; record delivered behavior in §5.
// NOT in scope: spread privacy (spread_privacy pins), collision
//   semantics themselves (spread_collision pins), Top no-op boundary,
//   CAID timing beyond one honest note (thunk-vs-solid store timing —
//   record what ships).

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
        "nlang-fwdspread-{}-{}",
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

fn flat_chain(n: usize) -> String {
    vec!["1"; n].join(" + ")
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — spread expands at observation convergence, order-free
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore] // RED GATE — remove at delivery
fn red_fwd_spread_basic_field() {
    // L2-81 twin.
    assert_obs("q: {...later, b: 1}\nlater: {a: 7}\nout: q.a", "7");
}

#[test]
#[ignore] // RED GATE — remove at delivery
fn red_fwd_spread_collision_intersect() {
    assert_obs("w: {a: 1..5, ...src}\nsrc: {a: 1}\nout: w.a", "1");
}

#[test]
#[ignore] // RED GATE — remove at delivery
fn red_fwd_spread_bottom_propagates() {
    // L2-82 twin: ⊥ source spreads its cause regardless of position.
    assert_obs("w: {b: 1, ...bot}\nbot: 1 & 2\nout: (w).%cause", "#conflict");
}

#[test]
#[ignore] // RED GATE — remove at delivery
fn red_fwd_spread_blur_absorbs() {
    // Blur source absorbs the target verbatim (SPEC_03 §3.1 Blur row).
    let got = observe_nlang(
        &format!("w: {{b: 1, ...big}}\nbig: {}\nout: w", flat_chain(4000)),
        "out",
    );
    assert!(
        got.starts_with("#blur") && got.contains("fuel_exhausted"),
        "forward blur source must absorb the target: {got:?}"
    );
}

#[test]
#[ignore] // RED GATE — remove at delivery
fn red_fwd_spread_alias_chain() {
    assert_obs("q: {...al, b: 1}\nal: src\nsrc: {a: 7}\nout: q.a", "7");
}

#[test]
#[ignore] // RED GATE — remove at delivery
fn red_fwd_spread_commutativity_eq() {
    // The law face itself: both textual orders give the SAME combo.
    assert_obs(
        "q1: {...s1, b: 1}\ns1: {a: 7}\ns2: {a: 7}\nq2: {...s2, b: 1}\nout: q1 = q2",
        "#true",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — healthy boundaries that must not move
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_backward_spread_unchanged() {
    assert_obs("later: {a: 7}\nq: {...later, b: 1}\nout: q.a", "7");
    assert_obs("src: {a: 1}\nw: {a: 1..5, ...src}\nout: w.a", "1");
    assert_obs("bot: 1 & 2\nw: {b: 1, ...bot}\nout: (w).%cause", "#conflict");
}

#[test]
fn pin_never_defined_source_noop() {
    // Undefined source = open _ → Top spread no-op (existing law) —
    // green today in BOTH orders; the fix must not turn this into ⊥.
    assert_obs("q: {...never, b: 1}\nout: q.b", "1");
    assert_obs("out: ({...never2, b: 1}).b", "1");
}

#[test]
#[ignore] // RED GATE — remove at delivery
fn red_fwd_cyclic_spread_divergent() {
    // CALIBRATION FINDING: today `_` — the eager no-op disease MASKS
    // the cycle (source never expands). After the timing fix the
    // alias-detour self-spread must be caught by the C4 guard, not
    // hang and not stay silent (Q4 + collision-arc alias improvement).
    assert_obs("al: a\na: {x: 1, ...al}\nout: (a.x).%cause", "#divergent");
}

#[test]
fn pin_spread_privacy_unchanged() {
    // Outsider spread still excludes ~ fields (spread_privacy law).
    assert_obs("p: {v: 1, ~s: 9}\nq: {...p}\nout: q.v", "1");
    let got = observe_nlang("p: {v: 1, ~s: 9}\nq: {...p}\nout: q.~s", "out");
    assert!(
        got.contains("#private_access_violation"),
        "outsider private read stays walled: {got:?}"
    );
}
