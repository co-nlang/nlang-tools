// A spread at the root.
// Order: nlang-tools/docs/a_spread_at_the_root_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-093.  D111 (2026-10-18): a root's
// `...c` (`c` a name in the universe) reads the current `c` every time.
// D112 (2026-10-19): the root keeps `...c` as a row of its own — identity
// axis 9, keyed by the source's spelling, value empty; the store frame
// carries it; encoding 7; the fields it landed stay as readings (`b: c.b`)
// and the rows land again whenever the universe changes.
//
// ── What is wrong (v0.94.0) ─────────────────────────────────────────────
//
// A root spread lands at evolve, field by field. What the source gains
// later never lands, and a source that arrives later is never read:
//   c: { b: 2 }, ...c; then c: { d: 3 }       ⟹ _.d = _
//   ...c; then (next commit) c: { b: 2 }      ⟹ the first commit has
//                                                "Nothing to commit"; _.b = _
//   ...c; then (next evolve) c: { b: 2 }      ⟹ _.b = _
// `ComboVal::pending_spreads` is skipped by the store codec and by
// identity, and working-set members use the same codec, so `...c` itself
// never survives one process.
// A spread one level down, or in a coordinate's record, is right (Q-092).
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
        let scratch = nlang_interpreter::ScratchDir::new(&format!("root-spread-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo`, killing it after 60 s: (output, rc).
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

    /// Evolve `src` from a new file: (rc, output).
    fn evolve(&self, src: &str) -> (i32, String) {
        let n = fs::read_dir(&self.ws).map(|d| d.count()).unwrap_or(0);
        let f = format!("f{n}.n");
        fs::write(self.ws.join(&f), src).unwrap();
        let (o, rc) = self.oo(&["evolve", &f]);
        (rc, o)
    }

    /// Evolve `src`, then commit: "" when both succeed, else what failed.
    /// A reading taken after a failed commit is a reading of the previous
    /// universe, so the caller records this.
    fn commit(&self, src: &str) -> String {
        let (rc, o) = self.evolve(src);
        if rc != 0 {
            return format!("evolve rc={rc}: {}", squash(&o));
        }
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        if rc != 0 {
            return format!("commit rc={rc}: {}", squash(&o));
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
        squash(&s)
    }

    /// The root digest of HEAD ("" when there is none).
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

    fn members(&self) -> Vec<PathBuf> {
        fs::read_dir(self.ws.join(".oo/injections"))
            .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect())
            .unwrap_or_default()
    }

    fn encoding(&self) -> String {
        fs::read_to_string(self.ws.join(".oo/objects.format")).unwrap_or_default().trim().to_string()
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
    let failed = w.commit(src);
    if !failed.is_empty() {
        return failed;
    }
    w.root()
}

/// `c: { b: 2 }` and `...c`, committed: the root every route below reaches.
/// D112: the row `c` on axis 9, `b` as the reading `c.b`, `c` itself.
const R0: &str = "5d15b3a675337d6649a2790e5965f9de6644fba7590bf89544e5436542680d43";

// ── Red ──────────────────────────────────────────────────────────────────

/// r1 — What the source gains after the spread was committed is on the
/// root. v0.94.0: every added field reads `_`; a narrowed one is right.
#[test]
fn r1_a_field_the_source_gains_later_is_on_the_root() {
    let mut bad = Vec::new();
    let cases: [(&str, &str, &[(&str, &str)]); 4] = [
        ("c: { b: 2 }\n...c\n", "c: { d: 3 }\n", &[("_.d", "3"), ("_.b", "2"), ("_.c.d", "3")]),
        ("s: { j: @int }\n...s\n", "s: { k: 2 }\n", &[("_.k", "2"), ("_.j", "{{ %kind: #type %name: \"int\" }}")]),
        ("~p: { b: 2 }\n...~p\n", "~p: { d: 3 }\n", &[("_.d", "3"), ("_.b", "2")]),
        ("s: { t: { j: 1 } }\n...s.t\n", "s.t.k: 2\n", &[("_.k", "2"), ("_.j", "1")]),
    ];
    let mut n = 0;
    for (i, (a, b, reads)) in cases.iter().enumerate() {
        let w = Ws::new(&format!("r1-{i}"));
        check(&mut bad, &format!("commit {a:?}"), w.commit(a), "");
        check(&mut bad, &format!("commit {b:?} after {a:?}"), w.commit(b), "");
        for (e, want) in reads.iter() {
            n += 1;
            check(&mut bad, &format!("{a:?} then {b:?}: {e}"), w.eval(e), want);
        }
    }
    // A field that was already there and narrows afterwards stays a reading.
    let w = Ws::new("r1-narrow");
    check(&mut bad, "narrow 1", w.commit("s: { j: @int }\n...s\n"), "");
    check(&mut bad, "narrow 2", w.commit("s: { k: 2 }\n"), "");
    check(&mut bad, "narrow 3", w.commit("s: { j: 5 }\n"), "");
    check(&mut bad, "after s gained k and j narrowed: _.j", w.eval("_.j"), "5");
    check(&mut bad, "after s gained k and j narrowed: _.k", w.eval("_.k"), "2");
    verdict(bad, n + 5);
}

/// r2 — A source that arrives in a later commit is read. The spread alone
/// is something to commit. v0.94.0: "Nothing to commit", and `_.b` is `_`.
#[test]
fn r2_a_source_that_arrives_in_a_later_commit() {
    let mut bad = Vec::new();
    let w = Ws::new("r2");
    check(&mut bad, "commit ...c alone", w.commit("...c\n"), "");
    check(&mut bad, "commit c after ...c", w.commit("c: { b: 2 }\n"), "");
    check(&mut bad, "_.b", w.eval("_.b"), "2");
    check(&mut bad, "root after both", w.root(), R0);
    // two sources, two spreads, the sources in one later commit
    let w = Ws::new("r2-two");
    check(&mut bad, "commit ...a ...b", w.commit("...a\n...b\n"), "");
    check(&mut bad, "commit a b", w.commit("a: { k: 1 }\nb: { m: 2 }\n"), "");
    check(&mut bad, "_.k", w.eval("_.k"), "1");
    check(&mut bad, "_.m", w.eval("_.m"), "2");
    verdict(bad, 8);
}

/// r3 — A source that arrives in a later evolve of the same working set is
/// read, and members written apart (two evolves that did not see each
/// other) are landed at commit. v0.94.0: `_.b` is `_`.
#[test]
fn r3_a_source_that_arrives_in_a_later_evolve() {
    let mut bad = Vec::new();
    let w = Ws::new("r3");
    let (rc, o) = w.evolve("...c\n");
    check(&mut bad, "evolve ...c", format!("{rc} {}", squash(&o)), "0 ");
    check(&mut bad, "then c", w.commit("c: { b: 2 }\n"), "");
    check(&mut bad, "_.b", w.eval("_.b"), "2");
    check(&mut bad, "root", w.root(), R0);
    // members apart: the `...c` member is out of sight while `c` evolves
    let w = Ws::new("r3-apart");
    check(&mut bad, "seed", w.commit("x: 0\n"), "");
    let (rc, _) = w.evolve("...c\n");
    let first: Vec<PathBuf> = w.members();
    check(&mut bad, "the ...c evolve left one member", format!("{rc} {}", first.len()), "0 1");
    let aside = w.root.join("aside");
    fs::create_dir_all(&aside).unwrap();
    for m in &first {
        fs::rename(m, aside.join(m.file_name().unwrap())).unwrap();
    }
    let (rc, _) = w.evolve("c: { b: 2 }\n");
    for m in &first {
        fs::rename(aside.join(m.file_name().unwrap()), m).unwrap();
    }
    let (o, crc) = w.oo(&["commit", "-m", "a"]);
    check(&mut bad, "evolve c apart, commit", format!("{rc} {crc}"), "0 0");
    if crc != 0 {
        bad.push(format!("  commit said: {}", squash(&o)));
    }
    check(&mut bad, "apart: _.b", w.eval("_.b"), "2");
    verdict(bad, 8);
}

/// r4 — One root, however `c: { b: 2 }` and `...c` are written: one file
/// either order, two evolves either order, `...c` twice. Writing `...c`
/// again later changes nothing. v0.94.0: `eff89b3a…` (no row) in order,
/// `d582ef4e…` when the spread comes first in its own evolve.
#[test]
fn r4_one_root_for_every_route() {
    let mut bad = Vec::new();
    let one = [
        "c: { b: 2 }\n...c\n",
        "...c\nc: { b: 2 }\n",
        "c: { b: 2 }\n...c\n...c\n",
    ];
    for (i, src) in one.iter().enumerate() {
        check(&mut bad, &format!("one file {src:?}"), first_commit_root(src, &format!("r4-{i}")), R0);
    }
    for (i, (a, b)) in [("c: { b: 2 }\n", "...c\n"), ("...c\n", "c: { b: 2 }\n")].iter().enumerate() {
        let w = Ws::new(&format!("r4e{i}"));
        let (r1, _) = w.evolve(a);
        let (r2, _) = w.evolve(b);
        let (o, r3) = w.oo(&["commit", "-m", "a"]);
        check(&mut bad, &format!("two evolves {a:?} {b:?} rc"), format!("{r1} {r2} {r3}"), "0 0 0");
        if r3 != 0 {
            bad.push(format!("  commit said: {}", squash(&o)));
        }
        check(&mut bad, &format!("two evolves {a:?} {b:?} root"), w.root(), R0);
    }
    let w = Ws::new("r4-again");
    check(&mut bad, "commit", w.commit("c: { b: 2 }\n...c\n"), "");
    let (rc, _) = w.evolve("...c\n");
    let (o, _) = w.oo(&["commit", "-m", "again"]);
    check(&mut bad, "...c again: evolve rc", rc.to_string(), "0");
    check(&mut bad, "...c again: commit", o.contains("Nothing to commit").to_string(), "true");
    check(&mut bad, "...c again: root", w.root(), R0);
    verdict(bad, 10);
}

/// r5 — A source that arrives later and disagrees with the root is refused
/// at evolve, by key, and leaves nothing to commit. v0.94.0: accepted.
#[test]
fn r5_a_later_source_that_clashes_is_refused_at_evolve() {
    let mut bad = Vec::new();
    let cases: [(&str, &str, &str, &str); 2] = [
        ("b: 3\n...c\n", "c: { b: 2 }\n", "#conflict at b", "_.c.b"),
        ("c: { b: 2 }\n...c\nd: 5\n", "c: { d: 6 }\n", "#conflict at d", "_.c.d"),
    ];
    for (i, (a, b, said, e)) in cases.iter().enumerate() {
        let w = Ws::new(&format!("r5-{i}"));
        check(&mut bad, &format!("commit {a:?}"), w.commit(a), "");
        let before = w.members().len();
        let (rc, o) = w.evolve(b);
        check(&mut bad, &format!("{b:?} after {a:?}: rc"), rc.to_string(), "1");
        check(&mut bad, &format!("{b:?} after {a:?}: names the key"), o.contains(said).to_string(), "true");
        check(&mut bad, &format!("{b:?} after {a:?}: no member"), (w.members().len() - before).to_string(), "0");
        check(&mut bad, &format!("{b:?} after {a:?}: {e}"), w.eval(e), "_");
    }
    // the same inside one working set: two evolves
    let w = Ws::new("r5-ev");
    let (r1, _) = w.evolve("b: 3\n...c\n");
    let (r2, o) = w.evolve("c: { b: 2 }\n");
    check(&mut bad, "two evolves: rc", format!("{r1} {r2}"), "0 1");
    check(&mut bad, "two evolves: names the key", o.contains("#conflict at b").to_string(), "true");
    // agreeing is not a clash
    let w = Ws::new("r5-agree");
    check(&mut bad, "agree 1", w.commit("b: 2\n...c\n"), "");
    check(&mut bad, "agree 2", w.commit("c: { b: 2 }\n"), "");
    check(&mut bad, "agree: _.b", w.eval("_.b"), "2");
    verdict(bad, 13);
}

/// r6 — Encoding 7. A new store declares it. A store that declares 6 reads
/// and writes as before and refuses a root spread of a universe name by
/// name, writing nothing; `migrate` says whom it locks out and advances
/// it; then the spread lands. v0.94.0: `encoding=6`, and the spread is
/// accepted (and forgotten).
#[test]
fn r6_encoding_seven() {
    let mut bad = Vec::new();
    let w = Ws::new("r6-new");
    check(&mut bad, "new store", w.commit("x: 0\n"), "");
    check(&mut bad, "new store declares", w.encoding(), "encoding=8"); // AMENDED 2026-10-21 for Q-097 (D114)
    let w = Ws::new("r6-old");
    check(&mut bad, "seed", w.commit("c: { b: 2 }\n"), "");
    fs::write(w.ws.join(".oo/objects.format"), "encoding=6\n").unwrap();
    check(&mut bad, "ordinary write on encoding=6", w.commit("x: 1\n"), "");
    let before = w.members().len();
    let (rc, o) = w.evolve("...c\n");
    check(&mut bad, "...c on encoding=6: rc", rc.to_string(), "1");
    check(&mut bad, "...c on encoding=6: names encoding 7", o.contains("encoding=7").to_string(), "true");
    check(&mut bad, "...c on encoding=6: names migrate", o.contains("oo migrate --grant migrate").to_string(), "true");
    check(&mut bad, "...c on encoding=6: no member", (w.members().len() - before).to_string(), "0");
    let (o, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    check(&mut bad, "migrate rc", rc.to_string(), "0");
    check(&mut bad, "migrate names whom it locks out", o.contains("oo v0.85.0 through v0.97.0").to_string(), "true"); // AMENDED 2026-10-21 for Q-097 (D114)
    check(&mut bad, "after migrate declares", w.encoding(), "encoding=8"); // AMENDED 2026-10-21 for Q-097 (D114)
    check(&mut bad, "...c after migrate", w.commit("...c\n"), "");
    check(&mut bad, "...c after migrate: _.b", w.eval("_.b"), "2");
    verdict(bad, 13);
}

// ── Guards (green today, must stay green) ─────────────────────────────────

/// g1 — A root without a spread of a universe name keeps every byte: the
/// identity red line, spreads that read nothing in the universe (they land
/// as answers), a coordinate's and a nested record's spread (Q-092), and
/// the readings a spread lands, written out by hand.
#[test]
fn g1_roots_without_a_row_do_not_move() {
    let mut bad = Vec::new();
    let cases: [(&str, &str); 8] = [
        ("x: 0\n", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        ("...~%Math\n", "42eadc23e9f62a7d379800d33ed7b716f0b2471ef86df8f923679d91e020fe14"),
        ("...{ j: 5 }\n", "2ab0bdf24ff774cc7b769ba9981bb9bc53a2f46912b7c5466aa481b2b7582615"),
        ("c: { b: 2 }\nr: { a: 1, ...c }\n", "f6be66e9989ce17b864992b6223061d8503f196a74b26c6311f2921d7e30ab13"),
        ("c: { b: 2 }\nr: { x: { a: 1, ...c } }\n", "125589e26e3677bbf7a05ab851876f3adb303fefd8a5fc377be264468862921c"),
        ("c: { b: 2 }\nb: c.b\n", "eff89b3a87264fdba1ac9b0ed6a16ef0423645210c93a36b2148adff9146ea85"),
        ("s: { j: 1 }\nj: s.j\n", "3c5c33274660ede1e845280deed19f644195a8da8ebc65b8ac6d744bf23fadf2"),
        ("~p: { b: 2 }\nq: ~p\n", "aabf08268f5a9fddba1aa6308e5c146c188759a7e44a363586a4804bebc00d9b"),
    ];
    for (i, (src, want)) in cases.iter().enumerate() {
        check(&mut bad, &format!("{src:?}"), first_commit_root(src, &format!("g1-{i}")), want);
    }
    verdict(bad, cases.len());
}

/// g2 — A spread that reads nothing in the universe needs no row, so a
/// store that declares encoding 6 takes it.
#[test]
fn g2_a_spread_that_reads_nothing_needs_no_row() {
    let mut bad = Vec::new();
    for (i, (src, e, want)) in [("...~%Math\n", "_./add (1, 2)", "3"), ("...{ j: 5 }\n", "_.j", "5")].iter().enumerate() {
        let w = Ws::new(&format!("g2-{i}"));
        check(&mut bad, "seed", w.commit("x: 0\n"), "");
        fs::write(w.ws.join(".oo/objects.format"), "encoding=6\n").unwrap();
        check(&mut bad, &format!("{src:?} on encoding=6"), w.commit(src), "");
        check(&mut bad, &format!("{src:?} on encoding=6: {e}"), w.eval(e), want);
    }
    verdict(bad, 6);
}

/// g3 — What is right today stays right: a narrowed source is read, and a
/// clash written in one file is refused at evolve.
#[test]
fn g3_narrowing_and_a_clash_in_one_file() {
    let mut bad = Vec::new();
    let w = Ws::new("g3-narrow");
    check(&mut bad, "narrow 1", w.commit("s: { j: @int }\n...s\n"), "");
    check(&mut bad, "narrow 2", w.commit("s: { j: 5 }\n"), "");
    check(&mut bad, "narrowed: _.j", w.eval("_.j"), "5");
    let w = Ws::new("g3-clash");
    let (rc, _) = w.evolve("c: { b: 2 }\nb: 3\n...c\n");
    check(&mut bad, "clash in one file: rc", rc.to_string(), "1");
    verdict(bad, 4);
}

// ── Added at acceptance (2026-10-19), repair round R-1 ───────────────────

/// `b: 2`, `...c`, `c: { b: 2 }`, and `x: 1`: one root however they are
/// split. `b` holds the answer the spread met, not the reading.
const R1: &str = "a75ab440b717bcc169d3b59b7c6d272882f674ba7d47c29bce654e995ee17888";

/// r7 — A write that does not touch a spread's keys does not change what
/// those keys hold. A key that already holds something other than the
/// spread's reading — an answer the spread met in the same write, or a
/// value `--pin` wrote — keeps it when the row lands again; the order of
/// an unrelated write does not matter. Delivery 85d865b: an unrelated
/// `x: 1` written later lands `b` again as a reading (`c8f03993…`).
#[test]
fn r7_an_unrelated_write_leaves_a_spread_key_as_it_was() {
    let mut bad = Vec::new();
    let w = Ws::new("r7-later");
    check(&mut bad, "commit b, ...c, c", w.commit("b: 2\n...c\nc: { b: 2 }\n"), "");
    check(&mut bad, "then x", w.commit("x: 1\n"), "");
    check(&mut bad, "x later: root", w.root(), R1);
    check(&mut bad, "x later: _.b", w.eval("_.b"), "2");
    check(&mut bad, "one commit", first_commit_root("b: 2\n...c\nc: { b: 2 }\nx: 1\n", "r7-one"), R1);
    let w = Ws::new("r7-first");
    check(&mut bad, "commit c, x", w.commit("c: { b: 2 }\nx: 1\n"), "");
    check(&mut bad, "then b, ...c", w.commit("b: 2\n...c\n"), "");
    check(&mut bad, "x first: root", w.root(), R1);
    // `...c` written under `--pin`, then `x: 1`; and the other order
    for (i, steps) in [["pin", "x"], ["x", "pin"]].iter().enumerate() {
        let w = Ws::new(&format!("r7-pin{i}"));
        check(&mut bad, "seed c", w.commit("c: { b: 2 }\n"), "");
        for s in steps.iter() {
            let (rc, o) = if *s == "pin" {
                let n = fs::read_dir(&w.ws).map(|d| d.count()).unwrap_or(0);
                fs::write(w.ws.join(format!("p{n}.n")), "...c\n").unwrap();
                let (o, rc) = w.oo(&["evolve", "--pin", "--grant", "pin", &format!("p{n}.n")]);
                if rc != 0 {
                    (rc, o)
                } else {
                    let (o, rc) = w.oo(&["commit", "--grant", "pin", "-m", "p"]);
                    (rc, o)
                }
            } else {
                let o = w.commit("x: 1\n");
                (if o.is_empty() { 0 } else { 1 }, o)
            };
            check(&mut bad, &format!("{steps:?} step {s}"), format!("{rc}"), "0");
            if rc != 0 {
                bad.push(format!("  said: {}", squash(&o)));
            }
        }
        check(&mut bad, &format!("{steps:?}: root"), w.root(), R1);
        check(&mut bad, &format!("{steps:?}: _.b"), w.eval("_.b"), "2");
    }
    verdict(bad, 14);
}

// ── Added at acceptance of R-1 (2026-10-19), repair round R-2 ────────────

/// r8 — A source that arrives later and disagrees with what a key holds is
/// refused at evolve, by key, whatever the key holds: an answer (r5), a
/// reading of another coordinate, an alias. One file refuses it today; two
/// commits and two evolves must too. Agreement is not a clash. R-1
/// delivery 332dff8: the reading and the alias are accepted, `_.b` stays
/// `3` beside `_.c.b` `2` (85d865b stored ⊥ over two commits).
#[test]
fn r8_a_later_source_that_clashes_with_a_reading_is_refused() {
    let mut bad = Vec::new();
    let held = ["d: { b: 3 }\nb: d.b\n...c\n", "n: 3\nb: n\n...c\n"];
    for (i, a) in held.iter().enumerate() {
        let w = Ws::new(&format!("r8-cm{i}"));
        check(&mut bad, &format!("commit {a:?}"), w.commit(a), "");
        let before = w.members().len();
        let (rc, o) = w.evolve("c: { b: 2 }\n");
        check(&mut bad, &format!("c after {a:?}: rc"), rc.to_string(), "1");
        check(&mut bad, &format!("c after {a:?}: names the key"), o.contains("#conflict at b").to_string(), "true");
        check(&mut bad, &format!("c after {a:?}: no member"), (w.members().len() - before).to_string(), "0");
        check(&mut bad, &format!("c after {a:?}: _.b"), w.eval("_.b"), "3");
        let w = Ws::new(&format!("r8-ev{i}"));
        let (r1, _) = w.evolve(a);
        let (r2, o) = w.evolve("c: { b: 2 }\n");
        check(&mut bad, &format!("two evolves {a:?}: rc"), format!("{r1} {r2}"), "0 1");
        check(&mut bad, &format!("two evolves {a:?}: names the key"), o.contains("#conflict at b").to_string(), "true");
        let (rc, _) = Ws::new(&format!("r8-one{i}")).evolve(&format!("{a}c: {{ b: 2 }}\n"));
        check(&mut bad, &format!("one file {a:?}: rc"), rc.to_string(), "1");
    }
    let w = Ws::new("r8-agree");
    check(&mut bad, "agree 1", w.commit("d: { b: 2 }\nb: d.b\n...c\n"), "");
    check(&mut bad, "agree 2", w.commit("c: { b: 2 }\n"), "");
    check(&mut bad, "agree: _.b", w.eval("_.b"), "2");
    verdict(bad, 17);
}
