// Static-cycle adjudication probes (2026-07-16, pre-committed by work
// order — docs/static_cycle_handover.md).
//
// RULING (approved 2026-07-16; SPEC_12 §1.1 #2/#3 amended + caused Top):
//   STATIC cycle — every hop in the loop is a PURE REFERENCE (bare name
//   or pure path; projection adds no information) → Top (solution set =
//   everything; SPEC_12 §1). The Top CARRIES provenance: %cause reads
//   #static_cycle (+ loop members in the detail) — user proposal.
//   TRANSFORM cycle — any hop is a non-pure-reference expr (arithmetic,
//   comparison, application, pipe, literal construction) → ⊥ #divergent
//   (solution set = ∅; SPEC_12 §2.2, own example `a: a+1` = L2-17 canon).
//   LEVEL-INDEPENDENT (#3): root and combo answers must agree.
// CAUSED-TOP GUARDRAILS: lattice-neutral (`= _` → #true, `& x` absorbs,
// unify/CAID/dedupe treat as bare Top — mirrors ⊥-cause never entering
// equality); NO propagation (an operation consuming it yields bare Top,
// provenance evaporates); display stays `_` (meta-only readability).
// MEASURED on v0.2.15: split by level, wrong BOTH ways — root minted ⊥
// #divergent for everything (static included); combo soft re-entry gave
// `_` for everything (transform included).
// MIGRATED at order time (by the acceptor): pin_ref_cycle_still_divergent
// (forward_ref), l217_self_identity_divergent + l217_path_cycle_divergent
// (divergence_probe) — pure-reference forms now lawful Top. Transform
// pins (`a: b+1` forms) stay red lines.

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
        "nlang-statcyc-{}-{}",
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

fn assert_divergent(src: &str) {
    let got = observe_nlang(src, "out");
    assert!(
        got.starts_with("_|_") && got.contains("divergent"),
        "{src:?} :: out — expected ⊥ #divergent, got {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — static cycles → Top (root level; today over-killed to ⊥)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_root_static_mutual_top() {
    // L2-53. Today: ⊥ #divergent.
    assert_obs("a: b\nb: a\nout: a", "_");
}

#[test]
fn red_root_static_self_top() {
    assert_obs("x: x\nout: x", "_");
}

#[test]
fn red_path_pure_cycle_top() {
    // Pure PATH loop = pure reference (projection adds no information).
    assert_obs("s: { v: s.v }\nout: s.v", "_");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — transform cycles → ⊥ #divergent (combo level; today `_`)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_combo_transform_self_divergent() {
    // L2-54 mirror. Both spellings.
    assert_divergent("c: { d: d + 1 }\nout: c.d");
    assert_divergent("c: { d: d + 1 }\ng: c.d\nout: g");
}

#[test]
fn red_combo_transform_mutual_divergent() {
    assert_divergent("c: { a2: b2 + 1, b2: a2 + 1 }\nout: c.a2");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — caused Top: %cause provenance (user proposal)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_static_cycle_cause_combo() {
    // L2-55. Today: `_`.
    assert_obs("c: { a2: b2, b2: a2 }\nout: (c.a2).%cause", "#static_cycle");
}

#[test]
fn red_static_cycle_cause_root() {
    // Today: #divergent (the over-kill's cause).
    assert_obs("a: b\nb: a\nout: a.%cause", "#static_cycle");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — caused-Top lattice neutrality
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_caused_top_unit_law() {
    // ⊤ & 5 = 5; provenance evaporates on consumption. Today: ⊥.
    assert_obs("a: b\nb: a\nout: a & 5", "5");
}

#[test]
fn red_caused_top_eq_top() {
    // Caused Top ≡ Top under the unique equality (G1). Today: #false.
    assert_obs("a: b\nb: a\nout: a = _", "#true");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — transform detection stays; boundaries
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_l217_canonical_transform_divergent() {
    // The L2-17 canon vector form — must stay ⊥ through the refinement.
    assert_divergent("a: a + 1\nout: a");
}

#[test]
fn pin_root_mutual_transform_divergent() {
    assert_divergent("a: b + 1\nb: a + 1\nout: a");
}

#[test]
fn pin_mixed_alias_transform_divergent() {
    // One pure hop + one transform hop = transform loop (any hop taints).
    assert_divergent("a: b\nb: a + 1\nout: a");
}

#[test]
fn pin_combo_static_display_unchanged() {
    // Display of static-cycle Top stays `_` (provenance is meta-only).
    assert_obs("c: { a2: b2, b2: a2 }\nout: c.a2", "_");
}

#[test]
fn pin_static_cycle_no_poison() {
    // Siblings outside the loop are untouched.
    assert_obs("c: { d: e, e: d, f: 5 }\nout: c.f", "5");
}

#[test]
fn pin_plain_top_no_cause() {
    // L2-56 / L2-91 split (caused_top ruling C, 2026-07-20):
    // navigation open-miss is caused Top `#no_coordinate`; bare-name miss
    // and literal `_` stay causeless (spread no-op law keys off bare Top).
    // MIGRATED-2: was both faces `_` under pre-ruling-C open-miss=bare.
    assert_obs("c: { a: 1 }\nout: (c.b).%cause", "#no_coordinate");
    assert_obs("out: zz.%cause", "_");
    assert_obs("t: _\nout: t.%cause", "_");
}

#[test]
fn pin_forward_ref_still_lives() {
    // Forward reference is NOT a cycle — the machinery this order touches
    // is exactly the forward-ref arc's; its living face must not regress.
    assert_obs("out: a\na: 5", "5");
}

// ─────────────────────────────────────────────────────────────────────────
// ACCEPTANCE-REPAIR PIN (2026-07-16): loop members must span the WHOLE
// loop. Delivery minted members from the chain alone — a mutual cycle
// showed ["a"], misreading 互指 as 自指; the ruling makes loop shape
// readable from the member list. Repair: cycle_reentry unions the
// re-entered coordinate into the members at all four mint sites.
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_cycle_members_span_loop() {
    let got = observe_nlang("a: b\nb: a\nout: <<a.%cause>>", "out");
    assert!(
        got.contains("\"a\"") && got.contains("\"b\""),
        "mutual cycle must list both members: {got:?}"
    );
    let got3 = observe_nlang("a: b\nb: c2\nc2: a\nout: <<a.%cause>>", "out");
    assert!(
        got3.contains("\"a\"") && got3.contains("\"b\"") && got3.contains("\"c2\""),
        "3-cycle must list all members: {got3:?}"
    );
    let gots = observe_nlang("x: x\nout: <<x.%cause>>", "out");
    assert!(
        gots.contains("\"x\"") && !gots.contains("\"a\""),
        "self cycle lists exactly itself: {gots:?}"
    );
}
