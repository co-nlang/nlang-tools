use std::sync::Arc;
use std::collections::HashMap;
use std::path::Path;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;

pub fn register_path_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // path.join: {0: base_str, 1: seg_str} → Str
    m.insert("path.join".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(v0), Some(v1)) = (c.get_field("0"), c.get_field("1")) {
                let f0 = oo.force(v0.clone(), ctx);
                let f1 = oo.force(v1.clone(), ctx);
                if let (Value::Atom(AtomKind::Str(base), _, _), Value::Atom(AtomKind::Str(seg), _, _)) =
                    (f0.collapse(), f1.collapse())
                {
                    let joined = Path::new(base.as_str()).join(seg.as_str());
                    return Value::Atom(AtomKind::Str(joined.to_string_lossy().into_owned()), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // path.dirname: {0: path_str} → Str | #none
    m.insert("path.dirname".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        if let Value::Atom(AtomKind::Str(path), _, _) = forced.collapse() {
            return match Path::new(path.as_str()).parent() {
                Some(p) => Value::Atom(AtomKind::Str(p.to_string_lossy().into_owned()), EffectTag::Pure, None),
                None    => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
            };
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // path.basename: {0: path_str} → Str | #none
    m.insert("path.basename".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        if let Value::Atom(AtomKind::Str(path), _, _) = forced.collapse() {
            return match Path::new(path.as_str()).file_name() {
                Some(name) => Value::Atom(AtomKind::Str(name.to_string_lossy().into_owned()), EffectTag::Pure, None),
                None       => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
            };
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // path.extension: {0: path_str} → Str | #none
    m.insert("path.extension".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        if let Value::Atom(AtomKind::Str(path), _, _) = forced.collapse() {
            return match Path::new(path.as_str()).extension() {
                Some(ext) => Value::Atom(AtomKind::Str(ext.to_string_lossy().into_owned()), EffectTag::Pure, None),
                None      => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
            };
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // path.is_absolute: {0: path_str} → #true | #false
    m.insert("path.is_absolute".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        if let Value::Atom(AtomKind::Str(path), _, _) = forced.collapse() {
            let tag = if Path::new(path.as_str()).is_absolute() { "true" } else { "false" };
            return Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::Pure, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);
}
