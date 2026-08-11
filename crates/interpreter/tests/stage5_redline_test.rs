// Stage 5 red-line probes — WRITTEN BY THE WORK ORDER (2026-07-08).
// Acceptance = remove the #[ignore] attributes and everything is green.
// Deleting or weakening a probe = work-order violation, escalate instead.
//
// R1: Route B point — memo survives evolve of an UNRELATED coordinate.
// R2: no-regression — evolve of a RELATED (read) coordinate still invalidates.
// R3: C₀ (path-free, $-free content) survives ANY evolve (permanent tier).

use nlang_interpreter::value::ComboVal;
use nlang_interpreter::{EvalContext, Ouroboros, Universe, Value};
use nlang_parser::ast::{AtomKind, Path, PathAnchor, Span};
use nlang_parser::parse_program;

fn path_of(segments: &[&str]) -> Path {
    Path {
        anchor: PathAnchor::Bare,
        segments: segments.iter().map(|s| s.to_string()).collect(),
        span: Span::default(),
    }
}

fn evolve_all(engine: &Ouroboros, universe: &mut Universe, src: &str) {
    let program =
        parse_program(src).unwrap_or_else(|e| panic!("parse failed for {:?}: {:?}", src, e));
    for field in &program.fields {
        universe
            .evolve(engine, field)
            .unwrap_or_else(|e| panic!("evolve failed for {:?}: {:?}", src, e));
    }
}

fn fuel_after_observe(
    engine: &Ouroboros,
    universe: &Universe,
    path: &Path,
    initial_fuel: u64,
) -> (Value, u64) {
    let root = engine.unify(
        Value::Combo(universe.root.clone()),
        Value::Combo(universe.staged.clone()),
    );
    let root_val = match root {
        Value::Combo(r) => r,
        _ => ComboVal::default(),
    };
    let mut ctx = EvalContext::new(root_val).with_fuel(initial_fuel);
    let val = engine.resolve_path(path, &mut ctx);
    let result = engine.force_recursive(val, &mut ctx);
    (result, ctx.fuel)
}

/// ACCEPTOR EDIT (the_meter_reads_two, 2026-08-11). Same observation, also
/// reporting how many force-memo entries were served.
///
/// The tests here used to detect a memo hit by "the second observe left more
/// fuel". That instrument is gone on purpose: if a warm cache leaves more
/// fuel, the horizon — and so the `#blur` CHS, and so its CAID — depends on
/// whether a value happened to be observed before. `force_memo_hit_count` is
/// diagnostic state that never touches fuel or identity, which is exactly what
/// a memo detector has to be.
fn observe_with_hits(
    engine: &Ouroboros,
    universe: &Universe,
    path: &Path,
    initial_fuel: u64,
) -> (Value, u64, u64) {
    let before = engine.force_memo_hit_count();
    let (v, fuel) = fuel_after_observe(engine, universe, path, initial_fuel);
    (v, fuel, engine.force_memo_hit_count() - before)
}

fn int_of(v: &Value) -> Option<i64> {
    match v {
        Value::Atom(AtomKind::Int(n), _, _) => n.try_into().ok(),
        _ => None,
    }
}

// R1 (THE Route B line): observe r.v (M-tier thunk, reads only its pipe
// context) twice to establish the memo, evolve an unrelated coordinate,
// observe again — fuel must STILL be reduced (entry not wiped by the
// unrelated root change).
#[test]
fn stage5_r1_memo_survives_unrelated_evolve() {
    let engine = Ouroboros::new_in_memory();
    let mut universe = Universe::new(None, ComboVal::default());
    evolve_all(
        &engine,
        &mut universe,
        "r: { a: 1, b: 2 } |> { v: $.a + $.b }",
    );
    let p = path_of(&["r", "v"]);

    // ACCEPTOR EDIT (the_meter_reads_two, 2026-08-11): a memo hit is detected
    // by the hit counter, not by fuel. Fuel must be identical warm and cold —
    // otherwise cache warmth moves the horizon and the #blur CAID with it.
    let (v1, cold, _) = observe_with_hits(&engine, &universe, &p, 1000);
    assert_eq!(int_of(&v1), Some(3));
    let (_, warm, warm_hits) = observe_with_hits(&engine, &universe, &p, 1000);
    assert!(warm_hits > 0, "precondition: memo hit before evolve");
    assert_eq!(warm, cold, "a memo hit changed the fuel account");

    // unrelated coordinate
    evolve_all(&engine, &mut universe, "zzz_unrelated: 9");

    let (v3, after, after_hits) = observe_with_hits(&engine, &universe, &p, 1000);
    assert_eq!(int_of(&v3), Some(3));
    assert!(after_hits > 0,
        "RED LINE (Route B): evolving an unrelated coordinate must not wipe the entry — the observe served no memo entry");
    assert_eq!(after, cold, "a memo hit changed the fuel account");
}

// R2 (no-regression): a thunk whose eval READS a root coordinate must be
// invalidated when that coordinate evolves. `t.flag` is read through the
// root; after re-evolving t, the observed value must reflect the new root.
#[test]
fn stage5_r2_related_evolve_still_invalidates() {
    let engine = Ouroboros::new_in_memory();
    let mut universe = Universe::new(None, ComboVal::default());
    // combo widening = uncontroversial monotone refinement of the read target
    evolve_all(
        &engine,
        &mut universe,
        "t: { flag: { x: 1 } }\nr: { a: 5 } |> { v: { got: t.flag } }",
    );
    let p = path_of(&["r", "v"]);

    // ACCEPTOR EDIT (the_meter_reads_two, 2026-08-11): a memo hit is detected
    // by the hit counter, not by fuel. Fuel must be identical warm and cold —
    // otherwise cache warmth moves the horizon and the #blur CAID with it.
    let (v1, cold, _) = observe_with_hits(&engine, &universe, &p, 1000);
    let (v2, warm, warm_hits) = observe_with_hits(&engine, &universe, &p, 1000);
    assert_eq!(v1.content_hash(), v2.content_hash(), "stable before evolve");
    assert!(warm_hits > 0, "precondition: entry cached");
    assert_eq!(warm, cold, "a memo hit changed the fuel account");

    // refine the READ coordinate: flag widens {x:1} -> {x:1, y:2}
    evolve_all(&engine, &mut universe, "t: { flag: { y: 2 } }");

    let (v3, _) = fuel_after_observe(&engine, &universe, &p, 1000);
    assert_ne!(v1.content_hash(), v3.content_hash(),
        "RED LINE: evolving a coordinate the thunk READ must invalidate its entry (got identical value {:?})", v3);
}

// R3 (C₀ permanence): a $-free path-free thunk's entry survives any evolve.
#[test]
fn stage5_r3_c0_survives_any_evolve() {
    let engine = Ouroboros::new_in_memory();
    let mut universe = Universe::new(None, ComboVal::default());
    evolve_all(
        &engine,
        &mut universe,
        "r: { a: 1, b: 2 } |> { v: $.a + $.b }",
    );
    let p = path_of(&["r", "v"]);

    // ACCEPTOR EDIT (the_meter_reads_two, 2026-08-11): a memo hit is detected
    // by the hit counter, not by fuel. Fuel must be identical warm and cold —
    // otherwise cache warmth moves the horizon and the #blur CAID with it.
    let (_, cold, _) = observe_with_hits(&engine, &universe, &p, 1000);
    let (_, warm, warm_hits) = observe_with_hits(&engine, &universe, &p, 1000);
    assert!(warm_hits > 0, "precondition: hit established");
    assert_eq!(warm, cold, "a memo hit changed the fuel account");

    for i in 0..3 {
        evolve_all(&engine, &mut universe, &format!("gen_{}: {}", i, i));
    }
    let (_, after, after_hits) = observe_with_hits(&engine, &universe, &p, 1000);
    assert!(
        after_hits > 0,
        "C0 entry (no root reads) must survive arbitrary evolves — the observe \
         served no memo entry"
    );
    assert_eq!(after, cold, "a memo hit changed the fuel account");
}
