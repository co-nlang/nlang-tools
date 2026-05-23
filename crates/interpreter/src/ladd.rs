use crate::value::{ContentHash, MasaRef};

/// Geometric Bounding Box — a node's approximate geometric description (APP_05 §2.2).
#[derive(Debug, Clone)]
pub struct GBB {
    pub node_caid: ContentHash,
    pub mass: f64,
    pub sketch_bytes: Vec<u8>,
    pub masa_ref: MasaRef,
}

/// Approximate spectral distance via sketch Hamming distance.
/// Phase 5: replace with real Chordal Distance sqrt(Tr(P_A+P_B-2·P_A⊓P_B)).
pub fn d_l_approx(a: &GBB, b: &GBB) -> f64 {
    if a.sketch_bytes.is_empty() || b.sketch_bytes.is_empty() {
        return 1.0;
    }
    let min_len = a.sketch_bytes.len().min(b.sketch_bytes.len());
    let xor_bits: u32 = a.sketch_bytes[..min_len]
        .iter()
        .zip(&b.sketch_bytes[..min_len])
        .map(|(x, y)| (x ^ y).count_ones())
        .sum();
    let max_bits = (min_len * 8) as f64;
    (xor_bits as f64) / max_bits
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
