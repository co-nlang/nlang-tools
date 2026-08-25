// A declared `#pure` survives one layer of morphism application.
// Order: nlang-tools/docs/an_obstruction_the_gate_did_not_read_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-034.  Ruling: meta/oo/STATUS.md O74.
//
// ── What this arc is ─────────────────────────────────────────────────────
//
// `{ %effect: #pure, v: ~%Time.now _ }` collapses, and so do the nested and
// in-list forms. Put one morphism application between the declaration and the
// clock and the same program is accepted:
//
//     { %effect: #pure, v: ((x -> ~%Time.now _) 1) }     → accepted
//     { %effect: #pure, v: ~%List./map (xs, impure) }    → accepted
//
// The sharpest cell: the accepted value PRINTS its own effect. Today
// `~%List./map ([1,2,3], (x -> { t: ~%Time.now _, r: 1 }.r))` displays as
// `[1 ;; %effect: #io, 1 ;; %effect: #io, 1 ;; %effect: #io]` and answers
// `#io` to `.%effect` — and the guard, standing next to it, says fine.
//
// ── Why the obvious repair is the wrong one (measured) ───────────────────
//
// The queue carried "make the guard dynamic — the value already has the right
// effect, nobody reads it". Measured across the callback surface at v0.34.0,
// that parenthetical holds in ONE cell out of five:
//
//     ~%List./map        (impure)            .%effect → #io    value honest
//     ~%List./filter     (impure predicate)  .%effect → #pure  value LIES
//     ~%List./take_while (impure predicate)  .%effect → #pure  value LIES
//     ~%Cond./match      (impure arm)        .%effect → #pure  value LIES
//     ~%Query./where     (PURE predicate)    blocked           over-blocks
//
// The mechanism: an effect survives only when the morphism's return value is
// physically placed in the result. `map` puts it in the output list. A
// predicate's boolean is consumed and thrown away, and the effect goes with
// it. So a dynamic guard reading `.%effect` would still let four of those
// five through — there is nothing on the value to read.
//
// ── The ruling (O74, 2026-08-25, user) ───────────────────────────────────
//
//   1. A builtin that applies a caller's morphism must union that
//      application's effect into its result, WHETHER OR NOT the returned
//      value is kept.
//   2. `SPEC_08` §4.3's "static guard" drops from ontology to implementation
//      note. The criterion is TAG_REGISTRY's: a declared section contradicted
//      by a non-zero obstruction.
//   3. The reason is not pragmatics. discussion 023 §3 lists this very guard
//      as "declared a global section (#pure) but the obstruction is non-zero
//      → lying → ⊥", and defines `#pure` as "the value is determined by its
//      CAID alone; a global section exists". An obstruction is a fact about
//      the value. `/filter` looked at the clock three times to decide what to
//      keep; discarding the booleans does not un-observe them.
//   4. `~%Query./where`'s unconditional IO floor comes off once (1) is done —
//      it currently blocks even an honest pure predicate.
//
// ── Out of scope, do not touch ───────────────────────────────────────────
//
//   * Identity. Effects do not enter the BN/ hash and must not start to:
//     G4 pins that a pure and an impure computation of the same `[1, 1, 1]`
//     keep one address.
//   * The cocoon shield (disc 023 §4: a cocoon is a local frame that carries
//     its own section, so contagion stops at its boundary). G1 pins it.
//   * `/if`'s morphism dispatch. `/if (#true, (@any -> 1), (@any -> 2))`
//     answers `_` today; that is a separate, already-recorded gap and no
//     probe here touches it.
//   * The `~%List` callbacks that give wrong answers under an always-true
//     predicate (`/any`, `/count`, `/find`, `/partition`, `/group_by`,
//     `/sort_by`) — filed separately, unrelated to effects.
//
// ── Probe integrity ──────────────────────────────────────────────────────
//
// Reds are `#[ignore]`d and MUST be red at the baseline for the reason stated
// in each. The delivery may remove `#[ignore]` and NOTHING else in this file.
// If a pin here is wrong, say so in the report — do not edit it.
//
// Baseline measured 2026-08-25 on dev 71eb69a / oo v0.34.0: 5 green, 5 red.

use std::fs;
use std::process::Command;

/// An impure morphism whose return value IS the observation.
const IMPURE: &str = "(x -> ~%Time.now _)";
/// An impure morphism returning a boolean — the shape a predicate takes, and
/// the shape whose effect is thrown away with the boolean today.
const IMPURE_PRED: &str = "(x -> ((~%Time.now _) = (~%Time.now _)))";
/// Impure, but its returned value is a plain `1`, so the output list is
/// `[1, 1, 1]` either way. Used both to catch the gate and to pin identity.
const IMPURE_ONE: &str = "(x -> { t: ~%Time.now _, r: 1 }.r)";

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("obstruction-{tag}"))
}

/// Write `out: <expr>` to a file and observe it.
fn observe(tag: &str, expr: &str) -> String {
    let d = scratch(tag);
    let f = d.path().join("main.n");
    fs::write(&f, format!("out: {expr}\n")).expect("write");
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(d.path())
        .env("OO_IDENTITY", d.path().join("identity-for-tests"))
        .env("OO_NODE_HOME", d.path().join("node-home-for-tests"));
    let o = c
        .args(["run", f.to_str().unwrap(), "--observe", "out"])
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

fn is_effect_violation(s: &str) -> bool {
    s.contains("_|_") && s.contains("effect_violation")
}

fn declared_pure(v: &str) -> String {
    format!("{{ %effect: #pure, v: {v} }}")
}

// ─────────────────────────────────────────────────────────────────────────
// RED — what the arc must make true.
// ─────────────────────────────────────────────────────────────────────────

/// The headline. One layer of application between the declaration and the
/// clock, and a false `#pure` is accepted. The three controls without that
/// layer all collapse, so the declaration is not the thing that is broken.
///
/// Baseline: accepted.
#[test]
#[ignore = "red at baseline: one morphism application launders the declaration"]
fn r1_one_layer_of_application_does_not_launder_a_declaration() {
    let got = observe("r1", &declared_pure(&format!("(({IMPURE}) 1)")));
    assert!(
        !got.is_empty(),
        "REACH: the program must produce something; got {got:?}"
    );
    assert!(
        is_effect_violation(&got),
        "`{IMPURE}` observes the clock whether or not a morphism application \
         stands between it and the declaration. The direct, nested and \
         in-list forms all collapse; this one must too. got {got:?}"
    );
}

/// The gate standing next to a value that says what it is. Every element of
/// this list prints `;; %effect: #io` and the whole answers `#io` to
/// `.%effect` — and the declaration above it is accepted anyway.
///
/// Baseline: accepted.
#[test]
#[ignore = "red at baseline: the value prints #io and the gate accepts it"]
fn r2_the_gate_reads_the_value_it_was_handed() {
    let inner = format!("~%List./map ([1,2,3], {IMPURE_ONE})");
    let effect = observe("r2-eff", &format!("({inner}).%effect"));
    assert_eq!(
        effect, "#io",
        "REACH: this cell is the one where the value is already honest — if \
         it stops saying #io the probe is measuring something else; got {effect:?}"
    );

    let got = observe("r2", &declared_pure(&inner));
    assert!(
        is_effect_violation(&got),
        "the value answers #io to `.%effect` and prints its effect on every \
         element. A guard that accepts it is not reading what it was handed. \
         got {got:?}"
    );
}

/// O74 ①/③: an obstruction is not a data-flow property. `/filter` consulted
/// the clock to decide what to keep; throwing the booleans away does not
/// un-observe it. The value itself must say so.
///
/// Baseline: `#pure`.
#[test]
#[ignore = "red at baseline: a predicate's effect is discarded with its boolean"]
fn r3_a_discarded_return_value_does_not_discard_the_obstruction() {
    let got = observe(
        "r3",
        &format!("(~%List./filter ([1,2,3], {IMPURE_PRED})).%effect"),
    );
    assert_eq!(
        got, "#io",
        "the predicate observed the clock three times. The result is not \
         determined by its CAID alone, so `#pure` is a false claim about it. \
         got {got:?}"
    );
}

/// The same law in a second family, so the repair is a rule rather than a
/// patch on `filter`. `take_while` returns the original elements too.
///
/// Baseline: `#pure`.
#[test]
#[ignore = "red at baseline: same laundering in take_while"]
fn r4_the_rule_holds_for_every_callback_not_just_one() {
    let got = observe(
        "r4",
        &format!("(~%List./take_while ([1,2,3], {IMPURE_PRED})).%effect"),
    );
    assert_eq!(
        got, "#io",
        "`take_while` applied an impure predicate; the obstruction is the \
         same one `filter` has. got {got:?}"
    );
}

/// O74 ④, the other end. `/where` is the only callback that catches an impure
/// predicate today, and it does it by flooring its result to IO
/// unconditionally — so an honest pure predicate is refused as well. Once the
/// effect survives the callback on its own, the floor is not needed and the
/// honest program must become writable.
///
/// This is the cost the conservative repair would have made permanent, which
/// is why it is pinned rather than left as a note.
///
/// Baseline: `_|_ #effect_violation`.
#[test]
#[ignore = "red at baseline: /where's IO floor refuses even a pure predicate"]
fn r5_an_honest_pure_predicate_is_writable() {
    let got = observe(
        "r5",
        &declared_pure("~%Query./where ([{ a: 1 }], (r -> #true))"),
    );
    assert!(
        !is_effect_violation(&got),
        "`(r -> #true)` observes nothing, so this declaration is true and must \
         stand. got {got:?}"
    );
    assert!(
        got.contains("#pure"),
        "REACH: the combo must come back carrying its declaration; got {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// GREEN — what the arc must NOT break.
// ─────────────────────────────────────────────────────────────────────────

/// The three forms that already collapse must keep collapsing. If a repair
/// moves the criterion it must not move it off these.
#[test]
fn g1_the_direct_forms_still_collapse() {
    for (tag, v) in [
        ("direct", "~%Time.now _".to_string()),
        ("nested", "{ w: ~%Time.now _ }".to_string()),
        ("inlist", "[~%Time.now _]".to_string()),
    ] {
        let got = observe(&format!("g1-{tag}"), &declared_pure(&v));
        assert!(
            is_effect_violation(&got),
            "`{tag}` must stay a violation; got {got:?}"
        );
    }
}

/// disc 023 §4: a cocoon is a local frame that carries its own section, so
/// contagion stops at its boundary. `{{ }}` is not a hole in the guard, it is
/// where the obstruction is discharged — and it must keep working.
#[test]
fn g2_the_cocoon_still_discharges() {
    let got = observe("g2", "{{ %effect: #pure, v: ~%Time.now _ }}");
    assert!(
        !is_effect_violation(&got),
        "the cocoon shield is not a bug to be closed; got {got:?}"
    );
}

/// An honest declaration must stay writable. A repair that catches the false
/// ones by refusing every callback has not done the job — that is exactly the
/// path this arc did not take.
#[test]
fn g3_an_honest_pure_declaration_still_stands() {
    let got = observe("g3", &declared_pure("~%List./map ([1,2,3], (x -> x))"));
    assert!(
        !is_effect_violation(&got),
        "`(x -> x)` observes nothing; got {got:?}"
    );
    assert!(got.contains("#pure"), "REACH: got {got:?}");
}

/// Identity red line. Effects do not enter the BN/ hash, and making them
/// propagate must not change that: two computations of `[1, 1, 1]`, one pure
/// and one that consults the clock on the way, keep one address.
///
/// Measured at baseline: both `a04c6b85…`.
#[test]
fn g4_an_effect_does_not_move_an_address() {
    let pure = observe(
        "g4-pure",
        "~%Discovery./identify (~%List./map ([1,2,3], (x -> 1)))",
    );
    let impure = observe(
        "g4-impure",
        &format!("~%Discovery./identify (~%List./map ([1,2,3], {IMPURE_ONE}))"),
    );
    assert!(
        pure.contains("a04c6b853f9d16a0b4c5b35f6e25aa73fc983bdaa5b05514cbe52ffc77c42e28"),
        "REACH: the pure form must still address as measured; got {pure:?}"
    );
    assert_eq!(
        pure, impure,
        "both compute `[1, 1, 1]`. An effect is not content, so it may not \
         move an address. got {pure:?} vs {impure:?}"
    );
}

/// Sanity: the file's own impure fixtures really are impure. Without this,
/// every red above could go green by the morphisms quietly becoming pure.
#[test]
fn g5_the_fixtures_are_actually_impure() {
    for (tag, m) in [("apply", IMPURE), ("pred", IMPURE_PRED), ("one", IMPURE_ONE)] {
        let got = observe(&format!("g5-{tag}"), &format!("(({m}) 1).%effect"));
        assert_eq!(got, "#io", "`{tag}` must be impure at application; got {got:?}");
    }
}

