use crate::value::{Value, MasaRef};
use crate::bn_serial::serialize_bn;
use sha2::{Sha256, Digest};
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};

const MAX_COMPONENTS: usize = 16;

/// Phase 5 structured spectral fingerprint (APP_05 §3.5).
pub fn compute_sketch_v2(value: &Value) -> String {
    let (amplitudes, phases) = extract_spectral_components(value);
    let encoded = encode_complex_spectrum(&amplitudes, &phases);
    STANDARD_NO_PAD.encode(&encoded)
}

fn extract_spectral_components(value: &Value) -> (Vec<f64>, Vec<f64>) {
    match value {
        Value::Top => (vec![0.0; MAX_COMPONENTS], vec![0.0; MAX_COMPONENTS]),
        Value::Combo(cv) => {
            let mut entries: Vec<(&str, &Value)> = Vec::new();
            for (k, v) in &cv.system  { entries.push((k, v)); }
            for (k, v) in &cv.meta    { entries.push((k, v)); }
            for (k, v) in &cv.types   { entries.push((k, v)); }
            for (k, v) in &cv.rules   { entries.push((k, v)); }
            for (k, v) in &cv.data    { entries.push((k, v)); }
            for (k, v) in &cv.local   { entries.push((k, v)); }
            for (k, v) in &cv.legacy_fields { entries.push((k, v)); }
            for (k, v) in &cv.legacy_local  { entries.push((k, v)); }

            let mut components: Vec<(f64, f64)> = entries.iter().map(|(key, val)| {
                (field_amplitude(val), field_phase(&cv.masa_ref, key))
            }).collect();
            components.sort_by(|a, b| {
                b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            });
            components.truncate(MAX_COMPONENTS);
            while components.len() < MAX_COMPONENTS { components.push((0.0, 0.0)); }
            components.into_iter().unzip()
        }
        Value::Union(branches) => {
            let mut amps: Vec<f64> = branches.iter().map(field_amplitude_value).collect();
            amps.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
            amps.truncate(MAX_COMPONENTS);
            while amps.len() < MAX_COMPONENTS { amps.push(0.0); }
            let phases = vec![0.0; MAX_COMPONENTS];
            (amps, phases)
        }
        other => {
            let amp = field_amplitude_value(other);
            let mut amps = vec![0.0; MAX_COMPONENTS];
            let phases = vec![0.0; MAX_COMPONENTS];
            amps[0] = amp;
            (amps, phases)
        }
    }
}

fn field_amplitude(value: &Value) -> f64 {
    let bn = serialize_bn(value);
    let hash = Sha256::digest(&bn);
    let hi = u64::from_be_bytes(hash[0..8].try_into().unwrap());
    hi as f64 / u64::MAX as f64
}

fn field_amplitude_value(value: &Value) -> f64 { field_amplitude(value) }

fn field_phase(masa_ref: &MasaRef, field_key: &str) -> f64 {
    match masa_ref {
        MasaRef::Top => 0.0,
        MasaRef::Digest(d) => {
            let mut h = Sha256::new();
            h.update(d);
            h.update(field_key.as_bytes());
            let hash = h.finalize();
            let raw = u64::from_be_bytes(hash[0..8].try_into().unwrap());
            (raw as f64 / u64::MAX as f64) * 2.0 * std::f64::consts::PI - std::f64::consts::PI
        }
    }
}

// ── Quantization & Encoding (APP_05 §3.5.3–3.5.4) ──────────────

fn encode_complex_spectrum(amplitudes: &[f64], phases: &[f64]) -> Vec<u8> {
    assert_eq!(amplitudes.len(), MAX_COMPONENTS);
    assert_eq!(phases.len(), MAX_COMPONENTS);

    let lambda_q: Vec<u64> = amplitudes.iter().map(|&v| quantize_amplitude(v)).collect();
    let theta_q: Vec<u64>  = phases.iter().map(|&p| quantize_phase(p)).collect();

    let delta_l = delta_encode(&lambda_q);
    let delta_t = delta_encode(&theta_q);

    let zz_l: Vec<u64> = delta_l.iter().map(|&d| zigzag(d as i64)).collect();
    let zz_t: Vec<u64> = delta_t.iter().map(|&d| zigzag(d as i64)).collect();

    let mut out = Vec::new();
    for i in 0..MAX_COMPONENTS {
        leb128_encode(zz_l[i], &mut out);
        leb128_encode(zz_t[i], &mut out);
    }
    out
}

fn quantize_amplitude(v: f64) -> u64 {
    (v * (u64::MAX as f64)) as u64
}

fn quantize_phase(phi: f64) -> u64 {
    let normalized = phi / std::f64::consts::PI;
    (normalized * (i64::MAX as f64)) as u64
}

fn delta_encode(seq: &[u64]) -> Vec<u64> {
    let mut out = Vec::with_capacity(seq.len());
    let mut prev = 0u64;
    for &v in seq {
        out.push(v.wrapping_sub(prev));
        prev = v;
    }
    out
}

fn zigzag(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}

fn leb128_encode(mut v: u64, out: &mut Vec<u8>) {
    loop {
        let byte = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 { out.push(byte); break; }
        out.push(byte | 0x80);
    }
}

/// Kept for backward-compatibility reference.
#[doc(hidden)]
pub fn compute_sketch_approximate(bn_bytes: &[u8]) -> String {
    let hash = Sha256::digest(bn_bytes);
    base64::Engine::encode(&STANDARD_NO_PAD, &hash[..12])
}
