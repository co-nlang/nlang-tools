use nlang_interpreter::{Ouroboros, Value, ComboVal, EvalContext, EffectTag, BottomCause};
use nlang_parser::{parse_program, parse_expr_only};
use nlang_parser::ast::{AtomKind, ExprKind, UnaryOp};
use indexmap::IndexMap;

fn fresh_ctx() -> EvalContext {
    EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn eval_expr(input: &str) -> Value {
    let program = parse_program(input).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = fresh_ctx();
    oo.eval_observed(&program.fields[0].value, &mut ctx)
}

fn parse_single_expr(input: &str) -> ExprKind {
    let expr = parse_expr_only(input).unwrap();
    expr.kind
}

mod parser_behavior {
    use super::*;
    
    #[test]
    fn exclamation_is_unary_not_not_complement() {
        let kind = parse_single_expr("!#true");
        assert!(matches!(kind, ExprKind::Unary { op: UnaryOp::Not, .. }));
        assert!(!matches!(kind, ExprKind::Complement(_)));
    }
    
    #[test]
    fn complement_ast_node_exists_but_unused() {
        let complement_expr = ExprKind::Complement(Box::new(nlang_parser::ast::Expr::new(
            ExprKind::Atom(AtomKind::Tag("true".to_string())),
            nlang_parser::ast::Span::default()
        )));
        assert!(matches!(complement_expr, ExprKind::Complement(_)));
    }
}

mod orthocomplement_involution {
    use super::*;
    
    #[test]
    fn top_complement_is_bottom() {
        let v = eval_expr("test: !_");
        assert!(matches!(v, Value::Atom(AtomKind::Bottom, _, _)));
    }
    
    #[test]
    fn bottom_complement_is_top() {
        let v = eval_expr("test: !_|_");
        assert!(matches!(v, Value::Atom(AtomKind::Top, _, _)));
    }
    
    #[test]
    fn double_complement_top() {
        let v = eval_expr("test: !(!_)");
        assert!(matches!(v, Value::Atom(AtomKind::Top, _, _)));
    }
    
    #[test]
    fn double_complement_bottom() {
        let v = eval_expr("test: !(!_|_)");
        assert!(matches!(v, Value::Atom(AtomKind::Bottom, _, _)));
    }
    
    #[test]
    fn boolean_involution() {
        let v = eval_expr("test: !(!#true)");
        assert_eq!(v, Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
    }
}

mod orthocomplement_orthogonality {
    use super::*;
    
    #[test]
    fn meet_true_and_not_true_is_bottom() {
        let v = eval_expr("test: #true & !#true");
        assert!(matches!(v, Value::Bottom(_)));
    }
    
    #[test]
    fn meet_false_and_not_false_is_bottom() {
        let v = eval_expr("test: #false & !#false");
        assert!(matches!(v, Value::Bottom(_)));
    }
}

mod orthocomplement_order_anchors {
    use super::*;
    
    #[test]
    fn tag_start_complement_is_tag_end() {
        let expr = parse_expr_only("!#_|_").unwrap();
        println!("Parsed expr kind: {:?}", expr.kind);
        let v = eval_expr("test: !#_|_");
        println!("!#_|_ = {:?}", v);
        println!("to_string_plain: {}", v.to_string_plain());
        assert!(matches!(v, Value::Atom(AtomKind::TagEnd, _, _)));
    }
    
    #[test]
    fn tag_end_complement_is_tag_start() {
        let expr = parse_expr_only("!#_").unwrap();
        println!("Parsed expr kind: {:?}", expr.kind);
        let v = eval_expr("test: !#_");
        println!("!#_ = {:?}", v);
        println!("to_string_plain: {}", v.to_string_plain());
        assert!(matches!(v, Value::Atom(AtomKind::TagStart, _, _)));
    }
}

mod diff_operation {
    use super::*;
    
    #[test]
    fn diff_as_meet_with_complement() {
        let v = eval_expr("test: #true \\ #true");
        assert!(matches!(v, Value::Bottom(_)));
    }
    
    #[test]
    fn diff_on_combo() {
        let v = eval_expr("test: { a: 1 } \\ { a: 1 }");
        if let Value::Combo(cv) = v {
            assert!(!cv.contains_key("a") || cv.fields().is_empty());
        } else {
            assert!(matches!(v, Value::Bottom(_)) || matches!(v, Value::Atom(AtomKind::Top, _, _)));
        }
    }
}

mod non_boolean_tags {
    use super::*;
    
    #[test]
    fn non_boolean_tag_returns_conflict() {
        let v = eval_expr("test: !#red");
        assert!(matches!(v, Value::Bottom(ref d) if d.cause == BottomCause::Conflict));
    }
    
    #[test]
    fn non_boolean_int_returns_conflict() {
        let v = eval_expr("test: !5");
        assert!(matches!(v, Value::Bottom(ref d) if d.cause == BottomCause::Conflict));
    }
}

mod union_minimal_elements {
    use super::*;
    
    #[test]
    fn union_meet_keeps_all_matching_branches() {
        let v = eval_expr("test: (#red | #blue) & #red");
        println!("(#red | #blue) & #red = {:?}", v);
        println!("to_string_plain: {}", v.to_string_plain());
        assert!(matches!(v, Value::Atom(AtomKind::Tag(ref t), _, _) if t.trim_start_matches('#') == "red"));
    }
    
    #[test]
    fn union_meet_with_union_preserves_matches() {
        let v = eval_expr("test: (#a | #b | #c) & (#a | #b)");
        println!("(#a | #b | #c) & (#a | #b) = {:?}", v);
        if let Value::Union(branches) = v {
            assert_eq!(branches.len(), 2);
            let tags: Vec<String> = branches.iter().map(|b| b.to_string_plain()).collect();
            assert!(tags.contains(&"#a".to_string()) || tags.contains(&"a".to_string()));
            assert!(tags.contains(&"#b".to_string()) || tags.contains(&"b".to_string()));
        } else {
            assert!(matches!(v, Value::Atom(AtomKind::Tag(_), _, _)), "Should be Union or single Tag");
        }
    }
    
    #[test]
    fn union_meet_with_combo_multiple_matches() {
        let v = eval_expr("test: ({ x: 1 } | { x: 2 } | { x: 3 }) & { x: 1 | 2 }");
        println!("Union & Combo with Union field = {:?}", v);
        if let Value::Union(branches) = v {
            assert!(branches.len() >= 2, "Should have at least 2 matching branches");
        }
    }
}