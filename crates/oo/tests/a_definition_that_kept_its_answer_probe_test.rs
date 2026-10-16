// A definition that kept the answer and forgot the question.
// Order: nlang-tools/docs/a_definition_that_kept_its_answer_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-091.  Ruling D110 (2026-10-16,
// user "都按建議"): a pure derived definition whose answer read the universe
// (a name, `_.`, a live reference, a stored open term) keeps its recipe, as
// D46 (i) says of every `#pure` cell; the criterion is "read the universe",
// so a closed computation (`~%Math./add (1, 1)`) keeps its answer. The read
// side follows SPEC_07 §4.2 P3 and its precedent: a stored open term is its
// definition, read where it is read, whether named in place or through a
// name.
//
// ── What is wrong (v0.92.0) ─────────────────────────────────────────────
//
// Evolve evaluates a definition and stores the answer unless the expression
// is arithmetic. An answer that read the universe is a reading of it as it
// is now, and stays that way:
//   commit r: <<_.y>>, q10: r = 5, j1: _.z = 5; then commit y: 5, z: 5:
//     q10 ⟹ #false, j1 ⟹ #false, while _.r = 5 ⟹ #true and _.z = 5 ⟹ #true
//   in one program, j1: z = 5 written before z: 5 ⟹ #false; after ⟹ #true
// Commit decides "is it ⊥" by forcing with references unread, so a
// computation over a reference through a name commits as a conflict:
//   n: 1, r: <<n>>, q: r + 1 ⟹ q is ⊥ #conflict after commit (run: 2)
// A stored open term reached through a name is carried out of the frame:
//   t: { a: $.b + 1 }, w: t:  { b: 1 } |> (x -> _.w) ⟹ { a: ⊥ #no_context }
//   while { b: 1 } |> (x -> _.t) ⟹ { a: 2 }
// A transform outside a loop is counted as one inside it:
//   c: { a2: b2, b2: a2 }:  (c.a2).%cause ⟹ #static_cycle, but
//   (_.c.a2).%cause ⟹ #divergent in the same program
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("kept-answer-{tag}"));
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

    /// `oo eval`, the trailing effect note dropped, whitespace collapsed.
    fn eval(&self, e: &str) -> String {
        let s = self.oo(&["eval", e]).0;
        let s = match s.find(";; %effect") {
            Some(i) => s[..i].trim().to_string(),
            None => s,
        };
        squash(&s)
    }

    /// Evolve `src`, then commit. Both must succeed: a reading taken after a
    /// failed commit is a reading of the previous universe.
    fn commit(&self, src: &str) {
        let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
        let f = format!("f{n}.n");
        fs::write(self.ws.join(&f), src).unwrap();
        let (o, rc) = self.oo(&["evolve", &f]);
        assert_eq!(rc, 0, "evolve {src:?}: {o}");
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        assert_eq!(rc, 0, "commit {src:?}: {o}");
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

    /// `run --observe path` of a program, whitespace collapsed.
    fn observe(&self, src: &str, path: &str) -> String {
        fs::write(self.ws.join("p.n"), src).unwrap();
        squash(&self.oo(&["run", "--observe", path, "p.n"]).0)
    }

    /// Observation savepoints in this universe.
    fn savepoints(&self) -> usize {
        fs::read_dir(self.ws.join(".oo/savepoints"))
            .map(|d| {
                d.flatten()
                    .filter(|e| {
                        let n = e.file_name().to_string_lossy().to_string();
                        e.path().is_file() && n != "LOG" && !n.starts_with('.')
                    })
                    .count()
            })
            .unwrap_or(0)
    }
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn check(bad: &mut Vec<String>, label: &str, got: String, want: &str) {
    if got != want {
        bad.push(format!("  {label}\n    want {want}\n    got  {got}"));
    }
}

fn verdict(bad: Vec<String>, n: usize) {
    assert!(bad.is_empty(), "{} of {} wrong:\n{}", bad.len(), n, bad.join("\n"));
}

const NO_CONTEXT: &str = "_|_ ;; %cause: #no_context ;; free `$` observed without an enclosing evolution (P3)";

// ── Red ──────────────────────────────────────────────────────────────────

/// r1 — A definition that read the universe answers what the universe holds
/// when it is read, as the same reading written in place does. Commit the
/// definitions while `y` and `z` are absent, then commit `y` and `z`.
/// v0.92.0: `q`, `q7` ⊥ #conflict, `q10`, `j1` #false, `l` [_, 2], `s` the
/// partial application it read, after the second commit; and an alias of a
/// coordinate narrowed later keeps the old value. Controls (the same
/// readings in place) are live on v0.92.0.
#[test]
fn r1_a_reading_of_the_universe_is_read_again() {
    let w = Ws::new("r1");
    w.commit(
        "r: <<_.y>>\nq: r + 1\nq7: (x -> x + 1) r\nq10: r = 5\nj1: _.z = 5\nl: [_.z, 2]\ns: ~%Str./len (_.u)\n",
    );
    w.commit("y: 5\nz: 5\nu: \"ab\"\n");
    let mut bad = Vec::new();
    let cases = [
        ("q", "6"),
        ("q7", "6"),
        ("q10", "#true"),
        ("j1", "#true"),
        ("l", "[5, 2]"),
        ("s", "2"),
        // controls: in place
        ("_.r + 1", "6"),
        ("(x -> x + 1) _.r", "6"),
        ("_.r = 5", "#true"),
        ("_.z = 5", "#true"),
        ("[_.z, 2]", "[5, 2]"),
        ("~%Str./len (_.u)", "2"),
    ];
    for (e, want) in cases {
        check(&mut bad, e, w.eval(e), want);
    }
    // A coordinate narrowed later: the alias and the test follow it.
    // v0.92.0: `b` stays @int and `c` #false once `a` is 5.
    let v = Ws::new("r1n");
    v.commit("a: @int\nb: a\nc: a = 5\n");
    v.commit("a: 5\n");
    let narrowed = [("b", "5"), ("c", "#true"), ("_.a", "5"), ("_.a = 5", "#true")];
    for (e, want) in narrowed {
        check(&mut bad, &format!("narrowed: {e}"), v.eval(e), want);
    }
    verdict(bad, cases.len() + narrowed.len());
}

/// r2 — A program answers the same whichever order its definitions are
/// written in. v0.92.0: with `z` last, `j1` ⟹ #false and `j6` ⟹ [_].
#[test]
fn r2_a_program_answers_the_same_in_any_order() {
    let w = Ws::new("r2");
    let defs = "j1: z = 5\nj5: (x -> x + 1) z\nj6: [z]\nj7: z & 5\nj8: ~%Str./len (s)\n";
    let first = format!("z: 5\ns: \"ab\"\n{defs}");
    let last = format!("{defs}z: 5\ns: \"ab\"\n");
    let mut bad = Vec::new();
    let cases = [("j1", "#true"), ("j5", "6"), ("j6", "[5]"), ("j7", "5"), ("j8", "2")];
    for (p, want) in cases {
        check(&mut bad, &format!("{p}, z first"), w.observe(&first, p), want);
        check(&mut bad, &format!("{p}, z last"), w.observe(&last, p), want);
    }
    verdict(bad, cases.len() * 2);
}

/// r3 — Commit keeps a computation over a reference as itself; it does not
/// decide "⊥" by forcing with the reference unread. v0.92.0: `q`, `q0`, `q2`
/// are ⊥ #conflict after commit (`run` answers 2). Controls: a real ⊥
/// stays one (D33/D44), and its cause is kept.
#[test]
fn r3_a_computation_over_a_reference_commits_as_itself() {
    let w = Ws::new("r3");
    let src = "n: 1\nr: <<n>>\nq: r + 1\nq0: r + 0\n~p: <<n>>\nq2: ~p + 1\nx: r & 2\nc: c + 1\n";
    w.commit(src);
    let mut bad = Vec::new();
    let cases = [
        ("q", "2"),
        ("q0", "1"),
        ("q2", "2"),
        ("x", "_|_ ;; %cause: #conflict"),
        ("c.%cause", "#divergent"),
    ];
    for (e, want) in cases {
        check(&mut bad, e, w.eval(e), want);
    }
    check(&mut bad, "run q", w.observe("n: 1\nr: <<n>>\nq: r + 1\n", "q"), "2");
    verdict(bad, cases.len() + 1);
}

/// r4 — A stored open term reached through a name is read where it is read:
/// under the frame's `$`, as the same term named directly is. v0.92.0: every
/// cell but the controls is `{ a: ⊥ #no_context }`.
#[test]
fn r4_an_open_term_read_through_a_name_binds_where_it_is_read() {
    let w = Ws::new("r4");
    let src = "t: { a: $.b + 1 }\nw: t\ng: { m: t }\ng3: { m: _.t }\n";
    w.commit(src);
    let mut bad = Vec::new();
    let cases = [
        ("{ b: 1 } |> (x -> _.w)", "{ a: 2 }"),
        ("{ b: 1 } |> (x -> _.g.m)", "{ a: 2 }"),
        ("{ b: 1 } |> (x -> _.g3.m)", "{ a: 2 }"),
        ("(x -> _.w) { b: 1 }", "{ a: 2 }"),
        // controls: named directly; a pipe binds the template it is handed
        ("{ b: 1 } |> (x -> _.t)", "{ a: 2 }"),
        ("{ b: 1 } |> _.w", "{ a: 2 b: 1 }"),
    ];
    for (e, want) in cases {
        check(&mut bad, e, w.eval(e), want);
    }
    let prog = format!("{src}o1: {{ b: 1 }} |> (x -> w)\no2: {{ b: 1 }} |> (x -> g.m)\no3: {{ b: 1 }} |> (x -> t)\n");
    check(&mut bad, "run o1", w.observe(&prog, "o1"), "{ a: 2 }");
    check(&mut bad, "run o2", w.observe(&prog, "o2"), "{ a: 2 }");
    check(&mut bad, "run o3 (control)", w.observe(&prog, "o3"), "{ a: 2 }");
    verdict(bad, cases.len() + 3);
}

/// r5 — A loop's cause is the loop's: a transform taken by whoever reads it,
/// outside the loop, does not make a static cycle divergent. v0.92.0:
/// `(_.c.a2).%cause` ⟹ #divergent in the program that answers
/// `(c.a2).%cause` ⟹ #static_cycle. Controls: loops with a transform in
/// them stay #divergent.
#[test]
fn r5_a_loop_has_the_same_cause_for_every_reader() {
    let w = Ws::new("r5");
    let prog = "c: { a2: b2, b2: a2 }\nout: (c.a2).%cause\nout3: (_.c.a2).%cause\nv: c.a2\nout4: v.%cause\n\
                a: b + 1\nb: a\nd1: a.%cause\nx: x + 1\nd2: x.%cause\n";
    let mut bad = Vec::new();
    let cases = [
        ("out", "#static_cycle"),
        ("out3", "#static_cycle"),
        ("out4", "#static_cycle"),
        ("d1", "#divergent"),
        ("d2", "#divergent"),
    ];
    for (p, want) in cases {
        check(&mut bad, &format!("run {p}"), w.observe(prog, p), want);
    }
    w.commit("c: { a2: b2, b2: a2 }\n");
    check(&mut bad, "eval (c.a2).%cause", w.eval("(c.a2).%cause"), "#static_cycle");
    check(&mut bad, "eval (_.c.a2).%cause", w.eval("(_.c.a2).%cause"), "#static_cycle");
    verdict(bad, cases.len() + 2);
}

/// r6 (R-1) — A table whose patterns read the universe is read again, as
/// the same table written in place is: its patterns are a reading too. The
/// first round kept tables out as "construction" — the acceptor's addition,
/// not the ruling's criterion. v0.92.0 and the first round: `k 5` stays ⊥
/// #conflict (the pattern froze to `_` while `n` was absent). Not here: an
/// application `j: k 5` committed while `n` is absent is a real ⊥ then (two
/// branches match `_`), and ⊥ is kept (D33/D44) — the Inbox row on
/// operations that answer ⊥ for `_`.
#[test]
fn r6_a_table_that_read_the_universe_reads_it_again() {
    let w = Ws::new("r6");
    w.commit("k: { @{ _.n }: 1, _: 0 }\nk2: { @{ n }: 1, _: 0 }\n");
    w.commit("n: 5\n");
    let mut bad = Vec::new();
    let cases = [
        ("k 5", "1"),
        ("k 4", "0"),
        ("k2 5", "1"),
        // controls: in place
        ("{ @{ _.n }: 1, _: 0 } 5", "1"),
        ("{ @{ _.n }: 1, _: 0 } 4", "0"),
    ];
    for (e, want) in cases {
        check(&mut bad, e, w.eval(e), want);
    }
    verdict(bad, cases.len());
}

// ── Green ────────────────────────────────────────────────────────────────

/// g1 — What reads nothing keeps its answer and its root; what is not a
/// derivation keeps its form; an effect still runs once and is stored as
/// its answer, and so is a reading of one (D46 (ii)). Every root is v0.92.0's.
#[test]
fn g1_what_reads_nothing_keeps_its_root() {
    let mut bad = Vec::new();
    let cases = [
        ("c1: ~%Math./add (1, 1)\n", "62c61785a4a3be671547891247ad11b5a282f3c881915460fa2f8eebae0473ed"),
        ("c2: (x -> x + 1) 2\n", "cf1330a656b92c3a21b359a7257be329535c09675b9c97435409c899ae1e719d"),
        ("k: { a: 1 } & { b: 2 }\n", "917b3ddb841747501e42d3618c1345fb0c401292d78ec953bb8bb38ee670a482"),
        ("a: 1\nb: a + 1\n", "6ef35a73ba9e12629b6d42bc27db08cf1469980e1ae9a3ca071533e167ec9c8e"),
        ("v: <<_.>> |> <<_.>>\n", "5ce55aa56a6260f0a38232802ce05cd798e7ec8b250c873b7d15c2b22022d168"),
        ("t: { a: 1 }\nc: { r: <<t>> }\nk: c.r\n", "e027a82b8688874a6cc7dc4a338baee22abae5de2559dc44ff1d7818ecb3a3d9"),
        ("t: { a: $.b + 1 }\ng: { m: t }\n", "505b71eeebf318bf7ce54529d05f0d9e4b2c4e5ef218c60db6ba757d4f366f79"),
        ("p: \"x.txt\"\nw: ~%Io./append_file (p, \"a\")\n", "77a310dff48f87848b190783ab27644ec2b22f291545059de9725e408dcdcf5f"),
        // a table whose patterns read nothing (R-1: one whose patterns read
        // a name is r6, not a guard)
        ("k: { @{ 4 }: 1, _: 0 }\n", "9a68d4a3badf7c39c6fe2d2a637c598b5eb08d3d433678b897b0b1116a0c3123"),
        ("k: { @{ @int }: 1, _: 0 }\n", "ae9e8a566f5a63a580d593de542f7cedaf954c1c1a4d49b9215fb5b4407619a1"),
    ];
    for (i, (src, want)) in cases.iter().enumerate() {
        let w = Ws::new(&format!("g1{i}"));
        w.commit(src);
        check(&mut bad, &format!("root of {:?}", src.replace('\n', "; ")), w.root(), want);
        if src.contains("append_file") {
            let _ = w.eval("w");
            let _ = w.eval("_.w");
            let wrote = fs::read_to_string(w.ws.join("x.txt")).unwrap_or_default();
            check(&mut bad, "the effect ran once", wrote, "a");
            check(&mut bad, "w", w.eval("w"), "#true");
        }
    }
    // A definition that reads an effectful coordinate is not pure: it keeps
    // its answer (conformance L2-83 is this cell on the runner's surface).
    let w = Ws::new("g1e");
    check(&mut bad, "run out (reads an #io coordinate)", w.observe("t: ~%Time.now _\nout: t.%effect\n", "out"), "#io");
    verdict(bad, cases.len() + 3);
}

/// g2 — A one-shot question is answered where it is asked: reading stored
/// data leaves no observation savepoint (SPEC_10, D91), and a question that
/// reduces a thunk leaves one, as on v0.92.0.
#[test]
fn g2_a_question_is_not_a_definition() {
    let w = Ws::new("g2");
    w.commit("a: 1\nb: a + 1\nr: <<a>>\n");
    let mut bad = Vec::new();
    let mut n = w.savepoints();
    for (e, want, leaves) in [
        ("_.a", "1", false),
        ("a", "1", false),
        ("[a]", "[1]", false),
        ("_.a = 1", "#true", false),
        ("r", "1", false),
        ("_.a + 1", "2", true),
        ("_.b", "2", true),
    ] {
        check(&mut bad, e, w.eval(e), want);
        let m = w.savepoints();
        check(&mut bad, &format!("savepoints after {e}"), (m - n).to_string(), if leaves { "1" } else { "0" });
        n = m;
    }
    check(&mut bad, "_.w-style template outside a frame", {
        let v = Ws::new("g2t");
        v.commit("t: { a: $.b + 1 }\nw: t\n");
        v.eval("_.w")
    }, &format!("{{ a: {NO_CONTEXT} }}"));
    verdict(bad, 15);
}
