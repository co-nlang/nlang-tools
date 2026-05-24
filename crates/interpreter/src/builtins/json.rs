use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use indexmap::IndexMap;

fn json_to_nlang(json: serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => {
            Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None)
        }
        serde_json::Value::Bool(b) => {
            Value::Atom(AtomKind::Tag(if b { "true" } else { "false" }.to_string()), EffectTag::Pure, None)
        }
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Atom(AtomKind::Int(BigInt::from(i)), EffectTag::Pure, None)
            } else if let Some(u) = n.as_u64() {
                Value::Atom(AtomKind::Int(BigInt::from(u)), EffectTag::Pure, None)
            } else {
                Value::Atom(AtomKind::Float(n.as_f64().unwrap_or(f64::NAN)), EffectTag::Pure, None)
            }
        }
        serde_json::Value::String(s) => {
            Value::Atom(AtomKind::Str(s), EffectTag::Pure, None)
        }
        serde_json::Value::Array(arr) => {
            let mut res = IndexMap::new();
            for (i, v) in arr.into_iter().enumerate() {
                res.insert(i.to_string(), json_to_nlang(v));
            }
            res.insert("%kind".to_string(),
                Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
            Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]))
        }
        serde_json::Value::Object(obj) => {
            let mut res = IndexMap::new();
            for (k, v) in obj {
                res.insert(k, json_to_nlang(v));
            }
            Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]))
        }
    }
}

fn nlang_to_json(val: &Value) -> serde_json::Value {
    match val.collapse() {
        Value::Atom(AtomKind::Tag(t), _, _) => {
            match t.trim_start_matches('#') {
                "none" | "null" => serde_json::Value::Null,
                "true"          => serde_json::Value::Bool(true),
                "false"         => serde_json::Value::Bool(false),
                other           => serde_json::Value::String(format!("#{}", other)),
            }
        }
        Value::Atom(AtomKind::Int(n), _, _) => {
            n.to_i64()
                .map(|i| serde_json::Value::Number(i.into()))
                .unwrap_or_else(|| serde_json::Value::String(n.to_string()))
        }
        Value::Atom(AtomKind::Float(f), _, _) => {
            serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        Value::Atom(AtomKind::Str(s), _, _) => serde_json::Value::String(s.clone()),
        Value::Atom(AtomKind::Bytes(b), _, _) => serde_json::Value::String(hex::encode(b)),
        Value::Combo(c) => {
            let is_list = c.get_field("%kind").map(|k| {
                matches!(k.collapse(),
                    Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "list")
            }).unwrap_or(false);

            if is_list {
                let mut arr = Vec::new();
                let mut i = 0usize;
                loop {
                    match c.get_field(&i.to_string()) {
                        Some(v) => { arr.push(nlang_to_json(v)); i += 1; }
                        None    => break,
                    }
                }
                serde_json::Value::Array(arr)
            } else {
                let mut obj = serde_json::Map::new();
                for (k, v) in c.fields_iter() {
                    if !k.starts_with('%') {
                        obj.insert(k.clone(), nlang_to_json(v));
                    }
                }
                serde_json::Value::Object(obj)
            }
        }
        _ => serde_json::Value::Null,
    }
}

pub fn register_json_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // json.parse: {0: str} → Value | #none (invalid JSON → #none)
    m.insert("json.parse".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
            return match serde_json::from_str::<serde_json::Value>(s) {
                Ok(json) => json_to_nlang(json),
                Err(_)   => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
            };
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // json.stringify: {0: value} → Str | #none
    m.insert("json.stringify".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        let json_val = nlang_to_json(&forced);
        match serde_json::to_string(&json_val) {
            Ok(s)  => Value::Atom(AtomKind::Str(s), EffectTag::Pure, None),
            Err(_) => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
        }
    }) as Arc<BuiltinFn>);

    // json.get: {0: key_str, 1: combo} → Value | #none
    // Returns #none for: key not found, key starts with %, arg1 not a Combo.
    m.insert("json.get".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vk), Some(vv)) = (c.get_field("0"), c.get_field("1")) {
                let fk = oo.force(vk.clone(), ctx);
                let fv = oo.force(vv.clone(), ctx);
                if let Value::Atom(AtomKind::Str(key), _, _) = fk.collapse() {
                    if key.starts_with('%') {
                        return Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
                    }
                    if let Value::Combo(ref cv) = fv.collapse() {
                        return match cv.get_field(key) {
                            Some(v) => v.clone(),
                            None    => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
                        };
                    }
                    return Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // json.keys: {0: combo} → list of Str (all non-%-prefixed keys, in insertion order)
    m.insert("json.keys".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        if let Value::Combo(ref cv) = forced.collapse() {
            let mut res = IndexMap::new();
            let mut i = 0usize;
            for (k, _) in cv.fields_iter() {
                if !k.starts_with('%') {
                    res.insert(i.to_string(),
                        Value::Atom(AtomKind::Str(k.clone()), EffectTag::Pure, None));
                    i += 1;
                }
            }
            res.insert("%kind".to_string(),
                Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
            return Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]));
        }
        Value::Top
    }) as Arc<BuiltinFn>);
}
