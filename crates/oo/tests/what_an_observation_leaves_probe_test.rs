// What an observation leaves.
// Order: nlang-tools/docs/what_an_observation_leaves_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-073.  Ruling: meta/oo/STATUS.md D91.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// SPEC_10 §3.1: a savepoint is produced when (b) an observation really
// reduces a thunk, and an observation that collapses to `_|_` MUST produce
// one — otherwise `%cause` has nothing to hang on. The specification has
// said since 2026-09-27 that the reference engine does neither. Measured on
// v0.74.0: no one-shot observation (`eval`, `run`, `test`, `repl`) writes a
// savepoint, `_|_` included.
//
// D91 (user, 2026-10-04, 甲): every one-shot observation in a universe that
// reduces a thunk, answers `_|_`, or reaches a horizon writes one
// observation savepoint. ① It is a leaf on the context it stood on, never
// the predecessor of an injection or a commit. ② It holds the point (HEAD,
// read now), the question (path or expression, plus what was injected), and
// the answer. ③ The same point, question and answer already recorded ⟹
// nothing new: "an observation is idempotent" (the user). ④ Reaching a
// horizon counts. A new kind of record means `layout=9` (REAL_02 §5.1.1);
// a store declaring less receives none.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// The observation savepoint's format is the delivery's. This file reads
// only what layout 8 already fixes — a savepoint is a file under
// `.oo/savepoints/` (not `LOG`, not dot-files) with a `parents:` line — and
// otherwise counts files and looks for a 64-hex HEAD digest or a cause tag
// in a new file's bytes. Outputs are compared, not read.
//
// The delivery may NOT edit this file. If a pin is wrong, say so in the
// report. `rustfmt` must not touch it.
//
// Baseline measured 2026-10-04 on dev 342d9e6 / oo v0.74.0: see the order.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
    away: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("observation-leaves-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        let away = root.join("away");
        fs::create_dir_all(&ws).unwrap();
        fs::create_dir_all(&away).unwrap();
        Ws { _scratch: scratch, root, ws, away }
    }

    fn cmd(&self, dir: &Path, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
        c.args(args)
            .current_dir(dir)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"));
        c
    }

    fn oo_in(&self, dir: &Path, args: &[&str]) -> (String, i32) {
        let o = self.cmd(dir, args).stdin(Stdio::null()).output().expect("oo runs");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
            o.status.code().unwrap_or(-1),
        )
    }

    fn oo(&self, args: &[&str]) -> (String, i32) {
        self.oo_in(&self.ws, args)
    }

    fn ok(&self, args: &[&str]) -> String {
        let (o, rc) = self.oo(args);
        assert_eq!(rc, 0, "setup `oo {}`: {o}", args.join(" "));
        o
    }

    fn evolve_commit(&self, name: &str, text: &str) {
        fs::write(self.ws.join(name), text).unwrap();
        self.ok(&["evolve", name]);
        self.ok(&["commit", "-m", name]);
    }

    fn committed(tag: &str, text: &str) -> Self {
        let w = Ws::new(tag);
        w.evolve_commit("a.n", text);
        w
    }

    fn repl(&self, lines: &[&str]) -> (String, i32) {
        let mut child = self
            .cmd(&self.ws, &["repl"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("oo repl spawns");
        let mut input = lines.join("\n");
        input.push_str("\nexit\n");
        child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        let o = child.wait_with_output().unwrap();
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
            o.status.code().unwrap_or(-1),
        )
    }

    /// Savepoint files by name → bytes.
    fn circles(&self) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        if let Ok(d) = fs::read_dir(self.ws.join(".oo/savepoints")) {
            for e in d.flatten() {
                let p = e.path();
                let n = p.file_name().unwrap().to_string_lossy().to_string();
                if p.is_file() && n != "LOG" && !n.starts_with('.') {
                    m.insert(n, fs::read_to_string(&p).unwrap_or_default());
                }
            }
        }
        m
    }

    fn count(&self) -> usize {
        self.circles().len()
    }

    /// Files that appeared since `before`.
    fn new_since(&self, before: &BTreeMap<String, String>) -> Vec<(String, String)> {
        self.circles().into_iter().filter(|(k, _)| !before.contains_key(k)).collect()
    }

    fn head_hex(&self) -> String {
        let h = fs::read_to_string(self.ws.join(".oo/HEAD")).unwrap();
        h.trim().rsplit(':').next().unwrap().to_string()
    }

    fn snapshot(&self, dir: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut m = BTreeMap::new();
        let oo = dir.join(".oo");
        files(&oo, &oo, &mut m);
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

/// HEAD, injections, declarations: everything but objects and savepoints.
fn state(m: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, Vec<u8>> {
    m.iter()
        .filter(|(k, _)| !k.starts_with("objects/") && !k.starts_with("savepoints/"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

fn parents(body: &str) -> BTreeSet<String> {
    body.lines()
        .find_map(|l| l.strip_prefix("parents:"))
        .map(|s| s.split(|c: char| c == ',' || c.is_whitespace()).filter(|t| !t.is_empty()).map(String::from).collect())
        .unwrap_or_default()
}

fn prints(o: &str, v: &str) -> bool {
    o.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').any(|t| t == v)
}

const PROGRAM: &str = "a: 1\nv: 1 + 1\nw: a + 40\nb: 1 & 2\n/f: n -> (n == 0 ? 0 : 1 + /f (n - 1))\nbig: /f 100000\n";

// ── Red ──────────────────────────────────────────────────────────────────

/// r1 — an observation that reduces a thunk leaves one savepoint.
#[test]
fn r1_a_reducing_observation_leaves_one() {
    let w = Ws::committed("r1", PROGRAM);
    let n = w.count();
    let (o, rc) = w.oo(&["eval", "_.v"]);
    assert!(rc == 0 && prints(&o, "2"), "VOID READING: eval _.v: {o}");
    assert_eq!(w.count(), n + 1, "`eval _.v` reduced a thunk and left {} savepoints", w.count() - n);
}

/// r2 — an observation that answers `_|_` leaves one, holding its cause.
#[test]
fn r2_a_bottom_observation_leaves_one_with_its_cause() {
    let w = Ws::committed("r2", PROGRAM);
    let before = w.circles();
    let (o, _) = w.oo(&["eval", "_.b"]);
    assert!(o.contains("conflict"), "VOID READING: eval _.b: {o}");
    let new = w.new_since(&before);
    assert_eq!(new.len(), 1, "`eval _.b` (⊥) left {} savepoints", new.len());
    assert!(new[0].1.contains("conflict"), "the savepoint does not hold the cause: {}", new[0].1);
}

/// r3 — an observation that reaches a horizon counts.
#[test]
fn r3_a_horizon_observation_leaves_one() {
    let w = Ws::committed("r3", PROGRAM);
    let n = w.count();
    let (o, _) = w.oo(&["eval", "_.big"]);
    assert!(o.contains("blur") || o.contains("incomplete"), "VOID READING: eval _.big: {o}");
    assert_eq!(w.count(), n + 1, "an observation that reached its horizon left {} savepoints", w.count() - n);
}

/// r4 — an observation savepoint is a leaf on the context it stood on: it
/// names that tip as its predecessor, and the next injection still names
/// the tip, not the observation.
#[test]
fn r4_an_observation_is_a_leaf() {
    let w = Ws::committed("r4", PROGRAM);
    let tips_before = w.circles();
    let commit_circle = tips_before
        .iter()
        .find(|(k, _)| !tips_before.values().any(|b| parents(b).contains(*k)))
        .map(|(k, _)| k.clone())
        .expect("VOID READING: no tip after commit");
    w.oo(&["eval", "_.v"]);
    w.oo(&["eval", "_.w"]);
    let obs = w.new_since(&tips_before);
    assert_eq!(obs.len(), 2, "two reducing observations left {} savepoints", obs.len());
    for (k, b) in &obs {
        assert_eq!(parents(b), BTreeSet::from([commit_circle.clone()]), "observation {k} does not stand on the tip: {b}");
    }
    let before_evolve = w.circles();
    fs::write(w.ws.join("c.n"), "c: 3\n").unwrap();
    w.ok(&["evolve", "c.n"]);
    let inj = w.new_since(&before_evolve);
    assert_eq!(inj.len(), 1, "VOID READING: evolve left {} savepoints", inj.len());
    assert_eq!(parents(&inj[0].1), BTreeSet::from([commit_circle]), "the injection stands on an observation: {}", inj[0].1);
}

/// r5 — the point is HEAD, read when the observation happens.
#[test]
fn r5_the_point_is_head_read_now() {
    let w = Ws::committed("r5", PROGRAM);
    let before = w.circles();
    w.oo(&["eval", "_.v"]);
    let first = w.new_since(&before);
    assert_eq!(first.len(), 1, "{first:?}");
    assert!(first[0].1.contains(&w.head_hex()), "the observation does not record HEAD: {}", first[0].1);
    w.evolve_commit("c.n", "c: 3\n");
    let before = w.circles();
    w.oo(&["eval", "_.v"]);
    let second = w.new_since(&before);
    assert_eq!(second.len(), 1, "after a new commit, the same question left {} savepoints", second.len());
    assert!(second[0].1.contains(&w.head_hex()), "the observation does not record the new HEAD: {}", second[0].1);
}

/// r6 — an observation is idempotent: the same point, question and answer
/// leave nothing new; another question does.
#[test]
fn r6_an_observation_is_idempotent() {
    let w = Ws::committed("r6", PROGRAM);
    let n = w.count();
    w.oo(&["eval", "_.v"]);
    w.oo(&["eval", "_.v"]);
    assert_eq!(w.count(), n + 1, "the same observation twice left {} savepoints", w.count() - n);
    w.oo(&["eval", "_.w"]);
    assert_eq!(w.count(), n + 2, "another question left no savepoint");
    w.oo(&["eval", "_.v"]);
    w.oo(&["eval", "_.b"]);
    w.oo(&["eval", "_.b"]);
    assert_eq!(w.count(), n + 3, "repeating observations grew the record");
}

/// r7 — `run`, `test` and `repl` observe too.
#[test]
fn r7_run_test_and_repl_observe() {
    let w = Ws::committed("r7", PROGRAM);
    fs::write(w.ws.join("q.n"), "q: _.v + 40\n").unwrap();
    fs::write(w.ws.join("t.n"), "test_ok: _.v == 2\n").unwrap();
    let mut wrong = Vec::new();
    let n = w.count();
    let (o, _) = w.oo(&["run", "--observe", "q", "q.n"]);
    assert!(prints(&o, "42"), "VOID READING: run: {o}");
    if w.count() == n {
        wrong.push(format!("run left nothing: {o}"));
    }
    let n = w.count();
    let (o, rc) = w.oo(&["test", "t.n"]);
    assert_eq!(rc, 0, "VOID READING: test: {o}");
    if w.count() == n {
        wrong.push(format!("test left nothing: {o}"));
    }
    let n = w.count();
    let (o, _) = w.repl(&["z: _.w + 1"]);
    assert!(prints(&o, "42"), "VOID READING: repl: {o}");
    if w.count() == n {
        wrong.push(format!("repl left nothing: {o}"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// r8 — the question includes what was injected: two runs that observe the
/// same path to the same answer from different sources are two questions.
#[test]
fn r8_the_question_includes_what_was_injected() {
    let w = Ws::committed("r8", PROGRAM);
    fs::write(w.ws.join("p.n"), "q: 1 + 41\n").unwrap();
    fs::write(w.ws.join("r.n"), "q: 43 - 1\n").unwrap();
    let n = w.count();
    let (a, _) = w.oo(&["run", "--observe", "q", "p.n"]);
    let (b, _) = w.oo(&["run", "--observe", "q", "r.n"]);
    assert!(prints(&a, "42") && prints(&b, "42"), "VOID READING: {a} / {b}");
    assert_eq!(w.count(), n + 2, "two different questions with one answer left {} savepoints", w.count() - n);
}

/// r9 — a new store declares layout 9; `migrate` takes a layout-8
/// declaration there, and only then does it receive observations.
#[test]
fn r9_layout_nine_and_an_older_declaration_receives_none() {
    let w = Ws::committed("r9", PROGRAM);
    let fmt = w.ws.join(".oo/format");
    assert_eq!(fs::read_to_string(&fmt).unwrap().trim(), "layout=9", "a new store's layout");
    fs::write(&fmt, "layout=8\n").unwrap();
    let n = w.count();
    let (o, rc) = w.oo(&["eval", "_.v"]);
    assert!(rc == 0 && prints(&o, "2"), "a layout-8 store no longer answers: {o}");
    assert_eq!(w.count(), n, "a layout-8 store received an observation savepoint");
    let (o, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    assert_eq!(rc, 0, "migrate: {o}");
    assert_eq!(fs::read_to_string(&fmt).unwrap().trim(), "layout=9", "migrate did not reach layout 9: {o}");
    assert!(o.contains("v0.64.0") && o.contains("v0.74.0"), "the cost must name v0.64.0 through v0.74.0 (they open layout 8): {o}");
    w.oo(&["eval", "_.v"]);
    assert_eq!(w.count(), n + 1, "after migrate the observation left nothing");
}

/// r10 — an observation whose savepoint cannot be written does not report
/// success, and moves nothing.
#[test]
fn r10_an_unrecorded_observation_does_not_report_success() {
    let w = Ws::committed("r10", PROGRAM);
    let before = state(&w.snapshot(&w.ws));
    let dir = w.ws.join(".oo/savepoints");
    let mut perm = fs::metadata(&dir).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perm.set_mode(0o555);
    fs::set_permissions(&dir, perm.clone()).unwrap();
    let (o, rc) = w.oo(&["eval", "_.v"]);
    perm.set_mode(0o755);
    fs::set_permissions(&dir, perm).unwrap();
    assert_ne!(rc, 0, "an observation that could not be recorded reported success: {o}");
    assert!(state(&w.snapshot(&w.ws)) == before, "it moved HEAD or the working set");
}

// ── Green ────────────────────────────────────────────────────────────────

/// g1 — reading stored data that is not a thunk reduces nothing and leaves
/// nothing.
#[test]
fn g1_an_atom_read_leaves_nothing() {
    let w = Ws::committed("g1", PROGRAM);
    let n = w.count();
    let (o, _) = w.oo(&["eval", "_.a"]);
    assert!(prints(&o, "1"), "{o}");
    assert_eq!(w.count(), n, "reading a stored atom left a savepoint");
}

/// g2 — `--ephemeral` and nowhere leave nothing in any universe.
#[test]
fn g2_ephemeral_and_nowhere_leave_nothing() {
    let w = Ws::committed("g2", PROGRAM);
    let before = w.snapshot(&w.ws);
    let (o, _) = w.oo(&["eval", "--ephemeral", "1 + 41"]);
    assert!(prints(&o, "42"), "{o}");
    assert!(w.snapshot(&w.ws) == before, "--ephemeral wrote into this universe");
    let (o, _) = w.oo_in(&w.away, &["eval", "1 + 41"]);
    assert!(prints(&o, "42"), "{o}");
    assert!(!w.away.join(".oo").exists(), "an observation where there is no universe created one");
}

/// g3 — observations move nothing else: HEAD, the working set, the
/// declarations, and what `status` and `log` say.
#[test]
fn g3_observations_move_nothing_else() {
    let w = Ws::committed("g3", PROGRAM);
    fs::write(w.ws.join("y.n"), "y: 7\n").unwrap();
    w.ok(&["evolve", "y.n"]);
    let before = state(&w.snapshot(&w.ws));
    let st = w.ok(&["status"]);
    let lg = w.ok(&["log"]);
    for q in ["_.v", "_.w", "_.b", "_.big"] {
        w.oo(&["eval", q]);
    }
    fs::write(w.ws.join("q.n"), "q: _.v + 40\n").unwrap();
    w.oo(&["run", "--observe", "q", "q.n"]);
    assert!(state(&w.snapshot(&w.ws)) == before, "observations moved HEAD, the working set or a declaration");
    assert_eq!(w.ok(&["status"]), st, "observations changed what status says");
    assert_eq!(w.ok(&["log"]), lg, "observations changed what log says");
}

/// g4 — a lost context still refuses by name and writes nothing (D79).
#[test]
fn g4_a_lost_context_still_refuses() {
    let w = Ws::committed("g4", PROGRAM);
    fs::rename(w.ws.join(".oo/HEAD"), w.root.join("HEAD.aside")).unwrap();
    let before = w.snapshot(&w.ws);
    let (o, rc) = w.oo(&["eval", "_.v"]);
    assert_ne!(rc, 0, "{o}");
    assert!(w.snapshot(&w.ws) == before, "an observation on a lost context wrote");
}

/// g5 — history is untouched: commits, their order, and a collection that
/// walks them read the same with observations in between.
#[test]
fn g5_history_reads_the_same() {
    let run = |tag: &str, observe: bool| -> (usize, i32, usize) {
        let w = Ws::committed(tag, PROGRAM);
        if observe {
            w.oo(&["eval", "_.v"]);
            w.oo(&["eval", "_.b"]);
        }
        w.evolve_commit("c.n", "c: 3\n");
        if observe {
            w.oo(&["eval", "_.w"]);
        }
        w.evolve_commit("d.n", "d: 4\n");
        let lg = w.ok(&["log"]);
        let (g, grc) = w.oo(&["gc", "--grant", "gc", "--dry-run"]);
        let commits = lg.lines().filter(|l| l.contains("hash:")).count();
        (commits, grc, g.lines().count())
    };
    let plain = run("g5a", false);
    assert_eq!(plain.1, 0, "VOID READING: gc --dry-run on the control");
    assert_eq!(run("g5b", true), plain, "observations changed what log or gc sees");
}
