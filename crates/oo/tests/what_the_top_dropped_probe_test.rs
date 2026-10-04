// What the top dropped.
// Order: nlang-tools/docs/what_the_top_dropped_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-072.  Ruling: meta/oo/STATUS.md D90.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// SYNTAX_03 §2 item 3: a dotted key is a nested coordinate — `a.b.c: v` is
// the nested combo `{a: {b: {c: v}}}`. Inside a combo the engine keeps that
// promise for every key shape. At the top of a file it keeps it for none:
// `a.b: 42`, `@a.b: 42`, `#t.x: 42`, `~h.b: 42`, `1.a: 42`, … are dropped
// without a word by `run`, `evolve`, `test` and `repl` (measured on every
// tag build back to at least v0.40.0). `evolve` answers rc=0, `status` does
// not show the key, the commit does not hold it, and a conflict it should
// raise is swallowed.
//
// D90 (user, 2026-10-04, 甲): a definition key is always relative to the
// container it is written in. A key may not anchor at the root (`_.`), just
// as it may not climb (`^`) or name another universe (`_{…}`). At the top
// `_.a:` would only be a second spelling of `a:`; inside a literal it would
// write through to the root — the locality break that abolished LHS `^` on
// 2026-07-17. The parser refuses it.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Wording-free: every red probe compares a dotted key with the nested
// combo it is defined to be, through the same command, in two directories
// holding a file of the same name. Commits are compared by root address
// (`oo inspect <HEAD>`), byte for byte. `.oo/` is compared byte for byte.
//
// The delivery may NOT edit this file. If a pin is wrong, say so in the
// report. `rustfmt` must not touch it.
//
// Baseline measured 2026-10-04 on dev 32904dc / oo v0.73.0: see the order.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const STD_HEX: &str = "7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911";

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    n: std::cell::Cell<usize>,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("top-dropped-{tag}"));
        let root = scratch.path().to_path_buf();
        Ws { _scratch: scratch, root, n: std::cell::Cell::new(0) }
    }

    /// A fresh, empty directory.
    fn dir(&self) -> PathBuf {
        let i = self.n.get();
        self.n.set(i + 1);
        let d = self.root.join(format!("d{i}"));
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn cmd(&self, dir: &Path, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
        c.args(args)
            .current_dir(dir)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"));
        c
    }

    fn oo(&self, dir: &Path, args: &[&str]) -> (String, i32) {
        let o = self.cmd(dir, args).stdin(Stdio::null()).output().expect("oo runs");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
            o.status.code().unwrap_or(-1),
        )
    }

    fn ok(&self, dir: &Path, args: &[&str]) -> String {
        let (o, rc) = self.oo(dir, args);
        assert_eq!(rc, 0, "setup `oo {}`: {o}", args.join(" "));
        o
    }

    /// Write `t.n` holding `text` into a fresh directory and run `oo <cmd> t.n`.
    fn on_file(&self, cmd: &[&str], text: &str) -> (String, i32) {
        let d = self.dir();
        fs::write(d.join("t.n"), text).unwrap();
        let mut a = cmd.to_vec();
        a.push("t.n");
        self.oo(&d, &a)
    }

    fn run(&self, text: &str) -> (String, i32) {
        self.on_file(&["run", "--format"], text)
    }

    /// A fresh universe holding one commit of `text`.
    fn committed(&self, text: &str) -> PathBuf {
        let d = self.dir();
        fs::write(d.join("a.n"), text).unwrap();
        self.ok(&d, &["evolve", "a.n"]);
        self.ok(&d, &["commit", "-m", "a"]);
        d
    }

    /// Evolve and commit `text` in `d`; the new root address, or what it said.
    fn commit_root(&self, d: &Path, text: &str) -> Result<String, String> {
        fs::write(d.join("b.n"), text).unwrap();
        let (o, rc) = self.oo(d, &["evolve", "b.n"]);
        if rc != 0 {
            return Err(format!("evolve rc={rc}: {o}"));
        }
        let (o, rc) = self.oo(d, &["commit", "-m", "b"]);
        if rc != 0 {
            return Err(format!("commit rc={rc}: {o}"));
        }
        Ok(self.root_of(d))
    }

    fn root_of(&self, d: &Path) -> String {
        let head = fs::read_to_string(d.join(".oo/HEAD")).unwrap().trim().to_string();
        let o = self.ok(d, &["inspect", &head]);
        o.lines()
            .find_map(|l| l.trim().strip_prefix("root:").map(|s| s.trim().to_string()))
            .unwrap_or_else(|| panic!("no root line: {o}"))
    }

    /// Run `oo repl` in `dir`, feeding `lines` then `exit`.
    fn repl(&self, dir: &Path, lines: &[&str]) -> (String, i32) {
        let mut child = self
            .cmd(dir, &["repl"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("oo repl spawns");
        let mut input = lines.join("\n");
        input.push_str("\nexit\n");
        child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        let o = child.wait_with_output().unwrap();
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
            o.status.code().unwrap_or(-1),
        )
    }

    fn snapshot(&self, d: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut m = BTreeMap::new();
        let oo = d.join(".oo");
        files(&oo, &oo, &mut m);
        m
    }
}

fn files(base: &Path, p: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    if let Ok(d) = fs::read_dir(p) {
        for e in d.flatten() {
            let q = e.path();
            if q.is_dir() {
                files(base, &q, out);
            } else {
                out.insert(q.strip_prefix(base).unwrap().display().to_string(), fs::read(&q).unwrap());
            }
        }
    }
}

fn prints(o: &str, v: &str) -> bool {
    o.split(|c: char| !c.is_ascii_alphanumeric()).any(|t| t == v)
}

/// (dotted, nested) — each pair is one definition in two spellings.
const SHAPES: &[(&str, &str)] = &[
    ("a.b: 42", "a: { b: 42 }"),
    ("a.b.c: 42", "a: { b: { c: 42 } }"),
    ("@a.b: 42", "@a: { b: 42 }"),
    ("#t.x: 42", "#t: { x: 42 }"),
    ("~h.b: 42", "~h: { b: 42 }"),
    ("/f.g: 42", "/f: { g: 42 }"),
    ("%m.g: 42", "%m: { g: 42 }"),
    ("1.a: 42", "1: { a: 42 }"),
    ("a.~h: 42", "a: { ~h: 42 }"),
    ("a.1: 42", "a: { 1: 42 }"),
];

// ── Red ──────────────────────────────────────────────────────────────────

/// r1 — a dotted key commits the same root as its nested combo.
#[test]
fn r1_a_dotted_key_commits_the_nested_root() {
    let w = Ws::new("r1");
    let d1 = w.dir();
    let d2 = w.dir();
    let dotted = w.commit_root(&d1, "a.b: 42\nc: 1\n");
    let nested = w.commit_root(&d2, "a: { b: 42 }\nc: 1\n");
    assert!(nested.is_ok(), "VOID READING: the nested combo did not commit: {nested:?}");
    assert_eq!(dotted, nested, "`a.b: 42` committed a different root from `a: {{ b: 42 }}`");
    let (o, _) = w.oo(&d1, &["eval", "_.a.b"]);
    assert!(prints(&o, "42"), "the committed `a.b` does not read back: {o}");
}

/// r2 — every key shape at the top is its nested combo (`run`).
#[test]
fn r2_every_key_shape_is_its_nested_combo() {
    let w = Ws::new("r2");
    let mut wrong = Vec::new();
    for (dotted, nested) in SHAPES {
        let n = w.run(&format!("{nested}\n"));
        assert!(n.1 == 0 && prints(&n.0, "42"), "VOID READING: `{nested}` itself: {n:?}");
        let d = w.run(&format!("{dotted}\n"));
        if d != n {
            wrong.push(format!("`{dotted}` → {d:?}\n   `{nested}` → {n:?}"));
        }
    }
    assert!(wrong.is_empty(), "top-level keys that are not their nested combo:\n{}", wrong.join("\n"));
}

/// r3 — a dotted key conflicts exactly as its nested combo (`run`, `evolve`).
#[test]
fn r3_a_dotted_key_conflicts_as_its_nested_combo() {
    let w = Ws::new("r3");
    let dotted = "a: { b: 1 }\na.b: 2\n";
    let nested = "a: { b: 1 }\na: { b: 2 }\n";
    for cmd in [&["run", "--format"][..], &["evolve"][..]] {
        let n = w.on_file(cmd, nested);
        assert_ne!(n.1, 0, "VOID READING: `{}` on the nested conflict: {n:?}", cmd.join(" "));
        let d = w.on_file(cmd, dotted);
        assert_eq!(d, n, "`oo {}`: a dotted conflict is not the nested one", cmd.join(" "));
    }
}

/// r4 — a dotted key meets its siblings: in one file, and across commits.
#[test]
fn r4_a_dotted_key_meets_its_siblings() {
    let w = Ws::new("r4");
    let n = w.run("a: { b: 1 }\na: { c: 2 }\n");
    assert!(n.1 == 0 && prints(&n.0, "2"), "VOID READING: {n:?}");
    assert_eq!(w.run("a: { b: 1 }\na.c: 2\n"), n, "`a.c: 2` did not meet `a: {{ b: 1 }}` in one file");

    let d1 = w.committed("a: { b: 1 }\n");
    let d2 = w.committed("a: { b: 1 }\n");
    let dotted = w.commit_root(&d1, "a.c: 2\n");
    let nested = w.commit_root(&d2, "a: { c: 2 }\n");
    assert!(nested.is_ok(), "VOID READING: {nested:?}");
    assert_eq!(dotted, nested, "`a.c: 2` on a committed `a: {{ b: 1 }}` is not `a: {{ c: 2 }}`");
}

/// r5 — `status` shows a staged dotted key as it shows the nested combo.
#[test]
fn r5_status_shows_a_dotted_key() {
    let w = Ws::new("r5");
    let show = |text: &str| {
        let d = w.dir();
        fs::write(d.join("t.n"), text).unwrap();
        w.ok(&d, &["evolve", "t.n"]);
        w.ok(&d, &["status"])
    };
    let n = show("c: 1\na: { b: 4242 }\n");
    assert!(prints(&n, "4242"), "VOID READING: {n}");
    assert_eq!(show("c: 1\na.b: 4242\n"), n, "status does not show a staged dotted key as its nested combo");
}

/// r6 — `test` reads a dotted key.
#[test]
fn r6_test_reads_a_dotted_key() {
    let w = Ws::new("r6");
    let n = w.on_file(&["test"], "t: { x: 42 }\ntest_ok: _.t.x + 1 == 43\n");
    assert_eq!(n.1, 0, "VOID READING: the nested test: {n:?}");
    let d = w.on_file(&["test"], "t.x: 42\ntest_ok: _.t.x + 1 == 43\n");
    assert_eq!(d.1, 0, "a test over a dotted key failed: {d:?}");
}

/// r7 — `repl` answers a dotted key with its value, and later lines see it.
#[test]
fn r7_repl_answers_a_dotted_key() {
    let w = Ws::new("r7");
    let d = w.dir();
    let (o, rc) = w.repl(&d, &["a.b: 4242", "z: a.b + 1"]);
    assert_eq!(rc, 0, "{o}");
    assert!(prints(&o, "4242"), "repl did not answer `a.b: 4242` with its value: {o}");
    assert!(prints(&o, "4243"), "a later line did not see `a.b`: {o}");
}

/// r8 — D90: a definition key may not anchor at the root, at the top or
/// inside a literal; the whole file is refused and nothing is written.
#[test]
fn r8_a_key_may_not_anchor_at_the_root() {
    let w = Ws::new("r8");
    let texts = ["_.a: 42\nc: 1\n", "_.a.b: 42\nc: 1\n", "x: { _.a: 42 }\nc: 1\n", "_.: 42\nc: 1\n", "v: [{ _.a: 42 }]\nc: 1\n"];
    let mut wrong = Vec::new();
    for t in texts {
        for cmd in [&["run", "--format"][..], &["fmt"][..]] {
            let (o, rc) = w.on_file(cmd, t);
            if rc == 0 {
                wrong.push(format!("`oo {}` accepted {t:?}: {o}", cmd.join(" ")));
            }
        }
        let d = w.committed("k: 1\n");
        let before = w.snapshot(&d);
        fs::write(d.join("t.n"), t).unwrap();
        let (o, rc) = w.oo(&d, &["evolve", "t.n"]);
        if rc == 0 {
            wrong.push(format!("`oo evolve` accepted {t:?}: {o}"));
        }
        if w.snapshot(&d) != before {
            wrong.push(format!("`oo evolve` of {t:?} wrote .oo/"));
        }
    }
    let (o, rc) = w.oo(&w.dir(), &["eval", "{ _.a: 42 }"]);
    if rc == 0 {
        wrong.push(format!("`oo eval '{{ _.a: 42 }}'` accepted: {o}"));
    }
    assert!(wrong.is_empty(), "root-anchored definition keys accepted:\n{}", wrong.join("\n"));
}

/// r9 — relative references in the value resolve as in the nested combo:
/// the containers a dotted key's path creates are containers (`^` counts
/// them), at the top and inside a combo.
#[test]
fn r9_a_dotted_value_sees_the_containers_its_path_makes() {
    let w = Ws::new("r9");
    let cases: &[(&str, &str, &str)] = &[
        ("a", "y: 1\na.b: ^.y\n", "y: 1\na: { b: ^.y }\n"),
        ("a", "y: 1\na.b.c: ^^.y\n", "y: 1\na: { b: { c: ^^.y } }\n"),
        ("x.a", "x: { y: 1, a.b: ^.y }\n", "x: { y: 1, a: { b: ^.y } }\n"),
        ("x.a", "x: { y: 1, a.b.c: ^^.y }\n", "x: { y: 1, a: { b: { c: ^^.y } } }\n"),
    ];
    let mut wrong = Vec::new();
    for (at, dotted, nested) in cases {
        let n = w.on_file(&["run", "--observe", at], nested);
        assert!(n.1 == 0 && prints(&n.0, "1"), "VOID READING: {nested:?} → {n:?}");
        let d = w.on_file(&["run", "--observe", at], dotted);
        if d != n {
            wrong.push(format!("{dotted:?} → {d:?}\n   {nested:?} → {n:?}"));
        }
    }
    assert!(wrong.is_empty(), "dotted values that do not resolve as their nested combo:\n{}", wrong.join("\n"));
}

// ── Green ────────────────────────────────────────────────────────────────

/// g1 — the root `~%Config.<knob>` family still lands as a partial override.
#[test]
fn g1_the_config_knob_still_lands() {
    let w = Ws::new("g1");
    let (o, rc) = w.on_file(&["run", "--observe", "v"], "~%Config.fuel: 50\nv: ~%Config.fuel + 1\n");
    assert_eq!(rc, 0, "{o}");
    assert!(prints(&o, "51"), "`~%Config.fuel: 50` no longer lands: {o}");
}

/// g2 — a dotted key into an engine-minted `~%` axis is still refused,
/// and the file stages nothing.
#[test]
fn g2_the_system_axis_is_still_refused() {
    let w = Ws::new("g2");
    let (o, rc) = w.run("~%Foo.b: 42\nc: 1\n");
    assert_ne!(rc, 0, "`run` accepted `~%Foo.b: 42`: {o}");
    let d = w.committed("k: 1\n");
    let before = w.snapshot(&d);
    fs::write(d.join("t.n"), "~%Foo.b: 42\nc: 1\n").unwrap();
    let (o, rc) = w.oo(&d, &["evolve", "t.n"]);
    assert_ne!(rc, 0, "`evolve` accepted `~%Foo.b: 42`: {o}");
    assert!(w.snapshot(&d) == before, "a refused `~%Foo.b` evolve wrote .oo/");
}

/// g3 — `_.` still reads from the root on the right-hand side, at the top
/// and inside a literal.
#[test]
fn g3_root_paths_still_read() {
    let w = Ws::new("g3");
    let d = w.committed("a: 4240\n");
    fs::write(d.join("t.n"), "v: _.a + 2\nx: { u: _.a + 3 }\n").unwrap();
    for (path, want) in [("v", "4242"), ("x.u", "4243")] {
        let (o, rc) = w.oo(&d, &["run", "--observe", path, "t.n"]);
        assert_eq!(rc, 0, "{o}");
        assert!(prints(&o, want), "`_.a` no longer reads the root at `{path}`: {o}");
    }
}

/// g4 — a quoted dot is part of one name, not a path (SYNTAX_03 §4 #8).
#[test]
fn g4_a_quoted_dot_is_one_name() {
    let w = Ws::new("g4");
    let q = w.run("\"a.b\": 42\n");
    let n = w.run("a: { b: 42 }\n");
    assert!(q.1 == 0 && prints(&q.0, "42"), "{q:?}");
    assert_ne!(q, n, "`\"a.b\": 42` became the nested `a: {{ b: 42 }}`");
}

/// g5 — inside a combo, dotted keys were already right; they stay right.
#[test]
fn g5_nested_dotted_keys_stay_right() {
    let w = Ws::new("g5");
    for (dotted, nested) in SHAPES {
        let n = w.run(&format!("x: {{ {nested} }}\n"));
        let d = w.run(&format!("x: {{ {dotted} }}\n"));
        assert!(n.1 == 0 && prints(&n.0, "42"), "{n:?}");
        assert_eq!(d, n, "inside a combo, `{dotted}` is no longer `{nested}`");
    }
    let n = w.run("x: { a: { b: 1 }, a: { b: 2 } }\n");
    assert_eq!(w.run("x: { a.b: 1, a: { b: 2 } }\n"), n, "a nested dotted conflict changed");
}

/// g6 — the other anchors and index segments stay refused as keys.
#[test]
fn g6_other_key_anchors_stay_refused() {
    let w = Ws::new("g6");
    let addr = format!("_{{sha256:{STD_HEX}}}.a: 1\n");
    for t in ["^.a: 1\n", "x: { ^.a: 1 }\n", addr.as_str(), "a[0]: 1\n", "x: { a: [1], a[0]: 1 }\n"] {
        let (o, rc) = w.run(t);
        assert_ne!(rc, 0, "{t:?} became a key: {o}");
    }
}
