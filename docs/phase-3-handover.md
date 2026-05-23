# Phase 3 交接文件：#refine 精煉機制

> **執行者**：引擎開發 Agent  
> **預估工作量**：2 週  
> **前置條件**：Phase 1a（BN/ + CAID v2）、Phase 2（StdLib）完成  
> **完成判斷**：通過本文末尾的驗收測試清單

---

## 背景

`#refine` 是 n/ 宇宙的核心進化機制：宣告一個精確節點 $E$「格論上包含」既有模糊節點 $B$（即 $E \sqsubseteq B$），引擎在**活躍觀測**和**暫存區**中自動將對 $B$ 的引用靜默重定向到 $E$。已固化的歷史 Commit 不受影響（因果邊界不可變性）。

Phase 3 的四個子任務：

| 子任務 | 說明 |
|:-------|:-----|
| A. Commit 擴充 | 加入 `CommitKind::Refine` 和 `RefineInfo` 結構 |
| B. RefineMap | `Ouroboros` 中維護 `source → targets` 的記憶體索引 |
| C. 幾何單調性驗證 | 宣告 `refine(old → new)` 時驗證 `new & old = new` |
| D. 自動重定向 + 循環阻斷 | 活躍觀測路徑解析時，透明替換；最大跳轉 16 次 |

---

## 規格書參考

| 任務 | 規格 |
|:-----|:-----|
| `#refine` Commit 結構 | `SPEC_10 §2.5` |
| 幾何單調性成立條件 | `SPEC_10 §2.5`（$ID_{new} \sqsubseteq ID_{old}$） |
| 自動重定向語義 | `SPEC_13 §5.2`（精煉語義與自動重定向） |
| 循環阻斷、深度限制 | `SPEC_13 §5.2.4`（循環阻斷，Max Hops = 16） |
| 不透明 CAID 驗證 | `REAL_03 §9.1` |

規格書位置：`nlang-spec/spec/zh_TW/`

---

## 現有程式碼定位

| 檔案 | 相關位置 |
|:-----|:--------|
| `crates/interpreter/src/value.rs:459` | `Commit` struct — 需擴充 `kind`、`refine_info` |
| `crates/interpreter/src/universe.rs:66` | `Universe::commit()` — 加入 `refine()` 方法 |
| `crates/interpreter/src/lib.rs` | `Ouroboros` struct — 加入 `refine_map` 欄位 |
| `crates/interpreter/src/lib.rs:resolve_path` | 路徑解析 — 加入重定向查詢 |
| `crates/interpreter/src/storage.rs` | `ObjectStore` — 不需修改（存取介面不變） |

---

## 子任務 A：擴充 `Commit` 結構

### 目前的 `Commit`（`value.rs:459`）

```rust
pub struct Commit {
    pub parent: Option<ContentHash>,
    pub root: ContentHash,
    pub meta: CommitMeta,
    pub cache_id: Arc<RwLock<Option<ContentHash>>>,
}
```

### 需要加入

```rust
// value.rs 新增

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommitKind {
    Standard,   // 一般演化 Commit（預設）
    Refine,     // 精煉 Commit
}

impl Default for CommitKind { fn default() -> Self { Self::Standard } }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefineInfo {
    pub source_caids: Vec<ContentHash>,   // 被精煉的舊 CAID（可為聯集）
    pub target_caids: Vec<ContentHash>,   // 精煉目標（可多個，代表幾何拆分）
    // %authority 欄位（Phase 3 不實作簽名驗證，留空即可）
    pub authority_signer: Option<String>, // architect CAID（字串即可，不驗簽）
}

// 更新 Commit struct：
pub struct Commit {
    pub parent: Option<ContentHash>,
    pub root: ContentHash,
    pub meta: CommitMeta,
    #[serde(default)]
    pub kind: CommitKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refine_info: Option<RefineInfo>,
    #[serde(skip, default = "default_cache_id")]
    pub cache_id: Arc<RwLock<Option<ContentHash>>>,
}
```

同時更新 `Commit::new()` 和 `Commit::default()` 補上新欄位（`kind: CommitKind::Standard`, `refine_info: None`）。

---

## 子任務 B：RefineMap

### 在 `Ouroboros` 加入索引

**位置**：`crates/interpreter/src/lib.rs`，`Ouroboros` struct。

```rust
pub struct Ouroboros {
    // 現有欄位...
    pub store: ObjectStore,
    pub unify_memo: RwLock<HashMap<(ContentHash, ContentHash), Value>>,
    // Phase 3 新增：
    pub refine_map: RwLock<HashMap<String, Vec<String>>>,
    // key = source CAID 字串, value = target CAID 字串列表
}
```

`Ouroboros::new()` 中加入：`refine_map: RwLock::new(HashMap::new())`

### 載入時重建索引

在 `Universe::load()` 後，掃描 ObjectStore 的所有 Commit，把 `CommitKind::Refine` 的條目加入 `refine_map`：

```rust
// crates/interpreter/src/lib.rs 或 universe.rs

pub fn rebuild_refine_map(oo: &Ouroboros, base_dir: &Path) -> Result<()> {
    let mut head = oo.store.get_head(base_dir)?;
    let mut visited = HashSet::new();
    while let Some(h) = head {
        if !visited.insert(h.to_string()) { break; }
        let commit = oo.store.get_commit(&h)?;
        if commit.kind == CommitKind::Refine {
            if let Some(ref ri) = commit.refine_info {
                let mut map = oo.refine_map.write().unwrap();
                for src in &ri.source_caids {
                    let targets: Vec<String> = ri.target_caids.iter()
                        .map(|t| t.to_string()).collect();
                    map.entry(src.to_string()).or_default().extend(targets);
                }
            }
        }
        head = commit.parent.clone();
    }
    Ok(())
}
```

---

## 子任務 C：幾何單調性驗證

**位置**：`Universe::refine()`（新方法，`universe.rs`）

精煉成立的充要條件：$E \sqsubseteq B$，等價於 $E \sqcap B = E$（meet 後等於精煉目標）。

```rust
// crates/interpreter/src/universe.rs

impl Universe {
    /// 建立 #refine Commit，驗證幾何單調性後加入 RefineMap
    pub fn refine(
        &mut self,
        engine: &Ouroboros,
        base_dir: &Path,
        source_caids: Vec<ContentHash>,
        target_caids: Vec<ContentHash>,
        meta: CommitMeta,
    ) -> Result<ContentHash> {
        // 步驟 1：幾何單調性驗證（若 source 和 target 都是 Live 值）
        // $E \sqcap B = E$，即 new & old = new
        for src in &source_caids {
            for tgt in &target_caids {
                if let (Ok(src_val), Ok(tgt_val)) = (
                    engine.store.get_value(src),
                    engine.store.get_value(tgt)
                ) {
                    let meet = engine.unify(tgt_val.clone(), src_val.clone());
                    // meet 應等於 tgt_val（新值更精確）
                    if meet.content_hash() != tgt_val.content_hash() {
                        return Err(anyhow::anyhow!(
                            "Refinement failed monotonicity: new ⋢ old (new & old ≠ new)"
                        ));
                    }
                }
                // 若任一 CAID 不透明（無法 get_value），略過驗證（由 authority 背書）
            }
        }

        // 步驟 2：建立 Refine Commit（root 繼承當前 HEAD 的 root）
        let current_root_hash = match &self.head {
            Some(h) => engine.store.get_commit(h)?.root.clone(),
            None => engine.store.put_value(&Value::Combo(self.root.clone()))?,
        };
        let commit = Commit {
            parent: self.head.clone(),
            root: current_root_hash,
            meta,
            kind: CommitKind::Refine,
            refine_info: Some(RefineInfo {
                source_caids: source_caids.clone(),
                target_caids: target_caids.clone(),
                authority_signer: None,
            }),
            cache_id: default_cache_id(),
        };
        let commit_hash = engine.store.put_commit(&commit)?;
        engine.store.set_head(base_dir, &commit_hash)?;
        self.head = Some(commit_hash.clone());

        // 步驟 3：更新 RefineMap
        let mut map = engine.refine_map.write().unwrap();
        for src in &source_caids {
            let targets: Vec<String> = target_caids.iter().map(|t| t.to_string()).collect();
            map.entry(src.to_string()).or_default().extend(targets);
        }

        Ok(commit_hash)
    }
}
```

---

## 子任務 D：自動重定向 + 循環阻斷

### 設計原則（SPEC_13 §5.2）

- **允許重定向**：Live Observation（`observe()`）和 Staged Area（`evolve()` 中的路徑解析）
- **禁止重定向**：歷史 Commit 的 `get_value()`（直接回傳原始值，不追蹤重定向）
- **最大跳轉次數**：16 次（創世預設值）

### 實作位置：新增 `follow_refine()` 輔助函式

**位置**：`crates/interpreter/src/lib.rs`（在 `Ouroboros` impl 中）

```rust
const MAX_REFINE_HOPS: usize = 16;

impl Ouroboros {
    /// 在 Live 觀測時，追蹤 RefineMap，回傳最終目標 CAID
    /// 如遇循環或超過深度 → 回傳 Err
    pub fn follow_refine(&self, caid: &ContentHash) -> Result<ContentHash, BottomCause> {
        let mut current = caid.to_string();
        let mut visited = HashSet::new();

        for _ in 0..MAX_REFINE_HOPS {
            if !visited.insert(current.clone()) {
                // 循環偵測
                return Err(BottomCause::Divergent); // 代表 #refinement_cycle
            }
            let map = self.refine_map.read().unwrap();
            match map.get(&current) {
                None => break, // 無更多重定向，返回當前
                Some(targets) if targets.is_empty() => break,
                Some(targets) => {
                    // 取第一個有效目標（未來 Phase 4 可做多目標消融）
                    current = targets[0].clone();
                }
            }
        }

        ContentHash::parse(&current).map_err(|_| BottomCause::InvalidPath)
    }

    /// 在 Live 觀測中取值，自動追蹤 refine 重定向
    pub fn get_live_value(&self, caid: &ContentHash) -> Result<Value> {
        let resolved = self.follow_refine(caid)
            .map_err(|_| anyhow::anyhow!("Refinement cycle detected"))?;
        self.store.get_value(&resolved)
    }
}
```

### 整合到 `Universe::observe()`

**位置**：`universe.rs:82`

目前的 `observe()` 直接 unify root + staged。Phase 3 之後，路徑解析到一個 CAID 引用時，應透過 `get_live_value()` 取值：

```rust
pub fn observe(&self, engine: &Ouroboros, path: &Path) -> Value {
    let current = engine.unify(
        Value::Combo(self.root.clone()),
        Value::Combo(self.staged.clone())
    );
    if let Value::Combo(r) = current {
        let mut ctx = EvalContext::new(r);
        ctx.refine_map_active = true; // Phase 3 新增旗標
        engine.resolve_path(path, &mut ctx)
    } else {
        BottomCause::Conflict.into()
    }
}
```

在 `EvalContext` 加入旗標：

```rust
// lib.rs 中 EvalContext struct
pub struct EvalContext {
    // 現有欄位...
    pub refine_map_active: bool, // Phase 3：是否啟用 live refine 重定向
}

// EvalContext::new() 中：
refine_map_active: false, // 預設不啟用，observe() 才啟用
```

在 `eval.rs` 的 CAID 引用解析處（`AtomKind::Str` 含 `"hash:"` 前綴時），加入重定向查詢：

```rust
// eval.rs 路徑解析相關分支：
if ctx.refine_map_active {
    if let Ok(resolved) = oo.follow_refine(&caid) {
        // 使用 resolved 取值，而非原始 caid
    }
}
```

---

## CLI 接口（選做）

在 `oo/src/main.rs` 加入 `oo refine` 子命令，讓使用者從命令列執行精煉：

```
oo refine --from <old-caid> --to <new-caid> [--message "..."]
```

如果時間不夠，可略過 CLI，Phase 3 只需引擎層 API 可用即可。

---

## 修改檔案清單

| 檔案 | 動作 | 說明 |
|:-----|:-----|:-----|
| `crates/interpreter/src/value.rs` | **修改** | 加入 `CommitKind`、`RefineInfo`；更新 `Commit` struct |
| `crates/interpreter/src/lib.rs` | **修改** | `Ouroboros` 加 `refine_map`；加 `follow_refine()`、`get_live_value()`；`EvalContext` 加 `refine_map_active` |
| `crates/interpreter/src/universe.rs` | **修改** | 加入 `Universe::refine()`；`observe()` 傳遞 `refine_map_active` |
| `crates/interpreter/src/eval.rs` | **修改** | CAID 引用解析時，若 `ctx.refine_map_active` 則呼叫 `follow_refine()` |

---

## 驗收測試清單

### Commit 結構

- [ ] `CommitKind::Standard` 和 `CommitKind::Refine` 存在
- [ ] `RefineInfo` 有 `source_caids`、`target_caids` 欄位
- [ ] 一般 `commit()` 產生的 Commit 的 `kind` 為 `Standard`

### 幾何單調性驗證

- [ ] 宣告 `refine(old → new)` 且 `new & old ≠ new` 時，`Universe::refine()` 回傳 `Err`
- [ ] 宣告 `refine(old → new)` 且 `new & old = new` 時，成功建立 Commit 並更新 `refine_map`

### 自動重定向

- [ ] `follow_refine(source)` 在 `refine_map` 有對應時，回傳 target CAID
- [ ] `follow_refine(source)` 在無對應時，回傳原始 CAID
- [ ] 循環鏈（A→B→A）觸發 `Divergent`（`#refinement_cycle`）
- [ ] 超過 16 跳轉時，`follow_refine()` 回傳 `Err`
- [ ] `Universe::observe()` 路徑解析時，對 CAID 引用自動追蹤 refine 重定向
- [ ] 已固化的歷史 Commit 的 `get_value()` **不追蹤**重定向（直接回傳原始值）

### 回歸測試

- [ ] `cargo build` 無錯誤
- [ ] `cargo test` 73 tests + 新增測試全部通過

---

## 不在 Phase 3 範圍內

| 項目 | 延後至 |
|:-----|:------|
| Ed25519 `%authority` 簽名驗證 | Phase 4（需要 crypto 依賴） |
| 影子精煉（Shadow Refinement，背景測試） | Phase 5 |
| 目標多重性消融（多個 target 的 union 坍縮） | Phase 4 |
| `oo refine` CLI 子命令 | Phase 3 選做 / Phase 4 |
| `#blur` 作為真正的 Value 狀態 | 待議（目前 Phase 2 以 Bottom(NumericalError) 暫代） |

---

## 快速定位

```bash
# Commit 結構位置
grep -n "pub struct Commit" crates/interpreter/src/value.rs

# Universe commit 位置
grep -n "pub fn commit" crates/interpreter/src/universe.rs

# Ouroboros struct
grep -n "pub struct Ouroboros" crates/interpreter/src/lib.rs

# 全部 BottomDetail 建構（refine_cycle 用 Divergent）
grep -rn "BottomCause::" crates/interpreter/src/

# 建置
cargo build -p nlang-interpreter
cargo test -p nlang-interpreter
```
