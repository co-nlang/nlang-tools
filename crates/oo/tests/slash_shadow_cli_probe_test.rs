// G2-S CLI probes (2026-07-12, pre-committed by work order —
// docs/g2_shadow_multiparam_handover.md).
//
// Measured today: `/add: (x -> …)` + `z: 42` → exit 0, every observation
// prints `_|_ (%cause: #conflict)` with no path (silent universe poison).
// A data-axis conflict (`a: 1` / `a: 2`) → exit 1, stderr names the field.
// Ruling: the root-builtin collision must join the loud path.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn run_observe(content: &str, observe: &str) -> (i32, String, String) {
    // ScratchDir must outlive the `oo` process that reads the file.
    let d = nlang_interpreter::ScratchDir::new("g2cli");
    let f = d.join("probe.n");
    fs::write(&f, content).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_oo"))
        .arg("run")
        .arg(&f)
        .arg("--observe")
        .arg(observe)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[test]
fn red_cli_slash_add_shadow_is_loud() {
    // today: exit 0, stdout "_|_ (%cause: #conflict)", stderr silent
    let (code, _stdout, stderr) = run_observe("/add: (x -> (y -> x + y))\nz: 42\n", "z");
    assert_ne!(code, 0, "root-builtin shadow must not exit 0");
    assert!(
        stderr.contains("Evolution Conflict"),
        "stderr must use the same loud label as data conflicts, got: {stderr}"
    );
    assert!(
        stderr.contains("add"),
        "stderr must name the colliding coordinate, got: {stderr}"
    );
}

#[test] // ACTIVE pin: data-axis conflict UX — the shape G2-S aligns with
fn pin_cli_data_conflict_is_loud() {
    let (code, _stdout, stderr) = run_observe("a: 1\na: 2\nz: 42\n", "z");
    assert_ne!(code, 0);
    assert!(stderr.contains("Evolution Conflict"), "got: {stderr}");
}

#[test] // ACTIVE pin: non-colliding slash def end-to-end through the CLI
fn pin_cli_slash_noncolliding_green() {
    let (code, stdout, _stderr) =
        run_observe("/myadd: (x -> (y -> x + y))\nout: myadd 3 5\n", "out");
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "8");
}
