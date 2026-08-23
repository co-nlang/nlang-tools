// Cause-cocoon reconciliation probes (2026-07-19, pre-committed by
// work order — docs/cocoon_shape_handover.md). Queue item "REAL_04
// cocoon 調和", RULED 2026-07-19 (option A + %type archaeology).
//
// MEASURED (v0.2.22):
//   - REAL_04 §1's old shape table (bare message/path/line/... fields)
//     was NEVER minted — engine mints %-prefixed diagnostics. Spec
//     rewritten to codify the engine core (%val + optional %-fields).
//   - `_: _` anti-collapse pad is engine scaffolding but LEAKS into
//     the structural view: <<(1&2).%cause>> prints a `_: _` line.
//   - `%type` is a FOSSIL (user archaeology: old node model stored
//     type content in a %type field; superseded by SPEC_03 §4
//     %kind + %super/%predicate). In cause cocoons %type always
//     twins %val; lib.rs accepts `.%cause`|`.%type` as aliases.
//   - Two-source ⊥×blur spread order dependence ALREADY HEALED by
//     the cause-canon arc (both orders → ⊥ #conflict) — pinned here.
//
// LAW (REAL_04 §1 rewritten 2026-07-19 + SPEC_08 §3.2.2 #4 sync):
//   1. Cocoon core = %val (sole duality core); diagnostics optional,
//      %-prefixed, vary by TAG_REGISTRY class.
//   2. %type ABOLISHED: cocoon field gone; `.%type` read retired —
//      on ⊥ it passes the ⊥ through verbatim (F1 compositionality),
//      on #blur it is absorbed (coordinate absorption #5).
//   3. Engine scaffolding (the `_: _` pad or successor mechanism)
//      MUST NOT appear in any user-visible projection; user-defined
//      fields named `_` are untouched.
// NOT in scope: nominal type machinery's %type field
//   (type_constraint.rs / dispatch.rs store @Name constraint names
//   there — that is the SPEC_02 §1.2 / SPEC_05 §2.1 vs SPEC_03 §4
//   super-conflict, a SEPARATE adjudication case; pinned below);
//   %kind (list/logic) minting; bn_serial mechanics (cause-cocoon
//   CAID shift from field removal = one-time legal diff, ledger).

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("cocoonshape")
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
// RED GATES — scaffolding invisible, %type retired
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_cocoon_structural_no_pad() {
    // Law 3: scaffolding must not appear in the structural view.
    let got = observe_nlang("e: 1 & 2\nout: <<e.%cause>>", "out");
    assert!(
        !got.contains("_: _") && got.contains("%val"),
        "cocoon structural view must hide scaffolding, keep %val: {got:?}"
    );
}

#[test]
fn red_cocoon_structural_no_type_field() {
    // Law 2: the %type twin column is gone from the cocoon.
    let got = observe_nlang("e: 1 & 2\nout: <<e.%cause>>", "out");
    assert!(
        !got.contains("%type"),
        "cocoon must not carry the fossil %type field: {got:?}"
    );
}

#[test]
fn red_divergent_cocoon_clean() {
    // Second mint site (divergent family): no %type, no pad.
    let got = observe_nlang("m: {a: m.a + 1}\nout: <<(m.a).%cause>>", "out");
    assert!(
        !got.contains("%type") && !got.contains("_: _") && got.contains("%val"),
        "divergent cocoon must be core-clean: {got:?}"
    );
}

#[test]
fn red_static_cycle_cocoon_clean() {
    // Third mint site (static-cycle cause combo): %members stays law,
    // %type/pad go.
    let got = observe_nlang("p: {v: p.v}\nout: <<(p.v).%cause>>", "out");
    assert!(
        !got.contains("%type") && !got.contains("_: _") && got.contains("%members"),
        "static-cycle cocoon must keep %members, drop fossil/pad: {got:?}"
    );
}

#[test]
fn red_bottom_type_seg_passthrough() {
    // Law 2: `.%type` on ⊥ is no meta read — ⊥ passes through verbatim.
    let got = observe_nlang("e: 1 & 2\nout: e.%type", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#conflict"),
        "⊥.%type must pass the ⊥ through (F1), not read a tag: {got:?}"
    );
}

#[test]
fn red_blur_type_seg_absorbed() {
    // Law 2 + SPEC_08 #5: `.%type` on #blur is absorbed like any
    // ordinary segment (today: alias returns the BlurCause tag).
    let got = observe_nlang(&format!("big: {}\nout: big.%type", flat_chain(4000)), "out");
    assert!(
        got.starts_with("#blur"),
        "blur.%type must absorb per coordinate absorption: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — duality core, adjacent law, fences
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_cause_direct_collapses_to_tag() {
    assert_obs("e: 1 & 2\nout: e.%cause", "#conflict");
}

#[test]
fn pin_cause_val_core_readable() {
    assert_obs("e: 1 & 2\nout: (e.%cause).%val", "#conflict");
}

#[test]
fn pin_cause_message_readable() {
    let got = observe_nlang("e: 1 & 2\nout: (e.%cause).%message", "out");
    assert!(
        got.contains("Incompatible"),
        "%message diagnostic must stay readable: {got:?}"
    );
}

#[test]
fn pin_blur_cause_tag_whitelist() {
    // SPEC_08 #4: %cause on blur still reads the BlurCause tag.
    let got = observe_nlang(
        &format!("big: {}\nout: big.%cause", flat_chain(4000)),
        "out",
    );
    assert_eq!(got, "#max_depth_exceeded");
}

#[test]
fn pin_user_underscore_field_survives() {
    // Law 3 boundary: a USER field named `_` is data, not scaffolding.
    let got = observe_nlang("w: {_: 5}\nout: w", "out");
    assert!(
        got.contains("_: 5"),
        "user-defined `_` field must survive display: {got:?}"
    );
}

#[test]
fn pin_spread_bottom_blur_order_free() {
    // HEALED face on record (cause-canon arc): ⊥ beats blur in BOTH
    // spread orders — lattice bottom is order-free.
    let big = flat_chain(4000);
    let a = observe_nlang(
        &format!("big: {big}\nbot: 1 & 2\nout: {{...bot, ...big}}"),
        "out",
    );
    let b = observe_nlang(
        &format!("big: {big}\nbot: 1 & 2\nout: {{...big, ...bot}}"),
        "out",
    );
    assert!(a.starts_with("_|_") && a.contains("#conflict"), "{a:?}");
    assert!(b.starts_with("_|_") && b.contains("#conflict"), "{b:?}");
}

#[test]
fn pin_nominal_type_machinery_untouched() {
    // FENCE: type_constraint/dispatch %type storage (the SPEC_02 §1.2
    // super-conflict, separate case) must not be dragged into this arc.
    assert_obs("@Pos: 1..\nx: 5 & @Pos\nout: x", "5");
    assert_obs("@Pos: 1..\nx: 0 & @Pos\nout: (x).%cause", "#conflict");
}

#[test]
fn pin_missing_key_cocoon_class() {
    // Cause classes vary their field sets — missing_key still reads.
    assert_obs("cc: {{a: 1}}\nout: (cc.b).%cause", "#missing_key");
}
