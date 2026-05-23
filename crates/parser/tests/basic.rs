use nlang_parser::parse_program;

#[test]
fn test_basic_data() {
    let input = r#"
        name: "Alice"
        age: 30
        status: #active
        @int: 0
    "#;
    let res = parse_program(input);
    assert!(res.is_ok(), "Failed to parse basic data: {:?}", res.err());
}

#[test]
fn test_complex_structures() {
    let input = r#"
        user: {
            id: 123
            profile: {
                bio: "Hello"
                tags: [#rust, #nlang]
            }
        }
        
        /add: x y -> x + y
        
        workflow: #draft | #review | #publish
        
        pipeline: data |> /process |> { result: $.val & @int }
    "#;
    let res = parse_program(input);
    assert!(res.is_ok(), "Failed to parse complex structures: {:?}", res.err());
}

#[test]
fn test_paths() {
    let input = r#"
        root: _.app.config
        rel: ^.local
        parent: ^^.global
    "#;
    let res = parse_program(input);
    assert!(res.is_ok(), "Failed to parse paths: {:?}", res.err());
}

#[test]
fn test_arithmetic_and_logic() {
    let input = r#"
        calc: 1 + 2 * 3
        check: x == y
        evens: @int & x % 2 == 0
    "#;
    let res = parse_program(input);
    assert!(res.is_ok(), "Failed to parse arithmetic and logic: {:?}", res.err());
}

#[test]
fn test_complex_numbers() {
    use nlang_parser::parse_expr_only;
    use nlang_parser::ast::{ExprKind, AtomKind, UnaryOp};
    
    // Direct complex numbers without leading negation
    let tests = vec![
        ("3+4i", 3.0, 4.0),
        ("2i", 0.0, 2.0),
        ("i", 0.0, 1.0),
        ("3-4i", 3.0, -4.0),
    ];
    
    for (input, expected_r, expected_i) in tests {
        let res = parse_expr_only(input);
        match res {
            Ok(expr) => {
                match expr.kind {
                    ExprKind::Atom(kind) => {
                        match kind {
                            AtomKind::Complex(r, i) => {
                                assert!((r - expected_r).abs() < 0.0001, "{}: Expected real {} but got {}", input, expected_r, r);
                                assert!((i - expected_i).abs() < 0.0001, "{}: Expected imag {} but got {}", input, expected_i, i);
                            },
                            other => panic!("{}: Expected Complex but got {:?}", input, other),
                        }
                    }
                    other => panic!("{}: Expected Atom but got {:?}", input, other),
                }
            }
            Err(e) => panic!("Failed to parse {}: {}", input, e),
        }
    }
    
    // -i and -3+4i are parsed as unary negation (eval will handle)
    let unary_tests = vec![
        ("-i", 0.0, 1.0),
        ("-3+4i", 3.0, 4.0),
    ];
    
    for (input, inner_r, inner_i) in unary_tests {
        let res = parse_expr_only(input);
        match res {
            Ok(expr) => {
                match expr.kind {
                    ExprKind::Unary { op, expr: inner } => {
                        assert!(matches!(op, UnaryOp::Neg), "{}: Expected Neg unary op", input);
                        match inner.kind {
                            ExprKind::Atom(AtomKind::Complex(r, i)) => {
                                assert!((r - inner_r).abs() < 0.0001, "{}: Expected inner real {} but got {}", input, inner_r, r);
                                assert!((i - inner_i).abs() < 0.0001, "{}: Expected inner imag {} but got {}", input, inner_i, i);
                            },
                            other => panic!("{}: Expected inner Complex but got {:?}", input, other),
                        }
                    },
                    other => panic!("{}: Expected Unary but got {:?}", input, other),
                }
            }
            Err(e) => panic!("Failed to parse {}: {}", input, e),
        }
    }
}
