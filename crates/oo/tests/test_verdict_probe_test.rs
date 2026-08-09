// `oo test` verdict probes (2026-07-20, pre-committed by work order —
// docs/test_verdict_handover.md).
//
// RULING B (2026-07-20): a test IS an observation, and passing means
// "this observation decided a definite fact" (SPEC_00 §1.2 靜與動;
// SPEC_16 §2.2 rewritten). PASS = converges to a DEFINITE value.
// FAIL = ⊥ (with %cause), #false/#fail (assertion refuted), Top /
// TopCaused (undetermined — an observation that decided nothing proves
// nothing; vacuous truth forbidden), #blur (undetermined within the
// horizon; report the blur %cause).
//
// MEASURED (v0.2.28): the runner follows the OLD SPEC_16 letter ("any
// non-⊥ passes") — `(_) == 5` → `_` → PASS (vacuous), runaway blur →
// PASS. Two real vacuous cases were found in the corpus before this
// arc: effect_taint.n (`_ == #io`, cured by the %effect lens arc) and
// test_canonical.n's `.%type` retired-spelling leftover (migrated to
// `.%cause` at this arc's open — the assertion had never actually been
// verified).
//
// NOT in scope: `--static-only` mode (passes without observation by
// design), test discovery rules, corpus content beyond the one open
// migration, conformance vectors (CLI harness face — not vectorable).

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

/// Writes content as test.n in a fresh temp dir; runs `oo test test.n`;
/// returns (exit_success, stdout+stderr).
fn run_test_cmd(content: &str) -> (bool, String) {
    let dir = nlang_interpreter::ScratchDir::new("verdict");
    let p: PathBuf = dir.join("test.n");
    fs::write(&p, content).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_oo"))
        .arg("test")
        .arg(&p)
        .current_dir(&dir)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), text)
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — undetermined observations must FAIL (SPEC_16 §2.2, ruling B)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_top_test_fails() {
    // The vacuous-truth face itself: `(_) == 5` observes to Top.
    let (ok, text) = run_test_cmd("test_vacuous: (_) == 5\n");
    assert!(!ok, "Top test must fail the run: {text}");
    assert!(
        text.contains("FAIL") && text.contains("test_vacuous"),
        "verdict names the vacuous test: {text}"
    );
}

#[test]
fn red_top_alias_test_fails() {
    // Orphan-twin shape: an undefined meta read compared to a tag.
    let (ok, text) = run_test_cmd("q: 5\ntest_ghost: q.%nonsense == #io\n");
    assert!(!ok, "undetermined comparison must fail: {text}");
    assert!(text.contains("FAIL"), "verdict says FAIL: {text}");
}

#[test]
fn red_blur_test_fails() {
    // Runaway recursion blurs at the horizon — undetermined, not proof.
    // Default budgets hit max_unification_depth before fuel (ERROR_CODES §2.7.2).
    let (ok, text) = run_test_cmd("/rec: x -> /rec (x + 1)\ntest_runaway: /rec 1\n");
    assert!(!ok, "blur test must fail the run: {text}");
    assert!(
        text.contains("FAIL") && text.contains("max_depth_exceeded"),
        "blur verdict reports the horizon cause: {text}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — definite verdicts stay exactly as they are
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_true_passes() {
    let (ok, text) = run_test_cmd("test_eq: 1 + 1 == 2\n");
    assert!(ok, "boolean truth passes: {text}");
    assert!(text.contains("PASS"), "{text}");
}

#[test]
fn pin_smoke_definite_value_passes() {
    // Ruling B keeps the smoke-test semantic: converging to a definite
    // non-boolean value is a lawful pass (the observation decided it).
    let (ok, text) = run_test_cmd("test_smoke: { a: 1, b: 2 }\n");
    assert!(ok, "definite combo passes: {text}");
    assert!(text.contains("PASS"), "{text}");
}

#[test]
fn pin_false_fails() {
    let (ok, text) = run_test_cmd("test_wrong: 1 + 1 == 3\n");
    assert!(!ok, "refuted assertion fails: {text}");
    assert!(text.contains("FAIL"), "{text}");
}

#[test]
fn pin_bottom_fails_with_cause() {
    let (ok, text) = run_test_cmd("test_conflict: 1 & 2\n");
    assert!(!ok, "⊥ fails: {text}");
    assert!(
        text.contains("FAIL") && text.to_lowercase().contains("conflict"),
        "⊥ verdict reports %cause: {text}"
    );
}

#[test]
fn pin_migrated_depth_test_genuinely_true() {
    // The open-migration twin: the .%cause respelling decides a fact.
    // Runaway under default budgets is #max_depth_exceeded (ERROR_CODES §2.7.2).
    let (ok, text) = run_test_cmd(
        "/rec: x -> /rec (x + 1)\n~e: (/rec 1).%cause\ntest_depth: ~e == #max_depth_exceeded\n",
    );
    assert!(ok, "migrated spelling passes genuinely: {text}");
    assert!(text.contains("PASS"), "{text}");
}
