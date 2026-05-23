use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, BottomCause};
use nlang_parser::ast::{AtomKind, Path, PathAnchor, Span};
use num_traits::ToPrimitive;

pub fn register_engine_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert("engine.observe".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Atom(AtomKind::Str(path_str), _, _) = arg.collapse() {
            let path = Path { anchor: PathAnchor::Bare, segments: path_str.split('.').map(|s| s.trim().to_string()).collect(), span: Span::default() };
            return oo.resolve_path(&path, ctx);
        }
        BottomCause::Conflict.into()
    }) as Arc<BuiltinFn>);
    
    m.insert("engine.save".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let fv = oo.force_recursive(v, ctx);
        if let Ok(hash) = oo.store.put_value(&fv) {
            return Value::Atom(AtomKind::Str(hash.to_string()), EffectTag::IO, None);
        }
        BottomCause::Conflict.into()
    }) as Arc<BuiltinFn>);

    // Phase NEW: /%differential.{1,2,3}
    m.insert("engine.differential".to_string(), Arc::new(|arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| {
        match &arg {
            Value::Atom(AtomKind::Int(n), _, _) => {
                let tag = match n.to_u8().unwrap_or(0) { 1 => "d1_converging", 2 => "d2_branching", 3 => "d3_horizon", _ => "unknown" };
                Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::Pure, None)
            }
            Value::Combo(ref c) => {
                if let Some(Value::Atom(AtomKind::Int(d), _, _)) = c.get_field("%degree") {
                    let tag = match d.to_u8().unwrap_or(1) { 1 => "d1_converging", 2 => "d2_branching", _ => "d3_horizon" };
                    Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::Pure, None)
                } else {
                    Value::Atom(AtomKind::Tag("d1_converging".to_string()), EffectTag::Pure, None)
                }
            }
            _ => Value::Atom(AtomKind::Tag("d1_converging".to_string()), EffectTag::Pure, None),
        }
    }) as Arc<BuiltinFn>);
}