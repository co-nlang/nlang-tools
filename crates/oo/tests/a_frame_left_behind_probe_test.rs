// A frame left behind.
// Order: nlang-tools/docs/a_frame_left_behind_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-102.  D118: a name resolves in
// the node it is written in, wherever it is forced (SPEC_04 §2.1). Q-102 is
// the first half: where. When — after a meet — is Q-103.
//
// ── What is wrong (v0.102.0; v0.40.0 and every release since) ───────────
//
// A thunk's closure lacks the frame of the literal it is written in; that
// frame is added only when a bare name forces it, and then the forcer's whole
// chain comes with it:
//   * a dotted path forces with the closure alone, so the literal's own names
//     are gone:  `{ p: 1, s: { t: p }, r: s.t }.r`  ⟹ `_`   (`(s).t` ⟹ 1)
//   * a bare name reads the forcer's names first (dynamic scope):
//       `{ p: 1, f: (x -> p), r: { p: 3, z: f 0 }.z }.r`  ⟹ 3
//       `{ p: 1, s: p, r: { p: 3, z: s }.z }.r`  ⟹ 3   (`.s` ⟹ 1)
//   * an arrow captures only the names its body reads, and the names a
//     pattern reads are not counted, so a table key in an arrow body matches
//     everything:  `{ @Pos: 1.., /f: a -> { @{ @Pos }: "y", _: "n" }, r: /f 1 0 }.r`  ⟹ "y"
// At the top level of a file the same forms read right: a missing name falls
// back to the root.  What is committed holds the wrong answer.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("frame-left-{tag}"));
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

/// r1 — A dotted path reads the names of the literal it reaches where they
/// were written. Data, type and function names alike, one and two levels
/// down, through an alias, an index, a table, an arrow, a meet; the nearer
/// of two same names wins when the field is read from outside.
#[test]
fn r1_a_dotted_path_reads_names_where_they_were_written() {
    let mut bad = Vec::new();
    let cells = [
        ("{ p: 1, s: { t: p }, r: s.t }.r", "1"),
        ("{ p: 1, s: { u: { t: p } }, r: s.u.t }.r", "1"),
        ("{ p: 1, s: { t: p }, q: s, r: q.t }.r", "1"),
        ("{ p: 1, s: [p], r: s.0 }.r", "1"),
        ("{ p: 1, s: { @{ 4 }: p }, r: s.4 }.r", "1"),
        ("{ p: 1, s: { t: { @{ 4 }: p } }, r: s.t 4 }.r", "1"),
        ("{ p: 1, s: { f: (x -> p) }, r: s.f 0 }.r", "1"),
        ("{ @Pos: 1.., s: { t: 0 & @Pos }, r: s.t }.r", "_|_ #conflict"),
        ("{ /g: x -> x + 1, s: { t: /g 1 }, r: s.t }.r", "2"),
        ("{ @Pos: 1.., s: { t: { @{ @Pos }: \"y\", _: \"n\" } }, r: s.t 0 }.r", "\"n\""),
        ("{ p: { ~s: 1, a: ~s }, q: p & { b: 2 }, r: q.a }.r", "1"),
        ("{ p: 1, s: { p: 2, q: p, t: q } }.s.t", "2"),
    ];
    table(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// r2 — A name is not read where it is forced: a closure, a sibling and a
/// path read by a literal that defines the same name, or by an arrow whose
/// parameter has that name, still read the definer's — and a name the
/// definer does not have is `_`, whatever the forcer has.
#[test]
fn r2_a_name_is_not_read_where_it_is_forced() {
    let mut bad = Vec::new();
    let cells = [
        ("{ p: 1, f: (x -> p), r: { p: 3, z: f 0 }.z }.r", "1"),
        ("{ p: 1, s: p, r: { p: 3, z: s }.z }.r", "1"),
        ("{ p: 1, s: { t: p }, r: { p: 3, z: s.t }.z }.r", "1"),
        ("{ p: 1, s: { t: p }, r: { p: 3, z: s }.z.t }.r", "1"),
        ("{ p: 1, f: (p -> s.t), s: { t: p }, r: f 9 }.r", "1"),
        ("{ p: 1, f: (p -> s), s: p, r: f 9 }.r", "1"),
        ("{ s: q, r: { q: 3, z: s }.z }.r", "_"),
        ("{ f: (x -> q), r: { q: 3, z: f 0 }.z }.r", "_"),
    ];
    table(&mut bad, &cells);
    verdict(bad, cells.len());
}

/// r3 — A pattern reads the names around the place it is written: a table
/// key or an arrow's parameter pattern inside an arrow body, a table an arrow
/// returns, the `QUICK_REFERENCE` §7 safe division in a literal.
#[test]
fn r3_a_pattern_reads_the_names_around_it() {
    let mut bad = Vec::new();
    let t = "{ @{ @Pos }: \"y\", _: \"n\" }";
    let cells = [
        (format!("{{ @Pos: 1.., /f: a -> {t}, r: /f 1 0 }}.r"), "\"n\""),
        (format!("{{ @Pos: 1.., /f: a -> {t}, r: /f 1 5 }}.r"), "\"y\""),
        ("{ @Pos: 1.., /f: a -> (x @Pos -> \"y\"), r: /f 1 0 }.r".to_string(), "_|_ #no_matching_branch"),
        (format!("{{ @Pos: 1.., g: {t}, f: (a -> g), r: (f 1) 0 }}.r"), "\"n\""),
        ("{ @NonZero: @int & !0, /safe_div: a -> { @{ @NonZero }: a / $ }, r: /safe_div 6 0 }.r".to_string(), "_|_ #no_matching_branch"),
        ("{ @NonZero: @int & !0, /safe_div: a -> { @{ @NonZero }: a / $ }, r: /safe_div 6 3 }.r".to_string(), "2"),
    ];
    for (e, want) in &cells {
        check(&mut bad, e, eval_here(e), want);
    }
    verdict(bad, cells.len());
}

/// r4 — What is committed reads the same as in place: a record that reads a
/// sibling's name through a path, a table built in an arrow body, both in a
/// field of the root.
#[test]
fn r4_what_is_committed_reads_the_same() {
    let mut bad = Vec::new();
    let w = Ws::new("r4");
    let src = "n: { p: 1, s: { t: p }, r: s.t }\nm: { @Pos: 1.., /f: a -> { @{ @Pos }: \"y\", _: \"n\" }, r: /f 1 0 }\n";
    check(&mut bad, "commit", w.writes(&[src]), "e0 c0");
    check(&mut bad, "_.n.r", w.eval("_.n.r"), "1");
    check(&mut bad, "_.n", w.eval("_.n"), "{ p: 1 r: 1 s: { t: 1 } }");
    check(&mut bad, "_.m.r", w.eval("_.m.r"), "\"n\"");
    verdict(bad, 4);
}

/// g1 — What reads right today reads the same (green today): the other ways
/// to reach the same field, an inner name shadowing an outer, the open world,
/// cycles, `^`, a private field across `&`, the same forms at the root.
#[test]
fn g1_what_reads_right_reads_the_same() {
    let mut bad = Vec::new();
    let cells = [
        ("{ p: 1, s: { t: p }, r: (s).t }.r", "1"),
        ("{ p: 1, s: { t: p }, r: { ...s }.t }.r", "1"),
        ("{ p: 1, s: { t: p }, r: s |> $.t }.r", "1"),
        ("{ p: 1, s: { @{ 4 }: p }, r: s 4 }.r", "1"),
        ("{ p: 1, f: (x -> p), r: f 0 }.r", "1"),
        ("{ p: 1, /f: a -> { @{ 4 }: p }, r: /f 1 4 }.r", "1"),
        ("{ p: 1, s: { t: p } }.s.t", "1"),
        ("{ p: 1, s: { p: 2, t: p }, r: s.t }.r", "2"),
        ("{ s: { t: nope }, r: s.t }.r", "_"),
        ("{ a: b, b: a }.a", "_"),
        ("{ s: { t: s.t }, r: s.t }.r", "_"),
        ("{ ...{ j: 5 }, q: ^.j }.q", "_"),
        ("{ y: 1, a: { ...{ j: 5 }, b: ^.y } }.a.b", "1"),
        ("{ ...{ j: 5 }, a: { b: ^.j } }.a.b", "5"),
        ("{ p: { ~s: 1, a: 2 }, q: p & { peek2: ~s }, r: q.peek2 }.r", "_"),
    ];
    table(&mut bad, &cells);
    let w = Ws::new("g1-root");
    let src = "p: 1\ns: { t: p }\nr: s.t\n@NonZero: @int & !0\n/safe_div: a -> { @{ @NonZero }: a / $ }\nx: /safe_div 6 0\ny: /safe_div 6 3\n";
    check(&mut bad, "root: commit", w.writes(&[src]), "e0 c0");
    check(&mut bad, "root: _.r", w.eval("_.r"), "1");
    check(&mut bad, "root: _.x", bottom(&w.eval("_.x")), "_|_ #no_matching_branch");
    check(&mut bad, "root: _.y", w.eval("_.y"), "2");
    verdict(bad, cells.len() + 4);
}

/// g2 — Roots that read nothing differently do not move (v0.102.0).
#[test]
fn g2_roots_that_read_the_same_do_not_move() {
    let mut bad = Vec::new();
    for (src, root) in [
        ("x: 0", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        ("v: 1 + 1", "df91a8ad239ea8727c3a03a71eb4b0a787312b92306db4d8b124a73faaf40199"),
        ("f: (x -> x + 1)", "258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f"),
        ("m: (_ -> 1)", "cf11d2ff2e95c11ecbae5ceba00ece7c60f87c7d81149287ffebce1b551a89a6"),
        ("k: { a: 1, _: 9 }", "420a0c9f4592b49ace8c8aacec690fd0a70953b1bd2158260e68c3acce710d9e"),
        ("t: { @{ @int }: 0, x: 3 }", "5d0e29990a9573057a9f821a2b2c2af218883912ffb85cf29d54983802ac3a7e"),
        ("p: 1\ns: { t: p }\nr: s.t", "31462702b5f1132753a9bfb2db47ce51bed0add691f668cc806e55c378926fe5"),
        ("p: 1\nf: (x -> p)\nq: { k: 2, s: k + p }", "89b0945cd406f1fafd4e2f7a00ab1709562d4da5a4413b7f9cfa3ebe35a851c2"),
        ("@Pos: 1..\n/f: a -> { @{ @Pos }: \"y\", _: \"n\" }", "2979f9d8c94c692324120156cff0dce3509bad75c48b75a17f11a09fbc70d52d"),
    ] {
        check(&mut bad, src, root_of(src), root);
    }
    verdict(bad, 9);
}
