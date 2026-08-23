// Blur boundary probes (2026-07-14, pre-committed by work order —
// docs/blur_boundary_handover.md).
//
// RULING (SPEC_08 §3.2.2 #5/#6 + #4 %caid, approved 2026-07-14): G3
// legislated value contexts only; coordinate context and the set family
// were explicitly excluded. Measured lies on v0.2.9:
//   - nav `big.name` → ⊥ #invalid_path (claims to KNOW the path is
//     invalid; behind a horizon nothing is known),
//   - `bigA = bigB` (same text, two bindings) → #false while both
//     denote 4000 — "definitely unequal" is PROVABLY wrong,
//   - union nav silently culls the blur branch (`({a:1}|big).a` → `1`,
//     horizon trace erased),
//   - `%caid` not navigable (R4 opened %cause/%type only).
// Law:
//   #5 coordinate absorption — non-meta navigation on #blur passes it
//      out unchanged; union per-branch projection keeps blur branches
//      (only ⊥ branches are culled).
//   #6 lattice-eq two-stage — both #blur with equal CAID → #true
//      (observation determinism; same relation as union dedupe, G1
//      unique equality); any other #blur operand → verdict is behind
//      the horizon → absorb left-priority (never #false).
//   #4 %caid meta whitelist — snapshot identity is totally decidable
//      via `x.%caid == y.%caid` (string atoms; `===` was rejected).
// EXCLUDED: `<`/`<=` × blur — order judgment is unimplemented on
// combos/unions globally (even `1 <= (1|2)` is ⊥ #conflict); blur is
// not the variable there (§4.10 case). Frozen as pins.

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("blurbnd")
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

/// Horizon verdict: #blur form with depth-exhaustion cause.
/// `flat_chain(4000)` exceeds `max_unification_depth` (default 256) long
/// before fuel runs out. TAG_REGISTRY §2.7.2 (the_name_points_at_the_remedy):
/// that situation must report `#max_depth_exceeded`, not `#fuel_exhausted`.
/// CAID is salted per engine instance — only form and cause are normative.
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
// RED GATES — #5 coordinate absorption
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_nav_blur_absorbs() {
    // Today: ⊥ #invalid_path — the "path is invalid" lie.
    assert_blur_horizon(&format!("big: {}\nout: big.name", flat_chain(4000)));
}

#[test]
fn red_nav_through_combo_absorbs() {
    assert_blur_horizon(&format!(
        "big: {}\nc: {{ x: big }}\nout: c.x.name",
        flat_chain(4000)
    ));
}

#[test]
fn red_nav_blur_cause_after_absorb() {
    // L2-24: absorbed blur still answers meta honestly.
    assert_obs(
        &format!("big: {}\nmid: big.name\nout: mid.%cause", flat_chain(4000)),
        "#max_depth_exceeded",
    );
}

#[test]
fn red_union_nav_blur_branch_survives() {
    // Today: `1` — blur branch silently culled. Law: only ⊥ branches
    // are culled; the horizon must remain visible in the result.
    let got = observe_nlang(
        &format!("big: {}\nu: {{ a: 1 }} | big\nout: u.a", flat_chain(4000)),
        "out",
    );
    assert!(
        got.contains(" | ") && got.contains("#blur") && got.contains("max_depth_exceeded"),
        "blur branch must survive union navigation: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — #4 %caid meta whitelist
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_caid_meta_navigable() {
    // Today: ⊥ #invalid_path. Expect the snapshot CAID string.
    let got = observe_nlang(&format!("big: {}\nout: big.%caid", flat_chain(4000)), "out");
    assert!(
        got.contains("hash:sha256:"),
        "%caid must read the snapshot CAID string: {got:?}"
    );
}

#[test]
fn red_caid_self_compare_true() {
    // L2-27: snapshot identity is totally decidable — no blur produced.
    assert_obs(
        &format!("big: {}\nout: big.%caid == big.%caid", flat_chain(4000)),
        "#true",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — #6 lattice-eq two-stage
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_eq_blur_vs_value_absorbs() {
    // Migrated from blur_horizon_probe_test::pin_lattice_eq_blur_current_behavior
    // (frozen #false "until the separate case" — this is that case;
    // migration performed by the ACCEPTOR in the order commit).
    assert_blur_horizon(&format!("big: {}\nout: big = 1", flat_chain(4000)));
}

#[test]
fn pin_eq_twin_blurs_same_text_true() {
    // O42 M4: Code node_content is span-free, so two bindings of the same
    // text mint the same CHS (partial CAID). SPEC_08 §3.2.2 #6(a) → #true.
    // Pre-M4 this was red as "absorb" because Debug-of-Expr baked spans into
    // the partial digest and the CAIDs disagreed (a lie about inequality).
    assert_obs(
        &format!(
            "bigA: {c}\nbigB: {c}\nout: bigA = bigB",
            c = flat_chain(4000)
        ),
        "#true",
    );
}

#[test]
fn pin_eq_twin_cause_meta_is_blank() {
    // L2-26: equality is #true, so .%cause has no cause tag.
    assert_obs(
        &format!(
            "bigA: {c}\nbigB: {c}\nout: (bigA = bigB).%cause",
            c = flat_chain(4000)
        ),
        "_",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — healthy faces frozen; excluded scope frozen
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_eq_self_same_snapshot_true() {
    // L2-25: force memo yields the SAME snapshot — #true by determinism.
    // Green today; #6(a) makes it law instead of accident.
    assert_obs(
        &format!("big: {}\nout: big = big", flat_chain(4000)),
        "#true",
    );
}

#[test]
fn pin_eq_normal_values_unaffected() {
    assert_obs("out: { a: 1 } = { a: 1 }", "#true");
}

#[test]
fn pin_eqeq_blur_absorbs_g3_law() {
    // G3 R1 (value context) — must keep holding through this arc.
    assert_blur_horizon(&format!("big: {}\nout: big == 1", flat_chain(4000)));
}

#[test]
fn pin_meet_blur_absorbs() {
    assert_blur_horizon(&format!("big: {}\nout: big & 1", flat_chain(4000)));
}

#[test]
fn pin_join_blur_first_class() {
    let got = observe_nlang(&format!("big: {}\nout: big | 1", flat_chain(4000)), "out");
    assert!(
        got.contains(" | ") && got.contains("#blur"),
        "join must keep blur as a first-class branch: {got:?}"
    );
}

#[test]
fn pin_combo_storage_transparent() {
    assert_blur_horizon(&format!(
        "big: {}\nc: {{ x: big }}\nout: c.x",
        flat_chain(4000)
    ));
}

#[test]
fn pin_cause_meta_unchanged() {
    assert_obs(
        &format!("big: {}\nout: big.%cause", flat_chain(4000)),
        "#max_depth_exceeded",
    );
}

#[test]
fn pin_lt_blur_frozen_conflict() {
    // MIGRATED (2026-07-20, order-wave W3 open): order × blur follows
    // the `=` two-stage law (SYNTAX_06 §4 #13) — different identity
    // absorbs into the horizon, never #false, never #conflict.
    let got = observe_nlang(&format!("big: {}\nout: big < 1", flat_chain(4000)), "out");
    assert!(
        got.starts_with("#blur"),
        "lt × blur absorbs (two-stage law): {got:?}"
    );
}

#[test]
fn pin_lte_blur_frozen_conflict() {
    // MIGRATED (2026-07-20, order-wave W3 open): same two-stage law.
    let got = observe_nlang(&format!("big: {}\nout: big <= 1", flat_chain(4000)), "out");
    assert!(
        got.starts_with("#blur"),
        "lte × blur absorbs (two-stage law): {got:?}"
    );
}

#[test]
fn pin_nav_bottom_passthrough() {
    // ⊥ in coordinate context keeps its cause today — must stay.
    let got = observe_nlang("out: (1 & 2).name", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#conflict"),
        "nav on ⊥ must keep cause: {got:?}"
    );
}

#[test]
fn pin_nav_blur_compositional() {
    // ACCEPTANCE REPAIR PIN (2026-07-14): the delivery's absorb arm bailed
    // out of the segment loop, so inline `big.name.%cause` returned the
    // whole #blur while the binding-split spelling returned the cause tag
    // — navigation compositionality (x.a.b ≡ (x.a).b) broken. Repair:
    // absorption continues the loop; later meta segments still answer.
    assert_obs(
        &format!("big: {}\nout: big.name.%cause", flat_chain(4000)),
        "#max_depth_exceeded",
    );
}

#[test]
fn pin_small_nav_unaffected() {
    assert_obs("c: { name: \"Bob\" }\nout: c.name", "\"Bob\"");
}
