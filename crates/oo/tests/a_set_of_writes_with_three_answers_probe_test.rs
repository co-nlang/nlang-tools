// A set of writes with three answers.
// Order: nlang-tools/docs/a_set_of_writes_with_three_answers_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-101.  D117: a field written with
// two conflicting values keeps the conflict in that field; the record is not
// ⊥ (SPEC_06 §2). A table and a plain record follow one rule, and any
// grouping or order of the same writes is one set of bytes (SPEC_07 §1.1.2,
// D101). The evolve boundary is as before: with no table, the second of two
// conflicting writes is refused by key; with a table, it is a branch.
//
// ── What is wrong (v0.101.0 and before) ─────────────────────────────────
//
// `{ x: 3 } & { x: 4 }` is a whole ⊥, while the literal `{ x: 3, x: 4 }` is
// `{ x: ⊥ }`. With a table the meet is not associative: `({x:3} & {x:4}) & T`
// is ⊥, `(T & {x:3}) & {x:4}` keeps a branch with bodies {3, 4}, and the
// literal keeps a branch with body ⊥ — three roots. The working set folds its
// members in the order of their random file names, so the commit is
// refused or accepted by chance (v0.101.0: 13 of 30 refused).
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("three-answers-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo`, killing it after 120 s: (output, rc).
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
            if start.elapsed() > Duration::from_secs(120) {
                let _ = child.kill();
                let _ = child.wait();
                return (format!("TIMEOUT after 120 s: oo {}", args.join(" ")), -1);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let o = child.wait_with_output().expect("output");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
            o.status.code().unwrap_or(-1),
        )
    }

    /// Evolve `src` from a new file: (rc, output).
    fn evolve(&self, src: &str) -> (i32, String) {
        let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
        let f = format!("f{n}.n");
        fs::write(self.ws.join(&f), src).unwrap();
        let (o, rc) = self.oo(&["evolve", &f]);
        (rc, squash(&o))
    }

    /// Evolve each source as its own file, then commit: the evolve exit codes
    /// and the commit's ("e0 e1 c0").
    fn writes(&self, srcs: &[&str]) -> String {
        let mut out = Vec::new();
        for s in srcs {
            out.push(format!("e{}", self.evolve(s).0));
        }
        let (_, rc) = self.oo(&["commit", "-m", "a"]);
        out.push(format!("c{rc}"));
        out.join(" ")
    }

    /// `oo eval`, the trailing effect note dropped, whitespace collapsed.
    fn eval(&self, e: &str) -> String {
        let s = self.oo(&["eval", e]).0;
        let s = match s.find(";; %effect") {
            Some(i) => s[..i].trim().to_string(),
            None => s,
        };
        squash(&s).chars().take(120).collect()
    }

    fn root(&self) -> String {
        let head = fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default();
        if head.trim().is_empty() {
            return String::new();
        }
        let (o, _) = self.oo(&["inspect", head.trim()]);
        o.lines()
            .find(|l| l.contains("root:"))
            .and_then(|l| l.rsplit(':').next())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    /// Copy `other`'s working-set members in: two writes that did not see
    /// each other, as two concurrent evolves leave them.
    fn take_members_of(&self, other: &Ws) {
        let from = other.ws.join(".oo").join("injections");
        let to = self.ws.join(".oo").join("injections");
        for e in fs::read_dir(&from).unwrap() {
            let p = e.unwrap().path();
            fs::copy(&p, to.join(p.file_name().unwrap())).unwrap();
        }
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

/// `oo eval` from an empty directory (no universe).
fn eval_here(e: &str) -> String {
    let w = Ws::new("eval");
    conflict(&w.eval(e))
}

fn conflict(got: &str) -> String {
    if got.starts_with("_|_") && got.contains("#conflict") { "_|_ #conflict".into() } else { got.into() }
}

/// The root a single evolve of `src` commits.
fn root_of(src: &str) -> String {
    let w = Ws::new("root");
    let got = w.writes(&[src]);
    if got != "e0 c0" {
        return format!("not committed: {got}");
    }
    w.root()
}

const T: &str = "@{ @int }: 0";
/// `t: { @{ @int }: 0, x: 3, x: 4 }` in every form.
const NESTED: &str = "fce318099e5621f92945b3d13c9009fd344244f3d85e95cd9bbca738bd7d58a1";
/// `@{ @int }: 0`, `x: 3`, `x: 4` at the root.
const AT_ROOT: &str = "81a42745d99c7d4c03a51f9027771c0a0f25077c94b70c8159587bde0daa0ad3";

/// r1 — The conflict stays in its field: the other fields read; a nested
/// conflict stays at its own depth.
#[test]
fn r1_a_conflict_stays_in_its_field() {
    let mut bad = Vec::new();
    let cells: [(&str, &str); 6] = [
        ("({ x: 3 } & { x: 4 }).x", "_|_ #conflict"),
        ("({ a: 1, b: 2 } & { a: 2 }).b", "2"),
        ("({ a: 1 } & { a: 2, b: 2 }).b", "2"),
        ("({ a: { b: 1, c: 5 } } & { a: { b: 2 } }).a.c", "5"),
        ("({ a: { b: 1 } } & { a: { b: 2 } }).a.b", "_|_ #conflict"),
        ("(({ x: 3 } & { x: 4 }) & { y: 1 }).y", "1"),
    ];
    for (e, want) in cells {
        check(&mut bad, e, eval_here(e), want);
    }
    verdict(bad, cells.len());
}

/// r2 — Any grouping and order of one set of writes is one root: literal,
/// `&` either way round, with and without a table.
#[test]
fn r2_any_grouping_is_one_root() {
    let mut bad = Vec::new();
    for src in [
        "t: { @{ @int }: 0, x: 3, x: 4 }",
        "t: { x: 3, x: 4, @{ @int }: 0 }",
        "t: { x: 3 } & { x: 4 } & { @{ @int }: 0 }",
        "t: { @{ @int }: 0 } & { x: 3 } & { x: 4 }",
        "t: { x: 4 } & ({ x: 3 } & { @{ @int }: 0 })",
    ] {
        check(&mut bad, src, root_of(src), NESTED);
    }
    let a = root_of("t: ({ x: 3 } & { x: 4 }) & { y: 1 }");
    check(&mut bad, "plain, grouped left = grouped right", root_of("t: { x: 3 } & ({ x: 4 } & { y: 1 })"), &a);
    check(&mut bad, "plain, grouped = literal", root_of("t: { x: 3, x: 4, y: 1 }"), &a);
    let b = root_of("t: { @{ \"b\" }: 2, b: 3 }");
    check(&mut bad, "pattern and data key, & one way", root_of("t: { @{ \"b\" }: 2 } & { b: 3 }"), &b);
    check(&mut bad, "pattern and data key, & the other way", root_of("t: { b: 3 } & { @{ \"b\" }: 2 }"), &b);
    verdict(bad, 5 + 2 + 2);
}

/// r3 — Writes a table holds land in any order and give one root; reading
/// the branch is ⊥, every other argument answers. (A bare `_` in an
/// expression is Top, so the root table is applied through a field.)
#[test]
fn r3_evolves_in_any_order_are_one_root() {
    let mut bad = Vec::new();
    for (label, srcs) in [
        ("T, 3, 4", vec![T, "x: 3", "x: 4"]),
        ("T, 4, 3", vec![T, "x: 4", "x: 3"]),
        ("3, T, 4", vec!["x: 3", T, "x: 4"]),
        ("one file, T first", vec!["@{ @int }: 0\nx: 3\nx: 4"]),
    ] {
        let w = Ws::new("r3");
        let codes: String = srcs.iter().map(|_| "e0 ").collect::<String>() + "c0";
        check(&mut bad, &format!("{label}: landed"), w.writes(&srcs), &codes);
        check(&mut bad, &format!("{label}: root"), w.root(), AT_ROOT);
        check(&mut bad, &format!("{label}: _.x"), conflict(&w.eval("_.x")), "_|_ #conflict");
    }
    let w = Ws::new("r3-nested");
    check(&mut bad, "nested table: landed", w.writes(&["t: { @{ @int }: 0, x: 3 }", "t: { x: 4 }"]), "e0 e0 c0");
    check(&mut bad, "nested table: root", w.root(), NESTED);
    check(&mut bad, "nested table: _.t 5", w.eval("_.t 5"), "0");
    check(&mut bad, "nested table: _.t.x", conflict(&w.eval("_.t.x")), "_|_ #conflict");
    verdict(bad, 4 * 3 + 4);
}

/// r4 — Members written apart (two evolves that did not see each other) are
/// judged on the whole working set, whatever their file names: with a table
/// the commit lands every time and on one root; without one it is refused
/// every time, naming `x`.
#[test]
fn r4_the_commit_does_not_depend_on_member_names() {
    let mut bad = Vec::new();
    let runs = 8;
    for _ in 0..runs {
        let a = Ws::new("r4-a");
        let b = Ws::new("r4-b");
        a.evolve(T);
        a.evolve("x: 3");
        b.evolve("x: 4");
        a.take_members_of(&b);
        let (o, rc) = a.oo(&["commit", "-m", "a"]);
        check(&mut bad, "with a table: commit", format!("rc={rc}"), "rc=0");
        if rc != 0 {
            bad.push(format!("    {}", squash(&o)));
        }
        check(&mut bad, "with a table: root", a.root(), AT_ROOT);
        let a = Ws::new("r4-c");
        let b = Ws::new("r4-d");
        a.evolve("x: 3");
        b.evolve("x: 4");
        a.take_members_of(&b);
        let (o, rc) = a.oo(&["commit", "-m", "a"]);
        check(&mut bad, "no table: commit", format!("rc={rc}"), "rc=1");
        check(&mut bad, "no table: names x", o.contains("#conflict at x").to_string(), "true");
    }
    verdict(bad, runs * 4);
}

/// r5 — Choosing among alternatives treats a conflict the meet made as
/// disjoint: a union keeps only the members that agree, dispatch does not
/// select a pattern the argument conflicts with, a complement leaves a value
/// that conflicts with what it removes. And `=` does not read a ⊥'s cause.
#[test]
fn r5_a_made_conflict_is_disjoint_when_choosing() {
    let mut bad = Vec::new();
    let cells: [(&str, &str); 6] = [
        ("({ a: 1 } | { a: 2 }) & { a: 1, b: 2 }", "{ a: 1 b: 2 }"),
        ("{ a: 1 } |> ({ a: 2 } | { c: 3 })", "{ a: 1 c: 3 }"),
        ("{ @{ { age: 18.. } }: \"adult\", _: \"minor\" } { age: 15 }", "\"minor\""),
        ("{ a: #false } & !{ a: #true }", "{ a: #false }"),
        ("({ x: 3 } & { x: 4 }) = ({ x: 4 } & { x: 3 })", "#true"),
        ("{ x: 1 & 2 } = ({ x: 3 } & { x: 4 })", "#true"),
    ];
    for (e, want) in cells {
        check(&mut bad, e, eval_here(e), want);
    }
    verdict(bad, cells.len());
}

/// r6 — Acceptance R-1. `<=` is `(A & B) = A` (SYNTAX_06 §2.1), and `=`
/// does not read a ⊥'s cause, so neither does `<=`: a record whose field is
/// ⊥ lies under any record that has a value at that field, and a union
/// absorbs it. Choosing (r5) still treats the conflict as disjoint. The other
/// half: a ⊥ field is not under a record that lacks it or adds a field.
#[test]
fn r6_the_order_agrees_with_the_meet() {
    let mut bad = Vec::new();
    let c = "({ x: 3 } & { x: 4 })";
    let cells: Vec<(String, &str)> = vec![
        (format!("{c} <= {{ x: 9 }}"), "#true"),
        (format!("{{ x: 9 }} >= {c}"), "#true"),
        ("{ a: 1 & 2 } <= { a: @int }".into(), "#true"),
        (format!("({c} <= {{ x: 9 }}) = (({c} & {{ x: 9 }}) = {c})"), "#true"),
        ("({ a: 1 & 2 } <= { a: 2 }) = (({ a: 1 & 2 } & { a: 2 }) = { a: 1 & 2 })".into(), "#true"),
        ("{ a: 2 } | ({ a: 1 } & { a: 2 })".into(), "{ a: 2 }"),
        ("({ a: 1, b: 2 } & { a: 2 }) | { a: 2, b: 2 }".into(), "{ a: 2 b: 2 }"),
        ("{ a: 1 & 2 } <= { b: 1 }".into(), "#false"),
        (format!("{c} <= ({c} & {{ y: 1 }})"), "#false"),
        ("{ a: 1 } <= { a: 2 }".into(), "#false"),
        ("{ a: 1 } | { a: 2 }".into(), "{ a: 1 } | { a: 2 }"),
    ];
    for (e, want) in &cells {
        check(&mut bad, e, eval_here(e), want);
    }
    verdict(bad, cells.len());
}

/// r7 — Acceptance R-2. What two complements leave out is a union, and a
/// member inside another is dropped by the same order as r6, so
/// `!A & !B` and `!(A | B)` are one value. Whether a value is left out
/// (membership) is still a choice (r5): a conflict the meet made is disjoint.
/// The other half: members that are not under one another both stay.
#[test]
fn r7_a_complement_drops_by_the_same_order() {
    let mut bad = Vec::new();
    let cells: [(&str, &str); 7] = [
        ("!{ a: 2 } & !({ a: 1 } & { a: 2 })", "!{ a: 2 }"),
        ("(!{ a: 2 } & !({ a: 1 } & { a: 2 })) = !({ a: 2 } | ({ a: 1 } & { a: 2 }))", "#true"),
        ("!({ a: 1, b: 2 } & { a: 2 }) & !{ a: 2, b: 2 }", "!{ a: 2 b: 2 }"),
        ("!{ a: 2 } & !{ a: 3 }", "!({ a: 2 } | { a: 3 })"),
        ("!({ a: 1 } & { a: 2 }) & !{ b: 1 }", "!({ a: _|_ ;; %cause: #conflict } | { b: 1 })"),
        ("({ a: 1 } & { a: 2 }) & !{ a: 2 }", "{ a: _|_ ;; %cause: #conflict }"),
        ("{ a: 3 } & (!{ a: 2 } & !({ a: 1 } & { a: 2 }))", "{ a: 3 }"),
    ];
    for (e, want) in cells {
        check(&mut bad, e, eval_here(e), want);
    }
    verdict(bad, cells.len());
}

/// g1 — The evolve boundary is as before (green today): with no table the
/// second of two conflicting writes is refused by key, nested too; a write
/// over a ⊥ already there is refused; a write elsewhere is not.
#[test]
fn g1_the_evolve_boundary_is_as_before() {
    let mut bad = Vec::new();
    for (label, srcs, want, names) in [
        ("no table", vec!["x: 3", "x: 4"], "e0 e1 c0", "#conflict at x"),
        ("nested, no table", vec!["t: { x: 3 }", "t: { x: 4 }"], "e0 e1 c0", "#conflict at t.x"),
        ("over a ⊥", vec!["b: 1 & 2", "b: 5"], "e0 e1 c0", "#conflict at b"),
        ("3, 4, then a table", vec!["x: 3", "x: 4", T], "e0 e1 e0 c0", "#conflict at x"),
    ] {
        let w = Ws::new("g1");
        let mut codes = Vec::new();
        let mut said = String::new();
        for s in &srcs {
            let (rc, o) = w.evolve(s);
            if rc != 0 {
                said = o;
            }
            codes.push(format!("e{rc}"));
        }
        let (_, rc) = w.oo(&["commit", "-m", "a"]);
        codes.push(format!("c{rc}"));
        check(&mut bad, &format!("{label}: codes"), codes.join(" "), want);
        check(&mut bad, &format!("{label}: names"), said.contains(names).to_string(), "true");
    }
    for (label, first, second, names) in [
        ("committed, then", "x: 3", "x: 4", "#conflict at x"),
        ("committed nested, then", "t: { x: 3 }", "t: { x: 4 }", "#conflict at t.x"),
        ("committed recipe, then", "a: { x: 3 }\nb: a", "b: { x: 4 }", "#conflict at b.x"),
    ] {
        let w = Ws::new("g1-root");
        check(&mut bad, &format!("{label}: first"), w.writes(&[first]), "e0 c0");
        let (rc, o) = w.evolve(second);
        check(&mut bad, &format!("{label}: refused"), rc.to_string(), "1");
        check(&mut bad, &format!("{label}: names"), o.contains(names).to_string(), "true");
    }
    let w = Ws::new("g1-else");
    check(&mut bad, "⊥ elsewhere", w.writes(&["b: 1 & 2", "c: 5"]), "e0 e0 c0");
    let w = Ws::new("g1-onefile");
    let (rc, o) = w.evolve("x: 3\nx: 4\n@{ @int }: 0");
    check(&mut bad, "one file, table last: refused", rc.to_string(), "1");
    check(&mut bad, "one file, table last: names x", o.contains("#conflict at x").to_string(), "true");
    let w = Ws::new("g1-recipe");
    w.evolve("a: { x: 3 }\nb: a");
    let (rc, o) = w.evolve("b: { x: 4 }");
    check(&mut bad, "staged recipe: refused", rc.to_string(), "1");
    check(&mut bad, "staged recipe: names", o.contains("#conflict at b.x").to_string(), "true");
    verdict(bad, 4 * 2 + 3 * 3 + 2 + 3);
}

/// g2 — Roots that hold no conflicting writes do not move (v0.101.0); a
/// Cocoon still collapses whole; two arrows meet as one table and conflict
/// only when applied (D105).
#[test]
fn g2_what_holds_no_conflict_does_not_move() {
    let mut bad = Vec::new();
    for (src, root) in [
        ("x: 0", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        ("t: { @{ @int }: 0, x: 3 }", "5d0e29990a9573057a9f821a2b2c2af218883912ffb85cf29d54983802ac3a7e"),
        ("t: { a: 1 } & { b: 2 }", "ee2c382ff3a0337e3b515e41d5c0891010ef8cf62c3323486fcf933bb2b02fea"),
        ("f: (x -> 1) & (x -> 2)", "70fe0048828891621ab54374ab718211e91bd9ff5f0326a395bb6e4e0c17b501"),
        ("b: 1 & 2", "fa347014373a8e7ce5d3e62202f27bf805b61979bed95baf2da3e8b5108541ae"),
    ] {
        check(&mut bad, src, root_of(src), root);
    }
    for (e, want) in [
        ("{{ a: 1 }} & {{ a: 2 }}", "_|_ #conflict"),
        ("((x -> 1) & (x -> 2)) 5", "_|_ #conflict"),
        ("{ f: (x -> 1) & (x -> 2), k: 1 }.k", "1"),
        ("({ a: 1, b: 2, a: 2 }).b", "2"),
    ] {
        check(&mut bad, e, eval_here(e), want);
    }
    verdict(bad, 5 + 4);
}
