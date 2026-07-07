// Stage 5 red-line probes — WRITTEN BY THE WORK ORDER (2026-07-08).
// Acceptance = remove the #[ignore] attributes and everything is green.
// Deleting or weakening a probe = work-order violation, escalate instead.
//
// R1: Route B point — memo survives evolve of an UNRELATED coordinate.
// R2: no-regression — evolve of a RELATED (read) coordinate still invalidates.
// R3: C₀ (path-free, $-free content) survives ANY evolve (permanent tier).

use nlang_interpreter::{Ouroboros, Universe, EvalContext, Value};
use nlang_interpreter::value::ComboVal;
use nlang_parser::parse_program;
use nlang_parser::ast::{Path, PathAnchor, Span, AtomKind};

fn path_of(segments: &[&str]) -> Path {
    Path { anchor: PathAnchor::Bare, segments: segments.iter().map(|s| s.to_string()).collect(), span: Span::default() }
}

fn evolve_all(engine: &Ouroboros, universe: &mut Universe, src: &str) {
    let program = parse_program(src).unwrap_or_else(|e| panic!("parse failed for {:?}: {:?}", src, e));
    for field in &program.fields {
        universe.evolve(engine, field).unwrap_or_else(|e| panic!("evolve failed for {:?}: {:?}", src, e));
    }
}

fn fuel_after_observe(engine: &Ouroboros, universe: &Universe, path: &Path, initial_fuel: u64) -> (Value, u64) {
    let root = engine.unify(Value::Combo(universe.root.clone()), Value::Combo(universe.staged.clone()));
    let root_val = match root { Value::Combo(r) => r, _ => ComboVal::default() };
    let mut ctx = EvalContext::new(root_val).with_fuel(initial_fuel);
    let val = engine.resolve_path(path, &mut ctx);
    let result = engine.force_recursive(val, &mut ctx);
    (result, ctx.fuel)
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
    evolve_all(&engine, &mut universe, "r: { a: 1, b: 2 } |> { v: $.a + $.b }");
    let p = path_of(&["r", "v"]);

    let (v1, cold) = fuel_after_observe(&engine, &universe, &p, 1000);
    assert_eq!(int_of(&v1), Some(3));
    let (_, warm) = fuel_after_observe(&engine, &universe, &p, 1000);
    assert!(warm > cold, "precondition: memo hit before evolve (cold={}, warm={})", cold, warm);

    // unrelated coordinate
    evolve_all(&engine, &mut universe, "zzz_unrelated: 9");

    let (v3, after) = fuel_after_observe(&engine, &universe, &p, 1000);
    assert_eq!(int_of(&v3), Some(3));
    assert!(after > cold,
        "RED LINE (Route B): evolving an unrelated coordinate must not wipe the entry — fuel should still show a hit (cold={}, after={})", cold, after);
}

// R2 (no-regression): a thunk whose eval READS a root coordinate must be
// invalidated when that coordinate evolves. `t.flag` is read through the
// root; after re-evolving t, the observed value must reflect the new root.
#[test]
fn stage5_r2_related_evolve_still_invalidates() {
    let engine = Ouroboros::new_in_memory();
    let mut universe = Universe::new(None, ComboVal::default());
    // combo widening = uncontroversial monotone refinement of the read target
    evolve_all(&engine, &mut universe, "t: { flag: { x: 1 } }\nr: { a: 5 } |> { v: { got: t.flag } }");
    let p = path_of(&["r", "v"]);

    let (v1, cold) = fuel_after_observe(&engine, &universe, &p, 1000);
    let (v2, warm) = fuel_after_observe(&engine, &universe, &p, 1000);
    assert_eq!(v1.content_hash(), v2.content_hash(), "stable before evolve");
    assert!(warm > cold, "precondition: entry cached (cold={}, warm={})", cold, warm);

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
    evolve_all(&engine, &mut universe, "r: { a: 1, b: 2 } |> { v: $.a + $.b }");
    let p = path_of(&["r", "v"]);

    let (_, cold) = fuel_after_observe(&engine, &universe, &p, 1000);
    let (_, warm) = fuel_after_observe(&engine, &universe, &p, 1000);
    assert!(warm > cold, "precondition: hit established");

    for i in 0..3 {
        evolve_all(&engine, &mut universe, &format!("gen_{}: {}", i, i));
    }
    let (_, after) = fuel_after_observe(&engine, &universe, &p, 1000);
    assert!(after > cold,
        "C0 entry (no root reads) must survive arbitrary evolves (cold={}, after={})", cold, after);
}
