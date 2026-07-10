use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, ComboVal, EffectTag, BlurDetail, BlurCause, HorizonParams, ObservationStrategy};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;

fn get_refl_morph(name: &str, oo: &Ouroboros) -> Value {
    let root = oo.root_with_system();
    let refl = root.get_field("~%Reflection").expect("~%Reflection exists");
    if let Value::Combo(ref c) = refl {
        c.get_field(name).cloned().expect(&format!("{} exists", name))
    } else { panic!("~%Reflection is not a Combo") }
}

fn apply_refl(morph_name: &str, val: Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Value {
    let mut arg_fields = IndexMap::new();
    arg_fields.insert("0".to_string(), val);
    let arg = Value::Combo(ComboVal::new(arg_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
    let morph = get_refl_morph(morph_name, oo);
    oo.force(oo.apply_morphism(morph, arg, ctx), ctx)
}

fn is_true(v: &Value) -> bool {
    matches!(v.collapse(), Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "true")
}
fn is_false(v: &Value) -> bool {
    matches!(v.collapse(), Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "false")
}

fn blur_val() -> Value {
    Value::Blur(BlurDetail {
        cause: BlurCause::FuelExhausted,
        horizon: HorizonParams {
            fuel_remaining: 0,
            strategy: ObservationStrategy::Blur,
            salt: nlang_interpreter::value::ContentHash::parse(
                "hash:sha256:v1:0000000000000000000000000000000000000000000000000000000000000000"
            ).unwrap(),
        },
        partial: None,
        effect: EffectTag::Pure,
    })
}

#[test]
fn refl_is_blur_on_blur() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let result = apply_refl("/is_blur", blur_val(), &oo, &mut ctx);
    assert!(is_true(&result), "is_blur(Blur) should be #true: {:?}", result);
}

#[test]
fn refl_is_blur_on_non_blur() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let result = apply_refl("/is_blur", Value::Top, &oo, &mut ctx);
    assert!(is_false(&result), "is_blur(Top) should be #false: {:?}", result);
}

#[test]
fn refl_is_bottom_on_bottom() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let bottom: Value = nlang_interpreter::value::BottomCause::Conflict.into();
    let result = apply_refl("/is_bottom", bottom, &oo, &mut ctx);
    assert!(is_true(&result), "is_bottom(Bottom) should be #true: {:?}", result);
}

#[test]
fn refl_is_some_and_is_none() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());

    let mut some_fields = IndexMap::new();
    some_fields.insert("%val".to_string(), Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None));
    let some_val = Value::Combo(ComboVal::new(some_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));

    let none_val = Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);

    assert!(is_true(&apply_refl("/is_some", some_val.clone(), &oo, &mut ctx)), "is_some(Some) = #true");
    assert!(is_false(&apply_refl("/is_none", some_val, &oo, &mut ctx)), "is_none(Some) = #false");
    assert!(is_false(&apply_refl("/is_some", none_val.clone(), &oo, &mut ctx)), "is_some(None) = #false");
    assert!(is_true(&apply_refl("/is_none", none_val, &oo, &mut ctx)), "is_none(None) = #true");
}

#[test]
fn refl_is_ok_and_is_err() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());

    let mut ok_fields = IndexMap::new();
    ok_fields.insert("%val".to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None));
    let ok_val = Value::Combo(ComboVal::new(ok_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));

    let mut err_fields = IndexMap::new();
    err_fields.insert("%cause".to_string(), Value::Atom(AtomKind::Tag("fail".to_string()), EffectTag::Pure, None));
    let err_val = Value::Combo(ComboVal::new(err_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));

    assert!(is_true(&apply_refl("/is_ok", ok_val.clone(), &oo, &mut ctx)), "is_ok(Ok) = #true");
    assert!(is_false(&apply_refl("/is_err", ok_val, &oo, &mut ctx)), "is_err(Ok) = #false");
    assert!(is_false(&apply_refl("/is_ok", err_val.clone(), &oo, &mut ctx)), "is_ok(Err) = #false");
    assert!(is_true(&apply_refl("/is_err", err_val, &oo, &mut ctx)), "is_err(Err) = #true");
}

#[test]
fn refl_to_str() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let val = Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None);
    let result = apply_refl("/to_str", val, &oo, &mut ctx);
    if let Value::Atom(AtomKind::Str(s), _, _) = result.collapse() {
        assert!(s.contains("42"), "to_str(42) should contain '42': {}", s);
    } else {
        panic!("Expected Str, got {:?}", result);
    }
}

#[test]
fn refl_bottom_cause_on_bottom() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let bottom: Value = nlang_interpreter::value::BottomCause::FuelExhausted.into();
    let result = apply_refl("/bottom_cause", bottom, &oo, &mut ctx);
    if let Value::Atom(AtomKind::Tag(t), _, _) = result.collapse() {
        assert!(t.contains("fuel"), "bottom_cause(FuelExhausted) should contain 'fuel': {}", t);
    } else {
        panic!("Expected Tag, got {:?}", result);
    }
}

#[test]
fn refl_bottom_cause_on_non_bottom() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let result = apply_refl("/bottom_cause", Value::Top, &oo, &mut ctx);
    if let Value::Atom(AtomKind::Tag(t), _, _) = result.collapse() {
        assert_eq!(t.trim_start_matches('#'), "none",
            "bottom_cause(non-Bottom) should return #none: {}", t);
    } else {
        panic!("Expected #none, got {:?}", result);
    }
}

#[test]
fn refl_type_of_blur() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let result = apply_refl("/type_of", blur_val(), &oo, &mut ctx);
    if let Value::Atom(AtomKind::Tag(t), _, _) = result.collapse() {
        assert_eq!(t.trim_start_matches('#'), "blur",
            "type_of(Blur) should return #blur: {}", t);
    } else {
        panic!("Expected #blur tag, got {:?}", result);
    }
}
