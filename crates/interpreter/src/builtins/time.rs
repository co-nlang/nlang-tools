use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;

pub fn register_time_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert("time.now".to_string(), Arc::new(|_arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| {
        Value::Atom(AtomKind::Tag("now".to_string()), EffectTag::IO, None)
    }) as Arc<BuiltinFn>);
}