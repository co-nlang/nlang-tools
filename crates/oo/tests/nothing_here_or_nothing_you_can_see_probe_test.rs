// Nothing here, or nothing you can see.
// Recon:   nlang-tools/docs/a_store_it_had_not_finished_making_recon.md
// Order:   nlang-tools/docs/nothing_here_or_nothing_you_can_see_handover.md
// Rulings: REAL_03 §6.6 (three outcomes must be distinguishable) and its
//          v0.49.0 precedent; O86 is open but this file does not depend on it.
//
// -- What this arc is ----------------------------------------------------
//
// `ObjectStore::init` decides "is this someone else's store?" by asking
// whether `.oo/format` exists -- and that is the first of two declarations
// it is itself about to write:
//
//     let new_store = !oo.join("format").exists() && !HEAD && !cas;
//     if new_store {
//         atomic_write(oo/"format",         "layout=5\n");   // (1)
//         atomic_write(oo/"objects.format", "encoding=5\n"); // (2)
//     } else { ensure_format()? }        // reads BOTH; missing one = bail
//
// Each write is atomic. The pair is not. A second process arriving between
// (1) and (2) sees `format`, concludes the store is someone else's, and
// then refuses to open it because the other declaration is not down yet.
// Two concurrent read-only commands are enough, and `oo` has no `init`
// subcommand, so an operator cannot pre-create the store to avoid it.
//
// The window is filesystem-dependent, which is why nobody saw it for six
// versions: measured 0/500 on ext4 at two-way concurrency and about 6% on
// tmpfs, and `ScratchDir` lives in the system temp dir. R1 therefore runs
// the race in BOTH places. A one-filesystem version of R1 is a void
// reading on the other machine.
//
// -- Why the guilty line must not simply be deleted ----------------------
//
// The `!format.exists()` clause was added in v0.43.0 for a stated reason:
// a prior engine may have staged injections without ever writing HEAD or a
// CAS object, and treating such a store as new would silently advance the
// layout merely by opening it, bypassing the explicit migration gate.
// G2 and G3 are that protection, pinned. If they go red the race was
// "fixed" by reintroducing the bug the clause was bought to prevent.
//
// NOT PROBED, stated so no one mistakes silence for coverage:
//   * `~%Io./exists` returns #false for a file it cannot read, which is
//     indistinguishable from one that is not there, and hands that to n/
//     programs as a value. Measured; it is the sharpest member of this
//     family. It is NOT here because what it should return instead is an
//     open ruling (O86 (ii)) and this arc is ruling-free.
//   * `.oo/peers/directory` treats an unreadable cache as a cold start, by
//     an explicit comment. Whether a peer directory is a cache or a record
//     needs a ruling too. An acceptance-time attempt to measure it was a
//     VOID READING (`oo node discover` exited 2 on a missing `--to`, so it
//     never reached the code) and is not quoted anywhere.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn cmd(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    c
}

fn oo(dir: &Path, args: &[&str]) -> (String, i32) {
    let o = cmd(dir).args(args).output().expect("oo runs");
    (
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        o.status.code().unwrap_or(-1),
    )
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("nothing-{tag}"))
}

fn read(d: &Path, rel: &str) -> Option<String> {
    fs::read_to_string(d.join(rel)).ok()
}

/// Deny every mode bit, and prove the denial actually bites. Running as
/// root would make every permission probe in this file a void reading.
fn seal(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(p, fs::Permissions::from_mode(0o000)).expect("chmod 000");
    let readable = if p.is_dir() {
        fs::read_dir(p).is_ok()
    } else {
        fs::read(p).is_ok()
    };
    assert!(
        !readable,
        "REACH: {} is still readable after chmod 000 -- running as root? \
         every permission assertion in this file would be vacuous",
        p.display()
    );
}

fn unseal(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(p, fs::Permissions::from_mode(0o755));
}

// ---------------------------------------------------------------------
// G1. The control. A fresh workspace gets exactly one complete pair of
// declarations. If this ever goes red the rest of the file is measuring
// the wrong thing.
// ---------------------------------------------------------------------
#[test]
fn g1_a_fresh_workspace_lands_both_declarations() {
    let s = scratch("g1");
    let d = s.path();
    fs::write(d.join("a.n"), "x: 1\n").expect("source");
    let (out, rc) = oo(d, &["status"]);
    assert_eq!(rc, 0, "REACH: status on a fresh workspace: {out}");
    assert_eq!(
        read(d, ".oo/format").as_deref(),
        Some("layout=5\n"),
        "the layout declaration"
    );
    assert_eq!(
        read(d, ".oo/objects.format").as_deref(),
        Some("encoding=5\n"),
        "the encoding declaration"
    );
}

// ---------------------------------------------------------------------
// G2. RED LINE. A legacy conflated store -- `.oo/format` holding a bare
// number and no `objects.format` at all -- must keep its own declaration.
// Opening it must not mint a layout it never declared.
// ---------------------------------------------------------------------
#[test]
fn g2_a_legacy_conflated_store_keeps_its_declaration() {
    let s = scratch("g2");
    let d = s.path();
    fs::create_dir_all(d.join(".oo")).expect("mkdir .oo");
    fs::write(d.join(".oo/format"), "3\n").expect("legacy declaration");
    fs::write(d.join("a.n"), "x: 1\n").expect("source");

    let (out, rc) = oo(d, &["status"]);
    assert_eq!(rc, 0, "REACH: a legacy store still opens: {out}");
    assert_eq!(
        read(d, ".oo/format").as_deref(),
        Some("3\n"),
        "the legacy declaration was rewritten merely by opening the store"
    );
    assert!(
        read(d, ".oo/objects.format").is_none(),
        "opening a legacy store minted an encoding declaration it never made"
    );
}

// ---------------------------------------------------------------------
// G3. RED LINE, and the reason the guilty line exists. A store that has
// declared itself and staged injections, but has never written HEAD or a
// CAS object, is NOT new. Opening it must not advance its layout.
// ---------------------------------------------------------------------
#[test]
fn g3_a_declared_store_without_history_is_not_a_new_store() {
    let s = scratch("g3");
    let d = s.path();
    fs::create_dir_all(d.join(".oo/injections")).expect("mkdir");
    fs::write(d.join(".oo/format"), "layout=4\n").expect("layout");
    fs::write(d.join(".oo/objects.format"), "encoding=4\n").expect("encoding");
    fs::write(d.join(".oo/injections/deadbeef"), "{}\n").expect("injection");
    fs::write(d.join("a.n"), "x: 1\n").expect("source");

    let (out, rc) = oo(d, &["status"]);
    assert_eq!(rc, 0, "REACH: such a store still opens: {out}");
    assert_eq!(
        read(d, ".oo/format").as_deref(),
        Some("layout=4\n"),
        "opening the store advanced its layout and bypassed the migration gate"
    );
    assert_eq!(
        read(d, ".oo/objects.format").as_deref(),
        Some("encoding=4\n"),
        "opening the store advanced its encoding"
    );
}

// ---------------------------------------------------------------------
// G4. v0.49.0's judgment, pinned. Absent and opaque are two answers, and
// the store layer already tells them apart. A repair to init must not
// spend that.
// ---------------------------------------------------------------------
#[test]
fn g4_absent_and_opaque_remain_two_answers() {
    let s = scratch("g4");
    let d = s.path();
    fs::write(d.join("a.n"), "x: 1\n").expect("source");
    let (out, rc) = oo(d, &["evolve", "a.n"]);
    assert_eq!(rc, 0, "REACH: evolve lands: {out}");
    let (out, rc) = oo(d, &["commit", "-m", "one"]);
    assert_eq!(rc, 0, "REACH: commit lands: {out}");

    // Opaque: the objects tree is there but cannot be read.
    let objects = d.join(".oo/objects");
    seal(&objects);
    let (opaque, opaque_rc) = oo(d, &["log"]);
    unseal(&objects);
    assert_ne!(
        opaque_rc, 0,
        "an unreadable store answered success: {opaque}"
    );
    assert!(
        opaque.contains("permission denied"),
        "opaque must be named as opaque, not as absent. Said:\n{opaque}"
    );
    assert!(
        !opaque.to_lowercase().contains("not found"),
        "opaque was reported as absent. Said:\n{opaque}"
    );

    // Absent: ask for a digest that was never stored.
    let (absent, absent_rc) = oo(
        d,
        &[
            "inspect",
            "hash:sha256:v1:0000000000000000000000000000000000000000000000000000000000000000",
        ],
    );
    assert_ne!(absent_rc, 0, "a missing object answered success: {absent}");
    assert!(
        absent.to_lowercase().contains("not found"),
        "absent must be named as absent. Said:\n{absent}"
    );
    assert!(
        !absent.contains("permission denied"),
        "absent was reported as opaque. Said:\n{absent}"
    );
}

// ---------------------------------------------------------------------
// R1. THE ONE THAT IS RED. No concurrent observer may see a half-made
// declaration.
//
// Arming, measured against the v0.49.0 tag binary before this file was
// written. Per-round probability that at least one process is refused:
//
//     ext4  (CARGO_TARGET_TMPDIR), 32 processes:  30/30 and 30/30 rounds
//     tmpfs (std::env::temp_dir), 4 processes:    11/50, 7/100 rounds
//
// Both groups run because the window's width is a property of the
// filesystem: two-way concurrency is 0/500 on ext4, and 32-way is only
// 1-6/30 on tmpfs. Either group alone is a void reading somewhere.
// ---------------------------------------------------------------------
fn race_group(root: &Path, rounds: usize, procs: usize, where_: &str) -> Vec<String> {
    let mut refusals = Vec::new();
    for r in 0..rounds {
        let d = root.join(format!("round-{r}"));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).expect("round dir");

        let kids: Vec<_> = (0..procs)
            .map(|_| {
                cmd(&d)
                    .arg("status")
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .expect("spawn oo status")
            })
            .collect();
        for k in kids {
            let o = k.wait_with_output().expect("status exits");
            if o.status.code() != Some(0) {
                refusals.push(format!(
                    "{where_} round {r}: rc={:?} {}",
                    o.status.code(),
                    String::from_utf8_lossy(&o.stderr)
                        .lines()
                        .next()
                        .unwrap_or("")
                ));
            }
        }
        assert!(
            d.join(".oo/format").exists(),
            "REACH: {where_} round {r} never created a store at all"
        );
        let _ = fs::remove_dir_all(&d);
    }
    refusals
}

#[test]
fn r1_a_concurrent_observer_never_sees_half_a_declaration() {
    let mut refusals = Vec::new();

    let ext4 = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("nothing-r1-ext4");
    let _ = fs::remove_dir_all(&ext4);
    fs::create_dir_all(&ext4).expect("target tmpdir");
    refusals.extend(race_group(&ext4, 30, 32, "target-tmpdir"));
    let _ = fs::remove_dir_all(&ext4);

    let tmp = scratch("r1-tmp");
    refusals.extend(race_group(tmp.path(), 30, 4, "system-tmpdir"));

    assert!(
        refusals.is_empty(),
        "{} of 1080 concurrent `oo status` processes were refused a store \
         that was being created for them. The declaration window is not \
         atomic. First ten:\n{}",
        refusals.len(),
        refusals
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
