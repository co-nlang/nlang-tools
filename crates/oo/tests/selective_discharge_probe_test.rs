// Selective discharge — privilege as a capability LATTICE (2026-07-25,
// pre-committed by work order: docs/selective_discharge_handover.md).
// 效應系統波後續弧 (SPEC_08 §4.3 / §6.2).
//
// RULING Q1 (2026-07-25, user — 兩軸複合): the privilege capability is a
// STRUCTURED value, not a boolean. Its fields are the five §6.2 privileged
// operations; `effect_override` carries an EFFECT-TAG SET (which active
// tags this horizon may discharge). The other four (`pin`/`commit`/
// `rollback`/`squash`) are DECLARED BUT INERT slots — accepted and stored,
// but no operation consumes them yet (their arcs fill the slots later).
// Rationale: axis 1 = WHICH privileged operation, axis 2 = WHICH effect
// tags inside `#effect_override`. Building axis 2 alone would force a
// reshape of the capability when #pin lands.
//
// RULING Q2 (2026-07-25, user — 全有全無): with capability set C and the
// value's forced active effects E, `runPure` discharges only when C ⊇ E.
// If E ⊄ C → `_|_ (%cause: #privileged_required)`. NO partial discharge:
// a morphism named runPure must never return a non-pure value (naming
// honesty; continues arc-3's "說謊即崩潰" discipline).
//
// The GATE is the capability, not the argument (arc-4 established): with
// no `effect_override` grant, even a PURE argument is refused — discharge
// is a privileged OPERATION.
//
// CLI (REAL_02 trusted channel — P1 says privilege can never be
// established in-program, so the CLI is the faithful instrument):
//   --privileged            grant ALL (v0.2.37 back-compat, unchanged)
//   --grant <SPEC>          repeatable; grants ACCUMULATE BY UNION
//     SPEC ::= effect_override[:<tag>[+<tag>]*]   (bare = all active tags)
//            | pin | commit | rollback | squash   (inert slots)
//   unknown SPEC → loud CLI error (never a silent ignore)
//
// MEASURED (baseline, v0.2.37): `--grant` is an unknown flag — clap emits
// "unexpected argument '--grant' found", so every RED fails for the right
// reason. All PINS were measured green on v0.2.37 before commit.
//
// Effect sources used (measured on v0.2.37):
//   #io              (~%Time.now _)
//   #nondet          (~%Math./random _)
//   #io | #nondet    { a: (~%Time.now _), b: (~%Math./random _) }
// (#state has no directly-forceable n/ spelling today — ~%Engine./set_strategy
// expects an unwrapped combo while apply wraps bare args as {0: …}; that is a
// pre-existing wart, ledgered, NOT this arc's business. io+nondet exercise the
// capability lattice completely.)

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

/// Writes `src` into a fresh temp dir, runs `oo run a.n <args…> --observe out`,
/// returns trimmed stdout+stderr.
fn run_cli(src: &str, args: &[&str]) -> String {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "nlang-seldis-{}-{}",
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&dir).ok();
    fs::create_dir_all(&dir).unwrap();
    let p: PathBuf = dir.join("a.n");
    fs::write(&p, src).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_oo"));
    cmd.arg("run").arg(&p);
    for a in args {
        cmd.arg(a);
    }
    cmd.arg("--observe").arg("out").current_dir(&dir);
    let out = cmd.output().unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    )
}

const IO: &str = "out: (~%Effect./runPure (~%Time.now _)).%effect";
const NONDET: &str = "out: (~%Effect./runPure (~%Math./random _)).%effect";
const MIXED: &str = "out: (~%Effect./runPure \
                     { a: (~%Time.now _), b: (~%Math./random _) }).%effect";
const PURE_ARG: &str = "out: ~%Effect./runPure 42";
const PLAIN_IO: &str = "out: (~%Time.now _).%effect";

fn is_refused(got: &str) -> bool {
    got.contains("privileged_required")
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — the capability lattice (Q1 axis 2: which tags)
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn red_grant_io_discharges_io() {
    // C = {io} ⊇ E = {io} → discharged.
    assert_eq!(run_cli(IO, &["--grant", "effect_override:io"]), "#pure");
}

#[test]
#[ignore]
fn red_grant_io_refuses_nondet() {
    // C = {io} ⊉ E = {nondet} → refused. The grant is per-tag, not blanket.
    let got = run_cli(NONDET, &["--grant", "effect_override:io"]);
    assert!(
        is_refused(&got),
        "C={{io}} must not discharge #nondet: {got:?}"
    );
}

#[test]
#[ignore]
fn red_grant_io_refuses_mixed_partial_coverage() {
    // RULING Q2, the load-bearing case: C = {io}, E = {io, nondet}.
    // Coverage is PARTIAL → all-or-nothing ⟹ ⊥. (Under a partial-discharge
    // design this would instead yield a value still carrying #nondet; the
    // ruling rejected that — runPure must never return non-pure.)
    let got = run_cli(MIXED, &["--grant", "effect_override:io"]);
    assert!(
        is_refused(&got),
        "partial coverage ⟹ ⊥ #privileged_required (no partial discharge): {got:?}"
    );
}

#[test]
#[ignore]
fn red_grant_both_discharges_mixed() {
    // C = {io, nondet} ⊇ E = {io, nondet} → discharged. `+` joins tags.
    assert_eq!(
        run_cli(MIXED, &["--grant", "effect_override:io+nondet"]),
        "#pure"
    );
}

#[test]
#[ignore]
fn red_grant_accumulates_by_repetition() {
    // Repetition accumulates by UNION — the lattice-natural reading of a
    // repeated capability flag.
    assert_eq!(
        run_cli(
            MIXED,
            &[
                "--grant",
                "effect_override:io",
                "--grant",
                "effect_override:nondet"
            ]
        ),
        "#pure"
    );
}

#[test]
#[ignore]
fn red_grant_bare_effect_override_covers_all_active() {
    // Bare `effect_override` (no `:tags`) = all active tags.
    assert_eq!(run_cli(MIXED, &["--grant", "effect_override"]), "#pure");
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — the operation axis (Q1 axis 1: which §6.2 operation)
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn red_pin_grant_does_not_authorize_effect_override() {
    // THE axis-1 test: `pin` is a DIFFERENT §6.2 operation. Granting it must
    // not authorize discharge — and per arc-4 the gate is the operation, not
    // the argument, so even a PURE argument is refused.
    let got = run_cli(PURE_ARG, &["--grant", "pin"]);
    assert!(
        is_refused(&got),
        "granting #pin must not authorize #effect_override: {got:?}"
    );
}

#[test]
#[ignore]
fn red_inert_slot_is_accepted_and_harmless() {
    // The four unimplemented slots are ACCEPTED and stored (forward
    // compatibility), and change nothing observable today.
    assert_eq!(run_cli(PLAIN_IO, &["--grant", "squash"]), "#io");
}

#[test]
#[ignore]
fn red_unknown_grant_is_a_loud_error() {
    // An unknown grant spec must die loudly, never be silently ignored
    // (the ~%Config lesson: a closed knob-family rejects unknown names).
    // NOTE: at baseline clap says "unexpected argument '--grant'", which is
    // an error but NOT this one — the assertion pins the specific wording so
    // the red cannot pass vacuously.
    let got = run_cli(PLAIN_IO, &["--grant", "bogus_capability"]);
    assert!(
        got.contains("bogus_capability"),
        "unknown grant must be named in the error: {got:?}"
    );
    assert!(
        got.to_lowercase().contains("grant") && !got.contains("unexpected argument"),
        "unknown grant ⟹ a grant-specific error, not clap's unknown-flag error: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — v0.2.37 behaviour must survive unchanged (back-compat)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_bare_privileged_still_grants_all() {
    // `--privileged` keeps meaning "grant everything" — the v0.2.37 contract.
    assert_eq!(run_cli(MIXED, &["--privileged"]), "#pure");
}

#[test]
fn pin_bare_privileged_pure_arg_returns_value() {
    // Discharged pure data stays clean (bare integer, no effect tail).
    assert_eq!(run_cli(PURE_ARG, &["--privileged"]), "42");
}

#[test]
fn pin_no_capability_refuses_even_pure_arg() {
    // arc-4 invariant: the capability is the gate, not the argument.
    let got = run_cli(PURE_ARG, &[]);
    assert!(is_refused(&got), "no capability ⟹ ⊥ even for a pure arg: {got:?}");
}

#[test]
fn pin_no_flag_plain_io_flows() {
    // An ordinary unprivileged run tracks io exactly as before.
    assert_eq!(run_cli(PLAIN_IO, &[]), "#io");
}

#[test]
fn pin_privileged_plain_io_is_opt_in() {
    // Privilege never blanket-purifies: only an explicit runPure discharges.
    assert_eq!(run_cli(PLAIN_IO, &["--privileged"]), "#io");
}

#[test]
fn pin_guard_runpure_seam_no_false_violation() {
    // arc-4 acceptance repair (predict_effect × runPure seam) must survive:
    // a #pure-declared container holding a discharged runPure must NOT trip
    // arc-3's static guard. Lesson carried forward: any change to the
    // discharge path must be re-tested on its combo contagion face.
    assert_eq!(
        run_cli(
            "out: { %effect: #pure, v: (~%Effect./runPure (~%Time.now _)) }.%effect",
            &["--privileged"]
        ),
        "#pure",
    );
}
