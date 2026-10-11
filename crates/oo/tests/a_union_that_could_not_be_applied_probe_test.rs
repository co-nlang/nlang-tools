// A union that could not be applied.
// Order: nlang-tools/docs/a_union_that_could_not_be_applied_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-080.  Ruling: meta/oo/STATUS.md D104.
// Spec: SPEC_07 §5.2 定義 4 (the two examples), §1.1 (dispatch inside one
// member), SPEC_06 §1.3 #4 (⊥ branches prune), REAL_04 §4 (all ⊥: the
// primary cause, as it arrived), SYNTAX_11 §3 (`/fmt`).
//
// ── What is wrong (v0.81.0; v0.27.0 … v0.75.0 the same) ──────────────────
//
// Applying a union of morphisms is ⊥ #conflict, always:
//   * `((x @int -> x + 1) | (x @str -> x + "!")) 5`   ⊥  (spec: 6)
//   * `((x -> 1) | (y -> 2)) 4`                         ⊥
//   * two branches that both miss                       #conflict, not
//     #no_matching_branch
// The union value itself is fine; navigation and an applied-to union
// distribute; only the applied union does not.
// And when an applied-to union loses every branch, the cause is replaced:
// `(x @int -> x) ("a" | "b")` is #conflict although both branches are
// #no_matching_branch.
//
// D104: a morphism's `|` is a Combo's `|`. Applying a union distributes —
// (A | B) x = A x | B x, ⊥ branches pruned, all ⊥ keeps the primary cause.
// Specificity is compared inside one member only; a table compares across
// its branches, a union does not compare across its members.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("union-not-applied-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo`, killing it after 60 s: a union must answer, not hang.
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

    /// Evolve then commit one source file in this universe.
    fn commit(&self, src: &str) {
        fs::write(self.ws.join("a.n"), src).unwrap();
        let (o, rc) = self.oo(&["evolve", "a.n"]);
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

// ── Red: the union is applied ───────────────────────────────────────────

/// SPEC_07 §5.2 定義 4's own examples, and SYNTAX_11 §3's `/fmt`.
#[test]
fn r1_the_spec_examples_run() {
    let w = Ws::new("r1");
    table(&w, &[
        (r#"((x @int -> x + 1) | (x @str -> x + "!")) 5"#, is, "6"),
        (r#"((x @int -> x + 1) | (x @str -> x + "!")) "hi""#, is, r#""hi!""#),
    ]);
    w.commit("/fmt: (x @int -> x + 1) | (x @str -> x + \"!\")\nu: /fmt 5\n");
    table(&w, &[
        ("_.u", is, "6"),
        ("/fmt 5", is, "6"),
        (r#"/fmt "a""#, is, r#""a!""#),
    ]);
}

/// D104: a member does not compete with another member on specificity;
/// every member that answers is in the result.
#[test]
fn r2_members_do_not_compete() {
    let w = Ws::new("r2");
    table(&w, &[
        (r#"(((@int -> "int") | (5 -> "five")) 5) = ("int" | "five")"#, is, "#true"),
        ("(((x -> 1) | (y -> 2)) 4) = (1 | 2)", is, "#true"),
        ("(((x @int -> x) | (y @int -> y + 100)) 1) = (1 | 101)", is, "#true"),
        ("(((x -> x.k) | (x -> x.j)) { k: 1, j: 2 }) = (1 | 2)", is, "#true"),
        ("(({ @{ @int }: 1 } | { @{ @str }: 2 }) 4)", is, "1"),
    ]);
}

/// A member with no stored pattern is applied as itself: a builtin, a
/// partial application, a Combo looked up by key; an atom is not
/// applicable and drops out.
#[test]
fn r3_every_kind_of_member_applies_as_itself() {
    let w = Ws::new("r3");
    table(&w, &[
        ("((~%Math./add 1 | (x @int -> x * 10)) 4) = (5 | 40)", is, "#true"),
        ("((~%Math./add 1 | (x -> x)) 4) = (5 | 4)", is, "#true"),
        (r#"(({ a: 1 } | (x @str -> "s")) "a") = (1 | "s")"#, is, "#true"),
        ("((x -> x + 1) | 5) 4", is, "5"),
    ]);
}

/// ⊥ members prune; all members ⊥ keeps the primary cause (REAL_04 §4).
#[test]
fn r4_bottom_members_prune_and_keep_their_cause() {
    let w = Ws::new("r4");
    table(&w, &[
        ("((x -> 1 & 2) | (y -> 3)) 4", is, "3"),
        (r#"((@int -> "int") | (@str -> "s")) #a"#, bot, "#no_matching_branch"),
        ("((x -> 1 & 2) | (y -> 1 & 3)) 4", bot, "#conflict"),
    ]);
}

/// The applied-to side loses its cause today when every branch is ⊥.
#[test]
fn r5_an_applied_to_union_keeps_its_cause() {
    let w = Ws::new("r5");
    table(&w, &[
        (r#"(x @int -> x) ("a" | "b")"#, bot, "#no_matching_branch"),
        (r#"{ a: 1 } ("b" | "c")"#, bot, "#no_matching_branch"),
        (r#"("a" | "b") |> (x @int -> x)"#, bot, "#no_matching_branch"),
    ]);
}

/// Currying: each step applies a union.
#[test]
fn r6_curried_members() {
    let w = Ws::new("r6");
    table(&w, &[
        ("(((x y -> x + y) | (a b -> a * b)) 2 3) = (5 | 6)", is, "#true"),
        ("((((x y -> x + y) | (a b -> a * b)) 2) 3) = (5 | 6)", is, "#true"),
    ]);
}

/// The pipe is application: x |> (A | B) = (x |> A) | (x |> B), whether the
/// union is written there or reached by a path.
#[test]
fn r7_the_pipe_distributes_too() {
    let w = Ws::new("r7");
    table(&w, &[
        ("(5 |> ((x -> x + 1) | (y -> y * 10))) = (6 | 50)", is, "#true"),
        (r#""hi" |> ((x @int -> x + 1) | (x @str -> x + "!"))"#, is, r#""hi!""#),
        ("#a |> ((@int -> 1) | (@str -> 2))", bot, "#no_matching_branch"),
        ("((1 | 2) |> ((x -> x + 1) | (y -> y * 10))) = (2 | 3 | 10 | 20)", is, "#true"),
    ]);
    w.commit("f: (x -> x + 1)\ng: (x -> x * 10)\nh: f | g\nw1: 5 |> h\nw2: 5 |> (f | g)\nt: { w3: 5 |> h }\n");
    table(&w, &[
        ("_.w1 = (6 | 50)", is, "#true"),
        ("_.w2 = (6 | 50)", is, "#true"),
        ("_.t.w3 = (6 | 50)", is, "#true"),
        ("(5 |> h) = (6 | 50)", is, "#true"),
    ]);
}

/// Named, nested, committed, and the same in one process.
#[test]
fn r8_a_named_union_after_commit() {
    let w = Ws::new("r8");
    let src = "f: (x -> x + 1)\ng: (x -> x * 10)\nh: f | g\nr: h 4\nt: { k: h 2, m: (h | (y -> 0)) 3 }\n";
    let before = [w.run(src, "r"), w.run(src, "t.k"), w.run(src, "t.m")];
    w.commit(src);
    table(&w, &[
        ("_.r = (5 | 40)", is, "#true"),
        ("_.t.k = (3 | 20)", is, "#true"),
        ("_.t.m = (0 | 4 | 30)", is, "#true"),
        ("(h 3) = (4 | 30)", is, "#true"),
        ("(_.h 3) = (4 | 30)", is, "#true"),
    ]);
    let after = [w.eval("_.r"), w.eval("_.t.k"), w.eval("_.t.m")];
    assert_eq!(before, after, "run --observe and the committed root answer alike");
}

/// Both sides are unions: every pair.
#[test]
fn r9_both_sides_unions() {
    let w = Ws::new("r9");
    table(&w, &[
        ("(((x -> x + 1) | (y -> y * 10)) (1 | 2)) = (2 | 3 | 10 | 20)", is, "#true"),
    ]);
}

/// Each member is applied once: its effect runs once.
#[test]
fn r10_each_member_runs_once() {
    let w = Ws::new("r10");
    let src = "r: ((x -> ~%Io./append_file (\"log.txt\", \"b\")) | (y -> ~%Io./append_file (\"log.txt\", \"c\"))) 2\n";
    let o = w.run(src, "r");
    let log = fs::read_to_string(w.ws.join("log.txt")).unwrap_or_default();
    let mut got: Vec<char> = log.chars().collect();
    got.sort();
    assert!(
        got == vec!['b', 'c'] && !o.starts_with("_|_"),
        "each member's effect runs once\n  r:   {o}\n  log: {log:?}"
    );
}

// ── Guards: green today, green after ────────────────────────────────────

/// Inside one member, dispatch is the minimal-element rule, as before.
#[test]
fn g1_one_member_still_dispatches_by_specificity() {
    let w = Ws::new("g1");
    table(&w, &[
        (r#"{ @{ @int }: "int", 5: "five" } 5"#, is, r#""five""#),
        (r#"{ @{ @int & 4.. }: "A", @{ @int & ..6 }: "B" } 9"#, is, r#""A""#),
        ("(x @int -> x + 1) 5", is, "6"),
        ("((x -> x) | (x -> x)) 7", is, "7"),
        (r#"{ @{ @int }: "n", _: "other" } "s""#, is, r#""other""#),
    ]);
}

/// An applied-to union with a surviving branch answers as before.
#[test]
fn g2_applied_to_union_with_a_survivor() {
    let w = Ws::new("g2");
    table(&w, &[
        (r#"(x @int -> x) ("a" | 3)"#, is, "3"),
        ("((1 | 2) (x -> x + 1)) = (2 | 3)", is, "#true"),
        ("((1 | 2) |> (x -> x * 10)) = (10 | 20)", is, "#true"),
    ]);
}

/// A pipe into a union of plain values is a meet with each, as before.
#[test]
fn g3_pipe_into_a_union_of_values() {
    let w = Ws::new("g3");
    table(&w, &[
        ("({ a: 1 } |> ({ b: 2 } | { c: 3 })) = ({ a: 1, b: 2 } | { a: 1, c: 3 })", is, "#true"),
        ("{ a: 1 } |> ({ a: 2 } | { c: 3 })", is, "{\n  a: 1\n  c: 3\n}"),
        ("5 |> (@int | @str)", is, "5"),
        ("5 |> (#ok | #no)", bot, "#conflict"),
    ]);
}

/// Union values do not move; atoms are still not applicable.
#[test]
fn g4_union_values_and_atoms() {
    let w = Ws::new("g4");
    table(&w, &[
        ("(({ a: 1 } | { a: 2 }).a) = (1 | 2)", is, "#true"),
        ("(1 | 2) 3", bot, "#conflict"),
        ("5 4", bot, "#conflict"),
    ]);
    let roots = [
        ("h: (x -> x + 1) | (x -> x * 10)\n", "0ca12aa4a3b1fc955657cfaf1892c4ba607ff7bdb4f4f87a75eb6523bdb617f7"),
        ("/fmt: (x @int -> x + 1) | (x @str -> x + \"!\")\n", "0e38286c2c4eff41a3a8314e50cad25685b388cc899f2c7b3d65a2e5f57b7fc1"),
        ("t: { @{ @int }: \"int\", 5: \"five\" }\n", "9d82851119c87c41b283e56f1e26cd14e5669050f20eff411f74a6d580a4da33"),
        ("u: ({ a: 1 } | { a: 2 }).a\n", "db62a6b124d87ddcf634275ca7cca543cb4fc0300cae6146982eb41094351499"),
    ];
    let mut wrong = Vec::new();
    for (i, (src, want)) in roots.iter().enumerate() {
        let w = Ws::new(&format!("g4-{i}"));
        w.commit(src);
        let got = w.root();
        if got != *want {
            wrong.push(format!("  {src:?}\n      want {want}\n      got  {got}"));
        }
    }
    assert!(wrong.is_empty(), "a value with no application in it keeps its root\n{}", wrong.join("\n"));
}

/// An applied-to union runs each branch once, as before.
#[test]
fn g5_applied_to_union_runs_each_branch_once() {
    let w = Ws::new("g5");
    let o = w.run("r: (1 | 2) |> (x -> ~%Io./append_file (\"log.txt\", \"d\"))\n", "r");
    let log = fs::read_to_string(w.ws.join("log.txt")).unwrap_or_default();
    assert!(log == "dd" && !o.starts_with("_|_"), "two branches, two writes\n  r:   {o}\n  log: {log:?}");
}

/// The meet of morphisms is not this card (SPEC_07 §5.3 定義 1 awaits a
/// ruling): it answers exactly as before.
#[test]
fn g6_the_meet_of_morphisms_is_untouched() {
    let w = Ws::new("g6");
    table(&w, &[
        (r#"({ @{ @int }: "A" } & { @{ @str }: "B" }) 4"#, is, r#""A""#),
        ("((x @int -> x) & (x @str -> x)) 4", is, "4"),
        ("((x -> 1) & (y -> 2)) 4", bot, "#missing_key"),
        ("(((x -> x + 1) | (y -> y * 10)) & (z -> z)) 4", bot, "#missing_key"),
    ]);
}
