use nlang_interpreter::{Ouroboros, Value, EvalContext, ComboVal, EffectTag};
use nlang_parser::parse_program;
use nlang_parser::ast::{AtomKind, FieldKey, PathAnchor};
use indexmap::IndexMap;
use num_bigint::BigInt;

/// 從 FieldKey 取得欄位名稱（處理 Named 和 Path 兩種情況）
fn field_name(key: &FieldKey) -> String {
    match key {
        FieldKey::Named { name, .. } => name.clone(),
        FieldKey::Path(p) if p.anchor == PathAnchor::Bare && p.segments.len() == 1 => {
            p.segments[0].clone()
        },
        FieldKey::Path(p) if p.anchor == PathAnchor::Bare => {
            p.segments.join(".")
        },
        FieldKey::Quoted(s) => s.clone(),
        _ => panic!("Unexpected field key: {:?}", key),
    }
}

#[test]
#[ignore = "Known Issue: Sibling resolution in combos"]
fn test_lexical_scoping_shadowing() {
    let input = "a: 1\ninner: {\na: 2\nb: a\n}\noutside_b: inner.b";
    let program = parse_program(input).unwrap();
    let oo = Ouroboros::new_in_memory();
    
    let root_val = ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]);
    let mut ctx = EvalContext::new(root_val);

    let a_val = oo.eval_observed(&program.fields[0].value, &mut ctx);
    std::sync::Arc::make_mut(&mut ctx.root).insert_field("a", a_val);

    let inner_val = oo.eval_observed(&program.fields[1].value, &mut ctx);
    std::sync::Arc::make_mut(&mut ctx.root).insert_field("inner", inner_val.clone());

    if let Value::Combo(cv) = inner_val {
        assert_eq!(cv.get_field("b").unwrap(), &Value::Atom(AtomKind::Int(BigInt::from(2)), EffectTag::Pure, None));
    } else {
        panic!("Expected Combo");
    }

    let outside_b = oo.eval_observed(&program.fields[2].value, &mut ctx);
    assert_eq!(outside_b, Value::Atom(AtomKind::Int(BigInt::from(2)), EffectTag::Pure, None));
}

#[test]
#[ignore = "Known Defect: Absolute path resolution in isolated evaluation context"]
fn test_absolute_path() {
    let input = "a: 1\ninner: {\na: 2\nroot_a: _.a\n}";
    let program = parse_program(input).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]));

    for f in &program.fields {
        let name = field_name(&f.key);
        let val = oo.eval_observed(&f.value, &mut ctx);
        std::sync::Arc::make_mut(&mut ctx.root).insert_field(&name, val);
    }

    let inner = ctx.root.get_field("inner").unwrap();
    if let Value::Combo(cv) = inner {
        assert_eq!(cv.get_field("root_a").unwrap(), &Value::Atom(AtomKind::Int(BigInt::from(1)), EffectTag::Pure, None));
    }
}

#[test]
fn test_deep_navigation() {
    let input = "config: {\nnetwork: {\nport: 8080\n}\n}\napp_port: config.network.port";
    let program = parse_program(input).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]));

    for f in &program.fields {
        let name = field_name(&f.key);
        let val = oo.eval_observed(&f.value, &mut ctx);
        std::sync::Arc::make_mut(&mut ctx.root).insert_field(&name, val);
    }

    assert_eq!(ctx.root.get_field("app_port").unwrap(), &Value::Atom(AtomKind::Int(BigInt::from(8080)), EffectTag::Pure, None));
}
