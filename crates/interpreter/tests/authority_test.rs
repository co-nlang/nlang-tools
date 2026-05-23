use nlang_interpreter::authority::{compute_refine_payload, sign_refine, verify_refine_authority, AuthVerifyResult};
use nlang_interpreter::value::{ContentHash, HashAlgorithm, CaidVersion, MasaRef};
use std::collections::HashSet;

fn dummy_caid(seed: u8) -> ContentHash {
    ContentHash { algorithm: HashAlgorithm::Sha256, version: CaidVersion::V1,
        masa_ref: MasaRef::Top, lattice_sketch: String::new(), digest: vec![seed; 32] }
}

fn test_identity() -> nlang_interpreter::value::Identity {
    nlang_interpreter::value::Identity::new_random()
}

#[test]
fn test_sign_and_verify_valid() {
    let id = test_identity();
    let src = vec![dummy_caid(1), dummy_caid(2)];
    let tgt = vec![dummy_caid(3)];
    let payload = compute_refine_payload(&src, &tgt);
    let auth = sign_refine(&payload, &id).unwrap();

    let mut registry = HashSet::new();
    registry.insert(hex::encode(&id.public_key));
    let result = verify_refine_authority(Some(&auth), &payload, &registry, false);
    assert!(matches!(result, AuthVerifyResult::Valid), "sign+verify should pass");
}

#[test]
fn test_verify_wrong_signature() {
    let id = test_identity();
    let payload = compute_refine_payload(&[dummy_caid(1)], &[dummy_caid(2)]);
    let mut auth = sign_refine(&payload, &id).unwrap();
    auth.signature_hex = "00".repeat(64); // 64 bytes hex = 128 chars
    let mut registry = HashSet::new();
    registry.insert(hex::encode(&id.public_key));
    let result = verify_refine_authority(Some(&auth), &payload, &registry, false);
    assert!(matches!(result, AuthVerifyResult::Invalid(_)), "wrong sig should fail");
}

#[test]
fn test_verify_signer_not_in_registry() {
    let id = test_identity();
    let payload = compute_refine_payload(&[dummy_caid(1)], &[dummy_caid(2)]);
    let auth = sign_refine(&payload, &id).unwrap();
    let registry = HashSet::new(); // empty
    let result = verify_refine_authority(Some(&auth), &payload, &registry, false);
    assert!(matches!(result, AuthVerifyResult::Invalid(_)), "signer not in registry");
}

#[test]
fn test_verify_no_authority_bootstrap_exempt() {
    let result = verify_refine_authority(None, b"test", &HashSet::new(), true);
    assert!(matches!(result, AuthVerifyResult::Exempt), "bootstrap exempt");
}

#[test]
fn test_verify_no_authority_non_bootstrap() {
    let result = verify_refine_authority(None, b"test", &HashSet::new(), false);
    assert!(matches!(result, AuthVerifyResult::Invalid(_)), "non-bootstrap requires authority");
}

#[test]
fn test_payload_deterministic() {
    let src = vec![dummy_caid(2), dummy_caid(1)];
    let tgt = vec![dummy_caid(4), dummy_caid(3)];
    let p1 = compute_refine_payload(&src, &tgt);
    let p2 = compute_refine_payload(&src, &tgt);
    assert_eq!(p1, p2, "same sources/targets → same payload");
}

#[test]
fn test_payload_different_caids() {
    let a = compute_refine_payload(&[dummy_caid(1)], &[dummy_caid(2)]);
    let b = compute_refine_payload(&[dummy_caid(1)], &[dummy_caid(3)]);
    assert_ne!(a, b, "different targets → different payload");
}

#[test]
fn test_universe_refine_with_authority() {
    use nlang_interpreter::*;
    use nlang_interpreter::value::{CommitMeta, Value, EffectTag};
    use nlang_parser::ast::AtomKind;
    use std::sync::Arc;

    let oo = Arc::new(Ouroboros::new_in_memory());
    let mut u = Universe::load(&oo, &std::path::Path::new("/tmp/_auth_test")).unwrap();
    let base_dir = std::env::temp_dir().join("nlang-auth-test");
    std::fs::create_dir_all(&base_dir).ok();

    let src_val = Value::Top;
    let tgt_val = Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None);
    let src = oo.store.put_value(&src_val).unwrap();
    let tgt = oo.store.put_value(&tgt_val).unwrap();

    let payload = compute_refine_payload(&[src.clone()], &[tgt.clone()]);
    let authority = sign_refine(&payload, &oo.identity).unwrap();
    let meta = CommitMeta { author: None, timestamp: 0, message: None };

    let result = u.refine(&oo, &base_dir, vec![src], vec![tgt], Some(authority), meta);
    assert!(result.is_ok(), "refine with valid authority should succeed");
}
