//! Durable peer directory (advert_persistence arc).
//!
//! On-disk: `.oo/peers/directory` — append-only lines, header names the owning
//! `node_id`. The Kademlia bucket index is **not** stored; it is rebuilt on
//! load from records ordered by `received_at` (or signed `ts` on identity
//! mismatch). See `docs/advert_persistence_handover.md`.

use crate::routing::{self, RoutingIndex};
use crate::PeerAdvert;
use serde_json::{json, Map, Value as JsonValue};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const PEERS_DIR: &str = "peers";
pub const PEERS_FILE: &str = "directory";
pub const FORMAT_TAG: &str = "oodp-peers:v1";

/// Result of loading (or deciding not to load) the durable directory.
#[derive(Debug, Clone, Default)]
pub struct LoadReport {
    pub records: usize,
    pub skipped: usize,
    /// Records whose stored signature did not verify (acceptance repair).
    pub unverifiable: usize,
    /// Log line for the serving process (if anything was attempted).
    pub log_line: Option<String>,
}

/// In-process handle: tracks data-line count for the 2× compaction gate.
#[derive(Debug, Default)]
pub struct PeerDirectoryState {
    /// Number of data lines currently in the file (not the live map size).
    pub file_lines: usize,
}

pub fn directory_path(base_dir: &Path) -> PathBuf {
    base_dir.join(".oo").join(PEERS_DIR).join(PEERS_FILE)
}

fn header_line(owner_node_id: &str) -> String {
    format!("# {FORMAT_TAG} node_id={owner_node_id}")
}

fn parse_header(line: &str) -> Option<String> {
    let t = line.trim();
    if !t.starts_with('#') {
        return None;
    }
    let rest = t.trim_start_matches('#').trim();
    if !rest.starts_with(FORMAT_TAG) {
        return None;
    }
    let nid = rest
        .split_whitespace()
        .find_map(|tok| tok.strip_prefix("node_id="))?;
    if nid.is_empty() {
        return None;
    }
    Some(nid.to_string())
}

fn secs_since_epoch(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn system_time_from_secs(s: i64) -> SystemTime {
    if s >= 0 {
        UNIX_EPOCH + Duration::from_secs(s as u64)
    } else {
        UNIX_EPOCH
    }
}

/// Encode one accepted record as a single JSON line (ad source verbatim).
pub fn encode_record_line(adv: &PeerAdvert) -> String {
    let mut m = Map::new();
    m.insert("ad".into(), JsonValue::String(adv.ad_source.clone()));
    m.insert("node_id".into(), JsonValue::String(adv.node_id.clone()));
    m.insert(
        "public_key".into(),
        JsonValue::String(adv.public_key_hex.clone()),
    );
    m.insert(
        "services".into(),
        JsonValue::Array(
            adv.services
                .iter()
                .map(|s| JsonValue::String(s.clone()))
                .collect(),
        ),
    );
    m.insert("listen_port".into(), json!(adv.listen_port));
    m.insert("capacity".into(), json!(adv.capacity));
    m.insert("ts".into(), json!(adv.ts));
    m.insert("ttl".into(), json!(adv.ttl));
    // Asserted half — present in the file; load applies R1 identity split.
    m.insert(
        "observed_host".into(),
        JsonValue::String(adv.observed_host.clone()),
    );
    m.insert("hops".into(), json!(adv.hops));
    m.insert(
        "received_at".into(),
        json!(secs_since_epoch(adv.received_at)),
    );
    m.insert("addr".into(), JsonValue::String(adv.addr.clone()));
    serde_json::to_string(&JsonValue::Object(m)).unwrap_or_else(|_| "{}".into())
}

fn decode_record_line(line: &str, restore_asserted: bool) -> Option<PeerAdvert> {
    let v: JsonValue = serde_json::from_str(line.trim()).ok()?;
    let o = v.as_object()?;
    let ad_source = o.get("ad")?.as_str()?.to_string();
    let node_id = o
        .get("node_id")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if node_id.is_empty() {
        return None;
    }
    let public_key_hex = o
        .get("public_key")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if public_key_hex.is_empty() {
        return None;
    }
    let services: Vec<String> = o
        .get("services")
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let listen_port = o.get("listen_port").and_then(|x| x.as_u64()).unwrap_or(0) as u16;
    let capacity = o.get("capacity").and_then(|x| x.as_i64()).unwrap_or(0);
    let ts = o.get("ts").and_then(|x| x.as_i64()).unwrap_or(0);
    let ttl = o.get("ttl").and_then(|x| x.as_i64()).unwrap_or(0);

    let (observed_host, hops, received_at, addr) = if restore_asserted {
        let host = o
            .get("observed_host")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let hops = o.get("hops").and_then(|x| x.as_i64()).unwrap_or(0);
        let ra = o.get("received_at").and_then(|x| x.as_i64()).unwrap_or(ts);
        let addr = o
            .get("addr")
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                if host.is_empty() {
                    String::new()
                } else {
                    format!("{host}:{listen_port}")
                }
            });
        (host, hops, system_time_from_secs(ra), addr)
    } else {
        // R1 mismatch: signed half only; ordering falls back to signed `ts`.
        (String::new(), 0, system_time_from_secs(ts), String::new())
    };

    Some(PeerAdvert {
        node_id,
        public_key_hex,
        services,
        addr,
        observed_host,
        listen_port,
        capacity,
        ttl,
        ts,
        hops,
        ad_source,
        received_at,
        // Derived at load / accept — never read from the durable line.
        verified_operator_key: None,
    })
}

/// Load directory into memory maps. Does not mint a node key.
///
/// `this_node_id`: `Some` when a node key already exists; `None` → treat as
/// identity mismatch for asserted fields (signed half only) and leave routing
/// unseeded (self id stays zeros until first live accept).
pub fn load(
    base_dir: &Path,
    this_node_id: Option<&str>,
) -> (
    HashMap<String, PeerAdvert>,
    RoutingIndex,
    PeerDirectoryState,
    LoadReport,
) {
    let path = directory_path(base_dir);
    let empty_rt = RoutingIndex::new([0u8; 20]);
    let empty_state = PeerDirectoryState::default();
    if !path.exists() {
        return (HashMap::new(), empty_rt, empty_state, LoadReport::default());
    }
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => {
            // Unreadable cache → cold start.
            return (HashMap::new(), empty_rt, empty_state, LoadReport::default());
        }
    };
    let mut lines = text.lines();
    let Some(first) = lines.next() else {
        return (HashMap::new(), empty_rt, empty_state, LoadReport::default());
    };
    let Some(owner) = parse_header(first) else {
        // Header damage → treat as absent.
        return (HashMap::new(), empty_rt, empty_state, LoadReport::default());
    };

    let restore_asserted = this_node_id.map(|id| id == owner).unwrap_or(false);

    let mut by_id: HashMap<String, PeerAdvert> = HashMap::new();
    // Preserve last-wins while tracking file order of appearance for rebuild.
    let mut order: Vec<String> = Vec::new();
    let mut skipped = 0usize;
    let mut file_lines = 0usize;

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        file_lines += 1;
        match decode_record_line(line, restore_asserted) {
            Some(adv) => {
                let nid = adv.node_id.clone();
                if !by_id.contains_key(&nid) {
                    order.push(nid.clone());
                }
                by_id.insert(nid, adv);
            }
            None => skipped += 1,
        }
    }

    // Sort live records by received_at (then node_id) for incumbent-first replay.
    let mut sorted: Vec<PeerAdvert> = by_id.values().cloned().collect();
    sorted.sort_by(|a, b| {
        a.received_at
            .cmp(&b.received_at)
            .then_with(|| a.node_id.cmp(&b.node_id))
    });

    let routing = if let Some(self_caid) = this_node_id {
        // Seed with real self id and insert in time order.
        let sid = if let Ok(ch) = crate::value::ContentHash::parse(self_caid) {
            routing::routing_id_from_caid(&ch)
        } else {
            // Fallback: first 20 bytes of hex digest if bare.
            let dig = self_caid.rsplit(':').next().unwrap_or("");
            let bytes = hex::decode(dig).unwrap_or_default();
            routing::routing_id_from_digest(&bytes)
        };
        let mut rt = RoutingIndex::new(sid);
        for adv in &sorted {
            if let Some(rid) = routing::routing_id_from_pubkey_hex(&adv.public_key_hex) {
                let _ = rt.insert(&adv.node_id, rid);
            }
        }
        rt
    } else {
        // No node key yet: directory loads, index stays unseeded.
        RoutingIndex::new([0u8; 20])
    };

    let records = by_id.len();
    let report = LoadReport {
        records,
        skipped,
        unverifiable: 0,
        log_line: Some(format!(
            "OODP Peers: loaded {records} records, skipped {skipped} damaged"
        )),
    };
    (by_id, routing, PeerDirectoryState { file_lines }, report)
}

/// Append one accepted advert. May compact. Returns log lines for the serve console.
pub fn append(
    base_dir: &Path,
    owner_node_id: &str,
    advert: &PeerAdvert,
    live: &HashMap<String, PeerAdvert>,
    state: &mut PeerDirectoryState,
) -> Vec<String> {
    let path = directory_path(base_dir);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let mut logs = Vec::new();
    let line = encode_record_line(advert);
    let line_bytes = line.len() as u64 + 1; // + newline

    let need_header = !path.exists() || fs::metadata(&path).map(|m| m.len() == 0).unwrap_or(true);
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut f) => {
            if need_header {
                let h = header_line(owner_node_id);
                let _ = writeln!(f, "{h}");
            }
            if writeln!(f, "{line}").is_ok() {
                let _ = f.flush();
                state.file_lines += 1;
                let live_n = live.len();
                logs.push(format!(
                    "OODP Peers: append {line_bytes} bytes ({live_n} live)"
                ));
            }
        }
        Err(_) => return logs,
    }

    // Compaction gate: data lines > 2 × live unique records.
    let live_n = live.len().max(1);
    if state.file_lines > 2 * live_n {
        if let Some(c) = compact(base_dir, owner_node_id, live, state) {
            logs.push(c);
        }
    }
    logs
}

/// Rewrite the file with only the live set (received_at order).
pub fn compact(
    base_dir: &Path,
    owner_node_id: &str,
    live: &HashMap<String, PeerAdvert>,
    state: &mut PeerDirectoryState,
) -> Option<String> {
    let path = directory_path(base_dir);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let mut sorted: Vec<&PeerAdvert> = live.values().collect();
    sorted.sort_by(|a, b| {
        a.received_at
            .cmp(&b.received_at)
            .then_with(|| a.node_id.cmp(&b.node_id))
    });

    let mut body = String::new();
    body.push_str(&header_line(owner_node_id));
    body.push('\n');
    for adv in &sorted {
        body.push_str(&encode_record_line(adv));
        body.push('\n');
    }
    let bytes = body.len() as u64;
    let tmp = path.with_extension("directory.tmp");
    fs::write(&tmp, &body).ok()?;
    fs::rename(&tmp, &path).ok()?;
    state.file_lines = sorted.len();
    let live_n = live.len();
    Some(format!("OODP Peers: compact {bytes} bytes ({live_n} live)"))
}

/// Drop stored records whose signature does not verify, rebuild the index, and
/// re-derive affiliations.
///
/// ACCEPTANCE REPAIR (advert_persistence). `load` runs while the engine is
/// being constructed, so it cannot verify: `verify_relayed_entry` needs an
/// `&Ouroboros` to compute the body CAID. This pass runs once the engine
/// exists and prunes what the loader could only take on trust.
///
/// The ladder is the same one a relayed record passes — literal-body gate,
/// field presence, ttl range, `CAID(pk) == node_id`, Ed25519 — so a stored
/// record earns its place on exactly the terms it earned it on the wire.
///
/// ACCEPTANCE REPAIR (affiliation_claim, 2026-07-30). The delivery weakened
/// this from "drop" to "keep the row, clear `services`", because R9 tampered
/// with the affiliation signature and then required the peer to stay listed.
/// R9 was miscalibrated: the claim lives **inside** the node-signed body, so
/// tampering with it necessarily breaks the node signature too — there is no
/// such thing as a claim-only tamper, and the probe was asking for a state
/// that cannot arise. Measured cost of the weakening: 50 fabricated rows
/// appended to `.oo/peers/directory` were all listed by `oo node peers` and
/// all survived further activity on disk. `.oo/` is writable by any n/ program
/// (SPEC_08 §6.3), so "keep the row" is an unbounded, attacker-chosen listing.
/// Reverted; R9 now tests the one load-specific thing that is real — a claim
/// that expires between receipt and load.
pub fn verify_loaded(engine: &crate::Ouroboros) -> usize {
    let Some(base) = engine.base_dir.clone() else {
        return 0;
    };
    let _ = base;
    let bad: Vec<String> = {
        let Ok(dir) = engine.peer_adverts.read() else {
            return 0;
        };
        dir.values()
            .filter(|adv| crate::oodp::verify_stored_ad(engine, &adv.ad_source).is_err())
            .map(|adv| adv.node_id.clone())
            .collect()
    };
    if !bad.is_empty() {
        if let Ok(mut dir) = engine.peer_adverts.write() {
            for nid in &bad {
                dir.remove(nid);
            }
        }
    }
    // Rebuild the index over what survived, in the same replay order.
    if let (Ok(dir), Ok(mut rt)) = (engine.peer_adverts.read(), engine.routing.write()) {
        let self_id = rt.self_id();
        let mut sorted: Vec<&PeerAdvert> = dir.values().collect();
        sorted.sort_by(|a, b| {
            a.received_at
                .cmp(&b.received_at)
                .then_with(|| a.node_id.cmp(&b.node_id))
        });
        let mut fresh = RoutingIndex::new(self_id);
        for adv in sorted {
            if let Some(rid) = routing::routing_id_from_pubkey_hex(&adv.public_key_hex) {
                let _ = fresh.insert(&adv.node_id, rid);
            }
        }
        *rt = fresh;
    }
    // Re-derive affiliation from verbatim ad (path three of three).
    // Only succeeds when the body (incl. claim) is intact under the node sig.
    refresh_affiliations(engine);
    bad.len()
}

/// Recompute `verified_operator_key` for every peer from `ad_source`.
pub fn refresh_affiliations(engine: &crate::Ouroboros) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let Ok(mut dir) = engine.peer_adverts.write() else {
        return;
    };
    for adv in dir.values_mut() {
        adv.verified_operator_key =
            crate::oodp::verified_operator_of_ad_source(engine, &adv.ad_source, &adv.node_id, now);
    }
}
