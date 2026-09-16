// Two values, one address.
// Ruling: nlang-spec/meta/oo/STATUS.md D69 (identity must not depend on a
//         normalisation that never happened to the value).
// Order:  nlang-tools/docs/two_values_one_address_handover.md
//
// -- What this arc is ----------------------------------------------------
//
// A commit reported success and stored somebody else's value. In one
// store, on the ordinary path:
//
//     v: "a"      -> root 3f9c002b…
//     v: " a "    -> rc 0, "Commit successful", root 3f9c002b…
//     inspect     -> v: "a"
//
// The language says those are two values: `("a" = " a ")` is #false and
// their reflected %id differ. The store says they are one. The cause is
// one line:
//
//     bn_serial.rs:191
//     AtomKind::Str(s) | AtomKind::MultilineStr(s) =>
//         encode_string(s.trim(), buf)
//
// It trims before hashing while the value keeps its whitespace, so two
// distinct values produce one byte string, and `write_object` skips the
// second because the path already exists.
//
// It was there from BN/'s first implementation (2026-05-23) and that
// commit's message lists four things it did, none of them this one.
//
// -- Why not "normalise the value instead" -------------------------------
//
// The arm is shared with MultilineStr, so the trim was probably meant for
// a `"""…"""` block's layout. It does not achieve that either: a multiline
// and its trimmed single-line form are still #false under `=`, and they
// also share an address.
//
// And the normalisation a multiline actually needs is of a different kind.
// SYNTAX_02 116 says strings have no escape sequences at all, and 110/129
// say a multiline's only escape is the triple quote. Measured, the engine
// honours the first: `"a\nb"` prints back `"a\nb"` and `"a\"b"` is a parse
// error. But unescaping `\"""` happens when the source is read -- it
// decides WHICH VALUE the text denotes. Trimming happens when the value is
// hashed -- it decides WHAT ADDRESS a value has. Wanting the first is not
// an argument for the second.
//
// D69: nothing is normalised at hash time.
//
// NOT PROBED, stated so no one mistakes silence for coverage:
//   * Whether the engine actually unescapes `\"""` when parsing. The
//     acceptor failed to measure it twice and recorded both failures:
//     `~%Str./contains` was applied in a shape that returns #false for a
//     control that should be #true, and `~%Io./write_file` turns out not
//     to accept a multiline string at all. It is a must-answer on the
//     order, not an assertion here.
//   * That `MultilineStr` is a second-class citizen -- `~%Str./concat`
//     answers "ab" for two single-line strings and `_|_ #conflict` when
//     one is multiline. Different layer, its own Inbox row.
//   * REAL_03 6.2's value tag table missing five tags. Separate card.

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

fn run(dir: &Path, args: &[&str]) -> (String, i32) {
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
    nlang_interpreter::ScratchDir::new(&format!("onaddr-{tag}"))
}

/// The CAID `~%Discovery./identify_and_store` mints for `expr`.
fn stored(dir: &Path, expr: &str) -> String {
    fs::write(
        dir.join("s.n"),
        format!("id: ~%Discovery./identify_and_store {expr}\n"),
    )
    .expect("write s.n");
    let (out, _) = run(dir, &["run", "s.n", "--observe", "id"]);
    let caid = out
        .trim()
        .trim_start_matches('"')
        .split('"')
        .next()
        .unwrap_or("")
        .split(";;")
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    assert!(
        caid.starts_with("hash:sha256:"),
        "REACH: stored() got {caid:?} for {expr}"
    );
    caid
}

fn commit_root(dir: &Path, source: &str, msg: &str) -> String {
    fs::write(dir.join("a.n"), source).expect("write a.n");
    let (out, rc) = run(dir, &["evolve", "a.n"]);
    assert_eq!(rc, 0, "REACH: evolve {source:?}: {out}");
    let (out, rc) = run(dir, &["commit", "-m", msg]);
    assert_eq!(rc, 0, "REACH: commit {source:?}: {out}");
    let (log, _) = run(dir, &["log"]);
    let commit = log
        .lines()
        .find_map(|l| l.trim().strip_prefix("commit "))
        .expect("REACH: a commit in the log")
        .to_string();
    let (info, _) = run(dir, &["inspect", &commit]);
    info.lines()
        .find_map(|l| l.trim().strip_prefix("root:"))
        .expect("REACH: a root on the commit")
        .trim()
        .to_string()
}

const PADDED: &str = "v: \" a \"\n";
const PLAIN: &str = "v: \"a\"\n";
const MULTI: &str = "v: \"\"\"\nhello\n\"\"\"\n";

// ---------------------------------------------------------------------
// G1. The control. Distinct strings already get distinct addresses. If
// this goes red the fixture is broken and nothing else here means
// anything.
// ---------------------------------------------------------------------
#[test]
fn g1_distinct_strings_already_have_distinct_addresses() {
    let s = scratch("g1");
    let d = s.path();
    let a = stored(d, "\"a\"");
    let b = stored(d, "\"b\"");
    assert_ne!(a, b, "CONTROL: \"a\" and \"b\" must differ");
}

// ---------------------------------------------------------------------
// G2. RED LINE. Equality is not what this arc touches. Two spellings of
// one text stay unequal; a multiline stays equal to itself.
// ---------------------------------------------------------------------
#[test]
fn g2_equality_is_untouched() {
    let s = scratch("g2");
    let d = s.path();
    for (expr, want) in [
        ("(\"a\" = \" a \")", "#false"),
        ("(\"a\" = \"a\")", "#true"),
    ] {
        let (out, _) = run(d, &["eval", expr]);
        assert!(out.contains(want), "{expr} must stay {want}: {out}");
    }
}

// ---------------------------------------------------------------------
// G3. RED LINE. Not an epoch where it would hurt. The standard root's
// 258 strings carry no leading or trailing whitespace and no newlines, so
// removing the trim must not move it.
// ---------------------------------------------------------------------
#[test]
fn g3_the_standard_root_does_not_move() {
    let s = scratch("g3");
    let d = s.path();
    let root = commit_root(d, "x: 0\n", "r");
    assert!(
        root.contains("31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a"),
        "the `x: 0` root moved: {root}"
    );
    let (status, _) = run(d, &["status"]);
    assert!(
        status.contains("7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911"),
        "the standard root moved: {status}"
    );
}

// ---------------------------------------------------------------------
// R1. THE ONE THAT MATTERS. A commit must store what was committed.
// ---------------------------------------------------------------------
#[test]
fn r1_a_commit_stores_what_was_committed() {
    let s = scratch("r1");
    let d = s.path();
    let first = commit_root(d, PLAIN, "first");
    let second = commit_root(d, PADDED, "second");

    assert_ne!(
        first, second,
        "committing `v: \" a \"` after `v: \"a\"` produced the same root, so \
         the second commit reported success and stored the first value"
    );
    let (shown, _) = run(d, &["inspect", &second]);
    assert!(
        shown.contains("\" a \""),
        "the root HEAD points at does not contain what was committed:\n{shown}"
    );
}

// ---------------------------------------------------------------------
// R2. The same collapse, minted directly, both shapes.
// ---------------------------------------------------------------------
#[test]
fn r2_whitespace_and_line_shape_are_part_of_a_string() {
    let s = scratch("r2");
    let d = s.path();

    let plain = stored(d, "\"a\"");
    let padded = stored(d, "\" a \"");
    assert_ne!(
        plain, padded,
        "\"a\" and \" a \" share an address while `=` calls them different"
    );

    let single = stored(d, "\"hello\"");
    let multi = stored(d, "\"\"\"\nhello\n\"\"\"");
    assert_ne!(
        single, multi,
        "a multiline string and its trimmed single-line form share an \
         address while `=` calls them different"
    );
}

// ---------------------------------------------------------------------
// R3. And the store must hand back what its address promised.
//
// The first version of this test stored `" a "` into a fresh store and
// read it back, which passes for the wrong reason: nothing had taken the
// address yet, so the bytes on disk were the right ones. The collapse
// only bites when somebody got there first, so store the competitor
// first and keep the control in the same run.
// ---------------------------------------------------------------------
#[test]
fn r3_the_address_hands_back_its_own_bytes() {
    let s = scratch("r3");
    let d = s.path();
    fs::write(
        d.join("s.n"),
        "first: ~%Discovery./identify_and_store \"a\"\n\
         second: ~%Discovery./identify_and_store \" a \"\n",
    )
    .expect("write s.n");

    let (out, rc) = run(d, &["run", "s.n", "--observe", "first"]);
    assert_eq!(rc, 0, "REACH: the store call ran: {out}");
    let (out2, rc) = run(d, &["run", "s.n", "--observe", "second"]);
    assert_eq!(rc, 0, "REACH: the store call ran: {out2}");

    let caid = out2
        .trim()
        .trim_start_matches('"')
        .split('"')
        .next()
        .unwrap_or("")
        .split(";;")
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    assert!(caid.starts_with("hash:sha256:"), "REACH: {caid:?}");

    let (shown, _) = run(d, &["inspect", &caid]);
    assert!(
        shown.contains("\" a \""),
        "the address minted for \" a \" handed back the value that reached \
         it first. A read verified by address returned bytes that address \
         never promised:\n{shown}"
    );
}

const _: &str = MULTI;
