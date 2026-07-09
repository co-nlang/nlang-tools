//! Golden-AST: SYNTAX §4 edge cases + known silent-deformation bugs.
//!
//! Compares span-free structural fingerprints (`Expr::shape` / `Program::shape`).
//! Source of truth for vectors: SYNTAX_01–12 §4 + ENGINE_SYNC / ROADMAP incidents.

use nlang_parser::ast::{AtomKind, ExprKind};
use nlang_parser::{parse_expr_only, parse_program};

fn shape_expr(src: &str) -> String {
    parse_expr_only(src)
        .unwrap_or_else(|e| panic!("parse_expr_only({src:?}) failed: {e}"))
        .shape()
}

fn assert_shape(src: &str, expected: &str) {
    let got = shape_expr(src);
    assert_eq!(got, expected, "shape mismatch for {src:?}");
}

fn assert_top_kind(src: &str, pred: impl FnOnce(&ExprKind) -> bool) {
    let e = parse_expr_only(src).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
    assert!(pred(&e.kind), "unexpected kind for {src:?}: {:?}", e.kind);
}

// ---------------------------------------------------------------------------
// SYNTAX_01 / 02 — lexical & literals
// ---------------------------------------------------------------------------

#[test]
fn golden_atoms_and_units() {
    assert_shape("_", "Atom(Top)");
    assert_shape("_|_", "Atom(Bottom)");
    assert_shape("()", "Atom(Unit)");
    assert_shape("#active", "Atom(Tag(active))");
    assert_shape("#_|_", "Atom(TagStart)");
    assert_shape("#_", "Atom(TagEnd)");
    assert_shape("{}", "Combo(closed=false, [], [])");
    assert_shape("{{}}", "Combo(closed=true, [], [])");
    assert_shape("@{}", "AnonSet(Atom(Bottom))");
}

#[test]
fn golden_numbers_and_complex() {
    assert_shape("42", "Atom(Int(42))");
    assert_shape("-7", "Atom(Int(-7))");
    assert_shape("1.5", "Atom(Float(1.5))");
    assert_shape("1.0e5", "Atom(Float(100000))");
    // complex: single token vs spaced addition (SYNTAX_02 §4.10)
    assert_shape("2+3i", "Atom(Complex(2,3))");
    assert_shape("3i", "Atom(Complex(0,3))");
    assert_shape("i", "Atom(Complex(0,1))");
    assert_shape("-i", "Atom(Complex(0,-1))");
    assert_shape("2 + 3i", "Add(Atom(Int(2)), Atom(Complex(0,3)))");
}

#[test]
fn golden_complex_does_not_eat_idents() {
    // 2026-07-06 silent deformation: complex_lit trailing guard
    assert_shape("io", "Path(Bare:io)");
    assert_shape("input", "Path(Bare:input)");
    assert_shape("i-1", "Path(Bare:i-1)");
    assert_shape("i2", "Path(Bare:i2)");
    assert_shape("i - 1", "Sub(Atom(Complex(0,1)), Atom(Int(1)))");
}

#[test]
fn golden_kebab_vs_subtraction() {
    // SYNTAX_01 §4.8 / SYNTAX_02 §4.3
    assert_shape("a-1", "Path(Bare:a-1)");
    // binary `-` consumes the operator; RHS is the following atom (`1`, not `-1`)
    assert_shape("a -1", "Sub(Path(Bare:a), Atom(Int(1)))");
    assert_shape("a - 1", "Sub(Path(Bare:a), Atom(Int(1)))");
    assert_shape("f (-1)", "Apply(Path(Bare:f), Atom(Int(-1)))");
}

#[test]
fn golden_strings() {
    assert_shape("\"hi\"", "Atom(Str(hi))");
    assert_shape("r\"a+b\"", "Atom(Regex(a+b))");
    assert_shape("p\"/tmp\"", "Atom(PathLit(/tmp))");
    assert_shape("u\"http://x\"", "Atom(Uri(http://x))");
    assert_shape("t\"2020-01-01\"", "Atom(Time(2020-01-01))");
}

// ---------------------------------------------------------------------------
// SYNTAX_03 — paths
// ---------------------------------------------------------------------------

#[test]
fn golden_paths() {
    assert_shape("a.b.c", "Path(Bare:a.b.c)");
    assert_shape("_.app", "Path(Root:app)");
    assert_shape("^.local", "Path(Parent(0):local)");
    assert_shape("^^.global", "Path(Parent(1):global)");
    assert_shape("_.", "Path(Root:)");
}

// ---------------------------------------------------------------------------
// SYNTAX_04 — containers, spread, range, tuple
// ---------------------------------------------------------------------------

#[test]
fn golden_list_tuple_spread() {
    assert_shape("[1, 2]", "List([Atom(Int(1)), Atom(Int(2))])");
    assert_shape("[1; 2; 3,]", "List([Atom(Int(1)), Atom(Int(2)), Atom(Int(3))])");
    assert_shape("(1, 2)", "Tuple([Atom(Int(1)), Atom(Int(2))])");
    assert_shape("(1,)", "Tuple([Atom(Int(1))])");
    // grouping, not 1-tuple
    assert_shape("(1)", "Atom(Int(1))");
    // element-position spread must not drop dots (ROADMAP silent bug #17)
    assert_shape("[...xs, 1]", "List([Spread(Path(Bare:xs)), Atom(Int(1))])");
    assert_shape("[xs, 1]", "List([Path(Bare:xs), Atom(Int(1))])");
}

#[test]
fn golden_range() {
    assert_shape("1..10", "Range(Atom(Int(1)), Atom(Int(10)))");
    assert_shape("-5..5", "Range(Atom(Int(-5)), Atom(Int(5)))");
    assert_shape("0..10..2", "Range(Atom(Int(0)), Atom(Int(10)), Atom(Int(2)))");
}

#[test]
fn golden_combo_fields() {
    let p = parse_program("a: 1\nb: 2").unwrap();
    // field_key prefers `path` over `named_key`, so bare idents are PathKey
    assert_eq!(
        p.shape(),
        "Program[PathKey(Bare:a):Atom(Int(1)); PathKey(Bare:b):Atom(Int(2))]"
    );
}

// ---------------------------------------------------------------------------
// SYNTAX_05 — prefixes
// ---------------------------------------------------------------------------

#[test]
fn golden_prefix_keys() {
    let p = parse_program("~@Schema: 1\n~/helper: x -> x\n~%sys: 1\n%meta: 1\n@int: 0\n/f: x -> x")
        .unwrap_or_else(|e| panic!("{e}"));
    let s = p.shape();
    // path-preferred keys: prefixes live inside the path segment text
    for needle in [
        "PathKey(Bare:~@Schema)",
        "PathKey(Bare:~/helper)",
        "PathKey(Bare:~%sys)",
        "PathKey(Bare:%meta)",
        "PathKey(Bare:@int)",
        "PathKey(Bare:/f)",
    ] {
        assert!(s.contains(needle), "missing {needle} in {s}");
    }
}

// ---------------------------------------------------------------------------
// SYNTAX_06–12 highlights
// ---------------------------------------------------------------------------

#[test]
fn golden_ops_and_precedence() {
    // meet tighter than cmp (SPEC_14 / ENGINE_SYNC #1)
    assert_shape("a < b & c", "Lt(Path(Bare:a), Meet(Path(Bare:b), Path(Bare:c)))");
    assert_shape("a = b", "LatticeEq(Path(Bare:a), Path(Bare:b))");
    assert_shape("a <=> b", "Probe(Path(Bare:a), Path(Bare:b))");
    assert_shape("a == b", "Eq(Path(Bare:a), Path(Bare:b))");
    assert_shape("a | b", "Join(Path(Bare:a), Path(Bare:b))");
    assert_shape("a \\ b", "Diff(Path(Bare:a), Path(Bare:b))");
    // grammar: unary_op = "!" → Unary(Not, …); Complement is a semantic alias
    assert_shape("!x", "Unary(Not, Path(Bare:x))");
}

#[test]
fn golden_pipe_morphism_structural_context() {
    assert_shape("x |> /f", "Pipe(Path(Bare:x), Path(Bare:/f))");
    assert_shape("x -> x", "Morphism(Path(Bare:x), Path(Bare:x))");
    assert_shape("<<x>>", "Structural(Path(Bare:x))");
    assert_shape("$", "Context");
}

#[test]
fn golden_poset() {
    assert_shape(
        "#{ #draft <= #review < #publish }",
        "Poset([Tag(draft)<=Tag(review), Tag(review)<Tag(publish)])",
    );
    assert_shape("#{ #a = #b }", "Poset([Tag(a)=Tag(b)])");
}

#[test]
fn golden_ternary_and_infix_logic() {
    assert_shape(
        "a ? b : c",
        "Ternary(Path(Bare:a), Path(Bare:b), Path(Bare:c))",
    );
    // a /f b → Apply(Apply(/f, a), b)
    assert_shape(
        "a /f b",
        "Apply(Apply(Path(Bare:/f), Path(Bare:a)), Path(Bare:b))",
    );
    assert_shape("a / b", "Div(Path(Bare:a), Path(Bare:b))");
}

#[test]
fn golden_lens() {
    // dotted bare idents are Paths; Lens is postfix on non-path primaries / indexes
    assert_shape("obj.key", "Path(Bare:obj.key)");
    assert_top_kind("a.b", |k| matches!(k, ExprKind::Path(_)));
    // index form always builds Lens
    assert_shape("obj[\"k\"]", "Lens(Path(Bare:obj), Atom(Str(k)))");
}

// ---------------------------------------------------------------------------
// Must-reject (silent accept would be a deformation of the other kind)
// ---------------------------------------------------------------------------

#[test]
fn golden_rejections() {
    assert!(parse_program("%@a: 1").is_err(), "%@a must reject");
    assert!(parse_program("`k${i}`: 1").is_err(), "interp key must reject");
    assert!(
        parse_program("k: a ? b : c ? d : e").is_err(),
        "chained ternary must reject"
    );
    assert!(
        parse_expr_only("{ #a < #b }").is_err(),
        "bare order chain in combo must reject"
    );
}

// ---------------------------------------------------------------------------
// Float print shape sanity (to_nlang must keep Float as Float)
// ---------------------------------------------------------------------------

#[test]
fn golden_float_canonical_keeps_decimal() {
    let e = parse_expr_only("1.0").unwrap();
    match &e.kind {
        ExprKind::Atom(AtomKind::Float(f)) => {
            let printed = AtomKind::Float(*f).to_string_canonical();
            assert!(
                printed.contains('.') || printed.contains('e') || printed.contains('E'),
                "float print lost decimal: {printed}"
            );
            let again = parse_expr_only(&printed).unwrap();
            assert!(
                matches!(again.kind, ExprKind::Atom(AtomKind::Float(_))),
                "reparse of {printed} not Float: {:?}",
                again.kind
            );
        }
        other => panic!("expected Float, got {other:?}"),
    }
}
