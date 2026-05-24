use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

pub fn register_process_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // process.exit: {0: code_int} → !  (terminates the process)
    m.insert("process.exit".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        let code = match forced.collapse() {
            Value::Atom(AtomKind::Int(n), _, _) => n.to_i32().unwrap_or(0),
            _ => 0,
        };
        std::process::exit(code);
        #[allow(unreachable_code)]
        Value::Top
    }) as Arc<BuiltinFn>);

    // process.pid: _ → Int(IO)
    m.insert("process.pid".to_string(), Arc::new(|_arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| {
        Value::Atom(AtomKind::Int(BigInt::from(std::process::id())), EffectTag::IO, None)
    }) as Arc<BuiltinFn>);
}
