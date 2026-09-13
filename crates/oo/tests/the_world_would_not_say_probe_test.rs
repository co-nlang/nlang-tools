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
// D66 splits it by the question, not by the error:
//
//   the question is good and answerable   -> the value  (#true/#false/text)
//   the question is good, the world will
//     not say (EACCES, EIO)               -> #blur + a registered %cause
//   the question itself is broken
//     (ENOTDIR, ELOOP)                    -> _|_  + a registered %cause
//   you may not ask (store boundary)      -> _|_  + #store_boundary  (done)
//
// ENOENT stays as it is: nothing is wrong with the question and "it is not
// there" is a real answer.
//
// NOT PROBED, stated so no one mistakes silence for coverage:
//   * `~%Io./write_file` and `~%Io./append_file` answer #none on every
//     failure too, which is the same collapse. They are ACTIONS, not
//     observations, and #blur means an observation that could not complete
//     -- so what a failed write should answer is a separate question.
//     Measured and filed as O87. Do not change them here.

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
// R2. A broken question is not an absent file either.
// ---------------------------------------------------------------------
#[test]
fn r2_a_broken_question_is_not_an_absent_file() {
    let s = world("r2");
    let d = s.path();
    let absent = eval(d, r#"(~%Io./exists "open/nosuch.txt")"#);
    let notdir = eval(d, r#"(~%Io./exists "open/f.txt/x")"#);
    let loopy = eval(d, r#"(~%Io./exists "open/loop")"#);
    unseal(d);

    assert!(absent.contains("#false"), "CONTROL: {absent}");
    for (what, out) in [
        ("a path through a file", &notdir),
        ("a symlink loop", &loopy),
    ] {
        assert_ne!(
            strip_effect(&absent),
            strip_effect(out),
            "{what} answers exactly what an absent file answers, and nothing \
             could have been there at all:\n  absent: {absent}\n  target: {out}"
        );
        assert!(
            out.contains("%cause"),
            "{what} must carry a registered cause (D66): {out}"
        );
    }
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
