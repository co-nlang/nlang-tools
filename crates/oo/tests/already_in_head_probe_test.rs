// Already in HEAD.
// Order: nlang-tools/docs/already_in_head_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-068.  Ruling: meta/oo/STATUS.md D86.
// Spec: SPEC_10 §4.1.3 (the late-comer's working set was consumed; its
// "known gap": the report is true but does not say the content landed).
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// A commit can find that what it was given is already in history, two ways:
//
//  C1  another commit, waiting on the same lock, took the working set and
//      landed it. Today: rc=1 "working set consumed by a concurrent
//      commit" — true, but it does not say the content is in HEAD.
//  C2  the working set holds an injection whose content HEAD already holds
//      (D84 (i): not a proposal). Today: rc=1 "Nothing to commit" — the
//      same words a workspace that never had anything gets.
//
// D86 (user, 2026-10-03, 乙): the exit code stays non-zero (it is still a
// refusal, D63); the two cases answer with one sentence, and that sentence
// names the HEAD that holds the content. Only when the engine has verified
// that HEAD holds it. An honest empty working set keeps its answer
// (SPEC_10 §4.1.3 last clause) — including a stage with nothing
// committable in it (`v: _`, `~%Config` only), which D84 (i) also counts as
// held, trivially.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// C1 is made deterministic: the probe holds the commit lock (`.oo/format`,
// the same exclusive file lock the engine takes), starts two commits, waits
// until both have listed the working set and blocked, then releases.
// Wording-free: "names HEAD" = the 64-hex digest of the current HEAD
// appears in the output; "one sentence" = the two outputs are equal after
// each one's own HEAD is replaced by a placeholder; "keeps its answer" =
// equal to the answer of a workspace that commits twice.
//
// The delivery may NOT edit this file. If a pin is wrong, say so in the
// report. `rustfmt` must not touch it.
//
// Baseline measured 2026-10-03 on dev ac8714e / oo v0.69.0: see the order.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("already-in-head-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
        c.args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .stdin(Stdio::null());
        c
    }

    fn oo(&self, args: &[&str]) -> (String, i32) {
        let o = self.cmd(args).output().expect("oo runs");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
            o.status.code().unwrap_or(-1),
        )
    }

    fn spawn(&self, args: &[&str]) -> Child {
        self.cmd(args).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("oo spawns")
    }

    fn ok(&self, args: &[&str]) -> String {
        let (o, rc) = self.oo(args);
        assert_eq!(rc, 0, "setup `oo {}`: {o}", args.join(" "));
        o
    }

    fn write(&self, name: &str, text: &str) {
        fs::write(self.ws.join(name), text).unwrap();
    }

    fn evolved(&self, name: &str, text: &str) {
        self.write(name, text);
        self.ok(&["evolve", name]);
    }

    fn committed(&self, name: &str, text: &str) {
        self.evolved(name, text);
        self.ok(&["commit", "-m", name]);
    }

    fn head(&self) -> String {
        fs::read_to_string(self.ws.join(".oo/HEAD")).unwrap().trim().to_string()
    }

    fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        let mut m = BTreeMap::new();
        files(&self.ws.join(".oo"), &self.ws.join(".oo"), &mut m);
        m
    }
}

fn files(base: &Path, p: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    if let Ok(d) = fs::read_dir(p) {
        for e in d.flatten() {
            let q = e.path();
            if q.is_dir() {
                files(base, &q, out);
            } else {
                out.insert(q.strip_prefix(base).unwrap().display().to_string(), fs::read(&q).unwrap());
            }
        }
    }
}

fn hex_of(caid: &str) -> String {
    let h = caid.rsplit(':').next().unwrap().to_string();
    assert_eq!(h.len(), 64, "VOID READING: not a digest: {caid}");
    h
}

/// Replace this workspace's HEAD (full CAID, then bare digest) by a placeholder.
fn normalized(o: &str, head: &str) -> String {
    o.replace(head, "<HEAD>").replace(&hex_of(head), "<HEAD>")
}

fn finish(c: Child) -> (String, i32) {
    let o = c.wait_with_output().unwrap();
    (
        format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
        o.status.code().unwrap_or(-1),
    )
}

/// The answer of a workspace that commits, then commits again.
fn honest_empty() -> (String, i32) {
    let w = Ws::new("honest");
    w.committed("a.n", "a: 1\n");
    w.oo(&["commit", "-m", "again"])
}

/// C2: an injection whose content HEAD already holds.
fn held() -> (Ws, String, i32) {
    let w = Ws::new("held");
    w.committed("x.n", "x: 1\n");
    w.ok(&["evolve", "x.n"]);
    let (o, rc) = w.oo(&["commit", "-m", "again"]);
    (w, o, rc)
}

/// C1: two commits wait on the lock with the same one-member working set.
/// Returns the workspace and both answers.
fn raced(tag: &str) -> (Ws, Vec<(String, i32)>) {
    let w = Ws::new(tag);
    w.committed("a.n", "a: 1\n");
    w.evolved("x.n", "x: 1\n");
    let lock = fs::OpenOptions::new().read(true).write(true).open(w.ws.join(".oo/format")).unwrap();
    lock.lock().unwrap();
    let a = w.spawn(&["commit", "-m", "A"]);
    let b = w.spawn(&["commit", "-m", "B"]);
    std::thread::sleep(Duration::from_millis(1500));
    lock.unlock().unwrap();
    drop(lock);
    let out = vec![finish(a), finish(b)];
    (w, out)
}

// ── Red ──────────────────────────────────────────────────────────────────

/// r1 — C1: the commit that lost the race names the HEAD that holds its
/// working set. Baseline: "working set consumed by a concurrent commit".
#[test]
fn r1_the_loser_of_a_race_names_the_head_that_holds_its_content() {
    let (w, out) = raced("r1");
    let won: Vec<_> = out.iter().filter(|(_, rc)| *rc == 0).collect();
    let lost: Vec<_> = out.iter().filter(|(_, rc)| *rc != 0).collect();
    assert_eq!(won.len(), 1, "expected exactly one commit to land: {out:?}");
    let (o, _) = lost[0];
    let (empty, _) = honest_empty();
    assert_ne!(o, &empty, "VOID READING: the loser had not listed the working set before the winner cleared it: {out:?}");
    assert!(o.contains(&hex_of(&w.head())), "the loser does not name the HEAD that holds its content ({}): {o}", w.head());
}

/// r2 — C2: an injection HEAD already holds. The answer names HEAD, is a
/// refusal, writes nothing, and is not the honest-empty answer
/// (SPEC_10 §4.1.3: the two must be distinguishable).
/// Baseline: "Nothing to commit", the honest-empty answer.
#[test]
fn r2_content_already_held_names_head_and_is_not_honest_empty() {
    let w = Ws::new("r2");
    w.committed("x.n", "x: 1\n");
    w.ok(&["evolve", "x.n"]);
    let before = w.snapshot();
    let (o, rc) = w.oo(&["commit", "-m", "again"]);
    assert!(rc != 0, "a commit that records nothing answered rc=0: {o}");
    assert!(w.snapshot() == before, "the refused commit wrote: {o}");
    assert!(o.contains(&hex_of(&w.head())), "the answer does not name the HEAD that holds the content: {o}");
    let (empty, _) = honest_empty();
    assert_ne!(o, empty, "content already in HEAD answered as a working set that never had anything");
}

/// r3 — C1 and C2 answer with one sentence.
#[test]
fn r3_a_race_and_a_held_injection_answer_alike() {
    let (rw, out) = raced("r3");
    let (lost, _) = out.iter().find(|(_, rc)| *rc != 0).cloned().unwrap_or_else(|| panic!("no loser: {out:?}"));
    let (hw, held_o, _) = held();
    assert_eq!(
        normalized(&lost, &rw.head()),
        normalized(&held_o, &hw.head()),
        "the race and the held injection give two answers"
    );
}

// ── Green: what must not change ─────────────────────────────────────────

/// g1 — nothing committable over a HEAD answers as honest empty: `v: _`
/// and a `~%Config`-only stage (D84 (i) counts both as held, trivially).
#[test]
fn g1_nothing_committable_keeps_the_honest_empty_answer() {
    let (empty, erc) = honest_empty();
    for (tag, text) in [("top", "v: _\n"), ("cfg", "~%Config.fuel: 12345\n")] {
        let w = Ws::new(&format!("g1-{tag}"));
        w.committed("a.n", "a: 1\n");
        w.write("p.n", text);
        let _ = w.oo(&["evolve", "p.n"]);
        let (o, rc) = w.oo(&["commit", "-m", "x"]);
        assert_eq!((o.as_str(), rc), (empty.as_str(), erc), "{tag}: nothing committable must answer as honest empty");
        assert!(!o.contains(&hex_of(&w.head())), "{tag}: {o}");
    }
}

/// g2 — a member that vanished without landing: the engine must not claim
/// HEAD holds it. Made by removing the injection while the commit waits.
#[test]
fn g2_a_vanished_member_that_did_not_land_is_not_claimed() {
    let w = Ws::new("g2");
    w.committed("a.n", "a: 1\n");
    w.evolved("x.n", "x: 1\n");
    let lock = fs::OpenOptions::new().read(true).write(true).open(w.ws.join(".oo/format")).unwrap();
    lock.lock().unwrap();
    let c = w.spawn(&["commit", "-m", "B"]);
    std::thread::sleep(Duration::from_millis(1500));
    let inj = w.ws.join(".oo/injections");
    let mut n = 0;
    for e in fs::read_dir(&inj).unwrap().flatten() {
        fs::remove_file(e.path()).unwrap();
        n += 1;
    }
    assert_eq!(n, 1, "VOID READING: expected one injection to remove, found {n}");
    lock.unlock().unwrap();
    drop(lock);
    let (o, rc) = finish(c);
    assert!(rc != 0, "{o}");
    assert!(!o.contains(&hex_of(&w.head())), "claimed HEAD holds a member that never landed: {o}");
    let (hw, ho, _) = held();
    assert_ne!(normalized(&o, &w.head()), normalized(&ho, &hw.head()), "a vanished member was reported as already in HEAD");
}

/// g3 — a held injection beside a new one is an ordinary commit.
#[test]
fn g3_held_beside_new_commits() {
    let w = Ws::new("g3");
    w.committed("x.n", "x: 1\n");
    w.ok(&["evolve", "x.n"]);
    w.evolved("y.n", "y: 2\n");
    let before = w.head();
    let (o, rc) = w.oo(&["commit", "-m", "b"]);
    assert_eq!(rc, 0, "{o}");
    assert_ne!(w.head(), before, "{o}");
}

/// g4 — the winner of the race is an ordinary commit and the loser's
/// content is in it (control for r1).
#[test]
fn g4_the_winner_lands_the_shared_working_set() {
    let (w, out) = raced("g4");
    assert_eq!(out.iter().filter(|(_, rc)| *rc == 0).count(), 1, "{out:?}");
    let root = w
        .ok(&["inspect", &w.head()])
        .lines()
        .find_map(|l| l.strip_prefix("root:").map(|r| r.trim().to_string()))
        .unwrap();
    let r = w.ok(&["inspect", &root]);
    assert!(r.contains("x: 1"), "the winner did not land x: 1: {r}");
}
