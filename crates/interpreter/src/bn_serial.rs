use crate::value::*;
use nlang_parser::ast::{
    AddressAlgo, AtomKind, Expr, ExprKind, Field, FieldKey, Path, PathAnchor, Prefix, RelOp,
    Relation, StringPart, UnaryOp,
};
use num_traits::ToPrimitive;
use sha2::{Digest, Sha256};
use std::collections::HashSet;

// ── Type tags (REAL_03 §6.2) ──────────────────────────────────
// Unused in Phase 1a; reserved for future use (List, Tuple, Bool, Ref)
#[allow(dead_code)]
const TAG_COMBO: u8 = 0x01;
#[allow(dead_code)]
const TAG_COCOON: u8 = 0x02;
#[allow(dead_code)]
const TAG_LIST: u8 = 0x03;
#[allow(dead_code)]
const TAG_TUPLE: u8 = 0x04;
#[allow(dead_code)]
const TAG_ATOM: u8 = 0x10;
#[allow(dead_code)]
const TAG_TAG: u8 = 0x11;
#[allow(dead_code)]
const TAG_INT64: u8 = 0x12;
#[allow(dead_code)]
const TAG_FLOAT: u8 = 0x13;
#[allow(dead_code)]
const TAG_COMPLEX: u8 = 0x14;
#[allow(dead_code)]
const TAG_BOOL: u8 = 0x15;
#[allow(dead_code)]
const TAG_REF: u8 = 0x16;
// Stage 2: Thunk serializes expr (Q-052: specified Expr node table, not
// to_nlang) + closure frames (each ComboVal) + context (#open or value)
// + effect — the full GUIDE_03 §11.3 memo-key triple. Without context,
// two thunks with the same expr but different bindings collide.
const TAG_THUNK: u8 = 0x17;
// Range [start, end] (optional step) — new CAID tag (2026-07-10); no prior
// values existed (all ranges were ⊥ via wildcard), so no CAID invalidation.
const TAG_RANGE: u8 = 0x18;
// Q-049: each AtomKind that used to share TAG_ATOM (0x10) gets its own
// byte so the kind enters the address (REAL_03 §6.7 clause 2). 0x10
// stays Str. 0x15 remains the unused Bool reservation.
const TAG_UNIT: u8 = 0x19;
const TAG_REGEX: u8 = 0x1A;
const TAG_URI: u8 = 0x1B;
const TAG_TIME: u8 = 0x1C;
const TAG_PATH: u8 = 0x1D;
const TAG_BYTES: u8 = 0x1E;
// D70: 0x1F was the Q-049 MultilineStr tag. Triple-quote is a spelling,
// not a kind, so this byte is dead and must not be reused.
#[allow(dead_code)]
const TAG_MULTILINE: u8 = 0x1F;
// Q-050: Code is a value, not a string. 0x10 stays Str. The payload is a
// tagged Expr tree (EX_* below), not a Rust Debug rendering.
const TAG_CODE: u8 = 0x20;
// Syntax-only atom inside a Code tree: a triple-quoted string node.
// D71: spelling is content of Code. Must not reuse dead 0x1F, and must
// not collapse onto TAG_ATOM (that is the value-layer D70 intern).
const TAG_EXPR_MULTILINE: u8 = 0x21;
// ExprKind tags. Only appear inside TAG_CODE's payload (a disjoint
// grammar from values). One byte per ExprKind variant.
const EX_ATOM: u8 = 0x40;
const EX_PATH: u8 = 0x41;
const EX_APPLY: u8 = 0x42;
const EX_PIPE: u8 = 0x43;
const EX_MORPHISM: u8 = 0x44;
const EX_COMBO: u8 = 0x45;
const EX_MEET: u8 = 0x46;
const EX_JOIN: u8 = 0x47;
const EX_DIFF: u8 = 0x48;
const EX_COMPLEMENT: u8 = 0x49;
const EX_TERNARY: u8 = 0x4A;
const EX_ADD: u8 = 0x4B;
const EX_SUB: u8 = 0x4C;
const EX_MUL: u8 = 0x4D;
const EX_DIV: u8 = 0x4E;
const EX_REM: u8 = 0x4F;
const EX_EQ: u8 = 0x50;
const EX_NE: u8 = 0x51;
const EX_LT: u8 = 0x52;
const EX_GT: u8 = 0x53;
const EX_LTE: u8 = 0x54;
const EX_GTE: u8 = 0x55;
const EX_LATTICE_EQ: u8 = 0x56;
const EX_PROBE: u8 = 0x57;
const EX_TYPE_ANNOTATION: u8 = 0x58;
const EX_UNARY: u8 = 0x59;
const EX_LIST: u8 = 0x5A;
const EX_TUPLE: u8 = 0x5B;
const EX_POSET: u8 = 0x5C;
const EX_LENS: u8 = 0x5D;
const EX_ANON_SET: u8 = 0x5E;
const EX_INTERPOLATED: u8 = 0x5F;
const EX_RANGE: u8 = 0x60;
const EX_CONTEXT: u8 = 0x61;
const EX_SPREAD: u8 = 0x62;
const EX_STRUCTURAL: u8 = 0x63;
const TAG_BLUR: u8 = 0xFD;
const TAG_BOTTOM: u8 = 0xFE;
const TAG_TOP: u8 = 0xFF;

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

/// Content digest of a combo without wrapping as `Value::Combo` (avoids
/// clone that would reset `cache_id`).
pub fn content_digest_combo(cv: &ComboVal) -> [u8; 32] {
    let mut buf = Vec::new();
    serialize_combo(cv, &mut buf);
    Sha256::digest(&buf).into()
}

// ── Internal serialization ────────────────────────────────────

fn serialize_value(val: &Value, buf: &mut Vec<u8>) {
    match val {
        // Caused Top serializes as bare Top (provenance is observation-only).
        Value::Top | Value::TopCaused { .. } => buf.push(TAG_TOP),
        Value::Bottom(_) => buf.push(TAG_BOTTOM),
        Value::Atom(kind, _effect, _rank) => serialize_atom(kind, buf),
        Value::Combo(cv) => serialize_combo(cv, buf),
        Value::Union(items) => serialize_union(items, buf),
        Value::Code(expr) => {
            // Q-050: identity is a specified Expr encoding, not Debug.
            // Spans are omitted (O42 M4) without cloning a spanless tree.
            // Walk is iterative: to_nlang / Debug recurse and overflow
            // the 64 MiB CLI stack on ~10³ left-associated terms (L2-65).
            buf.push(TAG_CODE);
            encode_expr(expr, buf);
        }
        Value::Ref(path) => {
            buf.push(TAG_REF);
            encode_path(path, buf);
        }
        // Stage 2: full Thunk serialization — expr (Q-052: specified Expr
        // node table, not to_nlang) + closure (frame) + context
        // (binding | #open) + effect. The context slot is load-bearing:
        // without it, `Thunk{$, ctx=lv1}` and `Thunk{$, ctx=j1}` hash
        // identically and lazy unify's CAID early-out collapses the
        // self-referential deepening (019 prop 3). GUIDE_03 §11.3 memo key.
        Value::Thunk {
            expr,
            closure,
            context,
            effect,
        } => {
            buf.push(TAG_THUNK);
            encode_expr(expr, buf);
            // Identity encoding (store CAID): still inlines frames via
            // serialize_combo so digests stay bit-stable (D1 success §5).
            // Force/in_flight uses a separate cycle key that hashes frame
            // digests only — see `thunk_cycle_id` — because inlining Arc
            // frames as a tree is 2^depth work.
            encode_unsigned_leb128(closure.len() as u64, buf);
            for cv in closure.iter() {
                serialize_combo(cv.as_ref(), buf);
            }
            // context: None = #open; Some(v) = recursive serialize_value
            match context {
                None => {
                    buf.push(0x00);
                }
                Some(v) => {
                    buf.push(0x01);
                    serialize_value(v, buf);
                }
            }
            buf.push(effect.to_serial_byte());
        }
        Value::Blur(bd) => {
            // O42 R-5: identity encoding is the CHS digest (same as blur_caid).
            // Full record set is recoverable from display/runtime fields only
            // for in-memory values; content-addressed store keys use CHS.
            buf.push(TAG_BLUR);
            let chs = bd.blur_caid();
            buf.extend_from_slice(&chs.digest);
            // Preserve runtime payload for reconstruct (not identity):
            // primary record fields + co_horizons length + each co digest.
            let cause_bytes = bd.cause.as_bytes();
            encode_unsigned_leb128(cause_bytes.len() as u64, buf);
            buf.extend_from_slice(cause_bytes);
            buf.extend_from_slice(&bd.horizon.fuel.to_le_bytes());
            buf.extend_from_slice(&bd.horizon.fuel_remaining.to_le_bytes());
            buf.push(bd.horizon.strategy_byte());
            buf.extend_from_slice(&bd.horizon.max_branches.to_le_bytes());
            buf.extend_from_slice(&bd.horizon.max_unification_depth.to_le_bytes());
            buf.extend_from_slice(&bd.horizon.max_lifting_depth.to_le_bytes());
            buf.extend_from_slice(&bd.horizon.max_pattern_nodes.to_le_bytes());
            // O42 repair: partial is a CAID, not an inlined tree.
            if let Some(partial) = &bd.partial {
                buf.push(0x01);
                encode_unsigned_leb128(partial.digest.len() as u64, buf);
                buf.extend_from_slice(&partial.digest);
            } else {
                buf.push(0x00);
            }
            encode_unsigned_leb128(bd.co_horizons.len() as u64, buf);
            for rec in &bd.co_horizons {
                let d = rec.chs_digest();
                encode_unsigned_leb128(d.len() as u64, buf);
                buf.extend_from_slice(&d);
            }
        }
        Value::Range { start, end, step } => {
            buf.push(TAG_RANGE);
            serialize_value(start, buf);
            serialize_value(end, buf);
            match step {
                None => buf.push(0x00),
                Some(s) => {
                    buf.push(0x01);
                    serialize_value(s, buf);
                }
            }
        }
    }
}

fn serialize_atom(kind: &AtomKind, buf: &mut Vec<u8>) {
    match kind {
        AtomKind::Str(s) | AtomKind::MultilineStr(s) => {
            buf.push(TAG_ATOM);
            encode_string(s, buf);
        }
        AtomKind::Int(n) => {
            buf.push(TAG_INT64);
            // REAL_03 §6.1: signed LEB128 is variable-length. Values that
            // fit in i64 keep the existing encoding (no epoch for small
            // integers). Wider values continue with more bytes — they
            // must not collapse to 0.
            encode_signed_leb128_int(n, buf);
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
        AtomKind::TagStart => {
            buf.push(TAG_TAG);
            encode_string("_", buf);
        }
        AtomKind::TagEnd => {
            buf.push(TAG_TAG);
            encode_string("_|_", buf);
        }
        AtomKind::PathLit(p) => {
            buf.push(TAG_PATH);
            encode_string(p, buf);
        }
        AtomKind::Top => buf.push(TAG_TOP),
        AtomKind::Bottom => buf.push(TAG_BOTTOM),
        AtomKind::Unit => {
            buf.push(TAG_UNIT);
        }
        AtomKind::Regex(r) => {
            buf.push(TAG_REGEX);
            encode_string(r, buf);
        }
        AtomKind::Uri(u) => {
            buf.push(TAG_URI);
            encode_string(u, buf);
        }
        AtomKind::Time(t) => {
            buf.push(TAG_TIME);
            encode_string(t, buf);
        }
        AtomKind::Bytes(b) => {
            buf.push(TAG_BYTES);
            encode_unsigned_leb128(b.len() as u64, buf);
            buf.extend_from_slice(b);
        }
    }
}

/// Work item for the iterative Expr walk. Lives at module scope so
/// helpers can name it; never part of the byte format.
enum ExprTask<'a> {
    Expr(&'a Expr),
    Atom(&'a AtomKind),
    Path(&'a Path),
    Field(&'a Field),
    FieldKey(&'a FieldKey),
    Relation(&'a Relation),
    StringPart(&'a StringPart),
    Byte(u8),
}

/// Structural identity of an `Expr` (Q-050). Spans are not content.
/// Iterative: the work list is an explicit stack, not the Rust call stack.
pub fn encode_expr(root: &Expr, buf: &mut Vec<u8>) {
    let mut stack = vec![ExprTask::Expr(root)];
    while let Some(task) = stack.pop() {
        match task {
            ExprTask::Byte(b) => buf.push(b),
            ExprTask::Atom(kind) => encode_expr_atom(kind, buf),
            ExprTask::Path(path) => encode_path(path, buf),
            ExprTask::Field(field) => {
                stack.push(ExprTask::Expr(&field.value));
                stack.push(ExprTask::FieldKey(&field.key));
            }
            ExprTask::FieldKey(key) => match key {
                FieldKey::Named { prefix, name } => {
                    buf.push(0x00);
                    match prefix {
                        None => buf.push(0x00),
                        Some(p) => {
                            buf.push(0x01);
                            buf.push(prefix_byte(*p));
                        }
                    }
                    encode_string(name, buf);
                }
                FieldKey::Quoted(s) => {
                    buf.push(0x01);
                    encode_string(s, buf);
                }
                FieldKey::Pattern(e) => {
                    buf.push(0x02);
                    stack.push(ExprTask::Expr(e));
                }
                FieldKey::Path(p) => {
                    buf.push(0x03);
                    stack.push(ExprTask::Path(p));
                }
            },
            ExprTask::Relation(r) => {
                stack.push(ExprTask::Atom(&r.right));
                stack.push(ExprTask::Byte(relop_byte(r.op)));
                stack.push(ExprTask::Atom(&r.left));
            }
            ExprTask::StringPart(part) => match part {
                StringPart::Literal(s) => {
                    buf.push(0x00);
                    encode_string(s, buf);
                }
                StringPart::Interpolated(e) => {
                    buf.push(0x01);
                    stack.push(ExprTask::Expr(e));
                }
            },
            ExprTask::Expr(expr) => match &expr.kind {
                ExprKind::Atom(kind) => {
                    buf.push(EX_ATOM);
                    stack.push(ExprTask::Atom(kind));
                }
                ExprKind::Path(path) => {
                    buf.push(EX_PATH);
                    stack.push(ExprTask::Path(path));
                }
                ExprKind::Apply(f, a) => push_bin(buf, &mut stack, EX_APPLY, f, a),
                ExprKind::Pipe(l, r) => push_bin(buf, &mut stack, EX_PIPE, l, r),
                ExprKind::Morphism { param, body } => {
                    buf.push(EX_MORPHISM);
                    stack.push(ExprTask::Expr(body));
                    stack.push(ExprTask::Expr(param));
                }
                ExprKind::Combo {
                    fields,
                    relations,
                    closed,
                } => {
                    buf.push(EX_COMBO);
                    buf.push(if *closed { 1 } else { 0 });
                    encode_unsigned_leb128(fields.len() as u64, buf);
                    encode_unsigned_leb128(relations.len() as u64, buf);
                    for r in relations.iter().rev() {
                        stack.push(ExprTask::Relation(r));
                    }
                    for f in fields.iter().rev() {
                        stack.push(ExprTask::Field(f));
                    }
                }
                ExprKind::Meet(a, b) => push_bin(buf, &mut stack, EX_MEET, a, b),
                ExprKind::Join(a, b) => push_bin(buf, &mut stack, EX_JOIN, a, b),
                ExprKind::Diff(a, b) => push_bin(buf, &mut stack, EX_DIFF, a, b),
                ExprKind::Complement(e) => {
                    buf.push(EX_COMPLEMENT);
                    stack.push(ExprTask::Expr(e));
                }
                ExprKind::Ternary {
                    cond,
                    then_branch,
                    else_branch,
                } => {
                    buf.push(EX_TERNARY);
                    stack.push(ExprTask::Expr(else_branch));
                    stack.push(ExprTask::Expr(then_branch));
                    stack.push(ExprTask::Expr(cond));
                }
                ExprKind::Add(a, b) => push_bin(buf, &mut stack, EX_ADD, a, b),
                ExprKind::Sub(a, b) => push_bin(buf, &mut stack, EX_SUB, a, b),
                ExprKind::Mul(a, b) => push_bin(buf, &mut stack, EX_MUL, a, b),
                ExprKind::Div(a, b) => push_bin(buf, &mut stack, EX_DIV, a, b),
                ExprKind::Rem(a, b) => push_bin(buf, &mut stack, EX_REM, a, b),
                ExprKind::Eq(a, b) => push_bin(buf, &mut stack, EX_EQ, a, b),
                ExprKind::Ne(a, b) => push_bin(buf, &mut stack, EX_NE, a, b),
                ExprKind::Lt(a, b) => push_bin(buf, &mut stack, EX_LT, a, b),
                ExprKind::Gt(a, b) => push_bin(buf, &mut stack, EX_GT, a, b),
                ExprKind::Lte(a, b) => push_bin(buf, &mut stack, EX_LTE, a, b),
                ExprKind::Gte(a, b) => push_bin(buf, &mut stack, EX_GTE, a, b),
                ExprKind::LatticeEq(a, b) => push_bin(buf, &mut stack, EX_LATTICE_EQ, a, b),
                ExprKind::Probe(a, b) => push_bin(buf, &mut stack, EX_PROBE, a, b),
                ExprKind::TypeAnnotation(a, b) => {
                    push_bin(buf, &mut stack, EX_TYPE_ANNOTATION, a, b)
                }
                ExprKind::Unary { op, expr } => {
                    buf.push(EX_UNARY);
                    buf.push(match op {
                        UnaryOp::Not => 0,
                        UnaryOp::Neg => 1,
                    });
                    stack.push(ExprTask::Expr(expr));
                }
                ExprKind::List(items) => {
                    buf.push(EX_LIST);
                    encode_unsigned_leb128(items.len() as u64, buf);
                    for item in items.iter().rev() {
                        stack.push(ExprTask::Expr(item));
                    }
                }
                ExprKind::Tuple(items) => {
                    buf.push(EX_TUPLE);
                    encode_unsigned_leb128(items.len() as u64, buf);
                    for item in items.iter().rev() {
                        stack.push(ExprTask::Expr(item));
                    }
                }
                ExprKind::Poset(relations) => {
                    buf.push(EX_POSET);
                    encode_unsigned_leb128(relations.len() as u64, buf);
                    for r in relations.iter().rev() {
                        stack.push(ExprTask::Relation(r));
                    }
                }
                ExprKind::Lens(a, b) => push_bin(buf, &mut stack, EX_LENS, a, b),
                ExprKind::AnonSet(e) => {
                    buf.push(EX_ANON_SET);
                    stack.push(ExprTask::Expr(e));
                }
                ExprKind::Interpolated(parts) => {
                    buf.push(EX_INTERPOLATED);
                    encode_unsigned_leb128(parts.len() as u64, buf);
                    for part in parts.iter().rev() {
                        stack.push(ExprTask::StringPart(part));
                    }
                }
                ExprKind::Range { start, end, step } => {
                    buf.push(EX_RANGE);
                    match step {
                        None => {
                            stack.push(ExprTask::Byte(0x00));
                            stack.push(ExprTask::Expr(end));
                            stack.push(ExprTask::Expr(start));
                        }
                        Some(s) => {
                            stack.push(ExprTask::Expr(s));
                            stack.push(ExprTask::Byte(0x01));
                            stack.push(ExprTask::Expr(end));
                            stack.push(ExprTask::Expr(start));
                        }
                    }
                }
                ExprKind::Context => buf.push(EX_CONTEXT),
                ExprKind::Spread(e) => {
                    buf.push(EX_SPREAD);
                    stack.push(ExprTask::Expr(e));
                }
                ExprKind::Structural(e) => {
                    buf.push(EX_STRUCTURAL);
                    stack.push(ExprTask::Expr(e));
                }
            },
        }
    }
}

fn push_bin<'a>(
    buf: &mut Vec<u8>,
    stack: &mut Vec<ExprTask<'a>>,
    tag: u8,
    a: &'a Expr,
    b: &'a Expr,
) {
    buf.push(tag);
    stack.push(ExprTask::Expr(b));
    stack.push(ExprTask::Expr(a));
}

fn encode_expr_atom(kind: &AtomKind, buf: &mut Vec<u8>) {
    match kind {
        AtomKind::MultilineStr(s) => {
            buf.push(TAG_EXPR_MULTILINE);
            encode_string(s, buf);
        }
        other => serialize_atom(other, buf),
    }
}

fn encode_path(path: &Path, buf: &mut Vec<u8>) {
    match path.anchor {
        PathAnchor::Bare => buf.push(0x00),
        PathAnchor::Root => buf.push(0x01),
        PathAnchor::Parent(n) => {
            buf.push(0x02);
            buf.extend_from_slice(&n.to_le_bytes());
        }
        PathAnchor::Current => buf.push(0x03),
        PathAnchor::Address { algo, digest } => {
            buf.push(0x04);
            buf.push(match algo {
                AddressAlgo::Sha256 => 0,
            });
            buf.extend_from_slice(&digest);
        }
    }
    encode_unsigned_leb128(path.segments.len() as u64, buf);
    for seg in &path.segments {
        encode_string(seg, buf);
    }
}

fn prefix_byte(p: Prefix) -> u8 {
    match p {
        Prefix::Data => 0,
        Prefix::Private => 1,
        Prefix::Logic => 2,
        Prefix::Type => 3,
        Prefix::Meta => 4,
        Prefix::System => 5,
        Prefix::Local => 6,
    }
}

fn relop_byte(op: RelOp) -> u8 {
    match op {
        RelOp::Lt => 0,
        RelOp::Gt => 1,
        RelOp::Lte => 2,
        RelOp::Gte => 3,
        RelOp::Eq => 4,
    }
}

fn serialize_combo(cv: &ComboVal, buf: &mut Vec<u8>) {
    buf.push(if cv.closed { TAG_COCOON } else { TAG_COMBO });

    let mut entries: Vec<(u8, &str, &Value)> = Vec::new();
    for (k, v) in &cv.system {
        entries.push((1, k.as_str(), v));
    }
    for (k, v) in &cv.meta {
        entries.push((2, k.as_str(), v));
    }
    for (k, v) in &cv.types {
        entries.push((3, k.as_str(), v));
    }
    for (k, v) in &cv.rules {
        entries.push((4, k.as_str(), v));
    }
    for (k, v) in &cv.data {
        entries.push((5, k.as_str(), v));
    }
    for (k, v) in &cv.local {
        entries.push((6, k.as_str(), v));
    }
    for (k, v) in &cv.legacy_fields {
        entries.push((5, k.as_str(), v));
    }
    for (k, v) in &cv.legacy_local {
        entries.push((6, k.as_str(), v));
    }

    entries.sort_by(|a, b| {
        let cmp = a.0.cmp(&b.0);
        if cmp != std::cmp::Ordering::Equal {
            return cmp;
        }
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
        if !more {
            break;
        }
    }
}

fn encode_signed_leb128_int(n: &num_bigint::BigInt, buf: &mut Vec<u8>) {
    if let Some(i) = n.to_i64() {
        encode_signed_leb128(i, buf);
        return;
    }
    let mut val = n.clone();
    let neg_one = num_bigint::BigInt::from(-1);
    loop {
        let byte = (&val & num_bigint::BigInt::from(0x7fu8))
            .to_u8()
            .expect("7-bit mask");
        val >>= 7u32;
        let more = !((val.sign() == num_bigint::Sign::NoSign && (byte & 0x40) == 0)
            || (val == neg_one && (byte & 0x40) != 0));
        buf.push(if more { byte | 0x80 } else { byte });
        if !more {
            break;
        }
    }
}

pub fn encode_unsigned_leb128(mut val: u64, buf: &mut Vec<u8>) {
    loop {
        let byte = (val as u8) & 0x7f;
        val >>= 7;
        buf.push(if val != 0 { byte | 0x80 } else { byte });
        if val == 0 {
            break;
        }
    }
}

fn encode_fixed128(val: f64, buf: &mut Vec<u8>) {
    let whole = val.trunc() as i64;
    let frac = (val.fract().abs() * 18446744073709551616.0) as u64; // 2^64 as float
    encode_signed_leb128(whole, buf);
    buf.extend_from_slice(&frac.to_le_bytes());
}
