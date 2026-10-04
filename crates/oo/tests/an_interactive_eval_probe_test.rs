// An interactive eval.
// Order: nlang-tools/docs/an_interactive_eval_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-071.  Ruling: meta/oo/STATUS.md D89.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// `oo repl` has printed nothing for any field since at least v0.40.0
// (measured on every tag build from v0.40.0 to v0.72.0). Cause, measured
// with a debug build: a bare key `z` parses as `FieldKey::Path`, and the
// loop observes and prints only `Named`/`Quoted` keys — the rest fall into
// `_ => continue`, silently.
//
// D89 (user, 2026-10-04, 甲): `repl` is an interactive `eval`. It starts
// from HEAD's root; the working set does not count (D88 C2); each line
// accumulates in this session's memory only — no staging, no commit (C4);
// it takes `--universe` / `--ephemeral` (C3); with no universe it answers
// from an empty root and creates nothing (D82). Other modes belong to
// `~%Repl` (SPEC_11 §1.2), not to the default.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Input is fed on stdin, ending with `exit`. "Prints v" = the output
// contains the text `oo eval` prints for that value; values are chosen to
// be distinctive (42, 4242). Wording-free otherwise; `.oo/` is compared
// byte for byte.
//
// The delivery may NOT edit this file. If a pin is wrong, say so in the
// report. `rustfmt` must not touch it.
//
// Baseline measured 2026-10-04 on dev dbd7dc6 / oo v0.72.0: see the order.

use std::collections::BTreeMap;
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("interactive-eval-{tag}"));
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

    fn oo(&self, args: &[&str]) -> (String, i32) {
        let o = self.cmd(&self.ws, args).stdin(Stdio::null()).output().expect("oo runs");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
            o.status.code().unwrap_or(-1),
        )
    }

    fn ok(&self, args: &[&str]) -> String {
        let (o, rc) = self.oo(args);
        assert_eq!(rc, 0, "setup `oo {}`: {o}", args.join(" "));
        o
    }

    /// Run `oo repl <args>` in `dir`, feeding `lines` then `exit`.
    fn repl(&self, dir: &Path, args: &[&str], lines: &[&str]) -> (String, i32) {
        let mut all = vec!["repl"];
        all.extend_from_slice(args);
        let mut child = self
            .cmd(dir, &all)
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

    fn committed(tag: &str, text: &str) -> Self {
        let w = Ws::new(tag);
        fs::write(w.ws.join("a.n"), text).unwrap();
        w.ok(&["evolve", "a.n"]);
        w.ok(&["commit", "-m", "a"]);
        w
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

/// HEAD, injections, declarations: everything but objects and savepoints
/// (savepoints are SPEC_10 §3.1's open observation gap, not this arc's).
fn state(m: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, Vec<u8>> {
    m.iter()
        .filter(|(k, _)| !k.starts_with("objects/") && !k.starts_with("savepoints/"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

fn objects(m: &BTreeMap<String, Vec<u8>>) -> usize {
    m.keys().filter(|k| k.starts_with("objects/")).count()
}

fn prints(o: &str, v: &str) -> bool {
    o.split(|c: char| !c.is_ascii_alphanumeric()).any(|t| t == v)
}

// ── Red ──────────────────────────────────────────────────────────────────

/// r1 — a field is evaluated and printed. Baseline: nothing.
#[test]
fn r1_a_field_is_printed() {
    let w = Ws::committed("r1", "a: 1\n");
    let (o, rc) = w.repl(&w.ws, &[], &["z: 40 + 2"]);
    assert_eq!(rc, 0, "{o}");
    assert!(prints(&o, "42"), "repl did not print z: 40 + 2: {o}");
}

/// r2 — it starts from HEAD's root.
#[test]
fn r2_it_reads_the_committed_root() {
    let w = Ws::committed("r2", "a: 40\n");
    let (o, _) = w.repl(&w.ws, &[], &["z: _.a + 2"]);
    assert!(prints(&o, "42"), "repl did not read the committed a: 40: {o}");
}

/// r3 — lines accumulate within the session.
#[test]
fn r3_lines_accumulate_in_the_session() {
    let w = Ws::committed("r3", "a: 1\n");
    let (o, _) = w.repl(&w.ws, &[], &["p: 4240", "q: p + 2"]);
    assert!(prints(&o, "4240"), "the first line was not printed: {o}");
    assert!(prints(&o, "4242"), "the second line did not see the first: {o}");
}

/// r4 — no universe: answers from an empty root, creates nothing (D82).
/// Baseline: refuses.
#[test]
fn r4_no_universe_answers_and_creates_nothing() {
    let w = Ws::new("r4");
    let (o, rc) = w.repl(&w.away, &[], &["z: 40 + 2"]);
    assert_eq!(rc, 0, "repl refused where there is no universe: {o}");
    assert!(prints(&o, "42"), "{o}");
    assert!(!w.away.join(".oo").exists(), "repl created a universe");
}

/// r5 — `--universe` from elsewhere.
#[test]
fn r5_universe_selector() {
    let w = Ws::committed("r5", "a: 40\n");
    let p = w.ws.display().to_string();
    let (o, rc) = w.repl(&w.away, &["--universe", &p], &["z: _.a + 2"]);
    assert_eq!(rc, 0, "{o}");
    assert!(prints(&o, "42"), "repl --universe did not read that universe: {o}");
    assert!(!w.away.join(".oo").exists(), "a universe appeared where the caller stands");
}

/// r6 — `--ephemeral` inside a universe answers as where there is none,
/// and writes nothing.
#[test]
fn r6_ephemeral() {
    let w = Ws::committed("r6", "a: 40\n");
    let before = w.snapshot(&w.ws);
    let (nowhere, nrc) = w.repl(&w.away, &[], &["z: _.a", "y: 4242"]);
    let (o, rc) = w.repl(&w.ws, &["--ephemeral"], &["z: _.a", "y: 4242"]);
    assert_eq!(rc, 0, "repl --ephemeral: {o}");
    assert!(prints(&o, "4242"), "repl --ephemeral printed nothing: {o}");
    assert_eq!((o.as_str(), rc), (nowhere.as_str(), nrc), "repl --ephemeral differs from repl where there is no universe");
    assert!(w.snapshot(&w.ws) == before, "repl --ephemeral changed .oo/");
}

/// r7 — F2: the same selectors, described as on `eval`.
#[test]
fn r7_same_selectors_as_eval() {
    let w = Ws::new("r7");
    let line = |cmd: &str, flag: &str| -> Option<String> {
        let (o, _) = w.oo(&[cmd, "--help"]);
        o.lines()
            .find(|l| l.trim_start().starts_with(flag))
            .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
    };
    for flag in ["--universe", "--ephemeral"] {
        let e = line("eval", flag).unwrap_or_else(|| panic!("VOID READING: eval has no {flag}"));
        let r = line("repl", flag).unwrap_or_else(|| panic!("repl has no {flag}"));
        assert_eq!(r, e, "repl {flag} is described differently from eval {flag}");
    }
}

// ── Green ────────────────────────────────────────────────────────────────

/// g1 — the working set does not count (D88 C2): a staged field reads as a
/// field nobody defined.
#[test]
fn g1_the_working_set_does_not_count() {
    let w = Ws::committed("g1", "a: 1\n");
    fs::write(w.ws.join("y.n"), "y: 4242\n").unwrap();
    w.ok(&["evolve", "y.n"]);
    let (staged, _) = w.repl(&w.ws, &[], &["z: _.y"]);
    let (nobody, _) = w.repl(&w.ws, &[], &["z: _.nobody_defined_this"]);
    assert!(!prints(&staged, "4242"), "repl saw a field that is only staged: {staged}");
    assert_eq!(staged, nobody, "a staged field and an undefined one read differently");
}

/// g2 — a session stages nothing and commits nothing.
#[test]
fn g2_a_session_stages_and_commits_nothing() {
    let w = Ws::committed("g2", "a: 1\n");
    let before = state(&w.snapshot(&w.ws));
    let _ = w.repl(&w.ws, &[], &["z: 1", "w: z + 1"]);
    assert!(state(&w.snapshot(&w.ws)) == before, "a repl session changed HEAD or the working set");
    let (st, _) = w.oo(&["status"]);
    assert!(!st.contains("w: z + 1") && !st.contains("z: 1"), "a repl line was staged: {st}");
}

/// g3 — a lost context is not an empty universe (D79, as for eval).
#[test]
fn g3_lost_context_refuses_by_name() {
    let w = Ws::committed("g3", "a: 1\n");
    fs::rename(w.ws.join(".oo/HEAD"), w.root.join("HEAD.aside")).unwrap();
    let before = w.snapshot(&w.ws);
    let (o, rc) = w.repl(&w.ws, &[], &["z: 1"]);
    assert_ne!(rc, 0, "repl read a lost context as an empty universe: {o}");
    assert!(o.contains("rollback"), "the refusal does not name the way back: {o}");
    assert!(w.snapshot(&w.ws) == before, "repl on a lost context wrote");
}

/// g4 — an explicit save is the program's own effect: it writes this
/// universe's object store and nothing else (D88, as in eval). Green on the
/// baseline too: evolving the line runs the save even though nothing is
/// printed.
#[test]
fn g4_explicit_save_writes_the_object_store_only() {
    let w = Ws::committed("r8", "a: 1\n");
    let before = w.snapshot(&w.ws);
    let (o, rc) = w.repl(&w.ws, &[], &["s: ~%Engine./save { q: 4242 }"]);
    assert_eq!(rc, 0, "{o}");
    let after = w.snapshot(&w.ws);
    assert!(objects(&after) > objects(&before), "repl's save did not write the object store: {o}");
    assert!(state(&after) == state(&before), "repl's save changed more than the object store");
}
