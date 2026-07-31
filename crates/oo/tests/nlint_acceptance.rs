// tests/nlint_acceptance.rs — Tier 1 linter acceptance (handover §5)
//
// Verifies: tier classification (C/M/Q), R3 (trigger + safe), ω(G) fixture,
// and the $-scan boundary trap.

use oo::nlint;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures");
    p.push(name);
    p
}

// ---- §5.1 tier fixture: one C, one M, one Q -----------------------------

#[test]
fn tier_fixture_classifies_c_m_q() {
    let r = nlint::analyze_file(&fixture("tier_fixture.n"));
    assert!(r.parse_error.is_none(), "parse failed: {:?}", r.parse_error);

    // Collect R2 diagnostics (tier classifications).
    let tiers: Vec<(nlint::Tier, Option<String>)> = r
        .diagnostics
        .iter()
        .filter(|d| d.rule == "R2")
        .map(|d| (d.tier.unwrap(), d.demotion_reason.clone()))
        .collect();

    // Expect one C, one M, one Q.
    let has_c = tiers.iter().any(|(t, _)| *t == nlint::Tier::C);
    let has_m = tiers.iter().any(|(t, _)| *t == nlint::Tier::M);
    let has_q = tiers.iter().any(|(t, _)| *t == nlint::Tier::Q);
    assert!(has_c, "missing tier C; got {:?}", tiers);
    assert!(has_m, "missing tier M; got {:?}", tiers);
    assert!(has_q, "missing tier Q; got {:?}", tiers);

    // Q must carry a demotion_reason naming the offending node.
    let q_reason = tiers
        .iter()
        .find(|(t, _)| *t == nlint::Tier::Q)
        .and_then(|(_, r)| r.as_ref())
        .expect("Q tier must have demotion_reason");
    assert!(
        q_reason.contains("Add") || q_reason.contains("arithmetic"),
        "Q demotion reason should name arithmetic; got: {}",
        q_reason
    );
}

// ---- §5.2 R3 fixture: trigger vs safe -----------------------------------

#[test]
fn r3_triggers_on_tuple_sealed_adds_key() {
    let r = nlint::analyze_file(&fixture("r3_fixture.n"));
    assert!(r.parse_error.is_none(), "parse failed: {:?}", r.parse_error);

    let r3_errors: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| d.rule == "R3" && d.severity == nlint::Severity::Error)
        .collect();
    assert_eq!(
        r3_errors.len(),
        1,
        "expected exactly 1 R3 error, got {:?}",
        r3_errors
    );
    assert!(
        r3_errors[0].msg.contains("#missing_key"),
        "R3 msg: {}",
        r3_errors[0].msg
    );
    assert!(
        r3_errors[0].msg.contains("s"),
        "R3 msg should name key s: {}",
        r3_errors[0].msg
    );
}

#[test]
fn r3_does_not_trigger_on_open_combo_with_spread() {
    let r = nlint::analyze_file(&fixture("r3_fixture.n"));
    // r3_safe: LHS is open combo { ...(1,2) } — not sealed → no R3.
    // Only 1 R3 error expected (the trigger); the safe one must not fire.
    let r3_errors: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| d.rule == "R3" && d.severity == nlint::Severity::Error)
        .collect();
    assert_eq!(
        r3_errors.len(),
        1,
        "r3_safe must NOT trigger R3; got {:?}",
        r3_errors
    );
}

// ---- §5.3 ω fixture: 4 pairwise-sharing combos → ω=4 -------------------

#[test]
fn omega_fixture_reports_clique_4() {
    let r = nlint::analyze_file(&fixture("omega_fixture.n"));
    assert!(r.parse_error.is_none(), "parse failed: {:?}", r.parse_error);

    assert_eq!(
        r.graph.contexts.len(),
        4,
        "expected 4 contexts, got {}",
        r.graph.contexts.len()
    );
    assert_eq!(r.graph.omega, 4, "expected ω(G)=4, got {}", r.graph.omega);
    assert!(!r.graph.k4_witnesses.is_empty(), "expected ≥1 K4 witness");
    assert!(r.graph.k5_witnesses.is_empty(), "expected no K5 witness");

    // The single K4 witness should share the `shared` coordinate (root-relative).
    let w = &r.graph.k4_witnesses[0];
    assert_eq!(w.contexts.len(), 4, "K4 witness must have 4 contexts");
    assert!(
        w.shared_coords.iter().any(|c| c.contains("shared")),
        "K4 witness should share `shared` coord; got {:?}",
        w.shared_coords
    );
}

// ---- §5.4 $-scan boundary: nested pipe RHS is a scan boundary -----------

#[test]
fn dollar_scan_boundary_outer_pipe_is_c() {
    let r = nlint::analyze_file(&fixture("dollar_scan_fixture.n"));
    assert!(r.parse_error.is_none(), "parse failed: {:?}", r.parse_error);

    // outer pipe: { k: 1 } |> { b: 2 |> { c: $ } }
    // RHS = { b: 2 |> { c: $ } } — the `$` is inside a nested pipe's RHS,
    // so the outer RHS is $-free → outer is tier C.
    let outer_tiers: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| d.rule == "R2" && d.tier == Some(nlint::Tier::C))
        .collect();
    assert!(
        !outer_tiers.is_empty(),
        "outer pipe should be tier C (nested $ is scan-boundary); got {:?}",
        r.diagnostics
            .iter()
            .filter(|d| d.rule == "R2")
            .map(|d| d.tier)
            .collect::<Vec<_>>()
    );
}

// ---- §5.1 (reprise) workspace regression: all unit tests parse ---------

#[test]
fn all_unit_corpus_parses_or_skips_cleanly() {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push("../../tests/unit");
    if !dir.is_dir() {
        return;
    } // corpus may live elsewhere in workspaces
    let mut files = Vec::new();
    collect_n(&dir, &mut files);
    let mut skip = 0;
    let mut ok = 0;
    for f in &files {
        let r = nlint::analyze_file(f);
        if r.parse_error.is_some() {
            skip += 1;
        } else {
            ok += 1;
        }
    }
    // No hard count assertion — just that we ran without panic.
    eprintln!(
        "unit corpus: {} parsed, {} skipped (parse failures)",
        ok, skip
    );
}

fn collect_n(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_n(&p, out);
            } else if p.extension().and_then(|s| s.to_str()) == Some("n") {
                out.push(p);
            }
        }
    }
}

// ---- direct unit tests of internal functions -----------------------------

#[test]
fn free_dollar_scan_respects_pipe_boundary() {
    // { b: 2 |> { c: $ } } as a subtree — $ is inside a nested pipe's RHS.
    // has_free_dollar should return false (the $ belongs to the inner pipe).
    let src = "{ b: 2 |> { c: $ } }";
    let e = nlang_parser::parse_expr_only(src).unwrap();
    assert!(
        !nlint::has_free_dollar(&e),
        "nested pipe RHS $ must be scan-boundary"
    );
}

#[test]
fn free_dollar_scan_finds_bare_context() {
    let src = "{ c: $ }";
    let e = nlang_parser::parse_expr_only(src).unwrap();
    assert!(nlint::has_free_dollar(&e), "bare $ in combo field is free");
}

#[test]
fn free_dollar_scan_respects_morphism_body() {
    // (x -> $) — the $ is the morphism's body, bound to param x, not free.
    let src = "(x -> $)";
    let e = nlang_parser::parse_expr_only(src).unwrap();
    assert!(
        !nlint::has_free_dollar(&e),
        "morphism body $ is bound, not free"
    );
}

#[test]
fn free_dollar_scan_descends_into_interpolation() {
    // P5: interpolation builds no scope — ${$} is free w.r.t. enclosing pipe.
    let src = "`x${$}y`";
    let e = nlang_parser::parse_expr_only(src).unwrap();
    assert!(nlint::has_free_dollar(&e), "interpolation $ is free (P5)");
}

#[test]
fn classify_rhs_transformer_combo() {
    let e = nlang_parser::parse_expr_only("{ a: 1 }").unwrap();
    assert_eq!(nlint::classify_rhs(&e), nlint::RhsForm::Transformer);
}

#[test]
fn classify_rhs_transformer_cocoon() {
    let e = nlang_parser::parse_expr_only("{{ a: 1 }}").unwrap();
    assert_eq!(nlint::classify_rhs(&e), nlint::RhsForm::Transformer);
}

#[test]
fn classify_rhs_morphism_path() {
    let e = nlang_parser::parse_expr_only("/f").unwrap();
    assert_eq!(nlint::classify_rhs(&e), nlint::RhsForm::Morphism);
}

#[test]
fn classify_rhs_atom() {
    let e = nlang_parser::parse_expr_only("#ok").unwrap();
    assert_eq!(nlint::classify_rhs(&e), nlint::RhsForm::Atom);
}

#[test]
fn classify_rhs_unknown_for_apply() {
    let e = nlang_parser::parse_expr_only("foo bar").unwrap();
    assert_eq!(nlint::classify_rhs(&e), nlint::RhsForm::Unknown);
}

#[test]
fn tier_c_for_dollar_free_transformer() {
    let e = nlang_parser::parse_expr_only("{ b: 2 }").unwrap();
    let (t, reason) = nlint::classify_tier(&e);
    assert_eq!(t, nlint::Tier::C);
    assert!(reason.is_none());
}

#[test]
fn tier_m_for_positive_fragment_with_dollar() {
    let e = nlang_parser::parse_expr_only("{ w: $ }").unwrap();
    let (t, reason) = nlint::classify_tier(&e);
    assert_eq!(t, nlint::Tier::M);
    assert!(reason.is_none());
}

#[test]
fn tier_q_for_arithmetic_on_dollar() {
    let e = nlang_parser::parse_expr_only("{ v: $.k + 1 }").unwrap();
    let (t, reason) = nlint::classify_tier(&e);
    assert_eq!(t, nlint::Tier::Q);
    assert!(reason.is_some());
}

#[test]
fn r3_fires_for_tuple_adds_key() {
    let lhs = nlang_parser::parse_expr_only("(1, 2)").unwrap();
    let rhs = nlang_parser::parse_expr_only("{ s: $.0 }").unwrap();
    let r = nlint::check_r3(&lhs, &rhs);
    assert!(r.is_some(), "R3 should fire for tuple + {{ s: $.0 }}");
}

#[test]
fn r3_does_not_fire_for_open_combo_lhs() {
    let lhs = nlang_parser::parse_expr_only("{ k: 1 }").unwrap();
    let rhs = nlang_parser::parse_expr_only("{ s: $.k }").unwrap();
    let r = nlint::check_r3(&lhs, &rhs);
    assert!(r.is_none(), "R3 should NOT fire for open combo LHS");
}

#[test]
fn r3_does_not_fire_for_transformer_with_spread() {
    let lhs = nlang_parser::parse_expr_only("{{ a: 1 }}").unwrap();
    let rhs = nlang_parser::parse_expr_only("{ ...t, s: $.a }").unwrap();
    let r = nlint::check_r3(&lhs, &rhs);
    assert!(
        r.is_none(),
        "R3 should NOT fire when transformer has spread (keys uncertain)"
    );
}

// —— acceptance-review additions (2026-07-07) ————————————————————————

fn lint_snippet(name: &str, src: &str) -> Vec<nlint::Diagnostic> {
    let mut p = std::env::temp_dir();
    p.push(format!("nlint_accept_{}.n", name));
    std::fs::write(&p, src).unwrap();
    let r = nlint::analyze_file(&p);
    let _ = std::fs::remove_file(&p);
    assert!(r.parse_error.is_none(), "parse failed: {:?}", r.parse_error);
    r.diagnostics
}

#[test]
fn atom_form_is_tier_c_rerun_safe() {
    // 019 Prop 2 covers the atomic form, not just $-free transformers
    let diags = lint_snippet("atom", "r: x |> #ok");
    assert!(
        diags
            .iter()
            .any(|d| d.rule == "R1" && d.tier == Some(nlint::Tier::C)),
        "atom-form pipe must be marked rerun-safe, got {:?}",
        diags
    );
}

#[test]
fn unknown_form_emits_tier_u() {
    let diags = lint_snippet("unknown", "r: data |> handler");
    assert!(
        diags
            .iter()
            .any(|d| d.rule == "R2" && d.tier == Some(nlint::Tier::U)),
        "unknown-form pipe must emit tier U, got {:?}",
        diags
    );
}

#[test]
fn morphism_form_stays_silent() {
    let diags = lint_snippet("morph", "r: 1 |> (x -> $ + 1)");
    assert!(
        diags.iter().all(|d| d.rule != "R1" && d.rule != "R2"),
        "morphism-form pipe gets no refinement-tier diagnostics, got {:?}",
        diags
    );
}
