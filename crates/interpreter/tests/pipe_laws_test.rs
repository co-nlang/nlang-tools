// ENGINE_SYNC #18: pipe algebraic laws (SPEC_07 §4; docs/discussion/018)
//
// |> is the Kleisli bind of the superposition monad (free join-semilattice-
// with-zero over the value lattice), effect-graded by (Pure<State<IO<NonDet,
// max). Laws encoded here, each verified against the engine:
//
//   additivity   (A|B) |> f  =  (A|>f) | (B|>f)   — branchwise $, ⊥ prunes
//   zero         _|_ |> f    =  _|_
//   identity     x |> (p -> $) = x ;  x |> {} = x
//   composition  chains associate left; result = g(f(x))
//   atomic form  x |> a  =  x & a  (forced intersection, NOT passthrough)

use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{ComboVal, EffectTag, Value};
use nlang_parser::{parse_program, ast::AtomKind};
use indexmap::IndexMap;
use num_bigint::BigInt;

fn eval_one(src: &str) -> Value {
    let program = parse_program(src).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]));
    oo.eval(&program.fields[0].value, &mut ctx)
}

fn int(v: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(v)), EffectTag::Pure, None) }

fn union_ints(v: &Value) -> Vec<i64> {
    match v {
        Value::Union(bs) => bs.iter().map(|b| match b {
            Value::Atom(AtomKind::Int(i), _, _) => i.try_into().unwrap(),
            other => panic!("expected int branch, got {:?}", other),
        }).collect(),
        other => panic!("expected Union, got {:?}", other),
    }
}

// —— additivity (bind distributes over |) ——————————————————————————

#[test]
fn additivity_morphism_form() {
    let v = eval_one("r: (1 | 2) |> (x -> $ + 1)");
    assert_eq!(union_ints(&v), vec![2, 3]);
}

#[test]
fn additivity_transformer_form_branchwise_context() {
    // each branch must get its OWN $ (not the whole superposition)
    let v = eval_one("r: ({k: 1} | {k: 2}) |> { v: $.k }");
    match v {
        Value::Union(bs) => {
            assert_eq!(bs.len(), 2);
            for (i, b) in bs.iter().enumerate() {
                match b {
                    Value::Combo(cv) => assert_eq!(cv.get_field("v"), Some(&int(i as i64 + 1)),
                        "branch {} must see its own $ binding", i),
                    other => panic!("expected Combo branch, got {:?}", other),
                }
            }
        }
        other => panic!("expected Union, got {:?}", other),
    }
}

#[test]
fn additivity_bottom_branch_prunes() {
    // (#a|>#a) | (#b|>#a) = #a | _|_ = #a — ⊥ is the identity of |
    let v = eval_one("r: (#a | #b) |> #a");
    assert_eq!(v, Value::Atom(AtomKind::Tag("a".to_string()), EffectTag::Pure, None));
}

#[test]
fn additivity_all_bottom_collapses() {
    let v = eval_one("r: (1 | 2) |> #a");
    assert!(matches!(v, Value::Bottom(_)), "all branches ⊥ must collapse, got {:?}", v);
}

// —— zero ——————————————————————————————————————————————————————————

#[test]
fn zero_absorbs() {
    let v = eval_one("r: _|_ |> (x -> $ + 1)");
    let is_bottom = matches!(v, Value::Bottom(_)) || matches!(v, Value::Atom(AtomKind::Bottom, _, _));
    assert!(is_bottom, "⊥ |> f must be ⊥, got {:?}", v);
}

// —— identity ———————————————————————————————————————————————————————

#[test]
fn identity_morphism() {
    assert_eq!(eval_one("r: 5 |> (x -> $)").collapse().clone(), int(5));
}

#[test]
fn identity_empty_transformer() {
    // {} is the unit of the refinement embedding: x & {} = x
    let v = eval_one("r: { a: 1 } |> { }");
    match v {
        Value::Combo(cv) => assert_eq!(cv.get_field("a"), Some(&int(1))),
        other => panic!("expected Combo, got {:?}", other),
    }
}

// —— composition ————————————————————————————————————————————————————

#[test]
fn chain_composes_left() {
    assert_eq!(eval_one("r: 1 |> (x -> $ + 1) |> (x -> $ * 2)").collapse().clone(), int(4));
}

// —— atomic collapse (form 3 = constant refinement) —————————————————

#[test]
fn atomic_form_intersects_compatible() {
    let v = eval_one("r: #ok |> #ok");
    assert_eq!(v, Value::Atom(AtomKind::Tag("ok".to_string()), EffectTag::Pure, None));
}

#[test]
fn atomic_form_conflicts_incompatible() {
    // was a passthrough returning #ok — forced intersection per SPEC_07 §4.1
    let v = eval_one("r: 5 |> #ok");
    assert!(matches!(v, Value::Bottom(_)), "5 & #ok must be ⊥, got {:?}", v);
}
