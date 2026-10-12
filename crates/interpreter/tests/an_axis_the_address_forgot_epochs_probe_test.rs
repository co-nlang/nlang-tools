// An axis the address forgot — two identity epochs, one address space.
// Order: nlang-tools/docs/an_axis_the_address_forgot_handover.md
// Queue: nlang-spec/meta/WORK_QUEUE.md — Q-083.  Ruling: D106 (乙: an
// epoch-1 store is read with the frozen epoch-1 encoding; every new write
// uses the new one).
//
// A reader that accepts a digest when EITHER encoding reproduces it is sound
// only if no epoch-2 preimage is also an epoch-1 preimage. The API this file
// names is part of the order: `bn_serial::with_legacy_identity(f)` runs `f`
// with the frozen v0.84.0 encoding.
//
// The pins in e1 are v0.84.0's own digests (`oo v0.84.0`, `content_digest`
// of the evaluated value, 2026-10-12). They are what an epoch-1 store holds
// and must never move: the frozen encoding is a museum piece, not a draft.
//
// The delivery may NOT edit this file. `rustfmt` must not touch it.

use indexmap::IndexMap;
use nlang_interpreter::bn_serial::{content_digest, with_legacy_identity};
use nlang_interpreter::value::{ComboVal, EffectTag};
use nlang_interpreter::{EvalContext, Ouroboros, Value};
use nlang_parser::parse_program;

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

const SAMPLES: &[(&str, &str)] = &[
    ("r: 0", "08da7c45cb204377e7e42249cda5713fa865116ddbb4cb5a1949b2e5b438a6ab"),
    ("r: \"a\"", "cdf2d551ff1682671f78193f5ce0d5c0dbf98e7c39fbd748f0505e3cb4687b11"),
    ("r: #a", "261266f60f83ec88ba93c0ba5beaa9b39ef83c4bb61c7035995cb7652f5bcb80"),
    ("r: {}", "47dc540c94ceb704a23875c11273e16bb0b8a87aed84de911f2133568115f254"),
    ("r: { a: 1 }", "9eb5af06c3f4eb8ac379ddfeaa294b339b5033b0c0f0eef01d4210be1f47c7ae"),
    // The defect itself, at byte level: two values, one epoch-1 address.
    ("r: { int: 1 }", "7aa311aa9abc86e12fd28829d415c3a63af18ab953ee56a3c2f88319b4e4eb6c"),
    ("r: { @int: 1 }", "7aa311aa9abc86e12fd28829d415c3a63af18ab953ee56a3c2f88319b4e4eb6c"),
    ("r: { m: 1 }", "bdb167fc22f8910927543c250b82b23354bd74a0534ac381455f8226dd1bfc63"),
    ("r: { %m: 1 }", "bdb167fc22f8910927543c250b82b23354bd74a0534ac381455f8226dd1bfc63"),
    ("r: (1 | 2)", "34c48921043aa2bd2833dd1ee567d0a175faa2370ac47ebb6e4274eced079fa3"),
];

/// The frozen encoding is v0.84.0's, byte for byte.
#[test]
fn e1_the_frozen_encoding_is_the_one_v0_84_wrote() {
    let mut wrong = Vec::new();
    for (src, want) in SAMPLES {
        let v = eval_one(src);
        let got = hex::encode(with_legacy_identity(|| content_digest(&v)));
        if got != *want {
            wrong.push(format!("  {src}\n      want {want}\n      got  {got}"));
        }
    }
    assert!(wrong.is_empty(), "the epoch-1 encoding moved:\n{}", wrong.join("\n"));
}

/// No value keeps its epoch-1 address — not even one the axis never touched
/// (an atom, an empty table). If one did, a reader trying both encodings
/// could not tell which epoch a digest belongs to.
#[test]
fn e2_no_value_keeps_its_epoch_1_address() {
    let mut kept = Vec::new();
    for (src, _) in SAMPLES {
        let v = eval_one(src);
        let new = content_digest(&v);
        let old = with_legacy_identity(|| content_digest(&v));
        if new == old {
            kept.push(format!("  {src} {}", hex::encode(new)));
        }
    }
    assert!(kept.is_empty(), "same digest in both epochs:\n{}", kept.join("\n"));
}

/// The new encoding tells the axes apart; the frozen one, by design, does
/// not. Control for e1: the pins are not vacuous.
#[test]
fn e3_the_new_encoding_tells_the_axes_apart() {
    for (a, b) in [("r: { int: 1 }", "r: { @int: 1 }"), ("r: { m: 1 }", "r: { %m: 1 }")] {
        let (va, vb) = (eval_one(a), eval_one(b));
        assert_ne!(content_digest(&va), content_digest(&vb), "{a} / {b}");
        assert_eq!(
            with_legacy_identity(|| content_digest(&va)),
            with_legacy_identity(|| content_digest(&vb)),
            "{a} / {b}: the frozen encoding must still collide (it is v0.84.0's)"
        );
    }
}

/// Leaving the frozen encoding restores the current one, even on unwind.
#[test]
fn e4_the_frozen_encoding_does_not_leak() {
    let v = eval_one("r: { int: 1 }");
    let before = content_digest(&v);
    let _ = std::panic::catch_unwind(|| with_legacy_identity(|| panic!("inside")));
    let _ = with_legacy_identity(|| content_digest(&v));
    assert_eq!(content_digest(&v), before);
}
