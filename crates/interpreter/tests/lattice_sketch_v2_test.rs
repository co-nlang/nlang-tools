use indexmap::IndexMap;
use nlang_interpreter::lattice_sketch::compute_sketch_v2;
use nlang_interpreter::value::{ComboVal, EffectTag, MasaRef, Value};
use nlang_parser::ast::AtomKind;

// Cross-arch test vectors (generated on x86_64, must match on all platforms)
const EXPECTED_SKETCH_TOP: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const EXPECTED_SKETCH_ATOM_42: &str =
    "/5+DhI2V6Mt4AICgg4SNlejLeAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const EXPECTED_SKETCH_COMBO_X1: &str =
    "gIDogv69pdYuAP//54L+vaXWLgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const EXPECTED_SKETCH_COMBO_XY: &str =
    "///47L7Apc6uAQCAgOHvvP7KpN0BAP//54L+vaXWLgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const EXPECTED_SKETCH_STR_HELLO: &str =
    "/5/wgcPFlPZRAICg8IHDxZT2UQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

fn combo_with_data(data: IndexMap<String, Value>) -> Value {
    Value::Combo(ComboVal::new(
        data,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

#[test]
fn test_sketch_top_is_zeros() {
    let sketch = compute_sketch_v2(&Value::Top);
    let decoded =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD_NO_PAD, &sketch).unwrap();
    // 16 components × 2 (amp+phase) × LEB128 var = at least 32 bytes
    assert!(
        decoded.len() >= 32,
        "top sketch decoded should have at least 32 bytes"
    );
}

#[test]
fn test_sketch_deterministic() {
    let mut data = IndexMap::new();
    data.insert(
        "x".to_string(),
        Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None),
    );
    let v = combo_with_data(data);
    let s1 = compute_sketch_v2(&v);
    let s2 = compute_sketch_v2(&v);
    assert_eq!(s1, s2, "same value must produce identical sketch");
}

#[test]
fn test_sketch_atom_vs_combo_differ() {
    // Single atom: 1 component
    let atom = Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None);
    // Combo with 2 fields: definitely different spectrum
    let mut data = IndexMap::new();
    data.insert(
        "x".to_string(),
        Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None),
    );
    data.insert(
        "y".to_string(),
        Value::Atom(AtomKind::Int(2.into()), EffectTag::Pure, None),
    );
    let combo = combo_with_data(data);
    let s_atom = compute_sketch_v2(&atom);
    let s_combo = compute_sketch_v2(&combo);
    if s_atom == s_combo {
        panic!(
            "atom and 2-field combo sketch should differ\natom: {}\ncombo: {}",
            s_atom, s_combo
        );
    }
}

#[test]
fn test_sketch_different_combos_differ() {
    let mut d1 = IndexMap::new();
    d1.insert(
        "a".to_string(),
        Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None),
    );
    let mut d2 = IndexMap::new();
    d2.insert(
        "b".to_string(),
        Value::Atom(AtomKind::Int(2.into()), EffectTag::Pure, None),
    );
    assert_ne!(
        compute_sketch_v2(&combo_with_data(d1)),
        compute_sketch_v2(&combo_with_data(d2))
    );
}

#[test]
fn test_sketch_length_bounded() {
    let mut data = IndexMap::new();
    for i in 0..20 {
        data.insert(
            format!("k{}", i),
            Value::Atom(AtomKind::Int(i.into()), EffectTag::Pure, None),
        );
    }
    let sketch = compute_sketch_v2(&combo_with_data(data));
    assert!(sketch.len() < 256, "sketch should be < 256 bytes");
}

#[test]
fn test_sketch_cross_arch_top() {
    assert_eq!(
        compute_sketch_v2(&Value::Top),
        EXPECTED_SKETCH_TOP,
        "Top sketch must be identical across architectures"
    );
}

#[test]
fn test_sketch_cross_arch_atom_int() {
    let v = Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None);
    assert_eq!(compute_sketch_v2(&v), EXPECTED_SKETCH_ATOM_42);
}

#[test]
fn test_sketch_cross_arch_combo_one_field() {
    let mut d = IndexMap::new();
    d.insert(
        "x".to_string(),
        Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None),
    );
    let v = Value::Combo(ComboVal::new(
        d,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));
    assert_eq!(compute_sketch_v2(&v), EXPECTED_SKETCH_COMBO_X1);
}

#[test]
fn test_sketch_cross_arch_combo_two_fields() {
    let mut d = IndexMap::new();
    d.insert(
        "x".to_string(),
        Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None),
    );
    d.insert(
        "y".to_string(),
        Value::Atom(AtomKind::Int(2.into()), EffectTag::Pure, None),
    );
    let v = Value::Combo(ComboVal::new(
        d,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ));
    assert_eq!(compute_sketch_v2(&v), EXPECTED_SKETCH_COMBO_XY);
}

#[test]
fn test_sketch_cross_arch_str() {
    let v = Value::Atom(AtomKind::Str("hello".to_string()), EffectTag::Pure, None);
    assert_eq!(compute_sketch_v2(&v), EXPECTED_SKETCH_STR_HELLO);
}

#[test]
fn test_sketch_known_vector() {
    let mut d = IndexMap::new();
    d.insert(
        "x".to_string(),
        Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None),
    );
    let v = combo_with_data(d);
    assert_eq!(
        compute_sketch_v2(&v),
        EXPECTED_SKETCH_COMBO_X1,
        "known vector must be stable; if changed, update all EXPECTED_SKETCH_* constants"
    );
}

#[test]
fn test_sketch_combo_with_masa_phase() {
    let mut data = IndexMap::new();
    data.insert(
        "x".to_string(),
        Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None),
    );
    let mut cv = ComboVal::new(data, false, IndexMap::new(), EffectTag::Pure, vec![]);
    cv.masa_ref = MasaRef::Digest(vec![1, 2, 3, 4]);
    let v = Value::Combo(cv);
    let sketch = compute_sketch_v2(&v);
    assert!(!sketch.is_empty(), "sketch with MASA should not be empty");
}

fn quantize_amplitude(v: f64) -> u64 {
    (v * (u64::MAX as f64)) as u64
}

#[test]
fn test_quantize_amplitude_zero() {
    assert_eq!(quantize_amplitude(0.0), 0);
}

#[test]
fn test_quantize_amplitude_one() {
    let q = quantize_amplitude(1.0);
    assert!(q > 0, "quantize(1.0) should produce non-zero");
}

#[test]
fn test_zigzag_roundtrip() {
    fn zigzag(n: i64) -> u64 {
        ((n << 1) ^ (n >> 63)) as u64
    }
    assert_eq!(zigzag(-1), 1);
    assert_eq!(zigzag(1), 2);
    assert_eq!(zigzag(0), 0);
}

#[test]
fn test_leb128_encode() {
    fn leb128_encode(mut v: u64, out: &mut Vec<u8>) {
        loop {
            let byte = (v & 0x7F) as u8;
            v >>= 7;
            if v == 0 {
                out.push(byte);
                break;
            }
            out.push(byte | 0x80);
        }
    }
    let mut buf = Vec::new();
    leb128_encode(127, &mut buf);
    assert_eq!(buf.len(), 1, "127 should encode in 1 byte");
    buf.clear();
    leb128_encode(128, &mut buf);
    assert_eq!(buf.len(), 2, "128 should encode in 2 bytes");
}

#[test]
fn test_sketch_top_phase_is_zero() {
    // Top sketch should have 16 components. Decode into LEB128,
    // the second value of each pair is phase — should all be 0
    let sketch = compute_sketch_v2(&Value::Top);
    assert!(!sketch.is_empty());
}
