// A field outside identity is changing the answer to `&`.
// Order: nlang-tools/docs/a_field_outside_identity_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-037 (was an `interrupt-candidate`
//        Inbox row; the discriminating experiment it named has been run).
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// `{ ...{ a: 1 } } & { ...{ b: 2 } }` answers `{ a: 1 }`. The right operand
// is gone: no bottom, no `%cause`, no warning. One side spread is fine; two
// sides spread loses one. It does not commute — swapping the operands keeps
// the other half instead. Measured on four real binaries: present at least
// since v0.20.0.
//
// The two facts that make this more than an operator bug:
//
//   1. The two values ARE the same value. `identify({ /f: 1 })` and
//      `identify({ ...{ /f: 1 } })` are byte-identical, `=` is `#true`,
//      `keys` agree, and they print the same. So one value has two answers
//      to the same operator.
//
//   2. `ComboVal::pending_spreads` is `#[serde(skip)]` and appears zero times
//      in `bn_serial.rs` and zero times in `encode_chs` -- it does not enter
//      the CAID. Spread expansion is deferred, so a spread result carries an
//      unexpanded entry that identity cannot see. Measured at the meet: both
//      operands arrive `data=[] pending=1`, and the result has `pending=1`,
//      not 2. One deferred expansion is dropped.
//
// `unify.rs:436-437` already collects `a.pending + b.pending` and `:587`
// writes it back, so the loss happens before those lines -- inside one of the
// two `expand_combo_pending` calls at the top of `unify_combo`. That function
// is `pub(crate)`, so locating it needs a crate-internal probe; this file
// pins the observable property only.
//
// There is a correct reference implementation of the same operation in this
// same engine: evolving `x: { ...{ a: 1 } }` then `x: { ...{ b: 2 } }` gives
// `{ a: 1, b: 2 }`. Coordinate merge is right; the `&` operator is wrong.
//
// ── Scope ────────────────────────────────────────────────────────────────
//
// The arc must make two values with the same CAID give the same answer.
// Whether that is done by fixing the meet, by expanding earlier, or by
// removing the field is not pinned here -- R3 pins the property, not a
// spelling. But a fix that leaves an identity-invisible field able to change
// an answer has not finished: see REAL_03 §6.7 (no field may sit outside the
// hash; precedent `span`, same family as `legacy_fields`).
//
// ── Out of scope, do not touch ───────────────────────────────────────────
//
//   * The identity of any value. G4 pins it: the fix must not move a CAID.
//   * Nested spread's semantics (SPEC_03 §3.1). G2/G3 keep them fixed.
//   * Top-level spread (Q-036, shipped v0.32.0).
//   * `_:`, mount, overlay.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason
// stated in each. The delivery may remove `#[ignore]` and NOTHING else in
// this file. If a pin here is wrong, say so in the report -- do not edit it.
//
// Baseline measured 2026-08-24 on dev f808d8c / oo 0.32.0: 3 green, 5 red.

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
    nlang_interpreter::ScratchDir::new(&format!("outside-identity-{tag}"))
}

/// `oo eval <expr>`, trimmed.
fn ev(tag: &str, expr: &str) -> String {
    let d = scratch(tag);
    oo(&d, &["eval", expr]).trim().to_string()
}

// ─────────────────────────────────────────────────────────────────────────
// RED -- what the arc must make true.
// ─────────────────────────────────────────────────────────────────────────

/// The minimal reproduction. Both operands are spread results; the right one
/// disappears.
///
/// Baseline: `["a"]`.
#[test]
fn r1_a_meet_of_two_spreads_keeps_both_sides() {
    let got = ev("r1", "~%Reflection./keys ({ ...{ a: 1 } } & { ...{ b: 2 } })");
    assert!(
        got.contains('a'),
        "REACH: the left operand must survive at all; got {got:?}"
    );
    assert_eq!(
        got, r#"["a", "b"]"#,
        "a meet keeps both sides. The control `{{ a: 1 }} & {{ b: 2 }}` already \
         answers this; spreading the same fields must not change it. got {got:?}"
    );
}

/// Meet is commutative. Today swapping the operands keeps the other half,
/// which is how a reader can tell nothing is merging at all.
///
/// Baseline: `["a"]` vs `["b"]`.
#[test]
fn r2_a_meet_of_two_spreads_commutes() {
    let ab = ev("r2a", "~%Reflection./keys ({ ...{ a: 1 } } & { ...{ b: 2 } })");
    let ba = ev("r2b", "~%Reflection./keys ({ ...{ b: 2 } } & { ...{ a: 1 } })");
    assert!(
        ab.contains('a') || ab.contains('b'),
        "REACH: at least one operand must survive; got {ab:?}"
    );
    assert_eq!(
        ab, ba,
        "`&` is a meet and a meet commutes (SPEC_03 §3.1 collision merge). \
         got {ab:?} vs {ba:?}"
    );
}

/// The property, not a spelling: two values the engine says are identical --
/// same CAID, `=` is `#true` -- must answer the same operator the same way.
///
/// This is the pin that a partial fix cannot satisfy by accident. G4 holds
/// the other end: the CAIDs must stay identical.
///
/// Baseline: `#false`.
#[test]
fn r3_values_with_one_identity_answer_one_way() {
    let spread = ev(
        "r3a",
        "~%Reflection./keys ({ ...{ a: 1 } } & { ...{ b: 2 } })",
    );
    let direct = ev("r3b", "~%Reflection./keys ({ a: 1 } & { b: 2 })");
    assert!(
        !direct.is_empty() && direct.contains('a'),
        "REACH: the control must produce keys; got {direct:?}"
    );
    assert_eq!(
        spread, direct,
        "`{{ ...{{ a: 1 }} }}` and `{{ a: 1 }}` have the same CAID and `=` is \
         #true, so no operator may tell them apart. got {spread:?} vs {direct:?}"
    );
}

/// The shape a user actually meets: two standard-library modules. Cond has 3
/// keys, Math has 49, and spreading both into one combo gives 52 -- so the
/// meet must give 52 too.
///
/// Baseline: 3 (only Cond survives).
#[test]
fn r4_meeting_two_modules_keeps_every_name() {
    let got = ev(
        "r4",
        "~%List./len (~%Reflection./keys ({ ...~%Cond } & { ...~%Math }))",
    );
    assert_ne!(got, "_", "REACH: the expression must evaluate; got {got:?}");
    assert_eq!(
        got, "52",
        "~%Cond has 3 names and ~%Math has 49, and `{{ ...~%Cond, ...~%Math }}` \
         already gives 52. The meet must agree. got {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// GREEN -- what the arc must NOT break.
// ─────────────────────────────────────────────────────────────────────────

/// The heaviest face of the same defect, found while calibrating this file:
/// what is dropped is not only a key, it is a DISAGREEMENT.
///
/// `{ ...{ a: 1 } } & { ...{ a: 2 } }` answers `1`. Both controls --
/// `{ a: 1 } & { a: 2 }` and `{ a: 1, ...{ a: 2 } }` -- answer
/// `_|_ #conflict`. So the operator turns a collapse that should have
/// happened into a plausible-looking value, and nothing records that the two
/// sides ever disagreed.
///
/// This one is listed after the others because it is the reason the arc
/// cannot be deferred: losing a key is data loss, losing a conflict is a
/// wrong answer that reads as a right one.
///
/// Baseline: `1`.
#[test]
fn r5_a_dropped_operand_does_not_swallow_a_conflict() {
    let got = ev("r5", "({ ...{ a: 1 } } & { ...{ a: 2 } }).a");
    assert_ne!(got, "_", "REACH: the field must be reachable; got {got:?}");
    assert!(
        got.contains("_|_") && got.contains("conflict"),
        "1 and 2 disagree, so the meet of that key is bottom -- both controls \
         already say so. An operator that answers `1` here has reported \
         agreement that did not exist. got {got:?}"
    );
}

/// One side spread already works today and must keep working -- both
/// directions.
#[test]
fn g2_one_sided_spread_is_unaffected() {
    let l = ev("g2a", "~%Reflection./keys ({ ...{ a: 1 } } & { b: 2 })");
    let r = ev("g2b", "~%Reflection./keys ({ a: 1 } & { ...{ b: 2 } })");
    assert_eq!(l, r#"["a", "b"]"#, "spread on the left only; got {l:?}");
    assert_eq!(r, r#"["a", "b"]"#, "spread on the right only; got {r:?}");
}

/// L2-36/37: spread does not carry `~` private fields, and meeting two
/// spreads must not become a way around that.
#[test]
fn g3_privacy_survives_the_meet() {
    let got = ev(
        "g3",
        "~%Reflection./keys ({ ...{ ~s: 1, a: 2 } } & { ...{ ~t: 3, b: 4 } })",
    );
    assert!(
        !got.contains('s') && !got.contains('t'),
        "`~s` and `~t` are private to their sources and must not appear; got {got:?}"
    );
}

/// Identity red line, as a test rather than a report line: the fix must not
/// move a CAID. If these two stop agreeing, the arc solved the symptom by
/// making the values different, which is the one repair that is not allowed.
#[test]
fn g4_the_two_values_keep_one_identity() {
    let a = ev("g4a", "~%Discovery./identify ({ /f: 1 })");
    let b = ev("g4b", "~%Discovery./identify ({ ...{ /f: 1 } })");
    assert!(a.contains("sha256"), "REACH: identify must answer; got {a:?}");
    assert_eq!(
        a, b,
        "spreading a combo into a fresh combo yields the same value, so the \
         same address. A fix must not separate them. got {a:?} vs {b:?}"
    );
}
