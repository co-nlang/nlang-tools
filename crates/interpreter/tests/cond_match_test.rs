use indexmap::IndexMap;
use nlang_interpreter::value::{ComboVal, EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;

fn make_atom_tag(t: &str) -> Value {
    Value::Atom(AtomKind::Tag(t.to_string()), EffectTag::Pure, None)
}

fn make_atom_int(n: i64) -> Value {
    Value::Atom(AtomKind::Int(n.into()), EffectTag::Pure, None)
}

fn make_list(items: Vec<Value>) -> Value {
    let mut fields = IndexMap::new();
    for (i, v) in items.into_iter().enumerate() {
        fields.insert(i.to_string(), v);
    }
    fields.insert("%kind".to_string(), make_atom_tag("list"));
    Value::Combo(ComboVal::new(
        fields,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

fn make_pair(pat: Value, action: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), pat);
    f.insert("1".to_string(), action);
    Value::Combo(ComboVal::new(
        f,
        true,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

fn make_match_arg(value: Value, patterns: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), value);
    f.insert("1".to_string(), patterns);
    Value::Combo(ComboVal::new(
        f,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

fn get_match_morph(oo: &Ouroboros) -> Value {
    let sys = oo.root_with_system();
    sys.get_field("~%Cond")
        .and_then(|v| {
            if let Value::Combo(ref c) = v {
                c.get_field("/match").cloned()
            } else {
                None
            }
        })
        .expect("/match in ~%Cond")
}

#[test]
fn match_first_pattern_wins() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system()).with_standard_root(oo.root_with_system());
    let value = make_atom_tag("#foo");
    let pat1 = make_atom_tag("#foo");
    let pat2 = make_atom_tag("#bar");
    let patterns = make_list(vec![
        make_pair(pat1, Value::Top),
        make_pair(pat2, Value::Top),
    ]);
    let arg = make_match_arg(value, patterns);
    let match_morph = get_match_morph(&oo);
    let result = oo.force(oo.apply_morphism(match_morph, arg, &mut ctx), &mut ctx);
    assert_eq!(
        result.collapse().to_string_plain().trim_start_matches('#'),
        "foo",
        "First matching pattern should win: {:?}",
        result
    );
}

#[test]
fn match_skips_non_matching() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system()).with_standard_root(oo.root_with_system());
    let value = make_atom_tag("#baz");
    let patterns = make_list(vec![
        make_pair(make_atom_tag("#foo"), Value::Top),
        make_pair(make_atom_tag("#baz"), Value::Top),
    ]);
    let arg = make_match_arg(value, patterns);
    let match_morph = get_match_morph(&oo);
    let result = oo.force(oo.apply_morphism(match_morph, arg, &mut ctx), &mut ctx);
    assert_eq!(
        result.collapse().to_string_plain().trim_start_matches('#'),
        "baz",
        "Should skip #foo and match #baz: {:?}",
        result
    );
}

#[test]
fn match_no_pattern_returns_not_bottom() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system()).with_standard_root(oo.root_with_system());
    let value = make_atom_tag("#qux");
    let patterns = make_list(vec![make_pair(make_atom_tag("#foo"), Value::Top)]);
    let arg = make_match_arg(value, patterns);
    let match_morph = get_match_morph(&oo);
    let result = oo.force(oo.apply_morphism(match_morph, arg, &mut ctx), &mut ctx);
    // No pattern matches; result should not be Bottom (match returns non-failure)
    assert!(
        !matches!(result.collapse(), Value::Bottom(_)),
        "No match should not be Bottom: {:?}",
        result
    );
    // The result should not equal the test value either (pattern didn't match)
    assert_ne!(
        result.collapse().to_string_plain(),
        "#qux",
        "Unmatched value should not appear as result"
    );
}

#[test]
fn match_top_pattern_catches_all() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system()).with_standard_root(oo.root_with_system());
    let value = make_atom_int(42);
    let patterns = make_list(vec![make_pair(Value::Top, Value::Top)]);
    let arg = make_match_arg(value.clone(), patterns);
    let match_morph = get_match_morph(&oo);
    let result = oo.force(oo.apply_morphism(match_morph, arg, &mut ctx), &mut ctx);
    if let Value::Atom(AtomKind::Int(n), _, _) = result.collapse() {
        assert_eq!(
            n.to_string(),
            "42",
            "Top pattern should match 42: {:?}",
            result
        );
    } else {
        panic!("Expected Int(42), got {:?}", result);
    }
}
