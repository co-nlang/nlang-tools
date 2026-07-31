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

fn advertise(oo: &Ouroboros, ctx: &mut EvalContext, val: Value) {
    oo.builtin_registry.get("disc.advertise").unwrap().clone()(val, oo, ctx);
}

fn find(oo: &Ouroboros, ctx: &mut EvalContext, query: Value) -> Value {
    oo.builtin_registry.get("disc.find").unwrap().clone()(query, oo, ctx)
}

// ─── 1. Value in store → returns in one hop ───────────────────────────────────

#[test]
fn test_find_returns_stored_value_in_one_hop() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let val = combo(&[("x", 1), ("y", 2)]);

    oo.store.put_value(&val).expect("put_value should succeed");
    advertise(&oo, &mut ctx, val.clone());

    let result = find(&oo, &mut ctx, combo(&[("x", 1)]));

    assert!(
        !matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::MissingKey)),
        "stored + advertised value should be findable"
    );
    assert_eq!(ctx.disc_routing_hops, 1, "single hop should suffice");
}

// ─── 2. Not in store → hops to semantically related node ──────────────────────

#[test]
fn test_find_multihop_skips_unstored_node() {
    let oo = oo();
    let mut ctx = oo.eval_context();

    let node_a = combo(&[("a", 10), ("b", 20)]);
    advertise(&oo, &mut ctx, node_a.clone());

    let node_b = combo(&[("b", 20), ("c", 30)]);
    oo.store.put_value(&node_b).expect("put_value");
    advertise(&oo, &mut ctx, node_b.clone());

    let result = find(&oo, &mut ctx, combo(&[("a", 10), ("b", 20)]));

    assert!(
        !matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::SemanticEclipse)),
        "two-hop routing should not exhaust budget"
    );
    assert!(
        ctx.disc_routing_hops >= 1,
        "should have made at least one hop"
    );
}

// ─── 3. Multi-hop increments disc_routing_hops ────────────────────────────────

#[test]
fn test_multihop_increments_hop_counter() {
    let oo = oo();
    let mut ctx = oo.eval_context();

    for i in 0..3_i64 {
        let v = combo(&[("x", i), ("y", i + 1)]);
        advertise(&oo, &mut ctx, v.clone());
        if i == 2 {
            oo.store.put_value(&v).expect("put_value");
        }
    }

    let _ = find(&oo, &mut ctx, combo(&[("x", 0), ("y", 1)]));

    assert!(ctx.disc_routing_hops >= 1);
}

// ─── 4. SemanticEclipse when budget exhausted ─────────────────────────────────

#[test]
fn test_multihop_semantic_eclipse_on_budget_exhaustion() {
    let oo = oo();
    let mut ctx = oo.eval_context();

    let node = combo(&[("z", 99)]);
    advertise(&oo, &mut ctx, node.clone());

    ctx.disc_routing_hops = 15;

    let _ = find(&oo, &mut ctx, combo(&[("z", 99)]));

    let _ = find(&oo, &mut ctx, combo(&[("z", 99)]));
    ctx.disc_routing_hops = 16;
    let eclipse = find(&oo, &mut ctx, combo(&[("z", 99)]));
    assert!(
        matches!(&eclipse, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::SemanticEclipse)),
        "disc_routing_hops >= 16 should give SemanticEclipse, got {:?}",
        eclipse
    );
}

// ─── 5. Empty registry → MissingKey (not SemanticEclipse) ────────────────────

#[test]
fn test_multihop_empty_registry_is_missing_key() {
    let oo = oo();
    let mut ctx = oo.eval_context();
    let result = find(&oo, &mut ctx, combo(&[("q", 1)]));
    assert!(
        matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::MissingKey)),
        "empty registry should be MissingKey"
    );
    assert_eq!(
        ctx.disc_routing_hops, 0,
        "no hop should occur when registry is empty"
    );
}

// ─── 6. Visited set grows across hops ─────────────────────────────────────────

#[test]
fn test_multihop_visited_set_accumulates() {
    let oo = oo();
    let mut ctx = oo.eval_context();

    let a = combo(&[("p", 1), ("q", 2)]);
    let b = combo(&[("p", 3), ("q", 4)]);
    advertise(&oo, &mut ctx, a);
    advertise(&oo, &mut ctx, b);

    let _ = find(&oo, &mut ctx, combo(&[("p", 1)]));

    assert!(
        !ctx.disc_routing_visited.is_empty(),
        "visited set should accumulate hops"
    );
    assert!(ctx.disc_routing_hops > 0, "hop counter should advance");
}
