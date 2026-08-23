// L2-17 divergence detection + ⊥ %cause printing probes (2026-07-11 —
// docs/divergence_cause_handover.md; conformance L2-17).
//
// Ruling: same thunk / public coordinate re-entered during force →
// ⊥ #divergent (before stack/fuel). Undefined names stay `_` (open world).
// Productive recursion (shrinking-argument morphism) is the load-bearing pin.
// ⊥ display: `_|_ (%cause: #<tag>)`; bn_serial untouched.

use nlang_interpreter::value::BottomCause;
use nlang_interpreter::{Ouroboros, Universe, Value};
use nlang_parser::ast::{AtomKind, Path, PathAnchor, Span};
use nlang_parser::parse_program;
use num_bigint::BigInt;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("l217")
}

/// Runs on a dedicated 64 MiB thread — debug test threads (2 MiB) are too
/// small for eval recursion (same approach as the engine's parser threads).
fn run_observe(src: &str, path: &str) -> Result<Value, String> {
    let src = src.to_string();
    let path = path.to_string();
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let dir = tmp_dir();
            let engine = Ouroboros::init(&dir).unwrap();
            let mut universe = Universe::new_with_standard(
                None,
                engine.root_with_system(),
                engine.root_with_system(),
            );
            let program = parse_program(&src).unwrap();
            for f in &program.fields {
                universe
                    .evolve(&engine, f)
                    .map_err(|e| format!("evolve: {e:?}"))?;
            }
            let p = Path {
                anchor: PathAnchor::Bare,
                segments: path.split('.').map(|s| s.to_string()).collect(),
                span: Span::default(),
            };
            Ok(universe.observe(&engine, &p))
        })
        .unwrap()
        .join()
        .map_err(|_| "observation thread panicked/aborted".to_string())?
}

fn assert_divergent(src: &str, path: &str) {
    match run_observe(src, path) {
        Ok(Value::Bottom(d)) => assert_eq!(
            d.cause,
            BottomCause::Divergent,
            "{src:?} :: {path} must be #divergent, got cause {:?}",
            d.cause
        ),
        Err(e) if e.contains("Divergent") => {}
        other => panic!("{src:?} :: {path} must be _|_ #divergent, got {other:?}"),
    }
}

fn assert_obs_int(src: &str, path: &str, expect: i64) {
    match run_observe(src, path) {
        Ok(Value::Atom(AtomKind::Int(n), _, _)) => {
            assert_eq!(n, BigInt::from(expect), "{src:?} :: {path}")
        }
        other => panic!("{src:?} :: {path} must be {expect}, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// RED LINES — self-reference today observes to `_` (claims-everything)
// ─────────────────────────────────────────────────────────────────────────

#[test]
// RED LINE: conformance L2-17 — `a: a + 1` is a cycle, not ⊤
fn l217_self_arith_divergent() {
    assert_divergent("a: a + 1\nout: a", "out");
}

// MIGRATED (2026-07-16, by the acceptor, in the static-cycle order
// commit): `l217_self_identity_divergent` pinned `x: x` → ⊥ #divergent,
// but that is a PURE-REFERENCE cycle — SPEC_12 §1.1 static cycle → Top
// (solution set = everything; L2-17's canon vector is `a: a+1`, a
// TRANSFORM cycle, which stays ⊥). Successor gate:
// red_root_static_self_top in static_cycle_probe_test.rs.

#[test]
// RED LINE: mutual cycle through two coordinates
fn l217_mutual_cycle_divergent() {
    assert_divergent("a: b + 1\nb: a + 1\nout: a", "out");
}

// MIGRATED (2026-07-16, by the acceptor): `l217_path_cycle_divergent`
// pinned `s: { v: s.v }` → ⊥, but a pure-PATH loop is still a pure
// reference (projection adds no information) — SPEC_12 §1.1 as amended
// classifies it static → Top. Successor gate:
// red_path_pure_cycle_top in static_cycle_probe_test.rs.

#[test]
// RED LINE: ⊥ canonical display carries %cause tag (Blur-precedent
// format `(%cause: #<tag>)`); bn_serial bytes must NOT change
fn bottom_display_carries_cause_tag() {
    let v: Value = BottomCause::Divergent.into();
    let s = v.to_nlang(0);
    assert!(
        s.contains("#divergent"),
        "to_nlang of divergent ⊥ must contain #divergent, got {s:?}"
    );
    let v2: Value = BottomCause::NoContext.into();
    assert!(v2.to_nlang(0).contains("#no_context"));
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE pins — the boundary the detector must NOT cross
// ─────────────────────────────────────────────────────────────────────────

#[test] // ACTIVE pin (LOAD-BEARING): productive recursion works today (120)
        // and must survive cycle detection — shrinking-argument morphism
        // recursion is NOT a same-thunk re-entry
fn pin_productive_recursion_factorial() {
    assert_obs_int(
        "f: (n -> n <= 1 ? 1 : n * (/f (n - 1)))\nout: /f 5",
        "out",
        120,
    );
}

#[test] // ACTIVE pin: recursive TYPE defs terminate (E4 arc; fields lazy)
fn pin_recursive_type_still_terminates() {
    assert_obs_int(
        "@Tree: { v: @int, next: @Tree | () }\nt: { v: 1, next: () } & @Tree\nout: t.v",
        "out",
        1,
    );
}

#[test] // ACTIVE pin: UNDEFINED name stays `_` (open world) — cycle detection
        // must key on in-flight thunks, not on lookup misses
fn pin_undefined_name_stays_top() {
    match run_observe("out: zzz_undefined", "out") {
        Ok(Value::Top) | Ok(Value::Atom(AtomKind::Top, _, _)) => {}
        other => panic!("undefined name must stay ⊤ (open world), got {other:?}"),
    }
}

#[test] // ACTIVE pin: free-$ observation is ⊥ #no_context (P3) — untouched
fn pin_free_context_no_context() {
    match run_observe("w: { x: $.s }\nout: w.x", "out") {
        Ok(Value::Bottom(d)) => assert_eq!(d.cause, BottomCause::NoContext),
        other => panic!("free $ must be ⊥ #no_context, got {other:?}"),
    }
}

#[test] // ACTIVE pin: runaway morphism recursion must TERMINATE (not hang).
        // G3: default Blur strategy yields first-class #blur #fuel_exhausted
        // (not ⊥ #conflict); Strict would be ⊥ #fuel_exhausted. Intent of the
        // pin is non-hanging terminal, not a specific lattice bottom.
fn pin_runaway_morphism_bottoms() {
    match run_observe("g: (n -> /g n)\nout: /g 1", "out") {
        Ok(Value::Bottom(_)) | Ok(Value::Blur(_)) | Err(_) => {}
        other => panic!("runaway recursion must terminate (⊥ or #blur), got {other:?}"),
    }
}
