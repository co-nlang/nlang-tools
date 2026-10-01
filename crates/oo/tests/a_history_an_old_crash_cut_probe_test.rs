// A history an old crash cut.
// Order: nlang-tools/docs/a_history_an_old_crash_cut_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-067.  Ruling: meta/oo/STATUS.md D85.
// Spec: SPEC_08 §6.2.1 (the roots of `#gc`; lost context is not empty).
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// Every engine from v0.41.0 through v0.67.0 moved HEAD before it wrote the
// commit circle (measured by strace on the tag builds; v0.68.0 is the first
// to write the circle first). A crash in that window leaves HEAD on a commit
// that no circle declares. Since D52 a commit names its predecessor only on
// its circle, so the walk from HEAD stops there, and `gc` deletes the
// history before the cut as unreachable — rc=0. If work continued on top,
// the cut is buried in the middle of the history and the same thing happens.
//
// D85 (user, 2026-10-02, 甲): walking the ancestry from HEAD, a new-form
// (value-addressed) commit that no circle declares is evidence that the
// history was cut. New-form commits exist only from v0.59.0, and every
// engine since writes a commit circle for every commit (squash and refine
// included), so the evidence has no innocent reading. `gc` refuses by name
// before deleting anything. Old-form commits (v0.41–v0.58 crashes) are a
// written gap: they cannot be told from a pre-D52 genesis without reading
// content or time, so they are not detected. The walk is from HEAD: a cut
// on an abandoned branch is not a cut in this history.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// A crash is reconstructed by removing, after a normal operation, the circle
// that declares the commit (the file the crash would not have written).
// Wording-free: the refusal is pinned by rc, by `.oo/` unchanged, and by the
// cut commit's digest appearing in the output.
//
// The delivery may NOT edit this file or the fixtures. If a pin is wrong,
// say so in the report. `rustfmt` must not touch it.
//
// Baseline measured 2026-10-02 on dev 76d7059 / oo v0.68.0: see the order.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("old-crash-cut-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    fn from_fixture(tag: &str, fixture: &str) -> Self {
        let w = Ws::new(tag);
        copy_dir(&fixture_dir(fixture), &w.ws.join(".oo"));
        w
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

    fn committed(&self, name: &str, text: &str) -> String {
        fs::write(self.ws.join(name), text).unwrap();
        self.ok(&["evolve", name]);
        self.ok(&["commit", "-m", name]);
        self.head()
    }

    fn dot_oo(&self) -> PathBuf {
        self.ws.join(".oo")
    }

    fn head(&self) -> String {
        fs::read_to_string(self.dot_oo().join("HEAD")).unwrap().trim().to_string()
    }

    fn root_of(&self, commit: &str) -> String {
        let o = self.ok(&["inspect", commit]);
        o.lines()
            .find_map(|l| l.strip_prefix("root:").map(|r| r.trim().to_string()))
            .unwrap_or_else(|| panic!("VOID READING: no root line: {o}"))
    }

    /// Remove the circle that declares `commit` as a commit — the file the
    /// old order had not yet written when it crashed.
    fn crash_before_circle(&self, commit: &str) {
        let hex = hex_of(commit);
        let mut hit = 0;
        for e in fs::read_dir(self.dot_oo().join("savepoints")).unwrap().flatten() {
            let s = fs::read_to_string(e.path()).unwrap_or_default();
            if s.lines().any(|l| l.trim() == format!("commit: {hex}")) {
                fs::remove_file(e.path()).unwrap();
                hit += 1;
            }
        }
        assert_eq!(hit, 1, "VOID READING: expected exactly one circle declaring {hex}, found {hit}");
    }

    fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        let mut m = BTreeMap::new();
        files(&self.dot_oo(), &self.dot_oo(), &mut m);
        m
    }

    fn object_present(&self, commit: &str) -> bool {
        let hex = hex_of(commit);
        self.dot_oo().join("objects/sha256").join(&hex[..2]).join(&hex[2..]).exists()
    }
}

fn hex_of(caid: &str) -> String {
    let h = caid.rsplit(':').next().unwrap().to_string();
    assert_eq!(h.len(), 64, "VOID READING: not a digest: {caid}");
    h
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
                out.insert(q.strip_prefix(base).unwrap().display().to_string(), fs::read(&q).unwrap());
            }
        }
    }
}

/// The refusal D85 asks for: rc≠0, `.oo/` untouched, the cut commit named.
fn assert_refused_by_name(w: &Ws, args: &[&str], cut: &str, earlier: &str) {
    assert!(w.object_present(earlier), "VOID READING: the earlier commit is already gone before gc");
    let before = w.snapshot();
    let (o, rc) = w.oo(args);
    let after = w.snapshot();
    assert!(rc != 0, "`oo {}` answered rc=0 over a cut history: {o}", args.join(" "));
    assert!(before == after, "`oo {}` changed .oo/ over a cut history: {o}", args.join(" "));
    assert!(w.object_present(earlier), "the commit before the cut was deleted: {o}");
    assert!(o.contains(&hex_of(cut)), "the refusal does not name the commit no circle declares ({}): {o}", hex_of(cut));
}

// ── Red: the cut, wherever the old order could leave it ─────────────────

/// r1 — a commit's circle never written; HEAD on the cut.
#[test]
fn r1_cut_at_head_gc_refuses_by_name() {
    let w = Ws::new("r1");
    let a = w.committed("a.n", "a: 1\n");
    let b = w.committed("b.n", "b: 2\n");
    w.crash_before_circle(&b);
    assert_refused_by_name(&w, &["gc", "--grant", "gc"], &b, &a);
}

/// r2 — a dry run must not report the history before the cut as garbage.
#[test]
fn r2_cut_at_head_dry_run_refuses_by_name() {
    let w = Ws::new("r2");
    let a = w.committed("a.n", "a: 1\n");
    let b = w.committed("b.n", "b: 2\n");
    w.crash_before_circle(&b);
    assert_refused_by_name(&w, &["gc", "--dry-run", "--grant", "gc"], &b, &a);
}

/// r3 — work continued after the crash; the cut is in the middle.
#[test]
fn r3_cut_buried_under_later_commits_gc_refuses_by_name() {
    let w = Ws::new("r3");
    let a = w.committed("a.n", "a: 1\n");
    let b = w.committed("b.n", "b: 2\n");
    w.crash_before_circle(&b);
    let _c = w.committed("c.n", "c: 3\n");
    assert_refused_by_name(&w, &["gc", "--grant", "gc"], &b, &a);
}

/// r4 — the same window in squash.
#[test]
fn r4_cut_left_by_squash_gc_refuses_by_name() {
    let w = Ws::new("r4");
    let a = w.committed("a.n", "a: 1\n");
    w.committed("b.n", "b: 2\n");
    w.committed("c.n", "c: 3\n");
    w.ok(&["squash", "--grant", "squash", &a]);
    let s = w.head();
    w.crash_before_circle(&s);
    assert_refused_by_name(&w, &["gc", "--grant", "gc"], &s, &a);
}

/// r5 — the same window in refine.
#[test]
fn r5_cut_left_by_refine_gc_refuses_by_name() {
    let w = Ws::new("r5");
    let a = w.committed("a.n", "a: 1\n");
    let root = w.root_of(&a);
    w.ok(&["refine", "--source", &root, "--target", &root, "-m", "r"]);
    let r = w.head();
    assert_ne!(r, a, "VOID READING: refine did not move HEAD");
    w.crash_before_circle(&r);
    assert_refused_by_name(&w, &["gc", "--grant", "gc"], &r, &a);
}

// ── Green: no innocent store is refused ─────────────────────────────────

/// g1 — an intact store collects as before.
#[test]
fn g1_intact_history_collects() {
    let w = Ws::new("g1");
    let a = w.committed("a.n", "a: 1\n");
    let (o, rc) = w.oo(&["gc", "--grant", "gc"]);
    assert_eq!(rc, 0, "{o}");
    w.committed("b.n", "b: 2\n");
    let (o, rc) = w.oo(&["gc", "--grant", "gc"]);
    assert_eq!(rc, 0, "{o}");
    assert!(w.object_present(&a), "an intact history lost a commit: {o}");
}

/// g2 — after a squash, commits are declared by circles and unreachable
/// from HEAD, and collecting them is right. The measured confounder.
#[test]
fn g2_squash_leftovers_are_collected() {
    let w = Ws::new("g2");
    let a = w.committed("a.n", "a: 1\n");
    let b = w.committed("b.n", "b: 2\n");
    w.committed("c.n", "c: 3\n");
    w.ok(&["squash", "--grant", "squash", &a]);
    let (o, rc) = w.oo(&["gc", "--grant", "gc"]);
    assert_eq!(rc, 0, "{o}");
    assert!(!w.object_present(&b), "a squashed-away commit was not collected: {o}");
    assert!(w.object_present(&a), "the squash base was collected: {o}");
}

/// g3 — after a rollback, the abandoned head's content is collected
/// (an abandonment is not a root, SPEC_08 §6.2.1).
#[test]
fn g3_rollback_leftovers_are_collected() {
    let w = Ws::new("g3");
    let a = w.committed("a.n", "a: 1\n");
    let b = w.committed("b.n", "b: 2\n");
    w.ok(&["rollback", &a, "--grant", "rollback"]);
    let (o, rc) = w.oo(&["gc", "--grant", "gc"]);
    assert_eq!(rc, 0, "{o}");
    assert!(!w.object_present(&b), "the abandoned commit was not collected: {o}");
}

/// g4 — the walk is from HEAD: a cut on a branch that was rolled back past
/// is not a cut in this history.
#[test]
fn g4_cut_on_an_abandoned_branch_is_not_this_history() {
    let w = Ws::new("g4");
    let a = w.committed("a.n", "a: 1\n");
    let b = w.committed("b.n", "b: 2\n");
    w.crash_before_circle(&b);
    w.ok(&["rollback", &a, "--grant", "rollback"]);
    // A commit after the rollback records the abandonment of `b`, so the
    // collector's abandoned-content walk does reach the cut commit.
    let c = w.committed("c.n", "c: 3\n");
    assert!(
        String::from_utf8_lossy(&fs::read(w.dot_oo().join("objects/sha256").join(&hex_of(&c)[..2]).join(&hex_of(&c)[2..])).unwrap())
            .contains(&hex_of(&b)),
        "VOID READING: the commit after the rollback does not name the abandoned head"
    );
    let (o, rc) = w.oo(&["gc", "--grant", "gc"]);
    assert_eq!(rc, 0, "{o}");
    assert!(w.object_present(&a), "{o}");
}

// Not `pre_sentinel_repo` (real v0.20.0): its root was written before the
// v0.56.0 epoch and this engine reads it as #caid_mismatch by design
// (Q-053); it is not an intact store to this engine.
const FIXTURES: [&str; 6] = [
    "encoding4_repo",      // real v0.35.0, JSON
    "layout2_framed_repo", // real v0.40.0, framed, no commit notes
    "layout5_repo",        // real v0.58.0, old-form commits with notes
    "layout5_signed_repo", // real v0.58.0
    "layout6_signed_repo", // real v0.60.0, new-form commits
    "layout7_repo",        // real v0.63.0
];

/// g5 — every real store, as written by its engine, collects.
#[test]
fn g5_real_stores_as_written_collect() {
    for f in FIXTURES {
        let w = Ws::from_fixture(&format!("g5-{f}"), f);
        let (o, rc) = w.oo(&["gc", "--dry-run", "--grant", "gc"]);
        assert_eq!(rc, 0, "{f}: dry run refused an intact real store: {o}");
        let (o, rc) = w.oo(&["gc", "--grant", "gc"]);
        assert_eq!(rc, 0, "{f}: gc refused an intact real store: {o}");
    }
}

/// g6 — every real store, continued by this engine, collects; and once
/// migrated, a new-form commit whose predecessor is old-form collects too.
#[test]
fn g6_real_stores_continued_and_migrated_collect() {
    for f in FIXTURES {
        let w = Ws::from_fixture(&format!("g6-{f}"), f);
        let before = w.head();
        w.committed("c.n", "c: 3\n");
        let (o, rc) = w.oo(&["gc", "--grant", "gc"]);
        assert_eq!(rc, 0, "{f}: gc refused a continued real store: {o}");
        assert!(w.object_present(&before), "{f}: the fixture's head was collected: {o}");
        w.ok(&["migrate", "--grant", "migrate"]);
        w.committed("d.n", "d: 4\n");
        assert!(w.head().contains(":v2:"), "VOID READING: {f}: no new-form commit after migrate: {}", w.head());
        let (o, rc) = w.oo(&["gc", "--grant", "gc"]);
        assert_eq!(rc, 0, "{f}: gc refused a migrated real store: {o}");
        assert!(w.object_present(&before), "{f}: the fixture's head was collected after migrate: {o}");
    }
}
