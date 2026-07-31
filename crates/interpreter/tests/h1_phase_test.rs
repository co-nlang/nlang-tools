use indexmap::IndexMap;
use nlang_interpreter::value::{BottomCause, ComboVal, EffectTag, Value};
use nlang_interpreter::{MasaRef, Ouroboros};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

const EPSILON_COHERENT: f64 = 0.1;

fn oo() -> Ouroboros {
    Ouroboros::new_in_memory()
}

fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}

fn top_combo(fields: &[(&str, Value)]) -> Value {
    let mut m = IndexMap::new();
    for (k, v) in fields {
        m.insert(k.to_string(), v.clone());
    }
    Value::Combo(ComboVal::new(
        m,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

fn masa_combo(digest: Vec<u8>, fields: &[(&str, Value)]) -> ComboVal {
    let mut m = IndexMap::new();
    for (k, v) in fields {
        m.insert(k.to_string(), v.clone());
    }
    let mut cv = ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]);
    cv.masa_ref = MasaRef::Digest(digest);
    cv
}

fn masa_digest(seed: u8) -> Vec<u8> {
    vec![seed; 32]
}

// ─── 1. phase_diff_between: identical combos ────────────────────────────────

#[test]
fn test_phase_diff_identical_combos_is_zero() {
    let digest = masa_digest(0xAB);
    let fields: &[(&str, Value)] = &[("x", int_val(1)), ("y", int_val(2)), ("z", int_val(3))];
    let a = masa_combo(digest.clone(), fields);
    let b = masa_combo(digest.clone(), fields);

    let theta = nlang_interpreter::lattice_sketch::phase_diff_between(&a, &b);
    assert!(
        theta < 1e-6,
        "identical combos should have theta ≈ 0, got {}",
        theta
    );
}

// ─── 2. phase_diff_between: many-field combos with different keys ─────────────

#[test]
fn test_phase_diff_different_field_keys_is_positive() {
    let digest = masa_digest(0x11);
    let fields_a: Vec<(&str, Value)> = (0..8_i64)
        .map(|i| {
            let key = Box::leak(format!("a{}", i).into_boxed_str()) as &str;
            (key, int_val(i))
        })
        .collect();
    let fields_b: Vec<(&str, Value)> = (0..8_i64)
        .map(|i| {
            let key = Box::leak(format!("b{}", i).into_boxed_str()) as &str;
            (key, int_val(i + 100))
        })
        .collect();

    let a = masa_combo(digest.clone(), &fields_a);
    let b = masa_combo(digest.clone(), &fields_b);

    let theta = nlang_interpreter::lattice_sketch::phase_diff_between(&a, &b);
    assert!(
        theta > 0.0,
        "different-key combos should have theta > 0, got {}",
        theta
    );
}

// ─── 3. Top-MASA combos: unify never H1Splits ────────────────────────────────

#[test]
fn test_top_masa_unify_never_h1splits() {
    let oo = oo();
    let a = top_combo(&[("x", int_val(1)), ("y", int_val(2))]);
    let b = top_combo(&[("z", int_val(3)), ("w", int_val(4))]);
    let result = oo.unify(a, b);
    assert!(
        !matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::H1Split)),
        "Top-MASA combos should never H1Split"
    );
}

// ─── 4. same-MASA different-data: H2 passes, H1 may fire ─────────────────────

#[test]
fn test_same_masa_combos_may_h1split() {
    let oo = oo();
    let digest = masa_digest(0x55);

    let mut m_a = IndexMap::new();
    let mut m_b = IndexMap::new();
    for i in 0..16_i64 {
        m_a.insert(format!("fa{}", i), int_val(i));
        m_b.insert(format!("fb{}", i), int_val(i + 1000));
    }
    let mut cv_a = ComboVal::new(m_a, false, IndexMap::new(), EffectTag::Pure, vec![]);
    cv_a.masa_ref = MasaRef::Digest(digest.clone());
    let mut cv_b = ComboVal::new(m_b, false, IndexMap::new(), EffectTag::Pure, vec![]);
    cv_b.masa_ref = MasaRef::Digest(digest);

    let result = oo.unify(Value::Combo(cv_a), Value::Combo(cv_b));

    assert!(
        matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::H1Split)),
        "16 orthogonal-field combos with explicit MASA should H1Split, got {:?}",
        result
    );
}

// ─── 5. H1Split Bottom has correct metadata ───────────────────────────────────

#[test]
fn test_h1split_bottom_has_theta_and_degree() {
    let oo = oo();
    let digest = masa_digest(0x77);

    let mut m_a = IndexMap::new();
    let mut m_b = IndexMap::new();
    for i in 0..16_i64 {
        m_a.insert(format!("aa{}", i), int_val(i));
        m_b.insert(format!("bb{}", i), int_val(i + 500));
    }
    let mut cv_a = ComboVal::new(m_a, false, IndexMap::new(), EffectTag::Pure, vec![]);
    cv_a.masa_ref = MasaRef::Digest(digest.clone());
    let mut cv_b = ComboVal::new(m_b, false, IndexMap::new(), EffectTag::Pure, vec![]);
    cv_b.masa_ref = MasaRef::Digest(digest);

    let result = oo.unify(Value::Combo(cv_a), Value::Combo(cv_b));
    if let Value::Bottom(ref bd) = result {
        assert!(
            matches!(bd.cause, BottomCause::H1Split),
            "cause should be H1Split"
        );
        assert_eq!(bd.obstruction_degree, Some(1), "H1 → degree 1");
        if let Some(nlang_interpreter::value::Holonomy::Phase(theta)) = bd.holonomy {
            assert!(
                theta >= EPSILON_COHERENT,
                "theta={} should be >= epsilon={}",
                theta,
                EPSILON_COHERENT
            );
        } else {
            panic!("holonomy should be Phase(theta), got {:?}", bd.holonomy);
        }
    }
}

// ─── 6. phase_diff_between: degenerate (empty) combo returns 0 ───────────────

#[test]
fn test_phase_diff_empty_combo_is_zero() {
    let digest = masa_digest(0xCC);
    let a = masa_combo(digest.clone(), &[]);
    let b = masa_combo(digest.clone(), &[("x", int_val(1))]);
    let theta = nlang_interpreter::lattice_sketch::phase_diff_between(&a, &b);
    assert!(
        theta < 1e-9,
        "degenerate (empty) combo → theta = 0, got {}",
        theta
    );
}

// ─── 7. MasaRef::Top + Digest combo: no H2 obstruction, no H1 check ──────────

#[test]
fn test_top_and_digest_masa_no_obstruction() {
    let oo = oo();
    let digest = masa_digest(0xDD);

    let top_c = top_combo(&[("p", int_val(99))]);
    let mut m_d = IndexMap::new();
    m_d.insert("q".to_string(), int_val(88));
    let mut cv_d = ComboVal::new(m_d, false, IndexMap::new(), EffectTag::Pure, vec![]);
    cv_d.masa_ref = MasaRef::Digest(digest);

    let result = oo.unify(top_c, Value::Combo(cv_d));
    assert!(
        !matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::H1Split)),
        "Top + Digest combos should not H1Split (theta=0 from match arm)"
    );
}
