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
use nlang_parser::parse_program;
use nlang_parser::ast::{Path, PathAnchor, Span};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!(
        "nlang-blurbnd-{}-{}",
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

/// Horizon verdict: #blur form with fuel-exhaustion cause. CAID is salted
/// per engine instance — only form and cause are normative.
fn assert_blur_fuel(src: &str) {
    let got = observe_nlang(src, "out");
    assert!(
        got.starts_with("#blur"),
        "{src:?} :: out — expected #blur horizon, got {got:?}"
    );
    assert!(
        got.contains("fuel_exhausted"),
        "{src:?} :: out — expected fuel_exhausted cause, got {got:?}"
    );
}

fn flat_chain(n: usize) -> String {
    vec!["1"; n].join(" + ")
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — #5 coordinate absorption
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "blur boundary red gate: awaiting SPEC_08 3.2.2 #5/#6 + %caid"]
fn red_nav_blur_absorbs() {
    // Today: ⊥ #invalid_path — the "path is invalid" lie.
    assert_blur_fuel(&format!("big: {}\nout: big.name", flat_chain(4000)));
}

#[test]
#[ignore = "blur boundary red gate: awaiting SPEC_08 3.2.2 #5/#6 + %caid"]
fn red_nav_through_combo_absorbs() {
    assert_blur_fuel(&format!(
        "big: {}\nc: {{ x: big }}\nout: c.x.name",
        flat_chain(4000)
    ));
}

#[test]
#[ignore = "blur boundary red gate: awaiting SPEC_08 3.2.2 #5/#6 + %caid"]
fn red_nav_blur_cause_after_absorb() {
    // L2-24: absorbed blur still answers meta honestly.
    assert_obs(
        &format!("big: {}\nmid: big.name\nout: mid.%cause", flat_chain(4000)),
        "#fuel_exhausted",
    );
}

#[test]
#[ignore = "blur boundary red gate: awaiting SPEC_08 3.2.2 #5/#6 + %caid"]
fn red_union_nav_blur_branch_survives() {
    // Today: `1` — blur branch silently culled. Law: only ⊥ branches
    // are culled; the horizon must remain visible in the result.
    let got = observe_nlang(
        &format!("big: {}\nu: {{ a: 1 }} | big\nout: u.a", flat_chain(4000)),
        "out",
    );
    assert!(
        got.contains(" | ") && got.contains("#blur") && got.contains("fuel_exhausted"),
        "blur branch must survive union navigation: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — #4 %caid meta whitelist
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "blur boundary red gate: awaiting SPEC_08 3.2.2 #5/#6 + %caid"]
fn red_caid_meta_navigable() {
    // Today: ⊥ #invalid_path. Expect the snapshot CAID string.
    let got = observe_nlang(&format!("big: {}\nout: big.%caid", flat_chain(4000)), "out");
    assert!(
        got.contains("hash:sha256:"),
        "%caid must read the snapshot CAID string: {got:?}"
    );
}

#[test]
#[ignore = "blur boundary red gate: awaiting SPEC_08 3.2.2 #5/#6 + %caid"]
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
#[ignore = "blur boundary red gate: awaiting SPEC_08 3.2.2 #5/#6 + %caid"]
fn red_eq_blur_vs_value_absorbs() {
    // Migrated from blur_horizon_probe_test::pin_lattice_eq_blur_current_behavior
    // (frozen #false "until the separate case" — this is that case;
    // migration performed by the ACCEPTOR in the order commit).
    assert_blur_fuel(&format!("big: {}\nout: big = 1", flat_chain(4000)));
}

#[test]
#[ignore = "blur boundary red gate: awaiting SPEC_08 3.2.2 #5/#6 + %caid"]
fn red_eq_twin_blurs_absorb() {
    // Two bindings, same text: snapshots differ (CAIDs differ) but both
    // denote 4000 — today's #false claims certain inequality, a lie.
    assert_blur_fuel(&format!(
        "bigA: {c}\nbigB: {c}\nout: bigA = bigB",
        c = flat_chain(4000)
    ));
}

#[test]
#[ignore = "blur boundary red gate: awaiting SPEC_08 3.2.2 #5/#6 + %caid"]
fn red_eq_twin_cause_meta() {
    // L2-26 mirror.
    assert_obs(
        &format!(
            "bigA: {c}\nbigB: {c}\nout: (bigA = bigB).%cause",
            c = flat_chain(4000)
        ),
        "#fuel_exhausted",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — healthy faces frozen; excluded scope frozen
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_eq_self_same_snapshot_true() {
    // L2-25: force memo yields the SAME snapshot — #true by determinism.
    // Green today; #6(a) makes it law instead of accident.
    assert_obs(&format!("big: {}\nout: big = big", flat_chain(4000)), "#true");
}

#[test]
fn pin_eq_normal_values_unaffected() {
    assert_obs("out: { a: 1 } = { a: 1 }", "#true");
}

#[test]
fn pin_eqeq_blur_absorbs_g3_law() {
    // G3 R1 (value context) — must keep holding through this arc.
    assert_blur_fuel(&format!("big: {}\nout: big == 1", flat_chain(4000)));
}

#[test]
fn pin_meet_blur_absorbs() {
    assert_blur_fuel(&format!("big: {}\nout: big & 1", flat_chain(4000)));
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
    assert_blur_fuel(&format!("big: {}\nc: {{ x: big }}\nout: c.x", flat_chain(4000)));
}

#[test]
fn pin_cause_meta_unchanged() {
    assert_obs(
        &format!("big: {}\nout: big.%cause", flat_chain(4000)),
        "#fuel_exhausted",
    );
}

#[test]
fn pin_lt_blur_frozen_conflict() {
    // EXCLUDED scope (§4.10): order judgment unimplemented on non-atoms
    // globally; frozen at today's verdict so this arc doesn't drive-by it.
    let got = observe_nlang(&format!("big: {}\nout: big < 1", flat_chain(4000)), "out");
    assert!(
        got.starts_with("_|_") && got.contains("#conflict"),
        "lt × blur is frozen #conflict (§4.10 case): {got:?}"
    );
}

#[test]
fn pin_lte_blur_frozen_conflict() {
    let got = observe_nlang(&format!("big: {}\nout: big <= 1", flat_chain(4000)), "out");
    assert!(
        got.starts_with("_|_") && got.contains("#conflict"),
        "lte × blur is frozen #conflict (§4.10 case): {got:?}"
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
fn pin_small_nav_unaffected() {
    assert_obs("c: { name: \"Bob\" }\nout: c.name", "\"Bob\"");
}
