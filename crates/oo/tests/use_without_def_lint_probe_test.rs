// R4 use-WITHOUT-def lint probes (2026-07-12, pre-committed by work
// order — docs/union_dedupe_lint_handover.md). 想法 D Tier 1.
//
// After the forward-ref arc, use-BEFORE-def is legal (L1-26/27,
// simultaneity) — the lint target is names never defined anywhere in
// the file. Open-world means this is legal SEMANTICS (observes to `_`),
// so R4 is a lint, not an error:
//   - bare name never defined  → R4 Warn (one-shot observation will be `_`)
//   - `& @Name` never defined  → R4 Warn, msg names the marker (silent
//     pass-through = the user believes they are enforcing — E4 legacy)
// Conservative stance (寧漏勿誤): a name defined ANYWHERE in the file
// (any nesting level), any morphism param, any builtin/system name is
// NEVER flagged — under-report rather than false-alarm.

use oo::nlint;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn lint_src(src: &str) -> Vec<nlint::Diagnostic> {
    let dir = nlang_interpreter::ScratchDir::new("r4lint");
    let p: PathBuf = dir.join("probe.n");
    fs::write(&p, src).unwrap();
    let report = nlint::analyze_file(&p);
    assert!(
        report.parse_error.is_none(),
        "probe source must parse: {:?}",
        report.parse_error
    );
    report.diagnostics
}

fn r4s(src: &str) -> Vec<nlint::Diagnostic> {
    lint_src(src)
        .into_iter()
        .filter(|d| d.rule == "R4")
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────
// RED LINES — R4 does not exist today
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn r4_undefined_bare_name_flagged() {
    let ds = r4s("out: zzz_undef + 1");
    assert!(
        ds.iter().any(|d| d.msg.contains("zzz_undef")),
        "must flag zzz_undef, got {ds:?}"
    );
}

#[test]
// (silent pass-through = enforcement the user believes in)
fn r4_undefined_type_marker_flagged() {
    let ds = r4s("x: { age: 20 } & @Never\nout: x");
    let hit = ds
        .iter()
        .find(|d| d.msg.contains("Never"))
        .unwrap_or_else(|| panic!("must flag @Never, got {ds:?}"));
    assert_eq!(hit.severity, nlint::Severity::Warn);
}

#[test]
fn r4_undefined_in_container_flagged() {
    let ds = r4s("s: { v: qqq_undef }\nout: s.v");
    assert!(
        ds.iter().any(|d| d.msg.contains("qqq_undef")),
        "must flag qqq_undef, got {ds:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE pins — what R4 must NEVER flag (寧漏勿誤).
// Vacuously green until R4 exists; load-bearing from delivery on.
// ─────────────────────────────────────────────────────────────────────────

#[test] // ACTIVE pin: forward ref is LEGAL (L1-26/27) — no R4
fn pin_r4_forward_ref_not_flagged() {
    assert!(r4s("out: a\na: 5").is_empty());
}

#[test] // ACTIVE pin: builtin type markers — no R4
fn pin_r4_builtin_marker_not_flagged() {
    assert!(r4s("x: 1 & @int\nout: x").is_empty());
}

#[test] // ACTIVE pin: morphism params — no R4
fn pin_r4_morphism_param_not_flagged() {
    assert!(r4s("f: (n -> n + 1)\nout: /f 2").is_empty());
}

#[test] // ACTIVE pin: system modules (~%) — no R4
fn pin_r4_system_module_not_flagged() {
    assert!(r4s("out: ~%Math.abs (0 - 3)").is_empty());
}

#[test] // ACTIVE pin: nested definition referenced by path — no R4
fn pin_r4_nested_def_not_flagged() {
    assert!(r4s("s: { v: 1 }\nout: s.v").is_empty());
}

#[test] // ACTIVE pin: @Name defined in-file (nominal, E4) — no R4
fn pin_r4_defined_type_not_flagged() {
    assert!(r4s("@Adult: { age: 18.. }\nx: { age: 20 } & @Adult\nout: x").is_empty());
}

#[test] // ACTIVE pin: `$` horizon and `_` literal — no R4
fn pin_r4_horizon_and_top_not_flagged() {
    assert!(r4s("w: { x: $.s }\ny: _\nout: y").is_empty());
}

#[test] // ACTIVE pin: existing rules unaffected — R3 still fires
        // (sealed LHS × keyed transformer, shape from r3_fixture.n)
fn pin_existing_r3_still_fires() {
    let ds = lint_src("r3_trigger: (1, 2) |> { s: $.0 }");
    assert!(
        ds.iter().any(|d| d.rule == "R3"),
        "R3 must still fire, got {ds:?}"
    );
}

#[test] // ACCEPTANCE GUARD (added at acceptance, 2026-07-12): multi-param
        // morphism (`x y -> …` = Apply-shaped param) binds ALL names —
        // the false positive found in tests/lib/test.n must stay dead
fn pin_r4_multi_param_morphism_not_flagged() {
    assert!(r4s("f: x y -> x == y\nout: /f 1 1").is_empty());
    assert!(r4s("g: a b c -> a + b + c\nout: /g 1 2 3").is_empty());
}
