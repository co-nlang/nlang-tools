// A contract that lives in Rust.
// Rulings: nlang-spec/meta/oo/STATUS.md O76 (declare your keys),
//          O77 (bottom must not fold into false), O78 (unify; declaration
//          lives in the registry, not in the value).
// Recon:   nlang-tools/docs/a_contract_that_lives_in_rust_recon.md
// Order:   nlang-tools/docs/a_contract_that_lives_in_rust_handover.md
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// Builtins bypass the language's own application machinery, so they redo --
// badly -- two things it already gets right.
//
//   Entry.  A builtin's accepted key set exists only as `get_field("0")`
//           calls inside a Rust closure. Nothing declares it, so a call with
//           the roles swapped passes the shape check and gets applied.
//           Measured v0.36.0: `~%List./sort_by([1,2,3], (x -> #true))` is
//           `[]` -- every element dropped, no error.
//
//   Exit.   Six lines in list.rs decide truth by PRINTED FORM:
//           `result.to_string_plain().trim_start_matches('#') == "true"`.
//           So a predicate that collapsed (`_|_`), one that said no
//           (`#false`), and one that returned `5` are the same cell; and the
//           STRING "true" counts as the atom `#true`.
//
// O78 ③ settled the house convention with a measurement rather than taste:
// `|>` puts the piped value in the NEXT FREE SLOT (lib.rs:3049), so the pipe
// composes only with data-last. Today `~%Query./where` -- the core morphism
// of the query module -- is `_|_` under a pipe.
//
// ── Out of scope, do not touch ───────────────────────────────────────────
//
//   * Any address. Root CAID and standard root digest are red lines (G5).
//   * The declaration must NOT enter a builtin's VALUE (O78 ②): that moves
//     the standard root digest and with it every root's identity (O58).
//     It lives in the registry.
//   * `unified_arg` composition (lib.rs:3027-3074) -- ruled and fixed
//     2026-07-25, and G4 keeps it fixed.
//   * `~%Cond`'s branch-table semantics. It only has to declare (G6).
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason
// stated in each. The delivery may remove `#[ignore]` and NOTHING else in
// this file. If a pin here is wrong, say so in the report -- do not edit it.
//
// Every red asserts REACH before it asserts a value.
//
// NOT PROBED, stated so no one mistakes silence for coverage:
//   * `list.fold`. It is a fifth shape (two keys, sniffs which is the list,
//     the other must be a record `{ %val: seed, %f: morphism }`), and at the
//     baseline NO spelling the acceptor tried reaches it -- both orders of
//     the record form give `_|_ #conflict`. A probe cannot be calibrated
//     against a target the acceptor cannot reach. The order asks for fold's
//     current and chosen protocol in writing instead (§7.4).
//
// Baseline measured 2026-08-26 on dev 1e3c752 / oo v0.36.0: 7 green, 8 red.
// Every red fails at ITS OWN assertion, not at a REACH guard: r4's canonical
// call answers 3 first, and r5's honest `#false` predicate counts 0 first.

use std::path::Path;
use std::process::Command;

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

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("contract-{tag}"))
}

/// Observe `expr` as coordinate `out` of a one-line program.
fn obs(tag: &str, expr: &str) -> String {
    let d = scratch(tag);
    std::fs::write(d.join("a.n"), format!("out: {expr}\n")).unwrap();
    oo(&d, &["run", "a.n", "-o", "out"]).trim().to_string()
}

/// Collapse whitespace so multi-line printed values compare as one line.
fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

const CANON_PRED_FIRST: &str = "~%List./count ((x -> #true), [1,2,3])";

// ─────────────────────────────────────────────────────────────────────────
// RED -- what the arc must make true.
// ─────────────────────────────────────────────────────────────────────────

/// O78 ③, the sharpest cell. `~%Query` is the query module; a query module
/// exists to be piped into. Baseline: `_|_ #conflict`, because `where` is
/// list-first and the pipe fills the LAST slot.
#[test]
fn r1_where_composes_with_the_pipe() {
    let got = flat(&obs("r1", "[{a:1}] |> ~%Query./where (r -> #true)"));
    assert!(
        !got.is_empty(),
        "REACH: the pipeline must produce an observation at all"
    );
    assert_eq!(
        got, "[{ a: 1 }]",
        "`data |> /where (pred)` must select. The pipe fills the next free \
         slot (lib.rs:3049), so a piped morphism must take its data last. \
         got {got:?}"
    );
}

/// Same rule, the silent half. Baseline `[]`: the pipe put the list where
/// the predicate was expected, and nothing said so.
#[test]
fn r2_take_while_composes_with_the_pipe() {
    let got = flat(&obs("r2", "[1,2,3] |> ~%List./take_while (x -> #true)"));
    assert_eq!(
        got, "[1, 2, 3]",
        "an always-true predicate takes the whole list. got {got:?}"
    );
}

/// The control that makes r2 mean something: an always-FALSE predicate must
/// drop nothing under `drop_while`. Baseline is `[]` for BOTH predicates,
/// which is exactly the indistinguishability this arc is about.
#[test]
fn r3_drop_while_composes_with_the_pipe() {
    let got = flat(&obs("r3", "[1,2,3] |> ~%List./drop_while (x -> #false)"));
    assert_eq!(
        got, "[1, 2, 3]",
        "an always-false predicate drops nothing. got {got:?}"
    );
}

/// O76 entry side. The undeclared shape must not be silently applied.
/// This pins a PROPERTY, not a spelling: whatever the mismatch becomes
/// (partial cocoon, `_|_`, anything), it must not be an answer that a
/// reader would accept as the count of a 3-element list.
#[test]
fn r4_a_swapped_call_does_not_silently_answer() {
    let canon = flat(&obs("r4a", CANON_PRED_FIRST));
    assert_eq!(
        canon, "3",
        "REACH: the declared shape must still answer 3 before we can say \
         anything about the undeclared one. got {canon:?}"
    );

    let got = flat(&obs("r4b", "~%List./count ([1,2,3], (x -> #true))"));
    assert!(
        got != "0" && got != "3",
        "a call whose keys do not match the declaration must not produce a \
         plausible number. Baseline answers `0` -- the morphism was applied \
         to the list and every application collapsed. got {got:?}"
    );
}

/// O77. A predicate that COLLAPSED and a predicate that said NO must not be
/// the same cell. Baseline: both `0`.
#[test]
fn r5_a_collapsed_predicate_is_not_a_false_one() {
    let bottom = flat(&obs("r5a", "~%List./count ((x -> (1 2)), [1,2,3])"));
    let no = flat(&obs("r5b", "~%List./count ((x -> #false), [1,2,3])"));
    assert_eq!(
        no, "0",
        "REACH: an honest `#false` predicate must still count 0. got {no:?}"
    );
    assert!(
        bottom != no,
        "`(x -> (1 2))` collapses on every element; `(x -> #false)` answers \
         no. SPEC_08 §4.3 (O77) requires the collapse to be distinguishable. \
         both are {bottom:?}"
    );
}

/// O77's other half, same line of Rust: truth is decided by printed form,
/// so the STRING "true" is counted as the atom `#true`. Baseline: 3.
#[test]
fn r6_a_string_is_not_the_atom_true() {
    let got = flat(&obs("r6", r#"~%List./count ((x -> "true"), [1,2,3])"#));
    assert_ne!(
        got, "3",
        "`\"true\"` is a string. Counting it as `#true` means truth is being \
         decided by `to_string_plain()`. got {got:?}"
    );
}

/// O78 ①. Sniffing is the third convention and it has to go: a morphism
/// that accepts both orders has not declared anything.
#[test]
fn r7_filter_does_not_accept_both_orders() {
    let a = flat(&obs("r7a", "~%List./filter ((x -> #true), [1,2,3])"));
    let b = flat(&obs("r7b", "~%List./filter ([1,2,3], (x -> #true))"));
    assert!(
        !(a == "[1, 2, 3]" && b == "[1, 2, 3]"),
        "`/filter` answers correctly for BOTH argument orders (list.rs:337 \
         `if oo.is_list(&f0, ctx)`). Exactly one of them is the declared \
         shape. got {a:?} and {b:?}"
    );
}

/// Same for `/map` -- measured 2026-08-26, it sniffs too. The Inbox row
/// named `/filter` only; this is the second one.
#[test]
fn r8_map_does_not_accept_both_orders() {
    let a = flat(&obs("r8a", "~%List./map ((x -> 9), [1,2,3])"));
    let b = flat(&obs("r8b", "~%List./map ([1,2,3], (x -> 9))"));
    assert!(
        !(a == "[9, 9, 9]" && b == "[9, 9, 9]"),
        "`/map` answers correctly for BOTH argument orders. got {a:?} and {b:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// GREEN -- must stay true. These are the red lines, not decoration.
// ─────────────────────────────────────────────────────────────────────────

/// The declared shape works today and must keep working. If this ever goes
/// red the arc broke the thing it was normalising towards.
#[test]
fn g1_the_declared_shape_still_answers() {
    assert_eq!(flat(&obs("g1", CANON_PRED_FIRST)), "3");
}

/// conformance L2-104's spelling. O76 allows named keys; the seven named
/// builtins are to DECLARE, not to be rewritten positionally. If this goes
/// red, the arc chose (A)-style normalisation the ruling did not authorise.
#[test]
fn g2_named_keys_still_reach_their_builtin() {
    let got = flat(&obs(
        "g2",
        "~%Engine./project_down { target: { x: 1, y: 2 }, masa: \
         \"hash:sha256:v2:_:sketch:0000000000000000000000000000000000000000000000000000000000000000\" }",
    ));
    assert!(
        got.contains("%projection: #down") && got.contains("x: 1") && got.contains("y: 2"),
        "L2-104 is a red line. got {got:?}"
    );
}

/// O76 did not touch curry or the positional combo. All three spellings of
/// the same two-argument call must still agree.
#[test]
fn g3_the_two_passing_conventions_still_agree() {
    for expr in [
        "~%Math./add 7 17",
        "~%Math./add (7, 17)",
        "~%Math./add { 0: 7, 1: 17 }",
    ] {
        assert_eq!(flat(&obs("g3", expr)), "24", "spelling {expr:?} changed");
    }
}

/// The 2026-07-25 ruling: named fields of the argument are lifted so
/// SPEC_08 §3.5 spellings reach their builtin. `add` binds neither `x` nor
/// `y`, so it must partially apply rather than answer or collapse.
#[test]
fn g4_an_unbound_named_call_partially_applies() {
    let got = flat(&obs("g4", "~%Math./add { x: 7, y: 17 }"));
    assert!(
        got.contains("%builtin: \"math.add\"") && !got.contains("_|_"),
        "an argument whose keys the builtin does not bind is a PARTIAL, not \
         an error and not an answer. got {got:?}"
    );
}

/// Identity. This arc declares in the registry precisely so these do not
/// move (O78 ②).
#[test]
fn g5_identity_does_not_move() {
    let d = scratch("g5");
    std::fs::write(d.join("a.n"), "app: { k1: 1 }\n").unwrap();
    oo(&d, &["evolve", "a.n"]);
    let commit = oo(&d, &["commit", "-m", "x"]);
    assert!(commit.contains("Commit successful"), "REACH: {commit}");
    let status = oo(&d, &["status"]);
    assert!(
        status.contains("7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911"),
        "standard root digest is a red line. {status}"
    );
}

/// `~%Cond`'s tuple branch table. The arc makes it declare; it must not
/// change what it accepts.
#[test]
fn g6_cond_still_takes_its_tuple_table() {
    assert_eq!(
        flat(&obs("g6", "~%Cond./cond ([ (#true, (@any -> \"A\")) ])")),
        "\"A\""
    );
}

/// conformance L2-116, rewritten data-last. Its subject is that a discarded
/// boolean keeps its obstruction -- measured 2026-08-26, that holds in BOTH
/// argument orders, so rewriting the vector is safe. This pins the rewritten
/// form as already true, so the arc cannot quietly lose it.
#[test]
fn g7_a_discarded_boolean_keeps_its_obstruction_data_last() {
    let got = flat(&obs(
        "g7",
        "{ %effect: #pure, v: ~%List./filter ((x -> ((~%Time.now _) = (~%Time.now _))), [1,2,3]) }",
    ));
    assert!(
        got.contains("_|_") && got.contains("#effect_violation"),
        "L2-116's subject must survive the rewrite. got {got:?}"
    );
}
