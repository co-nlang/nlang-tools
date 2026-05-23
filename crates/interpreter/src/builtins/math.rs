use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, BottomCause};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::{Signed, Zero, ToPrimitive};

pub fn register_math_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert("math.add".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg { 
            if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
                let fx = oo.force(vx.clone(), ctx); let fy = oo.force(vy.clone(), ctx);
                let x = fx.collapse(); let y = fy.collapse();
                return match (x, y) {
                    (Value::Atom(AtomKind::Int(ix), e1, _), Value::Atom(AtomKind::Int(iy), e2, _)) => Value::Atom(AtomKind::Int(ix + iy), (*e1).max(*e2), None),
                    (Value::Atom(AtomKind::Float(fx), e1, _), Value::Atom(AtomKind::Float(fy), e2, _)) => Value::Atom(AtomKind::Float(fx + fy), (*e1).max(*e2), None),
                    (Value::Atom(AtomKind::Int(ix), e1, _), Value::Atom(AtomKind::Float(fy), e2, _)) => Value::Atom(AtomKind::Float(ix.to_f64().unwrap_or(0.0) + fy), (*e1).max(*e2), None),
                    (Value::Atom(AtomKind::Float(fx), e1, _), Value::Atom(AtomKind::Int(iy), e2, _)) => Value::Atom(AtomKind::Float(fx + iy.to_f64().unwrap_or(0.0)), (*e1).max(*e2), None),
                    (Value::Atom(AtomKind::Complex(r1, i1), e1, _), Value::Atom(AtomKind::Complex(r2, i2), e2, _)) => Value::Atom(AtomKind::Complex(r1 + r2, i1 + i2), (*e1).max(*e2), None),
                    (Value::Atom(AtomKind::Complex(r, i), e1, _), Value::Atom(AtomKind::Int(y), e2, _)) => Value::Atom(AtomKind::Complex(r + y.to_f64().unwrap_or(0.0), *i), (*e1).max(*e2), None),
                    (Value::Atom(AtomKind::Complex(r, i), e1, _), Value::Atom(AtomKind::Float(y), e2, _)) => Value::Atom(AtomKind::Complex(r + y, *i), (*e1).max(*e2), None),
                    (Value::Atom(AtomKind::Int(x), e1, _), Value::Atom(AtomKind::Complex(r, i), e2, _)) => Value::Atom(AtomKind::Complex(x.to_f64().unwrap_or(0.0) + r, *i), (*e1).max(*e2), None),
                    (Value::Atom(AtomKind::Float(x), e1, _), Value::Atom(AtomKind::Complex(r, i), e2, _)) => Value::Atom(AtomKind::Complex(x + r, *i), (*e1).max(*e2), None),
                    (Value::Atom(AtomKind::Str(sx), e1, _), Value::Atom(AtomKind::Str(sy), e2, _)) => Value::Atom(AtomKind::Str(format!("{}{}", sx, sy)), (*e1).max(*e2), None),
                    _ => BottomCause::Conflict.into()
                };
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    m.insert("math.sub".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg { if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
            let x = oo.force(vx.clone(), ctx).collapse().clone(); let y = oo.force(vy.clone(), ctx).collapse().clone();
            let res_e = x.effect().max(y.effect());
            return match (x, y) {
                (Value::Atom(AtomKind::Int(ix), _, _), Value::Atom(AtomKind::Int(iy), _, _)) => Value::Atom(AtomKind::Int(ix - iy), res_e, None),
                (Value::Atom(AtomKind::Float(fx), _, _), Value::Atom(AtomKind::Float(fy), _, _)) => Value::Atom(AtomKind::Float(fx - fy), res_e, None),
                (Value::Atom(AtomKind::Int(ix), _, _), Value::Atom(AtomKind::Float(fy), _, _)) => Value::Atom(AtomKind::Float(ix.to_f64().unwrap_or(0.0) - fy), res_e, None),
                (Value::Atom(AtomKind::Float(fx), _, _), Value::Atom(AtomKind::Int(iy), _, _)) => Value::Atom(AtomKind::Float(fx - iy.to_f64().unwrap_or(0.0)), res_e, None),
                (Value::Atom(AtomKind::Complex(r1, i1), _, _), Value::Atom(AtomKind::Complex(r2, i2), _, _)) => Value::Atom(AtomKind::Complex(r1 - r2, i1 - i2), res_e, None),
                (Value::Atom(AtomKind::Complex(r, i), _, _), Value::Atom(AtomKind::Int(y), _, _)) => Value::Atom(AtomKind::Complex(r - y.to_f64().unwrap_or(0.0), i), res_e, None),
                (Value::Atom(AtomKind::Complex(r, i), _, _), Value::Atom(AtomKind::Float(y), _, _)) => Value::Atom(AtomKind::Complex(r - y, i), res_e, None),
                (Value::Atom(AtomKind::Int(x), _, _), Value::Atom(AtomKind::Complex(r, i), _, _)) => Value::Atom(AtomKind::Complex(x.to_f64().unwrap_or(0.0) - r, -i), res_e, None),
                (Value::Atom(AtomKind::Float(x), _, _), Value::Atom(AtomKind::Complex(r, i), _, _)) => Value::Atom(AtomKind::Complex(x - r, -i), res_e, None),
                _ => BottomCause::Conflict.into()
            };
        } }
        Value::Top
    }) as Arc<BuiltinFn>);

    m.insert("math.mul".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg { if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
            let x = oo.force(vx.clone(), ctx).collapse().clone(); let y = oo.force(vy.clone(), ctx).collapse().clone();
            let res_e = x.effect().max(y.effect());
            return match (x, y) {
                (Value::Atom(AtomKind::Int(ix), _, _), Value::Atom(AtomKind::Int(iy), _, _)) => Value::Atom(AtomKind::Int(ix * iy), res_e, None),
                (Value::Atom(AtomKind::Float(fx), _, _), Value::Atom(AtomKind::Float(fy), _, _)) => Value::Atom(AtomKind::Float(fx * fy), res_e, None),
                (Value::Atom(AtomKind::Int(ix), _, _), Value::Atom(AtomKind::Float(fy), _, _)) => Value::Atom(AtomKind::Float(ix.to_f64().unwrap_or(0.0) * fy), res_e, None),
                (Value::Atom(AtomKind::Float(fx), _, _), Value::Atom(AtomKind::Int(iy), _, _)) => Value::Atom(AtomKind::Float(fx * iy.to_f64().unwrap_or(0.0)), res_e, None),
                (Value::Atom(AtomKind::Complex(r1, i1), _, _), Value::Atom(AtomKind::Complex(r2, i2), _, _)) => Value::Atom(AtomKind::Complex(r1 * r2 - i1 * i2, r1 * i2 + i1 * r2), res_e, None),
                (Value::Atom(AtomKind::Complex(r, i), _, _), Value::Atom(AtomKind::Int(y), _, _)) => Value::Atom(AtomKind::Complex(r * y.to_f64().unwrap_or(0.0), i * y.to_f64().unwrap_or(0.0)), res_e, None),
                (Value::Atom(AtomKind::Complex(r, i), _, _), Value::Atom(AtomKind::Float(y), _, _)) => Value::Atom(AtomKind::Complex(r * y, i * y), res_e, None),
                (Value::Atom(AtomKind::Int(x), _, _), Value::Atom(AtomKind::Complex(r, i), _, _)) => Value::Atom(AtomKind::Complex(x.to_f64().unwrap_or(0.0) * r, x.to_f64().unwrap_or(0.0) * i), res_e, None),
                (Value::Atom(AtomKind::Float(x), _, _), Value::Atom(AtomKind::Complex(r, i), _, _)) => Value::Atom(AtomKind::Complex(x * r, x * i), res_e, None),
                _ => BottomCause::Conflict.into()
            };
        } }
        Value::Top
    }) as Arc<BuiltinFn>);

    m.insert("math.div".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg { if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
            let x = oo.force(vx.clone(), ctx).collapse().clone(); let y = oo.force(vy.clone(), ctx).collapse().clone();
            let res_e = x.effect().max(y.effect());
            return match (x, y) {
                (Value::Atom(AtomKind::Int(ix), _, _), Value::Atom(AtomKind::Int(iy), _, _)) => if iy.is_zero() { BottomCause::Conflict.into() } else { Value::Atom(AtomKind::Int(ix / iy), res_e, None) },
                (Value::Atom(AtomKind::Float(fx), _, _), Value::Atom(AtomKind::Float(fy), _, _)) => Value::Atom(AtomKind::Float(fx / fy), res_e, None),
                (Value::Atom(AtomKind::Int(ix), _, _), Value::Atom(AtomKind::Float(fy), _, _)) => Value::Atom(AtomKind::Float(ix.to_f64().unwrap_or(0.0) / fy), res_e, None),
                (Value::Atom(AtomKind::Float(fx), _, _), Value::Atom(AtomKind::Int(iy), _, _)) => Value::Atom(AtomKind::Float(fx / iy.to_f64().unwrap_or(0.0)), res_e, None),
                (Value::Atom(AtomKind::Complex(r1, i1), _, _), Value::Atom(AtomKind::Complex(r2, i2), _, _)) => {
                    let denom = r2 * r2 + i2 * i2;
                    if denom == 0.0 { return BottomCause::Conflict.into(); }
                    Value::Atom(AtomKind::Complex((r1 * r2 + i1 * i2) / denom, (i1 * r2 - r1 * i2) / denom), res_e, None)
                },
                (Value::Atom(AtomKind::Complex(r, i), _, _), Value::Atom(AtomKind::Int(y), _, _)) => {
                    let d = y.to_f64().unwrap_or(0.0);
                    if d == 0.0 { return BottomCause::Conflict.into(); }
                    Value::Atom(AtomKind::Complex(r / d, i / d), res_e, None)
                },
                (Value::Atom(AtomKind::Complex(r, i), _, _), Value::Atom(AtomKind::Float(y), _, _)) => {
                    if y == 0.0 { return BottomCause::Conflict.into(); }
                    Value::Atom(AtomKind::Complex(r / y, i / y), res_e, None)
                },
                (Value::Atom(AtomKind::Int(x), _, _), Value::Atom(AtomKind::Complex(r, i), _, _)) => {
                    let denom = r * r + i * i;
                    if denom == 0.0 { return BottomCause::Conflict.into(); }
                    let xv = x.to_f64().unwrap_or(0.0);
                    Value::Atom(AtomKind::Complex(xv * r / denom, -xv * i / denom), res_e, None)
                },
                (Value::Atom(AtomKind::Float(x), _, _), Value::Atom(AtomKind::Complex(r, i), _, _)) => {
                    let denom = r * r + i * i;
                    if denom == 0.0 { return BottomCause::Conflict.into(); }
                    Value::Atom(AtomKind::Complex(x * r / denom, -x * i / denom), res_e, None)
                },
                _ => BottomCause::Conflict.into()
            };
        } }
        Value::Top
    }) as Arc<BuiltinFn>);

    m.insert("math.rem".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg { if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
            let x = oo.force(vx.clone(), ctx).collapse().clone(); let y = oo.force(vy.clone(), ctx).collapse().clone();
            let res_e = x.effect().max(y.effect());
            return match (x, y) {
                (Value::Atom(AtomKind::Int(ix), _, _), Value::Atom(AtomKind::Int(iy), _, _)) => if iy.is_zero() { BottomCause::Conflict.into() } else { Value::Atom(AtomKind::Int(ix % iy), res_e, None) },
                (Value::Atom(AtomKind::Float(fx), _, _), Value::Atom(AtomKind::Float(fy), _, _)) => Value::Atom(AtomKind::Float(fx % fy), res_e, None),
                _ => BottomCause::Conflict.into()
            };
        } }
        Value::Top
    }) as Arc<BuiltinFn>);

    m.insert("math.pow".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg { if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
            let x = oo.force(vx.clone(), ctx).collapse().clone(); let y = oo.force(vy.clone(), ctx).collapse().clone();
            let res_e = x.effect().max(y.effect());
            return match (x, y) {
                (Value::Atom(AtomKind::Int(ix), _, _), Value::Atom(AtomKind::Int(iy), _, _)) => {
                    if iy < BigInt::zero() { BottomCause::Conflict.into() }
                    else { Value::Atom(AtomKind::Int(ix.pow(iy.to_u32().unwrap_or(0))), res_e, None) }
                },
                (Value::Atom(AtomKind::Float(fx), _, _), Value::Atom(AtomKind::Float(fy), _, _)) => Value::Atom(AtomKind::Float(fx.powf(fy)), res_e, None),
                (Value::Atom(AtomKind::Int(ix), _, _), Value::Atom(AtomKind::Float(fy), _, _)) => Value::Atom(AtomKind::Float(ix.to_f64().unwrap_or(0.0).powf(fy)), res_e, None),
                (Value::Atom(AtomKind::Float(fx), _, _), Value::Atom(AtomKind::Int(iy), _, _)) => Value::Atom(AtomKind::Float(fx.powf(iy.to_f64().unwrap_or(0.0))), res_e, None),
                _ => BottomCause::Conflict.into()
            };
        } }
        Value::Top
    }) as Arc<BuiltinFn>);

    m.insert("math.abs".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        match oo.force(v, ctx).collapse() {
            Value::Atom(AtomKind::Int(i), e, _) => Value::Atom(AtomKind::Int(i.abs()), *e, None),
            Value::Atom(AtomKind::Float(f), e, _) => Value::Atom(AtomKind::Float(f.abs()), *e, None),
            Value::Atom(AtomKind::Complex(r, i), e, _) => Value::Atom(AtomKind::Float((r * r + i * i).sqrt()), *e, None),
            _ => Value::Top
        }
    }) as Arc<BuiltinFn>);

    m.insert("math.bits".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        Value::Atom(AtomKind::Int(BigInt::from(oo.force(v, ctx).bits())), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);

    m.insert("math.random".to_string(), Arc::new(|_arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| {
        use ring::rand::SecureRandom;
        let mut bytes = [0u8; 8];
        if ring::rand::SystemRandom::new().fill(&mut bytes).is_ok() {
            let v = u64::from_le_bytes(bytes);
            return Value::Atom(AtomKind::Int(BigInt::from(v % 1000)), EffectTag::NonDet, None);
        }
        BottomCause::Conflict.into()
    }) as Arc<BuiltinFn>);

    // ── Phase 2: EML-derived functions ──────────────────────

    m.insert("math.sqrt".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        match oo.force(v, ctx).collapse() {
            Value::Atom(AtomKind::Float(f), e, _) if *f >= 0.0 => Value::Atom(AtomKind::Float(f.sqrt()), *e, None),
            Value::Atom(AtomKind::Float(f), e, _) => Value::Atom(AtomKind::Complex(0.0, f.abs().sqrt()), *e, None),
            Value::Atom(AtomKind::Int(n), e, _) => {
                let f = n.to_f64().unwrap_or(0.0);
                if f >= 0.0 { Value::Atom(AtomKind::Float(f.sqrt()), *e, None) }
                else { Value::Atom(AtomKind::Complex(0.0, f.abs().sqrt()), *e, None) }
            }
            Value::Atom(AtomKind::Complex(r, i), e, _) => {
                let r2 = (r * r + i * i).sqrt();
                let theta = i.atan2(*r) / 2.0;
                Value::Atom(AtomKind::Complex(r2 * theta.cos(), r2 * theta.sin()), *e, None)
            }
            _ => BottomCause::Conflict.into()
        }
    }) as Arc<BuiltinFn>);

    fn blur_singularity(cause_tag: &str) -> Value {
        Value::Bottom(Box::new(crate::value::BottomDetail {
            cause: BottomCause::NumericalError,
            path: None,
            message: Some(cause_tag.to_string()),
            expected: None,
            found: None,
            involved: vec![],
        }))
    }

    fn to_f64(v: &Value) -> Option<f64> {
        match v {
            Value::Atom(AtomKind::Int(n), _, _) => n.to_f64(),
            Value::Atom(AtomKind::Float(f), _, _) => Some(*f),
            _ => None,
        }
    }

    fn to_complex(v: &Value) -> Option<(f64, f64)> {
        match v {
            Value::Atom(AtomKind::Complex(r, i), _, _) => Some((*r, *i)),
            Value::Atom(AtomKind::Int(n), _, _) => n.to_f64().map(|x| (x, 0.0)),
            Value::Atom(AtomKind::Float(f), _, _) => Some((*f, 0.0)),
            _ => None,
        }
    }

    fn compute_exp(v: &Value) -> Option<Value> {
        match to_complex(v) {
            Some((r, i)) => {
                let mag = r.exp();
                Some(Value::Atom(AtomKind::Complex(mag * i.cos(), mag * i.sin()), v.effect(), None))
            }
            None => None,
        }
    }

    fn compute_ln(v: &Value) -> Option<Value> {
        let eff = v.effect();
        match to_complex(v) {
            Some((r, i)) => {
                let mag = (r * r + i * i).sqrt();
                if mag == 0.0 { return None; }
                let theta = i.atan2(r);
                Some(Value::Atom(AtomKind::Complex(mag.ln(), theta), eff, None))
            }
            None => None,
        }
    }

    m.insert("math.exp".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let v = oo.force(v, ctx).collapse().clone();
        compute_exp(&v).unwrap_or(BottomCause::Conflict.into())
    }) as Arc<BuiltinFn>);

    m.insert("math.ln".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let v = oo.force(v, ctx).collapse().clone();
        // ln(0) is singular
        if let Some(f) = to_f64(&v) {
            if f == 0.0 { return blur_singularity("#log_singularity"); }
        }
        if let Some((r, i)) = to_complex(&v) {
            if r == 0.0 && i == 0.0 { return blur_singularity("#log_singularity"); }
        }
        compute_ln(&v).unwrap_or(blur_singularity("#log_singularity"))
    }) as Arc<BuiltinFn>);

    m.insert("math.sin".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        match oo.force(v, ctx).collapse() {
            Value::Atom(AtomKind::Float(f), e, _) => Value::Atom(AtomKind::Float(f.sin()), *e, None),
            Value::Atom(AtomKind::Int(n), e, _) => Value::Atom(AtomKind::Float(n.to_f64().unwrap_or(0.0).sin()), *e, None),
            Value::Atom(AtomKind::Complex(r, i), e, _) => {
                Value::Atom(AtomKind::Complex(r.sin() * i.cosh(), r.cos() * i.sinh()), *e, None)
            }
            _ => BottomCause::Conflict.into()
        }
    }) as Arc<BuiltinFn>);

    m.insert("math.cos".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        match oo.force(v, ctx).collapse() {
            Value::Atom(AtomKind::Float(f), e, _) => Value::Atom(AtomKind::Float(f.cos()), *e, None),
            Value::Atom(AtomKind::Int(n), e, _) => Value::Atom(AtomKind::Float(n.to_f64().unwrap_or(0.0).cos()), *e, None),
            Value::Atom(AtomKind::Complex(r, i), e, _) => {
                Value::Atom(AtomKind::Complex(r.cos() * i.cosh(), -r.sin() * i.sinh()), *e, None)
            }
            _ => BottomCause::Conflict.into()
        }
    }) as Arc<BuiltinFn>);

    m.insert("math.eml".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
                let x = oo.force(vx.clone(), ctx).collapse().clone();
                let y = oo.force(vy.clone(), ctx).collapse().clone();
                // eml(x, y) = exp(x) - ln(y)
                let exp_x = compute_exp(&x);
                let ln_y = compute_ln(&y);
                return match (exp_x, ln_y) {
                    (Some(ex), Some(ly)) => {
                        // sub(ex, ly) — inline the Complex subtraction
                        let eff = ex.effect().max(ly.effect());
                        match (ex, ly) {
                            (Value::Atom(AtomKind::Complex(r1, i1), _, _), Value::Atom(AtomKind::Complex(r2, i2), _, _)) =>
                                Value::Atom(AtomKind::Complex(r1 - r2, i1 - i2), eff, None),
                            _ => BottomCause::Conflict.into(),
                        }
                    }
                    _ => blur_singularity("#eml_singularity"),
                };
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
}

pub fn register_complex_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert("complex.conj".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        match oo.force(v, ctx).collapse() {
            Value::Atom(AtomKind::Complex(r, i), e, _) => Value::Atom(AtomKind::Complex(*r, -*i), *e, None),
            Value::Atom(AtomKind::Int(i), e, _) => Value::Atom(AtomKind::Int(i.clone()), *e, None),
            Value::Atom(AtomKind::Float(f), e, _) => Value::Atom(AtomKind::Float(*f), *e, None),
            _ => Value::Top
        }
    }) as Arc<BuiltinFn>);
    
    m.insert("complex.phase".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        match oo.force(v, ctx).collapse() {
            Value::Atom(AtomKind::Complex(r, i), e, _) => Value::Atom(AtomKind::Float(i.atan2(*r)), *e, None),
            Value::Atom(AtomKind::Int(_), e, _) => Value::Atom(AtomKind::Float(0.0), *e, None),
            Value::Atom(AtomKind::Float(f), e, _) => {
                if *f >= 0.0 { Value::Atom(AtomKind::Float(0.0), *e, None) }
                else { Value::Atom(AtomKind::Float(std::f64::consts::PI), *e, None) }
            },
            _ => Value::Top
        }
    }) as Arc<BuiltinFn>);
    
    m.insert("complex.real".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        match oo.force(v, ctx).collapse() {
            Value::Atom(AtomKind::Complex(r, _), e, _) => Value::Atom(AtomKind::Float(*r), *e, None),
            Value::Atom(AtomKind::Int(i), e, _) => Value::Atom(AtomKind::Int(i.clone()), *e, None),
            Value::Atom(AtomKind::Float(f), e, _) => Value::Atom(AtomKind::Float(*f), *e, None),
            _ => Value::Top
        }
    }) as Arc<BuiltinFn>);
    
    m.insert("complex.imag".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        match oo.force(v, ctx).collapse() {
            Value::Atom(AtomKind::Complex(_, i), e, _) => Value::Atom(AtomKind::Float(*i), *e, None),
            Value::Atom(AtomKind::Int(_), e, _) => Value::Atom(AtomKind::Int(BigInt::zero()), *e, None),
            Value::Atom(AtomKind::Float(_), e, _) => Value::Atom(AtomKind::Float(0.0), *e, None),
            _ => Value::Top
        }
    }) as Arc<BuiltinFn>);
}