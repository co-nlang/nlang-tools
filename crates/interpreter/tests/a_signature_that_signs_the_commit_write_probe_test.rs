// A signature that signs the commit — the write side (Q-059, repair round R-1).
// Order: nlang-tools/docs/a_signature_that_signs_the_commit_handover.md §10.
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-059.  Ruling: meta/oo/STATUS.md D76.
//
// I1 of the order: at write time the judgement against the registry is
// unchanged — membership AND cryptography. On 6da0e09, in a `layout=7` store
// a caller-supplied `authority` is checked for membership only: 64 zero bytes,
// or a signature that is not even hex, from a registered key is stored with
// `authority_status: "verified"`. On the baseline (a39b879, the v0.60.0 code)
// the same two calls are refused ("Ed25519 signature verification failed" /
// "bad signature hex"). Only the library API reaches this path (the CLI never
// hands a pre-made authority to a layout=7 store), but the recorded word is
// the one D75 ② prints beside the signer.
//
// Invariant, not mechanism: a refine write never records "verified" for a
// signature it did not verify at that moment. Refusing a supplied authority in
// a layout=7 store, or verifying it, both pass.
//
// Baseline pole measured with an adapted copy (this arc changed `refine`'s
// arity and `AuthorityInfo`): green on a39b879; red on 6da0e09.
// The delivery may NOT edit this file. `rustfmt` must not touch it.

use nlang_interpreter::value::{AuthorityInfo, CommitMeta};
use nlang_interpreter::*;
use nlang_parser::ast::AtomKind;
use std::sync::Arc;

fn meta(t: u64) -> CommitMeta {
    CommitMeta { author: None, timestamp: t, message: None, abandoned: None, privileged_effect: None, reported_bottoms: None }
}

fn supplied(sig: &str) -> Option<String> {
    let oo = Arc::new(Ouroboros::new_in_memory());
    if !oo.store.signs_the_commit() {
        panic!("VOID READING: the in-memory store is not layout=7");
    }
    let base_dir = nlang_interpreter::ScratchDir::new("sig-commit-write");
    let pk = hex::encode(&oo.identity().unwrap().public_key);
    oo.architect_registry.write().unwrap().insert(pk.clone());
    let mut u = Universe::new_with_standard(None, oo.root_with_system(), oo.root_with_system());
    let ca = oo.store.put_value(&Value::Top).unwrap();
    let cb = oo.store.put_value(&Value::Atom(AtomKind::Int(300.into()), EffectTag::Pure, None)).unwrap();
    u.refine(&oo, &base_dir, vec![ca], vec![cb], None, meta(0), None)
        .unwrap_or_else(|e| panic!("VOID READING: the genesis refine failed: {e}"));
    let cc = oo.store.put_value(&Value::Top).unwrap();
    let cd = oo.store.put_value(&Value::Atom(AtomKind::Int(999.into()), EffectTag::Pure, None)).unwrap();
    let auth = AuthorityInfo { signer_pubkey_hex: pk, signature_hex: sig.to_string() };
    match u.refine(&oo, &base_dir, vec![cc], vec![cd], Some(auth), meta(1), None) {
        Ok(h) => oo.store.get_commit(&h).unwrap().refine_info.unwrap().authority_status,
        Err(_) => None,
    }
}

#[test]
fn w1_a_signature_nobody_checked_is_not_recorded_as_verified() {
    for sig in ["00".repeat(64), "zz".to_string()] {
        let word = supplied(&sig);
        assert_ne!(word.as_deref(), Some("verified"), "signature `{sig}` was recorded as verified without being checked");
    }
}
