use std::sync::Arc;
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, BottomCause, BottomDetail, ComboVal};
use crate::builtins::query::{parse_path, set_at_path, deep_merge_values};
use nlang_parser::ast::AtomKind;

// ── Value equality ───────────────────────────────────────────────────────────

fn same_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Top, Value::Top) => true,
        (Value::Bottom(_), Value::Bottom(_)) => true,
        (Value::Atom(ka, _, _), Value::Atom(kb, _, _)) => {
            format!("{:?}", ka) == format!("{:?}", kb)
        }
        (Value::Combo(ca), Value::Combo(cb)) => {
            let a_len = ca.all_fields_iter().count();
            let b_len = cb.all_fields_iter().count();
            if a_len != b_len { return false; }
            for (key, va) in ca.all_fields_iter() {
                match cb.get_field(&key) {
                    Some(vb) => if !same_value(&va, vb) { return false; }
                    None => return false,
                }
            }
            true
        }
        _ => false,
    }
}

// ── Diff collection helper ───────────────────────────────────────────────────

fn str_atom(s: impl Into<String>) -> Value {
    Value::Atom(AtomKind::Str(s.into()), EffectTag::Pure, None)
}

fn missing() -> Value {
    Value::Bottom(Box::new(BottomDetail {
        cause: BottomCause::MissingKey,
        ..Default::default()
    }))
}

fn collect_diffs(a: &Value, b: &Value, prefix: &str, acc: &mut Vec<Value>) {
    if same_value(a, b) { return; }
    match (a, b) {
        (Value::Combo(ca), Value::Combo(cb)) => {
            let mut keys: Vec<String> = Vec::new();
            for (k, _) in ca.all_fields_iter() { if !keys.contains(&k) { keys.push(k.clone()); } }
            for (k, _) in cb.all_fields_iter() { if !keys.contains(&k) { keys.push(k.clone()); } }
            for key in keys {
                let va = ca.get_field(&key).cloned().unwrap_or_else(missing);
                let vb = cb.get_field(&key).cloned().unwrap_or_else(missing);
                let child_prefix = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };
                collect_diffs(&va, &vb, &child_prefix, acc);
            }
        }
        _ => {
            let mut entry = IndexMap::new();
            entry.insert("path".to_string(), str_atom(prefix));
            entry.insert("from".to_string(), a.clone());
            entry.insert("to".to_string(), b.clone());
            acc.push(Value::Combo(ComboVal::new(entry, false, IndexMap::new(), EffectTag::Pure, vec![])));
        }
    }
}

fn build_list(items: Vec<Value>) -> Value {
    let mut m = IndexMap::new();
    m.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    for (i, v) in items.iter().enumerate() { m.insert(i.to_string(), v.clone()); }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn extract_list_items(list: &Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Vec<Value> {
    if let Value::Combo(c) = list {
        let mut items = Vec::new();
        for i in 0u32.. {
            if let Some(v) = c.get_field(&i.to_string()) {
                items.push(oo.force(v.clone(), ctx));
            } else { break; }
        }
        items
    } else { vec![] }
}

fn has_any_bottom(val: &Value) -> bool {
    match val {
        Value::Bottom(_) => true,
        Value::Combo(c) => c.all_fields_iter().any(|(_, v)| has_any_bottom(&v)),
        _ => false,
    }
}

pub fn register_diff_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // diff.diff: {0: a, 1: b} → @list of {path, from, to}
    m.insert("diff.diff".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
        let a = match c.get_field("0") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        let b = match c.get_field("1") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        let mut entries = Vec::new();
        collect_diffs(&a, &b, "", &mut entries);
        build_list(entries)
    }) as Arc<BuiltinFn>);

    // diff.patch: {0: val, 1: diff_list} → patched Value
    m.insert("diff.patch".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
        let mut val = match c.get_field("0") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        let diff_list = match c.get_field("1") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        let entries = extract_list_items(&diff_list, oo, ctx);
        for entry in entries {
            let entry = oo.force(entry, ctx);
            if let Value::Combo(ref ec) = entry {
                let path_str = match ec.get_field("path") {
                    Some(p) => {
                        let raw = oo.force(p.clone(), ctx).to_string_plain();
                        crate::value::strip_plain_quotes(&raw).to_string()
                    }
                    None => continue,
                };
                let new_val = match ec.get_field("to") {
                    Some(v) => oo.force(v.clone(), ctx),
                    None => continue,
                };
                let segments = parse_path(&path_str);
                val = set_at_path(val, &segments, new_val);
            }
        }
        val
    }) as Arc<BuiltinFn>);

    // diff.is_compatible: {0: a, 1: b} → #true / #false
    m.insert("diff.is_compatible".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
        let a = match c.get_field("0") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        let b = match c.get_field("1") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        let merged = deep_merge_values(a, b, oo, ctx);
        let tag = if has_any_bottom(&merged) { "false" } else { "true" };
        Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);
}
