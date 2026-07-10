# Phase 46 Handover：Stdlib Round 3 — B1 + B2（~%Set + ~%Stat）

> 日期：2026-05-25  
> 實作範圍：~%Set（8 態射）、~%Stat（6 態射），零新依賴  
> 預期測試：~492 → ~504（新增 ~12 個測試，2 個測試檔）

---

## 0. 設計摘要

| 模組 | 態射 | 底層表示 | Effect |
|:-----|:-----|:---------|:------:|
| ~%Set | from_list, union, intersection, difference, is_subset, is_superset, is_disjoint, contains | 基於 @list（去重） | Pure |
| ~%Stat | mean, variance, std_dev, median, percentile, histogram | 對數值 @list | Pure |

**`~%Set` 的集合表示**：使用與 `@list` 相同的 Combo 格式 `{%kind:#list, 0:v0, 1:v1, ...}`，保證有序且去重。
相等性判定（用於去重）：與 Phase 44 `diff.rs` 的 `same_value` 相同策略（format Debug 或 BN/序列化）。

---

## 1. 新建 `crates/interpreter/src/builtins/set.rs`

```rust
use std::sync::Arc;
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, BottomCause, BottomDetail, ComboVal};
use nlang_parser::ast::AtomKind;

// ── Value equality (for set membership) ──────────────────────────────────────

fn val_eq(a: &Value, b: &Value) -> bool {
    // 與 diff.rs 的 same_value 相同；若已 pub，直接 use crate::builtins::diff::same_value
    format!("{:?}", a) == format!("{:?}", b)
}

// ── @list helpers ─────────────────────────────────────────────────────────────

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
    // Deduplicate preserving order
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

// ── Builtin registration ──────────────────────────────────────────────────────

pub fn register_set_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // set.from_list: {0: list} → set（去重）
    m.insert("set.from_list".to_string(), Arc::new(|arg, oo, ctx| {
        let list = oo.force(arg, ctx);  // 或從 {0: list}
        build_set(extract_items(&list, oo, ctx))
    }) as Arc<BuiltinFn>);

    // set.union: {0: a, 1: b} → a ∪ b
    m.insert("set.union".to_string(), Arc::new(|arg, oo, ctx| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
        let a = oo.force(c.get_field("0").cloned().unwrap_or(Value::Top), ctx);
        let b = oo.force(c.get_field("1").cloned().unwrap_or(Value::Top), ctx);
        let mut items = extract_items(&a, oo, ctx);
        items.extend(extract_items(&b, oo, ctx));
        build_set(items)
    }) as Arc<BuiltinFn>);

    // set.intersection: {0: a, 1: b} → a ∩ b
    m.insert("set.intersection".to_string(), Arc::new(|arg, oo, ctx| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
        let a = oo.force(c.get_field("0").cloned().unwrap_or(Value::Top), ctx);
        let b = oo.force(c.get_field("1").cloned().unwrap_or(Value::Top), ctx);
        let items_a = extract_items(&a, oo, ctx);
        let items_b = extract_items(&b, oo, ctx);
        let intersected: Vec<Value> = items_a.into_iter()
            .filter(|v| contains_val(&items_b, v))
            .collect();
        build_set(intersected)
    }) as Arc<BuiltinFn>);

    // set.difference: {0: a, 1: b} → a \ b（在 a 中但不在 b 中）
    m.insert("set.difference".to_string(), Arc::new(|arg, oo, ctx| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
        let a = oo.force(c.get_field("0").cloned().unwrap_or(Value::Top), ctx);
        let b = oo.force(c.get_field("1").cloned().unwrap_or(Value::Top), ctx);
        let items_a = extract_items(&a, oo, ctx);
        let items_b = extract_items(&b, oo, ctx);
        let diff: Vec<Value> = items_a.into_iter()
            .filter(|v| !contains_val(&items_b, v))
            .collect();
        build_set(diff)
    }) as Arc<BuiltinFn>);

    // set.is_subset: {0: a, 1: b} → #true if a ⊆ b
    m.insert("set.is_subset".to_string(), Arc::new(|arg, oo, ctx| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
        let a = oo.force(c.get_field("0").cloned().unwrap_or(Value::Top), ctx);
        let b = oo.force(c.get_field("1").cloned().unwrap_or(Value::Top), ctx);
        let items_a = extract_items(&a, oo, ctx);
        let items_b = extract_items(&b, oo, ctx);
        let is_sub = items_a.iter().all(|v| contains_val(&items_b, v));
        bool_tag(is_sub)
    }) as Arc<BuiltinFn>);

    // set.is_superset: {0: a, 1: b} → #true if a ⊇ b
    m.insert("set.is_superset".to_string(), Arc::new(|arg, oo, ctx| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
        let a = oo.force(c.get_field("0").cloned().unwrap_or(Value::Top), ctx);
        let b = oo.force(c.get_field("1").cloned().unwrap_or(Value::Top), ctx);
        let items_a = extract_items(&a, oo, ctx);
        let items_b = extract_items(&b, oo, ctx);
        let is_sup = items_b.iter().all(|v| contains_val(&items_a, v));
        bool_tag(is_sup)
    }) as Arc<BuiltinFn>);

    // set.is_disjoint: {0: a, 1: b} → #true if a ∩ b = ∅
    m.insert("set.is_disjoint".to_string(), Arc::new(|arg, oo, ctx| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
        let a = oo.force(c.get_field("0").cloned().unwrap_or(Value::Top), ctx);
        let b = oo.force(c.get_field("1").cloned().unwrap_or(Value::Top), ctx);
        let items_a = extract_items(&a, oo, ctx);
        let items_b = extract_items(&b, oo, ctx);
        let disjoint = !items_a.iter().any(|v| contains_val(&items_b, v));
        bool_tag(disjoint)
    }) as Arc<BuiltinFn>);

    // set.contains: {0: set, 1: elem} → #true if elem ∈ set
    m.insert("set.contains".to_string(), Arc::new(|arg, oo, ctx| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
        let set_val = oo.force(c.get_field("0").cloned().unwrap_or(Value::Top), ctx);
        let elem = oo.force(c.get_field("1").cloned().unwrap_or(Value::Top), ctx);
        let items = extract_items(&set_val, oo, ctx);
        bool_tag(contains_val(&items, &elem))
    }) as Arc<BuiltinFn>);
}

fn bool_tag(b: bool) -> Value {
    Value::Atom(AtomKind::Tag(if b { "true" } else { "false" }.to_string()), EffectTag::Pure, None)
}
```

---

## 2. 新建 `crates/interpreter/src/builtins/stat.rs`

```rust
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

fn conflict() -> Value { BottomCause::Conflict.into() }

pub fn register_stat_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // stat.mean: {0: list} → Float
    m.insert("stat.mean".to_string(), Arc::new(|arg, oo, ctx| {
        let v = oo.force(arg, ctx);
        let nums = match extract_floats(&v, oo, ctx) { Some(n) => n, None => return conflict() };
        if nums.is_empty() { return conflict(); }
        float_val(nums.iter().sum::<f64>() / nums.len() as f64)
    }) as Arc<BuiltinFn>);

    // stat.variance: {0: list} → Float（母體變異數：(Σ(x-μ)²)/n）
    m.insert("stat.variance".to_string(), Arc::new(|arg, oo, ctx| {
        let v = oo.force(arg, ctx);
        let nums = match extract_floats(&v, oo, ctx) { Some(n) => n, None => return conflict() };
        if nums.is_empty() { return conflict(); }
        let mean = nums.iter().sum::<f64>() / nums.len() as f64;
        let var = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / nums.len() as f64;
        float_val(var)
    }) as Arc<BuiltinFn>);

    // stat.std_dev: {0: list} → Float（母體標準差：√variance）
    m.insert("stat.std_dev".to_string(), Arc::new(|arg, oo, ctx| {
        let v = oo.force(arg, ctx);
        let nums = match extract_floats(&v, oo, ctx) { Some(n) => n, None => return conflict() };
        if nums.is_empty() { return conflict(); }
        let mean = nums.iter().sum::<f64>() / nums.len() as f64;
        let var = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / nums.len() as f64;
        float_val(var.sqrt())
    }) as Arc<BuiltinFn>);

    // stat.median: {0: list} → Float（中位數：排序後取中間值）
    m.insert("stat.median".to_string(), Arc::new(|arg, oo, ctx| {
        let v = oo.force(arg, ctx);
        let mut nums = match extract_floats(&v, oo, ctx) { Some(n) => n, None => return conflict() };
        if nums.is_empty() { return conflict(); }
        nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = nums.len();
        let median = if n % 2 == 1 { nums[n/2] }
                     else { (nums[n/2 - 1] + nums[n/2]) / 2.0 };
        float_val(median)
    }) as Arc<BuiltinFn>);

    // stat.percentile: {0: list, 1: p} → Float（0 ≤ p ≤ 100，線性插值）
    m.insert("stat.percentile".to_string(), Arc::new(|arg, oo, ctx| {
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
        // 線性插值法（nearest-rank 的連續版）
        let rank = p / 100.0 * (n - 1) as f64;
        let lo = rank.floor() as usize;
        let hi = rank.ceil() as usize;
        let frac = rank - lo as f64;
        let result = nums[lo] * (1.0 - frac) + nums[hi.min(n-1)] * frac;
        float_val(result)
    }) as Arc<BuiltinFn>);

    // stat.histogram: {0: list, 1: bins} → @list of @list（每個 bin 的元素）
    // bins 為 Int，將數據的 [min, max] 均分為 bins 個區間
    // 返回 @list of @list，每個 inner @list 包含落入該 bin 的原始值
    m.insert("stat.histogram".to_string(), Arc::new(|arg, oo, ctx| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return conflict() };
        let list_v = oo.force(c.get_field("0").cloned().unwrap_or(Value::Top), ctx);
        let bins_v = oo.force(c.get_field("1").cloned().unwrap_or(Value::Top), ctx);
        let bins = match bins_v {
            Value::Atom(AtomKind::Int(ref n), _, _) => n.to_usize().unwrap_or(1).max(1),
            _ => return conflict(),
        };
        let nums = match extract_floats(&list_v, oo, ctx) { Some(n) => n, None => return conflict() };
        if nums.is_empty() {
            // 返回 bins 個空 bin
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
        let bucket_lists: Vec<Value> = buckets.into_iter().map(build_list_vals).collect();
        build_list_vals(bucket_lists)
    }) as Arc<BuiltinFn>);
}

fn build_list_vals(items: Vec<Value>) -> Value {
    let mut m = IndexMap::new();
    m.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    for (i, v) in items.iter().enumerate() { m.insert(i.to_string(), v.clone()); }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
```

---

## 3. 修改 `crates/interpreter/src/builtins/mod.rs`

```rust
mod set;
mod stat;
```

在 `create_default_builtins()` 末尾：

```rust
    set::register_set_builtins(&mut m);
    stat::register_stat_builtins(&mut m);
```

---

## 4. 修改 `crates/interpreter/src/lib.rs`

在 `~%Diff` 區塊之後加入 `~%Set` 和 `~%Stat` 模組：

```rust
        // ~%Set module
        let mut set_fields = IndexMap::new();
        let smorph = |id: &str, eff: EffectTag| -> Value { /* 同 qmorph 風格 */ make_morph(id, eff) };
        set_fields.insert("/from_list".to_string(),    smorph("set.from_list",    EffectTag::Pure));
        set_fields.insert("/union".to_string(),         smorph("set.union",        EffectTag::Pure));
        set_fields.insert("/intersection".to_string(),  smorph("set.intersection", EffectTag::Pure));
        set_fields.insert("/difference".to_string(),    smorph("set.difference",   EffectTag::Pure));
        set_fields.insert("/is_subset".to_string(),     smorph("set.is_subset",    EffectTag::Pure));
        set_fields.insert("/is_superset".to_string(),   smorph("set.is_superset",  EffectTag::Pure));
        set_fields.insert("/is_disjoint".to_string(),   smorph("set.is_disjoint",  EffectTag::Pure));
        set_fields.insert("/contains".to_string(),      smorph("set.contains",     EffectTag::Pure));
        let set_module = Value::Combo(ComboVal::new(set_fields, true, IndexMap::new(), EffectTag::Pure, vec![]));
        root.insert_field("~%Set", set_module);

        // ~%Stat module
        let mut stat_fields = IndexMap::new();
        stat_fields.insert("/mean".to_string(),        smorph("stat.mean",        EffectTag::Pure));
        stat_fields.insert("/variance".to_string(),    smorph("stat.variance",    EffectTag::Pure));
        stat_fields.insert("/std_dev".to_string(),     smorph("stat.std_dev",     EffectTag::Pure));
        stat_fields.insert("/median".to_string(),      smorph("stat.median",      EffectTag::Pure));
        stat_fields.insert("/percentile".to_string(),  smorph("stat.percentile",  EffectTag::Pure));
        stat_fields.insert("/histogram".to_string(),   smorph("stat.histogram",   EffectTag::Pure));
        let stat_module = Value::Combo(ComboVal::new(stat_fields, true, IndexMap::new(), EffectTag::Pure, vec![]));
        root.insert_field("~%Stat", stat_module);
```

---

## 5. 修改 `crates/interpreter/src/genesis.rs`

加入兩個新模組的 seed（與 SEED_QUERY / SEED_DIFF 相同流程）：

```rust
pub const SEED_SET:  &str = "hash:sha256:v2:_:<lattice_sketch>:<digest>";
pub const SEED_STAT: &str = "hash:sha256:v2:_:<lattice_sketch>:<digest>";
```

在 `all_seeds()` 中：

```rust
seeds.push(("~%Set",  SEED_SET));
seeds.push(("~%Stat", SEED_STAT));
```

---

## 6. 測試

### 6.1 `tests/set_p46_test.rs`

```rust
// （helper functions 與前面 test 檔相同：oo, int_val, str_val, tag, combo, list_of, call, args2）

#[test] fn test_set_from_list_dedup() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let list = list_of(&[int_val(1), int_val(2), int_val(1), int_val(3)]);
    let r = call(&oo, &mut ctx, "set.from_list", list);
    assert_eq!(list_len(&r), 3, "duplicates removed");
}

#[test] fn test_set_union() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let a = list_of(&[int_val(1), int_val(2)]);
    let b = list_of(&[int_val(2), int_val(3)]);
    let r = call(&oo, &mut ctx, "set.union", args2(a, b));
    assert_eq!(list_len(&r), 3);
}

#[test] fn test_set_intersection() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let a = list_of(&[int_val(1), int_val(2), int_val(3)]);
    let b = list_of(&[int_val(2), int_val(3), int_val(4)]);
    let r = call(&oo, &mut ctx, "set.intersection", args2(a, b));
    assert_eq!(list_len(&r), 2);
}

#[test] fn test_set_difference() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let a = list_of(&[int_val(1), int_val(2), int_val(3)]);
    let b = list_of(&[int_val(2)]);
    let r = call(&oo, &mut ctx, "set.difference", args2(a, b));
    assert_eq!(list_len(&r), 2);
}

#[test] fn test_set_is_subset() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let a = list_of(&[int_val(1), int_val(2)]);
    let b = list_of(&[int_val(1), int_val(2), int_val(3)]);
    let r = call(&oo, &mut ctx, "set.is_subset", args2(a, b));
    assert!(matches!(&r, Value::Atom(AtomKind::Tag(t), _, _) if t == "true"));
    let r2 = call(&oo, &mut ctx, "set.is_subset", args2(b, a));
    assert!(matches!(&r2, Value::Atom(AtomKind::Tag(t), _, _) if t == "false"));
}

#[test] fn test_set_contains() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let s = list_of(&[int_val(1), int_val(2), int_val(3)]);
    let r = call(&oo, &mut ctx, "set.contains", args2(s.clone(), int_val(2)));
    assert!(matches!(&r, Value::Atom(AtomKind::Tag(t), _, _) if t == "true"));
    let r2 = call(&oo, &mut ctx, "set.contains", args2(s, int_val(5)));
    assert!(matches!(&r2, Value::Atom(AtomKind::Tag(t), _, _) if t == "false"));
}
```

### 6.2 `tests/stat_p46_test.rs`

```rust
fn float_list(nums: &[f64]) -> Value {
    let items: Vec<Value> = nums.iter().map(|&f| float_val(f)).collect();
    list_of(&items)
}

#[test] fn test_stat_mean() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let list = float_list(&[1.0, 2.0, 3.0, 4.0, 5.0]);
    let r = call(&oo, &mut ctx, "stat.mean", list);
    assert!(matches!(&r, Value::Atom(AtomKind::Float(f), _, _) if (f - 3.0).abs() < 1e-10));
}

#[test] fn test_stat_median_odd() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let list = float_list(&[3.0, 1.0, 5.0, 2.0, 4.0]);
    let r = call(&oo, &mut ctx, "stat.median", list);
    assert!(matches!(&r, Value::Atom(AtomKind::Float(f), _, _) if (f - 3.0).abs() < 1e-10));
}

#[test] fn test_stat_std_dev() {
    let oo = oo(); let mut ctx = oo.eval_context();
    // std_dev([2,4,4,4,5,5,7,9]) = 2.0 (population)
    let list = float_list(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
    let r = call(&oo, &mut ctx, "stat.std_dev", list);
    assert!(matches!(&r, Value::Atom(AtomKind::Float(f), _, _) if (f - 2.0).abs() < 1e-10));
}

#[test] fn test_stat_percentile_50() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let list = float_list(&[10.0, 20.0, 30.0, 40.0, 50.0]);
    let p = Value::Atom(AtomKind::Float(50.0), EffectTag::Pure, None);
    let r = call(&oo, &mut ctx, "stat.percentile", args2(list, p));
    assert!(matches!(&r, Value::Atom(AtomKind::Float(f), _, _) if (f - 30.0).abs() < 1e-10));
}

#[test] fn test_stat_histogram_bins() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let list = float_list(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    let bins = Value::Atom(AtomKind::Int(BigInt::from(3)), EffectTag::Pure, None);
    let r = call(&oo, &mut ctx, "stat.histogram", args2(list, bins));
    // 3 bins for [1,6] range → 3 buckets
    assert_eq!(list_len(&r), 3);
}

#[test] fn test_stat_variance() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let list = float_list(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
    let r = call(&oo, &mut ctx, "stat.variance", list);
    assert!(matches!(&r, Value::Atom(AtomKind::Float(f), _, _) if (f - 4.0).abs() < 1e-10));
}
```

---

## 7. 修改 `crates/interpreter/Cargo.toml`

```toml
[[test]]
name = "set_p46_test"
path = "tests/set_p46_test.rs"

[[test]]
name = "stat_p46_test"
path = "tests/stat_p46_test.rs"
```

---

## 8. 完成後驗證

```bash
cargo test
```

預期：~504 tests，0 failed。

重點確認：
- `set.from_list([1,2,1,3])` → 3 個元素（去重）
- `set.intersection({1,2,3}, {2,3,4})` → 2 個元素
- `stat.std_dev([2,4,4,4,5,5,7,9])` = 2.0
- `stat.percentile([10..50], 50)` ≈ 30
- `stat.histogram` 回傳 bins 個 @list
- genesis_test 通過（SEED_SET / SEED_STAT 正確填入）

---

## 9. 修改摘要

| 檔案 | 改動 |
|:-----|:-----|
| `src/builtins/set.rs` | 新建：8 個 set builtins |
| `src/builtins/stat.rs` | 新建：6 個 stat builtins（含 `extract_floats` helper） |
| `src/builtins/mod.rs` | `mod set;` + `mod stat;` + 2 個 register 呼叫 |
| `src/lib.rs` | `~%Set`（8 態射）+ `~%Stat`（6 態射）模組定義 |
| `src/genesis.rs` | `SEED_SET` + `SEED_STAT` |
| `tests/set_p46_test.rs` | 新建，6 tests |
| `tests/stat_p46_test.rs` | 新建，6 tests |
| `Cargo.toml` | +2 個 `[[test]]` entries |
