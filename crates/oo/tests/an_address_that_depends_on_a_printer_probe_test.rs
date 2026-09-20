// An address that depends on a printer.
// Recon: nlang-tools/docs/an_address_that_depends_on_a_printer_recon.md
// Order: nlang-tools/docs/an_address_that_depends_on_a_printer_handover.md
//
// -- READ THIS FIRST: this baseline is ALL GREEN, deliberately --------------
//
// `Thunk`'s identity hashes `expr.to_nlang(0)` -- the output of a pretty
// printer. SYNTAX_02 4.12 requires only that printed output re-read to the
// same value; no clause anywhere fixes its bytes. So an implementation that
// prints `1+1` where this one prints `1 + 1` is conforming and computes a
// different address for the same deferred computation.
//
// That cannot be exhibited by one binary -- our printer is self-consistent
// by construction. Measured, it is also faithful about associativity:
// `1 + (1 + 1)` keeps its parentheses and does not collide with
// `1 + 1 + 1`. So there is no collision to repair here. The repair REMOVES A
// DEPENDENCY: identity stops going through the printer and goes through the
// Expr node table that v0.54.0 already specified.
//
// The four tests below are therefore RED LINES, not a baseline to flip, and
// four greens do NOT show that anything was fixed. The order says the same
// in its section 5, with the compensating requirements that replace a red.
//
// NOT PROBED, so silence is not mistaken for coverage:
//   * The `%id` path (`hash_recursive_with_salt`) -- Q-050 found a second
//     Debug exit there and the order asks the delivery about it.
//   * `closure` frames and `context`: they go through serialize_combo /
//     serialize_value and look unaffected, not confirmed item by item.
//   * That `=` forces a Thunk while its address is intensional, so two
//     things `=` calls equal can have different addresses. Measured, but it
//     violates no written clause -- same family as alpha-equivalence, its
//     own Inbox row.
//   * That `v: 1 + 1` is kept as a Thunk while `v: ~%Math./add (1,1)` is
//     collapsed to its result. Measured, own Inbox row, adjacent to D46/O79.

use std::path::Path;
use std::process::Command;

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("printaddr-{tag}"))
}

fn run(dir: &Path, args: &[&str]) -> (String, i32) {
    let o = Command::new(env!("CARGO_BIN_EXE_oo"))
        .args(args)
        .current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"))
        .output()
        .expect("oo runs");
    (
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        o.status.code().unwrap_or(-1),
    )
}

/// Commit `source` into a fresh store and return the root digest.
fn root(tag: &str, source: &str) -> String {
    let s = scratch(tag);
    let d = s.path();
    std::fs::write(d.join("a.n"), source).expect("write a.n");
    let (out, rc) = run(d, &["evolve", "a.n"]);
    assert_eq!(rc, 0, "REACH: evolve {source:?}: {out}");
    let (out, rc) = run(d, &["commit", "-m", "r"]);
    assert_eq!(rc, 0, "REACH: commit {source:?}: {out}");
    let (log, _) = run(d, &["log"]);
    let commit = log
        .split_whitespace()
        .find(|t| t.starts_with("hash:sha256:"))
        .expect("REACH: a commit in the log")
        .to_string();
    let (info, _) = run(d, &["inspect", &commit]);
    let r = info
        .lines()
        .find_map(|l| l.trim().strip_prefix("root:"))
        .expect("REACH: a root on the commit")
        .trim()
        .to_string();
    assert!(
        r.contains("hash:sha256:"),
        "REACH: root looks like a CAID, got {r:?}"
    );
    r
}

// ---------------------------------------------------------------------
// G1. The control. Two different deferred computations get two addresses.
// ---------------------------------------------------------------------
#[test]
fn g1_two_different_expressions_have_two_addresses() {
    assert_ne!(
        root("g1a", "v: 1 + 1\n"),
        root("g1b", "v: 1 + 1 + 1\n"),
        "CONTROL: two different expressions must differ"
    );
}

// ---------------------------------------------------------------------
// G2. RED LINE. How you wrote it must not reach the address. Whitespace
// and redundant parentheses are not nodes, so all three of these are one
// deferred computation and must stay one address -- under the printer
// today and under the node table tomorrow.
// ---------------------------------------------------------------------
#[test]
fn g2_source_spelling_does_not_reach_the_address() {
    let spaced = root("g2a", "v: 1 + 1\n");
    for (tag, src) in [("g2b", "v: 1+1\n"), ("g2c", "v: (1 + 1)\n")] {
        assert_eq!(
            root(tag, src),
            spaced,
            "{src:?} must land on the same address as `v: 1 + 1` -- \
             whitespace and redundant parentheses are not part of the tree"
        );
    }
}

// ---------------------------------------------------------------------
// G3. RED LINE, and the one a careless encoding breaks. Associativity IS
// part of the tree. `(1+1)+1` and `1+(1+1)` are two computations and must
// keep two addresses; an encoding that flattened a chain would make this
// red while leaving G1 and G2 green.
// ---------------------------------------------------------------------
#[test]
fn g3_associativity_is_part_of_the_tree() {
    let left = root("g3a", "v: (1 + 1) + 1\n");
    let right = root("g3b", "v: 1 + (1 + 1)\n");
    assert_ne!(
        left, right,
        "left- and right-associated chains are different deferred \
         computations and must not share an address"
    );
    // And the left-associated form is what a bare chain means.
    assert_eq!(
        root("g3c", "v: 1 + 1 + 1\n"),
        left,
        "a bare chain must mean the left-associated tree"
    );
}

// ---------------------------------------------------------------------
// G4. RED LINE. The epoch is bounded to universes that actually hold a
// Thunk. A universe whose fields are all values must not move, and the
// standard root must not move.
// ---------------------------------------------------------------------
#[test]
fn g4_a_universe_without_a_thunk_does_not_move() {
    let plain = root("g4a", "x: 0\n");
    assert!(
        plain.contains("31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a"),
        "the `x: 0` root moved, so the epoch is wider than Thunks: {plain}"
    );

    let s = scratch("g4b");
    let d = s.path();
    std::fs::write(d.join("a.n"), "x: 0\n").expect("write a.n");
    let (out, rc) = run(d, &["evolve", "a.n"]);
    assert_eq!(rc, 0, "REACH: evolve: {out}");
    let (out, rc) = run(d, &["commit", "-m", "r"]);
    assert_eq!(rc, 0, "REACH: commit: {out}");
    let (status, _) = run(d, &["status"]);
    assert!(
        status.contains("7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911"),
        "the standard root moved: {status}"
    );
}
