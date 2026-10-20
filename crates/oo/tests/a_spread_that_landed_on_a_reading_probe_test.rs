// A spread that landed on a reading.
// Order: nlang-tools/docs/a_spread_that_landed_on_a_reading_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-095.  D110 (SPEC_10 §2.3.1,
// MUST): storing syntax must not make a clash late — a write that clashes
// with what a coordinate holds is refused at evolve, judged on the answers
// of both sides.
//
// ── What is wrong (v0.94.0 through v0.96.0) ─────────────────────────────
//
// Commit `d: { b: 3 }`, `b: d.b`. In a later commit write `c: { b: 2 }`,
// `...c`. The evolve is accepted, the commit says `#conflict at b` and
// succeeds, and `_.b` is ⊥. The spread's first landing checks the root
// (G2-S) with a meet that cannot see the universe: the held reading `d.b`
// reads `_` there and clashes with nothing. v0.93.0 refuses it. The same
// clash written as a field, in one file, over two evolves, at a coordinate
// or nested, or relanded later, is refused today.
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("landed-on-reading-{tag}"));
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

    /// Evolve `src`, then commit: "" when both succeed.
    fn commit(&self, src: &str) -> String {
        let (rc, o) = self.evolve(src);
        if rc != 0 {
            return format!("evolve rc={rc}: {}", o.chars().take(200).collect::<String>());
        }
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        if rc != 0 {
            return format!("commit rc={rc}: {}", squash(&o).chars().take(200).collect::<String>());
        }
        String::new()
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

/// What `b` holds, how it reads, and the source value that clashes with it.
const HELD: [(&str, &str, &str, &str); 4] = [
    ("reading", "d: { b: 3 }\nb: d.b\n", "_.b", "2"),
    ("alias", "n: 3\nb: n\n", "_.b", "2"),
    ("apply", "n: 3\nb: n + 0\n", "_.b", "2"),
    ("nested", "d: { b: 3 }\nb: { k: d.b }\n", "_.b.k", "{ k: 2 }"),
];

/// A clash refused at evolve: refused, named by key, nothing moved.
fn refused(bad: &mut Vec<String>, label: &str, w: &Ws, got: String, seed: &str, read: &str) {
    check(bad, &format!("{label}: evolve refused"), got.starts_with("evolve rc=1").to_string(), "true");
    check(bad, &format!("{label}: names the key"), got.contains("#conflict at b").to_string(), "true");
    check(bad, &format!("{label}: root"), (w.root() == seed).to_string(), "true");
    check(bad, &format!("{label}: {read}"), w.eval(read), "3");
}

// ── Red ──────────────────────────────────────────────────────────────────

/// r1 — A root spread that lands for the first time in a later commit, over
/// a key the root holds as a recipe (a reading, an alias, an application, a
/// record holding a reading), and disagrees: refused at evolve, by key,
/// whichever way it arrives — source and spread together, `...c` alone over
/// a committed `c`, the source itself a reading. v0.96.0: accepted, the
/// commit stores ⊥ at `b`.
#[test]
fn r1_a_first_landing_over_a_held_recipe_is_refused() {
    let mut bad = Vec::new();
    for (name, held, read, v) in HELD {
        let w = Ws::new(&format!("r1-{name}-src"));
        check(&mut bad, &format!("{name}: seed"), w.commit(held), "");
        let seed = w.root();
        let got = w.commit(&format!("c: {{ b: {v} }}\n...c\n"));
        refused(&mut bad, &format!("{name}, source and spread"), &w, got, &seed, read);

        let w = Ws::new(&format!("r1-{name}-alone"));
        check(&mut bad, &format!("{name}: seed with c"), w.commit(&format!("{held}c: {{ b: {v} }}\n")), "");
        let seed = w.root();
        let got = w.commit("...c\n");
        refused(&mut bad, &format!("{name}, ...c alone"), &w, got, &seed, read);

        let w = Ws::new(&format!("r1-{name}-reading"));
        check(&mut bad, &format!("{name}: seed again"), w.commit(held), "");
        let seed = w.root();
        let got = w.commit(&format!("e: {{ b: {v} }}\nc: {{ b: e.b }}\n...c\n"));
        refused(&mut bad, &format!("{name}, source a reading"), &w, got, &seed, read);
    }
    let w = Ws::new("r1-two-evolves");
    check(&mut bad, "two evolves: seed", w.commit("d: { b: 3 }\nb: d.b\n"), "");
    let (rc, _) = w.evolve("c: { b: 2 }\n");
    check(&mut bad, "two evolves: c", rc.to_string(), "0");
    let (rc, o) = w.evolve("...c\n");
    check(&mut bad, "two evolves: ...c refused", rc.to_string(), "1");
    check(&mut bad, "two evolves: names the key", o.contains("#conflict at b").to_string(), "true");
    verdict(bad, 64);
}

/// r2 — The check is the universe's bookkeeping (D113): it reads what the
/// held recipes read and spends no fuel. 300 aliases committed; with the
/// staged fuel at 1000, a spread of 300 keys clashing at the 6th or the
/// 251st is refused by key. A check judged where the universe is but
/// metered refuses the 6th and commits ⊥ at the 251st.
#[test]
fn r2_a_late_clash_is_refused_without_counting() {
    let mut bad = Vec::new();
    let held: String = (0..300).map(|i| format!("n{i}: {i}\nb{i}: n{i}\n")).collect();
    let spread = |clash: Option<usize>| {
        let fields: Vec<String> = (0..300).map(|i| format!("b{i}: {}", if Some(i) == clash { 7 } else { i })).collect();
        format!("~%Config.fuel: 1000\nc: {{ {} }}\n...c\n", fields.join(", "))
    };
    for at in [5, 250] {
        let w = Ws::new(&format!("r2-clash{at}"));
        check(&mut bad, &format!("clash at b{at}: seed"), w.commit(&held), "");
        let seed = w.root();
        let got = w.commit(&spread(Some(at)));
        check(&mut bad, &format!("clash at b{at}: evolve refused"), got.starts_with("evolve rc=1").to_string(), "true");
        check(&mut bad, &format!("clash at b{at}: names the key"), got.contains(&format!("#conflict at b{at}")).to_string(), "true");
        check(&mut bad, &format!("clash at b{at}: root"), (w.root() == seed).to_string(), "true");
        check(&mut bad, &format!("clash at b{at}: _.b{at}"), w.eval(&format!("_.b{at}")), &at.to_string());
    }
    let w = Ws::new("r2-agree");
    check(&mut bad, "agree: seed", w.commit(&held), "");
    check(&mut bad, "agree: spread", w.commit(&spread(None)), "");
    for (e, want) in [("_.b5", "5"), ("_.b250", "250"), ("_.c.b250", "250")] {
        check(&mut bad, &format!("agree: {e}"), w.eval(e), want);
    }
    verdict(bad, 15);
}

// ── Guards (green today, must stay green) ─────────────────────────────────

/// g1 — A spread that agrees with what the root holds lands as today, byte
/// for byte; a clash with a held answer, a field clash and a clash at a
/// coordinate are refused as today.
#[test]
fn g1_agreement_lands_and_other_clashes_stay_refused() {
    let mut bad = Vec::new();
    let agree = [
        ("reading", "d: { b: 3 }\nb: d.b\n", "c: { b: 3 }\n...c\n", "_.b", "3", "58cfac64f9581104702cfc72b4e2579dbf45f7c8999506504a439cd91f07fc27"),
        ("alias", "n: 3\nb: n\n", "c: { b: 3 }\n...c\n", "_.b", "3", "96c7b48985376d6763a4fc6ea462a14c8cb82c934bea75212bb36e69f4a9d17e"),
        ("nested", "d: { b: 3 }\nb: { k: d.b }\n", "c: { b: { k: 3 } }\n...c\n", "_.b.k", "3", "8d79b9704d65714e436d2770d0222164bb8c8b5ccc3269463ccd4a2c2c772b0f"),
    ];
    for (name, held, later, read, want, root) in agree {
        let w = Ws::new(&format!("g1-{name}"));
        check(&mut bad, &format!("{name}: seed"), w.commit(held), "");
        check(&mut bad, &format!("{name}: agreeing spread"), w.commit(later), "");
        check(&mut bad, &format!("{name}: {read}"), w.eval(read), want);
        check(&mut bad, &format!("{name}: root"), w.root(), root);
    }
    let others = [
        ("answer held", "b: 3\n", "c: { b: 2 }\n...c\n", "_.b"),
        ("field over a reading", "d: { b: 3 }\nb: d.b\n", "b: 2\n", "_.b"),
        ("coordinate spread", "d: { b: 3 }\nr: { b: d.b }\n", "c: { b: 2 }\nr: { ...c }\n", "_.r.b"),
    ];
    for (name, held, later, read) in others {
        let w = Ws::new(&format!("g1-{}", name.replace(' ', "-")));
        check(&mut bad, &format!("{name}: seed"), w.commit(held), "");
        let got = w.commit(later);
        check(&mut bad, &format!("{name}: refused"), got.starts_with("evolve rc=1").to_string(), "true");
        check(&mut bad, &format!("{name}: {read}"), w.eval(read), "3");
    }
    verdict(bad, 21);
}
