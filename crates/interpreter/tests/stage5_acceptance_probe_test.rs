// Stage 5 acceptance probes (2026-07-08, second construction — the first
// used plain path fields, which are evolve-time value copies BY PINNED
// SEMANTICS (C-case: live reads need <<path>>), so they never exercise the
// memo. These constructions use pipe-transformer thunks, which force at
// observe time (verified via dep trace: collector=true records).
//
// P1: a memo HIT must float the entry's deps into the active outer
//     collector. w.y's thunk carries its own captured context ({b:1}), so
//     its key is identical no matter which outer forces it — the second
//     outer builds its entry over a HIT and must inherit dep "t".
// P2: a read resolving through the ROOT prefix fallback (bare `c` → stored
//     `/c`) must record the STORED name, and evolve must invalidate by the
//     STORED name — else the entry never dies.

use nlang_interpreter::value::ComboVal;
use nlang_interpreter::{EvalContext, Ouroboros, Universe, Value};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;

fn path_of(segments: &[&str]) -> Path {
    Path {
        anchor: PathAnchor::Bare,
        segments: segments.iter().map(|s| s.to_string()).collect(),
        span: Span::default(),
    }
}

fn evolve_all(engine: &Ouroboros, universe: &mut Universe, src: &str) {
    let program = parse_program(src).unwrap();
    for field in &program.fields {
        universe
            .evolve(engine, field)
            .unwrap_or_else(|e| panic!("evolve failed for {:?}: {:?}", src, e));
    }
}

fn observe(engine: &Ouroboros, universe: &Universe, path: &Path) -> Value {
    let root = engine.unify(
        Value::Combo(universe.root.clone()),
        Value::Combo(universe.staged.clone()),
    );
    let root_val = match root {
        Value::Combo(r) => r,
        _ => ComboVal::default(),
    };
    let mut ctx = EvalContext::new(root_val).with_fuel(10000);
    let val = engine.resolve_path(path, &mut ctx);
    engine.force_recursive(val, &mut ctx)
}

// P1: transitive deps must survive a memo HIT — via MID-EVAL SOLIDIFICATION.
// A cached value that merely embeds an unforced thunk refreshes lazily (the
// inner entry is invalidated separately). The hit-float matters when the
// inner value is solidified DURING eval — Meet forces its operands (value
// judgment), so the outer entry embeds the inner value SOLID and must
// inherit its deps.
//   w:  {b:1} |> {y: {ref: t.flag}}
//   r1: {a:5} |> {v: w.y & { z: 1 }}   — forces y (MISS): v deps ⊇ {t}
//   r2: {a:5} |> {u: w.y & { z: 2 }}   — forces y (HIT): u must inherit t
#[test]
fn p1_hit_path_must_float_transitive_deps() {
    let engine = Ouroboros::new_in_memory();
    let mut universe = Universe::new(None, ComboVal::default());
    evolve_all(&engine, &mut universe,
        "t: { flag: { x: 1 } }\nw: { b: 1 } |> { y: { ref: t.flag } }\nr1: { a: 5 } |> { v: w.y & { z: 1 } }\nr2: { a: 5 } |> { u: w.y & { z: 2 } }");

    let _ = observe(&engine, &universe, &path_of(&["r1", "v"])); // y MISS, cached
    let u1 = observe(&engine, &universe, &path_of(&["r2", "u"])); // y HIT — deps must float

    evolve_all(&engine, &mut universe, "t: { flag: { y2: 2 } }"); // widen the deep dep

    let u2 = observe(&engine, &universe, &path_of(&["r2", "u"]));
    assert_ne!(u1.content_hash(), u2.content_hash(),
        "RED LINE (transitive deps over HIT, mid-eval solidification): r2.u embeds w.y SOLID via Meet; refining t must invalidate r2.u's entry, got stale {:?}", u2);
}

// P2: prefix-fallback reads must record + invalidate by the STORED name.
#[test]
fn p2_prefix_fallback_read_must_record_dep() {
    let engine = Ouroboros::new_in_memory();
    let mut universe = Universe::new(None, ComboVal::default());
    evolve_all(
        &engine,
        &mut universe,
        "/c: { k: { x: 1 } }\nr: { a: 5 } |> { v: { got: c.k } }",
    );
    let p = path_of(&["r", "v"]);

    let v1 = observe(&engine, &universe, &p);
    let v1b = observe(&engine, &universe, &p);
    assert_eq!(
        v1.content_hash(),
        v1b.content_hash(),
        "stable before refine"
    );

    evolve_all(&engine, &mut universe, "/c: { k: { y: 2 } }"); // widen /c.k

    let v2 = observe(&engine, &universe, &p);
    assert_ne!(v1.content_hash(), v2.content_hash(),
        "RED LINE (prefix fallback): r.v reads /c via bare-name fallback; refining /c must invalidate its entry, got stale {:?}", v2);
}
