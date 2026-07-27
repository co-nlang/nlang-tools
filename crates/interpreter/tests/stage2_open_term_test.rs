// Stage 2 acceptance (handover §3.5):
//   (a) `w: { x: $ }` evolve stores `x` as a Thunk (open term really stored — P3)
//   (b) observing `w.x` → _|_ #no_context (collapse moved from evolve to observe)
//   (c) commit → reload preserves thunk behavior

use nlang_interpreter::{Ouroboros, Universe, EvalContext, Value, ComboVal, EffectTag};
use nlang_parser::parse_program;
use nlang_parser::ast::{FieldKey, AtomKind};
use indexmap::IndexMap;
use std::fs;
use std::path::PathBuf;

fn tmp_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!("nlang-stage2-{}-{}", tag, std::process::id()));
    fs::remove_dir_all(&d).ok();
    fs::create_dir_all(&d).unwrap();
    d
}

// (a) evolve stores open term as Thunk (P3: open terms may be stored)
#[test]
fn evolve_stores_open_term_as_thunk() {
    let dir = tmp_dir("a");
    let engine = Ouroboros::init(&dir).unwrap();
    let mut universe = Universe::new(None, engine.root_with_system());

    // w: { x: $.s } — x is an open term (free $, no enclosing evolution)
    let src = "w: { x: $.s }";
    let program = parse_program(src).unwrap();
    universe.evolve(&engine, &program.fields[0]).unwrap();

    // After evolve, the staged combo's `w` field should contain `x` as a Thunk
    // (not collapsed to _|_ #no_context at evolve time — that's Stage 2's move).
    let w = universe.staged.get_field("w").cloned();
    match w {
        Some(Value::Combo(wcv)) => {
            let x = wcv.get_field("x").cloned();
            match x {
                Some(Value::Thunk { .. }) => {} // ✓ open term stored
                Some(Value::Bottom(d)) => panic!(
                    "x collapsed to {:?} at evolve time — Stage 2 should keep it as Thunk (P3: open terms may be stored)",
                    d),
                other => panic!("expected Thunk for open-term x, got {:?}", other),
            }
        }
        other => panic!("expected w to be a Combo, got {:?}", other),
    }
}

// (b) observing w.x → _|_ #no_context (collapse happens at observe, not evolve)
#[test]
fn observe_open_term_collapses_no_context() {
    let dir = tmp_dir("b");
    let engine = Ouroboros::init(&dir).unwrap();
    let mut universe = Universe::new(None, engine.root_with_system());

    let src = "w: { x: $.s }";
    let program = parse_program(src).unwrap();
    universe.evolve(&engine, &program.fields[0]).unwrap();

    // Observe w.x — should collapse to _|_ #no_context (P3: free $ with no
    // enclosing evolution). The collapse time moved from evolve to observe;
    // the observation result is unchanged.
    let path = nlang_parser::ast::Path {
        anchor: nlang_parser::ast::PathAnchor::Bare,
        segments: vec!["w".to_string(), "x".to_string()],
        span: nlang_parser::ast::Span::default(),
    };
    let obs = universe.observe(&engine, &path);
    match obs {
        Value::Bottom(d) => assert_eq!(
            d.cause, nlang_interpreter::value::BottomCause::NoContext,
            "w.x should collapse to #no_context at observe time, got {:?}", d),
        other => panic!("expected _|_ #no_context, got {:?}", other),
    }
}

// (c) commit → reload preserves thunk behavior (Value derives Serialize)
#[test]
fn commit_reload_preserves_thunk_behavior() {
    let dir = tmp_dir("c");
    let engine = Ouroboros::init(&dir).unwrap();
    let mut universe = Universe::new(None, engine.root_with_system());

    // Use an open-term field (w: { x: $.s }) — x is a Thunk (open term).
    // This tests thunk serialization roundtrip, not pipe-result %val collapse.
    let src = "w: { x: $.s }";
    let program = parse_program(src).unwrap();
    universe.evolve(&engine, &program.fields[0]).unwrap();

    // commit (thunk serialized via Value's derive Serialize)
    let meta = nlang_interpreter::value::CommitMeta {
        message: Some("stage2 test".to_string()),
        timestamp: 0,
        author: Some("test".to_string()),
        abandoned: None,
        privileged_effect: None,
    };
    let _hash = universe.commit(&engine, &dir, meta).unwrap();

    // reload
    let engine2 = Ouroboros::init(&dir).unwrap();
    let universe2 = Universe::load(&engine2, &dir).unwrap();

    // After reload, w.x should still be an open term — observing it yields
    // _|_ #no_context (no enclosing evolution at observe time). The thunk
    // survived the commit→reload roundtrip (Stage 2: open terms may be stored).
    let path = nlang_parser::ast::Path {
        anchor: nlang_parser::ast::PathAnchor::Bare,
        segments: vec!["w".to_string(), "x".to_string()],
        span: nlang_parser::ast::Span::default(),
    };
    let obs = universe2.observe(&engine2, &path);
    match obs {
        Value::Bottom(d) => assert_eq!(
            d.cause, nlang_interpreter::value::BottomCause::NoContext,
            "after reload, w.x should still be #no_context (thunk survived), got {:?}", d),
        other => panic!("after commit→reload, w.x should be _|_ #no_context, got {:?}", other),
    }
}
