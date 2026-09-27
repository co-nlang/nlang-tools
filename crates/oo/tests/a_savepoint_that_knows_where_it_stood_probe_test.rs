// A savepoint that knows where it stood.
// Order: nlang-tools/docs/a_savepoint_that_knows_where_it_stood_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-062.  Ruling: meta/oo/STATUS.md D80.
// Design note: nlang-spec/meta/oo/commit.md §1.12 / §1.12.6′ (candidate).
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// An injection savepoint (○) records a snapshot of the working set and
// nothing else. Measured on v0.63.0:
//   * the ○ chain walks backwards: `{ a: 1 b: 2 }` → commit ○ `{}` → `{ a: 1 }`;
//   * criterion (a) of SPEC_10 §3.1 ("the injection changed the universe's
//     position in the lattice") is checked against the previous ○'s bytes, so
//     re-injecting an already-committed, identical `a` mints a ○ although the
//     position did not move (Q-014b recon Q4);
//   * a ○ cannot say which point it stood on — after a rollback its body is
//     ambiguous — and the point is not derivable, since rollback leaves no
//     trace on `parents` (D55).
//
// D80 (user, 2026-09-27, Q5 甲 + Q4 甲): an injection/commit ○ records the
// context — the point (the commit it stood on) and the proposals (staged),
// separately; criterion (a) compares the lattice position (the point's root
// ⊓ the proposals). By REAL_02 §5.1.1 the new field means layout=8: a store
// declared older keeps receiving the old form until explicitly migrated.
// "How deep we looked" is the observation ○'s content (Q6, not ruled).
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// A new ○ is found as the set difference of `.oo/savepoints/` before and
// after one command. "Records the point" is checked as: the new ○ file
// contains the point commit's 64-hex digest (and not another commit's). No
// field name is pinned. `layout=8` is pinned because REAL_02 §5.1.1 fixes it.
// `fixtures/layout7_repo` was built by the real v0.63.0 binary.
//
// The delivery may NOT edit this file. If a pin is wrong, say so in the
// report. `rustfmt` must not touch it.
//
// Baseline measured 2026-09-27 on dev 2f4b8d0 / oo v0.63.0: see the order.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Ws {
    _scratch: nlang_interpreter::ScratchDir,
    root: PathBuf,
    ws: PathBuf,
}

impl Ws {
    fn new(tag: &str) -> Self {
        let scratch = nlang_interpreter::ScratchDir::new(&format!("sp-point-{tag}"));
        let root = scratch.path().to_path_buf();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        Ws { _scratch: scratch, root, ws }
    }

    fn layout7(tag: &str) -> Self {
        let w = Ws::new(tag);
        let f = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/layout7_repo");
        copy_tree(&f.join("oo_dir"), &w.ws.join(".oo"));
        w
    }

    fn oo(&self, args: &[&str]) -> (String, i32) {
        let o = Command::new(env!("CARGO_BIN_EXE_oo"))
            .args(args)
            .current_dir(&self.ws)
            .env("OO_IDENTITY", self.root.join("identity"))
            .env("OO_NODE_HOME", self.root.join("node-home"))
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

    fn write(&self, name: &str, text: &str) {
        fs::write(self.ws.join(name), text).unwrap();
    }

    fn circles(&self) -> BTreeSet<String> {
        fs::read_dir(self.ws.join(".oo/savepoints"))
            .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.len() == 32).collect())
            .unwrap_or_default()
    }

    /// Run one `evolve`; return the ○ files it minted.
    fn evolve_new(&self, name: &str) -> Vec<String> {
        let before = self.circles();
        self.ok(&["evolve", name]);
        self.circles().difference(&before).map(|id| fs::read_to_string(self.ws.join(".oo/savepoints").join(id)).unwrap()).collect()
    }

    fn head_digest(&self) -> String {
        fs::read_to_string(self.ws.join(".oo/HEAD")).unwrap().trim().rsplit(':').next().unwrap().to_string()
    }

    fn layout(&self) -> String {
        fs::read_to_string(self.ws.join(".oo/format")).unwrap().trim().to_string()
    }
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dst = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_tree(&e.path(), &dst);
        } else {
            fs::copy(e.path(), &dst).unwrap();
        }
    }
}

fn one(minted: Vec<String>, what: &str) -> String {
    if minted.len() != 1 {
        panic!("VOID READING: expected exactly one new ○ for {what}, got {}: {minted:?}", minted.len());
    }
    minted.into_iter().next().unwrap()
}

// ── Guards (green on v0.63.0, must stay green) ───────────────────────────

/// Real movement still mints: a new definition, then another.
#[test]
fn g1_an_injection_that_moves_the_position_mints() {
    let w = Ws::new("g1");
    w.write("a.n", "a: 1\n");
    w.write("b.n", "b: 2\n");
    assert_eq!(w.evolve_new("a.n").len(), 1, "a first injection did not mint");
    assert_eq!(w.evolve_new("b.n").len(), 1, "a second, different injection did not mint");
}

/// SPEC_10 §3.1 (c) MUST NOT: a sequential repeat of the same injection mints
/// nothing.
#[test]
fn g2_a_repeated_injection_mints_nothing() {
    let w = Ws::new("g2");
    w.write("a.n", "a: 1\n");
    w.evolve_new("a.n");
    assert!(w.evolve_new("a.n").is_empty(), "a repeated injection minted a ○");
}

/// A commit still mints a ○ that names the commit (D52).
#[test]
fn g3_a_commit_savepoint_names_its_commit() {
    let w = Ws::new("g3");
    w.write("a.n", "a: 1\n");
    w.ok(&["evolve", "a.n"]);
    let before = w.circles();
    w.ok(&["commit", "-m", "c"]);
    let new: Vec<String> = w.circles().difference(&before).cloned().collect();
    let text = fs::read_to_string(w.ws.join(".oo/savepoints").join(&new[0])).unwrap();
    assert!(text.contains(&w.head_digest()), "the commit ○ does not name its commit: {text}");
}

/// An unmigrated layout=7 store keeps its declaration and receives the old
/// form (no point recorded), per REAL_02 §5.1.1.
#[test]
fn g4_a_layout7_store_keeps_the_old_form() {
    let w = Ws::layout7("g4");
    w.write("c.n", "c: 3\n");
    let head = w.head_digest();
    let text = one(w.evolve_new("c.n"), "c.n in a layout=7 store");
    assert_eq!(w.layout(), "layout=7", "a write changed the declaration it found");
    assert!(!text.contains(&head), "a layout=7 store received a ○ it cannot declare: {text}");
}

// ── Reds ─────────────────────────────────────────────────────────────────

/// D80 Q4: re-injecting an already-committed, identical definition does not
/// move the position, so it mints nothing. Baseline: one ○ `{ a: 1 }`.
#[test]
fn r1_reinjecting_what_is_committed_mints_nothing() {
    let w = Ws::new("r1");
    w.write("a.n", "a: 1\n");
    w.write("b.n", "b: 2\n");
    w.ok(&["evolve", "a.n"]);
    w.ok(&["evolve", "b.n"]);
    w.ok(&["commit", "-m", "c"]);
    let minted = w.evolve_new("a.n");
    assert!(minted.is_empty(), "the position did not move and a ○ was minted: {minted:?}");
}

/// The same in an unmigrated layout=7 store: (a) is semantic, not a matter of
/// form. Baseline: one ○.
#[test]
fn r2_reinjecting_what_is_committed_mints_nothing_in_layout7() {
    let w = Ws::layout7("r2");
    w.write("a.n", "a: 1\n");
    let minted = w.evolve_new("a.n");
    assert!(minted.is_empty(), "layout=7: the position did not move and a ○ was minted: {minted:?}");
}

/// D80 Q5: an injection ○ records the point it stood on.
/// Baseline: the ○ holds only `{ c: 3 }` and a parents line.
#[test]
fn r3_an_injection_savepoint_records_its_point() {
    let w = Ws::new("r3");
    w.write("a.n", "a: 1\n");
    w.ok(&["evolve", "a.n"]);
    w.ok(&["commit", "-m", "c"]);
    let point = w.head_digest();
    w.write("c.n", "c: 3\n");
    let text = one(w.evolve_new("c.n"), "c.n");
    assert!(text.contains(&point), "the ○ does not say which point it stood on ({point}): {text}");
}

/// After a rollback the point is the rolled-back-to commit, not the latest
/// one — which is exactly what the ○ graph alone cannot recover (D55).
/// Baseline: neither digest is in the ○.
#[test]
fn r4_after_a_rollback_the_point_is_where_you_stand() {
    let w = Ws::new("r4");
    w.write("a.n", "a: 1\n");
    w.ok(&["evolve", "a.n"]);
    w.ok(&["commit", "-m", "one"]);
    let c1 = w.head_digest();
    let c1_full = fs::read_to_string(w.ws.join(".oo/HEAD")).unwrap().trim().to_string();
    w.write("b.n", "b: 2\n");
    w.ok(&["evolve", "b.n"]);
    w.ok(&["commit", "-m", "two"]);
    let c2 = w.head_digest();
    w.ok(&["rollback", &c1_full, "--grant", "rollback"]);
    w.write("d.n", "d: 4\n");
    let text = one(w.evolve_new("d.n"), "d.n after rollback");
    assert!(text.contains(&c1) && !text.contains(&c2), "the ○ does not record the point it stood on after rollback (c1 {c1}, c2 {c2}): {text}");
}

/// REAL_02 §5.1.1: the new field is a new form ⟹ a fresh store declares
/// layout=8. Baseline: layout=7.
#[test]
fn r5_a_fresh_store_declares_layout8() {
    let w = Ws::new("r5");
    w.write("a.n", "a: 1\n");
    w.ok(&["evolve", "a.n"]);
    assert_eq!(w.layout(), "layout=8");
}

/// An explicit migrate advances layout=7 to what a fresh store declares,
/// names the engines it locks out (v0.61.0 through v0.63.0 open layout 7),
/// and then injections record their point. Baseline: "already current".
#[test]
fn r6_a_migrated_layout7_store_records_points() {
    let w = Ws::layout7("r6");
    let (o, rc) = w.oo(&["migrate", "--grant", "migrate"]);
    assert_eq!(rc, 0, "migrate: {o}");
    assert_eq!(w.layout(), "layout=8", "migrate did not advance the declaration: {o}");
    assert!(o.contains("v0.61.0") && o.contains("v0.63.0"), "the cost must name v0.61.0 through v0.63.0: {o}");
    let point = w.head_digest();
    w.write("c.n", "c: 3\n");
    let text = one(w.evolve_new("c.n"), "c.n after migrate");
    assert!(text.contains(&point), "a migrated store's ○ does not record its point: {text}");
}

// ── Added at acceptance (2026-09-27), repair round R-1 ────────────────────

/// Real movement after a commit still mints, each time: the R-1 fix must not
/// become "never mint after a commit".
#[test]
fn g5_after_a_commit_new_definitions_still_mint() {
    let w = Ws::new("g5");
    w.write("a.n", "a: 1\n");
    w.ok(&["evolve", "a.n"]);
    w.ok(&["commit", "-m", "c"]);
    w.write("b.n", "b: 2\n");
    w.write("c.n", "c: 3\n");
    assert_eq!(w.evolve_new("b.n").len(), 1, "b after a commit did not mint");
    assert_eq!(w.evolve_new("c.n").len(), 1, "c after a commit did not mint");
}

/// D80 Q4 says "the injection changed the position": the comparison is the
/// position BEFORE this injection against the position AFTER it. The
/// delivery (09bd609) compares against the point's root alone, which agrees
/// with that only when the working set is empty (r1). Here it is not:
/// committed { a }, then evolve b (position { a, b }), then re-inject the
/// committed a — the position is still { a, b }, and (c) cannot stop it
/// because the proposal bodies differ ({ b } vs { a, b }).
/// Baseline: one ○ on v0.63.0 and on 09bd609. The work order's I2 said
/// "judge by root ⊓ proposals" without saying against what — the acceptor's
/// wording let this through, and r1 measured only the empty-working-set path.
#[test]
fn r7_reinjecting_a_committed_value_over_other_proposals_mints_nothing() {
    let w = Ws::new("r7");
    w.write("a.n", "a: 1\n");
    w.ok(&["evolve", "a.n"]);
    w.ok(&["commit", "-m", "c"]);
    w.write("b.n", "b: 2\n");
    assert_eq!(w.evolve_new("b.n").len(), 1, "VOID READING: b did not mint");
    let minted = w.evolve_new("a.n");
    assert!(minted.is_empty(), "the position did not move ({{a, b}} before and after) and a ○ was minted: {minted:?}");
}

/// The same in an unmigrated layout=7 store (no recorded point to lean on).
/// Baseline: one ○ on v0.63.0 and on 09bd609.
#[test]
fn r8_the_same_in_layout7() {
    let w = Ws::layout7("r8");
    w.write("b.n", "b: 2\n");
    assert_eq!(w.evolve_new("b.n").len(), 1, "VOID READING: b did not mint");
    w.write("a.n", "a: 1\n");
    let minted = w.evolve_new("a.n");
    assert!(minted.is_empty(), "layout=7: the position did not move and a ○ was minted: {minted:?}");
}
