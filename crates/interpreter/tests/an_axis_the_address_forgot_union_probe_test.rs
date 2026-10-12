// An axis the address forgot — the union half.
// Order: nlang-tools/docs/an_axis_the_address_forgot_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-083.  Ruling: D106.
// Spec: REAL_03 §6.2 (the identity encoding is a prefix code), §6.7 second
// MUST NOT (two values that `=` tells apart never share an address).
//
// v0.84.0 writes a union as its members' bytes back to back: no tag, no
// count. Two values whose unions sit in different places can then have the
// same bytes. Here `(1 | 2)..3` and `1..(2 | 3)` are both
//     18 [12 01][12 02][12 03] 00
// The evaluator collapses both ranges to ⊥ before they reach a store, so no
// surface program found a witness (2026-10-12) — the encoding is still not a
// prefix code, and one argument away from a live collision.
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.

use nlang_interpreter::value::EffectTag;
use nlang_interpreter::Value;
use nlang_parser::ast::AtomKind;

fn int(n: i64) -> Value {
    Value::Atom(AtomKind::Int(n.into()), EffectTag::Pure, None)
}

fn digest(v: &Value) -> String {
    hex::encode(nlang_interpreter::bn_serial::content_digest(v))
}

fn range(start: Value, end: Value) -> Value {
    Value::Range { start: Box::new(start), end: Box::new(end), step: None }
}

/// A union ends where its bytes say it ends.
#[test]
fn r1_a_union_says_where_it_ends() {
    let left = range(Value::Union(vec![int(1), int(2)]), int(3));
    let right = range(int(1), Value::Union(vec![int(2), int(3)]));
    assert_ne!(
        digest(&left),
        digest(&right),
        "(1 | 2)..3 and 1..(2 | 3) are two values with one address"
    );
}

/// The same, one level up: a union as a list element and its neighbour.
#[test]
fn r2_a_union_beside_another_value() {
    let left = range(Value::Union(vec![int(1), int(2)]), Value::Union(vec![int(3), int(4)]));
    let right = range(int(1), Value::Union(vec![int(2), int(3), int(4)]));
    assert_ne!(digest(&left), digest(&right), "two values with one address");
}
