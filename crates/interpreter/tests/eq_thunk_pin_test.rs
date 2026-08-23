// eq × thunk forcing — PERMANENT LAW PINS (2026-07-17, acceptor-direct;
// no work order: the queued debt measured HEALED by intervening arcs).
//
// HISTORY: "eq×thunk 強迫語義" was queued when E2 (`x: {k:5, d:k+1}` vs
// `{k:5, d:6}`) measured #false — structural eq compared unforced
// thunks. The lexical-scope arc (soft re-entry forcing), the %id
// force_recursive repair, and the union multiset-equality repair paid
// the debt as side effects. Full battery measured green on v0.2.18+:
// SYNTAX_06 §4 #11/#13 extensional span-blind equality holds through
// lexical fields, deep chains, applied/pipe-produced fields, nesting,
// cocoons, ranges, unions (order-blind), morphism twins.
// These pins freeze the strongest faces so a future laziness change
// cannot silently regress them. Private axis PARTICIPATES in `=`
// (CAID/= six axes unchanged — SPEC_04 §3.1 #4 strips display only).

use nlang_interpreter::{Ouroboros, Universe};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("eqthunk")
}

/// 64 MiB thread — parser/eval recursion headroom (established pattern).
fn observe_nlang(src: &str, path: &str) -> String {
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
                let _ = universe.evolve(&engine, f);
            }
            let p = Path {
                anchor: PathAnchor::Bare,
                segments: path.split('.').map(|s| s.to_string()).collect(),
                span: Span::default(),
            };
            universe.observe(&engine, &p).to_nlang(0)
        })
        .unwrap()
        .join()
        .unwrap()
}

fn assert_obs(src: &str, expect: &str) {
    let got = observe_nlang(src, "out");
    assert_eq!(got, expect, "{src:?} :: out");
}

#[test]
fn pin_eq_forces_lexical_field() {
    // The original E2 face (was #false pre-lexical-arc).
    assert_obs("x: { k: 5, d: k + 1 }\nout: x = { k: 5, d: 6 }", "#true");
}

#[test]
fn pin_eq_forces_lexical_field_negative() {
    // Different forced values must stay distinguishable.
    assert_obs(
        "x: { k: 5, d: k + 1 }\ny: { k: 6, d: k + 1 }\nout: x = y",
        "#false",
    );
}

#[test]
fn pin_eq_forces_deep_chain() {
    assert_obs(
        "x: { k: 5, d: k + 1, e: d + k, g2: e + 1 }\nout: x = { k: 5, d: 6, e: 11, g2: 12 }",
        "#true",
    );
}

#[test]
fn pin_eq_forces_applied_field() {
    assert_obs(
        "f: (n -> n * 2)\np: { v: /f 3 }\nout: p = { v: 6 }",
        "#true",
    );
}

#[test]
fn pin_eq_pipe_produced_combo() {
    assert_obs("g: (n -> { r: n + 1 })\nout: (2 |> g) = { r: 3 }", "#true");
}

#[test]
fn pin_eq_nested_computed() {
    assert_obs("p: { a: { b: 1 + 1 } }\nout: p = { a: { b: 2 } }", "#true");
}

#[test]
fn pin_eq_cocoon_forced() {
    assert_obs(
        "cc1: {{ k: 5, d: k + 1 }}\nout: cc1 = {{ k: 5, d: 6 }}",
        "#true",
    );
}

#[test]
fn pin_eq_morphism_twins_and_differ() {
    assert_obs(
        "f1: { f: (n -> n + 1) }\nf2: { f: (n -> n + 1) }\nout: f1 = f2",
        "#true",
    );
    assert_obs(
        "f1: { f: (n -> n + 1) }\nf3: { f: (n -> n + 2) }\nout: f1 = f3",
        "#false",
    );
}

#[test]
fn pin_eq_private_axis_participates() {
    // Display strips the local axis; `=` and CAID do NOT (six axes law).
    assert_obs("w: { ~z: 1, k: 2 }\nout: w = { k: 2 }", "#false");
}
