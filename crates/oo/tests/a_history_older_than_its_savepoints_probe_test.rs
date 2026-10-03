// A history older than its savepoints.
// Order: nlang-tools/docs/a_history_older_than_its_savepoints_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-063.  Ruling: meta/oo/STATUS.md D81.
// Design note: nlang-spec/meta/oo/commit.md §1.12 (candidate).
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// D79 (Q-061) made a lost context refuse: HEAD absent while the store
// declares a commit ⟹ context-relative commands refuse by name, and
// `rollback` is the way back. Its evidence was one form of declaration only:
// the `commit:` note on a savepoint, which exists from D52 (v0.41.0) on.
// Measured on v0.64.0: a store written before that — real v0.40.0 (framed,
// `layout=2`) or real v0.35.0 (JSON, `encoding=4`) — with HEAD moved aside is
// read as an honest empty context: `status` "no committed root yet", `log`
// prints nothing, `gc --grant gc` "0 reachable" and deletes every object
// (5/5 and 3/3), and a `migrate` to `layout=8` first changes nothing.
//
// D81 (user, 2026-09-29, 甲): the evidence is that the STORE declares a
// commit — a savepoint's `commit:` note, OR an object the engine reads as a
// Commit. The kind of an object is what the engine wrote in its frame (or,
// in the JSON era, the Commit structure) — never inferred from content: an
// uncommitted workspace can `~%Engine./save` a value whose text contains
// "#nlang/store commit" and whose fields look like a commit (g4).
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Fixtures are real stores built by real old binaries, pinned by digest
// (g1). "Lost" is made by moving `.oo/HEAD` aside. Wording-free except:
// `rollback` (the way back, the command's name), and the current refusal's
// own claim "savepoint records a commit", which is false for a store whose
// savepoints record none (r6).
//
// The delivery may NOT edit this file or the fixtures. If a pin is wrong,
// say so in the report. `rustfmt` must not touch it.
//
// Baseline measured 2026-09-29 on dev 8bf0e5c / oo v0.64.0: see the order.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const F2: &str = "layout2_framed_repo"; // real v0.40.0, two commits, no notes
const F2_HEAD: &str = "hash:sha256:v1:55b12926437694d2aedcec2e96e45cf319fd4682966bcfb4a497067cd83982b7";
const F2_FIRST: &str = "hash:sha256:v1:84a0ebde30eecb29b7f6b879cb7e21bfa4624b360f2ced74d2221684e39d9fc6";
const F4: &str = "encoding4_repo"; // real v0.35.0, one commit, JSON
const F4_HEAD_HEX: &str = "3ced204289895c8656b7d33ffb0a414d604841a865c411a26205ea3e082d36d3";

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("older-hist-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    fn from_fixture(tag: &str, fixture: &str) -> Self {
        let w = Ws::new(tag);
        copy_dir(&fixture_dir(fixture), &w.ws.join(".oo"));
        fs::write(w.ws.join("c.n"), "c: 3\n").unwrap();
        w
    }

    fn oo_in(&self, args: &[&str], stdin: &str) -> (String, i32) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("oo runs");
        child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
        let o = child.wait_with_output().unwrap();
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
            o.status.code().unwrap_or(-1),
        )
    }

    fn oo(&self, args: &[&str]) -> (String, i32) {
        self.oo_in(args, "")
    }

    fn ok(&self, args: &[&str]) -> String {
        let (o, rc) = self.oo(args);
        assert_eq!(rc, 0, "setup `oo {}`: {o}", args.join(" "));
        o
    }

    fn committed(&self, name: &str, text: &str) {
        fs::write(self.ws.join(name), text).unwrap();
        self.ok(&["evolve", name]);
        self.ok(&["commit", "-m", name]);
    }

    fn head_path(&self) -> PathBuf {
        self.ws.join(".oo/HEAD")
    }

    fn head(&self) -> String {
        fs::read_to_string(self.head_path()).unwrap().trim().to_string()
    }

    fn lose_head(&self) {
        fs::rename(self.head_path(), self.root.join("HEAD.aside")).unwrap();
    }

    fn restore_head(&self) {
        fs::rename(self.root.join("HEAD.aside"), self.head_path()).unwrap();
    }

    fn store(&self) -> BTreeMap<String, Vec<u8>> {
        let mut m = BTreeMap::new();
        files(&self.ws.join(".oo"), &self.ws.join(".oo"), &mut m);
        m
    }

    /// The root CAID of a commit, read through `inspect` (context-free).
    fn root_of(&self, commit: &str) -> String {
        let o = self.ok(&["inspect", commit]);
        o.lines()
            .find_map(|l| l.strip_prefix("root:").map(|r| r.trim().to_string()))
            .unwrap_or_else(|| panic!("VOID READING: no root line in inspect: {o}"))
    }
}

fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name).join("oo_dir")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &to.join(e.file_name()));
        } else {
            fs::copy(&p, to.join(e.file_name())).unwrap();
        }
    }
}

fn files(base: &Path, p: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    if let Ok(d) = fs::read_dir(p) {
        for e in d.flatten() {
            let q = e.path();
            if q.is_dir() {
                files(base, &q, out);
            } else {
                let rel = q.strip_prefix(base).unwrap().to_string_lossy().to_string();
                out.insert(rel, fs::read(&q).unwrap());
            }
        }
    }
}

fn digest_of_tree(dir: &Path) -> (usize, String) {
    let mut m = BTreeMap::new();
    files(dir, dir, &mut m);
    let mut acc = String::new();
    for (rel, bytes) in &m {
        let h = ring::digest::digest(&ring::digest::SHA256, bytes);
        acc.push_str(&format!("{}  {}\n", hex::encode(h.as_ref()), rel));
    }
    (m.len(), hex::encode(ring::digest::digest(&ring::digest::SHA256, acc.as_bytes()).as_ref()))
}

/// The context-relative commands (D79's list) on a lost context, each on a
/// fresh copy of the fixture. Each must refuse, write nothing, delete
/// nothing, and name the way back. `commit` is r4 (it needs something staged
/// while HEAD was still present).
fn matrix(tag: &str, fixture: &str) -> Vec<String> {
    let probe = Ws::from_fixture(&format!("{tag}-h"), fixture);
    let head = probe.head();
    let root = probe.root_of(&head);
    let runs: Vec<(Vec<&str>, &str)> = vec![
        (vec!["status"], ""),
        (vec!["log"], ""),
        (vec!["evolve", "c.n"], ""),
        (vec!["gc", "--grant", "gc"], ""),
        (vec!["squash", head.as_str(), "--grant", "squash"], ""),
        (vec!["refine", "-s", root.as_str(), "-t", root.as_str(), "-m", "r"], ""),
        (vec!["repl"], "exit\n"),
    ];
    let mut bad = Vec::new();
    for (i, (args, stdin)) in runs.into_iter().enumerate() {
        let w = Ws::from_fixture(&format!("{tag}-{i}"), fixture);
        w.lose_head();
        let before = w.store();
        let (o, rc) = w.oo_in(&args, stdin);
        let after = w.store();
        let mut why = Vec::new();
        if rc == 0 {
            why.push("rc=0".to_string());
        }
        if before != after {
            why.push("store changed".to_string());
        }
        if !o.contains("rollback") {
            why.push("does not name rollback".to_string());
        }
        if !why.is_empty() {
            bad.push(format!("`oo {}`: {} — {}", args.join(" "), why.join(", "), o.lines().next().unwrap_or("")));
        }
    }
    bad
}

// ── Guards (green on v0.64.0, must stay green) ───────────────────────────

/// The fixtures are the real old stores, byte for byte.
#[test]
fn g1_the_fixtures_are_the_real_ones() {
    assert_eq!(
        digest_of_tree(&fixture_dir(F2)),
        (10, "fb1f80268e00eff586807c9f90106b2445a8d07a5624c5c39bc605e39e997ffd".to_string()),
        "layout2_framed_repo changed"
    );
    assert_eq!(
        digest_of_tree(&fixture_dir(F4)),
        (6, "acc3ad47634582504226b49aee1fec36ff5e2979100003dadd73e05ea0f728d8".to_string()),
        "encoding4_repo changed"
    );
}

/// With HEAD present, old stores still open and read their history.
#[test]
fn g2_old_stores_with_a_head_still_answer() {
    let w = Ws::from_fixture("g2a", F2);
    let log = w.ok(&["log"]);
    assert!(log.contains(F2_HEAD) && log.contains(F2_FIRST), "v0.40.0 history: {log}");
    w.ok(&["status"]);
    let w = Ws::from_fixture("g2b", F4);
    let log = w.ok(&["log"]);
    assert!(log.contains(F4_HEAD_HEX), "v0.35.0 history: {log}");
}

/// The way back, on both old stores: an explicit rollback to a named commit
/// restores the context and the history reads again.
#[test]
fn g3_rollback_is_the_way_back_on_old_stores() {
    let w = Ws::from_fixture("g3a", F2);
    w.lose_head();
    w.ok(&["rollback", F2_HEAD, "--grant", "rollback"]);
    let log = w.ok(&["log"]);
    assert!(log.contains(F2_HEAD) && log.contains(F2_FIRST), "after recovery: {log}");
    let w = Ws::from_fixture("g3b", F4);
    let h = fs::read_to_string(w.head_path()).unwrap().trim().to_string();
    w.lose_head();
    w.ok(&["rollback", &h, "--grant", "rollback"]);
    let log = w.ok(&["log"]);
    assert!(log.contains(F4_HEAD_HEX), "after recovery: {log}");
}

/// An honest empty context still answers — even when it holds a saved value
/// whose text says "#nlang/store commit" and whose fields look like a commit.
/// The kind of an object is its frame, not its content.
#[test]
fn g4_an_honest_empty_context_holding_a_commit_shaped_value_still_answers() {
    let w = Ws::new("g4");
    fs::write(w.ws.join("seed.n"), "seed: 0\n").unwrap(); // AMENDED 2026-09-30 for Q-064 (D82): only `evolve` creates a universe; `save` where there is none is ⊥ #no_universe (D82 ③)
    w.ok(&["evolve", "seed.n"]);
    w.ok(&["eval", r##"~%Engine./save { kind: #Standard, parent: "p", root: "r", s: "#nlang/store commit" }"##]);
    let held = w.store().iter().any(|(k, v)| {
        k.starts_with("objects") && String::from_utf8_lossy(v).contains("#nlang/store commit\"")
    });
    if !held {
        panic!("VOID READING: the saved commit-shaped value is not in the store: {:?}", w.store().keys().collect::<Vec<_>>());
    }
    assert!(!w.head_path().exists(), "VOID READING: a HEAD exists");
    w.ok(&["status"]);
    w.ok(&["log"]);
    w.ok(&["gc", "--grant", "gc"]);
    w.committed("a.n", "a: 1\n");
    w.ok(&["log"]);
}

/// Context-free operations are unaffected on a lost old store.
#[test]
fn g5_context_free_commands_still_answer_on_a_lost_old_store() {
    let w = Ws::from_fixture("g5", F2);
    w.lose_head();
    w.ok(&["inspect", F2_HEAD]);
    // AMENDED 2026-10-03 for Q-070 (D88): `eval` now starts from HEAD's root,
    // so on a lost context it is no longer context-free -- it may refuse by
    // name (SPEC_08 §6.2.1, D79), and its context-free form is
    // `eval --ephemeral` (measured in the_universe_you_are_in r10). What this
    // cell still guards: it never answers wrongly.
    for (expr, want) in [("~%Math./add (1, 2)", "3"), ("~%Math./add (1, 3)", "4")] {
        let (o, rc) = w.oo(&["eval", expr]);
        assert!(
            (rc == 0 && o.trim() == want) || (rc != 0 && o.contains("rollback")),
            "eval on a lost context neither answered {want} nor refused by name: rc={rc} {o}"
        );
    }
}

/// D79's own evidence (the savepoint note) still works as before.
#[test]
fn g6_the_noted_path_is_unchanged() {
    let w = Ws::new("g6");
    w.committed("a.n", "a: 1\n");
    w.committed("b.n", "b: 2\n");
    w.lose_head();
    let before = w.store();
    let (o, rc) = w.oo(&["gc", "--grant", "gc"]);
    assert!(rc != 0 && w.store() == before && o.contains("rollback"), "rc={rc}: {o}");
}

// ── Reds ─────────────────────────────────────────────────────────────────

/// Baseline: rc=0, "5 objects, 0 reachable", 5 deleted.
#[test]
fn r1_gc_on_a_store_older_than_its_notes_deletes_nothing() {
    let w = Ws::from_fixture("r1", F2);
    w.lose_head();
    let before = w.store();
    let (o, rc) = w.oo(&["gc", "--grant", "gc"]);
    let after = w.store();
    assert!(after == before, "gc changed a lost v0.40.0 store (rc={rc}; {} → {} files): {o}", before.len(), after.len());
    assert!(rc != 0, "gc with a lost context did not refuse: {o}");
    w.restore_head();
    let log = w.ok(&["log"]);
    assert!(log.contains(F2_FIRST), "history lost: {log}");
}

/// Baseline: rc=0, "3 objects, 0 reachable", 3 deleted. JSON-era commit.
#[test]
fn r2_gc_on_a_json_era_store_deletes_nothing() {
    let w = Ws::from_fixture("r2", F4);
    w.lose_head();
    let before = w.store();
    let (o, rc) = w.oo(&["gc", "--grant", "gc"]);
    let after = w.store();
    assert!(after == before, "gc changed a lost v0.35.0 store (rc={rc}; {} → {} files): {o}", before.len(), after.len());
    assert!(rc != 0, "gc with a lost context did not refuse: {o}");
}

/// Every context-relative command, both old stores. Baseline: status/log/
/// repl rc=0, evolve writes, gc deletes, squash "no HEAD to squash" and
/// refine refuse without naming the way back.
#[test]
fn r3_every_context_command_refuses_on_both_old_stores() {
    let mut bad = Vec::new();
    for (tag, fx) in [("r3a", F2), ("r3b", F4)] {
        for b in matrix(tag, fx) {
            bad.push(format!("[{fx}] {b}"));
        }
    }
    assert!(bad.is_empty(), "context-relative commands answered a lost old context:\n{}", bad.join("\n"));
}

/// Baseline (v0.61.0 shape, now on an old store): a staged commit over a
/// lost context starts a new genesis chain; the next gc deletes the old one.
#[test]
fn r4_commit_does_not_start_a_new_history_over_an_old_one() {
    let w = Ws::from_fixture("r4", F2);
    w.ok(&["evolve", "c.n"]);
    w.lose_head();
    let (o, rc) = w.oo(&["commit", "-m", "c"]);
    let made_head = w.head_path().exists();
    assert!(rc != 0 && !made_head, "commit started a new history over a lost old one (rc={rc}): {o}");
    let _ = w.oo(&["gc", "--grant", "gc"]);
    w.restore_head();
    let log = w.ok(&["log"]);
    assert!(log.contains(F2_FIRST), "the old history did not survive: {log}");
}

/// The evidence is the store, not only its savepoints: a current store whose
/// savepoints are gone (or a first commit interrupted before its note)
/// still declares its commits. Baseline: gc deletes everything.
#[test]
fn r5_the_evidence_does_not_live_only_in_the_savepoints() {
    let w = Ws::new("r5");
    w.committed("a.n", "a: 1\n");
    w.committed("b.n", "b: 2\n");
    let first = w.ok(&["log"]);
    w.lose_head();
    fs::rename(w.ws.join(".oo/savepoints"), w.root.join("savepoints.aside")).unwrap();
    let before = w.store();
    let (o, rc) = w.oo(&["gc", "--grant", "gc"]);
    assert!(w.store() == before, "gc changed a lost store whose savepoints are gone (rc={rc}): {o}");
    assert!(rc != 0 && o.contains("rollback"), "not refused by name (rc={rc}): {o}");
    let (s, src) = w.oo(&["status"]);
    assert!(src != 0 && !s.contains("no committed root yet"), "status (rc={src}): {s}");
    assert!(first.contains("commit hash:"), "VOID READING: {first}");
}

/// The refusal says what was found. These stores' savepoints record no
/// commit, so a refusal claiming one would be a second untrue sentence.
#[test]
fn r6_the_refusal_is_true_of_the_store_it_refuses() {
    let w = Ws::from_fixture("r6", F2);
    w.lose_head();
    let (o, rc) = w.oo(&["status"]);
    assert!(rc != 0, "not refused (rc=0): {o}");
    assert!(!o.contains("savepoint records a commit"), "the refusal claims a savepoint note this store does not have: {o}");
}

/// Migrating does not supply the missing notes, and must not need to.
/// Baseline: after `migrate` to the current layout, gc still deletes 5/5.
#[test]
fn r7_a_migrated_old_store_is_still_recognised() {
    let w = Ws::from_fixture("r7", F2);
    w.ok(&["migrate", "--grant", "migrate"]);
    w.lose_head();
    let before = w.store();
    let (o, rc) = w.oo(&["gc", "--grant", "gc"]);
    assert!(w.store() == before && rc != 0, "gc on a migrated lost old store (rc={rc}): {o}");
}
