// A recipe that met a write.
// Order: nlang-tools/docs/a_recipe_that_met_a_write_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-092.  D110 (2026-10-16): a pure
// definition whose answer read the universe keeps its recipe, and reading it
// reads the universe again. A coordinate written twice is the meet of both
// writes; when one of them is a recipe, the meet read the universe too.
// D111 (2026-10-18) rules the root-level spread; that is Q-093, not here.
//
// ── What is wrong (v0.93.0) ─────────────────────────────────────────────
//
// The universe's own merges (evolve, the working set, commit, observation)
// force a recipe where the universe is not visible. It reads as `_`, and
// `_ & { z: 9 }` is `{ z: 9 }`: the recipe's fields are gone, silently.
//   c: { b: 2 }, r: c, r.z: 9 ⟹ _.r = { z: 9 }   (v0.92.0 { b: 2 z: 9 })
// Same for an application, `c & { a: 1 }`, an open term through a name, a
// private name, a live reference, a field of a record — in one file, over
// two evolves, over two commits, in either order. Written as one
// definition, `r: c & { z: 9 }` is right and stays a reading.
// A spread written in a coordinate's record reads its source only when it
// lands, after evolve decided the record read nothing; the record keeps the
// answer of that moment:
//   c: { b: @int }, r: { a: 1, ...c }; then c: { b: 5 } ⟹ _.r.b = @int
// A spread one level down (`r: { x: { ...c } }`) is right on every face.
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
    /// Evolve `src` alone: (rc, output).
    fn evolve(&self, src: &str) -> (i32, String) {
        fs::write(self.ws.join("e.n"), src).unwrap();
        let (o, rc) = self.oo(&["evolve", "e.n"]);
        (rc, o)
    }

    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("met-a-write-{tag}"));
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

    /// `oo eval`, the trailing effect note dropped, whitespace collapsed.
    fn eval(&self, e: &str) -> String {
        let s = self.oo(&["eval", e]).0;
        let s = match s.find(";; %effect") {
            Some(i) => s[..i].trim().to_string(),
            None => s,
        };
        squash(&s)
    }

    /// Evolve `src`, then commit. Both must succeed: a reading taken after a
    /// failed commit is a reading of the previous universe.
    fn commit(&self, src: &str) {
        let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
        let f = format!("f{n}.n");
        fs::write(self.ws.join(&f), src).unwrap();
        let (o, rc) = self.oo(&["evolve", &f]);
        assert_eq!(rc, 0, "evolve {src:?}: {o}");
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        assert_eq!(rc, 0, "commit {src:?}: {o}");
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

    /// `run --observe path` of a program, whitespace collapsed.
    fn observe(&self, src: &str, path: &str) -> String {
        fs::write(self.ws.join("p.n"), src).unwrap();
        squash(&self.oo(&["run", "--observe", path, "p.n"]).0)
    }

    /// Observation savepoints in this universe.
    fn savepoints(&self) -> usize {
        fs::read_dir(self.ws.join(".oo/savepoints"))
            .map(|d| {
                d.flatten()
                    .filter(|e| {
                        let n = e.file_name().to_string_lossy().to_string();
                        e.path().is_file() && n != "LOG" && !n.starts_with('.')
                    })
                    .count()
            })
            .unwrap_or(0)
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


fn first_commit_root(src: &str, tag: &str) -> String {
    let w = Ws::new(tag);
    w.commit(src);
    w.root()
}

// ── Red ──────────────────────────────────────────────────────────────────

/// r1 — A recipe met by a record write keeps its fields. One program, one
/// commit. v0.93.0: every row but the control loses the recipe's side
/// (`{ z: 9 }`, `{ a: 1 z: 9 }`, the open term's `a` gone); v0.92.0 had all
/// but the live reference and the record field right.
#[test]
fn r1_a_recipe_met_by_a_record_keeps_its_fields() {
    let mut bad = Vec::new();
    let cases: [(&str, &str, &str); 9] = [
        ("c: { b: 2 }\nr: c\nr.z: 9\n", "_.r", "{ b: 2 z: 9 }"),
        ("c: { b: 2 }\nr: { z: 9 }\nr: c\n", "_.r", "{ b: 2 z: 9 }"),
        ("c: { b: 2 }\nr: c & { a: 1 }\nr: { z: 9 }\n", "_.r", "{ a: 1 b: 2 z: 9 }"),
        ("c: { b: 2 }\nr: (x -> x) c\nr: { z: 9 }\n", "_.r", "{ b: 2 z: 9 }"),
        ("t: { a: $.b + 1 }\nw: t\nw: { z: 9 }\n", "{ b: 1 } |> _.w", "{ a: 2 b: 1 z: 9 }"),
        ("c: { b: 2 }\nr: <<c>>\nr: { z: 9 }\n", "_.r", "{ b: 2 z: 9 }"),
        ("c: { b: 2 }\n~p: c\n~p: { z: 9 }\nq: ~p\n", "_.q", "{ b: 2 z: 9 }"),
        ("c: { b: 2 }\nr: { x: c }\nr: { x: { z: 9 } }\n", "_.r.x", "{ b: 2 z: 9 }"),
        // control: the meet written as one definition
        ("c: { b: 2 }\nr: c & { z: 9 }\n", "_.r", "{ b: 2 z: 9 }"),
    ];
    for (i, (src, e, want)) in cases.iter().enumerate() {
        let w = Ws::new(&format!("r1-{i}"));
        w.commit(src);
        check(&mut bad, &format!("{src:?} {e}"), w.eval(e), want);
    }
    verdict(bad, cases.len());
}

/// r2 — Neither the order of the writes nor how they were split into
/// evolves and commits changes the answer or the root. v0.93.0: `{ z: 9 }`
/// in every row.
#[test]
fn r2_the_order_and_the_sessions_do_not_matter() {
    let mut bad = Vec::new();
    let mut roots = Vec::new();
    // two evolves, one commit, both orders
    for (i, (a, b)) in [("c: { b: 2 }\nr: c\n", "r.z: 9\n"), ("c: { b: 2 }\nr.z: 9\n", "r: c\n")].iter().enumerate() {
        let w = Ws::new(&format!("r2e{i}"));
        let (rc, o) = w.evolve(a);
        assert_eq!(rc, 0, "{o}");
        fs::write(w.ws.join("g.n"), b).unwrap();
        let (o, rc) = w.oo(&["evolve", "g.n"]);
        assert_eq!(rc, 0, "{o}");
        let (o, rc) = w.oo(&["commit", "-m", "a"]);
        assert_eq!(rc, 0, "{o}");
        check(&mut bad, &format!("two evolves {i}: _.r"), w.eval("_.r"), "{ b: 2 z: 9 }");
        roots.push((format!("two evolves {i}"), w.root()));
    }
    // two commits, both orders
    for (i, (a, b)) in [("c: { b: 2 }\nr: c\n", "r.z: 9\n"), ("c: { b: 2 }\nr.z: 9\n", "r: c\n")].iter().enumerate() {
        let w = Ws::new(&format!("r2c{i}"));
        w.commit(a);
        w.commit(b);
        check(&mut bad, &format!("two commits {i}: _.r"), w.eval("_.r"), "{ b: 2 z: 9 }");
    }
    // one file, both orders
    roots.push(("one file c first".into(), first_commit_root("c: { b: 2 }\nr: c\nr.z: 9\n", "r2f0")));
    roots.push(("one file z first".into(), first_commit_root("c: { b: 2 }\nr.z: 9\nr: c\n", "r2f1")));
    let r0 = roots[0].1.clone();
    for (label, r) in &roots {
        check(&mut bad, &format!("root of {label} = root of {}", roots[0].0), r.clone(), &r0);
    }
    verdict(bad, 4 + roots.len());
}

/// r3 — The meet is still a reading: the source grows later and the
/// coordinate follows it, as `r: c & { z: 9 }` does. v0.93.0: `{ z: 9 }`.
#[test]
fn r3_a_recipe_met_by_a_write_is_still_a_reading() {
    let mut bad = Vec::new();
    let w = Ws::new("r3a");
    w.commit("c: { b: 2 }\nr: c\nr.z: 9\n");
    w.commit("c: { d: 3 }\n");
    check(&mut bad, "one commit, then c grows", w.eval("_.r"), "{ b: 2 d: 3 z: 9 }");
    let v = Ws::new("r3b");
    v.commit("c: { b: 2 }\nr: c\n");
    v.commit("r.z: 9\n");
    v.commit("c: { d: 3 }\n");
    check(&mut bad, "three commits", v.eval("_.r"), "{ b: 2 d: 3 z: 9 }");
    // control
    let u = Ws::new("r3c");
    u.commit("c: { b: 2 }\nr: c & { z: 9 }\n");
    u.commit("c: { d: 3 }\n");
    check(&mut bad, "control: written as one meet", u.eval("_.r"), "{ b: 2 d: 3 z: 9 }");
    verdict(bad, 3);
}

/// r4 — A spread written in a coordinate's record reads its source again:
/// narrowed, grown, or written later. v0.93.0: `@int`, `_`, `_`, `_`; the
/// nested controls are right on v0.93.0.
#[test]
fn r4_a_spread_in_a_coordinate_reads_its_source_again() {
    let mut bad = Vec::new();
    for (pos, wr, rd) in [("coordinate", "r: { a: 1, ...c }", "_.r"), ("after", "r: { ...c, a: 1 }", "_.r"), ("nested (control)", "r: { x: { a: 1, ...c } }", "_.r.x")] {
        let w = Ws::new("r4n");
        w.commit(&format!("c: {{ b: @int }}\n{wr}\n"));
        w.commit("c: { b: 5 }\n");
        check(&mut bad, &format!("{pos}: narrowed"), w.eval(&format!("{rd}.b")), "5");
        let w = Ws::new("r4a");
        w.commit(&format!("c: {{ b: 2 }}\n{wr}\n"));
        w.commit("c: { d: 3 }\n");
        check(&mut bad, &format!("{pos}: grown"), w.eval(&format!("{rd}.d")), "3");
        let w = Ws::new("r4l");
        w.commit(&format!("{wr}\n"));
        w.commit("c: { b: 2 }\n");
        check(&mut bad, &format!("{pos}: written later, two commits"), w.eval(&format!("{rd}.b")), "2");
        // two evolves, one commit, both orders: one answer, one root
        let mut roots = Vec::new();
        for (i, (a, b)) in [(wr.to_string(), "c: { b: 2 }".to_string()), ("c: { b: 2 }".to_string(), wr.to_string())].iter().enumerate() {
            let w = Ws::new(&format!("r4e{i}"));
            let (rc, o) = w.evolve(&format!("{a}\n"));
            assert_eq!(rc, 0, "{o}");
            fs::write(w.ws.join("g.n"), format!("{b}\n")).unwrap();
            let (o, rc) = w.oo(&["evolve", "g.n"]);
            assert_eq!(rc, 0, "{o}");
            let (o, rc) = w.oo(&["commit", "-m", "a"]);
            assert_eq!(rc, 0, "{o}");
            check(&mut bad, &format!("{pos}: two evolves {i}"), w.eval(&format!("{rd}.b")), "2");
            roots.push(w.root());
        }
        check(&mut bad, &format!("{pos}: two evolves, root of either order"), roots[1].clone(), &roots[0]);
    }
    verdict(bad, 18);
}

/// r5 — A spread in a coordinate, met by a later write, still reads its
/// source. v0.93.0: `{ a: 1 b: 2 z: 9 }`.
#[test]
fn r5_a_spread_in_a_coordinate_met_by_a_write() {
    let mut bad = Vec::new();
    let w = Ws::new("r5a");
    w.commit("c: { b: 2 }\nr: { a: 1, ...c }\nr: { z: 9 }\n");
    w.commit("c: { d: 3 }\n");
    check(&mut bad, "one file", w.eval("_.r"), "{ a: 1 b: 2 d: 3 z: 9 }");
    let v = Ws::new("r5b");
    v.commit("c: { b: 2 }\nr: { a: 1, ...c }\n");
    v.commit("r: { z: 9 }\nc: { d: 3 }\n");
    check(&mut bad, "two commits", v.eval("_.r"), "{ a: 1 b: 2 d: 3 z: 9 }");
    verdict(bad, 2);
}

/// r6 — A write that disagrees with what a coordinate's spread reads is
/// refused when it is evolved in, as the same write against an alias is
/// (g1). v0.93.0 (and v0.92.0): accepted, and the commit stores
/// `{ a: 1 b: ⊥ #conflict }`.
#[test]
fn r6_a_collision_with_a_spread_is_refused() {
    let w = Ws::new("r6");
    let (rc, o) = w.evolve("c: { b: 2 }\nr: { a: 1, ...c }\nr: { b: 3 }\n");
    assert_ne!(rc, 0, "evolve accepted a write that disagrees with r's spread:\n{o}");
}

/// r7 — A program run over a universe sees the universe's recipe met by
/// the program's write, as the commit would store it. v0.93.0: `{ z: 9 }`.
#[test]
fn r7_a_program_over_a_universe_meets_its_recipe() {
    let w = Ws::new("r7");
    w.commit("c: { b: 2 }\nr: c\n");
    let mut bad = Vec::new();
    check(&mut bad, "run --observe r over r: c", w.observe("r.z: 9\n", "r"), "{ b: 2 z: 9 }");
    verdict(bad, 1);
}

// ── Guards (green on v0.93.0; the delivery must keep them green) ─────────

/// g1 — A write that disagrees with a recipe is still refused when it is
/// evolved in (D110: judged on both answers), here and across a commit.
#[test]
fn g1_a_collision_is_still_refused() {
    let mut bad = Vec::new();
    for (i, src) in ["c: { b: 2 }\nr: c\nr: { b: 3 }\n", "c: { b: 2 }\nr: { b: 3 }\nr: c\n", "a: 1\nb: a + 1\nb: 3\n"].iter().enumerate() {
        let w = Ws::new(&format!("g1-{i}"));
        let (rc, o) = w.evolve(src);
        if rc == 0 {
            bad.push(format!("  {src:?}: evolve accepted it\n    {o}"));
        }
    }
    let w = Ws::new("g1x");
    w.commit("c: { b: 2 }\nr: c\n");
    let (rc, o) = w.evolve("r: { b: 3 }\n");
    if rc == 0 {
        bad.push(format!("  across a commit: evolve accepted r: {{ b: 3 }}\n    {o}"));
    }
    check(&mut bad, "across a commit: _.r", w.eval("_.r"), "{ b: 2 }");
    let v = Ws::new("g1s");
    v.commit("c: { b: 2 }\nr: { a: 1, ...c }\n");
    let (rc, o) = v.evolve("r: { b: 3 }\n");
    if rc == 0 {
        bad.push(format!("  spread, across a commit: evolve accepted r: {{ b: 3 }}\n    {o}"));
    }
    verdict(bad, 6);
}

/// g2 — Definitions that meet no recipe, and the forms D110 already moved,
/// keep their v0.93.0 roots.
#[test]
fn g2_what_meets_no_recipe_keeps_its_root() {
    let mut bad = Vec::new();
    let cases = [
        ("x: 0\n", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        ("a: 1\nb: a\n", "132ced2494ea0551192cab462b1d58afed6d742fccc99e60fa117e2b876ad269"),
        ("n: 1\nr: <<n>>\nq: r + 1\n", "6b232603821c66dd6a06895c48f139b697fb616a96b9d94b1de9316b016295d8"),
        ("t: { a: $.b + 1 }\nw: t\n", "6677b2c7cfc5769d80c6cc004551afa129745f93a3b981b8684cb8ec71692b16"),
        ("s: { j: 5 }\n...s\n", "a77a72fa45067d1670401364424c9b8178ca1793a9b4a2aaf69fb3862f555b8c"),
        ("c: { b: 2 }\nr: { x: { ...c } }\n", "7a0688650d0aa99e87e539e5b1aeb2c28aa375fc1c16daf22aa3b92955202f7f"),
        ("r: { a: 1, ...{ j: 5 } }\n", "862fc7a498b63dacaea04be09dea0b290817aa6c33e7a51ddf0933436fe749f8"),
        ("r: { b: 2 }\nr: { z: 9 }\n", "14c2852d5d03ffb5d807dbae8b78011bce511ee6c38332e7818508c6ab4b7e61"),
        ("c: { b: 2 }\nr: c\nr: c\n", "282e6c79fc12ff4732e2bfdb720aa6713f3ca206966dbbd275db0e8ba598d6b7"),
        ("v: <<_.>> |> <<_.>>\n", "5ce55aa56a6260f0a38232802ce05cd798e7ec8b250c873b7d15c2b22022d168"),
    ];
    for (i, (src, want)) in cases.iter().enumerate() {
        check(&mut bad, &format!("{src:?}"), first_commit_root(src, &format!("g2-{i}")), want);
    }
    verdict(bad, cases.len());
}

/// g3 — Writing again what a coordinate already holds moves nothing: the
/// same write in a later commit leaves the root where it was.
#[test]
fn g3_a_write_that_adds_nothing_does_not_move_the_root() {
    let mut bad = Vec::new();
    for (i, (first, again)) in [
        ("c: { b: 2 }\nr: c\nr.z: 9\n", "r.z: 9\n"),
        ("c: { b: 2 }\nr: c\nr.z: 9\n", "r: c\n"),
        ("c: { b: 2 }\nr: c\n", "r: c\n"),
        ("r: { b: 2 }\n", "r: { b: 2 }\n"),
    ]
    .iter()
    .enumerate()
    {
        let w = Ws::new(&format!("g3-{i}"));
        w.commit(first);
        let before = w.root();
        let a = w.eval("_.r");
        fs::write(w.ws.join("g.n"), again).unwrap();
        let (o, rc) = w.oo(&["evolve", "g.n"]);
        assert_eq!(rc, 0, "{o}");
        let _ = w.oo(&["commit", "-m", "again"]);
        check(&mut bad, &format!("{first:?} then {again:?}: root"), w.root(), &before);
        check(&mut bad, &format!("{first:?} then {again:?}: _.r"), w.eval("_.r"), &a);
    }
    verdict(bad, 8);
}
