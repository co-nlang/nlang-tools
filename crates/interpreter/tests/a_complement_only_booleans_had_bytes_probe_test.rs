// A complement only booleans had — the address, byte for byte.
// Order: nlang-tools/docs/a_complement_only_booleans_had_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-100.  D116 / REAL_03 §6: a
// complement is value tag 0x06, then `within`, then `without`. `!c` has
// `within` Top (0xFF). Epoch 2 hashes 0x80 ‖ V(value).
//
// `!0` is 80 06 FF 12 00; `!#true` is 80 06 FF 11 04 "true". v0.100.0 had no
// such value: `!0` was ⊥ (80 FE) and `!#true` was `#false`.
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.

use nlang_interpreter::value::EffectTag;
use nlang_interpreter::{Ouroboros, Value};
use nlang_parser::ast::AtomKind;
use sha2::{Digest, Sha256};

fn digest(v: Value) -> String {
    hex::encode(v.content_hash().digest)
}

fn spec(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// `!` of `v`, as `oo eval '!…'` builds it.
fn not(v: Value) -> Value {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = oo.eval_context();
    oo.orthocomplement(v, &mut ctx)
}

fn int(n: i64) -> Value {
    Value::Atom(AtomKind::Int(n.into()), EffectTag::Pure, None)
}

fn tag(t: &str) -> Value {
    Value::Atom(AtomKind::Tag(t.to_string()), EffectTag::Pure, None)
}

#[test]
fn r7_a_complement_is_tag_6() {
    assert_eq!(digest(not(int(0))), spec(&[0x80, 0x06, 0xFF, 0x12, 0x00]), "!0");
    let mut b = vec![0x80, 0x06, 0xFF, 0x11, 0x04];
    b.extend_from_slice(b"true");
    assert_eq!(digest(not(tag("true"))), spec(&b), "!#true");
}

#[test]
fn g3_values_without_a_complement_keep_their_bytes() {
    assert_eq!(digest(int(0)), spec(&[0x80, 0x12, 0x00]), "0");
    let mut b = vec![0x80, 0x11, 0x05];
    b.extend_from_slice(b"false");
    assert_eq!(digest(tag("false")), spec(&b), "#false");
}
