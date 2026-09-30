// What did not land.
// Order: nlang-tools/docs/what_did_not_land_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-065.  Ruling: meta/oo/STATUS.md D83.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// The host boundary on the write side, and the durable records under `.oo/`
// that are read before they are rewritten or consumed. Measured on v0.66.0:
//
//  M1  `.oo/abandoned` unreadable ⟹ `rollback` rc=0 and overwrites it
//      (2 lines → 1: an abandoned head is forgotten); `commit` rc=0, the new
//      commit carries NO `abandoned:` field, and the file is deleted. With a
//      readable file the commit carries `abandoned: [...]`. "This stretch was
//      abandoned" leaves history for good (SPEC_08 §6.2 R1; REAL_03 §6.6).
//  M2  `.oo/injections/` not writable ⟹ `commit` says "Commit successful",
//      HEAD moves, the folded injection stays; `status` lists the committed
//      `a: 1` as a staged proposal and the next commit folds it again.
//  M3  `objects/` not writable ⟹ `~%Engine./save` answers ⊥ `#conflict`.
//  M4  a directory the host will not write ⟹ `~%Io./write_file` and
//      `~%Io./append_file` answer `#none` (control: `#true`).
//
// D83 (user, 2026-09-30, 甲): a write the host refused answers ⊥ with the
// newly registered cause `#unwritable` — never `#conflict`, never `#none`.
// The rest is existing MUSTs: REAL_03 §6.6 (what cannot be read is not
// absent), SPEC_08 §6.2 R1 (an abandoned stretch happened), and Q-060 I2
// (what a command says is what happened).
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Faults are made with permissions only; every fault is checked to have
// taken effect before it is relied on (a probe run as root is a void
// reading). Wording-free except the causes `#unwritable`, `#conflict`,
// `#none` and the field name `abandoned:` on the commit object's own bytes.
//
// The delivery may NOT edit this file. If a pin is wrong, say so in the
// report. `rustfmt` must not touch it.
//
// Baseline measured 2026-09-30 on dev ac2a39d / oo v0.66.0: see the order.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Drop for Ws {
    fn drop(&mut self) {
        restore(&self.ws);
    }
}

fn restore(p: &Path) {
    if let Ok(d) = fs::read_dir(p) {
        for e in d.flatten() {
            let q = e.path();
            let _ = fs::set_permissions(&q, fs::Permissions::from_mode(if q.is_dir() { 0o755 } else { 0o644 }));
            if q.is_dir() {
                restore(&q);
            }
        }
    }
    let _ = fs::set_permissions(p, fs::Permissions::from_mode(0o755));
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("did-not-land-{tag}"));
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
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
            o.status.code().unwrap_or(-1),
        )
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

    fn dot_oo(&self) -> PathBuf {
        self.ws.join(".oo")
    }

    fn head(&self) -> String {
        fs::read_to_string(self.dot_oo().join("HEAD")).unwrap().trim().to_string()
    }

    /// Every readable file under `.oo/`, and the names of unreadable ones.
    fn store(&self) -> BTreeMap<String, Vec<u8>> {
        let mut m = BTreeMap::new();
        files(&self.dot_oo(), &self.dot_oo(), &mut m);
        m
    }

    /// The commit object HEAD names, as bytes on disk.
    fn head_object(&self) -> String {
        let h = self.head();
        let hex = h.rsplit(':').next().unwrap();
        let p = self.dot_oo().join("objects/sha256").join(&hex[..2]).join(&hex[2..]);
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("VOID READING: no commit object at {}: {e}", p.display()))
    }

    /// Two commits, then a rollback to the first: `.oo/abandoned` holds one
    /// line, and the first commit is HEAD.
    fn with_abandoned(tag: &str) -> Self {
        let w = Ws::new(tag);
        w.committed("a.n", "a: 1\n");
        let first = w.head();
        w.committed("b.n", "b: 2\n");
        w.ok(&["rollback", &first, "--grant", "rollback"]);
        let ab = fs::read_to_string(w.dot_oo().join("abandoned")).unwrap_or_default();
        if ab.lines().filter(|l| !l.trim().is_empty()).count() != 1 {
            panic!("VOID READING: expected one abandoned head after the rollback: {ab:?}");
        }
        w
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
                out.insert(rel, fs::read(&q).unwrap_or_else(|_| b"<unreadable>".to_vec()));
            }
        }
    }
}

fn chmod(p: &Path, mode: u32) {
    fs::set_permissions(p, fs::Permissions::from_mode(mode)).unwrap();
}

/// Undo a fault. The faulted file may be gone — deleting it is one of the
/// defects measured here.
fn relax(p: &Path, mode: u32) {
    let _ = fs::set_permissions(p, fs::Permissions::from_mode(mode));
}

/// A fault that did not take effect is a void reading.
fn unreadable(p: &Path) {
    chmod(p, 0o000);
    if fs::read(p).is_ok() {
        panic!("VOID READING: {} is still readable (running as root?)", p.display());
    }
}

fn unwritable_dir(p: &Path) {
    chmod(p, 0o555);
    let probe = p.join(".probe-write");
    if fs::write(&probe, b"x").is_ok() {
        let _ = fs::remove_file(&probe);
        panic!("VOID READING: {} is still writable (running as root?)", p.display());
    }
}

// ── Guards (green on v0.66.0, must stay green) ───────────────────────────

/// A readable abandoned record reaches the next commit and is then cleared.
#[test]
fn g1_a_readable_abandoned_record_reaches_the_commit() {
    let w = Ws::with_abandoned("g1");
    w.committed("c.n", "c: 3\n");
    let obj = w.head_object();
    assert!(obj.contains("abandoned:"), "the commit does not carry the abandoned head: {obj}");
    assert!(!w.dot_oo().join("abandoned").exists(), "the abandoned record outlived its commit");
}

/// Writes the host allows still answer as before.
#[test]
fn g2_writes_the_host_allows_still_answer() {
    let w = Ws::new("g2");
    let target = w.root.join("out.txt");
    let (o, rc) = w.oo(&["eval", &format!("~%Io./write_file (\"{}\", \"hi\")", target.display())]);
    assert!(rc == 0 && o.contains("#true"), "write_file: rc={rc} {o}");
    let (o, rc) = w.oo(&["eval", &format!("~%Io./append_file (\"{}\", \"!\")", target.display())]);
    assert!(rc == 0 && o.contains("#true"), "append_file: rc={rc} {o}");
    assert_eq!(fs::read_to_string(&target).unwrap(), "hi!");
    fs::write(w.ws.join("t.n"), "t: 0\n").unwrap();
    w.ok(&["evolve", "t.n"]);
    let o = w.ok(&["eval", "~%Engine./save { a: 1 }"]);
    assert!(o.contains("hash:sha256:"), "save in a universe: {o}");
}

/// An ordinary commit consumes its working set.
#[test]
fn g3_an_ordinary_commit_consumes_its_working_set() {
    let w = Ws::new("g3");
    w.committed("a.n", "a: 1\n");
    assert_eq!(fs::read_dir(w.dot_oo().join("injections")).unwrap().count(), 0);
    let (o, rc) = w.oo(&["commit", "-m", "again"]);
    assert!(rc != 0 && o.contains("Nothing to commit"), "rc={rc}: {o}");
}

// ── Reds ─────────────────────────────────────────────────────────────────

/// Baseline: rc=0, the unreadable record is replaced by one holding only
/// the new line — an abandoned head is forgotten.
#[test]
fn r1_rollback_does_not_overwrite_a_record_it_could_not_read() {
    let w = Ws::with_abandoned("r1");
    let target = fs::read_to_string(w.dot_oo().join("abandoned")).unwrap().trim().to_string();
    let ab = w.dot_oo().join("abandoned");
    let before = fs::read(&ab).unwrap();
    let store_before = w.store();
    unreadable(&ab);
    let (o, rc) = w.oo(&["rollback", &target, "--grant", "rollback"]);
    relax(&ab, 0o644);
    let after = fs::read(&ab).unwrap_or_default();
    assert!(rc != 0, "rollback answered over an abandoned record it could not read: {o}");
    assert_eq!(after, before, "the abandoned record was rewritten");
    assert!(w.store() == store_before, "rollback changed the store after refusing: {o}");
}

/// Baseline: rc=0, the commit carries no `abandoned:` and the record is
/// deleted.
#[test]
fn r2_commit_does_not_drop_an_abandoned_record_it_could_not_read() {
    let w = Ws::with_abandoned("r2");
    fs::write(w.ws.join("c.n"), "c: 3\n").unwrap();
    w.ok(&["evolve", "c.n"]);
    let head_before = w.head();
    let ab = w.dot_oo().join("abandoned");
    let before = fs::read(&ab).unwrap();
    unreadable(&ab);
    let (o, rc) = w.oo(&["commit", "-m", "c"]);
    relax(&ab, 0o644);
    let moved = w.head() != head_before;
    if moved {
        let obj = w.head_object();
        assert!(obj.contains("abandoned:"), "a commit landed without the abandoned head it could not read (rc={rc}): {o}");
    } else {
        assert!(rc != 0, "commit did not land and answered 0: {o}");
        assert_eq!(fs::read(&ab).unwrap_or_default(), before, "the abandoned record was touched");
    }
}

/// Baseline: "Commit successful", HEAD moves, and the committed `a: 1`
/// stays behind as a proposal that `status` lists and the next commit folds
/// again.
#[test]
fn r3_a_commit_that_lands_consumes_what_it_folded() {
    let w = Ws::new("r3");
    w.committed("base.n", "base: 0\n");
    fs::write(w.ws.join("a.n"), "a: 1\n").unwrap();
    w.ok(&["evolve", "a.n"]);
    let head_before = w.head();
    let inj = w.dot_oo().join("injections");
    unwritable_dir(&inj);
    let (o, rc) = w.oo(&["commit", "-m", "a"]);
    relax(&inj, 0o755);
    let moved = w.head() != head_before;
    if moved {
        let st = w.ok(&["status"]);
        assert!(!st.contains("a: 1"), "the commit landed (rc={rc}: {o}) and `status` still lists what it folded:\n{st}");
        let (c, crc) = w.oo(&["commit", "-m", "again"]);
        assert!(crc != 0 && c.contains("Nothing to commit"), "the next commit found something to commit after a landed one: rc={crc} {c}");
    } else {
        assert!(rc != 0, "commit did not land and answered 0: {o}");
    }
}

/// Baseline: ⊥ `#conflict`. A write the host refused is `#unwritable`.
#[test]
fn r4_save_that_did_not_land_says_so() {
    let w = Ws::new("r4");
    fs::write(w.ws.join("t.n"), "t: 0\n").unwrap();
    w.ok(&["evolve", "t.n"]);
    let objects = w.dot_oo().join("objects");
    fs::create_dir_all(&objects).unwrap();
    let mut dirs = vec![objects.clone()];
    if let Ok(d) = fs::read_dir(&objects) {
        for e in d.flatten() {
            if e.path().is_dir() {
                dirs.push(e.path());
                if let Ok(dd) = fs::read_dir(e.path()) {
                    dirs.extend(dd.flatten().map(|x| x.path()).filter(|x| x.is_dir()));
                }
            }
        }
    }
    for d in dirs.iter().rev() {
        unwritable_dir(d);
    }
    let (o, rc) = w.oo(&["eval", "~%Engine./save { never_stored: 42 }"]);
    for d in &dirs {
        relax(d, 0o755);
    }
    assert!(!o.contains("hash:sha256:"), "save returned an address for a value it could not keep: {o}");
    assert!(!o.contains("#conflict"), "a refused write was answered as a lattice conflict: {o}");
    assert!(o.contains("_|_") && o.contains("#unwritable"), "save did not answer ⊥ #unwritable (rc={rc}): {o}");
}

/// Baseline: `#none` for both. `#none` says "there is nothing"; the host
/// refused the write.
#[test]
fn r5_a_write_the_host_refused_is_not_nothing() {
    let w = Ws::new("r5");
    let ro = w.root.join("ro");
    fs::create_dir_all(&ro).unwrap();
    unwritable_dir(&ro);
    let mut bad = Vec::new();
    for (op, arg) in [("write_file", "hi"), ("append_file", "hi")] {
        let (o, rc) = w.oo(&["eval", &format!("~%Io./{op} (\"{}\", \"{arg}\")", ro.join("x.txt").display())]);
        if o.contains("#none") || !(o.contains("_|_") && o.contains("#unwritable")) {
            bad.push(format!("{op}: rc={rc} {}", o.trim()));
        }
    }
    relax(&ro, 0o755);
    assert!(bad.is_empty(), "a refused write was not ⊥ #unwritable:\n{}", bad.join("\n"));
}
