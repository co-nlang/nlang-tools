// Updated under the_name_points_at_the_remedy handover §7 (ERROR_CODES §2.7.1).
// Hop-budget exhaustion used to mint `#semantic_eclipse`; the registry
// renamed that situation to `#routing_budget_exceeded` so the tag points at
// the remedy (budget/topology), not at an attack. Expectations and the
// budget-exhaustion test name below follow that rename. `#semantic_eclipse`
// remains a readable BottomCause variant for stored universes — only minting
// stopped. No tests deleted; no assertions weakened.

use indexmap::IndexMap;
use nlang_interpreter::value::{BottomCause, ComboVal, EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

fn oo() -> Ouroboros {
    Ouroboros::new_in_memory()
}

fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}

fn combo(fields: &[(&str, i64)]) -> Value {
    let mut m = IndexMap::new();
    for (k, v) in fields {
        m.insert(k.to_string(), int_val(*v));
    }
    Value::Combo(ComboVal::new(
        m,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

fn call_find(oo: &Ouroboros, ctx: &mut EvalContext, arg: Value) -> Value {
    oo.builtin_registry.get("disc.find").unwrap().clone()(arg, oo, ctx)
}

fn call_advertise(oo: &Ouroboros, ctx: &mut EvalContext, arg: Value) -> Value {
    oo.builtin_registry.get("disc.advertise").unwrap().clone()(arg, oo, ctx)
}

// ─── 1. Empty registry → MissingKey (not SemanticEclipse) ────────────────────

#[test]
fn test_find_empty_registry_is_missing_key() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let result = call_find(&oo, &mut ctx, combo(&[("x", 1)]));
    assert!(
        matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::MissingKey)),
        "empty registry should be MissingKey, got {:?}",
        result
    );
}

// ─── 2. Normal find: adds chosen node to disc_routing_visited ─────────────────

#[test]
fn test_find_adds_to_visited() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let node = combo(&[("x", 1)]);
    oo.store.put_value(&node).expect("put_value");
    call_advertise(&oo, &mut ctx, node.clone());

    assert!(
        ctx.disc_routing_visited.is_empty(),
        "visited should start empty"
    );
    let _ = call_find(&oo, &mut ctx, node.clone());
    assert_eq!(
        ctx.disc_routing_hops, 1,
        "hop count should be 1 after one find"
    );
    assert!(
        !ctx.disc_routing_visited.is_empty(),
        "visited should be non-empty after find"
    );
}

// ─── 3. Budget exceeded → RoutingBudgetExceeded (ERROR_CODES §2.7.1) ──────────

#[test]
fn test_find_hop_budget_exceeded_returns_routing_budget_exceeded() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let node = combo(&[("x", 42)]);
    call_advertise(&oo, &mut ctx, node.clone());

    ctx.disc_routing_hops = 16;

    let result = call_find(&oo, &mut ctx, node);
    assert!(
        matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::RoutingBudgetExceeded)),
        "exceeded hop budget should return RoutingBudgetExceeded, got {:?}",
        result
    );
}

// ─── 4. Cause tags (readable retained + new mint name) ────────────────────────

#[test]
fn test_routing_budget_exceeded_as_tag() {
    assert_eq!(
        BottomCause::RoutingBudgetExceeded.as_tag(),
        "routing_budget_exceeded"
    );
    // Retained for stored-universe read (ERROR_CODES §2.7.1); not minted.
    assert_eq!(BottomCause::SemanticEclipse.as_tag(), "semantic_eclipse");
}

// ─── 5. All-visited fallback: still returns a result (not SemanticEclipse) ────

#[test]
fn test_find_all_visited_still_returns() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let node = combo(&[("p", 100), ("q", 200)]);
    oo.store.put_value(&node).expect("put_value");
    call_advertise(&oo, &mut ctx, node.clone());

    let r1 = call_find(&oo, &mut ctx, node.clone());
    assert_eq!(ctx.disc_routing_hops, 1);

    let r2 = call_find(&oo, &mut ctx, node.clone());
    assert!(
        !matches!(&r2, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::SemanticEclipse)),
        "all-visited with budget remaining should not SemanticEclipse, got {:?}",
        r2
    );
    assert_eq!(ctx.disc_routing_hops, 2);
}

// ─── 6. horizon_salt tiebreaker: same query → same chosen node (deterministic) ─

#[test]
fn test_find_tiebreaker_is_deterministic() {
    let oo = oo();

    let node_a = combo(&[("a", 1)]);
    let node_b = combo(&[("b", 2)]);

    let mut ctx = oo.eval_context();
    oo.store.put_value(&node_a).expect("put_value");
    oo.store.put_value(&node_b).expect("put_value");
    call_advertise(&oo, &mut ctx, node_a.clone());
    call_advertise(&oo, &mut ctx, node_b.clone());

    let mut ctx1 = oo.eval_context();
    let r1 = call_find(&oo, &mut ctx1, combo(&[("a", 1)]));

    let mut ctx2 = oo.eval_context();
    let r2 = call_find(&oo, &mut ctx2, combo(&[("a", 1)]));

    assert_eq!(ctx1.disc_routing_hops, 1);
    assert_eq!(ctx2.disc_routing_hops, 1);

    assert_eq!(ctx1.disc_routing_visited.len(), 1);
    assert_eq!(ctx2.disc_routing_visited.len(), 1);

    assert!(
        !matches!(&r1, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::SemanticEclipse))
    );
    assert!(
        !matches!(&r2, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::SemanticEclipse))
    );
}
