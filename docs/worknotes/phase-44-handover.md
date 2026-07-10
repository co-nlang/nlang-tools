# Phase 44 Handover：`~%Diff` 模組（Value 樹差異與修補）

> 日期：2026-05-25  
> 實作範圍：新模組 `~%Diff`，3 個態射；依賴 Phase 43 的 `parse_path`/`get_at_path`（`pub`）  
> 預期測試：~466 → ~474（新增 ~8 個測試）

---

## 0. 設計摘要

`~%Diff` 提供對 Value 樹的寫入操作（`set_at_path`）及結構差異計算。

| 態射 | 輸入 | 輸出 | Effect |
|:-----|:-----|:-----|:------:|
| `/diff` | `{0:a, 1:b}` | @list of `{path, from, to}` | Pure |
| `/patch` | `{0:val, 1:diff_list}` | 修補後的 Value | Pure |
| `/is_compatible` | `{0:a, 1:b}` | `#true` / `#false` | Pure |

**核心 helper**（新增至 `query.rs`，不在 `diff.rs` 自定義）：
```rust
// crates/interpreter/src/builtins/query.rs（現有檔案）
pub fn set_at_path(val: Value, path: &[String], new_val: Value) -> Value
pub fn deep_merge_values(a: Value, b: Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Value  // 改為 pub
```

**值相等判定**（`same_value`）：使用 BN/ 序列化位元組比較（`bn_serial`）取得確定性語義相等性。
若 bn_serial 無公開 API，改用遞歸結構比較（見第 1.3 節）。

---

## 1. 修改 `crates/interpreter/src/builtins/query.rs`

### 1.1 將 `deep_merge_values` 改為 `pub`

```rust
// 原本：
fn deep_merge_values(a: Value, b: Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Value {
// 改為：
pub fn deep_merge_values(a: Value, b: Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Value {
```

### 1.2 新增 `set_at_path`（pub，放在 `get_at_path` 之後）

```rust
/// Immutably update a Value tree at the given path.
/// Returns the rebuilt tree with new_val at path, or Bottom(MissingKey) if path traverses non-Combo.
pub fn set_at_path(val: Value, path: &[String], new_val: Value) -> Value {
    if path.is_empty() { return new_val; }
    match val {
        Value::Combo(mut c) => {
            let key = &path[0];
            let child = c.get_field(key).cloned().unwrap_or(Value::Top);
            let updated = set_at_path(child, &path[1..], new_val);
            c.insert_field(key, updated);
            Value::Combo(c)
        }
        _ => Value::Bottom(Box::new(BottomDetail {
            cause: BottomCause::MissingKey,
            path: Some(path.join(".")),
            message: Some("Cannot navigate into non-Combo value".to_string()),
            ..Default::default()
        })),
    }
}
```

### 1.3 值相等判定策略

`diff.rs` 內部需要比較兩個 Value 是否「語義相同」。優先方案按序：

1. **BN/ 序列化**：`crate::bn_serial` 模組若有 `pub fn to_bytes(val: &Value) -> Vec<u8>` 或類似函數，使用 `bn_serial::to_bytes(a) == bn_serial::to_bytes(b)`。（請 grep `bn_serial.rs` 確認 pub API。）
2. **Lattice Sketch CAID**：`lattice_sketch::sketch_caid(val)` 若存在。
3. **回退：遞歸結構比較**：
   ```rust
   fn same_value(a: &Value, b: &Value) -> bool {
       match (a, b) {
           (Value::Top, Value::Top) => true,
           (Value::Bottom(_), Value::Bottom(_)) => true,  // 兩者都是 Bottom 視為相同
           (Value::Atom(ka, _, _), Value::Atom(kb, _, _)) => {
               // AtomKind 應有 PartialEq 或 Debug 比較
               format!("{:?}", ka) == format!("{:?}", kb)
           }
           (Value::Combo(ca), Value::Combo(cb)) => {
               // 先快速比較 field 數量
               let a_keys: Vec<_> = ca.all_fields_iter().map(|(k,_)| k.clone()).collect();
               let b_keys: Vec<_> = cb.all_fields_iter().map(|(k,_)| k.clone()).collect();
               if a_keys.len() != b_keys.len() { return false; }
               for (key, va) in ca.all_fields_iter() {
                   match cb.get_field(&key) {
                       Some(vb) => if !same_value(&va, vb) { return false; }
                       None => return false,
                   }
               }
               true
           }
           _ => false,
       }
   }
   ```

---

## 2. 新建 `crates/interpreter/src/builtins/diff.rs`

```rust
use std::sync::Arc;
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, BottomCause, BottomDetail, ComboVal};
use crate::builtins::query::{parse_path, set_at_path, deep_merge_values};
use nlang_parser::ast::AtomKind;

// ── Value equality ───────────────────────────────────────────────────────────

fn same_value(a: &Value, b: &Value) -> bool {
    // 優先嘗試 bn_serial 方案（見 1.3 節）；此為回退版
    match (a, b) {
        (Value::Top, Value::Top) => true,
        (Value::Bottom(_), Value::Bottom(_)) => true,
        (Value::Atom(ka, _, _), Value::Atom(kb, _, _)) => {
            format!("{:?}", ka) == format!("{:?}", kb)
        }
        (Value::Combo(ca), Value::Combo(cb)) => {
            let a_len = ca.all_fields_iter().count();
            let b_len = cb.all_fields_iter().count();
            if a_len != b_len { return false; }
            for (key, va) in ca.all_fields_iter() {
                match cb.get_field(&key) {
                    Some(vb) => if !same_value(&va, vb) { return false; }
                    None => return false,
                }
            }
            true
        }
        _ => false,
    }
}

// ── Diff collection helper ───────────────────────────────────────────────────

fn str_atom(s: impl Into<String>) -> Value {
    Value::Atom(AtomKind::Str(s.into()), EffectTag::Pure, None)
}

fn missing() -> Value {
    Value::Bottom(Box::new(BottomDetail {
        cause: BottomCause::MissingKey,
        ..Default::default()
    }))
}

/// Recursively collect diff entries into `acc`.
/// Each entry: Combo { path: Str, from: Value, to: Value }.
fn collect_diffs(a: &Value, b: &Value, prefix: &str, acc: &mut Vec<Value>) {
    if same_value(a, b) { return; }
    match (a, b) {
        (Value::Combo(ca), Value::Combo(cb)) => {
            let mut keys = indexmap::IndexSet::new();
            for (k, _) in ca.all_fields_iter() { keys.insert(k.clone()); }
            for (k, _) in cb.all_fields_iter() { keys.insert(k.clone()); }
            for key in keys {
                let va = ca.get_field(&key).cloned().unwrap_or_else(missing);
                let vb = cb.get_field(&key).cloned().unwrap_or_else(missing);
                let child_prefix = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };
                collect_diffs(&va, &vb, &child_prefix, acc);
            }
        }
        _ => {
            let mut entry = IndexMap::new();
            entry.insert("path".to_string(), str_atom(prefix));
            entry.insert("from".to_string(), a.clone());
            entry.insert("to".to_string(), b.clone());
            acc.push(Value::Combo(ComboVal::new(entry, false, IndexMap::new(), EffectTag::Pure, vec![])));
        }
    }
}

fn build_list(items: Vec<Value>) -> Value {
    let mut m = IndexMap::new();
    m.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    for (i, v) in items.iter().enumerate() { m.insert(i.to_string(), v.clone()); }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn extract_list_items(list: &Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Vec<Value> {
    if let Value::Combo(c) = list {
        let mut items = Vec::new();
        for i in 0u32.. {
            if let Some(v) = c.get_field(&i.to_string()) {
                items.push(oo.force(v.clone(), ctx));
            } else { break; }
        }
        items
    } else { vec![] }
}

// ── has_any_bottom helper (for is_compatible) ────────────────────────────────

fn has_any_bottom(val: &Value) -> bool {
    match val {
        Value::Bottom(_) => true,
        Value::Combo(c) => c.all_fields_iter().any(|(_, v)| has_any_bottom(&v)),
        _ => false,
    }
}

// ── Builtin registration ─────────────────────────────────────────────────────

pub fn register_diff_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // diff.diff: {0: a, 1: b} → @list of {path, from, to}
    // Recursively walks both values, collecting leaf differences.
    // Keys present in b but not a: from = Bottom(MissingKey).
    // Keys present in a but not b: to = Bottom(MissingKey).
    m.insert("diff.diff".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
        let a = match c.get_field("0") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        let b = match c.get_field("1") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        let mut entries = Vec::new();
        collect_diffs(&a, &b, "", &mut entries);
        build_list(entries)
    }) as Arc<BuiltinFn>);

    // diff.patch: {0: val, 1: diff_list} → patched Value
    // Applies each diff entry {path, to} via set_at_path.
    // Applies entries in list order; later entries overwrite earlier ones.
    m.insert("diff.patch".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
        let mut val = match c.get_field("0") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        let diff_list = match c.get_field("1") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        let entries = extract_list_items(&diff_list, oo, ctx);
        for entry in entries {
            let entry = oo.force(entry, ctx);
            if let Value::Combo(ref ec) = entry {
                let path_str = match ec.get_field("path") {
                    Some(p) => oo.force(p.clone(), ctx).to_string_plain(),
                    None => continue,
                };
                let new_val = match ec.get_field("to") {
                    Some(v) => oo.force(v.clone(), ctx),
                    None => continue,
                };
                let segments = parse_path(&path_str);
                val = set_at_path(val, &segments, new_val);
            }
        }
        val
    }) as Arc<BuiltinFn>);

    // diff.is_compatible: {0: a, 1: b} → #true / #false
    // Returns #true if deep_merge of a and b produces no Bottom field.
    // Uses deep_merge_values from query.rs; any Bottom in result → #false.
    m.insert("diff.is_compatible".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
        let a = match c.get_field("0") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        let b = match c.get_field("1") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        let merged = deep_merge_values(a, b, oo, ctx);
        let tag = if has_any_bottom(&merged) { "false" } else { "true" };
        Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);
}
```

**注意**：`collect_diffs` 使用 `indexmap::IndexSet` 收集 union of keys。需要確認 `indexmap` crate 已在 `Cargo.toml` 引入（它已是 interpreter 的依賴），並在頂部加 `use indexmap::IndexSet;` 或直接在函數內收集到 `Vec<String>` 再 dedup。替代方案：

```rust
// 若 IndexSet 不在 prelude 中，改用：
let mut keys: Vec<String> = Vec::new();
for (k, _) in ca.all_fields_iter() { if !keys.contains(&k) { keys.push(k.clone()); } }
for (k, _) in cb.all_fields_iter() { if !keys.contains(&k) { keys.push(k.clone()); } }
```

---

## 3. 修改 `crates/interpreter/src/builtins/mod.rs`

### 3.1 加 `mod diff;`

```rust
mod diff;
```

### 3.2 在 `create_default_builtins()` 末尾加：

```rust
    diff::register_diff_builtins(&mut m);
```

---

## 4. 修改 `crates/interpreter/src/lib.rs`

在 `~%Query` 區塊之後，加入 `~%Diff` 模組（使用相同的 local closure 風格）：

```rust
        // ~%Diff module
        let mut diff_fields = IndexMap::new();
        let dmorph = |name: &str, id: &str, eff: EffectTag| -> Value {
            let mut f = IndexMap::new();
            f.insert("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
            f.insert("%builtin".to_string(), Value::Atom(AtomKind::Str(id.to_string()), EffectTag::Pure, None));
            f.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("logic".to_string()), EffectTag::Pure, None));
            Value::Combo(ComboVal::new(f, true, IndexMap::new(), eff, vec![]))
        };
        diff_fields.insert("/diff".to_string(),          dmorph("/diff",          "diff.diff",          EffectTag::Pure));
        diff_fields.insert("/patch".to_string(),         dmorph("/patch",         "diff.patch",         EffectTag::Pure));
        diff_fields.insert("/is_compatible".to_string(), dmorph("/is_compatible", "diff.is_compatible", EffectTag::Pure));
        let diff_module = Value::Combo(ComboVal::new(diff_fields, true, IndexMap::new(), EffectTag::Pure, vec![]));
        root.insert_field("~%Diff", diff_module);
```

---

## 5. 修改 `crates/interpreter/src/genesis.rs`

在 `SEED_QUERY` 之後加入：

```rust
pub const SEED_DIFF: &str = "hash:sha256:v2:_:<lattice_sketch>:<digest>";
// ↑ 執行 cargo test genesis_test -- --nocapture 取得實際值
```

在 `all_seeds()` 加入：

```rust
seeds.push(("~%Diff", SEED_DIFF));
```

**流程**：先實作完所有檔案 → `cargo test genesis_test -- --nocapture 2>&1 | grep -i diff` → 填入實際 seed。

---

## 6. 新增測試 `crates/interpreter/tests/diff_p44_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, BottomCause, ComboVal};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use num_bigint::BigInt;

fn oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}
fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}
fn tag(t: &str) -> Value {
    Value::Atom(AtomKind::Tag(t.to_string()), EffectTag::Pure, None)
}

fn combo(pairs: &[(&str, Value)]) -> Value {
    let mut m = IndexMap::new();
    for (k, v) in pairs { m.insert(k.to_string(), v.clone()); }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn list_of(items: &[Value]) -> Value {
    let mut m = IndexMap::new();
    m.insert("%kind".to_string(), tag("list"));
    for (i, v) in items.iter().enumerate() { m.insert(i.to_string(), v.clone()); }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}

fn args2(a: Value, b: Value) -> Value {
    combo(&[("0", a), ("1", b)])
}

fn list_len(v: &Value) -> usize {
    if let Value::Combo(c) = v {
        (0u32..).take_while(|i| c.get_field(&i.to_string()).is_some()).count()
    } else { 0 }
}

// ─── diff.diff ────────────────────────────────────────────────────────────────

#[test]
fn test_diff_identical_returns_empty() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let val = combo(&[("x", int_val(1)), ("y", str_val("hello"))]);
    let result = call(&oo, &mut ctx, "diff.diff", args2(val.clone(), val));
    assert_eq!(list_len(&result), 0, "identical values → empty diff");
}

#[test]
fn test_diff_changed_leaf() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let a = combo(&[("x", int_val(1))]);
    let b = combo(&[("x", int_val(2))]);
    let result = call(&oo, &mut ctx, "diff.diff", args2(a, b));
    assert_eq!(list_len(&result), 1, "one changed field → one diff entry");
    if let Value::Combo(rc) = &result {
        if let Some(entry) = rc.get_field("0") {
            if let Value::Combo(ec) = entry {
                let path = ec.get_field("path").expect("diff entry has path");
                assert!(matches!(path, Value::Atom(AtomKind::Str(s), _, _) if s == "x"));
            }
        }
    }
}

#[test]
fn test_diff_added_field() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let a = combo(&[("x", int_val(1))]);
    let b = combo(&[("x", int_val(1)), ("y", int_val(2))]);
    let result = call(&oo, &mut ctx, "diff.diff", args2(a, b));
    // y added → one entry; from = Bottom(MissingKey), to = int(2)
    assert_eq!(list_len(&result), 1);
    if let Value::Combo(rc) = &result {
        if let Some(entry) = rc.get_field("0") {
            if let Value::Combo(ec) = entry {
                let from = ec.get_field("from").expect("has from");
                assert!(matches!(from, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::MissingKey)));
            }
        }
    }
}

#[test]
fn test_diff_nested_change() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let a = combo(&[("nested", combo(&[("val", int_val(10))]))]);
    let b = combo(&[("nested", combo(&[("val", int_val(99))]))]);
    let result = call(&oo, &mut ctx, "diff.diff", args2(a, b));
    assert_eq!(list_len(&result), 1);
    // path should be "nested.val"
    if let Value::Combo(rc) = &result {
        if let Some(entry) = rc.get_field("0") {
            if let Value::Combo(ec) = entry {
                let path_val = ec.get_field("path").expect("has path");
                assert!(matches!(path_val, Value::Atom(AtomKind::Str(s), _, _) if s == "nested.val"),
                    "nested path should be 'nested.val', got {:?}", path_val);
            }
        }
    }
}

// ─── diff.patch ───────────────────────────────────────────────────────────────

#[test]
fn test_patch_empty_diff_returns_original() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let val = combo(&[("x", int_val(42))]);
    let empty_diff = list_of(&[]);
    let result = call(&oo, &mut ctx, "diff.patch", args2(val, empty_diff));
    if let Value::Combo(rc) = &result {
        let x = rc.get_field("x").expect("x preserved");
        assert!(matches!(x, Value::Atom(AtomKind::Int(n), _, _) if n == &BigInt::from(42i64)));
    } else { panic!("expected Combo"); }
}

#[test]
fn test_patch_applies_single_change() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let val = combo(&[("score", int_val(0))]);
    let entry = combo(&[("path", str_val("score")), ("to", int_val(100))]);
    let diff_list = list_of(&[entry]);
    let result = call(&oo, &mut ctx, "diff.patch", args2(val, diff_list));
    if let Value::Combo(rc) = &result {
        let score = rc.get_field("score").expect("score field");
        assert!(matches!(score, Value::Atom(AtomKind::Int(n), _, _) if n == &BigInt::from(100i64)));
    } else { panic!("expected Combo"); }
}

// ─── diff.is_compatible ───────────────────────────────────────────────────────

#[test]
fn test_is_compatible_disjoint_fields() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let a = combo(&[("x", int_val(1))]);
    let b = combo(&[("y", int_val(2))]);
    let result = call(&oo, &mut ctx, "diff.is_compatible", args2(a, b));
    assert!(matches!(&result, Value::Atom(AtomKind::Tag(t), _, _) if t == "true"),
        "disjoint fields are compatible");
}

#[test]
fn test_is_compatible_conflicting_atoms() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let a = combo(&[("x", int_val(1))]);
    let b = combo(&[("x", int_val(2))]);
    // unify_internal(Int(1), Int(2)) → Bottom(Conflict)
    let result = call(&oo, &mut ctx, "diff.is_compatible", args2(a, b));
    assert!(matches!(&result, Value::Atom(AtomKind::Tag(t), _, _) if t == "false"),
        "conflicting same field → not compatible");
}
```

---

## 7. 修改 `crates/interpreter/Cargo.toml`

```toml
[[test]]
name = "diff_p44_test"
path = "tests/diff_p44_test.rs"
```

---

## 8. 完成後驗證

```bash
cargo test
```

預期：~474 tests，0 failed。

重點確認：
- `diff.diff` 對相同 Value 返回空 list
- `diff.diff` 正確計算巢狀路徑（`nested.val`）
- `diff.patch` 套用 diff 後欄位更新
- `diff.is_compatible` 對衝突 atom 返回 `#false`
- genesis seed 穩定（`cargo test genesis_test` 通過）

---

## 9. 實作注意事項

| 事項 | 說明 |
|:-----|:-----|
| `query.rs` 的 `deep_merge_values` 改 `pub` | `diff.rs` 的 `is_compatible` 需要 `use crate::builtins::query::deep_merge_values` |
| `set_at_path` 放在 `query.rs` | 雖然語義上屬於 Diff，但作為跨模組共用 helper 放 query.rs 更自然（與 `parse_path`/`get_at_path` 同檔） |
| `collect_diffs` 的 union-of-keys | 用 `Vec<String>` + `contains` 避免 `IndexSet` import 複雜度；兩個 Combo 的 field 數通常不大 |
| `same_value` 的 f64 比較 | `AtomKind::Float(f)` 用 `format!("{:?}", f)` 比較時 NaN ≠ NaN，但 `f64::to_bits` 比較更正確。若遇到 Float diff 測試失敗，改為 `a.to_bits() == b.to_bits()` |
| `diff.patch` 的 `path` 欄位 | diff entry 格式為 `{path: Str, from: Value, to: Value}`；patch 只讀 `path` 和 `to`，忽略 `from` |
| `diff.diff` 的 root-level 路徑 | `prefix = ""` 時，第一層 key 直接為路徑（如 `"x"`）；巢狀為 `"nested.val"`。empty prefix 的 diff entry path 為 `""`（空字串）只在兩個 non-Combo 直接比較時出現，測試應避免此情況。 |

---

## 10. 修改摘要（5 個檔案，1 個修改）

| 檔案 | 改動 |
|:-----|:-----|
| `src/builtins/query.rs` | `deep_merge_values` → `pub`；新增 `pub fn set_at_path` |
| `src/builtins/diff.rs` | 新建：`same_value`、`collect_diffs`、`has_any_bottom` + 3 個 diff builtins |
| `src/builtins/mod.rs` | `mod diff;` + `diff::register_diff_builtins(&mut m)` |
| `src/lib.rs` | `root_with_system()` 加 `~%Diff` 模組定義（3 個態射） |
| `src/genesis.rs` | `SEED_DIFF` 常數 + `all_seeds()` 插入 |
| `tests/diff_p44_test.rs` | 新建，8 個測試 |
| `Cargo.toml` | +3 行 `[[test]]` entry |
