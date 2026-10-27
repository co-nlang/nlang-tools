// A number that carries fields.
// Order: nlang-tools/docs/a_number_that_carries_fields_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-109.  SYNTAX_06 §4 #6: an atom met
// with a record is a hybrid node, `1 & { name: "Alice" }` being
// `{ %val: 1, name: "Alice" }`, and every value context (arithmetic,
// `==`/`!=`, a morphism's argument, a builtin) reads its `%val` (G6). D119:
// an operand not yet a value answers `_`. D121: so does a record without
// `%val` — met with a value it is a hybrid. `&` is a meet: associative and
// commutative (SPEC_12).
//
// ── What is wrong (v0.109.0) ────────────────────────────────────────────
//
//   `(@int & 5) & { a: 1 }` is the hybrid 5, `@int & ({ a: 1 } & 5)` is ⊥;
//   so `x: @int` then `x: 5 & { note: "n" }` is refused. `{ %val: 5, a: 1 }
//   + 1` is ⊥ (its `%val` is still lazy); `~%Math./add (5 & { a: 1 }, 1)`
//   is ⊥; `(1..) & { a: 1 }` is ⊥; `{ a: 1 } + 1` is ⊥ and frozen into
//   history once the record is met with 5 later.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("hybrid-{tag}"));
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

    /// `oo fmt` of `src` written to a file.
    fn fmt(&self, src: &str) -> String {
        fs::write(self.ws.join("fmt.n"), src).unwrap();
        self.oo(&["fmt", "fmt.n"]).0
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

fn bottom(got: &str) -> String {
    for cause in ["#conflict", "#missing_key", "#arithmetic_on_anchor", "#no_matching_branch"] {
        if got.starts_with("_|_") && got.contains(cause) {
            return format!("_|_ {cause}");
        }
    }
    got.into()
}

/// `oo eval` from an empty directory (no universe), a ⊥ reduced to its cause.
fn eval_here(e: &str) -> String {
    let w = Ws::new("eval");
    bottom(&w.eval(e))
}

fn table(bad: &mut Vec<String>, cells: &[(&str, &str)]) {
    for (e, want) in cells {
        check(bad, e, eval_here(e), want);
    }
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

/// The readings of `q` after `w1` then `w2` written as one file, as two
/// evolves and one commit, and as two commits; and the three roots.
fn three_ways(w1: &str, w2: &str, q: &str) -> (Vec<String>, Vec<String>) {
    let mut got = Vec::new();
    let mut roots = Vec::new();
    for ways in [vec![vec![format!("{w1}\n{w2}")]], vec![vec![w1.to_string(), w2.to_string()]], vec![vec![w1.to_string()], vec![w2.to_string()]]] {
        let w = Ws::new("three");
        let mut rc = Vec::new();
        for commit in &ways {
            let srcs: Vec<&str> = commit.iter().map(|s| s.as_str()).collect();
            rc.push(w.writes(&srcs));
        }
        got.push(format!("{} | {}", rc.join(" "), bottom(&w.eval(q))));
        roots.push(w.root());
    }
    (got, roots)
}

const INT: &str = "{{ %kind: #type %name: \"int\" }}";

/// r1 — A hybrid in a value context reads its `%val`, however it was
/// written: met (`5 & { a: 1 }`), written out (`{ %val: 5, a: 1 }`), in an
/// arithmetic operand, `==`, a pipe, a builtin's argument.
#[test]
fn r1_a_hybrid_reads_its_val_however_written() {
    let mut bad = Vec::new();
    let cells = [
        ("{ %val: 5, a: 1 } + 1", "6"),
        ("1 + { %val: 5, a: 1 }", "6"),
        ("{ %val: 5, a: 1 } * 2", "10"),
        ("{ %val: 5, a: 1 } - { %val: 5, a: 1 }", "0"),
        ("{ %val: 5 } + 1", "6"),
        ("{ %val: 5, a: 1 } |> (x -> x + 1)", "6"),
        ("{ %val: 5, a: 1 } == 5", "#true"),
        ("~%Math./add ((5 & { a: 1 }), 1)", "6"),
        ("~%Math./add (({ a: 1 } & 5), 1)", "6"),
        ("~%Math./add ({ %val: 5, a: 1 }, 1)", "6"),
        ("~%Math./add (1, { %val: 2 })", "3"),
        ("~%Math./sqrt (4 & { a: 1 })", "2"),
        ("~%Math./abs (5 & { a: 1 })", "5"),
        ("({ a: 1 } & { %val: 5 }) + 1", "6"),
        ("(\"s\" & { a: 1 }) + \"t\"", "\"st\""),
        ("(@float & (5 & { a: 1 })) / 2", "2.5"),
        ("{ %val: 5, a: 1 }.a", "1"),
    ];
    table(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// r2 — What is not yet a value answers `_` (D119, D121): a hybrid whose
/// `%val` is a type or a range, and a record without `%val`.
#[test]
fn r2_what_is_not_yet_a_value_answers_top() {
    let mut bad = Vec::new();
    let cells = [
        ("(@int & { a: 1 }) + 1", "_"),
        ("1 + (@int & { a: 1 })", "_"),
        ("(@int & { a: 1 }) == 5", "_"),
        ("(@int & { a: 1 }) |> (x -> x + 1)", "_"),
        ("~%Math./add ((@int & { a: 1 }), 1)", "_"),
        ("{ %val: @int, a: 1 } + 1", "_"),
        ("((1..) & { a: 1 }) * 2", "_"),
        ("{ a: 1 } + 1", "_"),
        ("1 + { a: 1 }", "_"),
        ("{{ a: 1 }} + 1", "_"),
        ("{ a: 1 } == 5", "_"),
        ("{ a: 1 } != 5", "_"),
        ("{ a: 1 } == { a: 1 }", "_"),
        ("~%Math./add ({ a: 1 }, 1)", "_"),
        ("(_ & { a: 1 }) + 1", "_"),
        ("~%Math./sqrt (@num)", "_"),
    ];
    table(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// r3 — `&` is a meet: every order and grouping of the same three operands
/// gives one node — its value and its field.
#[test]
fn r3_a_meet_is_associative_and_commutative() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (ops, value_q, value, field_q, field) in [
        (["@int", "5", "{ a: 1 }"], "+ 1", "6", ".a", "1"),
        (["1..9", "5", "{ a: 1 }"], "+ 1", "6", ".a", "1"),
        (["@float", "5", "{ a: 1 }"], "/ 2", "2.5", ".a", "1"),
        (["@str", "\"s\"", "{ l: \"en\" }"], "+ \"t\"", "\"st\"", ".l", "\"en\""),
        (["@int", "{ %val: 5 }", "{ a: 1 }"], "+ 1", "6", ".a", "1"),
        (["@int", "{ a: 1 }", "{ b: 2 }"], "+ 1", "_", ".b", "2"),
    ] {
        let perms = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
        for p in perms {
            let (x, y, z) = (ops[p[0]], ops[p[1]], ops[p[2]]);
            for e in [format!("(({x}) & ({y})) & ({z})"), format!("({x}) & (({y}) & ({z}))")] {
                check(&mut bad, &format!("({e}) {value_q}"), eval_here(&format!("({e}) {value_q}")), value);
                check(&mut bad, &format!("({e}){field_q}"), eval_here(&format!("({e}){field_q}")), field);
                n += 2;
            }
        }
    }
    let cells = [
        ("<<@int & { a: 1 }>>", "{ %val: {{ %kind: #type %name: \"int\" }} a: 1 }"),
        ("(@int & { a: 1 })", INT),
        ("((1..) & { a: 1 })", "1..#_"),
        ("@int & (5 & { a: 1 })", "5"),
        ("(1..) & (5 & { a: 1 })", "5"),
        ("@int & { %val: 5, a: 1 }", "5"),
    ];
    for (e, want) in cells {
        check(&mut bad, e, eval_here(e), want);
        n += 1;
    }
    verdict(bad, n);
}

/// r4 — A typed coordinate and a hybrid, a record and a later value: one
/// file, two evolves, two commits — one reading, one root.
#[test]
fn r4_three_ways_one_reading_one_root() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (w1, w2, q, want) in [
        ("x: @int", "x: 5 & { note: \"n\" }", "_.x + 1", "6"),
        ("x: 5 & { note: \"n\" }", "x: @int\ny: 1", "_.x + 1", "6"),
        ("x: @int", "x: 5 & { note: \"n\" }", "_.x.note", "\"n\""),
        ("x: @int", "x: { %val: 5, note: \"n\" }", "_.x + 1", "6"),
        ("h: { a: 1 }\nr: h + 1", "h: 5", "_.r", "6"),
        ("h: 5", "h: { a: 1 }\nr: h + 1", "_.r", "6"),
        ("n: @int & { unit: \"m\" }\nr: n * 2", "n: 5 & { unit: \"m\" }", "_.r", "10"),
        ("n: { %val: @int, unit: \"m\" }\nr: n + 1", "n: { %val: 5 }", "_.r", "6"),
        ("h: { k: @int & { u: 1 }, r: k + 1 }", "h: { k: 5 }", "_.h.r", "6"),
        ("x: 1..10", "x: 5 & { note: \"n\" }", "_.x * 2", "10"),
        ("x: @str", "x: \"a\" & { lang: \"en\" }", "_.x + \"b\"", "\"ab\""),
    ] {
        let (got, roots) = three_ways(w1, w2, q);
        for (way, g) in ["one file", "two evolves", "two commits"].iter().zip(got) {
            let rc = if *way == "two evolves" { "e0 e0 c0" } else if *way == "one file" { "e0 c0" } else { "e0 c0 e0 c0" };
            check(&mut bad, &format!("{w1} / {w2}, {way}: {q}"), g, &format!("{rc} | {want}"));
            n += 1;
        }
        let mut r = roots.clone();
        r.sort();
        r.dedup();
        check(&mut bad, &format!("{w1} / {w2}: one root"), r.len().to_string(), "1");
        n += 1;
    }
    verdict(bad, n);
}

/// r5 — Three writes grouped five ways: one reading, one root.
#[test]
fn r5_three_writes_every_grouping_one_root() {
    let mut bad = Vec::new();
    let mut n = 0;
    for (a, b, c, q, want) in [
        ("n: @int", "n: { unit: \"m\" }", "n: 5", "_.n + 1", "6"),
        ("h: { a: 1 }", "r: h + 1", "h: 5", "_.r", "6"),
    ] {
        let groupings: Vec<(&str, Vec<Vec<String>>)> = vec![
            ("one file", vec![vec![format!("{a}\n{b}\n{c}")]]),
            ("three evolves", vec![vec![a.into(), b.into(), c.into()]]),
            ("three commits", vec![vec![a.into()], vec![b.into()], vec![c.into()]]),
            ("one, then two", vec![vec![a.into()], vec![format!("{b}\n{c}")]]),
            ("two, then one", vec![vec![format!("{a}\n{b}")], vec![c.into()]]),
        ];
        let mut roots = Vec::new();
        for (label, commits) in &groupings {
            let w = Ws::new("r5");
            for commit in commits {
                let srcs: Vec<&str> = commit.iter().map(|s| s.as_str()).collect();
                w.writes(&srcs);
            }
            check(&mut bad, &format!("{a} / {b} / {c}, {label}: {q}"), bottom(&w.eval(q)), want);
            n += 1;
            roots.push(w.root());
        }
        roots.sort();
        roots.dedup();
        check(&mut bad, &format!("{a} / {b} / {c}: one root over five groupings"), roots.len().to_string(), "1");
        n += 1;
    }
    verdict(bad, n);
}

/// g1 — The other half: true contradictions stay ⊥, the set family does not
/// peel, navigation enters the record, a record given to a builtin as its
/// argument is named parameters (a partial), lists and arrows are not
/// records, anchors are sealed first.
#[test]
fn g1_what_is_not_a_hybrid_answers_as_before() {
    let mut bad = Vec::new();
    let cells = [
        ("@str & (5 & { a: 1 })", "_|_ #conflict"),
        ("(6..) & (5 & { a: 1 })", "_|_ #conflict"),
        ("{ %val: 5, a: 1 } & 6", "_|_ #conflict"),
        ("(\"a\" & { a: 1 }) + 1", "_|_ #conflict"),
        ("[1] + 1", "_|_ #conflict"),
        ("(x -> x) + 1", "_|_ #conflict"),
        ("@int & [1]", "_|_ #conflict"),
        ("#_ + { a: 1 }", "_|_ #arithmetic_on_anchor"),
        ("(3 & { note: \"n\" }) = 3", "#false"),
        ("{ a: 1 } = { a: 1 }", "#true"),
        ("(5 & { a: 1 }) + 1", "6"),
        ("(5 & { a: 1 }) == 5", "#true"),
        ("((1 | 2) & { a: 1 }) + 1", "2 | 3"),
        ("(!0 & { a: 1 }) + 1", "_"),
        ("(5 & { a: 1 }).a", "1"),
        ("{ a: 1 }", "{ a: 1 }"),
        ("5 & {{ a: 1 }}", "5"),
        ("<<@combo & { a: 1 }>>", "{ a: 1 }"),
        ("@list & { a: 1 }", "_|_ #conflict"),
    ];
    table(&mut bad, &cells);
    for e in ["~%Math./add { x: 7, y: 17 }", "~%Math./abs ({ a: 1 })"] {
        let got = eval_here(e);
        check(&mut bad, &format!("{e}: a partial"), (got.contains("%builtin") && !got.contains("_|_")).to_string(), "true");
    }
    verdict(bad, cells.len() + 2);
}

/// g2 — Roots of what already read right do not move (v0.109.0).
#[test]
fn g2_roots_that_read_right_do_not_move() {
    let mut bad = Vec::new();
    let cases = [
        ("x: 0", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        ("x: 5 & { a: 1 }", "a8ee5b1237e4743c03e49c4668327832abc92421c2d3ff9f5969a4b85095bd8d"),
        ("x: 5 & { a: 1 }\nr: x + 1", "4105d5f1f2db920bf504ed4a2b3c60a3f725d0ae852c7ab27f06903e20e3fc23"),
        ("x: (1 | 2) & { a: 1 }", "a5df9de542dd2fda9b09f7258f6d748f81c52288675dc22cd45844063b01ff26"),
        ("x: \"s\" & { lang: \"en\" }", "0841c18ca04f00f8c3a3ad97110c69b1a5670f77b99082b844facea06be465ee"),
        ("r: (5 & { a: 1 }) == 5", "c3a30d61731b73081d005194f9d5e9e73aca91403867869d5f3260f4738af2b6"),
        ("x: { a: 1 }", "8f5d218e1b5eb2282ba9c0431b57e6c3cf347ab6a4280e48a5b6a47b1cc9356b"),
        ("x: @int", "53922aa4a79553dc425d46ca84de6e368b53ab9992195ca389b9fbf4d54a565c"),
        ("x: { a: 1 } & !0", "e84bbb9b4a1094d3d12078c4ec6bfc650592345788cdb7da2faadfef665963cd"),
        ("r: (3 & { note: \"n\" }) = 3", "f45f31f58083b415002cc32fdc2074799f7f0db60b396379534249fa49736f42"),
        ("f: (x -> x + 1)", "258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f"),
    ];
    for (src, root) in cases {
        check(&mut bad, src, root_of(src), root);
    }
    verdict(bad, cases.len());
}
