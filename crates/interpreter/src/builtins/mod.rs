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
/// Q-112: a union read here is a choice point — this run takes one branch
/// and the dispatch runs the builtin again for the others (`run_builtin`);
/// a horizon is the call's answer (G3).
pub fn value_of(oo: &Ouroboros, v: Value, ctx: &mut EvalContext) -> Option<Value> {
    let v = oo.read_value_context(v, ctx);
    if let Value::Union(bs) = &v {
        let b = bs[choose(ctx, bs.len())].clone();
        return value_of(oo, b, ctx);
    }
    if let Value::Blur(_) = &v {
        ctx.builtin_horizon.get_or_insert(v);
        return None;
    }
    if v.is_top() || crate::eval::not_yet_a_value(&v) {
        ctx.builtin_not_yet = true;
        return None;
    }
    Some(v)
}

/// Q-112: an element a builtin shows (`str.format`), read as interpolation
/// reads it — `_` shows as `_` — with a union a choice point and a horizon
/// the call's answer (`None`).
pub fn shown_of(oo: &Ouroboros, v: Value, ctx: &mut EvalContext) -> Option<Value> {
    match oo.read_value_context(v, ctx) {
        Value::Union(bs) => {
            let b = bs[choose(ctx, bs.len())].clone();
            shown_of(oo, b, ctx)
        }
        v @ Value::Blur(_) => {
            ctx.builtin_horizon.get_or_insert(v);
            None
        }
        v => Some(v),
    }
}

/// Q-112: an element a builtin reads and also keeps (`list.sort`). A union
/// is a choice point, and what is kept is the branch this run read — not
/// the union, whose other branches would sit where this one was ordered.
pub fn branch_of(oo: &Ouroboros, v: Value, ctx: &mut EvalContext) -> Value {
    match oo.force(v, ctx) {
        Value::Union(bs) => {
            let b = bs[choose(ctx, bs.len())].clone();
            branch_of(oo, b, ctx)
        }
        v => v,
    }
}

/// The branch this run takes at the next choice point of `n` branches.
fn choose(ctx: &mut EvalContext, n: usize) -> usize {
    let taken = match ctx.builtin_choices.get(ctx.builtin_cursor) {
        Some(&(t, _)) => t,
        None => {
            ctx.builtin_choices.push((0, n));
            0
        }
    };
    ctx.builtin_cursor += 1;
    taken
}

/// D122: a builtin whose work a number sets bills that work to the fuel
/// before doing it — 1 + 0.1 MBU per unit it will make (an element, a
/// character, a digit), as a combo of that many fields is billed
/// (SPEC_08 §3.1.1). Too little fuel answers the horizon, not a crash.
pub fn bill_output(ctx: &mut EvalContext, units: u128) -> Result<(), Value> {
    let cost = u64::try_from(1 + units.div_ceil(10)).unwrap_or(u64::MAX);
    ctx.check_resources(cost).map_err(|e| {
        crate::observation::handle_resource_exhausted(e, ctx.strategy, ctx, None, crate::value::EffectTag::Pure)
    })
}

/// A count as units to bill: negative is none, too big for `u128` is all.
pub fn units_of(n: &num_bigint::BigInt) -> u128 {
    use num_traits::ToPrimitive;
    if n.sign() == num_bigint::Sign::Minus { 0 } else { n.to_u128().unwrap_or(u128::MAX) }
}

pub fn pred_is_true(oo: &Ouroboros, v: &Value, ctx: &mut EvalContext) -> Result<bool, Value> {
    // G6: a predicate's answer is read as a value — a hybrid `#true` is true,
    // and one not yet a value makes the builtin answer `_`.
    let v = oo.read_value_context(v.clone(), ctx);
    match &v {
        Value::Union(bs) => {
            let b = bs[choose(ctx, bs.len())].clone();
            pred_is_true(oo, &b, ctx)
        }
        Value::Blur(_) => {
            ctx.builtin_horizon.get_or_insert(v.clone());
            Err(Value::Top)
        }
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
