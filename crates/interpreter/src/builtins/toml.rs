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

fn toml_to_value(t: &::toml::Value) -> Value {
    match t {
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
            let mut m = IndexMap::new();
            m.insert(
                "%kind".to_string(),
                Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
            );
            for (i, v) in arr.iter().enumerate() {
                m.insert(i.to_string(), toml_to_value(v));
            }
            Value::Combo(ComboVal::new(
                m,
                false,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            ))
        }
        ::toml::Value::Table(tbl) => {
            let mut fields = IndexMap::new();
            for (k, v) in tbl {
                fields.insert(k.clone(), toml_to_value(v));
            }
            Value::Combo(ComboVal::new(
                fields,
                false,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            ))
        }
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
            match ::toml::from_str::<::toml::Value>(&s) {
                Ok(tv) => toml_to_value(&tv),
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
                    Some(inner) => oo.force(inner.clone(), ctx),
                    None => v,
                },
                _ => v,
            };
            match value_to_toml(&target) {
                Some(tv) => match ::toml::to_string_pretty(&tv) {
                    Ok(s) => Value::Atom(AtomKind::Str(s), EffectTag::Pure, None),
                    Err(e) => Value::Bottom(Box::new(BottomDetail {
                        cause: BottomCause::Conflict,
                        message: Some(format!("toml.stringify: {}", e)),
                        ..Default::default()
                    })),
                },
                None => conflict(),
            }
        }) as Arc<BuiltinFn>,
    );
}
