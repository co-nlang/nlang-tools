// Where there is no universe.
// Order: nlang-tools/docs/where_there_is_no_universe_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-064.  Ruling: meta/oo/STATUS.md D82.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// Measured on v0.65.0, in an empty directory: 14 of 17 commands create
// `.oo/` (with its two declarations) — `status`, `log`, `eval`, `run`,
// `test`, `inspect`, `repl`, `node id`, `node peers`, `gc`, `migrate`,
// `commit`, `evolve`; the node family adds `node trust add` (a `.oo/` holding
// only `discovery.n`, no declaration), `node serve` (`.oo/peers/`),
// `node affiliate`, `discover`, `advertise`, `find-node`. Four of them also
// say something untrue: `migrate` "Nothing was changed" (it just created the
// store), `run` describes itself as not writing the workspace, `commit`
// fails and creates anyway, `inspect` "not found in local store" (the store
// it just made).
//
// D82 (user, 2026-09-30, 乙): where there is no store, reading does not
// write; a command that needs a universe to answer refuses by name (rc≠0);
// a command that does not need one answers and writes nothing; the only
// command that creates a store is `evolve`. It does not decide the shape of
// a future `init` (commit.md §4.4, arc D item 2).
//
// D82 ② (甲): D82 governs the UNIVERSE, not the directory. `.oo/` holds two
// layers — the universe (declarations, objects, HEAD, proposals, savepoints)
// and node settings (`discovery.n`, `peers/`), which storage.rs already
// treats as living there before a store exists. `node` commands keep reading
// and writing node settings where there is no universe; they never write a
// declaration or universe content.
// D82 ③ (甲): where there is no universe, `~%Engine./save` answers ⊥ with the
// newly registered cause `#no_universe` and no address — an address would
// claim the value was kept.
//
// Folded in (no new ruling; REAL_02 §5.1.1 "absent ⟹ refuse to open", MUST,
// and "no declaration written on a read path", MUST NOT): a store that holds
// only proposals (staged / injections / savepoints, no objects) and has lost
// its `format` is read as a NEW store, and a read-only `status` writes the
// CURRENT layout onto it — on every engine measured since v0.27.0. A store
// with commits is refused correctly. The engine's "is there a store here"
// test is "are there objects", which is the same predicate as D82's.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Wording-free except: `evolve` (the one way to start a universe, which a
// refusal must name — the command's name), and the four untrue sentences
// the defect prints ("Nothing was changed", "not found in local store",
// "no committed root yet", "Universe is static"). Identity and node home
// live outside the workspace directory.
//
// The delivery may NOT edit this file. If a pin is wrong, say so in the
// report. `rustfmt` must not touch it.
//
// Baseline measured 2026-09-30 on dev 0442198 / oo v0.65.0: see the order.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const NO_CAID: &str = "hash:sha256:v1:0000000000000000000000000000000000000000000000000000000000000000";
const KEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const NOWHERE: &str = "127.0.0.1:1";

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("no-universe-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        fs::write(ws.join("c.n"), "c: ~%Math./add (1, 2)\n").unwrap();
        fs::write(ws.join("t.n"), "test_t: 1\n").unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
        c.args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        c
    }

    fn oo_in(&self, args: &[&str], stdin: &str) -> (String, i32) {
        let mut child = self.cmd(args).spawn().expect("oo runs");
        child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
        let o = child.wait_with_output().unwrap();
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
            o.status.code().unwrap_or(-1),
        )
    }

    fn oo(&self, args: &[&str]) -> (String, i32) {
        self.oo_in(args, "exit\n")
    }

    fn ok(&self, args: &[&str]) -> String {
        let (o, rc) = self.oo(args);
        assert_eq!(rc, 0, "setup `oo {}`: {o}", args.join(" "));
        o
    }

    /// `node serve` runs until killed. Returns (still serving after 3 s, output, rc).
    fn serve(&self, port: u16) -> (bool, String, i32) {
        let p = port.to_string();
        let mut child = self.cmd(&["node", "serve", "-p", &p]).spawn().expect("oo runs");
        drop(child.stdin.take());
        let start = Instant::now();
        loop {
            if let Some(_) = child.try_wait().unwrap() {
                let o = child.wait_with_output().unwrap();
                return (
                    false,
                    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
                    o.status.code().unwrap_or(-1),
                );
            }
            if start.elapsed() > Duration::from_secs(3) {
                let _ = child.kill();
                let _ = child.wait();
                return (true, String::new(), 0);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn dot_oo(&self) -> PathBuf {
        self.ws.join(".oo")
    }

    /// Every entry of the workspace directory except the two input files.
    fn written(&self) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(&self.ws)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n != "c.n" && n != "t.n")
            .collect();
        v.sort();
        v
    }

    fn store(&self) -> BTreeMap<String, Vec<u8>> {
        let mut m = BTreeMap::new();
        files(&self.dot_oo(), &self.dot_oo(), &mut m);
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
                let rel = q.strip_prefix(base).unwrap().to_string_lossy().to_string();
                out.insert(rel, fs::read(&q).unwrap());
            }
        }
    }
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

/// Commands that need a universe to answer (D82): each must refuse with
/// rc≠0 in a directory with no store.
fn needs_a_universe() -> Vec<Vec<&'static str>> {
    vec![
        vec!["status"],
        vec!["log"],
        vec!["commit", "-m", "x"],
        vec!["gc", "--grant", "gc"],
        vec!["migrate", "--grant", "migrate"],
        vec!["squash", NO_CAID, "--grant", "squash"],
        vec!["refine", "-s", NO_CAID, "-t", NO_CAID, "-m", "r"],
        vec!["rollback", NO_CAID, "--grant", "rollback"],
        // AMENDED 2026-10-04 for Q-071 (D89): `repl` is an interactive eval
        // and answers from an empty root where there is no universe; it
        // moved to needs_no_universe(), where r1 still requires it to write
        // nothing. The new answer is measured in an_interactive_eval r4.
        vec!["inspect", NO_CAID],
    ]
}

/// Node commands (D82 ②): where there is no universe they may leave node
/// settings under `.oo/` and nothing else.
fn node_commands() -> Vec<Vec<&'static str>> {
    vec![
        vec!["node", "id"],
        vec!["node", "affiliate"],
        vec!["node", "peers"],
        vec!["node", "trust", "list"],
        vec!["node", "trust", "add", KEY],
        vec!["node", "trust", "remove", KEY],
        vec!["node", "discover", "--to", NOWHERE, "--target", NO_CAID],
        vec!["node", "advertise", "--to", NOWHERE],
        vec!["node", "find-node", "--to", NOWHERE, "--target", "0000000000000000000000000000000000000000"],
    ]
}

/// Files under `.oo/` other than node settings.
fn universe_files(w: &Ws) -> Vec<String> {
    w.store()
        .keys()
        .filter(|k| k.as_str() != "discovery.n" && !k.starts_with("peers/"))
        .cloned()
        .collect()
}

/// Commands that do not need a universe: each must answer without writing.
fn needs_no_universe() -> Vec<Vec<&'static str>> {
    vec![
        vec!["eval", "~%Math./add (1, 2)"],
        vec!["eval", "_."],
        vec!["run", "c.n", "-o", "c"],
        vec!["test", "t.n"],
        vec!["fmt", "c.n"],
        vec!["lint", "c.n"],
        vec!["identity"],
        vec!["repl"], // AMENDED 2026-10-04 for Q-071 (D89): see needs_a_universe()
    ]
}

// ── Guards (green on v0.65.0, must stay green) ───────────────────────────

/// `evolve` starts a universe; after it, the honest empty store answers
/// (SPEC_08 §6.2.1) and `commit` records the first commit.
#[test]
fn g1_evolve_starts_a_universe_and_it_answers() {
    let w = Ws::new("g1");
    w.ok(&["evolve", "c.n"]);
    let format = fs::read_to_string(w.dot_oo().join("format")).expect("evolve wrote no layout declaration");
    assert!(format.starts_with("layout="), "{format}");
    w.ok(&["status"]);
    w.ok(&["log"]);
    w.ok(&["commit", "-m", "first"]);
    let log = w.ok(&["log"]);
    assert!(log.contains("commit hash:"), "{log}");
}

/// Commands that need no universe still give their answers where there is
/// none (the known answers, not only rc).
#[test]
fn g2_store_free_commands_still_answer() {
    let w = Ws::new("g2");
    assert_eq!(w.ok(&["eval", "~%Math./add (1, 2)"]).trim(), "3");
    assert_eq!(w.ok(&["eval", "~%Math./add (1, 3)"]).trim(), "4");
    assert_eq!(w.ok(&["eval", "_."]).trim(), "{}");
    assert!(w.ok(&["run", "c.n", "-o", "c"]).trim().starts_with('3'));
    w.ok(&["test", "t.n"]);
    assert!(w.ok(&["fmt", "c.n"]).contains("c:"));
    w.ok(&["lint", "c.n"]);
    w.ok(&["identity"]);
    w.ok(&["node", "id"]);
}

/// A store with a commit that lost its layout declaration is refused and
/// nothing is written (REAL_02 §5.1.1) — correct on every engine measured.
#[test]
fn g3_a_committed_store_without_its_declaration_is_refused() {
    let w = Ws::new("g3");
    w.ok(&["evolve", "c.n"]);
    w.ok(&["commit", "-m", "a"]);
    fs::remove_file(w.dot_oo().join("format")).unwrap();
    let before = w.store();
    let (o, rc) = w.oo(&["status"]);
    assert!(rc != 0 && w.store() == before, "rc={rc}: {o}");
}

/// Inside an existing store, the node family keeps writing what it writes.
#[test]
fn g4_an_existing_store_keeps_its_node_state() {
    let w = Ws::new("g4");
    w.ok(&["evolve", "c.n"]);
    w.ok(&["node", "trust", "add", KEY]);
    let list = w.ok(&["node", "trust", "list"]);
    assert!(list.contains(KEY), "{list}");
    w.ok(&["node", "peers"]);
    w.ok(&["status"]);
}

/// An older store opens and its declaration is not touched.
#[test]
fn g5_an_older_store_keeps_its_declaration() {
    let w = Ws::new("g5");
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/layout7_repo/oo_dir"),
        &w.dot_oo(),
    );
    let before = w.store();
    w.ok(&["status"]);
    w.ok(&["log"]);
    assert!(w.store() == before, "reading an older store changed it");
}

/// Node settings live without a universe (D82 ②): trust add writes only
/// `discovery.n`, and the list reads it back.
#[test]
fn g6_node_settings_live_without_a_universe() {
    let w = Ws::new("g6");
    w.ok(&["node", "trust", "add", KEY]);
    assert_eq!(w.store().keys().cloned().collect::<Vec<_>>(), vec!["discovery.n".to_string()]);
    let list = w.ok(&["node", "trust", "list"]);
    assert!(list.contains(KEY), "{list}");
    w.ok(&["node", "peers"]);
}

/// Inside a universe, `save` keeps the value and returns its address.
#[test]
fn g7_save_inside_a_universe_keeps_the_value() {
    let w = Ws::new("g7");
    w.ok(&["evolve", "t.n"]);
    let before = w.store().keys().filter(|k| k.starts_with("objects/")).count();
    let o = w.ok(&["eval", "~%Engine./save { a: 1 }"]);
    assert!(o.contains("hash:sha256:"), "{o}");
    let after = w.store().keys().filter(|k| k.starts_with("objects/")).count();
    assert!(after > before, "save returned an address and kept nothing: {o}");
}

// ── Reds ─────────────────────────────────────────────────────────────────

/// Baseline: every command below except fmt / lint / identity / trust list
/// creates `.oo/` with its declarations. Only `evolve` may create a
/// universe; non-node commands write nothing at all; node commands may
/// leave node settings and nothing else (D82 ②).
#[test]
fn r1_nothing_but_evolve_creates_a_universe() {
    let mut bad = Vec::new();
    for (i, args) in needs_a_universe().into_iter().chain(needs_no_universe()).enumerate() {
        let w = Ws::new(&format!("r1-{i}"));
        let (o, rc) = w.oo(&args);
        let written = w.written();
        if !written.is_empty() {
            bad.push(format!("`oo {}` (rc={rc}) wrote {:?} — {}", args.join(" "), written, o.lines().next().unwrap_or("")));
        }
    }
    for (i, args) in node_commands().into_iter().enumerate() {
        let w = Ws::new(&format!("r1-node-{i}"));
        let (o, rc) = w.oo(&args);
        let u = universe_files(&w);
        let other: Vec<String> = w.written().into_iter().filter(|n| n != ".oo").collect();
        if !u.is_empty() || !other.is_empty() {
            bad.push(format!("`oo {}` (rc={rc}) wrote universe files {:?} / {:?} — {}", args.join(" "), u, other, o.lines().next().unwrap_or("")));
        }
    }
    let w = Ws::new("r1-serve");
    let (serving, o, rc) = w.serve(18761);
    let u = universe_files(&w);
    if !u.is_empty() {
        bad.push(format!("`oo node serve` (serving={serving}, rc={rc}) wrote universe files {:?} — {o}", u));
    }
    assert!(bad.is_empty(), "a universe was created where there is none:\n{}", bad.join("\n"));
}

/// Baseline: status / log / gc / migrate / repl rc=0. A command that needs a
/// universe must refuse where there is none.
#[test]
fn r2_a_command_that_needs_a_universe_refuses() {
    let mut bad = Vec::new();
    for (i, args) in needs_a_universe().into_iter().enumerate() {
        let w = Ws::new(&format!("r2-{i}"));
        let (o, rc) = w.oo(&args);
        if rc == 0 {
            bad.push(format!("`oo {}` rc=0 — {}", args.join(" "), o.lines().next().unwrap_or("")));
        }
    }
    assert!(bad.is_empty(), "answered without a universe:\n{}", bad.join("\n"));
}

/// The refusal names the way to start one, and the four untrue sentences
/// are gone. Baseline: status "no committed root yet … Universe is static",
/// migrate "Nothing was changed", inspect "not found in local store".
#[test]
fn r3_the_answer_is_true_where_there_is_no_universe() {
    let mut bad = Vec::new();
    for args in [vec!["status"], vec!["log"], vec!["gc", "--grant", "gc"], vec!["migrate", "--grant", "migrate"]] {
        let w = Ws::new("r3");
        let (o, _) = w.oo(&args);
        if !o.contains("evolve") {
            bad.push(format!("`oo {}` does not name `evolve`: {o}", args.join(" ")));
        }
    }
    for (args, untrue) in [
        (vec!["status"], "no committed root yet"),
        (vec!["status"], "Universe is static"),
        (vec!["migrate", "--grant", "migrate"], "Nothing was changed"),
        (vec!["inspect", NO_CAID], "not found in local store"),
    ] {
        let w = Ws::new("r3u");
        let (o, _) = w.oo(&args);
        if o.contains(untrue) {
            bad.push(format!("`oo {}` says \"{untrue}\": {o}", args.join(" ")));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Baseline: a store holding only proposals (no objects) whose `format` is
/// gone is read as new; `status` and `evolve` write the current layout onto
/// it. Each command on a fresh copy.
#[test]
fn r4_a_store_of_proposals_without_its_declaration_is_not_a_new_store() {
    let mut bad = Vec::new();
    for (i, args) in [vec!["status"], vec!["log"], vec!["evolve", "t.n"], vec!["gc", "--grant", "gc"]].into_iter().enumerate() {
        let w = Ws::new(&format!("r4-{i}"));
        w.ok(&["evolve", "c.n"]);
        fs::remove_file(w.dot_oo().join("format")).unwrap();
        let before = w.store();
        let (o, rc) = w.oo(&args);
        let changed = w.store() != before;
        if rc == 0 || changed {
            bad.push(format!("`oo {}` rc={rc}, store changed={changed} — {}", args.join(" "), o.lines().next().unwrap_or("")));
        }
    }
    assert!(bad.is_empty(), "a store missing its declaration was opened as a new one:\n{}", bad.join("\n"));
}

/// Baseline: a `.oo/` holding only `discovery.n` or only `peers/` (what
/// trust add and serve leave today) is answered as an empty universe and
/// declared by a read. Whether such a `.oo/` is pre-store configuration
/// (storage.rs says `discovery.n` may live there before a store exists) or a
/// store missing its declaration, `status` must neither answer it as a
/// universe nor write a declaration onto it. The `discovery.n` bytes are the
/// ones the engine itself writes.
#[test]
fn r5_a_dot_oo_without_a_universe_is_neither_answered_nor_declared() {
    let mut bad = Vec::new();
    for (name, make) in [
        ("discovery.n", Box::new(|d: &Path| fs::write(d.join("discovery.n"), format!("affiliation_roots: [\n    \"{KEY}\",\n]\n")).unwrap()) as Box<dyn Fn(&Path)>),
        ("peers/directory", Box::new(|d: &Path| {
            fs::create_dir_all(d.join("peers")).unwrap();
            fs::write(d.join("peers/directory"), "").unwrap();
        })),
    ] {
        let w = Ws::new("r5");
        fs::create_dir_all(w.dot_oo()).unwrap();
        make(&w.dot_oo());
        let before = w.store();
        let (o, rc) = w.oo(&["status"]);
        if rc == 0 || w.store() != before {
            bad.push(format!("[{name}] `oo status` rc={rc}, store changed={} — {}", w.store() != before, o.lines().next().unwrap_or("")));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Baseline: `save` returns an address and creates `.oo/` to hold it. Where
/// there is no universe it answers ⊥ `#no_universe` and no address (D82 ③).
#[test]
fn r6_save_where_there_is_no_universe_keeps_nothing_and_says_so() {
    let w = Ws::new("r6");
    let (o, rc) = w.oo(&["eval", "~%Engine./save { a: 1 }"]);
    let written = w.written();
    assert!(written.is_empty(), "save wrote {:?} where there is no universe (rc={rc}): {o}", written);
    assert!(!o.contains("hash:sha256:"), "save returned an address for a value it did not keep: {o}");
    assert!(o.contains("_|_") && o.contains("#no_universe"), "save did not answer ⊥ #no_universe: {o}");
}
