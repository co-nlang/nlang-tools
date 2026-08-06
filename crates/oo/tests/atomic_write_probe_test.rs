// Durable writes must not tear — REAL_01 §4 / SPEC_10 §4.1 (2026-08-06).
// Pre-committed by work order: docs/atomic_writes_handover.md
//
// ── The defect ───────────────────────────────────────────────────────────
//
// Every durable write in the engine is a non-atomic in-place rewrite.
// `std::fs::write` opens with O_TRUNC and then writes; between those two steps
// the file is shorter than both its old and its new content, so a concurrent
// reader sees a truncated file.
//
// Measured 2026-08-06 — a 400-field universe (`.oo/staged` = 94,413 bytes),
// 60 `oo evolve` cycles, a reader loop parsing continuously:
//
//   63,769 reads · 12 parse failures · 0.02% per read
//
// Per read that is tiny. Per *write* it is 0.2 failures, so a reader running
// across 60 writes expects ~12 and P(zero) is about 6e-6. The window is real
// and reachable, not theoretical.
//
// It already has a user-visible symptom: the decode failure reads "object
// present for … but cannot be decoded (**integrity unknown**)" — and
// "integrity unknown" is exactly what a partial write looks like. The engine
// has the error path; it does not know this is one of its causes.
//
// ── Why this probe does not race ─────────────────────────────────────────
//
// The obvious probe is a racing reader. It was measured and rejected: 0.02%
// per read means a slow test that is red by probability rather than by
// construction, and a gate that flakes teaches its reader to re-run instead of
// to look.
//
// In-place rewrite reuses the **inode**. A temp-file-plus-rename write
// installs a new one, every time, on every filesystem that can rename.
// Measured at baseline, five writes to `.oo/staged`:
//
//   write 1 → inode 304362      write 4 → inode 304362
//   write 2 → inode 304362      write 5 → inode 304362
//   write 3 → inode 304362
//
// So R1–R3 are red every run and green every run after the change, with no
// timing dependence at all.
//
// ── What is not pinned, and why it is said out loud ──────────────────────
//
// **Objects.** `write_object` short-circuits on `path.exists()`, so writing
// the same object twice does nothing and the inode never moves. There is no
// race-free signature for that path. `.oo/objects/**` is still in scope for
// the fix; its atomicity is verified by the acceptor's race measurement
// (work order §6.5), and P1/P4 are what guard it inside the suite.
//
// **Lost updates.** temp+rename fixes torn reads. It does not fix
// A-reads → B-reads → A-renames → B-renames, where A's field is simply gone.
// Measured at baseline: 40 concurrent `oo evolve` each adding a distinct
// field, expected 41, **got 2**. That number is recorded in the work order
// and is deliberately **not** a probe here — a red this delivery cannot turn
// green is a countdown timer. It belongs to the CAS-and-retry arc.
//
// **Windows.** Inodes are POSIX. R1–R3 are `#[cfg(unix)]` and the property is
// unpinned on Windows; saying so is cheaper than pretending otherwise.
//
// ── Two limits found during calibration ──────────────────────────────────
//
// 1. **A new inode is necessary, not sufficient.** Deleting the file and
//    creating it fresh also moves the inode, and is just as unsafe — it swaps
//    a truncated-file window for an absent-file window. R1–R3 cannot tell the
//    two apart. What closes it is the acceptor's race measurement (work order
//    §6.4), and that measurement must count **missing-file errors as well as
//    parse errors**, or delete-and-recreate reads as a clean pass.
//
// 2. **A temp file inside `objects/` would be counted as an object.**
//    `local_gc_probe_test.rs`'s `store_map` walks `objects/sha256/<2hex>/`
//    and keys **every file** it finds as `<2hex><rest>` — so a leftover temp
//    would appear as a phantom object and move that suite's counts and byte
//    totals. That is a failure mode **this arc could introduce**, which is
//    what P1 is guarding, and why `local_gc` is on the independent re-run
//    list rather than merely nearby.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

// ── harness ─────────────────────────────────────────────────────────────

fn fresh_dir(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("atomicwrite-{tag}"))
}

fn oo_cmd(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    c
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let out = oo_cmd(dir).args(args).output().unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    )
}

/// Evolve one fresh field. Each call rewrites `.oo/staged`.
fn evolve_field(dir: &Path, n: usize) -> String {
    let f = format!("w{n}.n");
    fs::write(dir.join(&f), format!("k{n}: {n}\n")).unwrap();
    oo(dir, &["evolve", &f])
}

fn ino(p: &Path) -> u64 {
    fs::metadata(p)
        .unwrap_or_else(|e| panic!("stat {}: {e}", p.display()))
        .ino()
}

fn staged(dir: &Path) -> PathBuf {
    dir.join(".oo").join("staged")
}

/// Collect every regular file under `.oo/`, relative to the workspace.
fn walk_oo(dir: &Path) -> Vec<String> {
    fn rec(base: &Path, at: &Path, out: &mut Vec<String>) {
        let Ok(rd) = fs::read_dir(at) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                rec(base, &p, out);
            } else {
                out.push(
                    p.strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    let mut out = Vec::new();
    rec(dir, &dir.join(".oo"), &mut out);
    out.sort();
    out
}

/// A workspace with `.oo/staged` already on disk, so the first measured write
/// is a rewrite rather than a create. (A create legitimately mints a new
/// inode; measuring one would let R1 pass for the wrong reason.)
fn workspace(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = fresh_dir(tag);
    evolve_field(d.path(), 0);
    assert!(
        staged(d.path()).exists(),
        "fixture did not create .oo/staged"
    );
    d
}

// ── controls ────────────────────────────────────────────────────────────

/// C1 — the fixture really rewrites `staged`: the walker finds the file, and
/// its **content** changes between two evolves. Without this, a fixture that
/// silently stopped writing would leave every red red for the wrong reason
/// (and, after the change, would leave them red forever).
#[test]
fn c1_the_fixture_rewrites_staged_and_content_moves() {
    let d = workspace("c1");
    let p = staged(d.path());

    let before = fs::read(&p).unwrap();
    evolve_field(d.path(), 1);
    let after = fs::read(&p).unwrap();

    assert_ne!(
        before, after,
        "evolve did not change .oo/staged — the fixture is not exercising the write path"
    );

    let files = walk_oo(d.path());
    assert!(
        files.iter().any(|f| f == ".oo/staged"),
        "walker did not see .oo/staged; saw {files:?}"
    );
}

// ── reds ────────────────────────────────────────────────────────────────

/// R1 — `.oo/staged` must get a new inode on every write.
#[test]
fn r1_staged_is_replaced_not_rewritten_in_place() {
    let d = workspace("r1");
    let p = staged(d.path());

    let mut seen = vec![ino(&p)];
    for n in 1..=5 {
        evolve_field(d.path(), n);
        seen.push(ino(&p));
    }

    let stuck: Vec<_> = seen.windows(2).filter(|w| w[0] == w[1]).collect();
    assert!(
        stuck.is_empty(),
        ".oo/staged kept its inode across writes — in-place truncate+write, \
         so a concurrent reader can see a half file. inodes: {seen:?}"
    );
}

/// R2 — `.oo/HEAD` must get a new inode on every write.
#[test]
fn r2_head_is_replaced_not_rewritten_in_place() {
    let d = workspace("r2");
    let head = d.path().join(".oo").join("HEAD");

    evolve_field(d.path(), 1);
    oo(d.path(), &["commit", "-m", "one"]);
    assert!(head.exists(), "fixture did not create .oo/HEAD");

    let mut seen = vec![ino(&head)];
    for n in 2..=4 {
        evolve_field(d.path(), n);
        let out = oo(d.path(), &["commit", "-m", "more"]);
        assert!(
            out.contains("Commit successful"),
            "fixture commit {n} did not land: {out}"
        );
        seen.push(ino(&head));
    }

    let stuck: Vec<_> = seen.windows(2).filter(|w| w[0] == w[1]).collect();
    assert!(
        stuck.is_empty(),
        ".oo/HEAD kept its inode across commits. inodes: {seen:?}"
    );
}

/// R3 — the pending files beside `staged` likewise. `pin_pending` stands for
/// the family: `effect_pending` and `abandoned` are written by the same
/// `save_staged` / `save_abandoned` code path, so a fix that misses them
/// would also miss this one.
#[test]
fn r3_pin_pending_is_replaced_not_rewritten_in_place() {
    let d = workspace("r3");
    let pin = d.path().join(".oo").join("pin_pending");

    let mut seen = Vec::new();
    for n in 1..=4 {
        let f = format!("p{n}.n");
        fs::write(d.path().join(&f), format!("k0: {n}\n")).unwrap();
        let out = oo(d.path(), &["evolve", &f, "--pin", "--grant", "pin"]);
        assert!(
            !out.contains("error") && !out.contains("Usage:"),
            "fixture pin {n} did not land: {out}"
        );
        assert!(pin.exists(), "pin {n} did not write .oo/pin_pending: {out}");
        seen.push(ino(&pin));
    }

    let stuck: Vec<_> = seen.windows(2).filter(|w| w[0] == w[1]).collect();
    assert!(
        stuck.is_empty(),
        ".oo/pin_pending kept its inode across writes. inodes: {seen:?}"
    );
}

// ── pins ────────────────────────────────────────────────────────────────

/// P1 — no temp-shaped leftover anywhere under `.oo/`. Leads with a control
/// on the walker itself: a scan that silently returns nothing would let this
/// pass by finding no files at all.
#[test]
fn p1_no_temp_shaped_leftovers_under_oo() {
    let d = workspace("p1");
    for n in 1..=3 {
        evolve_field(d.path(), n);
    }
    oo(d.path(), &["commit", "-m", "one"]);

    let files = walk_oo(d.path());
    assert!(
        files.len() >= 3,
        "walker control failed: expected several files under .oo/, saw {files:?}"
    );

    let strays: Vec<_> = files
        .iter()
        .filter(|f| {
            let name = f.rsplit('/').next().unwrap().to_ascii_lowercase();
            name.contains("tmp") || name.ends_with('~') || name.ends_with(".new")
        })
        .collect();
    assert!(
        strays.is_empty(),
        "temp-shaped files left under .oo/: {strays:?} (all files: {files:?})"
    );
}

/// P2 — this arc changes how bytes land, not what they mean.
#[test]
fn p2_format_is_not_bumped() {
    let d = workspace("p2");
    evolve_field(d.path(), 1);
    oo(d.path(), &["commit", "-m", "one"]);

    let fmt = fs::read_to_string(d.path().join(".oo").join("format")).unwrap();
    assert_eq!(fmt.trim(), "1", ".oo/format moved");
}

/// P3 — a workspace written by one invocation still loads in the next: the
/// staged universe survives, and a commit on top of it lands.
#[test]
fn p3_a_workspace_reopens_and_still_commits() {
    let d = workspace("p3");
    evolve_field(d.path(), 1);
    evolve_field(d.path(), 2);

    let status = oo(d.path(), &["status"]);
    assert!(
        status.contains("k1") && status.contains("k2"),
        "reopened workspace lost staged fields: {status}"
    );

    let out = oo(d.path(), &["commit", "-m", "reopen"]);
    assert!(out.contains("Commit successful"), "commit failed: {out}");

    let log = oo(d.path(), &["log"]);
    assert!(log.contains("reopen"), "commit missing from log: {log}");
}

/// P4 — objects still round-trip: what a commit writes, `inspect` reads back.
#[test]
fn p4_committed_objects_still_decode() {
    let d = workspace("p4");
    evolve_field(d.path(), 1);

    let out = oo(d.path(), &["commit", "-m", "round-trip"]);
    let caid = out
        .split_whitespace()
        .find(|t| t.starts_with("hash:sha256:"))
        .unwrap_or_else(|| panic!("no CAID in commit output: {out}"))
        .to_string();

    let inspected = oo(d.path(), &["inspect", &caid]);
    assert!(
        inspected.contains("root:"),
        "inspect did not decode the commit: {inspected}"
    );
    assert!(
        !inspected.contains("integrity unknown"),
        "object decoded as damaged right after being written: {inspected}"
    );
}
