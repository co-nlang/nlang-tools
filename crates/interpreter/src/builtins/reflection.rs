use std::sync::Arc;
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, ComboVal, EffectTag};
use nlang_parser::ast::AtomKind;

pub fn register_reflection_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert("refl.keys".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Combo(c) = oo.force(v, ctx).collapse() {
            let mut res = IndexMap::new();
            let mut count = 0;
            let mut keys: Vec<_> = c.fields().keys().filter(|k| !k.starts_with('%')).cloned().collect();
            keys.sort();
            for k in keys {
                res.insert(count.to_string(), Value::Atom(AtomKind::Str(k), EffectTag::Pure, None));
                count += 1;
            }
            res.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
            return Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]));
        }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("refl.has".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vkey), Some(vobj)) = (c.get_field("0"), c.get_field("1")) {
                let key = oo.force(vkey.clone(), ctx).to_string_plain();
                if let Value::Combo(oc) = oo.force(vobj.clone(), ctx).collapse() {
                    return Value::Atom(AtomKind::Tag(if oc.fields().contains_key(&key) { "true".to_string() } else { "false".to_string() }), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("refl.is_cocoon".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Combo(c) = oo.force(v, ctx).collapse() {
            return Value::Atom(AtomKind::Tag(if c.closed { "true".to_string() } else { "false".to_string() }), EffectTag::Pure, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("refl.type_of".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let fv = oo.force(v, ctx);
        let tag = match fv.collapse() {
            Value::Top => "top",
            Value::Bottom(_) => "bottom",
            Value::Blur(_) => "blur",
            Value::Atom(kind, _, _) => match kind {
                AtomKind::Int(_) => "int",
                AtomKind::Float(_) => "float",
                AtomKind::Str(_) | AtomKind::MultilineStr(_) => "str",
                AtomKind::Tag(_) | AtomKind::TagStart | AtomKind::TagEnd => "tag",
                AtomKind::Top => "top",
                AtomKind::Bottom => "bottom",
                AtomKind::Bytes(_) => "bytes",
                _ => "atom",
            },
            Value::Combo(c) => if c.contains_key("%morphism") || c.contains_key("%rules") || c.contains_key("%builtin") { "logic" } 
                               else if c.get_field("%kind").map(|k| k.to_string_plain() == "#list").unwrap_or(false) { "list" }
                               else { "combo" },
            Value::Union(_) => "union",
            _ => "unknown",
        };
        Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);

    // ── Phase 16: predicates + to_str + bottom_cause ─────────────

    m.insert("refl.is_blur".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let fv = oo.force(v, ctx);
        let is = matches!(fv.collapse(), Value::Blur(_));
        Value::Atom(AtomKind::Tag(if is { "true" } else { "false" }.to_string()), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);

    m.insert("refl.is_bottom".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let fv = oo.force(v, ctx);
        let is = matches!(fv.collapse(), Value::Bottom(_));
        Value::Atom(AtomKind::Tag(if is { "true" } else { "false" }.to_string()), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);

    m.insert("refl.is_some".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let fv = oo.force(v, ctx);
        let is = match &fv {
            Value::Combo(ref cv) => cv.get_field("%val").is_some(),
            _ => false,
        };
        Value::Atom(AtomKind::Tag(if is { "true" } else { "false" }.to_string()), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);

    m.insert("refl.is_none".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let fv = oo.force(v, ctx);
        let is = matches!(&fv, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none");
        Value::Atom(AtomKind::Tag(if is { "true" } else { "false" }.to_string()), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);

    m.insert("refl.is_ok".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let fv = oo.force(v, ctx);
        let is = match &fv {
            Value::Combo(ref cv) => cv.get_field("%val").is_some() && cv.get_field("%cause").is_none(),
            _ => false,
        };
        Value::Atom(AtomKind::Tag(if is { "true" } else { "false" }.to_string()), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);

    m.insert("refl.is_err".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let fv = oo.force(v, ctx);
        let is = match &fv {
            Value::Combo(ref cv) => cv.get_field("%cause").is_some(),
            _ => false,
        };
        Value::Atom(AtomKind::Tag(if is { "true" } else { "false" }.to_string()), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);

    m.insert("refl.to_str".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let fv = oo.force(v, ctx);
        Value::Atom(AtomKind::Str(fv.collapse().to_string_plain()), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);

    m.insert("refl.bottom_cause".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let fv = oo.force(v, ctx);
        if let Value::Bottom(ref bd) = fv.collapse() {
            Value::Atom(AtomKind::Tag(bd.cause.as_tag().to_string()), EffectTag::Pure, None)
        } else {
            Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None)
        }
    }) as Arc<BuiltinFn>);

    // ── Phase 20: Dynamic Combo access ───────────────────────────

    m.insert("refl.get".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vkey), Some(vobj)) = (c.get_field("0"), c.get_field("1")) {
                let key = oo.force(vkey.clone(), ctx).to_string_plain();
                let obj = oo.force(vobj.clone(), ctx);
                if let Value::Combo(ref oc) = obj.collapse() {
                    return oc.get_field(&key).cloned().unwrap_or(Value::Top);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    m.insert("refl.set".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vkey), Some(vval), Some(vobj)) = (c.get_field("0"), c.get_field("1"), c.get_field("2")) {
                let key = oo.force(vkey.clone(), ctx).to_string_plain();
                let val = oo.force(vval.clone(), ctx);
                let obj = oo.force(vobj.clone(), ctx);
                if let Value::Combo(ref oc) = obj.collapse() {
                    let mut new_combo = oc.clone();
                    new_combo.insert_field(&key, val);
                    return Value::Combo(new_combo);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    m.insert("refl.delete".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vkey), Some(vobj)) = (c.get_field("0"), c.get_field("1")) {
                let key = oo.force(vkey.clone(), ctx).to_string_plain();
                let obj = oo.force(vobj.clone(), ctx);
                if let Value::Combo(ref oc) = obj.collapse() {
                    let mut new_combo = oc.clone();
                    new_combo.remove_field(&key);
                    return Value::Combo(new_combo);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    m.insert("refl.values".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg {
            c.get_field("0").cloned().unwrap_or_else(|| arg.clone())
        } else { arg.clone() };
        if let Value::Combo(c) = oo.force(v, ctx).collapse() {
            let mut pairs: Vec<(String, Value)> = c.fields().into_iter()
                .filter(|(k, _)| !k.starts_with('%'))
                .collect();
            pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
            let mut res = IndexMap::new();
            for (i, (_, val)) in pairs.into_iter().enumerate() {
                res.insert(i.to_string(), val);
            }
            res.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
            return Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]));
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    m.insert("refl.entries".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg {
            c.get_field("0").cloned().unwrap_or_else(|| arg.clone())
        } else { arg.clone() };
        if let Value::Combo(c) = oo.force(v, ctx).collapse() {
            let mut pairs: Vec<(String, Value)> = c.fields().into_iter()
                .filter(|(k, _)| !k.starts_with('%'))
                .collect();
            pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
            let mut res = IndexMap::new();
            for (i, (key, val)) in pairs.into_iter().enumerate() {
                let mut entry = IndexMap::new();
                entry.insert("key".to_string(), Value::Atom(AtomKind::Str(key), EffectTag::Pure, None));
                entry.insert("val".to_string(), val);
                let entry_combo = ComboVal::new(entry, true, IndexMap::new(), EffectTag::Pure, vec![]);
                res.insert(i.to_string(), Value::Combo(entry_combo));
            }
            res.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
            return Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]));
        }
        Value::Top
    }) as Arc<BuiltinFn>);
}