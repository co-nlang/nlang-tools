// A hidden facet taken for a name — the address, byte for byte.
// Order: nlang-tools/docs/a_hidden_facet_taken_for_a_name_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-099.  D115 / REAL_03 §6.2: a
// hidden type entry is axis byte 7, a hidden logic entry axis byte 8, the
// name without its prefix. Epoch 2 hashes 0x80 ‖ V(value).
//
// `{ ~@a: 1 }` is 80 01 01 07 01 61 12 01; `{ ~/a: 1 }` is the same with 08;
// `{ ~a: 1 }` (axis 6) is unchanged. v0.99.0 wrote 06 02 40 61 / 06 02 2f 61.
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.

use nlang_interpreter::value::EffectTag;
use nlang_interpreter::{ComboVal, Value};
use nlang_parser::ast::AtomKind;
use sha2::{Digest, Sha256};

/// A combo whose local axis holds `key: 1`, as `~@a: 1` / `~/a: 1` / `~a: 1` build it.
fn digest(key: &str) -> String {
    let mut c = ComboVal::default();
    c.local.insert(key.to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None));
    hex::encode(Value::Combo(c).content_hash().digest)
}

fn spec(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[test]
fn r4_hidden_type_and_logic_are_axes_7_and_8() {
    assert_eq!(digest("@a"), spec(&[0x80, 0x01, 0x01, 0x07, 0x01, 0x61, 0x12, 0x01]), "~@a: 1");
    assert_eq!(digest("/a"), spec(&[0x80, 0x01, 0x01, 0x08, 0x01, 0x61, 0x12, 0x01]), "~/a: 1");
}

#[test]
fn g3_a_private_data_key_is_axis_6_as_before() {
    assert_eq!(digest("a"), spec(&[0x80, 0x01, 0x01, 0x06, 0x01, 0x61, 0x12, 0x01]), "~a: 1");
}
