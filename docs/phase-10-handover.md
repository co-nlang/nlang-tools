# Phase 10 Handover: `oo refine` CLI + `bootstrap_exempt` Epoch 判定

**Date:** 2026-05-24  
**Status:** Ready for implementation  
**Depends on:** Phase 8 (authority signing, complete), Phase 9 (complete)  
**Spec refs:** SPEC_10 §2.5, SPEC_13 §5.2

---

## 目標

完成兩個 P1 遺留項目：

1. **`oo refine` CLI 子命令** — 讓使用者從命令列宣告 Source CAID → Target CAID 精炼，可選自動簽署
2. **`bootstrap_exempt = true` 常數 → 真實 Epoch 判定** — 當系統有 HEAD 且有已登記 Architect 時，強制要求 `%authority` 簽署

---

## 現狀

### `universe.rs:114`
```rust
let bootstrap_exempt = true; // TODO Phase 9: set false when Epoch >= 0
```
這行使所有 `#refine` Commit 永遠免簽，Phase 8 的 `engine.add_architect` / `engine.sign_refine` 完全沒有強制效力。

### `crates/oo/src/main.rs`
`Commands` 枚舉目前有：`Run`, `Evolve`, `Test`, `Repl`, `Status`, `Log`, `Commit`, `Fmt`, `Serve`。無 `Refine`。`Universe::refine()` 函數本身已在 `universe.rs` 實作完畢，只缺 CLI 入口。

---

## 任務一：`bootstrap_exempt` → Epoch 判定

### 改動位置：`crates/interpreter/src/universe.rs`，`refine()` 函數

**原始程式碼（約 113–122 行）：**
```rust
// Step 1b: authority verification (Phase 8)
let bootstrap_exempt = true; // TODO Phase 9: set false when Epoch >= 0
let payload = crate::authority::compute_refine_payload(&source_caids, &target_caids);
let architect_reg = engine.architect_registry.read().map_err(|e| anyhow::anyhow!("{:?}", e))?;
match crate::authority::verify_refine_authority(authority.as_ref(), &payload, &architect_reg, bootstrap_exempt) {
```

**修改後：**
```rust
// Step 1b: authority verification
let payload = crate::authority::compute_refine_payload(&source_caids, &target_caids);
let architect_reg = engine.architect_registry.read().map_err(|e| anyhow::anyhow!("{:?}", e))?;
// Epoch judgment: exempt only in genesis state (no HEAD) OR before any architect registered
let bootstrap_exempt = self.head.is_none() || architect_reg.is_empty();
match crate::authority::verify_refine_authority(authority.as_ref(), &payload, &architect_reg, bootstrap_exempt) {
```

**變更說明：**
- 移除 `let bootstrap_exempt = true;` 這行（和其 TODO 註解）
- 把 `architect_reg` read **移到** `bootstrap_exempt` 計算之前（原本在後，現在合為一個 block）
- `bootstrap_exempt = self.head.is_none() || architect_reg.is_empty()`：
  - `self.head.is_none()` — Genesis 狀態，還沒有任何 Commit，雞蛋問題：第一個 refine 必須能免簽
  - `architect_reg.is_empty()` — 沒有登記任何 Architect，無法驗證任何簽章，只能免簽
  - 兩者只要有一個成立 → 免簽（bootstrap）
  - 兩者都不成立（有 HEAD 且有 Architect）→ 強制要求簽章

**不影響現有測試：** 所有 `refine_test.rs` 使用 `Ouroboros::new_in_memory()`，其 `architect_registry` 為空（或只有 local pubkey 視實作而定），且以 `Universe::load(path)` 得到 `head = None`。兩個豁免條件至少有一個成立，現有測試行為不變。

---

## 任務二：`oo refine` CLI 子命令

### 改動位置：`crates/oo/src/main.rs`

#### 2a. 新增 `Refine` 到 `Commands` 枚舉

在 `Commit` 行之後加入：

```rust
Refine {
    /// Source CAID(s) — the broader/vaguer values being refined away
    #[arg(short, long, required = true, num_args = 1..)]
    source: Vec<String>,
    /// Target CAID(s) — the precise values being refined toward
    #[arg(short, long, required = true, num_args = 1..)]
    target: Vec<String>,
    /// Auto-sign with local identity key
    #[arg(long)]
    sign: bool,
    /// Commit message
    #[arg(short, long)]
    message: Option<String>,
},
```

#### 2b. 新增 match arm

在 `main()` 的 match 塊中加入：

```rust
Commands::Refine { source, target, sign, message } => run_refine(source, target, sign, message),
```

#### 2c. 新增 `run_refine()` 函數

```rust
fn run_refine(
    sources: Vec<String>,
    targets: Vec<String>,
    sign: bool,
    message: Option<String>,
) -> anyhow::Result<()> {
    let cur = std::env::current_dir()?;
    let engine = Ouroboros::init(&cur)?;
    let mut universe = load_universe(&engine, &cur)?;

    let source_caids: Vec<ContentHash> = sources
        .iter()
        .map(|s| ContentHash::parse(s)
            .map_err(|e| anyhow::anyhow!("Invalid source CAID '{}': {}", s, e)))
        .collect::<anyhow::Result<_>>()?;

    let target_caids: Vec<ContentHash> = targets
        .iter()
        .map(|s| ContentHash::parse(s)
            .map_err(|e| anyhow::anyhow!("Invalid target CAID '{}': {}", s, e)))
        .collect::<anyhow::Result<_>>()?;

    let authority = if sign {
        let payload = nlang_interpreter::authority::compute_refine_payload(
            &source_caids,
            &target_caids,
        );
        let auth = nlang_interpreter::authority::sign_refine(&payload, &engine.identity)
            .map_err(|e| anyhow::anyhow!("Signing failed: {}", e))?;
        Some(auth)
    } else {
        None
    };

    let meta = CommitMeta {
        message,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as u64,
        author: Some("oo-cli".to_string()),
    };

    let hash = universe.refine(&engine, &cur, source_caids, target_caids, authority, meta)?;
    println!("Refine commit: {}", hash);
    Ok(())
}
```

#### 2d. 新增 import（若尚未有）

```rust
use nlang_interpreter::AuthorityInfo;  // 若 run_refine 需要明示型別
```

`nlang_interpreter::authority` 模組和 `compute_refine_payload`、`sign_refine` 應已在 Phase 8 設為 `pub`。若編譯時找不到，在 `crates/interpreter/src/lib.rs` 確認有：

```rust
pub mod authority;
```

---

## 測試：新增至 `crates/interpreter/tests/refine_test.rs`

### Test A：`bootstrap_exempt` — 有 HEAD 但無 Architect → 仍免簽（architect_reg 空）

```rust
#[test]
fn bootstrap_exempt_when_no_architects() {
    let oo = Arc::new(Ouroboros::new_in_memory());
    // Verify architect_registry is empty (or only local key by design)
    // Either way: after first refine, head is set, but if registry is empty → still exempt

    let base_dir = std::env::temp_dir().join("nlang-refine-epoch-a");
    let _ = std::fs::create_dir_all(&base_dir);

    // Clear architect_registry to simulate "no architects" state
    { oo.architect_registry.write().unwrap().clear(); }

    let mut u = Universe::new(None, oo.root_with_system());

    let val_a = Value::Top;
    let val_b = Value::Atom(nlang_parser::ast::AtomKind::Int(100.into()), EffectTag::Pure, None);
    let ca = oo.store.put_value(&val_a).unwrap();
    let cb = oo.store.put_value(&val_b).unwrap();

    // First refine: head=None → exempt (regardless of architect_reg)
    let meta1 = CommitMeta { author: None, timestamp: 0, message: None };
    u.refine(&oo, &base_dir, vec![ca.clone()], vec![cb.clone()], None, meta1).unwrap();
    assert!(u.head.is_some());

    // Second refine: head is set, architect_reg is empty → still exempt
    let val_c = Value::Atom(nlang_parser::ast::AtomKind::Int(99.into()), EffectTag::Pure, None);
    let cb2 = oo.store.put_value(&val_c).unwrap();
    let ca2 = Value::Top;
    let ca2_hash = oo.store.put_value(&ca2).unwrap();
    let meta2 = CommitMeta { author: None, timestamp: 1, message: None };
    // Top & 99 = 99 → monotonicity holds
    let result = u.refine(&oo, &base_dir, vec![ca2_hash], vec![cb2], None, meta2);
    assert!(result.is_ok(), "should be exempt when architect_reg empty: {:?}", result);
}
```

### Test B：有 Architect + 有 HEAD → 無簽章 → 失敗

```rust
#[test]
fn not_exempt_when_architect_registered_and_has_head() {
    let oo = Arc::new(Ouroboros::new_in_memory());
    let base_dir = std::env::temp_dir().join("nlang-refine-epoch-b");
    let _ = std::fs::create_dir_all(&base_dir);

    // Register local key as architect
    let local_pk = hex::encode(&oo.identity.public_key);
    { oo.architect_registry.write().unwrap().insert(local_pk); }

    let mut u = Universe::new(None, oo.root_with_system());

    let val_a = Value::Top;
    let val_b = Value::Atom(nlang_parser::ast::AtomKind::Int(200.into()), EffectTag::Pure, None);
    let ca = oo.store.put_value(&val_a).unwrap();
    let cb = oo.store.put_value(&val_b).unwrap();

    // First refine: head=None → exempt (bootstrap)
    let meta1 = CommitMeta { author: None, timestamp: 0, message: None };
    u.refine(&oo, &base_dir, vec![ca], vec![cb], None, meta1).unwrap();
    assert!(u.head.is_some(), "head should be set after first refine");

    // Second refine: head set, architect registered → NOT exempt
    // Without signature → should fail
    let val_c = Value::Top;
    let val_d = Value::Atom(nlang_parser::ast::AtomKind::Int(42.into()), EffectTag::Pure, None);
    let cc = oo.store.put_value(&val_c).unwrap();
    let cd = oo.store.put_value(&val_d).unwrap();
    let meta2 = CommitMeta { author: None, timestamp: 1, message: None };
    let result = u.refine(&oo, &base_dir, vec![cc], vec![cd], None, meta2);
    assert!(result.is_err(), "refine without signature should fail when architect is registered and head is set");
}
```

### Test C：有 Architect + 有 HEAD + 有效簽章 → 成功

```rust
#[test]
fn exempt_with_valid_signature_when_architect_registered() {
    let oo = Arc::new(Ouroboros::new_in_memory());
    let base_dir = std::env::temp_dir().join("nlang-refine-epoch-c");
    let _ = std::fs::create_dir_all(&base_dir);

    // Register local key as architect
    let local_pk = hex::encode(&oo.identity.public_key);
    { oo.architect_registry.write().unwrap().insert(local_pk); }

    let mut u = Universe::new(None, oo.root_with_system());

    let val_a = Value::Top;
    let val_b = Value::Atom(nlang_parser::ast::AtomKind::Int(300.into()), EffectTag::Pure, None);
    let ca = oo.store.put_value(&val_a).unwrap();
    let cb = oo.store.put_value(&val_b).unwrap();

    // First refine: exempt (head=None)
    let meta1 = CommitMeta { author: None, timestamp: 0, message: None };
    u.refine(&oo, &base_dir, vec![ca], vec![cb], None, meta1).unwrap();

    // Second refine: with valid signature → should succeed
    let val_c = Value::Top;
    let val_d = Value::Atom(nlang_parser::ast::AtomKind::Int(999.into()), EffectTag::Pure, None);
    let cc = oo.store.put_value(&val_c).unwrap();
    let cd = oo.store.put_value(&val_d).unwrap();

    let payload = nlang_interpreter::authority::compute_refine_payload(&[cc.clone()], &[cd.clone()]);
    let auth = nlang_interpreter::authority::sign_refine(&payload, &oo.identity).unwrap();

    let meta2 = CommitMeta { author: None, timestamp: 1, message: None };
    let result = u.refine(&oo, &base_dir, vec![cc], vec![cd], Some(auth), meta2);
    assert!(result.is_ok(), "refine with valid signature should succeed: {:?}", result);
}
```

**注意：** 這三個測試需要 `hex` crate。`refine_test.rs` 的 `use` 清單加：
```rust
use hex;
```
確認 `hex` 已在 `crates/interpreter/Cargo.toml` 的 dependencies 中（Phase 8 應已加入）。

---

## 驗收條件

1. `cargo test -p nlang-interpreter 2>&1 | grep -E "FAILED|passed"` — 全部通過（含 3 個新測試）
2. `oo refine --source <caid> --target <caid>` — CLI 有效，能執行
3. `oo refine --source <caid> --target <caid> --sign` — CLI 以本機 identity 自動簽署並成功
4. `oo refine --help` — 顯示 source/target/sign/message 說明
5. 在 architect 登記且 HEAD 存在的環境下執行無簽 refine → CLI 回傳錯誤訊息
6. `universe.rs` 中 `let bootstrap_exempt = true;` 這行不再存在

---

## 不在本 Phase 的工作

- **Shadow refinement**（歷史 Commit 背景驗證）— P2，Phase 11+ 
- **`nerve_structure` 真實 MASA 交集** — P2，Phase 11+
- **architects 清單持久化**（Phase 8 TODO）— `engine.architect_registry` 目前只在記憶體，重啟後遺失；持久化至 ObjectStore 特殊 key 留待後續
- **Epoch 數字型別**（`u64`）— 目前 Epoch 判定只用 `head.is_none()`（0/1 判斷），精確的 Epoch 計數器待 Commit chain 完整後再引入
