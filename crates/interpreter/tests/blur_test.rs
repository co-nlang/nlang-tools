use indexmap::IndexMap;
use nlang_interpreter::value::{
    BlurCause, BlurDetail, ComboVal, ContentHash, EffectTag, HorizonParams, ObservationStrategy,
    Value,
};
use nlang_interpreter::*;
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

fn salt0() -> ContentHash {
    ContentHash::v1(vec![0u8; 32])
}

fn salt1() -> ContentHash {
    ContentHash::v1(vec![1u8; 32])
}

fn make_blur(fuel: u64, cause: BlurCause, salt: ContentHash) -> Value {
    Value::Blur(BlurDetail {
        cause,
        horizon: HorizonParams {
            fuel_remaining: fuel,
            strategy: ObservationStrategy::Blur,
            salt,
        },
        partial: None,
        effect: EffectTag::Pure,
    })
}

// Test 1: blur from fuel exhaustion has deterministic CAID
#[test]
fn blur_fuel_caid_deterministic() {
    let bd1 = BlurDetail {
        cause: BlurCause::FuelExhausted,
        horizon: HorizonParams {
            fuel_remaining: 42,
            strategy: ObservationStrategy::Blur,
            salt: salt0(),
        },
        partial: None,
        effect: EffectTag::Pure,
    };
    let bd2 = bd1.clone();
    assert_eq!(bd1.blur_caid(), bd2.blur_caid());
}

// Test 2: different fuel → different CAID
#[test]
fn blur_different_fuel_different_caid() {
    let make = |fuel: u64| BlurDetail {
        cause: BlurCause::FuelExhausted,
        horizon: HorizonParams {
            fuel_remaining: fuel,
            strategy: ObservationStrategy::Blur,
            salt: salt0(),
        },
        partial: None,
        effect: EffectTag::Pure,
    };
    assert_ne!(make(10).blur_caid(), make(20).blur_caid());
}

// Test 3: blur ∧ Top = Blur
#[test]
fn blur_unify_top_is_blur() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    let blur = make_blur(0, BlurCause::FuelExhausted, ctx.horizon_salt.clone());
    let result = oo.unify_internal(blur.clone(), Value::Top, &mut ctx);
    assert!(matches!(result, Value::Blur(_)));
}

// Test 4: blur ∧ Bottom = Bottom
#[test]
fn blur_unify_bottom_is_bottom() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    let blur = make_blur(0, BlurCause::FuelExhausted, ctx.horizon_salt.clone());
    let bottom = Value::Bottom(Box::new(Default::default()));
    let result = oo.unify_internal(blur, bottom, &mut ctx);
    assert!(matches!(result, Value::Bottom(_)));
}

// Test 5: blur ∧ concrete records partial
#[test]
fn blur_unify_concrete_records_partial() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    let blur = make_blur(0, BlurCause::FuelExhausted, ctx.horizon_salt.clone());
    let concrete = Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None);
    let result = oo.unify_internal(blur, concrete, &mut ctx);
    if let Value::Blur(bd) = result {
        assert!(bd.partial.is_some());
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
    let salt = salt0();
    let result = handle_resource_exhausted(
        ResourceExhausted::FuelExhausted,
        ObservationStrategy::Blur,
        &salt,
        77,
        None,
        EffectTag::Pure,
    );
    assert!(matches!(result, Value::Blur(_)));
    if let Value::Blur(bd) = result {
        assert_eq!(bd.horizon.fuel_remaining, 77);
        assert!(matches!(bd.cause, BlurCause::FuelExhausted));
    }
}

// Test 9: blur complement is blur
#[test]
fn blur_complement_is_blur() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    let blur = make_blur(0, BlurCause::Timeout, ctx.horizon_salt.clone());
    let result = oo.orthocomplement(blur, &mut ctx);
    assert!(matches!(result, Value::Blur(_)));
}

// Test 10: blur content_hash is deterministic and includes horizon
#[test]
fn blur_content_hash_deterministic() {
    let bd = BlurDetail {
        cause: BlurCause::MathSingularity("log_singularity".to_string()),
        horizon: HorizonParams {
            fuel_remaining: 100,
            strategy: ObservationStrategy::Blur,
            salt: salt1(),
        },
        partial: None,
        effect: EffectTag::Pure,
    };
    let v = Value::Blur(bd);
    let h1 = v.content_hash();
    let h2 = v.content_hash();
    assert_eq!(h1, h2);
}

// Test 11: blur BN/ serialization produces non-empty bytes with 0xFD prefix
#[test]
fn blur_bn_serial_deterministic() {
    use nlang_interpreter::bn_serial::serialize_bn;
    let bd = BlurDetail {
        cause: BlurCause::FuelExhausted,
        horizon: HorizonParams {
            fuel_remaining: 0,
            strategy: ObservationStrategy::Blur,
            salt: salt0(),
        },
        partial: None,
        effect: EffectTag::Pure,
    };
    let v = Value::Blur(bd);
    let b1 = serialize_bn(&v);
    let b2 = serialize_bn(&v);
    assert!(!b1.is_empty());
    assert_eq!(b1, b2);
    assert_eq!(b1[0], 0xFD);
}
