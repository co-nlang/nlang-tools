// Stage 3 acceptance: call-by-observation binding propagation (handover §4, 2026-07-07)
//
// C option (live reference late binding): <<_.>> evaluates to a symbolic Ref,
// pipe with Ref stays as thunk at evolve, force at observe when root contains v.
//
// Full vector (GUIDE_03 §11.1 / handover §4):
//   s: "Logic" |> { a: $ }     ;; observe s.a = "Logic" (3-pre)
//   w: { x: $.s }               ;; observe w.x → _|_ #no_context (Stage 2)
//   v: <<_.>> |> <<_.>>         ;; evolve stores thunk, observe resolves late
//   _: v                        ;; full → #fuel_exhausted (self-referential)

use indexmap::IndexMap;
use nlang_interpreter::value::{BottomCause, ComboVal, EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::{ast::AtomKind, parse_program};

fn eval_one(src: &str) -> Value {
    let program = parse_program(src).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(
        IndexMap::new(),
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));
    oo.eval_observed(&program.fields[0].value, &mut ctx)
}

fn field_of(v: &Value, key: &str) -> Value {
    match v {
        Value::Combo(cv) => cv.get_field(key).cloned().unwrap_or(Value::Top),
        _ => panic!("expected Combo, got {:?}", v),
    }
}

#[test]
fn stage3_pre_pipe_field_navigation() {
    let v = eval_one("s: \"Logic\" |> { a: $ }");
    let a_val = field_of(&v, "a");
    match &a_val {
        Value::Atom(AtomKind::Str(s), _, _) => assert_eq!(s, "Logic"),
        other => panic!("expected Atom(Str(\"Logic\")), got {:?}", other),
    }
}

#[test]
fn stage2_open_term_no_context() {
    let program = parse_program("w: { x: $.s }").unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(
        IndexMap::new(),
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));
    let result = oo.eval_observed(&program.fields[0].value, &mut ctx);
    let x_val = field_of(&result, "x");
    let is_no_context = matches!(&x_val, Value::Bottom(d) if matches!(d.cause, BottomCause::NoContext))
        || matches!(&x_val, Value::Atom(AtomKind::Bottom, _, _));
    assert!(
        is_no_context,
        "free $ should collapse to _|_ #no_context, got {:?}",
        x_val
    );
}

#[test]
fn stage3_ref_structural_creates_ref() {
    let program = parse_program("x: <<_.>>").unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(ComboVal::new(
        IndexMap::new(),
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));
    let result = oo.eval(&program.fields[0].value, &mut ctx);
    match &result {
        Value::Ref(_) => {}
        other => panic!("<<_.>> should produce Ref, got {:?}", other),
    }
}

#[test]
fn stage3_pipe_with_ref_late_binding() {
    // v: <<_.>> |> <<_.>> — late binding: the references are read when the
    // pipe is evaluated (observed), against the root as it is then — not
    // snapshotted when the field is defined. Q-089 (SPEC_07 §4.2 P1): the
    // engine may return the reference itself, or read it at this
    // application so the pipe's input binds `$`; either way the result is
    // the root of this context. (The stored field staying live is
    // stage3_probe_test::probe_evolve_stores_live_ref_not_snapshot.)
    let src = "v: <<_.>> |> <<_.>>";
    let program = parse_program(src).unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut fields = IndexMap::new();
    fields.insert(
        "z".to_string(),
        Value::Atom(nlang_parser::ast::AtomKind::Int(1.into()), EffectTag::Pure, None),
    );
    let mut ctx = EvalContext::new(ComboVal::new(
        fields,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));
    let result = oo.eval(&program.fields[0].value, &mut ctx);
    match &result {
        Value::Ref(p) => assert!(
            p.anchor == nlang_parser::ast::PathAnchor::Root
                || p.anchor == nlang_parser::ast::PathAnchor::Bare
        ),
        other => {
            let seen = oo.force_recursive(other.clone(), &mut ctx);
            match seen {
                Value::Combo(c) => assert!(
                    matches!(c.get_field("z"), Some(Value::Atom(nlang_parser::ast::AtomKind::Int(_), _, _))),
                    "the pipe read a root without `z`: {:?}",
                    c
                ),
                other => panic!("pipe over <<_.>> should be the root it read, got {:?}", other),
            }
        }
    }
}

#[test]
fn stage3_ref_forces_against_root() {
    // Force <<_.>> → resolves Ref against ctx.root at observation time.
    let program = parse_program("x: <<_.>>").unwrap();
    let oo = Ouroboros::new_in_memory();
    let mut root = ComboVal::new(
        IndexMap::new(),
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    );
    root.insert_field(
        "y",
        Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None),
    );
    let mut ctx = EvalContext::new(root);
    let result = oo.eval(&program.fields[0].value, &mut ctx);
    // result is Ref(root)
    let forced = oo.force(result, &mut ctx);
    match forced {
        Value::Combo(ref root_cv) => {
            assert!(
                root_cv.get_field("y").is_some(),
                "forced Ref should resolve to root which contains y"
            );
        }
        other => panic!("forced Ref should be Combo (root), got {:?}", other),
    }
}

#[test]
fn stage3_ref_content_hash_is_deterministic() {
    // CAID of Ref depends on the path's syntactic geometry, not the resolved
    // value. This makes Refs suitable as memo keys and lazy-unify early-out.
    let p1 = eval_one("a: <<_.>>");
    let p2 = eval_one("b: <<_.>>");
    // Both are Refs to the same path → same CAID
    assert_eq!(
        p1.content_hash(),
        p2.content_hash(),
        "Refs to same path should have same CAID"
    );
}
