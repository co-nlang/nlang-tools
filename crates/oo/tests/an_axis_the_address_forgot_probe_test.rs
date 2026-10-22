// An axis the address forgot.
// Order: nlang-tools/docs/an_axis_the_address_forgot_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-083.  Ruling: D106 (甲 an axis
// byte on every entry; 乙 an epoch-1 store is read with the frozen epoch-1
// encoding, O63's format gate; not bundled with O88).
// Spec: REAL_03 §6.2/§6.3 (the identity encoding), §6.6 (a read recomputes
// the address), §6.7 second MUST NOT (two values that `=` tells apart never
// share an address), §6.8.1 (the container declares the encoding).
//
// ── What is wrong (v0.84.0) ──────────────────────────────────────────────
//
// `bn_serial::serialize_combo` sorts entries by (axis, name) and then writes
// only the name. So `{ int: 1 }`, `{ @int: 1 }` and `{ ~int: 1 }` are one
// address; so are `m:`/`%m:`, `f:`/`/f:`, `h:`/`~h:`. `=` tells each pair
// apart. Three answers are silently wrong:
//   * `{ int: 1 } & { @int: 1 }` ⟹ `{ int: 1 }` (unify returns early on
//     equal digests), and the reverse order ⟹ `{ @int: 1 }`;
//   * `--pin` from `t: { int: 1 }` to `t: { @int: 1 }` commits "successfully"
//     and `_.t` still reads `{ int: 1 }` (the new root's address is the old
//     root's);
//   * a table with `@{ { m: 1 } }:` and `@{ { %m: 1 } }:` has ONE branch
//     (branches are named by the pattern's digest): applied, ⊥ #conflict.
//
// The invariant: values that `=` tells apart have different addresses, in a
// store this engine creates — and a store an epoch-1 engine wrote still opens,
// answers, keeps its history, and catches tampering.
//
// The epoch-1 stores are `tests/fixtures/epoch1_repo` and
// `tests/fixtures/epoch1_tables_repo` (real `oo v0.84.0`).
//
// ACCEPTOR (Q-083 R-1, 2026-10-12): r5 as first shipped pinned the work
// order's calibration point 3 — "an encoding=5 store is redeclared
// encoding=6 before the first epoch-2 write". That point broke REAL_02
// §5.1.1 (a write must not change the declarations it finds; advancing one
// is an explicit, priced act; a store declaring the older form receives
// that form). The acceptor's error, not the delivery's. r5 is narrowed to
// the store this engine creates; g6, g7, r7, r9, r10 pin the corrected rule; g7 (first written as r8) is green on v0.84.0, so it is a guard.
//
// The delivery may NOT edit this file or the fixture. `rustfmt` must not
// touch it.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("axis-forgot-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// A workspace holding a copy of the epoch-1 store.
    fn epoch1(tag: &str) -> Self {
        let w = Ws::new(tag);
        copy_dir(&fixture(), &w.ws.join(".oo"));
        w
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

    /// `oo eval`, whitespace collapsed to single spaces.
    fn eval(&self, e: &str) -> String {
        squash(&self.oo(&["eval", e]).0)
    }

    /// Evolve then commit `src`. Both must succeed: a reading taken after a
    /// failed commit is a reading of the previous universe.
    fn commit(&self, src: &str) {
        fs::write(self.ws.join("a.n"), src).unwrap();
        let (o, rc) = self.oo(&["evolve", "a.n"]);
        assert_eq!(rc, 0, "evolve {src:?}: {o}");
        let (o, rc) = self.oo(&["commit", "-m", "a"]);
        assert_eq!(rc, 0, "commit {src:?}: {o}");
    }

    fn head(&self) -> String {
        fs::read_to_string(self.ws.join(".oo").join("HEAD")).unwrap_or_default().trim().to_string()
    }

    /// The root digest of HEAD.
    fn root(&self) -> String {
        let (o, _) = self.oo(&["inspect", &self.head()]);
        o.lines()
            .find(|l| l.contains("root:"))
            .and_then(|l| l.rsplit(':').next())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    fn objects_format(&self) -> String {
        fs::read_to_string(self.ws.join(".oo").join("objects.format")).unwrap_or_default().trim().to_string()
    }

    /// The on-disk object for a 64-hex digest.
    fn object_path(&self, digest: &str) -> PathBuf {
        self.ws.join(".oo/objects/sha256").join(&digest[..2]).join(&digest[2..])
    }
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/epoch1_repo/oo_dir")
}

fn tables_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/epoch1_tables_repo/oo_dir")
}

fn read_decl(w: &Ws, name: &str) -> String {
    fs::read_to_string(w.ws.join(".oo").join(name)).unwrap_or_default().trim().to_string()
}


fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let dst = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &dst);
        } else {
            fs::copy(entry.path(), &dst).unwrap();
        }
    }
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The epoch-1 store's two commits and its standard root, as `oo v0.84.0`
/// wrote them (fixture README).
const EPOCH1_FIRST: &str = "2858af651809a9aa86cddd06558846d326fbb15a196d874a21ca7559c06f93a3";
const EPOCH1_HEAD: &str = "f8e94c4719238893d5662b01365f8c3dd0300c2e449ea01504f2b95ca495e7fd";
const EPOCH1_STANDARD: &str = "7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911";

/// What `oo v0.84.0` answers in that store (fixture README).
const EPOCH1_ANSWERS: &[(&str, &str)] = &[
    ("_.t", "{ int: 1 }"),
    ("_.k.t \"s\"", "9"),
    ("_.k.t 5", "1"),
    ("_.f 1", "2"),
    ("_.u", "1 | 2"),
    ("_.x", "0"),
];

// ── Red: values that `=` tells apart have different addresses ────────────

/// Every axis a key can be written on is its own address. The pairs are the
/// row's; the list is the whole family, compared all against all.
#[test]
fn r1_each_axis_is_its_own_address() {
    let sources = [
        "t: { int: 1 }",
        "t: { @int: 1 }",
        "t: { ~int: 1 }",
        "t: { m: 1 }",
        "t: { %m: 1 }",
        "t: { f: (x -> x) }",
        "t: { /f: (x -> x) }",
        "t: { h: 3 }",
        "t: { ~h: 3 }",
        "h: 3",
        "~h: 3",
        "t: { a: 1, b: 2 }",
        "t: { a: 1, ~b: 2 }",
    ];
    let mut seen: Vec<(String, &str)> = Vec::new();
    let mut shared = Vec::new();
    for (i, src) in sources.iter().enumerate() {
        let w = Ws::new(&format!("r1-{i}"));
        w.commit(&format!("{src}\n"));
        let root = w.root();
        assert_eq!(root.len(), 64, "{src}: no root read back ({root:?})");
        if let Some((_, other)) = seen.iter().find(|(r, _)| *r == root) {
            shared.push(format!("  {other:?} and {src:?} share {}…", &root[..8]));
        }
        seen.push((root, src));
    }
    assert!(shared.is_empty(), "two values, one address:\n{}", shared.join("\n"));
}

/// A meet keeps both fields, in either order.
#[test]
fn r2_a_meet_keeps_both_axes() {
    let w = Ws::new("r2");
    for (a, b, k1, k2) in [("{ int: 1 }", "{ @int: 1 }", "int:", "@int:"), ("{ m: 1 }", "{ %m: 1 }", "m:", "%m:")] {
        for (x, y) in [(a, b), (b, a)] {
            let got = w.eval(&format!("{x} & {y}"));
            let keys: Vec<&str> = got.split_whitespace().filter(|t| t.ends_with(':')).collect();
            assert!(keys.contains(&k1) && keys.contains(&k2), "{x} & {y} ⟹ {got}");
        }
        assert_eq!(w.eval(&format!("({a} & {b}) = ({b} & {a})")), "#true", "{a} & {b} commutes");
    }
}

/// A pin that moves a field to another axis lands.
#[test]
fn r3_a_pin_across_axes_lands() {
    let w = Ws::new("r3");
    w.commit("t: { int: 1 }\n");
    let before = w.root();
    fs::write(w.ws.join("b.n"), "t: { @int: 1 }\n").unwrap();
    let (o, rc) = w.oo(&["evolve", "--pin", "--grant", "pin", "b.n"]);
    assert_eq!(rc, 0, "pin evolve: {o}");
    let (o, rc) = w.oo(&["commit", "--grant", "pin", "-m", "b"]);
    assert_eq!(rc, 0, "pin commit: {o}");
    assert_eq!(w.eval("_.t"), "{ @int: 1 }", "the pinned value must be what HEAD holds");
    assert_ne!(w.root(), before, "the pinned root is the old root's address");
}

/// Two patterns that differ only by an axis are two branches. The control
/// is the same table with `%m: 2`: two branches today, and the answer the
/// table must give.
#[test]
fn r4_patterns_that_differ_by_axis_are_two_branches() {
    let w = Ws::new("r4");
    let table = "{ @{ { m: 1 } }: \"a\", @{ { %m: 1 } }: \"b\" }";
    let control = "{ @{ { m: 1 } }: \"a\", @{ { %m: 2 } }: \"b\" }";
    assert_eq!(w.eval(control).matches("@{").count(), 2, "control: {}", w.eval(control));
    assert_eq!(w.eval(table).matches("@{").count(), 2, "one branch: {}", w.eval(table));
    // Each argument meets both of its table's patterns, the control's too.
    for (arg, control_arg) in [("{ m: 1 }", "{ m: 1 }"), ("{ %m: 1 }", "{ %m: 2 }")] {
        let want = w.eval(&format!("({control}) {control_arg}"));
        assert!(!want.contains("_|_"), "control answers: {want}");
        assert_eq!(w.eval(&format!("({table}) {arg}")), want, "applied to {arg}");
    }
}

/// A store this engine creates declares the new encoding.
#[test]
fn r5_a_store_this_engine_creates_declares_encoding_6() {
    let fresh = Ws::new("r5");
    fresh.commit("x: 0\n");
    assert_eq!(fresh.objects_format(), "encoding=10"); // AMENDED 2026-10-19 for Q-093 (D112); AMENDED 2026-10-21 for Q-097 (D114): encoding 8; AMENDED 2026-10-22 for Q-099 (D115): encoding 9; AMENDED 2026-10-22 for Q-100 (D116): encoding 10
}

/// The new identity red lines. Each is the root the reference encoding
/// computes (REAL_03 §6.2/§6.3 as D106 rewrote them); a second implementation
/// of that text must land on the same bytes.
#[test]
fn r6_the_new_red_lines() {
    let cases = [
        ("x: 0", "b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f"),
        ("v: 1 + 1", "df91a8ad239ea8727c3a03a71eb4b0a787312b92306db4d8b124a73faaf40199"),
        ("f: (x -> x + 1)", "258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f"),
        ("k: { a: 1, _: 9 }", "420a0c9f4592b49ace8c8aacec690fd0a70953b1bd2158260e68c3acce710d9e"),
        ("m: (_ -> 1)", "cf11d2ff2e95c11ecbae5ceba00ece7c60f87c7d81149287ffebce1b551a89a6"),
    ];
    let mut wrong = Vec::new();
    for (i, (src, want)) in cases.iter().enumerate() {
        let w = Ws::new(&format!("r6-{i}"));
        w.commit(&format!("{src}\n"));
        let got = w.root();
        if got != *want {
            wrong.push(format!("  {src:?}\n      want {want}\n      got  {got}"));
        }
        if i == 0 {
            let (s, _) = w.oo(&["status"]);
            let std = "c96c29378c9395936ea4999db37e9b6a28291b57a47d4c016ca3cdf6833fed3b";
            if !s.contains(std) {
                wrong.push(format!("  standard root\n      want {std}\n      got  {s}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

// ── Guard: an epoch-1 store still opens, answers, and keeps its history ──

/// The fixture opens and answers what `oo v0.84.0` answered.
#[test]
fn g1_an_epoch_1_store_opens_and_answers() {
    let w = Ws::epoch1("g1");
    let (log, rc) = w.oo(&["log"]);
    assert_eq!(rc, 0, "log: {log}");
    assert!(log.contains(EPOCH1_HEAD) && log.contains(EPOCH1_FIRST), "log: {log}");
    let (s, rc) = w.oo(&["status"]);
    assert_eq!(rc, 0, "status: {s}");
    assert!(s.contains(&format!("{EPOCH1_STANDARD} (available)")), "status: {s}");
    for (e, want) in EPOCH1_ANSWERS {
        assert_eq!(w.eval(e), *want, "{e}");
    }
}

/// A commit into the epoch-1 store keeps everything that was there: both
/// old commits still open by their epoch-1 addresses, and the old answers.
#[test]
fn g2_an_epoch_1_store_keeps_its_history_after_a_write() {
    let w = Ws::epoch1("g2");
    w.commit("y: 1\n");
    assert_ne!(w.head(), "", "HEAD");
    let (log, rc) = w.oo(&["log"]);
    assert_eq!(rc, 0, "log: {log}");
    assert_eq!(log.matches("commit ").count(), 3, "log: {log}");
    assert!(log.contains(EPOCH1_HEAD) && log.contains(EPOCH1_FIRST), "log: {log}");
    for c in [EPOCH1_HEAD, EPOCH1_FIRST] {
        let caid = log
            .lines()
            .filter_map(|l| l.strip_prefix("commit "))
            .find(|a| a.ends_with(c))
            .unwrap_or_else(|| panic!("{c} not in log: {log}"));
        let (o, rc) = w.oo(&["inspect", caid]);
        assert_eq!(rc, 0, "inspect {caid}: {o}");
    }
    for (e, want) in EPOCH1_ANSWERS {
        assert_eq!(w.eval(e), *want, "{e}");
    }
    assert_eq!(w.eval("_.y"), "1");
}

/// A read still recomputes the address, in either epoch: a value changed in
/// place is caught, with the same name, in an epoch-1 store and in a store
/// this engine wrote. Controls: both read cleanly before the change.
#[test]
fn g3_tampering_is_caught_in_both_epochs() {
    let old = Ws::epoch1("g3a");
    let fresh = Ws::new("g3b");
    fresh.commit("t: { int: 1 }\nx: 0\n");
    for (name, w) in [("epoch-1", &old), ("epoch-2", &fresh)] {
        assert_eq!(w.eval("_.t"), "{ int: 1 }", "{name} control");
        let root = w.root();
        assert_eq!(root.len(), 64, "{name}: root");
        let p = w.object_path(&root);
        let body = fs::read_to_string(&p).unwrap();
        assert!(body.contains("int: 1"), "{name}: root object {body}");
        fs::write(&p, body.replacen("int: 1", "int: 2", 1)).unwrap();
        let (o, rc) = w.oo(&["eval", "_.t"]);
        assert_ne!(rc, 0, "{name}: a changed value read back: {o}");
        assert!(o.contains("caid_mismatch"), "{name}: {o}");
    }
}

/// `gc` on the epoch-1 store finds every reachable object intact and
/// deletes none of them.
#[test]
fn g4_gc_keeps_an_epoch_1_store_whole() {
    let w = Ws::epoch1("g4");
    let (o, rc) = w.oo(&["gc", "--grant", "gc"]);
    assert_eq!(rc, 0, "gc: {o}");
    let (log, rc) = w.oo(&["log"]);
    assert_eq!(rc, 0, "log after gc: {log}");
    assert!(log.contains(EPOCH1_FIRST), "log after gc: {log}");
    for (e, want) in EPOCH1_ANSWERS {
        assert_eq!(w.eval(e), *want, "{e} after gc");
    }
}

/// The fixture is the real one, byte for byte where it matters.
#[test]
fn g5_the_fixture_is_the_real_one() {
    let f = fixture();
    assert_eq!(fs::read_to_string(f.join("objects.format")).unwrap().trim(), "encoding=5");
    assert_eq!(fs::read_to_string(f.join("format")).unwrap().trim(), "layout=9");
    assert!(fs::read_to_string(f.join("HEAD")).unwrap().trim().ends_with(EPOCH1_HEAD));
    for d in [EPOCH1_HEAD, EPOCH1_FIRST, EPOCH1_STANDARD] {
        assert!(f.join("objects/sha256").join(&d[..2]).join(&d[2..]).exists(), "{d}");
    }
}

// ── R-1 (acceptor, 2026-10-12): an epoch-1 store stays an epoch-1 store ──

/// REAL_02 §5.1.1: a write does not change the declarations it finds.
/// v0.84.0 green; the first delivery redeclared encoding=6 on this commit,
/// and v0.84.0 then refused a store nobody had asked to migrate.
#[test]
fn g6_an_ordinary_write_moves_no_declaration() {
    let w = Ws::epoch1("g6");
    w.commit("y: 1\n");
    assert_eq!(read_decl(&w, "format"), "layout=9");
    assert_eq!(read_decl(&w, "objects.format"), "encoding=5", "an ordinary commit advanced the encoding");
}

/// Where the epoch-1 address cannot tell two values apart, the write is
/// refused before anything is written, and the refusal names the way out.
/// After that explicit migrate the same pin lands.
#[test]
fn r7_a_pin_an_epoch_1_store_cannot_hold_is_refused_by_name() {
    let w = Ws::epoch1("r7");
    let head = w.head();
    fs::write(w.ws.join("b.n"), "t: { @int: 1 }\n").unwrap();
    let (e, erc) = w.oo(&["evolve", "--pin", "--grant", "pin", "b.n"]);
    let (c, crc) = if erc == 0 { w.oo(&["commit", "--grant", "pin", "-m", "b"]) } else { (e.clone(), erc) };
    assert_ne!(crc, 0, "the pin was accepted into an epoch-1 store: {c}");
    assert!(c.contains("migrate"), "the refusal must name the migrate: {c}");
    assert_eq!(w.head(), head, "HEAD moved");
    assert_eq!(w.eval("_.t"), "{ int: 1 }");
    assert_eq!(read_decl(&w, "objects.format"), "encoding=5");

    let (m, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    assert_eq!(rc, 0, "migrate: {m}");
    assert_eq!(read_decl(&w, "objects.format"), "encoding=10"); // AMENDED 2026-10-19 for Q-093 (D112); AMENDED 2026-10-21 for Q-097 (D114): encoding 8; AMENDED 2026-10-22 for Q-099 (D115): encoding 9; AMENDED 2026-10-22 for Q-100 (D116): encoding 10
    if erc != 0 {
        let (o, rc) = w.oo(&["evolve", "--pin", "--grant", "pin", "b.n"]);
        assert_eq!(rc, 0, "pin evolve after migrate: {o}");
    }
    let (o, rc) = w.oo(&["commit", "--grant", "pin", "-m", "b"]);
    assert_eq!(rc, 0, "pin commit after migrate: {o}");
    assert_eq!(w.eval("_.t"), "{ @int: 1 }");
    let (log, _) = w.oo(&["log"]);
    assert!(log.contains(EPOCH1_HEAD) && log.contains(EPOCH1_FIRST), "history after migrate: {log}");
}

/// Tables `oo v0.84.0` stored answer as they did there (fixture README).
#[test]
fn g7_epoch_1_tables_are_the_same_tables() {
    let w = Ws::new("r8");
    copy_dir(&tables_fixture(), &w.ws.join(".oo"));
    let cases = [
        ("_.k \"s\"", "9"),
        ("_.k 5", "1"),
        ("_.b \"k\"", "{ a: 1 b: 2 }"),
        ("_.b \"z\"", "0"),
        ("_.k = { @{ @str }: 9, @{ @int }: 1 }", "#true"),
        ("(_.b & { @{ \"k\" }: { c: 3 } }) \"k\"", "{ a: 1 b: 2 c: 3 }"),
    ];
    let mut wrong = Vec::new();
    for (e, want) in cases {
        let got = w.eval(e);
        if got != want {
            wrong.push(format!("  {e}\n      want {want}\n      got  {got}"));
        }
    }
    for e in ["(_.k & { @{ @str }: 10 }) \"s\"", "(_.b & { @{ \"k\" }: { a: 5 } }) \"k\""] {
        let got = w.eval(e);
        if !got.contains("#conflict") {
            wrong.push(format!("  {e}\n      want ⊥ #conflict\n      got  {got}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The migrate is now the only way to encoding=6, so its price is the only
/// warning: it must name the engines it locks out (v0.84.0 opens this store
/// today and will not after).
#[test]
fn r9_the_migrate_names_what_it_locks_out() {
    let w = Ws::epoch1("r9");
    let (o, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    assert_eq!(rc, 0, "migrate: {o}");
    // AMENDED 2026-10-19 for Q-093 (D112): encoding=7 locks out through v0.94.0.
    assert!(o.contains("v0.94.0") && !o.contains("locks out no engine"), "migrate cost: {o}");
}

// ── Acceptance (acceptor, 2026-10-13): the way out the refusal names exists ──

/// `tests/fixtures/epoch1_encoding3_repo` (real `oo v0.25.0`): `t: { int: 1 }`.
fn encoding3_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/epoch1_encoding3_repo/oo_dir")
}

const ENCODING3_HEAD: &str = "a733006555c09668cdd080db1aea6df3a142f7f9feeb74d1c8241a6aec80d989";

/// REAL_03 §2.3 and §6.8: a refusal that names a remedy names one that works.
/// `migrate` leaves encoding=3 as it is (its roots keep the pre-v0.26.0
/// address rule, O63). So in that store the cross-axis pin is refused, and the
/// refusal may not send the operator to `oo migrate`: here that prints
/// "Nothing was changed" and the same pin is refused again.
/// v0.84.0 red (the pin is accepted and does not land). R-1 delivery red
/// (the refusal says "run `oo migrate`"; migrate leaves encoding=3).
#[test]
fn r11_the_way_out_a_refusal_names_exists() {
    let w = Ws::new("r11");
    copy_dir(&encoding3_fixture(), &w.ws.join(".oo"));
    // layout=2 cannot hold a pinned injection; this migrate is the layout's.
    let (m, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    assert_eq!(rc, 0, "layout migrate: {m}");
    assert_eq!(read_decl(&w, "objects.format"), "encoding=3", "precondition: migrate keeps encoding=3: {m}");
    let head = w.head();
    fs::write(w.ws.join("b.n"), "t: { @int: 1 }\n").unwrap();
    let (e, erc) = w.oo(&["evolve", "--pin", "--grant", "pin", "b.n"]);
    let (c, crc) = if erc == 0 { w.oo(&["commit", "--grant", "pin", "-m", "b"]) } else { (e.clone(), erc) };
    assert_ne!(crc, 0, "the pin was accepted into an encoding=3 store: {c}");
    assert!(c.contains("#identity_collision"), "the refusal must be named: {c}");
    assert_eq!(w.head(), head, "HEAD moved");
    assert_eq!(w.eval("_.t"), "{ int: 1 }");
    if c.contains("run `oo migrate") {
        let (m, rc) = w.oo(&["migrate", "--grant", "migrate"]);
        assert_eq!(rc, 0, "migrate: {m}");
        assert_eq!(
            read_decl(&w, "objects.format"),
            "encoding=7", // AMENDED 2026-10-19 for Q-093 (D112)
            "the refusal sent the operator to `oo migrate`, and migrate did not advance the encoding: {c} / {m}"
        );
    }
}

/// The encoding=3 fixture is the one `oo v0.25.0` wrote.
#[test]
fn g8_the_encoding_3_fixture_is_the_real_one() {
    let f = encoding3_fixture();
    assert_eq!(fs::read_to_string(f.join("objects.format")).unwrap().trim(), "encoding=3");
    assert_eq!(fs::read_to_string(f.join("format")).unwrap().trim(), "layout=2");
    assert!(fs::read_to_string(f.join("HEAD")).unwrap().trim().ends_with(ENCODING3_HEAD));
    assert!(f.join("objects/sha256").join(&ENCODING3_HEAD[..2]).join(&ENCODING3_HEAD[2..]).exists());
}
