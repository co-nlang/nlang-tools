// A tag key read as a string.
// Order: nlang-tools/docs/a_tag_key_read_as_a_string_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-085.  Ruling: STATUS D107 (a
// table reads a key by the coordinate it spells; quotes only spell the
// name). Spec: SPEC_07 §1 (a value with a pattern key is a table; every
// non-meta key is a constraint, D97), STATUS D101 (one normal form), D102
// (how a table prints).
//
// ── What is wrong (v0.86.0; v0.78.0 on) ──────────────────────────────────
//
// In a table, `#a:` is the constraint "the string `#a`", not the tag:
//   `({ @{ @str }: 1, #a: 2 }) #a`        ⟹ #no_matching_branch
//   `({ @{ @str }: 1, #a: 2 }) "#a"`      ⟹ 2
//   `({ @{ @str }: 1, #a: 2, _: 0 }) #a`  ⟹ 0     (silent)
// in a field, after commit, built by `&` or a spread, through `|>`. The key
// parses as a one-segment path and lands as the coordinate "#a" — the same
// coordinate as `"#a":` (the address carries only the string) — and the
// table reads that coordinate as a string. The printer has the mirror
// fault: the string constraints `@{ "4" }:` and `@{ "_" }:` print as `4:`
// and `_:`, which read back as the integer and as Top.
//
// D107: the table reads the coordinate's spelling as the parser reads that
// token — an integer, a tag (`#a`, `#_`, `#_|_`), `_`, else the string —
// so `#a:` and `"#a":` are both the tag. A string that spells another atom
// is written `@{ "#a" }:` and stays a string. A table prints an atomic
// constraint bare only when the bare key reads back as that constraint.
//
// The invariants: every route to a table reads a tag key as the tag (r1,
// r3, r4); quoting a key does not change its constraint, and `@{ "…" }`
// keeps the string (r2); a printed table, read back, is the same table (r5),
// and prints bare whatever reads back bare (r6).
//
// Guard: a combo with no pattern key is not a table — `#a:` and `"#a":` are
// one coordinate there and nothing about it moves (g1).
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("tag-key-{tag}"));
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

// ── Red ──────────────────────────────────────────────────────────────────

/// A tag key in a table is the tag (D97, D107). v0.86.0: #no_matching_branch,
/// the string answers instead, and with `_:` the tag silently gets `0`.
#[test]
fn r1_a_tag_key_is_the_tag() {
    answers("r1", &[
        ("({ @{ @str }: 1, #a: 2 }) #a", "2"),
        ("({ @{ @str }: 1, #a: 2, _: 0 }) #a", "2"),
        ("({ @{ @str }: 1, #a: 2 }) \"#a\"", "1"),
        ("({ @{ @str }: 1, #a: 2, #b: 3 }) #b", "3"),
        ("({ @{ @str }: 1, #a: (x -> 5) }) #a", "5"),
        ("({ @{ @str }: 1, #_: 2 }) #_", "2"),
        ("({ @{ @str }: 1, #_|_: 2 }) #_|_", "2"),
        // Controls, right on v0.86.0: the same constraint as a pattern key.
        ("({ @{ @str }: 1, @{ #a }: 2, _: 0 }) #a", "2"),
        ("({ @{ @str }: 1, @{ #a }: 2, _: 0 }) \"#a\"", "1"),
    ]);
}

/// Quotes spell the name; they do not change the coordinate or its
/// constraint. A string that spells another atom is written `@{ "…" }`.
/// v0.86.0: `"#a":` is the string, and `#a:` is not the same table as
/// `@{ #a }:`.
#[test]
fn r2_quotes_do_not_change_the_constraint() {
    answers("r2", &[
        ("({ @{ @str }: 1, \"#a\": 2 }) #a", "2"),
        ("({ @{ @str }: 1, \"#a\": 2, _: 0 }) \"#a\"", "1"),
        ("{ @{ @int }: 1, #a: 2 } = { @{ @int }: 1, @{ #a }: 2 }", "#true"),
        ("{ @{ @int }: 1, @{ \"#a\" }: 2 } = { @{ @int }: 1, #a: 2 }", "#false"),
        // Controls, right on v0.86.0.
        ("{ @{ @int }: 1, #a: 2 } = { @{ @int }: 1, \"#a\": 2 }", "#true"),
        ("({ @{ @str }: 1, @{ \"#a\" }: 2, _: 0 }) \"#a\"", "2"),
        ("({ @{ @str }: 1, @{ \"#a\" }: 2, _: 0 }) #a", "0"),
        ("({ @{ @str }: 1, \"4\": 2 }) 4", "2"),
        ("({ @{ @int }: 1, @{ \"4\" }: 2 }) \"4\"", "2"),
        ("({ @{ @str }: 1, \"j\": 2, _: 0 }) \"j\"", "2"),
    ]);
}

/// Every way of reaching the table reads the key the same way (D101).
/// v0.86.0: #no_matching_branch or `0` throughout.
#[test]
fn r3_every_route_to_the_table() {
    answers("r3", &[
        ("({ @{ @str }: 1 } & { #a: 2 }) #a", "2"),
        ("({ ...{ @{ @str }: 1 }, #a: 2, _: 0 }) #a", "2"),
        ("({ ...{ #a: 2 }, @{ @str }: 1, _: 0 }) #a", "2"),
        ("{ t: { @{ @str }: 1, #a: 2, _: 0 } }.t #a", "2"),
        ("#a |> { @{ @str }: 1, #a: 2 }", "2"),
        ("(#a | #b) |> { @{ @str }: 1, #a: 2, #b: 3 }", "2 | 3"),
        ("({ @{ @str }: 1, #a: 2 }) (#a | \"s\")", "1 | 2"),
    ]);
}

/// The same after commit: the answer, and the address is the address of
/// the table written with `@{ #a }:`. v0.86.0: `0`, and two roots. The
/// address is compared at the top only: a table nested in a field is kept
/// as its source, and spelling there enters the address today for every
/// key kind (`4:` and `@{ 4 }:` too) — another Inbox row, not this card.
#[test]
fn r4_after_commit() {
    let w = Ws::new("r4");
    w.commit(&["k: { @{ @str }: 1, #a: 2, _: 0 }\nd: { t: { @{ @str }: 1, \"#b\": 3, _: 0 } }\n"]);
    let mut bad = Vec::new();
    for (e, want) in [("_.k #a", "2"), ("_.k \"#a\"", "1"), ("_.d.t #b", "3"), ("_.d.t \"#b\"", "1")] {
        let got = w.eval(e);
        if got != want {
            bad.push(format!("{e}: want {want} got {got}"));
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
    let w = Ws::new("r4b");
    w.commit(&["k: { @{ @str }: 1, #a: 2, _: 0 }\n"]);
    let tag_key = w.root();
    let w = Ws::new("r4c");
    w.commit(&["k: { @{ @str }: 1, @{ #a }: 2, _: 0 }\n"]);
    assert_eq!(tag_key, w.root(), "`#a:` and `@{{ #a }}:` are one table, so one address");
}

/// A printed table, read back, is the same table (SYNTAX_02 §4 #12). Each
/// table keeps one non-atomic row, so the all-atomic case (another Inbox
/// row) is not measured here. v0.86.0: `@{ "4" }:`, `@{ "_" }:`, `@{ "+4" }:`
/// print bare and read back as other constraints.
#[test]
fn r5_a_printed_table_reads_back_as_itself() {
    let w = Ws::new("r5");
    let tables = [
        "{ @{ @int }: 1, @{ \"4\" }: 2 }",
        "{ @{ @int }: 1, @{ \"_\" }: 2 }",
        "{ @{ @int }: 1, @{ \"+4\" }: 2 }",
        "{ @{ @int }: 1, @{ \"#a\" }: 2 }",
        "{ @{ @int }: 1, @{ \"#_\" }: 2 }",
        "{ @{ @int }: 1, #a: 2, #_: 3, #_|_: 4 }",
        "{ @{ @str }: 1, 4: 2, j: 3, _: 0 }",
        "{ @{ @int }: 1, @{ #a }: 2, @{ \"#a\" }: 3, @{ 4 }: 4, @{ \"4\" }: 5, @{ _ }: 6, @{ \"_\" }: 7, @{ \"j\" }: 8 }",
    ];
    let mut bad = Vec::new();
    for t in tables {
        let printed = w.eval_raw(t);
        let back = w.eval(&format!("({t}) = ({printed})"));
        if back != "#true" {
            bad.push(format!("  {t}\n    printed {}\n    read back = {back}", squash(&printed)));
        }
    }
    assert!(bad.is_empty(), "{} of {} do not read back:\n{}", bad.len(), tables.len(), bad.join("\n"));
}

/// D102 under D107: an atomic constraint prints bare when the bare key reads
/// back as it, and as `@{ … }:` otherwise. v0.86.0: `#a:` prints `"#a":`,
/// and the strings below print bare.
#[test]
fn r6_prints_bare_what_reads_back_bare() {
    answers("r6", &[
        ("{ @{ @int }: 1, #a: 2 }", "{ #a: 2 @{ @int }: 1 }"),
        ("{ @{ @int }: 1, @{ \"4\" }: 2 }", "{ @{ \"4\" }: 2 @{ @int }: 1 }"),
        ("{ @{ @int }: 1, @{ \"_\" }: 2 }", "{ @{ \"_\" }: 2 @{ @int }: 1 }"),
        ("{ @{ @int }: 1, @{ \"#a\" }: 2 }", "{ @{ \"#a\" }: 2 @{ @int }: 1 }"),
        // Controls, right on v0.86.0.
        ("{ @{ @int }: 1, @{ \"j\" }: 2 }", "{ @{ @int }: 1 j: 2 }"),
        ("{ @{ @int }: 1, @{ 4 }: 2, _: 0 }", "{ 4: 2 @{ @int }: 1 _: 0 }"),
    ]);
}

// ── Guard (green on v0.86.0) ─────────────────────────────────────────────

/// A combo with no pattern key is not a table: it is applied by key lookup
/// (D97), `#a:` and `"#a":` are one coordinate there, and its address does
/// not move.
#[test]
fn g1_a_combo_without_a_pattern_key_is_not_a_table() {
    answers("g1", &[
        ("{ #a: 2 } = { \"#a\": 2 }", "#true"),
        ("{ #a: 2, _: 0 } = { \"#a\": 2, _: 0 }", "#true"),
        ("({ #a: 2, _: 0 }) #a", "2"),
        ("({ #a: 2, _: 0 }) \"#a\"", "2"),
        ("{ \"#a\": 1 } #a", "1"),
    ]);
    let w = Ws::new("g1c");
    w.commit(&["k: { #a: 2 }\n"]);
    assert_eq!(w.root(), "e841a181509be360123c271537de552e48434e667ad5946c3355db623bf2e756");
}
