use crate::value::*;
use nlang_parser::ast::AtomKind;
use num_traits::ToPrimitive;
use sha2::{Sha256, Digest};
use std::collections::HashSet;

// ── Type tags (REAL_03 §6.2) ──────────────────────────────────
// Unused in Phase 1a; reserved for future use (List, Tuple, Bool, Ref)
#[allow(dead_code)]
const TAG_COMBO:  u8 = 0x01;
#[allow(dead_code)]
const TAG_COCOON: u8 = 0x02;
#[allow(dead_code)]
const TAG_LIST:   u8 = 0x03;
#[allow(dead_code)]
const TAG_TUPLE:  u8 = 0x04;
#[allow(dead_code)]
const TAG_ATOM:   u8 = 0x10;
#[allow(dead_code)]
const TAG_TAG:    u8 = 0x11;
#[allow(dead_code)]
const TAG_INT64:  u8 = 0x12;
#[allow(dead_code)]
const TAG_FLOAT:  u8 = 0x13;
#[allow(dead_code)]
const TAG_COMPLEX: u8 = 0x14;
#[allow(dead_code)]
const TAG_BOOL:   u8 = 0x15;
#[allow(dead_code)]
const TAG_REF:    u8 = 0x16;

// ── Public API ────────────────────────────────────────────────

pub fn serialize_bn(value: &Value) -> Vec<u8> {
    let mut buf = Vec::new();
    serialize_value(value, &mut buf);
    buf
}

pub fn content_digest(value: &Value) -> [u8; 32] {
    let bytes = serialize_bn(value);
    Sha256::digest(&bytes).into()
}

// ── Internal serialization ────────────────────────────────────

fn serialize_value(val: &Value, buf: &mut Vec<u8>) {
    match val {
        Value::Top => buf.push(0xFF),
        Value::Bottom(_) => buf.push(0xFE),
        Value::Atom(kind, _effect, _rank) => serialize_atom(kind, buf),
        Value::Combo(cv) => serialize_combo(cv, buf),
        Value::Union(items) => serialize_union(items, buf),
        Value::Code(expr) => { buf.push(TAG_ATOM); encode_string(&format!("{:?}", expr), buf); }
        Value::Thunk { expr, .. } => { buf.push(TAG_ATOM); encode_string(&format!("{:?}", expr), buf); }
    }
}

fn serialize_atom(kind: &AtomKind, buf: &mut Vec<u8>) {
    match kind {
        AtomKind::Str(s) | AtomKind::MultilineStr(s) => {
            buf.push(TAG_ATOM);
            encode_string(s.trim(), buf);
        }
        AtomKind::Int(n) => {
            buf.push(TAG_INT64);
            if let Some(i) = n.to_i64() {
                encode_signed_leb128(i, buf);
            } else {
                encode_signed_leb128(0, buf); // overflow fallback
            }
        }
        AtomKind::Float(f) => {
            buf.push(TAG_FLOAT);
            encode_fixed128(*f, buf);
        }
        AtomKind::Complex(r, i) => {
            buf.push(TAG_COMPLEX);
            encode_fixed128(*r, buf);
            encode_fixed128(*i, buf);
        }
        AtomKind::Tag(t) => {
            buf.push(TAG_TAG);
            encode_string(t.trim_start_matches('#'), buf);
        }
        AtomKind::TagStart => { buf.push(TAG_TAG); encode_string("_", buf); }
        AtomKind::TagEnd => { buf.push(TAG_TAG); encode_string("_|_", buf); }
        AtomKind::PathLit(p) => { buf.push(TAG_ATOM); encode_string(p, buf); }
        AtomKind::Top => buf.push(0xFF),
        AtomKind::Bottom => buf.push(0xFE),
        AtomKind::Unit => { buf.push(TAG_ATOM); encode_string("()", buf); }
        AtomKind::Regex(r) => { buf.push(TAG_ATOM); encode_string(r, buf); }
        AtomKind::Uri(u) => { buf.push(TAG_ATOM); encode_string(u, buf); }
        AtomKind::Time(t) => { buf.push(TAG_ATOM); encode_string(t, buf); }
        AtomKind::Bytes(b) => { buf.push(TAG_ATOM); encode_string(&hex::encode(b), buf); }
    }
}

fn serialize_combo(cv: &ComboVal, buf: &mut Vec<u8>) {
    buf.push(if cv.closed { TAG_COCOON } else { TAG_COMBO });

    let mut entries: Vec<(u8, &str, &Value)> = Vec::new();
    for (k, v) in &cv.system  { entries.push((1, k.as_str(), v)); }
    for (k, v) in &cv.meta    { entries.push((2, k.as_str(), v)); }
    for (k, v) in &cv.types   { entries.push((3, k.as_str(), v)); }
    for (k, v) in &cv.rules   { entries.push((4, k.as_str(), v)); }
    for (k, v) in &cv.data    { entries.push((5, k.as_str(), v)); }
    for (k, v) in &cv.local   { entries.push((6, k.as_str(), v)); }
    for (k, v) in &cv.legacy_fields { entries.push((5, k.as_str(), v)); }
    for (k, v) in &cv.legacy_local  { entries.push((6, k.as_str(), v)); }

    entries.sort_by(|a, b| {
        let cmp = a.0.cmp(&b.0);
        if cmp != std::cmp::Ordering::Equal { return cmp; }
        a.1.cmp(b.1)
    });

    encode_unsigned_leb128(entries.len() as u64, buf);
    for (_prio, key, val) in &entries {
        encode_string(key, buf);
        serialize_value(val, buf);
    }
}

fn serialize_union(items: &[Value], buf: &mut Vec<u8>) {
    let mut sorted: Vec<Vec<u8>> = items.iter().map(|v| serialize_bn(v)).collect();
    sorted.sort();
    let mut seen = HashSet::new();
    for bytes in &sorted {
        if seen.insert(bytes.clone()) {
            buf.extend_from_slice(bytes);
        }
    }
}

// ── Encoding helpers ──────────────────────────────────────────

pub fn encode_string(s: &str, buf: &mut Vec<u8>) {
    let utf8 = s.as_bytes();
    encode_unsigned_leb128(utf8.len() as u64, buf);
    buf.extend_from_slice(utf8);
}

pub fn encode_signed_leb128(mut val: i64, buf: &mut Vec<u8>) {
    loop {
        let byte = (val as u8) & 0x7f;
        val >>= 7;
        let more = !((val == 0 && (byte & 0x40) == 0) || (val == -1 && (byte & 0x40) != 0));
        buf.push(if more { byte | 0x80 } else { byte });
        if !more { break; }
    }
}

pub fn encode_unsigned_leb128(mut val: u64, buf: &mut Vec<u8>) {
    loop {
        let byte = (val as u8) & 0x7f;
        val >>= 7;
        buf.push(if val != 0 { byte | 0x80 } else { byte });
        if val == 0 { break; }
    }
}

fn encode_fixed128(val: f64, buf: &mut Vec<u8>) {
    let whole = val.trunc() as i64;
    let frac = (val.fract().abs() * 18446744073709551616.0) as u64; // 2^64 as float
    encode_signed_leb128(whole, buf);
    buf.extend_from_slice(&frac.to_le_bytes());
}
