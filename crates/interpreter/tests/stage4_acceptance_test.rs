// Stage 4 acceptance probes: observation memo (handover §4, 2026-07-07)
//
// Verifies:
//   1. Same observe twice → second fuel strictly less (memo hit), CAID equal
//   2. Evolve between observes → memo miss, new root reflected
//   3. Q-tier expr → no cross-observation memo (fuel not reduced)
//   4. Blur/Bottom/NonDet not inserted into force_memo

use nlang_interpreter::value::{ComboVal, EffectTag};
use nlang_interpreter::{EvalContext, Ouroboros, Universe, Value};
use nlang_parser::{
    ast::{AtomKind, Path, PathAnchor, Span},
    parse_program,
};

fn path_of(segments: &[&str]) -> Path {
    Path {
        anchor: PathAnchor::Bare,
        segments: segments.iter().map(|s| s.to_string()).collect(),
        span: Span::default(),
    }
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

// Probe 1: same path observed twice — second fuel strictly less, same CAID
#[test]
fn stage4_memo_reduces_fuel_on_second_observe() {
    let (engine, universe) = new_oo_and_universe("r: { a: 1, b: 2 } |> { v: $.a + $.b }");
    let p = path_of(&["r", "v"]);

    let (v1, fuel1) = fuel_after_observe(&engine, &universe, &p, 1000);
    assert_eq!(
        v1,
        Value::Atom(AtomKind::Int(3.into()), EffectTag::Pure, None)
    );

    let (v2, fuel2, hits) = observe_with_hits(&engine, &universe, &p, 1000);
    assert_eq!(
        v1.content_hash(),
        v2.content_hash(),
        "memo hit must return same value"
    );
    // ACCEPTOR EDIT (the_meter_reads_two, 2026-08-11). This asserted
    // `fuel2 > fuel1` — the memo had to be CHEAPER. That is now forbidden, and
    // the two assertions below say why in the right order:
    assert!(
        hits > 0,
        "no memo entry was served on the second observe — the memo is not \
         working, and the fuel assertion below would be vacuous"
    );
    assert_eq!(
        fuel2, fuel1,
        "a memo hit changed the fuel account (cold={fuel1}, warm={fuel2}) — \
         cache warmth would then move the horizon, and with it the #blur CHS \
         and its CAID: the same program would address differently depending on \
         whether it had been observed before"
    );
}

// Probe 2: Q-tier not memo'd across observations
#[test]
fn stage4_qtier_no_cross_observation_memo() {
    let (engine, universe) = new_oo_and_universe("r: { k: 1 } |> { v: $.k == 1 }");
    let p = path_of(&["r", "v"]);

    let (v1, fuel1) = fuel_after_observe(&engine, &universe, &p, 1000);
    let (v2, fuel2) = fuel_after_observe(&engine, &universe, &p, 1000);
    assert_eq!(v1.content_hash(), v2.content_hash(), "same value");
    assert!(
        (fuel1 as i64 - fuel2 as i64).abs() < 20,
        "Q-tier should NOT memo across observations: fuel1={}, fuel2={}",
        fuel1,
        fuel2
    );
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
