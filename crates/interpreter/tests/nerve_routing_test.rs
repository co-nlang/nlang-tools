use nlang_interpreter::ladd::{GBB, NerveEntry, nerve_overlap, d_l_approx, gravitational_weight, masa_compatible};
use nlang_interpreter::value::{ContentHash, MasaRef, HashAlgorithm, CaidVersion};

fn dummy_caid() -> ContentHash {
    ContentHash { algorithm: HashAlgorithm::Sha256, version: CaidVersion::V2,
        masa_ref: MasaRef::Top, lattice_sketch: String::new(), digest: vec![0; 32] }
}

fn gbb(masa: MasaRef, nerve: Vec<NerveEntry>) -> GBB {
    GBB { node_caid: dummy_caid(), mass: 1.0, sketch_bytes: vec![0x42],
          masa_ref: masa, nerve_structure: nerve }
}

#[test]
fn test_nerve_overlap_both_empty() {
    let a = gbb(MasaRef::Top, vec![]);
    let b = gbb(MasaRef::Top, vec![]);
    assert!(nerve_overlap(&a, &b), "both empty → true (no pruning info)");
}

#[test]
fn test_nerve_overlap_no_common() {
    let a = gbb(MasaRef::Top, vec![NerveEntry { masa_caid: "m1".into(), overlapping_masa_caids: vec![] }]);
    let b = gbb(MasaRef::Top, vec![NerveEntry { masa_caid: "m2".into(), overlapping_masa_caids: vec![] }]);
    assert!(!nerve_overlap(&a, &b), "different MASA → false");
}

#[test]
fn test_nerve_overlap_direct_match() {
    let a = gbb(MasaRef::Top, vec![NerveEntry { masa_caid: "m1".into(), overlapping_masa_caids: vec![] }]);
    let b = gbb(MasaRef::Top, vec![NerveEntry { masa_caid: "m1".into(), overlapping_masa_caids: vec![] }]);
    assert!(nerve_overlap(&a, &b), "same masa_caid → true");
}

#[test]
fn test_nerve_overlap_via_overlapping() {
    let a = gbb(MasaRef::Top, vec![NerveEntry { masa_caid: "m1".into(), overlapping_masa_caids: vec![] }]);
    let b = gbb(MasaRef::Top, vec![NerveEntry { masa_caid: "m2".into(), overlapping_masa_caids: vec!["m1".into()] }]);
    assert!(nerve_overlap(&a, &b), "overlapping includes query's masa → true");
}

#[test]
fn test_d_l_approx_after_phase5() {
    let a = gbb(MasaRef::Top, vec![]);
    let mut b = a.clone();
    b.sketch_bytes = vec![0xFF];
    let d = d_l_approx(&a, &b);
    assert!(d > 0.0, "different sketch → d_L > 0");
}

#[test]
fn test_masa_and_nerve_combined() {
    let q = gbb(MasaRef::Top, vec![NerveEntry { masa_caid: "m1".into(), overlapping_masa_caids: vec![] }]);
    let p = gbb(MasaRef::Digest(vec![1]), vec![NerveEntry { masa_caid: "m2".into(), overlapping_masa_caids: vec![] }]);
    assert!(masa_compatible(&q, &p), "Top × Digest → compatible");
    assert!(!nerve_overlap(&q, &p), "different nerve → incompatible");
}

#[test]
fn test_gravitational_weight_with_nerve_gbb() {
    let q = gbb(MasaRef::Top, vec![]);
    let p = GBB { mass: 2.0, ..gbb(MasaRef::Top, vec![]) };
    let w = gravitational_weight(&q, &p, 1e-6);
    assert!(w > 0.0, "weight should be positive");
}
