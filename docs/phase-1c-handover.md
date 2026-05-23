# Phase 1c 交接文件：MASA 創世種子 CAID

> **執行者**：引擎開發 Agent  
> **預估工作量**：0.5 週  
> **前置條件**：Phase 1a 完成（BN/ 序列化與 `content_hash_v1()` 可用）  
> **完成判斷**：通過本文末尾的驗收測試清單

---

## 背景

CAID 系統有個自舉悖論：要計算 `~%Math` 的 CAID，需要引擎先運行；但引擎運行需要 `~%Math` 的 CAID 已知。n/ 的解法是「創世種子 (Seed Nodes)」——引擎在**編譯時**內建一組固定的 v1 CAID，作為宇宙起點（SPEC_13 §3.1）。

Phase 1c 的任務：

1. **計算**每個內建模組（`~%Math`、`~%List` 等）的穩定 v1 CAID
2. **硬編碼**這些 CAID 為常數
3. **在 `root_with_system()` 中加入 `%id` 欄位**，讓每個系統模組知道自己的 CAID

---

## 規格書參考

| 任務 | 規格 |
|:-----|:-----|
| 種子節點清單與語義 | `SPEC_13 §3.1`（引擎內建種子） |
| 創世 Commit 結構 | `SPEC_13 §3.2`（Genesis Commit） |
| v1 CAID 格式 | `REAL_03 §2.1` |

規格書位置：`nlang-spec/spec/zh_TW/`

---

## 需要建立種子的模組清單

依據 `SPEC_13 §3.1`，以下是**必須**有硬編碼種子 CAID 的核心模組：

| 種子路徑（`root_with_system()` 的欄位名） | 角色 |
|:------------------------------------------|:-----|
| `~%Math` | 算術與 EML 運算 |
| `~%List` | 列表處理原語 |
| `~%Cond` | 態射與條件控制（對應規格的 `~%Logic`） |
| `~%Engine` | 觀測與演化原語（`~%Engine`/`~%Discovery` 合併） |
| `~%Discovery` | 發現與 CAID 解析 |

另外建議也加入：`~%String`、`~%Complex`、`~%Reflection`、`~%Time`（引擎已有的模組）。

---

## 步驟一：計算並硬編碼種子 CAID

### 做法

Phase 1a 的 `content_hash_v1()` 已經可以對任意 `Value` 計算 v1 CAID。種子 CAID 就是對 `root_with_system()` 中每個系統 Combo 計算 `content_hash_v1()` 的結果。

**一次性計算步驟**（執行一次，把結果硬編碼）：

```rust
// 臨時測試程式碼（用完刪除）
let root = oo.root_with_system();
let math_val = root.get_field("~%Math").unwrap();
println!("SEED_MATH = {}", math_val.content_hash_v1());

let list_val = root.get_field("~%List").unwrap();
println!("SEED_LIST = {}", list_val.content_hash_v1());
// ... 依此類推
```

執行 `cargo run -p oo -- repl`（或寫一個 test）把輸出記錄下來。

### 硬編碼位置

新建 `crates/interpreter/src/genesis.rs`：

```rust
// crates/interpreter/src/genesis.rs
// 種子 CAID 由 Phase 1c 一次性計算後固定，不得在執行時重算。

pub const SEED_MATH:      &str = "hash:sha256:v1:<計算所得 64 hex>";
pub const SEED_LIST:      &str = "hash:sha256:v1:<計算所得 64 hex>";
pub const SEED_COND:      &str = "hash:sha256:v1:<計算所得 64 hex>";
pub const SEED_ENGINE:    &str = "hash:sha256:v1:<計算所得 64 hex>";
pub const SEED_DISCOVERY: &str = "hash:sha256:v1:<計算所得 64 hex>";
pub const SEED_STRING:    &str = "hash:sha256:v1:<計算所得 64 hex>";
pub const SEED_COMPLEX:   &str = "hash:sha256:v1:<計算所得 64 hex>";
pub const SEED_REFL:      &str = "hash:sha256:v1:<計算所得 64 hex>";
pub const SEED_TIME:      &str = "hash:sha256:v1:<計算所得 64 hex>";

/// 返回所有種子的 (路徑, CAID 字串) 列表
pub fn all_seeds() -> Vec<(&'static str, &'static str)> {
    vec![
        ("~%Math",       SEED_MATH),
        ("~%List",       SEED_LIST),
        ("~%Cond",       SEED_COND),
        ("~%Engine",     SEED_ENGINE),
        ("~%Discovery",  SEED_DISCOVERY),
        ("~%String",     SEED_STRING),
        ("~%Complex",    SEED_COMPLEX),
        ("~%Reflection", SEED_REFL),
        ("~%Time",       SEED_TIME),
    ]
}
```

在 `lib.rs` 加入 `mod genesis;`。

---

## 步驟二：在 `root_with_system()` 加入 `%id` 欄位

**位置**：`crates/interpreter/src/lib.rs`，`root_with_system()` 函式。

對每個系統 Combo，在插入 `fields` **之前**，先把 `%id` 注入該 Combo 的 meta 欄位：

```rust
use crate::genesis;

// 現有程式碼（節錄）：
let mut math_builtins = IndexMap::new();
// ... 建立 math_builtins ...

// 新增：注入 %id
math_builtins.insert(
    "%id".to_string(),
    Value::Atom(AtomKind::Str(genesis::SEED_MATH.to_string()), EffectTag::Pure, None)
);

fields.insert("~%Math".to_string(), Value::Combo(
    ComboVal::new(math_builtins, true, IndexMap::new(), EffectTag::Pure, vec![])
));
```

對 `~%List`、`~%Cond`、`~%Discovery`、`~%String`、`~%Complex`、`~%Reflection`、`~%Time` 依此類推。

> **注意**：`%id` 是引擎自省欄位（SPEC_09 §6.1），使用者嚴禁賦值，但引擎可以在建構系統根時注入。

---

## 步驟三：驗證種子穩定性

加入一個 `#[test]` 確認種子 CAID 不會因引擎修改而改變：

```rust
// crates/interpreter/tests/genesis_test.rs
#[test]
fn seed_caids_are_stable() {
    use nlang_interpreter::{Ouroboros, genesis};
    let oo = Ouroboros::new();
    let root = oo.root_with_system();
    
    let math_val = root.get_field("~%Math").expect("~%Math missing");
    let computed = math_val.content_hash_v1().to_string();
    assert_eq!(computed, genesis::SEED_MATH,
        "~%Math 的種子 CAID 已改變！若這是刻意修改，請更新 genesis.rs 的常數並重新凍結。");
    
    // 對 List、Cond 等其他模組重複相同斷言...
}
```

這個測試平時會通過。**當有人修改 `root_with_system()` 中的任何一個模組時，它會失敗**——強迫開發者有意識地重新凍結種子，而不是意外改動。

---

## 步驟四（選做）：創世 Commit $C_0$

如果時間允許，可以在 `storage.rs` 加入 `genesis_commit()` 函式：

```rust
// crates/interpreter/src/storage.rs

pub fn genesis_commit(store: &ObjectStore, root: &ComboVal) -> Result<ContentHash> {
    use crate::genesis::all_seeds;
    // 1. 將 root_with_system() 的每個系統模組存入 ObjectStore
    // 2. 建立 Commit C_0：parent=None，用 content_hash_v1() 計算
    // 3. 寫入 HEAD
    // 4. 回傳 C_0 的 CAID
    todo!("Phase 1c optional: genesis commit")
}
```

**此步驟不阻斷 Phase 1c 驗收**，但建議做，因為 Phase 4 的 LADD 路由需要有初始化好的 ObjectStore。

---

## 修改檔案清單

| 檔案 | 動作 | 說明 |
|:-----|:-----|:-----|
| `crates/interpreter/src/genesis.rs` | **新建** | 所有種子 CAID 常數 + `all_seeds()` |
| `crates/interpreter/src/lib.rs` | **修改** | `mod genesis;` + `root_with_system()` 中各模組加 `%id` |
| `crates/interpreter/tests/genesis_test.rs` | **新建** | 種子穩定性驗證測試 |

---

## 驗收測試清單

- [ ] `genesis.rs` 存在，所有 `SEED_*` 常數為真實計算的 64-hex SHA256（非 placeholder）
- [ ] `root_with_system()` 中每個系統 Combo 有 `%id` 欄位
- [ ] `seed_caids_are_stable()` 測試通過
- [ ] `cargo build` 無錯誤
- [ ] `cargo test` 全部通過（包含新測試）

---

## 不在 Phase 1c 範圍內

| 項目 | 延後至 |
|:-----|:------|
| 非 Top 的 `masa_ref`（MASA 自身的種子） | Phase 4 |
| 創世 Commit 寫入磁碟 | Phase 1c 選做，或 Phase 4 |
| 跨版本種子 `#refine`（格式版本升級時） | Phase 4 / SPEC_10 |
