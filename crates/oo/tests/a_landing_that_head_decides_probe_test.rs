// A landing that HEAD decides.
// Order: nlang-tools/docs/a_landing_that_head_decides_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-066.  Ruling: meta/oo/STATUS.md D84.
// Design note: nlang-spec/meta/oo/commit.md §1.12 (HEAD is the one register
// in `.oo/` that is not monotone).
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// A commit lands in several durable steps (objects, HEAD, the commit
// circle, clearing the folded injections, clearing the abandoned record);
// a rollback in two (the abandoned record, HEAD). A crash can stop between
// any two. SPEC_10 §4.1 and REAL_01 §4.6 already require the commit to be
// all-or-nothing. Crash states, reconstructed from real files on v0.67.0:
//
//  S3  HEAD moved, circle written, a folded injection not cleared (Inbox
//      row 99): `status` lists the committed `x: 2` as staged; an unrelated
//      commit is asked for `--grant pin`, and `log` prints `pin` on it.
//  S5  a commit recorded the abandoned head; the record was not cleared:
//      the next commit records the same abandonment again.
//  S6  rollback wrote the abandoned record; HEAD did not move: the next
//      commit claims to have abandoned its own parent.
//  P   the circle written before HEAD moved (the order D84 asks for): the
//      proposal is still staged, gc collects the unlanded commit, history
//      is whole — already correct today (g1).
//
// D84 (user, 2026-10-01, 丁): HEAD is the only commit point; everything a
// landing needs is written to its final place before HEAD moves; what comes
// after is collection; every reader decides by HEAD — (i) an injection
// whose content is already at HEAD's position is not a proposal; (ii) an
// abandonment already recorded in HEAD's history is not recorded again;
// (iii) an "abandoned" head that is HEAD or one of its ancestors is void.
// Accepted cost: a new injection identical to what is committed (pinned
// or not) is a no-op, not a proposal.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// A crash is reconstructed by putting back, after a normal operation, the
// file the crash would have left. Wording-free except `pin` as the word
// `log` prints for a pinned commit, `Nothing to commit`, the flag
// `--grant pin`, and the field `abandoned:` in a commit object's own bytes.
//
// The delivery may NOT edit this file. If a pin is wrong, say so in the
// report. `rustfmt` must not touch it.
//
// Baseline measured 2026-10-01 on dev faa09d0 / oo v0.67.0: see the order.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("head-decides-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    fn oo(&self, args: &[&str]) -> (String, i32) {
        let o = Command::new(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .stdin(Stdio::null())
            .output()
            .expect("oo runs");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
            o.status.code().unwrap_or(-1),
        )
    }

    fn ok(&self, args: &[&str]) -> String {
        let (o, rc) = self.oo(args);
        assert_eq!(rc, 0, "setup `oo {}`: {o}", args.join(" "));
        o
    }

    fn write(&self, name: &str, text: &str) {
        fs::write(self.ws.join(name), text).unwrap();
    }

    fn committed(&self, name: &str, text: &str) {
        self.write(name, text);
        self.ok(&["evolve", name]);
        self.ok(&["commit", "-m", name]);
    }

    fn dot_oo(&self) -> PathBuf {
        self.ws.join(".oo")
    }

    fn head(&self) -> String {
        fs::read_to_string(self.dot_oo().join("HEAD")).unwrap().trim().to_string()
    }

    fn commit_object(&self, caid: &str) -> String {
        let hex = caid.rsplit(':').next().unwrap();
        let p = self.dot_oo().join("objects/sha256").join(&hex[..2]).join(&hex[2..]);
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("VOID READING: no commit object at {}: {e}", p.display()))
    }

    fn injections(&self) -> Vec<(String, Vec<u8>)> {
        let mut v: Vec<(String, Vec<u8>)> = fs::read_dir(self.dot_oo().join("injections"))
            .map(|d| {
                d.flatten()
                    .map(|e| (e.file_name().to_string_lossy().to_string(), fs::read(e.path()).unwrap()))
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }

    fn put_back(&self, saved: &[(String, Vec<u8>)]) {
        for (name, bytes) in saved {
            fs::write(self.dot_oo().join("injections").join(name), bytes).unwrap();
        }
    }

    /// `log` blocks: (commit caid, the lines under it).
    fn log(&self) -> Vec<(String, String)> {
        let o = self.ok(&["log"]);
        let mut out: Vec<(String, String)> = Vec::new();
        for line in o.lines() {
            if let Some(c) = line.strip_prefix("commit ") {
                out.push((c.trim().to_string(), String::new()));
            } else if let Some(last) = out.last_mut() {
                last.1.push_str(line.trim());
                last.1.push('\n');
            }
        }
        out
    }

    /// `x: 1` committed, then a pinned `x: 2` committed with its injection
    /// put back — the crash between HEAD moving and the folded injection
    /// being cleared (S3).
    fn s3(tag: &str) -> Self {
        let w = Ws::new(tag);
        w.committed("a.n", "x: 1\n");
        w.write("p.n", "x: 2\n");
        w.ok(&["evolve", "p.n", "--pin", "--grant", "pin"]);
        let saved = w.injections();
        if saved.len() != 1 {
            panic!("VOID READING: expected one pinned injection, got {}", saved.len());
        }
        w.ok(&["commit", "-m", "pinned", "--grant", "pin"]);
        if !w.injections().is_empty() {
            panic!("VOID READING: the commit did not clear its injection");
        }
        w.put_back(&saved);
        w
    }
}

// ── Guards (green on v0.67.0, must stay green) ───────────────────────────

/// P: the circle written, HEAD not moved. The commit did not happen: the
/// proposal is still staged, the next commit folds it, gc keeps history.
#[test]
fn g1_a_circle_written_before_head_moved_is_a_commit_that_did_not_happen() {
    let w = Ws::new("g1");
    w.committed("a.n", "a: 1\n");
    let first = w.head();
    w.write("x.n", "x: 9\n");
    w.ok(&["evolve", "x.n"]);
    let saved = w.injections();
    w.ok(&["commit", "-m", "x"]);
    fs::write(w.dot_oo().join("HEAD"), &first).unwrap();
    w.put_back(&saved);
    let st = w.ok(&["status"]);
    assert!(st.contains("x: 9"), "the proposal of a commit that did not land is gone:\n{st}");
    w.ok(&["commit", "-m", "x again"]);
    let _ = w.oo(&["gc", "--grant", "gc"]);
    let log = w.log();
    assert_eq!(log.len(), 2, "history after an unlanded commit and gc: {log:?}");
    assert!(log.iter().any(|(c, _)| c == &first), "the first commit is gone");
}

/// A pin that changes something is still a privileged proposal.
#[test]
fn g2_a_pin_that_changes_something_still_needs_the_grant() {
    let w = Ws::new("g2");
    w.committed("a.n", "x: 1\n");
    w.write("p.n", "x: 3\n");
    w.ok(&["evolve", "p.n", "--pin", "--grant", "pin"]);
    let (o, rc) = w.oo(&["commit", "-m", "p"]);
    assert!(rc != 0 && o.contains("--grant pin"), "an overwriting pin committed without its grant: rc={rc} {o}");
    w.ok(&["commit", "-m", "p", "--grant", "pin"]);
    assert!(w.log()[0].1.contains("pin"), "a pinned commit is not marked pin: {:?}", w.log()[0]);
}

/// A rollback followed by a commit records the abandonment once.
#[test]
fn g3_a_rollback_is_recorded_once() {
    let w = Ws::new("g3");
    w.committed("a.n", "a: 1\n");
    let first = w.head();
    w.committed("b.n", "b: 2\n");
    w.ok(&["rollback", &first, "--grant", "rollback"]);
    w.committed("c.n", "c: 3\n");
    assert!(w.commit_object(&w.head()).contains("abandoned:"), "the abandonment was not recorded");
    w.committed("d.n", "d: 4\n");
    assert!(!w.commit_object(&w.head()).contains("abandoned:"), "the abandonment was recorded twice");
}

/// A genuinely new proposal after a commit is still a proposal.
#[test]
fn g4_a_new_proposal_is_still_a_proposal() {
    let w = Ws::new("g4");
    w.committed("a.n", "x: 1\n");
    w.write("y.n", "y: 5\n");
    w.ok(&["evolve", "y.n"]);
    let st = w.ok(&["status"]);
    assert!(st.contains("y: 5"), "{st}");
    w.ok(&["commit", "-m", "y"]);
}

// ── Reds ─────────────────────────────────────────────────────────────────

/// S3. Baseline: `status` lists the committed `x: 2` as staged.
#[test]
fn r1_a_folded_injection_left_behind_is_not_a_proposal() {
    let w = Ws::s3("r1");
    let st = w.ok(&["status"]);
    assert!(!st.contains("x: 2"), "the injection HEAD already holds is listed as a proposal:\n{st}");
    // AMENDED 2026-10-03 for Q-068 (D86): this is the held-injection case
    // D86 gives its own answer (it names HEAD); "not a proposal" is measured
    // by the refusal and HEAD not moving, not by the words `Nothing to commit`.
    let before = w.head();
    let (o, rc) = w.oo(&["commit", "-m", "again"]);
    assert!(rc != 0 && w.head() == before, "a commit found something to commit in what HEAD already holds: rc={rc} {o}");
}

/// S3. Baseline: an unrelated commit is asked for `--grant pin`, and once
/// given, `log` prints `pin` on a commit that overwrote nothing.
#[test]
fn r2_an_unrelated_commit_is_not_privileged_by_a_leftover() {
    let w = Ws::s3("r2");
    w.write("y.n", "y: 5\n");
    w.ok(&["evolve", "y.n"]);
    let (o, rc) = w.oo(&["commit", "-m", "y"]);
    assert!(rc == 0, "an unrelated commit was refused because of a leftover pin: {o}");
    let top = &w.log()[0];
    assert!(!top.1.lines().any(|l| l.trim() == "pin"), "an unrelated commit is marked pin in history: {top:?}");
}

/// S3 with two proposals, one of them already folded (a partial clear).
/// Baseline: the folded one comes back as a proposal.
#[test]
fn r3_a_partial_clear_leaves_only_what_is_new() {
    let w = Ws::new("r3");
    w.committed("a.n", "x: 1\n");
    w.write("b.n", "b: 2\n");
    w.ok(&["evolve", "b.n"]);
    let saved = w.injections();
    w.ok(&["commit", "-m", "b"]);
    w.put_back(&saved);
    w.write("c.n", "c: 3\n");
    w.ok(&["evolve", "c.n"]);
    let st = w.ok(&["status"]);
    assert!(st.contains("c: 3"), "VOID READING: the new proposal is not staged:\n{st}");
    assert!(!st.contains("b: 2"), "the folded proposal came back next to the new one:\n{st}");
}

/// Accepted cost of D84: a new injection identical to what is committed is
/// a no-op. Baseline: a pinned identical injection asks for `--grant pin`.
#[test]
fn r4_an_injection_identical_to_head_is_not_a_proposal() {
    let w = Ws::new("r4");
    w.committed("a.n", "x: 2\n");
    w.write("p.n", "x: 2\n");
    let _ = w.oo(&["evolve", "p.n", "--pin", "--grant", "pin"]);
    // AMENDED 2026-10-03 for Q-068 (D86): as r1 — measured by the refusal
    // and HEAD not moving, not by the words `Nothing to commit`.
    let before = w.head();
    let (o, rc) = w.oo(&["commit", "-m", "same"]);
    assert!(rc != 0 && w.head() == before, "an injection identical to HEAD was committed as a proposal: rc={rc} {o}");
}

/// S5. Baseline: the next commit records the same abandonment again.
#[test]
fn r5_an_abandonment_already_in_history_is_not_recorded_again() {
    let w = Ws::new("r5");
    w.committed("a.n", "a: 1\n");
    let first = w.head();
    w.committed("b.n", "b: 2\n");
    w.ok(&["rollback", &first, "--grant", "rollback"]);
    let record = fs::read(w.dot_oo().join("abandoned")).expect("VOID READING: no abandoned record after rollback");
    w.committed("c.n", "c: 3\n");
    if !w.commit_object(&w.head()).contains("abandoned:") {
        panic!("VOID READING: the first commit after the rollback did not record it");
    }
    fs::write(w.dot_oo().join("abandoned"), &record).unwrap();
    w.committed("d.n", "d: 4\n");
    assert!(!w.commit_object(&w.head()).contains("abandoned:"), "the same abandonment was recorded a second time");
}

/// S6. Baseline: the next commit claims to have abandoned its own parent.
#[test]
fn r6_an_abandonment_of_head_itself_is_void() {
    let w = Ws::new("r6");
    w.committed("a.n", "a: 1\n");
    let first = w.head();
    w.committed("b.n", "b: 2\n");
    let second = w.head();
    w.ok(&["rollback", &first, "--grant", "rollback"]);
    fs::write(w.dot_oo().join("HEAD"), &second).unwrap();
    w.committed("c.n", "c: 3\n");
    let obj = w.commit_object(&w.head());
    let second_hex = second.rsplit(':').next().unwrap();
    assert!(!(obj.contains("abandoned:") && obj.contains(second_hex)), "a commit claims to have abandoned its own parent:\n{obj}");
}
