// A cause nobody could read.
// Order: nlang-tools/docs/a_cause_nobody_could_read_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-075.  Ruling: meta/oo/STATUS.md D93.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// REAL_02 §5.1.1: a state an engine cannot judge must not be reported as
// another state it can. A stored `_|_` carries its cause as a tag
// (`~%__nlang_bottom: #conflict`), and by D65 the cause is not part of the
// value's address. Measured on v0.76.0: rewrite that tag in a stored object
// to one this engine does not know (`#from_the_future`), or to something
// that is not a tag at all (`42`, `"conflict"`), and the value reads back as
// `_|_ ;; %cause: #conflict` — the integrity check does not notice (the
// address does not cover the cause), and the decoder's fallback is
// `_ => BottomCause::Conflict`. A cause written by another engine, a newer
// one or a second implementation, would be read the same way.
//
// D93 (user, 2026-10-05, 甲): a cause this engine cannot read is never
// reported as a cause it can read. The encoder and the decoder are kept
// exhaustive by the compiler (no `_ =>` tail).
// D94 (AMENDED 2026-10-05 at Q-075 R-1 (D94)): the answer is a new cause, `#unrecognized_cause` — not
// `#object_undecodable`, which REAL_03 §6.6 defines as an integrity verdict
// ("cannot decode, integrity unknown"); here the address verified and only
// the cause is unreadable (D65). The acceptor's order named the wrong tag.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Cause tags are the TAG_REGISTRY's names, so this file reads them. It
// rewrites only the cause tag inside a stored object (which, by D65, leaves
// the address unchanged); nothing else is touched. The delivery may NOT
// edit this file. `rustfmt` must not touch it.
//
// Baseline measured 2026-10-05 on dev 499315f / oo v0.76.0: see the order.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const MARK: &str = "~%__nlang_bottom: #conflict";

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("cause-nobody-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    fn oo(&self, args: &[&str]) -> (String, i32) {
        let o = Command::new(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
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

    fn ok(&self, args: &[&str]) -> String {
        let (o, rc) = self.oo(args);
        assert_eq!(rc, 0, "setup `oo {}`: {o}", args.join(" "));
        o
    }

    fn committed(tag: &str, text: &str) -> Self {
        let w = Ws::new(tag);
        fs::write(w.ws.join("a.n"), text).unwrap();
        w.ok(&["evolve", "a.n"]);
        w.oo(&["commit", "-m", "a"]);
        w
    }

    /// Every file under `dir` whose bytes hold `MARK`.
    fn holding(&self, dir: &str) -> Vec<PathBuf> {
        let mut out = Vec::new();
        walk(&self.ws.join(".oo").join(dir), &mut out);
        out.into_iter().filter(|p| fs::read_to_string(p).map(|t| t.contains(MARK)).unwrap_or(false)).collect()
    }

    /// Rewrite the cause tag in every such file under `dir`.
    fn rewrite(&self, dir: &str, cause: &str) -> usize {
        let files = self.holding(dir);
        for p in &files {
            let t = fs::read_to_string(p).unwrap();
            let mut perm = fs::metadata(p).unwrap().permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perm.set_readonly(false);
            fs::set_permissions(p, perm).unwrap();
            fs::write(p, t.replace(MARK, &format!("~%__nlang_bottom: {cause}"))).unwrap();
        }
        files.len()
    }
}

fn walk(p: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(d) = fs::read_dir(p) {
        for e in d.flatten() {
            let q = e.path();
            if q.is_dir() {
                walk(&q, out);
            } else {
                out.push(q);
            }
        }
    }
}

const FOREIGN: &[&str] = &["#from_the_future", "42", "\"conflict\"", "{ x: 1 }"];

/// AMENDED 2026-10-05 at Q-075 R-1 (D94): the unreadable cause is named as such, and as nothing else.
fn unrecognized(o: &str) -> bool {
    o.contains("#unrecognized_cause") && !o.contains("#conflict") && !o.contains("#object_undecodable")
}

// ── Red ──────────────────────────────────────────────────────────────────

/// r1 — a committed `_|_` whose stored cause this engine cannot read is
/// reported as `#unrecognized_cause`, never as `#conflict` and never as the
/// integrity verdict `#object_undecodable` (D94).
#[test]
fn r1_a_committed_foreign_cause_is_undecodable() {
    let mut wrong = Vec::new();
    for (i, cause) in FOREIGN.iter().enumerate() {
        let w = Ws::committed(&format!("r1-{i}"), "a: 1\nb: 1 & 2\n");
        let (before, _) = w.oo(&["eval", "_.b"]);
        assert!(before.contains("#conflict"), "VOID READING: {before}");
        assert!(w.rewrite("objects", cause) >= 1, "VOID READING: no stored object holds {MARK}");
        let (o, _) = w.oo(&["eval", "_.b"]);
        if !unrecognized(&o) { // AMENDED 2026-10-05 at Q-075 R-1 (D94)
            wrong.push(format!("{cause}: {o}"));
        }
    }
    assert!(wrong.is_empty(), "foreign causes read as a cause this engine knows:\n{}", wrong.join("\n"));
}

/// r2 — the same for a staged `_|_` (the working set is durable too).
#[test]
fn r2_a_staged_foreign_cause_is_undecodable() {
    let w = Ws::new("r2");
    fs::write(w.ws.join("a.n"), "a: 1\nb: 1 & 2\n").unwrap();
    w.ok(&["evolve", "a.n"]);
    let (before, _) = w.oo(&["status"]);
    assert!(before.contains("#conflict"), "VOID READING: {before}");
    assert!(w.rewrite("injections", "#from_the_future") >= 1, "VOID READING: no injection holds {MARK}");
    let (o, _) = w.oo(&["status"]);
    assert!(unrecognized(&o), "a staged foreign cause: {o}"); // AMENDED 2026-10-05 at Q-075 R-1 (D94)
}

/// r3 — every surface agrees: `run --format` over the committed root says
/// what `eval` says.
#[test]
fn r3_every_surface_says_undecodable() {
    let w = Ws::committed("r3", "a: 1\nb: 1 & 2\n");
    w.rewrite("objects", "#from_the_future");
    fs::write(w.ws.join("q.n"), "q: _.b\n").unwrap();
    let mut wrong = Vec::new();
    for args in [&["eval", "_.b"][..], &["run", "--observe", "q", "q.n"][..], &["run", "--observe", "b", "q.n"][..]] {
        let (o, _) = w.oo(args);
        if !unrecognized(&o) { // AMENDED 2026-10-05 at Q-075 R-1 (D94)
            wrong.push(format!("`oo {}`: {o}", args.join(" ")));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

// ── Green ────────────────────────────────────────────────────────────────

/// g1 — causes this engine wrote read back as written.
#[test]
fn g1_known_causes_read_back_as_written() {
    let w = Ws::committed("g1", "b: 1 & 2\nc: c + 1\nt: ~%Math./add (1, \"x\")\n");
    for (k, cause) in [("b", "#conflict"), ("c", "#divergent"), ("t", "#conflict")] {
        let (o, _) = w.oo(&["eval", &format!("_.{k}")]);
        assert!(o.contains(cause) && !o.contains("#object_undecodable") && !o.contains("#unrecognized_cause"), "`_.{k}` did not read back {cause}: {o}"); // AMENDED 2026-10-05 at Q-075 R-1 (D94)
    }
}

/// g2 — only the cause is unreadable: the rest of the value and of the
/// root read normally, and nothing reports an integrity failure.
#[test]
fn g2_only_the_cause_is_unreadable() {
    let w = Ws::committed("g2", "a: 41\nb: 1 & 2\n");
    w.rewrite("objects", "#from_the_future");
    let (o, rc) = w.oo(&["eval", "_.a + 1"]);
    assert!(rc == 0 && o.contains("42"), "the rest of the root no longer reads: {o}");
    assert!(!o.contains("caid_mismatch") && !o.contains("integrity"), "{o}");
}

/// g3 — a known cause swapped for another known cause reads as written
/// (D65: the cause is not part of the address; this arc does not change
/// that).
#[test]
fn g3_a_known_cause_is_read_as_written() {
    let w = Ws::committed("g3", "a: 1\nb: 1 & 2\n");
    w.rewrite("objects", "#divergent");
    let (o, _) = w.oo(&["eval", "_.b"]);
    assert!(o.contains("#divergent"), "a readable cause was not read: {o}");
}
