// What the shelf does not hold.
//
// Rulings: nlang-spec/meta/oo/STATUS.md O64 / O65 / O66 (2026-08-20),
// plus the closed-world ruling for the system axis (2026-08-20).
//
// ── Four changes, one identity move ──────────────────────────────────────
//
//   O64  the string module is `~%Str`, not `~%String`
//   O65  `/add` leaves the top-level rules axis
//   O66  the locally synthesised `~%Official` shell goes
//   ---  an absent `~%` name answers `⊥ #missing_key`, not `_`
//
// The first three change the standard root's contents, so its digest moves
// and every new root's address moves with it. WORK_QUEUE §9.0: identity
// moves once. They ride together, and they ride with the %differential
// rename already delivered.
//
// ── Why the fourth ───────────────────────────────────────────────────────
//
// SPEC_13 §135 says the honest answer for a global object this engine does
// not hold is `⊥ #missing_key`. Removing the shell alone yields `_`, which
// is the answer for "I know nothing about this name" -- and the system
// axis is the one axis where the engine has complete knowledge, because
// `~%` is engine-minted only (system_axis_probe_test, SPEC_09 ownership
// clause 2026-07-16). Saying "unknown" there is saying something untrue.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason
// stated in each. The delivery may remove `#[ignore]` and NOTHING else in
// this file. Assertions pin behaviour and causes, never message wording.

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
    nlang_interpreter::ScratchDir::new(&format!("shelf-{tag}"))
}

/// Observe `app` for one source.
fn observe(tag: &str, src: &str) -> String {
    let d = scratch(tag);
    std::fs::write(d.join("a.n"), src).unwrap();
    oo(&d, &["run", "a.n", "--observe", "app"])
}

// ── C1..C5 ── controls: green at baseline, MUST stay green ───────────────

/// Every root written by a released engine names one of these. This arc
/// moves the digest again, so all three must remain supported.
///
/// Repair 1 of the previous arc is why this control exists: the historical
/// row held the BUILDER's return value, while what shipped was that value
/// after `for_cas_storage`, and every existing store became unopenable.
/// A row is historical only if some released `root_with_system()` returned
/// exactly it.
#[test]
fn c1_control_every_previously_shipped_standard_root_stays_supported() {
    let d = scratch("c1");
    let engine = nlang_interpreter::Ouroboros::init(&d).unwrap();
    for (digest, era) in [
        (
            "65f52e2da48baa550d7340c0fdc214fd1f9925577a96ffec59bc34f8b2bcbe72",
            "pre-v0.26.0 (builder, before for_cas_storage existed)",
        ),
        (
            "2da5b71371649291cfa5dc5d0cd019464d248e98645b3901938e1c08d2172c2c",
            "v0.26.0 ..= v0.26.1",
        ),
        (
            "229be911057aaff665b13691115e5ee271d2007b649ebbfaef40cf70b6802c24",
            "the quoted-names arc, superseded by this one",
        ),
    ] {
        assert!(
            engine.supports_standard_root(digest),
            "stores from {era} name {digest}; dropping it makes all of them unopenable"
        );
    }
}

/// The arithmetic family stays where it lives. O65 removes the top-level
/// duplicate, never the module.
#[test]
fn c2_control_the_math_module_still_holds_add() {
    let out = observe("c2", "app: { v: ~%Math./add }\n");
    assert!(
        out.contains("math.add"),
        "`~%Math./add` must survive O65 untouched, got: {out}"
    );
}

/// Guards over-removal: only the three named modules change.
#[test]
fn c3_control_the_other_modules_are_untouched() {
    for m in ["List", "Cond", "Math", "Engine"] {
        let out = observe("c3", &format!("app: {{ v: ~%{m} }}\n"));
        assert!(
            !out.contains("v: _"),
            "`~%{m}` is not in this arc's scope and must still resolve, got: {out}"
        );
    }
}

/// The `~%` axis stays engine-minted: a user write is still rejected.
/// This arc changes what an ABSENT name answers, never who may write.
#[test]
fn c4_control_the_system_axis_still_refuses_a_user_write() {
    let d = scratch("c4");
    std::fs::write(d.join("a.n"), "~%Mine: 5\napp: { v: 1 }\n").unwrap();
    let out = oo(&d, &["evolve", "a.n"]);
    assert!(
        out.contains("Error"),
        "writing to `~%` must stay illegal (SPEC_09 ownership), got: {out}"
    );
}

/// O65 removes the standard root's `/add`. It must NOT disturb the
/// prefix-stripping fallback that lets `/add` reach a user's `add` --
/// conformance L2/06-currying is exactly this, and it never depended on
/// the standard root at all.
#[test]
fn c5_control_the_prefix_fallback_still_reaches_a_users_add() {
    let out = observe(
        "c5",
        "add: (x -> (y -> x + y))\napp: { v: (/add 3) 4 }\n",
    );
    assert!(
        out.contains("v: 7"),
        "`/add` must still fall back to the user's `add`, got: {out}"
    );
}

// ── R1..R5 ── reds ───────────────────────────────────────────────────────

/// RED (O64): the module for the type `str` is `~%Str`. The established
/// pattern is the TitleCase of the type name -- `list`→`~%List`,
/// `complex`→`~%Complex` -- and `String` is the TitleCase of a type that
/// does not exist.
#[test]
fn r1_the_string_module_is_spelled_str() {
    let out = observe("r1", "app: { v: ~%Str./len }\n");
    assert!(
        out.contains("str.len"),
        "`~%Str./len` must resolve after the rename, got: {out}"
    );
}

/// RED (O64 + closed world): the old spelling is gone, and gone on the
/// system axis means `⊥ #missing_key`, not silence.
#[test]
fn r2_the_old_spelling_answers_missing_key() {
    let out = observe("r2", "app: { c: (~%String).%cause }\n");
    assert!(
        out.contains("c: #missing_key"),
        "after the rename `~%String` must answer #missing_key, got: {out}"
    );
}

/// RED (O65): with no user definition, `/add` is not provided by the
/// standard root. The rules axis is open-world, so this is `_` -- the
/// closed-world ruling is about `~%` only.
#[test]
fn r3_the_top_level_add_is_no_longer_provided() {
    let out = observe("r3", "app: { v: /add }\n");
    assert!(
        out.contains("v: _"),
        "with no user `add`, `/add` must not be provided by the standard root, got: {out}"
    );
}

/// RED (O66): SPEC_13 §135 -- a global governance object this engine does
/// not hold must answer `⊥ #missing_key`. Today a synthesised empty shell
/// stands in its place, which the clause calls another thing borrowing
/// its name.
#[test]
fn r4_official_answers_missing_key() {
    let out = observe("r4", "app: { c: (~%Official).%cause }\n");
    assert!(
        out.contains("c: #missing_key"),
        "`~%Official` must answer #missing_key, not stand in for the real object, got: {out}"
    );
}

/// RED (closed world): the rule is general, not a special case for one
/// name. `~%` is engine-minted only, so the engine knows exactly what it
/// holds; answering `_` there claims an ignorance it does not have.
#[test]
fn r5_any_absent_system_name_answers_missing_key() {
    let out = observe("r5", "app: { c: (~%NoSuchModule).%cause }\n");
    assert!(
        out.contains("c: #missing_key"),
        "an absent `~%` name must answer #missing_key, got: {out}"
    );
    // Not a special case for Official: a second novel name behaves alike.
    let out2 = observe("r5b", "app: { c: (~%AlsoNotHere).%cause }\n");
    assert!(
        out2.contains("c: #missing_key"),
        "the closed-world rule must be general, got: {out2}"
    );
}
