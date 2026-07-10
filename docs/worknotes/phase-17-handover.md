# Phase 17 交接文件

> 狀態：待實作  
> 前置：Phase 16 完成（198 tests passing）  
> 目標：Option 組合子完整化 + List Monad bind + Čech nerve 精確交集

---

## 概覽

Phase 17 三個任務：

| 任務 | 位置 | 新增測試數 |
|:-----|:-----|:---------:|
| Task 1：`option.or` / `option.unwrap_or` / `option.filter` | `builtins/engine.rs` | 6 |
| Task 2：`list.flat_map` | `builtins/list.rs` | 3 |
| Task 3：`NerveEntry.field_keys` + nerve 精確交集 | `ladd.rs`, `builtins/disc.rs` | 4 |

預期完成後：198 + 13 ≈ **211 tests**

---

## Task 1：Option 組合子（`option.or` / `option.unwrap_or` / `option.filter`）

### 位置

`crates/interpreter/src/builtins/engine.rs`

加在現有 `option.and_then` 之後。

### 語義定義

```
option.or       : {0: default_opt, 1: opt} → Option
  Some(x) → Some(x)（原樣返回）
  None    → default_opt（原樣返回，呼叫者提供的是 Option 值）

option.unwrap_or : {0: default_value, 1: opt} → Value
  Some({%val: v}) → v（解包內部值）
  None            → default_value（原始值，不包裝）

option.filter   : {0: pred_fn, 1: opt} → Option
  None           → None
  Some({%val: v}) where pred_fn(v) = #true  → Some({%val: v})（原樣保留）
  Some({%val: v}) where pred_fn(v) ≠ #true  → None
```

### 實作

在 `engine_morphisms()` 函數中，於 `option.and_then` 之後插入：

```rust
// option.or: {0: default_opt, 1: opt} → the opt if Some, else default_opt
m.insert("option.or".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(default_v), Some(opt_v)) = (c.get_field("0"), c.get_field("1")) {
            let default_v = default_v.clone();
            let opt = oo.force(opt_v.clone(), ctx);
            return match opt.collapse() {
                Value::Atom(AtomKind::Tag(ref t), _, _) if t.trim_start_matches('#') == "none" => {
                    default_v
                }
                other => other,
            };
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);

// option.unwrap_or: {0: default_value, 1: opt} → inner value or default
m.insert("option.unwrap_or".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(default_v), Some(opt_v)) = (c.get_field("0"), c.get_field("1")) {
            let default_v = default_v.clone();
            let opt = oo.force(opt_v.clone(), ctx);
            return match opt.collapse() {
                Value::Atom(AtomKind::Tag(ref t), _, _) if t.trim_start_matches('#') == "none" => {
                    default_v
                }
                Value::Combo(ref cv) => {
                    cv.get_field("%val").cloned().unwrap_or(Value::Top)
                }
                _ => default_v,
            };
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);

// option.filter: {0: pred_fn, 1: opt} → Option
m.insert("option.filter".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let none_val = Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
    if let Value::Combo(ref c) = arg {
        if let (Some(pred_f), Some(opt_v)) = (c.get_field("0"), c.get_field("1")) {
            let pred_f = pred_f.clone();
            let opt = oo.force(opt_v.clone(), ctx);
            return match opt.collapse() {
                Value::Atom(AtomKind::Tag(ref t), _, _) if t.trim_start_matches('#') == "none" => {
                    none_val
                }
                some_v @ Value::Combo(_) => {
                    let inner = if let Value::Combo(ref cv) = some_v {
                        cv.get_field("%val").cloned().unwrap_or(Value::Top)
                    } else { Value::Top };
                    let result = oo.apply_morphism(pred_f, inner, ctx);
                    match result.collapse() {
                        Value::Atom(AtomKind::Tag(ref t), _, _) if t.trim_start_matches('#') == "true" => {
                            some_v
                        }
                        _ => none_val,
                    }
                }
                _ => none_val,
            };
        }
    }
    none_val
}) as Arc<BuiltinFn>);
```

### 注意事項

- `option.or` 的語義：保留 Some，替換 None（兩邊都是 Option 值）
- `option.unwrap_or` 的語義：解包 Some 得到內部值，None 退回原始 default（不是 Option）
- `option.filter` 中 pred_fn 必須返回 `#true` 才通過（`#false`、Bottom、其他值都算失敗）
- `apply_morphism` 是 `Ouroboros` 的方法，參照現有 `option.map` 實作的用法

### 不需要修改 genesis seeds

這三個是純 standalone builtins，不加入 `@option` 的 type definition。原因同 Phase 16 的 `option.and_then`：避免連續 Phase 都要更新 SEED_OPTION。

### 新增測試

測試檔位置：現有的 `tests/functor_test.rs`，在最後加入新測試群組。

```rust
#[test]
fn test_option_or_with_none() {
    // option.or({0: Some(99), 1: None}) → Some(99)
    let oo = make_ouroboros();
    let mut ctx = oo.eval_context();
    let default_opt = make_some_int(&oo, 99);
    let none_v = make_none();
    let arg = make_combo_2(&oo, default_opt, none_v);
    let result = oo.call_builtin("option.or", arg, &mut ctx);
    assert_is_some_int(&result, 99);
}

#[test]
fn test_option_or_with_some() {
    // option.or({0: Some(99), 1: Some(42)}) → Some(42)（保留原來的 Some）
    let oo = make_ouroboros();
    let mut ctx = oo.eval_context();
    let default_opt = make_some_int(&oo, 99);
    let some_42 = make_some_int(&oo, 42);
    let arg = make_combo_2(&oo, default_opt, some_42);
    let result = oo.call_builtin("option.or", arg, &mut ctx);
    assert_is_some_int(&result, 42);
}

#[test]
fn test_option_unwrap_or_none() {
    // option.unwrap_or({0: 99, 1: None}) → 99
    let oo = make_ouroboros();
    let mut ctx = oo.eval_context();
    let default_v = Value::Atom(AtomKind::Int(99.into()), EffectTag::Pure, None);
    let none_v = make_none();
    let arg = make_combo_2(&oo, default_v, none_v);
    let result = oo.call_builtin("option.unwrap_or", arg, &mut ctx);
    // result should be 99 (raw int, not wrapped)
    assert_eq!(result.to_string_plain(), "99");
}

#[test]
fn test_option_unwrap_or_some() {
    // option.unwrap_or({0: 99, 1: Some(42)}) → 42
    let oo = make_ouroboros();
    let mut ctx = oo.eval_context();
    let default_v = Value::Atom(AtomKind::Int(99.into()), EffectTag::Pure, None);
    let some_42 = make_some_int(&oo, 42);
    let arg = make_combo_2(&oo, default_v, some_42);
    let result = oo.call_builtin("option.unwrap_or", arg, &mut ctx);
    assert_eq!(result.to_string_plain(), "42");
}

#[test]
fn test_option_filter_pass() {
    // option.filter({0: is_positive, 1: Some(5)}) → Some(5)
    // is_positive: v > 0 → #true
    let oo = make_ouroboros();
    let mut ctx = oo.eval_context();
    // Build predicate as a Code value that checks > 0
    // Use a simpler approach: build a morphism that returns #true for positive int
    // Alternatively, use an inline function via test helper
    let pred = make_tag_true_pred(&oo); // helper: always returns #true
    let some_5 = make_some_int(&oo, 5);
    let arg = make_combo_2(&oo, pred, some_5);
    let result = oo.call_builtin("option.filter", arg, &mut ctx);
    assert_is_some_int(&result, 5);
}

#[test]
fn test_option_filter_fail() {
    // option.filter({0: pred_false, 1: Some(5)}) → None
    let oo = make_ouroboros();
    let mut ctx = oo.eval_context();
    let pred = make_tag_false_pred(&oo); // helper: always returns #false
    let some_5 = make_some_int(&oo, 5);
    let arg = make_combo_2(&oo, pred, some_5);
    let result = oo.call_builtin("option.filter", arg, &mut ctx);
    assert_is_none(&result);
}
```

**Note**: `make_some_int`, `make_none`, `make_combo_2`, `make_tag_true_pred`, `make_tag_false_pred`, `assert_is_some_int`, `assert_is_none` 應參照 `functor_test.rs` 中現有的 helper 模式實作。如果 `call_builtin` 不是公開方法，則改用 `oo.builtin_registry.get("option.or").unwrap()(arg, &oo, &mut ctx)` 的方式調用。

---

## Task 2：`list.flat_map`

### 位置

`crates/interpreter/src/builtins/list.rs`

### 語義定義

```
list.flat_map : {0: f, 1: list} → List
  f: Value → List<Value>
  對 list 中每個元素 x 應用 f(x)，將所有結果 list 串接為一個 flat list
```

範例：
```
list.flat_map({0: x→[x, x*2], 1: [1, 2, 3]})
→ [1, 2, 2, 4, 3, 6]
```

### 實作

在 `list_morphisms()` 函數中，加在 `list.map` 或 `list.fold` 之後：

```rust
m.insert("list.flat_map".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(f), Some(list_v)) = (c.get_field("0"), c.get_field("1")) {
            let f = f.clone();
            let list = oo.force(list_v.clone(), ctx);
            // Extract items using existing list extraction pattern
            let items = extract_list_items(&list);
            let mut result: Vec<Value> = Vec::new();
            for item in items {
                let sub = oo.apply_morphism(f.clone(), item, ctx);
                let sub_forced = oo.force(sub, ctx);
                // Flatten: extract sub-list items
                let sub_items = extract_list_items(&sub_forced);
                result.extend(sub_items);
            }
            return build_list_value(result);
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);
```

### 關鍵細節

`extract_list_items` 和 `build_list_value` 是 `list.rs` 中現有的內部 helper（或等效邏輯）。  
參照 `list.map` 的實作方式抽取 item 並重組。

list 在 n/lang 中以 Combo 表示，形如：
```
{0: val0, 1: val1, ..., n-1: valn-1, %len: n}
```
`extract_list_items` 讀取 "0".."n-1"（或按 `%len`），`build_list_value` 重建相同格式。

如果沒有現成 helper，直接寫 inline：
```rust
// extract
let len = if let Value::Combo(ref lc) = list {
    lc.get_field("%len")
        .and_then(|v| if let Value::Atom(AtomKind::Int(n), ..) = v { n.to_usize() } else { None })
        .unwrap_or(0)
} else { 0 };
let items: Vec<Value> = (0..len)
    .filter_map(|i| if let Value::Combo(ref lc) = list { lc.get_field(&i.to_string()).cloned() } else { None })
    .collect();

// build
let mut out = ComboVal::new();
for (i, v) in result.iter().enumerate() {
    out.data.insert(i.to_string(), v.clone());
}
out.data.insert("%len".to_string(), Value::Atom(AtomKind::Int(result.len().into()), EffectTag::Pure, None));
Value::Combo(out)
```

### 注意事項

- 如果 `f(x)` 返回非 list（如單一 Value），`extract_list_items` 回傳空 vec，那個元素就被丟棄。這是 Monad 語義正確的行為。
- 如果原始 list 為空，結果也為空 list。
- 不需要修改 genesis seeds（standalone builtin）。

### 新增測試

測試檔位置：`tests/list_test.rs` 或新建 `tests/flat_map_test.rs`。

```rust
#[test]
fn test_list_flat_map_empty() {
    // list.flat_map({0: f, 1: []}) → []
    let oo = make_ouroboros();
    let mut ctx = oo.eval_context();
    let f = make_identity_fn(&oo); // 任意函數，不重要
    let empty_list = make_list(&oo, vec![]);
    let arg = make_combo_2(&oo, f, empty_list);
    let result = call_builtin("list.flat_map", arg, &oo, &mut ctx);
    assert_list_len(&result, 0);
}

#[test]
fn test_list_flat_map_doubles() {
    // f(x) = [x, x+x] (list of 2 elements)
    // flat_map([1, 2]) → [1, 2, 2, 4]
    // 此測試用 mock f (返回固定 list) 驗證 flatten 邏輯
    let oo = make_ouroboros();
    let mut ctx = oo.eval_context();
    // f: 返回包含 2 個元素的 list（此處簡化為靜態 [0, 0]）
    let list_in = make_list(&oo, vec![int_val(1), int_val(2)]);
    // 用 list.map + list.concat 等效驗證 flat_map 等於 concat(map(f))
    // 或直接用 make_fn_returns_pair 建立一個 f，對任意 x 返回 [x, x]
    let f = make_fn_dup_list(&oo); // f(x) = [x, x]
    let arg = make_combo_2(&oo, f, list_in);
    let result = call_builtin("list.flat_map", arg, &oo, &mut ctx);
    assert_list_len(&result, 4);
}

#[test]
fn test_list_flat_map_monad_law() {
    // flat_map(pure, [a]) == [a]
    // pure(x) = [x]（單元素 list）
    // flat_map(pure, [42]) → [42]
    let oo = make_ouroboros();
    let mut ctx = oo.eval_context();
    let list_in = make_list(&oo, vec![int_val(42)]);
    let f = make_fn_wrap_singleton(&oo); // f(x) = [x]
    let arg = make_combo_2(&oo, f, list_in);
    let result = call_builtin("list.flat_map", arg, &oo, &mut ctx);
    assert_list_len(&result, 1);
    assert_list_item_int(&result, 0, 42);
}
```

**Note**: 測試 helper 的命名和結構應與現有 `list_test.rs` 一致。如果現有 helper 不足，仿照現有模式撰寫 mock。

---

## Task 3：`NerveEntry.field_keys` + 精確 Čech nerve 交集

### 背景

目前 `NerveEntry.overlapping_masa_caids` 永遠為 `vec![]`，`nerve_overlap()` 的第二個條件（overlapping list）從未觸發。  
兩個 Combo 如果有相同 MASA（完全相同的 field keys）才會通過 nerve 過濾，否則一律通過（empty → true）。

Phase 17 要讓 nerve_overlap 能精確偵測 **部分重疊**：兩個 Combo 共享至少一個 field key 就應該算 nerve 交集，即使 MASA 不同。

### 修改 1：`ladd.rs` — 擴展 NerveEntry

```rust
/// Čech nerve position entry (APP_05 §4.3).
#[derive(Debug, Clone)]
pub struct NerveEntry {
    pub masa_caid: String,
    pub overlapping_masa_caids: Vec<String>,
    pub field_keys: Vec<String>,  // ← 新增：用於動態交集計算
}
```

更新 `nerve_overlap()` 函數，加入 field_keys 交集檢查：

```rust
/// Nerve overlap check (APP_05 §4.3). Empty → passes (no pruning info).
pub fn nerve_overlap(query: &GBB, peer: &GBB) -> bool {
    if query.nerve_structure.is_empty() || peer.nerve_structure.is_empty() {
        return true;
    }
    let query_masas: HashSet<&str> =
        query.nerve_structure.iter().map(|e| e.masa_caid.as_str()).collect();
    let query_keys: HashSet<&str> = query.nerve_structure.iter()
        .flat_map(|e| e.field_keys.iter().map(|k| k.as_str()))
        .collect();

    peer.nerve_structure.iter().any(|pe| {
        // 1. 完全相同的 MASA（最精確）
        query_masas.contains(pe.masa_caid.as_str())
        // 2. 預計算的 overlapping 列表（向後相容）
        || pe.overlapping_masa_caids.iter().any(|m| query_masas.contains(m.as_str()))
        // 3. Field key 直接交集（Phase 17 新增）
        || (!query_keys.is_empty()
            && !pe.field_keys.is_empty()
            && pe.field_keys.iter().any(|k| query_keys.contains(k.as_str())))
    })
}
```

### 修改 2：`builtins/disc.rs` — 填充 field_keys

在 `disc.advertise` 中，建立 NerveEntry 時同時儲存 field_keys：

```rust
// Phase 11: nerve_structure from field key MASA computation
// Phase 17: also store field_keys for dynamic intersection
let nerve_structure: Vec<crate::ladd::NerveEntry> = if let Value::Combo(ref cv) = arg {
    let keys: Vec<String> = cv.all_fields_iter().map(|(k, _)| k).collect();
    if keys.is_empty() {
        vec![]
    } else {
        // Phase 17: compute overlapping_masa_caids from existing registry
        let my_masa = field_key_masa_id(cv);
        let my_key_set: std::collections::HashSet<&str> = keys.iter().map(|s| s.as_str()).collect();
        let overlapping: Vec<String> = if let Ok(reg) = oo.gbb_registry.read() {
            reg.values()
                .flat_map(|g| g.nerve_structure.iter())
                .filter(|ne| ne.masa_caid != my_masa)
                .filter(|ne| ne.field_keys.iter().any(|k| my_key_set.contains(k.as_str())))
                .map(|ne| ne.masa_caid.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect()
        } else { vec![] };

        vec![crate::ladd::NerveEntry {
            masa_caid: my_masa,
            overlapping_masa_caids: overlapping,
            field_keys: keys,
        }]
    }
} else {
    vec![]
};
```

在 `disc.find` 中，建立查詢 GBB 時也填充 field_keys（不需要計算 overlapping）：

```rust
let query_nerve = if let Value::Combo(ref cv) = arg {
    let keys: Vec<String> = cv.all_fields_iter().map(|(k, _)| k).collect();
    if keys.is_empty() { vec![] }
    else { vec![crate::ladd::NerveEntry {
        masa_caid: field_key_masa_id(cv),
        overlapping_masa_caids: vec![],
        field_keys: keys,                // ← 新增
    }] }
} else { vec![] };
```

### 注意事項

- `disc.advertise` 中讀取 `gbb_registry` 是為了計算 overlapping MASA，此時不寫入（只讀），不會死鎖。
- overlapping_masa_caids 使用 HashSet 去重，避免重複。
- 不需要 back-propagation（不修改已存在的 GBB）。新 GBB 知道自己與哪些舊 GBB 重疊；舊 GBB 不知道新 GBB。這在查詢時由 field_keys 直接交集補足，所以沒有問題。
- 所有現有的 `NerveEntry { masa_caid: ..., overlapping_masa_caids: ... }` 結構字面量都需要加 `field_keys: vec![]` 欄位（編譯器會報 missing field）。搜尋整個 codebase 找到所有建立 NerveEntry 的地方，統一加上。

### 修改所有現有 NerveEntry 建立點

搜尋命令：
```bash
grep -rn "NerveEntry {" crates/
```

所有找到的地方都加 `field_keys: vec![]`（如果沒有在 disc.advertise/disc.find 中已按上面修改）。

### 新增測試

測試檔位置：`tests/nerve_routing_test.rs`

```rust
#[test]
fn test_nerve_overlap_same_masa() {
    // Two GBBs with identical field keys → same MASA → overlap
    let ne_a = NerveEntry { masa_caid: "masa:fk:abc".into(), overlapping_masa_caids: vec![], field_keys: vec!["x".into(), "y".into()] };
    let ne_b = NerveEntry { masa_caid: "masa:fk:abc".into(), overlapping_masa_caids: vec![], field_keys: vec!["x".into(), "y".into()] };
    let gbb_a = make_gbb_with_nerve(vec![ne_a]);
    let gbb_b = make_gbb_with_nerve(vec![ne_b]);
    assert!(nerve_overlap(&gbb_a, &gbb_b));
}

#[test]
fn test_nerve_overlap_partial_field_keys() {
    // Two GBBs with different MASA but sharing field key "x" → overlap via field_keys
    let ne_a = NerveEntry { masa_caid: "masa:fk:aaa".into(), overlapping_masa_caids: vec![], field_keys: vec!["x".into(), "y".into()] };
    let ne_b = NerveEntry { masa_caid: "masa:fk:bbb".into(), overlapping_masa_caids: vec![], field_keys: vec!["x".into(), "z".into()] };
    let gbb_a = make_gbb_with_nerve(vec![ne_a]);
    let gbb_b = make_gbb_with_nerve(vec![ne_b]);
    assert!(nerve_overlap(&gbb_a, &gbb_b)); // ← 在 Phase 16 這會是 false！
}

#[test]
fn test_nerve_overlap_disjoint_field_keys() {
    // Two GBBs with different MASA and no shared field keys → no overlap
    let ne_a = NerveEntry { masa_caid: "masa:fk:aaa".into(), overlapping_masa_caids: vec![], field_keys: vec!["x".into(), "y".into()] };
    let ne_b = NerveEntry { masa_caid: "masa:fk:bbb".into(), overlapping_masa_caids: vec![], field_keys: vec!["z".into(), "w".into()] };
    let gbb_a = make_gbb_with_nerve(vec![ne_a]);
    let gbb_b = make_gbb_with_nerve(vec![ne_b]);
    assert!(!nerve_overlap(&gbb_a, &gbb_b)); // disjoint → false
}

#[test]
fn test_nerve_overlap_precomputed_overlapping() {
    // GBB B has overlapping_masa_caids that includes A's masa → overlap
    let ne_a = NerveEntry { masa_caid: "masa:fk:aaa".into(), overlapping_masa_caids: vec![], field_keys: vec![] };
    let ne_b = NerveEntry {
        masa_caid: "masa:fk:bbb".into(),
        overlapping_masa_caids: vec!["masa:fk:aaa".into()], // pre-computed
        field_keys: vec![],
    };
    let gbb_a = make_gbb_with_nerve(vec![ne_a]);
    let gbb_b = make_gbb_with_nerve(vec![ne_b]);
    assert!(nerve_overlap(&gbb_a, &gbb_b)); // via overlapping_masa_caids
}
```

---

## 執行順序

1. **Task 3 先做**：`NerveEntry` 結構變更是 breaking change，需要先修好才能編譯。  
   順序：`ladd.rs` 加 `field_keys` → `grep -rn "NerveEntry {"` 找所有建立點 → 全部加 `field_keys: vec![]` → `disc.advertise`/`disc.find` 按上面改 → `nerve_overlap()` 更新。
   
2. **Task 1 和 Task 2** 可以並行做，互不干涉。

3. 最後 `cargo test` 確認全部通過。

## 驗證清單

```bash
cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：211 tests passing, 0 failed

# 重點確認
cargo test nerve_overlap -- --nocapture   # Task 3 的 4 個新測試
cargo test option_or -- --nocapture       # Task 1
cargo test option_unwrap -- --nocapture   # Task 1
cargo test option_filter -- --nocapture   # Task 1
cargo test flat_map -- --nocapture        # Task 2
```

## 背景知識

- `apply_morphism(f, arg, ctx)` 是 `Ouroboros` 的方法，用來把一個 Value（通常是 Thunk 或 Code）當成函數應用到 arg。
- `oo.force(v, ctx)` 強制求值一個 Thunk。
- n/lang 中 None 表示為 `Value::Atom(AtomKind::Tag("none"), EffectTag::Pure, None)`，`#` prefix 在 `Tag` 內部通常不存在，但 `trim_start_matches('#')` 是防禦性寫法，與現有 `option.map` 一致。
- n/lang list 格式：`{0: v0, 1: v1, ..., %len: n}`（Combo with integer string keys）。
