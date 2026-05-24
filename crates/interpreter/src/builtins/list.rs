use std::sync::Arc;
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, ComboVal, EffectTag};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

pub fn register_list_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert("list.len".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let target = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let flist = oo.force(target, ctx);
        if let Value::Combo(ref cv) = flist.collapse() {
            let count = cv.fields().keys().filter(|k| k.parse::<usize>().is_ok()).count();
            return Value::Atom(AtomKind::Int(BigInt::from(count)), EffectTag::Pure, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("list.at".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg { 
            if let (Some(vidx), Some(vlist)) = (c.get_field("0"), c.get_field("1")) {
                let fidx = oo.force(vidx.clone(), ctx).collapse().clone();
                let flist = oo.force(vlist.clone(), ctx).collapse().clone();
                if let (Value::Atom(AtomKind::Int(idx), _, _), Value::Combo(lc)) = (fidx, flist) {
                    if let Some(v) = lc.get_field(&idx.to_string()) { return v.clone(); }
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("list.concat".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg { 
            if let (Some(v0), Some(v1)) = (c.get_field("0"), c.get_field("1")) {
                let f0 = oo.force(v0.clone(), ctx).collapse().clone();
                let f1 = oo.force(v1.clone(), ctx).collapse().clone();
                if let (Value::Combo(c0), Value::Combo(c1)) = (f0, f1) {
                    let mut res = IndexMap::new();
                    let mut count = 0;
                    for (_k, v) in &c0.fields() { if _k.parse::<usize>().is_ok() { res.insert(count.to_string(), v.clone()); count += 1; } }
                    for (_k, v) in &c1.fields() { if _k.parse::<usize>().is_ok() { res.insert(count.to_string(), v.clone()); count += 1; } }
                    res.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
                    return Value::Combo(ComboVal::new(res, false, IndexMap::new(), c0.effect.max(c1.effect), vec![]));
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("list.reverse".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let fv = oo.force(v, ctx).collapse().clone();
        if let Value::Combo(c) = fv {
            let mut items = Vec::new();
            let mut i = 0;
            while let Some(v) = c.get_field(&i.to_string()) { items.push(v.clone()); i += 1; }
            items.reverse();
            let mut res = IndexMap::new();
            for (idx, v) in items.into_iter().enumerate() { res.insert(idx.to_string(), v); }
            res.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
            return Value::Combo(ComboVal::new(res, false, IndexMap::new(), c.effect, vec![]));
        }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("list.slice".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vstart), Some(vend), Some(vl)) = (c.get_field("0"), c.get_field("1"), c.get_field("2")) {
                let fs = oo.force(vstart.clone(), ctx).collapse().clone();
                let fe = oo.force(vend.clone(), ctx).collapse().clone();
                let fl = oo.force(vl.clone(), ctx).collapse().clone();
                if let (Value::Atom(AtomKind::Int(s), _, _), Value::Atom(AtomKind::Int(e), _, _), Value::Combo(lc)) = (fs, fe, fl) {
                    let mut res = IndexMap::new();
                    let mut count = 0;
                    let mut i = s.clone();
                    while i < e {
                        if let Some(v) = lc.get_field(&i.to_string()) {
                            res.insert(count.to_string(), v.clone());
                            count += 1;
                        }
                        i += 1;
                    }
                    res.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
                    return Value::Combo(ComboVal::new(res, false, IndexMap::new(), lc.effect, vec![]));
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("list.zip".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vl1), Some(vl2)) = (c.get_field("0"), c.get_field("1")) {
                let fl1 = oo.force(vl1.clone(), ctx).collapse().clone();
                let fl2 = oo.force(vl2.clone(), ctx).collapse().clone();
                if let (Value::Combo(c1), Value::Combo(c2)) = (fl1, fl2) {
                    let mut res = IndexMap::new();
                    let mut i = 0;
                    while let (Some(v1), Some(v2)) = (c1.get_field(&i.to_string()), c2.get_field(&i.to_string())) {
                        let mut tuple = IndexMap::new();
                        tuple.insert("0".to_string(), v1.clone());
                        tuple.insert("1".to_string(), v2.clone());
                        res.insert(i.to_string(), Value::Combo(ComboVal::new(tuple, true, IndexMap::new(), v1.effect().max(v2.effect()), vec![])));
                        i += 1;
                    }
                    res.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
                    return Value::Combo(ComboVal::new(res, false, IndexMap::new(), c1.effect.max(c2.effect), vec![]));
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("list.sort".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let fv = oo.force(v, ctx).collapse().clone();
        if let Value::Combo(c) = fv {
            let mut items = Vec::new();
            let mut i = 0;
            while let Some(v) = c.get_field(&i.to_string()) { items.push(oo.force(v.clone(), ctx)); i += 1; }
            items.sort_by(|a, b| {
                let sa = a.to_string_plain();
                let sb = b.to_string_plain();
                sa.cmp(&sb)
            });
            let mut res = IndexMap::new();
            for (idx, v) in items.into_iter().enumerate() { res.insert(idx.to_string(), v); }
            res.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
            return Value::Combo(ComboVal::new(res, false, IndexMap::new(), c.effect, vec![]));
        }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("list.map".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg { 
            if let (Some(v0), Some(v1)) = (c.get_field("0"), c.get_field("1")) {
                let f0 = oo.force(v0.clone(), ctx); let f1 = oo.force(v1.clone(), ctx);
                let (lv, fv) = if oo.is_list(&f0, ctx) { (f0.clone(), f1.clone()) } 
                               else if oo.is_list(&f1, ctx) { (f1.clone(), f0.clone()) }
                               else { return Value::Top; };
                
                if let (Value::Combo(lc), f) = (lv.collapse().clone(), fv) {
                    let mut res = IndexMap::new(); let mut max_e = f.effect();
                    for (k, v) in &lc.fields() { if k.parse::<usize>().is_ok() { let item = oo.force(v.clone(), ctx); let mapped = oo.apply_morphism(f.clone(), item, ctx); let solidified = oo.force_recursive(mapped, ctx); max_e = max_e.max(solidified.effect()); res.insert(k.clone(), solidified); } }
                    for (k, v) in &lc.fields() { if !k.parse::<usize>().is_ok() { res.insert(k.clone(), v.clone()); } }
                    let mut out = lc.clone();
                    for (k, v) in res { out.insert_field(&k, v); }
                    out.effect = max_e; return Value::Combo(out);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("list.fold".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg { 
            if let (Some(v0), Some(v1)) = (c.get_field("0"), c.get_field("1")) {
                let f0 = oo.force(v0.clone(), ctx); let f1 = oo.force(v1.clone(), ctx);
                let (lv, fv) = if oo.is_list(&f0, ctx) { (f0.clone(), f1.clone()) } 
                               else if oo.is_list(&f1, ctx) { (f1.clone(), f0.clone()) }
                               else { (f0.clone(), f1.clone()) };

                if let (Value::Combo(lc), Value::Combo(fc)) = (lv.collapse().clone(), fv.clone()) { 
                    let mut acc = fc.get_field("%val").cloned().unwrap_or(Value::Top); 
                    acc = oo.force(acc, ctx);
                    let f = fc.get_field("%f").cloned().unwrap_or(Value::Top); 
                    if f.is_top() { return acc; }
                    for (k, v) in &lc.fields() {
                        if k.parse::<usize>().is_ok() {
                            let item = oo.force(v.clone(), ctx); 
                            let f_acc = oo.apply_morphism(f.clone(), acc, ctx);
                            let res = oo.apply_morphism(f_acc, item, ctx);
                            acc = oo.force_recursive(res, ctx);
                        }
                    }
                    return acc;
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
    
    m.insert("list.filter".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg { 
            if let (Some(v0), Some(v1)) = (c.get_field("0"), c.get_field("1")) {
                let f0 = oo.force(v0.clone(), ctx); let f1 = oo.force(v1.clone(), ctx);
                let (lv, fv) = if oo.is_list(&f0, ctx) { (f0.clone(), f1.clone()) } else { (f1.clone(), f0.clone()) };
                if let (Value::Combo(lc), f) = (lv.collapse().clone(), fv) {
                    let mut res = IndexMap::new(); let mut count = 0;
                    for (k, v) in &lc.fields() { if k.parse::<usize>().is_ok() { let item = oo.force(v.clone(), ctx); let pred = oo.apply_morphism(f.clone(), item.clone(), ctx); if pred.to_string_plain().trim_start_matches('#') == "true" { res.insert(count.to_string(), item); count += 1; } } }
                    res.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
                    return Value::Combo(ComboVal::new(res, false, IndexMap::new(), lc.effect.max(f.effect()), vec![]));
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // ── Phase 17: list.flat_map ───────────────────────────────────

    fn extract_list_items(list: &Value) -> Vec<Value> {
        let mut items = Vec::new();
        if let Value::Combo(ref lc) = list {
            let len = lc.get_field("%len")
                .and_then(|v| if let Value::Atom(AtomKind::Int(n), _, _) = v { n.to_usize() } else { None })
                .unwrap_or_else(|| lc.fields().keys().filter_map(|k| k.parse::<usize>().ok()).count());
            for i in 0..len {
                if let Some(v) = lc.get_field(&i.to_string()) { items.push(v.clone()); }
            }
        }
        items
    }

    fn build_list_value(items: Vec<Value>) -> Value {
        let mut out = ComboVal::default();
        for (i, v) in items.iter().enumerate() {
            out.insert_field(&i.to_string(), v.clone());
        }
        out.insert_field("%kind", Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
        out.insert_field("%len", Value::Atom(AtomKind::Int(BigInt::from(items.len())), EffectTag::Pure, None));
        Value::Combo(out)
    }

    m.insert("list.flat_map".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(f), Some(list_v)) = (c.get_field("0"), c.get_field("1")) {
                let f = f.clone();
                let list = oo.force(list_v.clone(), ctx);
                let items = extract_list_items(&list);
                let mut result: Vec<Value> = Vec::new();
                for item in items {
                    let sub = oo.apply_morphism(f.clone(), item, ctx);
                    let sub_forced = oo.force(sub, ctx);
                    let sub_items = extract_list_items(&sub_forced);
                    result.extend(sub_items);
                }
                return build_list_value(result);
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // ── Phase 18: list.any ────────────────────────────────────────

    m.insert("list.any".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(pred_f), Some(list_v)) = (c.get_field("0"), c.get_field("1")) {
                let pred_f = pred_f.clone();
                let list = oo.force(list_v.clone(), ctx);
                let items = extract_list_items(&list);
                for item in items {
                    let result = oo.apply_morphism(pred_f.clone(), item, ctx);
                    if result.to_string_plain().trim_start_matches('#') == "true" {
                        return Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None);
                    }
                }
                return Value::Atom(AtomKind::Tag("false".to_string()), EffectTag::Pure, None);
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // ── Phase 18: list.all ────────────────────────────────────────

    m.insert("list.all".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(pred_f), Some(list_v)) = (c.get_field("0"), c.get_field("1")) {
                let pred_f = pred_f.clone();
                let list = oo.force(list_v.clone(), ctx);
                let items = extract_list_items(&list);
                for item in items {
                    let result = oo.apply_morphism(pred_f.clone(), item, ctx);
                    if result.to_string_plain().trim_start_matches('#') != "true" {
                        return Value::Atom(AtomKind::Tag("false".to_string()), EffectTag::Pure, None);
                    }
                }
                return Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None);
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // ── Phase 18: list.find ───────────────────────────────────────

    m.insert("list.find".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let none_val = Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
        if let Value::Combo(ref c) = arg {
            if let (Some(pred_f), Some(list_v)) = (c.get_field("0"), c.get_field("1")) {
                let pred_f = pred_f.clone();
                let list = oo.force(list_v.clone(), ctx);
                let items = extract_list_items(&list);
                for item in items {
                    let result = oo.apply_morphism(pred_f.clone(), item.clone(), ctx);
                    if result.to_string_plain().trim_start_matches('#') == "true" {
                        let mut fields = IndexMap::new();
                        fields.insert("%val".to_string(), item);
                        return Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
                    }
                }
                return none_val;
            }
        }
        none_val
    }) as Arc<BuiltinFn>);

    // ── Phase 18: list.head ───────────────────────────────────────

    m.insert("list.head".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let none_val = Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
        let list = if let Value::Combo(ref c) = arg {
            // If arg has %kind: #list, it IS the list. Otherwise check "0" field.
            if c.get_field("%kind").map(|k| k.to_string_plain().trim_start_matches('#') == "list").unwrap_or(false) {
                oo.force(arg, ctx)
            } else {
                oo.force(c.get_field("0").cloned().unwrap_or_else(|| arg.clone()), ctx)
            }
        } else { oo.force(arg, ctx) };
        let items = extract_list_items(&list);
        if items.is_empty() {
            return none_val;
        }
        let mut fields = IndexMap::new();
        fields.insert("%val".to_string(), items[0].clone());
        Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]))
    }) as Arc<BuiltinFn>);

    // ── Phase 18: list.tail ───────────────────────────────────────

    m.insert("list.tail".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let list = if let Value::Combo(ref c) = arg {
            if c.get_field("%kind").map(|k| k.to_string_plain().trim_start_matches('#') == "list").unwrap_or(false) {
                oo.force(arg, ctx)
            } else {
                oo.force(c.get_field("0").cloned().unwrap_or_else(|| arg.clone()), ctx)
            }
        } else { oo.force(arg, ctx) };
        let items = extract_list_items(&list);
        if items.len() <= 1 {
            return build_list_value(vec![]);
        }
        build_list_value(items[1..].to_vec())
    }) as Arc<BuiltinFn>);

    // ── Phase 18: list.take ───────────────────────────────────────

    m.insert("list.take".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vn), Some(vlist)) = (c.get_field("0"), c.get_field("1")) {
                let n_forced = oo.force(vn.clone(), ctx);
                let list = oo.force(vlist.clone(), ctx);
                if let Value::Atom(AtomKind::Int(ref n), _, _) = n_forced {
                    let n = n.to_usize().unwrap_or(0);
                    let items = extract_list_items(&list);
                    let taken = items.into_iter().take(n).collect();
                    return build_list_value(taken);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // ── Phase 18: list.drop ───────────────────────────────────────

    m.insert("list.drop".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vn), Some(vlist)) = (c.get_field("0"), c.get_field("1")) {
                let n_forced = oo.force(vn.clone(), ctx);
                let list = oo.force(vlist.clone(), ctx);
                if let Value::Atom(AtomKind::Int(ref n), _, _) = n_forced {
                    let n = n.to_usize().unwrap_or(0);
                    let items = extract_list_items(&list);
                    let dropped = items.into_iter().skip(n).collect();
                    return build_list_value(dropped);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
}