# Phase 20 交接文件

> 狀態：待實作  
> 前置：Phase 19 完成（248 tests passing）  
> 目標：動態 Combo 操作 — `refl.get` / `refl.set` / `refl.delete` / `refl.values` / `refl.entries`

---

## 概覽

| 任務 | 位置 | 說明 |
|:-----|:-----|:-----|
| Task 1 | `value.rs` | 新增 `ComboVal::remove_field()` |
| Task 2 | `builtins/reflection.rs` | 5 個新 builtins |
| Task 3 | `lib.rs` + `genesis.rs` | 更新 `refl_morphisms` + `SEED_REFL` |

新增測試數：8（`tests/refl_dynamic_test.rs`）  
預期完成後：248 + 8 ≈ **256 tests**

---

## 語義總覽

```
refl.get    : {0: key_str, 1: combo} → Value | Top
  按字串 key 動態讀取欄位（key 包含前綴，如 "name", "%meta", "@type", "/rule"）
  存在 → 欄位值；不存在 → Top

refl.set    : {0: key_str, 1: value, 2: combo} → Combo
  函數式更新：返回一個新 Combo，其中 key 欄位被設為 value
  不修改原 Combo（複製後設定）

refl.delete : {0: key_str, 1: combo} → Combo
  函數式刪除：返回一個新 Combo，其中 key 欄位被移除
  key 不存在時，返回原 Combo（無副作用）

refl.values : combo → List of values
  與 refl.keys 鏡像：相同的 key 篩選與排序，返回對應的 value 列表
  順序與 refl.keys 對齊（refl.keys(c)[i] 對應 refl.values(c)[i]）

refl.entries : combo → List of {key: str, val: Value}
  返回 {key: "field_name", val: field_value} 的 Combo 列表
  篩選與排序同 refl.keys
```

**關於 `refl.keys` 的篩選規則**（現有實作）：
```rust
c.fields().keys().filter(|k| !k.starts_with('%')).cloned().collect()
// 即：排除 %meta 欄位，保留 data、/rules、@types、~%system 欄位
```
`refl.values` 和 `refl.entries` 使用完全相同的篩選。

---

## Task 1：新增 `ComboVal::remove_field()`

### 位置

`crates/interpreter/src/value.rs`，在 `contains_key()` 之後（約 line 220 之後）插入：

```rust
pub fn remove_field(&mut self, key: &str) {
    let key_trimmed = key.trim();
    if key_trimmed.starts_with("~%") {
        self.system.shift_remove(&key_trimmed[2..]);
    } else if key_trimmed.starts_with('/') {
        self.rules.shift_remove(&key_trimmed[1..]);
    } else if key_trimmed.starts_with('@') {
        self.types.shift_remove(&key_trimmed[1..]);
    } else if key_trimmed.starts_with('%') {
        self.meta.shift_remove(&key_trimmed[1..]);
    } else if key_trimmed.starts_with('~') {
        self.local.shift_remove(&key_trimmed[1..]);
    } else {
        self.data.shift_remove(key_trimmed);
    }
}
```

**注意**：`IndexMap` 的刪除方法是 `shift_remove`（保持順序）或 `swap_remove`（不保持順序）。  
使用 `shift_remove` 以保持欄位插入順序，與現有 `fields()` 的一致性一致。

確認 `IndexMap` 的 `shift_remove` 方法簽名：
```rust
// indexmap::IndexMap::shift_remove(key) -> Option<V>
```
如果版本不支援 `shift_remove`，改用 `.remove(key_trimmed)`（`swap_remove` 的別名）。

---

## Task 2：5 個新 builtins

### 位置

`crates/interpreter/src/builtins/reflection.rs`，加在 `refl.bottom_cause` 之後（`}` 之前）。

```rust
// ── Phase 20: Dynamic Combo access ───────────────────────────

m.insert("refl.get".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(vkey), Some(vobj)) = (c.get_field("0"), c.get_field("1")) {
            let key = oo.force(vkey.clone(), ctx).to_string_plain();
            let obj = oo.force(vobj.clone(), ctx);
            if let Value::Combo(ref oc) = obj.collapse() {
                return oc.get_field(&key).cloned().unwrap_or(Value::Top);
            }
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);

m.insert("refl.set".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(vkey), Some(vval), Some(vobj)) =
            (c.get_field("0"), c.get_field("1"), c.get_field("2"))
        {
            let key = oo.force(vkey.clone(), ctx).to_string_plain();
            let val = oo.force(vval.clone(), ctx);
            let obj = oo.force(vobj.clone(), ctx);
            if let Value::Combo(ref oc) = obj.collapse() {
                let mut new_combo = oc.clone();
                new_combo.insert_field(&key, val);
                return Value::Combo(new_combo);
            }
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);

m.insert("refl.delete".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(vkey), Some(vobj)) = (c.get_field("0"), c.get_field("1")) {
            let key = oo.force(vkey.clone(), ctx).to_string_plain();
            let obj = oo.force(vobj.clone(), ctx);
            if let Value::Combo(ref oc) = obj.collapse() {
                let mut new_combo = oc.clone();
                new_combo.remove_field(&key);
                return Value::Combo(new_combo);
            }
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);

m.insert("refl.values".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = if let Value::Combo(ref c) = arg {
        c.get_field("0").cloned().unwrap_or_else(|| arg.clone())
    } else { arg.clone() };
    if let Value::Combo(c) = oo.force(v, ctx).collapse() {
        let mut pairs: Vec<(String, Value)> = c.fields().into_iter()
            .filter(|(k, _)| !k.starts_with('%'))
            .collect();
        pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
        let mut res = IndexMap::new();
        for (i, (_, val)) in pairs.into_iter().enumerate() {
            res.insert(i.to_string(), val);
        }
        res.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
        return Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]));
    }
    Value::Top
}) as Arc<BuiltinFn>);

m.insert("refl.entries".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = if let Value::Combo(ref c) = arg {
        c.get_field("0").cloned().unwrap_or_else(|| arg.clone())
    } else { arg.clone() };
    if let Value::Combo(c) = oo.force(v, ctx).collapse() {
        let mut pairs: Vec<(String, Value)> = c.fields().into_iter()
            .filter(|(k, _)| !k.starts_with('%'))
            .collect();
        pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
        let mut res = IndexMap::new();
        for (i, (key, val)) in pairs.into_iter().enumerate() {
            let mut entry = IndexMap::new();
            entry.insert("key".to_string(), Value::Atom(AtomKind::Str(key), EffectTag::Pure, None));
            entry.insert("val".to_string(), val);
            let entry_combo = ComboVal::new(entry, true, IndexMap::new(), EffectTag::Pure, vec![]);
            res.insert(i.to_string(), Value::Combo(entry_combo));
        }
        res.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
        return Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]));
    }
    Value::Top
}) as Arc<BuiltinFn>);
```

### 注意事項

- `refl.get` 與 `refl.has` 參數格式完全一致：`{0: key, 1: combo}`。
- `refl.set` 三個參數：`{0: key, 1: value, 2: combo}`（key 在前，與 refl.get 一致，target 在最後）。
- `refl.set` 和 `refl.delete` 都是**函數式**（不可變）：`clone()` 後修改，返回新 Combo。原 Combo 不受影響。
- `refl.entries` 的 entry Combo 使用 `closed: true`（Cocoon 格式），因為它是固定結構的 pair。
- `refl.values` 和 `refl.entries` 的篩選與排序邏輯與現有 `refl.keys` 完全一致，確保三者索引對齊。
- `refl.get` 返回 `Top` 而非 `Bottom` 表示「不存在」，與 `list.at` 找不到元素時的行為一致。

---

## Task 3：更新 `refl_morphisms` + `SEED_REFL`

### 3a：更新 `lib.rs`

搜尋 `refl_morphisms` 在 `lib.rs` 中的位置（`root_with_system()` 函數）。  
找到 `let refl_morphisms = vec![...]` 或類似的地方，加入 5 個新名稱：

```rust
// 在現有 refl_morphisms vec 末尾加入（參照 Phase 16 加入的 is_blur 等）：
"refl.get",
"refl.set",
"refl.delete",
"refl.values",
"refl.entries",
```

### 3b：更新 `SEED_REFL`

`refl_morphisms` 改變了 `~%Refl` Combo 的內容，因此 CAID 會變。需要重新生成常數：

```bash
cargo test --manifest-path crates/interpreter/Cargo.toml \
    seed_caids_are_stable -- --nocapture 2>&1 | grep "SEED_REFL\|Actual"
```

把輸出的新值更新到 `crates/interpreter/src/genesis.rs` 中的 `SEED_REFL` 常數。

**驗證**：更新後重跑 `cargo test` 確認 `seed_caids_are_stable` 測試通過。

---

## Task 4：測試（`tests/refl_dynamic_test.rs`）

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}
fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}

fn make_combo(fields: Vec<(&str, Value)>) -> Value {
    let mut cv = ComboVal::default();
    for (k, v) in fields { cv.insert_field(k, v); }
    Value::Combo(cv)
}

fn make_combo_2(a: Value, b: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), a);
    f.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_combo_3(a: Value, b: Value, c: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), a);
    f.insert("1".to_string(), b);
    f.insert("2".to_string(), c);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}

fn is_top(v: &Value) -> bool { matches!(v, Value::Top) }

#[test]
fn test_refl_get_existing() {
    // refl.get({0: "name", 1: {name: "Alice", age: 30}}) → "Alice"
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let obj = make_combo(vec![("name", str_val("Alice")), ("age", int_val(30))]);
    let arg = make_combo_2(str_val("name"), obj);
    let r = call(&oo, &mut ctx, "refl.get", arg);
    assert_eq!(r.to_string_plain(), "Alice");
}

#[test]
fn test_refl_get_missing() {
    // refl.get({0: "nonexistent", 1: {name: "Alice"}}) → Top
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let obj = make_combo(vec![("name", str_val("Alice"))]);
    let arg = make_combo_2(str_val("nonexistent"), obj);
    let r = call(&oo, &mut ctx, "refl.get", arg);
    assert!(is_top(&r), "missing key should return Top, got {:?}", r);
}

#[test]
fn test_refl_set_new_field() {
    // refl.set({0: "city", 1: "Taipei", 2: {name: "Alice"}}) → {name: "Alice", city: "Taipei"}
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let obj = make_combo(vec![("name", str_val("Alice"))]);
    let arg = make_combo_3(str_val("city"), str_val("Taipei"), obj.clone());
    let r = call(&oo, &mut ctx, "refl.set", arg);
    if let Value::Combo(ref cv) = r {
        assert_eq!(cv.get_field("name").unwrap().to_string_plain(), "Alice");
        assert_eq!(cv.get_field("city").unwrap().to_string_plain(), "Taipei");
    } else { panic!("expected Combo, got {:?}", r); }
    // 原 obj 不受影響
    if let Value::Combo(ref cv) = obj {
        assert!(cv.get_field("city").is_none(), "original combo should be unchanged");
    }
}

#[test]
fn test_refl_set_update_field() {
    // refl.set({0: "age", 1: 31, 2: {name: "Alice", age: 30}}) → {name: "Alice", age: 31}
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let obj = make_combo(vec![("name", str_val("Alice")), ("age", int_val(30))]);
    let arg = make_combo_3(str_val("age"), int_val(31), obj);
    let r = call(&oo, &mut ctx, "refl.set", arg);
    if let Value::Combo(ref cv) = r {
        assert_eq!(cv.get_field("age").unwrap().to_string_plain(), "31");
    } else { panic!("expected Combo"); }
}

#[test]
fn test_refl_delete_existing() {
    // refl.delete({0: "age", 1: {name: "Alice", age: 30}}) → {name: "Alice"}
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let obj = make_combo(vec![("name", str_val("Alice")), ("age", int_val(30))]);
    let arg = make_combo_2(str_val("age"), obj);
    let r = call(&oo, &mut ctx, "refl.delete", arg);
    if let Value::Combo(ref cv) = r {
        assert!(cv.get_field("age").is_none(), "age should be removed");
        assert_eq!(cv.get_field("name").unwrap().to_string_plain(), "Alice");
    } else { panic!("expected Combo"); }
}

#[test]
fn test_refl_delete_missing_is_noop() {
    // refl.delete({0: "city", 1: {name: "Alice"}}) → {name: "Alice"} (unchanged)
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let obj = make_combo(vec![("name", str_val("Alice"))]);
    let arg = make_combo_2(str_val("city"), obj);
    let r = call(&oo, &mut ctx, "refl.delete", arg);
    if let Value::Combo(ref cv) = r {
        assert_eq!(cv.get_field("name").unwrap().to_string_plain(), "Alice");
    } else { panic!("expected Combo"); }
}

#[test]
fn test_refl_values_parallel_to_keys() {
    // {a: 1, b: 2, c: 3} → values should be [1, 2, 3] (sorted by key: a, b, c)
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let obj = make_combo(vec![("c", int_val(3)), ("a", int_val(1)), ("b", int_val(2))]);
    let keys_r = call(&oo, &mut ctx, "refl.keys", obj.clone());
    let vals_r  = call(&oo, &mut ctx, "refl.values", obj);
    // Both should have 3 elements; keys[0]="a" corresponds to vals[0]=1
    if let (Value::Combo(ref kc), Value::Combo(ref vc)) = (&keys_r, &vals_r) {
        let k0 = kc.get_field("0").unwrap().to_string_plain();
        let v0 = vc.get_field("0").unwrap().to_string_plain();
        assert_eq!(k0, "a");
        assert_eq!(v0, "1");
        // All three present
        assert!(kc.get_field("2").is_some());
        assert!(vc.get_field("2").is_some());
    } else { panic!("expected list combos"); }
}

#[test]
fn test_refl_entries_format() {
    // {x: 10} → [{key: "x", val: 10}]
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let obj = make_combo(vec![("x", int_val(10))]);
    let r = call(&oo, &mut ctx, "refl.entries", obj);
    if let Value::Combo(ref lc) = r {
        let entry = lc.get_field("0").expect("should have entry at index 0");
        if let Value::Combo(ref ec) = entry {
            assert_eq!(ec.get_field("key").unwrap().to_string_plain(), "x");
            assert_eq!(ec.get_field("val").unwrap().to_string_plain(), "10");
        } else { panic!("entry should be Combo, got {:?}", entry); }
    } else { panic!("expected list Combo"); }
}
```

---

## 執行順序

1. **Task 1 先做**：`value.rs` 加 `remove_field()`，確認可以編譯。
2. **Task 2**：`reflection.rs` 加 5 個 builtins。
3. **Task 3a**：`lib.rs` 的 `refl_morphisms` 加 5 個新名稱。
4. **Task 3b**：執行測試取得新 SEED_REFL，更新 `genesis.rs`。
5. **Task 4**：新建 `tests/refl_dynamic_test.rs`。

```bash
cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：~256 tests, 0 failed
```

## 設計說明

**為什麼 `refl.get` 缺少欄位返回 Top 而非 Bottom？**  
Top 在格論中是「萬有子空間」——不否認任何可能性。這與 `list.at` 對越界 index 返回 Top 的行為一致。如果呼叫者需要區分「找到」和「找不到」，應先用 `refl.has` 確認。

**為什麼 `refl.set` / `refl.delete` 不可變（複製語義）？**  
n/lang 是值語義（value semantics）語言，所有更新都返回新值。這與 `list.slice`、`option.map` 等其他操作一致。

**`refl.entries` 的 entry 格式 `{key, val}` 而非 `{key, value}`？**  
與 `option`/`result` 慣用的 `%val` 前綴保持視覺一致性（雖然 entry 這裡沒有 `%` 前綴，因為 `key` 和 `val` 是普通資料欄位）。
