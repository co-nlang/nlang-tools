// A successful `commit` can leave a repo that neither engine will open.
// Order: nlang-tools/docs/a_commit_that_closes_the_door_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-038.  Ruling: meta/oo/STATUS.md O73.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// Run one `evolve` + `commit` against any repo written before the sentinel
// (`REAL_03` §6.8: a root names its standard root by one digest). The commit
// reports success and exits 0. Afterwards `status`, `log` and `evolve` all
// die with
//
//     refusing root: standard root digest 47dc540c… is unavailable
//
// and the engine that created the repo cannot open it either:
//
//     store format version layout=2 is not supported by this engine
//
// Two independent defects, one action, on two different axes.
//
//   (A) The commit writes a root that names digest `47dc540c…` and never
//       stores the table it names. Measured: `objects/sha256/47/dc540c…`
//       does not exist, and two repos with unrelated contents brick to the
//       SAME digest — it is the hash of the empty standard table, which is
//       what `standard_for_root` returns for a pre-sentinel repo. §6.8's
//       third MUST then obliges every reader to refuse.
//
//   (B) The same commit rewrites `.oo/format` from `2` to `layout=2` and adds
//       `.oo/objects.format`. Measured by isolating the two files: only the
//       `format` line locks the old engine out — with `objects.format` left
//       in place and `format` restored to `2`, the OLD engine reads the NEW
//       engine's commit in full. So the bytes were always readable; what
//       closed the door was a declaration nobody asked for.
//
// ── The ruling (O73, 2026-08-25, user) ───────────────────────────────────
//
//   1. `commit` must not touch any format declaration.
//   2. A root written into a pre-sentinel repo stays self-contained and names
//      no digest — so `47dc540c…` is never written, rather than being fixed
//      by storing an empty table.
//   3. Migrating is something you ask for, never a side effect of `commit`.
//   4. Migration advances the container layout only and does not touch any
//      root — so §6.8's "name it by one digest" does not apply and nothing
//      needs to be stored.
//
// ── The fixture ──────────────────────────────────────────────────────────
//
// `fixtures/pre_sentinel_repo/` was built by the real `oo v0.20.0` binary.
// It is checked in because generating one needs an engine that predates the
// sentinel and no such binary ships here. See its README.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason stated
// in each. The delivery may remove `#[ignore]` and NOTHING else in this file.
// If a pin here is wrong, say so in the report — do not edit it.
//
// Baseline measured 2026-08-25 on dev ae85c8c / oo v0.33.0: 3 green, 5 red.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("pre_sentinel_repo")
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("closes-the-door-{tag}"))
}

fn oo(dir: &Path, args: &[&str]) -> String {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    let o = c.args(args).output().expect("oo runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("mkdir");
    for e in fs::read_dir(from).expect("readdir") {
        let e = e.expect("entry");
        let dst = to.join(e.file_name());
        if e.file_type().expect("file type").is_dir() {
            copy_tree(&e.path(), &dst);
        } else {
            fs::copy(e.path(), &dst).expect("copy");
        }
    }
}

/// Lay the checked-in pre-sentinel repo down in a scratch directory, as `.oo`.
fn lay_out_legacy(tag: &str) -> nlang_interpreter::ScratchDir {
    let d = scratch(tag);
    let f = fixture();
    copy_tree(&f.join("oo_dir"), &d.path().join(".oo"));
    fs::copy(f.join("main.n"), d.path().join("main.n")).expect("main.n");
    d
}

fn read_decl(dir: &Path) -> (String, Option<String>) {
    let layout = fs::read_to_string(dir.join(".oo/format")).unwrap_or_default();
    let enc = fs::read_to_string(dir.join(".oo/objects.format")).ok();
    (layout, enc)
}

fn head(dir: &Path) -> String {
    fs::read_to_string(dir.join(".oo/HEAD")).unwrap_or_default()
}

// ─────────────────────────────────────────────────────────────────────────
// RED — what the arc must make true.
// ─────────────────────────────────────────────────────────────────────────

/// The load-bearing claim: one successful commit must not make a repo
/// unopenable. Everything else in this file is a way that claim can fail.
///
/// Baseline: `status` answers `… 47dc540c… (unavailable)` and `log` errors.
#[test]
fn r1_a_pre_sentinel_repo_stays_openable_after_a_commit() {
    let d = lay_out_legacy("r1");
    let before = oo(d.path(), &["log"]);
    assert!(
        before.contains("commit hash:"),
        "REACH: the fixture must open before we write to it; got {before:?}"
    );

    fs::write(d.path().join("main.n"), "app: { k1: 1, k2: 2 }\n").expect("write");
    let ev = oo(d.path(), &["evolve", "main.n"]);
    let ci = oo(d.path(), &["commit", "-m", "x"]);
    assert!(
        ci.contains("Commit successful"),
        "REACH: the commit must report success (it does today); got {ev:?} / {ci:?}"
    );

    let after = oo(d.path(), &["log"]);
    assert!(
        after.contains("commit hash:"),
        "a commit that reported success left a repo whose history will not \
         open. got {after:?}"
    );
}

/// (B). The declaration is the repo's claim about itself (O53), and `commit`
/// is not entitled to change it. Restoring this one line is, measurably, the
/// whole difference between the creating engine reading the repo and not.
///
/// Baseline: `2` becomes `layout=2` and `objects.format` appears.
#[test]
fn r2_a_commit_does_not_rewrite_the_layout_declaration() {
    let d = lay_out_legacy("r2");
    let (before, before_enc) = read_decl(d.path());
    assert_eq!(
        before.trim(),
        "2",
        "REACH: the fixture must declare the old layout; got {before:?}"
    );
    assert!(before_enc.is_none(), "REACH: fixture has no objects.format");

    fs::write(d.path().join("main.n"), "app: { k1: 1, k2: 2 }\n").expect("write");
    oo(d.path(), &["evolve", "main.n"]);
    oo(d.path(), &["commit", "-m", "x"]);

    let (after, after_enc) = read_decl(d.path());
    assert_eq!(
        after.trim(),
        "2",
        "commit must leave the layout declaration exactly as it found it; \
         got {after:?}"
    );
    assert!(
        after_enc.is_none(),
        "commit must not add a declaration the repo did not carry; got {after_enc:?}"
    );
}

/// (A). A root written into a pre-sentinel repo stays self-contained, so no
/// digest is named and there is nothing to store. This is the pin that says
/// the phantom is removed rather than fed: the fix must not be "also write
/// the empty table into the store".
///
/// Baseline: `status` names `47dc540c…`.
#[test]
fn r3_a_root_written_into_a_pre_sentinel_repo_stays_self_contained() {
    let d = lay_out_legacy("r3");
    fs::write(d.path().join("main.n"), "app: { k1: 1, k2: 2 }\n").expect("write");
    oo(d.path(), &["evolve", "main.n"]);
    oo(d.path(), &["commit", "-m", "x"]);

    let st = oo(d.path(), &["status"]);
    assert!(
        !st.contains("47dc540c"),
        "the empty standard table's digest must never be written into a root. \
         got {st:?}"
    );
    assert!(
        !st.contains("unavailable"),
        "no root written by this engine may name a dependency this engine \
         does not have. got {st:?}"
    );

    // And the store must not have gained the empty table either: the ruling
    // removes the phantom, it does not stock it.
    let phantom = d.path().join(".oo/objects/sha256/47");
    assert!(
        !phantom.exists(),
        "the fix must not be to store the empty table; {phantom:?} exists"
    );
}

/// Migrating is something you ask for. This repo already has a spelling for
/// "you must ask": `rollback`, `squash` and `gc` all require `--grant <verb>`.
/// Without the grant, `migrate` must refuse and change nothing.
///
/// Baseline: there is no `migrate` subcommand at all.
#[test]
fn r4_migrating_without_a_grant_refuses_and_changes_nothing() {
    let d = lay_out_legacy("r4");
    let (before, before_enc) = read_decl(d.path());
    let head_before = head(d.path());

    let out = oo(d.path(), &["migrate"]);
    assert!(
        !out.contains("unrecognized subcommand"),
        "migrating must be something you can ask for; got {out:?}"
    );

    let (after, after_enc) = read_decl(d.path());
    assert_eq!(after, before, "a refused migrate must not touch the layout");
    assert_eq!(after_enc, before_enc, "a refused migrate must not add files");
    assert_eq!(head(d.path()), head_before, "a refused migrate must not move HEAD");
}

/// A granted migrate advances the container and nothing else. The roots are
/// not touched, so HEAD does not move and the history still reads — this is
/// the half of O73 that says migration is a container operation, not a
/// rewrite of anybody's identity.
///
/// Baseline: there is no `migrate` subcommand at all.
#[test]
fn r5_a_granted_migrate_moves_the_container_and_not_the_root() {
    let d = lay_out_legacy("r5");
    let head_before = head(d.path());

    let out = oo(d.path(), &["migrate", "--grant", "migrate"]);
    assert!(
        !out.contains("unrecognized subcommand"),
        "REACH: migrate must exist; got {out:?}"
    );

    let (after, after_enc) = read_decl(d.path());
    assert_eq!(
        after.trim(),
        "layout=2",
        "a granted migrate advances the layout declaration; got {after:?}"
    );
    assert!(
        after_enc.is_some(),
        "the new layout carries an encoding declaration; got {after_enc:?}"
    );

    assert_eq!(
        head(d.path()),
        head_before,
        "migration is a container operation: HEAD must not move"
    );
    let lg = oo(d.path(), &["log"]);
    assert!(
        lg.contains("commit hash:"),
        "the history must still read after migrating; got {lg:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// GREEN — what the arc must NOT break.
// ─────────────────────────────────────────────────────────────────────────

/// A repo this engine created keeps its own declarations across commits. The
/// rule is "do not change what you found", not "never write a declaration".
#[test]
fn g1_a_current_repo_keeps_its_own_declarations() {
    let d = scratch("g1");
    fs::write(d.path().join("main.n"), "app: { a: 1 }\n").expect("write");
    oo(d.path(), &["evolve", "main.n"]);
    oo(d.path(), &["commit", "-m", "one"]);
    let (l1, e1) = read_decl(d.path());
    assert_eq!(l1.trim(), "layout=2", "REACH: a fresh repo declares its layout");
    assert!(e1.is_some(), "REACH: a fresh repo declares its encoding");

    fs::write(d.path().join("main.n"), "app: { a: 1, b: 2 }\n").expect("write");
    oo(d.path(), &["evolve", "main.n"]);
    oo(d.path(), &["commit", "-m", "two"]);
    let (l2, e2) = read_decl(d.path());
    assert_eq!(l2, l1, "a second commit must not move the layout declaration");
    assert_eq!(e2, e1, "a second commit must not move the encoding declaration");

    let st = oo(d.path(), &["status"]);
    assert!(
        st.contains("(available)"),
        "a current repo still names a standard root it has; got {st:?}"
    );
}

/// O53, as a test rather than a report line: reading must never write. In a
/// pre-sentinel repo, `status`, `log` and `evolve` must leave every
/// declaration and HEAD exactly as they were. This is green today and the
/// repair must not buy openability by widening what a read may do.
#[test]
fn g2_reading_a_pre_sentinel_repo_writes_nothing() {
    for cmd in ["status", "log", "evolve"] {
        let d = lay_out_legacy(&format!("g2-{cmd}"));
        let (l0, e0) = read_decl(d.path());
        let h0 = head(d.path());

        if cmd == "evolve" {
            fs::write(d.path().join("main.n"), "app: { k1: 1, k9: 9 }\n").expect("write");
            oo(d.path(), &["evolve", "main.n"]);
        } else {
            oo(d.path(), &[cmd]);
        }

        let (l1, e1) = read_decl(d.path());
        assert_eq!(l1, l0, "`{cmd}` must not rewrite the layout declaration");
        assert_eq!(e1, e0, "`{cmd}` must not add an encoding declaration");
        assert_eq!(head(d.path()), h0, "`{cmd}` must not move HEAD");
    }
}

/// Identity red line. Nothing in this arc may move an address: the same
/// source must still commit to the same root object, and the standard root
/// this engine names must not change.
#[test]
fn g3_the_identity_of_a_current_repo_does_not_move() {
    let d = scratch("g3");
    fs::write(d.path().join("main.n"), "app: { k1: 1 }\n").expect("write");
    oo(d.path(), &["evolve", "main.n"]);
    let ci = oo(d.path(), &["commit", "-m", "x"]);
    let caid = ci
        .split_whitespace()
        .find(|w| w.starts_with("hash:sha256:v1:"))
        .expect("commit prints a CAID")
        .to_string();

    let ins = oo(d.path(), &["inspect", &caid]);
    assert!(
        ins.contains("932a9f9dd62297a7cb3cb9c9fb56907a06a8c4d4e945cc3dfc4782a6987fb0cb"),
        "the root of `app: {{ k1: 1 }}` must not move; got {ins:?}"
    );

    let st = oo(d.path(), &["status"]);
    assert!(
        st.contains("7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911"),
        "the standard root digest must not move; got {st:?}"
    );
}

/// ── ADDED BY THE ACCEPTOR, 2026-08-25 (Q-038 repair round) ───────────────
///
/// Regression fence. This property was GREEN at the order's baseline
/// (`dev ae85c8c`) and is RED on delivery `0a7b6f0`; no test in the tree
/// covered it, which is why the whole suite stayed green.
///
/// `encoding=3` is NOT pre-sentinel. Such a store names its standard root by
/// one digest (`REAL_03` §6.8) and addresses its roots by the hydrated body
/// (O63). O73 ② is about pre-sentinel stores, and the write path must tell
/// the two apart by the same predicate the READ path already uses — O54:
/// "hydrate only roots that carry the sentinel", i.e. whether the standard
/// table is there, not what number the encoding axis says.
///
/// Measured on the two real binaries, same source, same synthetic store:
///   baseline `oo v0.33.0` → root `c8fca4d9…`, status names `7038e250…`
///   delivery `0a7b6f0`    → root `e1b4d90f…`, status says self-contained
/// and, worse, a store whose FIRST commit named the standard root drops to
/// self-contained when the second commit is written.
#[test]
fn g4_an_encoding_3_store_keeps_its_sentinel() {
    let d = scratch("g4");
    let oo_dir = d.path().join(".oo");
    fs::create_dir_all(oo_dir.join("objects")).expect("mkdir");
    fs::write(oo_dir.join("format"), "layout=2").expect("layout");
    fs::write(oo_dir.join("objects.format"), "encoding=3").expect("encoding");
    fs::write(oo_dir.join("objects").join(".legacy-fixture-anchor"), b"").expect("anchor");

    fs::write(d.path().join("main.n"), "app: { k1: 1 }\n").expect("write");
    oo(d.path(), &["evolve", "main.n"]);
    let ci = oo(d.path(), &["commit", "-m", "one"]);
    assert!(
        ci.contains("Commit successful"),
        "REACH: the encoding-3 store must accept a commit; got {ci:?}"
    );

    let st = oo(d.path(), &["status"]);
    assert!(
        !st.contains("self-contained"),
        "an encoding-3 store is not pre-sentinel: its root must name a \
         standard root. got {st:?}"
    );
    assert!(
        st.contains("7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911"),
        "the root must name THIS engine's standard root by digest \
         (REAL_03 §6.8, first two MUSTs). got {st:?}"
    );

    // The half that bites hardest: a second commit must not change the rule
    // the store's history is already written under.
    fs::write(d.path().join("main.n"), "app: { k1: 1, k2: 2 }\n").expect("write");
    oo(d.path(), &["evolve", "main.n"]);
    oo(d.path(), &["commit", "-m", "two"]);
    let st2 = oo(d.path(), &["status"]);
    assert!(
        !st2.contains("self-contained"),
        "a second commit silently dropped the sentinel this store's history \
         was written under. got {st2:?}"
    );
}
