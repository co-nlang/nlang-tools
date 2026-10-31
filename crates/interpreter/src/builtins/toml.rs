use crate::value::{BottomCause, BottomDetail, ComboVal, EffectTag, Value};
use crate::{BuiltinFn, EvalContext, Ouroboros};
use indexmap::IndexMap;
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use std::collections::HashMap;
use std::sync::Arc;

fn conflict() -> Value {
    BottomCause::Conflict.into()
}

fn toml_to_value(t: &::toml::Value, ctx: &mut EvalContext) -> Result<Value, Value> {
    Ok(match t {
        ::toml::Value::String(s) => Value::Atom(AtomKind::Str(s.clone()), EffectTag::Pure, None),
        ::toml::Value::Integer(i) => {
            Value::Atom(AtomKind::Int(BigInt::from(*i)), EffectTag::Pure, None)
        }
        ::toml::Value::Float(f) => Value::Atom(AtomKind::Float(*f), EffectTag::Pure, None),
        ::toml::Value::Boolean(b) => Value::Atom(
            AtomKind::Tag(if *b { "true" } else { "false" }.to_string()),
            EffectTag::Pure,
            None,
        ),
        ::toml::Value::Datetime(dt) => {
            Value::Atom(AtomKind::Str(dt.to_string()), EffectTag::Pure, None)
        }
        ::toml::Value::Array(arr) => {
            super::read_deeper(ctx)?;
            let mut m = IndexMap::new();
            m.insert(
                "%kind".to_string(),
                Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
            );
            for (i, v) in arr.iter().enumerate() {
                let v = toml_to_value(v, ctx);
                m.insert(i.to_string(), v.map_err(|h| { ctx.depth -= 1; h })?);
            }
            ctx.depth -= 1;
            Value::Combo(ComboVal::new(
                m,
                false,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            ))
        }
        ::toml::Value::Table(tbl) => {
            super::read_deeper(ctx)?;
            let mut fields = IndexMap::new();
            for (k, v) in tbl {
                let v = toml_to_value(v, ctx);
                fields.insert(k.clone(), v.map_err(|h| { ctx.depth -= 1; h })?);
            }
            ctx.depth -= 1;
            Value::Combo(ComboVal::new(
                fields,
                false,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            ))
        }
    })
}

/// Q-114: an upper bound on the depth of the value a TOML text reads as.
/// A statement costs the dots of its keys (in a header, before `=`, in an
/// inline table — not in a value, where `1.5` is a number), the most
/// containers it opens and one; a key/value statement adds the table
/// header it sits under. Strings and comments skipped.
fn toml_nesting_bound(s: &str) -> usize {
    let b = s.as_bytes();
    let (mut i, mut header, mut most) = (0usize, 0usize, 0usize);
    let mut open: Vec<u8> = Vec::new();
    let (mut dots, mut nest, mut is_header, mut fresh, mut in_key) = (0usize, 0usize, false, true, true);
    loop {
        let c = b.get(i).copied();
        if matches!(c, None | Some(b'\n')) && open.is_empty() {
            if !fresh {
                let cost = dots + nest + 1;
                if is_header {
                    header = cost;
                }
                most = most.max(if is_header { cost } else { header + cost });
            }
            (dots, nest, is_header, fresh, in_key) = (0, 0, false, true, true);
            if c.is_none() {
                return most;
            }
            i += 1;
            continue;
        }
        let Some(c) = c else { return most.max(header + dots + nest + 1) };
        match c {
            b'"' | b'\'' => {
                let q: &[u8] = match (c, b[i..].starts_with(b"\"\"\""), b[i..].starts_with(b"'''")) {
                    (b'"', true, _) => b"\"\"\"",
                    (b'\'', _, true) => b"'''",
                    (b'"', ..) => b"\"",
                    _ => b"'",
                };
                i += q.len();
                while i < b.len() && !b[i..].starts_with(q) {
                    i += if c == b'"' && b[i] == b'\\' { 2 } else { 1 };
                }
                i += q.len();
                fresh = false;
                continue;
            }
            b'#' => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b' ' | b'\t' | b'\r' | b'\n' => {}
            _ => {
                if fresh && open.is_empty() {
                    is_header = c == b'[';
                }
                fresh = false;
                match c {
                    b'[' | b'{' => {
                        open.push(c);
                        nest = nest.max(open.len());
                        in_key = c == b'{' || is_header;
                    }
                    b']' | b'}' => {
                        open.pop();
                        in_key = is_header && open.is_empty();
                    }
                    b',' => in_key = open.last() == Some(&b'{'),
                    b'=' => in_key = false,
                    b'.' if in_key => dots += 1,
                    _ => {}
                }
            }
        }
        i += 1;
    }
}

fn value_to_toml(v: &Value) -> Option<::toml::Value> {
    match v {
        Value::Atom(AtomKind::Str(s), _, _) => Some(::toml::Value::String(s.clone())),
        Value::Atom(AtomKind::Int(n), _, _) => {
            use num_traits::ToPrimitive;
            n.to_i64().map(::toml::Value::Integer)
        }
        Value::Atom(AtomKind::Float(f), _, _) => Some(::toml::Value::Float(*f)),
        Value::Atom(AtomKind::Tag(t), _, _) => {
            if t == "true" {
                Some(::toml::Value::Boolean(true))
            } else if t == "false" {
                Some(::toml::Value::Boolean(false))
            } else {
                None
            }
        }
        Value::Combo(c) => {
            if let Some(Value::Atom(AtomKind::Tag(kind), _, _)) = c.get_field("%kind") {
                if kind == "list" {
                    let mut arr = Vec::new();
                    for i in 0u32.. {
                        match c.get_field(&i.to_string()) {
                            Some(v) => match value_to_toml(v) {
                                Some(tv) => arr.push(tv),
                                None => return None,
                            },
                            None => break,
                        }
                    }
                    return Some(::toml::Value::Array(arr));
                }
            }
            let mut tbl = ::toml::map::Map::new();
            for (k, v) in c.all_fields_iter() {
                if k.starts_with('%') || k.starts_with('@') || k.starts_with('~') {
                    continue;
                }
                match value_to_toml(&v) {
                    Some(tv) => {
                        tbl.insert(k.clone(), tv);
                    }
                    None => return None,
                }
            }
            Some(::toml::Value::Table(tbl))
        }
        _ => None,
    }
}

fn extract_str_arg(v: &Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Option<String> {
    match v {
        Value::Atom(AtomKind::Str(s), _, _) => Some(s.clone()),
        Value::Combo(c) => match c.get_field("0") {
            Some(v) => match oo.force(v.clone(), ctx) {
                Value::Atom(AtomKind::Str(s), _, _) => Some(s),
                _ => None,
            },
            None => None,
        },
        _ => None,
    }
}

pub fn register_toml_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert(
        "toml.parse".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = oo.force(arg, ctx);
            let s = match extract_str_arg(&v, oo, ctx) {
                Some(s) => s,
                None => return conflict(),
            };
            // Q-114: as deep as the text is; past the depth a value may
            // have, the horizon — not a conflict, which says "not TOML".
            if let Some(h) = super::text_too_deep(toml_nesting_bound(&s), ctx) {
                return h;
            }
            match ::toml::from_str::<::toml::Value>(&s) {
                Ok(tv) => toml_to_value(&tv, ctx).unwrap_or_else(|h| h),
                Err(e) => Value::Bottom(Box::new(BottomDetail {
                    cause: BottomCause::Conflict,
                    message: Some(format!("toml.parse: {}", e)),
                    ..Default::default()
                })),
            }
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "toml.stringify".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = oo.force(arg, ctx);
            let target = match &v {
                Value::Combo(c) => match c.get_field("0") {
                    Some(inner) => inner.clone(),
                    None => v,
                },
                _ => v,
            };
            // Q-113: the value written, read field by field.
            super::written_text(oo, target, ctx, &mut |target, _| match value_to_toml(target) {
                Some(tv) => match ::toml::to_string_pretty(&tv) {
                    Ok(s) => Value::Atom(AtomKind::Str(s), EffectTag::Pure, None),
                    Err(e) => Value::Bottom(Box::new(BottomDetail {
                        cause: BottomCause::Conflict,
                        message: Some(format!("toml.stringify: {}", e)),
                        ..Default::default()
                    })),
                },
                None => conflict(),
            })
        }) as Arc<BuiltinFn>,
    );
}
