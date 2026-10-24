// A name bound before the meet.
// Order: nlang-tools/docs/a_name_bound_before_the_meet_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-103.  D118: a name resolves in
// the node it is written in, as that node is after the meet (the second
// half; Q-102 did "wherever it is forced"). D118 Q2: a private key is seen
// only by names written in a literal that declares it.
//
// ── What is wrong (v0.103.0) ────────────────────────────────────────────
//
// A meet binds names before it meets:
//   `({ p: @int, s: p } & { p: 5 }).s`  ⟹ `@int`   (one literal ⟹ 5)
//   `({ s: p } & { p: 5 }).s`  ⟹ `_`
// A field written on both sides is met at once, its names unresolved —
// inside one literal too:
//   `{ p: @int, s: p + 1, p: 5, s: @int }.s`  ⟹ `@int`   (6)
// SPEC_05 §5.1's generic does not specialise:
//   `{ value: "hi" } & @IntBox`  ⟹ `{ value: "hi" }`
// Committed, the same writes read `_` in place, in one file, over two
// evolves and over two commits.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("bound-meet-{tag}"));
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

/// `oo eval` from an empty directory (no universe), a ⊥ reduced to its cause.
fn eval_here(e: &str) -> String {
    let w = Ws::new("eval");
    bottom(&w.eval(e))
}

fn bottom(got: &str) -> String {
    for cause in ["#conflict", "#no_matching_branch"] {
        if got.starts_with("_|_") && got.contains(cause) {
            return format!("_|_ {cause}");
        }
    }
    got.into()
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

fn table(bad: &mut Vec<String>, cells: &[(&str, &str)]) {
    for (e, want) in cells {
        check(bad, e, eval_here(e), want);
    }
}

/// r1 — A meet's fields read the met node: one way round and the other, a
/// name only the other side has, nested, computed, through a closure, a
/// path, a reader that has the name too, three ways, a meet of meets, a
/// spread.
#[test]
fn r1_a_meets_fields_read_the_met_node() {
    let mut bad = Vec::new();
    let cells = [
        ("({ p: @int, s: p } & { p: 5 }).s", "5"),
        ("({ p: 5 } & { p: @int, s: p }).s", "5"),
        ("({ s: p } & { p: 5 }).s", "5"),
        ("({ p: @int, s: { t: p } } & { p: 5 }).s.t", "5"),
        ("{ a: { p: @int, s: p + 1 }, b: a & { p: 5 } }.b.s", "6"),
        ("{ a: { p: @int, f: (x -> p) }, b: a & { p: 7 }, r: b.f 0 }.r", "7"),
        ("{ a: { p: @int, s: { t: p } }, b: a & { p: 5 }, r: b.s.t }.r", "5"),
        ("{ a: { p: @int, s: p }, b: a & { p: 5 }, r: { p: 9, z: b.s }.z }.r", "5"),
        ("(({ s: p } & { q: 1 }) & { p: q }).s", "1"),
        ("({ a: { p: @int, s: p } } & { a: { p: 5 } }).a.s", "5"),
        ("({ ...{ j: 5 } } & { q: j }).q", "5"),
    ];
    table(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// r2 — A field written on both sides meets in the met node, in two
/// literals and in one; the same writes in any grouping give one answer,
/// and a conflict they make carries no message (D117).
#[test]
fn r2_a_field_both_sides_write_meets_in_the_met_node() {
    let mut bad = Vec::new();
    let cells = [
        ("({ p: @int, s: p + 1 } & { p: 5, s: @int }).s", "6"),
        ("{ p: @int, s: p + 1, p: 5, s: @int }.s", "6"),
        ("{ s: p, p: 1, s: @int }.s", "1"),
        ("{ s: p, p: 1, s: 2 }.s", "_|_ #conflict"),
        ("{ s: 2, p: 1, s: p }.s", "_|_ #conflict"),
        ("({ p: 1, s: 2 } & { s: p }).s", "_|_ #conflict"),
    ];
    table(&mut bad, &cells);
    let w = Ws::new("r2-raw");
    for e in ["{ s: p, p: 1, s: 2 }.s", "({ p: 1, s: 2 } & { s: p }).s"] {
        check(&mut bad, &format!("no message: {e}"), w.eval(e), "_|_ ;; %cause: #conflict");
    }
    verdict(bad, cells.len() + 2);
}

/// r3 — `SPEC_05` §5.1: a generic specialises by meeting its parameter.
#[test]
fn r3_a_generic_specialises_by_the_meet() {
    let mut bad = Vec::new();
    let g = "@Box: { ~@T: @any, value: ~@T }, @IntBox: @Box & { ~@T: @int }";
    let cells = [
        (format!("{{ {g}, ok: {{ value: 42 }} & @IntBox }}.ok"), "{ value: 42 }"),
        (format!("{{ {g}, bad: {{ value: \"hi\" }} & @IntBox }}.bad"), "{ value: _|_ ;; %cause: #conflict }"),
        (format!("{{ {g}, bad: {{ value: \"hi\" }} & @IntBox }}.bad.value"), "_|_ #conflict"),
    ];
    for (e, want) in &cells {
        check(&mut bad, e, eval_here(e), want);
    }
    verdict(bad, cells.len());
}

/// r4 — A private key declared on both sides is one field, read at its met
/// value by both (Q2); a public name of the met node wins over a private one
/// of the same name, as in one literal.
#[test]
fn r4_a_private_key_declared_on_both_sides() {
    let mut bad = Vec::new();
    let cells = [
        ("{ p: { ~s: 1, a: 2 }, q: p & { ~s: _, peek: ~s }, r: q.peek }.r", "1"),
        ("{ p: { ~s: 1, a: ~s }, q: p & { ~s: @int }, r: q.a }.r", "1"),
        ("({ ~s: 1, a: s } & { s: 2 }).a", "2"),
    ];
    table(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// r5 — Committed, the same writes read the met node: in place, in one
/// file, over two evolves, over two commits; nested. One file, two evolves
/// and two commits keep one root (as v0.103.0 did).
#[test]
fn r5_what_is_committed_reads_the_met_node() {
    let mut bad = Vec::new();
    let mut roots = Vec::new();
    for (label, writes) in [
        ("in place", vec!["r: { z: y } & { y: 5 }"]),
        ("one file", vec!["r: { z: y }\nr: { y: 5 }"]),
        ("two evolves", vec!["r: { z: y }", "r: { y: 5 }"]),
    ] {
        let w = Ws::new("r5");
        let want = format!("{} c0", vec!["e0"; writes.len()].join(" "));
        check(&mut bad, &format!("{label}: commit"), w.writes(&writes), &want);
        check(&mut bad, &format!("{label}: _.r.z"), w.eval("_.r.z"), "5");
        roots.push((label, w.root()));
    }
    let w = Ws::new("r5-two");
    check(&mut bad, "two commits: first", w.writes(&["r: { z: y }"]), "e0 c0");
    check(&mut bad, "two commits: second", w.writes(&["r: { y: 5 }"]), "e0 c0");
    check(&mut bad, "two commits: _.r.z", w.eval("_.r.z"), "5");
    roots.push(("two commits", w.root()));
    for (label, r) in &roots[2..] {
        check(&mut bad, &format!("{label}: root as one file"), r.clone(), &roots[1].1);
    }
    let w = Ws::new("r5-nested");
    check(&mut bad, "nested: commit", w.writes(&["r: { a: { p: @int, s: { t: p } } & { p: 5 } }"]), "e0 c0");
    check(&mut bad, "nested: _.r.a.s.t", w.eval("_.r.a.s.t"), "5");
    verdict(bad, 6 + 3 + 2 + 2);
}

/// g1 — What does not meet a name reads as before (green today): a private
/// key the other side does not declare; same-name values meet at once; a
/// write and itself; Q-102's readings; `^`; the open world.
#[test]
fn g1_what_meets_no_name_reads_as_before() {
    let mut bad = Vec::new();
    let cells = [
        ("{ p: { ~s: 1, a: 2 }, q: p & { peek: ~s }, r: q.peek }.r", "_"),
        ("({ a: 1 } & { a: 2 }).a", "_|_ #conflict"),
        ("({ a: 1 } & { a: 1, b: 2 }).b", "2"),
        ("{ a: 1, a: 1 }", "{ a: 1 }"),
        ("{ p: 1, s: { t: p }, r: s.t }.r", "1"),
        ("{ p: 1, f: (x -> p), r: { p: 3, z: f 0 }.z }.r", "1"),
        ("{ y: 1, a: { ...{ j: 5 }, b: ^.y } }.a.b", "1"),
        ("{ s: q, r: { q: 3, z: s }.z }.r", "_"),
        ("({ s: q } & { p: 5 }).s", "_"),
    ];
    table(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// g2 — Roots that hold no meet of names do not move (v0.103.0).
#[test]
fn g2_roots_without_a_meet_of_names_do_not_move() {
    let mut bad = Vec::new();
    for (src, root) in [
        ("x: 0", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        ("v: 1 + 1", "df91a8ad239ea8727c3a03a71eb4b0a787312b92306db4d8b124a73faaf40199"),
        ("f: (x -> x + 1)", "258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f"),
        ("a: 1\nb: a", "132ced2494ea0551192cab462b1d58afed6d742fccc99e60fa117e2b876ad269"),
        ("c: { b: 2 }\nr: c\nr: c", "282e6c79fc12ff4732e2bfdb720aa6713f3ca206966dbbd275db0e8ba598d6b7"),
        ("t: { a: 1 } & { b: 2 }", "ee2c382ff3a0337e3b515e41d5c0891010ef8cf62c3323486fcf933bb2b02fea"),
        ("p: 1\ns: { t: p }\nr: s.t", "31462702b5f1132753a9bfb2db47ce51bed0add691f668cc806e55c378926fe5"),
    ] {
        check(&mut bad, src, root_of(src), root);
    }
    verdict(bad, 7);
}
