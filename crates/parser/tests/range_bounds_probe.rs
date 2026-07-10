// Range default-bound probes (2026-07-10, work order docs/range_eval_handover.md).
//
// SPEC_02 §3 (explicit, pre-dates the parser): omitted bounds default to the
// ORDER anchors `#_|_` (start) / `#_` (end) — the order extremes, NOT the
// information-lattice `_` (Top). The 08c0fd5 parser fix used Top for missing
// bounds, conflating the two lattices (SPEC firewall: 序極值 ≠ 資訊極值).
//
// NOTE for the fixer: golden_ast.rs currently pins the WRONG (Top) shapes for
// `..`/`..10`/`1..` — the work order explicitly AUTHORIZES updating those
// three vectors to the anchor shapes below (this is the one sanctioned golden
// change; everything else in golden_ast.rs stays).
//
// Acceptance = remove the #[ignore]s, everything green.

use nlang_parser::parse_expr_only;

fn shape(src: &str) -> String {
    parse_expr_only(src).unwrap_or_else(|e| panic!("parse {src:?}: {e}")).shape()
}

#[test]
#[ignore = "missing bounds default to Top, spec says order anchors (baseline 2026-07-10)"]
fn omitted_end_defaults_to_order_supremum() {
    assert_eq!(shape("1.."), "Range(Atom(Int(1)), Atom(TagEnd))");
}

#[test]
#[ignore = "missing bounds default to Top, spec says order anchors (baseline 2026-07-10)"]
fn omitted_start_defaults_to_order_infimum() {
    assert_eq!(shape("..10"), "Range(Atom(TagStart), Atom(Int(10)))");
}

#[test]
#[ignore = "missing bounds default to Top, spec says order anchors (baseline 2026-07-10)"]
fn full_range_defaults_to_both_anchors() {
    assert_eq!(shape(".."), "Range(Atom(TagStart), Atom(TagEnd))");
}
