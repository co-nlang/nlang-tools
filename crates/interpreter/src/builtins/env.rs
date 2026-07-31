use crate::value::{ComboVal, EffectTag, Value};
use crate::{BuiltinFn, EvalContext, Ouroboros};
use indexmap::IndexMap;
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use std::collections::HashMap;
use std::sync::Arc;

fn build_str_list(items: Vec<String>) -> Value {
    let mut data = IndexMap::new();
    data.insert(
        "%kind".to_string(),
        Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::IO, None),
    );
    for (i, s) in items.into_iter().enumerate() {
        data.insert(
            i.to_string(),
            Value::Atom(AtomKind::Str(s), EffectTag::IO, None),
        );
    }
    Value::Combo(ComboVal::new(
        data,
        false,
        IndexMap::new(),
        EffectTag::IO,
        vec![],
    ))
}

pub fn register_env_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    // env.get: {0: name_str} → Str(IO) | #none(IO)
    m.insert(
        "env.get".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            let forced = oo.force(v, ctx);
            if let Value::Atom(AtomKind::Str(name), _, _) = forced.collapse() {
                return match std::env::var(name.as_str()) {
                    Ok(val) => Value::Atom(AtomKind::Str(val), EffectTag::IO, None),
                    Err(_) => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::IO, None),
                };
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // env.args: _ → list of Str(IO)  (includes argv[0])
    m.insert(
        "env.args".to_string(),
        Arc::new(|_arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| {
            let args: Vec<String> = std::env::args().collect();
            build_str_list(args)
        }) as Arc<BuiltinFn>,
    );

    // env.cwd: _ → Str(IO) | #none(IO)
    m.insert(
        "env.cwd".to_string(),
        Arc::new(
            |_arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| match std::env::current_dir() {
                Ok(path) => Value::Atom(
                    AtomKind::Str(path.to_string_lossy().into_owned()),
                    EffectTag::IO,
                    None,
                ),
                Err(_) => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::IO, None),
            },
        ) as Arc<BuiltinFn>,
    );
}
