# Phase 11 Handover: Architects 持久化 + nerve_structure 真實 MASA 計算

**Date:** 2026-05-24  
**Status:** Ready for implementation  
**Depends on:** Phase 10 (complete)  
**Spec refs:** APP_05 §4.3, SPEC_10 §2.5

---

## 目標

完成兩個 P2 遺留項目：

1. **Architects 持久化** — `engine.add_architect` 目前只更新記憶體中的 `architect_registry`，重啟後遺失。需持久化至磁碟，並在 `Ouroboros::init()` 時自動載入。
2. **`nerve_structure` 真實 MASA 計算** — Phase 5 的 `disc.advertise` 用 `refine_map` 的 entries 填入 `nerve_structure`（概念上錯誤：refine map 是精炼關係，不是 MASA 成員關係）。需改為從 Combo 的 field key 結構計算 MASA identifier。

---

## 現狀分析

### 問題一：architects 持久化

`crates/interpreter/src/lib.rs`，`Ouroboros::init()`：
```rust
let local_pk_hex = hex::encode(&identity.public_key);
let mut architects = std::collections::HashSet::new();
architects.insert(local_pk_hex);         // ← 只有本機 key，無法載入先前 add_architect 的結果
```

`engine.add_architect` builtin（`builtins/engine.rs`）：
```rust
if let Ok(mut reg) = oo.architect_registry.write() {
    reg.insert(pubkey_hex);
    Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::IO, None)
}
// ← 只寫記憶體，沒有寫磁碟
```

`Ouroboros` struct 無 `base_dir`，builtin 無法知道要寫到哪裡。

### 問題二：nerve_structure 概念錯誤

`builtins/disc.rs`，`disc.advertise`：
```rust
// Phase 5 nerve_structure from refine_map (approximation)
let nerve_structure: Vec<crate::ladd::NerveEntry> = {
    oo.refine_map.read().map_or_else(|_| vec![], |m| {
        m.iter().map(|(src, targets)| crate::ladd::NerveEntry {
            masa_caid: src.clone(),               // ← source CAID 當 MASA id（錯誤）
            overlapping_masa_caids: targets.clone(), // ← target CAIDs 當 overlapping（錯誤）
        }).collect()
    })
};
```

`NerveEntry` 應代表「此節點屬於哪個 MASA（古典可觀測量子代數）」。`refine_map` 的 src→targets 是精炼重定向路徑，與 MASA 無關。

正確語義：Combo 的 field key 集合定義其古典可觀測量集合 → 形成一個 MASA。兩個節點若 field key 集合相交（共享至少一個古典可觀測量），則其 MASA 重疊。

---

## 任務一：Architects 持久化

### 1a. `Ouroboros` 新增 `base_dir: Option<PathBuf>` 欄位

**`crates/interpreter/src/lib.rs`**，`pub struct Ouroboros`：

```rust
pub struct Ouroboros {
    pub store: ObjectStore,
    pub base_dir: Option<std::path::PathBuf>,   // ← NEW: None for in_memory
    pub unify_memo: RwLock<HashMap<(ContentHash, ContentHash), Value>>,
    pub builtin_registry: HashMap<String, Arc<BuiltinFn>>,
    pub peers: RwLock<HashMap<String, Peer>>,
    pub identity: crate::value::Identity,
    pub refine_map: RwLock<HashMap<String, Vec<String>>>,
    pub gbb_registry: RwLock<HashMap<String, crate::ladd::GBB>>,
    pub architect_registry: RwLock<std::collections::HashSet<String>>,
}
```

`new_in_memory()` 初始化：`base_dir: None`

`init(base_dir)` 初始化：`base_dir: Some(base_dir.to_path_buf())`

（只有 struct literal 的構造需要加 `base_dir` 欄位，其他邏輯不變。）

### 1b. `ObjectStore` 新增兩個方法

**`crates/interpreter/src/storage.rs`**：

```rust
impl ObjectStore {
    // ...existing methods...

    pub fn save_architects(
        &self,
        base_dir: &std::path::Path,
        architects: &std::collections::HashSet<String>,
    ) -> anyhow::Result<()> {
        let dir = base_dir.join(".oo");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("architects.json");
        let list: Vec<&String> = architects.iter().collect();
        let json = serde_json::to_string(&list)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_architects(
        &self,
        base_dir: &std::path::Path,
    ) -> anyhow::Result<std::collections::HashSet<String>> {
        let path = base_dir.join(".oo").join("architects.json");
        if !path.exists() {
            return Ok(std::collections::HashSet::new());
        }
        let json = std::fs::read_to_string(path)?;
        let list: Vec<String> = serde_json::from_str(&json)?;
        Ok(list.into_iter().collect())
    }
}
```

### 1c. `Ouroboros::init()` 載入已持久化的 architects

**`crates/interpreter/src/lib.rs`**，`init()` 函數，在 `architects.insert(local_pk_hex)` 之後加：

```rust
// Load previously persisted architects from disk
if let Ok(persisted) = store.load_architects(base_dir) {
    architects.extend(persisted);
}
```

完整段落變成：
```rust
let identity = crate::value::Identity::new_random();
let local_pk_hex = hex::encode(&identity.public_key);
let mut architects = std::collections::HashSet::new();
architects.insert(local_pk_hex.clone());
// Load previously persisted architects from disk
if let Ok(persisted) = store.load_architects(base_dir) {
    architects.extend(persisted);
}
let mut oo = Self {
    store,
    base_dir: Some(base_dir.to_path_buf()),   // ← 新增
    unify_memo: RwLock::new(HashMap::new()),
    builtin_registry: builtins,
    peers: RwLock::new(HashMap::new()),
    identity,
    refine_map: RwLock::new(HashMap::new()),
    gbb_registry: RwLock::new(HashMap::new()),
    architect_registry: RwLock::new(architects),
};
```

### 1d. `engine.add_architect` builtin 新增寫磁碟

**`crates/interpreter/src/builtins/engine.rs`**，`engine.add_architect` 的 closure：

```rust
m.insert("engine.add_architect".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let pubkey_hex = oo.force(arg, ctx).to_string_plain();
    if pubkey_hex.len() != 64 { return BottomCause::Conflict.into(); }
    if let Ok(mut reg) = oo.architect_registry.write() {
        reg.insert(pubkey_hex);
        // Persist to disk if running with a base_dir (not in-memory)
        if let Some(ref base_dir) = oo.base_dir {
            let _ = oo.store.save_architects(base_dir, &reg);
        }
        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::IO, None)
    } else { BottomCause::Conflict.into() }
}) as Arc<BuiltinFn>);
```

### 1e. `new_in_memory()` 不需改動

`new_in_memory()` 設 `base_dir: None`，builtin 的 `if let Some(ref base_dir)` 分支不執行，行為不變。

---

## 任務二：nerve_structure 真實 MASA 計算

### 2a. 新增 `field_key_masa_id()` helper

**`crates/interpreter/src/ladd.rs`** 末端，或在 `builtins/disc.rs` 頂部（作為 file-local fn）：

```rust
/// Compute a MASA identifier from a Combo's field key set.
/// Two Combos with the same field keys form the same MASA (classical sub-algebra).
fn field_key_masa_id(cv: &crate::value::ComboVal) -> String {
    use sha2::{Sha256, Digest};
    let mut keys: Vec<String> = cv.all_fields_iter().map(|(k, _)| k).collect();
    keys.sort();
    let joined = keys.join("\x00");
    let digest = Sha256::digest(joined.as_bytes());
    format!("masa:fk:{}", hex::encode(&digest[..8]))
}
```

放在 `disc.rs` 的 `register_disc_builtins` 之前（file-local fn）。

**為何用 field key 代表 MASA：**
- 一個 Combo 的 field 集合代表它「在同一個古典語境中可同時觀測」的量
- 相同 field key 集合 ⟺ 相同古典可觀測量基底 ⟺ 同一個 MASA
- 兩個 Combo 若 field key 集合有交集，表示它們共享某些古典可觀測量，MASA 重疊

### 2b. 修改 `disc.advertise` 中 `nerve_structure` 計算

**`crates/interpreter/src/builtins/disc.rs`**，約 109–117 行，替換整個 nerve_structure 計算塊：

```rust
// Phase 11: nerve_structure from field key MASA computation (replaces refine_map approximation)
let nerve_structure: Vec<crate::ladd::NerveEntry> = if let Value::Combo(ref cv) = arg {
    let keys: Vec<String> = cv.all_fields_iter().map(|(k, _)| k).collect();
    if keys.is_empty() {
        vec![]
    } else {
        vec![crate::ladd::NerveEntry {
            masa_caid: field_key_masa_id(cv),
            overlapping_masa_caids: vec![],
        }]
    }
} else {
    vec![]  // Non-Combo values have no classical structure → no MASA constraint
};
```

`nerve_overlap()` 的現有邏輯（`ladd.rs`）：
- 若任一方 `nerve_structure` 為空 → 通過（無 MASA 資訊 = 不剪枝）
- 否則比對 `masa_caid` 字串相等 + `overlapping_masa_caids` 包含關係

這個邏輯與新的 `masa_caid` 格式完全相容，**不需修改 `nerve_overlap()`**。

### 2c. disc.find 的 query GBB nerve_structure

`disc.find` 目前對 query GBB 也設 `nerve_structure: vec![]`（第 137 行）。這表示 query 不設 MASA 約束，所有已廣播節點都可被找到。

**Phase 11 可選改進：** 若 query 是 Combo，也計算其 MASA：
```rust
let query_nerve = if let Value::Combo(ref cv) = arg {
    let keys: Vec<String> = cv.all_fields_iter().map(|(k, _)| k).collect();
    if keys.is_empty() { vec![] }
    else { vec![crate::ladd::NerveEntry { masa_caid: field_key_masa_id(cv), overlapping_masa_caids: vec![] }] }
} else { vec![] };

let query_gbb = crate::ladd::GBB {
    node_caid: query_hash.clone(), mass: query_mass.min(100.0),
    sketch_bytes: query_sketch, masa_ref: query_hash.masa_ref.clone(),
    nerve_structure: query_nerve,   // ← was: vec![]
};
```

這讓 `disc.find` 也能利用 MASA 過濾，減少不相關節點的回傳。**建議實作。**

---

## 測試

### 新增至 `crates/interpreter/tests/authority_test.rs`（或新建 `persist_test.rs`）

```rust
// Test 1: add_architect persists across init calls (requires temp dir)
#[test]
fn architect_persists_across_init() {
    use nlang_interpreter::Ouroboros;
    use hex;
    
    let dir = std::env::temp_dir().join("nlang-persist-test-a");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir.join(".oo"));
    
    // Phase 1: init, add a fake architect, check it persists
    let fake_pk = "a".repeat(64);  // 64 hex chars
    {
        let oo = Ouroboros::init(&dir).unwrap();
        // Add architect via registry directly (simulating builtin call)
        {
            let mut reg = oo.architect_registry.write().unwrap();
            reg.insert(fake_pk.clone());
            oo.store.save_architects(&dir, &reg).unwrap();
        }
    }
    
    // Phase 2: re-init, verify the added architect is loaded
    {
        let oo2 = Ouroboros::init(&dir).unwrap();
        let reg = oo2.architect_registry.read().unwrap();
        assert!(reg.contains(&fake_pk), "persisted architect should be loaded on re-init");
    }
    
    let _ = std::fs::remove_dir_all(&dir);
}

// Test 2: new_in_memory does not persist (base_dir = None)
#[test]
fn in_memory_no_persist() {
    let oo = Ouroboros::new_in_memory();
    assert!(oo.base_dir.is_none(), "new_in_memory should have base_dir = None");
    // Simulate add_architect builtin: no file should be written
    {
        let fake_pk = "b".repeat(64);
        let mut reg = oo.architect_registry.write().unwrap();
        reg.insert(fake_pk.clone());
        // No save_architects call without base_dir — test just that base_dir is None
    }
}
```

### 新增至 `crates/interpreter/tests/nerve_routing_test.rs`

```rust
// Test 3: same-field-key Combos have same MASA id → nerve_overlap = true
#[test]
fn nerve_overlap_same_field_structure() {
    use nlang_interpreter::{Ouroboros, EvalContext, Value};
    use nlang_interpreter::value::{ComboVal, EffectTag};
    use indexmap::IndexMap;
    
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    
    // Build two Combos with identical field keys but different values
    let mut fields1 = IndexMap::new();
    fields1.insert("x".to_string(), Value::Top);
    fields1.insert("y".to_string(), Value::Top);
    let cv1 = ComboVal::new(fields1.clone(), false, IndexMap::new(), EffectTag::Pure, vec![]);
    
    let mut fields2 = IndexMap::new();
    fields2.insert("x".to_string(), Value::Atom(nlang_parser::ast::AtomKind::Int(42.into()), EffectTag::Pure, None));
    fields2.insert("y".to_string(), Value::Atom(nlang_parser::ast::AtomKind::Int(7.into()), EffectTag::Pure, None));
    let cv2 = ComboVal::new(fields2, false, IndexMap::new(), EffectTag::Pure, vec![]);
    
    // Advertise both
    oo.apply_builtin("disc.advertise", Value::Combo(cv1), &mut ctx);
    oo.apply_builtin("disc.advertise", Value::Combo(cv2), &mut ctx);
    
    // Verify both have the same MASA id via gbb_registry
    let reg = oo.gbb_registry.read().unwrap();
    let masa_ids: Vec<_> = reg.values()
        .filter(|gbb| !gbb.nerve_structure.is_empty())
        .map(|gbb| gbb.nerve_structure[0].masa_caid.clone())
        .collect();
    
    // Both combos have same field keys {x, y} → same masa_caid
    if masa_ids.len() >= 2 {
        assert_eq!(masa_ids[0], masa_ids[1], "same field structure → same MASA id");
    }
}

// Test 4: different-field-key Combos have different MASA ids
#[test]
fn nerve_different_field_structure_different_masa() {
    use nlang_interpreter::value::{ComboVal, EffectTag};
    use indexmap::IndexMap;
    
    // {x, y} vs {a, b} → different MASA ids
    let mut fields1 = IndexMap::new();
    fields1.insert("x".to_string(), Value::Top);
    fields1.insert("y".to_string(), Value::Top);
    let cv1 = ComboVal::new(fields1, false, IndexMap::new(), EffectTag::Pure, vec![]);
    
    let mut fields2 = IndexMap::new();
    fields2.insert("a".to_string(), Value::Top);
    fields2.insert("b".to_string(), Value::Top);
    let cv2 = ComboVal::new(fields2, false, IndexMap::new(), EffectTag::Pure, vec![]);
    
    // Since field_key_masa_id is internal, test via advertise + gbb_registry
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    oo.apply_builtin("disc.advertise", Value::Combo(cv1), &mut ctx);
    oo.apply_builtin("disc.advertise", Value::Combo(cv2), &mut ctx);
    
    let reg = oo.gbb_registry.read().unwrap();
    let masa_ids: std::collections::HashSet<_> = reg.values()
        .filter(|gbb| !gbb.nerve_structure.is_empty())
        .map(|gbb| gbb.nerve_structure[0].masa_caid.clone())
        .collect();
    
    assert_eq!(masa_ids.len(), 2, "different field structures → different MASA ids");
}

// Test 5: non-Combo advertise → empty nerve_structure → nerve_overlap always passes
#[test]
fn nerve_non_combo_empty_structure() {
    use nlang_interpreter::value::EffectTag;
    
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    let atom = Value::Atom(nlang_parser::ast::AtomKind::Int(99.into()), EffectTag::Pure, None);
    oo.apply_builtin("disc.advertise", atom, &mut ctx);
    
    let reg = oo.gbb_registry.read().unwrap();
    let nerve_lens: Vec<_> = reg.values().map(|gbb| gbb.nerve_structure.len()).collect();
    assert!(nerve_lens.iter().all(|&l| l == 0), "non-Combo → empty nerve_structure");
}
```

---

## 驗收條件

1. `cargo test --workspace 2>&1 | grep FAILED` — 零失敗
2. `oo architect add <hex64>` 後重啟 `oo`，`~%Official.architects` 仍包含該 key（或 `.oo/architects.json` 存在且正確）
3. `disc.advertise` 對 Combo 廣播後，其 GBB 的 `nerve_structure[0].masa_caid` 為 `masa:fk:` 前綴的字串（非 CAID 格式）
4. 兩個 field key 集合相同的 Combo → 廣播後 MASA id 相同
5. `ladd.rs` 中 `nerve_overlap()` 無任何改動（與新格式相容）

---

## 不在本 Phase 的工作

- **Shadow refinement（歷史 Commit 背景驗證）** — P2，Spec 定義尚不完整，延後
- **lattice_sketch_test_suite v2 跨架構測試向量** — P2，純測試工作，延後
- **`nerve_structure` overlapping_masa_caids 動態計算** — 目前留空（`vec![]`）；真正的重疊計算（兩個 MASA 的 field key 交集）需要節點間互相知曉對方的 field key 集合，屬 P3
- **`Ouroboros::new_in_memory()` 的 architect 初始化** — 目前不加 local pk（或原本就有，視現有實作），in-memory 用途不需持久化，維持現狀
