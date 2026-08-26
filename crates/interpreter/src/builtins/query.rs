use crate::value::{BottomCause, BottomDetail, ComboVal, EffectTag, Value};
use crate::{BuiltinFn, EvalContext, Ouroboros};
use indexmap::IndexMap;
use nlang_parser::ast::AtomKind;
use std::collections::HashMap;
use std::sync::Arc;

// ── Shared path helpers (also used by Phase 44 ~%Diff) ─────────────────────

pub fn parse_path(s: &str) -> Vec<String> {
    if s.is_empty() {
        return vec![];
    }
    s.split('.').map(|seg| seg.to_string()).collect()
}

pub fn get_at_path(
    val: &Value,
    path: &[String],
    oo: &Ouroboros,
    ctx: &mut EvalContext,
) -> Option<Value> {
    if path.is_empty() {
        return Some(val.clone());
    }
    match val {
        Value::Combo(c) => {
            let field = c.get_field(&path[0])?;
            let next = oo.force(field.clone(), ctx);
            get_at_path(&next, &path[1..], oo, ctx)
        }
        _ => None,
    }
}

/// Immutably update a Value tree at the given path.
/// Returns the rebuilt tree with new_val at path, or Bottom(MissingKey) if path traverses non-Combo.
pub fn set_at_path(val: Value, path: &[String], new_val: Value) -> Value {
    if path.is_empty() {
        return new_val;
    }
    match val {
        Value::Combo(mut c) => {
            let key = &path[0];
            let child = c.get_field(key).cloned().unwrap_or(Value::Top);
            let updated = set_at_path(child, &path[1..], new_val);
            c.insert_field(key, updated);
            Value::Combo(c)
        }
        _ => Value::Bottom(Box::new(BottomDetail {
            cause: BottomCause::MissingKey,
            path: Some(path.join(".")),
            message: Some("Cannot navigate into non-Combo value".to_string()),
            ..Default::default()
        })),
    }
}

fn extract_list_items(list: &Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Vec<Value> {
    if let Value::Combo(c) = list {
        let mut items = Vec::new();
        for i in 0u32.. {
            if let Some(v) = c.get_field(&i.to_string()) {
                items.push(oo.force(v.clone(), ctx));
            } else {
                break;
            }
        }
        items
    } else {
        vec![]
    }
}

fn build_list(items: Vec<Value>, effect: EffectTag) -> Value {
    let mut m = IndexMap::new();
    m.insert(
        "%kind".to_string(),
        Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
    );
    for (i, v) in items.iter().enumerate() {
        m.insert(i.to_string(), v.clone());
    }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), effect, vec![]))
}

pub fn deep_merge_values(a: Value, b: Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Value {
    match (&a, &b) {
        (Value::Combo(ca), Value::Combo(cb)) => {
            let mut merged = ca.clone();
            for (key, vb) in cb.all_fields_iter() {
                let va = ca.get_field(&key).cloned().unwrap_or(Value::Top);
                let result = deep_merge_values(va, vb, oo, ctx);
                merged.insert_field(&key, result);
            }
            Value::Combo(merged)
        }
        _ => oo.unify_internal(a, b, ctx),
    }
}

pub fn register_query_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    // query.select: {0: value, 1: path_str} → value at path | Bottom(MissingKey)
    m.insert(
        "query.select".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let c = match arg {
                Value::Combo(ref c) => c.clone(),
                _ => return BottomCause::Conflict.into(),
            };
            let val = match c.get_field("0") {
                Some(v) => oo.force(v.clone(), ctx),
                None => return BottomCause::Conflict.into(),
            };
            let path_str = match c.get_field("1").or_else(|| c.get_field("path")) {
                Some(v) => oo.force(v.clone(), ctx).to_string_plain(),
                None => return val,
            };
            let segments = parse_path(&path_str);
            get_at_path(&val, &segments, oo, ctx).unwrap_or_else(|| {
                Value::Bottom(Box::new(BottomDetail {
                    cause: BottomCause::MissingKey,
                    path: Some(path_str),
                    message: Some("Path not found in value".to_string()),
                    ..Default::default()
                }))
            })
        }) as Arc<BuiltinFn>,
    );

    // query.where: {0: pred, 1: list} → filtered @list  (data last, O78 ③)
    m.insert(
        "query.where".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let c = match arg {
                Value::Combo(ref c) => c.clone(),
                _ => return Value::Top,
            };
            let pred = match c.get_field("0") {
                Some(v) => v.clone(),
                None => return Value::Top,
            };
            let list_val = match c.get_field("1") {
                Some(v) => oo.force(v.clone(), ctx),
                None => return Value::Top,
            };
            if !oo.is_list(&list_val, ctx) {
                return Value::Top;
            }
            let items = extract_list_items(&list_val, oo, ctx);
            let mut kept = Vec::new();
            let mut max_effect = EffectTag::Pure;
            for item in items {
                let result = oo.apply_morphism(pred.clone(), item.clone(), ctx);
                max_effect = max_effect.union(result.effect());
                match super::pred_is_true(&result) {
                    Err(bottom) => return bottom,
                    Ok(true) => kept.push(item),
                    Ok(false) => {}
                }
            }
            // O74 ④: no unconditional IO floor. An honest pure predicate
            // must be writable; an impure one is carried by S1 (the
            // applications are unioned into the builtin result).
            build_list(kept, max_effect)
        }) as Arc<BuiltinFn>,
    );

    // query.pluck: {0: combo, 1: key_list} → Combo with only specified keys
    m.insert(
        "query.pluck".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let c = match arg {
                Value::Combo(ref c) => c.clone(),
                _ => return BottomCause::Conflict.into(),
            };
            let combo_val = match c.get_field("0") {
                Some(v) => oo.force(v.clone(), ctx),
                None => return BottomCause::Conflict.into(),
            };
            let key_list_val = match c.get_field("1") {
                Some(v) => oo.force(v.clone(), ctx),
                None => return BottomCause::Conflict.into(),
            };
            let keys: Vec<String> = extract_list_items(&key_list_val, oo, ctx)
                .into_iter()
                .map(|v| v.to_string_plain())
                .collect();

            let src = match combo_val {
                Value::Combo(c) => c,
                _ => return BottomCause::Conflict.into(),
            };
            let mut result_fields = IndexMap::new();
            for key in &keys {
                if let Some(v) = src.get_field(key) {
                    result_fields.insert(key.clone(), v.clone());
                }
            }
            Value::Combo(ComboVal::new(
                result_fields,
                false,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            ))
        }) as Arc<BuiltinFn>,
    );

    // query.deep_merge: {0: a, 1: b} → recursively merged Combo
    m.insert(
        "query.deep_merge".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let c = match arg {
                Value::Combo(ref c) => c.clone(),
                _ => return BottomCause::Conflict.into(),
            };
            let a = match c.get_field("0") {
                Some(v) => oo.force(v.clone(), ctx),
                None => return BottomCause::Conflict.into(),
            };
            let b = match c.get_field("1") {
                Some(v) => oo.force(v.clone(), ctx),
                None => return BottomCause::Conflict.into(),
            };
            deep_merge_values(a, b, oo, ctx)
        }) as Arc<BuiltinFn>,
    );
}
