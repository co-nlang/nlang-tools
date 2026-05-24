use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, ComboVal, EffectTag};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;

fn make_int(n: i64) -> Value {
    Value::Atom(AtomKind::Int(n.into()), EffectTag::Pure, None)
}

fn make_some(val: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("%val".to_string(), val);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_none() -> Value {
    Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None)
}

fn make_ok(val: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("%val".to_string(), val);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_err(cause: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("%cause".to_string(), cause);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn get_morph_builtin(name: &str) -> Value {
    let mut f = IndexMap::new();
    f.insert("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
    f.insert("%builtin".to_string(), Value::Atom(AtomKind::Str(name.to_string()), EffectTag::Pure, None));
    Value::Combo(ComboVal::new(f, true, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_map_arg(f: Value, val: Value) -> Value {
    let mut fields = IndexMap::new();
    fields.insert("0".to_string(), f);
    fields.insert("1".to_string(), val);
    Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

#[test]
fn option_map_some() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let opt = make_some(make_int(42));
    let morph = get_morph_builtin("option.map");
    let arg = make_map_arg(Value::Top, opt);
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    // The result is a Combo with %val in meta (not collapsed)
    if let Value::Combo(ref c) = result {
        assert!(c.get_field("%val").is_some(), "option.map(Some(42)) should return Some: {:?}", result);
    } else {
        panic!("Expected Combo with %val, got {:?}", result);
    }
}

#[test]
fn option_map_none() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let opt = make_none();
    let morph = get_morph_builtin("option.map");
    let arg = make_map_arg(Value::Top, opt);
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    if let Value::Atom(AtomKind::Tag(t), _, _) = result.collapse() {
        assert_eq!(t.trim_start_matches('#'), "none",
            "option.map(#none) should return #none: {:?}", result);
    } else {
        panic!("Expected #none tag, got {:?}", result);
    }
}

#[test]
fn result_map_ok() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let res = make_ok(make_int(99));
    let morph = get_morph_builtin("result.map");
    let arg = make_map_arg(Value::Top, res);
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    if let Value::Combo(ref c) = result {
        assert!(c.get_field("%val").is_some(), "result.map(Ok(99)) should return Ok: {:?}", result);
        assert!(c.get_field("%cause").is_none(), "result.map(Ok) should not have %cause");
    } else {
        panic!("Expected Ok combo, got {:?}", result);
    }
}

#[test]
fn result_map_err_passthrough() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let err_cause = Value::Atom(AtomKind::Tag("oops".to_string()), EffectTag::Pure, None);
    let res = make_err(err_cause);
    let morph = get_morph_builtin("result.map");
    let arg = make_map_arg(Value::Top, res);
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    if let Value::Combo(ref c) = result {
        assert!(c.get_field("%cause").is_some(), "result.map(Err) should preserve %cause: {:?}", result);
        assert!(c.get_field("%val").is_none(), "result.map(Err) should not have %val");
    } else {
        panic!("Expected Err combo, got {:?}", result);
    }
}

#[test]
fn result_map_err_maps_cause() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let err_cause = Value::Atom(AtomKind::Tag("oops".to_string()), EffectTag::Pure, None);
    let res = make_err(err_cause);
    let morph = get_morph_builtin("result.map_err");
    let arg = make_map_arg(Value::Top, res);
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    if let Value::Combo(ref c) = result {
        assert!(c.get_field("%cause").is_some(), "result.map_err(Err) should have %cause: {:?}", result);
    } else {
        panic!("Expected Err combo, got {:?}", result);
    }
}

#[test]
fn option_fmap_accessible_from_type() {
    let oo = Ouroboros::new_in_memory();
    let root = oo.root_with_system();
    let opt_type = root.get_field("@option").expect("@option should exist");
    if let Value::Combo(ref c) = opt_type {
        assert!(c.get_field("%fmap").is_some(),
            "@option should have %fmap field after Phase 15");
    } else {
        panic!("@option should be a Combo");
    }
}

// ── Phase 16: Monad bind (and_then) tests ──

#[test]
fn option_and_then_some_chains() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let opt = make_some(make_int(42));
    let morph = get_morph_builtin("option.and_then");
    let arg = make_map_arg(Value::Top, opt);
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    if let Value::Atom(AtomKind::Int(n), _, _) = result.collapse() {
        assert_eq!(n.to_string(), "42", "and_then Some should chain: {:?}", result);
    } else {
        panic!("Expected Int(42), got {:?}", result);
    }
}

#[test]
fn option_and_then_none_propagates() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let opt = make_none();
    let morph = get_morph_builtin("option.and_then");
    let arg = make_map_arg(Value::Top, opt);
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    if let Value::Atom(AtomKind::Tag(t), _, _) = result.collapse() {
        assert_eq!(t.trim_start_matches('#'), "none",
            "and_then None should propagate #none: {:?}", result);
    } else {
        panic!("Expected #none, got {:?}", result);
    }
}

#[test]
fn result_and_then_ok_chains() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let res = make_ok(make_int(99));
    let morph = get_morph_builtin("result.and_then");
    let arg = make_map_arg(Value::Top, res);
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    if let Value::Atom(AtomKind::Int(n), _, _) = result.collapse() {
        assert_eq!(n.to_string(), "99", "result.and_then Ok should chain: {:?}", result);
    } else {
        panic!("Expected Int(99), got {:?}", result);
    }
}

#[test]
fn result_and_then_err_propagates() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let err_cause = Value::Atom(AtomKind::Tag("fail".to_string()), EffectTag::Pure, None);
    let res = make_err(err_cause);
    let morph = get_morph_builtin("result.and_then");
    let arg = make_map_arg(Value::Top, res);
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    if let Value::Combo(ref c) = result.collapse() {
        assert!(c.get_field("%cause").is_some(),
            "result.and_then Err should propagate: {:?}", result);
    } else {
        panic!("Expected Err combo, got {:?}", result);
    }
}
