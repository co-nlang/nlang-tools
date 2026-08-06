// G5 tuple-param destructure probes (2026-07-12, pre-committed by work
// order — docs/tuple_destructure_handover.md).
//
// SYNTAX_11 rule 4: `((x, y) -> …)` is ONE tuple parameter (positional
// destructure), strictly dual to curried `(x y -> …)`. Measured today:
// the param parses correctly as Morphism(Tuple([x,y]), body), but eval
// packaging degrades the rule key to "_" and never binds x/y — every
// application form returns Top (`_`). Dispatch-side destructure is
// UNIMPLEMENTED (G5, exposed during the G2 re-diagnosis).
//
// RULING (adjudicated 2026-07-12):
//   Packaging: Tuple param whose elements are ALL bare single-segment
//   paths → single rule carrying `%params` metadata (index → name);
//   other tuple shapes (nested tuples, non-path elements, tuple-mixed-
//   with-curry `((x,y) z -> …)`) keep current behavior — strict gate,
//   寧漏勿誤.
//   Binding: on application, the argument must be a tuple-shaped combo
//   with positional fields 0..k-1 and EXACT arity k; bind each name,
//   keep the existing `it`/`$` (whole argument) bindings. Non-tuple
//   argument or arity mismatch → ⊥ #conflict (destructure failure).
//   Curry-vs-tuple duality (SYNTAX_09 §2) must survive: a curried
//   morphism applied to one tuple binds the WHOLE tuple to its first
//   param (pinned below) — no implicit cross-over in either direction.

use nlang_interpreter::{Ouroboros, Universe, Value};
use nlang_parser::ast::{Path, PathAnchor, Span};
use nlang_parser::parse_program;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir() -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new("g5probe")
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
            let mut universe = Universe::new(None, engine.root_with_system());
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

// ─────────────────────────────────────────────────────────────────────────
// RED GATES — destructure must fire in every application form
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn red_tuple_destructure_juxta() {
    // today: "_" (x/y never bound)
    assert_obs("tf: ((x, y) -> x + y)\nout: tf (3, 5)", "8");
}

#[test]
fn red_tuple_destructure_inline() {
    assert_obs("out: ((x, y) -> x * 10 + y) (3, 5)", "35");
}

#[test]
fn red_tuple_destructure_pipe() {
    assert_obs("tf: ((x, y) -> x + y)\nout: (3, 5) |> tf", "8");
}

#[test]
fn red_tuple_destructure_three() {
    assert_obs("tf: ((a, b, c) -> a + b + c)\nout: tf (1, 2, 3)", "6");
}

#[test]
fn red_tuple_destructure_slash_def() {
    assert_obs("/tfirst: ((x, y) -> x)\nout: tfirst (1, 2)", "1");
}

#[test]
fn red_tuple_body_sees_context_whole() {
    // $ (whole argument) and destructured names coexist
    assert_obs("tf: ((x, y) -> $.0 + y)\nout: tf (3, 5)", "8");
}

#[test]
fn red_tuple_arity_mismatch_bottom() {
    // 2-param tuple, 3-tuple argument — exact arity, no partial destructure
    assert_obs(
        "tf: ((x, y) -> x + y)\nout: tf (1, 2, 3)",
        "_|_ (%cause: #conflict)",
    );
}

#[test]
fn red_tuple_nontuple_arg_bottom() {
    // definition side chose tuple → application side must use tuple
    assert_obs(
        "tf: ((x, y) -> x + y)\nout: tf 5",
        "_|_ (%cause: #conflict)",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// ACTIVE pins — adjacent semantics that must SURVIVE
// ─────────────────────────────────────────────────────────────────────────

#[test] // ACTIVE pin: single-param morphism + positional navigation
fn pin_single_param_tuple_nav() {
    assert_obs("sp: (t -> t.0 + t.1)\nout: (3, 5) |> sp", "8");
}

#[test] // ACTIVE pin: L2-04 conformance shape ($-navigation on tuple)
fn pin_context_tuple_nav() {
    assert_obs("out: (1, 2) |> (p -> $.0 + $.1)", "3");
}

#[test] // ACTIVE pin (curry-vs-tuple STRICT DUAL): a curried morphism
        // applied to one tuple binds the WHOLE tuple to its first param —
        // it must NOT silently destructure
fn pin_curry_applied_to_tuple_binds_whole() {
    assert_obs("out: (x y -> y) (3, 5) 9", "9");
}

#[test] // ACTIVE pin: dual's other face — first param really is the tuple
fn pin_curry_applied_to_tuple_first_is_tuple() {
    assert_obs("out: ((x y -> x) (3, 5) 9).1", "5");
}

#[test] // ACTIVE pin: tuple value shape + navigation unchanged
fn pin_tuple_value_nav() {
    assert_obs("t: (3, 5)\nout: t.0", "3");
}

#[test] // ACTIVE pin: single-param morphism unaffected
fn pin_single_param_morphism() {
    assert_obs("inc: (x -> x + 1)\nout: inc 4", "5");
}

#[test] // ACTIVE pin: multiparam auto-curry (G2-M) unaffected
fn pin_multiparam_still_curries() {
    assert_obs("m: x y -> x + y\nout: m 3 5", "8");
}
