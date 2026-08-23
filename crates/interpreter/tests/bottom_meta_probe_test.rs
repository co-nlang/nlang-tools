// Bottom-meta rectification probes (2026-07-14, pre-committed by work
// order — docs/bottom_meta_handover.md).
//
// RULING (approved 2026-07-14; engine-follows-law, all sources existing):
//   F1 ⊥ coordinate navigation is compositional (x.a.b ≡ (x.a).b) — the
//      Bottom arm bailed out of the segment loop exactly like the Blur
//      repair's bug (inline meta reads returned the whole ⊥).
//   F2 %cause is a Cocoon with %val (REAL_04 §1): direct observation
//      collapses to the tag; <<path>> keeps the causal chain. Engine
//      returns a raw diagnostic combo (has %type, LACKS %val) — adding
//      %val lets the G6 value-context projection do the rest for free.
//   F3 never-collapsed nodes have no cause to report → `_` (SYNTAX_08
//      §4 #2: the query must not mint a fresh conflict). Combos already
//      open-miss; only atoms fell into the poisoned catch-all.
//   F4 #invalid_path ABOLISHED — never legislated (absent from REAL_04
//      taxonomy and TAG_REGISTRY; G4's ruling text copied the engine's
//      spelling). Mint sites redirect:
//        nav catch-all (atom/Top)  → `_` (open world: an atom's data
//          axis can grow fields via `&` hybridization — "definitely
//          absent" is an overclaim);
//        ^ overflow                → ⊥ #out_of_horizon (canonical tag,
//          TAG_REGISTRY §1 — honest even while ^ resolution in
//          observation contexts remains unwired, separate case);
//        union all-⊥ survivors     → primary cause per REAL_04 §4.
//      G4 clause revised: `({a:1}|7).a` = `1 | _` (honest superposition,
//      same rule as the kept Top-miss branch).
// BottomCause enum: variants are APPEND-ONLY (fmt v2 freeze) —
// InvalidPath stays readable for stored universes, minting stops.

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("botmeta")
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
            let mut universe = Universe::new_with_standard(
                None,
                engine.root_with_system(),
                engine.root_with_system(),
            );
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
// RED GATES — F1 compositionality (inline ≡ binding-split)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_bottom_nav_compositional_type() {
    // L2-28. cocoon_shape 2026-07-19: %type alias retired — non-meta on
    // ⊥ passes the ⊥ through (F1); still compositional with intermediate
    // open-miss segments.
    let got = observe_nlang("bad: 1 & 2\nout: bad.name.%type", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#conflict"),
        "⊥.%type must pass the ⊥ through: {got:?}"
    );
}

#[test]
fn red_bottom_nav_compositional_cause() {
    // F1 + F2 combined: passthrough, then cocoon collapse.
    assert_obs("bad: 1 & 2\nout: bad.name.%cause", "#conflict");
}

#[test]
fn red_divergent_nav_compositional() {
    // %type retired: compositional nav still carries the ⊥ #divergent.
    let got = observe_nlang("a: a + 1\nout: a.name.%type", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#divergent"),
        "divergent ⊥.%type must pass through: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — F2 %cause duality (cocoon %val)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_cause_collapses_to_tag() {
    // L2-29. Today: raw diagnostic combo.
    assert_obs("bad: 1 & 2\nout: bad.%cause", "#conflict");
}

#[test]
fn red_cause_structural_keeps_cocoon() {
    // Duality face: <<path>> keeps the chain — and the cocoon now carries
    // its %val duality core.
    let got = observe_nlang("bad: 1 & 2\nout: <<bad.%cause>>", "out");
    assert!(
        got.contains("%val") && got.contains("#conflict"),
        "structural cause must be a cocoon with %val: {got:?}"
    );
}

#[test]
fn red_cause_val_navigable() {
    // Today: `_` (no %val field on the diagnostic combo).
    assert_obs("bad: 1 & 2\nm: bad.%cause\nout: m.%val", "#conflict");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — F3 no cause to report → open
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_no_cause_open_atom() {
    // L2-30. Today: ⊥ #invalid_path — the query minting a fresh conflict,
    // exactly what SYNTAX_08 §4 #2 forbids.
    assert_obs("b: 123\nout: b.%cause", "_");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — F4 #invalid_path abolition
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_atom_nav_open() {
    // L2-31.
    assert_obs("out: (7).a", "_");
}

#[test]
fn red_atom_nav_deep_open() {
    // Compositional openness: Top stays Top through further segments.
    assert_obs("out: (7).a.b", "_");
}

#[test]
fn red_top_nav_open() {
    assert_obs("c: { a: 1 }\nout: c.nope.deeper", "_");
}

#[test]
fn red_union_atoms_nav_open() {
    // Successor of G4's pin_union_nav_all_bottom_is_invalid_path
    // (migrated by the ACCEPTOR): atom branches open-miss, kept →
    // normalize of two `_` = `_`. No empty-survivor mint reached.
    assert_obs("out: (1 | 2).a", "_");
}

#[test]
fn red_union_atom_branch_open_miss() {
    // MIGRATED (2026-07-20, union_absorption): Top-family collapse —
    // open-miss `_` is lattice Top and absorbs the sibling (`1 | _` → `_`).
    // Same voice as Join `9 | _` → `_` (SPEC_01 §2.4.2).
    // MIGRATED-2 (2026-07-20, caused_top ruling C): the open-miss /
    // static-cycle Top is a CAUSED Top = diagnostic member — exempt from
    // absorption (SPEC_01 §2.4.2). Bare `_` still absorbs.
    assert_obs("out: ({ a: 1 } | 7).a", "1 | _");
}

#[test]
fn red_parent_overflow_out_of_horizon() {
    // Canonical tag on ^ depth overflow (TAG_REGISTRY §1). Valid parent
    // shapes resolve on the sealed container chain (caret ascent arc);
    // only true overshoot (past root) lands here.
    let got = observe_nlang("out: ^^^.x", "out");
    assert!(
        got.starts_with("_|_") && got.contains("out_of_horizon"),
        "^ overflow must be #out_of_horizon: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — unchanged laws and frozen adjacent scope
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_combo_open_miss() {
    assert_obs("out: ({ a: 1 }).b", "_");
}

#[test]
fn pin_bottom_nonmeta_tail_passthrough() {
    // Non-meta tail on ⊥: passthrough, cause preserved (observably
    // unchanged by the loop-continue fix).
    let got = observe_nlang("bad: 1 & 2\nout: bad.name.foo", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#conflict"),
        "bottom passthrough must keep cause: {got:?}"
    );
}

#[test]
fn pin_bottom_type_direct() {
    // cocoon_shape: %type is not a meta read — ⊥ passes through verbatim.
    let got = observe_nlang("bad: 1 & 2\nout: bad.%type", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#conflict"),
        "⊥.%type must pass the ⊥ through: {got:?}"
    );
}

#[test]
fn pin_bottom_display_unchanged() {
    let got = observe_nlang("bad: 1 & 2\nout: bad", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#conflict"),
        "bottom display form must not change: {got:?}"
    );
}

#[test]
fn pin_union_bottom_build_dropped() {
    // Build-time normalize already culls ⊥ branches (cause-preserving
    // when empty) — untouched by this order.
    assert_obs("out: (1 & 2) | 5", "5");
}

#[test]
fn pin_blur_nav_absorb_unchanged() {
    let got = observe_nlang(&format!("big: {}\nout: big.name", flat_chain(4000)), "out");
    assert!(
        got.starts_with("#blur") && got.contains("max_depth_exceeded"),
        "blur nav absorption must survive this arc: {got:?}"
    );
}

#[test]
fn pin_blur_cause_tag_unchanged() {
    assert_obs(
        &format!("big: {}\nout: big.%cause", flat_chain(4000)),
        "#max_depth_exceeded",
    );
}

#[test]
fn pin_selfref_stays_divergent() {
    // L2-17.
    let got = observe_nlang("a: a + 1\nout: a", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#divergent"),
        "L2-17 regressed: {got:?}"
    );
}

#[test]
fn pin_hybrid_nav_works() {
    // The open-atom rationale: atoms CAN grow data fields via `&`.
    assert_obs("h: 3 & { note: \"n\" }\nout: h.note", "\"n\"");
}

#[test]
fn pin_union_commutativity_eq() {
    // ACCEPTANCE REPAIR PIN (2026-07-14): the delivery dropped the
    // build-time tropical sort to satisfy encounter-order gates, which
    // silently broke `=` commutativity ((1|2) = (2|1) → #false) — Union
    // PartialEq was Vec-order-sensitive. Repair: multiset branch equality
    // (SPEC_01 `|` commutative; G1 集合觀). Display keeps encounter order.
    assert_obs("out: (1 | 2) = (2 | 1)", "#true");
}

#[test]
fn pin_union_commutativity_combo_eq() {
    assert_obs(
        "out: ({ a: 1 } | { b: 2 }) = ({ b: 2 } | { a: 1 })",
        "#true",
    );
}

#[test]
fn pin_union_display_encounter_order() {
    // SPEC_01 §2.4.1 (2026-07-18): display is canonical sorted spelling,
    // not encounter order. (Former freeze: "canonical display question
    // ledgered separately" — that case closed by display_order arc.)
    assert_obs("out: 2 | 1", "1 | 2");
}

// pin_private_axis_current_behavior MIGRATED 2026-07-15 by the ACCEPTOR:
// its freeze clause read "separate case" — that case is the private-axis
// enforcement arc (SPEC_04 §3.1). Successor red gate:
// private_axis_probe_test::red_outward_dotted_blocked.
