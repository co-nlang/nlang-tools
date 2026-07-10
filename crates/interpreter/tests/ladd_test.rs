use nlang_interpreter::ladd::{GBB, d_l_approx, gravitational_weight, masa_compatible};
use nlang_interpreter::value::{ContentHash, MasaRef, HashAlgorithm, CaidVersion};

fn dummy_caid() -> ContentHash {
    ContentHash {
        algorithm: HashAlgorithm::Sha256,
        version: CaidVersion::V2,
        masa_ref: MasaRef::Top,
        lattice_sketch: String::new(),
        digest: vec![0; 32],
    }
}

fn gbb_with_sketch(sketch: &[u8], masa: MasaRef) -> GBB {
    GBB { node_caid: dummy_caid(), mass: 0.5, sketch_bytes: sketch.to_vec(), masa_ref: masa, nerve_structure: vec![] }
}

#[test]
fn test_d_l_approx_identical() {
    let sk = vec![0xAB, 0xCD, 0xEF];
    let a = gbb_with_sketch(&sk, MasaRef::Top);
    let b = gbb_with_sketch(&sk, MasaRef::Top);
    assert_eq!(d_l_approx(&a, &b), 0.0, "identical sketch → d_L = 0");
}

#[test]
fn test_d_l_approx_empty() {
    let a = gbb_with_sketch(&[], MasaRef::Top);
    let b = gbb_with_sketch(&[0x01], MasaRef::Top);
    assert_eq!(d_l_approx(&a, &b), 1.0, "empty sketch → d_L = 1");
}

#[test]
fn test_d_l_approx_range() {
    let a = gbb_with_sketch(&[0x00], MasaRef::Top);
    let b = gbb_with_sketch(&[0xFF], MasaRef::Top);
    let d = d_l_approx(&a, &b);
    assert!(d >= 0.0 && d <= 1.0, "d_L must be in [0,1]");
}

#[test]
fn test_masa_compatible_top() {
    let q = gbb_with_sketch(&[1], MasaRef::Top);
    let p = gbb_with_sketch(&[2], MasaRef::Digest(vec![42]));
    assert!(masa_compatible(&q, &p), "Top × anything → compatible");
    assert!(masa_compatible(&p, &q), "anything × Top → compatible");
}

#[test]
fn test_masa_compatible_same_digest() {
    let d = vec![1, 2, 3];
    let a = gbb_with_sketch(&[], MasaRef::Digest(d.clone()));
    let b = gbb_with_sketch(&[], MasaRef::Digest(d));
    assert!(masa_compatible(&a, &b));
}

#[test]
fn test_masa_incompatible() {
    let a = gbb_with_sketch(&[], MasaRef::Digest(vec![1]));
    let b = gbb_with_sketch(&[], MasaRef::Digest(vec![2]));
    assert!(!masa_compatible(&a, &b));
}

#[test]
fn test_gravitational_weight_positive() {
    let q = gbb_with_sketch(&[0x00], MasaRef::Top);
    let p = GBB { mass: 1.0, ..gbb_with_sketch(&[0x01], MasaRef::Top) };
    let w = gravitational_weight(&q, &p, 1e-6);
    assert!(w > 0.0, "gravitational weight should be positive");
}

#[test]
fn test_disc_advertise_and_find() {
    use std::sync::Arc;
    use nlang_interpreter::*;
    use nlang_parser::ast::{AtomKind};

    let oo = Arc::new(Ouroboros::new_in_memory());
    let val = Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None);
    let mut ctx = EvalContext::new(oo.root_with_system());

    // Call disc.advertise
    let builtins = &oo.builtin_registry;
    let advertise_fn = builtins.get("disc.advertise").unwrap();
    let result = advertise_fn(val.clone(), &oo, &mut ctx);
    assert!(result.to_string_plain().contains("true"), "advertise should return #true");

    // Check gbb_registry has an entry
    let reg = oo.gbb_registry.read().unwrap();
    assert!(!reg.is_empty(), "gbb_registry should have entries after advertise");
}

// ── Phase 16: cosine d_l_approx tests ──

#[test]
fn test_d_l_approx_cosine_different() {
    let a = GBB {
        node_caid: dummy_caid(),
        mass: 1.0,
        sketch_bytes: vec![1u8, 0, 0, 0],
        masa_ref: MasaRef::Top,
        nerve_structure: vec![],
    };
    let b = GBB {
        node_caid: dummy_caid(),
        mass: 1.0,
        sketch_bytes: vec![0u8, 1, 0, 0],
        masa_ref: MasaRef::Top,
        nerve_structure: vec![],
    };
    let d = d_l_approx(&a, &b);
    assert!((d - 0.5).abs() < 1e-10, "orthogonal sketches → d_L ≈ 0.5, got {}", d);
}

#[test]
fn test_d_l_approx_identical_still_zero() {
    let bytes = vec![42u8, 17, 255, 0, 128];
    let a = GBB { node_caid: dummy_caid(), mass: 1.0,
        sketch_bytes: bytes.clone(), masa_ref: MasaRef::Top, nerve_structure: vec![] };
    let b = GBB { node_caid: dummy_caid(), mass: 1.0,
        sketch_bytes: bytes, masa_ref: MasaRef::Top, nerve_structure: vec![] };
    assert_eq!(d_l_approx(&a, &b), 0.0, "identical sketch → d_L = 0");
}
