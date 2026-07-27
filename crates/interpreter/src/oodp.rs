//! OODP wire envelope (REAL_02 §3.2 / oodp_packet_format arc).
//!
//! Request / response are JSON objects with `%`-prefixed keys (n/ meta axis
//! names). The value under `%result` is the engine's usual `serde_json` of
//! `Value` — one encoding, applied to the envelope as a whole.
//!
//! A peer's `%status` is a **claim**, not a verification. Clients must still
//! re-address `%result` against the requested CAID (REAL_03 §6.6).
//!
//! `%from` on requests is likewise a **claim, not authentication** — unsigned,
//! any peer can invent any value. Serving, verification and every outcome must
//! be identical whatever `%from` says (node_identity arc D3 / P1).

use crate::storage::{StoreReadError, value_address_matches};
use crate::value::{ContentHash, Value, BottomCause};
use crate::{IntegrityKind, Ouroboros};
use nlang_parser::ast::{AtomKind as AstAtom, ExprKind, FieldKey, Prefix};
use nlang_parser::parse_expr_only;
use serde_json::{json, Map, Value as JsonValue};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Default read timeout for peer fetch (D3). Connect timeout stays 5s.
pub const OODP_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OodpOp {
    Fetch,
    Discover,
    Advertise,
    Unknown(String),
}

impl OodpOp {
    pub fn as_tag(&self) -> &str {
        match self {
            OodpOp::Fetch => "fetch",
            OodpOp::Discover => "discover",
            OodpOp::Advertise => "advertise",
            OodpOp::Unknown(s) => s.as_str(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OodpRequest {
    pub op: OodpOp,
    pub hash: Option<ContentHash>,
    /// Claimed sender node id. **Never used for authorization** — parsed only
    /// for observability; see module docs.
    pub from: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OodpStatus {
    Success,
    NotFound,
    Conflict,
    /// Node understands the op name but does not serve it here.
    NotImplemented,
}

impl OodpStatus {
    pub fn as_tag(&self) -> &'static str {
        match self {
            OodpStatus::Success => "success",
            OodpStatus::NotFound => "not_found",
            OodpStatus::Conflict => "conflict",
            OodpStatus::NotImplemented => "not_implemented",
        }
    }
}

/// Encode a response envelope. `%result` absent when there is no payload.
pub fn encode_response(
    status: OodpStatus,
    result: Option<&Value>,
    source: &str,
    hops: i64,
) -> String {
    let mut map = Map::new();
    map.insert(
        "%status".to_string(),
        JsonValue::String(format!("#{}", status.as_tag())),
    );
    if let Some(v) = result {
        map.insert(
            "%result".to_string(),
            serde_json::to_value(v).unwrap_or(JsonValue::Null),
        );
    }
    map.insert("%source".to_string(), JsonValue::String(source.to_string()));
    map.insert("%hops".to_string(), json!(hops));
    serde_json::to_string(&JsonValue::Object(map)).unwrap_or_else(|_| "{}".to_string())
}

/// Parse a request line: n/ cocoon `{{ %op, %hash, %from? }}` or JSON envelope.
///
/// **D5 (node_identity):** the transition-period bare-CAID form is retired.
/// A bare CAID is a malformed request → `#conflict`, not a served fetch.
pub fn parse_request(line: &str) -> Result<OodpRequest, String> {
    let line = line.trim();
    if line.is_empty() {
        return Err("empty request".into());
    }

    // JSON envelope with %op
    if let Ok(j) = serde_json::from_str::<JsonValue>(line) {
        if let Some(obj) = j.as_object() {
            if obj.contains_key("%op") || obj.contains_key("op") {
                return parse_json_request(obj);
            }
        }
    }

    // n/ cocoon / combo
    if line.starts_with('{') {
        if let Ok(expr) = parse_expr_only(line) {
            if let Ok(req) = parse_nlang_request(&expr) {
                return Ok(req);
            }
        }
    }

    // Retired: bare CAID (v0.2.48 transition surface). Do not serve.
    if ContentHash::parse(line).is_ok() {
        return Err(
            "legacy bare-CAID request retired; use {{ %op: #fetch, %hash: \"…\" }}".into(),
        );
    }

    Err(format!("unrecognized OODP request: {line}"))
}

fn parse_json_request(obj: &Map<String, JsonValue>) -> Result<OodpRequest, String> {
    let op_s = obj
        .get("%op")
        .or_else(|| obj.get("op"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing %op".to_string())?;
    let op = parse_op_tag(op_s);
    let hash = obj
        .get("%hash")
        .or_else(|| obj.get("hash"))
        .and_then(|v| v.as_str())
        .and_then(|s| ContentHash::parse(s).ok());
    // %from is a claim — recorded, never consulted for outcomes.
    let from = obj
        .get("%from")
        .or_else(|| obj.get("from"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let _ = from.as_ref(); // silence until logging wants it
    Ok(OodpRequest { op, hash, from })
}

fn parse_op_tag(s: &str) -> OodpOp {
    let t = s.trim().trim_start_matches('#');
    match t {
        "fetch" => OodpOp::Fetch,
        "discover" => OodpOp::Discover,
        "advertise" => OodpOp::Advertise,
        other => OodpOp::Unknown(other.to_string()),
    }
}

fn field_key_name(key: &FieldKey) -> Option<String> {
    match key {
        FieldKey::Named { name, prefix } => {
            let p = match prefix {
                Some(Prefix::Meta) => "%",
                Some(Prefix::System) => "~%",
                Some(Prefix::Logic) => "/",
                Some(Prefix::Type) => "@",
                Some(Prefix::Private) => "~",
                Some(Prefix::Local) => "^",
                _ => "",
            };
            Some(format!("{p}{}", name.trim()))
        }
        FieldKey::Quoted(n) => Some(n.trim().to_string()),
        // Combo field keys like `%op` often parse as a bare Path segment
        // (not Named/Meta) — see parser FieldKey::Path for `%…`.
        FieldKey::Path(path) if path.segments.len() == 1 => {
            Some(path.segments[0].trim().to_string())
        }
        _ => None,
    }
}

fn parse_nlang_request(expr: &nlang_parser::ast::Expr) -> Result<OodpRequest, String> {
    let ExprKind::Combo { fields, .. } = &expr.kind else {
        return Err("request is not a combo".into());
    };
    let mut op = None;
    let mut hash = None;
    let mut from = None;
    for f in fields {
        let Some(name) = field_key_name(&f.key) else {
            continue;
        };
        match name.as_str() {
            "%op" | "op" => {
                op = Some(match &f.value.kind {
                    ExprKind::Atom(AstAtom::Tag(t)) => parse_op_tag(t),
                    ExprKind::Atom(AstAtom::Str(s)) => parse_op_tag(s),
                    _ => OodpOp::Unknown("?".into()),
                });
            }
            "%hash" | "hash" => {
                if let ExprKind::Atom(AstAtom::Str(s)) = &f.value.kind {
                    hash = ContentHash::parse(s).ok();
                }
            }
            "%from" | "from" => {
                // Claim only — never branch on this value.
                if let ExprKind::Atom(AstAtom::Str(s)) = &f.value.kind {
                    from = Some(s.clone());
                }
            }
            _ => {}
        }
    }
    let op = op.ok_or_else(|| "missing %op".to_string())?;
    let _ = from.as_ref();
    Ok(OodpRequest { op, hash, from })
}

/// Serve one OODP request against a local store. Returns wire JSON body.
///
/// `source_id` must be this node's id (CAID of the node public key), not a port.
/// Request `%from` is ignored for all outcomes (claim, not auth).
pub fn serve_request(engine: &Ouroboros, line: &str, source_id: &str) -> (String, String /* log line */) {
    let req = match parse_request(line) {
        Ok(r) => r,
        Err(e) => {
            let body = encode_response(OodpStatus::Conflict, None, source_id, 0);
            return (body, format!("OODP bad request: {e}"));
        }
    };
    // Deliberately do not consult req.from for any branch below.
    let _ = &req.from;

    match req.op {
        OodpOp::Fetch => {
            let Some(hash) = req.hash else {
                let body = encode_response(OodpStatus::Conflict, None, source_id, 0);
                return (body, "OODP #fetch missing %hash".into());
            };
            match engine.store.get_value(&hash) {
                Ok(val) => {
                    let body = encode_response(OodpStatus::Success, Some(&val), source_id, 0);
                    (body, format!("OODP Served: {hash}"))
                }
                Err(e) => match e.downcast_ref::<StoreReadError>() {
                    Some(StoreReadError::NotFound { .. }) | None => {
                        let body = encode_response(OodpStatus::NotFound, None, source_id, 0);
                        (body, format!("OODP Miss: {hash}"))
                    }
                    Some(StoreReadError::CaidMismatch { requested, recomputed }) => {
                        let body = encode_response(OodpStatus::Conflict, None, source_id, 0);
                        (
                            body,
                            format!(
                                "OODP integrity #caid_mismatch: {requested} (recomputed {recomputed})"
                            ),
                        )
                    }
                    Some(StoreReadError::ObjectUndecodable { requested, detail }) => {
                        let body = encode_response(OodpStatus::Conflict, None, source_id, 0);
                        (
                            body,
                            format!("OODP integrity #object_undecodable: {requested} ({detail})"),
                        )
                    }
                },
            }
        }
        OodpOp::Discover | OodpOp::Advertise => {
            // Known ops, not implemented on this node yet — explicit status.
            let body = encode_response(OodpStatus::NotImplemented, None, source_id, 0);
            (
                body,
                format!("OODP op #{} not implemented on this node", req.op.as_tag()),
            )
        }
        OodpOp::Unknown(ref name) => {
            let body = encode_response(OodpStatus::Conflict, None, source_id, 0);
            (body, format!("OODP unknown %op: #{name}"))
        }
    }
}

/// Client: fetch via OODP envelope, verify result address.
///
/// Outcomes:
/// - `Ok(val)` — `#success` and address matches
/// - `Err(MissingKey)` — peer `#not_found` (absence, not conflict)
/// - `Err(CaidMismatch)` — peer `#conflict`, bad envelope, or address fail
/// - `Err(Timeout)` — read/connect deadline (distinct from all three)
/// - `Err(Conflict)` — connection refused / other transport failure
pub fn remote_fetch_oodp(
    oo: &Ouroboros,
    addr: &str,
    hash: &ContentHash,
) -> Result<Value, BottomCause> {
    let sock_addr = addr.parse().map_err(|_| BottomCause::Conflict)?;
    let mut stream = TcpStream::connect_timeout(&sock_addr, Duration::from_secs(5)).map_err(
        |e| {
            if e.kind() == std::io::ErrorKind::TimedOut {
                BottomCause::PeerTimeout
            } else {
                BottomCause::Conflict
            }
        },
    )?;
    stream
        .set_read_timeout(Some(OODP_READ_TIMEOUT))
        .map_err(|_| BottomCause::Conflict)?;
    stream
        .set_write_timeout(Some(OODP_READ_TIMEOUT))
        .map_err(|_| BottomCause::Conflict)?;

    // Mint/load node identity on first network use (Q2). `%from` is always
    // present rather than sometimes empty. It is a claim on the wire — peers
    // must not trust it (and we never trust theirs).
    let from = match oo.node_id() {
        Ok(nid) => nid.to_string(),
        Err(_) => return Err(BottomCause::Conflict),
    };
    let req = format!(
        "{{{{ %op: #fetch, %hash: \"{}\", %from: \"{}\" }}}}\n",
        hash, from
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|_| BottomCause::Conflict)?;
    stream.flush().map_err(|_| BottomCause::Conflict)?;

    let mut buffer = Vec::new();
    match stream.read_to_end(&mut buffer) {
        Ok(0) => {
            // Clean close with no body — treat as transport absence, not timeout.
            return Err(BottomCause::Conflict);
        }
        Ok(_) => {}
        Err(e) => {
            let timed = e.kind() == std::io::ErrorKind::TimedOut
                || e.kind() == std::io::ErrorKind::WouldBlock;
            return Err(if timed {
                BottomCause::PeerTimeout
            } else {
                BottomCause::Conflict
            });
        }
    }

    let source = format!("tcp://{addr}");

    // Peer %status is a claim — always re-verify bytes (REAL_03 §6.6).
    let envelope: JsonValue = match serde_json::from_slice(&buffer) {
        Ok(v) => v,
        Err(_) => {
            // Legacy bare-value reply or garbage — try as Value and verify.
            return legacy_or_fail(oo, hash, &buffer, &source);
        }
    };

    let obj = match envelope.as_object() {
        Some(o) if o.contains_key("%status") || o.contains_key("status") => o,
        _ => return legacy_or_fail(oo, hash, &buffer, &source),
    };

    let status = obj
        .get("%status")
        .or_else(|| obj.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("#conflict");
    let status_tag = status.trim().trim_start_matches('#');

    match status_tag {
        "not_found" => return Err(BottomCause::MissingKey),
        "not_implemented" => {
            oo.record_integrity(hash, &source, IntegrityKind::Mismatch);
            return Err(BottomCause::CaidMismatch);
        }
        "conflict" => {
            oo.record_integrity(hash, &source, IntegrityKind::Mismatch);
            return Err(BottomCause::CaidMismatch);
        }
        "success" => {}
        _ => {
            oo.record_integrity(hash, &source, IntegrityKind::Undecodable);
            return Err(BottomCause::CaidMismatch);
        }
    }

    let result_j = obj.get("%result").or_else(|| obj.get("result")).ok_or_else(|| {
        oo.record_integrity(hash, &source, IntegrityKind::Undecodable);
        BottomCause::CaidMismatch
    })?;
    // `#success` with no result / null
    if result_j.is_null() {
        oo.record_integrity(hash, &source, IntegrityKind::Undecodable);
        return Err(BottomCause::CaidMismatch);
    }

    let val: Value = serde_json::from_value(result_j.clone()).map_err(|_| {
        oo.record_integrity(hash, &source, IntegrityKind::Undecodable);
        BottomCause::CaidMismatch
    })?;

    let recomputed = val.content_hash();
    if !value_address_matches(hash, &recomputed) {
        oo.record_integrity(hash, &source, IntegrityKind::Mismatch);
        return Err(BottomCause::CaidMismatch);
    }
    Ok(val)
}

fn legacy_or_fail(
    oo: &Ouroboros,
    hash: &ContentHash,
    buffer: &[u8],
    source: &str,
) -> Result<Value, BottomCause> {
    // Old protocol: bare JSON value. Still verify address — never trust the peer.
    let val: Value = match serde_json::from_slice(buffer) {
        Ok(v) => v,
        Err(_) => {
            oo.record_integrity(hash, source, IntegrityKind::Undecodable);
            return Err(BottomCause::CaidMismatch);
        }
    };
    let recomputed = val.content_hash();
    if !value_address_matches(hash, &recomputed) {
        oo.record_integrity(hash, source, IntegrityKind::Mismatch);
        return Err(BottomCause::CaidMismatch);
    }
    Ok(val)
}
