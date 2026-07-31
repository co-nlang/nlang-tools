use crate::value::{EffectTag, Value};
use crate::{BuiltinFn, EvalContext, Ouroboros};
use base64::{engine::general_purpose, Engine as _};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use ring::hmac;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;

pub fn register_bytes_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    // bytes.from_str: {0: str} → Bytes (UTF-8 encoded)
    m.insert(
        "bytes.from_str".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
                return Value::Atom(
                    AtomKind::Bytes(s.as_bytes().to_vec()),
                    EffectTag::Pure,
                    None,
                );
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // bytes.to_str: {0: bytes} → Str | #none (UTF-8 decode)
    m.insert(
        "bytes.to_str".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            if let Value::Atom(AtomKind::Bytes(b), _, _) = oo.force(v, ctx).collapse() {
                return match String::from_utf8(b.clone()) {
                    Ok(s) => Value::Atom(AtomKind::Str(s), EffectTag::Pure, None),
                    Err(_) => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
                };
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // bytes.len: {0: bytes} → Int
    m.insert(
        "bytes.len".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            if let Value::Atom(AtomKind::Bytes(b), _, _) = oo.force(v, ctx).collapse() {
                return Value::Atom(AtomKind::Int(BigInt::from(b.len())), EffectTag::Pure, None);
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // bytes.at: {0: idx, 1: bytes} → Int (0–255), Top if out of range
    m.insert(
        "bytes.at".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vi), Some(vb)) = (c.get_field("0"), c.get_field("1")) {
                    let fi = oo.force(vi.clone(), ctx);
                    let fb = oo.force(vb.clone(), ctx);
                    if let (
                        Value::Atom(AtomKind::Int(idx), _, _),
                        Value::Atom(AtomKind::Bytes(b), _, _),
                    ) = (fi.collapse(), fb.collapse())
                    {
                        if let Some(i) = idx.to_usize() {
                            if let Some(&byte_val) = b.get(i) {
                                return Value::Atom(
                                    AtomKind::Int(BigInt::from(byte_val)),
                                    EffectTag::Pure,
                                    None,
                                );
                            }
                        }
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // bytes.concat: {0: bytes_a, 1: bytes_b} → Bytes
    m.insert(
        "bytes.concat".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(va), Some(vb)) = (c.get_field("0"), c.get_field("1")) {
                    let fa = oo.force(va.clone(), ctx);
                    let fb = oo.force(vb.clone(), ctx);
                    if let (
                        Value::Atom(AtomKind::Bytes(ba), _, _),
                        Value::Atom(AtomKind::Bytes(bb), _, _),
                    ) = (fa.collapse(), fb.collapse())
                    {
                        let mut out = ba.clone();
                        out.extend_from_slice(bb);
                        return Value::Atom(AtomKind::Bytes(out), EffectTag::Pure, None);
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // bytes.slice: {0: start, 1: end, 2: bytes} → Bytes (silently clamped)
    m.insert(
        "bytes.slice".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vs), Some(ve), Some(vb)) =
                    (c.get_field("0"), c.get_field("1"), c.get_field("2"))
                {
                    let fs = oo.force(vs.clone(), ctx);
                    let fe = oo.force(ve.clone(), ctx);
                    let fb = oo.force(vb.clone(), ctx);
                    if let (
                        Value::Atom(AtomKind::Int(s), _, _),
                        Value::Atom(AtomKind::Int(e), _, _),
                        Value::Atom(AtomKind::Bytes(b), _, _),
                    ) = (fs.collapse(), fe.collapse(), fb.collapse())
                    {
                        let len = b.len();
                        let start = s.to_usize().unwrap_or(0).min(len);
                        let end = e.to_usize().unwrap_or(0).min(len);
                        let sliced = if start <= end {
                            b[start..end].to_vec()
                        } else {
                            vec![]
                        };
                        return Value::Atom(AtomKind::Bytes(sliced), EffectTag::Pure, None);
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // bytes.to_hex: {0: bytes} → Str (lowercase hex, no 0x prefix)
    m.insert(
        "bytes.to_hex".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            if let Value::Atom(AtomKind::Bytes(b), _, _) = oo.force(v, ctx).collapse() {
                return Value::Atom(AtomKind::Str(hex::encode(b)), EffectTag::Pure, None);
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // bytes.from_hex: {0: str} → Bytes | #none (invalid hex → #none)
    m.insert(
        "bytes.from_hex".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
                return match hex::decode(s.trim()) {
                    Ok(bytes) => Value::Atom(AtomKind::Bytes(bytes), EffectTag::Pure, None),
                    Err(_) => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
                };
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // bytes.sha256: {0: bytes} → Bytes (32-byte SHA-256 hash)
    m.insert(
        "bytes.sha256".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            if let Value::Atom(AtomKind::Bytes(b), _, _) = oo.force(v, ctx).collapse() {
                let mut hasher = Sha256::new();
                hasher.update(b);
                return Value::Atom(
                    AtomKind::Bytes(hasher.finalize().to_vec()),
                    EffectTag::Pure,
                    None,
                );
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // bytes.base64_encode: {0: bytes} → Str (standard base64, with padding)
    m.insert(
        "bytes.base64_encode".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            if let Value::Atom(AtomKind::Bytes(b), _, _) = oo.force(v, ctx).collapse() {
                return Value::Atom(
                    AtomKind::Str(general_purpose::STANDARD.encode(b)),
                    EffectTag::Pure,
                    None,
                );
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // bytes.base64_decode: {0: str} → Bytes | #none (invalid base64 → #none)
    m.insert(
        "bytes.base64_decode".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
                return match general_purpose::STANDARD.decode(s.trim()) {
                    Ok(bytes) => Value::Atom(AtomKind::Bytes(bytes), EffectTag::Pure, None),
                    Err(_) => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
                };
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // bytes.hmac_sha256: {0: key_bytes, 1: msg_bytes} → Bytes (32-byte HMAC-SHA256 tag)
    m.insert(
        "bytes.hmac_sha256".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vk), Some(vm)) = (c.get_field("0"), c.get_field("1")) {
                    let fk = oo.force(vk.clone(), ctx);
                    let fm = oo.force(vm.clone(), ctx);
                    if let (
                        Value::Atom(AtomKind::Bytes(key), _, _),
                        Value::Atom(AtomKind::Bytes(msg), _, _),
                    ) = (fk.collapse(), fm.collapse())
                    {
                        let signing_key = hmac::Key::new(hmac::HMAC_SHA256, key);
                        let tag = hmac::sign(&signing_key, msg);
                        return Value::Atom(
                            AtomKind::Bytes(tag.as_ref().to_vec()),
                            EffectTag::Pure,
                            None,
                        );
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );
}
