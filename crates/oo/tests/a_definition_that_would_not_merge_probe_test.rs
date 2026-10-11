// A definition that would not merge.
// Order: nlang-tools/docs/a_definition_that_would_not_merge_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-081.  Ruling: meta/oo/STATUS.md D105.
// Spec: SPEC_07 §5.3 (定義 1 the formula, 定義 3 cases A and B), §1.1 (one
// table, dispatch by specificity), §1.1.2 (one branch per constraint; its
// bodies are a set, forced only when selected); SYNTAX_11 §2 #6 (an arrow
// is a table row; a binder-only arrow is the Top row), §4 #6 (a conflict
// collapses at application, a definition is not checked).
//
// ── What is wrong (v0.82.0) ──────────────────────────────────────────────
//
// Two tables meet as one table; so do two arrows with patterns. An arrow
// that is only a binder does not:
//   * same binder: the bodies' CODE meets at merge time —
//     `((x -> x) & (x -> 5)) 5` ⊥ "x vs 5" (both readings: 5);
//     SPEC_07 §5.3 案例 B ⊥ (spec: 5)
//   * other binder, or beside a table or a builtin: #missing_key —
//     `((x -> 1) & (y -> 2)) 4`, `((x -> 1) & { @{ @int }: 2 }) 5`,
//     `(~%Math./add 1 & (x -> x + 1)) 5`
// And at evolve, re-defining `f: (x -> x + 1)` as `f: (x -> x * 2)` is
// refused, while the same move on a table is taken.
//
// D105: `f & g` of morphisms is one table — the rows of both, one branch
// per constraint, a branch's bodies meet (each with its own binder) only
// when dispatch selects it. A binder-only arrow is the Top row; a morphism
// with no rows (a builtin, a partial application) is the Top row whose body
// applies it.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("would-not-merge-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo`, killing it after 60 s: a meet must answer, not hang.
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

    fn eval(&self, e: &str) -> String {
        self.oo(&["eval", e]).0
    }

    /// Evolve one source file; the exit code and output.
    fn evolve(&self, src: &str) -> (String, i32) {
        fs::write(self.ws.join("a.n"), src).unwrap();
        self.oo(&["evolve", "a.n"])
    }

    /// Evolve then commit one source file in this universe.
    fn commit(&self, src: &str) {
        let (o, rc) = self.evolve(src);
        assert_eq!(rc, 0, "evolve {src:?}: {o}");
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        assert_eq!(rc, 0, "commit {src:?}: {o}");
    }

    /// The root digest the last commit wrote.
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

    /// `run --observe <target>` on a file in a directory with no universe.
    fn run(&self, src: &str, target: &str) -> String {
        fs::write(self.ws.join("r.n"), src).unwrap();
        self.oo(&["run", "--observe", target, "r.n"]).0
    }
}

fn is(o: &str, want: &str) -> bool {
    o == want
}

/// ⊥ with this cause.
fn bot(o: &str, cause: &str) -> bool {
    o.starts_with("_|_") && o.contains(&format!("%cause: {cause}"))
}

/// Run every `(expression, check, expectation)` and report all misses at once.
fn table(w: &Ws, cases: &[(&str, fn(&str, &str) -> bool, &str)]) {
    let mut wrong = Vec::new();
    for (e, check, want) in cases {
        let o = w.eval(e);
        if !check(&o, want) {
            wrong.push(format!("  {e}\n      want {want}\n      got  {o}"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// Commit each source in a fresh universe; compare the root it writes.
fn roots(tag: &str, cases: &[(&str, &str)], why: &str) {
    let mut wrong = Vec::new();
    for (i, (src, want)) in cases.iter().enumerate() {
        let w = Ws::new(&format!("{tag}-{i}"));
        w.commit(src);
        let got = w.root();
        if got != *want {
            wrong.push(format!("  {src:?}\n      want {want}\n      got  {got}"));
        }
    }
    assert!(wrong.is_empty(), "{why}\n{}", wrong.join("\n"));
}

// ── Red: two definitions merge into one table ───────────────────────────

/// SPEC_07 §5.3 定義 3 案例 B, as the spec writes it, and its formula's
/// other cell. 案例 A (a conflict) is guard g4.
#[test]
fn r1_the_spec_case_b_runs() {
    let w = Ws::new("r1");
    let src = "/check_int: x -> @int\n/check_pos: x -> @{ $ & !(..0) }\n/is_pos_int: /check_int & /check_pos\nr: /is_pos_int 5\nn: /is_pos_int -3\n";
    let r = w.run(src, "r");
    let n = w.run(src, "n");
    assert!(is(&r, "5"), "/is_pos_int 5: want 5, got {r}");
    assert!(bot(&n, "#conflict"), "/is_pos_int -3: want ⊥ #conflict, got {n}");
}

/// Same binder: the bodies meet when applied, not their code when merged.
#[test]
fn r2_same_binder_bodies_meet_when_applied() {
    let w = Ws::new("r2");
    table(&w, &[
        ("((x -> x) & (x -> 5)) 5", is, "5"),
        ("((x -> x) & (x -> 5)) 6", bot, "#conflict"),
        ("((x -> x + 1) & (x -> x * 2)) 1", is, "2"),
        ("((x -> x + 1) & (x -> x * 2)) 3", bot, "#conflict"),
    ]);
}

/// Another binder: the same Top row, each body with its own binder.
#[test]
fn r3_other_binders_are_the_same_row() {
    let w = Ws::new("r3");
    table(&w, &[
        ("((x -> 1) & (y -> 1)) 4", is, "1"),
        ("((x -> 1) & (y -> 2)) 4", bot, "#conflict"),
        ("((x -> x + 1) & (y -> y * 2)) 1", is, "2"),
        ("(((x, y) -> x + y) & ((a, b) -> a + b)) (1, 2)", is, "3"),
    ]);
}

/// Beside a table or a patterned arrow, the binder-only arrow is the Top
/// row: the more specific row wins where it matches.
#[test]
fn r4_the_top_row_beside_patterns() {
    let w = Ws::new("r4");
    table(&w, &[
        ("((x -> 1) & { @{ @int }: 2 }) 5", is, "2"),
        (r#"((x -> 1) & { @{ @int }: 2 }) "s""#, is, "1"),
        (r#"({ @{ @int }: "A" } & (x -> "B")) 4"#, is, r#""A""#),
        (r#"({ @{ @int }: "A" } & (x -> "B")) "s""#, is, r#""B""#),
        (r#"((x @int -> "A") & (y -> "B")) #t"#, is, r#""B""#),
    ]);
}

/// A morphism with no rows — a builtin, a partial application — is the Top
/// row whose body applies it.
#[test]
fn r5_a_builtin_is_the_top_row() {
    let w = Ws::new("r5");
    table(&w, &[
        ("(~%Math./add 1 & (x -> x + 1)) 5", is, "6"),
        ("(~%Math./add 1 & (x -> x + 2)) 5", bot, "#conflict"),
        ("(~%Math./add 1 & { @{ 5 }: 99 }) 5", is, "99"),
        ("(~%Math./add 1 & { @{ 5 }: 99 }) 6", is, "7"),
    ]);
}

/// Curried arrows and partial applications meet the same way at each step.
#[test]
fn r6_curried() {
    let w = Ws::new("r6");
    table(&w, &[
        ("((x -> y -> x + y) & (x -> y -> y + x)) 1 2", is, "3"),
        ("(((x -> y -> x + y) 1) & (z -> z + 1)) 2", is, "3"),
        ("((x -> y -> x) & (x -> y -> y)) 1 2", bot, "#conflict"),
    ]);
}

/// One table, one normal form: order and grouping of the meet are not in
/// the bytes, and it answers.
#[test]
fn r7_one_normal_form() {
    let a = "t: (x -> x + 1) & (y -> y * 2)\n";
    let b = "t: (y -> y * 2) & (x -> x + 1)\n";
    let c = "t: ((x -> x + 1) & (y -> y * 2)) & (x -> x + 1)\n";
    let mut rs = Vec::new();
    let mut ans = Vec::new();
    for (i, src) in [a, b, c].iter().enumerate() {
        let w = Ws::new(&format!("r7-{i}"));
        w.commit(src);
        rs.push(w.root());
        ans.push(w.eval("_.t 1"));
    }
    assert!(rs[0] == rs[1] && rs[1] == rs[2], "one table, one root\n  {rs:?}");
    assert!(ans.iter().all(|a| a == "2"), "_.t 1: want 2 each time, got {ans:?}");
}

/// A merged definition stored in a root answers after commit as it does in
/// place. (Top-level fields: a table nested in a container loses its
/// patterns at evolve since v0.78.0 — a separate defect, filed in the Inbox
/// on 2026-10-12, not this card.)
#[test]
fn r8_after_commit() {
    let w = Ws::new("r8");
    let src = "g: (x -> x + 1)\nh: g & (y -> y * 2)\nm: h 1\nn: (h & { @{ 3 }: 9 }) 3\n";
    let before = [w.run(src, "m"), w.run(src, "n")];
    w.commit(src);
    table(&w, &[
        ("_.m", is, "2"),
        ("_.n", is, "9"),
        ("_.h 1", is, "2"),
        ("_.h 3", bot, "#conflict"),
    ]);
    let after = [w.eval("_.m"), w.eval("_.n")];
    assert_eq!(before, after, "run --observe and the committed root answer alike");
}

/// Re-defining an arrow is a merge, as it is for a table: evolve takes it,
/// and the conflict collapses where the bodies disagree (SYNTAX_11 §4 #6).
#[test]
fn r9_redefinition_merges_at_evolve() {
    let w = Ws::new("r9");
    w.commit("f: (x -> x)\n");
    let (o, rc) = w.evolve("f: (x -> 5)\n");
    assert_eq!(rc, 0, "evolve f: (x -> 5) over f: (x -> x): {o}");
    let (o, rc) = w.oo(&["commit", "-m", "b"]);
    assert_eq!(rc, 0, "commit: {o}");
    table(&w, &[
        ("_.f 5", is, "5"),
        ("_.f 6", bot, "#conflict"),
    ]);
}

/// Each selected body runs once; an unselected one never runs.
#[test]
fn r10_bodies_run_only_when_selected() {
    let w = Ws::new("r10");
    let o = w.run("r: ((x -> ~%Io./append_file (\"log.txt\", \"a\")) & (y -> ~%Io./append_file (\"log.txt\", \"b\"))) 5\n", "r");
    let mut log: Vec<char> = fs::read_to_string(w.ws.join("log.txt")).unwrap_or_default().chars().collect();
    log.sort();
    assert!(log == vec!['a', 'b'] && !o.starts_with("_|_"), "both Top bodies run, once each\n  r:   {o}\n  log: {log:?}");
    let w = Ws::new("r10b");
    let o = w.run("r: ((x -> ~%Io./append_file (\"log.txt\", \"a\")) & { @{ @int }: 0 }) 5\n", "r");
    let log = fs::read_to_string(w.ws.join("log.txt")).unwrap_or_default();
    assert!(o.starts_with("0") && log.is_empty(), "the Top body is not selected for 5\n  r:   {o}\n  log: {log:?}");
}

// ── Guards: what is already right stays so ──────────────────────────────

/// A lone morphism keeps its root; a meet with itself is itself.
#[test]
fn g1_a_lone_morphism_keeps_its_root() {
    roots("g1", &[
        ("f: (x -> x + 1)\n", "039d07351a998261d3150af05a84bd0fcf0ad4133b24d644d35ab1e59517d2b4"),
        ("f: (x -> x + 1) & (x -> x + 1)\n", "039d07351a998261d3150af05a84bd0fcf0ad4133b24d644d35ab1e59517d2b4"),
        ("m: (_ -> 1)\n", "c23f8a56608b9aa32b3fa089aa5d54ec4b0385b990d0cc2fe695970db114fb85"),
        ("p: (x @int -> x)\n", "8b3111a414e8b72b013abaf1da291b995497ce47bce8a796016c7f1fc2099c6b"),
        ("b: ~%Math./add 1\n", "aebe4b5d40bd461403c00c7d0c5c3a016fb1a77ac6655db7361001dbc1901088"),
        ("b: ~%Math./add 1 & ~%Math./add 1\n", "aebe4b5d40bd461403c00c7d0c5c3a016fb1a77ac6655db7361001dbc1901088"),
    ], "a lone morphism, or its meet with itself, keeps its root");
    let w = Ws::new("g1e");
    table(&w, &[
        ("(x -> 1) = ((x -> 1) & (x -> 1))", is, "#true"),
        ("(~%Math./add 1 & ~%Math./add 1) 5", is, "6"),
    ]);
}

/// Two tables, and two patterned arrows, already meet as one table.
#[test]
fn g2_tables_meet_as_before() {
    let w = Ws::new("g2");
    table(&w, &[
        (r#"({ @{ @int }: "A" } & { @{ @str }: "B" }) 4"#, is, r#""A""#),
        (r#"({ @{ @int }: "A" } & { @{ 4 }: "B" }) 4"#, is, r#""B""#),
        (r#"({ @{ @int }: "A" } & { @{ @int }: "B" }) 4"#, bot, "#conflict"),
        ("((x @int -> x) & (y @int -> 5)) 5", is, "5"),
        (r#"((x @int -> "A") & (y @str -> "B")) "s""#, is, r#""B""#),
    ]);
    roots("g2r", &[
        ("t: { @{ @int }: \"A\" } & { @{ @str }: \"B\" }\n", "bf22771bb06b2da1c435a2aa2331676f26b384289facd8129d300e25498ec29a"),
    ], "a meet of two tables keeps its root");
}

/// Not this card: a morphism meeting plain data (SPEC_07 §5.3 定義 2) or an
/// atom (§5.3 邊界) answers as before; a union of morphisms is D104's.
#[test]
fn g3_out_of_scope_unchanged() {
    let w = Ws::new("g3");
    table(&w, &[
        (r#"((x -> 1) & { name: "inc" }) 4"#, bot, "#missing_key"),
        ("((x -> 1) & 5)", bot, "#conflict"),
        ("((x -> 1) | (y -> 2)) 4 = (1 | 2)", is, "#true"),
    ]);
}

/// SPEC_07 §5.3 定義 3 案例 A: two outputs that disagree are a conflict.
#[test]
fn g4_the_spec_case_a_conflicts() {
    let w = Ws::new("g4");
    let src = "/f: x -> 1\n/g: x -> 2\n/h: /f & /g\nr: /h 10\n";
    let r = w.run(src, "r");
    assert!(bot(&r, "#conflict"), "/h 10: want ⊥ #conflict, got {r}");
}

/// Re-stating the same definition is not a change.
#[test]
fn g5_restating_a_definition_is_no_change() {
    let w = Ws::new("g5");
    w.commit("f: (x -> x + 1)\n");
    let before = w.root();
    let (o, rc) = w.evolve("f: (x -> x + 1)\n");
    assert_eq!(rc, 0, "evolve the same f: {o}");
    let (o, _) = w.oo(&["commit", "-m", "b"]);
    assert!(o.contains("Nothing to commit") || w.root() == before, "same f, same root: {o}");
    assert_eq!(w.eval("_.f 3"), "4");
}
