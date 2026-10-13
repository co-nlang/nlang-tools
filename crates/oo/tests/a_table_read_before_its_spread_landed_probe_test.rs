// A table read before its spread landed.
// Order: nlang-tools/docs/a_table_read_before_its_spread_landed_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-084.  No ruling needed: D101 (a
// table has one normal form; every way of building it is the same table).
// Spec: SPEC_03 §3.1 (spread), SPEC_07 §1 (a value with a pattern key is a
// table, D97), §4 (`|>`: a morphism on the right is applied, a combo is
// merged), STATUS D101.
//
// ── What is wrong (v0.85.0; v0.75.0 on) ──────────────────────────────────
//
// A table built with a spread and placed in a field answers as if the spread
// had not happened:
//   `{ t: { ...{ @{ @int }: 1 }, _: 0 } }.t 7`  ⟹ `0`   (the table says 1)
//   `{ t: { ...{ b: 1 } } }.t "b"`             ⟹ #no_matching_branch
// The same after commit (`_.k.t 7` ⟹ `0`), at depth 2, and with the spread
// source a sibling. At the top `{ ...{ @{ @int }: 1 }, _: 0 } 7` ⟹ `1`.
// `=` says the field's table equals the written one, printing shows the
// pattern branch, navigation `.t.b` finds the field — only applying it is
// wrong. Silent: the wrong answer is a value, and the branch it picks runs
// (a `write_file` in the default branch writes).
//
// Separately, `7 |> { ...{ @{ @int }: 1 }, _: 0 }` ⟹ ⊥ #conflict at the top
// level too: the pipe decides "morphism or combo" before the spread lands,
// and merges a table it should have applied.
//
// The invariant: how a table is written (directly, by spreading a literal,
// by spreading a name) and where it lives (top, field, depth, after commit)
// do not change what it answers, through either entrance (application,
// `|>`), and the branch it does not choose is not run.
//
// Guards: `|>` with a combo on the right still merges (g1); the roots of
// these universes do not move — the fix is in reading, not storing (g2).
// What was already right (the top level, `=`, printing, navigation) sits as
// control cases inside r1–r3, so a red there names what broke.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("spread-landed-{tag}"));
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

    /// `oo eval`, whitespace collapsed, the trailing effect note dropped.
    fn eval(&self, e: &str) -> String {
        let s = squash(&self.oo(&["eval", e]).0);
        match s.find(" ;; %effect") {
            Some(i) => s[..i].to_string(),
            None => s,
        }
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

/// A spread table in a field, applied in place. v0.85.0: `0` for the int.
#[test]
fn r1_a_table_in_a_field_answers_as_the_table() {
    answers("r1", &[
        ("{ t: { ...{ @{ @int }: 1 }, _: 0 } }.t 7", "1"),
        ("{ t: { ...{ @{ @int }: 1 }, _: 0 } }.t \"s\"", "0"),
        ("({ t: { ...{ @{ @int }: 1 }, _: 0 } }.t) 7", "1"),
        ("{ g: { t: { ...{ @{ @int }: 1 }, _: 0 } }.t }.g 7", "1"),
        // Controls, right on v0.85.0: the top level, `=`, the printed form.
        ("{ ...{ @{ @int }: 1 }, _: 0 } 7", "1"),
        ("{ t: { @{ @int }: 1, _: 0 } }.t 7", "1"),
        ("{ t: { ...{ @{ @int }: 1 }, _: 0 } }.t = { @{ @int }: 1, _: 0 }", "#true"),
        ("{ t: { ...{ @{ @int }: 1 }, _: 0 } }.t", "{ @{ @int }: 1 _: 0 }"),
    ]);
}

/// The same table committed, one and two levels down.
#[test]
fn r2_after_commit_the_table_still_answers() {
    let w = Ws::new("r2");
    w.commit(&["k: { t: { ...{ @{ @int }: 1 }, _: 0 } }\nd: { t: { v: { ...{ @{ @int }: 1 }, _: 0 } } }\ne: { ...{ @{ @int }: 1 }, _: 0 }\n"]);
    let mut bad = Vec::new();
    // `_.e 7` is the control: the top level, right on v0.85.0.
    for (e, want) in [("_.k.t 7", "1"), ("_.k.t \"s\"", "0"), ("_.d.t.v 7", "1"), ("_.d.t.v \"s\"", "0"), ("_.e 7", "1")] {
        let got = w.eval(e);
        if got != want {
            bad.push(format!("{e}: want {want} got {got}"));
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}

/// A table with no pattern key is applied by key lookup (D97); a spread
/// brings its keys in. v0.85.0: `0` and #no_matching_branch.
#[test]
fn r3_a_keyed_table_brought_in_by_a_spread() {
    answers("r3", &[
        ("{ t: { ...{ b: 1 }, _: 0 } }.t \"b\"", "1"),
        ("{ t: { ...{ b: 1 } } }.t \"b\"", "1"),
        ("{ t: { ...{ b: 1 }, _: 0 } }.t \"z\"", "0"),
        // Control: navigation already finds the key.
        ("{ t: { ...{ b: 1 } } }.t.b", "1"),
    ]);
}

/// A spread beside a written pattern key: both are branches.
#[test]
fn r4_a_spread_beside_a_written_pattern() {
    answers("r4", &[
        ("{ t: { ...{ @{ @int }: 1 }, @{ @str }: 2 } }.t 7", "1"),
        ("{ t: { ...{ @{ @int }: 1 }, @{ @str }: 2 } }.t \"s\"", "2"),
        ("{ t: { @{ @str }: 2, ...{ @{ @int }: 1 } } }.t 7", "1"),
    ]);
}

/// The spread source is a sibling — before it, after it, and committed.
#[test]
fn r5_a_spread_of_a_name() {
    answers("r5", &[
        ("{ s: { @{ @int }: 1 }, t: { ...s, _: 0 } }.t 7", "1"),
        ("{ t: { ...s, _: 0 }, s: { @{ @int }: 1 } }.t 7", "1"),
    ]);
    let w = Ws::new("r5c");
    w.commit(&["s: { @{ @int }: 1 }\nt: { u: { ...s, _: 0 } }\n"]);
    assert_eq!(w.eval("_.t.u 7"), "1");
}

/// SPEC_07 §4: a morphism on the right of `|>` is applied. A table is one,
/// whether or not it was built with a spread. v0.85.0: ⊥ #conflict (merged).
#[test]
fn r6_the_pipe_sees_the_table_after_its_spread() {
    answers("r6", &[
        ("7 |> { ...{ @{ @int }: 1 }, _: 0 }", "1"),
        ("\"s\" |> { ...{ @{ @int }: 1 }, _: 0 }", "0"),
        ("7 |> { t: { ...{ @{ @int }: 1 }, _: 0 } }.t", "1"),
        ("7 |> { @{ @int }: 1, _: 0 }", "1"),
    ]);
}

/// The branch the table does not choose is not run. v0.85.0 chooses `_`
/// and runs its `write_file`.
#[test]
fn r7_the_branch_not_chosen_is_not_run() {
    let w = Ws::new("r7");
    let got = w.eval("{ t: { ...{ @{ @int }: 1 }, _: ~%Io./write_file (\"p.txt\", \"x\") } }.t 7");
    assert_eq!(got, "1");
    assert!(!w.ws.join("p.txt").exists(), "the default branch ran");
    // Control: the same table written directly.
    let got = w.eval("{ t: { @{ @int }: 1, _: ~%Io./write_file (\"q.txt\", \"x\") } }.t 7");
    assert_eq!(got, "1");
    assert!(!w.ws.join("q.txt").exists(), "control: the default branch ran");
}

/// A branch whose value is an arrow is applied to the input (D100).
#[test]
fn r8_a_branch_that_is_an_arrow() {
    answers("r8", &[
        ("{ t: { ...{ @{ @int }: (x -> x * 10) }, _: 0 } }.t 7", "70"),
        ("{ t: { @{ @int }: (x -> x * 10), _: 0 } }.t 7", "70"),
    ]);
}

// ── Guards (green on v0.85.0) ─────────────────────────────────────────────

/// `|>` with a plain combo on the right merges (SPEC_07 §4).
#[test]
fn g1_structural_evolution_still_merges() {
    answers("g1", &[
        ("{ a: 1 } |> { ...{ b: 2 } }", "{ a: 1 b: 2 }"),
        ("{ a: 1 } |> { ...$, b: 2 }", "{ a: 1 b: 2 }"),
        ("{ a: 1 } |> { b: 2 }", "{ a: 1 b: 2 }"),
    ]);
}

/// The answer is in reading, not in storing: these roots do not move.
#[test]
fn g2_the_roots_do_not_move() {
    let w = Ws::new("g2a");
    w.commit(&["k: { t: { ...{ @{ @int }: 1 }, _: 0 } }\n"]);
    assert_eq!(w.root(), "84df117faf575e96bcf3db4a95f91a00dd98ed58494ad9974995535dc2dca6aa");
    let w = Ws::new("g2b");
    w.commit(&["k: { t: { @{ @int }: 1, _: 0 } }\n"]);
    assert_eq!(w.root(), "275bc55ddc3801d21b306de8cb06f1d9a8d455aea7bed823ab889975620d559f");
}

// ── Acceptance (acceptor, 2026-10-13): two cells the reference missed ────

/// The delivery found a third reader the acceptor's reference did not fix:
/// `|>` distributes a union on the right member by member (D104), and each
/// member is read as a table only after its spreads land. And a table whose
/// chosen branch holds a spread table applies it to the input (D100), as it
/// does the same table written directly.
/// v0.85.0 red (`#conflict`; `2`; the inner table itself). The acceptor's
/// reference (q084-ref 66dfcc3) red on the first two. Delivery e270e8b green.
#[test]
fn r9_a_union_member_and_a_branch_value_land_first() {
    answers("r9", &[
        ("7 |> ({ ...{ @{ @int }: 1 }, _: 0 } | { @{ @str }: 2 })", "1"),
        ("\"s\" |> ({ ...{ @{ @int }: 1 }, _: 0 } | { @{ @str }: 2 })", "0 | 2"),
        ("{ @{ @int }: { ...{ @{ @int }: 1 }, _: 0 } } 7", "1"),
        // Controls, right on v0.85.0: the same, written directly.
        ("\"s\" |> ({ @{ @int }: 1, _: 0 } | { @{ @str }: 2 })", "0 | 2"),
        ("{ @{ @int }: { @{ @int }: 1, _: 0 } } 7", "1"),
    ]);
}
