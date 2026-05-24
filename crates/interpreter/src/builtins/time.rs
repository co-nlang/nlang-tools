use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use chrono::{DateTime, Utc};

pub fn register_time_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // time.now → current Unix timestamp in milliseconds (Int, IO)
    m.insert("time.now".to_string(), Arc::new(|_arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| {
        let ms = Utc::now().timestamp_millis();
        Value::Atom(AtomKind::Int(BigInt::from(ms)), EffectTag::IO, None)
    }) as Arc<BuiltinFn>);

    // time.format: {0: fmt_str, 1: ms} → Str
    m.insert("time.format".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vfmt), Some(vms)) = (c.get_field("0"), c.get_field("1")) {
                let fmt_forced = oo.force(vfmt.clone(), ctx);
                let ms_forced  = oo.force(vms.clone(), ctx);

                let fmt_str = match fmt_forced.collapse() {
                    Value::Atom(AtomKind::Str(s), _, _) => {
                        if s.is_empty() { "%Y-%m-%dT%H:%M:%S%.3fZ".to_string() }
                        else { s.clone() }
                    }
                    _ => return Value::Top,
                };

                let ms_i64: i64 = match ms_forced.collapse() {
                    Value::Atom(AtomKind::Int(n), _, _)   => n.to_i64().unwrap_or(0),
                    Value::Atom(AtomKind::Float(f), _, _) => *f as i64,
                    _ => return Value::Top,
                };

                if let Some(dt) = DateTime::from_timestamp_millis(ms_i64) {
                    let formatted = dt.format(&fmt_str).to_string();
                    return Value::Atom(AtomKind::Str(formatted), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // time.diff: {0: t1_ms, 1: t2_ms} → Int  (t1 - t2)
    m.insert("time.diff".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vt1), Some(vt2)) = (c.get_field("0"), c.get_field("1")) {
                let t1 = oo.force(vt1.clone(), ctx);
                let t2 = oo.force(vt2.clone(), ctx);
                let to_i64 = |v: &Value| -> Option<i64> {
                    match v.collapse() {
                        Value::Atom(AtomKind::Int(n), _, _)   => n.to_i64(),
                        Value::Atom(AtomKind::Float(f), _, _) => Some(*f as i64),
                        _ => None,
                    }
                };
                if let (Some(i1), Some(i2)) = (to_i64(&t1), to_i64(&t2)) {
                    return Value::Atom(AtomKind::Int(BigInt::from(i1 - i2)), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // time.add_ms: {0: offset_ms, 1: timestamp_ms} → Int
    m.insert("time.add_ms".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(voffset), Some(vts)) = (c.get_field("0"), c.get_field("1")) {
                let offset = oo.force(voffset.clone(), ctx);
                let ts     = oo.force(vts.clone(), ctx);
                let to_i64 = |v: &Value| -> Option<i64> {
                    match v.collapse() {
                        Value::Atom(AtomKind::Int(n), _, _)   => n.to_i64(),
                        Value::Atom(AtomKind::Float(f), _, _) => Some(*f as i64),
                        _ => None,
                    }
                };
                if let (Some(off), Some(t)) = (to_i64(&offset), to_i64(&ts)) {
                    return Value::Atom(AtomKind::Int(BigInt::from(t + off)), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
}
