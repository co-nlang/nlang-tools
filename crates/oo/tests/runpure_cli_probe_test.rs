// ~%Effect./runPure privilege CLI probes (2026-07-24, pre-committed by
// work order — docs/effect_runpure_handover.md). 效應系統波 arc 4.
//
// RULING P1 (user 2026-07-24): privileged mode is a horizon capability set
// ONLY via the trusted channel `oo run --privileged` (SPEC_08 §6.1.2 — a
// program cannot self-authorize). These CLI probes are the FAITHFUL test
// of P1: privilege genuinely cannot be established from inside an n/
// program, so the privileged path is only reachable through the flag.
//
//   oo run --privileged <runPure io>   → discharged, %effect #pure
//   oo run            <runPure io>     → ⊥ #privileged_required (no backdoor)
//   oo run --privileged <plain io>     → #io  (privilege is opt-in per
//                                         runPure; it does NOT auto-purify)
//
// MEASURED (baseline, v0.2.36): `--privileged` is an unknown flag (clap
// rejects it) and `~%Effect` is unregistered (runPure → `_`). All
// --privileged gates are red; the no-flag io pin already passes.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

/// Writes `content` into a fresh temp dir, runs `oo run a.n --observe out`
/// (optionally with `--privileged`), returns trimmed stdout+stderr.
fn run_cli(content: &str, privileged: bool) -> String {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "nlang-runpurecli-{}-{}",
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&dir).ok();
    fs::create_dir_all(&dir).unwrap();
    let p: PathBuf = dir.join("a.n");
    fs::write(&p, content).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_oo"));
    cmd.arg("run").arg(&p);
    if privileged {
        cmd.arg("--privileged");
    }
    cmd.arg("--observe").arg("out").current_dir(&dir);
    let out = cmd.output().unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    )
}

const RUNPURE_IO: &str = "out: (~%Effect./runPure (~%Time.now _)).%effect";
const RUNPURE_VAL: &str = "out: ~%Effect./runPure (~%Time.now _)";
const PLAIN_IO: &str = "out: (~%Time.now _).%effect";

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — privilege only via the flag; runPure discharges under it
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn cli_runpure_privileged_discharges() {
    // Under the trusted flag, runPure solidifies the io result to #pure.
    assert_eq!(run_cli(RUNPURE_IO, true), "#pure");
}

#[test]
fn cli_runpure_privileged_clean_value() {
    // The discharged value keeps its content and carries NO #io tail and is
    // not ⊥ — it is lawful pure data now. ~%Time.now discharges to a bare
    // integer timestamp (all digits, no `;; %effect` tail). The positive
    // digit check keeps a baseline clap/usage error from passing vacuously.
    let got = run_cli(RUNPURE_VAL, true);
    assert!(
        !got.is_empty() && got.chars().all(|c| c.is_ascii_digit()),
        "discharged value is clean pure data (bare integer, no tail): {got:?}"
    );
}

#[test]
fn cli_runpure_no_flag_blocked() {
    // Without the flag a program calling runPure is refused — it cannot
    // grant itself privilege (SPEC_08 §6.1.2, no backdoor).
    let got = run_cli(RUNPURE_IO, false);
    assert!(
        got.contains("privileged_required"),
        "unprivileged runPure ⟹ ⊥ #privileged_required: {got:?}"
    );
}

#[test]
fn cli_privileged_plain_io_is_opt_in() {
    // Privilege does NOT blanket-purify: plain io in a privileged run is
    // still #io. Only an explicit runPure discharges.
    assert_eq!(run_cli(PLAIN_IO, true), "#io");
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PIN — the ordinary (unprivileged) run is unchanged
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn cli_normal_run_io_flows() {
    // A normal run (no flag) tracks io as always — privilege absent = the
    // ambient program horizon, unchanged.
    assert_eq!(run_cli(PLAIN_IO, false), "#io");
}
