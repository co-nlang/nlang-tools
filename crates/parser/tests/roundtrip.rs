//! parse → to_nlang → re-parse → span-free AST equality.
//!
//! Catches silent token drops and printer/parser skew (CAID stability premise).

use nlang_parser::ast::Expr;
use nlang_parser::{parse_expr_only, parse_program};

fn roundtrip_expr(src: &str) {
    let once = parse_expr_only(src).unwrap_or_else(|e| panic!("first parse {src:?}: {e}"));
    let printed = once.to_nlang(0);
    let twice = parse_expr_only(&printed)
        .unwrap_or_else(|e| panic!("re-parse of printed {printed:?} (from {src:?}): {e}"));
    let a = once.without_spans();
    let b = twice.without_spans();
    assert_eq!(
        a, b,
        "AST mismatch after roundtrip\n  src:     {src:?}\n  printed: {printed:?}\n  shape1:  {}\n  shape2:  {}",
        a.shape(),
        b.shape()
    );
}

fn roundtrip_program(src: &str) {
    let once = parse_program(src).unwrap_or_else(|e| panic!("first parse program: {e}\n{src}"));
    let printed = once.to_nlang();
    let twice = parse_program(&printed)
        .unwrap_or_else(|e| panic!("re-parse program printed:\n{printed}\nerr: {e}"));
    let a = once.without_spans();
    let b = twice.without_spans();
    assert_eq!(
        a, b,
        "Program AST mismatch\n--- printed ---\n{printed}\nshape1: {}\nshape2: {}",
        a.shape(),
        b.shape()
    );
}

#[test]
fn expr_roundtrip_corpus() {
    let cases = [
        // atoms
        "_",
        "_|_",
        "()",
        "42",
        "-7",
        "1.5",
        "1.0",
        "i",
        "-i",
        "3i",
        "2+3i",
        "-3+4i",
        "\"hi\"",
        "#tag",
        "#_|_",
        "#_",
        "r\"x+y\"",
        "p\"/a\"",
        "u\"http://z\"",
        "t\"2020-01-01\"",
        // paths
        "a",
        "a-b",
        "io",
        "a.b.c",
        "_.app.config",
        "^.local",
        "^^.x",
        // containers
        "[]",
        "[1, 2, 3]",
        "[...xs, 1]",
        "(1, 2)",
        "(1,)",
        "{}",
        "{{}}",
        "{ a: 1 }",
        // ops
        "a & b",
        "a | b",
        "a \\ b",
        "!x",
        "a + b",
        "a - b",
        "a * b",
        "a / b",
        "a % b",
        "a == b",
        "a != b",
        "a < b",
        "a <= b",
        "a > b",
        "a >= b",
        "a = b",
        "a <=> b",
        "a < b & c",
        "1 + 2 * 3",
        // higher forms
        "x |> y",
        "x -> x",
        "a ? b : c",
        "<<x>>",
        "$",
        "1..10",
        "0..10..2",
        "-5..5",
        "#{ #a <= #b < #c }",
        "@{ x }",
        "@{}",
        "f (-1)",
        "a /f b",
        "`hi ${x}`",
        "obj.key",
        "obj[\"k\"]",
    ];
    for src in cases {
        roundtrip_expr(src);
    }
}

#[test]
fn program_roundtrip_corpus() {
    roundtrip_program(
        r#"
name: "Alice"
age: 30
@int: 0
/add: x y -> x + y
~%sys: 1
root: _.app
rel: ^.local
list: [1, 2, ...xs]
tup: (1, 2)
"#,
    );
}

/// Double roundtrip: printed form is a fixed point (idempotent printer).
#[test]
fn expr_printer_idempotent() {
    let cases = ["2+3i", "[...xs, 1]", "^.x", "a < b & c", "x |> /f |> /g"];
    for src in cases {
        let e1 = parse_expr_only(src).unwrap();
        let p1 = e1.to_nlang(0);
        let e2 = parse_expr_only(&p1).unwrap();
        let p2 = e2.to_nlang(0);
        assert_eq!(p1, p2, "printer not idempotent for {src:?}: {p1:?} vs {p2:?}");
        assert_eq!(e1.without_spans(), e2.without_spans());
    }
}

/// Helper used by fuzz: structural equality after one print cycle.
#[allow(dead_code)]
pub fn assert_expr_roundtrip(e: &Expr) {
    let printed = e.to_nlang(0);
    let again = parse_expr_only(&printed)
        .unwrap_or_else(|err| panic!("re-parse {printed:?}: {err}"));
    assert_eq!(
        e.without_spans(),
        again.without_spans(),
        "roundtrip fail printed={printed:?}\n  {}\n  {}",
        e.shape(),
        again.shape()
    );
}
