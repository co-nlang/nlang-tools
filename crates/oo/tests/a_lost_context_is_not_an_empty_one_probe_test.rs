// A lost context is not an empty one.
// Order: nlang-tools/docs/a_lost_context_is_not_an_empty_one_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-061.  Ruling: meta/oo/STATUS.md D79.
// Design note: nlang-spec/meta/oo/commit.md §1.12 (candidate).
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// SPEC_08 §6.2.1 roots gc at HEAD (then the ancestor chain, then the root
// trees; abandoned heads are NOT roots). Measured on v0.61.0 / v0.62.0: with
// `.oo/HEAD` absent in a store whose savepoints record commits, gc says
// "0 reachable", rc=0, and deletes every object. The same lost context also
// opens a second door: `status` says "no committed root yet", `log` prints
// nothing with rc=0, and `commit` succeeds by starting a new genesis chain —
// after which gc, now rooted at the new HEAD, lawfully deletes the old
// history.
//
// D79 (user, 2026-09-27, "丙′"): gc keeps §6.2.1 (abandoned heads stay
// collectable); HEAD unreadable OR absent while the savepoints record commits
// ⟹ a named refusal. commit.md §1.12: HEAD is the "point" layer of the
// context; commands that need a "now" (status, log, evolve, commit, gc)
// cannot answer without one. A lost context is not an empty one. The one way
// back — `rollback <commit> --grant rollback`, an explicit, privileged choice
// of a stage — measured working with HEAD absent, and must keep working.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// "Lost" is made by moving `.oo/HEAD` aside after two real commits (their
// savepoints record both). Wording-free except: the defect's own sentence
// ("no committed root yet"), and `rollback`, which the refusal must name as
// the way back (the command's name, not a phrasing).
//
// The delivery may NOT edit this file. If a pin is wrong, say so in the
// report. `rustfmt` must not touch it.
//
// Baseline measured 2026-09-27 on dev c8a95ec / oo v0.62.0: see the order.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("lost-ctx-{tag}"));
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

    fn committed(&self, name: &str, text: &str) {
        fs::write(self.ws.join(name), text).unwrap();
        self.ok(&["evolve", name]);
        self.ok(&["commit", "-m", name]);
    }

    fn head_path(&self) -> PathBuf {
        self.ws.join(".oo/HEAD")
    }

    fn head(&self) -> String {
        fs::read_to_string(self.head_path()).unwrap().trim().to_string()
    }

    fn objects(&self) -> usize {
        walk(&self.ws.join(".oo/objects"))
    }

    /// Two commits, then HEAD moved aside. Returns the lost HEAD.
    fn lost(tag: &str) -> (Self, String) {
        let w = Ws::new(tag);
        w.committed("a.n", "a: 1\n");
        w.committed("b.n", "b: 2\n");
        let h = w.head();
        let log = w.ok(&["log"]);
        if log.matches("commit hash:").count() != 2 {
            panic!("VOID READING: expected two commits before losing HEAD: {log}");
        }
        fs::rename(w.head_path(), w.root.join("HEAD.aside")).unwrap();
        (w, h)
    }

    fn restore(&self) {
        fs::rename(self.root.join("HEAD.aside"), self.head_path()).unwrap();
    }
}

fn walk(p: &Path) -> usize {
    fs::read_dir(p)
        .map(|d| d.flatten().map(|e| if e.path().is_dir() { walk(&e.path()) } else { 1 }).sum())
        .unwrap_or(0)
}

// ── Guards (green on v0.62.0, must stay green) ───────────────────────────

/// A workspace that never committed has no HEAD and nothing lost: it still
/// evolves and commits (the honest empty context keeps its answers).
#[test]
fn g1_a_workspace_that_never_committed_still_commits() {
    let w = Ws::new("g1");
    fs::write(w.ws.join("seed.n"), "seed: 0\n").unwrap(); // AMENDED 2026-09-30 for Q-064 (D82): only `evolve` creates a universe
    w.ok(&["evolve", "seed.n"]);
    w.ok(&["status"]);
    w.committed("a.n", "a: 1\n");
    w.ok(&["log"]);
}

/// §6.2.1 "abandoned is not a root" stays: after a rollback, gc still
/// collects the abandoned head's content (D79 is not 甲).
#[test]
fn g2_gc_still_collects_what_rollback_abandoned() {
    let w = Ws::new("g2");
    w.committed("a.n", "a: 1\n");
    let first = w.head();
    w.committed("b.n", "b: 2\n");
    w.ok(&["rollback", &first, "--grant", "rollback"]);
    let before = w.objects();
    w.ok(&["gc", "--grant", "gc"]);
    assert!(w.objects() < before, "gc no longer collects abandoned content (abandoned became a root?)");
    w.ok(&["log"]);
}

/// The way back: with HEAD absent, an explicit `rollback` to a named commit
/// restores a context, and the history is readable again.
#[test]
fn g3_rollback_is_the_way_back() {
    let (w, h) = Ws::lost("g3");
    w.ok(&["rollback", &h, "--grant", "rollback"]);
    let log = w.ok(&["log"]);
    assert_eq!(log.matches("commit hash:").count(), 2, "history after recovery: {log}");
}

/// A healthy gc keeps everything reachable.
#[test]
fn g4_a_healthy_gc_deletes_nothing() {
    let w = Ws::new("g4");
    w.committed("a.n", "a: 1\n");
    w.committed("b.n", "b: 2\n");
    let before = w.objects();
    w.ok(&["gc", "--grant", "gc"]);
    assert_eq!(w.objects(), before);
}

// ── Reds ─────────────────────────────────────────────────────────────────

/// Baseline: rc=0, "0 reachable", every object deleted; `log` after
/// restoring HEAD answers `CAID not found`.
#[test]
fn r1_gc_without_a_context_deletes_nothing() {
    let (w, _) = Ws::lost("r1");
    let before = w.objects();
    let (o, rc) = w.oo(&["gc", "--grant", "gc"]);
    let after = w.objects();
    w.restore();
    assert_eq!(after, before, "gc deleted objects with HEAD absent (rc={rc}): {o}");
    assert!(rc != 0, "gc with a lost context did not refuse: {o}");
    let (l, lrc) = w.oo(&["log"]);
    assert_eq!(lrc, 0, "history lost: {l}");
}

/// Baseline: rc=0, a new genesis commit; the next gc deletes the old history.
#[test]
fn r2_commit_does_not_start_a_new_history_over_a_lost_one() {
    let (w, h) = Ws::lost("r2");
    fs::write(w.ws.join("c.n"), "c: 3\n").unwrap();
    let _ = w.oo(&["evolve", "c.n"]);
    let (o, rc) = w.oo(&["commit", "-m", "c"]);
    let made_head = w.head_path().exists();
    assert!(rc != 0 && !made_head, "commit started a new history with HEAD lost (rc={rc}): {o}");
    let _ = w.oo(&["gc", "--grant", "gc"]);
    w.restore();
    let (l, lrc) = w.oo(&["log"]);
    assert!(lrc == 0 && l.contains(&h), "the old history did not survive: {l}");
}

/// Baseline: rc=0, "Standard root dependency: current (no committed root yet)".
#[test]
fn r3_status_does_not_say_there_is_no_history() {
    let (w, _) = Ws::lost("r3");
    let (o, rc) = w.oo(&["status"]);
    assert!(!o.contains("no committed root yet"), "status says there is no history: {o}");
    assert!(rc != 0, "status answered a lost context as if it were fine: {o}");
}

/// Baseline: rc=0 and nothing printed.
#[test]
fn r4_log_does_not_answer_an_empty_history() {
    let (w, _) = Ws::lost("r4");
    let (o, rc) = w.oo(&["log"]);
    assert!(rc != 0, "log printed an empty history for a lost context: [{o}]");
}

/// Baseline: rc=0 (the injection is staged against no point).
#[test]
fn r5_evolve_does_not_stage_against_no_point() {
    let (w, _) = Ws::lost("r5");
    fs::write(w.ws.join("c.n"), "c: 3\n").unwrap();
    let (o, rc) = w.oo(&["evolve", "c.n"]);
    assert!(rc != 0, "evolve staged against a lost context: {o}");
}

/// The refusal names the way back. Baseline: status says nothing of it.
#[test]
fn r6_the_refusal_names_the_way_back() {
    let (w, _) = Ws::lost("r6");
    let (o, _) = w.oo(&["status"]);
    assert!(o.contains("rollback"), "a lost context is refused without naming the way back: {o}");
}
