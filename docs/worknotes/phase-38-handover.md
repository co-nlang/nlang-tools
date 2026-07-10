# Phase 38 Handover：NerveEntry.field_keys 精確交集

> 日期：2026-05-25  
> 實作範圍：disc.rs 精確 field_keys 過濾  
> 預期測試：~429 → ~434（5 個新測試，加入 nerve_routing_test.rs）

---

## 0. 問題診斷

**現象**：`disc.advertise` 和 `disc.find` 使用 `all_fields_iter()` 收集 field_keys，包含 `%morphism`、`%builtin`、`%kind`、`~%Config` 等 meta/system 欄位。

**影響**：
- 態射節點 `{%morphism: #true, %builtin: "str.len"}` 的 MASA 基於 `[%builtin, %morphism]` 計算，而非語義內容
- 兩個語義相同但 `%kind` 不同的節點，MASA 可能不同
- Čech nerve 用 implementation 細節做路由，而非語義結構

**修正**：在 `field_key_masa_id`、`disc.advertise`、`disc.find` 三處過濾出 `%`- 和 `~%`-前綴的 key，只保留語義欄位（data keys、`/`-prefixed rule keys、`@`-prefixed type keys）。

---

## 1. 修改 `builtins/disc.rs`

**只需修改三個地方，全部在同一個檔案。**

### 修改 1：`field_key_masa_id` 函數（第 24 行附近）

```rust
// 舊：
fn field_key_masa_id(cv: &crate::value::ComboVal) -> String {
    use sha2::{Sha256, Digest};
    let mut keys: Vec<String> = cv.all_fields_iter().map(|(k, _)| k).collect();
    keys.sort();
    let joined = keys.join("\x00");
    let digest = Sha256::digest(joined.as_bytes());
    format!("masa:fk:{}", hex::encode(&digest[..8]))
}

// 新：
fn field_key_masa_id(cv: &crate::value::ComboVal) -> String {
    use sha2::{Sha256, Digest};
    let mut keys: Vec<String> = cv.all_fields_iter()
        .map(|(k, _)| k)
        .filter(|k| !k.starts_with('%') && !k.starts_with("~%"))
        .collect();
    keys.sort();
    let joined = keys.join("\x00");
    let digest = Sha256::digest(joined.as_bytes());
    format!("masa:fk:{}", hex::encode(&digest[..8]))
}
```

### 修改 2：`disc.advertise` 的 `keys` 收集（第 122–123 行附近）

```rust
// 舊：
let nerve_structure: Vec<crate::ladd::NerveEntry> = if let Value::Combo(ref cv) = arg {
    let keys: Vec<String> = cv.all_fields_iter().map(|(k, _)| k).collect();

// 新：
let nerve_structure: Vec<crate::ladd::NerveEntry> = if let Value::Combo(ref cv) = arg {
    let keys: Vec<String> = cv.all_fields_iter()
        .map(|(k, _)| k)
        .filter(|k| !k.starts_with('%') && !k.starts_with("~%"))
        .collect();
```

### 修改 3：`disc.find` 的 `keys` 收集（第 164–165 行附近）

```rust
// 舊：
let query_nerve = if let Value::Combo(ref cv) = arg {
    let keys: Vec<String> = cv.all_fields_iter().map(|(k, _)| k).collect();

// 新：
let query_nerve = if let Value::Combo(ref cv) = arg {
    let keys: Vec<String> = cv.all_fields_iter()
        .map(|(k, _)| k)
        .filter(|k| !k.starts_with('%') && !k.starts_with("~%"))
        .collect();
```

---

## 2. 過濾邏輯說明

```
all_fields_iter() 回傳的 key 前綴：

  data  keys   → "x", "y", "name", "0", "1"   ← 保留
  rules keys   → "/len", "/map"                ← 保留 (/ 前綴)
  types keys   → "@option", "@list"            ← 保留 (@ 前綴)
  meta  keys   → "%kind", "%morphism", "%builtin", "%val", "%cause" ← 過濾
  system keys  → "~%Config", "~%Math"          ← 過濾 (starts_with "~%")

filter: !k.starts_with('%') && !k.starts_with("~%")
```

**結果**：
- 純資料節點 `{name: "Alice", age: 30}` → keys = `["age", "name"]` ✓
- 態射節點 `{%morphism: #true, %builtin: "str.len"}` → keys = `[]` → 空 nerve → 不參與 nerve 過濾 ✓
- 列表節點 `{%kind: #list, 0: v0, 1: v1}` → keys = `["0", "1"]`（不含 `%kind`）✓
- 兩個 `{x: 1}` 和 `{x: 2}` → 相同 MASA（只看 key，不看 value）✓

---

## 3. 現有測試影響分析

所有現有 `nerve_routing_test.rs` 測試均**不受影響**：

| 測試 | 原因 |
|:-----|:-----|
| `nerve_overlap_same_field_structure` | 使用 `"x"`, `"y"` data keys（不含 `%`），行為不變 |
| `nerve_different_field_structure_different_masa` | 使用 `"x"`, `"y"` vs `"a"`, `"b"`，行為不變 |
| `nerve_non_combo_empty_structure` | Atom 節點，nerve 本來就空 |
| `test_nerve_overlap_*`（unit tests） | 直接構造 `NerveEntry`，不經過 disc.advertise，不受影響 |

---

## 4. 新增測試（加入 `nerve_routing_test.rs` 尾端）

在現有測試之後新增一個 section：

```rust
// ── Phase 38: 精確交集 field_keys 過濾 ──

#[test]
fn test_morphism_node_gets_empty_nerve() {
    // 態射節點的所有欄位都是 %-前綴，過濾後為空 → 不參與 nerve 路由
    use nlang_interpreter::*;
    use nlang_interpreter::value::{ComboVal, EffectTag};
    use nlang_parser::ast::AtomKind;
    use indexmap::IndexMap;
    use std::sync::Arc;

    let oo = Arc::new(Ouroboros::new_in_memory());
    let mut ctx = EvalContext::new(oo.root_with_system());

    // 模擬一個態射節點
    let mut fields = IndexMap::new();
    fields.insert("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
    fields.insert("%builtin".to_string(), Value::Atom(AtomKind::Str("str.len".to_string()), EffectTag::Pure, None));
    let cv = ComboVal::new(fields, true, IndexMap::new(), EffectTag::Pure, vec![]);

    let advertise_fn = oo.builtin_registry.get("disc.advertise").unwrap();
    advertise_fn(Value::Combo(cv), &oo, &mut ctx);

    let reg = oo.gbb_registry.read().unwrap();
    let all_empty = reg.values().all(|gbb| gbb.nerve_structure.is_empty());
    assert!(all_empty, "morphism node (only %-keys) → empty nerve_structure");
}

#[test]
fn test_data_node_nerve_excludes_percent_keys() {
    // 混合節點：data keys + % keys → nerve 只含 data keys
    use nlang_interpreter::*;
    use nlang_interpreter::value::{ComboVal, EffectTag};
    use nlang_parser::ast::AtomKind;
    use indexmap::IndexMap;
    use std::sync::Arc;

    let oo = Arc::new(Ouroboros::new_in_memory());
    let mut ctx = EvalContext::new(oo.root_with_system());

    let mut fields = IndexMap::new();
    fields.insert("name".to_string(), Value::Atom(AtomKind::Str("Alice".to_string()), EffectTag::Pure, None));
    fields.insert("age".to_string(), Value::Atom(AtomKind::Int(30.into()), EffectTag::Pure, None));
    fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("person".to_string()), EffectTag::Pure, None));
    let cv = ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]);

    let advertise_fn = oo.builtin_registry.get("disc.advertise").unwrap();
    advertise_fn(Value::Combo(cv), &oo, &mut ctx);

    let reg = oo.gbb_registry.read().unwrap();
    let all_nerves: Vec<_> = reg.values()
        .filter(|gbb| !gbb.nerve_structure.is_empty())
        .flat_map(|gbb| gbb.nerve_structure[0].field_keys.iter().cloned())
        .collect();

    assert!(!all_nerves.is_empty(), "data+%kind node should have non-empty nerve");
    assert!(
        all_nerves.iter().all(|k| !k.starts_with('%') && !k.starts_with("~%")),
        "nerve field_keys must not contain %-prefixed keys, got: {:?}", all_nerves
    );
    assert!(all_nerves.contains(&"name".to_string()), "nerve should contain 'name'");
    assert!(all_nerves.contains(&"age".to_string()), "nerve should contain 'age'");
}

#[test]
fn test_same_structure_diff_percent_same_masa() {
    // 兩個 data-field 相同但 %kind 不同的節點 → 相同 MASA
    use nlang_interpreter::*;
    use nlang_interpreter::value::{ComboVal, EffectTag};
    use nlang_parser::ast::AtomKind;
    use indexmap::IndexMap;
    use std::sync::Arc;

    let oo = Arc::new(Ouroboros::new_in_memory());
    let mut ctx = EvalContext::new(oo.root_with_system());

    let mut f1 = IndexMap::new();
    f1.insert("x".to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None));
    f1.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("foo".to_string()), EffectTag::Pure, None));
    let cv1 = ComboVal::new(f1, false, IndexMap::new(), EffectTag::Pure, vec![]);

    let mut f2 = IndexMap::new();
    f2.insert("x".to_string(), Value::Atom(AtomKind::Int(2.into()), EffectTag::Pure, None));
    f2.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("bar".to_string()), EffectTag::Pure, None));
    let cv2 = ComboVal::new(f2, false, IndexMap::new(), EffectTag::Pure, vec![]);

    let advertise_fn = oo.builtin_registry.get("disc.advertise").unwrap();
    advertise_fn(Value::Combo(cv1), &oo, &mut ctx);
    advertise_fn(Value::Combo(cv2), &oo, &mut ctx);

    let reg = oo.gbb_registry.read().unwrap();
    let masas: Vec<_> = reg.values()
        .filter(|gbb| !gbb.nerve_structure.is_empty())
        .map(|gbb| gbb.nerve_structure[0].masa_caid.clone())
        .collect();

    assert!(masas.len() >= 2, "should have at least 2 advertised nodes with nerve");
    // All should have same MASA (same semantic field "x", ignoring %kind)
    let first = &masas[0];
    assert!(masas.iter().all(|m| m == first),
        "same data-field structure → same MASA regardless of %kind: {:?}", masas);
}

#[test]
fn test_list_node_nerve_uses_index_keys() {
    // 列表節點 {%kind:#list, 0:v0, 1:v1} → nerve uses ["0","1"], not "%kind"
    use nlang_interpreter::*;
    use nlang_interpreter::value::{ComboVal, EffectTag};
    use nlang_parser::ast::AtomKind;
    use indexmap::IndexMap;
    use std::sync::Arc;

    let oo = Arc::new(Ouroboros::new_in_memory());
    let mut ctx = EvalContext::new(oo.root_with_system());

    let mut fields = IndexMap::new();
    fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    fields.insert("0".to_string(), Value::Atom(AtomKind::Int(10.into()), EffectTag::Pure, None));
    fields.insert("1".to_string(), Value::Atom(AtomKind::Int(20.into()), EffectTag::Pure, None));
    let cv = ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]);

    let advertise_fn = oo.builtin_registry.get("disc.advertise").unwrap();
    advertise_fn(Value::Combo(cv), &oo, &mut ctx);

    let reg = oo.gbb_registry.read().unwrap();
    let nerve_keys: Vec<_> = reg.values()
        .filter(|gbb| !gbb.nerve_structure.is_empty())
        .flat_map(|gbb| gbb.nerve_structure[0].field_keys.iter().cloned())
        .collect();

    assert!(nerve_keys.contains(&"0".to_string()), "list nerve should have key '0'");
    assert!(nerve_keys.contains(&"1".to_string()), "list nerve should have key '1'");
    assert!(!nerve_keys.iter().any(|k| k.starts_with('%')),
        "list nerve must not contain %kind: {:?}", nerve_keys);
}

#[test]
fn test_empty_after_filter_is_transparent() {
    // 過濾後為空的節點（只有 % 欄位）→ nerve_structure 為空 → nerve_overlap 回 true
    use nlang_interpreter::ladd::{GBB, NerveEntry, nerve_overlap};
    use nlang_interpreter::value::{ContentHash, MasaRef, HashAlgorithm, CaidVersion};

    let dummy = ContentHash { algorithm: HashAlgorithm::Sha256, version: CaidVersion::V2,
        masa_ref: MasaRef::Top, lattice_sketch: String::new(), digest: vec![0; 32] };

    // 模擬一個被過濾後空 nerve 的節點和一個有 nerve 的節點
    let empty_nerve = GBB { node_caid: dummy.clone(), mass: 1.0, sketch_bytes: vec![],
        masa_ref: MasaRef::Top, nerve_structure: vec![] };
    let has_nerve = GBB { node_caid: dummy, mass: 1.0, sketch_bytes: vec![],
        masa_ref: MasaRef::Top,
        nerve_structure: vec![NerveEntry {
            masa_caid: "masa:fk:abc".into(),
            overlapping_masa_caids: vec![],
            field_keys: vec!["x".into()],
        }],
    };

    // 任一方 nerve 為空 → overlap = true（不過濾，透明通過）
    assert!(nerve_overlap(&empty_nerve, &has_nerve), "empty nerve passes through");
    assert!(nerve_overlap(&has_nerve, &empty_nerve), "symmetric: empty nerve passes through");
}
```

---

## 5. 完成後驗證

```bash
cargo test
```

預期：~434 tests，0 failed。

重點確認：
- `test_morphism_node_gets_empty_nerve`: 態射節點 nerve 為空
- `test_data_node_nerve_excludes_percent_keys`: 混合節點 nerve 只含 data keys
- `test_same_structure_diff_percent_same_masa`: `%kind` 不影響 MASA 計算
- `test_list_node_nerve_uses_index_keys`: 列表的 "0","1" 進入 nerve，`%kind` 不進入
- `test_empty_after_filter_is_transparent`: 空 nerve 節點透明通過 nerve_overlap
- 所有現有 `nerve_routing_test.rs` 測試仍通過（data keys 不受影響）

---

## 6. 實作注意事項

| 事項 | 說明 |
|:-----|:-----|
| 三處必須一致過濾 | `field_key_masa_id`、`disc.advertise` 的 keys、`disc.find` 的 keys — 過濾邏輯完全相同 |
| `~%` 的判斷 | `k.starts_with("~%")` 必須獨立於 `k.starts_with('%')` — `~%Config` 不以 `%` 開頭 |
| 無 genesis/lib.rs 變更 | 本 Phase 只改 disc.rs，不影響 root_with_system()，不需重算種子 |
| 無 Cargo.toml 變更 | 測試加入現有 nerve_routing_test.rs，不新增 `[[test]]` 入口 |
| `ComboVal::new(fields, ...)` in tests | `fields` 放入 `data` submap — 所有 data keys 不含 `%`，測試正確 |
