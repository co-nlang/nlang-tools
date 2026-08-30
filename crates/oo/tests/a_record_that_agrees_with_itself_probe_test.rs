// A record that agrees with itself.
// Rulings: nlang-spec/meta/oo/STATUS.md D50, D51 (and D43, D47 they amend)
//          (generalisation: meta/oo/commit.md 1.11, load-bearing sentence 12)
// Recon:   nlang-tools/docs/a_record_that_agrees_with_itself_recon.md
//          (+ appendix 1, plural parents)
// Order:   nlang-tools/docs/a_record_that_agrees_with_itself_handover.md
//
// -- What this arc is ----------------------------------------------------
//
// Q-014 gave the working set forty of forty. The circle layer was left
// alone on purpose, and the gap that opened is the point of this arc:
// forty concurrent evolves mint six to ten circles, and the LOG that
// records them is contiguous, duplicate-free and ORPHAN-FREE. It is a
// record that agrees with itself and disagrees with the values.
//
// It agrees with itself because colliding processes read the same count,
// mint the same id, and OVERWRITE each other. An overwrite leaves no
// orphan. Any assertion that checks the LOG against itself is stably
// green on this defect -- not accidentally green, stably.
//
// -- The two rulings that shape it ---------------------------------------
//
// D50: a savepoint's predecessors are PLURAL. `parent` singular came
// across from `Commit` when the field moved (commit.md 1.7.2); the
// control-plane spec REAL_01 section 2 has said `"parents": [...]` since
// before that move, and the Rust struct is the half that never agreed.
// Order is not a total order: it is a covering relation. A bare partial
// order tells you x precedes y; it does not tell you which edge to draw.
//
// D51: a circle is minted when the covering relation changes, even if the
// combo bytes did not move. Two writers converging on the same value is
// TWO events. Today it is one: measured, two concurrent evolves of the
// SAME file leave two injection files and one circle.
//
// -- The trap D51 sets, and the green that guards it ----------------------
//
// D51 must NOT be read as "every successful evolve mints a circle". A
// sequential no-op evolve produces a candidate whose only parent is the
// current unique tip and whose combo equals that tip's combo. That adds
// nothing and must not mint, or every no-op grows the directory forever --
// the fork-bomb the `fork()` reading warns about. The dedup rule is:
//
//   skip iff the candidate's parents are exactly one tip T
//   AND the candidate's combo equals T's combo.
//
// G1 is that guard and it is GREEN today. If it goes red, D51 was
// implemented as "always mint" and the growth curve is unbounded.
//
// -- Out of scope, do not touch ------------------------------------------
//
//   * compare-and-swap and retry (Q-016). Forking is DATA under D50, not
//     an error to be squeezed out.
//   * Observation writing a circle, and `oo run`/`eval` seeing the
//     committed universe (Q-018). Recon Q6: even wired to a universe, the
//     current `record(combo)` signature cannot express what an observation
//     reduced. SPEC_10 3.1 clauses (b) and bottom stay UNMET this arc and
//     the order says so verbatim.
//   * `Commit.parent` retiring, wal/audit (Q-015, arc A items A2/A3).
//   * Garbage-collecting circles. It collides with D43 (durable at birth).
//     The growth cost is real and the order asks for a measurement, not a
//     fix.
//
// -- Probe integrity ------------------------------------------------------
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason
// stated in each. The delivery may remove `#[ignore]` and NOTHING else in
// this file -- `rustfmt` included. A fmt pass over this file makes "the
// rest of the file is untouched" a false sentence, which is what happened
// last arc. If a pin here is wrong, say so in the report; do not edit it.
//
// NOT PROBED, stated so no one mistakes silence for coverage:
//   * D51's merge case at CLI level. Two concurrent evolves of the same
//     file legitimately produce EITHER two circles (a real fork) OR one
//     (the second process saw the first's circle, so its candidate was a
//     linear no-op). Both are correct; which one happens depends on
//     scheduling. A count assertion here would pin the scheduler. The
//     order asks for a runnable demonstration in writing instead (Q3).
//   * Crash windows. Every finding about them came from reconstructing
//     state with `cp`, not from real crash injection; a probe that fakes a
//     crash pins the faking. Recon Q10 measured both windows by hand.
//   * The on-disk spelling of `parents:`. R2 asserts a line exists and
//     that the DAG has one root; it does not pin separators or padding.
//
// Baseline measured 2026-08-30 against the v0.39.0 tag build
// (known-answer `~%Math./add (1,2)` -> `3`): 4 green, 2 red. Each red
// fails at ITS OWN assertion, not at a REACH guard.
//
// R1's width is calibrated, not chosen. At forty processes it is red on
// the RELEASE build every time (circles 6, 8, 10 against 40) but went
// GREEN ONE RUN IN FIVE under `cargo test`, because a debug `oo` spends
// most of its life starting up and the read-modify-write window is a
// small fraction of it. A red that is only red four times in five is not
// a pin: run it once, see green, conclude nothing needs doing. At one
// hundred and twenty it was red twelve times out of twelve, worst margin
// 115 of 120. If you widen or narrow it, say so in the report and give
// the runs.

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

fn oo_ok(dir: &Path, args: &[&str]) -> bool {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    c.args(args).status().expect("oo runs").success()
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("agrees-{tag}"))
}

/// Every regular file under `.oo/savepoints/` that is not `LOG` and does
/// not start with a dot. Deliberately reads the DIRECTORY and not `LOG`:
/// `LOG` is the second truth this arc is allowed to remove, and pinning it
/// would pin the very shape under discussion.
fn circles(d: &Path) -> Vec<std::path::PathBuf> {
    let dir = d.join(".oo").join("savepoints");
    let mut out: Vec<std::path::PathBuf> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .filter(|p| {
                let n = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                n != "LOG" && !n.starts_with('.')
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    out.sort();
    out
}

/// The `parents:` line of one circle, if the frame carries one.
/// `None` means the frame has no such line at all -- which is the whole of
/// today's baseline, and what R2 is red on.
fn parents_line(p: &Path) -> Option<String> {
    let text = std::fs::read_to_string(p).ok()?;
    text.lines()
        .take_while(|l| !l.trim_start().starts_with('{'))
        .find(|l| l.trim_start().starts_with("parents:"))
        .map(|l| l.trim().to_string())
}

/// Spawn one `oo evolve` per source, all at once; count the successes.
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
        .map(|k| k.wait().expect("wait").success())
        .filter(|ok| *ok)
        .count()
}

const STD_ROOT: &str = "7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911";
const X0_ROOT: &str = "31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a";

// ── R1 ── a lattice move that left no circle is a lie ────────────────────
//
// RED at the baseline. N concurrent evolves, N DISTINCT fields, so no two
// conflict and every one of them genuinely moves the position on the
// lattice -- D47 clause (a) fires N times. Measured on the v0.39.0 tag at
// N=40: injections 40/40/40 across three trials, circles 10, 8 and 6.
// N is 120 here for the reason in the header.
//
// The assertion is the EQUALITY, not the number forty. A count can be
// wrong for reasons outside this arc (a process failing to start). A
// lattice move that left no trace in the circle layer cannot.
#[test]
#[ignore = "RED at baseline: 120 successful lattice moves mint 96-115 circles \
            (counter-derived ids collide and overwrite). Remove when D50/D51 land."]
fn r1_every_lattice_move_leaves_a_circle() {
    const N: usize = 120;
    let d = scratch("r1");
    let mut files = Vec::new();
    for i in 0..N {
        let f = format!("e{i}.n");
        std::fs::write(d.join(&f), format!("f{i}: {i}\n")).unwrap();
        files.push(f);
    }
    let claimed = evolve_all_at_once(&d, &files);

    let status = oo(&d, &["status"]);
    // REACH: prove we are reading a working set at all before counting.
    assert!(
        status.contains("Staged changes"),
        "REACH: `oo status` did not report a working set, so nothing below \
         would be measuring anything. Got:\n{status}"
    );
    let reflected = (0..N)
        .filter(|i| status.contains(&format!("f{i}:")))
        .count();
    // REACH: the working set half is Q-014's, already shipped. If it
    // regressed, R1's real assertion would be measuring the wrong defect.
    assert_eq!(
        claimed, reflected,
        "REACH: the WORKING SET lost updates ({claimed} claimed, {reflected} \
         survive). That is Q-014's invariant and it shipped in v0.39.0; \
         this arc's assertion below assumes it holds."
    );

    let n = circles(&d).len();
    assert_eq!(
        claimed, n,
        "{claimed} evolves each moved the universe on the lattice and the \
         circle layer records {n} of them. D47 clause (a) says a move mints \
         a savepoint; under concurrency it does not, because `mint_id` \
         derives identity from a count it just read and colliding processes \
         overwrite one another. An overwrite leaves no orphan, which is why \
         the LOG stays self-consistent while this is happening."
    );
}

// ── R2 ── a circle must say where it came from ───────────────────────────
//
// RED at the baseline: no circle carries a `parents:` line at all
// (`encode_savepoint` is frame + combo, store_codec.rs:165-167).
//
// A genesis circle is minted FIRST so the directory is never empty when
// the forty race. That matters: with an empty directory every process
// legitimately derives an empty tip set, and several roots would be
// correct. With a genesis present, `ids != {} and tips == {}` is a
// corruption signal and exactly one root is the invariant.
#[test]
#[ignore = "RED at baseline: the savepoint frame has no `parents:` line. \
            Remove when D50 lands."]
fn r2_a_circle_declares_its_parents() {
    let d = scratch("r2");
    std::fs::write(d.join("g.n"), "g: 0\n").unwrap();
    assert!(
        oo_ok(&d, &["evolve", "g.n"]),
        "REACH: the genesis evolve failed, so there is no directory for the \
         race below to start from."
    );
    assert_eq!(
        circles(&d).len(),
        1,
        "REACH: the genesis evolve minted {} circles, not one; the root \
         count asserted below would not mean what it says.",
        circles(&d).len()
    );

    let mut files = Vec::new();
    for i in 0..40 {
        let f = format!("e{i}.n");
        std::fs::write(d.join(&f), format!("f{i}: {i}\n")).unwrap();
        files.push(f);
    }
    evolve_all_at_once(&d, &files);

    let cs = circles(&d);
    let without: Vec<String> = cs
        .iter()
        .filter(|p| parents_line(p).is_none())
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(
        without.is_empty(),
        "{} of {} circles carry no `parents:` line. D50: the predecessor is \
         plural and DECLARED. A bare order tells you x precedes y; it does \
         not tell you which edge to draw, which is why a hypothetical \
         `oo log --graph` cannot render today's chain. Missing: {:?}",
        without.len(),
        cs.len(),
        without
    );

    let roots = cs
        .iter()
        .filter(|p| {
            parents_line(p)
                .map(|l| l.trim_end_matches(':').trim() == "parents" || l == "parents:")
                .unwrap_or(false)
        })
        .count();
    assert_eq!(
        roots, 1,
        "the DAG has {roots} roots. One genesis circle existed before the \
         race, so `ids != {{}}` throughout: any further empty-parents circle \
         is a second genesis, disconnected from the history it was born \
         into. `ids != {{}} and tips == {{}}` must be refused, not treated \
         as an empty repository."
    );
}

// ── G1 ── the fork-bomb guard ────────────────────────────────────────────
//
// GREEN today and it MUST STAY GREEN. D51 says a merge mints even when the
// combo did not move; it does NOT say every successful evolve mints. A
// sequential no-op's candidate has exactly one parent -- the current
// unique tip -- and a combo equal to that tip's. It adds nothing.
//
// If this goes red, D51 was implemented as "always mint" and the directory
// grows without bound on repeated no-ops. Recon Q7: `savepoints/` is
// already O(N^2) in bytes, `oo gc` does not walk it, and no path deletes a
// circle.
#[test]
fn g1_a_sequential_no_op_mints_nothing() {
    let d = scratch("g1");
    std::fs::write(d.join("a.n"), "a: 1\n").unwrap();
    assert!(oo_ok(&d, &["evolve", "a.n"]), "REACH: first evolve failed");
    let after_first = circles(&d).len();
    assert_eq!(
        after_first, 1,
        "REACH: the first evolve minted {after_first} circles, not one."
    );

    assert!(oo_ok(&d, &["evolve", "a.n"]), "REACH: second evolve failed");
    assert_eq!(
        circles(&d).len(),
        1,
        "re-injecting an identical definition with nothing else in between \
         minted a second circle. Nothing happened: same combo, and the only \
         parent is the tip that already carries it. D47's dedup must survive \
         D51, or every no-op evolve grows a directory that has no GC."
    );
}

// ── G2 ── circles outlive the commit (D43) ───────────────────────────────
#[test]
fn g2_circles_outlive_the_commit() {
    let d = scratch("g2");
    std::fs::write(d.join("a.n"), "a: 1\n").unwrap();
    std::fs::write(d.join("b.n"), "b: 2\n").unwrap();
    assert!(oo_ok(&d, &["evolve", "a.n"]), "REACH: evolve a failed");
    assert!(oo_ok(&d, &["evolve", "b.n"]), "REACH: evolve b failed");
    let before = circles(&d).len();
    assert_eq!(before, 2, "REACH: expected two circles, got {before}");

    assert!(
        oo_ok(&d, &["commit", "-m", "one"]),
        "REACH: the commit failed, so nothing was tested about surviving it"
    );

    assert_eq!(
        circles(&d).len(),
        before,
        "the commit removed circles. SPEC_10 3.1 durability (MUST): every \
         savepoint is durable at birth and MUST continue to exist after a \
         commit. Persisted and committed are two different things."
    );
}

// ── G3 ── a sequential conflict is still refused (SPEC_10 2.2.1) ─────────
#[test]
fn g3_a_sequential_conflict_is_still_refused() {
    let d = scratch("g3");
    std::fs::write(d.join("a.n"), "a: 1\n").unwrap();
    std::fs::write(d.join("bad.n"), "a: \"x\"\n").unwrap();
    assert!(oo_ok(&d, &["evolve", "a.n"]), "REACH: first evolve failed");
    let before = circles(&d).len();

    let out = oo(&d, &["evolve", "bad.n"]);
    assert!(
        !oo_ok(&d, &["evolve", "bad.n"]),
        "a conflicting sequential evolve was accepted. SPEC_10 2.2.1 \
         (Core Requirement): when the meet is bottom the evolution MUST NOT \
         happen. Output was:\n{out}"
    );
    assert_eq!(
        circles(&d).len(),
        before,
        "a refused evolve minted a circle. Nothing moved and no covering \
         edge was added, so neither D47 clause fires."
    );
}

// ── G4 ── identity is a red line ─────────────────────────────────────────
//
// Circles are NOT content-addressed (SPEC_10 3.1 identity MUST NOT) and
// they do not live in `objects/`. If this arc moves either, an all-solid
// universe stops having three objects.
#[test]
fn g4_identity_is_a_red_line() {
    let d = scratch("g4");
    std::fs::write(d.join("x.n"), "x: 0\n").unwrap();
    assert!(oo_ok(&d, &["evolve", "x.n"]), "REACH: evolve failed");
    assert!(oo_ok(&d, &["commit", "-m", "x"]), "REACH: commit failed");

    let shard = d.join(".oo").join("objects").join("sha256");
    let mut digests: Vec<String> = Vec::new();
    for a in std::fs::read_dir(&shard).expect("sha256 dir").flatten() {
        if !a.path().is_dir() {
            continue;
        }
        let hi = a.file_name().to_string_lossy().to_string();
        for b in std::fs::read_dir(a.path()).expect("shard").flatten() {
            digests.push(format!("{hi}{}", b.file_name().to_string_lossy()));
        }
    }
    digests.sort();

    assert_eq!(
        digests.len(),
        3,
        "`x: 0` committed leaves {} objects, not three. A circle that became \
         content-addressed would show up exactly here. Got: {:?}",
        digests.len(),
        digests
    );
    assert!(
        digests.iter().any(|h| h == X0_ROOT),
        "the root of `x: 0` moved. Expected {X0_ROOT}, got {digests:?}"
    );
    assert!(
        digests.iter().any(|h| h == STD_ROOT),
        "the standard root moved. Expected {STD_ROOT}, got {digests:?}"
    );
}
