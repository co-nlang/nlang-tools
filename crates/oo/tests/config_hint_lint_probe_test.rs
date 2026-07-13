// R5 node-level horizon-hint lint probes (2026-07-13, pre-committed by
// work order — docs/config_home_handover.md). 想法 D Tier 1 instrument.
//
// RULING (SPEC_08 §3.1): node-level %fuel/%timeout/%strategy/%max_* are
// ADVISORY hints — this engine ignores them (normative home = ~%Config).
// A silently ignored config is a trap (same family as the ~% shadow
// silence gap): the user believes they set a budget; nothing happened.
// R5 = Warn on node-level horizon-param declarations, message pointing
// at the ~%Config home. Lint, not error — the spelling stays LEGAL
// (advisory latitude for other engines).
//
// Conservative stance (寧漏勿誤, R4 precedent): only the seven horizon
// param names are flagged; %kind/%fmap/%bind/%termination_proof and all
// other % traits are NEVER flagged.

use oo::nlint;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn lint_src(src: &str) -> Vec<nlint::Diagnostic> {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "nlang-r5lint-{}-{}",
        std::process::id(),
        DIR_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::remove_dir_all(&dir).ok();
    fs::create_dir_all(&dir).unwrap();
    let p: PathBuf = dir.join("probe.n");
    fs::write(&p, src).unwrap();
    let report = nlint::analyze_file(&p);
    assert!(
        report.parse_error.is_none(),
        "parse failed: {:?}",
        report.parse_error
    );
    report.diagnostics
}

fn r5(diags: &[nlint::Diagnostic]) -> Vec<&nlint::Diagnostic> {
    diags.iter().filter(|d| d.rule == "R5").collect()
}

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — R5 fires on node-level horizon hints
// ─────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "config red gate: awaiting R5 horizon-hint lint"]
fn red_r5_node_fuel_hint_warns() {
    let diags = lint_src("x: {\n    %fuel: 5000\n    val: 42\n}\n");
    let hits = r5(&diags);
    assert!(
        !hits.is_empty(),
        "R5 must flag node-level %fuel; diagnostics: {diags:?}"
    );
    let msg = &hits[0].msg;
    assert!(
        msg.contains("%fuel"),
        "R5 message must name the hinted field: {msg:?}"
    );
    assert!(
        msg.contains("~%Config"),
        "R5 message must point at the normative home: {msg:?}"
    );
}

#[test]
#[ignore = "config red gate: awaiting R5 horizon-hint lint"]
fn red_r5_node_strategy_hint_warns() {
    let diags = lint_src("y: {\n    %strategy: #strict\n    v: 1\n}\n");
    assert!(
        !r5(&diags).is_empty(),
        "R5 must flag node-level %strategy; diagnostics: {diags:?}"
    );
}

#[test]
#[ignore = "config red gate: awaiting R5 horizon-hint lint"]
fn red_r5_nested_hint_warns() {
    // Hints at any nesting depth are still hints.
    let diags = lint_src("outer: {\n    inner: {\n        %timeout: 50\n    }\n}\n");
    assert!(
        !r5(&diags).is_empty(),
        "R5 must flag nested %timeout; diagnostics: {diags:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE PINS — non-horizon traits never flagged; R4 coexistence
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn pin_r5_silent_on_kind_trait() {
    let diags = lint_src("x: {\n    %kind: #data\n    val: 1\n}\n");
    assert!(
        r5(&diags).is_empty(),
        "%kind is a real trait, never R5: {diags:?}"
    );
}

#[test]
fn pin_r5_silent_on_termination_proof() {
    let diags = lint_src("f: (x -> x)\ng: {\n    %termination_proof: #manual\n    h: 1\n}\n");
    assert!(
        r5(&diags).is_empty(),
        "%termination_proof is a real trait, never R5: {diags:?}"
    );
}

#[test]
fn pin_r4_still_fires_alongside() {
    // Cross-rule guard: R4 (use-without-def) keeps firing.
    let diags = lint_src("out: never_defined_name\n");
    assert!(
        diags.iter().any(|d| d.rule == "R4"),
        "R4 regression: {diags:?}"
    );
}
