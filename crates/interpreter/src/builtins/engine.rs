use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, BottomCause};
use nlang_parser::ast::{AtomKind, Path, PathAnchor, Span};

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
}