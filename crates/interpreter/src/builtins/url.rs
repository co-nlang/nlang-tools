use crate::value::{BottomCause, BottomDetail, ComboVal, EffectTag, Value};
use crate::{BuiltinFn, EvalContext, Ouroboros};
use indexmap::IndexMap;
use nlang_parser::ast::AtomKind;
use std::collections::HashMap;
use std::sync::Arc;

fn str_atom(s: impl Into<String>) -> Value {
    Value::Atom(AtomKind::Str(s.into()), EffectTag::Pure, None)
}

fn conflict() -> Value {
    BottomCause::Conflict.into()
}

fn extract_str(v: &Value) -> Option<String> {
    match v {
        Value::Atom(AtomKind::Str(s), _, _) => Some(s.clone()),
        _ => None,
    }
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

fn url_decode(s: &str) -> String {
    let mut bytes: Vec<u8> = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' && i + 2 < chars.len() {
            let hex: String = chars[i + 1..=i + 2].iter().collect();
            if let Ok(b) = u8::from_str_radix(&hex, 16) {
                bytes.push(b);
                i += 3;
                continue;
            }
        }
        bytes.extend_from_slice(chars[i].to_string().as_bytes());
        i += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
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

pub fn register_url_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert(
        "url.parse".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = oo.force(arg, ctx);
            let s = match extract_str_arg(&v, oo, ctx) {
                Some(s) => s,
                None => return conflict(),
            };
            match ::url::Url::parse(&s) {
                Ok(u) => {
                    let mut fields = IndexMap::new();
                    fields.insert("scheme".to_string(), str_atom(u.scheme()));
                    fields.insert("host".to_string(), str_atom(u.host_str().unwrap_or("")));
                    fields.insert("path".to_string(), str_atom(u.path()));
                    fields.insert("query".to_string(), str_atom(u.query().unwrap_or("")));
                    fields.insert("fragment".to_string(), str_atom(u.fragment().unwrap_or("")));
                    Value::Combo(ComboVal::new(
                        fields,
                        false,
                        IndexMap::new(),
                        EffectTag::Pure,
                        vec![],
                    ))
                }
                Err(e) => Value::Bottom(Box::new(BottomDetail {
                    cause: BottomCause::Conflict,
                    message: Some(format!("url.parse: {}", e)),
                    ..Default::default()
                })),
            }
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "url.encode".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = oo.force(arg, ctx);
            let s = match extract_str_arg(&v, oo, ctx) {
                Some(s) => s,
                None => return conflict(),
            };
            str_atom(url_encode(&s))
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "url.decode".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = oo.force(arg, ctx);
            let s = match extract_str_arg(&v, oo, ctx) {
                Some(s) => s,
                None => return conflict(),
            };
            str_atom(url_decode(&s))
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "url.join".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let c = match arg {
                Value::Combo(ref c) => c.clone(),
                _ => return conflict(),
            };
            let base_s =
                extract_str(&oo.force(c.get_field("0").cloned().unwrap_or(Value::Top), ctx))
                    .unwrap_or_default();
            let rel_s =
                extract_str(&oo.force(c.get_field("1").cloned().unwrap_or(Value::Top), ctx))
                    .unwrap_or_default();
            match ::url::Url::parse(&base_s).and_then(|base| base.join(&rel_s)) {
                Ok(u) => str_atom(u.as_str()),
                Err(e) => Value::Bottom(Box::new(BottomDetail {
                    cause: BottomCause::Conflict,
                    message: Some(format!("url.join: {}", e)),
                    ..Default::default()
                })),
            }
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "url.query_params".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = oo.force(arg, ctx);
            let s = match extract_str_arg(&v, oo, ctx) {
                Some(s) => s,
                None => return conflict(),
            };
            match ::url::Url::parse(&s) {
                Ok(u) => {
                    let mut fields = IndexMap::new();
                    for (k, v) in u.query_pairs() {
                        fields.insert(k.into_owned(), str_atom(v.into_owned()));
                    }
                    Value::Combo(ComboVal::new(
                        fields,
                        false,
                        IndexMap::new(),
                        EffectTag::Pure,
                        vec![],
                    ))
                }
                Err(_) => Value::Combo(ComboVal::new(
                    IndexMap::new(),
                    false,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                )),
            }
        }) as Arc<BuiltinFn>,
    );
}
