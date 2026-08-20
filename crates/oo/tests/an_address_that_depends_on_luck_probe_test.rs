// An address that depends on luck.
// Recon: docs/an_address_that_depends_on_luck_recon.md
//
// ── Two defects, one arc ─────────────────────────────────────────────────
//
//   A  committing a universe that quotes its own root reports success and
//      leaves a store nothing can read -- not even the engine that wrote
//      it. Independent of blur and of fuel.
//   B  the order in which top-level fields are evaluated is drawn fresh
//      per process, so a fuel-limited universe addresses a different blur
//      each run, and that reaches committed root addresses.
//
// They share an infrastructure but not a red line, so they are guarded
// separately: A is data loss, B is identity.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason
// stated in each. The delivery may remove `#[ignore]` and NOTHING else in
// this file.
//
// B's reds run N fresh processes and require agreement. At the baseline
// the two outcomes are near evenly split, so N = 12 makes an accidental
// pass about 1 in 2000. A fix must make them agree by construction, not
// by luck -- if a delivery reports these as "sometimes green", that is a
// report of the bug, not of a fix.

use std::path::Path;
use std::process::Command;

fn oo(dir: &Path, args: &[&str]) -> String {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    let o = c.args(args).output().expect("oo runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("luck-{tag}"))
}

/// Commit `src` into a fresh store, then report whether the store is still
/// readable. Returns (commit output, status output).
fn commit_then_read(tag: &str, src: &str) -> (String, String) {
    let d = scratch(tag);
    std::fs::write(d.join("a.n"), src).unwrap();
    oo(&d, &["evolve", "a.n"]);
    let c = oo(&d, &["commit", "-m", "x"]);
    let s = oo(&d, &["status"]);
    (c, s)
}

fn assert_store_readable(tag: &str, src: &str, why: &str) {
    let (c, s) = commit_then_read(tag, src);
    assert!(
        c.contains("Commit successful"),
        "harness: commit must succeed for {why}, got: {c}"
    );
    assert!(
        !s.contains("Error"),
        "{why}: commit reported success and the store cannot be read back.\n  status: {s}"
    );
}

/// The blur CAID this program observes, or None if it produced no blur.
fn blur_caid(tag: &str, src: &str) -> Option<String> {
    let d = scratch(tag);
    std::fs::write(d.join("a.n"), src).unwrap();
    let out = oo(&d, &["run", "a.n", "--observe", "out"]);
    out.find("hash:sha256:v1:")
        .map(|i| out[i..].chars().take(79).collect())
}

/// The committed root address for `src` in a fresh store.
fn committed_root(tag: &str, src: &str) -> String {
    let d = scratch(tag);
    std::fs::write(d.join("a.n"), src).unwrap();
    oo(&d, &["evolve", "a.n"]);
    let c = oo(&d, &["commit", "-m", "x"]);
    let addr = c
        .split_whitespace()
        .find(|t| t.starts_with("hash:sha256:"))
        .unwrap_or_else(|| panic!("harness: commit printed no address: {c}"))
        .to_string();
    let head = oo(&d, &["inspect", &addr]);
    head.lines()
        .find_map(|l| l.strip_prefix("root:"))
        .unwrap_or_else(|| panic!("harness: commit names no root: {head}"))
        .trim()
        .to_string()
}

/// The two-field program the recon measured. `out` and `v` are the SAME
/// value; whichever is forced second meets the exhausted budget, so which
/// one becomes the blur reveals the evaluation order.
const TWO_FIELDS: &str = "~%Config.fuel: 5\nv: <<_.>>\nout: v\n";

// ── C1..C4 ── controls: green at baseline, MUST stay green ───────────────

#[test]
fn c1_control_a_plain_commit_leaves_a_readable_store() {
    assert_store_readable("c1", "app: { k: 1 }\n", "a plain commit");
}

/// Quoting something that is NOT the root is fine today and must stay fine.
/// This is what isolates defect A to self-reference.
#[test]
fn c2_control_quoting_a_non_root_value_leaves_a_readable_store() {
    assert_store_readable(
        "c2",
        "f: (x -> x + 1)\nv: <<f>>\napp: { k: 1 }\n",
        "a quote of a non-root value",
    );
}

/// A blur with no self-reference is fine today and must stay fine. This is
/// what isolates defect A from blur.
#[test]
fn c3_control_a_blur_without_self_reference_leaves_a_readable_store() {
    assert_store_readable(
        "c3",
        "~%Config.fuel: 1\napp: { k: 1 + 1 }\n",
        "a fuel-exhausted blur with no self-reference",
    );
}

/// The single-field case is already deterministic -- twelve processes, one
/// address. Guards the fix against making the stable case unstable, and
/// shows the harness can detect agreement at all.
#[test]
fn c4_control_a_single_field_blur_already_has_one_address() {
    let src = "~%Config.fuel: 5\nout: <<_.>>\n";
    let first = blur_caid("c4", src).expect("harness: this program must blur");
    for i in 0..12 {
        assert_eq!(
            Some(&first),
            blur_caid(&format!("c4-{i}"), src).as_ref(),
            "the single-field case is deterministic at the baseline and must stay so"
        );
    }
}

// ── R1..R2 ── defect A: the store cannot be read back ────────────────────

/// RED: commit says success; status, log, evolve and commit then all fail
/// with #object_undecodable. Fuel plays no part -- there is none here.
#[test]
#[ignore = "RED: committing a root self-quote leaves an undecodable object"]
fn r1_a_universe_that_quotes_its_root_stays_readable() {
    assert_store_readable(
        "r1",
        "v: <<_.>>\napp: { k: 1 }\n",
        "a universe quoting its own root",
    );
}

/// RED: the store is not merely unreadable, it is unusable -- the history
/// cannot be continued. Separate from r1 because "can still be read" and
/// "can still be written" are different promises.
#[test]
#[ignore = "RED: the store is bricked, so no further commit is possible"]
fn r2_history_can_continue_after_a_root_self_quote() {
    let d = scratch("r2");
    std::fs::write(d.join("a.n"), "v: <<_.>>\napp: { k: 1 }\n").unwrap();
    oo(&d, &["evolve", "a.n"]);
    assert!(
        oo(&d, &["commit", "-m", "one"]).contains("Commit successful"),
        "harness: the first commit must succeed"
    );
    std::fs::write(d.join("b.n"), "app: { k: 1, j: 2 }\n").unwrap();
    let e = oo(&d, &["evolve", "b.n"]);
    let c = oo(&d, &["commit", "-m", "two"]);
    assert!(
        !e.contains("Error") && c.contains("Commit successful"),
        "a committed universe must remain writable.\n  evolve: {e}\n  commit: {c}"
    );
}

// ── R3..R4 ── defect B: the address depends on the draw ──────────────────

/// RED: same program, same fuel, twelve fresh processes. At the baseline
/// this yields two addresses in roughly even proportion.
#[test]
#[ignore = "RED: which field becomes the blur is drawn per process"]
fn r3_the_same_program_addresses_the_same_blur_in_every_process() {
    let first = blur_caid("r3", TWO_FIELDS).expect("harness: this program must blur");
    for i in 0..12 {
        assert_eq!(
            Some(&first),
            blur_caid(&format!("r3-{i}"), TWO_FIELDS).as_ref(),
            "run {i} addressed a different blur; the same program at the same \
             horizon must address the same blur in every process"
        );
    }
}

/// RED, and the one that matters: the non-determinism reaches DURABLE
/// identity. Twelve fresh stores, one source, and the committed root
/// address must not depend on which way the draw went.
#[test]
#[ignore = "RED: the committed root address varies per process"]
fn r4_the_committed_root_address_does_not_depend_on_the_draw() {
    let src = "~%Config.fuel: 5\nv: <<_.>>\napp: { k: 1 }\n";
    let first = committed_root("r4", src);
    for i in 0..12 {
        assert_eq!(
            first,
            committed_root(&format!("r4-{i}"), src),
            "store {i} committed the same source to a different address"
        );
    }
}
