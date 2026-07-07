// Atom(Top) unify probes (2026-07-08, pre-committed by work order —
// docs/atom_top_unify_handover.md). Top is the lattice identity: `x & _ = x`
// for every x. The engine has TWO spellings of Top — the Value::Top variant
// and Atom(AtomKind::Top) (what the literal `_` evaluates to) — and unify's
// identity arms only match the former, so the law breaks for the literal.
//
// Acceptance = remove the #[ignore]s, everything green, no other suite breaks.

use nlang_interpreter::{Ouroboros, Universe, EvalContext, Value};
use nlang_interpreter::value::{ComboVal, EffectTag};
use nlang_parser::parse_program;
use nlang_parser::ast::{AtomKind, Path, PathAnchor, Span};
use indexmap::IndexMap;
use num_bigint::BigInt;

fn eval_one(src: &str) -> Value {
    let program = parse_program(src).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]));
    oo.eval_observed(&program.fields[0].value, &mut ctx)
}

// Law: `_ & x = x` at the literal level.
#[test]
#[ignore = "Atom(Top) unify bug: un-ignore = acceptance (baseline 2026-07-08: Conflict via do_unify atom-mismatch arm)"]
fn top_literal_meet_is_identity() {
    let v = eval_one("r: _ & 5");
    match &v {
        Value::Atom(AtomKind::Int(n), _, _) => assert_eq!(n, &BigInt::from(5)),
        other => panic!("`_ & 5` must be 5 (Top = meet identity), got {:?}", other),
    }
}

// Law: unify(Atom(Top), x) = x — engine-level, both operand orders.
#[test]
#[ignore = "Atom(Top) unify bug: un-ignore = acceptance (baseline 2026-07-08: Conflict via do_unify atom-mismatch arm)"]
fn engine_unify_atom_top_is_identity() {
    let oo = Ouroboros::new_in_memory();
    let top = Value::Atom(AtomKind::Top, EffectTag::Pure, None);
    let five = Value::Atom(AtomKind::Int(5.into()), EffectTag::Pure, None);
    let r1 = oo.unify(top.clone(), five.clone());
    let r2 = oo.unify(five.clone(), top);
    assert_eq!(r1.content_hash(), five.content_hash(), "unify(_, 5) must be 5, got {:?}", r1);
    assert_eq!(r2.content_hash(), five.content_hash(), "unify(5, _) must be 5, got {:?}", r2);
}

// The original field-evolution repro: declaring a coordinate at Top and
// refining it later is the canonical monotone evolution — it must not Conflict.
#[test]
#[ignore = "Atom(Top) unify bug: un-ignore = acceptance (baseline 2026-07-08: Conflict via do_unify atom-mismatch arm)"]
fn evolve_can_refine_from_top_literal() {
    let engine = Ouroboros::new_in_memory();
    let mut universe = Universe::new(None, ComboVal::default());
    let p1 = parse_program("t: { flag: _ }").unwrap();
    universe.evolve(&engine, &p1.fields[0]).unwrap();
    let p2 = parse_program("t: { flag: 2 }").unwrap();
    universe.evolve(&engine, &p2.fields[0])
        .unwrap_or_else(|e| panic!("refining flag from `_` (Top) to 2 is monotone and must succeed, got {:?}", e));

    let path = Path { anchor: PathAnchor::Bare, segments: vec!["t".into(), "flag".into()], span: Span::default() };
    let obs = universe.observe(&engine, &path);
    match &obs {
        Value::Atom(AtomKind::Int(n), _, _) => assert_eq!(n, &BigInt::from(2)),
        other => panic!("t.flag after refine must be 2, got {:?}", other),
    }
}
