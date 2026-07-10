use crate::value::{ContentHash, MasaRef};
use std::collections::HashSet;

/// Čech nerve position entry (APP_05 §4.3).
#[derive(Debug, Clone)]
pub struct NerveEntry {
    pub masa_caid: String,
    pub overlapping_masa_caids: Vec<String>,
    pub field_keys: Vec<String>,
}

/// Geometric Bounding Box (APP_05 §2.2).
#[derive(Debug, Clone)]
pub struct GBB {
    pub node_caid: ContentHash,
    pub mass: f64,
    pub sketch_bytes: Vec<u8>,
    pub masa_ref: MasaRef,
    pub nerve_structure: Vec<NerveEntry>,
}

/// Approximate spectral distance via sketch cosine similarity (APP_05 §3.2).
/// d_L = arccos(cos_sim) / π  ∈ [0, 1].
pub fn d_l_approx(a: &GBB, b: &GBB) -> f64 {
    if a.sketch_bytes.is_empty() || b.sketch_bytes.is_empty() {
        return 1.0;
    }
    let min_len = a.sketch_bytes.len().min(b.sketch_bytes.len());
    let av: Vec<f64> = a.sketch_bytes[..min_len].iter().map(|&x| x as i8 as f64).collect();
    let bv: Vec<f64> = b.sketch_bytes[..min_len].iter().map(|&x| x as i8 as f64).collect();
    let dot: f64   = av.iter().zip(bv.iter()).map(|(x, y)| x * y).sum();
    let na: f64    = av.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb: f64    = bv.iter().map(|x| x * x).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return if na == nb { 0.0 } else { 1.0 };
    }
    let cos_sim = (dot / (na * nb)).clamp(-1.0, 1.0);
    cos_sim.acos() / std::f64::consts::PI
}

/// Gravitational routing weight: W = mass / (d_L² + ε).
pub fn gravitational_weight(query: &GBB, peer: &GBB, epsilon: f64) -> f64 {
    let d = d_l_approx(query, peer);
    peer.mass / (d * d + epsilon)
}

/// MASA compatibility pre-filter (H² obstruction, APP_05 §4.1).
pub fn masa_compatible(query: &GBB, peer: &GBB) -> bool {
    match (&query.masa_ref, &peer.masa_ref) {
        (MasaRef::Top, _) | (_, MasaRef::Top) => true,
        (MasaRef::Digest(a), MasaRef::Digest(b)) => a == b,
    }
}

/// Nerve overlap check (APP_05 §4.3). Empty → passes (no pruning info).
pub fn nerve_overlap(query: &GBB, peer: &GBB) -> bool {
    if query.nerve_structure.is_empty() || peer.nerve_structure.is_empty() {
        return true;
    }
    let query_masas: HashSet<&str> =
        query.nerve_structure.iter().map(|e| e.masa_caid.as_str()).collect();
    let query_keys: HashSet<&str> = query.nerve_structure.iter()
        .flat_map(|e| e.field_keys.iter().map(|k| k.as_str()))
        .collect();

    peer.nerve_structure.iter().any(|pe| {
        query_masas.contains(pe.masa_caid.as_str())
        || pe.overlapping_masa_caids.iter().any(|m| query_masas.contains(m.as_str()))
        || (!query_keys.is_empty()
            && !pe.field_keys.is_empty()
            && pe.field_keys.iter().any(|k| query_keys.contains(k.as_str())))
    })
}
