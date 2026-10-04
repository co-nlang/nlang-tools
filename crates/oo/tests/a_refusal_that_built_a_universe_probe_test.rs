// A refusal that built a universe.
// Order: nlang-tools/docs/a_refusal_that_built_a_universe_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-074.  Reading: meta/oo/STATUS.md D92.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// REAL_02 §5.1.1 (D82, D82 ②): declarations are universe content, and only
// the act that writes a proposal into the workspace creates a universe.
// SPEC_10 §3.1 (Q-065): an evolve that reports failure leaves no proposal.
// Measured on v0.75.0: an `evolve` refused in a place with no universe —
// eight ways (parse error, missing file, in-file conflict, `~%` ownership,
// a root-anchored key, an unknown knob, an unknown grant, `--pin` without
// its grant) — leaves `.oo/format`, `.oo/objects.format` and `objects/`
// behind. From then on the directory is an empty universe: `status` says
// "Universe is static" where it said "no universe here". An evolve of a file
// with nothing in it does the same and also mints an empty savepoint.
// Proposals were never the leak: a two-file evolve whose second file fails
// stages nothing.
//
// D92 (user confirmed the reading, 2026-10-04): an evolve that writes no
// proposal — refused, or with nothing to propose — creates no universe.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Wording-free: "nothing left" is `.oo/` byte for byte as it was before;
// "still no universe" is `status`/`log` answering exactly as in a directory
// nobody touched. The delivery may NOT edit this file. `rustfmt` must not
// touch it.
//
// Baseline measured 2026-10-04 on dev e658530 / oo v0.75.0: see the order.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const KEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    n: std::cell::Cell<usize>,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("refusal-built-{tag}"));
        let root = scratch.path().to_path_buf();
        Ws { _scratch: scratch, root, n: std::cell::Cell::new(0) }
    }

    fn dir(&self) -> PathBuf {
        let i = self.n.get();
        self.n.set(i + 1);
        let d = self.root.join(format!("d{i}"));
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn oo(&self, dir: &Path, args: &[&str]) -> (String, i32) {
        let o = Command::new(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(dir)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .stdin(Stdio::null())
            .output()
            .expect("oo runs");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
            o.status.code().unwrap_or(-1),
        )
    }

    fn snapshot(&self, d: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut m = BTreeMap::new();
        let oo = d.join(".oo");
        files(&oo, &oo, &mut m);
        m
    }

    /// What `status` and `log` say in a directory nobody touched.
    fn untouched_answers(&self) -> Vec<(String, i32)> {
        let d = self.dir();
        vec![self.oo(&d, &["status"]), self.oo(&d, &["log"])]
    }
}

fn files(base: &Path, p: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    if let Ok(d) = fs::read_dir(p) {
        for e in d.flatten() {
            let q = e.path();
            if q.is_dir() {
                out.insert(format!("{}/", q.strip_prefix(base).unwrap().display()), Vec::new());
                files(base, &q, out);
            } else {
                out.insert(q.strip_prefix(base).unwrap().display().to_string(), fs::read(&q).unwrap());
            }
        }
    }
}

/// (name, file contents, evolve arguments after the file list) — each is
/// refused today.
const REFUSALS: &[(&str, &str, &[&str])] = &[
    ("parse error", "a: (\n", &["f.n"]),
    ("missing file", "a: 1\n", &["nofile.n"]),
    ("in-file conflict", "a: 1\na: 2\n", &["f.n"]),
    ("system axis", "~%Foo: 1\n", &["f.n"]),
    ("root-anchored key", "_.a: 1\n", &["f.n"]),
    ("unknown knob", "~%Config.bogus: 1\n", &["f.n"]),
    ("unknown grant", "a: 1\n", &["f.n", "--grant", "nonsense"]),
    ("pin without grant", "a: 1\n", &["--pin", "f.n"]),
];

// ── Red ──────────────────────────────────────────────────────────────────

/// r1 — every refused evolve, where there is no universe, leaves nothing.
#[test]
fn r1_a_refused_evolve_leaves_nothing() {
    let w = Ws::new("r1");
    let mut wrong = Vec::new();
    for (name, text, args) in REFUSALS {
        let d = w.dir();
        fs::write(d.join("f.n"), text).unwrap();
        let mut a = vec!["evolve"];
        a.extend_from_slice(args);
        let (o, rc) = w.oo(&d, &a);
        assert_ne!(rc, 0, "VOID READING: `{name}` was not refused: {o}");
        if d.join(".oo").exists() {
            wrong.push(format!("{name}: left {:?}", w.snapshot(&d).keys().collect::<Vec<_>>()));
        }
    }
    let d = w.dir();
    fs::write(d.join("good.n"), "a: 1\n").unwrap();
    fs::write(d.join("bad.n"), "b: (\n").unwrap();
    let (o, rc) = w.oo(&d, &["evolve", "good.n", "bad.n"]);
    assert_ne!(rc, 0, "VOID READING: a two-file evolve with a bad second file: {o}");
    if d.join(".oo").exists() {
        wrong.push(format!("good + bad: left {:?}", w.snapshot(&d).keys().collect::<Vec<_>>()));
    }
    assert!(wrong.is_empty(), "refused evolves that built a universe:\n{}", wrong.join("\n"));
}

/// r2 — a refused evolve leaves node settings exactly as they were (D82 ②).
#[test]
fn r2_a_refused_evolve_leaves_node_settings_alone() {
    let w = Ws::new("r2");
    let d = w.dir();
    let (o, rc) = w.oo(&d, &["node", "trust", "add", KEY]);
    assert_eq!(rc, 0, "VOID READING: trust add: {o}");
    let before = w.snapshot(&d);
    assert!(before.contains_key("discovery.n") && !before.contains_key("format"), "VOID READING: {before:?}");
    fs::write(d.join("f.n"), "a: (\n").unwrap();
    let (o, rc) = w.oo(&d, &["evolve", "f.n"]);
    assert_ne!(rc, 0, "{o}");
    assert_eq!(w.snapshot(&d).keys().collect::<Vec<_>>(), before.keys().collect::<Vec<_>>(), "a refused evolve added to a node-settings-only container");
    assert!(w.snapshot(&d) == before, "a refused evolve changed node settings");
}

/// r3 — after a refusal there is still no universe: `status` and `log`
/// answer exactly as where nobody has been.
#[test]
fn r3_after_a_refusal_there_is_still_no_universe() {
    let w = Ws::new("r3");
    let untouched = w.untouched_answers();
    assert!(untouched.iter().all(|(_, rc)| *rc != 0), "VOID READING: an untouched directory answers as a universe: {untouched:?}");
    let d = w.dir();
    fs::write(d.join("f.n"), "a: (\n").unwrap();
    w.oo(&d, &["evolve", "f.n"]);
    let after = vec![w.oo(&d, &["status"]), w.oo(&d, &["log"])];
    assert_eq!(after, untouched, "after a refused evolve, status/log answer as if a universe existed");
}

/// r4 — an evolve with nothing to propose creates nothing — including one
/// whose injection moves no position (D84: not a proposal).
#[test]
fn r4_nothing_to_propose_creates_nothing() {
    let w = Ws::new("r4");
    let untouched = w.untouched_answers();
    let mut wrong = Vec::new();
    // `v: _` writes an injection that moves no position: not a proposal (D84).
    for (name, text) in [("empty file", ""), ("comments only", ";; nothing\n"), ("only Top", "v: _\n")] {
        let d = w.dir();
        fs::write(d.join("f.n"), text).unwrap();
        let (o, _) = w.oo(&d, &["evolve", "f.n"]);
        if d.join(".oo").exists() {
            wrong.push(format!("{name}: left {:?} ({o})", w.snapshot(&d).keys().collect::<Vec<_>>()));
        }
        let after = vec![w.oo(&d, &["status"]), w.oo(&d, &["log"])];
        if after != untouched {
            wrong.push(format!("{name}: status/log now {after:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

// ── Green ────────────────────────────────────────────────────────────────

/// g1 — an evolve that writes a proposal still creates the universe, a
/// field that is `_|_` included.
#[test]
fn g1_a_proposal_still_creates_the_universe() {
    let w = Ws::new("g1");
    for text in ["a: 1\n", "x: 1\ny: 1 & 2\n"] {
        let d = w.dir();
        fs::write(d.join("f.n"), text).unwrap();
        let (o, rc) = w.oo(&d, &["evolve", "f.n"]);
        assert_eq!(rc, 0, "{o}");
        let snap = w.snapshot(&d);
        assert!(snap.contains_key("format") && snap.contains_key("objects.format"), "no declarations after a proposal: {snap:?}");
        let (st, rc) = w.oo(&d, &["status"]);
        assert!(rc == 0 && (st.contains("x: 1") || st.contains("a: 1")), "status does not show the proposal: {st}");
    }
}

/// g2 — in an existing universe a refused evolve changes nothing, and the
/// universe stays (no over-eager cleanup).
#[test]
fn g2_a_refusal_in_a_universe_changes_nothing() {
    let w = Ws::new("g2");
    let d = w.dir();
    fs::write(d.join("k.n"), "k: 1\n").unwrap();
    assert_eq!(w.oo(&d, &["evolve", "k.n"]).1, 0);
    assert_eq!(w.oo(&d, &["commit", "-m", "k"]).1, 0);
    fs::write(d.join("y.n"), "y: 7\n").unwrap();
    assert_eq!(w.oo(&d, &["evolve", "y.n"]).1, 0);
    let before = w.snapshot(&d);
    for (name, text, args) in REFUSALS {
        fs::write(d.join("f.n"), text).unwrap();
        let mut a = vec!["evolve"];
        a.extend_from_slice(args);
        let (o, rc) = w.oo(&d, &a);
        assert_ne!(rc, 0, "VOID READING: `{name}` was not refused in a universe: {o}");
        assert!(w.snapshot(&d) == before, "`{name}` changed an existing universe");
    }
    let (st, _) = w.oo(&d, &["status"]);
    assert!(st.contains("y: 7"), "the universe or its working set is gone: {st}");
}
