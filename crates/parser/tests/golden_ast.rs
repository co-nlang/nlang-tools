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
// SYNTAX_06 — comparison / subtyping precedence
// ---------------------------------------------------------------------------

#[test]
fn golden_ops_and_precedence() {
    // meet tighter than cmp (SPEC_14 / ENGINE_SYNC #1 / SYNTAX_06 §4.8)
    assert_shape("a < b & c", "Lt(Path(Bare:a), Meet(Path(Bare:b), Path(Bare:c)))");
    assert_shape("a & b = a", "LatticeEq(Meet(Path(Bare:a), Path(Bare:b)), Path(Bare:a))");
    // set ops looser than cmp — bare form is almost never intended (SYNTAX_06 §4.9)
    assert_shape(
        "a \\ b = c",
        "Diff(Path(Bare:a), LatticeEq(Path(Bare:b), Path(Bare:c)))",
    );
    assert_shape(
        "(a \\ b) = c",
        "LatticeEq(Diff(Path(Bare:a), Path(Bare:b)), Path(Bare:c))",
    );
    assert_shape("a = b", "LatticeEq(Path(Bare:a), Path(Bare:b))");
    assert_shape("a <=> b", "Probe(Path(Bare:a), Path(Bare:b))");
    assert_shape("a == b", "Eq(Path(Bare:a), Path(Bare:b))");
    assert_shape("a != b", "Ne(Path(Bare:a), Path(Bare:b))");
    assert_shape("a | b", "Join(Path(Bare:a), Path(Bare:b))");
    assert_shape("a \\ b", "Diff(Path(Bare:a), Path(Bare:b))");
    assert_shape("a >= b", "Gte(Path(Bare:a), Path(Bare:b))");
    assert_shape("3 <= 5", "Lte(Atom(Int(3)), Atom(Int(5)))");
    // grammar: unary_op = "!" → Unary(Not, …); Complement is a semantic alias
    assert_shape("!x", "Unary(Not, Path(Bare:x))");
}

// ---------------------------------------------------------------------------
// SYNTAX_07 — observation duality (structural brackets)
// ---------------------------------------------------------------------------

#[test]
fn golden_structural() {
    assert_shape("<<x>>", "Structural(Path(Bare:x))");
    assert_shape("<<a & b>>", "Structural(Meet(Path(Bare:a), Path(Bare:b)))");
    // postfix after >> is ordinary Lens, not "structural field" (SYNTAX_07 §4.6)
    assert_shape(
        "<<x>>.foo",
        "Lens(Structural(Path(Bare:x)), Atom(Str(foo)))",
    );
    assert_shape(
        "<<a>>.%cause",
        "Lens(Structural(Path(Bare:a)), Atom(Str(%cause)))",
    );
    // whole-path structuralization
    assert_shape("<<a.%cause>>", "Structural(Path(Bare:a.%cause))");
    assert_shape("<<x>> <= x", "Lte(Structural(Path(Bare:x)), Path(Bare:x))");
}

// ---------------------------------------------------------------------------
// SYNTAX_08 / paths — metadata access
// ---------------------------------------------------------------------------

#[test]
fn golden_metadata_paths() {
    // %len as path segment (meta key), not Rem
    assert_shape("a.%len", "Path(Bare:a.%len)");
    assert_shape("a % b", "Rem(Path(Bare:a), Path(Bare:b))");
    assert_shape("$.val", "Lens(Context, Atom(Str(val)))");
}

// ---------------------------------------------------------------------------
// SYNTAX_09 — morphism application
// ---------------------------------------------------------------------------

#[test]
fn golden_application() {
    // apply tighter than meet (SYNTAX_09 §4.3)
    assert_shape(
        "/func a & b",
        "Meet(Apply(Path(Bare:/func), Path(Bare:a)), Path(Bare:b))",
    );
    assert_shape(
        "/func (a & b)",
        "Apply(Path(Bare:/func), Meet(Path(Bare:a), Path(Bare:b)))",
    );
    // whitespace optional for apply (SYNTAX_09 §4.1)
    assert_shape("/func ()", "Apply(Path(Bare:/func), Atom(Unit))");
    assert_shape("/func()", "Apply(Path(Bare:/func), Atom(Unit))");
    // binary `-` wins over apply; negative args need parens (SYNTAX_09 §4.9)
    assert_shape("f -1", "Sub(Path(Bare:f), Atom(Int(1)))");
    assert_shape("f (-1)", "Apply(Path(Bare:f), Atom(Int(-1)))");
    // morphism-as-arg needs parens so logic_infix does not steal
    assert_shape("f (/g)", "Apply(Path(Bare:f), Path(Bare:/g))");
    // three-way `/` (SYNTAX_09 §4.7)
    assert_shape(
        "a /f b",
        "Apply(Apply(Path(Bare:/f), Path(Bare:a)), Path(Bare:b))",
    );
    assert_shape("a / b", "Div(Path(Bare:a), Path(Bare:b))");
}

// ---------------------------------------------------------------------------
// SYNTAX_10 — enum / poset
// ---------------------------------------------------------------------------

#[test]
fn golden_poset() {
    assert_shape("#{}", "Poset([])");
    assert_shape(
        "#{ #draft <= #review < #publish }",
        "Poset([Tag(draft)<=Tag(review), Tag(review)<Tag(publish)])",
    );
    assert_shape("#{ #a = #b }", "Poset([Tag(a)=Tag(b)])");
    // mixed-direction chain (SYNTAX_10 §4.2)
    assert_shape(
        "#{ #a < #c > #b }",
        "Poset([Tag(a)<Tag(c), Tag(c)>Tag(b)])",
    );
    // enum member as path segment
    assert_shape("Status.#draft", "Path(Bare:Status.#draft)");
}

// ---------------------------------------------------------------------------
// SYNTAX_11 — morphism definition
// ---------------------------------------------------------------------------

#[test]
fn golden_morphism_definition() {
    // bare join of arrows nests wrong (SYNTAX_11 §4.1) — pin the bad shape
    assert_shape(
        "A -> B | C -> D",
        "Morphism(Path(Bare:A), Morphism(Join(Path(Bare:B), Path(Bare:C)), Path(Bare:D)))",
    );
    // canonical: parenthesize each branch
    assert_shape(
        "(A -> B) | (C -> D)",
        "Join(Morphism(Path(Bare:A), Path(Bare:B)), Morphism(Path(Bare:C), Path(Bare:D)))",
    );
    // pipe tighter than arrow (SYNTAX_11 §4.2)
    assert_shape(
        "x -> x |> /g",
        "Morphism(Path(Bare:x), Pipe(Path(Bare:x), Path(Bare:/g)))",
    );
    assert_shape(
        "data |> (x -> x + 1)",
        "Pipe(Path(Bare:data), Morphism(Path(Bare:x), Add(Path(Bare:x), Atom(Int(1)))))",
    );
    // G2-M: multi-param sugar desugars to nested curry at AST build
    // (was Morphism(Apply(x,y), body); SYNTAX_11 auto-curry)
    assert_shape(
        "x y -> x",
        "Morphism(Path(Bare:x), Morphism(Path(Bare:y), Path(Bare:x)))",
    );
    assert_shape(
        "(x, y) -> x",
        "Morphism(Tuple([Path(Bare:x), Path(Bare:y)]), Path(Bare:x))",
    );
    // type annotation on param (op is @, not :)
    assert_shape(
        "x @int -> x",
        "Morphism(TypeAnnotation(Path(Bare:x), Path(Bare:int)), Path(Bare:x))",
    );
    assert_shape(
        "x@int -> x",
        "Morphism(TypeAnnotation(Path(Bare:x), Path(Bare:int)), Path(Bare:x))",
    );
    assert_shape(
        "x @ int",
        "TypeAnnotation(Path(Bare:x), Path(Bare:int))",
    );
}

// ---------------------------------------------------------------------------
// SYNTAX_12 — pipe / ternary / context
// ---------------------------------------------------------------------------

#[test]
fn golden_pipe_ternary_context() {
    assert_shape("x |> /f", "Pipe(Path(Bare:x), Path(Bare:/f))");
    assert_shape("x -> x", "Morphism(Path(Bare:x), Path(Bare:x))");
    assert_shape("$", "Context");
    assert_shape(
        "a ? b : c",
        "Ternary(Path(Bare:a), Path(Bare:b), Path(Bare:c))",
    );
    assert_shape(
        "a ? b : (c ? d : e)",
        "Ternary(Path(Bare:a), Path(Bare:b), Ternary(Path(Bare:c), Path(Bare:d), Path(Bare:e)))",
    );
    // | tighter than |> (SYNTAX_12 §4.3)
    assert_shape(
        "1 | 2 |> /f",
        "Pipe(Join(Atom(Int(1)), Atom(Int(2))), Path(Bare:/f))",
    );
    // open ranges: omitted bounds = order anchors (SPEC_02 §3), not Top
    assert_shape("..", "Range(Atom(TagStart), Atom(TagEnd))");
    assert_shape("..10", "Range(Atom(TagStart), Atom(Int(10)))");
    assert_shape("1..", "Range(Atom(Int(1)), Atom(TagEnd))");
    assert_shape(
        "-5..5..1",
        "Range(Atom(Int(-5)), Atom(Int(5)), Atom(Int(1)))",
    );
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
    // single tag is not a poset chain (SYNTAX_10 §4.2)
    assert!(
        parse_expr_only("#{ #a }").is_err(),
        "singleton tag in poset must reject"
    );
    // non-associative cmp — trailing second probe must not be silently dropped
    // (fixed: parse_expr_only now uses expr_toplevel with EOI)
    assert!(
        parse_expr_only("a <=> b <=> c").is_err(),
        "chained <=> must reject (was silent partial parse)"
    );
    assert!(
        parse_expr_only("a < b < c").is_err(),
        "chained < must reject"
    );
    // trailing junk after a complete expr must fail
    assert!(
        parse_expr_only("x: leftover").is_err(),
        "trailing after expr must reject"
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
