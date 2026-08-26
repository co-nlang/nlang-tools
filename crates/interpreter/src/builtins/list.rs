use crate::value::{ComboVal, EffectTag, Value};
use crate::{BuiltinFn, EvalContext, Ouroboros};
use indexmap::IndexMap;
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive};
use std::collections::HashMap;
use std::sync::Arc;

pub fn register_list_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert(
        "list.len".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let target = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            let flist = oo.force(target, ctx);
            if let Value::Combo(ref cv) = flist.collapse() {
                let count = cv
                    .fields()
                    .keys()
                    .filter(|k| k.parse::<usize>().is_ok())
                    .count();
                return Value::Atom(AtomKind::Int(BigInt::from(count)), EffectTag::Pure, None);
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "list.at".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vidx), Some(vlist)) = (c.get_field("0"), c.get_field("1")) {
                    let fidx = oo.force(vidx.clone(), ctx).collapse().clone();
                    let flist = oo.force(vlist.clone(), ctx).collapse().clone();
                    if let (Value::Atom(AtomKind::Int(idx), _, _), Value::Combo(lc)) = (fidx, flist)
                    {
                        if let Some(v) = lc.get_field(&idx.to_string()) {
                            return v.clone();
                        }
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "list.concat".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(v0), Some(v1)) = (c.get_field("0"), c.get_field("1")) {
                    let f0 = oo.force(v0.clone(), ctx).collapse().clone();
                    let f1 = oo.force(v1.clone(), ctx).collapse().clone();
                    if let (Value::Combo(c0), Value::Combo(c1)) = (f0, f1) {
                        let mut res = IndexMap::new();
                        let mut count = 0;
                        for (_k, v) in &c0.fields() {
                            if _k.parse::<usize>().is_ok() {
                                res.insert(count.to_string(), v.clone());
                                count += 1;
                            }
                        }
                        for (_k, v) in &c1.fields() {
                            if _k.parse::<usize>().is_ok() {
                                res.insert(count.to_string(), v.clone());
                                count += 1;
                            }
                        }
                        res.insert(
                            "%kind".to_string(),
                            Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
                        );
                        return Value::Combo(ComboVal::new(
                            res,
                            false,
                            IndexMap::new(),
                            c0.effect.union(c1.effect),
                            vec![],
                        ));
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "list.reverse".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            let fv = oo.force(v, ctx).collapse().clone();
            if let Value::Combo(c) = fv {
                let mut items = Vec::new();
                let mut i = 0;
                while let Some(v) = c.get_field(&i.to_string()) {
                    items.push(v.clone());
                    i += 1;
                }
                items.reverse();
                let mut res = IndexMap::new();
                for (idx, v) in items.into_iter().enumerate() {
                    res.insert(idx.to_string(), v);
                }
                res.insert(
                    "%kind".to_string(),
                    Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
                );
                return Value::Combo(ComboVal::new(res, false, IndexMap::new(), c.effect, vec![]));
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "list.slice".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vstart), Some(vend), Some(vl)) =
                    (c.get_field("0"), c.get_field("1"), c.get_field("2"))
                {
                    let fs = oo.force(vstart.clone(), ctx).collapse().clone();
                    let fe = oo.force(vend.clone(), ctx).collapse().clone();
                    let fl = oo.force(vl.clone(), ctx).collapse().clone();
                    if let (
                        Value::Atom(AtomKind::Int(s), _, _),
                        Value::Atom(AtomKind::Int(e), _, _),
                        Value::Combo(lc),
                    ) = (fs, fe, fl)
                    {
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
                        res.insert(
                            "%kind".to_string(),
                            Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
                        );
                        return Value::Combo(ComboVal::new(
                            res,
                            false,
                            IndexMap::new(),
                            lc.effect,
                            vec![],
                        ));
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "list.zip".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vl1), Some(vl2)) = (c.get_field("0"), c.get_field("1")) {
                    let fl1 = oo.force(vl1.clone(), ctx).collapse().clone();
                    let fl2 = oo.force(vl2.clone(), ctx).collapse().clone();
                    if let (Value::Combo(c1), Value::Combo(c2)) = (fl1, fl2) {
                        let mut res = IndexMap::new();
                        let mut i = 0;
                        while let (Some(v1), Some(v2)) =
                            (c1.get_field(&i.to_string()), c2.get_field(&i.to_string()))
                        {
                            let mut tuple = IndexMap::new();
                            tuple.insert("0".to_string(), v1.clone());
                            tuple.insert("1".to_string(), v2.clone());
                            res.insert(
                                i.to_string(),
                                Value::Combo(ComboVal::new(
                                    tuple,
                                    true,
                                    IndexMap::new(),
                                    v1.effect().union(v2.effect()),
                                    vec![],
                                )),
                            );
                            i += 1;
                        }
                        res.insert(
                            "%kind".to_string(),
                            Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
                        );
                        return Value::Combo(ComboVal::new(
                            res,
                            false,
                            IndexMap::new(),
                            c1.effect.union(c2.effect),
                            vec![],
                        ));
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "list.sort".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            let fv = oo.force(v, ctx).collapse().clone();
            if let Value::Combo(c) = fv {
                let mut items = Vec::new();
                let mut i = 0;
                while let Some(v) = c.get_field(&i.to_string()) {
                    items.push(oo.force(v.clone(), ctx));
                    i += 1;
                }
                items.sort_by(|a, b| {
                    let sa = a.to_string_plain();
                    let sb = b.to_string_plain();
                    sa.cmp(&sb)
                });
                let mut res = IndexMap::new();
                for (idx, v) in items.into_iter().enumerate() {
                    res.insert(idx.to_string(), v);
                }
                res.insert(
                    "%kind".to_string(),
                    Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
                );
                return Value::Combo(ComboVal::new(res, false, IndexMap::new(), c.effect, vec![]));
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "list.map".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(v0), Some(v1)) = (c.get_field("0"), c.get_field("1")) {
                    // Declared: 0 = morphism, 1 = list. No sniffing (O78).
                    let fv = oo.force(v0.clone(), ctx);
                    let lv = oo.force(v1.clone(), ctx);
                    if !oo.is_list(&lv, ctx) {
                        return Value::Top;
                    }

                    if let (Value::Combo(lc), f) = (lv.collapse().clone(), fv) {
                        let mut res = IndexMap::new();
                        let mut max_e = f.effect();
                        for (k, v) in &lc.fields() {
                            if k.parse::<usize>().is_ok() {
                                let item = oo.force(v.clone(), ctx);
                                let mapped = oo.apply_morphism(f.clone(), item, ctx);
                                let solidified = oo.force_recursive(mapped, ctx);
                                max_e = max_e.union(solidified.effect());
                                res.insert(k.clone(), solidified);
                            }
                        }
                        for (k, v) in &lc.fields() {
                            if !k.parse::<usize>().is_ok() {
                                res.insert(k.clone(), v.clone());
                            }
                        }
                        let mut out = lc.clone();
                        for (k, v) in res {
                            out.insert_field(&k, v);
                        }
                        out.effect = max_e;
                        return Value::Combo(out);
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "list.fold".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(v0), Some(v1)) = (c.get_field("0"), c.get_field("1")) {
                    // Declared: 0 = { %val: seed, %f: morphism }, 1 = list.
                    // Data last so `xs |> /fold { %val, %f }` fills slot 1.
                    let rec = oo.force(v0.clone(), ctx);
                    let lv = oo.force(v1.clone(), ctx);
                    if !oo.is_list(&lv, ctx) {
                        return Value::Top;
                    }

                    if let (Value::Combo(lc), Value::Combo(fc)) =
                        (lv.collapse().clone(), rec.clone())
                    {
                        let mut acc = fc.get_field("%val").cloned().unwrap_or(Value::Top);
                        acc = oo.force(acc, ctx);
                        let f = fc.get_field("%f").cloned().unwrap_or(Value::Top);
                        if f.is_top() {
                            return acc;
                        }
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
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "list.filter".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(v0), Some(v1)) = (c.get_field("0"), c.get_field("1")) {
                    // Declared: 0 = predicate, 1 = list. No sniffing (O78).
                    let fv = oo.force(v0.clone(), ctx);
                    let lv = oo.force(v1.clone(), ctx);
                    if !oo.is_list(&lv, ctx) {
                        return Value::Top;
                    }
                    if let (Value::Combo(lc), f) = (lv.collapse().clone(), fv) {
                        let mut res = IndexMap::new();
                        let mut count = 0;
                        let mut max_e = lc.effect.union(f.effect());
                        for (k, v) in &lc.fields() {
                            if k.parse::<usize>().is_ok() {
                                let item = oo.force(v.clone(), ctx);
                                let pred = oo.apply_morphism(f.clone(), item.clone(), ctx);
                                max_e = max_e.union(pred.effect());
                                match super::pred_is_true(&pred) {
                                    Err(bottom) => return bottom,
                                    Ok(true) => {
                                        res.insert(count.to_string(), item);
                                        count += 1;
                                    }
                                    Ok(false) => {}
                                }
                            }
                        }
                        res.insert(
                            "%kind".to_string(),
                            Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
                        );
                        return Value::Combo(ComboVal::new(
                            res,
                            false,
                            IndexMap::new(),
                            max_e,
                            vec![],
                        ));
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // ── Phase 17: list.flat_map ───────────────────────────────────

    fn extract_list_items(list: &Value) -> Vec<Value> {
        let mut items = Vec::new();
        if let Value::Combo(ref lc) = list {
            let len = lc
                .get_field("%len")
                .and_then(|v| {
                    if let Value::Atom(AtomKind::Int(n), _, _) = v {
                        n.to_usize()
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| {
                    lc.fields()
                        .keys()
                        .filter_map(|k| k.parse::<usize>().ok())
                        .count()
                });
            for i in 0..len {
                if let Some(v) = lc.get_field(&i.to_string()) {
                    items.push(v.clone());
                }
            }
        }
        items
    }

    fn build_list_value(items: Vec<Value>) -> Value {
        let mut out = ComboVal::default();
        for (i, v) in items.iter().enumerate() {
            out.insert_field(&i.to_string(), v.clone());
        }
        out.insert_field(
            "%kind",
            Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
        );
        out.insert_field(
            "%len",
            Value::Atom(
                AtomKind::Int(BigInt::from(items.len())),
                EffectTag::Pure,
                None,
            ),
        );
        Value::Combo(out)
    }

    m.insert(
        "list.flat_map".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(f), Some(list_v)) = (c.get_field("0"), c.get_field("1")) {
                    let f = f.clone();
                    let list = oo.force(list_v.clone(), ctx);
                    if !oo.is_list(&list, ctx) {
                        return Value::Top;
                    }
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
        }) as Arc<BuiltinFn>,
    );

    // ── Phase 18: list.any ────────────────────────────────────────

    m.insert(
        "list.any".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(pred_f), Some(list_v)) = (c.get_field("0"), c.get_field("1")) {
                    let pred_f = pred_f.clone();
                    let list = oo.force(list_v.clone(), ctx);
                    if !oo.is_list(&list, ctx) {
                        return Value::Top;
                    }
                    let items = extract_list_items(&list);
                    for item in items {
                        let result = oo.apply_morphism(pred_f.clone(), item, ctx);
                        match super::pred_is_true(&result) {
                            Err(bottom) => return bottom,
                            Ok(true) => {
                                return Value::Atom(
                                    AtomKind::Tag("true".to_string()),
                                    EffectTag::Pure,
                                    None,
                                );
                            }
                            Ok(false) => {}
                        }
                    }
                    return Value::Atom(AtomKind::Tag("false".to_string()), EffectTag::Pure, None);
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // ── Phase 18: list.all ────────────────────────────────────────

    m.insert(
        "list.all".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(pred_f), Some(list_v)) = (c.get_field("0"), c.get_field("1")) {
                    let pred_f = pred_f.clone();
                    let list = oo.force(list_v.clone(), ctx);
                    if !oo.is_list(&list, ctx) {
                        return Value::Top;
                    }
                    let items = extract_list_items(&list);
                    for item in items {
                        let result = oo.apply_morphism(pred_f.clone(), item, ctx);
                        match super::pred_is_true(&result) {
                            Err(bottom) => return bottom,
                            Ok(true) => {}
                            Ok(false) => {
                                return Value::Atom(
                                    AtomKind::Tag("false".to_string()),
                                    EffectTag::Pure,
                                    None,
                                );
                            }
                        }
                    }
                    return Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None);
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // ── Phase 18: list.find ───────────────────────────────────────

    m.insert(
        "list.find".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let none_val = Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
            if let Value::Combo(ref c) = arg {
                if let (Some(pred_f), Some(list_v)) = (c.get_field("0"), c.get_field("1")) {
                    let pred_f = pred_f.clone();
                    let list = oo.force(list_v.clone(), ctx);
                    if !oo.is_list(&list, ctx) {
                        return Value::Top;
                    }
                    let items = extract_list_items(&list);
                    for item in items {
                        let result = oo.apply_morphism(pred_f.clone(), item.clone(), ctx);
                        match super::pred_is_true(&result) {
                            Err(bottom) => return bottom,
                            Ok(true) => {
                                let mut fields = IndexMap::new();
                                fields.insert("%val".to_string(), item);
                                return Value::Combo(ComboVal::new(
                                    fields,
                                    false,
                                    IndexMap::new(),
                                    EffectTag::Pure,
                                    vec![],
                                ));
                            }
                            Ok(false) => {}
                        }
                    }
                    return none_val;
                }
            }
            none_val
        }) as Arc<BuiltinFn>,
    );

    // ── Phase 18: list.head ───────────────────────────────────────

    m.insert(
        "list.head".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let none_val = Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
            let list = if let Value::Combo(ref c) = arg {
                // If arg has %kind: #list, it IS the list. Otherwise check "0" field.
                if c.get_field("%kind")
                    .map(|k| k.to_string_plain().trim_start_matches('#') == "list")
                    .unwrap_or(false)
                {
                    oo.force(arg, ctx)
                } else {
                    oo.force(
                        c.get_field("0").cloned().unwrap_or_else(|| arg.clone()),
                        ctx,
                    )
                }
            } else {
                oo.force(arg, ctx)
            };
            let items = extract_list_items(&list);
            if items.is_empty() {
                return none_val;
            }
            let mut fields = IndexMap::new();
            fields.insert("%val".to_string(), items[0].clone());
            Value::Combo(ComboVal::new(
                fields,
                false,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            ))
        }) as Arc<BuiltinFn>,
    );

    // ── Phase 18: list.tail ───────────────────────────────────────

    m.insert(
        "list.tail".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let list = if let Value::Combo(ref c) = arg {
                if c.get_field("%kind")
                    .map(|k| k.to_string_plain().trim_start_matches('#') == "list")
                    .unwrap_or(false)
                {
                    oo.force(arg, ctx)
                } else {
                    oo.force(
                        c.get_field("0").cloned().unwrap_or_else(|| arg.clone()),
                        ctx,
                    )
                }
            } else {
                oo.force(arg, ctx)
            };
            let items = extract_list_items(&list);
            if items.len() <= 1 {
                return build_list_value(vec![]);
            }
            build_list_value(items[1..].to_vec())
        }) as Arc<BuiltinFn>,
    );

    // ── Phase 18: list.take ───────────────────────────────────────

    m.insert(
        "list.take".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
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
        }) as Arc<BuiltinFn>,
    );

    // ── Phase 18: list.drop ───────────────────────────────────────

    m.insert(
        "list.drop".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
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
        }) as Arc<BuiltinFn>,
    );

    // ── Phase 19: List count ─────────────────────────────────────

    m.insert(
        "list.count".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(pred_f), Some(list_v)) = (c.get_field("0"), c.get_field("1")) {
                    let pred_f = pred_f.clone();
                    let list = oo.force(list_v.clone(), ctx);
                    if !oo.is_list(&list, ctx) {
                        return Value::Top;
                    }
                    let items = extract_list_items(&list);
                    let mut count: usize = 0;
                    for item in items {
                        let result = oo.apply_morphism(pred_f.clone(), item, ctx);
                        match super::pred_is_true(&result) {
                            Err(bottom) => return bottom,
                            Ok(true) => count += 1,
                            Ok(false) => {}
                        }
                    }
                    return Value::Atom(AtomKind::Int(BigInt::from(count)), EffectTag::Pure, None);
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // ── Phase 19: List zip_with ──────────────────────────────────

    m.insert(
        "list.zip_with".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(f), Some(va), Some(vb)) =
                    (c.get_field("0"), c.get_field("1"), c.get_field("2"))
                {
                    let f = f.clone();
                    let list_a = oo.force(va.clone(), ctx);
                    let list_b = oo.force(vb.clone(), ctx);
                    let items_a = extract_list_items(&list_a);
                    let items_b = extract_list_items(&list_b);
                    let min_len = items_a.len().min(items_b.len());
                    let mut result: Vec<Value> = Vec::with_capacity(min_len);
                    for i in 0..min_len {
                        let mut pair_fields = IndexMap::new();
                        pair_fields.insert("0".to_string(), items_a[i].clone());
                        pair_fields.insert("1".to_string(), items_b[i].clone());
                        let pair = Value::Combo(ComboVal::new(
                            pair_fields,
                            false,
                            IndexMap::new(),
                            EffectTag::Pure,
                            vec![],
                        ));
                        let mapped = oo.apply_morphism(f.clone(), pair, ctx);
                        result.push(mapped);
                    }
                    return build_list_value(result);
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // ── Phase 22: List extras ─────────────────────────────────────

    m.insert(
        "list.partition".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(pred_f), Some(list_v)) = (c.get_field("0"), c.get_field("1")) {
                    let pred_f = pred_f.clone();
                    let list = oo.force(list_v.clone(), ctx);
                    if !oo.is_list(&list, ctx) {
                        return Value::Top;
                    }
                    let items = extract_list_items(&list);
                    let mut yes_items: Vec<Value> = Vec::new();
                    let mut no_items: Vec<Value> = Vec::new();
                    for item in items {
                        let result = oo.apply_morphism(pred_f.clone(), item.clone(), ctx);
                        match super::pred_is_true(&result) {
                            Err(bottom) => return bottom,
                            Ok(true) => yes_items.push(item),
                            Ok(false) => no_items.push(item),
                        }
                    }
                    let mut out = ComboVal::default();
                    out.insert_field("yes", build_list_value(yes_items));
                    out.insert_field("no", build_list_value(no_items));
                    return Value::Combo(out);
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "list.flatten".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let outer = if let Value::Combo(ref c) = arg {
                if c.get_field("%kind")
                    .map(|k| k.to_string_plain().trim_start_matches('#') == "list")
                    .unwrap_or(false)
                {
                    oo.force(arg, ctx)
                } else {
                    oo.force(
                        c.get_field("0").cloned().unwrap_or_else(|| arg.clone()),
                        ctx,
                    )
                }
            } else {
                oo.force(arg, ctx)
            };
            let outer_items = extract_list_items(&outer);
            let mut result: Vec<Value> = Vec::new();
            for item in outer_items {
                let item_forced = oo.force(item.clone(), ctx);
                if oo.is_list(&item_forced, ctx) {
                    let inner = extract_list_items(&item_forced);
                    result.extend(inner);
                } else {
                    result.push(item_forced);
                }
            }
            build_list_value(result)
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "list.sum".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let list = if let Value::Combo(ref c) = arg {
                if c.get_field("%kind")
                    .map(|k| k.to_string_plain().trim_start_matches('#') == "list")
                    .unwrap_or(false)
                {
                    oo.force(arg, ctx)
                } else {
                    oo.force(
                        c.get_field("0").cloned().unwrap_or_else(|| arg.clone()),
                        ctx,
                    )
                }
            } else {
                oo.force(arg, ctx)
            };
            let items = extract_list_items(&list);
            let mut int_sum = BigInt::from(0i64);
            let mut float_sum: f64 = 0.0;
            let mut has_float = false;
            for item in items {
                match oo.force(item, ctx).collapse().clone() {
                    Value::Atom(AtomKind::Int(n), _, _) => {
                        if has_float {
                            float_sum += n.to_f64().unwrap_or(0.0);
                        } else {
                            int_sum += n;
                        }
                    }
                    Value::Atom(AtomKind::Float(f), _, _) => {
                        if !has_float {
                            float_sum = int_sum.to_f64().unwrap_or(0.0);
                            has_float = true;
                        }
                        float_sum += f;
                    }
                    _ => {}
                }
            }
            if has_float {
                Value::Atom(AtomKind::Float(float_sum), EffectTag::Pure, None)
            } else {
                Value::Atom(AtomKind::Int(int_sum), EffectTag::Pure, None)
            }
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "list.min_by".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(key_f), Some(list_v)) = (c.get_field("0"), c.get_field("1")) {
                    let key_f = key_f.clone();
                    let list = oo.force(list_v.clone(), ctx);
                    if !oo.is_list(&list, ctx) {
                        return Value::Top;
                    }
                    let items = extract_list_items(&list);
                    let mut best_elem: Option<Value> = None;
                    let mut best_key: f64 = f64::INFINITY;
                    for item in items {
                        let k = oo.apply_morphism(key_f.clone(), item.clone(), ctx);
                        let kf = match k.collapse() {
                            Value::Atom(AtomKind::Float(f), _, _) => *f,
                            Value::Atom(AtomKind::Int(n), _, _) => {
                                n.to_f64().unwrap_or(f64::INFINITY)
                            }
                            _ => continue,
                        };
                        if kf < best_key {
                            best_key = kf;
                            best_elem = Some(item);
                        }
                    }
                    return best_elem.unwrap_or(Value::Top);
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "list.max_by".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(key_f), Some(list_v)) = (c.get_field("0"), c.get_field("1")) {
                    let key_f = key_f.clone();
                    let list = oo.force(list_v.clone(), ctx);
                    if !oo.is_list(&list, ctx) {
                        return Value::Top;
                    }
                    let items = extract_list_items(&list);
                    let mut best_elem: Option<Value> = None;
                    let mut best_key: f64 = f64::NEG_INFINITY;
                    for item in items {
                        let k = oo.apply_morphism(key_f.clone(), item.clone(), ctx);
                        let kf = match k.collapse() {
                            Value::Atom(AtomKind::Float(f), _, _) => *f,
                            Value::Atom(AtomKind::Int(n), _, _) => {
                                n.to_f64().unwrap_or(f64::NEG_INFINITY)
                            }
                            _ => continue,
                        };
                        if kf > best_key {
                            best_key = kf;
                            best_elem = Some(item);
                        }
                    }
                    return best_elem.unwrap_or(Value::Top);
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // list.unique: remove duplicates (first occurrence preserved)
    m.insert(
        "list.unique".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                if c.get_field("%kind")
                    .map(|k| k.to_string_plain().trim_start_matches('#') == "list")
                    .unwrap_or(false)
                {
                    oo.force(arg, ctx)
                } else {
                    oo.force(
                        c.get_field("0").cloned().unwrap_or_else(|| arg.clone()),
                        ctx,
                    )
                }
            } else {
                oo.force(arg, ctx)
            };
            let items = extract_list_items(&v);
            let mut seen = std::collections::HashSet::new();
            let mut out = Vec::new();
            for item in items {
                let forced = oo.force(item, ctx);
                let key = forced.to_nlang(0);
                if seen.insert(key) {
                    out.push(forced);
                }
            }
            build_list_value(out)
        }) as Arc<BuiltinFn>,
    );

    // list.range: [start, start+1, ..., end-1]
    m.insert(
        "list.range".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vs), Some(ve)) = (c.get_field("0"), c.get_field("1")) {
                    let fs = oo.force(vs.clone(), ctx);
                    let fe = oo.force(ve.clone(), ctx);
                    if let (
                        Value::Atom(AtomKind::Int(start), _, _),
                        Value::Atom(AtomKind::Int(end), _, _),
                    ) = (fs.collapse(), fe.collapse())
                    {
                        let mut items = Vec::new();
                        let mut i = start.clone();
                        while i < *end {
                            items.push(Value::Atom(
                                AtomKind::Int(i.clone()),
                                EffectTag::Pure,
                                None,
                            ));
                            i += 1;
                        }
                        return build_list_value(items);
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // list.reduce: fold with first element as initial accumulator
    m.insert(
        "list.reduce".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vf), Some(vl)) = (c.get_field("0"), c.get_field("1")) {
                    let func = vf.clone();
                    let list = oo.force(vl.clone(), ctx);
                    if !oo.is_list(&list, ctx) {
                        return Value::Top;
                    }
                    let items = extract_list_items(&list);
                    if items.is_empty() {
                        return Value::Top;
                    }
                    let mut acc = oo.force(items[0].clone(), ctx);
                    for item in items.into_iter().skip(1) {
                        let item_forced = oo.force(item, ctx);
                        let mut pair = IndexMap::new();
                        pair.insert("0".to_string(), acc);
                        pair.insert("1".to_string(), item_forced);
                        let pair_val = Value::Combo(ComboVal::new(
                            pair,
                            true,
                            IndexMap::new(),
                            EffectTag::Pure,
                            vec![],
                        ));
                        acc = oo.apply_morphism(func.clone(), pair_val, ctx);
                    }
                    return acc;
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // list.group_by: {0: key_fn, 1: list} → Combo { key → list }
    m.insert(
        "list.group_by".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vf), Some(vl)) = (c.get_field("0"), c.get_field("1")) {
                    let key_fn = vf.clone();
                    let list = oo.force(vl.clone(), ctx);
                    if !oo.is_list(&list, ctx) {
                        return Value::Top;
                    }
                    let items = extract_list_items(&list);
                    let mut groups: IndexMap<String, Vec<Value>> = IndexMap::new();
                    for item in items {
                        let item_forced = oo.force(item, ctx);
                        let key = oo.apply_morphism(key_fn.clone(), item_forced.clone(), ctx);
                        let key_str = key.collapse().to_string_plain();
                        groups
                            .entry(key_str)
                            .or_insert_with(Vec::new)
                            .push(item_forced);
                    }
                    let mut out = ComboVal::default();
                    for (key, group_items) in groups {
                        out.insert_field(&key, build_list_value(group_items));
                    }
                    return Value::Combo(out);
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // list.chunk: {0: n, 1: list} → list of lists (each sub-list size n, last may be smaller)
    m.insert(
        "list.chunk".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vn), Some(vl)) = (c.get_field("0"), c.get_field("1")) {
                    let fn_ = oo.force(vn.clone(), ctx);
                    let list = oo.force(vl.clone(), ctx);
                    if let Value::Atom(AtomKind::Int(n), _, _) = fn_.collapse() {
                        let size = match n.to_usize() {
                            Some(s) if s > 0 => s,
                            _ => return Value::Top,
                        };
                        let items = extract_list_items(&list);
                        let chunks: Vec<Value> = items
                            .chunks(size)
                            .map(|chunk| build_list_value(chunk.to_vec()))
                            .collect();
                        return build_list_value(chunks);
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // list.window: {0: n, 1: list} → list of lists (sliding windows of size n)
    m.insert(
        "list.window".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vn), Some(vl)) = (c.get_field("0"), c.get_field("1")) {
                    let fn_ = oo.force(vn.clone(), ctx);
                    let list = oo.force(vl.clone(), ctx);
                    if let Value::Atom(AtomKind::Int(n), _, _) = fn_.collapse() {
                        let size = match n.to_usize() {
                            Some(s) if s > 0 => s,
                            _ => return Value::Top,
                        };
                        let items = extract_list_items(&list);
                        if items.len() < size {
                            return build_list_value(vec![]);
                        }
                        let windows: Vec<Value> = (0..=(items.len() - size))
                            .map(|i| build_list_value(items[i..i + size].to_vec()))
                            .collect();
                        return build_list_value(windows);
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // list.enumerate: {0: list} → list of {0: Int, 1: Value}
    m.insert(
        "list.enumerate".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            let list = oo.force(v, ctx);
            let items = extract_list_items(&list);
            let pairs: Vec<Value> = items
                .into_iter()
                .enumerate()
                .map(|(i, item)| {
                    let mut pair = IndexMap::new();
                    pair.insert(
                        "0".to_string(),
                        Value::Atom(AtomKind::Int(BigInt::from(i)), EffectTag::Pure, None),
                    );
                    pair.insert("1".to_string(), item);
                    Value::Combo(ComboVal::new(
                        pair,
                        false,
                        IndexMap::new(),
                        EffectTag::Pure,
                        vec![],
                    ))
                })
                .collect();
            build_list_value(pairs)
        }) as Arc<BuiltinFn>,
    );

    // list.sort_by: {0: cmp_fn, 1: list} → sorted list (stable sort)
    m.insert(
        "list.sort_by".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vf), Some(vl)) = (c.get_field("0"), c.get_field("1")) {
                    let cmp_fn = vf.clone();
                    let list = oo.force(vl.clone(), ctx);
                    if !oo.is_list(&list, ctx) {
                        return Value::Top;
                    }
                    let mut items = extract_list_items(&list);
                    items.sort_by(|a, b| {
                        let mut pair = IndexMap::new();
                        pair.insert("0".to_string(), a.clone());
                        pair.insert("1".to_string(), b.clone());
                        let pair_val = Value::Combo(ComboVal::new(
                            pair,
                            true,
                            IndexMap::new(),
                            EffectTag::Pure,
                            vec![],
                        ));
                        let result = oo.apply_morphism(cmp_fn.clone(), pair_val, ctx);
                        match result.collapse() {
                            Value::Atom(AtomKind::Int(n), _, _) if n.is_negative() => {
                                std::cmp::Ordering::Less
                            }
                            Value::Atom(AtomKind::Int(n), _, _) if n.is_positive() => {
                                std::cmp::Ordering::Greater
                            }
                            _ => std::cmp::Ordering::Equal,
                        }
                    });
                    return build_list_value(items);
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // list.dedup: {0: list} → list (remove consecutive duplicates, stable)
    m.insert(
        "list.dedup".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            let list = oo.force(v, ctx);
            let items = extract_list_items(&list);
            let mut out: Vec<Value> = Vec::new();
            let mut last_key: Option<String> = None;
            for item in items {
                let forced = oo.force(item, ctx);
                let key = forced.to_nlang(0);
                if Some(&key) != last_key.as_ref() {
                    last_key = Some(key);
                    out.push(forced);
                }
            }
            build_list_value(out)
        }) as Arc<BuiltinFn>,
    );

    // list.intersperse: {0: sep, 1: list} → list with sep inserted between elements
    m.insert(
        "list.intersperse".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vs), Some(vl)) = (c.get_field("0"), c.get_field("1")) {
                    let sep = vs.clone();
                    let list = oo.force(vl.clone(), ctx);
                    let items = extract_list_items(&list);
                    if items.len() <= 1 {
                        return build_list_value(items);
                    }
                    let mut out = Vec::new();
                    for (i, item) in items.into_iter().enumerate() {
                        if i > 0 {
                            out.push(sep.clone());
                        }
                        out.push(item);
                    }
                    return build_list_value(out);
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // ── Phase 45: List extras 2 ────────────────────────────────────

    // list.scan: {0: f, 1: init, 2: list} → @list  (data last, O78 ③)
    m.insert(
        "list.scan".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vf), Some(vi), Some(vl)) =
                    (c.get_field("0"), c.get_field("1"), c.get_field("2"))
                {
                    let morph = vf.clone();
                    let mut acc = oo.force(vi.clone(), ctx);
                    let list = oo.force(vl.clone(), ctx);
                    if !oo.is_list(&list, ctx) {
                        return Value::Top;
                    }
                    let items = extract_list_items(&list);
                    let mut result: Vec<Value> = Vec::new();
                    for item in items {
                        let f_acc = oo.apply_morphism(morph.clone(), acc, ctx);
                        let new_acc = oo.apply_morphism(f_acc, item, ctx);
                        acc = oo.force_recursive(new_acc, ctx);
                        result.push(acc.clone());
                    }
                    return build_list_value(result);
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // list.take_while: {0: pred, 1: list} → @list  (data last, O78 ③)
    m.insert(
        "list.take_while".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vp), Some(vl)) = (c.get_field("0"), c.get_field("1")) {
                    let pred = vp.clone();
                    let list = oo.force(vl.clone(), ctx);
                    if !oo.is_list(&list, ctx) {
                        return Value::Top;
                    }
                    let items = extract_list_items(&list);
                    let mut result: Vec<Value> = Vec::new();
                    for item in items {
                        let p = oo.apply_morphism(pred.clone(), item.clone(), ctx);
                        match super::pred_is_true(&p) {
                            Err(bottom) => return bottom,
                            Ok(true) => result.push(item),
                            Ok(false) => break,
                        }
                    }
                    return build_list_value(result);
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // list.drop_while: {0: pred, 1: list} → @list  (data last, O78 ③)
    m.insert(
        "list.drop_while".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vp), Some(vl)) = (c.get_field("0"), c.get_field("1")) {
                    let pred = vp.clone();
                    let list = oo.force(vl.clone(), ctx);
                    if !oo.is_list(&list, ctx) {
                        return Value::Top;
                    }
                    let items = extract_list_items(&list);
                    let mut dropping = true;
                    let mut result: Vec<Value> = Vec::new();
                    for item in items {
                        if dropping {
                            let p = oo.apply_morphism(pred.clone(), item.clone(), ctx);
                            match super::pred_is_true(&p) {
                                Err(bottom) => return bottom,
                                Ok(true) => continue,
                                Ok(false) => dropping = false,
                            }
                        }
                        result.push(item);
                    }
                    return build_list_value(result);
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // list.product: {0: list} → Int/Float
    m.insert(
        "list.product".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let list = if let Value::Combo(ref c) = arg {
                if c.get_field("%kind")
                    .map(|k| k.to_string_plain().trim_start_matches('#') == "list")
                    .unwrap_or(false)
                {
                    oo.force(arg, ctx)
                } else {
                    oo.force(
                        c.get_field("0").cloned().unwrap_or_else(|| arg.clone()),
                        ctx,
                    )
                }
            } else {
                oo.force(arg, ctx)
            };
            let items = extract_list_items(&list);
            let mut int_prod = BigInt::from(1i64);
            let mut float_prod: f64 = 1.0;
            let mut has_float = false;
            for item in items {
                match oo.force(item, ctx).collapse().clone() {
                    Value::Atom(AtomKind::Int(n), _, _) => {
                        if has_float {
                            float_prod *= n.to_f64().unwrap_or(0.0);
                        } else {
                            int_prod *= n;
                        }
                    }
                    Value::Atom(AtomKind::Float(f), _, _) => {
                        if !has_float {
                            float_prod = int_prod.to_f64().unwrap_or(1.0);
                            has_float = true;
                        }
                        float_prod *= f;
                    }
                    _ => {}
                }
            }
            if has_float {
                Value::Atom(AtomKind::Float(float_prod), EffectTag::Pure, None)
            } else {
                Value::Atom(AtomKind::Int(int_prod), EffectTag::Pure, None)
            }
        }) as Arc<BuiltinFn>,
    );

    // list.transpose: {0: list} → @list of @list
    m.insert(
        "list.transpose".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let list_val = if let Value::Combo(ref c) = arg {
                if c.get_field("%kind")
                    .map(|k| k.to_string_plain().trim_start_matches('#') == "list")
                    .unwrap_or(false)
                {
                    oo.force(arg, ctx)
                } else {
                    oo.force(
                        c.get_field("0").cloned().unwrap_or_else(|| arg.clone()),
                        ctx,
                    )
                }
            } else {
                oo.force(arg, ctx)
            };
            let outer_items = extract_list_items(&list_val);
            if outer_items.is_empty() {
                return build_list_value(vec![]);
            }
            let mut rows: Vec<Vec<Value>> = Vec::new();
            for item in outer_items {
                let forced = oo.force(item, ctx);
                rows.push(extract_list_items(&forced));
            }
            if rows.is_empty() || rows.iter().any(|r| r.is_empty()) {
                return build_list_value(vec![]);
            }
            let min_cols = rows.iter().map(|r| r.len()).min().unwrap_or(0);
            let mut result: Vec<Value> = Vec::with_capacity(min_cols);
            for col in 0..min_cols {
                let mut col_items: Vec<Value> = Vec::with_capacity(rows.len());
                for row in &rows {
                    col_items.push(row[col].clone());
                }
                result.push(build_list_value(col_items));
            }
            build_list_value(result)
        }) as Arc<BuiltinFn>,
    );
}
