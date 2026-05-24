use std::sync::Arc;
use std::collections::HashMap;
use std::str::FromStr;
use indexmap::IndexMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, ComboVal, EffectTag, BottomCause, BottomDetail};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

pub fn register_string_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert("str.concat".to_string(), m.get("math.add").unwrap().clone());
    
    m.insert("str.len".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), e, _) = oo.force(v, ctx).collapse() { return Value::Atom(AtomKind::Int(num_bigint::BigInt::from(s.len())), *e, None); }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("str.trim".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), e, _) = oo.force(v, ctx).collapse() { return Value::Atom(AtomKind::Str(s.trim().to_string()), *e, None); }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("str.split".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg { 
            if let (Some(vsep), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
                let fsep = oo.force(vsep.clone(), ctx).collapse().clone();
                let fs = oo.force(vs.clone(), ctx).collapse().clone();
                if let (Value::Atom(AtomKind::Str(sep), e1, _), Value::Atom(AtomKind::Str(s), e2, _)) = (fsep, fs) {
                    let mut res = IndexMap::new(); for (i, p) in s.split(&*sep).enumerate() { res.insert(i.to_string(), Value::Atom(AtomKind::Str(p.to_string()), e1.max(e2), None)); }
                    res.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
                    return Value::Combo(ComboVal::new(res, false, IndexMap::new(), e1.max(e2), vec![]));
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("str.join".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg { 
            if let (Some(vsep), Some(vlist)) = (c.get_field("0"), c.get_field("1")) {
                let fsep = oo.force(vsep.clone(), ctx).collapse().clone();
                let flist = oo.force(vlist.clone(), ctx);
                let lv = flist.collapse();
                if let (Value::Atom(AtomKind::Str(sep), e1, _), Value::Combo(lc)) = (fsep, lv) {
                    let mut parts: Vec<String> = Vec::new(); let mut max_e = e1;
                    for (k, v) in &lc.fields() { if k.parse::<usize>().is_ok() { if let Value::Atom(AtomKind::Str(s), e, _) = oo.force(v.clone(), ctx).collapse() { parts.push(s.clone()); max_e = max_e.max(*e); } } }
                    return Value::Atom(AtomKind::Str(parts.join(&sep)), max_e, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("str.replace".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vpat), Some(vrep), Some(vs)) = (c.get_field("0"), c.get_field("1"), c.get_field("2")) {
                let fp = oo.force(vpat.clone(), ctx).collapse().clone();
                let fr = oo.force(vrep.clone(), ctx).collapse().clone();
                let fs = oo.force(vs.clone(), ctx).collapse().clone();
                if let (Value::Atom(AtomKind::Str(p), e1, _), Value::Atom(AtomKind::Str(r), e2, _), Value::Atom(AtomKind::Str(s), e3, _)) = (fp, fr, fs) {
                    return Value::Atom(AtomKind::Str(s.replace(&p, &r)), e1.max(e2).max(e3), None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("str.to_lower".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), e, _) = oo.force(v, ctx).collapse() { return Value::Atom(AtomKind::Str(s.to_lowercase()), *e, None); }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("str.to_upper".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), e, _) = oo.force(v, ctx).collapse() { return Value::Atom(AtomKind::Str(s.to_uppercase()), *e, None); }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("str.starts_with".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vpre), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
                let fp = oo.force(vpre.clone(), ctx).collapse().clone();
                let fs = oo.force(vs.clone(), ctx).collapse().clone();
                if let (Value::Atom(AtomKind::Str(p), e1, _), Value::Atom(AtomKind::Str(s), e2, _)) = (fp, fs) {
                    return Value::Atom(AtomKind::Tag(if s.starts_with(&p) { "true".to_string() } else { "false".to_string() }), e1.max(e2), None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("str.ends_with".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vsuf), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
                let fsf = oo.force(vsuf.clone(), ctx).collapse().clone();
                let fs = oo.force(vs.clone(), ctx).collapse().clone();
                if let (Value::Atom(AtomKind::Str(sf), e1, _), Value::Atom(AtomKind::Str(s), e2, _)) = (fsf, fs) {
                    return Value::Atom(AtomKind::Tag(if s.ends_with(&sf) { "true".to_string() } else { "false".to_string() }), e1.max(e2), None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("str.contains".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vpat), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
                let fp = oo.force(vpat.clone(), ctx).collapse().clone();
                let fs = oo.force(vs.clone(), ctx).collapse().clone();
                if let (Value::Atom(AtomKind::Str(p), e1, _), Value::Atom(AtomKind::Str(s), e2, _)) = (fp, fs) {
                    return Value::Atom(AtomKind::Tag(if s.contains(&p) { "true".to_string() } else { "false".to_string() }), e1.max(e2), None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // ── Phase 19: String conversions ─────────────────────────────

    m.insert("str.parse_int".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
            match BigInt::from_str(s.trim()) {
                Ok(n) => return Value::Atom(AtomKind::Int(n), EffectTag::Pure, None),
                Err(_) => return Value::Bottom(Box::new(BottomDetail {
                    cause: BottomCause::Conflict,
                    message: Some(format!("parse_int: invalid integer {:?}", s)),
                    ..Default::default()
                })),
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    m.insert("str.from_int".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        match forced.collapse() {
            Value::Atom(AtomKind::Int(n), e, _) => Value::Atom(AtomKind::Str(n.to_string()), *e, None),
            Value::Atom(AtomKind::Float(f), e, _) => Value::Atom(AtomKind::Str(format!("{}", f)), *e, None),
            other => Value::Atom(AtomKind::Str(other.to_string_plain()), EffectTag::Pure, None),
        }
    }) as Arc<BuiltinFn>);

    m.insert("str.repeat".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vn), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
                let fn_v = oo.force(vn.clone(), ctx).collapse().clone();
                let fs   = oo.force(vs.clone(), ctx).collapse().clone();
                if let (Value::Atom(AtomKind::Int(n), _, _), Value::Atom(AtomKind::Str(s), e, _)) = (fn_v, fs) {
                    let count = n.to_usize().unwrap_or(0);
                    return Value::Atom(AtomKind::Str(s.repeat(count)), e, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // ── Phase 21: str.format ──────────────────────────────────────

    m.insert("str.format".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vfmt), Some(vlist)) = (c.get_field("0"), c.get_field("1")) {
                let fmt_forced = oo.force(vfmt.clone(), ctx);
                let list_forced = oo.force(vlist.clone(), ctx);

                let fmt_str = match fmt_forced.collapse() {
                    Value::Atom(AtomKind::Str(s), _, _) => s.clone(),
                    _ => return Value::Top,
                };

                let args: Vec<String> = {
                    let mut items = Vec::new();
                    let mut i = 0usize;
                    loop {
                        match &list_forced {
                            Value::Combo(ref lc) => {
                                match lc.get_field(&i.to_string()) {
                                    Some(v) => {
                                        items.push(oo.force(v.clone(), ctx).to_string_plain());
                                        i += 1;
                                    }
                                    None => break,
                                }
                            }
                            _ => break,
                        }
                    }
                    items
                };

                let mut result = String::with_capacity(fmt_str.len());
                let mut chars = fmt_str.chars().peekable();
                let mut auto_idx = 0usize;

                while let Some(ch) = chars.next() {
                    match ch {
                        '{' => {
                            match chars.peek() {
                                Some(&'{') => {
                                    chars.next();
                                    result.push('{');
                                }
                                Some(&'}') => {
                                    chars.next();
                                    result.push_str(args.get(auto_idx).map(|s| s.as_str()).unwrap_or(""));
                                    auto_idx += 1;
                                }
                                _ => {
                                    let mut inner = String::new();
                                    loop {
                                        match chars.next() {
                                            Some('}') => break,
                                            Some(c)   => inner.push(c),
                                            None      => break,
                                        }
                                    }
                                    match inner.trim().parse::<usize>() {
                                        Ok(idx) => {
                                            result.push_str(args.get(idx).map(|s| s.as_str()).unwrap_or(""));
                                        }
                                        Err(_) => {
                                            let name = inner.trim();
                                            if let Value::Combo(ref nc) = list_forced {
                                                if let Some(v) = nc.get_field(name) {
                                                    result.push_str(&oo.force(v.clone(), ctx).to_string_plain());
                                                } else {
                                                    result.push('{');
                                                    result.push_str(&inner);
                                                    result.push('}');
                                                }
                                            } else {
                                                result.push('{');
                                                result.push_str(&inner);
                                                result.push('}');
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        '}' => {
                            if chars.peek() == Some(&'}') {
                                chars.next();
                                result.push('}');
                            } else {
                                result.push('}');
                            }
                        }
                        other => result.push(other),
                    }
                }

                return Value::Atom(AtomKind::Str(result), EffectTag::Pure, None);
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // str.char_at: get nth Unicode character (0-indexed), Top if out of range
    m.insert("str.char_at".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vi), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
                let fi = oo.force(vi.clone(), ctx);
                let fs = oo.force(vs.clone(), ctx);
                if let (Value::Atom(AtomKind::Int(idx), _, _), Value::Atom(AtomKind::Str(s), _, _)) =
                    (fi.collapse(), fs.collapse())
                {
                    if let Some(n) = idx.to_usize() {
                        if let Some(ch) = s.chars().nth(n) {
                            return Value::Atom(AtomKind::Str(ch.to_string()), EffectTag::Pure, None);
                        }
                    }
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // str.chars: split string into list of single-character strings
    m.insert("str.chars".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
            let mut res = IndexMap::new();
            for (i, ch) in s.chars().enumerate() {
                res.insert(i.to_string(), Value::Atom(AtomKind::Str(ch.to_string()), EffectTag::Pure, None));
            }
            res.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
            return Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]));
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // str.index_of: {0: needle, 1: haystack} → Int (char index) or #none
    m.insert("str.index_of".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vn), Some(vh)) = (c.get_field("0"), c.get_field("1")) {
                let needle   = oo.force(vn.clone(), ctx);
                let haystack = oo.force(vh.clone(), ctx);
                if let (Value::Atom(AtomKind::Str(n), _, _), Value::Atom(AtomKind::Str(h), _, _)) =
                    (needle.collapse(), haystack.collapse())
                {
                    return match h.find(n.as_str()) {
                        None => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
                        Some(byte_idx) => {
                            let char_idx = h[..byte_idx].chars().count();
                            Value::Atom(AtomKind::Int(BigInt::from(char_idx)), EffectTag::Pure, None)
                        }
                    };
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // str.pad_left: {0: width, 1: str} → Str (space-pad on left)
    m.insert("str.pad_left".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vw), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
                let fw = oo.force(vw.clone(), ctx);
                let fs = oo.force(vs.clone(), ctx);
                if let (Value::Atom(AtomKind::Int(w), _, _), Value::Atom(AtomKind::Str(s), _, _)) =
                    (fw.collapse(), fs.collapse())
                {
                    if let Some(width) = w.to_usize() {
                        let char_count = s.chars().count();
                        if char_count >= width {
                            return Value::Atom(AtomKind::Str(s.clone()), EffectTag::Pure, None);
                        }
                        let pad = " ".repeat(width - char_count);
                        return Value::Atom(AtomKind::Str(format!("{}{}", pad, s)), EffectTag::Pure, None);
                    }
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // str.pad_right: {0: width, 1: str} → Str (space-pad on right)
    m.insert("str.pad_right".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vw), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
                let fw = oo.force(vw.clone(), ctx);
                let fs = oo.force(vs.clone(), ctx);
                if let (Value::Atom(AtomKind::Int(w), _, _), Value::Atom(AtomKind::Str(s), _, _)) =
                    (fw.collapse(), fs.collapse())
                {
                    if let Some(width) = w.to_usize() {
                        let char_count = s.chars().count();
                        if char_count >= width {
                            return Value::Atom(AtomKind::Str(s.clone()), EffectTag::Pure, None);
                        }
                        let pad = " ".repeat(width - char_count);
                        return Value::Atom(AtomKind::Str(format!("{}{}", s, pad)), EffectTag::Pure, None);
                    }
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // str.trim_start: {0: str} → Str (remove leading whitespace)
    m.insert("str.trim_start".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
            return Value::Atom(AtomKind::Str(s.trim_start().to_string()), EffectTag::Pure, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // str.trim_end: {0: str} → Str (remove trailing whitespace)
    m.insert("str.trim_end".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
            return Value::Atom(AtomKind::Str(s.trim_end().to_string()), EffectTag::Pure, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);
}