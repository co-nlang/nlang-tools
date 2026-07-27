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
use crate::value::{ContentHash, Value, BottomCause, ComboVal, Identity};
use crate::{IntegrityKind, Ouroboros, PeerAdvert};
use nlang_parser::ast::{AtomKind as AstAtom, Expr, ExprKind, FieldKey, Prefix};
use nlang_parser::parse_expr_only;
use ring::signature::{self, UnparsedPublicKey};
use serde_json::{json, Map, Value as JsonValue};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Default read timeout for peer fetch (D3). Connect timeout stays 5s.
pub const OODP_READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Domain separation for advertisement signatures (must match probe / CLI).
pub const ADVERT_DOMAIN: &str = "oodp-advert:v1:";

/// `|now − ts| ≤ STALE_SKEW_SECS` or `#stale`.
pub const STALE_SKEW_SECS: i64 = 60;

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
    /// Request understood but refused; discrimination lives in `%reason`.
    Rejected,
}

impl OodpStatus {
    pub fn as_tag(&self) -> &'static str {
        match self {
            OodpStatus::Success => "success",
            OodpStatus::NotFound => "not_found",
            OodpStatus::Conflict => "conflict",
            OodpStatus::NotImplemented => "not_implemented",
            OodpStatus::Rejected => "rejected",
        }
    }
}

/// Encode a response envelope. `%result` absent when there is no payload.
/// `%reason` is present **iff** status is `#rejected` (advertise_wire Q3).
pub fn encode_response(
    status: OodpStatus,
    result: Option<&Value>,
    source: &str,
    hops: i64,
) -> String {
    encode_response_reason(status, None, result, source, hops)
}

pub fn encode_response_reason(
    status: OodpStatus,
    reason: Option<&str>,
    result: Option<&Value>,
    source: &str,
    hops: i64,
) -> String {
    let mut map = Map::new();
    map.insert(
        "%status".to_string(),
        JsonValue::String(format!("#{}", status.as_tag())),
    );
    if matches!(status, OodpStatus::Rejected) {
        if let Some(r) = reason {
            map.insert(
                "%reason".to_string(),
                JsonValue::String(format!("#{r}")),
            );
        }
    }
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

fn rejected(source_id: &str, reason: &str, from: Option<&str>, detail: &str) -> (String, String) {
    let body = encode_response_reason(OodpStatus::Rejected, Some(reason), None, source_id, 0);
    let from_s = from.unwrap_or("?");
    let log = format!("OODP Advert rejected: #{reason} from={from_s} ({detail})");
    (body, log)
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
/// `peer_host` is the **observed** host of the TCP peer (for `#advertise` addr).
/// Request `%from` is ignored for `#fetch` outcomes (claim, not auth); for
/// `#advertise` it is checked against `%ad.node_id` only.
pub fn serve_request(
    engine: &Ouroboros,
    line: &str,
    source_id: &str,
    peer_host: &str,
) -> (String, String /* log line */) {
    let req = match parse_request(line) {
        Ok(r) => r,
        Err(e) => {
            let body = encode_response(OodpStatus::Conflict, None, source_id, 0);
            return (body, format!("OODP bad request: {e}"));
        }
    };

    match req.op {
        OodpOp::Fetch => {
            // Deliberately do not consult req.from for any fetch branch.
            let _ = &req.from;
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
        OodpOp::Advertise => serve_advertise(engine, line, req.from.as_deref(), source_id, peer_host),
        OodpOp::Discover => {
            let body = encode_response(OodpStatus::NotImplemented, None, source_id, 0);
            (
                body,
                "OODP op #discover not implemented on this node".into(),
            )
        }
        OodpOp::Unknown(ref name) => {
            let body = encode_response(OodpStatus::Conflict, None, source_id, 0);
            (body, format!("OODP unknown %op: #{name}"))
        }
    }
}

/// Evaluate an n/ expression to a Value.
pub fn eval_nlang_value(engine: &Ouroboros, src: &str) -> Result<Value, String> {
    let expr = parse_expr_only(src.trim()).map_err(|e| format!("parse: {e}"))?;
    let mut ctx = engine.eval_context();
    Ok(engine.eval(&expr, &mut ctx))
}

fn eval_expr_value(engine: &Ouroboros, expr: &Expr) -> Value {
    let mut ctx = engine.eval_context();
    engine.eval(expr, &mut ctx)
}

/// CAID string as produced by `~%Discovery./identify` (what advertisement
/// signatures commit to — see §3.2 and probe `caid_of`).
pub fn identify_caid(engine: &Ouroboros, val: &Value) -> Result<String, String> {
    let src = val.to_nlang(0);
    let id = eval_nlang_value(engine, &format!("~%Discovery./identify {src}"))?;
    Ok(id.to_string_plain())
}

/// CAID of a body written as n/ source (sender-side signing).
pub fn identify_caid_src(engine: &Ouroboros, body_src: &str) -> Result<String, String> {
    let id = eval_nlang_value(engine, &format!("~%Discovery./identify {body_src}"))?;
    Ok(id.to_string_plain())
}

fn extract_ad_expr(line: &str) -> Result<Expr, String> {
    let line = line.trim();
    // JSON path
    if let Ok(j) = serde_json::from_str::<JsonValue>(line) {
        if let Some(obj) = j.as_object() {
            if let Some(ad) = obj.get("%ad").or_else(|| obj.get("ad")) {
                // Re-embed as n/ is hard; convert via serde to Value instead.
                // For probe we only use n/ cocoon; JSON ad is optional.
                let s = serde_json::to_string(ad).map_err(|e| e.to_string())?;
                // Prefer parse if it looks like n/; else treat as value JSON later.
                if s.starts_with('{') {
                    if let Ok(e) = parse_expr_only(&s) {
                        return Ok(e);
                    }
                }
                return Err("json %ad not expressible as n/ combo".into());
            }
            return Err("missing %ad".into());
        }
    }
    let expr = parse_expr_only(line).map_err(|e| format!("parse request: {e}"))?;
    let ExprKind::Combo { fields, .. } = &expr.kind else {
        return Err("request is not a combo".into());
    };
    for f in fields {
        let Some(name) = field_key_name(&f.key) else {
            continue;
        };
        if name == "%ad" || name == "ad" {
            return Ok(f.value.clone());
        }
    }
    Err("missing %ad".into())
}

/// Nesting cap for an advertisement body. The body is a flat record whose only
/// nested member is `services`, so 8 is generous. Bounded on purpose: an
/// unbounded recursive walk over remote input is itself a way to fell the node.
const MAX_AD_DEPTH: usize = 8;

/// Is this expression literal **data**? Allow-list, not deny-list: everything
/// unlisted is refused, so a future AST node is refused until someone decides
/// it belongs on the wire.
///
/// ACCEPTOR REPAIR (advertise_wire) — see the call site in `serve_advertise`.
fn ensure_literal_body(expr: &Expr, depth: usize) -> Result<(), String> {
    if depth > MAX_AD_DEPTH {
        return Err(format!("advertisement nested deeper than {MAX_AD_DEPTH}"));
    }
    match &expr.kind {
        ExprKind::Atom(a) => match a {
            AstAtom::Int(_)
            | AstAtom::Float(_)
            | AstAtom::Str(_)
            | AstAtom::MultilineStr(_)
            | AstAtom::Tag(_)
            | AstAtom::Bytes(_)
            | AstAtom::Top
            | AstAtom::Bottom
            | AstAtom::Unit => Ok(()),
            // Interpolation, URIs, regexes, times and path literals all reach
            // back into the engine or the host on evaluation.
            other => Err(format!("advertisement holds a non-data atom: {other:?}")),
        },
        ExprKind::List(items) | ExprKind::Tuple(items) => items
            .iter()
            .try_for_each(|e| ensure_literal_body(e, depth + 1)),
        ExprKind::Combo {
            fields, relations, ..
        } => {
            if !relations.is_empty() {
                return Err("advertisement carries order relations, not data".into());
            }
            for f in fields {
                if field_key_name(&f.key).is_none() {
                    return Err("advertisement has a computed field key".into());
                }
                ensure_literal_body(&f.value, depth + 1)?;
            }
            Ok(())
        }
        // Apply / Pipe / Morphism / Path / arithmetic / Ternary / Lens / Spread
        // / Structural / Context / AnonSet / Interpolated / Range …
        _ => Err("advertisement body must be literal data, not an expression".into()),
    }
}

fn field_as_str(cv: &ComboVal, key: &str) -> Option<String> {
    cv.get_field(key).map(|v| v.to_string_plain())
}

fn field_as_i64(cv: &ComboVal, key: &str) -> Option<i64> {
    let v = cv.get_field(key)?;
    let s = v.to_string_plain();
    s.parse().ok()
}

/// Services list: do **not** sort/dedupe (M5 — order is significant for CAID).
fn field_as_str_list(cv: &ComboVal, key: &str) -> Option<Vec<String>> {
    let v = cv.get_field(key)?;
    match v {
        Value::Combo(list) => {
            // List is a combo with "0","1",… and %kind: #list
            let mut pairs: Vec<(usize, String)> = Vec::new();
            for (k, val) in list.all_fields_iter() {
                if k.starts_with('%') || k.starts_with("~%") {
                    continue;
                }
                if let Ok(i) = k.parse::<usize>() {
                    pairs.push((i, val.to_string_plain()));
                }
            }
            pairs.sort_by_key(|(i, _)| *i);
            Some(pairs.into_iter().map(|(_, s)| s).collect())
        }
        Value::Union(branches) => Some(branches.iter().map(|b| b.to_string_plain()).collect()),
        _ => Some(vec![v.to_string_plain()]),
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Advertisement verification ladder (advertise_wire §3.4). Order is normative.
fn serve_advertise(
    engine: &Ouroboros,
    line: &str,
    from: Option<&str>,
    source_id: &str,
    peer_host: &str,
) -> (String, String) {
    // 1 — %ad present, combo, required fields
    let ad_expr = match extract_ad_expr(line) {
        Ok(e) => e,
        Err(e) => return rejected(source_id, "malformed", from, &e),
    };
    // ACCEPTOR REPAIR (advertise_wire). An advertisement body is DATA, and the
    // engine must not run the interpreter on it. As delivered, the next line
    // was `eval_expr_value(engine, &ad_expr)` — the FIRST thing done with an
    // unauthenticated packet, before any of §3.4's five checks. Measured on the
    // delivered build:
    //
    //   {{ %op: #advertise, %from: "x",
    //      %ad: ~%Io./write_file("/tmp/pwned.txt", "owned…") }}
    //   → {"%status":"#rejected","%reason":"#malformed"}   and the file exists
    //
    // No signature, no key, no identity: an arbitrary effect on any node the
    // attacker can reach. The verdict was right and the effect had already
    // happened. That inverts the arc's own thesis — this arc exists so the wire
    // authenticates its speaker, and it ran the speaker's payload first.
    //
    // The gate is structural and comes before evaluation: only literal atoms,
    // lists/tuples and combos of literals are an advertisement. Evaluating a
    // *validated literal* is inert, so the CAID keeps being computed exactly as
    // both sides already sign it (§3.2) — this repair changes no wire bytes.
    if let Err(why) = ensure_literal_body(&ad_expr, 0) {
        return rejected(source_id, "malformed", from, &why);
    }
    let ad_val = eval_expr_value(engine, &ad_expr);
    let Value::Combo(mut cv) = ad_val else {
        return rejected(source_id, "malformed", from, "%ad is not a combo");
    };

    let required = [
        "node_id",
        "public_key",
        "signature",
        "services",
        "listen_port",
        "capacity",
        "ts",
        "ttl",
    ];
    for k in required {
        if cv.get_field(k).is_none() {
            return rejected(
                source_id,
                "malformed",
                from,
                &format!("missing required field `{k}`"),
            );
        }
    }

    let node_id = match field_as_str(&cv, "node_id") {
        Some(s) if !s.is_empty() => s,
        _ => return rejected(source_id, "malformed", from, "empty node_id"),
    };
    let public_key_hex = match field_as_str(&cv, "public_key") {
        Some(s) if !s.is_empty() => s,
        _ => return rejected(source_id, "malformed", from, "empty public_key"),
    };
    let signature_hex = match field_as_str(&cv, "signature") {
        Some(s) if !s.is_empty() => s,
        _ => return rejected(source_id, "malformed", from, "empty signature"),
    };
    let listen_port = match field_as_i64(&cv, "listen_port") {
        Some(p) if p > 0 && p <= u16::MAX as i64 => p as u16,
        _ => return rejected(source_id, "malformed", from, "bad listen_port"),
    };
    let capacity = field_as_i64(&cv, "capacity").unwrap_or(0);
    let ts = match field_as_i64(&cv, "ts") {
        Some(t) => t,
        None => return rejected(source_id, "malformed", from, "bad ts"),
    };
    let ttl = field_as_i64(&cv, "ttl").unwrap_or(0);
    let services = field_as_str_list(&cv, "services").unwrap_or_default();

    // 2 — CAID(public_key bytes) == node_id
    let pk_bytes = match hex::decode(public_key_hex.trim()) {
        Ok(b) if !b.is_empty() => b,
        _ => {
            return rejected(
                source_id,
                "malformed",
                from,
                "public_key is not hex",
            );
        }
    };
    // Same computation as Identity::node_id_caid: CAID of Bytes(pubkey).
    let id_from_key = {
        let tmp = Identity {
            public_key: pk_bytes.clone(),
            private_key: Vec::new(),
        };
        tmp.node_id_caid().to_string()
    };
    if id_from_key != node_id {
        return rejected(
            source_id,
            "identity_mismatch",
            from,
            "CAID(public_key) ≠ node_id",
        );
    }

    // 3 — %from == %ad.node_id
    match from {
        Some(f) if f == node_id => {}
        Some(_) => {
            return rejected(
                source_id,
                "identity_mismatch",
                from,
                "%from ≠ %ad.node_id",
            );
        }
        None => {
            return rejected(
                source_id,
                "identity_mismatch",
                from,
                "missing %from",
            );
        }
    }

    // 4 — signature over oodp-advert:v1: + CAID(body without signature).
    // CAID must be computed the same way the probe/CLI signs: via
    // `~%Discovery./identify` (morph application can differ from a bare
    // `content_hash()` of `eval(body)` — measured).
    cv.remove_field("signature");
    let body_val = Value::Combo(cv);
    let body_caid = match identify_caid(engine, &body_val) {
        Ok(c) => c,
        Err(e) => {
            return rejected(source_id, "malformed", from, &format!("body caid: {e}"));
        }
    };
    let payload = format!("{ADVERT_DOMAIN}{body_caid}");
    let sig_bytes = match hex::decode(signature_hex.trim()) {
        Ok(b) => b,
        Err(_) => {
            return rejected(source_id, "malformed", from, "signature is not hex");
        }
    };
    let vk = UnparsedPublicKey::new(&signature::ED25519, &pk_bytes);
    if vk.verify(payload.as_bytes(), &sig_bytes).is_err() {
        return rejected(
            source_id,
            "bad_signature",
            from,
            "Ed25519 verification failed",
        );
    }

    // 5 — freshness
    let now = now_secs();
    if (now - ts).abs() > STALE_SKEW_SECS {
        return rejected(
            source_id,
            "stale",
            from,
            &format!("|now−ts|={}s > {STALE_SKEW_SECS}s", (now - ts).abs()),
        );
    }

    // Success — store in peer directory (host observed, port claimed).
    let addr = format!("{peer_host}:{listen_port}");
    let n_services = services.len();
    engine.record_peer_advert(PeerAdvert {
        node_id: node_id.clone(),
        public_key_hex,
        services,
        addr: addr.clone(),
        capacity,
        ttl,
        received_at: SystemTime::now(),
    });

    let body = encode_response(OodpStatus::Success, None, source_id, 0);
    let log = format!(
        "OODP Advert: {node_id} addr={addr} services={n_services} ttl={ttl}"
    );
    (body, log)
}

/// Format an OODP `#advertise` request line for the wire.
pub fn format_advertise_request(from: &str, ad_nlang: &str) -> String {
    format!("{{{{ %op: #advertise, %from: \"{from}\", %ad: {ad_nlang} }}}}\n")
}

/// n/ source of a signed advertisement (for CLI).
pub fn signed_advert_nlang(
    identity: &Identity,
    services: &[String],
    listen_port: u16,
    capacity: i64,
    ttl: i64,
    engine: &Ouroboros,
) -> Result<(String /* ad nlang */, String /* node_id */, String /* request line */), String> {
    let node_id = identity.node_id_caid().to_string();
    let pk = identity.public_key_hex();
    let ts = now_secs();
    let services_n = services
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let body_src = format!(
        "{{{{ node_id: \"{node_id}\", public_key: \"{pk}\", services: [{services_n}], \
         listen_port: {listen_port}, capacity: {capacity}, ts: {ts}, ttl: {ttl} }}}}"
    );
    let body_caid = identify_caid_src(engine, &body_src)?;
    let payload = format!("{ADVERT_DOMAIN}{body_caid}");
    let key_pair = signature::Ed25519KeyPair::from_pkcs8(&identity.private_key)
        .map_err(|e| format!("node key: {e:?}"))?;
    let sig = hex::encode(key_pair.sign(payload.as_bytes()).as_ref());
    let ad = format!(
        "{{{{ node_id: \"{node_id}\", public_key: \"{pk}\", services: [{services_n}], \
         listen_port: {listen_port}, capacity: {capacity}, ts: {ts}, ttl: {ttl}, \
         signature: \"{sig}\" }}}}"
    );
    let req = format_advertise_request(&node_id, &ad);
    Ok((ad, node_id, req))
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
















#[cfg(test)]
mod advert_debug {
    use super::*;
    use crate::Ouroboros;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    fn identify_caid(oo: &Ouroboros, val_or_src: &str) -> String {
        eval_nlang_value(oo, &format!("~%Discovery./identify {val_or_src}"))
            .unwrap()
            .to_string_plain()
    }

    #[test]
    fn sign_via_identify_serve() {
        let dir = tempfile::tempdir().unwrap();
        let oo = Ouroboros::init(dir.path()).unwrap();
        let id = oo.node_identity().unwrap();
        let node_id = id.node_id_caid().to_string();
        let pk = id.public_key_hex();
        let ts = now_secs();
        let body = format!(
            "{{{{ node_id: \"{node_id}\", public_key: \"{pk}\", services: [], listen_port: 8080, capacity: 10, ts: {ts}, ttl: 15 }}}}"
        );
        let caid = identify_caid(&oo, &body);
        let payload = format!("{ADVERT_DOMAIN}{caid}");
        let kp = Ed25519KeyPair::from_pkcs8(&id.private_key).unwrap();
        let sig = hex::encode(kp.sign(payload.as_bytes()).as_ref());
        let ad = format!(
            "{{{{ node_id: \"{node_id}\", public_key: \"{pk}\", services: [], listen_port: 8080, capacity: 10, ts: {ts}, ttl: 15, signature: \"{sig}\" }}}}"
        );
        let req = format!("{{{{ %op: #advertise, %from: \"{node_id}\", %ad: {ad} }}}}\n");

        // What would server compute?
        let ad_val = eval_nlang_value(&oo, &ad).unwrap();
        let Value::Combo(mut cv) = ad_val else { panic!() };
        cv.remove_field("signature");
        let body_val = Value::Combo(cv);
        let body_nlang = body_val.to_nlang(0);
        println!("body_nlang:\n{body_nlang}");
        let server_caid = identify_caid(&oo, &body_nlang);
        println!("sign caid={caid}");
        println!("server caid={server_caid}");
        assert_eq!(caid, server_caid);

        // Patch verify to use identify — test serve after we fix
        let (reply, log) = serve_request(&oo, &req, "src", "127.0.0.1");
        println!("reply={reply}\nlog={log}");
    }
}
