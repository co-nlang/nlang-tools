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
            Value::Atom(kind, _, _) => match kind {
                AtomKind::Int(_) => "int",
                AtomKind::Float(_) => "float",
                AtomKind::Str(_) | AtomKind::MultilineStr(_) => "str",
                AtomKind::Tag(_) | AtomKind::TagStart | AtomKind::TagEnd => "tag",
                AtomKind::Top => "top",
                AtomKind::Bottom => "bottom",
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
}