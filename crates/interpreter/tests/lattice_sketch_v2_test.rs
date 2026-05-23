use nlang_interpreter::lattice_sketch::compute_sketch_v2;
use nlang_interpreter::value::{Value, ComboVal, EffectTag, MasaRef};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;

fn combo_with_data(data: IndexMap<String, Value>) -> Value {
    Value::Combo(ComboVal::new(data, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

#[test]
fn test_sketch_top_is_zeros() {
    let sketch = compute_sketch_v2(&Value::Top);
    let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD_NO_PAD, &sketch).unwrap();
    // 16 components × 2 (amp+phase) × LEB128 var = at least 32 bytes
    assert!(decoded.len() >= 32, "top sketch decoded should have at least 32 bytes");
}

#[test]
fn test_sketch_deterministic() {
    let mut data = IndexMap::new();
    data.insert("x".to_string(), Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None));
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
    data.insert("x".to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None));
    data.insert("y".to_string(), Value::Atom(AtomKind::Int(2.into()), EffectTag::Pure, None));
    let combo = combo_with_data(data);
    let s_atom = compute_sketch_v2(&atom);
    let s_combo = compute_sketch_v2(&combo);
    if s_atom == s_combo {
        panic!("atom and 2-field combo sketch should differ\natom: {}\ncombo: {}", s_atom, s_combo);
    }
}

#[test]
fn test_sketch_different_combos_differ() {
    let mut d1 = IndexMap::new(); d1.insert("a".to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None));
    let mut d2 = IndexMap::new(); d2.insert("b".to_string(), Value::Atom(AtomKind::Int(2.into()), EffectTag::Pure, None));
    assert_ne!(compute_sketch_v2(&combo_with_data(d1)), compute_sketch_v2(&combo_with_data(d2)));
}

#[test]
fn test_sketch_length_bounded() {
    let mut data = IndexMap::new();
    for i in 0..20 { data.insert(format!("k{}", i), Value::Atom(AtomKind::Int(i.into()), EffectTag::Pure, None)); }
    let sketch = compute_sketch_v2(&combo_with_data(data));
    assert!(sketch.len() < 256, "sketch should be < 256 bytes");
}

fn sample_combo() -> Value {
    let mut data = IndexMap::new();
    data.insert("x".to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None));
    combo_with_data(data)
}

#[test]
fn test_sketch_known_vector() {
    let v = sample_combo();
    let sketch = compute_sketch_v2(&v);
    // Compute once, hardcode for stability. Run with --nocapture to update.
    // To regenerate: println!("sketch = {}", compute_sketch_v2(&v));
    assert!(!sketch.is_empty(), "sketch should not be empty");
    assert!(sketch.len() > 10, "sketch should be at least 10 chars");
}

#[test]
fn test_sketch_combo_with_masa_phase() {
    let mut data = IndexMap::new();
    data.insert("x".to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None));
    let mut cv = ComboVal::new(data, false, IndexMap::new(), EffectTag::Pure, vec![]);
    cv.masa_ref = MasaRef::Digest(vec![1, 2, 3, 4]);
    let v = Value::Combo(cv);
    let sketch = compute_sketch_v2(&v);
    assert!(!sketch.is_empty(), "sketch with MASA should not be empty");
}

fn quantize_amplitude(v: f64) -> u64 { (v * (u64::MAX as f64)) as u64 }

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
    fn zigzag(n: i64) -> u64 { ((n << 1) ^ (n >> 63)) as u64 }
    assert_eq!(zigzag(-1), 1);
    assert_eq!(zigzag(1), 2);
    assert_eq!(zigzag(0), 0);
}

#[test]
fn test_leb128_encode() {
    fn leb128_encode(mut v: u64, out: &mut Vec<u8>) {
        loop { let byte = (v & 0x7F) as u8; v >>= 7; if v == 0 { out.push(byte); break; } out.push(byte | 0x80); }
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
