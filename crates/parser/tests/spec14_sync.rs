// ENGINE_SYNC regression tests: n.pest vs SPEC_14 (2026-07-05 finalization pass)
use nlang_parser::{parse_expr_only, parse_program};
use nlang_parser::ast::{ExprKind, AtomKind, RelOp};

// #1: & (L10) binds tighter than cmp (L11) — anti-C declaration (SPEC_14 §2.3)
#[test]
fn meet_tighter_than_cmp() {
    let e = parse_expr_only("a < b & c").unwrap();
    match e.kind {
        ExprKind::Lt(_, r) => assert!(matches!(r.kind, ExprKind::Meet(_, _)), "rhs should be (b & c), got {:?}", r.kind),
        other => panic!("expected Lt at top, got {:?}", other),
    }
}

// #2: cmp_op gains `=` (lattice equality) and `<=>` (direction probe)
#[test]
fn lattice_eq_and_probe() {
    assert!(matches!(parse_expr_only("a = b").unwrap().kind, ExprKind::LatticeEq(_, _)));
    assert!(matches!(parse_expr_only("a <=> b").unwrap().kind, ExprKind::Probe(_, _)));
    // == stays the atomic family
    assert!(matches!(parse_expr_only("a == b").unwrap().kind, ExprKind::Eq(_, _)));
}

// #3: structural brackets are double angles
#[test]
fn structural_double_angle() {
    assert!(matches!(parse_expr_only("<<x>>").unwrap().kind, ExprKind::Structural(_)));
    assert!(matches!(parse_expr_only("<<_.>> |> <<_.>>").unwrap().kind, ExprKind::Pipe(_, _)));
}

// #4: structured prefixes — legal stacks parse as keys, illegal stacks don't
#[test]
fn structured_prefixes() {
    assert!(parse_program("~@Schema: 1\n~/helper: x -> x\n~%sys: 1\n%meta: 1").is_ok());
    // %@a: free stacking now rejected — `%@a: 1` must not parse as a single named key
    let r = parse_program("%@a: 1");
    assert!(r.is_err(), "%@a should be rejected as a field key");
}

// #5: tuple — comma-delimited; (x) stays grouping; (x,) is a 1-tuple
#[test]
fn tuples() {
    match parse_expr_only("(1, 2)").unwrap().kind {
        ExprKind::Tuple(items) => assert_eq!(items.len(), 2),
        other => panic!("expected Tuple, got {:?}", other),
    }
    match parse_expr_only("(1,)").unwrap().kind {
        ExprKind::Tuple(items) => assert_eq!(items.len(), 1),
        other => panic!("expected 1-Tuple, got {:?}", other),
    }
    assert!(matches!(parse_expr_only("(1)").unwrap().kind, ExprKind::Atom(AtomKind::Int(_))));
    // list accepts ; separator and trailing separator
    assert!(matches!(parse_expr_only("[1; 2; 3,]").unwrap().kind, ExprKind::List(_)));
}

// #6: infix logic L7 — `a /f b` desugars to /f applied to both operands
#[test]
fn infix_logic() {
    match parse_expr_only("a /f b").unwrap().kind {
        ExprKind::Apply(inner, _) => match &inner.kind {
            ExprKind::Apply(f, _) => match &f.kind {
                ExprKind::Path(p) => assert_eq!(p.segments, vec!["/f".to_string()]),
                other => panic!("expected /f path, got {:?}", other),
            },
            other => panic!("expected inner Apply, got {:?}", other),
        },
        other => panic!("expected Apply, got {:?}", other),
    }
    // `a / b` (spaced) stays division
    assert!(matches!(parse_expr_only("a / b").unwrap().kind, ExprKind::Div(_, _)));
}

// #7: ternary branches are pipe-level — bare chained ternary is illegal
#[test]
fn ternary_branches() {
    assert!(parse_expr_only("a ? b : (c ? d : e)").is_ok());
    // parse_expr_only tolerates trailing input, so test rejection at program level
    let r = parse_program("k: a ? b : c ? d : e");
    assert!(r.is_err(), "bare chained ternary should be rejected (SYNTAX_12 §4.1)");
}

// #9: multiline string escape for """
#[test]
fn multiline_escape() {
    assert!(parse_program("s: \"\"\"a \\\"\"\" b\"\"\"").is_ok());
}

// #10: interpolated strings are not field keys (tag keys are)
#[test]
fn field_key_rules() {
    assert!(parse_program("`k${i}`: 1").is_err(), "interp key must be rejected");
    assert!(parse_program("#adult: \"Adult\"").is_ok(), "tag key is canonical");
}

// #12/#13: poset literal #{} with order chains; = allowed in chains
#[test]
fn poset_literal() {
    match parse_expr_only("#{ #draft <= #review < #publish }").unwrap().kind {
        ExprKind::Poset(rels) => {
            assert_eq!(rels.len(), 2);
            assert_eq!(rels[0].op, RelOp::Lte);
            assert_eq!(rels[1].op, RelOp::Lt);
        }
        other => panic!("expected Poset, got {:?}", other),
    }
    match parse_expr_only("#{ #a = #b }").unwrap().kind {
        ExprKind::Poset(rels) => assert_eq!(rels[0].op, RelOp::Eq),
        other => panic!("expected Poset, got {:?}", other),
    }
    // multi-chain with anchors
    assert!(parse_expr_only("#{ #_|_ < #init, #a < #c, #b < #c }").is_ok());
    // bare order chain in a combo is no longer legal
    assert!(parse_expr_only("{ #a < #b }").is_err(), "bare order chain in {{}} must be rejected");
}

// pinned edge cases from the SYNTAX finalization (ENGINE_SYNC test vector list)
#[test]
fn pinned_edge_cases() {
    // a-1 is ONE kebab-case ident; a -1 is subtraction; f (-1) is application
    assert!(matches!(parse_expr_only("a-1").unwrap().kind, ExprKind::Path(_)));
    assert!(matches!(parse_expr_only("a -1").unwrap().kind, ExprKind::Sub(_, _)));
    assert!(matches!(parse_expr_only("f (-1)").unwrap().kind, ExprKind::Apply(_, _)));
    // 2+3i atomic vs spaced addition
    assert!(matches!(parse_expr_only("2+3i").unwrap().kind, ExprKind::Atom(AtomKind::Complex(_, _))));
    assert!(matches!(parse_expr_only("2 + 3i").unwrap().kind, ExprKind::Add(_, _)));
}

// deep nesting must not blow the stack (test_enum_auto_number.n shape, regression)
#[test]
fn deep_nesting_regression() {
    let src = std::fs::read_to_string("../../tests/unit/test_enum_auto_number.n").unwrap();
    assert!(parse_program(&src).is_ok());
}
