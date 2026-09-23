// A refusal written as an answer.
// Order: nlang-tools/docs/a_refusal_written_as_an_answer_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-055.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// Several commands, on finding that they cannot do their own job, say so in
// one surface and deny it in another — or say something else entirely:
//
//   * `oo status` prints `Universe unavailable: …` and exits 0.
//   * `oo inspect` against a store that EXISTS but refuses to open (an
//     `objects.format` it does not understand, or — D73 — a node key that is
//     present and unreadable) answers `CAID not found in local store`.
//     The object is there. The engine never looked.
//   * `oo eval '_{sha256:<root>}.x'` in the same store answers
//     `_|_ ;; %cause: #missing_key` — byte-for-byte the answer for an
//     address that does not exist anywhere. The refusal became a value.
//     Under D73 the same `eval` walks straight past the refusal that `run`,
//     `status` and `log` all honour.
//   * `oo migrate` states its cost after it has acted, names one of the four
//     reference engines it locks out, and on a store that is already current
//     reports "Migrated …" while changing no byte.
//   * Two refusals speak the host's language (`expected value at line 1
//     column 1`; `KeyRejected("InvalidEncoding")`).
//
// The eval/inspect cases share one line, present since the first CLI
// (2026-05-24): `Ouroboros::init(&cur).unwrap_or_else(|_| new_in_memory())`.
// It swallows EVERY init failure and substitutes an empty store.
//
// ── What is NOT the defect ───────────────────────────────────────────────
//
// The in-memory fallback has one honest use: there is no store, and none can
// be made (a read-only directory). `g2` pins that it survives. The rule is the
// three-state rule of D73 and REAL_03 §6.6, applied to the store instead of
// the key: absent may be replaced; present-but-unopenable may not.
//
// A peer's refusal printed as the answer to a network command (`oo node
// discover` → `#rejected`, exit 0) is NOT probed here: whether an answer that
// is a refusal belongs on the exit code is O84, unruled.
//
// NOT PROBED, so silence is not mistaken for coverage:
//   * `Error: Commit failed` (universe.rs:1113/1121) has no coordinate and no
//     code; the acceptor could not reach it. The order asks for reachability.
//   * `oo log` prints a commit re-read failure on stderr and exits 0
//     (main.rs:1029); reachable only by a race the acceptor did not build.
//   * `oo node discover` discards `record_peer_advert`'s log lines, and
//     `peers::append` returns silently when the directory cannot be opened or
//     written. Read, not measured.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Every red asserts an observation, not a mechanism. `k1` applies each red's
// predicate to a REAL output that already behaves correctly today, so a red
// cannot be red because its predicate is unsatisfiable (calibrate both poles).
// Every fault injection is checked to have reached its target before the red
// assertion runs; if it did not, the probe panics with `VOID READING`.
//
// The delivery may NOT edit this file. If a pin is wrong, say so in the
// report. `rustfmt` must not touch it.
//
// Baseline measured 2026-09-23 on dev b87f226 / oo v0.57.0: see the order.
// r9, r10 added at acceptance. r9 (repair round R-1): green on v0.57.0, red on the
// delivery e2831a3. r10: red on v0.57.0, green on e2831a3 (pins an ordering
// the delivery relies on). Both poles measured with real builds; see §9.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("refusal-answer-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    /// Run `oo` in the workspace. Returns (stdout ++ stderr, exit code).
    /// The exit code is the child's own — never taken after a pipeline.
    fn oo(&self, args: &[&str]) -> (String, i32) {
        self.oo_in(&self.ws, args)
    }

    fn oo_in(&self, dir: &Path, args: &[&str]) -> (String, i32) {
        let o = Command::new(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(dir)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .output()
            .expect("oo runs");
        (
            format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ),
            o.status.code().unwrap_or(-1),
        )
    }

    fn write(&self, name: &str, body: &str) {
        fs::write(self.ws.join(name), body).unwrap();
    }

    fn oo_dir(&self) -> PathBuf {
        self.ws.join(".oo")
    }

    /// A committed `x: 1`. Returns (commit CAID, root digest hex).
    fn committed(&self) -> (String, String) {
        self.write("a.n", "x: 1\n");
        let (o, rc) = self.oo(&["evolve", "a.n"]);
        assert_eq!(rc, 0, "setup evolve: {o}");
        let (o, rc) = self.oo(&["commit"]);
        assert_eq!(rc, 0, "setup commit: {o}");
        let (log, rc) = self.oo(&["log"]);
        assert_eq!(rc, 0, "setup log: {log}");
        let commit = log
            .lines()
            .find_map(|l| l.strip_prefix("commit "))
            .unwrap_or_else(|| panic!("no commit in log: {log}"))
            .trim()
            .to_string();
        let (ins, rc) = self.oo(&["inspect", &commit]);
        assert_eq!(rc, 0, "setup inspect: {ins}");
        let root = ins
            .lines()
            .find_map(|l| l.strip_prefix("root:"))
            .unwrap_or_else(|| panic!("no root line: {ins}"))
            .trim()
            .to_string();
        let hex = root.rsplit(':').next().unwrap().to_string();
        assert!(
            hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()),
            "root digest is not 64 hex: {root}"
        );
        (commit, hex)
    }

    /// Replace the object-encoding declaration with one no engine understands.
    /// Checked: `status` must refuse, or the injection did not reach its target.
    fn refuse_store(&self) {
        fs::write(self.oo_dir().join("objects.format"), "encoding=99\n").unwrap();
        let (o, rc) = self.oo(&["status"]);
        if rc == 0 || !o.contains("refusing to open") {
            panic!("VOID READING: the store was supposed to refuse to open, status gave rc={rc}: {o}");
        }
    }

    /// Mint this workspace's node key, then truncate it to 20 bytes (D73).
    /// Checked: `status` must refuse by name.
    fn break_node_key(&self) {
        let (o, rc) = self.oo(&["node", "id"]);
        assert_eq!(rc, 0, "setup node id: {o}");
        let nodes = self.root.join("node-home").join("nodes");
        let keys: Vec<_> = fs::read_dir(&nodes)
            .unwrap_or_else(|e| panic!("no node key dir {}: {e}", nodes.display()))
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(keys.len(), 1, "expected exactly one node key: {keys:?}");
        let bytes = fs::read(&keys[0]).unwrap();
        fs::write(&keys[0], &bytes[..20]).unwrap();
        let (o, rc) = self.oo(&["status"]);
        if rc == 0 || !o.contains("PKCS#8") {
            panic!("VOID READING: D73 should refuse this workspace, status gave rc={rc}: {o}");
        }
    }

    /// Corrupt the one injection file so the working set cannot be read.
    /// Checked: `commit` must refuse, or the injection did not reach its target.
    fn corrupt_injection(&self) {
        self.write("a.n", "x: 1\n");
        let (o, rc) = self.oo(&["evolve", "a.n"]);
        assert_eq!(rc, 0, "setup evolve: {o}");
        let inj = self.oo_dir().join("injections");
        let files: Vec<_> = fs::read_dir(&inj).unwrap().map(|e| e.unwrap().path()).collect();
        assert_eq!(files.len(), 1, "expected one injection: {files:?}");
        fs::write(&files[0], "garbage{{{").unwrap();
        let (o, rc) = self.oo(&["commit"]);
        if rc == 0 {
            panic!("VOID READING: commit accepted a corrupted working set: {o}");
        }
    }

    fn declarations(&self) -> (Vec<u8>, Vec<u8>) {
        (
            fs::read(self.oo_dir().join("format")).unwrap(),
            fs::read(self.oo_dir().join("objects.format")).unwrap(),
        )
    }
}

fn chmod(p: &Path, mode: u32) {
    fs::set_permissions(p, fs::Permissions::from_mode(mode)).unwrap();
}

// ── The red predicates, named so `k1` can calibrate them ─────────────────

/// An observation that cannot be told apart from "this object does not exist".
fn reads_as_absence(out: &str, rc: i32) -> bool {
    out.contains("not found") || (rc == 0 && out.contains("#missing_key"))
}

/// A refusal written in the host's words rather than the engine's.
fn speaks_host(out: &str) -> bool {
    out.contains("line 1 column 1") || out.contains("KeyRejected(")
}

/// The migrate cost names the newest reference engine it locks out.
/// Measured 2026-09-23: after layout=2 → layout=5, real v0.40.0, v0.41.0,
/// v0.42.0 and v0.43.0 refuse to open; v0.44.0 opens. Either end of that
/// boundary satisfies this — "up to v0.43.0" or "before v0.44.0".
fn names_the_boundary(out: &str) -> bool {
    out.contains("v0.43.0") || out.contains("v0.44.0")
}

// ── Controls and guards (green at baseline, must stay green) ─────────────

/// The address literal really reaches the store: the committed value comes
/// back, and an address that is nowhere comes back as absence. Without this,
/// r3/r4 could be red for a reason that has nothing to do with the store.
#[test]
fn c1_the_address_literal_reaches_the_store() {
    let w = Ws::new("c1");
    let (_, hex) = w.committed();
    let (o, rc) = w.oo(&["eval", &format!("_{{sha256:{hex}}}.x")]);
    assert_eq!((o.trim(), rc), ("1", 0), "the committed value must come back");
    let nowhere = "ab".repeat(32);
    let (o, rc) = w.oo(&["eval", &format!("_{{sha256:{nowhere}}}.x")]);
    assert!(rc == 0 && o.contains("#missing_key"), "absence must read as absence: rc={rc} {o}");
}

/// Absence is still absence after the repair: a CAID that is not in a healthy
/// store is reported as not found. The repair must not turn every miss into
/// a refusal.
#[test]
fn g1_absence_is_still_absence() {
    let w = Ws::new("g1");
    let _ = w.committed();
    let nowhere = format!("hash:sha256:v1:{}", "cd".repeat(32));
    let (o, rc) = w.oo(&["inspect", &nowhere]);
    assert!(rc != 0 && o.contains("not found"), "a real miss must say so: rc={rc} {o}");
}

/// The in-memory fallback's one honest use survives: no store exists and none
/// can be made (read-only directory), and a pure expression still evaluates.
#[test]
fn g2_no_store_and_none_possible_still_evaluates() {
    let w = Ws::new("g2");
    let ro = w.root.join("readonly");
    fs::create_dir_all(&ro).unwrap();
    chmod(&ro, 0o555);
    let (o, rc) = w.oo_in(&ro, &["eval", "~%Math./add (1,2)"]);
    chmod(&ro, 0o755);
    assert!(!ro.join(".oo").exists(), "VOID READING: a store was created in a read-only directory");
    assert_eq!((o.trim(), rc), ("3", 0), "a pure expression with no store must still evaluate");
}

/// `migrate` still needs its grant, and still migrates.
#[test]
fn g3_migrate_still_needs_its_grant_and_still_migrates() {
    let w = Ws::new("g3");
    let _ = w.committed();
    fs::write(w.oo_dir().join("format"), "layout=2\n").unwrap();
    let before = w.declarations();
    let (o, rc) = w.oo(&["migrate"]);
    assert!(rc != 0 && o.contains("#privileged_required"), "no grant must refuse: rc={rc} {o}");
    assert_eq!(w.declarations(), before, "a refused migrate must not touch the declarations");
    let (o, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    assert_eq!(rc, 0, "a granted migrate must succeed: {o}");
    let (layout, _) = w.declarations();
    assert_eq!(String::from_utf8_lossy(&layout).trim(), "layout=5", "migrate must advance the layout");
}

/// Each red predicate is satisfiable by a real, already-correct output — so a
/// red below is red because of the engine, not because the predicate cannot
/// be met. (The other pole: every red is red at baseline; see the order.)
#[test]
fn k1_every_red_predicate_is_met_by_a_real_refusal() {
    // r1/r2/r3: `status` against a refused store is a correct refusal.
    let w = Ws::new("k1a");
    let _ = w.committed();
    w.refuse_store();
    let (o, rc) = w.oo(&["status"]);
    assert!(rc != 0, "status refuses: {o}");
    assert!(!reads_as_absence(&o, rc), "a correct refusal must not read as absence: {o}");
    assert!(!speaks_host(&o), "a correct refusal speaks the engine's words: {o}");

    // r4: `run` under D73 is a correct refusal.
    let w = Ws::new("k1b");
    let _ = w.committed();
    w.break_node_key();
    w.write("b.n", "r: 1\n");
    let (o, rc) = w.oo(&["run", "b.n", "--observe", "r"]);
    assert!(rc != 0, "run refuses under D73: {o}");
    assert!(!reads_as_absence(&o, rc), "{o}");

    // r6/r7: the predicate is met by a sentence of the required kind.
    assert!(names_the_boundary("engines before oo v0.44.0 will no longer open this store"));
    assert!(names_the_boundary("oo v0.40.0 through v0.43.0 will no longer open this store"));
    assert!(!names_the_boundary(
        "An engine that only reads layout=2 (oo v0.41.0) will no longer open this store."
    ));
}

// ── Reds (red at baseline; the delivery makes them green) ────────────────

/// `status` could not load the universe and says so — and exits 0.
/// Baseline: rc=0, `Universe unavailable: expected value at line 1 column 1`.
#[test]
fn r1_a_status_that_cannot_load_the_universe_does_not_exit_zero() {
    let w = Ws::new("r1");
    w.corrupt_injection();
    let (o, rc) = w.oo(&["status"]);
    assert!(rc != 0, "a status that could not load the universe exited 0: {o}");
}

/// The store exists and refuses to open; `inspect` of an object that IS in it
/// must not read as "that object does not exist".
/// Baseline: `CAID not found in local store`, rc=1.
#[test]
fn r2_inspect_does_not_report_an_unopenable_store_as_a_missing_object() {
    let w = Ws::new("r2");
    let (commit, _) = w.committed();
    w.refuse_store();
    let (o, rc) = w.oo(&["inspect", &commit]);
    assert!(rc != 0, "inspect against a store it cannot open exited 0: {o}");
    assert!(!reads_as_absence(&o, rc), "an unopenable store was reported as a missing object: {o}");
}

/// Same store; `eval` of an address literal naming the committed root must
/// not answer as if that address did not exist, and must not answer with the
/// value either (the engine declared it cannot read that encoding).
/// Baseline: `_|_ ;; %cause: #missing_key`, rc=0 — identical to `c1`'s miss.
#[test]
fn r3_eval_does_not_answer_an_unopenable_store_with_absence() {
    let w = Ws::new("r3");
    let (_, hex) = w.committed();
    w.refuse_store();
    let (o, rc) = w.oo(&["eval", &format!("_{{sha256:{hex}}}.x")]);
    assert!(!reads_as_absence(&o, rc), "an unopenable store was answered as absence: rc={rc} {o}");
    assert!(!(rc == 0 && o.trim() == "1"), "the engine read an encoding it declared it cannot read: {o}");
}

/// D73: a present, unreadable node key refuses the workspace by name. Every
/// path honours it — `status`, `log`, `run` already do; `eval` and `inspect`
/// must not walk past it.
/// Baseline: `eval` pure → `3` rc=0; `eval` address → `#missing_key` rc=0;
/// `inspect` → `CAID not found`.
#[test]
fn r4_an_unreadable_node_key_refuses_on_every_path() {
    let w = Ws::new("r4");
    let (commit, hex) = w.committed();
    w.break_node_key();
    let (o, rc) = w.oo(&["eval", "~%Math./add (1,2)"]);
    assert!(rc != 0, "eval walked past the D73 refusal that run/status/log honour: {o}");
    let (o, rc) = w.oo(&["eval", &format!("_{{sha256:{hex}}}.x")]);
    assert!(!reads_as_absence(&o, rc), "D73 refusal answered as absence: rc={rc} {o}");
    let (o, rc) = w.oo(&["inspect", &commit]);
    assert!(!reads_as_absence(&o, rc), "D73 refusal reported as a missing object: {o}");
}

/// A migrate that changed no byte must not say it migrated.
/// Baseline: rc=0, `Migrated store layout to layout=5. …`, declarations
/// byte-identical.
#[test]
fn r5_a_migrate_that_changed_nothing_does_not_say_it_migrated() {
    let w = Ws::new("r5");
    let _ = w.committed();
    let before = w.declarations();
    let (o, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    if w.declarations() != before {
        panic!("VOID READING: the store was not already current (declarations changed): {o}");
    }
    assert!(!o.contains("Migrated store layout to"), "claimed a migration that changed nothing: rc={rc} {o}");
}

/// REAL_02 §5.1.1: the cost is stated BEFORE the act. Make the act fail
/// (read-only `.oo`); the cost must already be on the operator's screen.
/// Baseline: nothing but the host error — the cost sentence is printed only
/// after a successful write.
#[test]
fn r6_the_cost_is_on_the_screen_before_the_store_is_touched() {
    let w = Ws::new("r6");
    let _ = w.committed();
    fs::write(w.oo_dir().join("format"), "layout=2\n").unwrap();
    let before = w.declarations();
    chmod(&w.oo_dir(), 0o555);
    let (o, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    chmod(&w.oo_dir(), 0o755);
    if rc == 0 || w.declarations() != before {
        panic!("VOID READING: the write was supposed to fail (running as root?): rc={rc} {o}");
    }
    assert!(
        o.contains("layout=5") && names_the_boundary(&o),
        "the act was attempted before its cost was stated: {o}"
    );
}

/// REAL_02 §5.1.1: "特別是哪些引擎此後將打不開這個儲存" — all of them.
/// Baseline: names `oo v0.41.0` only; v0.40.0–v0.43.0 are locked out.
#[test]
fn r7_migrate_names_every_engine_it_locks_out() {
    let w = Ws::new("r7");
    let _ = w.committed();
    fs::write(w.oo_dir().join("format"), "layout=2\n").unwrap();
    let (o, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    assert_eq!(rc, 0, "setup: {o}");
    assert!(names_the_boundary(&o), "the cost names only part of what it locks out: {o}");
}

/// REAL_01 §1.3 (i): a refusal of a corrupt working set, in the engine's words.
/// Baseline: `expected value at line 1 column 1` (serde_json) in both.
#[test]
fn r8a_a_corrupt_working_set_is_refused_in_the_engines_words() {
    let w = Ws::new("r8a");
    w.corrupt_injection();
    let (s, _) = w.oo(&["status"]);
    let (c, _) = w.oo(&["commit"]);
    assert!(!speaks_host(&s), "status speaks the host: {s}");
    assert!(!speaks_host(&c), "commit speaks the host: {c}");
}

/// REAL_01 §1.3 (i): the D73 refusal itself, in the engine's words.
/// Baseline: `… not a valid PKCS#8 Ed25519 key (KeyRejected("InvalidEncoding"))`
/// — a Rust `Debug` of the crypto library's error. The acceptor accepted Q-054
/// with this text and did not notice.
#[test]
fn r8b_the_d73_refusal_is_in_the_engines_words() {
    let w = Ws::new("r8b");
    let _ = w.committed();
    w.break_node_key();
    let (o, _) = w.oo(&["status"]);
    assert!(!speaks_host(&o), "the D73 refusal speaks the host: {o}");
}

// ── Added at acceptance (2026-09-23), repair round R-1 ───────────────────

/// `migrate` advances TWO declarations — `format` and `objects.format`
/// (`storage.rs` `migrate_layout`). The delivered "already current" check
/// reads only the layout, so a store at `layout=5` / `encoding=4` — exactly
/// the half-migrated state the delivery measured for Q4 — is told "Nothing
/// was changed" and can no longer be completed by the one explicit action
/// that advances the encoding. v0.57.0 completed it (encoding → 5).
/// Baseline: green on v0.57.0; red on the delivery e2831a3.
#[test]
fn r9_a_half_migrated_store_can_still_be_completed() {
    let w = Ws::new("r9");
    let _ = w.committed();
    fs::write(w.oo_dir().join("objects.format"), "encoding=4\n").unwrap();
    let (o, rc) = w.oo(&["status"]);
    if rc != 0 {
        panic!("VOID READING: a layout=5 / encoding=4 store was supposed to open: {o}");
    }
    let (o, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    assert_eq!(rc, 0, "migrate on a half-migrated store: {o}");
    let (_, enc) = w.declarations();
    assert_eq!(
        String::from_utf8_lossy(&enc).trim(),
        "encoding=5",
        "an explicit migrate left the encoding axis behind and said: {o}"
    );
}

/// D73 holds before any store exists. The delivery tells "absent" from
/// "present but unopenable" by asking whether a durable store is there
/// AFTER `init` failed — and that works here only because `init` lays down
/// `.oo/format` before it reads the node key. This pins the observation so a
/// reordering of `init` cannot quietly reopen the path `r4` closed.
/// Baseline: v0.57.0 answers `3`, rc=0 (red); the delivery refuses (green).
#[test]
fn r10_an_unreadable_node_key_refuses_even_before_there_is_a_store() {
    let w = Ws::new("r10");
    w.break_node_key();
    fs::remove_dir_all(w.oo_dir()).unwrap();
    let (o, rc) = w.oo(&["eval", "~%Math./add (1,2)"]);
    assert!(rc != 0, "eval walked past an unreadable node key in a workspace with no store yet: {o}");
    assert!(o.contains("PKCS#8"), "the refusal must name the key: {o}");
}
