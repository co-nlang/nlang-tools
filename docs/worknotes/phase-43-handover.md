# Phase 43 Handover：`~%Query` 模組（Combo 路徑查詢）

> 日期：2026-05-25  
> 實作範圍：新模組 `~%Query`，4 個態射；共用 path helper（Phase 44 ~%Diff 複用）  
> 預期測試：~458 → ~466（新增 ~8 個測試）

---

## 0. 設計摘要

`~%Query` 提供對嵌套 Combo 的讀取操作，不修改 Value 樹。

| 態射 | 輸入 | 輸出 | Effect |
|:-----|:-----|:-----|:------:|
| `/select` | `{0:val, 1:path_str}` | 路徑指定的值 | Pure |
| `/where` | `{0:list, 1:pred}` | 過濾後的 @list | IO |
| `/pluck` | `{0:combo, 1:key_list}` | 僅含指定欄位的 Combo | Pure |
| `/deep_merge` | `{0:a, 1:b}` | 遞歸合併後的 Combo | Pure |

**路徑格式**（供 Phase 44 `~%Diff` 共用）：
- 點號分隔字串：`"field.subfield.2"`
- 欄位鍵名保留前綴：`"%kind"`, `"/rule"`, `"@type"`, `"~%System"`
- 整數字串表示 list index：`"items.0.name"`

**`all_fields_iter()` 鍵名語義**：回傳欄位時已帶完整前綴（`%meta`、`/rule`、`@type`），
可直接傳給 `insert_field` ← 這是 `set_at_path`（Phase 44）的前提。

---

## 1. 新建 `crates/interpreter/src/builtins/query.rs`

```rust
use std::sync::Arc;
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, BottomCause, BottomDetail, ComboVal};
use nlang_parser::ast::AtomKind;

// ── Shared path helpers (also used by Phase 44 ~%Diff) ─────────────────────

/// Split "field.%meta.0" → ["field", "%meta", "0"].
/// Empty string → empty vec (selects root).
pub fn parse_path(s: &str) -> Vec<String> {
    if s.is_empty() { return vec![]; }
    s.split('.').map(|seg| seg.to_string()).collect()
}

/// Navigate a Value along path segments. Returns None if any segment is missing.
pub fn get_at_path(val: &Value, path: &[String], oo: &Ouroboros, ctx: &mut EvalContext) -> Option<Value> {
    if path.is_empty() { return Some(val.clone()); }
    match val {
        Value::Combo(c) => {
            let field = c.get_field(&path[0])?;
            let next = oo.force(field.clone(), ctx);
            get_at_path(&next, &path[1..], oo, ctx)
        }
        _ => None,
    }
}

/// Extract numeric indices from a @list Combo: {%kind:#list, 0:v0, 1:v1, ...} → vec of Values.
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

/// Build a @list Combo from a Vec<Value>.
fn build_list(items: Vec<Value>, effect: EffectTag) -> Value {
    let mut m = IndexMap::new();
    m.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    for (i, v) in items.iter().enumerate() {
        m.insert(i.to_string(), v.clone());
    }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), effect, vec![]))
}

/// Check if a value is truthy: #true or any non-Bottom non-false value.
fn is_truthy(val: &Value) -> bool {
    match val {
        Value::Bottom(_) => false,
        Value::Atom(AtomKind::Tag(t), _, _) => t != "false",
        _ => true,
    }
}

// ── deep_merge helper ────────────────────────────────────────────────────────

/// Recursively merge two Values: Combo+Combo → field-wise merge; leaf → unify.
fn deep_merge_values(a: Value, b: Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Value {
    match (&a, &b) {
        (Value::Combo(ca), Value::Combo(cb)) => {
            let mut merged = ca.clone();
            // For each field in b, recursively merge with a's field (or use b's value if a lacks it)
            for (key, vb) in cb.all_fields_iter() {
                let va = ca.get_field(&key).cloned().unwrap_or(Value::Top);
                let result = deep_merge_values(va, vb, oo, ctx);
                merged.insert_field(&key, result);
            }
            Value::Combo(merged)
        }
        _ => oo.unify_internal(a, b, ctx),
    }
}

// ── Builtin registration ─────────────────────────────────────────────────────

pub fn register_query_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // query.select: {0: value, 1: path_str} → value at path | Bottom(MissingKey)
    // Path: "field.sub.0" — dot-separated, integer for list index, %/@ prefixes OK.
    m.insert("query.select".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
        let val = match c.get_field("0") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        // Second arg can be positional "1" or named "path"
        let path_str = match c.get_field("1").or_else(|| c.get_field("path")) {
            Some(v) => oo.force(v.clone(), ctx).to_string_plain(),
            None => return val,  // no path → return root
        };
        let segments = parse_path(&path_str);
        get_at_path(&val, &segments, oo, ctx)
            .unwrap_or_else(|| Value::Bottom(Box::new(BottomDetail {
                cause: BottomCause::MissingKey,
                path: Some(path_str),
                message: Some("Path not found in value".to_string()),
                ..Default::default()
            })))
    }) as Arc<BuiltinFn>);

    // query.where: {0: list, 1: pred} → filtered @list
    // pred is applied to each element; element kept if result is truthy.
    // Effect: IO (predicate may have arbitrary effects).
    m.insert("query.where".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
        let list_val = match c.get_field("0") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        let pred = match c.get_field("1") {
            Some(v) => v.clone(),
            None => return BottomCause::Conflict.into(),
        };
        let items = extract_list_items(&list_val, oo, ctx);
        let mut kept = Vec::new();
        let mut max_effect = EffectTag::Pure;
        for item in items {
            let result = oo.apply_morphism(pred.clone(), item.clone(), ctx);
            max_effect = max_effect.max(result.effect());
            if is_truthy(&result) {
                kept.push(item);
            }
        }
        build_list(kept, max_effect.max(EffectTag::IO))
    }) as Arc<BuiltinFn>);

    // query.pluck: {0: combo, 1: key_list} → Combo with only specified keys
    // key_list is a @list of Str values: {%kind:#list, 0:"field_a", 1:"%meta_b"}
    m.insert("query.pluck".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
        let combo_val = match c.get_field("0") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        let key_list_val = match c.get_field("1") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        let keys: Vec<String> = extract_list_items(&key_list_val, oo, ctx)
            .into_iter()
            .map(|v| v.to_string_plain())
            .collect();

        let src = match combo_val { Value::Combo(c) => c, _ => return BottomCause::Conflict.into() };
        let mut result_fields = IndexMap::new();
        for key in &keys {
            if let Some(v) = src.get_field(key) {
                result_fields.insert(key.clone(), v.clone());
            }
        }
        Value::Combo(ComboVal::new(result_fields, false, IndexMap::new(), EffectTag::Pure, vec![]))
    }) as Arc<BuiltinFn>);

    // query.deep_merge: {0: a, 1: b} → recursively merged Combo
    // Combo+Combo → field-wise recursive; leaf → unify_internal.
    // If unification produces Bottom for a field, that field is Bottom in result.
    m.insert("query.deep_merge".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
        let a = match c.get_field("0") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        let b = match c.get_field("1") {
            Some(v) => oo.force(v.clone(), ctx),
            None => return BottomCause::Conflict.into(),
        };
        deep_merge_values(a, b, oo, ctx)
    }) as Arc<BuiltinFn>);
}
```

---

## 2. 修改 `crates/interpreter/src/builtins/mod.rs`

### 2.1 加 `mod query;`（在現有 mod 列表末尾）

```rust
mod query;
```

### 2.2 在 `create_default_builtins()` 末尾加：

```rust
    query::register_query_builtins(&mut m);
```

---

## 3. 修改 `crates/interpreter/src/lib.rs`

### 3.1 在 `root_with_system()` 加 `~%Query` 模組

在加入 `~%Path` 的區塊之後，加入（使用與其他模組相同的 morphism helper 風格）：

```rust
        // ~%Query module
        let mut query_fields = IndexMap::new();
        let qmorph = |name: &str, id: &str, eff: EffectTag| -> Value {
            let mut f = IndexMap::new();
            f.insert("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
            f.insert("%builtin".to_string(), Value::Atom(AtomKind::Str(id.to_string()), EffectTag::Pure, None));
            f.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("logic".to_string()), EffectTag::Pure, None));
            Value::Combo(ComboVal::new(f, true, IndexMap::new(), eff, vec![]))
        };
        query_fields.insert("/select".to_string(),     qmorph("/select",     "query.select",     EffectTag::Pure));
        query_fields.insert("/where".to_string(),      qmorph("/where",      "query.where",      EffectTag::IO));
        query_fields.insert("/pluck".to_string(),      qmorph("/pluck",      "query.pluck",      EffectTag::Pure));
        query_fields.insert("/deep_merge".to_string(), qmorph("/deep_merge", "query.deep_merge", EffectTag::Pure));
        let query_module = Value::Combo(ComboVal::new(query_fields, true, IndexMap::new(), EffectTag::Pure, vec![]));
        root.insert_field("~%Query", query_module);
```

**注意**：`qmorph` lambda 的捕獲模式與其他模組的 local helper 一致；若 lib.rs 已有通用 morph helper（如 `engine_morph`），可直接複用，不必新定義。

### 3.2 加入 genesis seed

在 `genesis.rs` 的 `all_seeds()` 中加入 `SEED_QUERY`（計算方式見下）。

---

## 4. 修改 `crates/interpreter/src/genesis.rs`

在現有 SEED 常數列表末尾（如 `SEED_PATH` 之後）加入：

```rust
pub const SEED_QUERY: &str = "hash:sha256:v2:_:<lattice_sketch>:<digest>";
// ↑ 執行 cargo test genesis_test 取得實際值，或暫時留空再補
```

在 `all_seeds()` 函數中加入對應的插入（與 `SEED_PATH` 等模式一致）：

```rust
seeds.push(("~%Query", SEED_QUERY));
```

**流程**：先實作完 query.rs + mod.rs + lib.rs → 執行 `cargo test genesis_test 2>&1 | grep QUERY` 取得實際 seed 字串 → 填入 `SEED_QUERY`。

---

## 5. 新增測試 `crates/interpreter/tests/query_p43_test.rs`

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

// ─── query.select ─────────────────────────────────────────────────────────────

#[test]
fn test_select_top_level_field() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let val = combo(&[("name", str_val("Alice")), ("age", int_val(30))]);
    let result = call(&oo, &mut ctx, "query.select", args2(val, str_val("name")));
    assert!(matches!(&result, Value::Atom(AtomKind::Str(s), _, _) if s == "Alice"));
}

#[test]
fn test_select_nested_path() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let inner = combo(&[("city", str_val("Taipei"))]);
    let outer = combo(&[("address", inner)]);
    let result = call(&oo, &mut ctx, "query.select", args2(outer, str_val("address.city")));
    assert!(matches!(&result, Value::Atom(AtomKind::Str(s), _, _) if s == "Taipei"));
}

#[test]
fn test_select_missing_path_returns_missing_key() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let val = combo(&[("x", int_val(1))]);
    let result = call(&oo, &mut ctx, "query.select", args2(val, str_val("y.z")));
    assert!(matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::MissingKey)));
}

#[test]
fn test_select_list_index() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let lst = list_of(&[int_val(10), int_val(20), int_val(30)]);
    let container = combo(&[("items", lst)]);
    let result = call(&oo, &mut ctx, "query.select", args2(container, str_val("items.1")));
    assert!(matches!(&result, Value::Atom(AtomKind::Int(n), _, _) if n == &BigInt::from(20i64)));
}

// ─── query.pluck ──────────────────────────────────────────────────────────────

#[test]
fn test_pluck_extracts_specified_fields() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let val = combo(&[("a", int_val(1)), ("b", int_val(2)), ("c", int_val(3))]);
    let keys = list_of(&[str_val("a"), str_val("c")]);
    let result = call(&oo, &mut ctx, "query.pluck", args2(val, keys));
    if let Value::Combo(ref cv) = result {
        assert!(cv.get_field("a").is_some(), "should have field a");
        assert!(cv.get_field("b").is_none(), "should not have field b");
        assert!(cv.get_field("c").is_some(), "should have field c");
    } else { panic!("expected Combo, got {:?}", result); }
}

// ─── query.deep_merge ─────────────────────────────────────────────────────────

#[test]
fn test_deep_merge_combines_disjoint_fields() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let a = combo(&[("x", int_val(1))]);
    let b = combo(&[("y", int_val(2))]);
    let result = call(&oo, &mut ctx, "query.deep_merge", args2(a, b));
    if let Value::Combo(ref cv) = result {
        assert!(cv.get_field("x").is_some());
        assert!(cv.get_field("y").is_some());
    } else { panic!("expected Combo"); }
}

#[test]
fn test_deep_merge_recurses_nested_combos() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let a = combo(&[("nested", combo(&[("x", int_val(1))]))]);
    let b = combo(&[("nested", combo(&[("y", int_val(2))]))]);
    let result = call(&oo, &mut ctx, "query.deep_merge", args2(a, b));
    let nested = if let Value::Combo(ref cv) = result {
        cv.get_field("nested").cloned().expect("nested field")
    } else { panic!("expected Combo"); };
    if let Value::Combo(ref nc) = nested {
        assert!(nc.get_field("x").is_some(), "x from a");
        assert!(nc.get_field("y").is_some(), "y from b");
    } else { panic!("nested should be Combo"); }
}

// ─── query.where ──────────────────────────────────────────────────────────────

// Note: testing query.where requires a predicate morphism.
// We use a simple cond.if or check for #true tag as predicate.
// For simplicity, we construct a filter using the existing cond builtins or
// verify list structure is preserved for trivially-truthy values.

#[test]
fn test_where_empty_list_returns_empty() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let empty_list = list_of(&[]);
    // Any predicate on empty list → empty list
    // Use Value::Top as a stand-in pred (will return Top which is truthy for all items)
    let result = call(&oo, &mut ctx, "query.where", args2(empty_list, Value::Top));
    if let Value::Combo(ref cv) = result {
        assert!(cv.get_field("0").is_none(), "empty list should have no items");
    } else { panic!("expected Combo list, got {:?}", result); }
}
```

---

## 6. 修改 `crates/interpreter/Cargo.toml`

在 `[[test]]` 末尾加入：

```toml
[[test]]
name = "query_p43_test"
path = "tests/query_p43_test.rs"
```

---

## 7. 完成後驗證

```bash
cargo test
```

預期：~466 tests，0 failed。

重點確認：
- `query.select` 導航巢狀 Combo 和 list index
- 路徑不存在 → `Bottom(MissingKey)`
- `query.pluck` 只保留指定欄位
- `query.deep_merge` 遞歸合併，相同欄位走 unify_internal
- genesis seed 穩定（`cargo test genesis_test` 通過）

---

## 8. 實作注意事項

| 事項 | 說明 |
|:-----|:-----|
| `parse_path` 和 `get_at_path` 要 `pub` | Phase 44 的 `~%Diff` 會從 `crate::builtins::query` 直接引用這兩個函數 |
| `query.where` 的 `is_truthy` | Bottom → false；`#false` → false；其他全部 → true（包含 Top、Combo、數字、字串） |
| `all_fields_iter()` 鍵名帶前綴 | `deep_merge_values` 的 `cb.all_fields_iter()` 回傳 `("%kind", v)` 等；`insert_field(&key, v)` 會正確路由到 meta submap |
| `qmorph` lambda 範圍 | 宣告為 lib.rs 函數內的 local closure，只在 `~%Query` 區塊用；不需 export |
| `query.where` Effect::IO | 宣告 IO 因為 pred 效果在編譯時未知；實際傳播由 `max_effect` 計算決定 |
| genesis.rs 更新流程 | 1. 實作所有檔案 2. `cargo test genesis_test -- --nocapture` 看輸出的實際 CAID 3. 填入 `SEED_QUERY` |

---

## 9. Phase 44 預告（~%Diff）

Phase 43 的 `parse_path` / `get_at_path` 作為公開函數，Phase 44 會加入：

```rust
pub fn set_at_path(val: Value, path: &[String], new_val: Value) -> Value
```

然後基於此實作：
- `diff.diff(a, b)` → `@list of {path, from, to}`
- `diff.patch(a, diff)` → 重建 Value 樹
- `diff.is_compatible(a, b)` → 複用 `deep_merge` 判斷是否無 Bottom

---

## 10. 修改摘要（4 個檔案）

| 檔案 | 改動 |
|:-----|:-----|
| `src/builtins/query.rs` | 新建：`parse_path`、`get_at_path`（pub）+ 4 個 query builtins |
| `src/builtins/mod.rs` | `mod query;` + `query::register_query_builtins(&mut m)` |
| `src/lib.rs` | `root_with_system()` 加 `~%Query` 模組定義（4 個態射） |
| `src/genesis.rs` | `SEED_QUERY` 常數 + `all_seeds()` 插入 |
| `tests/query_p43_test.rs` | 新建，8 個測試 |
| `Cargo.toml` | +3 行 `[[test]]` entry |
