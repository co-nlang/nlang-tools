//! Encoding 5: durable `.oo/` objects as n/ text inside a store frame.
//!
//! The frame lives **outside** the value (O31). Two literals are legal only
//! inside it:
//!
//! * (c-1) `~%Name:` system-axis keys — reconstructed onto `ComboVal.system`.
//!   Ordinary eval still mints ⊥ `#system_reserved` (G3 / L2-60 / L2-61).
//! * (c-2) `~%__nlang_effect:` — a store-only system key the decoder strips
//!   and applies to the effect **slot**. It is never left as a value field,
//!   so it cannot move a CAID. Combos that already carry hashed `%effect`
//!   meta (O61 materialize) print that field and do not use this tag.
//!
//! Reading is decoding, not evaluation (O35): the body is parsed to an AST
//! and walked into `Value` / `Commit`. One parser, one reconstruction.

use crate::value::{
    BlurCause, BlurDetail, BottomCause, BottomDetail, CaidVersion, ComboVal, Commit, CommitKind,
    CommitMeta, ContentHash, EffectTag, HashAlgorithm, HorizonParams, MasaRef, ObservationStrategy,
    RefineInfo, Value,
};
use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use nlang_parser::ast::{AtomKind, Expr, ExprKind, Field, FieldKey, Path, Prefix};
use std::sync::Arc;

pub const FRAME: &str = "#nlang/store";

const THUNK: &str = "__nlang_thunk";
const EXPR: &str = "__nlang_expr";
const CLOSURE: &str = "__nlang_closure";
const CONTEXT: &str = "__nlang_context";
const EFFECT: &str = "__nlang_effect";
const CODE: &str = "__nlang_code";
const BOTTOM: &str = "__nlang_bottom";
const BLUR: &str = "__nlang_blur";
const BYTES: &str = "__nlang_bytes";
const RANK: &str = "__nlang_rank";
const REF: &str = "__nlang_ref";
const TOP_CAUSE: &str = "__nlang_top_cause";
const HASH: &str = "__nlang_hash";

#[derive(Debug)]
pub enum StoreDocument {
    Value(Value),
    Commit(Commit),
    Staged(ComboVal),
}

pub fn is_framed(bytes: &str) -> bool {
    bytes.trim_start().starts_with(FRAME)
}

pub fn is_framed_bytes(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).ok().is_some_and(is_framed)
}

/// True when the file is a CAS *value* (root / standard root / put_value),
/// not a commit. Encoding 4 values open with `{"Combo":`; encoding 5 values
/// open with `#nlang/store` and not `commit`.
pub fn is_cas_value_object(bytes: &[u8]) -> bool {
    if let Ok(s) = std::str::from_utf8(bytes) {
        let t = s.trim_start();
        if t.starts_with(FRAME) {
            return !t[FRAME.len()..].trim_start().starts_with("commit");
        }
    }
    bytes.starts_with(br#"{"Combo":"#)
}

/// Read a commit object written in either encoding. Encoding 5 is n/;
/// encoding ≤ 4 is the historical serde JSON. The JSON view keeps the old
/// `root.digest` 32-int array so existing probes that look at the commit
/// as JSON keep measuring the same property.
pub fn value_json_view(bytes: &[u8]) -> anyhow::Result<serde_json::Value> {
    if let Ok(s) = std::str::from_utf8(bytes) {
        if is_framed(s) {
            let v = decode_value(s)?;
            return Ok(serde_json::to_value(&v)?);
        }
    }
    Ok(serde_json::from_slice(bytes)?)
}

pub fn object_json_view(bytes: &[u8]) -> anyhow::Result<serde_json::Value> {
    if let Ok(s) = std::str::from_utf8(bytes) {
        if is_framed(s) {
            let rest = s.trim_start();
            if rest[FRAME.len()..].trim_start().starts_with("commit") {
                return commit_json_view(bytes);
            }
            return value_json_view(bytes);
        }
    }
    serde_json::from_slice(bytes).map_err(|e| anyhow::anyhow!("{e}"))
}

pub fn commit_json_view(bytes: &[u8]) -> anyhow::Result<serde_json::Value> {
    if let Ok(s) = std::str::from_utf8(bytes) {
        if is_framed(s) {
            let c = decode_commit(s)?;
            return Ok(serde_json::to_value(&c)?);
        }
    }
    Ok(serde_json::from_slice(bytes)?)
}

/// 64-hex digest of the universe root named by a commit object.
pub fn commit_root_digest_hex(bytes: &[u8]) -> Option<String> {
    let j = commit_json_view(bytes).ok()?;
    hex_of_digest_field(j.get("root")?.get("digest")?)
}

fn hex_of_digest_field(v: &serde_json::Value) -> Option<String> {
    if let Some(s) = v.as_str() {
        return (s.len() == 64).then(|| s.to_string());
    }
    let a = v.as_array()?;
    let hex: String = a
        .iter()
        .map(|b| format!("{:02x}", b.as_u64().unwrap_or(0)))
        .collect();
    (hex.len() == 64).then_some(hex)
}

/// Standard-root digest a root object names, from bytes, without the engine.
pub fn named_standard_digest(bytes: &str) -> Option<String> {
    let i = bytes.find("__nlang_system_digest")?;
    let tail = &bytes[i..];
    // encoding 4: `"Str":"<64 hex>"`   encoding 5: `: "hex"` after the key
    if let Some(j) = tail.find("\"Str\":\"") {
        let hex: String = tail[j + 7..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .collect();
        if hex.len() == 64 {
            return Some(hex);
        }
    }
    if let Some(j) = tail.find(": \"") {
        let hex: String = tail[j + 3..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .collect();
        if hex.len() == 64 {
            return Some(hex);
        }
    }
    None
}

pub fn encode_value(value: &Value) -> String {
    format!("{FRAME}\n{}", write_value(value, 0))
}

pub fn encode_commit(commit: &Commit) -> String {
    format!("{FRAME} commit\n{}", write_commit(commit))
}

pub fn encode_staged(combo: &ComboVal) -> String {
    format!("{FRAME} staged\n{}", write_combo(combo, 0))
}

pub fn decode_document(bytes: &str) -> Result<StoreDocument> {
    let rest = bytes.trim_start();
    if !rest.starts_with(FRAME) {
        anyhow::bail!("not a store document");
    }
    let after = &rest[FRAME.len()..];
    let (kind, body) = match after.strip_prefix(" commit") {
        Some(rest) => ("commit", rest),
        None => match after.strip_prefix(" staged") {
            Some(rest) => ("staged", rest),
            None => ("value", after),
        },
    };
    let body = body.trim_start_matches(['\r', '\n', ' ', '\t']);
    match kind {
        "commit" => Ok(StoreDocument::Commit(decode_commit_body(body)?)),
        "staged" => match expr_to_value(&parse_body(body)?)? {
            Value::Combo(c) => Ok(StoreDocument::Staged(c)),
            other => anyhow::bail!("staged document is not a combo: {other:?}"),
        },
        _ => Ok(StoreDocument::Value(expr_to_value(&parse_body(body)?)?)),
    }
}

pub fn decode_value(bytes: &str) -> Result<Value> {
    match decode_document(bytes)? {
        StoreDocument::Value(v) => Ok(v),
        StoreDocument::Staged(c) => Ok(Value::Combo(c)),
        StoreDocument::Commit(_) => anyhow::bail!("store document is a commit, not a value"),
    }
}

pub fn decode_commit(bytes: &str) -> Result<Commit> {
    match decode_document(bytes)? {
        StoreDocument::Commit(c) => Ok(c),
        _ => anyhow::bail!("store document is not a commit"),
    }
}

pub fn decode_staged(bytes: &str) -> Result<ComboVal> {
    match decode_document(bytes)? {
        StoreDocument::Staged(c) => Ok(c),
        StoreDocument::Value(Value::Combo(c)) => Ok(c),
        _ => anyhow::bail!("store document is not a staged combo"),
    }
}

/// Digests this document names (GC walk). Encoding 5 dropped the three
/// JSON-syntax scans; this is the semantic replacement.
pub fn refs_of_document(bytes: &str) -> Vec<String> {
    refs_of_document_ex(bytes, false)
}

pub fn refs_of_document_ex(bytes: &str, follow_abandoned: bool) -> Vec<String> {
    let mut out = Vec::new();
    match decode_document(bytes) {
        Ok(StoreDocument::Value(v)) => refs_of_value(&v, &mut out),
        Ok(StoreDocument::Staged(c)) => refs_of_combo(&c, &mut out),
        Ok(StoreDocument::Commit(c)) => refs_of_commit(&c, follow_abandoned, &mut out),
        Err(_) => {}
    }
    out
}

fn parse_body(body: &str) -> Result<Expr> {
    nlang_parser::parse_expr_only(body).map_err(|e| anyhow!("store n/ parse: {e}"))
}

// ── write ────────────────────────────────────────────────────────────────

fn write_value(v: &Value, indent: usize) -> String {
    match v {
        Value::Top => "_".into(),
        Value::TopCaused { cause, members } => write_wrapper(
            TOP_CAUSE,
            &[
                ("cause", write_tag(cause)),
                (
                    "members",
                    format!(
                        "[{}]",
                        members
                            .iter()
                            .map(|m| quote_string(m))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ),
            ],
            indent,
        ),
        Value::Atom(kind, effect, rank) => write_atom(kind, *effect, *rank, indent),
        Value::Combo(c) => write_combo(c, indent),
        Value::Union(branches) => {
            let parts: Vec<String> = branches.iter().map(|b| write_value(b, indent)).collect();
            if parts.len() == 1 {
                parts.into_iter().next().unwrap()
            } else {
                format!("({})", parts.join(" | "))
            }
        }
        Value::Code(expr) => write_wrapper(CODE, &[(EXPR, expr.to_nlang(0))], indent),
        Value::Thunk {
            expr,
            closure,
            context,
            effect,
        } => write_thunk(expr, closure, context.as_deref(), *effect, indent),
        Value::Ref(path) => format!("<<{path}>>"),
        Value::Bottom(d) => write_bottom(d, indent),
        Value::Blur(bd) => write_blur(bd, indent),
        Value::Range { start, end, step } => {
            let mut s = format!("{}..{}", write_value(start, 0), write_value(end, 0));
            if let Some(st) = step {
                s.push_str(&format!("..{}", write_value(st, 0)));
            }
            s
        }
    }
}

fn write_atom(kind: &AtomKind, effect: EffectTag, rank: Option<i64>, indent: usize) -> String {
    if matches!(kind, AtomKind::Bytes(_)) || rank.is_some() || !effect.is_pure() {
        if let AtomKind::Bytes(b) = kind {
            let hex = hex::encode(b);
            let mut fields = vec![(BYTES, quote_string(&hex))];
            if !effect.is_pure() {
                fields.push((EFFECT, write_effect(effect)));
            }
            if let Some(r) = rank {
                fields.push((RANK, r.to_string()));
            }
            return write_wrapper_fields(&fields, indent);
        }
        if rank.is_some() || !effect.is_pure() {
            let mut fields = vec![("__nlang_atom", atom_lit(kind))];
            if !effect.is_pure() {
                fields.push((EFFECT, write_effect(effect)));
            }
            if let Some(r) = rank {
                fields.push((RANK, r.to_string()));
            }
            return write_wrapper_fields(&fields, indent);
        }
    }
    atom_lit(kind)
}

fn atom_lit(kind: &AtomKind) -> String {
    match kind {
        AtomKind::Int(i) => i.to_string(),
        AtomKind::Float(f) => {
            let s = f.to_string();
            if s.contains('.') || s.contains('e') || s.contains('E') {
                s
            } else {
                format!("{s}.0")
            }
        }
        AtomKind::Complex(r, i) => {
            if *i >= 0.0 {
                format!("{r}+{i}i")
            } else {
                format!("{r}{i}i")
            }
        }
        AtomKind::Str(s) => quote_string(s),
        AtomKind::MultilineStr(s) => quote_string(s),
        AtomKind::Tag(t) => format!("#{t}"),
        AtomKind::TagStart => "#_|_".into(),
        AtomKind::TagEnd => "#_".into(),
        AtomKind::Regex(s) => format!("r{}", quote_string(s)),
        AtomKind::PathLit(s) => format!("p{}", quote_string(s)),
        AtomKind::Bytes(b) => {
            let s: String = b.iter().map(|&c| c as char).collect();
            if s.contains('"') {
                format!("{{ ~%{BYTES}: {} }}", quote_string(&hex::encode(b)))
            } else {
                format!("b\"{s}\"")
            }
        }
        AtomKind::Uri(s) => format!("u{}", quote_string(s)),
        AtomKind::Time(s) => format!("t{}", quote_string(s)),
        AtomKind::Top => "_".into(),
        AtomKind::Bottom => "_|_".into(),
        AtomKind::Unit => "()".into(),
    }
}

fn write_combo(c: &ComboVal, indent: usize) -> String {
    let open = if c.closed { "{{" } else { "{" };
    let close = if c.closed { "}}" } else { "}" };
    let mut rows: Vec<(String, String)> = Vec::new();
    let push = |rows: &mut Vec<(String, String)>, prefix: &str, k: &str, v: &Value| {
        rows.push((quote_key(prefix, k), write_value(v, indent)));
    };
    for (k, v) in &c.data {
        push(&mut rows, "", k, v);
    }
    for (k, v) in &c.types {
        push(&mut rows, "@", k, v);
    }
    for (k, v) in &c.rules {
        push(&mut rows, "/", k, v);
    }
    for (k, v) in &c.meta {
        push(&mut rows, "%", k, v);
    }
    for (k, v) in &c.system {
        push(&mut rows, "~%", k, v);
    }
    for (k, v) in &c.local {
        push(&mut rows, "~", k, v);
    }
    // (c-2) runtime slot when it is not already the hashed `%effect` field.
    if !c.effect.is_pure() {
        let materialized = c.meta.get("effect").map(value_as_effect) == Some(c.effect);
        if !materialized {
            rows.push((format!("~%{EFFECT}"), write_effect(c.effect)));
        }
    }
    if rows.is_empty() {
        return if c.closed {
            "{{ }}".into()
        } else {
            "{}".into()
        };
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    let inner = rows
        .into_iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!("{open} {inner} {close}")
}

fn write_thunk(
    expr: &Expr,
    closure: &[Arc<ComboVal>],
    context: Option<&Value>,
    effect: EffectTag,
    indent: usize,
) -> String {
    let frames: Vec<String> = closure.iter().map(|f| write_combo(f, 0)).collect();
    let mut fields = vec![
        (THUNK, "#true".into()),
        (EXPR, expr.to_nlang(0)),
        (CLOSURE, format!("[{}]", frames.join(", "))),
    ];
    if let Some(ctx) = context {
        fields.push((CONTEXT, write_value(ctx, 0)));
    }
    if !effect.is_pure() {
        fields.push((EFFECT, write_effect(effect)));
    }
    write_wrapper_fields(&fields, indent)
}

fn write_bottom(d: &BottomDetail, indent: usize) -> String {
    let mut fields = vec![(BOTTOM, write_tag(d.cause.as_tag()))];
    if let Some(p) = &d.path {
        fields.push(("path", quote_string(p)));
    }
    if let Some(m) = &d.message {
        fields.push(("message", quote_string(m)));
    }
    write_wrapper_fields(&fields, indent)
}

fn write_blur(bd: &BlurDetail, indent: usize) -> String {
    let mut fields = vec![
        (BLUR, "#true".into()),
        ("cause", write_tag(bd.cause.as_str())),
        ("fuel", bd.horizon.fuel.to_string()),
        ("fuel_remaining", bd.horizon.fuel_remaining.to_string()),
        ("strategy", write_tag(strategy_name(bd.horizon.strategy))),
        ("max_branches", bd.horizon.max_branches.to_string()),
        (
            "max_unification_depth",
            bd.horizon.max_unification_depth.to_string(),
        ),
        (
            "max_lifting_depth",
            bd.horizon.max_lifting_depth.to_string(),
        ),
        (
            "max_pattern_nodes",
            bd.horizon.max_pattern_nodes.to_string(),
        ),
    ];
    if let Some(h) = &bd.partial {
        fields.push(("partial", write_hash(h)));
    }
    if !bd.effect.is_pure() {
        fields.push((EFFECT, write_effect(bd.effect)));
    }
    write_wrapper_fields(&fields, indent)
}

fn write_commit(c: &Commit) -> String {
    let mut rows = vec![
        format!("kind: {}", write_tag(commit_kind_name(c.kind))),
        format!("root: {}", write_hash(&c.root)),
        format!("meta: {}", write_commit_meta(&c.meta)),
    ];
    if let Some(p) = &c.parent {
        rows.push(format!("parent: {}", write_hash(p)));
    } else {
        rows.push("parent: _".into());
    }
    if let Some(ri) = &c.refine_info {
        rows.push(format!("refine: {}", write_refine(ri)));
    }
    format!("{{ {} }}", rows.join(" "))
}

fn write_commit_meta(m: &CommitMeta) -> String {
    let mut rows = Vec::new();
    if let Some(a) = &m.author {
        rows.push(format!("author: {}", quote_string(a)));
    }
    rows.push(format!("timestamp: {}", m.timestamp));
    if let Some(msg) = &m.message {
        rows.push(format!("message: {}", quote_string(msg)));
    }
    if let Some(ab) = &m.abandoned {
        let items: Vec<String> = ab.iter().map(|s| quote_string(s)).collect();
        rows.push(format!("abandoned: [{}]", items.join(", ")));
    }
    if let Some(p) = m.privileged_effect {
        rows.push(format!(
            "privileged_effect: {}",
            if p { "#true" } else { "#false" }
        ));
    }
    format!("{{ {} }}", rows.join(" "))
}

fn write_refine(ri: &RefineInfo) -> String {
    let src: Vec<String> = ri.source_caids.iter().map(write_hash).collect();
    let tgt: Vec<String> = ri.target_caids.iter().map(write_hash).collect();
    let mut rows = vec![
        format!("source: [{}]", src.join(", ")),
        format!("target: [{}]", tgt.join(", ")),
    ];
    if !ri.shadow_affected.is_empty() {
        let sh: Vec<String> = ri.shadow_affected.iter().map(write_hash).collect();
        rows.push(format!("shadow: [{}]", sh.join(", ")));
    }
    if let Some(st) = &ri.authority_status {
        rows.push(format!("authority_status: {}", quote_string(st)));
    }
    if let Some(a) = &ri.authority {
        rows.push(format!(
            "authority: {{ signer_pubkey_hex: {} signature_hex: {} }}",
            quote_string(&a.signer_pubkey_hex),
            quote_string(&a.signature_hex),
        ));
    }
    format!("{{ {} }}", rows.join(" "))
}

fn write_hash(h: &ContentHash) -> String {
    let digest = hex::encode(&h.digest);
    let mut fields = vec![
        (HASH, "#true".into()),
        ("digest", quote_string(&digest)),
        (
            "version",
            match h.version {
                CaidVersion::V1 => "#v1".into(),
                CaidVersion::V2 => "#v2".into(),
            },
        ),
    ];
    if matches!(h.version, CaidVersion::V2) {
        let masa = match &h.masa_ref {
            MasaRef::Top => "_".into(),
            MasaRef::Digest(d) => quote_string(&hex::encode(d)),
        };
        fields.push(("masa", masa));
        fields.push(("sketch", quote_string(&h.lattice_sketch)));
    }
    write_wrapper_fields(&fields, 0)
}

fn write_wrapper(kind: &str, fields: &[(&str, String)], indent: usize) -> String {
    let mut all = vec![(kind, "#true".into())];
    all.extend(fields.iter().cloned().map(|(k, v)| (k, v)));
    write_wrapper_fields(&all, indent)
}

fn write_wrapper_fields(fields: &[(&str, String)], _indent: usize) -> String {
    let inner = fields
        .iter()
        .map(|(k, v)| {
            let key = if *k == EXPR || k.starts_with("__nlang_") {
                format!("~%{k}")
            } else {
                (*k).to_string()
            };
            format!("{key}: {v}")
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("{{ {inner} }}")
}

fn write_effect(e: EffectTag) -> String {
    let names = effect_names(e);
    if names.len() == 1 {
        write_tag(names[0])
    } else {
        names
            .into_iter()
            .map(write_tag)
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

fn write_tag(name: &str) -> String {
    format!("#{name}")
}

fn quote_string(s: &str) -> String {
    if s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"\"\"{}\"\"\"", s.replace("\"\"\"", "\\\"\"\""))
    } else {
        format!("\"{s}\"")
    }
}

fn quote_key(prefix: &str, name: &str) -> String {
    if is_plain_ident(name) {
        format!("{prefix}{name}")
    } else if name.contains('"') || name.contains('\n') || name.contains('\r') {
        format!("{prefix}{}", quote_string(name))
    } else {
        format!("{prefix}\"{name}\"")
    }
}

fn is_plain_ident(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    let mut cs = name.chars();
    matches!(cs.next(), Some(c) if c.is_alphabetic() || c == '_')
        && cs.all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

fn commit_kind_name(k: CommitKind) -> &'static str {
    match k {
        CommitKind::Standard => "Standard",
        CommitKind::Refine => "Refine",
        CommitKind::Pin => "Pin",
        CommitKind::Squash => "Squash",
    }
}

fn strategy_name(s: ObservationStrategy) -> &'static str {
    match s {
        ObservationStrategy::Blur => "blur",
        ObservationStrategy::Strict => "strict",
        ObservationStrategy::Approximate => "approximate",
    }
}

fn effect_names(e: EffectTag) -> Vec<&'static str> {
    if e.is_pure() {
        return vec!["pure"];
    }
    let mut n = Vec::new();
    if e.contains(EffectTag::IO) {
        n.push("io");
    }
    if e.contains(EffectTag::NonDet) {
        n.push("nondet");
    }
    if e.contains(EffectTag::State) {
        n.push("state");
    }
    if e.contains(EffectTag::Cached) {
        n.push("cached");
    }
    if n.is_empty() {
        n.push("pure");
    }
    n
}

// ── read ─────────────────────────────────────────────────────────────────

fn expr_to_value(expr: &Expr) -> Result<Value> {
    match &expr.kind {
        ExprKind::Atom(kind) => match kind {
            AtomKind::Top | AtomKind::Unit => Ok(Value::Top),
            AtomKind::Bottom => Ok(Value::Bottom(Box::new(BottomDetail {
                cause: BottomCause::Conflict,
                ..Default::default()
            }))),
            other => Ok(Value::Atom(other.clone(), EffectTag::Pure, None)),
        },
        ExprKind::Combo { fields, closed, .. } => combo_expr_to_value(fields, *closed),
        ExprKind::Join(a, b) => {
            let mut branches = Vec::new();
            flatten_join(a, &mut branches)?;
            flatten_join(b, &mut branches)?;
            Ok(Value::Union(branches))
        }
        ExprKind::List(xs) => {
            let mut fields = IndexMap::new();
            fields.insert(
                "%kind".into(),
                Value::Atom(AtomKind::Tag("list".into()), EffectTag::Pure, None),
            );
            for (i, x) in xs.iter().enumerate() {
                fields.insert(i.to_string(), expr_to_value(x)?);
            }
            Ok(Value::Combo(ComboVal::new(
                fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )))
        }
        ExprKind::Range { start, end, step } => Ok(Value::Range {
            start: Box::new(expr_to_value(start)?),
            end: Box::new(expr_to_value(end)?),
            step: match step {
                Some(s) => Some(Box::new(expr_to_value(s)?)),
                None => None,
            },
        }),
        ExprKind::Structural(inner) => {
            if let ExprKind::Path(p) = &inner.kind {
                Ok(Value::Ref(p.clone()))
            } else {
                Ok(Value::Ref(Path {
                    anchor: nlang_parser::ast::PathAnchor::Bare,
                    segments: vec![inner.to_nlang(0)],
                    span: inner.span,
                }))
            }
        }
        ExprKind::Path(p) => Ok(Value::Ref(p.clone())),
        ExprKind::Context
        | ExprKind::Lens(_, _)
        | ExprKind::Unary { .. }
        | ExprKind::Morphism { .. }
        | ExprKind::Apply(_, _)
        | ExprKind::Pipe(_, _)
        | ExprKind::Meet(_, _)
        | ExprKind::Diff(_, _)
        | ExprKind::Complement(_)
        | ExprKind::Ternary { .. }
        | ExprKind::Add(_, _)
        | ExprKind::Sub(_, _)
        | ExprKind::Mul(_, _)
        | ExprKind::Div(_, _)
        | ExprKind::Rem(_, _)
        | ExprKind::Eq(_, _)
        | ExprKind::Ne(_, _)
        | ExprKind::Lt(_, _)
        | ExprKind::Gt(_, _)
        | ExprKind::Lte(_, _)
        | ExprKind::Gte(_, _)
        | ExprKind::LatticeEq(_, _)
        | ExprKind::Probe(_, _)
        | ExprKind::TypeAnnotation(_, _)
        | ExprKind::Spread(_)
        | ExprKind::Interpolated(_)
        | ExprKind::AnonSet(_)
        | ExprKind::Poset(_)
        | ExprKind::Tuple(_) => Ok(Value::Code(Box::new(expr.clone()))),
        other => anyhow::bail!("store decode: unsupported expr {other:?}"),
    }
}

fn field_axis(key: &FieldKey) -> Result<(&'static str, String)> {
    let raw = match key {
        FieldKey::Named { prefix, name } => {
            return Ok(match prefix {
                Some(Prefix::System) => ("system", name.clone()),
                Some(Prefix::Meta) => ("meta", name.clone()),
                Some(Prefix::Type) => ("types", name.clone()),
                Some(Prefix::Logic) => ("rules", name.clone()),
                Some(Prefix::Private) | Some(Prefix::Local) => ("local", name.clone()),
                _ => ("data", name.clone()),
            });
        }
        FieldKey::Quoted(name) => return Ok(("data", name.clone())),
        FieldKey::Path(p) if p.segments.len() == 1 => p.segments[0].clone(),
        other => anyhow::bail!("store decode: unsupported field key {other:?}"),
    };
    Ok(split_prefixed_name(&raw))
}

fn split_prefixed_name(s: &str) -> (&'static str, String) {
    if let Some(rest) = s.strip_prefix("~%") {
        ("system", rest.to_string())
    } else if let Some(rest) = s.strip_prefix('%') {
        ("meta", rest.to_string())
    } else if let Some(rest) = s.strip_prefix('@') {
        ("types", rest.to_string())
    } else if let Some(rest) = s.strip_prefix('/') {
        ("rules", rest.to_string())
    } else if let Some(rest) = s.strip_prefix('~') {
        ("local", rest.to_string())
    } else {
        ("data", s.to_string())
    }
}

fn flatten_join(expr: &Expr, out: &mut Vec<Value>) -> Result<()> {
    if let ExprKind::Join(a, b) = &expr.kind {
        flatten_join(a, out)?;
        flatten_join(b, out)?;
    } else {
        out.push(expr_to_value(expr)?);
    }
    Ok(())
}

fn combo_expr_to_value(fields: &[Field], closed: bool) -> Result<Value> {
    let mut sys: IndexMap<String, (Expr, Option<Value>)> = IndexMap::new();
    let mut combo = ComboVal::default();
    combo.closed = closed;
    for f in fields {
        let (axis, name) = field_axis(&f.key)?;
        if axis == "system" {
            sys.insert(name, (f.value.clone(), None));
        } else {
            let v = expr_to_value(&f.value)?;
            match axis {
                "meta" => {
                    combo.meta.insert(name, v);
                }
                "types" => {
                    combo.types.insert(name, v);
                }
                "rules" => {
                    combo.rules.insert(name, v);
                }
                "local" => {
                    combo.local.insert(name, v);
                }
                _ => {
                    combo.data.insert(name, v);
                }
            }
        }
    }

    if sys.contains_key(THUNK) {
        return decode_thunk(&sys);
    }
    if sys.contains_key(CODE) {
        let expr = sys
            .get(EXPR)
            .map(|(e, _)| e.clone())
            .ok_or_else(|| anyhow!("code wrapper missing expr"))?;
        return Ok(Value::Code(Box::new(expr)));
    }
    if sys.contains_key(BOTTOM) {
        return decode_bottom(&sys, &combo);
    }
    if sys.contains_key(BLUR) {
        return decode_blur(&sys, &combo);
    }
    if sys.contains_key(BYTES) {
        return decode_bytes_wrap(&sys);
    }
    if sys.contains_key(HASH) {
        return Ok(hash_as_value(&decode_hash_from_combo(&combo)?));
    }
    if sys.contains_key(REF) {
        let v = expr_to_value(
            &sys.get("path")
                .ok_or_else(|| anyhow!("ref missing path"))?
                .0,
        )?;
        return Ok(v);
    }
    if sys.contains_key(TOP_CAUSE) {
        return decode_top_cause(&sys);
    }
    if sys.contains_key("__nlang_atom") {
        return decode_atom_wrap(&sys);
    }

    for (name, (expr, _)) in &sys {
        if name == EFFECT {
            continue;
        }
        combo.system.insert(name.clone(), expr_to_value(expr)?);
    }
    if let Some(e) = combo.meta.get("effect") {
        combo.effect = value_as_effect(e);
    }
    if let Some((expr, _)) = sys.get(EFFECT) {
        combo.effect = value_as_effect(&expr_to_value(expr)?);
    }
    Ok(Value::Combo(combo))
}

fn decode_thunk(sys: &IndexMap<String, (Expr, Option<Value>)>) -> Result<Value> {
    let expr = sys
        .get(EXPR)
        .map(|(e, _)| Box::new(e.clone()))
        .ok_or_else(|| anyhow!("thunk missing expr"))?;
    let closure = match sys.get(CLOSURE) {
        Some((e, _)) => match &e.kind {
            ExprKind::List(xs) => {
                let mut frames = Vec::new();
                for x in xs {
                    match expr_to_value(x)? {
                        Value::Combo(c) => frames.push(Arc::new(c)),
                        other => anyhow::bail!("thunk frame is not a combo: {other:?}"),
                    }
                }
                frames
            }
            _ => anyhow::bail!("thunk closure is not a list"),
        },
        None => Vec::new(),
    };
    let context = match sys.get(CONTEXT) {
        Some((e, _)) => Some(Box::new(expr_to_value(e)?)),
        None => None,
    };
    let effect = match sys.get(EFFECT) {
        Some((e, _)) => value_as_effect(&expr_to_value(e)?),
        None => EffectTag::Pure,
    };
    Ok(Value::Thunk {
        expr,
        closure,
        context,
        effect,
    })
}

fn decode_bottom(sys: &IndexMap<String, (Expr, Option<Value>)>, combo: &ComboVal) -> Result<Value> {
    let cause = match sys.get(BOTTOM) {
        Some((e, _)) => cause_from_value(&expr_to_value(e)?)?,
        None => BottomCause::Conflict,
    };
    let message = combo.data.get("message").and_then(string_of).or_else(|| {
        sys.get("message")
            .and_then(|(e, _)| string_of(&expr_to_value(e).ok()?))
    });
    let path = combo.data.get("path").and_then(string_of).or_else(|| {
        sys.get("path")
            .and_then(|(e, _)| string_of(&expr_to_value(e).ok()?))
    });
    Ok(Value::Bottom(Box::new(BottomDetail {
        cause,
        path,
        message,
        ..Default::default()
    })))
}

fn decode_blur(sys: &IndexMap<String, (Expr, Option<Value>)>, combo: &ComboVal) -> Result<Value> {
    let cause = match combo.data.get("cause") {
        Some(v) => blur_cause_from_value(v)?,
        None => match sys.get("cause") {
            Some((e, _)) => blur_cause_from_value(&expr_to_value(e)?)?,
            None => BlurCause::Timeout,
        },
    };
    let num = |k: &str| -> Result<u64> {
        match combo.data.get(k) {
            Some(Value::Atom(AtomKind::Int(i), _, _)) => Ok(i.to_u64().unwrap_or(0)),
            _ => match sys.get(k) {
                Some((e, _)) => match expr_to_value(e)? {
                    Value::Atom(AtomKind::Int(i), _, _) => Ok(i.to_u64().unwrap_or(0)),
                    _ => Ok(0),
                },
                None => Ok(0),
            },
        }
    };
    use num_traits::ToPrimitive;
    let strategy = match combo.data.get("strategy").or(None) {
        Some(Value::Atom(AtomKind::Tag(t), _, _)) => match t.as_str() {
            "strict" => ObservationStrategy::Strict,
            "approximate" => ObservationStrategy::Approximate,
            _ => ObservationStrategy::Blur,
        },
        _ => match sys.get("strategy") {
            Some((e, _)) => match expr_to_value(e)? {
                Value::Atom(AtomKind::Tag(t), _, _) => match t.as_str() {
                    "strict" => ObservationStrategy::Strict,
                    "approximate" => ObservationStrategy::Approximate,
                    _ => ObservationStrategy::Blur,
                },
                _ => ObservationStrategy::Blur,
            },
            None => ObservationStrategy::Blur,
        },
    };
    let horizon = HorizonParams {
        fuel: num("fuel")?,
        fuel_remaining: num("fuel_remaining")?,
        strategy,
        max_branches: num("max_branches")?,
        max_unification_depth: num("max_unification_depth")?,
        max_lifting_depth: num("max_lifting_depth")?,
        max_pattern_nodes: num("max_pattern_nodes")?,
    };
    let partial = match combo.data.get("partial") {
        Some(v) => Some(value_as_hash(v)?),
        None => match sys.get("partial") {
            Some((e, _)) => Some(value_as_hash(&expr_to_value(e)?)?),
            None => None,
        },
    };
    let effect = match sys.get(EFFECT) {
        Some((e, _)) => value_as_effect(&expr_to_value(e)?),
        None => EffectTag::Pure,
    };
    Ok(Value::Blur(BlurDetail {
        cause,
        horizon,
        partial,
        partial_body: None,
        effect,
        co_horizons: Vec::new(),
    }))
}

fn decode_bytes_wrap(sys: &IndexMap<String, (Expr, Option<Value>)>) -> Result<Value> {
    let hex = sys
        .get(BYTES)
        .and_then(|(e, _)| string_of(&expr_to_value(e).ok()?))
        .ok_or_else(|| anyhow!("bytes wrapper missing hex"))?;
    let b = hex::decode(&hex).map_err(|e| anyhow!("bytes hex: {e}"))?;
    let effect = match sys.get(EFFECT) {
        Some((e, _)) => value_as_effect(&expr_to_value(e)?),
        None => EffectTag::Pure,
    };
    let rank = match sys.get(RANK) {
        Some((e, _)) => match expr_to_value(e)? {
            Value::Atom(AtomKind::Int(i), _, _) => i.to_i64(),
            _ => None,
        },
        None => None,
    };
    use num_traits::ToPrimitive;
    Ok(Value::Atom(AtomKind::Bytes(b), effect, rank))
}

fn decode_atom_wrap(sys: &IndexMap<String, (Expr, Option<Value>)>) -> Result<Value> {
    let inner = expr_to_value(
        &sys.get("__nlang_atom")
            .ok_or_else(|| anyhow!("atom wrapper missing payload"))?
            .0,
    )?;
    let effect = match sys.get(EFFECT) {
        Some((e, _)) => value_as_effect(&expr_to_value(e)?),
        None => EffectTag::Pure,
    };
    let rank = match sys.get(RANK) {
        Some((e, _)) => match expr_to_value(e)? {
            Value::Atom(AtomKind::Int(i), _, _) => i.to_i64(),
            _ => None,
        },
        None => None,
    };
    use num_traits::ToPrimitive;
    match inner {
        Value::Atom(k, _, _) => Ok(Value::Atom(k, effect, rank)),
        Value::Top => Ok(Value::Atom(AtomKind::Top, effect, rank)),
        other => Ok(other),
    }
}

fn decode_top_cause(sys: &IndexMap<String, (Expr, Option<Value>)>) -> Result<Value> {
    let cause = sys
        .get("cause")
        .and_then(|(e, _)| match expr_to_value(e).ok()? {
            Value::Atom(AtomKind::Tag(t), _, _) => Some(t),
            _ => None,
        })
        .unwrap_or_else(|| "no_coordinate".into());
    let members = match sys.get("members") {
        Some((e, _)) => match &e.kind {
            ExprKind::List(xs) => {
                let mut m = Vec::new();
                for x in xs {
                    if let Some(s) = string_of(&expr_to_value(x)?) {
                        m.push(s);
                    }
                }
                m
            }
            _ => Vec::new(),
        },
        None => Vec::new(),
    };
    Ok(Value::TopCaused { cause, members })
}

fn decode_hash_from_combo(c: &ComboVal) -> Result<ContentHash> {
    let digest = c
        .data
        .get("digest")
        .and_then(string_of)
        .ok_or_else(|| anyhow!("hash missing digest"))?;
    let digest = hex::decode(&digest).map_err(|e| anyhow!("hash digest: {e}"))?;
    let version = match c.data.get("version") {
        Some(Value::Atom(AtomKind::Tag(t), _, _)) if t == "v2" => CaidVersion::V2,
        _ => CaidVersion::V1,
    };
    let mut h = ContentHash {
        algorithm: HashAlgorithm::Sha256,
        version: version.clone(),
        masa_ref: MasaRef::Top,
        lattice_sketch: String::new(),
        digest,
    };
    if matches!(version, CaidVersion::V2) {
        match c.data.get("masa") {
            Some(Value::Top) | Some(Value::TopCaused { .. }) | None => {}
            Some(Value::Atom(AtomKind::Str(s), _, _)) => {
                if let Ok(d) = hex::decode(s) {
                    h.masa_ref = MasaRef::Digest(d);
                }
            }
            _ => {}
        }
        if let Some(s) = c.data.get("sketch").and_then(string_of) {
            h.lattice_sketch = s;
        }
    }
    Ok(h)
}

fn decode_commit_body(body: &str) -> Result<Commit> {
    let expr = parse_body(body)?;
    let Value::Combo(c) = expr_to_value(&expr)? else {
        anyhow::bail!("commit body is not a combo");
    };
    let kind = match c.data.get("kind") {
        Some(Value::Atom(AtomKind::Tag(t), _, _)) => match t.as_str() {
            "Refine" => CommitKind::Refine,
            "Pin" => CommitKind::Pin,
            "Squash" => CommitKind::Squash,
            _ => CommitKind::Standard,
        },
        _ => CommitKind::Standard,
    };
    let root = match c.data.get("root") {
        Some(v) => value_as_hash(v)?,
        None => anyhow::bail!("commit missing root"),
    };
    let parent = match c.data.get("parent") {
        None | Some(Value::Top) | Some(Value::TopCaused { .. }) => None,
        Some(v) => Some(value_as_hash(v)?),
    };
    let meta = match c.data.get("meta") {
        Some(Value::Combo(m)) => decode_commit_meta(m),
        _ => CommitMeta::default(),
    };
    let refine_info = match c.data.get("refine") {
        Some(Value::Combo(r)) => Some(decode_refine(r)?),
        _ => None,
    };
    Ok(Commit {
        parent,
        root,
        meta,
        kind,
        refine_info,
        cache_id: crate::value::default_cache_id(),
    })
}

fn decode_commit_meta(m: &ComboVal) -> CommitMeta {
    let author = m.data.get("author").and_then(string_of);
    let message = m.data.get("message").and_then(string_of);
    let timestamp = match m.data.get("timestamp") {
        Some(Value::Atom(AtomKind::Int(i), _, _)) => {
            use num_traits::ToPrimitive;
            i.to_u64().unwrap_or(0)
        }
        _ => 0,
    };
    let abandoned = match m.data.get("abandoned") {
        Some(Value::Combo(list)) => {
            let mut v = Vec::new();
            let mut i = 0;
            while let Some(item) = list.data.get(&i.to_string()) {
                if let Some(s) = string_of(item) {
                    v.push(s);
                }
                i += 1;
            }
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        }
        Some(Value::Atom(AtomKind::Str(s), _, _)) => Some(vec![s.clone()]),
        _ => None,
    };
    let privileged_effect = match m.data.get("privileged_effect") {
        Some(Value::Atom(AtomKind::Tag(t), _, _)) => Some(t == "true"),
        _ => None,
    };
    CommitMeta {
        author,
        timestamp,
        message,
        abandoned,
        privileged_effect,
    }
}

fn decode_refine(r: &ComboVal) -> Result<RefineInfo> {
    let authority = match r.data.get("authority") {
        Some(Value::Combo(a)) => {
            let signer = a.data.get("signer_pubkey_hex").and_then(string_of);
            let sig = a.data.get("signature_hex").and_then(string_of);
            match (signer, sig) {
                (Some(signer_pubkey_hex), Some(signature_hex)) => {
                    Some(crate::value::AuthorityInfo {
                        signer_pubkey_hex,
                        signature_hex,
                        timestamp: None,
                    })
                }
                _ => None,
            }
        }
        _ => None,
    };
    Ok(RefineInfo {
        source_caids: list_hashes(r.data.get("source"))?,
        target_caids: list_hashes(r.data.get("target"))?,
        authority,
        shadow_affected: list_hashes(r.data.get("shadow")).unwrap_or_default(),
        authority_status: r.data.get("authority_status").and_then(string_of),
    })
}

fn list_hashes(v: Option<&Value>) -> Result<Vec<ContentHash>> {
    let Some(v) = v else {
        return Ok(Vec::new());
    };
    match v {
        Value::Combo(c) => {
            let mut out = Vec::new();
            let mut i = 0;
            while let Some(item) = c.data.get(&i.to_string()) {
                out.push(value_as_hash(item)?);
                i += 1;
            }
            Ok(out)
        }
        other => Ok(vec![value_as_hash(other)?]),
    }
}

fn value_as_hash(v: &Value) -> Result<ContentHash> {
    match v {
        Value::Combo(c) if c.system.contains_key(HASH) || c.data.contains_key("digest") => {
            let digest = c
                .data
                .get("digest")
                .or_else(|| c.system.get("digest"))
                .and_then(string_of)
                .ok_or_else(|| anyhow!("hash combo missing digest"))?;
            let digest = hex::decode(&digest).map_err(|e| anyhow!("hash digest: {e}"))?;
            let version = match c.data.get("version").or_else(|| c.meta.get("version")) {
                Some(Value::Atom(AtomKind::Tag(t), _, _)) if t == "v2" => CaidVersion::V2,
                _ => CaidVersion::V1,
            };
            let mut h = ContentHash {
                algorithm: HashAlgorithm::Sha256,
                version: version.clone(),
                masa_ref: MasaRef::Top,
                lattice_sketch: String::new(),
                digest,
            };
            if matches!(version, CaidVersion::V2) {
                match c.data.get("masa") {
                    Some(Value::Atom(AtomKind::Str(s), _, _)) => {
                        if let Ok(d) = hex::decode(s) {
                            h.masa_ref = MasaRef::Digest(d);
                        }
                    }
                    _ => {}
                }
                if let Some(s) = c.data.get("sketch").and_then(string_of) {
                    h.lattice_sketch = s;
                }
            }
            Ok(h)
        }
        Value::Atom(AtomKind::Str(s), _, _) => ContentHash::parse(s).or_else(|_| {
            let d = hex::decode(s).map_err(|e| anyhow!("hash string: {e}"))?;
            Ok(ContentHash::v1(d))
        }),
        _ => anyhow::bail!("not a content hash: {v:?}"),
    }
}

fn hash_as_value(h: &ContentHash) -> Value {
    let mut c = ComboVal::default();
    c.system.insert(
        HASH.into(),
        Value::Atom(AtomKind::Tag("true".into()), EffectTag::Pure, None),
    );
    c.data.insert(
        "digest".into(),
        Value::Atom(AtomKind::Str(hex::encode(&h.digest)), EffectTag::Pure, None),
    );
    c.data.insert(
        "version".into(),
        Value::Atom(
            AtomKind::Tag(match h.version {
                CaidVersion::V1 => "v1".into(),
                CaidVersion::V2 => "v2".into(),
            }),
            EffectTag::Pure,
            None,
        ),
    );
    if matches!(h.version, CaidVersion::V2) {
        c.data.insert(
            "masa".into(),
            match &h.masa_ref {
                MasaRef::Top => Value::Top,
                MasaRef::Digest(d) => {
                    Value::Atom(AtomKind::Str(hex::encode(d)), EffectTag::Pure, None)
                }
            },
        );
        c.data.insert(
            "sketch".into(),
            Value::Atom(
                AtomKind::Str(h.lattice_sketch.clone()),
                EffectTag::Pure,
                None,
            ),
        );
    }
    Value::Combo(c)
}

fn value_as_effect(v: &Value) -> EffectTag {
    match v {
        Value::Atom(AtomKind::Tag(t), _, _) => effect_from_name(t),
        Value::Union(xs) => xs
            .iter()
            .fold(EffectTag::Pure, |a, b| a.union(value_as_effect(b))),
        _ => EffectTag::Pure,
    }
}

fn effect_from_name(t: &str) -> EffectTag {
    match t.trim_start_matches('#') {
        "io" => EffectTag::IO,
        "nondet" => EffectTag::NonDet,
        "state" => EffectTag::State,
        "cached" => EffectTag::Cached,
        _ => EffectTag::Pure,
    }
}

fn string_of(v: &Value) -> Option<String> {
    match v {
        Value::Atom(AtomKind::Str(s), _, _) | Value::Atom(AtomKind::MultilineStr(s), _, _) => {
            Some(s.clone())
        }
        _ => None,
    }
}

fn cause_from_value(v: &Value) -> Result<BottomCause> {
    let tag = match v {
        Value::Atom(AtomKind::Tag(t), _, _) => t.trim_start_matches('#').to_string(),
        _ => return Ok(BottomCause::Conflict),
    };
    Ok(match tag.as_str() {
        "conflict" => BottomCause::Conflict,
        "missing_key" => BottomCause::MissingKey,
        "fuel_exhausted" => BottomCause::FuelExhausted,
        "timeout" => BottomCause::Timeout,
        "peer_timeout" => BottomCause::PeerTimeout,
        "divergent" => BottomCause::Divergent,
        "invalid_path" => BottomCause::InvalidPath,
        "private_access_violation" => BottomCause::PrivateAccessViolation,
        "numerical_error" => BottomCause::NumericalError,
        "arithmetic_on_anchor" => BottomCause::ArithmeticOnAnchor,
        "h1_split" => BottomCause::H1Split,
        "h2_split" => BottomCause::H2Split,
        "semantic_eclipse" => BottomCause::SemanticEclipse,
        "no_context" => BottomCause::NoContext,
        "out_of_horizon" => BottomCause::OutOfHorizon,
        "system_reserved" => BottomCause::SystemReserved,
        "invalid_config" => BottomCause::InvalidConfig,
        "effect_violation" => BottomCause::EffectViolation,
        "privileged_required" => BottomCause::PrivilegedRequired,
        "store_boundary" => BottomCause::StoreBoundary,
        "caid_mismatch" => BottomCause::CaidMismatch,
        "peer_not_implemented" => BottomCause::PeerNotImplemented,
        "peer_unknown_status" => BottomCause::PeerUnknownStatus,
        "peer_refused" => BottomCause::PeerRefused,
        "routing_budget_exceeded" => BottomCause::RoutingBudgetExceeded,
        "max_depth_exceeded" => BottomCause::MaxDepthExceeded,
        "stack_overflow" => BottomCause::StackOverflow,
        "object_undecodable" => BottomCause::ObjectUndecodable,
        "standard_root_unavailable" => BottomCause::StandardRootUnavailable,
        "no_standard_root" => BottomCause::NoStandardRoot,
        "unprojected_builtin" => BottomCause::UnprojectedBuiltin,
        "unprovided_builtin" => BottomCause::UnprovidedBuiltin,
        _ => BottomCause::Conflict,
    })
}

fn blur_cause_from_value(v: &Value) -> Result<BlurCause> {
    let tag = match v {
        Value::Atom(AtomKind::Tag(t), _, _) => t.trim_start_matches('#').to_string(),
        Value::Atom(AtomKind::Str(s), _, _) => s.clone(),
        _ => return Ok(BlurCause::Timeout),
    };
    Ok(match tag.as_str() {
        "fuel_exhausted" => BlurCause::FuelExhausted,
        "stack_overflow" => BlurCause::StackOverflow,
        "max_depth_exceeded" => BlurCause::MaxDepthExceeded,
        "timeout" => BlurCause::Timeout,
        other => BlurCause::MathSingularity(other.to_string()),
    })
}

// ── GC refs ──────────────────────────────────────────────────────────────

pub fn refs_of_value(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Combo(c) => refs_of_combo(c, out),
        Value::Union(xs) => {
            for x in xs {
                refs_of_value(x, out);
            }
        }
        Value::Atom(AtomKind::Str(s), _, _) => push_digest_string(s, out),
        Value::Thunk {
            closure, context, ..
        } => {
            for f in closure {
                refs_of_combo(f, out);
            }
            if let Some(c) = context {
                refs_of_value(c, out);
            }
        }
        Value::Blur(bd) => {
            if let Some(h) = &bd.partial {
                out.push(hex::encode(&h.digest));
            }
            if let Some(body) = &bd.partial_body {
                refs_of_value(body, out);
            }
            for rec in &bd.co_horizons {
                if let Some(h) = &rec.partial {
                    out.push(hex::encode(&h.digest));
                }
            }
        }
        Value::Range { start, end, step } => {
            refs_of_value(start, out);
            refs_of_value(end, out);
            if let Some(s) = step {
                refs_of_value(s, out);
            }
        }
        _ => {}
    }
}

fn refs_of_combo(c: &ComboVal, out: &mut Vec<String>) {
    for map in [&c.data, &c.types, &c.rules, &c.meta, &c.system, &c.local] {
        for v in map.values() {
            refs_of_value(v, out);
        }
    }
}

fn refs_of_commit(c: &Commit, follow_abandoned: bool, out: &mut Vec<String>) {
    out.push(hex::encode(&c.root.digest));
    if let Some(p) = &c.parent {
        out.push(hex::encode(&p.digest));
    }
    if let Some(ri) = &c.refine_info {
        for h in ri.source_caids.iter().chain(ri.target_caids.iter()) {
            out.push(hex::encode(&h.digest));
        }
    }
    if follow_abandoned {
        if let Some(ab) = &c.meta.abandoned {
            for s in ab {
                push_digest_string(s, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thunk_wrapper_roundtrips_as_thunk() {
        let src = r#"{
  ~%__nlang_thunk: #true
  ~%__nlang_expr: 1
  ~%__nlang_closure: []
}"#;
        let v = expr_to_value(&parse_body(src).expect("parse")).expect("decode");
        assert!(matches!(v, Value::Thunk { .. }), "got {v:?}");
    }
}

fn push_digest_string(s: &str, out: &mut Vec<String>) {
    if s.len() == 64
        && s.bytes()
            .all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F'))
    {
        out.push(s.to_lowercase());
        return;
    }
    if let Some(d) = s.strip_prefix("hash:sha256:") {
        if let Some(hex) = d.rsplit(':').next() {
            if hex.len() == 64 {
                out.push(hex.to_lowercase());
            }
        }
    }
}
