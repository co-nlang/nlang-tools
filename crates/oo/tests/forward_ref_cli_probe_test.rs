// Forward-reference CLI probes (2026-07-12, pre-committed by work order —
// docs/forward_ref_handover.md).
//
// Symptom: `oo run` observes each field RIGHT AFTER its own evolve
// (run_one_shot store-put loop) — solidifying reified thunks before later
// fields land. One-shot fields are one simultaneous snapshot (SPEC_03
// commutativity): forward refs must resolve, across files too.
// The store-put purpose (values into Store for CAID refs) must be kept —
// move it after ALL evolves, don't delete it.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

/// Writes each (name, content) into a fresh temp dir; runs
/// `oo run <files…> --observe <path>` there; returns trimmed stdout.
fn run_cli(files: &[(&str, &str)], observe: &str) -> String {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "nlang-fwdcli-{}-{}",
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&dir).ok();
    fs::create_dir_all(&dir).unwrap();
    let mut paths: Vec<PathBuf> = Vec::new();
    for (name, content) in files {
        let p = dir.join(name);
        fs::write(&p, content).unwrap();
        paths.push(p);
    }
    let out = Command::new(env!("CARGO_BIN_EXE_oo"))
        .arg("run")
        .args(&paths)
        .arg("--observe")
        .arg(observe)
        .current_dir(&dir)
        .output()
        .unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    )
}

// ─────────────────────────────────────────────────────────────────────────
// RED LINES — per-field observe in run_one_shot solidifies too early
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore] // RED LINE: bare forward ref through the CLI (today `_`)
fn cli_fwd_bare_resolves() {
    assert_eq!(run_cli(&[("a.n", "out: a\na: 5")], "out"), "5");
}

#[test]
#[ignore] // RED LINE: forward chain through the CLI (today `_`)
fn cli_fwd_chain_resolves() {
    assert_eq!(
        run_cli(&[("a.n", "out: mid\nmid: base\nbase: 1")], "out"),
        "1"
    );
}

#[test]
#[ignore] // RED LINE: cross-FILE forward ref — one-shot = one snapshot
fn cli_multifile_fwd_resolves() {
    assert_eq!(
        run_cli(&[("a.n", "out: x + 1"), ("b.n", "x: 4")], "out"),
        "5"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE pins — CLI behavior that must survive the fix
// ─────────────────────────────────────────────────────────────────────────

#[test] // ACTIVE pin: backward ref via CLI
fn pin_cli_bwd_control() {
    assert_eq!(run_cli(&[("a.n", "a: 5\nout: a + 1")], "out"), "6");
}

#[test] // ACTIVE pin: cross-file backward ref via CLI
fn pin_cli_multifile_bwd() {
    assert_eq!(
        run_cli(&[("b.n", "x: 4"), ("a.n", "out: x + 1")], "out"),
        "5"
    );
}

#[test] // ACTIVE pin (⊥ side): mutual cycle stays #divergent via CLI (L2-17)
fn pin_cli_mutual_cycle_divergent() {
    let got = run_cli(&[("a.n", "a: b + 1\nb: a + 1\nout: a")], "out");
    assert!(
        got.contains("#divergent"),
        "mutual cycle must print #divergent, got {got:?}"
    );
}

#[test] // ACTIVE pin: in-field conflict keeps the observe channel + cause
fn pin_cli_conflict_channel() {
    let got = run_cli(&[("a.n", "a: 1 & \"\"\nout: a")], "out");
    assert!(
        got.contains("#conflict"),
        "in-field conflict must print #conflict, got {got:?}"
    );
}
