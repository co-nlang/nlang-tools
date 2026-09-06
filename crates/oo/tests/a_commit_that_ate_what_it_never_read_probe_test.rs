// A commit that ate what it never read.
// Recon:  nlang-tools/docs/twenty_commits_and_one_survivor_recon.md
// Order:  nlang-tools/docs/a_commit_that_ate_what_it_never_read_handover.md
// Rulings: none needed. SPEC_10 3 clause 2 + D48 give axis C; SPEC_10 2.2.1
//          gives axis A. Axis B (HEAD topology) is NOT this arc.
//
// -- What this arc is ----------------------------------------------------
//
// A commit reads the working set at universe.rs:1000 and deletes it at
// :1197, and what it deletes is not what it read:
//
//     pub fn clear(base: &Path) -> Result<()> {
//         for p in paths(base)? { let _ = fs::remove_file(p); }
//         let _ = fs::remove_dir(&d);
//     }
//
// `clear` takes no set. It lists the directory at call time. Anything
// injected in the two hundred lines between those points is on disk, is
// not in the fold, and is then unlinked. The definition reaches neither
// the root nor the working set. Measured: three rounds of twenty parallel
// workers left 20, 16 and 19 of 20 coordinates in the surviving root.
//
// The same TOCTOU is where the operator-facing messages go wrong. When a
// member vanishes between `paths()` and `read_to_string`, or the
// directory between `exists()` and `read_dir`, the error reaches the CLI
// as a bare `No such file or directory (os error 2)` -- an implementation
// representation, which 2.2.1 forbids, with no coordinate and no code.
// When the directory is already gone at load time, the same commit says
// `Nothing to commit`, which is false: there was something, and another
// process consumed it.
//
// Neither needs a ruling. A commit consuming what it never folded
// contradicts "staged is the SET of definitions injected since the last
// commit" and the whole reason D48 made it a set. A refusal that leaks an
// errno and names nothing contradicts 2.2.1 directly.
//
// -- Arming, and why every probe here checks it --------------------------
//
// Both axes need two processes, so every red here can go green by never
// reaching the contested state. That is the failure this session hit
// three times (a `--help` count drowned in test names, twenty rollbacks
// that all exited 1, a background run from the wrong directory reporting
// zero targets). So each probe asserts it ARRIVED before it asserts
// anything else, and says so in its failure text. A round that did not
// arm is a void reading and fails as one -- it never passes.
//
// NOT PROBED, stated so no one mistakes silence for coverage:
//   * Axis B. Whether concurrent commits should converge, fork or refuse
//     is unruled, and nothing here presumes an answer. `oo log` reaching
//     only one of several successful commits is NOT asserted either way.
//   * The success rate under contention. It was 7/10/13 of 20 in the
//     recon and 20/17/3 in the acceptor's runs. No probe may pin it.

use std::path::Path;
use std::process::{Child, Command, Stdio};

fn cmd(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    c
}

fn oo(dir: &Path, args: &[&str]) -> (String, i32) {
    let o = cmd(dir).args(args).output().expect("oo runs");
    (
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        o.status.code().unwrap_or(-1),
    )
}

fn spawn(dir: &Path, args: &[&str]) -> Child {
    cmd(dir)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("oo spawns")
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("ate-{tag}"))
}

fn write(d: &Path, name: &str, body: &str) {
    std::fs::write(d.join(name), body).expect("write source");
}

fn injections(d: &Path) -> usize {
    match std::fs::read_dir(d.join(".oo").join("injections")) {
        Ok(rd) => rd
            .flatten()
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .count(),
        Err(_) => 0,
    }
}

fn objects(d: &Path) -> usize {
    let mut n = 0;
    let mut stack = vec![d.join(".oo").join("objects")];
    while let Some(p) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&p) {
            for e in rd.flatten() {
                let q = e.path();
                if q.is_dir() {
                    stack.push(q);
                } else {
                    n += 1;
                }
            }
        }
    }
    n
}

// ---------------------------------------------------------------------
// R1. Axis C, and the nail. Every definition whose evolve succeeded must
// be somewhere afterwards -- in a committed root, or still in the working
// set for the next commit. It may not be deleted unread.
//
// The shape separates the roles rather than trying to win a timing race:
// ten processes that only evolve, ten that only commit, started
// together. That aims straight at the window between the fold and the
// clear, and it lands -- measured four rounds out of four, losing 7, 1,
// 3 and 2 of ten coordinates. It is still a race, so it is armed
// explicitly: every evolve must have succeeded and at least one commit
// must have, or the round proves nothing.
// ---------------------------------------------------------------------
#[test]
fn r1_a_definition_that_was_accepted_is_never_deleted_unread() {
    let n = 10;
    let mut armed = 0;
    for round in 0..3 {
        let s = scratch(&format!("r1-{round}"));
        let d = s.path();
        write(d, "base.n", "x: 0\n");
        assert_eq!(oo(d, &["evolve", "base.n"]).1, 0, "REACH: base evolve");
        assert_eq!(oo(d, &["commit", "-m", "base"]).1, 0, "REACH: base commit");
        write(d, "seed.n", "seed: 1\n");
        assert_eq!(
            oo(d, &["evolve", "seed.n"]).1,
            0,
            "REACH: something to commit"
        );

        for i in 0..n {
            write(d, &format!("e{i}.n"), &format!("e{i}: {i}\n"));
        }
        let mut kids = Vec::new();
        for i in 0..n {
            let f = format!("e{i}.n");
            let mut c = cmd(d);
            c.args(["evolve", &f])
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            kids.push(c.spawn().expect("spawn evolve"));
        }
        let mut committers = Vec::new();
        for i in 0..n {
            let m = format!("c{i}");
            let mut c = cmd(d);
            c.args(["commit", "-m", &m])
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            committers.push(c.spawn().expect("spawn commit"));
        }
        let mut evolved = 0;
        for mut k in kids {
            if k.wait().expect("evolve exits").success() {
                evolved += 1;
            }
        }
        let mut committed = 0;
        for mut c in committers {
            if c.wait().expect("commit exits").success() {
                committed += 1;
            }
        }
        if evolved != n || committed == 0 {
            continue; // did not arm: no accepted definition, or no clear ran
        }
        armed += 1;

        // Everything the engine accepted must still be reachable.
        let (status, _) = oo(d, &["status"]);
        let (log, _) = oo(d, &["log"]);
        let head = log
            .lines()
            .find(|l| l.starts_with("commit "))
            .and_then(|l| l.split_whitespace().nth(1))
            .unwrap_or("")
            .to_string();
        let (insp, _) = oo(d, &["inspect", &head]);
        let root = insp
            .lines()
            .find(|l| l.starts_with("root:"))
            .map(|l| l.trim_start_matches("root:").trim().to_string())
            .unwrap_or_default();
        let (rootval, _) = oo(d, &["inspect", &root]);

        let mut lost = Vec::new();
        for i in 0..n {
            let coord = format!("e{i}: {i}");
            if !rootval.contains(&coord) && !status.contains(&coord) {
                lost.push(format!("e{i}"));
            }
        }
        assert!(
            lost.is_empty(),
            "round {round}: {} of {n} accepted definitions are in neither \
             the committed root nor the working set: {lost:?}. Their \
             evolves exited 0, so the engine took them. A commit deleted \
             members it never folded -- `clear` lists the directory at \
             call time, while the fold read it two hundred lines earlier.\n\
             --- status ---\n{status}\n--- root ---\n{rootval}",
            lost.len()
        );
    }
    assert!(
        armed > 0,
        "VOID READING: no round both accepted every definition and \
         completed a commit, so nothing contended and this probe proves \
         nothing."
    );
}

// ---------------------------------------------------------------------
// R2. Axis A. A bare OS error must never reach the operator. 2.2.1
// forbids leaking implementation representation and requires a
// coordinate and a code.
//
// Arming: the round ran twenty overlapping commits. Failures are not
// required — a queued commit that succeeds is still a contended round.
// ---------------------------------------------------------------------
#[test]
fn r2_no_raw_os_error_reaches_the_operator() {
    let mut armed = 0;
    for round in 0..3 {
        let s = scratch(&format!("r2-{round}"));
        let d = s.path();
        write(d, "seed.n", "seed: 0\n");
        assert_eq!(oo(d, &["evolve", "seed.n"]).1, 0, "REACH: seed evolve");
        assert_eq!(oo(d, &["commit", "-m", "base"]).1, 0, "REACH: seed commit");

        let n = 20;
        for i in 0..n {
            write(d, &format!("f{i}.n"), &format!("k{i}: {i}\n"));
        }
        let mut kids = Vec::new();
        for i in 0..n {
            let f = format!("f{i}.n");
            let m = format!("c{i}");
            let mut c = cmd(d);
            c.args(["evolve", &f])
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            kids.push((c.spawn().expect("spawn evolve"), m));
        }
        let mut commits = Vec::new();
        for (mut k, m) in kids {
            k.wait().expect("evolve exits");
            commits.push(spawn(d, &["commit", "-m", &m]));
        }
        let mut outs = Vec::new();
        for c in commits {
            let o = c.wait_with_output().expect("commit exits");
            outs.push(format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ));
        }
        armed += 1;
        for o in &outs {
            assert!(
                !o.to_lowercase().contains("os error"),
                "round {round}: a commit handed the operator a raw OS \
                 error. 2.2.1 forbids leaking implementation \
                 representation and requires the coordinate and a code. \
                 It said:\n{o}"
            );
        }
    }
    assert!(
        armed > 0,
        "VOID READING: no round ran overlapping commits, so nothing \
         contended and this probe proves nothing."
    );
}

// ---------------------------------------------------------------------
// R3. Axis A. A commit whose own evolve succeeded, and whose working set
// was then consumed by someone else, may not report that there was
// nothing to commit. That is a false statement about what happened, not
// merely an unhelpful one.
// ---------------------------------------------------------------------
#[test]
fn r3_a_consumed_working_set_is_not_reported_as_empty() {
    let mut armed = 0;
    for round in 0..3 {
        let s = scratch(&format!("r3-{round}"));
        let d = s.path();
        write(d, "seed.n", "seed: 0\n");
        assert_eq!(oo(d, &["evolve", "seed.n"]).1, 0, "REACH: seed evolve");
        assert_eq!(oo(d, &["commit", "-m", "base"]).1, 0, "REACH: seed commit");

        let n = 20;
        for i in 0..n {
            write(d, &format!("f{i}.n"), &format!("k{i}: {i}\n"));
        }
        let mut evolved = 0;
        let mut kids = Vec::new();
        for i in 0..n {
            let f = format!("f{i}.n");
            let mut c = cmd(d);
            c.args(["evolve", &f])
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            kids.push((c.spawn().expect("spawn evolve"), format!("c{i}")));
        }
        let mut commits = Vec::new();
        for (mut k, m) in kids {
            if k.wait().expect("evolve exits").success() {
                evolved += 1;
            }
            commits.push(spawn(d, &["commit", "-m", &m]));
        }
        assert!(evolved > 0, "REACH: at least one evolve landed");

        let mut failed = 0;
        let mut said_empty = Vec::new();
        for c in commits {
            let o = c.wait_with_output().expect("commit exits");
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            if o.status.code().unwrap_or(-1) != 0 {
                failed += 1;
            }
            if text.contains("Nothing to commit") {
                said_empty.push(text);
            }
        }
        if evolved == 0 {
            continue;
        }
        armed += 1;
        assert!(
            said_empty.is_empty(),
            "round {round}: {} of {n} commits said there was nothing to \
             commit, in a workspace where {evolved} evolves had just \
             succeeded and {failed} commits failed. Something was there; \
             another process consumed it. First one said:\n{}",
            said_empty.len(),
            said_empty[0]
        );
    }
    assert!(
        armed > 0,
        "VOID READING: no round accepted an evolve, so nothing \
         contended and this probe proves nothing."
    );
}

// ---------------------------------------------------------------------
// G1. Identity is a red line.
// ---------------------------------------------------------------------
#[test]
fn g1_identity_is_a_red_line() {
    let s = scratch("g1");
    let d = s.path();
    write(d, "a.n", "x: 0\n");
    assert_eq!(oo(d, &["evolve", "a.n"]).1, 0);
    assert_eq!(oo(d, &["commit", "-m", "identity"]).1, 0);
    assert_eq!(objects(d), 3, "a solid `x: 0` universe is three objects");
    let (out, _) = oo(d, &["status"]);
    assert!(
        out.contains("7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911"),
        "the standard root digest must not move; status said:\n{out}"
    );
}

// ---------------------------------------------------------------------
// G2. The sequential contract is untouched: a commit still consumes the
// members it folded, and the working set is empty afterwards.
// ---------------------------------------------------------------------
#[test]
fn g2_a_commit_still_consumes_what_it_folded() {
    let s = scratch("g2");
    let d = s.path();
    write(d, "a.n", "x: 1\n");
    assert_eq!(oo(d, &["evolve", "a.n"]).1, 0);
    write(d, "b.n", "y: 2\n");
    assert_eq!(oo(d, &["evolve", "b.n"]).1, 0);
    assert_eq!(injections(d), 2, "REACH: two members staged");
    assert_eq!(oo(d, &["commit", "-m", "both"]).1, 0);
    assert_eq!(injections(d), 0, "a commit consumes what it folded");
    let (out, _) = oo(d, &["status"]);
    assert!(
        out.contains("Universe is static"),
        "and the working set is empty; status said:\n{out}"
    );
}

// ---------------------------------------------------------------------
// G3. The honest empty case keeps its sentence. Whatever R3 gets fixed
// into must not cost a genuinely empty workspace its plain answer.
// ---------------------------------------------------------------------
#[test]
fn g3_a_genuinely_empty_workspace_still_says_so() {
    let s = scratch("g3");
    let d = s.path();
    write(d, "a.n", "x: 1\n");
    assert_eq!(oo(d, &["evolve", "a.n"]).1, 0);
    assert_eq!(oo(d, &["commit", "-m", "one"]).1, 0);
    let (out, rc) = oo(d, &["commit", "-m", "again"]);
    assert_eq!(rc, 1, "a second commit with nothing staged is refused");
    assert!(
        out.contains("Nothing to commit"),
        "and says so plainly; it said:\n{out}"
    );
}
