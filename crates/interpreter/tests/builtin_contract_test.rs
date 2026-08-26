use nlang_interpreter::builtins::{contract_for, contract_ids};
use nlang_interpreter::Ouroboros;

#[test]
fn every_registered_builtin_declares_its_keys() {
    let oo = Ouroboros::new_in_memory();
    let mut missing = Vec::new();
    for id in oo.builtin_registry.keys() {
        if contract_for(id).is_none() {
            missing.push(id.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "registered builtins with no contract: {missing:?}"
    );
}

#[test]
fn every_contract_has_a_registered_builtin() {
    let oo = Ouroboros::new_in_memory();
    let mut extra = Vec::new();
    for id in contract_ids() {
        if !oo.builtin_registry.contains_key(id) {
            extra.push(id);
        }
    }
    assert!(
        extra.is_empty(),
        "contracts with no registered builtin: {extra:?}"
    );
}

#[test]
fn a_named_builtin_declares_named_keys_not_slots() {
    let c = contract_for("engine.project_down").expect("declared");
    assert_eq!(c.required, &["target", "masa"]);
}

#[test]
fn an_arity_zero_builtin_declares_no_required_keys() {
    let c = contract_for("math.random").expect("declared");
    assert!(c.required.is_empty());
}
