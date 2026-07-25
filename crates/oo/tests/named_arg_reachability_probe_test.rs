// Named-parameter morphisms must be reachable (2026-07-25, pre-committed by
// work order: docs/named_arg_reachability_handover.md).
//
// DEFECT (measured on v0.2.38): `apply_morphism` (lib.rs ~1129-1140) builds
// `unified_arg` from POSITIONAL (numeric) keys only. A morphism argument's
// NAMED fields never reach the builtin. Every builtin that reads a top-level
// named field off `arg` is therefore unreachable from n/ source — by ANY
// spelling:
//
//   f #blur                  → unified_arg = {0: #blur}
//   f { strategy: #blur }    → unified_arg = {0: {strategy: #blur}}   (nested!)
//   f { 0: #blur }           → unified_arg = {0: #blur}
//
// none of which has a top-level `strategy` field. Consequences by builtin:
//
//   engine.check_oml     (a, b)          → VACUOUS #oml_valid for EVERY input
//   engine.project_up    (sections)      → vacuous (sections defaults to Top)
//   engine.project_down  (target, masa)  → ⊥ #conflict (dead, but loud)
//   engine.set_strategy  (strategy)      → ⊥ #conflict (dead, but loud)
//   disc fetch/find      (target)        → direct-lookup mode never fires;
//                                          silently degrades to similarity
//
// check_oml is the dangerous one: a VERIFICATION morphism that answers
// "valid" to everything, including deliberately non-equivalent inputs. Not a
// crash — a silent false assurance.
//
// This is a SPEC × ENGINE conformance gap, not merely an engine wart:
// SPEC_08 §3.5 normatively spells these with named parameters
// (`~%Engine./project_down { target: @Combo, masa: @caid }`).
//
// WHY IT SURVIVED: the existing tests (crates/interpreter/tests/bohr_test.rs)
// call the builtins DIRECTLY from `oo.builtin_registry`, hand-building the
// named-field combo — bypassing apply_morphism entirely. The registry-level
// contract and the apply-level contract disagree and nothing tested the seam.
// These probes are deliberately n/-LEVEL (via the CLI) for that reason.
//
// RULING (2026-07-25, user): fix at the APPLY layer — `unified_arg` also
// carries the argument's non-`%` named fields. One change fixes the whole
// family AND makes the engine match the spelling SPEC_08 §3.5 already
// documents (no spec change needed).
//
// BLAST RADIUS (measured): `unified_arg` is consumed at exactly TWO sites —
// the builtin call, and the curry/partial-application path (when a builtin
// returns Top, unified_arg's fields are merged into the partial morphism).
// The `%rules` branch and the pattern-key dispatch branch both read `&arg`,
// NOT unified_arg, and both `return` before the builtin branch — so they
// cannot be affected. The REAL risk is the curry path: named fields merged
// into a partial morphism could make it look like a pattern-dispatch table
// on the NEXT apply (`has_pattern_fields` = any non-%, non-numeric key).
// The pins below hold that seam.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn run_cli(src: &str) -> String {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "nlang-namedarg-{}-{}",
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&dir).ok();
    fs::create_dir_all(&dir).unwrap();
    let p: PathBuf = dir.join("a.n");
    fs::write(&p, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_oo"))
        .arg("run")
        .arg(&p)
        .arg("--observe")
        .arg("out")
        .current_dir(&dir)
        .output()
        .unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    )
}

fn is_bottom(s: &str) -> bool {
    s.starts_with("_|_")
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — the silent false assurance (worst first)
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn red_check_oml_sees_its_arguments() {
    // THE load-bearing case. check_oml must actually inspect a and b. Today
    // both default to Top (never delivered), so verify_oml(Top, Top) answers
    // #oml_valid to everything. A verification morphism that cannot fail is
    // worse than one that errors.
    //
    // Post-fix: deliberately incompatible operands must NOT read #oml_valid.
    let got = run_cli(r#"out: ~%Engine./check_oml { a: @int, b: "str" }"#);
    assert!(
        !got.contains("oml_valid"),
        "check_oml must not certify incompatible operands as valid: {got:?}"
    );
}

#[test]
#[ignore]
fn red_check_oml_discriminates() {
    // Both halves of the boundary in ONE assertion, because either half alone
    // passes vacuously at baseline (everything answers #oml_valid, so the
    // "compatible verifies" half is green for the wrong reason, and pinning
    // only the negative would let "always ⊥" through). Measured baseline:
    // compatible == incompatible == "#oml_valid". They must DIFFER, and the
    // compatible side must be the one that verifies.
    let compatible = run_cli("out: ~%Engine./check_oml { a: 1, b: 1 }");
    let incompatible = run_cli(r#"out: ~%Engine./check_oml { a: @int, b: "str" }"#);
    assert_ne!(
        compatible, incompatible,
        "check_oml must distinguish compatible from incompatible operands"
    );
    assert!(
        compatible.contains("oml_valid"),
        "compatible operands must verify: {compatible:?}"
    );
}

#[test]
#[ignore]
fn red_project_up_sees_sections() {
    // project_up reads `sections` → never delivered → defaults to Top → the
    // builtin returns Top → apply hands back a PARTIAL morphism combo (that
    // is the `{{ %builtin: "engine.project_up" … }}` seen at baseline, not a
    // reconstructed state). So the observable is: it must stop being a
    // partial, i.e. actually consume `sections` and produce a value.
    let got = run_cli("out: ~%Engine./project_up { sections: [1, 2] }");
    assert!(
        !got.contains("%builtin"),
        "project_up must consume `sections`, not hand back a partial: {got:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — the loud-dead morphisms (SPEC_08 §3.5 normative spelling)
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn red_set_strategy_reachable() {
    // §3.2/§4.4 strategy override. Today ⊥ #conflict in every spelling.
    let got = run_cli("out: ~%Engine./set_strategy { strategy: #blur }");
    assert!(
        !is_bottom(&got),
        "set_strategy must be reachable with its named parameter: {got:?}"
    );
}

#[test]
#[ignore]
fn red_set_strategy_discriminates_valid_from_bogus() {
    // Reachability must not become permissiveness. Paired for the same reason
    // as check_oml: at baseline BOTH spellings are ⊥ #conflict (for the wrong
    // reason — the argument never arrived), so a lone "bogus still ⊥" pin
    // would pass vacuously. Post-fix the builtin's own validation actually
    // runs: a known strategy succeeds, an unknown one still collapses.
    let good = run_cli("out: ~%Engine./set_strategy { strategy: #blur }");
    let bogus = run_cli("out: ~%Engine./set_strategy { strategy: #bogus }");
    assert!(
        !is_bottom(&good),
        "a known strategy must be accepted: {good:?}"
    );
    assert!(
        is_bottom(&bogus),
        "an unknown strategy must still collapse: {bogus:?}"
    );
}

#[test]
#[ignore]
fn red_project_down_receives_target() {
    // SPEC_08 §3.5 normative spelling. A bad MASA collapses either way (the
    // builtin's own ContentHash::parse guard), so ⊥-vs-not cannot separate
    // "arg never arrived" from "arg arrived and was rejected". Discriminate
    // instead on the TARGET: with the fix, omitting `target` and supplying it
    // must not produce byte-identical output. Measured baseline: identical
    // (both are the generic arg-shape ⊥).
    let with_target =
        run_cli(r#"out: ~%Engine./project_down { target: { x: 1 }, masa: "_" }"#);
    let without_target = run_cli(r#"out: ~%Engine./project_down { masa: "_" }"#);
    assert_ne!(
        with_target, without_target,
        "project_down must actually receive `target`"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — what the apply-layer change must NOT break
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_curry_positional_still_works() {
    // unified_arg consumer #2: partial application. Numeric slots must keep
    // accumulating exactly as before.
    assert_eq!(run_cli("out: (~%Math./add 1) 2"), "3");
}

#[test]
fn pin_curry_via_binding_still_works() {
    // Same, through a binding (the partial morphism is stored and re-applied
    // — this is the path where a stray named field would persist).
    assert_eq!(run_cli("add1: ~%Math./add 1\nout: add1 5"), "6");
}

#[test]
fn pin_curry_argpack_overwrites_slot() {
    // Measured baseline: `{0: 2}` is an ARG PACK (has "0", no "%kind"), so it
    // splices over slot 0 rather than filling slot 1 — the result stays a
    // partial. Pinned as-is; the apply-layer change must not disturb the
    // arg-pack path while adding named fields alongside it.
    let got = run_cli("out: (~%Math./add 1) { 0: 2 }");
    assert!(
        got.contains("%builtin: \"math.add\"") && got.contains("0: 2"),
        "arg-pack splice must keep overwriting slot 0 into a partial: {got:?}"
    );
}

#[test]
fn pin_named_arg_partial_does_not_become_a_pattern_table() {
    // THE seam pin for this arc. After the fix a named-field argument's keys
    // are merged into `unified_arg`; when a builtin partials (returns Top)
    // those keys land in the partial morphism combo. On the NEXT apply the
    // pattern-key branch fires on ANY non-%, non-numeric key — so the partial
    // could be misread as a dispatch table and silently answer from it
    // instead of computing. Measured baseline: this collapses to ⊥ #conflict.
    // It must NOT start returning a dispatched value (e.g. the stray `foo`).
    let got = run_cli("p: ~%Math./add { foo: 1 }\nout: p 2");
    assert!(
        is_bottom(&got),
        "a partial carrying named keys must not turn into a pattern table: {got:?}"
    );
}

#[test]
fn pin_pattern_dispatch_numeric_keys_unaffected() {
    // Pattern-key dispatch reads &arg, not unified_arg — must be untouched.
    assert_eq!(
        run_cli("f: { %morphism: #true, 1: \"one\", 2: \"two\" }\nout: f 1"),
        "\"one\""
    );
}

#[test]
fn pin_pattern_dispatch_range_keys_unaffected() {
    assert_eq!(
        run_cli("g: { %morphism: #true, @{1..3}: \"low\", @{4..}: \"high\" }\nout: g 5"),
        "\"high\""
    );
}

#[test]
fn pin_plain_builtin_bare_arg_unaffected() {
    // The overwhelmingly common case: a bare positional argument.
    assert_eq!(run_cli("out: ~%Math./sqrt 16"), "4");
}

#[test]
fn pin_named_arg_to_rules_morphism_unchanged() {
    // The %rules branch also reads &arg and returns before the builtin
    // branch; its behaviour for a named-field argument must not shift.
    assert!(is_bottom(&run_cli(
        "h: { %morphism: #true, %rules: { a: \"A\" } }\nout: h { a: 1 }"
    )));
}
