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
use ring::rand::{SecureRandom, SystemRandom};
use ring::signature::{self, UnparsedPublicKey};
use serde_json::{json, Map, Value as JsonValue};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Default read timeout for peer fetch (D3). Connect timeout stays 5s.
pub const OODP_READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Domain separation for advertisement signatures (must match probe / CLI).
pub const ADVERT_DOMAIN: &str = "oodp-advert:v1:";

/// Affiliation claim domain (operator key; affiliation_claim / #3c-a).
/// Carries `:v1:` — unlike `refine:`, which omitted one.
pub const AFFILIATION_DOMAIN: &str = "oodp-affiliation:v1:";

/// Maximum claim lifetime (ruling 4). Same style as `STALE_SKEW_SECS`.
pub const MAX_AFFILIATION_LIFETIME_SECS: i64 = 30 * 24 * 3600;

/// Signed affiliation payload: binds claim to **this** node and **this** expiry.
pub fn affiliation_payload(node_id: &str, expires: i64) -> String {
    format!("{AFFILIATION_DOMAIN}{node_id}:{expires}")
}

/// `|now − ts| ≤ STALE_SKEW_SECS` or `#stale` at **#advertise** accept.
pub const STALE_SKEW_SECS: i64 = 60;

/// Local availability bound for **index search / relay receive** (R-f):
/// 15 minutes. Not a wire field — engine policy, visible in the discover log.
pub const DISCOVER_STALE_SECS: i64 = 15 * 60;

/// Max `%peers` entries per `#discover` response (R-b / §3.6).
pub const MAX_DISCOVER_PEERS: usize = 8;

/// Max `#discover` response body size (bytes).
pub const MAX_DISCOVER_RESPONSE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OodpOp {
    Fetch,
    Discover,
    Advertise,
    FindNode,
    Unknown(String),
}

impl OodpOp {
    pub fn as_tag(&self) -> &str {
        match self {
            OodpOp::Fetch => "fetch",
            OodpOp::Discover => "discover",
            OodpOp::Advertise => "advertise",
            OodpOp::FindNode => "find_node",
            OodpOp::Unknown(s) => s.as_str(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OodpRequest {
    pub op: OodpOp,
    pub hash: Option<ContentHash>,
    /// Raw `%hash` string when the field was present but failed CAID parse.
    /// Distinguishes `#missing_field` from `#unparseable_caid` (wire_says_why).
    pub hash_raw: Option<String>,
    /// Claimed sender node id. **Never used for authorization** on fetch/discover
    /// (R-c) — parsed only for observability. On `#advertise` it is checked
    /// against `%ad.node_id`.
    pub from: Option<String>,
    /// `#discover` target CAID (required for that op).
    pub target: Option<String>,
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
/// `%reason` is required on every non-`#success` (wire_says_why / REAL_02 §3.2).
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
    // wire_says_why R1: every non-success carries %reason. Older clients ignore
    // unknown fields; reject-only reason is no longer the fence.
    if !matches!(status, OodpStatus::Success) {
        if let Some(r) = reason {
            let tag = if r.starts_with('#') {
                r.to_string()
            } else {
                format!("#{r}")
            };
            map.insert("%reason".to_string(), JsonValue::String(tag));
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

/// Non-success envelope with a required reason tag (no leading `#` needed).
fn refuse(status: OodpStatus, reason: &str, source_id: &str) -> String {
    encode_response_reason(status, Some(reason), None, source_id, 0)
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
    let hash_raw = obj
        .get("%hash")
        .or_else(|| obj.get("hash"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let hash = hash_raw.as_deref().and_then(|s| ContentHash::parse(s).ok());
    // %from is a claim — recorded, never consulted for outcomes.
    let from = obj
        .get("%from")
        .or_else(|| obj.get("from"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let target = obj
        .get("%target")
        .or_else(|| obj.get("target"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok(OodpRequest {
        op,
        hash,
        hash_raw,
        from,
        target,
    })
}

fn parse_op_tag(s: &str) -> OodpOp {
    let t = s.trim().trim_start_matches('#');
    match t {
        "fetch" => OodpOp::Fetch,
        "discover" => OodpOp::Discover,
        "advertise" => OodpOp::Advertise,
        "find_node" => OodpOp::FindNode,
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
    let mut hash_raw = None;
    let mut from = None;
    let mut target = None;
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
                match &f.value.kind {
                    ExprKind::Atom(AstAtom::Str(s)) => {
                        hash_raw = Some(s.clone());
                        hash = ContentHash::parse(s).ok();
                    }
                    other => {
                        // Field present but not a CAID string.
                        hash_raw = Some(format!("{other:?}"));
                        hash = None;
                    }
                }
            }
            "%from" | "from" => {
                if let ExprKind::Atom(AstAtom::Str(s)) = &f.value.kind {
                    from = Some(s.clone());
                }
            }
            "%target" | "target" => {
                if let ExprKind::Atom(AstAtom::Str(s)) = &f.value.kind {
                    target = Some(s.clone());
                }
            }
            _ => {}
        }
    }
    let op = op.ok_or_else(|| "missing %op".to_string())?;
    Ok(OodpRequest {
        op,
        hash,
        hash_raw,
        from,
        target,
    })
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
            let body = refuse(OodpStatus::Conflict, "malformed", source_id);
            return (body, format!("OODP bad request: {e}"));
        }
    };

    match req.op {
        OodpOp::Fetch => {
            // Deliberately do not consult req.from for any fetch branch.
            let _ = &req.from;
            let hash = match (&req.hash, &req.hash_raw) {
                (Some(h), _) => h.clone(),
                (None, Some(raw)) => {
                    let body = refuse(OodpStatus::Conflict, "unparseable_caid", source_id);
                    return (body, format!("OODP #fetch unparseable %hash: {raw}"));
                }
                (None, None) => {
                    let body = refuse(OodpStatus::Conflict, "missing_field", source_id);
                    return (body, "OODP #fetch missing %hash".into());
                }
            };
            match engine.store.get_value(&hash) {
                Ok(val) => {
                    let body = encode_response(OodpStatus::Success, Some(&val), source_id, 0);
                    (body, format!("OODP Served: {hash}"))
                }
                Err(e) => match e.downcast_ref::<StoreReadError>() {
                    Some(StoreReadError::NotFound { .. }) | None => {
                        let body = refuse(OodpStatus::NotFound, "not_held", source_id);
                        (body, format!("OODP Miss: {hash}"))
                    }
                    Some(StoreReadError::CaidMismatch { requested, recomputed }) => {
                        let body = refuse(OodpStatus::Conflict, "caid_mismatch", source_id);
                        (
                            body,
                            format!(
                                "OODP integrity #caid_mismatch: {requested} (recomputed {recomputed})"
                            ),
                        )
                    }
                    Some(StoreReadError::ObjectUndecodable { requested, detail }) => {
                        // Corrupt-on-disk is still the integrity half of §3.2.
                        let body = refuse(OodpStatus::Conflict, "caid_mismatch", source_id);
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
            // %from is a claim on #discover (R-c) — never branch on it.
            serve_discover(
                engine,
                req.target.as_deref(),
                req.from.as_deref(),
                source_id,
            )
        }
        OodpOp::FindNode => {
            // %from is a claim — never inserted into the table (R-b).
            serve_find_node(engine, req.target.as_deref(), source_id)
        }
        OodpOp::Unknown(ref name) => {
            let body = refuse(OodpStatus::NotImplemented, "unknown_op", source_id);
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

/// Extract `%ad` as AST plus **verbatim** source substring (for relay).
fn extract_ad_expr_and_source(line: &str) -> Result<(Expr, String), String> {
    let line = line.trim();
    // JSON path: carry the JSON text of %ad as "source" (rare on this arc).
    if let Ok(j) = serde_json::from_str::<JsonValue>(line) {
        if let Some(obj) = j.as_object() {
            if let Some(ad) = obj.get("%ad").or_else(|| obj.get("ad")) {
                let s = serde_json::to_string(ad).map_err(|e| e.to_string())?;
                if s.starts_with('{') {
                    if let Ok(e) = parse_expr_only(&s) {
                        return Ok((e, s));
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
            let start = f.value.span.start;
            let end = f.value.span.end;
            let src = if end <= line.len() && start < end {
                line[start..end].to_string()
            } else {
                // Fallback: re-serialise (last resort; relay prefers span bytes).
                f.value.to_nlang(0)
            };
            return Ok((f.value.clone(), src));
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

/// Verify an optional `affiliation` block on an advert body (signature already
/// stripped or still present — field is independent). **Additive only**: never
/// fails the advert; returns `None` when absent or unproven.
pub fn verified_operator_of_body(body: &Value, advert_node_id: &str, now: i64) -> Option<String> {
    let Value::Combo(cv) = body else {
        return None;
    };
    let aff = cv.get_field("affiliation")?;
    let Value::Combo(ac) = aff else {
        return None;
    };
    let op_key = field_as_str(ac, "operator_key")?;
    let sig_hex = field_as_str(ac, "signature")?;
    let expires = field_as_i64(ac, "expires")?;
    if expires <= now {
        return None;
    }
    if expires > now + MAX_AFFILIATION_LIFETIME_SECS {
        return None;
    }
    let op_bytes = hex::decode(op_key.trim()).ok()?;
    if op_bytes.is_empty() {
        return None;
    }
    let sig_bytes = hex::decode(sig_hex.trim()).ok()?;
    let payload = affiliation_payload(advert_node_id, expires);
    let vk = UnparsedPublicKey::new(&signature::ED25519, &op_bytes);
    if vk.verify(payload.as_bytes(), &sig_bytes).is_err() {
        return None;
    }
    Some(op_key.trim().to_lowercase())
}

/// Re-derive a verified operator key from verbatim `%ad` source (load / peers).
pub fn verified_operator_of_ad_source(
    engine: &Ouroboros,
    ad_src: &str,
    advert_node_id: &str,
    now: i64,
) -> Option<String> {
    let ad_expr = parse_expr_only(ad_src.trim()).ok()?;
    ensure_literal_body(&ad_expr, 0).ok()?;
    let ad_val = eval_expr_value(engine, &ad_expr);
    let Value::Combo(mut cv) = ad_val else {
        return None;
    };
    // Body CAID ignores signature; affiliation is part of the signed body.
    cv.remove_field("signature");
    verified_operator_of_body(&Value::Combo(cv), advert_node_id, now)
}

/// Path of the durable claim beside the node key: `{node_key_path}.affiliation`.
pub fn affiliation_claim_path(node_key_path: &std::path::Path) -> std::path::PathBuf {
    let mut s = node_key_path.as_os_str().to_os_string();
    s.push(".affiliation");
    std::path::PathBuf::from(s)
}

/// On-disk claim (public material only). Not a secret.
#[derive(Debug, Clone)]
pub struct AffiliationClaim {
    pub operator_key: String,
    pub signature: String,
    pub expires: i64,
}

impl AffiliationClaim {
    pub fn to_nlang_block(&self) -> String {
        format!(
            "affiliation: {{{{ operator_key: \"{}\", signature: \"{}\", expires: {} }}}}",
            self.operator_key, self.signature, self.expires
        )
    }

    pub fn write_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Simple line format — public fields only.
        let body = format!(
            "operator_key: {}\nsignature: {}\nexpires: {}\n",
            self.operator_key, self.signature, self.expires
        );
        std::fs::write(path, body)
    }

    pub fn read_file(path: &std::path::Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        let mut operator_key = None;
        let mut signature = None;
        let mut expires = None;
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("operator_key:") {
                operator_key = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("signature:") {
                signature = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("expires:") {
                expires = rest.trim().parse().ok();
            }
        }
        Some(Self {
            operator_key: operator_key?,
            signature: signature?,
            expires: expires?,
        })
    }

    /// Still within lifetime and under the MAX ceiling.
    pub fn is_live(&self, now: i64) -> bool {
        self.expires > now && self.expires <= now + MAX_AFFILIATION_LIFETIME_SECS
    }
}

/// Sign an affiliation claim with the operator identity for `node_id`.
pub fn mint_affiliation_claim(
    operator: &Identity,
    node_id: &str,
    expires: i64,
) -> Result<AffiliationClaim, String> {
    use ring::signature::KeyPair;
    if expires <= now_secs() {
        return Err("expires must be in the future".into());
    }
    if expires > now_secs() + MAX_AFFILIATION_LIFETIME_SECS {
        return Err(format!(
            "expires exceeds maximum lifetime of {MAX_AFFILIATION_LIFETIME_SECS}s"
        ));
    }
    let payload = affiliation_payload(node_id, expires);
    let key_pair = signature::Ed25519KeyPair::from_pkcs8(&operator.private_key)
        .map_err(|e| format!("operator key: {e:?}"))?;
    let sig = hex::encode(key_pair.sign(payload.as_bytes()).as_ref());
    Ok(AffiliationClaim {
        operator_key: operator.public_key_hex(),
        signature: sig,
        expires,
    })
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
    let (ad_expr, ad_source) = match extract_ad_expr_and_source(line) {
        Ok(pair) => pair,
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
    let ttl = match field_as_i64(&cv, "ttl") {
        Some(t) if (0..=15).contains(&t) => t,
        Some(_) => {
            return rejected(
                source_id,
                "malformed",
                from,
                "ttl outside 0..=15 (REAL_02 §4.2)",
            );
        }
        None => return rejected(source_id, "malformed", from, "bad ttl"),
    };
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

    // Affiliation (optional, additive): never rejects the advert.
    let verified_operator_key = verified_operator_of_body(&body_val, &node_id, now);

    // Success — store in peer directory (host observed, port claimed).
    // Direct advertise → arrival hops = 0 (REAL_02 §3.2).
    let addr = format!("{peer_host}:{listen_port}");
    let n_services = services.len();
    let routing_logs = engine.record_peer_advert(PeerAdvert {
        node_id: node_id.clone(),
        public_key_hex,
        services,
        addr: addr.clone(),
        observed_host: peer_host.to_string(),
        listen_port,
        capacity,
        ttl,
        ts,
        hops: 0,
        ad_source,
        received_at: SystemTime::now(),
        verified_operator_key: verified_operator_key.clone(),
    });

    let body = encode_response(OodpStatus::Success, None, source_id, 0);
    let mut log = format!(
        "OODP Advert: {node_id} addr={addr} services={n_services} ttl={ttl}"
    );
    if let Some(ref op) = verified_operator_key {
        log.push_str(&format!(" affiliation={op}"));
    }
    for line in routing_logs {
        log.push('\n');
        log.push_str(&line);
    }
    (body, log)
}

// ── #find_node (Kademlia closest) ───────────────────────────────────────

fn serve_find_node(
    engine: &Ouroboros,
    target: Option<&str>,
    source_id: &str,
) -> (String, String) {
    let Some(t) = target else {
        let body = refuse(OodpStatus::Conflict, "missing_field", source_id);
        return (body, "OODP #find_node missing %target".into());
    };
    let Some(target_id) = crate::routing::parse_find_node_target(t) else {
        // Wrong shape / length — not a well-formed target.
        let body = refuse(OodpStatus::Conflict, "malformed", source_id);
        return (
            body,
            format!("OODP #find_node %target must be 40 lowercase hex: {t}"),
        );
    };

    let closest_ids = {
        let rt = match engine.routing.read() {
            Ok(r) => r,
            Err(_) => {
                let body = refuse(OodpStatus::Conflict, "malformed", source_id);
                return (body, "OODP #find_node routing lock poisoned".into());
            }
        };
        let ads = match engine.peer_adverts.read() {
            Ok(d) => d.clone(),
            Err(_) => HashMap::new(),
        };
        // Ensure self_id is set for distance (may be zeros before first insert).
        rt.closest(
            &target_id,
            &ads,
            crate::routing::MAX_FIND_NODE_PEERS,
        )
    };

    let ads = engine.peer_adverts.read().ok();
    let mut peers: Vec<(String, String)> = Vec::new();
    if let Some(ref map) = ads {
        for nid in closest_ids {
            if let Some(adv) = map.get(&nid) {
                peers.push((adv.ad_source.clone(), adv.observed_host.clone()));
                let trial = encode_discover_response(source_id, 1, &peers);
                if trial.len() > MAX_DISCOVER_RESPONSE_BYTES {
                    peers.pop();
                    break;
                }
            }
        }
    }
    let body = encode_discover_response(source_id, 1, &peers);
    let log = format!(
        "OODP FindNode: target={} peers={}",
        hex::encode(target_id),
        peers.len()
    );
    (body, log)
}

// ── #discover (service index) ───────────────────────────────────────────

fn encode_discover_response(
    source: &str,
    hops: i64,
    peers: &[(String /* ad_source */, String /* observed_host */)],
) -> String {
    let mut map = Map::new();
    map.insert(
        "%status".to_string(),
        JsonValue::String("#success".into()),
    );
    map.insert("%source".to_string(), JsonValue::String(source.to_string()));
    map.insert("%hops".to_string(), json!(hops));
    let peers_j: Vec<JsonValue> = peers
        .iter()
        .map(|(ad, host)| {
            let mut e = Map::new();
            e.insert("%ad".to_string(), JsonValue::String(ad.clone()));
            // R1 / advert_persistence: an observation that was never made
            // (empty host after identity mismatch load) must not be claimed.
            if !host.is_empty() {
                e.insert(
                    "%observed_host".to_string(),
                    JsonValue::String(host.clone()),
                );
            }
            JsonValue::Object(e)
        })
        .collect();
    map.insert("%peers".to_string(), JsonValue::Array(peers_j));
    serde_json::to_string(&JsonValue::Object(map)).unwrap_or_else(|_| "{}".to_string())
}

/// Search the advert directory for exact `services` matches; emit relayed
/// `%peers`. Does **not** consult the object store (discover_index §3.2).
fn serve_discover(
    engine: &Ouroboros,
    target: Option<&str>,
    from: Option<&str>,
    source_id: &str,
) -> (String, String) {
    let Some(target) = target.filter(|t| !t.is_empty()) else {
        let body = refuse(OodpStatus::Conflict, "missing_field", source_id);
        return (body, "OODP #discover missing %target".into());
    };
    // Target must be a well-formed CAID string (or at least non-garbage).
    if ContentHash::parse(target).is_err() && !target.starts_with("hash:") {
        let body = refuse(OodpStatus::Conflict, "unparseable_caid", source_id);
        return (
            body,
            format!("OODP #discover unparseable %target: {target}"),
        );
    }

    let now = now_secs();
    let dir = match engine.peer_adverts.read() {
        Ok(d) => d,
        Err(_) => {
            let body = refuse(OodpStatus::Conflict, "malformed", source_id);
            return (body, "OODP #discover directory lock poisoned".into());
        }
    };

    let mut matched = 0usize;
    let mut excl_no_relay = 0usize;
    let mut excl_stale = 0usize;
    let mut candidates: Vec<&PeerAdvert> = Vec::new();

    for adv in dir.values() {
        // Exact string match on services — no sort/dedupe of the list (M5).
        if !adv.services.iter().any(|s| s == target) {
            continue;
        }
        matched += 1;
        // Exclusion BEFORE the cap (§3.2).
        if adv.ttl == 0 {
            excl_no_relay += 1;
            continue;
        }
        if (now - adv.ts).abs() > DISCOVER_STALE_SECS {
            excl_stale += 1;
            continue;
        }
        candidates.push(adv);
    }

    // Cap after exclusions (§4.3.2 before §4.3.5). Under the cap, return all.
    // Overflow → uniform sample without replacement, **per query**
    // (discover_sampling / #3c-b1). Not HashMap iteration order, not capacity-
    // weighted, not asker-keyed (REAL_02 §3.2).
    let mut candidates = candidates;
    sample_uniform_cap(&mut candidates, MAX_DISCOVER_PEERS);

    let mut peers: Vec<(String, String)> = Vec::new();
    for adv in candidates {
        peers.push((adv.ad_source.clone(), adv.observed_host.clone()));
        // 64 KiB body budget — emit fewer if over.
        let trial = encode_discover_response(source_id, 1, &peers);
        if trial.len() > MAX_DISCOVER_RESPONSE_BYTES {
            peers.pop();
            break;
        }
    }

    let body = encode_discover_response(source_id, 1, &peers);
    let from_s = from.unwrap_or("");
    let log = format!(
        "OODP Discover: target={target} matched={matched} capped={} excluded={excl_no_relay} no_relay,{excl_stale} stale from={from_s}",
        peers.len()
    );
    (body, log)
}

/// Uniform sample without replacement down to at most `k` items, in place.
/// When `items.len() <= k`, the vector is unchanged (every candidate returned).
/// Partial Fisher–Yates; each query draws independently (ring SystemRandom).
fn sample_uniform_cap<T>(items: &mut Vec<T>, k: usize) {
    let n = items.len();
    if n <= k || k == 0 {
        if k == 0 {
            items.clear();
        }
        return;
    }
    let rng = SystemRandom::new();
    for i in 0..k {
        // Uniform j in [i, n).
        let j = i + random_below(&rng, n - i);
        items.swap(i, j);
    }
    items.truncate(k);
}

/// Uniform integer in `0..bound` (rejection sampling so the modulus is unbiased).
fn random_below(rng: &SystemRandom, bound: usize) -> usize {
    if bound <= 1 {
        return 0;
    }
    let bound = bound as u64;
    // Largest multiple of `bound` that fits in u64.
    let max = u64::MAX - (u64::MAX % bound);
    loop {
        let mut buf = [0u8; 8];
        // SystemRandom::fill is fallible only on OS entropy failure; treat as 0.
        if rng.fill(&mut buf).is_err() {
            return 0;
        }
        let v = u64::from_le_bytes(buf);
        if v < max {
            return (v % bound) as usize;
        }
    }
}

/// One accepted peer from a `#discover` reply (after §3.4 verification).
#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    pub node_id: String,
    pub observed_host: String,
    pub listen_port: u16,
    pub public_key_hex: String,
    pub ad_source: String,
    /// Verified affiliation operator key, if any (affiliation_claim).
    pub verified_operator_key: Option<String>,
}

#[derive(Debug, Default)]
pub struct DiscoverProcessResult {
    pub peers_in: usize,
    pub accepted: Vec<DiscoveredPeer>,
    pub dropped: usize,
    pub drop_reasons: std::collections::BTreeMap<String, usize>,
    pub envelope_hops: i64,
    pub status: String,
}

fn bump(map: &mut std::collections::BTreeMap<String, usize>, key: &str) {
    *map.entry(key.to_string()).or_insert(0) += 1;
}

/// Verify each `%peers` entry independently (R-e). Body is data: `ensure_literal_body`
/// before any evaluation (§3.5).
pub fn process_discover_reply(
    engine: &Ouroboros,
    reply_body: &str,
) -> DiscoverProcessResult {
    let mut result = DiscoverProcessResult::default();

    let Ok(envelope) = serde_json::from_str::<JsonValue>(reply_body.trim()) else {
        result.status = "undecodable".into();
        return result;
    };
    let status = envelope
        .get("%status")
        .or_else(|| envelope.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("#conflict");
    result.status = status.trim().trim_start_matches('#').to_string();
    result.envelope_hops = envelope
        .get("%hops")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    if result.status == "not_implemented" {
        return result;
    }
    if result.status != "success" {
        return result;
    }

    let Some(arr) = envelope
        .get("%peers")
        .or_else(|| envelope.get("peers"))
        .and_then(|v| v.as_array())
    else {
        return result;
    };
    result.peers_in = arr.len();
    let now = now_secs();

    for entry in arr {
        let Some(ad_src) = entry
            .get("%ad")
            .or_else(|| entry.get("ad"))
            .and_then(|v| v.as_str())
        else {
            bump(&mut result.drop_reasons, "malformed");
            result.dropped += 1;
            continue;
        };
        let observed_host = entry
            .get("%observed_host")
            .or_else(|| entry.get("observed_host"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        match verify_relayed_entry(engine, ad_src, now) {
            Ok(peer) => {
                result.accepted.push(DiscoveredPeer {
                    observed_host,
                    ..peer
                });
            }
            Err(reason) => {
                bump(&mut result.drop_reasons, &reason);
                result.dropped += 1;
            }
        }
    }
    result
}

/// §3.4 ladder for one relayed `%ad` source. No `%from` check (speaker is the
/// relayer). Returns node_id / listen_port / pk on success.
/// Verify a stored advertisement body exactly as a relayed one is verified.
///
/// ACCEPTANCE REPAIR (advert_persistence). The durable directory holds
/// **signed** records, and R1's ruling — "the signed face travels, because it
/// is true whoever holds it" — is only true of a signature somebody checks. An
/// unverified signed record is not a self-authenticating fact; it is an
/// assertion wearing a signature, which is precisely the stratum confusion
/// discussion 025 exists to keep apart.
///
/// Measured before the repair: corrupting sixteen hex digits of a stored
/// signature and restarting produced `loaded 1 records, skipped 0 damaged`,
/// and the node relayed the record. Since `.oo/` is writable by any n/ program
/// (`~%Io./write_file`, open ledger item since the `#pin` arc), that is a
/// free and permanent seat in this node's routing table — which is exactly
/// what SPEC_15 §7.1's cost model prices in minted identities, and it costs
/// none of them.
pub fn verify_stored_ad(engine: &Ouroboros, ad_src: &str) -> Result<(), String> {
    verify_relayed_entry(engine, ad_src, now_secs()).map(|_| ())
}

fn verify_relayed_entry(
    engine: &Ouroboros,
    ad_src: &str,
    now: i64,
) -> Result<DiscoveredPeer, String> {
    // 1 — parse + literal-body gate BEFORE any eval
    let ad_expr = parse_expr_only(ad_src.trim()).map_err(|e| format!("malformed:{e}"))?;
    ensure_literal_body(&ad_expr, 0).map_err(|e| format!("malformed:{e}"))?;

    let ad_val = eval_expr_value(engine, &ad_expr);
    let Value::Combo(mut cv) = ad_val else {
        return Err("malformed".into());
    };
    for k in [
        "node_id",
        "public_key",
        "signature",
        "services",
        "listen_port",
        "capacity",
        "ts",
        "ttl",
    ] {
        if cv.get_field(k).is_none() {
            return Err("malformed".into());
        }
    }

    let node_id = field_as_str(&cv, "node_id").ok_or_else(|| "malformed".to_string())?;
    let public_key_hex = field_as_str(&cv, "public_key").ok_or_else(|| "malformed".to_string())?;
    let signature_hex = field_as_str(&cv, "signature").ok_or_else(|| "malformed".to_string())?;
    let listen_port = field_as_i64(&cv, "listen_port").ok_or_else(|| "malformed".to_string())? as u16;
    let ts = field_as_i64(&cv, "ts").ok_or_else(|| "malformed".to_string())?;
    let ttl = field_as_i64(&cv, "ttl").ok_or_else(|| "malformed".to_string())?;

    // 4 — ttl range (receiver); 0 is valid and may be accepted on the wire
    // even though an honest index would not *emit* it (R7).
    if !(0..=15).contains(&ttl) {
        return Err("malformed".into());
    }

    // 2 — CAID(pk) == node_id  (before signature — forger supplies both)
    let pk_bytes = hex::decode(public_key_hex.trim()).map_err(|_| "malformed".to_string())?;
    let id_from_key = Identity {
        public_key: pk_bytes.clone(),
        private_key: Vec::new(),
    }
    .node_id_caid()
    .to_string();
    if id_from_key != node_id {
        return Err("identity_mismatch".into());
    }

    // 3 — signature
    cv.remove_field("signature");
    let body_val = Value::Combo(cv);
    let body_caid = identify_caid(engine, &body_val).map_err(|_| "malformed".to_string())?;
    let payload = format!("{ADVERT_DOMAIN}{body_caid}");
    let sig_bytes = hex::decode(signature_hex.trim()).map_err(|_| "malformed".to_string())?;
    let vk = UnparsedPublicKey::new(&signature::ED25519, &pk_bytes);
    if vk.verify(payload.as_bytes(), &sig_bytes).is_err() {
        return Err("bad_signature".into());
    }

    // 5 — local staleness (R-f)
    if (now - ts).abs() > DISCOVER_STALE_SECS {
        return Err("stale".into());
    }

    // Affiliation: additive; does not affect accept/reject of the peer record.
    let verified_operator_key = verified_operator_of_body(&body_val, &node_id, now);

    Ok(DiscoveredPeer {
        node_id,
        observed_host: String::new(), // filled by caller
        listen_port,
        public_key_hex,
        ad_source: ad_src.to_string(),
        verified_operator_key,
    })
}

/// Client: send `#find_node` and process the reply (same peer ladder as discover).
pub fn remote_find_node_oodp(
    oo: &Ouroboros,
    addr: &str,
    target_hex: &str,
) -> Result<DiscoverProcessResult, BottomCause> {
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

    let from = oo.node_id().map(|n| n.to_string()).unwrap_or_default();
    let req = format!(
        "{{{{ %op: #find_node, %from: \"{from}\", %target: \"{target_hex}\" }}}}\n"
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|_| BottomCause::Conflict)?;
    stream.flush().map_err(|_| BottomCause::Conflict)?;

    // 64 KiB client bound with #oversize naming (same as discover).
    let mut buffer = Vec::new();
    let mut limited = (&mut stream).take(MAX_DISCOVER_RESPONSE_BYTES as u64 + 1);
    match limited.read_to_end(&mut buffer) {
        Ok(0) => return Err(BottomCause::Conflict),
        Ok(n) if n > MAX_DISCOVER_RESPONSE_BYTES => {
            let mut r = DiscoverProcessResult {
                status: "oversize".into(),
                ..Default::default()
            };
            bump(&mut r.drop_reasons, "oversize");
            return Ok(r);
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
    let text = String::from_utf8_lossy(&buffer);
    // Old peers answer unknown ops with #conflict — surface as status, not crash.
    Ok(process_discover_reply(oo, &text))
}

/// Client: send `#discover` and process the reply.
pub fn remote_discover_oodp(
    oo: &Ouroboros,
    addr: &str,
    target: &str,
) -> Result<DiscoverProcessResult, BottomCause> {
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

    let from = oo.node_id().map(|n| n.to_string()).unwrap_or_default();
    let req = format!(
        "{{{{ %op: #discover, %from: \"{from}\", %target: \"{target}\" }}}}\n"
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|_| BottomCause::Conflict)?;
    stream.flush().map_err(|_| BottomCause::Conflict)?;

    // ACCEPTOR REPAIR (discover_index). §3.6's budget was delivered on the
    // responder — correctly, 8 peers and 64 KiB — but a budget only the honest
    // side keeps is not a budget. Measured on the delivered build: a relayer
    // that streams without ever pausing sent 67.1 MB, the client buffered all
    // of it and peaked at 143 MB RSS, parsing 8156 entries. `read_to_end`
    // has no bound, and `set_read_timeout` fires on a STALL, not on volume —
    // a sender that keeps the bytes coming never trips it.
    //
    // The verification ladder held throughout (all 8156 dropped as
    // `#malformed`), so this is resource, not bypass. The bound here is not a
    // new policy: it is the same 64 KiB §3.6 already puts on the responder,
    // now enforced symmetrically by the side that can actually be hurt.
    // `#fetch` shares the unbounded read (ledger item 1) and is deliberately
    // NOT changed — an object has no specified maximum size, so capping it
    // would need a spec ruling; a discover reply already has one.
    let mut buffer = Vec::new();
    match (&mut stream)
        .take(MAX_DISCOVER_RESPONSE_BYTES as u64 + 1)
        .read_to_end(&mut buffer)
    {
        Ok(0) => return Err(BottomCause::Conflict),
        Ok(n) if n > MAX_DISCOVER_RESPONSE_BYTES => {
            let mut r = DiscoverProcessResult {
                status: "oversize".into(),
                ..Default::default()
            };
            // Named, not silent: a truncated prefix must never be processed as
            // if it were a short answer.
            bump(&mut r.drop_reasons, "oversize");
            return Ok(r);
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
    let text = String::from_utf8_lossy(&buffer);
    Ok(process_discover_reply(oo, &text))
}

/// Format an OODP `#advertise` request line for the wire.
pub fn format_advertise_request(from: &str, ad_nlang: &str) -> String {
    format!("{{{{ %op: #advertise, %from: \"{from}\", %ad: {ad_nlang} }}}}\n")
}

/// n/ source of a signed advertisement (for CLI).
///
/// If a live affiliation claim sits beside the node key, it is embedded in the
/// body **before** the node signs (affiliation_claim §3.3). Serving never
/// needs the operator private key.
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
    // Optional live claim (public material only) — no operator private key.
    let aff_extra = if let Some(base) = engine.base_dir.as_ref() {
        if let Ok(nk) = Identity::node_key_path(base) {
            let path = affiliation_claim_path(&nk);
            if let Some(claim) = AffiliationClaim::read_file(&path) {
                if claim.is_live(ts) {
                    format!(
                        ", affiliation: {{{{ operator_key: \"{}\", signature: \"{}\", expires: {} }}}}",
                        claim.operator_key, claim.signature, claim.expires
                    )
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    let body_src = format!(
        "{{{{ node_id: \"{node_id}\", public_key: \"{pk}\", services: [{services_n}], \
         listen_port: {listen_port}, capacity: {capacity}, ts: {ts}, ttl: {ttl}{aff_extra} }}}}"
    );
    let body_caid = identify_caid_src(engine, &body_src)?;
    let payload = format!("{ADVERT_DOMAIN}{body_caid}");
    let key_pair = signature::Ed25519KeyPair::from_pkcs8(&identity.private_key)
        .map_err(|e| format!("node key: {e:?}"))?;
    let sig = hex::encode(key_pair.sign(payload.as_bytes()).as_ref());
    let ad = format!(
        "{{{{ node_id: \"{node_id}\", public_key: \"{pk}\", services: [{services_n}], \
         listen_port: {listen_port}, capacity: {capacity}, ts: {ts}, ttl: {ttl}{aff_extra}, \
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
    let reason_tag = obj
        .get("%reason")
        .or_else(|| obj.get("reason"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().trim_start_matches('#').to_string());

    // wire_says_why §3.2 — integrity incidents only for substantiated
    // #caid_mismatch. Protocol-level answers are peer causes, not corruption.
    match status_tag {
        "not_found" => return Err(BottomCause::MissingKey),
        "not_implemented" => return Err(BottomCause::PeerNotImplemented),
        "conflict" => {
            if reason_tag.as_deref() == Some("caid_mismatch") {
                oo.record_integrity(hash, &source, IntegrityKind::Mismatch);
                return Err(BottomCause::CaidMismatch);
            }
            // Other reasons, or no reason (older peer): refusal, not accusation.
            return Err(BottomCause::PeerRefused);
        }
        "rejected" => return Err(BottomCause::PeerRefused),
        "success" => {}
        _ => return Err(BottomCause::PeerUnknownStatus),
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
