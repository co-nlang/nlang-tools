// A reference held in a field.
// Order: nlang-tools/docs/a_reference_held_in_a_field_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-090.  No ruling needed:
// SYNTAX_07 §2 #4 (looking through a live reference sees what the coordinate
// holds when it is observed) and its precedent (2026-10-15, Q-089: a
// reference does not carry a frame; it is read where it is observed);
// SPEC_07 §4.2 P3 (an open term is bound by the nearest enclosing evolution
// at observation); navigation is compositional, `x.a.b ≡ (x.a).b`.
//
// ── What is wrong (v0.91.0; v0.89.0 and v0.90.0 the same) ───────────────
//
// Navigating through a reference that is held in a field — not one that is
// a top-level coordinate — answers `_`. Forcing the field yields the
// reference; navigation does not read it, and falls into the non-navigable
// arm (⊤ #no_coordinate):
//   t: { a: 1 }, c: { r: <<t>> }:  c.r ⟹ { a: 1 };  c.r.a ⟹ _
//   c.r = t ⟹ #true;  c.r.a = 1 ⟹ #false
//   { inner: <<t>> }.inner.a ⟹ _
// A top-level reference (`r: <<t>>`, `r.a` ⟹ 1) and one returned by an
// arrow (`((y -> <<t>>) 0).a` ⟹ 1) are read. Conformance L2-129..131 are
// the same cells on the runner's surface.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("held-ref-{tag}"));
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

/// Commit `src`, then evaluate each `(expr, want)`.
fn after_commit(tag: &str, src: &str, cases: &[(&str, &str)]) -> Vec<String> {
    let w = Ws::new(tag);
    w.commit(&[src]);
    let mut bad = Vec::new();
    for (e, want) in cases {
        let got = w.eval(e);
        if got != *want {
            bad.push(format!("  [{src}] {e}\n    want {want}\n    got  {got}"));
        }
    }
    bad
}

// ── Red ──────────────────────────────────────────────────────────────────

const NO_CONTEXT: &str = "_|_ ;; %cause: #no_context";

fn observe(w: &Ws, src: &str, path: &str) -> String {
    fs::write(w.ws.join("p.n"), src).unwrap();
    squash(&w.oo(&["run", "--observe", path, "p.n"]).0)
}

/// A reference held in a field reads the coordinate, on every surface.
/// v0.91.0: each of these is `_` (the equalities `#false`). Controls: a
/// top-level reference and one returned by an arrow are read.
#[test]
fn r1_a_held_reference_reads_the_coordinate() {
    let src = "t: { a: 1 }\nc: { r: <<t>> }\nrr: <<c>>\nh: (y -> <<t>>)\nd: { r: <<_.>> }\n";
    let mut bad = after_commit(
        "r1",
        src,
        &[
            ("c.r.a", "1"),
            ("_.c.r.a", "1"),
            ("(c.r).a", "1"),
            ("c.r.a = 1", "#true"),
            ("{ inner: <<t>> }.inner.a", "1"),
            ("rr.r.a", "1"),
            ("d.r.t.a", "1"),
            ("d.r.c.r.a", "1"),
            // controls
            ("c.r", "{ a: 1 }"),
            ("(_.h 0).a", "1"),
            ("_.t.a", "1"),
        ],
    );
    let w = Ws::new("r1-run");
    let got = observe(&w, src, "c.r.a");
    if got != "1" {
        bad.push(format!("  run --observe c.r.a\n    want 1\n    got  {got}"));
    }
    assert!(bad.is_empty(), "{} wrong:\n{}", bad.len(), bad.join("\n"));
}

/// A held reference to an open term is read where it is observed (SYNTAX_07
/// §2 #4 precedent): inside a pipe's transformer the read takes the pipe's
/// `$`; carried out of the frame it has none; and the read itself binds
/// no `$`. v0.91.0: `_` in all four red cells. Control: the in-place `_.t` in the same positions.
#[test]
fn r2_a_held_reference_to_an_open_term_is_read_where_observed() {
    let w = Ws::new("r2");
    w.commit(&["s: \"Logic\" |> { a: $ }\nw: { x: $.s }\nt: { a: $.b + 1 }\nc: { r: <<t>> }\nd: { r: <<_.>> }\n"]);
    let mut bad = Vec::new();
    for (e, want) in [
        ("{ b: 1 } |> { z: c.r.a }", "{ b: 1 z: 2 }"),
        ("{ b: 1 } |> { z: _.t.a }", "{ b: 1 z: 2 }"),
        ("({ b: 1 } |> { inner: _.t }).inner.a", "2"),
    ] {
        let got = w.eval(e);
        if got != want {
            bad.push(format!("  {e}\n    want {want}\n    got  {got}"));
        }
    }
    // Reading through the held reference binds no `$` (Q-089 I1): `w` is
    // open and nothing pipes it.
    for e in ["c.r.a", "({ b: 1 } |> { inner: <<t>> }).inner.a", "d.r.w.x"] {
        let got = w.eval(e);
        if !got.starts_with(NO_CONTEXT) {
            bad.push(format!("  {e}\n    want {NO_CONTEXT} …\n    got  {got}"));
        }
    }
    assert!(bad.is_empty(), "{} wrong:\n{}", bad.len(), bad.join("\n"));
}

// ── Added by the acceptor in R-1 (2026-10-15) ───────────────────────────
//
// The first round (c2c4ac9, the same shape as the reference 44a2a65) reads
// a held reference only when a later path segment navigates through it. A
// path that stops on the held reference hands its user the unread
// reference: `c.r` prints 1 and `c.r + 1` is ⊥ #conflict. The delivery
// measured the arithmetic and pipe cells (8.1) and left them because the
// reference had; the probe had no cell. The acceptor's.

/// A held reference is a value to whoever uses it, exactly as a top-level
/// reference is (`r: <<n>>`, `r + 1` ⟹ 2), including a computed one (an
/// arrow returning it, a branch picking it). v0.91.0 and the first round:
/// every red operand cell ⊥ #conflict; `run --observe c.r` collapsed. Controls: the top-level reference and a
/// field holding a copy (`u: { r: t }`).
#[test]
fn r3_a_held_reference_is_a_value_to_its_user() {
    let src = "n: 1\nr: <<n>>\nc: { r: <<n>> }\nt: { a: 1 }\nrt: <<t>>\nct: { r: <<t>> }\nu: { r: t }\ng: (y -> <<n>>)\nc2: { r: _.g 0 }\nc3: { r: #true ? <<n>> : 0 }\n";
    let mut bad = after_commit(
        "r3",
        src,
        &[
            ("c.r + 1", "2"),
            ("_.c.r + 1", "2"),
            ("~%Math./add (c.r, 1)", "2"),
            ("ct.r & { b: 2 }", "{ a: 1 b: 2 }"),
            ("ct.r |> { b: $.a }", "{ a: 1 b: 1 }"),
            ("c2.r + 1", "2"),
            ("c3.r + 1", "2"),
            // controls
            ("r + 1", "2"),
            ("rt & { b: 2 }", "{ a: 1 b: 2 }"),
            ("u.r & { b: 2 }", "{ a: 1 b: 2 }"),
            ("c.r", "1"),
        ],
    );
    let w = Ws::new("r3-run");
    fs::write(w.ws.join("q.n"), format!("{src}out: c.r + 1\n")).unwrap();
    let got = squash(&w.oo(&["run", "--observe", "out", "q.n"]).0);
    if got != "2" {
        bad.push(format!("  run --observe out (out: c.r + 1)\n    want 2\n    got  {got}"));
    }
    // Observing the path that stops on the held reference is a reference-
    // mediated observation on every surface (SYNTAX_07 §4 #6), as `oo eval
    // 'c.r'` already is. v0.91.0: `run --observe c.r` prints the collapsed
    // "Logic" while `oo eval 'c.r'` prints the node.
    fs::write(w.ws.join("s.n"), "s: \"Logic\" |> { a: $ }\nc: { r: <<s>> }\n").unwrap();
    let got = squash(&w.oo(&["run", "--observe", "c.r", "s.n"]).0);
    let want = "{ %val: \"Logic\" a: \"Logic\" }";
    if got != want {
        bad.push(format!("  run --observe c.r\n    want {want}\n    got  {got}"));
    }
    assert!(bad.is_empty(), "{} wrong:\n{}", bad.len(), bad.join("\n"));
}

// ── Green fence (v0.91.0 and the reference alike) ───────────────────────

/// What is NOT changed: the reference itself, observed at the end of a
/// path, keeps its structural view (SYNTAX_07 §4 #6); the record that holds
/// prints as the reference's content.
#[test]
fn g1_the_held_reference_itself_is_unchanged() {
    let src = "s: \"Logic\" |> { a: $ }\nw: { x: $.s }\nc: { r: <<s>> }\nd: { r: <<_.>> }\nk: c.r\n";
    let w = Ws::new("g1");
    w.commit(&[src]);
    let mut bad = Vec::new();
    for (e, want) in [
        ("c.r", "{ %val: \"Logic\" a: \"Logic\" }"),
        ("_.c.r", "{ %val: \"Logic\" a: \"Logic\" }"),
        ("<<c>>.r", "{ %val: \"Logic\" a: \"Logic\" }"),
        ("c", "{ r: \"Logic\" }"),
        // a definition whose value is the held reference is that reference
        ("k", "{ %val: \"Logic\" a: \"Logic\" }"),
    ] {
        let got = w.eval(e);
        if got != want {
            bad.push(format!("  {e}\n    want {want}\n    got  {got}"));
        }
    }
    assert!(bad.is_empty(), "{} wrong:\n{}", bad.len(), bad.join("\n"));
}
