// A reference that bound what it read.
// Order: nlang-tools/docs/a_reference_that_bound_what_it_read_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-089.  No ruling needed:
// SYNTAX_07 §2 #4 (a path in `<<…>>` is a live reference: what it reads is
// bound when it is observed — the coordinate's content, nothing else; its
// identity is the path's syntax), SPEC_07 §4.2 P1 (`$` rebinds only at an
// evolution: `|>` and application), P3 (an open term observed is bound by
// the nearest enclosing evolution; with none, ⊥ #no_context), §4.2.2
// ("whoever pipes an open term binds its `$`"; `v.w.x.a` is "Logic").
//
// ── What is wrong (v0.90.0) ─────────────────────────────────────────────
//
// The engine binds `$` when it dereferences a live reference (code comment
// "F1"), to the value it read. The canonical `v: <<_.>> |> <<_.>>` is
// stored as the bare reference — the pipe is gone — and answers "Logic"
// only on the surfaces that dereference with F1:
//   run --observe v.w.x.a  ⟹ "Logic";  oo eval '_.v.w.x.a' ⟹ ⊥ #no_context
//   oo eval '(<<_.>> |> <<_.>>).w.x.a' ⟹ ⊥ #no_context
// A pipe through a reference does not bind its input:
//   { b: 1 } |> <<t>> ⟹ { a: ⊥, b: 1 };  { b: 1 } |> <<_.t>> ⟹ ⊥ #conflict
// A reference with no pipe binds anyway (v0.89.0: ⊥; v0.90.0, since open
// terms are stored as syntax): `r: <<_.>>`, run --observe r.w.x ⟹ "Logic".
// And `oo eval` writes its question into the universe it asks about:
//   oo eval '<<_.>>' prints a field `__eval_result` nobody defined.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("live-ref-{tag}"));
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

const CANON: &str = "s: \"Logic\" |> { a: $ }\nw: { x: $.s }\nv: <<_.>> |> <<_.>>\nr: <<_.>>\nt: { a: $.b + 1 }\n";

fn observe(w: &Ws, path: &str) -> String {
    fs::write(w.ws.join("empty.n"), "").unwrap();
    squash(&w.oo(&["run", "--observe", path, "empty.n"]).0)
}

/// `oo eval` asks about the universe; the question is not in it. v0.90.0:
/// over `x: 1`, `<<_.>>.__eval_result` is ⊥ #divergent — a field nobody
/// defined, holding the question, which reads itself; printing `<<_.>>`
/// nests it until the horizon. Wanted: `_` (no such coordinate). Control:
/// `<<_.>>.x` is 1.
#[test]
fn r1_the_question_is_not_in_the_universe_it_asks_about() {
    let w = Ws::new("r1");
    w.commit(&["x: 1\n"]);
    let mut bad = Vec::new();
    for (e, want) in [("<<_.>>.__eval_result", "_"), ("<<_.>>.x", "1")] {
        let got = w.eval(e);
        if got != want {
            bad.push(format!("  {e}\n    want {want}\n    got  {got}"));
        }
    }
    let w = Ws::new("r1c");
    w.commit(&[CANON]);
    // Control, right on v0.90.0.
    let got = w.eval("_.r.s");
    if got != "\"Logic\"" {
        bad.push(format!("  _.r.s: {got}"));
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

/// SPEC_07 §4.2.2's own vector, on every surface. v0.90.0: right through
/// `run --observe` and a bare path in `oo eval`; ⊥ through `_.` and through
/// the pipe written in place.
#[test]
fn r2_the_canonical_example_on_every_surface() {
    let w = Ws::new("r2");
    w.commit(&[CANON]);
    let mut bad = Vec::new();
    for e in ["_.v.w.x.a", "v.w.x.a", "(<<_.>> |> <<_.>>).w.x.a"] {
        let got = w.eval(e);
        if got != "\"Logic\"" {
            bad.push(format!("  oo eval {e}: {got}"));
        }
    }
    let got = observe(&w, "v.w.x.a");
    if got != "\"Logic\"" {
        bad.push(format!("  run --observe v.w.x.a: {got}"));
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

/// P1 through a reference: whoever pipes an open term binds its `$`.
/// Control, right on v0.90.0: the same template read by path.
#[test]
fn r3_a_pipe_through_a_reference_binds_its_input() {
    let w = Ws::new("r3");
    w.commit(&[CANON]);
    let mut bad = Vec::new();
    for e in ["{ b: 1 } |> <<t>>", "{ b: 1 } |> <<_.t>>", "{ b: 1 } |> _.t"] {
        let got = w.eval(e);
        if got != "{ a: 2 b: 1 }" {
            bad.push(format!("  {e}: {got}"));
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

/// The other half: reading through a reference binds no `$`. `r: <<_.>>`
/// is the root, and `r.w.x` is `w.x` — observed with no evolution, ⊥
/// #no_context (P3; SPEC_07 §4.2.2 `_: w.x`). v0.90.0: "Logic" through
/// `run --observe` and a bare path (v0.89.0: ⊥). The cause is compared.
/// Controls, right on v0.90.0: `_.r.w.x`; a closed field through the
/// reference; a definition committed later is seen (the reference is live).
#[test]
fn r4_reading_through_a_reference_binds_nothing() {
    let w = Ws::new("r4");
    w.commit(&[CANON]);
    let mut bad = Vec::new();
    for (how, got) in [
        ("oo eval _.r.w.x", w.eval("_.r.w.x")),
        ("oo eval r.w.x", w.eval("r.w.x")),
        ("run --observe r.w.x", observe(&w, "r.w.x")),
    ] {
        if !got.starts_with("_|_ ;; %cause: #no_context") {
            bad.push(format!("  {how}: {got}"));
        }
    }
    let got = w.eval("r.s");
    if got != "\"Logic\"" {
        bad.push(format!("  r.s: {got}"));
    }
    fs::write(w.ws.join("later.n"), "z: 5\n").unwrap();
    for args in [&["evolve", "later.n"][..], &["commit", "-m", "b"][..]] {
        let (o, rc) = w.oo(args);
        assert_eq!(rc, 0, "{args:?}: {o}");
    }
    for e in ["_.r.z", "r.z"] {
        let got = w.eval(e);
        if got != "5" {
            bad.push(format!("  {e} after a later commit: {got}"));
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

/// A stored open term read through a reference is its definition, read
/// there (SPEC_07 §4.2.1 P3 判例): `k: <<_.m>>` with `m` an open term. And
/// printing the reference whole binds no `$` either. v0.90.0: `_.k.w` is
/// `_` (v0.89.0: `{ x: ⊥ }` — the stored syntax is not read through the
/// reference), and `_.k` prints `w: { x: "Logic" … }` (the dereference bound
/// `$` to `m`). Controls, right on v0.90.0: `_.m.w.x`; a reference observed
/// whole is the structural view (`s` prints its `%val`, SYNTAX_07 §4 #6).
#[test]
fn r5_an_open_term_read_through_a_reference() {
    let w = Ws::new("r5");
    w.commit(&["m: { s: \"Logic\" |> { a: $ }, w: { x: $.s } }\nk: <<_.m>>\n"]);
    let mut bad = Vec::new();
    for (e, want) in [
        ("_.k.w", "{ x: _|_ ;; %cause: #no_context"),
        ("_.k.w.x", "_|_ ;; %cause: #no_context"),
        // Control.
        ("_.m.w.x", "_|_ ;; %cause: #no_context"),
    ] {
        let got = w.eval(e);
        if !got.starts_with(want) {
            bad.push(format!("  {e}\n    want {want}…\n    got  {got}"));
        }
    }
    let got = w.eval("_.k");
    if !got.contains("w: { x: _|_ ;; %cause: #no_context") {
        bad.push(format!("  _.k printed whole: {got}"));
    }
    // Control.
    if !got.contains("s: { %val: \"Logic\" a: \"Logic\" }") {
        bad.push(format!("  _.k is not the structural view: {got}"));
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

// ── Guard (green on v0.90.0) ─────────────────────────────────────────────

/// A bare reference keeps its root; values with no reference keep theirs.
#[test]
fn g1_a_bare_reference_keeps_its_root() {
    for (src, root) in [
        ("r: <<_.>>\n", "a92a04780fd6bd7e3227a1b2b8fecff8debef28f1703290b21a8c5b748854071"),
        ("x: 0\n", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        ("t: { a: $.b + 1 }\n", "3d28862809c08c1356d8ed093871cc8ea270988a4c44fccd3ec20d700044d720"),
    ] {
        let w = Ws::new("g1");
        w.commit(&[src]);
        assert_eq!(w.root(), root, "{src}");
    }
}
