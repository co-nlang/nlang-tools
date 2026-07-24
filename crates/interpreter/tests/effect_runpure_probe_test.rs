// ~%Effect./runPure + privilege probes (2026-07-24, pre-committed by work
// order — docs/effect_runpure_handover.md). 效應系統波 arc 4 (§4.3 / §6).
//
// RULING P1 (2026-07-24, user — trusted-channel capability): privileged
// mode is a CAPABILITY on the horizon (EvalContext.privileged), set ONLY
// via a trusted out-of-program channel (CLI `oo run --privileged` / init).
// A normal n/ program CANNOT self-authorize (SPEC_08 §6.1.2 — no implicit
// tokenless backdoor). The token STRING's minting/lifecycle is REAL_02
// (protocol), not the language — the language sees a boolean capability.
//
// ~%Effect./runPure <node> (SPEC_08 §4.3 = the #effect_override privileged
// op, §6.2): with privilege, force the node and solidify its %effect to
// #pure (the io is externally proxied → lawful pure data); WITHOUT
// privilege, collapse to ⊥ (%cause: #privileged_required). Observation
// projection only — the original node's CAID is unchanged (§4.3 / §6.2:
// privilege changes the convergence process, not the geometric fingerprint).
//
// This file = the NON-privileged path (default harness is unprivileged).
// The privileged path is tested via the CLI trusted channel in
// crates/oo/tests/runpure_cli_probe_test.rs (--privileged flag) — the
// faithful test of P1, since privilege cannot be established in-program.
//
// MEASURED (baseline, v0.2.36 dev): `~%Effect` is unregistered, so
// `~%Effect./runPure (io)` navigates to open-miss `_` (not ⊥). Normal io
// flows #io (unaffected).
//
// NOT in scope (ledgered follow-on): #pin + the other §6 privileged ops
// (#commit/#rollback/#squash — same capability infra, separate arcs),
// commit-level audit transparency (§6.1.3 for non-⊥ results), token-string
// validation (REAL_02), physical-thread isolation (§6.1.1), #ext: (§4.1).

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
        "nlang-runpure-{}-{}",
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&d).ok();
    fs::create_dir_all(&d).unwrap();
    d
}

/// 64 MiB thread. Default (UNPRIVILEGED) engine — the ambient program horizon.
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
    assert_eq!(observe_nlang(src, "out"), expect, "{src:?} :: out");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — runPure without privilege is blocked (§4.3 / §6.1.2)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_runpure_blocked_unprivileged() {
    // An un-privileged program calling runPure is refused (no backdoor).
    let got = observe_nlang("out: ~%Effect./runPure (~%Time.now _)", "out");
    assert!(
        got.starts_with("_|_") && got.contains("privileged_required"),
        "unprivileged runPure ⟹ ⊥ #privileged_required, got: {got:?}"
    );
}

#[test]
fn red_runpure_read_propagates_bottom() {
    // Reading .%effect on the refused runPure passes the ⊥ through.
    let got = observe_nlang("out: (~%Effect./runPure (~%Time.now _)).%effect", "out");
    assert!(
        got.starts_with("_|_") && got.contains("privileged_required"),
        "unprivileged runPure.%effect ⟹ ⊥, got: {got:?}"
    );
}

#[test]
fn red_runpure_pure_arg_blocked_unprivileged() {
    // Even a pure argument is refused without privilege — the GATE is the
    // capability, not the argument's effect. (Discharge is a privileged op.)
    let got = observe_nlang("out: ~%Effect./runPure 42", "out");
    assert!(
        got.starts_with("_|_") && got.contains("privileged_required"),
        "unprivileged runPure on pure arg still ⊥, got: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — the ambient horizon stays unprivileged & effects untouched
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_normal_io_flows_unaffected() {
    // Plain io in the default horizon is tracked, not discharged — runPure
    // is the ONLY discharge path (arc-1/2/3 hold).
    assert_obs("out: (~%Time.now _).%effect", "#io");
    assert_obs("out: { v: (~%Time.now _) }.%effect", "#io");
}

#[test]
fn pin_multi_active_unaffected() {
    assert_obs(
        "out: { a: (~%Time.now _), b: (~%Math./random _) }.%effect",
        "#io | #nondet",
    );
}

#[test]
fn pin_bottom_meta_whitelist_unchanged() {
    let got = observe_nlang("bot: 1 & 2\nout: bot.%effect", "out");
    assert!(
        got.starts_with("_|_") && got.contains("#conflict"),
        "⊥ conflict passes through: {got:?}"
    );
}
