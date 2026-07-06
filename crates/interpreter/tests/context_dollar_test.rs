// ENGINE_SYNC #16: `$` semantics P1-P5 (SPEC_07 §4.2 rewrite, 2026-07-05; SYNTAX_12 §2.4)
//
// Implemented on the current eager engine:
//   P1 rebind only at evolution boundaries (pipe / morphism application);
//      bare combos never rebind
//   P2 pipes opaque — inner pipe binding cannot leak in or out
//   P3 free `$` observed without an enclosing evolution -> _|_ #no_context
//   P4 `$` only means the input; tuple input via $.0 / $.1
//   P5 interpolation is transparent — `${$}` sees the same binding
//
// Known gap (lazy engine, GUIDE_03): open terms are collapsed at evolve time,
// so the Ouroboros vector `v: <<_.>> |> <<_.>>` / `v.w.x.a` is not yet realizable.

use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{ComboVal, EffectTag, Value, BottomCause};
use nlang_parser::{parse_program, ast::AtomKind};
use indexmap::IndexMap;
use num_bigint::BigInt;

fn eval_one(src: &str) -> Value {
    let program = parse_program(src).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]));
    oo.eval(&program.fields[0].value, &mut ctx)
}

fn field_of(v: &Value, key: &str) -> Value {
    match v {
        Value::Combo(cv) => cv.get_field(key).cloned().unwrap_or(Value::Top),
        _ => panic!("expected Combo, got {:?}", v),
    }
}

fn int(v: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(v)), EffectTag::Pure, None) }

// P1: pipe binds $ to the input — canonical vector first half (SPEC_07 §4.2)
#[test]
fn p1_pipe_binds_input() {
    let v = eval_one(r#"s: "Logic" |> { a: $ }"#);
    assert_eq!(field_of(&v, "a"), Value::Atom(AtomKind::Str("Logic".to_string()), EffectTag::Pure, None));
}

// P1: bare combos never rebind — inner combo still sees the pipe input
#[test]
fn p1_bare_combo_never_rebinds() {
    let v = eval_one("r: 7 |> { o: { i: $ } }");
    assert_eq!(field_of(&field_of(&v, "o"), "i"), int(7));
}

// P1: morphism application is an evolution boundary — $ = the argument
#[test]
fn p1_application_binds() {
    let v = eval_one("r: 41 |> (x -> $ + 1)");
    assert_eq!(v.collapse().clone(), int(42));
}

// P2: pipes are opaque — inner pipe rebinds locally, outer binding unaffected
#[test]
fn p2_nested_pipe_opaque() {
    let v = eval_one("r: 1 |> { a: $, b: 2 |> { c: $ } }");
    assert_eq!(field_of(&v, "a"), int(1));
    assert_eq!(field_of(&field_of(&v, "b"), "c"), int(2));
}

// P2: field order irrelevant — a after the inner pipe still sees the outer $
#[test]
fn p2_no_leak_after_inner_pipe() {
    let v = eval_one("r: 1 |> { b: 2 |> { c: $ }, a: $ }");
    assert_eq!(field_of(&v, "a"), int(1));
}

// P3: free $ with no enclosing evolution -> _|_ %cause #no_context
#[test]
fn p3_free_context_collapses_no_context() {
    let v = eval_one("w: { x: $ }");
    match field_of(&v, "x") {
        Value::Bottom(d) => assert_eq!(d.cause, BottomCause::NoContext),
        other => panic!("expected _|_ #no_context, got {:?}", other),
    }
}

// P3: same for a navigating free $ ($.s)
#[test]
fn p3_free_context_navigation() {
    let v = eval_one("w: { x: $.s }");
    match field_of(&v, "x") {
        Value::Bottom(d) => assert_eq!(d.cause, BottomCause::NoContext),
        other => panic!("expected _|_ #no_context, got {:?}", other),
    }
}

// P4: tuple input is addressed positionally via $.0 / $.1.
// Note: tuple |> {combo-transformer} correctly collapses (_|_ #missing_key) —
// tuples are sealed numeric cocoons and structural evolution may not add fields;
// positional input therefore pairs with morphism evolution.
#[test]
fn p4_tuple_positional_input() {
    let v = eval_one("t: (1, 2) |> (p -> $.0 + $.1)");
    assert_eq!(v.collapse().clone(), int(3));
}

// P4 corollary: the sealed tuple rejects structural field addition
#[test]
fn p4_tuple_sealed_against_structural_add() {
    let v = eval_one("t: (1, 2) |> { s: $.0 + $.1 }");
    assert!(matches!(v, Value::Bottom(_)), "sealed tuple must reject new field, got {:?}", v);
}

// P5: interpolation does not open a scope — ${$} is the pipe input
#[test]
fn p5_interpolation_transparent() {
    let v = eval_one("m: 5 |> { s: `v=${$}` }");
    match field_of(&v, "s") {
        Value::Atom(AtomKind::Str(s), _, _) => assert_eq!(s, "v=5"),
        other => panic!("expected string, got {:?}", other),
    }
}
