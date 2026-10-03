// One writer at a time.
// Order: nlang-tools/docs/one_writer_at_a_time_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-069.  Ruling: meta/oo/STATUS.md D87.
// Spec: SPEC_10 §4.1 (strong atomicity; the late arriver); SPEC_08 §6.2.1
// (`#gc` declares it expects exclusive use — unchanged by D87).
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// Four commands move HEAD: commit, refine, squash, rollback. Only commit
// takes the commit critical section (an exclusive lock on `.oo/format`,
// Q-016b). Measured on v0.70.0: commit beside refine loses the landed
// commit 30/30, squash beside refine 10/10, squash beside squash 3/10,
// refine beside refine 1/10 — both sides rc=0, the loser's commit gone
// from history, collected by the next gc. And `migrate` replaces
// `.oo/format` by rename without taking the lock, so after it a commit
// enters the critical section while another still holds it.
//
// D87 (user, 2026-10-03, 甲): the operations that move HEAD are
// serializable — each reads HEAD inside the critical section, and the
// result is as if they ran one after another in some order. `#gc` keeps
// its declared exclusive-use (not in this arc). Reads do not wait.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// The red cells use the engine's own critical section as the instrument:
// the probe holds the same exclusive lock commit takes, starts a command,
// waits, and looks at whether HEAD moved while the section was held. Two
// cells race real processes and look only at the outcome (every commit a
// command reported is in history); on the baseline they lose nearly every
// time, on a correct engine never. Wording-free.
//
// The delivery may NOT edit this file or the fixtures. If a pin is wrong,
// say so in the report. `rustfmt` must not touch it.
//
// Baseline measured 2026-10-03 on dev 2d052cc / oo v0.70.0: see the order.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const HOLD: Duration = Duration::from_millis(1500);

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("one-writer-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    fn from_fixture(tag: &str, fixture: &str) -> Self {
        let w = Ws::new(tag);
        copy_dir(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(fixture).join("oo_dir"),
            &w.ws.join(".oo"),
        );
        w
    }

    fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
        c.args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
            .stdin(Stdio::null());
        c
    }

    fn oo(&self, args: &[&str]) -> (String, i32) {
        let o = self.cmd(args).output().expect("oo runs");
        (
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
            o.status.code().unwrap_or(-1),
        )
    }

    fn spawn(&self, args: &[&str]) -> Child {
        self.cmd(args).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("oo spawns")
    }

    fn ok(&self, args: &[&str]) -> String {
        let (o, rc) = self.oo(args);
        assert_eq!(rc, 0, "setup `oo {}`: {o}", args.join(" "));
        o
    }

    fn committed(&self, name: &str, text: &str) -> String {
        fs::write(self.ws.join(name), text).unwrap();
        self.ok(&["evolve", name]);
        self.ok(&["commit", "-m", name]);
        self.head()
    }

    fn head(&self) -> String {
        fs::read_to_string(self.ws.join(".oo/HEAD")).unwrap().trim().to_string()
    }

    fn root_of(&self, commit: &str) -> String {
        let o = self.ok(&["inspect", commit]);
        o.lines()
            .find_map(|l| l.strip_prefix("root:").map(|r| r.trim().to_string()))
            .unwrap_or_else(|| panic!("VOID READING: no root line: {o}"))
    }

    /// Take the commit critical section the way the engine does.
    fn hold(&self) -> fs::File {
        let f = fs::OpenOptions::new().read(true).write(true).open(self.ws.join(".oo/format")).unwrap();
        f.lock().unwrap();
        f
    }

    fn log_has(&self, caid: &str) -> bool {
        self.ok(&["log"]).contains(caid.rsplit(':').next().unwrap())
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

fn finish(c: Child) -> (String, i32) {
    let o = c.wait_with_output().unwrap();
    (
        format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string(),
        o.status.code().unwrap_or(-1),
    )
}

/// The commit a command reported it wrote (the full CAID after the colon of
/// its first line), if any.
fn reported(out: &str) -> Option<String> {
    let first = out.lines().next()?;
    let (_, rest) = first.split_once(": ")?;
    let caid = rest.split_whitespace().next()?;
    if caid.starts_with("hash:") { Some(caid.to_string()) } else { None }
}

/// While the probe holds the critical section, `args` must not move HEAD;
/// once released, it completes and HEAD has moved.
fn waits_for_the_section(w: &Ws, args: &[&str]) {
    let before = w.head();
    let lock = w.hold();
    let mut c = w.spawn(args);
    std::thread::sleep(HOLD);
    let moved_while_held = w.head() != before;
    let finished_while_held = c.try_wait().unwrap().is_some();
    lock.unlock().unwrap();
    drop(lock);
    let (o, rc) = finish(c);
    assert!(
        !moved_while_held,
        "`oo {}` moved HEAD while another writer held the critical section (finished={finished_while_held}): {o}",
        args.join(" ")
    );
    assert_eq!(rc, 0, "VOID READING: `oo {}` did not complete after the section was released: {o}", args.join(" "));
    assert_ne!(w.head(), before, "VOID READING: `oo {}` completed without moving HEAD: {o}", args.join(" "));
}

// ── Red: every HEAD writer waits for the section ────────────────────────

/// r1 — refine. Baseline: moves HEAD while the section is held.
#[test]
fn r1_refine_waits_for_the_section() {
    let w = Ws::new("r1");
    let a = w.committed("a.n", "a: 1\n");
    let root = w.root_of(&a);
    waits_for_the_section(&w, &["refine", "--source", &root, "--target", &root, "-m", "r"]);
}

/// r2 — squash.
#[test]
fn r2_squash_waits_for_the_section() {
    let w = Ws::new("r2");
    let a = w.committed("a.n", "a: 1\n");
    w.committed("b.n", "b: 2\n");
    w.committed("c.n", "c: 3\n");
    waits_for_the_section(&w, &["squash", "--grant", "squash", &a]);
}

/// r3 — rollback.
#[test]
fn r3_rollback_waits_for_the_section() {
    let w = Ws::new("r3");
    let a = w.committed("a.n", "a: 1\n");
    w.committed("b.n", "b: 2\n");
    waits_for_the_section(&w, &["rollback", &a, "--grant", "rollback"]);
}

/// r4 — the section still excludes after `migrate` replaced the file it
/// lives on. Baseline: migrate swaps `.oo/format`; a commit then locks the
/// new file and lands while the old one is still held.
#[test]
fn r4_migrate_does_not_open_the_section() {
    let w = Ws::from_fixture("r4", "layout7_repo");
    fs::write(w.ws.join("x.n"), "x: 1\n").unwrap();
    w.ok(&["evolve", "x.n"]);
    let before = w.head();
    let lock = w.hold();
    let m = w.spawn(&["migrate", "--grant", "migrate"]);
    std::thread::sleep(Duration::from_millis(500));
    let mut c = w.spawn(&["commit", "-m", "c"]);
    std::thread::sleep(HOLD);
    let moved_while_held = w.head() != before;
    let finished = c.try_wait().unwrap().is_some();
    lock.unlock().unwrap();
    drop(lock);
    let (mo, mrc) = finish(m);
    let (co, crc) = finish(c);
    assert!(!moved_while_held, "a commit landed while the section was held, after migrate (finished={finished}): {co}");
    assert_eq!(mrc, 0, "VOID READING: migrate did not complete: {mo}");
    assert_eq!(crc, 0, "VOID READING: the commit did not complete: {co}");
    assert!(fs::read_to_string(w.ws.join(".oo/format")).unwrap().contains("layout=8"), "VOID READING: migrate did not advance the declaration");
}

/// r5 — outcome: commit raced against refine. Every commit a command
/// reported is in history. Baseline: the landed commit is lost 30/30.
#[test]
fn r5_commit_beside_refine_loses_nothing() {
    for t in 0..5 {
        let w = Ws::new(&format!("r5-{t}"));
        let a = w.committed("a.n", "a: 1\n");
        let root = w.root_of(&a);
        fs::write(w.ws.join("x.n"), "x: 1\n").unwrap();
        w.ok(&["evolve", "x.n"]);
        let c = w.spawn(&["commit", "-m", "c"]);
        let r = w.spawn(&["refine", "--source", &root, "--target", &root, "-m", "r"]);
        let (co, crc) = finish(c);
        let (ro, rrc) = finish(r);
        assert_eq!((crc, rrc), (0, 0), "trial {t}: commit {co} / refine {ro}");
        for (who, o) in [("commit", &co), ("refine", &ro)] {
            let caid = reported(o).unwrap_or_else(|| panic!("VOID READING: trial {t}: {who} reported no commit: {o}"));
            assert!(w.log_has(&caid), "trial {t}: {who} reported {caid} and it is not in history");
        }
    }
}

/// r6 — outcome: squash raced against refine. AMENDED 2026-10-03 for Q-069
/// R-1 (acceptor's error): the first version required both reported commits
/// in history, and that is not what serializability gives — in the serial
/// order refine-then-squash, the squash folds the refine away, rightly. What
/// every serial order does give: the squash's own commit is in history (it
/// is HEAD, or the parent of the refine that came after it). Baseline: the
/// squash is the one lost, 10/10. Trials alternate which command starts
/// first, so both serial orders are exercised (the first version always
/// started squash first, and passed a lock-only reference by that luck).
#[test]
fn r6_squash_beside_refine_loses_nothing() {
    for t in 0..6 {
        let w = Ws::new(&format!("r6-{t}"));
        let a = w.committed("a.n", "a: 1\n");
        w.committed("b.n", "b: 2\n");
        let c = w.committed("c.n", "c: 3\n");
        let root = w.root_of(&c);
        let squash = ["squash", "--grant", "squash", a.as_str()];
        let refine = ["refine", "--source", root.as_str(), "--target", root.as_str(), "-m", "r"];
        let (s, r) = if t % 2 == 0 {
            let s = w.spawn(&squash);
            (s, w.spawn(&refine))
        } else {
            let r = w.spawn(&refine);
            (w.spawn(&squash), r)
        };
        let (so, src) = finish(s);
        let (ro, rrc) = finish(r);
        assert_eq!((src, rrc), (0, 0), "trial {t}: squash {so} / refine {ro}");
        let caid = reported(&so).unwrap_or_else(|| panic!("VOID READING: trial {t}: squash reported no commit: {so}"));
        assert!(w.log_has(&caid), "trial {t}: squash reported {caid} and it is not in history (refine: {ro})");
    }
}

// ── Green: what must not wait ────────────────────────────────────────────

/// While the section is held, `args` answers.
fn answers_while_held(w: &Ws, args: &[&str]) {
    let lock = w.hold();
    let mut c = w.spawn(args);
    std::thread::sleep(HOLD);
    let done = c.try_wait().unwrap().is_some();
    lock.unlock().unwrap();
    drop(lock);
    let (o, rc) = finish(c);
    assert!(done, "`oo {}` waited for the commit critical section: {o}", args.join(" "));
    assert_eq!(rc, 0, "`oo {}`: {o}", args.join(" "));
}

/// g1 — reads do not wait: status, log, inspect.
#[test]
fn g1_reads_do_not_wait() {
    let w = Ws::new("g1");
    let a = w.committed("a.n", "a: 1\n");
    answers_while_held(&w, &["status"]);
    answers_while_held(&w, &["log"]);
    answers_while_held(&w, &["inspect", &a]);
}

/// g2 — evolve does not wait (concurrent injection is D49's, not this arc's).
#[test]
fn g2_evolve_does_not_wait() {
    let w = Ws::new("g2");
    w.committed("a.n", "a: 1\n");
    fs::write(w.ws.join("y.n"), "y: 2\n").unwrap();
    answers_while_held(&w, &["evolve", "y.n"]);
}

/// g3 — gc keeps its declared exclusive use (D87 甲: not in this arc).
#[test]
fn g3_gc_is_not_in_this_arc() {
    let w = Ws::new("g3");
    w.committed("a.n", "a: 1\n");
    answers_while_held(&w, &["gc", "--dry-run", "--grant", "gc"]);
}

/// g4 — one after another, every writer still lands (control for r5/r6).
#[test]
fn g4_sequential_writers_all_land() {
    let w = Ws::new("g4");
    let a = w.committed("a.n", "a: 1\n");
    let b = w.committed("b.n", "b: 2\n");
    let root = w.root_of(&b);
    let (ro, _) = w.oo(&["refine", "--source", &root, "--target", &root, "-m", "r"]);
    let r = reported(&ro).unwrap_or_else(|| panic!("VOID READING: refine reported no commit: {ro}"));
    assert!(w.log_has(&r), "{ro}");
    let (so, src) = w.oo(&["squash", "--grant", "squash", &a]);
    assert_eq!(src, 0, "{so}");
    let s = reported(&so).unwrap_or_else(|| panic!("VOID READING: squash reported no commit: {so}"));
    assert!(w.log_has(&s), "{so}");
    w.ok(&["rollback", &a, "--grant", "rollback"]);
    assert_eq!(w.head(), a);
}
