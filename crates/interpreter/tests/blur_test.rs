use nlang_interpreter::value::{
    BlurCause, BlurDetail, EffectTag, HorizonParams, ObservationStrategy, Value,
};
use nlang_interpreter::*;
use nlang_parser::ast::AtomKind;

fn default_horizon(fuel_remaining: u64) -> HorizonParams {
    HorizonParams {
        fuel: 10000,
        fuel_remaining,
        strategy: ObservationStrategy::Blur,
        max_branches: 64,
        max_unification_depth: 256,
        max_lifting_depth: 32,
        max_pattern_nodes: 1024,
    }
}

fn make_blur(fuel_remaining: u64, cause: BlurCause) -> Value {
    Value::Blur(BlurDetail::from_single(
        cause,
        default_horizon(fuel_remaining),
        None,
        EffectTag::Pure,
    ))
}

// Test 1: blur CAID is deterministic (O42 CHS)
#[test]
fn blur_fuel_caid_deterministic() {
    let bd1 = BlurDetail::from_single(
        BlurCause::FuelExhausted,
        default_horizon(42),
        None,
        EffectTag::Pure,
    );
    let bd2 = bd1.clone();
    assert_eq!(bd1.blur_caid(), bd2.blur_caid());
}

// Test 2: different fuel BUDGET → different CAID (remaining is not identity)
#[test]
fn blur_different_fuel_budget_different_caid() {
    let make = |budget: u64| {
        let mut h = default_horizon(0);
        h.fuel = budget;
        BlurDetail::from_single(BlurCause::FuelExhausted, h, None, EffectTag::Pure)
    };
    assert_ne!(make(10).blur_caid(), make(20).blur_caid());
}

// Test 2b: fuel_remaining alone does NOT change CAID (O42 R-2)
#[test]
fn blur_fuel_remaining_not_in_caid() {
    let a = BlurDetail::from_single(
        BlurCause::FuelExhausted,
        default_horizon(10),
        None,
        EffectTag::Pure,
    );
    let b = BlurDetail::from_single(
        BlurCause::FuelExhausted,
        default_horizon(20),
        None,
        EffectTag::Pure,
    );
    assert_eq!(
        a.blur_caid(),
        b.blur_caid(),
        "fuel_remaining must not enter identity"
    );
}

// Test 3: blur ∧ Top = Blur
#[test]
fn blur_unify_top_is_blur() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    let blur = make_blur(0, BlurCause::FuelExhausted);
    let result = oo.unify_internal(blur.clone(), Value::Top, &mut ctx);
    assert!(matches!(result, Value::Blur(_)));
}

// Test 4: blur ∧ Bottom = Bottom
#[test]
fn blur_unify_bottom_is_bottom() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    let blur = make_blur(0, BlurCause::FuelExhausted);
    let bottom = Value::Bottom(Box::new(Default::default()));
    let result = oo.unify_internal(blur, bottom, &mut ctx);
    assert!(matches!(result, Value::Bottom(_)));
}

// Test 5: O47 — blur ∧ concrete leaves the blur snapshot unrewritten
#[test]
fn blur_unify_concrete_preserves_snapshot() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    let blur = make_blur(0, BlurCause::FuelExhausted);
    let before = if let Value::Blur(bd) = &blur {
        bd.blur_caid()
    } else {
        panic!("expected blur");
    };
    let concrete = Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None);
    let result = oo.unify_internal(blur, concrete, &mut ctx);
    if let Value::Blur(bd) = result {
        assert!(bd.partial.is_none(), "O47: absorption must not rewrite partial");
        assert_eq!(bd.blur_caid(), before, "O47: absorption must not move CAID");
    } else {
        panic!("expected Blur");
    }
}

// Test 6: math ln(0) returns Blur in Blur strategy
#[test]
fn math_ln_zero_returns_blur() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system()).with_strategy(ObservationStrategy::Blur);
    let root = oo.root_with_system();
    let math_combo = root.get_field("~%Math").unwrap().clone();
    let ln_morph = match &math_combo {
        Value::Combo(c) => c.get_field("/ln").unwrap().clone(),
        _ => panic!("expected Combo"),
    };
    let arg = Value::Atom(AtomKind::Float(0.0), EffectTag::Pure, None);
    let result = oo.apply_morphism(ln_morph, arg, &mut ctx);
    assert!(
        matches!(result, Value::Blur(_)),
        "ln(0) should return Blur in Blur mode, got {:?}",
        result
    );
}

// Test 7: math ln(0) returns Blur in Strict strategy too (math singularity always blur)
#[test]
fn math_ln_zero_strict_returns_blur() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx =
        EvalContext::new(oo.root_with_system()).with_strategy(ObservationStrategy::Strict);
    let root = oo.root_with_system();
    let math_combo = root.get_field("~%Math").unwrap().clone();
    let ln_morph = match &math_combo {
        Value::Combo(c) => c.get_field("/ln").unwrap().clone(),
        _ => panic!("expected Combo"),
    };
    let arg = Value::Atom(AtomKind::Float(0.0), EffectTag::Pure, None);
    let result = oo.apply_morphism(ln_morph, arg, &mut ctx);
    assert!(matches!(result, Value::Blur(_)));
}

// Test 8: handle_resource_exhausted → Value::Blur
#[test]
fn handle_resource_exhausted_returns_blur() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    ctx.fuel = 77;
    ctx.fuel_budget = 10000;
    let result = handle_resource_exhausted(
        ResourceExhausted::FuelExhausted,
        ObservationStrategy::Blur,
        &ctx,
        None,
        EffectTag::Pure,
    );
    assert!(matches!(result, Value::Blur(_)));
    if let Value::Blur(bd) = result {
        assert_eq!(bd.horizon.fuel_remaining, 77);
        assert_eq!(bd.horizon.fuel, 10000);
        assert!(matches!(bd.cause, BlurCause::FuelExhausted));
    }
}

// Test 9: blur complement is blur
#[test]
fn blur_complement_is_blur() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    let blur = make_blur(0, BlurCause::Timeout);
    let result = oo.orthocomplement(blur, &mut ctx);
    assert!(matches!(result, Value::Blur(_)));
}

// Test 10: blur content_hash is deterministic and includes horizon budgets
#[test]
fn blur_content_hash_deterministic() {
    let mut h = default_horizon(100);
    h.fuel = 5000;
    let bd = BlurDetail::from_single(
        BlurCause::MathSingularity("log_singularity".to_string()),
        h,
        None,
        EffectTag::Pure,
    );
    let v = Value::Blur(bd);
    let h1 = v.content_hash();
    let h2 = v.content_hash();
    assert_eq!(h1, h2);
}

// Test 11: blur BN/ serialization produces non-empty bytes with 0xFD prefix
#[test]
fn blur_bn_serial_deterministic() {
    use nlang_interpreter::bn_serial::serialize_bn;
    let bd = BlurDetail::from_single(
        BlurCause::FuelExhausted,
        default_horizon(0),
        None,
        EffectTag::Pure,
    );
    let v = Value::Blur(bd);
    let b1 = serialize_bn(&v);
    let b2 = serialize_bn(&v);
    assert!(!b1.is_empty());
    assert_eq!(b1, b2);
    assert_eq!(b1[0], 0xFD);
}
