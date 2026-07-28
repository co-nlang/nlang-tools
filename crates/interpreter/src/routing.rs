//! Kademlia routing index (kademlia_table arc).
//!
//! Second index over accepted `#advertise` records (`PeerAdvert`). Populated
//! only after the advertise ladder passes — never from unsigned `%from`.
//!
//! Self id = first 20 bytes of `node_id` CAID digest. Bucket *i* = leading
//! zero-bit count of XOR(self, peer). k = 20, incumbent-first.

use crate::value::{ContentHash, Identity};
use crate::PeerAdvert;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const ID_BYTES: usize = 20;
pub const K: usize = 20;
pub const N_BUCKETS: usize = ID_BYTES * 8; // 160
pub const MAX_FIND_NODE_PEERS: usize = K;

/// 160-bit routing id: first 20 bytes of a CAID's content digest.
pub fn routing_id_from_digest(digest: &[u8]) -> [u8; ID_BYTES] {
    let mut out = [0u8; ID_BYTES];
    let n = digest.len().min(ID_BYTES);
    out[..n].copy_from_slice(&digest[..n]);
    out
}

pub fn routing_id_from_caid(caid: &ContentHash) -> [u8; ID_BYTES] {
    routing_id_from_digest(&caid.digest)
}

/// Routing id of a peer from its Ed25519 public key (same as `node_id` CAID).
pub fn routing_id_from_pubkey(pk: &[u8]) -> [u8; ID_BYTES] {
    let caid = Identity {
        public_key: pk.to_vec(),
        private_key: Vec::new(),
    }
    .node_id_caid();
    routing_id_from_caid(&caid)
}

pub fn routing_id_from_pubkey_hex(hex_s: &str) -> Option<[u8; ID_BYTES]> {
    let pk = hex::decode(hex_s.trim()).ok()?;
    if pk.is_empty() {
        return None;
    }
    Some(routing_id_from_pubkey(&pk))
}

pub fn xor_id(a: &[u8; ID_BYTES], b: &[u8; ID_BYTES]) -> [u8; ID_BYTES] {
    let mut o = [0u8; ID_BYTES];
    for i in 0..ID_BYTES {
        o[i] = a[i] ^ b[i];
    }
    o
}

/// Bucket index = leading zero bits of XOR(self, peer). 160 means equal (self).
pub fn bucket_index(self_id: &[u8; ID_BYTES], peer: &[u8; ID_BYTES]) -> usize {
    let x = xor_id(self_id, peer);
    let mut n = 0usize;
    for b in x.iter() {
        if *b == 0 {
            n += 8;
        } else {
            n += b.leading_zeros() as usize;
            break;
        }
    }
    n.min(N_BUCKETS)
}

/// Exactly 40 lowercase hex characters → 160-bit target.
pub fn parse_find_node_target(s: &str) -> Option<[u8; ID_BYTES]> {
    let t = s.trim();
    if t.len() != 40 {
        return None;
    }
    if !t.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    let bytes = hex::decode(t).ok()?;
    if bytes.len() != ID_BYTES {
        return None;
    }
    let mut out = [0u8; ID_BYTES];
    out.copy_from_slice(&bytes);
    Some(out)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoutingIndex {
    /// First 20 bytes of this node's node_id digest (hex for serde).
    pub self_id_hex: String,
    /// Per-bucket lists of `node_id` strings, insertion order (incumbents first).
    pub buckets: Vec<Vec<String>>,
    pub dropped_full: usize,
}

impl RoutingIndex {
    pub fn new(self_id: [u8; ID_BYTES]) -> Self {
        Self {
            self_id_hex: hex::encode(self_id),
            buckets: vec![Vec::new(); N_BUCKETS],
            dropped_full: 0,
        }
    }

    pub fn self_id(&self) -> [u8; ID_BYTES] {
        let b = hex::decode(&self.self_id_hex).unwrap_or_default();
        routing_id_from_digest(&b)
    }

    pub fn total(&self) -> usize {
        self.buckets.iter().map(|b| b.len()).sum()
    }

    /// Insert or refresh. Returns log line(s) for the operator console.
    ///
    /// - already present → refresh only (no occupancy change, no drop count)
    /// - self → no insert
    /// - bucket full → drop, count, log
    /// - else → push, log
    pub fn insert(&mut self, node_id: &str, peer_rid: [u8; ID_BYTES]) -> Vec<String> {
        let self_id = self.self_id();
        let b = bucket_index(&self_id, &peer_rid);
        if b >= N_BUCKETS {
            // Equal to self — never stored.
            return vec![];
        }
        // Already present in any bucket? Refresh only (position unchanged).
        for slot in &self.buckets {
            if slot.iter().any(|id| id == node_id) {
                return vec![];
            }
        }
        let slot = &mut self.buckets[b];
        if slot.len() >= K {
            self.dropped_full += 1;
            return vec![format!(
                "OODP Routing: bucket {b} full, incumbent kept, dropped {node_id}"
            )];
        }
        slot.push(node_id.to_string());
        let n = slot.len();
        vec![format!(
            "OODP Routing: +{node_id} bucket={b} occupancy={n}/{K}"
        )]
    }

    /// k closest known peers to `target` by XOR distance, ascending; ties by id.
    /// Searches the **whole table**, not one bucket.
    pub fn closest(
        &self,
        target: &[u8; ID_BYTES],
        adverts: &HashMap<String, PeerAdvert>,
        k: usize,
    ) -> Vec<String> {
        let mut scored: Vec<([u8; ID_BYTES], String)> = Vec::new();
        for slot in &self.buckets {
            for nid in slot {
                let Some(adv) = adverts.get(nid) else {
                    continue;
                };
                let Some(rid) = routing_id_from_pubkey_hex(&adv.public_key_hex) else {
                    continue;
                };
                scored.push((rid, nid.clone()));
            }
        }
        scored.sort_by(|a, b| {
            let da = xor_id(&a.0, target);
            let db = xor_id(&b.0, target);
            da.cmp(&db).then_with(|| a.0.cmp(&b.0))
        });
        scored.truncate(k);
        scored.into_iter().map(|(_, nid)| nid).collect()
    }

    pub fn format_cli(&self) -> String {
        let mut lines = Vec::new();
        for (i, slot) in self.buckets.iter().enumerate() {
            if slot.is_empty() {
                continue;
            }
            lines.push(format!("bucket {i}: {}", slot.len()));
        }
        lines.push(format!("total: {}", self.total()));
        lines.push(format!("dropped_full: {}", self.dropped_full));
        lines.join("\n")
    }
}

/// Workspace-local path for the process-shared index (not REAL_02 §5.1
/// `.oo/routing/` — that blueprint is deliberately not created).
pub fn index_path(base: &Path) -> PathBuf {
    base.join(".oo").join("oodp_index.json")
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OodpIndexFile {
    pub routing: RoutingIndex,
    /// Minimal advert fields needed for find_node relay + discover.
    pub adverts: HashMap<String, PeerAdvertSer>,
}

/// Serializable peer advert (SystemTime as unix secs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerAdvertSer {
    pub node_id: String,
    pub public_key_hex: String,
    pub services: Vec<String>,
    pub addr: String,
    pub observed_host: String,
    pub listen_port: u16,
    pub capacity: i64,
    pub ttl: i64,
    pub ts: i64,
    pub hops: i64,
    pub ad_source: String,
    pub received_at_secs: u64,
}

impl From<&PeerAdvert> for PeerAdvertSer {
    fn from(a: &PeerAdvert) -> Self {
        let received_at_secs = a
            .received_at
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            node_id: a.node_id.clone(),
            public_key_hex: a.public_key_hex.clone(),
            services: a.services.clone(),
            addr: a.addr.clone(),
            observed_host: a.observed_host.clone(),
            listen_port: a.listen_port,
            capacity: a.capacity,
            ttl: a.ttl,
            ts: a.ts,
            hops: a.hops,
            ad_source: a.ad_source.clone(),
            received_at_secs,
        }
    }
}

impl From<PeerAdvertSer> for PeerAdvert {
    fn from(a: PeerAdvertSer) -> Self {
        Self {
            node_id: a.node_id,
            public_key_hex: a.public_key_hex,
            services: a.services,
            addr: a.addr,
            observed_host: a.observed_host,
            listen_port: a.listen_port,
            capacity: a.capacity,
            ttl: a.ttl,
            ts: a.ts,
            hops: a.hops,
            ad_source: a.ad_source,
            received_at: std::time::UNIX_EPOCH
                + std::time::Duration::from_secs(a.received_at_secs),
        }
    }
}

pub fn load_index(base: &Path) -> Option<OodpIndexFile> {
    let p = index_path(base);
    let bytes = std::fs::read(&p).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn save_index(base: &Path, file: &OodpIndexFile) -> std::io::Result<()> {
    let p = index_path(base);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = p.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(file).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, e)
    })?;
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, &p)?;
    Ok(())
}
