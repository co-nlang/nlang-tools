// A table that stops being one when printed.
// Order: nlang-tools/docs/a_table_that_stops_being_one_when_printed_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-086.  Ruling: STATUS D108 (a
// table's atomic rows print bare only when some row of the table keeps its
// `@{ … }`; otherwise every row keeps it). Spec: SYNTAX_02 §4 #12 (what is
// printed reads back as the same value), SPEC_07 §1 (a value with a pattern
// key is a table, D97; how a table prints, D102, D107).
//
// ── What is wrong (v0.87.0; since D102, v0.79.0) ─────────────────────────
//
// A table whose rows are all atomic constraints prints with no pattern key,
// and reads back as a record applied by key lookup:
//   `{ @{ 4 }: "f", _: "z" }`   prints  `{ 4: "f" _: "z" }`
//   `({ @{ 4 }: "f", _: "z" }) = ({ 4: "f", _: "z" })`  ⟹  #false
// and when a branch holds a table, the record hands the table back where
// the table applies it (D100): `{ @{ 4 }: { @{ 5 }: 1 } } 4` is not
// `{ 4: { 5: 1 } } 4`. `oo eval`, `oo run --observe` and `oo status` print
// alike. Storage is not involved: commit goes through the node table, and
// `oo fmt` prints source, where a pattern key keeps `@{ … }`.
//
// D108: when no row of a table would print as `@{ … }`, every atomic row
// does. Mixed tables print as before (D102, D107).
//
// The invariants: a printed table, read back, is the same table and answers
// the same (r1); the rule above, and only it, decides which rows keep
// `@{ … }` (r2); every interface that prints a value prints it so (r3).
//
// Not here: branch bodies. A body holding `$` prints as a forced ⊥, several
// bodies on one constraint print joined by ` & ` and meet when read back,
// and an arrow prints in a form that does not apply (Inbox rows). Every
// table below has atomic or table values.
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.

#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("table-shape-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo`, killing it after 60 s.
    fn oo(&self, args: &[&str]) -> (String, i32) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("oo runs");
        let start = Instant::now();
        loop {
            if child.try_wait().expect("wait").is_some() {
                break;
            }
            if start.elapsed() > Duration::from_secs(60) {
                let _ = child.kill();
                let _ = child.wait();
                return (format!("TIMEOUT after 60 s: oo {}", args.join(" ")), -1);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let o = child.wait_with_output().expect("output");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
            o.status.code().unwrap_or(-1),
        )
    }

    /// `oo eval` as printed, the trailing effect note dropped.
    fn eval_raw(&self, e: &str) -> String {
        let s = self.oo(&["eval", e]).0;
        match s.find(";; %effect") {
            Some(i) => s[..i].trim().to_string(),
            None => s,
        }
    }

    /// `oo eval`, whitespace collapsed.
    fn eval(&self, e: &str) -> String {
        squash(&self.eval_raw(e))
    }

    /// Evolve each source in turn, then commit. Every step must succeed: a
    /// reading taken after a failed commit is a reading of the previous
    /// universe.
    fn commit(&self, srcs: &[&str]) {
        for (i, src) in srcs.iter().enumerate() {
            let f = format!("f{i}.n");
            fs::write(self.ws.join(&f), src).unwrap();
            let (o, rc) = self.oo(&["evolve", &f]);
            assert_eq!(rc, 0, "evolve {src:?}: {o}");
        }
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        assert_eq!(rc, 0, "commit {srcs:?}: {o}");
    }

    /// The root digest of HEAD.
    fn root(&self) -> String {
        let head = fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default();
        let (o, _) = self.oo(&["inspect", head.trim()]);
        o.lines()
            .find(|l| l.contains("root:"))
            .and_then(|l| l.rsplit(':').next())
            .unwrap_or("")
            .trim()
            .to_string()
    }
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Each `(expr, want)` evaluated in one fresh workspace with no universe.
fn answers(tag: &str, cases: &[(&str, &str)]) {
    let w = Ws::new(tag);
    let mut bad = Vec::new();
    for (e, want) in cases {
        let got = w.eval(e);
        if got != *want {
            bad.push(format!("  {e}\n    want {want}\n    got  {got}"));
        }
    }
    assert!(bad.is_empty(), "{} of {} wrong:\n{}", bad.len(), cases.len(), bad.join("\n"));
}

// ── Red ──────────────────────────────────────────────────────────────────

/// Tables whose rows are all atomic, written every way and placed every
/// where; plus two that v0.87.0 already reads back (controls). Each must
/// read back `=` itself, and answer what it answers.
const TABLES: &[&str] = &[
    "{ @{ 4 }: \"f\", _: \"z\" }",
    "{ @{ \"j\" }: 1 }",
    "{ @{ _ }: 1 }",
    "{ @{ #a }: 1, _: 0 }",
    "{ @{ #_ }: 1, @{ 5 }: 2 }",
    "{ @{ 4 }: \"f\", 5: \"g\" }",
    "{ t: { @{ 4 }: \"f\", _: \"z\" } }",
    "{ @{ 4 }: { @{ 5 }: 1 }, _: 0 }",
    "{ ...{ @{ 4 }: \"f\" }, _: \"z\" }",
    "{ @{ 4 }: \"f\" } & { _: \"z\" }",
    "{ @{ 4 }: \"f\", /h: 1 }",
    // Controls, right on v0.87.0: a row keeps `@{ … }` already.
    "{ @{ @int }: 1, j: 2, _: 0 }",
    "{ @{ \"4\" }: 1, @{ 4 }: 2 }",
];

/// SYNTAX_02 §4 #12: what is printed reads back as the same value. v0.87.0:
/// every all-atomic table reads back as a record.
#[test]
fn r1_a_printed_table_reads_back_as_itself() {
    let w = Ws::new("r1");
    let mut bad = Vec::new();
    for t in TABLES {
        let printed = w.eval_raw(t);
        let back = w.eval(&format!("({t}) = ({printed})"));
        if back != "#true" {
            bad.push(format!("  {t}\n    printed {}\n    read back = {back}", squash(&printed)));
        }
    }
    assert!(bad.is_empty(), "{} of {} do not read back:\n{}", bad.len(), TABLES.len(), bad.join("\n"));
}

/// The printed form answers what the table answers. v0.87.0: a branch that
/// holds a table is applied to the input by the table (D100) and handed
/// back by the record.
#[test]
fn r1b_the_printed_form_answers_the_same() {
    let w = Ws::new("r1b");
    let mut bad = Vec::new();
    for t in ["{ @{ 4 }: { @{ 5 }: 1 }, _: 0 }", "{ @{ 4 }: { @{ 4 }: 1 }, _: 0 }", "{ @{ 4 }: \"f\", _: \"z\" }"] {
        let printed = w.eval_raw(t);
        for a in ["4", "5", "\"j\""] {
            let want = w.eval(&format!("({t}) {a}"));
            let got = w.eval(&format!("({printed}) {a}"));
            if got != want {
                bad.push(format!("  ({t}) {a} = {want}\n    printed {} answers {got}", squash(&printed)));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// D108 decides which rows keep `@{ … }`: all of them when none would
/// otherwise, and D102 / D107 as before when one does. v0.87.0: the first
/// three print bare.
#[test]
fn r2_which_rows_keep_their_pattern() {
    answers("r2", &[
        ("{ @{ 4 }: \"f\", _: \"z\" }", "{ @{ 4 }: \"f\" @{ _ }: \"z\" }"),
        ("{ @{ \"j\" }: 1 }", "{ @{ \"j\" }: 1 }"),
        ("{ @{ #a }: 1, _: 0 }", "{ @{ #a }: 1 @{ _ }: 0 }"),
        // Controls, right on v0.87.0: one row keeps `@{ … }`, the atomic
        // rows print bare; a record is not a table.
        ("{ @{ @int }: 1, j: 2, _: 0 }", "{ @{ @int }: 1 _: 0 j: 2 }"),
        ("{ @{ \"4\" }: 1, @{ 4 }: 2 }", "{ 4: 2 @{ \"4\" }: 1 }"),
        ("{ 4: \"f\", _: \"z\" }", "{ 4: \"f\" _: \"z\" }"),
    ]);
}

/// Every interface that prints a value: `oo run --observe` and `oo status`.
/// v0.87.0: both print `4: "f"`.
#[test]
fn r3_every_printer() {
    let w = Ws::new("r3");
    fs::write(w.ws.join("a.n"), "k: { @{ 4 }: \"f\", _: \"z\" }\n").unwrap();
    let (observed, rc) = w.oo(&["run", "--observe", "k", "a.n"]);
    assert_eq!(rc, 0, "run --observe: {observed}");
    let (o, rc) = w.oo(&["evolve", "a.n"]);
    assert_eq!(rc, 0, "evolve: {o}");
    let (status, _) = w.oo(&["status"]);
    let mut bad = Vec::new();
    for (name, out) in [("run --observe", &observed), ("status", &status)] {
        let s = squash(out);
        if !(s.contains("@{ 4 }: \"f\"") && s.contains("@{ _ }: \"z\"")) {
            bad.push(format!("{name}: {s}"));
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}
