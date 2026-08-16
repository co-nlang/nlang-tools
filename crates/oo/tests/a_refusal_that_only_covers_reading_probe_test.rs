// A refusal that only covers reading (Q-029, pre-committed by work order:
// docs/a_refusal_that_only_covers_reading_handover.md).
//
// ── The claim ────────────────────────────────────────────────────────────
//
// REAL_03 §6.8, third MUST / MUST NOT:
//   "引擎讀到自己不具備的標準根摘要時，必須拒絕開啟該根 … 不得以自身的標準根
//    代入後繼續"
//
// Measured 2026-08-16 with two real, untampered binaries (v0.22.0+one with
// standard root a63ef70b…, and v0.24.0 with 65f52e2d…): `oo log` and
// `oo inspect` refuse correctly, and then `oo evolve` stages silently and
// `oo commit` reports "Commit successful". The store ends with two roots
// carrying different standard-root digests, two parentless commits, HEAD on
// the foreign one, and the ORIGINAL engine can no longer read its own store.
//
// The refusal covers reading. Writing was never asked.
//
// ── How these probes build an unreadable store ───────────────────────────
//
// No second binary exists in-tree, so `unreadable()` builds a real store
// through the real write path and then rewrites the root object's
// `__nlang_system_digest` sentinel to all-zeros -- a digest no build ships.
//
// This leaves the object's bytes at an address that no longer matches them.
// That is ACCEPTABLE here and must stay understood: measured, the read path
// resolves the standard root BEFORE verifying the address (a tampered store
// answers `refusing root: … is unavailable`, not `#caid_mismatch`), so the
// probes below exercise the intended path. Each red therefore asserts the
// SPECIFIC refusal, never merely "an error" -- an assertion of "it failed"
// would be satisfied by the tampering itself and would witness nothing.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason
// stated in each. The delivery may remove `#[ignore]` and NOTHING else in
// this file. C0 runs first: every assertion below is vacuous if the harness
// never built a store, and `unreadable()` is a rewrite of a file whose path
// this test computes -- if that computation is wrong, every "it refused"
// becomes true for the wrong reason.

use std::path::Path;
use std::process::Command;

const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn fresh(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("refusal-{tag}"))
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let o = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .output()
        .expect("oo runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// Every object file under `.oo/objects/`.
fn objects(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let root = dir.join(".oo/objects/sha256");
    if let Ok(top) = std::fs::read_dir(&root) {
        for a in top.flatten() {
            if let Ok(inner) = std::fs::read_dir(a.path()) {
                for b in inner.flatten() {
                    out.push(b.path());
                }
            }
        }
    }
    out.sort();
    out
}

/// The object files that carry a standard-root sentinel (i.e. the roots).
fn roots_with_sentinel(dir: &Path) -> Vec<(std::path::PathBuf, String)> {
    let mut out = Vec::new();
    for p in objects(dir) {
        let s = std::fs::read_to_string(&p).unwrap_or_default();
        if let Some(i) = s.find("__nlang_system_digest") {
            // The digest is the next 64-hex run after the key.
            let tail = &s[i..];
            let digest = tail
                .split('"')
                .find(|t| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()))
                .unwrap_or("")
                .to_string();
            out.push((p, digest));
        }
    }
    out
}

/// Build a real store, then point its root at a standard root nobody ships.
fn unreadable(tag: &str, source: &str) -> nlang_interpreter::ScratchDir {
    let d = fresh(tag);
    std::fs::write(d.join("a.n"), source).expect("write source");
    oo(&d, &["evolve", "a.n"]);
    oo(&d, &["commit", "-m", "one"]);

    let roots = roots_with_sentinel(&d);
    assert_eq!(
        roots.len(),
        1,
        "harness expected exactly one root carrying a sentinel, found {}",
        roots.len()
    );
    let (path, real) = &roots[0];
    assert_eq!(real.len(), 64, "harness failed to read the sentinel digest");
    let s = std::fs::read_to_string(path).unwrap();
    let swapped = s.replace(real.as_str(), ZERO);
    assert_ne!(s, swapped, "harness rewrote nothing");
    std::fs::write(path, swapped).unwrap();
    d
}

// ── C0 ── the harness itself ─────────────────────────────────────────────

/// Green at the baseline. An ordinary store commits, reads back, and names a
/// standard root this build DOES ship; the tampered twin names one it does
/// not. Without this, "it refused" below could be refusing something else.
#[test]
fn c0_the_harness_builds_a_store_and_then_breaks_exactly_one_thing() {
    let good = fresh("c0-good");
    std::fs::write(good.join("a.n"), "app: { k1: 1 }\n").unwrap();
    oo(&good, &["evolve", "a.n"]);
    let out = oo(&good, &["commit", "-m", "one"]);
    assert!(
        out.contains("Commit successful"),
        "control: an ordinary commit must succeed, got: {out}"
    );
    let log = oo(&good, &["log"]);
    assert!(
        log.contains("commit hash:sha256:"),
        "control: an ordinary log must read back, got: {log}"
    );
    let roots = roots_with_sentinel(&good);
    assert_eq!(roots.len(), 1, "control: exactly one root object");
    assert_ne!(roots[0].1, ZERO, "control: a real store names a real digest");

    let bad = unreadable("c0-bad", "app: { k1: 1 }\n");
    let roots = roots_with_sentinel(&bad);
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].1, ZERO, "harness: the tampered store names 0000…");
    let st = oo(&bad, &["status"]);
    assert!(
        st.contains(ZERO) && st.contains("unavailable"),
        "harness: the engine must SEE it as unavailable, got: {st}"
    );
}

// ── C1, C2 ── what must not regress ──────────────────────────────────────

/// Green at the baseline and must STAY green. `oo status` is the only command
/// that can tell an operator why every other command fails. The repair must
/// not turn the diagnostic into another refusal.
#[test]
fn c1_status_still_answers_and_names_the_missing_digest() {
    let d = unreadable("c1", "app: { k1: 1 }\n");
    let out = oo(&d, &["status"]);
    assert!(
        out.contains("Standard root dependency"),
        "status must keep reporting the dependency, got: {out}"
    );
    assert!(
        out.contains(ZERO),
        "status must name WHICH digest is missing, got: {out}"
    );
    assert!(
        out.contains("unavailable"),
        "status must say it is unavailable, got: {out}"
    );
}

/// Green at the baseline and must STAY green: reading already refuses, and
/// the message already names the missing digest (REAL_03 §6.8: "訊息必須指出
/// 所缺者為何"). This arc must not disturb the half that works.
#[test]
fn c2_log_and_inspect_still_refuse_by_name() {
    let d = unreadable("c2", "app: { k1: 1 }\n");
    let log = oo(&d, &["log"]);
    assert!(
        log.contains("refusing root") && log.contains(ZERO),
        "log must refuse and name the digest, got: {log}"
    );
}

// ── P1..P4 ── the write path ─────────────────────────────────────────────

/// RED at the baseline: `oo evolve` stages silently on a store whose standard
/// root is unavailable. §6.8 third MUST says the root must not be opened;
/// staging opens it.
#[test]
fn p1_evolve_refuses_when_the_standard_root_is_unavailable() {
    let d = unreadable("p1", "app: { k1: 1 }\n");
    std::fs::write(d.join("n.n"), "app: { k1: 1, NEW: 42 }\n").unwrap();
    let out = oo(&d, &["evolve", "n.n"]);
    assert!(
        out.contains("refusing root") && out.contains(ZERO),
        "evolve must refuse and name the digest, got: {out:?}"
    );
    let st = oo(&d, &["status"]);
    assert!(
        !st.contains("Staged changes"),
        "a refused evolve must leave nothing staged, got: {st}"
    );
}

/// RED at the baseline: `oo commit` reports "Commit successful" on a store
/// whose standard root is unavailable.
#[test]
fn p2_commit_refuses_when_the_standard_root_is_unavailable() {
    let d = unreadable("p2", "app: { k1: 1 }\n");
    std::fs::write(d.join("n.n"), "app: { k1: 1, NEW: 42 }\n").unwrap();
    oo(&d, &["evolve", "n.n"]);
    let out = oo(&d, &["commit", "-m", "forged"]);
    assert!(
        !out.contains("Commit successful"),
        "commit must not succeed, got: {out}"
    );
    assert!(
        out.contains("refusing root") && out.contains(ZERO),
        "commit must refuse and name the digest, got: {out}"
    );
}

/// RED at the baseline, and this is the MUST NOT itself: after a write
/// attempt, the store must contain NO root naming a standard root other than
/// the one it started with. Today it contains one naming this build's own.
///
/// The red asserts an absence, so it also asserts a presence in the same run:
/// the ORIGINAL root must still be there and still name 0000….
#[test]
fn p3_no_root_is_written_under_this_engines_own_standard_root() {
    let d = unreadable("p3", "app: { k1: 1 }\n");
    std::fs::write(d.join("n.n"), "app: { k1: 1, NEW: 42 }\n").unwrap();
    oo(&d, &["evolve", "n.n"]);
    oo(&d, &["commit", "-m", "forged"]);

    let roots = roots_with_sentinel(&d);
    assert!(
        roots.iter().any(|(_, dg)| dg == ZERO),
        "presence: the original root must survive the refused write, found {:?}",
        roots.iter().map(|(_, d)| d).collect::<Vec<_>>()
    );
    let foreign: Vec<&String> = roots.iter().map(|(_, d)| d).filter(|d| *d != ZERO).collect();
    assert!(
        foreign.is_empty(),
        "MUST NOT (REAL_03 §6.8): no root may be written under a substituted \
         standard root, found {foreign:?}"
    );
}

/// RED at the baseline: HEAD moves onto a commit this engine had no standing
/// to write. Separate from P3 because a store can gain an orphan object
/// without losing its head, and losing the head is the data-loss half.
#[test]
fn p4_head_does_not_move_on_a_store_this_engine_cannot_open() {
    let d = unreadable("p4", "app: { k1: 1 }\n");
    let head_before = std::fs::read_to_string(d.join(".oo/HEAD")).unwrap_or_default();
    assert!(
        head_before.contains("hash:sha256:"),
        "presence: the store must have a HEAD to begin with, got {head_before:?}"
    );
    std::fs::write(d.join("n.n"), "app: { k1: 1, NEW: 42 }\n").unwrap();
    oo(&d, &["evolve", "n.n"]);
    oo(&d, &["commit", "-m", "forged"]);
    let head_after = std::fs::read_to_string(d.join(".oo/HEAD")).unwrap_or_default();
    assert_eq!(
        head_before.trim(),
        head_after.trim(),
        "HEAD must not move when the engine cannot open the root"
    );
}

// ── P5 ── the other write-shaped entry points ────────────────────────────

/// Green at the baseline and must STAY green. `oo rollback` is the ONE
/// write-shaped command that already gets this right, and the reason is
/// visible in-tree: `Universe::rollback` calls
/// `engine.store.get_root(&target_commit.root, &engine.standard_roots)?`
/// itself, so the refusal propagates past the fallback in `load_universe`.
///
/// This is the worked example the repair should copy, not a site to change.
#[test]
fn c3_rollback_already_refuses_and_leaves_head_alone() {
    let d = unreadable("c3", "app: { k1: 1 }\n");
    let head_before = std::fs::read_to_string(d.join(".oo/HEAD")).unwrap_or_default();
    let caid = head_before.trim().to_string();
    assert!(
        caid.starts_with("hash:sha256:"),
        "presence: HEAD must hold a real CAID, got {caid:?}"
    );
    let out = oo(&d, &["rollback", &caid, "--grant", "rollback"]);
    assert!(
        out.contains("refusing root") && out.contains(ZERO),
        "rollback must refuse and name the digest, got: {out}"
    );
    let head_after = std::fs::read_to_string(d.join(".oo/HEAD")).unwrap_or_default();
    assert_eq!(head_before.trim(), head_after.trim(), "HEAD must not move");
}

/// RED at the baseline: `oo squash` answers `no HEAD to squash` on a store
/// that visibly has a HEAD. The sentence is true of the phantom Universe the
/// fallback built and false of the store, which is the defect.
#[test]
fn p5_squash_refuses_by_name_instead_of_denying_the_head_exists() {
    let d = unreadable("p5", "app: { k1: 1 }\n");
    let head = std::fs::read_to_string(d.join(".oo/HEAD")).unwrap_or_default();
    let caid = head.trim().to_string();
    assert!(
        caid.starts_with("hash:sha256:"),
        "presence: the store HAS a head — that is what makes the message false, got {caid:?}"
    );
    let out = oo(&d, &["squash", &caid, "--grant", "squash"]);
    assert!(
        !out.contains("no HEAD"),
        "squash must not deny the head exists, got: {out}"
    );
    assert!(
        out.contains("refusing root") && out.contains(ZERO),
        "squash must refuse and name the digest, got: {out}"
    );
}

/// RED at the baseline: `oo refine` commits and moves HEAD. It is the worst
/// of the write-shaped commands, because its monotonicity check reads the
/// operands through `universe.rs:989`, whose own comment says
/// "pretending it passed is the fail-open this arc exists to close" —
/// and an unavailable standard root lands in exactly that `| None` arm.
#[test]
fn p6_refine_refuses_when_the_standard_root_is_unavailable() {
    let d = unreadable("p6", "app: { k1: 1 }\n");
    let head_before = std::fs::read_to_string(d.join(".oo/HEAD")).unwrap_or_default();
    assert!(
        head_before.contains("hash:sha256:"),
        "presence: the store must have a HEAD to begin with"
    );
    let roots = roots_with_sentinel(&d);
    let root_caid = format!(
        "hash:sha256:v1:{}{}",
        roots[0].0.parent().unwrap().file_name().unwrap().to_string_lossy(),
        roots[0].0.file_name().unwrap().to_string_lossy()
    );
    let out = oo(
        &d,
        &["refine", "--source", &root_caid, "--target", &root_caid, "-m", "x"],
    );
    assert!(
        !out.contains("Refine commit"),
        "refine must not commit, got: {out}"
    );
    assert!(
        out.contains("refusing root") && out.contains(ZERO),
        "refine must refuse and name the digest, got: {out}"
    );
    let head_after = std::fs::read_to_string(d.join(".oo/HEAD")).unwrap_or_default();
    assert_eq!(head_before.trim(), head_after.trim(), "HEAD must not move");
}
