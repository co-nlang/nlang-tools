use std::sync::Arc;
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, BottomCause, BottomDetail, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

fn extract_floats(list: &Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Option<Vec<f64>> {
    if let Value::Combo(c) = list {
        let mut out = Vec::new();
        for i in 0u32.. {
            match c.get_field(&i.to_string()) {
                Some(v) => {
                    let v = oo.force(v.clone(), ctx);
                    let f = match v {
                        Value::Atom(AtomKind::Float(f), _, _) => f,
                        Value::Atom(AtomKind::Int(ref n), _, _) => n.to_f64()?,
                        _ => return None,
                    };
                    out.push(f);
                }
                None => break,
            }
        }
        Some(out)
    } else { None }
}

fn float_val(f: f64) -> Value {
    Value::Atom(AtomKind::Float(f), EffectTag::Pure, None)
}

fn build_list_vals(items: Vec<Value>) -> Value {
    let mut m = IndexMap::new();
    m.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    for (i, v) in items.iter().enumerate() { m.insert(i.to_string(), v.clone()); }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn conflict() -> Value { BottomCause::Conflict.into() }

pub fn register_stat_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert("stat.mean".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = oo.force(arg, ctx);
        let nums = match extract_floats(&v, oo, ctx) { Some(n) => n, None => return conflict() };
        if nums.is_empty() { return conflict(); }
        float_val(nums.iter().sum::<f64>() / nums.len() as f64)
    }) as Arc<BuiltinFn>);

    m.insert("stat.variance".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = oo.force(arg, ctx);
        let nums = match extract_floats(&v, oo, ctx) { Some(n) => n, None => return conflict() };
        if nums.is_empty() { return conflict(); }
        let mean = nums.iter().sum::<f64>() / nums.len() as f64;
        let var = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / nums.len() as f64;
        float_val(var)
    }) as Arc<BuiltinFn>);

    m.insert("stat.std_dev".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = oo.force(arg, ctx);
        let nums = match extract_floats(&v, oo, ctx) { Some(n) => n, None => return conflict() };
        if nums.is_empty() { return conflict(); }
        let mean = nums.iter().sum::<f64>() / nums.len() as f64;
        let var = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / nums.len() as f64;
        float_val(var.sqrt())
    }) as Arc<BuiltinFn>);

    m.insert("stat.median".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = oo.force(arg, ctx);
        let mut nums = match extract_floats(&v, oo, ctx) { Some(n) => n, None => return conflict() };
        if nums.is_empty() { return conflict(); }
        nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = nums.len();
        let median = if n % 2 == 1 { nums[n/2] }
                     else { (nums[n/2 - 1] + nums[n/2]) / 2.0 };
        float_val(median)
    }) as Arc<BuiltinFn>);

    m.insert("stat.percentile".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return conflict() };
        let list_v = oo.force(c.get_field("0").cloned().unwrap_or(Value::Top), ctx);
        let p_v    = oo.force(c.get_field("1").cloned().unwrap_or(Value::Top), ctx);
        let p = match p_v {
            Value::Atom(AtomKind::Float(f), _, _) => f,
            Value::Atom(AtomKind::Int(ref n), _, _) => n.to_f64().unwrap_or(0.0),
            _ => return conflict(),
        };
        let mut nums = match extract_floats(&list_v, oo, ctx) { Some(n) => n, None => return conflict() };
        if nums.is_empty() { return conflict(); }
        nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = nums.len();
        let rank = p / 100.0 * (n - 1) as f64;
        let lo = rank.floor() as usize;
        let hi = rank.ceil() as usize;
        let frac = rank - lo as f64;
        float_val(nums[lo] * (1.0 - frac) + nums[hi.min(n-1)] * frac)
    }) as Arc<BuiltinFn>);

    m.insert("stat.histogram".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return conflict() };
        let list_v = oo.force(c.get_field("0").cloned().unwrap_or(Value::Top), ctx);
        let bins_v = oo.force(c.get_field("1").cloned().unwrap_or(Value::Top), ctx);
        let bins = match bins_v {
            Value::Atom(AtomKind::Int(ref n), _, _) => n.to_usize().unwrap_or(1).max(1),
            _ => return conflict(),
        };
        let nums = match extract_floats(&list_v, oo, ctx) { Some(n) => n, None => return conflict() };
        if nums.is_empty() {
            let empty_bins: Vec<Value> = (0..bins).map(|_| build_list_vals(vec![])).collect();
            return build_list_vals(empty_bins);
        }
        let min = nums.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let width = if (max - min).abs() < 1e-15 { 1.0 } else { (max - min) / bins as f64 };
        let mut buckets: Vec<Vec<Value>> = vec![Vec::new(); bins];
        for &x in &nums {
            let idx = ((x - min) / width).floor() as usize;
            let idx = idx.min(bins - 1);
            buckets[idx].push(float_val(x));
        }
        build_list_vals(buckets.into_iter().map(build_list_vals).collect())
    }) as Arc<BuiltinFn>);
}
