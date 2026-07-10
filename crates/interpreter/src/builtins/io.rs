use std::sync::Arc;
use std::collections::HashMap;
use std::io::Write;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;

pub fn register_io_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // io.read_file: {0: path_str} → Str | #none  (IO)
    m.insert("io.read_file".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(path), _, _) = oo.force(v, ctx).collapse() {
            return match std::fs::read_to_string(path.as_str()) {
                Ok(content) => Value::Atom(AtomKind::Str(content), EffectTag::IO, None),
                Err(_)      => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::IO, None),
            };
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // io.write_file: {0: path_str, 1: content_str} → #true | #none  (IO)
    m.insert("io.write_file".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vp), Some(vc)) = (c.get_field("0"), c.get_field("1")) {
                let fp = oo.force(vp.clone(), ctx);
                let fc = oo.force(vc.clone(), ctx);
                if let (Value::Atom(AtomKind::Str(path), _, _), Value::Atom(AtomKind::Str(content), _, _)) =
                    (fp.collapse(), fc.collapse())
                {
                    let tag = if std::fs::write(path.as_str(), content.as_bytes()).is_ok() { "true" } else { "none" };
                    return Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::IO, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // io.exists: {0: path_str} → #true | #false  (IO)
    m.insert("io.exists".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(path), _, _) = oo.force(v, ctx).collapse() {
            let tag = if std::path::Path::new(path.as_str()).exists() { "true" } else { "false" };
            return Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::IO, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // io.append_file: {0: path_str, 1: content_str} → #true | #none  (IO)
    m.insert("io.append_file".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vp), Some(vc)) = (c.get_field("0"), c.get_field("1")) {
                let fp = oo.force(vp.clone(), ctx);
                let fc = oo.force(vc.clone(), ctx);
                if let (Value::Atom(AtomKind::Str(path), _, _), Value::Atom(AtomKind::Str(content), _, _)) =
                    (fp.collapse(), fc.collapse())
                {
                    let result = std::fs::OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open(path.as_str())
                        .and_then(|mut f| f.write_all(content.as_bytes()));
                    let tag = if result.is_ok() { "true" } else { "none" };
                    return Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::IO, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
}
