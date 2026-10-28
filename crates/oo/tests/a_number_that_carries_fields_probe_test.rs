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
// R-1 (acceptance round 1): r6–r8 and g3 at the end of this file.
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

// ── R-1 (acceptance round 1, 2026-10-28) ───────────────────────────────
//
// Measured outside the probe: G6 holds at the arithmetic builtins only.
// Every other builtin given a hybrid answers a partial or a wrong value
// (`~%Str./len ("ab" & { n: 1 })` is a partial, `~%List./sum ([1 & { n: 1
// }, 2])` is 2, `~%Str./from_int (5 & { a: 1 })` is "{...}"); key lookup,
// the ternary condition, a range bound, interpolation and a predicate's
// answer do not read `%val` either; and a builtin given an operand not yet
// a value answers a partial or a string (`~%Str./from_int (_)` is "_",
// `~%List./sum ([_, 2])` is 2) — refined later, it answers something else.

/// `oo eval` from an empty directory, the effect note dropped, whitespace
/// collapsed, not cut short; a panic is "PANIC".
fn eval_full(e: &str) -> String {
    let w = Ws::new("r1");
    let s = w.oo(&["eval", e]).0;
    if s.contains("panicked") {
        return "PANIC".into();
    }
    let s = match s.find(";; %effect") {
        Some(i) => s[..i].to_string(),
        None => s,
    };
    squash(&s)
}

fn eval_all(srcs: &[String]) -> Vec<String> {
    let mut out = vec![String::new(); srcs.len()];
    let per = srcs.len().div_ceil(8).max(1);
    std::thread::scope(|s| {
        for (o, i) in out.chunks_mut(per).zip(srcs.chunks(per)) {
            s.spawn(move || {
                for (o, e) in o.iter_mut().zip(i) {
                    *o = eval_full(e);
                }
            });
        }
    });
    out
}

/// Split a call's operands at top-level commas.
fn operands(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth, mut q) = (Vec::new(), String::new(), 0i32, false);
    for ch in s.chars() {
        if q {
            cur.push(ch);
            q = ch != '"';
            continue;
        }
        match ch {
            '"' => q = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
                continue;
            }
            _ => {}
        }
        cur.push(ch);
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// The type of an atom operand, and a second value of that type.
fn atom_type(a: &str) -> Option<(&'static str, Option<&'static str>)> {
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let a2 = a.strip_prefix('-').unwrap_or(a);
    if digits(a2) {
        return Some(("@int", Some("9")));
    }
    if let Some((i, f)) = a2.split_once('.') {
        if digits(i) && digits(f) {
            return Some(("@float", Some("9.5")));
        }
    }
    if a.len() >= 2 && a.starts_with('"') && a.ends_with('"') && !a[1..a.len() - 1].contains('"') {
        return Some(("@str", Some("\"zz\"")));
    }
    if a == "#true" || a == "#false" {
        return Some(("@bool", None));
    }
    if a.ends_with('i') && a.contains(['+', '-']) {
        return Some(("@complex", Some("9+9i")));
    }
    None
}

struct Variant {
    id: &'static str,
    kind: String,
    plain: &'static str,
    src: String,
    /// What it must refine (r7): a call it is less refined than.
    above: Vec<String>,
}

/// From each call: every atom operand, and the first element of every list
/// operand, written as a hybrid two ways (r6) and as `_`, its type, or (in a
/// pack or a list) a record without `%val` (r7).
fn variants() -> Vec<Variant> {
    let mut out = Vec::new();
    for &(id, call, plain) in CALLS {
        let open = call.find(" (").unwrap();
        let head = &call[..open];
        let args = operands(&call[open + 2..call.len() - 1]);
        for i in 0..args.len() {
            let a = &args[i];
            let (x, ty, alt, el): (String, &str, Option<&str>, Option<Vec<String>>) = if let Some((ty, alt)) = atom_type(a) {
                (a.clone(), ty, alt, None)
            } else if a.starts_with('[') && a.ends_with(']') {
                let el = operands(&a[1..a.len() - 1]);
                match el.first().and_then(|e| atom_type(e)) {
                    Some((ty, alt)) => (el[0].clone(), ty, alt, Some(el)),
                    None => continue,
                }
            } else {
                continue;
            };
            let put = |y: &str| {
                let mut b = args.clone();
                b[i] = match &el {
                    Some(el) => {
                        let mut e = el.clone();
                        e[0] = y.to_string();
                        format!("[{}]", e.join(", "))
                    }
                    None => y.to_string(),
                };
                format!("{head} ({})", b.join(", "))
            };
            let w = if el.is_some() { "element " } else { "" };
            let hy = put(&format!("({x} & {{ n: 1 }})"));
            out.push(Variant { id, kind: format!("{w}met"), plain, src: hy.clone(), above: vec![] });
            out.push(Variant { id, kind: format!("{w}written"), plain, src: put(&format!("{{ %val: {x}, n: 1 }}")), above: vec![] });
            // A list operand used as the morphism is a key lookup of `_`: D97, not here.
            if args[0].starts_with('[') && FN_FIRST.contains(&id) {
                continue;
            }
            let (c2, hy2) = match alt {
                Some(alt) => (put(alt), put(&format!("({alt} & {{ n: 1 }})"))),
                None => (call.to_string(), hy.clone()),
            };
            out.push(Variant { id, kind: format!("{w}_"), plain, src: put("_"), above: vec![call.to_string(), c2.clone()] });
            out.push(Variant { id, kind: format!("{w}type"), plain, src: put(&format!("({ty})")), above: vec![call.to_string(), c2] });
            if el.is_some() || args.len() > 1 {
                out.push(Variant { id, kind: format!("{w}record"), plain, src: put("{ n: 1 }"), above: vec![hy, hy2] });
            }
        }
    }
    out
}

/// Builtins whose operand 0 is a morphism.
const FN_FIRST: &[&str] = &[
    "list.all", "list.any", "list.count", "list.drop_while", "list.filter", "list.find", "list.flat_map",
    "list.group_by", "list.map", "list.max_by", "list.min_by", "list.partition", "list.reduce", "list.scan",
    "list.sort_by", "list.take_while",
];

/// r6 — Every builtin reads a hybrid operand, or a hybrid element of a list
/// operand, as its `%val`: written met or written out, the answer is the
/// bare operand's. The bare calls answer what they answered at v0.109.0.
#[test]
fn r6_every_builtin_reads_a_hybrid_as_its_value() {
    let mut bad = Vec::new();
    let bare: Vec<String> = CALLS.iter().map(|c| c.1.to_string()).collect();
    for (c, got) in CALLS.iter().zip(eval_all(&bare)) {
        check(&mut bad, &format!("{} (bare)", c.1), got, c.2);
    }
    let vs: Vec<Variant> = variants().into_iter().filter(|v| v.above.is_empty()).collect();
    let srcs: Vec<String> = vs.iter().map(|v| v.src.clone()).collect();
    for (v, got) in vs.iter().zip(eval_all(&srcs)) {
        check(&mut bad, &format!("{} [{}] {}", v.id, v.kind, v.src), got, v.plain);
    }
    verdict(bad, CALLS.len() + vs.len());
}

/// Does reading `v` stand above reading `p` — is `p` `v` with each `_`, each
/// atom type and each `{ n: 1 }` in `v` refined to something? Read off the
/// printed forms: meeting two builtin partials does not terminate
/// (pre-existing, v0.109.0), so the probe does not ask the engine.
fn above(v: &str, p: &str) -> bool {
    if v == "_" || v == p {
        return true;
    }
    let holes: Vec<String> = ["int", "float", "str", "bool", "complex"]
        .iter()
        .map(|t| format!("{{{{ %kind: #type %name: \"{t}\" }}}}"))
        .chain(["{ n: 1 }".to_string()])
        .collect();
    // Split `v` into literal pieces around its holes.
    let (mut pieces, mut cur, mut i) = (Vec::new(), String::new(), 0);
    let b = v.as_bytes();
    while i < v.len() {
        if let Some(h) = holes.iter().find(|h| v[i..].starts_with(h.as_str())) {
            pieces.push(std::mem::take(&mut cur));
            i += h.len();
            continue;
        }
        let lone = b[i] == b'_'
            && (i == 0 || b" [(,{:".contains(&b[i - 1]))
            && (i + 1 == v.len() || b" ,])}".contains(&b[i + 1]));
        if lone {
            pieces.push(std::mem::take(&mut cur));
            i += 1;
            continue;
        }
        cur.push(b[i] as char);
        i += 1;
    }
    pieces.push(cur);
    if pieces.len() == 1 {
        return false;
    }
    fn fits(p: &str, pieces: &[String]) -> bool {
        match pieces {
            [] => p.is_empty(),
            [last] => p.ends_with(last.as_str()) && p.len() > last.len(),
            [first, rest @ ..] => (1..=p.len()).any(|k| p.is_char_boundary(k) && p[k..].starts_with(first.as_str()) && fits(&p[k + first.len()..], rest)),
        }
    }
    p.starts_with(pieces[0].as_str()) && fits(&p[pieces[0].len()..], &pieces[1..])
}

/// r7 — An operand or element not yet a value (`_`, its type, a record
/// without `%val` in a pack or a list) answers something its refinements
/// refine: `_`, or the bare call's reading and a second refinement's with
/// some holes still open (SPEC_12 §2.1; D119, D121). Not a partial, not a
/// string, not a dropped element.
#[test]
fn r7_a_builtin_given_what_is_not_yet_a_value_answers_monotonically() {
    let mut bad = Vec::new();
    let vs: Vec<Variant> = variants().into_iter().filter(|v| !v.above.is_empty()).collect();
    let srcs: Vec<String> = vs.iter().map(|v| v.src.clone()).collect();
    let got = eval_all(&srcs);
    let mut tops: Vec<String> = vs.iter().flat_map(|v| v.above.clone()).collect();
    tops.sort();
    tops.dedup();
    let seen: std::collections::HashMap<String, String> = tops.iter().cloned().zip(eval_all(&tops)).collect();
    for (v, g) in vs.iter().zip(&got) {
        for p in &v.above {
            if !above(g, &seen[p]) {
                bad.push(format!("  {} [{}] {}\n    answers {g}\n    not above {p} = {}", v.id, v.kind, v.src, seen[p]));
            }
        }
    }
    verdict(bad, vs.len());
}

/// r8 — The other value contexts the engine reads: key lookup (D97 — a
/// hybrid's key is its `%val`, as in a table with a pattern key, D107), the
/// ternary condition, a range bound, interpolation, a pipe into a builtin,
/// a predicate's answer, a key function's answer.
#[test]
fn r8_lookup_condition_bound_interpolation_predicate() {
    let mut bad = Vec::new();
    let cells = [
        ("{ 5: \"hit\", _: \"miss\" } (5 & { a: 1 })", "\"hit\""),
        ("{ 5: \"hit\", _: \"miss\" } ({ %val: 5, a: 1 })", "\"hit\""),
        ("{ \"a\": \"hit\", _: \"miss\" } (\"a\" & { n: 1 })", "\"hit\""),
        ("{ #t: \"hit\", _: \"miss\" } (#t & { a: 1 })", "\"hit\""),
        ("{ @{ @str }: \"s\", 5: \"hit\", _: \"miss\" } (5 & { a: 1 })", "\"hit\""),
        ("[10, 20] (1 & { n: 1 })", "20"),
        ("[10, 20] ({ %val: 1, n: 1 })", "20"),
        ("(#true & { n: 1 }) ? 1 : 2", "1"),
        ("{ %val: #false, n: 1 } ? 1 : 2", "2"),
        ("{ x: 5 & { a: 1 }, r: x .. 7 }.r", "5..7"),
        ("{ x: { %val: 5, a: 1 }, r: 6 & (x .. 7) }.r", "6"),
        ("`v=${5 & { a: 1 }}`", "\"v=5\""),
        ("`v=${{ %val: 5, a: 1 }}`", "\"v=5\""),
        ("~%Str./format (\"{}\", [1 & { n: 1 }])", "\"1\""),
        ("(5 & { a: 1 }) |> ~%Str./from_int", "\"5\""),
        ("\"ab\" & { n: 1 } |> ~%Str./len", "2"),
        ("~%List./filter ((x -> #true & { n: 1 }), [1, 2])", "[1, 2]"),
        ("~%List./count ((x -> { %val: #true, n: 1 }), [1, 2])", "2"),
        ("~%List./find ((x -> #true & { n: 1 }), [1, 2])", "1"),
        ("~%List./any ((x -> #true & { n: 1 }), [1, 2])", "#true"),
        ("~%List./filter ((x -> _), [1, 2])", "_"),
        ("~%List./max_by ((x -> x & { k: 1 }), [5, 1])", "5"),
        ("(~%List./max_by ((x -> x), [5 & { a: 1 }, 1])).a", "1"),
        ("~%List./max_by ((x -> x + 1), [_, 2, 3])", "_"),
        ("~%List./dedup (_)", "_"),
        ("~%List./sum ([_, 2])", "_"),
        ("~%List./sum ([{ a: 1 }, 2])", "_"),
        ("~%Str./repeat (2, { a: 1 })", "_"),
        ("~%Str./from_int (_)", "_"),
        ("~%Str./len (_)", "_"),
        ("~%List./len (_)", "_"),
        ("~%Json./stringify (5 & { a: 1 })", "\"5\""),
    ];
    table(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// Builtins r6/r7 do not vary, and why.
const NOT_VARIED: &[(&str, &str)] = &[
    ("cond.", "carries its operands (and answers ⊥ or a partial with bare ones at v0.109.0)"),
    ("option.", "carries its operands"),
    ("result.", "carries its operands"),
    ("refl.", "reads structure"),
    ("diff.", "compares as `=` does: no %val (SYNTAX_06 §4 #6)"),
    ("set.", "compares as `=` does: no %val (§4 #11, one equality)"),
    ("list.dedup", "compares as `=` does"),
    ("list.unique", "compares as `=` does"),
    ("engine.", "engine reflection"),
    ("disc.", "network"),
    ("effect.", "effects"),
    ("env.", "host"),
    ("io.", "host"),
    ("process.", "host"),
    ("query.", "reads structure"),
    ("math.random", "draws"),
    ("time.now", "reads the clock"),
    ("csv.read_csv", "host"),
    ("csv.stringify", "no bare call answers a value at v0.109.0"),
    ("toml.stringify", "reads structure"),
    ("stat.mean", "no bare call answers a value at v0.109.0"),
    ("stat.median", "no bare call answers a value at v0.109.0"),
    ("stat.std_dev", "no bare call answers a value at v0.109.0"),
    ("stat.variance", "no bare call answers a value at v0.109.0"),
    ("time.parse", "no bare call answers a value at v0.109.0"),
    ("list.fold", "no bare call answers a value at v0.109.0"),
    ("list.zip_with", "no bare call answers a value at v0.109.0"),
];

/// g3 — The other half. Every builtin is in r6/r7's table or named in
/// NOT_VARIED. What a builtin hands back stays whole (fields kept); the `=`
/// family does not peel; a record or `_` given to key lookup goes to `_:`
/// (D97); interpolation shows `_` as `_`; a record that is the argument
/// itself is named parameters; roots of what read right do not move.
#[test]
fn g3_what_a_builtin_carries_and_the_equal_family() {
    let mut bad = Vec::new();
    for id in nlang_interpreter::builtins::contract_ids() {
        let varied = CALLS.iter().any(|c| c.0 == id);
        let named = NOT_VARIED.iter().any(|(p, _)| id == *p || (p.ends_with('.') && id.starts_with(p)));
        if varied == named {
            bad.push(format!("  {id}: varied {varied}, named in NOT_VARIED {named}"));
        }
    }
    let cells = [
        ("(~%List./at (0, [5 & { a: 1 }])).a", "1"),
        ("(~%List./at (1, ~%List./reverse ([5 & { a: 1 }, 1]))).a", "1"),
        ("(~%List./at (1, ~%List./sort ([5 & { a: 1 }, 1]))).a", "1"),
        ("~%List./sort ([5 & { a: 1 }, 1])", "[1, 5]"),
        ("(~%List./at (0, ~%List./map ((x -> x), [5 & { a: 1 }]))).a", "1"),
        ("(~%List./head ([5 & { a: 1 }])).a", "1"),
        ("(~%List./at (1, ~%List./intersperse (5 & { a: 1 }, [1, 2]))).a", "1"),
        ("~%List./len ([5 & { a: 1 }, _])", "2"),
        ("~%Json./keys (5 & { a: 1 })", "[\"a\"]"),
        ("~%Json./get (\"a\", { a: 1 })", "1"),
        ("~%Set./contains ([1, 2], 1 & { a: 1 })", "#false"),
        ("~%Set./contains ([1 & { a: 1 }, 2], 1)", "#false"),
        ("~%List./dedup ([1 & { a: 1 }, 1])", "[1, 1]"),
        ("~%Diff./is_compatible ((3 & { n: 1 }), 3)", "#true"),
        ("{ 5: \"hit\", _: \"miss\" } ({ a: 1 })", "\"miss\""),
        ("{ 5: \"hit\", _: \"miss\" } (_)", "\"miss\""),
        ("{ 5: \"hit\", _: \"miss\" } (6 & { a: 1 })", "\"miss\""),
        ("`v=${_}`", "\"v=_\""),
    ];
    table(&mut bad, &cells);
    for e in ["~%Str./len { a: 1 }", "~%Math./add { x: 7, y: 17 }"] {
        let got = eval_here(e);
        check(&mut bad, &format!("{e}: a partial"), (got.contains("%builtin") && !got.contains("_|_")).to_string(), "true");
    }
    let roots = [
        ("r: ~%Str./len (\"ab\")", "b9d2a36282ad0226c11f8dadb920193b012a7c42411ed0e0344ad8a3d57f7d64"),
        ("r: [10, 20] (1)", "5456b1535b980c55bdb0490c1b9e459bc43450090714afaffb4bbb9a169ff81a"),
        ("r: #true ? 1 : 2", "04f389b7b5b4af1bca66bdf05e0ac406f47d3aff4a54b2b0a8c7884a0fc09c2c"),
        ("r: `v=${5}`", "cd63b4211b2ad56c587b162e2cbfd642e93b1ae73dd2f22ea6f4695bf9e361b5"),
    ];
    for (src, root) in roots {
        check(&mut bad, src, root_of(src), root);
    }
    verdict(bad, cells.len() + roots.len() + 2);
}
/// Each builtin's calls that answer a value with bare operands at v0.109.0,
/// and that answer (`acc/q109/pick.json`, `acc/q109/sweep.py`).
const CALLS: &[(&str, &str, &str)] = &[
    ("bytes.at", "~%Bytes./at (1, (~%Bytes./from_str (\"ab\")))", "98"),
    ("bytes.base64_decode", "~%Bytes./base64_decode (\"ab\")", "#none"),
    ("bytes.base64_encode", "~%Bytes./base64_encode ((~%Bytes./from_str (\"ab\")))", "\"YWI=\""),
    ("bytes.base64_encode", "~%Bytes./base64_encode ((~%Bytes./from_str (\"b\")))", "\"Yg==\""),
    ("bytes.concat", "~%Bytes./concat ((~%Bytes./from_str (\"ab\")), (~%Bytes./from_str (\"ab\")))", "b\"[97, 98, 97, 98]\""),
    ("bytes.concat", "~%Bytes./concat ((~%Bytes./from_str (\"ab\")), (~%Bytes./from_str (\"b\")))", "b\"[97, 98, 98]\""),
    ("bytes.concat", "~%Bytes./concat ((~%Bytes./from_str (\"b\")), (~%Bytes./from_str (\"ab\")))", "b\"[98, 97, 98]\""),
    ("bytes.from_hex", "~%Bytes./from_hex (\"ab\")", "b\"[171]\""),
    ("bytes.from_hex", "~%Bytes./from_hex (\"a,b\")", "#none"),
    ("bytes.from_str", "~%Bytes./from_str (\"ab\")", "b\"[97, 98]\""),
    ("bytes.from_str", "~%Bytes./from_str (\"a,b\")", "b\"[97, 44, 98]\""),
    ("bytes.from_str", "~%Bytes./from_str (\"b\")", "b\"[98]\""),
    ("bytes.hmac_sha256", "~%Bytes./hmac_sha256 ((~%Bytes./from_str (\"ab\")), (~%Bytes./from_str (\"ab\")))", "b\"[226, 66, 70, 98, 5, 172, 228, 34, 37, 5, 196, 97, 127, 193, 19, 14, 231, 10, 203, 37, 174, 92, 22, 159, 18, 144, 190, 166, 190, 68, 104, 92]\""),
    ("bytes.hmac_sha256", "~%Bytes./hmac_sha256 ((~%Bytes./from_str (\"ab\")), (~%Bytes./from_str (\"b\")))", "b\"[221, 237, 136, 205, 55, 238, 187, 81, 147, 90, 131, 117, 188, 63, 142, 69, 203, 122, 167, 83, 133, 3, 187, 171, 75, 9, 79, 238, 25, 128, 114, 233]\""),
    ("bytes.hmac_sha256", "~%Bytes./hmac_sha256 ((~%Bytes./from_str (\"b\")), (~%Bytes./from_str (\"ab\")))", "b\"[120, 150, 122, 164, 208, 122, 95, 79, 179, 133, 195, 128, 228, 221, 20, 155, 178, 108, 88, 32, 31, 202, 158, 69, 242, 97, 220, 166, 233, 206, 69, 32]\""),
    ("bytes.len", "~%Bytes./len ((~%Bytes./from_str (\"ab\")))", "2"),
    ("bytes.len", "~%Bytes./len ((~%Bytes./from_str (\"b\")))", "1"),
    ("bytes.sha256", "~%Bytes./sha256 ((~%Bytes./from_str (\"ab\")))", "b\"[251, 142, 32, 252, 46, 76, 63, 36, 140, 96, 195, 155, 214, 82, 243, 193, 52, 114, 152, 187, 151, 123, 139, 77, 89, 3, 184, 80, 85, 98, 6, 3]\""),
    ("bytes.sha256", "~%Bytes./sha256 ((~%Bytes./from_str (\"b\")))", "b\"[62, 35, 232, 22, 0, 57, 89, 74, 51, 137, 79, 101, 100, 225, 177, 52, 139, 189, 122, 0, 136, 212, 44, 74, 203, 115, 238, 174, 213, 156, 0, 157]\""),
    ("bytes.slice", "~%Bytes./slice (1, 1, (~%Bytes./from_str (\"ab\")))", "b\"[]\""),
    ("bytes.slice", "~%Bytes./slice (1, 2, (~%Bytes./from_str (\"ab\")))", "b\"[98]\""),
    ("bytes.to_hex", "~%Bytes./to_hex ((~%Bytes./from_str (\"ab\")))", "\"6162\""),
    ("bytes.to_hex", "~%Bytes./to_hex ((~%Bytes./from_str (\"b\")))", "\"62\""),
    ("bytes.to_str", "~%Bytes./to_str ((~%Bytes./from_str (\"b\")))", "\"b\""),
    ("bytes.to_str", "~%Bytes./to_str ((~%Bytes./from_str (\"ab\")))", "\"ab\""),
    ("complex.conj", "~%Complex./conj (1.5)", "1.5"),
    ("complex.conj", "~%Complex./conj (1+2i)", "1-2i"),
    ("complex.imag", "~%Complex./imag (1)", "0"),
    ("complex.imag", "~%Complex./imag (1+2i)", "2"),
    ("complex.phase", "~%Complex./phase (1+2i)", "1.1071487177940904"),
    ("complex.phase", "~%Complex./phase (1)", "0"),
    ("complex.real", "~%Complex./real (1.5)", "1.5"),
    ("complex.real", "~%Complex./real (1)", "1"),
    ("complex.real", "~%Complex./real (2)", "2"),
    ("csv.parse", "~%Csv./parse (\"ab\")", "[[\"ab\"]]"),
    ("csv.parse", "~%Csv./parse (\"a,b\")", "[[\"a\", \"b\"]]"),
    ("csv.parse", "~%Csv./parse (\"b\")", "[[\"b\"]]"),
    ("csv.parse_with_headers", "~%Csv./parse_with_headers (\"ab\")", "[]"),
    ("json.get", "~%Json./get (\"ab\", 1)", "#none"),
    ("json.keys", "~%Json./keys ([1, 2, 3])", "[\"0\", \"1\", \"2\"]"),
    ("json.keys", "~%Json./keys ([\"a\", \"b\"])", "[\"0\", \"1\"]"),
    ("json.parse", "~%Json./parse (\"ab\")", "#none"),
    ("json.stringify", "~%Json./stringify (1)", "\"1\""),
    ("json.stringify", "~%Json./stringify (2)", "\"2\""),
    ("json.stringify", "~%Json./stringify (3)", "\"3\""),
    ("list.all", "~%List./all ([1, 2, 3], [1, 2, 3])", "#false"),
    ("list.any", "~%List./any ((x -> x + 1), [1, 2, 3])", "#false"),
    ("list.at", "~%List./at (1, [\"a\", \"b\"])", "\"b\""),
    ("list.at", "~%List./at (1, [1, 2, 3])", "2"),
    ("list.at", "~%List./at (2, [1, 2, 3])", "3"),
    ("list.chunk", "~%List./chunk (1, [1, 2, 3])", "[[1], [2], [3]]"),
    ("list.chunk", "~%List./chunk (1, [\"a\", \"b\"])", "[[\"a\"], [\"b\"]]"),
    ("list.chunk", "~%List./chunk (2, [1, 2, 3])", "[[1, 2], [3]]"),
    ("list.concat", "~%List./concat ([1, 2, 3], [1, 2, 3])", "[1, 2, 3, 1, 2, 3]"),
    ("list.concat", "~%List./concat ([1, 2, 3], [\"a\", \"b\"])", "[1, 2, 3, \"a\", \"b\"]"),
    ("list.concat", "~%List./concat ([\"a\", \"b\"], [1, 2, 3])", "[\"a\", \"b\", 1, 2, 3]"),
    ("list.count", "~%List./count ((x -> x + 1), [1, 2, 3])", "0"),
    ("list.drop", "~%List./drop (1, [1, 2, 3])", "[2, 3]"),
    ("list.drop", "~%List./drop (1, [\"a\", \"b\"])", "[\"b\"]"),
    ("list.drop", "~%List./drop (2, [1, 2, 3])", "[3]"),
    ("list.drop_while", "~%List./drop_while ([1, 2, 3], [1, 2, 3])", "[1, 2, 3]"),
    ("list.enumerate", "~%List./enumerate ([1, 2, 3])", "[{ 0: 0 1: 1 }, { 0: 1 1: 2 }, { 0: 2 1: 3 }]"),
    ("list.enumerate", "~%List./enumerate (1)", "[]"),
    ("list.filter", "~%List./filter ((x -> x + 1), [1, 2, 3])", "[]"),
    ("list.find", "~%List./find ((x -> x + 1), [1, 2, 3])", "#none"),
    ("list.flat_map", "~%List./flat_map (1, [1, 2, 3])", "[]"),
    ("list.flatten", "~%List./flatten ([1, 2, 3])", "[1, 2, 3]"),
    ("list.flatten", "~%List./flatten ([\"a\", \"b\"])", "[\"a\", \"b\"]"),
    ("list.group_by", "~%List./group_by ((x -> x), [1, 2, 1])", "{ 1: [1, 1] 2: [2] }"),
    ("list.head", "~%List./head ([\"a\", \"b\"])", "\"a\""),
    ("list.head", "~%List./head (1)", "#none"),
    ("list.head", "~%List./head ([1, 2, 3])", "1"),
    ("list.intersperse", "~%List./intersperse (1, [1, 2, 3])", "[1, 1, 2, 1, 3]"),
    ("list.intersperse", "~%List./intersperse (1, [\"a\", \"b\"])", "[\"a\", 1, \"b\"]"),
    ("list.intersperse", "~%List./intersperse (2, [1, 2, 3])", "[1, 2, 2, 2, 3]"),
    ("list.len", "~%List./len ([1, 2, 3])", "3"),
    ("list.len", "~%List./len ([\"a\", \"b\"])", "2"),
    ("list.len", "~%List./len ((x -> x + 1))", "0"),
    ("list.map", "~%List./map ((x -> x + 1), [1, 2, 3])", "[2, 3, 4]"),
    ("list.max_by", "~%List./max_by ([1, 2, 3], [1, 2, 3])", "2"),
    ("list.max_by", "~%List./max_by ((x -> x + 1), [1, 2, 3])", "3"),
    ("list.min_by", "~%List./min_by ([1, 2, 3], [1, 2, 3])", "1"),
    ("list.partition", "~%List./partition ((x -> x + 1), [1, 2, 3])", "{ no: [1, 2, 3] yes: [] }"),
    ("list.product", "~%List./product ([1, 2, 3])", "6"),
    ("list.product", "~%List./product (1)", "1"),
    ("list.range", "~%List./range (1, 2)", "[1]"),
    ("list.range", "~%List./range (1, 3)", "[1, 2]"),
    ("list.range", "~%List./range (2, 3)", "[2]"),
    ("list.reduce", "~%List./reduce ((x -> x.0 + x.1), [1, 2, 3])", "6"),
    ("list.reverse", "~%List./reverse ([1, 2, 3])", "[3, 2, 1]"),
    ("list.reverse", "~%List./reverse ([\"a\", \"b\"])", "[\"b\", \"a\"]"),
    ("list.scan", "~%List./scan ((a b -> a + b), 0, [1, 2, 3])", "[1, 3, 6]"),
    ("list.slice", "~%List./slice (1, 2, [1, 2, 3])", "[2]"),
    ("list.slice", "~%List./slice (1, 2, [\"a\", \"b\"])", "[\"b\"]"),
    ("list.slice", "~%List./slice (1, 3, [1, 2, 3])", "[2, 3]"),
    ("list.sort", "~%List./sort ([1, 2, 3])", "[1, 2, 3]"),
    ("list.sort", "~%List./sort ([\"a\", \"b\"])", "[\"a\", \"b\"]"),
    ("list.sort_by", "~%List./sort_by (1, [1, 2, 3])", "[1, 2, 3]"),
    ("list.sort_by", "~%List./sort_by (1, [\"a\", \"b\"])", "[\"a\", \"b\"]"),
    ("list.sum", "~%List./sum ([1, 2, 3])", "6"),
    ("list.sum", "~%List./sum (1)", "0"),
    ("list.tail", "~%List./tail ([1, 2, 3])", "[2, 3]"),
    ("list.tail", "~%List./tail ([\"a\", \"b\"])", "[\"b\"]"),
    ("list.take", "~%List./take (1, [1, 2, 3])", "[1]"),
    ("list.take", "~%List./take (1, [\"a\", \"b\"])", "[\"a\"]"),
    ("list.take", "~%List./take (2, [1, 2, 3])", "[1, 2]"),
    ("list.take_while", "~%List./take_while ([1, 2, 3], [1, 2, 3])", "[]"),
    ("list.transpose", "~%List./transpose (1)", "[]"),
    ("list.window", "~%List./window (1, [1, 2, 3])", "[[1], [2], [3]]"),
    ("list.window", "~%List./window (1, [\"a\", \"b\"])", "[[\"a\"], [\"b\"]]"),
    ("list.window", "~%List./window (2, [1, 2, 3])", "[[1, 2], [2, 3]]"),
    ("list.zip", "~%List./zip ([1, 2, 3], (x -> x + 1))", "[]"),
    ("math.abs", "~%Math./abs (1.5)", "1.5"),
    ("math.abs", "~%Math./abs (1+2i)", "2.23606797749979"),
    ("math.add", "~%Math./add (1, 2)", "3"),
    ("math.atan2", "~%Math./atan2 (1, 1)", "0.7853981633974483"),
    ("math.atan2", "~%Math./atan2 (1, 2)", "0.4636476090008061"),
    ("math.atan2", "~%Math./atan2 (1, 3)", "0.3217505543966422"),
    ("math.bits", "~%Math./bits (\"ab\")", "16"),
    ("math.bits", "~%Math./bits (\"a,b\")", "24"),
    ("math.bits", "~%Math./bits (\"b\")", "8"),
    ("math.ceil", "~%Math./ceil (1)", "1"),
    ("math.ceil", "~%Math./ceil (2)", "2"),
    ("math.ceil", "~%Math./ceil (3)", "3"),
    ("math.choose", "~%Math./choose (1, 1)", "1"),
    ("math.choose", "~%Math./choose (1, 2)", "0"),
    ("math.choose", "~%Math./choose (2, 1)", "2"),
    ("math.clamp", "~%Math./clamp (1, 2, 1.5)", "1.5"),
    ("math.clamp", "~%Math./clamp (1, 1, 1)", "1"),
    ("math.clamp", "~%Math./clamp (1, 2, 2)", "2"),
    ("math.cos", "~%Math./cos (1)", "0.5403023058681398"),
    ("math.cos", "~%Math./cos (2)", "-0.4161468365471424"),
    ("math.cos", "~%Math./cos (3)", "-0.9899924966004454"),
    ("math.cosh", "~%Math./cosh (1)", "1.5430806348152437"),
    ("math.cosh", "~%Math./cosh (2)", "3.7621956910836314"),
    ("math.cosh", "~%Math./cosh (3)", "10.067661995777765"),
    ("math.div", "~%Math./div (1, 1.5)", "0.6666666666666666"),
    ("math.div", "~%Math./div (1, 1+2i)", "0.2-0.4i"),
    ("math.div", "~%Math./div (2, 1.5)", "1.3333333333333333"),
    ("math.eml", "~%Math./eml (1, 1)", "2.718281828459045+0i"),
    ("math.eml", "~%Math./eml (1, 2)", "2.0251346478990997+0i"),
    ("math.eml", "~%Math./eml (1, 3)", "1.6196695397909353+0i"),
    ("math.exp", "~%Math./exp (1)", "2.718281828459045+0i"),
    ("math.exp", "~%Math./exp (2)", "7.38905609893065+0i"),
    ("math.exp", "~%Math./exp (3)", "20.085536923187668+0i"),
    ("math.factorial", "~%Math./factorial (3)", "6"),
    ("math.factorial", "~%Math./factorial (1)", "1"),
    ("math.factorial", "~%Math./factorial (2)", "2"),
    ("math.floor", "~%Math./floor (1)", "1"),
    ("math.floor", "~%Math./floor (2)", "2"),
    ("math.floor", "~%Math./floor (3)", "3"),
    ("math.fract", "~%Math./fract (1.5)", "0.5"),
    ("math.fract", "~%Math./fract (1)", "0"),
    ("math.gcd", "~%Math./gcd (1, 1)", "1"),
    ("math.gcd", "~%Math./gcd (2, 2)", "2"),
    ("math.gcd", "~%Math./gcd (3, 3)", "3"),
    ("math.gt", "~%Math./gt (1, 1)", "#false"),
    ("math.gt", "~%Math./gt (2, 1)", "#true"),
    ("math.gte", "~%Math./gte (1, 1)", "#true"),
    ("math.gte", "~%Math./gte (1, 2)", "#false"),
    ("math.hypot", "~%Math./hypot (1, 1)", "1.4142135623730951"),
    ("math.hypot", "~%Math./hypot (1, 2)", "2.23606797749979"),
    ("math.hypot", "~%Math./hypot (1, 3)", "3.1622776601683795"),
    ("math.is_prime", "~%Math./is_prime (1)", "#false"),
    ("math.is_prime", "~%Math./is_prime (2)", "#true"),
    ("math.lcm", "~%Math./lcm (2, 3)", "6"),
    ("math.lcm", "~%Math./lcm (1, 1)", "1"),
    ("math.lcm", "~%Math./lcm (1, 2)", "2"),
    ("math.ln", "~%Math./ln (1)", "0+0i"),
    ("math.ln", "~%Math./ln (2)", "0.6931471805599453+0i"),
    ("math.ln", "~%Math./ln (3)", "1.0986122886681098+0i"),
    ("math.log10", "~%Math./log10 (2)", "0.3010299956639812"),
    ("math.log10", "~%Math./log10 (3)", "0.47712125471966244"),
    ("math.log10", "~%Math./log10 (1.5)", "0.17609125905568124"),
    ("math.log2", "~%Math./log2 (3)", "1.584962500721156"),
    ("math.log2", "~%Math./log2 (1.5)", "0.5849625007211562"),
    ("math.lt", "~%Math./lt (1, 1)", "#false"),
    ("math.lt", "~%Math./lt (1, 2)", "#true"),
    ("math.lte", "~%Math./lte (1, 1)", "#true"),
    ("math.lte", "~%Math./lte (2, 1)", "#false"),
    ("math.max", "~%Math./max (1, 1.5)", "1.5"),
    ("math.max", "~%Math./max (1, 1)", "1"),
    ("math.max", "~%Math./max (1, 2)", "2"),
    ("math.min", "~%Math./min (2, 1.5)", "1.5"),
    ("math.min", "~%Math./min (1, 1)", "1"),
    ("math.min", "~%Math./min (2, 2)", "2"),
    ("math.mul", "~%Math./mul (1, 1.5)", "1.5"),
    ("math.mul", "~%Math./mul (1, 1+2i)", "1+2i"),
    ("math.mul", "~%Math./mul (2, 2)", "4"),
    ("math.pow", "~%Math./pow (2, 2)", "4"),
    ("math.pow", "~%Math./pow (2, 3)", "8"),
    ("math.pow", "~%Math./pow (2, 1.5)", "2.8284271247461903"),
    ("math.pow_mod", "~%Math./pow_mod (1, 1, 1)", "0"),
    ("math.pow_mod", "~%Math./pow_mod (1, 1, 2)", "1"),
    ("math.pow_mod", "~%Math./pow_mod (2, 1, 3)", "2"),
    ("math.rem", "~%Math./rem (1, 1)", "0"),
    ("math.rem", "~%Math./rem (1, 2)", "1"),
    ("math.rem", "~%Math./rem (2, 3)", "2"),
    ("math.round", "~%Math./round (1)", "1"),
    ("math.round", "~%Math./round (2)", "2"),
    ("math.round", "~%Math./round (3)", "3"),
    ("math.sign", "~%Math./sign (1)", "1"),
    ("math.sin", "~%Math./sin (1)", "0.8414709848078965"),
    ("math.sin", "~%Math./sin (2)", "0.9092974268256817"),
    ("math.sin", "~%Math./sin (3)", "0.1411200080598672"),
    ("math.sinh", "~%Math./sinh (1)", "1.1752011936438014"),
    ("math.sinh", "~%Math./sinh (2)", "3.626860407847019"),
    ("math.sinh", "~%Math./sinh (3)", "10.017874927409903"),
    ("math.sqrt", "~%Math./sqrt (2)", "1.4142135623730951"),
    ("math.sqrt", "~%Math./sqrt (3)", "1.7320508075688772"),
    ("math.sqrt", "~%Math./sqrt (1.5)", "1.224744871391589"),
    ("math.sub", "~%Math./sub (1, 2)", "-1"),
    ("math.sub", "~%Math./sub (1, 3)", "-2"),
    ("math.sub", "~%Math./sub (1, 1.5)", "-0.5"),
    ("math.tanh", "~%Math./tanh (1)", "0.7615941559557649"),
    ("math.tanh", "~%Math./tanh (2)", "0.9640275800758169"),
    ("math.tanh", "~%Math./tanh (3)", "0.9950547536867305"),
    ("math.to_float", "~%Math./to_float (1.5)", "1.5"),
    ("math.to_float", "~%Math./to_float (1)", "1"),
    ("math.to_float", "~%Math./to_float (2)", "2"),
    ("math.trunc", "~%Math./trunc (1)", "1"),
    ("math.trunc", "~%Math./trunc (2)", "2"),
    ("math.trunc", "~%Math./trunc (3)", "3"),
    ("path.basename", "~%Path./basename (\"a,b\")", "\"a,b\""),
    ("path.basename", "~%Path./basename (\"b\")", "\"b\""),
    ("path.dirname", "~%Path./dirname (\"ab\")", "\"\""),
    ("path.extension", "~%Path./extension (\"ab\")", "#none"),
    ("path.is_absolute", "~%Path./is_absolute (\"ab\")", "#false"),
    ("path.join", "~%Path./join (\"ab\", \"ab\")", "\"ab/ab\""),
    ("path.join", "~%Path./join (\"ab\", \"a,b\")", "\"ab/a,b\""),
    ("path.join", "~%Path./join (\"ab\", \"b\")", "\"ab/b\""),
    ("regex.find", "~%Regex./find (\"ab\", \"ab\")", "{ end: 2 match: \"ab\" start: 0 }"),
    ("regex.find", "~%Regex./find (\"ab\", \"a,b\")", "#none"),
    ("regex.match", "~%Regex./match (\"ab\", \"ab\")", "#true"),
    ("regex.match", "~%Regex./match (\"ab\", \"a,b\")", "#false"),
    ("regex.replace", "~%Regex./replace (\"ab\", \"ab\", \"a,b\")", "\"a,b\""),
    ("regex.replace", "~%Regex./replace (\"ab\", \"ab\", \"b\")", "\"b\""),
    ("regex.replace", "~%Regex./replace (\"b\", \"ab\", \"ab\")", "\"aab\""),
    ("regex.split", "~%Regex./split (\"ab\", \"ab\")", "[\"\", \"\"]"),
    ("regex.split", "~%Regex./split (\"ab\", \"a,b\")", "[\"a,b\"]"),
    ("regex.split", "~%Regex./split (\"ab\", \"b\")", "[\"b\"]"),
    ("stat.histogram", "~%Stat./histogram ([1, 2, 3], 1)", "[[1, 2, 3]]"),
    ("stat.histogram", "~%Stat./histogram ([1, 2, 3], 2)", "[[1], [2, 3]]"),
    ("stat.histogram", "~%Stat./histogram ([1, 2, 3], 3)", "[[1], [2], [3]]"),
    ("stat.percentile", "~%Stat./percentile ([1, 2, 3], 1)", "1.02"),
    ("stat.percentile", "~%Stat./percentile ([1, 2, 3], 2)", "1.04"),
    ("stat.percentile", "~%Stat./percentile ([1, 2, 3], 3)", "1.06"),
    ("str.char_at", "~%Str./char_at (1, \"ab\")", "\"b\""),
    ("str.char_at", "~%Str./char_at (1, \"a,b\")", "\",\""),
    ("str.chars", "~%Str./chars (\"ab\")", "[\"a\", \"b\"]"),
    ("str.chars", "~%Str./chars (\"a,b\")", "[\"a\", \",\", \"b\"]"),
    ("str.chars", "~%Str./chars (\"b\")", "[\"b\"]"),
    ("str.concat", "~%Str./concat (1, 3)", "4"),
    ("str.concat", "~%Str./concat (1, 1.5)", "2.5"),
    ("str.concat", "~%Str./concat (1, 1+2i)", "2+2i"),
    ("str.contains", "~%Str./contains (\"ab\", \"ab\")", "#true"),
    ("str.contains", "~%Str./contains (\"ab\", \"a,b\")", "#false"),
    ("str.count", "~%Str./count (\"ab\", \"ab\")", "1"),
    ("str.count", "~%Str./count (\"ab\", \"a,b\")", "0"),
    ("str.decode_uri", "~%Str./decode_uri (\"a,b\")", "\"a,b\""),
    ("str.decode_uri", "~%Str./decode_uri (\"b\")", "\"b\""),
    ("str.encode_uri", "~%Str./encode_uri (\"a,b\")", "\"a%2Cb\""),
    ("str.encode_uri", "~%Str./encode_uri (\"b\")", "\"b\""),
    ("str.ends_with", "~%Str./ends_with (\"ab\", \"ab\")", "#true"),
    ("str.ends_with", "~%Str./ends_with (\"ab\", \"a,b\")", "#false"),
    ("str.format", "~%Str./format (\"a,b\", 1)", "\"a,b\""),
    ("str.format", "~%Str./format (\"b\", 1)", "\"b\""),
    ("str.from_int", "~%Str./from_int (1)", "\"1\""),
    ("str.from_int", "~%Str./from_int (2)", "\"2\""),
    ("str.from_int", "~%Str./from_int (3)", "\"3\""),
    ("str.index_of", "~%Str./index_of (\"ab\", \"ab\")", "0"),
    ("str.index_of", "~%Str./index_of (\"ab\", \"a,b\")", "#none"),
    ("str.index_of", "~%Str./index_of (\"b\", \"ab\")", "1"),
    ("str.is_empty", "~%Str./is_empty (\"ab\")", "#false"),
    ("str.join", "~%Str./join (\"ab\", [\"a\", \"b\"])", "\"aabb\""),
    ("str.join", "~%Str./join (\"a,b\", [\"a\", \"b\"])", "\"aa,bb\""),
    ("str.join", "~%Str./join (\"b\", [\"a\", \"b\"])", "\"abb\""),
    ("str.len", "~%Str./len (\"ab\")", "2"),
    ("str.len", "~%Str./len (\"a,b\")", "3"),
    ("str.len", "~%Str./len (\"b\")", "1"),
    ("str.levenshtein", "~%Str./levenshtein (\"ab\", \"ab\")", "0"),
    ("str.levenshtein", "~%Str./levenshtein (\"ab\", \"a,b\")", "1"),
    ("str.levenshtein", "~%Str./levenshtein (\"a,b\", \"b\")", "2"),
    ("str.lines", "~%Str./lines (\"ab\")", "[\"ab\"]"),
    ("str.lines", "~%Str./lines (\"a,b\")", "[\"a,b\"]"),
    ("str.lines", "~%Str./lines (\"b\")", "[\"b\"]"),
    ("str.pad_left", "~%Str./pad_left (1, \"a,b\")", "\"a,b\""),
    ("str.pad_left", "~%Str./pad_left (1, \"b\")", "\"b\""),
    ("str.pad_left", "~%Str./pad_left (2, \"b\")", "\" b\""),
    ("str.pad_right", "~%Str./pad_right (1, \"a,b\")", "\"a,b\""),
    ("str.pad_right", "~%Str./pad_right (1, \"b\")", "\"b\""),
    ("str.pad_right", "~%Str./pad_right (2, \"b\")", "\"b \""),
    ("str.parse_float", "~%Str./parse_float (\"1.5\")", "1.5"),
    ("str.parse_int", "~%Str./parse_int (\"5\")", "5"),
    ("str.repeat", "~%Str./repeat (1, \"a,b\")", "\"a,b\""),
    ("str.repeat", "~%Str./repeat (1, \"b\")", "\"b\""),
    ("str.repeat", "~%Str./repeat (2, \"ab\")", "\"abab\""),
    ("str.replace", "~%Str./replace (\"ab\", \"ab\", \"a,b\")", "\"a,b\""),
    ("str.replace", "~%Str./replace (\"ab\", \"ab\", \"b\")", "\"b\""),
    ("str.replace", "~%Str./replace (\"b\", \"ab\", \"ab\")", "\"aab\""),
    ("str.reverse", "~%Str./reverse (\"ab\")", "\"ba\""),
    ("str.reverse", "~%Str./reverse (\"a,b\")", "\"b,a\""),
    ("str.reverse", "~%Str./reverse (\"b\")", "\"b\""),
    ("str.slice", "~%Str./slice (1, 2, \"ab\")", "\"b\""),
    ("str.slice", "~%Str./slice (1, 2, \"a,b\")", "\",\""),
    ("str.slice", "~%Str./slice (1, 3, \"a,b\")", "\",b\""),
    ("str.split", "~%Str./split (\"ab\", \"ab\")", "[\"\", \"\"]"),
    ("str.split", "~%Str./split (\"ab\", \"a,b\")", "[\"a,b\"]"),
    ("str.split", "~%Str./split (\"ab\", \"b\")", "[\"b\"]"),
    ("str.starts_with", "~%Str./starts_with (\"ab\", \"ab\")", "#true"),
    ("str.starts_with", "~%Str./starts_with (\"ab\", \"a,b\")", "#false"),
    ("str.title_case", "~%Str./title_case (\"ab\")", "\"Ab\""),
    ("str.title_case", "~%Str./title_case (\"a,b\")", "\"A,b\""),
    ("str.title_case", "~%Str./title_case (\"b\")", "\"B\""),
    ("str.to_lower", "~%Str./to_lower (\"a,b\")", "\"a,b\""),
    ("str.to_lower", "~%Str./to_lower (\"b\")", "\"b\""),
    ("str.to_upper", "~%Str./to_upper (\"ab\")", "\"AB\""),
    ("str.to_upper", "~%Str./to_upper (\"a,b\")", "\"A,B\""),
    ("str.to_upper", "~%Str./to_upper (\"b\")", "\"B\""),
    ("str.trim", "~%Str./trim (\"a,b\")", "\"a,b\""),
    ("str.trim", "~%Str./trim (\"b\")", "\"b\""),
    ("str.trim_end", "~%Str./trim_end (\"a,b\")", "\"a,b\""),
    ("str.trim_end", "~%Str./trim_end (\"b\")", "\"b\""),
    ("str.trim_start", "~%Str./trim_start (\"a,b\")", "\"a,b\""),
    ("str.trim_start", "~%Str./trim_start (\"b\")", "\"b\""),
    ("str.word_count", "~%Str./word_count (\"ab\")", "1"),
    ("time.add_days", "~%Time./add_days (1, 1)", "86400001"),
    ("time.add_days", "~%Time./add_days (1, 2)", "172800001"),
    ("time.add_days", "~%Time./add_days (1, 3)", "259200001"),
    ("time.add_hours", "~%Time./add_hours (1, 1)", "3600001"),
    ("time.add_hours", "~%Time./add_hours (1, 2)", "7200001"),
    ("time.add_hours", "~%Time./add_hours (1, 3)", "10800001"),
    ("time.add_ms", "~%Time./add_ms (1, 3)", "4"),
    ("time.add_ms", "~%Time./add_ms (2, 3)", "5"),
    ("time.add_ms", "~%Time./add_ms (3, 3)", "6"),
    ("time.diff", "~%Time./diff (1, 2)", "-1"),
    ("time.diff", "~%Time./diff (1, 3)", "-2"),
    ("time.format", "~%Time./format (\"a,b\", 1)", "\"a,b\""),
    ("time.format", "~%Time./format (\"b\", 1)", "\"b\""),
    ("time.to_iso8601", "~%Time./to_iso8601 (1)", "\"1970-01-01T00:00:00.001Z\""),
    ("time.to_iso8601", "~%Time./to_iso8601 (2)", "\"1970-01-01T00:00:00.002Z\""),
    ("time.to_iso8601", "~%Time./to_iso8601 (3)", "\"1970-01-01T00:00:00.003Z\""),
    ("time.weekday", "~%Time./weekday (1)", "#thursday"),
    ("toml.parse", "~%Toml./parse (\"a = 1\")", "{ a: 1 }"),
    ("url.decode", "~%Url./decode (\"a%20b\")", "\"a b\""),
    ("url.encode", "~%Url./encode (\"a b\")", "\"a%20b\""),
    ("url.join", "~%Url./join (\"http://a/\", \"b\")", "\"http://a/b\""),
    ("url.parse", "~%Url./parse (\"http://a/b\")", "{ fragment: \"\" host: \"a\" path: \"/b\" query: \"\" scheme: \"http\" }"),
    ("url.query_params", "~%Url./query_params (\"http://a/b?x=1\")", "{ x: \"1\" }"),
];
