// The world would not say.
// Ruling: nlang-spec/meta/oo/STATUS.md D66 (carrier follows the question;
//         the difference is recorded in %cause, never in a new tag value).
// Order:  nlang-tools/docs/the_world_would_not_say_handover.md
// Recon:  nlang-tools/docs/a_store_it_had_not_finished_making_recon.md §8.1, §9
//
// -- What this arc is ----------------------------------------------------
//
// `~%Io./exists` answers #false for a file it cannot read, which is the
// same answer it gives for a file that is not there. `~%Io./read_file`
// answers #none for both. Measured on v0.49.0+Q-045, four host failures
// each, one answer each:
//
//     exists     ENOENT #false  EACCES #false  ENOTDIR #false  ELOOP #false
//     read_file  ENOENT #none   EACCES #none   ENOTDIR #none   ELOOP #none
//
// The model is already inside the same two functions. When the store
// boundary refuses, both return `_|_ ;; %cause: #store_boundary` -- a
// carrier plus a registered cause. Only the host-error path collapses.
//
// D67 (which re-rules D66) splits it by the question, not by the error,
// and the carrier for "no answer" is the UNDER-determined end of the
// lattice, not the over-determined one:
//
//   the question is good and answerable      -> the value (#true/#false/text)
//   the question is good, the world will
//     not say (EACCES, EIO)                  -> _  + a registered %cause
//   a pure reference cycle (ELOOP)           -> _  + a registered %cause
//   you may not ask (store boundary)         -> _|_ + #store_boundary  (done)
//
// Two cases that look like failures are NOT failures, and G3 pins them:
//
//   ENOENT                     nothing is there. #false is true.
//   ENOTDIR (a/b where a is a
//     regular file)            nothing CAN be there, for anyone, ever.
//                              #false is true, and it is the same answer.
//   a dangling symlink         the OS resolves it and finds nothing:
//                              try_exists() is Ok(false), not Err. #false.
//
// Why not `_|_` for the two that have no answer: `_|_` says the coordinate
// is over-determined -- that is what `1 & 2` gives. A filesystem never
// hands you contradictory information; it either tells you or it does not.
// And `_|_` is the meet annihilator (measured: `(1 & 2) & 1` is `_|_`),
// so one operator's permission problem would destroy another's data, and
// a later successful read would turn `_|_` into `#true`, which refinement
// forbids. `_` is the meet identity (measured: `_ & 1` is `1`), so it
// pollutes nobody, and narrowing it later is an ordinary refinement.
//
// Why not `#blur`: TAG_REGISTRY 2.7.3 -- `#blur` claims an addressable
// snapshot, and measured, every `#blur` really does carry a %caid
// (`fuel: 0` on `c: 3` still prints one). EACCES produces no partial
// observation to address. And 1.2 gives `#blur` the remedy "raise
// %fuel", which for a permission denial is an invalid remedy -- the exact
// failure 2.7.1, 2.7.2 and 2.7.3 all exist to prevent.
//
// ELOOP is `_` and not `_|_` because the registry already draws this line
// for cycles: `#divergent` (dynamic non-termination, a cycle WITH a
// transformation) is carried on `_|_`, while `#static_cycle` (a pure
// reference cycle) is carried on Top and is explicitly 非錯誤. A symlink
// loop is a name pointing at a name. It is the second one.
//
// NOT PROBED, stated so no one mistakes silence for coverage:
//   * `~%Io./write_file` and `~%Io./append_file` answer #none on every
//     failure too, which is the same collapse. They are ACTIONS, not
//     observations, so what a write that provably did not happen should
//     answer is a separate question. Measured and filed as O87.
//   * ENAMETOOLONG, ENOMEM and every other errno. The order asks for a
//     TOTAL mapping, and the acceptor will test errnos this file does not.

use std::fs;
use std::path::Path;
use std::process::Command;

fn cmd(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    c
}

fn eval(dir: &Path, expr: &str) -> String {
    let o = cmd(dir).args(["eval", expr]).output().expect("oo runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
    .trim()
    .to_string()
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("world-{tag}"))
}

/// A workspace with: a readable file, a file inside a sealed directory, a
/// symlink loop, and a committed store so the boundary path is live.
fn world(tag: &str) -> nlang_interpreter::ScratchDir {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let s = scratch(tag);
    let d = s.path();
    fs::create_dir_all(d.join("open")).expect("mkdir open");
    fs::create_dir_all(d.join("locked")).expect("mkdir locked");
    fs::write(d.join("open/f.txt"), "hi\n").expect("write open");
    fs::write(d.join("locked/f.txt"), "hi\n").expect("write locked");
    symlink("loop", d.join("open/loop")).expect("symlink loop");
    fs::write(d.join("a.n"), "x: 1\n").expect("source");
    let o = cmd(d).args(["evolve", "a.n"]).output().expect("evolve");
    assert!(o.status.success(), "REACH: evolve must land");
    let o = cmd(d)
        .args(["commit", "-m", "one"])
        .output()
        .expect("commit");
    assert!(o.status.success(), "REACH: commit must land");

    fs::set_permissions(d.join("locked"), fs::Permissions::from_mode(0o000)).expect("chmod 000");
    assert!(
        fs::read_dir(d.join("locked")).is_err(),
        "REACH: the sealed directory is still readable -- running as root? \
         every EACCES assertion in this file would be vacuous"
    );
    s
}

fn unseal(d: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(d.join("locked"), fs::Permissions::from_mode(0o755));
}

// ---------------------------------------------------------------------
// G1. The control. The answerable cases keep their answers. If this goes
// red the arc broke the ordinary path and nothing else here matters.
// ---------------------------------------------------------------------
#[test]
fn g1_the_answerable_questions_keep_their_answers() {
    let s = world("g1");
    let d = s.path();
    let present = eval(d, r#"(~%Io./exists "open/f.txt")"#);
    let absent = eval(d, r#"(~%Io./exists "open/nosuch.txt")"#);
    let content = eval(d, r#"(~%Io./read_file "open/f.txt")"#);
    let no_content = eval(d, r#"(~%Io./read_file "open/nosuch.txt")"#);
    unseal(d);

    assert!(
        present.contains("#true"),
        "a readable file exists: {present}"
    );
    assert!(
        absent.contains("#false"),
        "an absent file does not: {absent}"
    );
    assert!(
        content.contains("hi"),
        "reading gives the content: {content}"
    );
    assert!(
        no_content.contains("#none"),
        "reading nothing gives #none: {no_content}"
    );
}

// ---------------------------------------------------------------------
// G2. RED LINE. The store boundary is the model this arc copies. It must
// answer exactly as it does today.
// ---------------------------------------------------------------------
#[test]
fn g2_the_store_boundary_still_answers_the_way_it_already_did() {
    let s = world("g2");
    let d = s.path();
    let ex = eval(d, r#"(~%Io./exists ".oo/HEAD")"#);
    let rd = eval(d, r#"(~%Io./read_file ".oo/HEAD")"#);
    unseal(d);
    for (what, out) in [("exists", &ex), ("read_file", &rd)] {
        assert!(
            out.contains("_|_") && out.contains("#store_boundary"),
            "{what} on the store boundary must stay `_|_ ;; %cause: \
             #store_boundary`: {out}"
        );
    }
}

// ---------------------------------------------------------------------
// R1. THE ONE. `exists` cannot tell "not there" from "will not say".
// ---------------------------------------------------------------------
#[test]
fn r1_exists_separates_absent_from_unreadable() {
    let s = world("r1");
    let d = s.path();
    let absent = eval(d, r#"(~%Io./exists "open/nosuch.txt")"#);
    let opaque = eval(d, r#"(~%Io./exists "locked/f.txt")"#);
    unseal(d);

    assert!(
        absent.contains("#false"),
        "CONTROL: a genuinely absent file must still answer #false: {absent}"
    );
    assert_ne!(
        strip_effect(&absent),
        strip_effect(&opaque),
        "a file the engine may not read and a file that is not there give \
         the same answer, so no program can tell them apart:\n  absent: \
         {absent}\n  opaque: {opaque}"
    );
    assert!(
        opaque.contains("%cause"),
        "the unreadable case must carry a registered cause (D66): {opaque}"
    );
}

// ---------------------------------------------------------------------
// G3. RED LINE. Two things that are not failures must keep the answer
// they have. A path through a regular file, and a symlink that resolves
// to nothing, are both simply absent -- `#false` is true for each, and
// inventing a difference here would be the same mistake in the other
// direction.
// ---------------------------------------------------------------------
#[test]
fn g3_a_path_that_cannot_hold_anything_is_simply_absent() {
    let s = world("g3");
    let d = s.path();
    std::os::unix::fs::symlink("nowhere-at-all", d.join("open/dangling"))
        .expect("dangling symlink");
    let absent = eval(d, r#"(~%Io./exists "open/nosuch.txt")"#);
    let notdir = eval(d, r#"(~%Io./exists "open/f.txt/x")"#);
    let dangling = eval(d, r#"(~%Io./exists "open/dangling")"#);
    unseal(d);

    assert!(absent.contains("#false"), "CONTROL: {absent}");
    assert_eq!(
        strip_effect(&absent),
        strip_effect(&notdir),
        "a path through a regular file can never hold anything, for anyone, \
         so `nothing is there` is the true and complete answer -- the same \
         one an absent file gets:\n  absent: {absent}\n  notdir: {notdir}"
    );
    assert_eq!(
        strip_effect(&absent),
        strip_effect(&dangling),
        "the host resolves a dangling symlink and finds nothing \
         (try_exists is Ok(false), not Err), so it is absent, not opaque:\n  \
         absent: {absent}\n  dangling: {dangling}"
    );
}

// ---------------------------------------------------------------------
// R2. A pure reference cycle is not an absent file. Something IS at that
// path -- symlink_metadata succeeds -- so `#false` is false.
// ---------------------------------------------------------------------
#[test]
fn r2_a_symlink_loop_is_not_an_absent_file() {
    let s = world("r2");
    let d = s.path();
    let absent = eval(d, r#"(~%Io./exists "open/nosuch.txt")"#);
    let loopy = eval(d, r#"(~%Io./exists "open/loop")"#);
    unseal(d);

    assert!(absent.contains("#false"), "CONTROL: {absent}");
    assert_ne!(
        strip_effect(&absent),
        strip_effect(&loopy),
        "a symlink loop answers exactly what an absent file answers, and \
         yet something is at that path -- symlink_metadata succeeds on it, \
         so `nothing is there` is not true:\n  absent: {absent}\n  loop: {loopy}"
    );
    assert!(
        loopy.contains("%cause"),
        "a pure reference cycle must carry a registered cause (D67): {loopy}"
    );
}

// ---------------------------------------------------------------------
// R3. The same collapse in `read_file`, which is the other observation.
// ---------------------------------------------------------------------
#[test]
fn r3_read_file_separates_absent_from_unreadable() {
    let s = world("r3");
    let d = s.path();
    let absent = eval(d, r#"(~%Io./read_file "open/nosuch.txt")"#);
    let opaque = eval(d, r#"(~%Io./read_file "locked/f.txt")"#);
    unseal(d);

    assert!(
        absent.contains("#none"),
        "CONTROL: reading an absent file must still answer #none: {absent}"
    );
    assert_ne!(
        strip_effect(&absent),
        strip_effect(&opaque),
        "reading a file the engine may not read is indistinguishable from \
         reading one that is not there:\n  absent: {absent}\n  opaque: {opaque}"
    );
    assert!(
        opaque.contains("%cause"),
        "the unreadable case must carry a registered cause (D66): {opaque}"
    );
}

/// `;; %effect: #io` is on every answer here and carries no information
/// about which of them this is.
fn strip_effect(s: &str) -> String {
    s.split(";;")
        .filter(|p| !p.trim_start().starts_with("%effect"))
        .collect::<Vec<_>>()
        .join(";;")
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------
// R4 (repair round 1). Added by the acceptor after delivery 1, on the
// user's ruling (乙): the host-level loop gets its own tag.
//
// Delivery 1 marked a filesystem symlink loop `#static_cycle`. The shape
// is right -- a name pointing at a name, pure reference, no
// transformation -- and the acceptor's own order invited the reuse by
// calling it "a precedent of the same shape". Same shape is not the same
// tag, and three things follow from reusing it:
//
//   * the registry defines `#static_cycle` by reference to an n/
//     construct (SPEC_12 1.1), and marks it 非錯誤 / 消費即蒸發;
//   * a broken symlink is something an operator can go and fix, so it is
//     not 非錯誤;
//   * measured, the two are told apart today only by `members` being
//     empty. `.%cause` gives the same name to both, so once the value
//     travels, the receiver cannot tell whether to look at the `.n` or at
//     the filesystem -- and D65 is explicit that the cause must travel
//     with the value and stand on its own.
//
// The name: `#path_cycle`, following the registry's `<domain>_cycle`
// pattern (`#static_cycle`, `#refinement_cycle`), axis 原因 (an operator
// must act) and carrier Top -- the same cell as `#unreadable`, which this
// arc also creates. Renaming stays cheap: measured, the cause is not in
// the address.
//
// This probe does not require that spelling. It requires that the two
// cycles do not answer with the same cause.
// ---------------------------------------------------------------------

/// `oo run FILE --observe FIELD`, for the causes that need a universe.
fn observe(dir: &Path, program: &str, field: &str) -> String {
    fs::write(dir.join("obs.n"), program).expect("write obs.n");
    let o = cmd(dir)
        .args(["run", "obs.n", "--observe", field])
        .output()
        .expect("oo runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
    .trim()
    .to_string()
}

#[test]
fn r4_a_symlink_loop_and_an_n_cycle_do_not_share_a_cause() {
    let s = world("r4");
    let d = s.path();

    // Control 1: n/'s own pure reference cycle keeps its registered cause.
    // If this ever changes, the repair went the wrong way -- the arc must
    // not move n/'s tag, only stop borrowing it.
    let n_cycle = observe(d, "a: b\nb: a\nc: (a.%cause)\n", "c");
    assert!(
        n_cycle.contains("#static_cycle"),
        "CONTROL: an n/ pure reference cycle must still answer \
         #static_cycle: {n_cycle}"
    );

    // Control 2: the other fibre this arc mints is untouched.
    let opaque = eval(d, r#"((~%Io./exists "locked/f.txt").%cause)"#);
    let host_loop = eval(d, r#"((~%Io./exists "open/loop").%cause)"#);
    unseal(d);
    assert!(
        opaque.contains("#unreadable"),
        "CONTROL: EACCES must still answer #unreadable: {opaque}"
    );

    // Control 3: the measurement reaches the loop at all.
    assert!(
        !host_loop.trim().is_empty() && !host_loop.trim_end().ends_with('_'),
        "REACH: the symlink loop produced no cause at all: {host_loop}"
    );

    assert!(
        !host_loop.contains("#static_cycle"),
        "a filesystem symlink loop answers with n/'s own tag for a pure \
         reference cycle between definitions. Once this value travels, the \
         receiver reads `#static_cycle` and cannot tell whether to look at \
         the .n or at the filesystem -- and the registry marks that tag \
         非錯誤 / 消費即蒸發, while a broken symlink is something an \
         operator can go and fix. Said: {host_loop}"
    );
}
