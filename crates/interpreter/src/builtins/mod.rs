mod bytes;
mod cond;
pub mod contract;
mod csv;
mod diff;
mod disc;
mod engine;
mod env;
/// Language → store trust boundary (SPEC_08 §6.3).
pub mod fs_guard;
mod io;
mod json;
mod list;
mod math;
mod path;
mod process;
mod query;
mod reflection;
mod regex;
mod set;
mod stat;
mod string;
mod time;
mod toml;
mod url;

use crate::value::Value;
use crate::{BuiltinFn, EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;
use std::collections::HashMap;
use std::sync::Arc;

pub use contract::{carries, contract_for, contract_ids, BuiltinContract};

/// O77: only the atom `#true` is true. A collapsed predicate is
/// distinguishable — the `Err` arm carries the `_|_`. The STRING `"true"`
/// is not the atom (same line of code that used to print-then-compare).
/// G6: an element or operand a builtin reads as a value. A hybrid gives its
/// `%val`; one not yet a value (D119, D121) gives `None`, and the builtin
/// answers `Value::Top` — which the dispatch reads as `_`, not a partial.
pub fn value_of(oo: &Ouroboros, v: Value, ctx: &mut EvalContext) -> Option<Value> {
    let v = oo.read_value_context(v, ctx);
    if v.is_top() || crate::eval::not_yet_a_value(&v) {
        ctx.builtin_not_yet = true;
        return None;
    }
    Some(v)
}

pub fn pred_is_true(oo: &Ouroboros, v: &Value, ctx: &mut EvalContext) -> Result<bool, Value> {
    // G6: a predicate's answer is read as a value — a hybrid `#true` is true,
    // and one not yet a value makes the builtin answer `_`.
    let v = oo.read_value_context(v.clone(), ctx);
    match &v {
        Value::Bottom(_) => Err(v),
        _ if v.is_top() || crate::eval::not_yet_a_value(&v) => {
            ctx.builtin_not_yet = true;
            Err(Value::Top)
        }
        Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "true" => Ok(true),
        _ => Ok(false),
    }
}

pub fn create_default_builtins() -> HashMap<String, Arc<BuiltinFn>> {
    let mut m = HashMap::new();

    math::register_math_builtins(&mut m);
    math::register_complex_builtins(&mut m);
    cond::register_cond_builtins(&mut m);
    string::register_string_builtins(&mut m);
    list::register_list_builtins(&mut m);
    disc::register_disc_builtins(&mut m);
    reflection::register_reflection_builtins(&mut m);
    engine::register_engine_builtins(&mut m);
    time::register_time_builtins(&mut m);
    bytes::register_bytes_builtins(&mut m);
    regex::register_regex_builtins(&mut m);
    json::register_json_builtins(&mut m);
    io::register_io_builtins(&mut m);
    env::register_env_builtins(&mut m);
    process::register_process_builtins(&mut m);
    path::register_path_builtins(&mut m);
    query::register_query_builtins(&mut m);
    diff::register_diff_builtins(&mut m);
    set::register_set_builtins(&mut m);
    stat::register_stat_builtins(&mut m);
    csv::register_csv_builtins(&mut m);
    url::register_url_builtins(&mut m);
    toml::register_toml_builtins(&mut m);

    m
}
