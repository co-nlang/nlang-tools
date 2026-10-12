// A table that lost its patterns.
// Order: nlang-tools/docs/a_table_that_lost_its_patterns_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-082.  No new ruling: D95/D96/D101.
// Spec: SPEC_07 §1.1 (a table dispatches by specificity), SYNTAX_05 §2 #1
// (a bare `@T:` key is a type facet, not a pattern), SYNTAX_02 §4.12
// (output must read back, and read back as the same value), D101 (a table
// has one normal form whichever way it was written).
//
// ── What is wrong (v0.83.0) ──────────────────────────────────────────────
//
// A table at the top of a file commits and dispatches. The same table one
// field down — or in an arrow's body — does not:
//   * `k: { t: { @{ @str }: 9, @{ @int }: 1 } }` commits, and then
//     `_.k.t "s"` / `_.k.t 5` ⊥ #no_matching_branch (in place: 9 / 1);
//   * `f: (x -> { @{ @str }: 9, @{ @int }: 1 } x)` the same;
//   * `@{ 4 }:` reads back as the literal key `4:`, `@{ _ }:` as the
//     default `_:`, `@{ #a }:` as the bare `#a:` — each a different key;
//   * `@{ @int & 4.. }:`, `@{ 1 | 2 }:` stage bytes that do not parse:
//     the commit fails (rc=1, "store n/ parse").
// `oo fmt --write` rewrites the same keys the same way, in the user's file.
//
// The invariant: a table answers the same wherever it is written — at the
// top, in a field, in a field of a field, in an arrow's body, in a private
// field, after `oo fmt`. Values that hold no pattern key keep their roots.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("lost-its-patterns-{tag}"));
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

    fn eval(&self, e: &str) -> String {
        self.oo(&["eval", e]).0
    }

    /// Evolve then commit one source file in this universe. Both must
    /// succeed: a reading taken after a failed commit is a reading of an
    /// empty universe.
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

    /// `oo fmt --write` on a file holding `src`; the file afterwards.
    fn fmt_write(&self, name: &str, src: &str) -> String {
        let p = self.ws.join(name);
        fs::write(&p, src).unwrap();
        let (o, rc) = self.oo(&["fmt", "--write", name]);
        assert_eq!(rc, 0, "fmt --write {src:?}: {o}");
        fs::read_to_string(&p).unwrap()
    }
}

/// Commit `src` in a fresh universe, then ask each `(expression, answer)`;
/// report every miss at once.
fn committed(tag: &str, src: &str, cases: &[(&str, &str)]) {
    let w = Ws::new(tag);
    w.commit(src);
    let mut wrong = Vec::new();
    for (e, want) in cases {
        let got = w.eval(e);
        if got != *want {
            wrong.push(format!("  {e}\n      want {want}\n      got  {got}"));
        }
    }
    assert!(wrong.is_empty(), "after committing {src:?}\n{}", wrong.join("\n"));
}

/// Commit each source in a fresh universe; compare the root it writes.
fn roots(tag: &str, cases: &[(&str, &str)], why: &str) {
    let mut wrong = Vec::new();
    for (i, (src, want)) in cases.iter().enumerate() {
        let w = Ws::new(&format!("{tag}-{i}"));
        w.commit(src);
        let got = w.root();
        if got != *want {
            wrong.push(format!("  {src:?}\n      want {want}\n      got  {got}"));
        }
    }
    assert!(wrong.is_empty(), "{why}\n{}", wrong.join("\n"));
}

// ── Red: a table keeps its patterns wherever it is written ───────────────

/// The row's own example: a table one field down.
#[test]
fn r1_a_table_in_a_field_dispatches_after_commit() {
    committed("r1", "k: { t: { @{ @str }: 9, @{ @int }: 1 } }\n", &[
        ("_.k.t \"s\"", "9"),
        ("_.k.t 5", "1"),
    ]);
}

/// Deeper, and in a private field read from inside.
#[test]
fn r2_deeper_and_private() {
    committed("r2a", "k: { a: { t: { @{ @str }: 9, @{ @int }: 1 } } }\n", &[
        ("_.k.a.t \"s\"", "9"),
        ("_.k.a.t 5", "1"),
    ]);
    committed("r2b", "~k: { t: { @{ @str }: 9, @{ @int }: 1 } }\n", &[
        ("k.t \"s\"", "9"),
        ("k.t 5", "1"),
    ]);
}

/// An arrow's body is stored as code; a table in it is the same table.
#[test]
fn r3_a_table_in_an_arrow_body() {
    committed("r3", "f: (x -> { @{ @str }: 9, @{ @int }: 1 } x)\n", &[
        ("_.f \"s\"", "9"),
        ("_.f 5", "1"),
    ]);
}

/// Each key form keeps its meaning one field down — not the meaning of the
/// bare key it would print as (`4:`, `_:`, `#a:`).
#[test]
fn r4_every_key_form_keeps_its_meaning() {
    committed("r4a", "k: { t: { @{ 4 }: \"four\", @{ @int }: \"int\" } }\n", &[
        ("_.k.t 4", "\"four\""),
        ("_.k.t 5", "\"int\""),
    ]);
    committed("r4b", "k: { t: { @{ _ }: \"top\", @{ @int }: \"int\" } }\n", &[
        ("_.k.t 4", "\"int\""),
        ("_.k.t \"s\"", "\"top\""),
    ]);
    committed("r4c", "k: { t: { @{ #a }: 1, @{ @tag }: 2 } }\n", &[
        ("_.k.t #a", "1"),
        ("_.k.t #b", "2"),
    ]);
}

/// A key that printed into bytes that do not parse: the commit must
/// succeed, and the table answer.
#[test]
fn r5_a_compound_key_commits() {
    committed("r5a", "z: { t: { @{ @int & 4.. }: \"A\", _: \"Z\" } }\n", &[
        ("_.z.t 2", "\"Z\""),
        ("_.z.t 5", "\"A\""),
    ]);
    committed("r5b", "k: { t: { @{ 1 | 2 }: \"small\", _: \"other\" } }\n", &[
        ("_.k.t 1", "\"small\""),
        ("_.k.t 7", "\"other\""),
    ]);
    committed("r5c", "k: { t: { @{ 1..9 }: \"in\", @{ ..0 }: \"neg\", _: \"no\" } }\n", &[
        ("_.k.t 3", "\"in\""),
        ("_.k.t (0 - 1)", "\"neg\""),
        ("_.k.t 12", "\"no\""),
    ]);
}

/// An arrow meeting a table, one field down (D105 holds at the top).
#[test]
fn r6_an_arrow_and_a_table_one_field_down() {
    committed("r6", "k: { t: (x @str -> 9) & { @{ @int }: 1 } }\n", &[
        ("_.k.t 5", "1"),
        ("_.k.t \"s\"", "9"),
    ]);
}

/// The invariant itself: committed at the top, committed one field down,
/// or asked in place, the same table gives the same answer to every input.
#[test]
fn r7_the_same_table_answers_the_same_at_the_top_and_in_a_field() {
    let tables = [
        "{ @{ @str }: 9, @{ @int }: 1 }",
        "{ @{ 4 }: \"four\", @{ @int & 4.. }: \"big\", _: \"z\" }",
        "{ @{ _ }: 0, @{ 1 | 2 }: 12, @{ #a }: \"a\" }",
        "{ @{ { a: @int } }: \"rec\", @{ [@int, @int] }: \"pair\", _: \"z\" }",
    ];
    let inputs = ["\"s\"", "1", "4", "9", "#a", "{ a: 1 }", "[1, 2]"];
    let mut wrong = Vec::new();
    for (i, t) in tables.iter().enumerate() {
        let top = Ws::new(&format!("r7-top-{i}"));
        top.commit(&format!("k: {t}\n"));
        let nested = Ws::new(&format!("r7-in-{i}"));
        nested.commit(&format!("k: {{ t: {t} }}\n"));
        for x in inputs {
            let a = top.eval(&format!("_.k {x}"));
            let b = nested.eval(&format!("_.k.t {x}"));
            let c = top.eval(&format!("{{ k: {{ t: {t} }} }}.k.t {x}"));
            if a != b || a != c {
                wrong.push(format!("  {t} applied to {x}\n      at the top  {a}\n      in a field  {b}\n      in place    {c}"));
            }
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// `oo fmt --write` must not change what a file means, and must be stable.
#[test]
fn r8_fmt_keeps_a_table_a_table() {
    let src = "k: { t: { @{ @str }: 9, @{ @int & 4.. }: 1, @{ 1 | 2 }: 3, _: 6 } }\nf: (x -> { @{ @str }: 9, @{ @int }: 1 } x)\n";
    let w = Ws::new("r8");
    let once = w.fmt_write("a.n", src);
    let twice = w.fmt_write("b.n", &once);
    assert_eq!(once, twice, "fmt is not stable:\n--- once ---\n{once}\n--- twice ---\n{twice}");
    let cases = ["_.k.t \"s\"", "_.k.t 5", "_.k.t 1", "_.k.t 0", "_.f \"s\"", "_.f 5"];
    let before = Ws::new("r8-src");
    before.commit(src);
    let after = Ws::new("r8-fmt");
    after.commit(&once);
    let mut wrong = Vec::new();
    for e in cases {
        let (a, b) = (before.eval(e), after.eval(e));
        if a != b {
            wrong.push(format!("  {e}\n      as written  {a}\n      after fmt   {b}"));
        }
    }
    assert!(wrong.is_empty(), "fmt changed what the file means:\n{once}\n{}", wrong.join("\n"));
    assert_eq!(before.eval("_.k.t \"s\""), "9", "the written file itself must dispatch");
}

// ── Guards ───────────────────────────────────────────────────────────────

/// Values that hold no pattern key keep their roots — including keys that
/// print bare (`"a.b"`, `#a`, `_`, `@int`) one field down, open ranges
/// one field down, and the identity red lines.
#[test]
fn g1_roots_without_a_pattern_key_stay() {
    roots("g1", &[
        ("x: 0\n", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"), // Q-083／D106 紀元
        ("f: (x -> x + 1)\n", "258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f"), // Q-083／D106 紀元
        ("k: { @{ @str }: 9, @{ @int }: 1 }\n", "8210e484a870e9312794392d57ccce1ff1bb212bc0d1aacb0b52d1676e2b5f3b"), // Q-083／D106 紀元
        ("k: { t: { a: 1, b: \"s\" } }\n", "3f94650b153f5b8d94a715fa607e6dd28625f6dc401f02cad3b452b1e4c67a8e"), // Q-083／D106 紀元
        ("k: { t: { \"a.b\": 1, _: 2 } }\n", "1c69b0dc36393a7de3123eeb710dbae0e0130588874ed1ec4a63ed24470653e0"), // Q-083／D106 紀元
        ("k: { t: { #a: 1, #b: 2 } }\n", "848fee88bc34a0af83330a0dc21df5ca6998fe520f36497327525217afe9e4c9"), // Q-083／D106 紀元
        ("k: { t: { a: 4.., b: ..0 } }\n", "93ddfe5bcfbf83900f80706e2ad1a93a62d1c1ed3cdde1cc56c00e8d0a12bcf0"), // Q-083／D106 紀元
        ("k: { t: { @int: 1 } }\n", "0307f8f168689163ef9aafbd1c31f96a5304d958d10eec5236c792e57353e805"), // Q-083／D106 紀元
        ("f: (x -> { a: x, \"q.r\": x })\n", "84a385e8ddf9140590b6516cebd7a92673119e174e31cfa53650ed97cdb797a6"), // Q-083／D106 紀元
        ("g: (x @str -> 9) & (y @int -> 1)\n", "0c5271d2f1dafa56b41036624acc72e8baf89896d30a3d9b930640c6f1e6edf4"), // Q-083／D106 紀元
        ("k: { t: (x @str -> 9) & (y @int -> 1) }\n", "3256228ab6abcece3e30d78af84746f8a35372aa1a2f82338a129317d6080765"), // Q-083／D106 紀元
    ], "a value with no pattern key moved");
}

/// Keys that are not patterns keep their bare spelling under `oo fmt`.
#[test]
fn g2_bare_keys_print_bare() {
    let w = Ws::new("g2");
    let out = w.fmt_write("a.n", "k: { t: { \"q.r\": 5, #a: 4, _: 6, a: 1, @int: 2, %m: 3, /f: (x -> x) } }\n");
    assert!(!out.contains("@{"), "a key that is not a pattern was wrapped:\n{out}");
    for k in ["\"q.r\": 5", "#a: 4", "_: 6", "a: 1", "@int: 2", "%m: 3", "/f:"] {
        assert!(out.contains(k), "fmt lost the spelling {k:?}:\n{out}");
    }
}
