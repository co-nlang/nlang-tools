// The universe you are in.
// Order: nlang-tools/docs/the_universe_you_are_in_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-070.  Ruling: meta/oo/STATUS.md D88.
// Design: nlang-spec/meta/oo/cli_surface.md §6 (C1–C4).
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// SYNTAX_03 §143: "`_.` is this universe's root". Measured on v0.71.0: in a
// workspace that has committed `a: 1`, `oo eval '_.a'` answers `_`; `oo run`
// on `b: a + 1` observes `_`; a test that reads `_.a` fails. The one-shot
// evaluators run in a blank universe the specification never carved out.
//
// D88 (user, 2026-10-03):
//  C1  in a universe, `eval` / `run` / `test` start from HEAD's root;
//      outside one, empty (D82);
//  C2  the working set does not count (`status` shows it);
//  C3  `--universe <path>` selects another directory's universe, for every
//      command that needs a universe and for the one-shot evaluators;
//      `--ephemeral` (an anonymous temporary universe even inside one) is
//      for the one-shot evaluators only; one flag, one signature, one
//      description (REAL_01 §1.4 F2);
//  C4  `run` injects its files into a read-only view and observes; it does
//      not stage or commit. An explicit `~%Engine./save` is the program's
//      own effect and writes this universe's object store, as `eval` does
//      today; with no universe it answers ⊥ `#no_universe` (D82 ③).
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Wording-free except the registered tag `#no_universe` and the flag names
// the ruling chose. "Same as" is byte equality of output between the flag
// form and the `cd` form. `.oo/` is compared byte for byte.
//
// The delivery may NOT edit this file. If a pin is wrong, say so in the
// report. `rustfmt` must not touch it.
//
// Baseline measured 2026-10-03 on dev c07dd6b / oo v0.71.0: see the order.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    /// The universe.
    ws: PathBuf,
    /// Somewhere else, with no universe: where the caller stands.
    away: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("universe-in-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        let away = root.join("away");
        fs::create_dir_all(&ws).unwrap();
        fs::create_dir_all(&away).unwrap();
        Ws { _scratch: scratch, root, ws, away }
    }

    fn oo_in(&self, dir: &Path, args: &[&str]) -> (String, i32) {
        let o = Command::new(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(dir)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .stdin(Stdio::null())
            .output()
            .expect("oo runs");
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

    fn write(&self, dir: &Path, name: &str, text: &str) {
        fs::write(dir.join(name), text).unwrap();
    }

    /// A universe with `a: 1` committed.
    fn committed(tag: &str) -> Self {
        let w = Ws::new(tag);
        w.write(&w.ws, "a.n", "a: 1\n");
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

    fn ws_str(&self) -> String {
        self.ws.display().to_string()
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

fn objects(m: &BTreeMap<String, Vec<u8>>) -> usize {
    m.keys().filter(|k| k.starts_with("objects/")).count()
}

/// Everything in `.oo/` except the object store: HEAD, injections,
/// savepoints, declarations.
fn state(m: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, Vec<u8>> {
    m.iter().filter(|(k, _)| !k.starts_with("objects/")).map(|(k, v)| (k.clone(), v.clone())).collect()
}

// ── Red: C1 — the one-shot evaluators read HEAD ─────────────────────────

/// r1 — eval. Baseline: `_`.
#[test]
fn r1_eval_reads_the_committed_root() {
    let w = Ws::committed("r1");
    let (o, rc) = w.oo(&["eval", "_.a"]);
    assert_eq!((o.as_str(), rc), ("1", 0), "eval '_.a' in a universe that committed a: 1");
}

/// r2 — run: a file evolved onto HEAD's root sees it. Baseline: `_`.
#[test]
fn r2_run_sees_the_committed_root() {
    let w = Ws::committed("r2");
    w.write(&w.ws, "b.n", "b: a + 1\n");
    let before = w.snapshot(&w.ws);
    let (o, rc) = w.oo(&["run", "b.n", "--observe", "b"]);
    assert_eq!((o.as_str(), rc), ("2", 0), "run observing b: a + 1 over a committed a: 1");
    assert!(w.snapshot(&w.ws) == before, "run changed .oo/");
}

/// r3 — test: a test that reads the committed root passes; its control
/// (a wrong expectation) fails. Baseline: both fail.
#[test]
fn r3_test_sees_the_committed_root() {
    let w = Ws::committed("r3");
    w.write(&w.ws, "t1.n", "test_a: _.a = 1\n");
    w.write(&w.ws, "t2.n", "test_a: _.a = 2\n");
    let (o2, rc2) = w.oo(&["test", "t2.n"]);
    assert_ne!(rc2, 0, "VOID READING: a test expecting the wrong value passed: {o2}");
    let (o1, rc1) = w.oo(&["test", "t1.n"]);
    assert_eq!(rc1, 0, "a test reading the committed a: 1 failed: {o1}");
}

// ── Red: C3 — the selectors ──────────────────────────────────────────────

/// r4 — `eval --universe <p>` from elsewhere answers as `eval` inside p,
/// and creates nothing where the caller stands.
#[test]
fn r4_eval_with_universe_is_eval_inside_it() {
    let w = Ws::committed("r4");
    let p = w.ws_str();
    let (inside, irc) = w.oo(&["eval", "_.a"]);
    let (o, rc) = w.oo_in(&w.away, &["eval", "--universe", &p, "_.a"]);
    assert_eq!((o.as_str(), rc), (inside.as_str(), irc), "eval --universe differs from eval inside it");
    assert_eq!(o, "1", "{o}");
    assert!(!w.away.join(".oo").exists(), "eval --universe created a universe where the caller stands");
}

/// r5 — the commands that need a universe take the same selector: status
/// and log answer as inside it; evolve reads its file from where the caller
/// stands and stages into p; commit lands in p.
#[test]
fn r5_universe_commands_take_the_selector() {
    let w = Ws::committed("r5");
    let p = w.ws_str();
    for cmd in [&["status"][..], &["log"][..]] {
        let (inside, irc) = w.oo(cmd);
        let mut args = cmd.to_vec();
        args.extend(["--universe", p.as_str()]);
        let (o, rc) = w.oo_in(&w.away, &args);
        assert_eq!((o.as_str(), rc), (inside.as_str(), irc), "`oo {}` differs from `oo {}` inside it", args.join(" "), cmd.join(" "));
    }
    let head = fs::read_to_string(w.ws.join(".oo/HEAD")).unwrap();
    w.write(&w.away, "c.n", "c: 3\n");
    let (o, rc) = w.oo_in(&w.away, &["evolve", "--universe", &p, "c.n"]);
    assert_eq!(rc, 0, "evolve --universe with a file where the caller stands: {o}");
    let (o, rc) = w.oo_in(&w.away, &["commit", "--universe", &p, "-m", "c"]);
    assert_eq!(rc, 0, "commit --universe: {o}");
    assert_ne!(fs::read_to_string(w.ws.join(".oo/HEAD")).unwrap(), head, "commit --universe did not move p's HEAD");
    assert!(!w.away.join(".oo").exists(), "a universe appeared where the caller stands");
    let (o, _) = w.oo(&["eval", "_.c"]);
    assert_eq!(o, "3", "the commit made through --universe is not in p's root");
}

/// r6 — `--universe` naming a directory with no universe: a command that
/// reads refuses by name and creates nothing there.
#[test]
fn r6_universe_naming_nothing_refuses() {
    let w = Ws::committed("r6");
    // The selector must exist and work on a real universe first; otherwise a
    // usage error would pass the refusal below for the wrong reason.
    let (o, rc) = w.oo_in(&w.away, &["eval", "--universe", &w.ws_str(), "_.a"]);
    assert_eq!((o.as_str(), rc), ("1", 0), "the selector does not work on a real universe: {o}");
    let empty = w.root.join("empty");
    fs::create_dir_all(&empty).unwrap();
    let e = empty.display().to_string();
    for args in [vec!["eval", "--universe", e.as_str(), "_.a"], vec!["status", "--universe", e.as_str()]] {
        let (o, rc) = w.oo_in(&w.away, &args);
        assert_ne!(rc, 0, "`oo {}` answered rc=0 for a directory with no universe: {o}", args.join(" "));
        assert!(!empty.join(".oo").exists(), "`oo {}` created a universe in the named directory", args.join(" "));
    }
}

/// r7 — `--ephemeral` inside a universe: the answer of a place with no
/// universe, and nothing written.
#[test]
fn r7_ephemeral_is_no_universe_even_inside_one() {
    let w = Ws::committed("r7");
    let (nowhere, nrc) = w.oo_in(&w.away, &["eval", "_.a"]);
    let before = w.snapshot(&w.ws);
    let (o, rc) = w.oo(&["eval", "--ephemeral", "_.a"]);
    assert_eq!((o.as_str(), rc), (nowhere.as_str(), nrc), "eval --ephemeral inside a universe differs from eval where there is none");
    w.write(&w.ws, "b.n", "b: a + 1\n");
    let (ro, rrc) = w.oo(&["run", "--ephemeral", "b.n", "--observe", "b"]);
    let (rn, rnrc) = {
        w.write(&w.away, "b.n", "b: a + 1\n");
        w.oo_in(&w.away, &["run", "b.n", "--observe", "b"])
    };
    assert_eq!((ro.as_str(), rrc), (rn.as_str(), rnrc), "run --ephemeral inside a universe differs from run where there is none");
    assert!(w.snapshot(&w.ws) == before, "--ephemeral changed .oo/");
}

/// r8 — an explicit save with no universe to write: ⊥ `#no_universe`, and
/// the universe the caller stands in is untouched.
#[test]
fn r8_ephemeral_save_has_no_universe() {
    let w = Ws::committed("r8");
    let before = w.snapshot(&w.ws);
    let (o, rc) = w.oo(&["eval", "--ephemeral", "~%Engine./save { q: 7 }"]);
    assert_eq!(rc, 0, "a value-carrier answer is rc=0 (D63): {o}");
    assert!(o.contains("#no_universe"), "save under --ephemeral did not answer #no_universe: {o}");
    assert!(w.snapshot(&w.ws) == before, "save under --ephemeral wrote the universe the caller stands in");
}

/// r9 — F2: one flag, one signature, one description. `--universe` on every
/// command that needs a universe and on the one-shot evaluators;
/// `--ephemeral` on the one-shot evaluators, and on nothing that writes.
#[test]
fn r9_one_flag_one_description() {
    let w = Ws::new("r9");
    let help_line = |cmd: &str, flag: &str| -> Option<String> {
        let (o, _) = w.oo_in(&w.away, &[cmd, "--help"]);
        // Whitespace is normalized: clap pads the description column to each
        // command's longest flag, so the layout differs where the words do not.
        o.lines()
            .find(|l| l.trim_start().starts_with(flag) || l.contains(&format!(" {flag} ")))
            .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
    };
    let universe_cmds = [
        "eval", "run", "test", "evolve", "status", "log", "commit", "rollback", "squash", "refine", "gc", "migrate", "inspect",
    ];
    let mut seen: Option<String> = None;
    for c in universe_cmds {
        let l = help_line(c, "--universe").unwrap_or_else(|| panic!("`oo {c}` has no --universe"));
        match &seen {
            None => seen = Some(l),
            Some(s) => assert_eq!(&l, s, "`oo {c} --universe` is described differently"),
        }
    }
    let mut seen: Option<String> = None;
    for c in ["eval", "run", "test"] {
        let l = help_line(c, "--ephemeral").unwrap_or_else(|| panic!("`oo {c}` has no --ephemeral"));
        match &seen {
            None => seen = Some(l),
            Some(s) => assert_eq!(&l, s, "`oo {c} --ephemeral` is described differently"),
        }
    }
    for c in ["evolve", "commit", "rollback", "squash", "refine", "gc", "migrate"] {
        assert!(help_line(c, "--ephemeral").is_none(), "`oo {c}` takes --ephemeral; a writer has nothing to write in an anonymous universe");
    }
}

/// r10 — C1 with D79: once eval starts from HEAD's root, a lost context is
/// not an empty one. Plain eval refuses by name and writes nothing; the
/// context-free answer is `--ephemeral`. Baseline: plain eval answers.
#[test]
fn r10_a_lost_context_is_not_an_empty_universe() {
    let w = Ws::committed("r10");
    fs::rename(w.ws.join(".oo/HEAD"), w.root.join("HEAD.aside")).unwrap();
    let before = w.snapshot(&w.ws);
    let (o, rc) = w.oo(&["eval", "_.a"]);
    assert_ne!(rc, 0, "eval read a lost context as an empty universe: {o}");
    assert!(o.contains("rollback"), "the refusal does not name the way back: {o}");
    assert!(w.snapshot(&w.ws) == before, "eval on a lost context wrote");
    let (e, erc) = w.oo(&["eval", "--ephemeral", "~%Math./add (1, 2)"]);
    assert_eq!((e.as_str(), erc), ("3", 0), "the context-free form does not answer: {e}");
}

// ── Green: what must not change ─────────────────────────────────────────

/// g1 — C2: the working set does not count. A staged field reads as a field
/// nobody defined.
#[test]
fn g1_the_working_set_does_not_count() {
    let w = Ws::committed("g1");
    w.write(&w.ws, "y.n", "y: 2\n");
    w.ok(&["evolve", "y.n"]);
    let (staged, _) = w.oo(&["eval", "_.y"]);
    let (nobody, _) = w.oo(&["eval", "_.nobody_defined_this"]);
    assert_eq!(staged, nobody, "eval saw a field that is only staged");
}

/// g2 — D82: where there is no universe, eval answers and creates nothing.
#[test]
fn g2_no_universe_is_still_empty() {
    let w = Ws::new("g2");
    let (o, rc) = w.oo_in(&w.away, &["eval", "_.a"]);
    assert_eq!(rc, 0, "{o}");
    assert!(!w.away.join(".oo").exists(), "eval created a universe");
    let (o2, _) = w.oo_in(&w.away, &["eval", "1 + 1"]);
    assert_eq!(o2, "2", "VOID READING: eval does not evaluate here: {o2}");
}

/// g3 — C4: run and eval neither stage nor commit: HEAD, injections and
/// savepoints are byte-identical after them.
#[test]
fn g3_one_shot_does_not_stage_or_commit() {
    let w = Ws::committed("g3");
    w.write(&w.ws, "b.n", "b: 5\n");
    let before = state(&w.snapshot(&w.ws));
    let _ = w.oo(&["run", "b.n", "--observe", "b"]);
    let _ = w.oo(&["eval", "{ z: 9 }"]);
    let _ = w.oo(&["test", "b.n"]);
    assert!(state(&w.snapshot(&w.ws)) == before, "a one-shot evaluator changed HEAD, the working set or the savepoints");
    let (st, _) = w.oo(&["status"]);
    assert!(!st.contains("b: 5"), "run staged its file: {st}");
}

/// g4 — the existing contract (cas_integrity R-2): an explicit save in eval
/// writes this universe's object store, and nothing else. D88 C4: run does
/// the same.
#[test]
fn g4_explicit_save_writes_the_object_store_only() {
    let w = Ws::committed("g4");
    let before = w.snapshot(&w.ws);
    let (o, rc) = w.oo(&["eval", "~%Engine./save { q: 7 }"]);
    assert_eq!(rc, 0, "{o}");
    let mid = w.snapshot(&w.ws);
    assert!(objects(&mid) > objects(&before), "eval's save did not write the object store: {o}");
    assert!(state(&mid) == state(&before), "eval's save changed more than the object store");
    w.write(&w.ws, "s.n", "s: ~%Engine./save { q: 8 }\n");
    let (o, rc) = w.oo(&["run", "s.n", "--observe", "s"]);
    assert_eq!(rc, 0, "{o}");
    let after = w.snapshot(&w.ws);
    assert!(objects(&after) > objects(&mid), "run's save did not write the object store: {o}");
    assert!(state(&after) == state(&mid), "run's save changed more than the object store");
}
