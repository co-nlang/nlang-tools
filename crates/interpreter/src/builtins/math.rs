use crate::value::{BlurCause, BlurDetail, BottomCause, EffectTag, Value};
use crate::{BuiltinFn, EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::collections::HashMap;
use std::sync::Arc;

fn bigint_gcd(mut a: BigInt, mut b: BigInt) -> BigInt {
    a = a.abs();
    b = b.abs();
    while !b.is_zero() {
        let t = b.clone();
        b = a % &b;
        a = t;
    }
    a
}

fn bigint_factorial(n: BigInt) -> BigInt {
    let mut result = BigInt::one();
    let mut i = BigInt::from(2i64);
    while i <= n {
        result *= &i;
        i += 1i64;
    }
    result
}

fn bigint_choose(n: &BigInt, k: &BigInt) -> BigInt {
    if k.is_negative() || k > n {
        return BigInt::zero();
    }
    let n_minus_k = n - k;
    let k_eff = if n_minus_k < *k { n_minus_k } else { k.clone() };
    let mut result = BigInt::one();
    let mut i = BigInt::zero();
    while i < k_eff {
        result = &result * (n - &i) / (&i + BigInt::one());
        i += 1i64;
    }
    result
}

fn bigint_modpow(mut base: BigInt, mut exp: BigInt, modulus: &BigInt) -> BigInt {
    if modulus == &BigInt::one() {
        return BigInt::zero();
    }
    let mut result = BigInt::one();
    base = base % modulus;
    while exp > BigInt::zero() {
        if &exp % 2i64 == BigInt::one() {
            result = &result * &base % modulus;
        }
        exp /= 2i64;
        base = &base * &base % modulus;
    }
    result
}

fn is_prime_miller_rabin(n: &BigInt) -> bool {
    if n < &BigInt::from(2i32) {
        return false;
    }
    if n == &BigInt::from(2i32) || n == &BigInt::from(3i32) {
        return true;
    }
    if (n % 2i32) == BigInt::zero() {
        return false;
    }

    let mut d = n - BigInt::one();
    let mut r = 0u32;
    while (&d % 2i32) == BigInt::zero() {
        d /= 2i32;
        r += 1;
    }

    let witnesses: &[i64] = &[2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];
    let n_minus_one = n - BigInt::one();

    'witness: for &w in witnesses {
        let a = BigInt::from(w);
        if &a >= n {
            continue;
        }
        let mut x = bigint_modpow(a, d.clone(), n);
        if x == BigInt::one() || x == n_minus_one {
            continue;
        }
        for _ in 0..(r - 1) {
            x = &x * &x % n;
            if x == n_minus_one {
                continue 'witness;
            }
        }
        return false;
    }
    true
}

pub fn register_math_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert(
        "math.add".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
                    let fx = oo.force(vx.clone(), ctx);
                    let fy = oo.force(vy.clone(), ctx);
                    let x = fx.collapse();
                    let y = fy.collapse();
                    return match (x, y) {
                        (
                            Value::Atom(AtomKind::Int(ix), e1, _),
                            Value::Atom(AtomKind::Int(iy), e2, _),
                        ) => Value::Atom(AtomKind::Int(ix + iy), (*e1).union(*e2), None),
                        (
                            Value::Atom(AtomKind::Float(fx), e1, _),
                            Value::Atom(AtomKind::Float(fy), e2, _),
                        ) => Value::Atom(AtomKind::Float(fx + fy), (*e1).union(*e2), None),
                        (
                            Value::Atom(AtomKind::Int(ix), e1, _),
                            Value::Atom(AtomKind::Float(fy), e2, _),
                        ) => Value::Atom(
                            AtomKind::Float(ix.to_f64().unwrap_or(0.0) + fy),
                            (*e1).union(*e2),
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Float(fx), e1, _),
                            Value::Atom(AtomKind::Int(iy), e2, _),
                        ) => Value::Atom(
                            AtomKind::Float(fx + iy.to_f64().unwrap_or(0.0)),
                            (*e1).union(*e2),
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Complex(r1, i1), e1, _),
                            Value::Atom(AtomKind::Complex(r2, i2), e2, _),
                        ) => {
                            Value::Atom(AtomKind::Complex(r1 + r2, i1 + i2), (*e1).union(*e2), None)
                        }
                        (
                            Value::Atom(AtomKind::Complex(r, i), e1, _),
                            Value::Atom(AtomKind::Int(y), e2, _),
                        ) => Value::Atom(
                            AtomKind::Complex(r + y.to_f64().unwrap_or(0.0), *i),
                            (*e1).union(*e2),
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Complex(r, i), e1, _),
                            Value::Atom(AtomKind::Float(y), e2, _),
                        ) => Value::Atom(AtomKind::Complex(r + y, *i), (*e1).union(*e2), None),
                        (
                            Value::Atom(AtomKind::Int(x), e1, _),
                            Value::Atom(AtomKind::Complex(r, i), e2, _),
                        ) => Value::Atom(
                            AtomKind::Complex(x.to_f64().unwrap_or(0.0) + r, *i),
                            (*e1).union(*e2),
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Float(x), e1, _),
                            Value::Atom(AtomKind::Complex(r, i), e2, _),
                        ) => Value::Atom(AtomKind::Complex(x + r, *i), (*e1).union(*e2), None),
                        (
                            Value::Atom(AtomKind::Str(sx), e1, _),
                            Value::Atom(AtomKind::Str(sy), e2, _),
                        ) => Value::Atom(
                            AtomKind::Str(format!("{}{}", sx, sy)),
                            (*e1).union(*e2),
                            None,
                        ),
                        _ => BottomCause::Conflict.into(),
                    };
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "math.sub".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
                    let x = oo.force(vx.clone(), ctx).collapse().clone();
                    let y = oo.force(vy.clone(), ctx).collapse().clone();
                    let res_e = x.effect().union(y.effect());
                    return match (x, y) {
                        (
                            Value::Atom(AtomKind::Int(ix), _, _),
                            Value::Atom(AtomKind::Int(iy), _, _),
                        ) => Value::Atom(AtomKind::Int(ix - iy), res_e, None),
                        (
                            Value::Atom(AtomKind::Float(fx), _, _),
                            Value::Atom(AtomKind::Float(fy), _, _),
                        ) => Value::Atom(AtomKind::Float(fx - fy), res_e, None),
                        (
                            Value::Atom(AtomKind::Int(ix), _, _),
                            Value::Atom(AtomKind::Float(fy), _, _),
                        ) => Value::Atom(
                            AtomKind::Float(ix.to_f64().unwrap_or(0.0) - fy),
                            res_e,
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Float(fx), _, _),
                            Value::Atom(AtomKind::Int(iy), _, _),
                        ) => Value::Atom(
                            AtomKind::Float(fx - iy.to_f64().unwrap_or(0.0)),
                            res_e,
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Complex(r1, i1), _, _),
                            Value::Atom(AtomKind::Complex(r2, i2), _, _),
                        ) => Value::Atom(AtomKind::Complex(r1 - r2, i1 - i2), res_e, None),
                        (
                            Value::Atom(AtomKind::Complex(r, i), _, _),
                            Value::Atom(AtomKind::Int(y), _, _),
                        ) => Value::Atom(
                            AtomKind::Complex(r - y.to_f64().unwrap_or(0.0), i),
                            res_e,
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Complex(r, i), _, _),
                            Value::Atom(AtomKind::Float(y), _, _),
                        ) => Value::Atom(AtomKind::Complex(r - y, i), res_e, None),
                        (
                            Value::Atom(AtomKind::Int(x), _, _),
                            Value::Atom(AtomKind::Complex(r, i), _, _),
                        ) => Value::Atom(
                            AtomKind::Complex(x.to_f64().unwrap_or(0.0) - r, -i),
                            res_e,
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Float(x), _, _),
                            Value::Atom(AtomKind::Complex(r, i), _, _),
                        ) => Value::Atom(AtomKind::Complex(x - r, -i), res_e, None),
                        _ => BottomCause::Conflict.into(),
                    };
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "math.mul".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
                    let x = oo.force(vx.clone(), ctx).collapse().clone();
                    let y = oo.force(vy.clone(), ctx).collapse().clone();
                    let res_e = x.effect().union(y.effect());
                    return match (x, y) {
                        (
                            Value::Atom(AtomKind::Int(ix), _, _),
                            Value::Atom(AtomKind::Int(iy), _, _),
                        ) => Value::Atom(AtomKind::Int(ix * iy), res_e, None),
                        (
                            Value::Atom(AtomKind::Float(fx), _, _),
                            Value::Atom(AtomKind::Float(fy), _, _),
                        ) => Value::Atom(AtomKind::Float(fx * fy), res_e, None),
                        (
                            Value::Atom(AtomKind::Int(ix), _, _),
                            Value::Atom(AtomKind::Float(fy), _, _),
                        ) => Value::Atom(
                            AtomKind::Float(ix.to_f64().unwrap_or(0.0) * fy),
                            res_e,
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Float(fx), _, _),
                            Value::Atom(AtomKind::Int(iy), _, _),
                        ) => Value::Atom(
                            AtomKind::Float(fx * iy.to_f64().unwrap_or(0.0)),
                            res_e,
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Complex(r1, i1), _, _),
                            Value::Atom(AtomKind::Complex(r2, i2), _, _),
                        ) => Value::Atom(
                            AtomKind::Complex(r1 * r2 - i1 * i2, r1 * i2 + i1 * r2),
                            res_e,
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Complex(r, i), _, _),
                            Value::Atom(AtomKind::Int(y), _, _),
                        ) => Value::Atom(
                            AtomKind::Complex(
                                r * y.to_f64().unwrap_or(0.0),
                                i * y.to_f64().unwrap_or(0.0),
                            ),
                            res_e,
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Complex(r, i), _, _),
                            Value::Atom(AtomKind::Float(y), _, _),
                        ) => Value::Atom(AtomKind::Complex(r * y, i * y), res_e, None),
                        (
                            Value::Atom(AtomKind::Int(x), _, _),
                            Value::Atom(AtomKind::Complex(r, i), _, _),
                        ) => Value::Atom(
                            AtomKind::Complex(
                                x.to_f64().unwrap_or(0.0) * r,
                                x.to_f64().unwrap_or(0.0) * i,
                            ),
                            res_e,
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Float(x), _, _),
                            Value::Atom(AtomKind::Complex(r, i), _, _),
                        ) => Value::Atom(AtomKind::Complex(x * r, x * i), res_e, None),
                        _ => BottomCause::Conflict.into(),
                    };
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "math.div".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
                    let x = oo.force(vx.clone(), ctx).collapse().clone();
                    let y = oo.force(vy.clone(), ctx).collapse().clone();
                    let res_e = x.effect().union(y.effect());
                    return match (x, y) {
                        (
                            Value::Atom(AtomKind::Int(ix), _, _),
                            Value::Atom(AtomKind::Int(iy), _, _),
                        ) => {
                            if iy.is_zero() {
                                BottomCause::Conflict.into()
                            } else {
                                Value::Atom(AtomKind::Int(ix / iy), res_e, None)
                            }
                        }
                        (
                            Value::Atom(AtomKind::Float(fx), _, _),
                            Value::Atom(AtomKind::Float(fy), _, _),
                        ) => Value::Atom(AtomKind::Float(fx / fy), res_e, None),
                        (
                            Value::Atom(AtomKind::Int(ix), _, _),
                            Value::Atom(AtomKind::Float(fy), _, _),
                        ) => Value::Atom(
                            AtomKind::Float(ix.to_f64().unwrap_or(0.0) / fy),
                            res_e,
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Float(fx), _, _),
                            Value::Atom(AtomKind::Int(iy), _, _),
                        ) => Value::Atom(
                            AtomKind::Float(fx / iy.to_f64().unwrap_or(0.0)),
                            res_e,
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Complex(r1, i1), _, _),
                            Value::Atom(AtomKind::Complex(r2, i2), _, _),
                        ) => {
                            let denom = r2 * r2 + i2 * i2;
                            if denom == 0.0 {
                                return BottomCause::Conflict.into();
                            }
                            Value::Atom(
                                AtomKind::Complex(
                                    (r1 * r2 + i1 * i2) / denom,
                                    (i1 * r2 - r1 * i2) / denom,
                                ),
                                res_e,
                                None,
                            )
                        }
                        (
                            Value::Atom(AtomKind::Complex(r, i), _, _),
                            Value::Atom(AtomKind::Int(y), _, _),
                        ) => {
                            let d = y.to_f64().unwrap_or(0.0);
                            if d == 0.0 {
                                return BottomCause::Conflict.into();
                            }
                            Value::Atom(AtomKind::Complex(r / d, i / d), res_e, None)
                        }
                        (
                            Value::Atom(AtomKind::Complex(r, i), _, _),
                            Value::Atom(AtomKind::Float(y), _, _),
                        ) => {
                            if y == 0.0 {
                                return BottomCause::Conflict.into();
                            }
                            Value::Atom(AtomKind::Complex(r / y, i / y), res_e, None)
                        }
                        (
                            Value::Atom(AtomKind::Int(x), _, _),
                            Value::Atom(AtomKind::Complex(r, i), _, _),
                        ) => {
                            let denom = r * r + i * i;
                            if denom == 0.0 {
                                return BottomCause::Conflict.into();
                            }
                            let xv = x.to_f64().unwrap_or(0.0);
                            Value::Atom(
                                AtomKind::Complex(xv * r / denom, -xv * i / denom),
                                res_e,
                                None,
                            )
                        }
                        (
                            Value::Atom(AtomKind::Float(x), _, _),
                            Value::Atom(AtomKind::Complex(r, i), _, _),
                        ) => {
                            let denom = r * r + i * i;
                            if denom == 0.0 {
                                return BottomCause::Conflict.into();
                            }
                            Value::Atom(
                                AtomKind::Complex(x * r / denom, -x * i / denom),
                                res_e,
                                None,
                            )
                        }
                        _ => BottomCause::Conflict.into(),
                    };
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "math.rem".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
                    let x = oo.force(vx.clone(), ctx).collapse().clone();
                    let y = oo.force(vy.clone(), ctx).collapse().clone();
                    let res_e = x.effect().union(y.effect());
                    return match (x, y) {
                        (
                            Value::Atom(AtomKind::Int(ix), _, _),
                            Value::Atom(AtomKind::Int(iy), _, _),
                        ) => {
                            if iy.is_zero() {
                                BottomCause::Conflict.into()
                            } else {
                                Value::Atom(AtomKind::Int(ix % iy), res_e, None)
                            }
                        }
                        (
                            Value::Atom(AtomKind::Float(fx), _, _),
                            Value::Atom(AtomKind::Float(fy), _, _),
                        ) => Value::Atom(AtomKind::Float(fx % fy), res_e, None),
                        _ => BottomCause::Conflict.into(),
                    };
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "math.pow".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
                    let x = oo.force(vx.clone(), ctx).collapse().clone();
                    let y = oo.force(vy.clone(), ctx).collapse().clone();
                    let res_e = x.effect().union(y.effect());
                    return match (x, y) {
                        (
                            Value::Atom(AtomKind::Int(ix), _, _),
                            Value::Atom(AtomKind::Int(iy), _, _),
                        ) => {
                            if iy < BigInt::zero() {
                                BottomCause::Conflict.into()
                            } else {
                                Value::Atom(
                                    AtomKind::Int(ix.pow(iy.to_u32().unwrap_or(0))),
                                    res_e,
                                    None,
                                )
                            }
                        }
                        (
                            Value::Atom(AtomKind::Float(fx), _, _),
                            Value::Atom(AtomKind::Float(fy), _, _),
                        ) => Value::Atom(AtomKind::Float(fx.powf(fy)), res_e, None),
                        (
                            Value::Atom(AtomKind::Int(ix), _, _),
                            Value::Atom(AtomKind::Float(fy), _, _),
                        ) => Value::Atom(
                            AtomKind::Float(ix.to_f64().unwrap_or(0.0).powf(fy)),
                            res_e,
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Float(fx), _, _),
                            Value::Atom(AtomKind::Int(iy), _, _),
                        ) => Value::Atom(
                            AtomKind::Float(fx.powf(iy.to_f64().unwrap_or(0.0))),
                            res_e,
                            None,
                        ),
                        _ => BottomCause::Conflict.into(),
                    };
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "math.abs".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            match oo.force(v, ctx).collapse() {
                Value::Atom(AtomKind::Int(i), e, _) => {
                    Value::Atom(AtomKind::Int(i.abs()), *e, None)
                }
                Value::Atom(AtomKind::Float(f), e, _) => {
                    Value::Atom(AtomKind::Float(f.abs()), *e, None)
                }
                Value::Atom(AtomKind::Complex(r, i), e, _) => {
                    Value::Atom(AtomKind::Float((r * r + i * i).sqrt()), *e, None)
                }
                _ => Value::Top,
            }
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "math.bits".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            Value::Atom(
                AtomKind::Int(BigInt::from(oo.force(v, ctx).bits())),
                EffectTag::Pure,
                None,
            )
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "math.random".to_string(),
        Arc::new(|_arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| {
            use ring::rand::SecureRandom;
            let mut bytes = [0u8; 8];
            if ring::rand::SystemRandom::new().fill(&mut bytes).is_ok() {
                let v = u64::from_le_bytes(bytes);
                return Value::Atom(
                    AtomKind::Int(BigInt::from(v % 1000)),
                    EffectTag::NonDet,
                    None,
                );
            }
            BottomCause::Conflict.into()
        }) as Arc<BuiltinFn>,
    );

    // ── Phase 2: EML-derived functions ──────────────────────

    m.insert(
        "math.sqrt".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let branch: i64 = if let Value::Combo(ref c) = arg {
                c.get_field("%branch")
                    .and_then(|v| {
                        if let Value::Atom(AtomKind::Int(n), _, _) = v {
                            n.to_i64()
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0)
            } else {
                0
            };
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            let result = match oo.force(v, ctx).collapse() {
                Value::Atom(AtomKind::Int(n), e, _) => {
                    let f = n.to_f64().unwrap_or(0.0);
                    if f < 0.0 {
                        Value::Atom(AtomKind::Complex(0.0, f.abs().sqrt()), *e, None)
                    } else {
                        Value::Atom(AtomKind::Float(f.sqrt()), *e, None)
                    }
                }
                Value::Atom(AtomKind::Float(f), e, _) => {
                    if *f < 0.0 {
                        Value::Atom(AtomKind::Complex(0.0, f.abs().sqrt()), *e, None)
                    } else {
                        Value::Atom(AtomKind::Float(f.sqrt()), *e, None)
                    }
                }
                Value::Atom(AtomKind::Complex(r, i), e, _) => {
                    let mag = (r * r + i * i).sqrt();
                    let new_r = ((mag + r) / 2.0).sqrt();
                    let new_i = if *i >= 0.0 {
                        ((mag - r) / 2.0).sqrt()
                    } else {
                        -((mag - r) / 2.0).sqrt()
                    };
                    Value::Atom(AtomKind::Complex(new_r, new_i), *e, None)
                }
                _ => BottomCause::Conflict.into(),
            };
            if branch == 1 {
                match result {
                    Value::Atom(AtomKind::Float(f), e, r) => Value::Atom(AtomKind::Float(-f), e, r),
                    Value::Atom(AtomKind::Complex(re, im), e, r) => {
                        Value::Atom(AtomKind::Complex(-re, -im), e, r)
                    }
                    other => other,
                }
            } else {
                result
            }
        }) as Arc<BuiltinFn>,
    );

    fn blur_singularity(cause_tag: &str, ctx: &crate::EvalContext) -> Value {
        Value::Blur(BlurDetail::from_single(
            BlurCause::MathSingularity(cause_tag.trim_start_matches('#').to_string()),
            ctx.horizon_params(),
            None,
            EffectTag::Pure,
        ))
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
                Some(Value::Atom(
                    AtomKind::Complex(mag * i.cos(), mag * i.sin()),
                    v.effect(),
                    None,
                ))
            }
            None => None,
        }
    }

    fn compute_ln(v: &Value) -> Option<Value> {
        let eff = v.effect();
        match to_complex(v) {
            Some((r, i)) => {
                let mag = (r * r + i * i).sqrt();
                if mag == 0.0 {
                    return None;
                }
                let theta = i.atan2(r);
                Some(Value::Atom(AtomKind::Complex(mag.ln(), theta), eff, None))
            }
            None => None,
        }
    }

    m.insert(
        "math.exp".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            let v = oo.force(v, ctx).collapse().clone();
            compute_exp(&v).unwrap_or(BottomCause::Conflict.into())
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "math.ln".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let branch: i64 = if let Value::Combo(ref c) = arg {
                c.get_field("%branch")
                    .and_then(|v| {
                        if let Value::Atom(AtomKind::Int(n), _, _) = v {
                            n.to_i64()
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0)
            } else {
                0
            };
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            let v = oo.force(v, ctx).collapse().clone();
            // ln(0) is singular
            if let Some(f) = to_f64(&v) {
                if f == 0.0 {
                    return blur_singularity("#log_singularity", &*ctx);
                }
            }
            if let Some((r, i)) = to_complex(&v) {
                if r == 0.0 && i == 0.0 {
                    return blur_singularity("#log_singularity", &*ctx);
                }
            }
            let base_result = compute_ln(&v).unwrap_or(blur_singularity("#log_singularity", &*ctx));
            if branch != 0 {
                let offset_imag = 2.0 * std::f64::consts::PI * (branch as f64);
                match base_result {
                    Value::Atom(AtomKind::Complex(r, i), e, rank) => {
                        Value::Atom(AtomKind::Complex(r, i + offset_imag), e, rank)
                    }
                    Value::Atom(AtomKind::Float(r), e, rank) => {
                        Value::Atom(AtomKind::Complex(r, offset_imag), e, rank)
                    }
                    other => other,
                }
            } else {
                base_result
            }
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "math.sin".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            match oo.force(v, ctx).collapse() {
                Value::Atom(AtomKind::Float(f), e, _) => {
                    Value::Atom(AtomKind::Float(f.sin()), *e, None)
                }
                Value::Atom(AtomKind::Int(n), e, _) => {
                    Value::Atom(AtomKind::Float(n.to_f64().unwrap_or(0.0).sin()), *e, None)
                }
                Value::Atom(AtomKind::Complex(r, i), e, _) => Value::Atom(
                    AtomKind::Complex(r.sin() * i.cosh(), r.cos() * i.sinh()),
                    *e,
                    None,
                ),
                _ => BottomCause::Conflict.into(),
            }
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "math.cos".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            match oo.force(v, ctx).collapse() {
                Value::Atom(AtomKind::Float(f), e, _) => {
                    Value::Atom(AtomKind::Float(f.cos()), *e, None)
                }
                Value::Atom(AtomKind::Int(n), e, _) => {
                    Value::Atom(AtomKind::Float(n.to_f64().unwrap_or(0.0).cos()), *e, None)
                }
                Value::Atom(AtomKind::Complex(r, i), e, _) => Value::Atom(
                    AtomKind::Complex(r.cos() * i.cosh(), -r.sin() * i.sinh()),
                    *e,
                    None,
                ),
                _ => BottomCause::Conflict.into(),
            }
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "math.eml".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let branch: i64 = if let Value::Combo(ref c) = arg {
                c.get_field("%branch")
                    .and_then(|v| {
                        if let Value::Atom(AtomKind::Int(n), _, _) = v {
                            n.to_i64()
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0)
            } else {
                0
            };
            if let Value::Combo(ref c) = arg {
                if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
                    let x = oo.force(vx.clone(), ctx).collapse().clone();
                    let y = oo.force(vy.clone(), ctx).collapse().clone();
                    let exp_x = compute_exp(&x);
                    let ln_y = compute_ln(&y);
                    return match (exp_x, ln_y) {
                        (Some(ex), Some(ly)) => {
                            let eff = ex.effect().union(ly.effect());
                            let base = match (ex, ly) {
                                (
                                    Value::Atom(AtomKind::Complex(r1, i1), _, _),
                                    Value::Atom(AtomKind::Complex(r2, i2), _, _),
                                ) => Value::Atom(AtomKind::Complex(r1 - r2, i1 - i2), eff, None),
                                _ => return BottomCause::Conflict.into(),
                            };
                            if branch != 0 {
                                let offset_imag = 2.0 * std::f64::consts::PI * (branch as f64);
                                match base {
                                    Value::Atom(AtomKind::Complex(r, i), e, rank) => {
                                        Value::Atom(AtomKind::Complex(r, i - offset_imag), e, rank)
                                    }
                                    other => other,
                                }
                            } else {
                                base
                            }
                        }
                        _ => blur_singularity("#eml_singularity", &*ctx),
                    };
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );
}

pub fn register_complex_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert(
        "complex.conj".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            match oo.force(v, ctx).collapse() {
                Value::Atom(AtomKind::Complex(r, i), e, _) => {
                    Value::Atom(AtomKind::Complex(*r, -*i), *e, None)
                }
                Value::Atom(AtomKind::Int(i), e, _) => {
                    Value::Atom(AtomKind::Int(i.clone()), *e, None)
                }
                Value::Atom(AtomKind::Float(f), e, _) => Value::Atom(AtomKind::Float(*f), *e, None),
                _ => Value::Top,
            }
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "complex.phase".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            match oo.force(v, ctx).collapse() {
                Value::Atom(AtomKind::Complex(r, i), e, _) => {
                    Value::Atom(AtomKind::Float(i.atan2(*r)), *e, None)
                }
                Value::Atom(AtomKind::Int(_), e, _) => Value::Atom(AtomKind::Float(0.0), *e, None),
                Value::Atom(AtomKind::Float(f), e, _) => {
                    if *f >= 0.0 {
                        Value::Atom(AtomKind::Float(0.0), *e, None)
                    } else {
                        Value::Atom(AtomKind::Float(std::f64::consts::PI), *e, None)
                    }
                }
                _ => Value::Top,
            }
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "complex.real".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            match oo.force(v, ctx).collapse() {
                Value::Atom(AtomKind::Complex(r, _), e, _) => {
                    Value::Atom(AtomKind::Float(*r), *e, None)
                }
                Value::Atom(AtomKind::Int(i), e, _) => {
                    Value::Atom(AtomKind::Int(i.clone()), *e, None)
                }
                Value::Atom(AtomKind::Float(f), e, _) => Value::Atom(AtomKind::Float(*f), *e, None),
                _ => Value::Top,
            }
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "complex.imag".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            match oo.force(v, ctx).collapse() {
                Value::Atom(AtomKind::Complex(_, i), e, _) => {
                    Value::Atom(AtomKind::Float(*i), *e, None)
                }
                Value::Atom(AtomKind::Int(_), e, _) => {
                    Value::Atom(AtomKind::Int(BigInt::zero()), *e, None)
                }
                Value::Atom(AtomKind::Float(_), e, _) => {
                    Value::Atom(AtomKind::Float(0.0), *e, None)
                }
                _ => Value::Top,
            }
        }) as Arc<BuiltinFn>,
    );

    // ── Phase 19: Math comparison and rounding ────────────────────

    m.insert(
        "math.min".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(va), Some(vb)) = (c.get_field("0"), c.get_field("1")) {
                    let a = oo.force(va.clone(), ctx).collapse().clone();
                    let b = oo.force(vb.clone(), ctx).collapse().clone();
                    let res_e = a.effect().union(b.effect());
                    return match (&a, &b) {
                        (
                            Value::Atom(AtomKind::Int(ia), _, _),
                            Value::Atom(AtomKind::Int(ib), _, _),
                        ) => Value::Atom(
                            AtomKind::Int(if ia <= ib { ia.clone() } else { ib.clone() }),
                            res_e,
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Float(fa), _, _),
                            Value::Atom(AtomKind::Float(fb), _, _),
                        ) => Value::Atom(AtomKind::Float(fa.min(*fb)), res_e, None),
                        (
                            Value::Atom(AtomKind::Int(ia), _, _),
                            Value::Atom(AtomKind::Float(fb), _, _),
                        ) => Value::Atom(
                            AtomKind::Float(ia.to_f64().unwrap_or(0.0).min(*fb)),
                            res_e,
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Float(fa), _, _),
                            Value::Atom(AtomKind::Int(ib), _, _),
                        ) => Value::Atom(
                            AtomKind::Float(fa.min(ib.to_f64().unwrap_or(0.0))),
                            res_e,
                            None,
                        ),
                        _ => BottomCause::Conflict.into(),
                    };
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "math.max".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(va), Some(vb)) = (c.get_field("0"), c.get_field("1")) {
                    let a = oo.force(va.clone(), ctx).collapse().clone();
                    let b = oo.force(vb.clone(), ctx).collapse().clone();
                    let res_e = a.effect().union(b.effect());
                    return match (&a, &b) {
                        (
                            Value::Atom(AtomKind::Int(ia), _, _),
                            Value::Atom(AtomKind::Int(ib), _, _),
                        ) => Value::Atom(
                            AtomKind::Int(if ia >= ib { ia.clone() } else { ib.clone() }),
                            res_e,
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Float(fa), _, _),
                            Value::Atom(AtomKind::Float(fb), _, _),
                        ) => Value::Atom(AtomKind::Float(fa.max(*fb)), res_e, None),
                        (
                            Value::Atom(AtomKind::Int(ia), _, _),
                            Value::Atom(AtomKind::Float(fb), _, _),
                        ) => Value::Atom(
                            AtomKind::Float(ia.to_f64().unwrap_or(0.0).max(*fb)),
                            res_e,
                            None,
                        ),
                        (
                            Value::Atom(AtomKind::Float(fa), _, _),
                            Value::Atom(AtomKind::Int(ib), _, _),
                        ) => Value::Atom(
                            AtomKind::Float(fa.max(ib.to_f64().unwrap_or(0.0))),
                            res_e,
                            None,
                        ),
                        _ => BottomCause::Conflict.into(),
                    };
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "math.floor".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            match oo.force(v, ctx).collapse().clone() {
                Value::Atom(AtomKind::Float(f), e, _) => {
                    Value::Atom(AtomKind::Float(f.floor()), e, None)
                }
                Value::Atom(AtomKind::Int(i), e, _) => Value::Atom(AtomKind::Int(i), e, None),
                _ => BottomCause::Conflict.into(),
            }
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "math.ceil".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            match oo.force(v, ctx).collapse().clone() {
                Value::Atom(AtomKind::Float(f), e, _) => {
                    Value::Atom(AtomKind::Float(f.ceil()), e, None)
                }
                Value::Atom(AtomKind::Int(i), e, _) => Value::Atom(AtomKind::Int(i), e, None),
                _ => BottomCause::Conflict.into(),
            }
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "math.round".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            match oo.force(v, ctx).collapse().clone() {
                Value::Atom(AtomKind::Float(f), e, _) => {
                    Value::Atom(AtomKind::Float(f.round()), e, None)
                }
                Value::Atom(AtomKind::Int(i), e, _) => Value::Atom(AtomKind::Int(i), e, None),
                _ => BottomCause::Conflict.into(),
            }
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "math.clamp".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vlo), Some(vhi), Some(vx)) =
                    (c.get_field("0"), c.get_field("1"), c.get_field("2"))
                {
                    let lo = oo.force(vlo.clone(), ctx).collapse().clone();
                    let hi = oo.force(vhi.clone(), ctx).collapse().clone();
                    let x = oo.force(vx.clone(), ctx).collapse().clone();
                    let res_e = lo.effect().union(hi.effect()).union(x.effect());
                    let to_f = |v: &Value| -> Option<f64> {
                        match v {
                            Value::Atom(AtomKind::Float(f), _, _) => Some(*f),
                            Value::Atom(AtomKind::Int(i), _, _) => i.to_f64(),
                            _ => None,
                        }
                    };
                    if let (Some(flo), Some(fhi), Some(fx)) = (to_f(&lo), to_f(&hi), to_f(&x)) {
                        let clamped = fx.clamp(flo, fhi);
                        return match &x {
                            Value::Atom(AtomKind::Int(ix), _, _) => {
                                if (clamped - fx).abs() < f64::EPSILON {
                                    Value::Atom(AtomKind::Int(ix.clone()), res_e, None)
                                } else {
                                    Value::Atom(AtomKind::Float(clamped), res_e, None)
                                }
                            }
                            _ => Value::Atom(AtomKind::Float(clamped), res_e, None),
                        };
                    }
                    return BottomCause::Conflict.into();
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // math.gcd: {0: a, 1: b} → Int (gcd of |a|, |b|)
    m.insert(
        "math.gcd".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(va), Some(vb)) = (c.get_field("0"), c.get_field("1")) {
                    let fa = oo.force(va.clone(), ctx);
                    let fb = oo.force(vb.clone(), ctx);
                    if let (
                        Value::Atom(AtomKind::Int(a), _, _),
                        Value::Atom(AtomKind::Int(b), _, _),
                    ) = (fa.collapse(), fb.collapse())
                    {
                        return Value::Atom(
                            AtomKind::Int(bigint_gcd(a.clone(), b.clone())),
                            EffectTag::Pure,
                            None,
                        );
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // math.lcm: {0: a, 1: b} → Int (lcm of |a|, |b|)
    m.insert(
        "math.lcm".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(va), Some(vb)) = (c.get_field("0"), c.get_field("1")) {
                    let fa = oo.force(va.clone(), ctx);
                    let fb = oo.force(vb.clone(), ctx);
                    if let (
                        Value::Atom(AtomKind::Int(a), _, _),
                        Value::Atom(AtomKind::Int(b), _, _),
                    ) = (fa.collapse(), fb.collapse())
                    {
                        let g = bigint_gcd(a.clone(), b.clone());
                        if g.is_zero() {
                            return Value::Atom(
                                AtomKind::Int(BigInt::from(0)),
                                EffectTag::Pure,
                                None,
                            );
                        }
                        let lcm = (a.clone().abs() / &g) * b.clone().abs();
                        return Value::Atom(AtomKind::Int(lcm), EffectTag::Pure, None);
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // math.sign: {0: x} → Int (-1/0/1) or Float (-1.0/0.0/1.0)
    m.insert(
        "math.sign".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            match oo.force(v, ctx).collapse().clone() {
                Value::Atom(AtomKind::Int(i), e, _) => {
                    let s = if i.is_positive() {
                        1i64
                    } else if i.is_negative() {
                        -1
                    } else {
                        0
                    };
                    Value::Atom(AtomKind::Int(BigInt::from(s)), e, None)
                }
                Value::Atom(AtomKind::Float(f), e, _) => {
                    let s = if f > 0.0 {
                        1.0f64
                    } else if f < 0.0 {
                        -1.0
                    } else {
                        0.0
                    };
                    Value::Atom(AtomKind::Float(s), e, None)
                }
                _ => Value::Top,
            }
        }) as Arc<BuiltinFn>,
    );

    // math.log2: {0: x} → Float; log2(0) or log2(negative) → Blur
    m.insert(
        "math.log2".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            let fv = oo.force(v, ctx);
            let x: f64 = match fv.collapse() {
                Value::Atom(AtomKind::Float(f), _, _) => *f,
                Value::Atom(AtomKind::Int(i), _, _) => match i.to_f64() {
                    Some(f) => f,
                    None => return Value::Top,
                },
                _ => return Value::Top,
            };
            if x <= 0.0 {
                return Value::Blur(BlurDetail::from_single(
                    BlurCause::MathSingularity("log2".to_string()),
                    ctx.horizon_params(),
                    None,
                    EffectTag::Pure,
                ));
            }
            Value::Atom(AtomKind::Float(x.log2()), EffectTag::Pure, None)
        }) as Arc<BuiltinFn>,
    );

    // math.log10: {0: x} → Float; log10(0) or log10(negative) → Blur
    m.insert(
        "math.log10".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            let fv = oo.force(v, ctx);
            let x: f64 = match fv.collapse() {
                Value::Atom(AtomKind::Float(f), _, _) => *f,
                Value::Atom(AtomKind::Int(i), _, _) => match i.to_f64() {
                    Some(f) => f,
                    None => return Value::Top,
                },
                _ => return Value::Top,
            };
            if x <= 0.0 {
                return Value::Blur(BlurDetail::from_single(
                    BlurCause::MathSingularity("log10".to_string()),
                    ctx.horizon_params(),
                    None,
                    EffectTag::Pure,
                ));
            }
            Value::Atom(AtomKind::Float(x.log10()), EffectTag::Pure, None)
        }) as Arc<BuiltinFn>,
    );

    // math.factorial: {0: n} → Int; n < 0 → Bottom
    m.insert(
        "math.factorial".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            if let Value::Atom(AtomKind::Int(n), _, _) = oo.force(v, ctx).collapse() {
                if n.is_negative() {
                    return BottomCause::Conflict.into();
                }
                return Value::Atom(
                    AtomKind::Int(bigint_factorial(n.clone())),
                    EffectTag::Pure,
                    None,
                );
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // math.choose: {0: n, 1: k} → Int (C(n,k)); n < 0 → Bottom; k < 0 or k > n → 0
    m.insert(
        "math.choose".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vn), Some(vk)) = (c.get_field("0"), c.get_field("1")) {
                    let fn_v = oo.force(vn.clone(), ctx);
                    let fk_v = oo.force(vk.clone(), ctx);
                    if let (
                        Value::Atom(AtomKind::Int(n), _, _),
                        Value::Atom(AtomKind::Int(k), _, _),
                    ) = (fn_v.collapse(), fk_v.collapse())
                    {
                        if n.is_negative() {
                            return BottomCause::Conflict.into();
                        }
                        return Value::Atom(
                            AtomKind::Int(bigint_choose(n, k)),
                            EffectTag::Pure,
                            None,
                        );
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // math.is_prime: {0: n} → #true | #false (deterministic Miller-Rabin)
    m.insert(
        "math.is_prime".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            if let Value::Atom(AtomKind::Int(n), _, _) = oo.force(v, ctx).collapse() {
                let tag = if is_prime_miller_rabin(n) {
                    "true"
                } else {
                    "false"
                };
                return Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::Pure, None);
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // math.pow_mod: {0: base, 1: exp, 2: mod} → Int ((base^exp) % mod)
    m.insert(
        "math.pow_mod".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vb), Some(ve), Some(vm)) =
                    (c.get_field("0"), c.get_field("1"), c.get_field("2"))
                {
                    let fb = oo.force(vb.clone(), ctx);
                    let fe = oo.force(ve.clone(), ctx);
                    let fm = oo.force(vm.clone(), ctx);
                    if let (
                        Value::Atom(AtomKind::Int(base), _, _),
                        Value::Atom(AtomKind::Int(exp), _, _),
                        Value::Atom(AtomKind::Int(modulus), _, _),
                    ) = (fb.collapse(), fe.collapse(), fm.collapse())
                    {
                        if base.is_negative() || exp.is_negative() || modulus <= &BigInt::zero() {
                            return BottomCause::Conflict.into();
                        }
                        return Value::Atom(
                            AtomKind::Int(bigint_modpow(base.clone(), exp.clone(), modulus)),
                            EffectTag::Pure,
                            None,
                        );
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // ── Phase 45: Numeric extras ──────────────────────────────────

    // math.atan2: {0: y, 1: x} → Float
    m.insert(
        "math.atan2".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vy), Some(vx)) = (c.get_field("0"), c.get_field("1")) {
                    let fy = oo.force(vy.clone(), ctx);
                    let fx = oo.force(vx.clone(), ctx);
                    let y: f64 = match fy.collapse() {
                        Value::Atom(AtomKind::Float(f), _, _) => *f,
                        Value::Atom(AtomKind::Int(n), _, _) => match n.to_f64() {
                            Some(f) => f,
                            None => return Value::Top,
                        },
                        _ => return Value::Top,
                    };
                    let x: f64 = match fx.collapse() {
                        Value::Atom(AtomKind::Float(f), _, _) => *f,
                        Value::Atom(AtomKind::Int(n), _, _) => match n.to_f64() {
                            Some(f) => f,
                            None => return Value::Top,
                        },
                        _ => return Value::Top,
                    };
                    return Value::Atom(AtomKind::Float(y.atan2(x)), EffectTag::Pure, None);
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // math.hypot: {0: x, 1: y} → Float
    m.insert(
        "math.hypot".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
                    let fx = oo.force(vx.clone(), ctx);
                    let fy = oo.force(vy.clone(), ctx);
                    let x: f64 = match fx.collapse() {
                        Value::Atom(AtomKind::Float(f), _, _) => *f,
                        Value::Atom(AtomKind::Int(n), _, _) => match n.to_f64() {
                            Some(f) => f,
                            None => return Value::Top,
                        },
                        _ => return Value::Top,
                    };
                    let y: f64 = match fy.collapse() {
                        Value::Atom(AtomKind::Float(f), _, _) => *f,
                        Value::Atom(AtomKind::Int(n), _, _) => match n.to_f64() {
                            Some(f) => f,
                            None => return Value::Top,
                        },
                        _ => return Value::Top,
                    };
                    return Value::Atom(AtomKind::Float(x.hypot(y)), EffectTag::Pure, None);
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // math.sinh: unary Float → Float
    m.insert(
        "math.sinh".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            match oo.force(v, ctx).collapse() {
                Value::Atom(AtomKind::Float(f), e, _) => {
                    Value::Atom(AtomKind::Float(f.sinh()), *e, None)
                }
                Value::Atom(AtomKind::Int(n), e, _) => {
                    Value::Atom(AtomKind::Float(n.to_f64().unwrap_or(0.0).sinh()), *e, None)
                }
                _ => BottomCause::Conflict.into(),
            }
        }) as Arc<BuiltinFn>,
    );

    // math.cosh: unary Float → Float
    m.insert(
        "math.cosh".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            match oo.force(v, ctx).collapse() {
                Value::Atom(AtomKind::Float(f), e, _) => {
                    Value::Atom(AtomKind::Float(f.cosh()), *e, None)
                }
                Value::Atom(AtomKind::Int(n), e, _) => {
                    Value::Atom(AtomKind::Float(n.to_f64().unwrap_or(0.0).cosh()), *e, None)
                }
                _ => BottomCause::Conflict.into(),
            }
        }) as Arc<BuiltinFn>,
    );

    // math.tanh: unary Float → Float
    m.insert(
        "math.tanh".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            match oo.force(v, ctx).collapse() {
                Value::Atom(AtomKind::Float(f), e, _) => {
                    Value::Atom(AtomKind::Float(f.tanh()), *e, None)
                }
                Value::Atom(AtomKind::Int(n), e, _) => {
                    Value::Atom(AtomKind::Float(n.to_f64().unwrap_or(0.0).tanh()), *e, None)
                }
                _ => BottomCause::Conflict.into(),
            }
        }) as Arc<BuiltinFn>,
    );

    // math.trunc: unary Float → Float
    m.insert(
        "math.trunc".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            match oo.force(v, ctx).collapse().clone() {
                Value::Atom(AtomKind::Float(f), e, _) => {
                    Value::Atom(AtomKind::Float(f.trunc()), e, None)
                }
                Value::Atom(AtomKind::Int(i), e, _) => Value::Atom(AtomKind::Int(i), e, None),
                _ => BottomCause::Conflict.into(),
            }
        }) as Arc<BuiltinFn>,
    );

    // math.fract: unary Float → Float
    m.insert(
        "math.fract".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            match oo.force(v, ctx).collapse().clone() {
                Value::Atom(AtomKind::Float(f), e, _) => {
                    Value::Atom(AtomKind::Float(f.fract()), e, None)
                }
                Value::Atom(AtomKind::Int(_), e, _) => Value::Atom(AtomKind::Float(0.0), e, None),
                _ => BottomCause::Conflict.into(),
            }
        }) as Arc<BuiltinFn>,
    );

    // math.to_float: unary Int/Float → Float
    m.insert(
        "math.to_float".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            match oo.force(v, ctx).collapse() {
                Value::Atom(AtomKind::Float(f), e, _) => Value::Atom(AtomKind::Float(*f), *e, None),
                Value::Atom(AtomKind::Int(n), _, _) => match n.to_f64() {
                    Some(f) => Value::Atom(AtomKind::Float(f), EffectTag::Pure, None),
                    None => Value::Top,
                },
                _ => BottomCause::Conflict.into(),
            }
        }) as Arc<BuiltinFn>,
    );

    // ── Order wave W1: numeric order predicates (SPEC_09 §3) ───────────
    // Binary {0: x, 1: y} → #true | #false; int/float cross by value;
    // non-numeric → ⊥ #conflict (math-family error form).
    fn numeric_pair(x: &Value, y: &Value) -> Option<(f64, f64)> {
        let to_f = |v: &Value| -> Option<f64> {
            match v {
                Value::Atom(AtomKind::Int(n), _, _) => n.to_f64(),
                Value::Atom(AtomKind::Float(f), _, _) => Some(*f),
                _ => None,
            }
        };
        Some((to_f(x)?, to_f(y)?))
    }
    fn bool_tag(b: bool, e: EffectTag) -> Value {
        Value::Atom(
            AtomKind::Tag(if b { "true".into() } else { "false".into() }),
            e,
            None,
        )
    }
    macro_rules! math_cmp_pred {
        ($name:expr, $op:expr) => {
            m.insert(
                $name.to_string(),
                Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
                    if let Value::Combo(ref c) = arg {
                        if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
                            let x = oo.force(vx.clone(), ctx).collapse().clone();
                            let y = oo.force(vy.clone(), ctx).collapse().clone();
                            let res_e = x.effect().union(y.effect());
                            return match numeric_pair(&x, &y) {
                                Some((a, b)) => bool_tag(($op)(a, b), res_e),
                                None => BottomCause::Conflict.into(),
                            };
                        }
                    }
                    Value::Top
                }) as Arc<BuiltinFn>,
            );
        };
    }
    math_cmp_pred!("math.lt", |a: f64, b: f64| a < b);
    math_cmp_pred!("math.lte", |a: f64, b: f64| a <= b);
    math_cmp_pred!("math.gt", |a: f64, b: f64| a > b);
    math_cmp_pred!("math.gte", |a: f64, b: f64| a >= b);
}
