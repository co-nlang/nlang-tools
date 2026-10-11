// A private field that was never committed.
// Order: nlang-tools/docs/a_private_never_committed_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-079.  Ruling: meta/oo/STATUS.md D103.
// Spec: SPEC_04 §3.1 (#1 inward visibility, #4 identity keeps the local axis,
// #5 bare name = insider, `.~key` = outside), REAL_03 §6 (~ axes are axes),
// SPEC_03 §3.1 (a spread source resolves in its container; §1 example).
//
// ── What is wrong (v0.80.0; v0.72.0 … v0.79.0 the same) ──────────────────
//
// A `~` field written at the top never reaches the universe:
//   * a file of only `~h: 3` builds no universe at all;
//   * beside other fields, the working-set member omits it, the committed
//     root lacks it, a later evolve reads `_`;
//   * `~h: 3; k: 1`, `~h: 4; k: 1` and `k: 1` commit to ONE root;
//   * a child, a deeper descendant, or a morphism that reads `h` answers `_`
//     after commit (the same file under `run --observe` answers 3).
// And a field written at the top whose value spreads a name defined at the
// top (`r: { a: 1, ...c }`) loses that spread across evolve → commit:
// `_.r` is `{ a: 1 }` — SPEC_03 §1's own `result_spread` example.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("private-never-committed-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo`, killing it after 60 s: a cycle must answer, not hang.
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
    fn commit(&self, src: &str) -> (String, i32) {
        fs::write(self.ws.join("a.n"), src).unwrap();
        let (o, rc) = self.oo(&["evolve", "a.n"]);
        if rc != 0 {
            return (o, rc);
        }
        self.oo(&["commit", "-m", "a"])
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
}

fn is(o: &str, want: &str) -> bool {
    o == want
}

fn has(o: &str, want: &str) -> bool {
    !o.starts_with("_|_") && o.contains(want)
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
            wrong.push(format!("`{e}` => {o}   (wanted {want})"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// Each `(source, expression, want)` in a fresh universe: commit, then read.
fn committed(cases: &[(&str, &str, &str)]) -> Vec<String> {
    let mut wrong = Vec::new();
    for (i, (src, e, want)) in cases.iter().enumerate() {
        let w = Ws::new(&format!("c{i}"));
        let (o, rc) = w.commit(src);
        if rc != 0 {
            wrong.push(format!("commit of `{}` failed: {o}", src.replace('\n', "; ")));
            continue;
        }
        let o = w.eval(e);
        if o != *want {
            wrong.push(format!("after `{}`: `{e}` => {o}   (wanted {want})", src.replace('\n', "; ")));
        }
    }
    wrong
}

fn root_of(tag: &str, src: &str) -> Result<String, String> {
    let w = Ws::new(tag);
    let (o, rc) = w.commit(src);
    if rc != 0 {
        return Err(format!("commit of `{}` failed: {o}", src.replace('\n', "; ")));
    }
    Ok(w.root())
}

// ── Red on v0.80.0 ────────────────────────────────────────────────────────

/// A `~` field at the top is a field of the root: a file of only that field
/// is a proposal, and the field is there for the next evolve.
#[test]
fn r1_a_root_private_reaches_the_universe() {
    let mut wrong = Vec::new();
    let w = Ws::new("r1a");
    let (o, rc) = w.commit("~h: 3\n");
    if rc != 0 || !w.ws.join(".oo").exists() {
        wrong.push(format!("commit of `~h: 3` => rc={rc} {o}   (wanted a universe and a commit)"));
    }
    let w = Ws::new("r1b");
    let _ = w.commit("~h: 3\nk: 1\n");
    let _ = w.commit("k2: h + 1\n");
    let o = w.eval("_.k2");
    if o != "4" {
        wrong.push(format!("after `~h: 3; k: 1` then `k2: h + 1`: `_.k2` => {o}   (wanted 4)"));
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// SPEC_04 §3.1 #1: a root `~` field is visible to every descendant and to
/// every morphism defined inside — after commit as before it.
#[test]
fn r2_committed_descendants_read_a_root_private() {
    let wrong = committed(&[
        ("~h: 3\nt: { k: h }\n", "_.t.k", "3"),
        ("~h: 3\nt: { u: { k: h } }\n", "_.t.u.k", "3"),
        ("~h: 3\nt: { k: h + 1 }\n", "_.t.k", "4"),
        ("~h: 3\nf: (x -> x + h)\n", "_.f 1", "4"),
        ("~h: 3\nt: { g: (x -> x + h) }\n", "_.t.g 1", "4"),
        ("~c: { b: 2 }\nr: { q: c.b }\n", "_.r.q", "2"),
    ]);
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// SPEC_04 §3.1 #4, REAL_03 §6: identity keeps the local axis. Universes that
/// differ only in a root `~` field commit to different roots.
#[test]
fn r3_a_root_private_is_part_of_the_root() {
    let roots: Vec<(String, Result<String, String>)> = ["~h: 3\nk: 1\n", "~h: 4\nk: 1\n", "k: 1\n"]
        .iter()
        .enumerate()
        .map(|(i, s)| (s.replace('\n', "; "), root_of(&format!("r3{i}"), s)))
        .collect();
    let mut wrong = Vec::new();
    for i in 0..roots.len() {
        for j in i + 1..roots.len() {
            match (&roots[i].1, &roots[j].1) {
                (Ok(a), Ok(b)) if a != b && !a.is_empty() => {}
                (a, b) => wrong.push(format!("`{}` {:?} vs `{}` {:?}   (wanted two roots)", roots[i].0, a, roots[j].0, b)),
            }
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// D103: a one-shot evaluation in a universe stands inside its root (the
/// root is the working directory), so a bare name finds a root `~` field.
#[test]
fn r4_a_one_shot_evaluation_is_inside_the_root() {
    let w = Ws::new("r4");
    let _ = w.commit("~h: 3\nk: 1\n");
    let mut wrong = Vec::new();
    for (e, want) in [("h", "3"), ("h + k", "4"), ("{ q: h }.q", "3")] {
        let o = w.eval(e);
        if o != want {
            wrong.push(format!("`{e}` => {o}   (wanted {want})"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// A root `~` field evolves monotonically like any root field: a different
/// value is refused, the same value lands.
#[test]
fn r5_a_root_private_collides_like_a_field() {
    let mut wrong = Vec::new();
    let w = Ws::new("r5");
    let _ = w.commit("~h: 3\n");
    fs::write(w.ws.join("b.n"), "~h: 4\n").unwrap();
    let (o, rc) = w.oo(&["evolve", "b.n"]);
    if rc == 0 || !o.contains("#conflict") {
        wrong.push(format!("after `~h: 3`, evolve `~h: 4` => rc={rc} {o}   (wanted a refusal naming #conflict)"));
    }
    let o = w.eval("h");
    if o != "3" {
        wrong.push(format!("`h` => {o}   (wanted 3)"));
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// SPEC_03 §3.1: a spread source resolves in its container, wherever its
/// definition is written — across evolve and commit too. SPEC_03 §1's own
/// example included.
#[test]
fn r6_a_committed_field_spreads_a_root_name() {
    let mut wrong = committed(&[
        ("c: { b: 2 }\nr: { a: 1, ...c }\n", "_.r.b", "2"),
        ("r: { a: 1, ...c }\nc: { b: 2 }\n", "_.r.b", "2"),
        ("c: { b: 2 }\nr: { ...c }\n", "_.r.b", "2"),
        ("~c: {{ b: 2 }}\n~o: { a: 1 }\nresult_spread: { a: 1, ...~c }\n", "_.result_spread.b", "2"),
    ]);
    // A collision through such a spread is a bottom field, whichever line
    // comes first — as `{ a: 1, ...{ a: 2 } }` is.
    wrong.extend(committed(&[
        ("c: { a: 2 }\nr: { a: 1, ...c }\n", "_.r.a", "_|_  ;; %cause: #conflict"),
        ("r: { a: 1, ...c }\nc: { a: 2 }\n", "_.r.a", "_|_  ;; %cause: #conflict"),
    ]));
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

// ── Guards: green on v0.80.0, must stay green ────────────────────────────

/// SPEC_04 §3.1 #3–#5: a path that descends to `~h` from outside is a
/// violation, and the displayed root shows no local axis. (Mutation: let a
/// `.~key` segment read the field ⟹ red.)
#[test]
fn g1_outside_stays_outside() {
    let w = Ws::new("g1");
    let _ = w.commit("~h: 3\nk: 1\n");
    let mut wrong = Vec::new();
    let o = w.eval("_.~h");
    if !bot(&o, "#private_access_violation") {
        wrong.push(format!("`_.~h` => {o}   (wanted #private_access_violation)"));
    }
    let o = w.eval("_.");
    if o.contains("h:") || !o.contains("k: 1") {
        wrong.push(format!("`_.` printed:\n{o}\n(wanted k: 1 and no local field)"));
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// Roots without a root `~` field keep their address — a `~` field inside a
/// Combo always reached history. (Mutation: drop the local axis of every
/// Combo at commit ⟹ red.)
#[test]
fn g2_other_roots_do_not_move() {
    let mut wrong = Vec::new();
    for (i, (src, want)) in [
        ("k: 1\n", "ced4d79f3c4ae4d6cbf797089541b9b751e0fd6f6d61286624dde71be96a47d9"),
        ("t: { ~h: 3, k: 1 }\n", "05dc9846f0de0fa238ae44c23d1cc8c4545d275aaa2583afa1b12dc9d6dba051"),
        ("t: { ~h: 3, u: { k: h } }\n", "9c306206734fb59b1cbb3a560c2314618feafe7201fbc4273aac34d6069a38ab"),
        ("c: { b: 2 }\nr: { q: c.b }\n", "94d96d72d96b6c944f6bce50156fa5941f5b0feda16b283a8df29523b8f28cbf"),
    ]
    .iter()
    .enumerate()
    {
        match root_of(&format!("g2{i}"), src) {
            Ok(r) if r == *want => {}
            other => wrong.push(format!("`{}` => {:?}   (wanted {want})", src.replace('\n', "; "), other)),
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// SPEC_03 §3.1 spread isolation: a spread written outside its source does
/// not carry the source's `~` fields. (Mutation: carry them ⟹ red.)
#[test]
fn g3_a_spread_does_not_leak_privates() {
    let wrong = committed(&[("...{ ~h: 3 }\nt: { k: h }\n", "_.t.k", "_")]);
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// In one process nothing changes: `run --observe` reads the root `~` field
/// from inside, by either spelling; a never-defined spread source is still a
/// no-op. (Mutation: refuse a never-defined source ⟹ red.)
#[test]
fn g4_in_process_and_no_ops() {
    let w = Ws::new("g4");
    fs::write(w.ws.join("a.n"), "~h: 3\nt: { k: h }\n").unwrap();
    let mut wrong = Vec::new();
    for (p, want) in [("t.k", "3"), ("h", "3"), ("~h", "3")] {
        let (o, _) = w.oo(&["run", "--observe", p, "a.n"]);
        if o != want {
            wrong.push(format!("`run --observe {p}` => {o}   (wanted {want})"));
        }
    }
    wrong.extend(committed(&[("r: { a: 1, ...nothing }\n", "_.r.a", "1")]));
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}
