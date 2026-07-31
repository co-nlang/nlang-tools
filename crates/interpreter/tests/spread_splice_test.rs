// ENGINE_SYNC #17: element-position spread splicing (SPEC_03 §3.1 / SYNTAX_04 §4.8)
//
// Rule: `...x` in a list/tuple element position splices the numeric-keyed
// public fields of x in index order, reindexed into the target. Unboxing
// releases inner effect tags; shell-less values (atoms, Top) contribute
// nothing (isomorphism reading: atom ≅ {%val: x} has no positional fields);
// a Bottom spread source collapses the whole container.

use indexmap::IndexMap;
use nlang_interpreter::value::{ComboVal, EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::{ast::AtomKind, parse_program};
use num_bigint::BigInt;

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

fn int(v: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(v)), EffectTag::Pure, None)
}

fn elems(v: &Value) -> Vec<Value> {
    match v {
        Value::Combo(cv) => {
            let mut keys: Vec<usize> = cv
                .data
                .keys()
                .filter_map(|k| k.parse::<usize>().ok())
                .collect();
            keys.sort_unstable();
            keys.into_iter()
                .map(|k| cv.data.get(&k.to_string()).cloned().unwrap())
                .collect()
        }
        _ => panic!("expected Combo, got {:?}", v),
    }
}

#[test]
fn list_splice_basic() {
    let v = eval_one("r: [...[1, 2], 3]");
    assert_eq!(elems(&v), vec![int(1), int(2), int(3)]);
}

#[test]
fn list_splice_middle_reindexes() {
    let v = eval_one("r: [0, ...[1, 2], 3]");
    assert_eq!(elems(&v), vec![int(0), int(1), int(2), int(3)]);
}

#[test]
fn list_splice_two_sources() {
    let v = eval_one("r: [...[1, 2], ...[3, 4]]");
    assert_eq!(elems(&v), vec![int(1), int(2), int(3), int(4)]);
}

#[test]
fn tuple_splice_stays_sealed() {
    let v = eval_one("r: (...(1, 2), 3)");
    assert_eq!(elems(&v), vec![int(1), int(2), int(3)]);
    match v {
        Value::Combo(cv) => assert!(cv.closed, "tuple result must stay sealed"),
        _ => unreachable!(),
    }
}

#[test]
fn tuple_splices_into_list() {
    // cross-container unboxing: shell type does not survive the move (SPEC_03 §3.1)
    let v = eval_one("r: [...(1, 2), 3]");
    assert_eq!(elems(&v), vec![int(1), int(2), int(3)]);
    match v {
        Value::Combo(cv) => assert!(!cv.closed),
        _ => unreachable!(),
    }
}

#[test]
fn empty_source_contributes_nothing() {
    let v = eval_one("r: [...[], 1]");
    assert_eq!(elems(&v), vec![int(1)]);
}

#[test]
fn shell_less_source_contributes_nothing() {
    // atom ≅ {%val: x}: no numeric-keyed fields, nothing to splice
    let v = eval_one("r: [...5, 1]");
    assert_eq!(elems(&v), vec![int(1)]);
}

#[test]
fn bottom_source_collapses_container() {
    let v = eval_one("r: [..._|_, 1]");
    let is_bottom =
        matches!(v, Value::Bottom(_)) || matches!(v, Value::Atom(AtomKind::Bottom, _, _));
    assert!(
        is_bottom,
        "bottom spread source must collapse the list, got {:?}",
        v
    );
}

#[test]
fn splice_releases_effect_tags() {
    // SPEC_03 §3.1: unboxing releases inner effect tags into the target
    let oo = Ouroboros::new_in_memory();
    let mut root = ComboVal::default();
    let mut io_cv = ComboVal::default();
    io_cv.effect = EffectTag::IO;
    let mut seq = ComboVal::default();
    seq.insert_field("0", Value::Combo(io_cv));
    root.insert_field("xs", Value::Combo(seq));
    let mut ctx = EvalContext::new(root);
    let program = parse_program("r: [...xs, 1]").unwrap();
    let v = oo.eval_observed(&program.fields[0].value, &mut ctx);
    assert_eq!(
        v.effect(),
        EffectTag::IO,
        "spliced element effect must surface, got {:?}",
        v
    );
}
