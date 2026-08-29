// A working set that is a set.
// Rulings: nlang-spec/meta/oo/STATUS.md D48, D49
//          (full text meta/oo/commit.md 1.2.2; split recorded in WORK_QUEUE 3)
// Recon:   nlang-tools/docs/an_object_you_can_swap_recon.md, appendix 2
// Order:   nlang-tools/docs/a_working_set_that_is_a_set_handover.md
//
// -- What this arc is ----------------------------------------------------
//
// SPEC_10 section 3 clause 2 has always said it: "Staged: the SET of
// definitions injected since the last commit and not yet committed."
// The engine stores that set as one mutable file it rewrites on every
// evolve. That single cell is the whole defect. Forty concurrent evolves,
// each adding a DIFFERENT field, leave three to six of them -- with zero
// errors and every process reporting success.
//
// D48 takes option C: evolve mints one immutable injection instead of
// read-modify-writing a shared cell. The working set is the fold. This is
// not a new design; it is the first implementation that matches the
// sentence already in the spec.
//
// -- The two clauses that bound it ---------------------------------------
//
// SPEC_10 2.2.1 (Core Requirement): when the meet with the existing set is
// bottom, the evolution MUST NOT happen. Today's "first wins" IS the
// compliant behaviour and G1 pins it. C must validate BEFORE it writes.
// That validating read tolerates staleness -- distinct fields never
// conflict -- which is why R1 can go green without any coordination.
//
// D49: when two concurrent injections each validate and JOINTLY meet to
// bottom, both are kept and the fold reports bottom AT THE COORDINATE.
// Rejecting the later one would need an order, and this arc has none.
//
// -- Out of scope, do not touch ------------------------------------------
//
//   * `.oo/savepoints/` and everything about savepoint identity or order.
//     D48 split those out as Q-014b; SPEC_10 3.1's two MUSTs (survives
//     commit, decidable order) bind THERE, not here. G2 pins that this arc
//     leaves the circle layer alone. An injection is NOT a savepoint.
//   * pin ordering, compare-and-swap, retry. All Q-016.
//   * The derived snapshot that would make the fold O(1). It is derived,
//     so it can be added later without redoing this; recon Q16 priced it.
//   * Observation writing a circle; CLI savepoint verbs; moving identity.
//
// -- Probe integrity ------------------------------------------------------
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason
// stated in each. The delivery may remove `#[ignore]` and NOTHING else in
// this file. If a pin here is wrong, say so in the report -- do not edit it.
//
// The invariant the reds assert is deliberately NOT "the working set has
// forty fields". It is: THE NUMBER OF PROCESSES TOLD THEY SUCCEEDED EQUALS
// THE NUMBER OF INJECTIONS THE WORKING SET REFLECTS. A count can be wrong
// for reasons that have nothing to do with this arc (a process failing to
// start, D47 skipping a byte-identical body). A success that left no trace
// is a lie no matter what the count is.
//
// That framing is itself a correction. The acceptor first wrote the
// completion condition as "every LOG line has a file, and LOG lines <=
// directory files" -- and then measured it against the real orphan from
// trial 0 (LOG 5, directory 6): BOTH CLAUSES PASS while the defect is
// present. The invariant was pointing the wrong way. Reverse-direction
// invariants are the ones that catch things.
//
// NOT PROBED, stated so no one mistakes silence for coverage:
//   * The on-disk shape of an injection. This arc is free to choose it and
//     a probe would pin an invented format. The order asks for the shape
//     and a runnable command in writing instead (Q1, Q3).
//   * Crash windows. Every finding about them this week came from
//     reconstructing state with `cp`, not from real crash injection, and a
//     probe that fakes a crash pins the faking.
//
// Baseline measured 2026-08-30 against the v0.38.0 tag build
// (known-answer `~%Math./add (1,2)` -> `3`): 4 green, 2 red. Each red
// fails at ITS OWN assertion, not at a REACH guard.

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

/// Run `oo evolve` and report ONLY whether the process claimed success.
/// The whole point of R1/R2 is comparing this against what survived.
fn evolve_claims_success(dir: &Path, file: &str) -> bool {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    c.args(["evolve", file])
        .status()
        .expect("oo runs")
        .success()
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("workingset-{tag}"))
}

/// `oo status` is the authority on the working set. Reading `.oo/staged`
/// would pin the very file this arc is allowed to remove.
fn working_set(d: &Path) -> String {
    oo(d, &["status"])
}

/// Spawn one `oo evolve` per source, all at once, and return how many
/// processes claimed success.
fn evolve_all_at_once(d: &Path, files: &[String]) -> usize {
    let mut kids: Vec<std::process::Child> = Vec::new();
    for f in files {
        let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
        c.current_dir(d)
            .env("OO_IDENTITY", d.join("identity-for-tests"))
            .env("OO_NODE_HOME", d.join("node-home-for-tests"))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        kids.push(c.args(["evolve", f]).spawn().expect("spawn"));
    }
    kids.iter_mut()
        .filter(|_| true)
        .map(|k| k.wait().expect("wait").success())
        .filter(|ok| *ok)
        .count()
}

const STD_ROOT: &str = "7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911";
const X0_ROOT: &str = "31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a";

// ── R1 ── a success that left no trace is a lie ──────────────────────────
//
// RED at the baseline. Forty concurrent evolves, forty DISTINCT fields, so
// no two of them conflict and SPEC_10 2.2.1 never fires. Every process is
// told it succeeded; three to six fields survive. Measured on the v0.38.0
// tag: 3/40, and 4/40, 4/40, 5/40, 4/40 over five earlier runs.
//
// The assertion is the equality, not the number forty. See the header.
#[test]
#[ignore = "RED at baseline: 40 processes claim success, 3-6 injections survive"]
fn r1_every_success_leaves_a_trace() {
    let d = scratch("r1");
    let mut files = Vec::new();
    for i in 0..40 {
        let f = format!("e{i}.n");
        std::fs::write(d.join(&f), format!("f{i}: {i}\n")).unwrap();
        files.push(f);
    }
    let claimed = evolve_all_at_once(&d, &files);

    let status = working_set(&d);
    // REACH: prove we are reading a working set at all before counting it.
    assert!(
        status.contains("Staged changes"),
        "REACH: `oo status` did not report a working set, so the count below \
         would be measuring nothing. Got:\n{status}"
    );
    let reflected = (0..40)
        .filter(|i| status.contains(&format!("f{i}:")))
        .count();

    assert_eq!(
        claimed, reflected,
        "{claimed} processes were told they succeeded and {reflected} \
         injections survive. A success that left no trace is a lie. \
         (D48: the shared mutable cell is the defect; there is nothing to \
         lose an update on once each injection is its own immutable file.)"
    );
}

// ── R2 ── the same lie, at one coordinate ────────────────────────────────
//
// RED at the baseline. Two concurrent evolves write the SAME coordinate
// with incompatible values. Today the shared cell means last-writer-wins:
// both processes are told they succeeded, one injection is gone, and
// nothing anywhere says so.
//
// D49 says both are kept and the fold reports bottom AT `a`. Either
// outcome satisfies this probe -- one process rejected (claimed == 1,
// reflected == 1), or both kept and the coordinate is bottom (claimed == 2,
// reflected == 2). What must not survive is 2 and 1.
#[test]
#[ignore = "RED at baseline: both processes claim success, one injection vanishes"]
fn r2_a_conflicting_pair_does_not_vanish_silently() {
    let d = scratch("r2");
    std::fs::write(d.join("p.n"), "a: 1\n").unwrap();
    std::fs::write(d.join("q.n"), "a: \"x\"\n").unwrap();
    let claimed = evolve_all_at_once(&d, &["p.n".to_string(), "q.n".to_string()]);

    let status = working_set(&d);
    assert!(
        status.contains("Staged changes") || status.contains("Conflict"),
        "REACH: `oo status` reported neither a working set nor a conflict; \
         the assertion below would be measuring nothing. Got:\n{status}"
    );
    // An injection is reflected if its value is visible, or if the
    // coordinate carries the bottom that D49 requires the fold to report.
    let bottom_at_a = status.contains("_|_") || status.contains("#conflict");
    let reflected = if bottom_at_a {
        2
    } else {
        usize::from(status.contains("a: 1")) + usize::from(status.contains("a: \"x\""))
    };

    assert_eq!(
        claimed, reflected,
        "{claimed} processes were told they succeeded and {reflected} \
         injections are accounted for. D49: a pair that jointly meets to \
         bottom is KEPT and the fold reports bottom at the coordinate -- \
         it is never silently one of the two.\nstatus:\n{status}"
    );
}

// ── G1 ── SPEC_10 2.2.1 must keep holding for the sequential case ────────
//
// GREEN today and MUST STAY GREEN. When the meet with the existing set is
// bottom, the evolution MUST NOT happen. Today's behaviour is compliant:
// the second evolve exits non-zero and the working set keeps the first.
//
// C validates before it writes. If this goes red, C skipped that step and
// turned a ruled MUST NOT into a fold-time surprise.
#[test]
fn g1_a_sequential_conflict_is_still_refused() {
    let d = scratch("g1");
    std::fs::write(d.join("p.n"), "a: 1\n").unwrap();
    std::fs::write(d.join("q.n"), "a: \"x\"\n").unwrap();
    assert!(
        evolve_claims_success(&d, "p.n"),
        "REACH: the first evolve failed, so the second proves nothing"
    );
    assert!(
        !evolve_claims_success(&d, "q.n"),
        "SPEC_10 2.2.1: meeting to bottom means the evolution MUST NOT \
         happen. The second evolve reported success."
    );
    let status = working_set(&d);
    assert!(
        status.contains("a: 1") && !status.contains("a: \"x\""),
        "the refused injection must not be in the working set. status:\n{status}"
    );
}

// ── G2 ── this arc does not touch the circle layer ───────────────────────
//
// GREEN today and MUST STAY GREEN. D48 split savepoint identity and order
// into Q-014b. SPEC_10 3.1's two MUSTs (a savepoint survives commit, and
// carries a decidable order) bind there. An injection is not a savepoint,
// and if this arc quietly merges the two, that ruling is broken in passing.
#[test]
fn g2_the_circle_layer_is_left_alone() {
    let d = scratch("g2");
    std::fs::write(d.join("s.n"), "k: 7\n").unwrap();
    assert!(evolve_claims_success(&d, "s.n"), "REACH: evolve failed");
    let log = d.join(".oo").join("savepoints").join("LOG");
    assert!(
        log.exists(),
        "the savepoint LOG is gone. Q-014b owns that file, not this arc."
    );
    let before = std::fs::read_to_string(&log).unwrap();
    oo(&d, &["commit", "-m", "s"]);
    assert!(
        log.exists(),
        "SPEC_10 3.1: a savepoint MUST survive commit (D43). It did not."
    );
    assert_eq!(
        before,
        std::fs::read_to_string(&log).unwrap(),
        "commit rewrote the savepoint LOG. This arc must not touch it."
    );
}

// ── G3 ── the horizon parameter keeps its session lifetime ───────────────
//
// GREEN today and MUST STAY GREEN. Commit strips `~%Config` from the
// committed meet and re-stages it, so fuel set in a session outlives the
// commit (O37). Recon Q18 names this as the thing C most easily breaks:
// "fold, then clear the injection directory" takes the Config injection
// with it and the next observation drops back to genesis fuel.
#[test]
fn g3_config_outlives_the_commit() {
    let d = scratch("g3");
    std::fs::write(d.join("c.n"), "~%Config.fuel: 12345\n").unwrap();
    assert!(evolve_claims_success(&d, "c.n"), "REACH: evolve failed");
    assert!(
        working_set(&d).contains("12345"),
        "REACH: the horizon parameter never reached the working set"
    );
    oo(&d, &["commit", "-m", "c"]);
    let after = working_set(&d);
    assert!(
        after.contains("12345"),
        "O37: `~%Config` is session-scoped and must survive the commit that \
         clears the working set. status after commit:\n{after}"
    );
}

// ── G4 ── identity does not move ─────────────────────────────────────────
//
// GREEN today and MUST STAY GREEN. An all-solid universe must keep its
// root and its object count: injections live beside the store, never in
// `.oo/objects/`. Putting the working set into CAS would move both.
#[test]
fn g4_identity_is_a_red_line() {
    let d = scratch("g4");
    std::fs::write(d.join("x.n"), "x: 0\n").unwrap();
    assert!(evolve_claims_success(&d, "x.n"), "REACH: evolve failed");
    let out = oo(&d, &["commit", "-m", "x"]);
    let commit = out
        .split_whitespace()
        .find(|w| w.starts_with("hash:"))
        .unwrap_or_default()
        .to_string();
    assert!(!commit.is_empty(), "REACH: commit produced no hash: {out}");

    let objects = d.join(".oo").join("objects");
    let count = walkdir_count(&objects);
    assert_eq!(count, 3, "`x: 0` must hold exactly 3 CAS objects, found {count}");

    let meta = oo(&d, &["inspect", &commit]);
    assert!(
        meta.contains(X0_ROOT),
        "the root of `x: 0` moved. Expected {X0_ROOT}\ngot:\n{meta}"
    );
    assert!(
        oo(&d, &["status"]).contains(STD_ROOT),
        "the standard root moved."
    );
}

fn walkdir_count(dir: &Path) -> usize {
    let mut n = 0;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                n += walkdir_count(&p);
            } else {
                n += 1;
            }
        }
    }
    n
}
