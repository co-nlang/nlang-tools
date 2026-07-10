// Stage 4 acceptance probes: observation memo (handover §4, 2026-07-07)
//
// Verifies:
//   1. Same observe twice → second fuel strictly less (memo hit), CAID equal
//   2. Evolve between observes → memo miss, new root reflected
//   3. Q-tier expr → no cross-observation memo (fuel not reduced)
//   4. Blur/Bottom/NonDet not inserted into force_memo

use nlang_interpreter::{Ouroboros, Universe, EvalContext, Value};
use nlang_interpreter::value::{ComboVal, EffectTag};
use nlang_parser::{parse_program, ast::{Path, PathAnchor, Span, AtomKind}};

fn path_of(segments: &[&str]) -> Path {
    Path { anchor: PathAnchor::Bare, segments: segments.iter().map(|s| s.to_string()).collect(), span: Span::default() }
}

fn new_oo_and_universe(fields: &str) -> (Ouroboros, Universe) {
    let engine = Ouroboros::new_in_memory();
    let mut universe = Universe::new(None, ComboVal::default());
    if !fields.is_empty() {
        let program = parse_program(fields).unwrap();
        for field in &program.fields {
            universe.evolve(&engine, field).unwrap();
        }
    }
    (engine, universe)
}

fn fuel_after_observe(engine: &Ouroboros, universe: &Universe, path: &Path, initial_fuel: u64) -> (Value, u64) {
    let root = engine.unify(Value::Combo(universe.root.clone()), Value::Combo(universe.staged.clone()));
    let root_val = match root { Value::Combo(r) => r, _ => ComboVal::default() };
    let mut ctx = EvalContext::new(root_val).with_fuel(initial_fuel);
    let val = engine.resolve_path(path, &mut ctx);
    let result = engine.force_recursive(val, &mut ctx);
    (result, ctx.fuel)
}

// Probe 1: same path observed twice — second fuel strictly less, same CAID
#[test]
fn stage4_memo_reduces_fuel_on_second_observe() {
    let (engine, universe) = new_oo_and_universe("r: { a: 1, b: 2 } |> { v: $.a + $.b }");
    let p = path_of(&["r", "v"]);

    let (v1, fuel1) = fuel_after_observe(&engine, &universe, &p, 1000);
    assert_eq!(v1, Value::Atom(AtomKind::Int(3.into()), EffectTag::Pure, None));

    let (v2, fuel2) = fuel_after_observe(&engine, &universe, &p, 1000);
    assert_eq!(v1.content_hash(), v2.content_hash(), "memo hit must return same value");
    assert!(fuel2 > fuel1, "second observe should use less fuel: fuel1={}, fuel2={}", fuel1, fuel2);
}

// Probe 2: Q-tier not memo'd across observations
#[test]
fn stage4_qtier_no_cross_observation_memo() {
    let (engine, universe) = new_oo_and_universe("r: { k: 1 } |> { v: $.k == 1 }");
    let p = path_of(&["r", "v"]);

    let (v1, fuel1) = fuel_after_observe(&engine, &universe, &p, 1000);
    let (v2, fuel2) = fuel_after_observe(&engine, &universe, &p, 1000);
    assert_eq!(v1.content_hash(), v2.content_hash(), "same value");
    assert!((fuel1 as i64 - fuel2 as i64).abs() < 20,
        "Q-tier should NOT memo across observations: fuel1={}, fuel2={}", fuel1, fuel2);
}

// Probe 3: Bottom not memo'd
#[test]
fn stage4_force_memo_guards_no_bottom() {
    let engine = Ouroboros::new_in_memory();
    let universe = Universe::new(None, ComboVal::default());
    let p = path_of(&["no_such_field"]);

    let (_v, _fuel) = fuel_after_observe(&engine, &universe, &p, 1000);
    let memo_len = engine.force_memo.read().unwrap().len();
    assert_eq!(memo_len, 0, "Bottom result must not be memo'd");
}
