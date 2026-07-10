use std::sync::Arc;
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, BottomCause, BottomDetail, ComboVal};
use nlang_parser::ast::AtomKind;

fn val_eq(a: &Value, b: &Value) -> bool {
    format!("{:?}", a) == format!("{:?}", b)
}

fn extract_items(v: &Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Vec<Value> {
    if let Value::Combo(c) = v {
        let mut out = Vec::new();
        for i in 0u32.. {
            match c.get_field(&i.to_string()) {
                Some(v) => out.push(oo.force(v.clone(), ctx)),
                None => break,
            }
        }
        out
    } else { vec![] }
}

fn build_set(items: Vec<Value>) -> Value {
    let mut seen: Vec<Value> = Vec::new();
    for item in items {
        if !seen.iter().any(|s| val_eq(s, &item)) {
            seen.push(item);
        }
    }
    let mut m = IndexMap::new();
    m.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    for (i, v) in seen.iter().enumerate() { m.insert(i.to_string(), v.clone()); }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn contains_val(set: &[Value], v: &Value) -> bool {
    set.iter().any(|s| val_eq(s, v))
}

fn bool_tag(b: bool) -> Value {
    Value::Atom(AtomKind::Tag(if b { "true" } else { "false" }.to_string()), EffectTag::Pure, None)
}

fn get_two_args(arg: &Value, oo: &Ouroboros, ctx: &mut EvalContext) -> (Value, Value) {
    let c = match arg { Value::Combo(ref c) => c.clone(), _ => return (Value::Top, Value::Top) };
    let a = oo.force(c.get_field("0").cloned().unwrap_or(Value::Top), ctx);
    let b = oo.force(c.get_field("1").cloned().unwrap_or(Value::Top), ctx);
    (a, b)
}

pub fn register_set_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert("set.from_list".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let list = oo.force(arg, ctx);
        build_set(extract_items(&list, oo, ctx))
    }) as Arc<BuiltinFn>);

    m.insert("set.union".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let (a, b) = get_two_args(&arg, oo, ctx);
        let mut items = extract_items(&a, oo, ctx);
        items.extend(extract_items(&b, oo, ctx));
        build_set(items)
    }) as Arc<BuiltinFn>);

    m.insert("set.intersection".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let (a, b) = get_two_args(&arg, oo, ctx);
        let items_a = extract_items(&a, oo, ctx);
        let items_b = extract_items(&b, oo, ctx);
        build_set(items_a.into_iter().filter(|v| contains_val(&items_b, v)).collect())
    }) as Arc<BuiltinFn>);

    m.insert("set.difference".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let (a, b) = get_two_args(&arg, oo, ctx);
        let items_a = extract_items(&a, oo, ctx);
        let items_b = extract_items(&b, oo, ctx);
        build_set(items_a.into_iter().filter(|v| !contains_val(&items_b, v)).collect())
    }) as Arc<BuiltinFn>);

    m.insert("set.is_subset".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let (a, b) = get_two_args(&arg, oo, ctx);
        let items_a = extract_items(&a, oo, ctx);
        let items_b = extract_items(&b, oo, ctx);
        bool_tag(items_a.iter().all(|v| contains_val(&items_b, v)))
    }) as Arc<BuiltinFn>);

    m.insert("set.is_superset".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let (a, b) = get_two_args(&arg, oo, ctx);
        let items_a = extract_items(&a, oo, ctx);
        let items_b = extract_items(&b, oo, ctx);
        bool_tag(items_b.iter().all(|v| contains_val(&items_a, v)))
    }) as Arc<BuiltinFn>);

    m.insert("set.is_disjoint".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let (a, b) = get_two_args(&arg, oo, ctx);
        let items_a = extract_items(&a, oo, ctx);
        let items_b = extract_items(&b, oo, ctx);
        bool_tag(!items_a.iter().any(|v| contains_val(&items_b, v)))
    }) as Arc<BuiltinFn>);

    m.insert("set.contains".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let (set_val, elem) = get_two_args(&arg, oo, ctx);
        let items = extract_items(&set_val, oo, ctx);
        bool_tag(contains_val(&items, &elem))
    }) as Arc<BuiltinFn>);
}
