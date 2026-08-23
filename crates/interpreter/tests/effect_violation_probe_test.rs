// Static guard / #effect_violation probes (2026-07-24, pre-committed by
// work order — docs/effect_violation_handover.md). 效應系統波 arc 3 (§4.3).
//
// RULING (2026-07-24, user — Model A "declared purity contradicted"):
// the "pure context" the static guard protects is an EXPLICIT
// `%effect: #pure` declaration (SYNTAX_08 writable meta — the only
// purity-assertion mechanism). When that declaration is CONTRADICTED by
// the value's actual active contagion effect (#io/#nondet/#state), the
// promise is a lie and the value collapses to ⊥ (%cause:
// #effect_violation) — "formalization is a type system that crashes when
// you lie." NOT the ambient default context (that would collapse every
// io value and shatter L2-83 + arc-1/2). Opt-in, non-breaking.
//
// COCOON is the escape valve, AUTOMATICALLY exempt: a cocoon `{{ }}`
// genuinely shields (closed combos skip effect accumulation, §4.2.1), so
// its actual c.effect IS #pure — declared #pure MATCHES actual #pure, no
// contradiction, no violation. (runPure §4.3 = the other, privileged
// discharge — ledgered follow-on.)
//
// MEASURED (baseline, v0.2.35 dev): the engine already exposes the
// contradiction but silently lets the spoof win —
//   { %effect: #pure, v: io }.%effect     → #pure  ;; %effect: #io
//   { %effect: #pure, v: nondet }.%effect → #pure  ;; %effect: #nondet
//   { %effect: #pure, v: state }.%effect  → #pure  ;; %effect: #state
// The `.%effect` read returns the DECLARED #pure; the tail betrays the
// ACTUAL #io. Guard turns the lie into ⊥ #effect_violation.
//
// The guard fires on ACTIVE tags (io/nondet/state) only. #cached and
// #pure never trigger it. Under-declaration (declared #io over actual
// io|nondet) is a DIFFERENT lie, NOT this arc — ledgered.
//
// NOT in scope (ledgered follow-on): ~%Effect./runPure + %privilege_token
// (§4.3), #ext: tags (§4.1), full tag-set CAID participation (§4.1),
// under-declaration.

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("effviol")
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

fn assert_violation(src: &str) {
    let got = observe_nlang(src, "out");
    assert!(
        got.starts_with("_|_") && got.contains("effect_violation"),
        "expected ⊥ #effect_violation, got: {got:?} :: {src:?}"
    );
}

// io = ~%Time.now _ , nondet = ~%Math./random _ , state = ~%Engine./equivalence_map _

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — declared #pure contradicted by active effect ⟹ ⊥ (§4.3)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_violation_io() {
    // Declaring #pure over an io field is a false promise → ⊥.
    assert_violation("out: { %effect: #pure, v: (~%Time.now _) }");
}

#[test]
fn red_violation_nondet() {
    assert_violation("out: { %effect: #pure, v: (~%Math./random _) }");
}

#[test]
fn red_violation_state() {
    assert_violation("out: { %effect: #pure, v: (~%Engine./equivalence_map _) }");
}

#[test]
fn red_violation_nested() {
    // Grandchild contagion propagates up — a deep io still contradicts the
    // outer #pure declaration.
    assert_violation("out: { %effect: #pure, v: { w: (~%Time.now _) } }");
}

#[test]
fn red_violation_propagates_through_effect_read() {
    // The whole value is ⊥, so reading .%effect on it passes the ⊥ through
    // (was: #pure — the silent lie).
    assert_violation("c: { %effect: #pure, v: (~%Time.now _) }\nout: c.%effect");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — the guard must be opt-in, cocoon-exempt, and never over-fire
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_declared_pure_over_pure_ok() {
    // A TRUE #pure declaration (pure content) is not a lie → #pure.
    assert_obs("out: { %effect: #pure, v: 42 }.%effect", "#pure");
}

#[test]
fn pin_cocoon_shield_is_the_escape() {
    // §4.2.1: a cocoon genuinely seals io, so its actual effect IS #pure —
    // declared #pure matches, NO violation. This is the legitimate way to
    // hold io behind a pure boundary.
    assert_obs(
        "out: {{ %effect: #pure, v: (~%Time.now _) }}.%effect",
        "#pure",
    );
}

#[test]
fn pin_declared_io_is_honest() {
    // Declaring #io over io is truthful (effect_meta spoof pin) — the guard
    // only fires on a #pure declaration, never on an active one, so this is
    // NOT collapsed. (The spoof read carries a #io tail — pre-existing
    // effect_meta behavior; the point here is: no #effect_violation.)
    let got = observe_nlang("out: { %effect: #io, v: (~%Time.now _) }.%effect", "out");
    assert!(
        !got.starts_with("_|_") && got.contains("#io"),
        "declared #io is honest, not a violation: {got:?}"
    );
}

#[test]
fn pin_undeclared_io_flows() {
    // No purity declaration → no guard. Un-annotated io is tracked, not
    // blocked (L2-83 / arc-1/2 hold).
    assert_obs("out: { v: (~%Time.now _) }.%effect", "#io");
    assert_obs("out: (~%Time.now _).%effect", "#io");
}

#[test]
fn pin_undeclared_multi_active_flows() {
    // arc-1 set-union on un-declared values is untouched.
    assert_obs(
        "out: { a: (~%Time.now _), b: (~%Math./random _) }.%effect",
        "#io | #nondet",
    );
}

#[test]
fn pin_bottom_meta_whitelist_unchanged() {
    // Pre-existing ⊥ (conflict) still passes through unchanged.
    let got = observe_nlang("bot: 1 & 2\nout: bot.%effect", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#conflict"),
        "⊥ conflict passes through: {got:?}"
    );
}
