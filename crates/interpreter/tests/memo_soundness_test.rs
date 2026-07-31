// Route-A (unify memo) soundness — GUIDE_03 §2 route A hardening.
//
// The memo key is (CAID, CAID) with no horizon parameters, so:
//   1. a fuel-exhausted partial (Blur) must NOT be memoized — otherwise a
//      later full-fuel unify of the same pair replays the stale partial
//      ("blur poisoning"; GUIDE_03 §2A.1 requires horizon params in the key,
//      the cheap sound alternative is exact-results-only memoization)
//   2. #nondet operands must bypass the memo entirely (GUIDE_03 §2A.3)

use indexmap::IndexMap;
use nlang_interpreter::value::{ComboVal, EffectTag, Value};
use nlang_interpreter::{EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

fn big_combo(tag: &str, n: usize) -> Value {
    let mut f = IndexMap::new();
    for i in 0..n {
        f.insert(
            format!("{}_{}", tag, i),
            Value::Atom(AtomKind::Int(BigInt::from(i as i64)), EffectTag::Pure, None),
        );
    }
    Value::Combo(ComboVal::new(
        f,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

#[test]
fn blur_partial_is_not_memoized() {
    // unify itself only fuel-checks at entry, so top-level starvation cannot
    // poison. The reachable path is EMBEDDED: quoted-key fields are thunks;
    // per-field unify forces them, a starved force yields Blur inside the
    // merged combo, and the combo (non-Bottom) lands in the memo.
    let oo = Ouroboros::new_in_memory();

    // combo with an expensive thunked field (quoted key => Thunk)
    let chain = format!(
        "{{ {} }}",
        (0..50)
            .map(|i| format!("f{}: {}", i, i))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let program = nlang_parser::parse_program(&format!("r: {{ \"k\": {} }}", chain)).unwrap();
    let mut build_ctx = EvalContext::new(ComboVal::default());
    // NOTE: use eval (structural), NOT eval_observed — this test needs the
    // thunked field to probe force-induced blur mid-unify. eval_observed would
    // solidify the thunk away. The structural/observation API split (Stage 2)
    // is exactly what makes this distinction expressible.
    let a = oo.eval(&program.fields[0].value, &mut build_ctx);
    assert!(
        matches!(&a, Value::Combo(cv) if matches!(cv.data.get("k"), Some(Value::Thunk{..}))),
        "fixture must hold a thunked field, got {:?}",
        kind_of(&a)
    );
    let b = big_combo("b", 2);

    // 1. starved unify: force(k) blurs mid-combo-eval; the merged combo embeds
    //    a bare Blur field (proven reachable 2026-07-07 — arithmetic bodies instead
    //    collapse Blur to Bottom, which the old guard already excluded)
    let mut starved = EvalContext::new(ComboVal::default()).with_fuel(15);
    let first = oo.unify_internal(a.clone(), b.clone(), &mut starved);

    // 2. full-fuel unify of the SAME pair must be exact: k == 200
    let second = oo.unify(a.clone(), b.clone());
    match &second {
        Value::Combo(cv) => {
            let k_raw = cv.data.get("k").cloned().expect("k present");
            assert!(!k_raw.contains_blur(),
                "full-fuel unify must not replay a starved Blur partial; k = {:?} (starved run: {:?})",
                kind_of(&k_raw), kind_of(&first));
            // Stage 2 (call-by-observation): unify is lazy — k may still be a
            // Thunk. Force it here to verify the observed value is the exact
            // 50-field combo (this is the observation-side check; the memo
            // soundness is already verified by !contains_blur above).
            let mut obs_ctx = EvalContext::new(ComboVal::default());
            let k = oo.force_recursive(k_raw, &mut obs_ctx);
            match &k {
                Value::Combo(kc) => {
                    assert_eq!(kc.data.len(), 50, "k must be the exact 50-field combo")
                }
                other => panic!("k must be a combo after force, got {:?}", kind_of(other)),
            }
        }
        other => panic!("expected Combo, got {:?}", kind_of(other)),
    }
}

#[test]
fn nondet_operand_bypasses_memo() {
    let oo = Ouroboros::new_in_memory();
    let mut nd = ComboVal::default();
    nd.effect = EffectTag::NonDet;
    nd.insert_field(
        "x",
        Value::Atom(AtomKind::Int(BigInt::from(1)), EffectTag::Pure, None),
    );
    let nd = Value::Combo(nd);
    let pure = big_combo("p", 3);

    // unify twice; result correctness aside, the memo must not hold an entry
    // keyed on a #nondet operand (replaying nondet is a semantic lie)
    let _ = oo.unify(nd.clone(), pure.clone());
    let _ = oo.unify(nd.clone(), pure.clone());
    let memo_len = oo.unify_memo.read().unwrap().len();
    assert_eq!(
        memo_len, 0,
        "nondet operand must bypass memo, found {} entries",
        memo_len
    );
}

fn kind_of(v: &Value) -> &'static str {
    match v {
        Value::Top | Value::TopCaused { .. } => "Top",
        Value::Atom(..) => "Atom",
        Value::Combo(_) => "Combo",
        Value::Union(_) => "Union",
        Value::Code(_) => "Code",
        Value::Thunk { .. } => "Thunk",
        Value::Bottom(_) => "Bottom",
        Value::Blur(_) => "Blur",
        Value::Ref(_) => "Ref",
        Value::Range { .. } => "Range",
    }
}
