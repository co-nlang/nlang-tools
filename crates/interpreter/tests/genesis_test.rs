use nlang_interpreter::*;
use nlang_interpreter::type_constraint::{TypeConstraint, ValidationResult};
use nlang_interpreter::value::{ComboVal, EffectTag};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use num_traits::ToPrimitive;

/// Verify genesis seed CAIDs are stable.
/// If this test fails, the module definitions in root_with_system() have changed.
/// Run with --nocapture to see the "UPDATE:" lines, copy them into genesis.rs.
#[test]
fn seed_caids_are_stable() {
    let oo = Ouroboros::new_in_memory();
    let root = oo.root_with_system();

    let seeds: Vec<(&str, &str)> = vec![
        ("~%Math",       nlang_interpreter::genesis::SEED_MATH),
        ("~%List",       nlang_interpreter::genesis::SEED_LIST),
        ("~%Cond",       nlang_interpreter::genesis::SEED_COND),
        ("~%String",     nlang_interpreter::genesis::SEED_STRING),
        ("~%Complex",    nlang_interpreter::genesis::SEED_COMPLEX),
        ("~%Reflection", nlang_interpreter::genesis::SEED_REFL),
        ("~%Time",       nlang_interpreter::genesis::SEED_TIME),
        ("~%Discovery",  nlang_interpreter::genesis::SEED_DISCOVERY),
        ("@option",      nlang_interpreter::genesis::SEED_OPTION),
        ("@result",      nlang_interpreter::genesis::SEED_RESULT),
        ("~%Config",     nlang_interpreter::genesis::SEED_CONFIG),
    ];

    // Verify every seed matches its constant
    let mut all_ok = true;
    for (path, expected_seed) in &seeds {
        let val = root.get_field(path).unwrap();
        let computed = val.content_hash_v1().to_string();
        if &computed != expected_seed {
            eprintln!("UPDATE: {} => \"{}\"", path, computed);
            all_ok = false;
        }
    }

    if !all_ok {
        panic!("Seed CAID mismatch. Copy the UPDATE: lines above into genesis.rs");
    }
}

// ── Phase 12: @option / @result tests ──

#[test]
fn at_option_in_root_with_system() {
    let oo = Ouroboros::new_in_memory();
    let root = oo.root_with_system();
    assert!(root.get_field("@option").is_some(), "@option should be in root_with_system");
}

#[test]
fn at_result_in_root_with_system() {
    let oo = Ouroboros::new_in_memory();
    let root = oo.root_with_system();
    assert!(root.get_field("@result").is_some(), "@result should be in root_with_system");
}

#[test]
fn type_constraint_option_accepts_none() {
    let v = Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
    assert!(matches!(TypeConstraint::Option.validate_value(&v), ValidationResult::Pass));
}

#[test]
fn type_constraint_option_accepts_some() {
    let mut fields = IndexMap::new();
    fields.insert("%val".to_string(), Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None));
    let v = Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
    assert!(matches!(TypeConstraint::Option.validate_value(&v), ValidationResult::Pass));
}

#[test]
fn config_in_root_with_system() {
    let oo = Ouroboros::new_in_memory();
    let root = oo.root_with_system();
    let config = root.get_field("~%Config").expect("~%Config should exist");
    if let Value::Combo(cv) = config {
        let fuel = cv.get_field("%fuel").expect("%fuel should exist");
        if let Value::Atom(AtomKind::Int(n), _, _) = fuel {
            assert_eq!(n.to_u64().unwrap_or(0), 10000, "%fuel default should be 10000");
        } else {
            panic!("%fuel should be an Int");
        }
    } else {
        panic!("~%Config should be a Combo");
    }
}

fn type_constraint_result_accepts_ok_and_err() {
    let mut ok_fields = IndexMap::new();
    ok_fields.insert("%val".to_string(), Value::Top);
    let ok = Value::Combo(ComboVal::new(ok_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
    assert!(matches!(TypeConstraint::Result.validate_value(&ok), ValidationResult::Pass));

    let mut err_fields = IndexMap::new();
    err_fields.insert("%cause".to_string(), Value::Atom(AtomKind::Tag("timeout".to_string()), EffectTag::Pure, None));
    let err = Value::Combo(ComboVal::new(err_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
    assert!(matches!(TypeConstraint::Result.validate_value(&err), ValidationResult::Pass));
}

// ── Phase 14: ~%Config → eval_context() tests ──

#[test]
fn eval_context_reads_config_fuel() {
    let oo = Ouroboros::new_in_memory();
    let ctx = oo.eval_context();
    assert_eq!(ctx.fuel, 10000, "eval_context() should read fuel from ~%Config");
}

#[test]
fn eval_context_reads_config_max_branches() {
    let oo = Ouroboros::new_in_memory();
    let ctx = oo.eval_context();
    assert_eq!(ctx.max_branches, 64, "eval_context() should read max_branches from ~%Config");
}

#[test]
fn eval_context_reads_config_strategy() {
    let oo = Ouroboros::new_in_memory();
    let ctx = oo.eval_context();
    assert!(matches!(ctx.strategy, ObservationStrategy::Blur),
        "~%Config %strategy: #blur should map to ObservationStrategy::Blur");
}

// ── Phase 15: %timeout → timeout_deadline tests ──

#[test]
fn eval_context_sets_timeout_deadline() {
    let oo = Ouroboros::new_in_memory();
    let ctx = oo.eval_context();
    assert!(ctx.timeout_deadline.is_some(),
        "eval_context() should set timeout_deadline from ~%Config %timeout");
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let deadline = ctx.timeout_deadline.unwrap();
    assert!(deadline > now_ms, "timeout_deadline should be in the future");
    assert!(deadline < now_ms + 2000, "timeout_deadline should be within 2 seconds");
}

#[test]
fn eval_context_new_has_no_timeout() {
    let oo = Ouroboros::new_in_memory();
    let ctx = EvalContext::new(oo.root_with_system());
    assert!(ctx.timeout_deadline.is_none(),
        "EvalContext::new() should not set timeout_deadline");
}
