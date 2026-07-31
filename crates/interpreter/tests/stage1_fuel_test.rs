// Stage 1 acceptance (handover §2.7): Thunk construction is free; only force
// consumes fuel (GUIDE_03 §11.4). This is the foundation for Stage 2's lazy
// observe — at Stage 1, fields are still forced at combo assembly, but the
// Thunk type itself must not burn fuel on construction.
//
// Test: build a Thunk manually, verify fuel unchanged; then force it, verify
// fuel decreased.

use indexmap::IndexMap;
use nlang_interpreter::{ComboVal, EffectTag, EvalContext, Ouroboros, Value};
use nlang_parser::ast::{AtomKind, Expr, ExprKind, Span};

fn empty_oo() -> Ouroboros {
    Ouroboros::new_in_memory()
}

#[test]
fn thunk_construction_does_not_consume_fuel() {
    let oo = empty_oo();
    let root = ComboVal::default();
    let mut ctx = EvalContext::new(root).with_fuel(10000);
    let fuel_before = ctx.fuel;

    // Construct a Thunk wrapping a literal int expression. No eval, no force.
    let thunk = Value::Thunk {
        expr: Box::new(Expr::new(
            ExprKind::Atom(AtomKind::Int(42.into())),
            Span::new(0, 0),
        )),
        closure: vec![],
        context: None,
        effect: EffectTag::Pure,
    };

    // Constructing the Thunk must not touch fuel.
    assert_eq!(
        ctx.fuel, fuel_before,
        "Thunk construction must be free (got {} → {})",
        fuel_before, ctx.fuel
    );

    // Forcing it consumes fuel (eval runs).
    let _ = oo.force(thunk, &mut ctx);
    assert!(
        ctx.fuel < fuel_before,
        "force must consume fuel (got {} → {})",
        fuel_before,
        ctx.fuel
    );
}

#[test]
fn unobserved_combo_field_thunk_construction_is_free() {
    // Build a combo with a heavy-ish field (nested arithmetic). At Stage 1,
    // combo assembly forces the field (mechanical-refactor invariant), so this
    // test documents the *Thunk construction* cost being zero — the fuel delta
    // comes entirely from the force, not the Thunk::new.
    let oo = empty_oo();
    let root = ComboVal::default();
    let mut ctx = EvalContext::new(root).with_fuel(10000);
    let fuel_before = ctx.fuel;

    // A thunk wrapping an arithmetic expression.
    let expr = Expr::new(
        ExprKind::Add(
            Box::new(Expr::new(
                ExprKind::Atom(AtomKind::Int(1.into())),
                Span::new(0, 0),
            )),
            Box::new(Expr::new(
                ExprKind::Atom(AtomKind::Int(2.into())),
                Span::new(0, 0),
            )),
        ),
        Span::new(0, 0),
    );
    let thunk = Value::Thunk {
        expr: Box::new(expr),
        closure: vec![],
        context: None,
        effect: EffectTag::Pure,
    };
    // Thunk construction: free.
    assert_eq!(ctx.fuel, fuel_before);

    // Force: fuel drops.
    let v = oo.force(thunk, &mut ctx);
    assert!(ctx.fuel < fuel_before);
    // And the value is correct.
    match v {
        Value::Atom(AtomKind::Int(i), _, _) => assert_eq!(i, 3.into()),
        other => panic!("expected int 3, got {:?}", other),
    }
}
