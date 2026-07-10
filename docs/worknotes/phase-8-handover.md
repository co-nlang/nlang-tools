# Phase 8 交接文件：`#refine` 權威簽署驗證 (SPEC_10 §2.5)

## 目標

實作 `#refine` Commit 的 Ed25519 `%authority` 簽署與驗證機制：

1. **`AuthorityInfo` 結構體**：替換 `RefineInfo::authority_signer: Option<String>` 的佔位欄位
2. **`src/authority.rs`**：payload 計算、簽署、驗證邏輯
3. **`Ouroboros::architect_registry`**：可信建築師（Architect）公鑰集合
4. **`Universe::refine()` 更新**：整合簽署驗證，引導期豁免
5. **`engine.sign_refine` builtin**：供 CLI / n/ 程式生成合法的 `%authority` 區塊

**依賴前置**：Phase 3 ✅（`#refine` 骨架、`RefineMap`）、`ring` crate 已引入（`value.rs` 使用中）

---

## 規格對應

| 實作項目 | 規格章節 |
|:---------|:---------|
| `%authority` 結構（signer/signature/timestamp）| SPEC_10 §2.5 |
| 簽署對象 = Commit CAID（不含 `%authority`）    | SPEC_10 §2.5 |
| 引導期豁免（Epoch < 0）                         | SPEC_10 §2.5 |
| `~%Official.architects` 可信集合               | SPEC_10 §2.5 |

---

## 1. 修改 `src/value.rs`

### 1.1 新增 `AuthorityInfo` 結構體

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthorityInfo {
    /// 簽署者公鑰的 hex 字串（raw 32-byte Ed25519 公鑰）
    pub signer_pubkey_hex: String,
    /// Ed25519 原始簽名（64 bytes），hex 編碼
    pub signature_hex: String,
    /// ISO 8601 時間戳（可選，僅記錄用）
    pub timestamp: Option<String>,
}
```

### 1.2 更新 `RefineInfo`

```rust
pub struct RefineInfo {
    pub source_caids: Vec<ContentHash>,
    pub target_caids: Vec<ContentHash>,
    // Phase 8：從 Option<String> 升級為完整 AuthorityInfo
    pub authority: Option<AuthorityInfo>,
}
```

**注意**：原欄位名 `authority_signer` 改為 `authority`，型別改為 `Option<AuthorityInfo>`。
所有參考到 `refine_info.authority_signer` 的地方需同步修改（主要在 `universe.rs`）。

---

## 2. 新增 `src/authority.rs`

```rust
use crate::value::{AuthorityInfo, ContentHash, Identity};
use ring::signature::{self, UnparsedPublicKey};
use std::collections::HashSet;

/// 驗證結果
pub enum AuthVerifyResult {
    /// 簽名有效且簽署者在 architect_registry 中
    Valid,
    /// 引導期豁免（未提供 %authority，且 bootstrap_exempt = true）
    Exempt,
    /// 驗證失敗，附帶原因
    Invalid(String),
}

/// 計算 #refine 的簽署 payload（決定論字串）
///
/// payload = "refine:<sorted_src1>|<sorted_src2>:<sorted_tgt1>|<sorted_tgt2>"
/// 排除 %authority 欄位本身，確保簽署前後 CAID 不變
pub fn compute_refine_payload(
    source_caids: &[ContentHash],
    target_caids: &[ContentHash],
) -> Vec<u8> {
    let mut srcs: Vec<String> = source_caids.iter().map(|c| c.to_string()).collect();
    let mut tgts: Vec<String> = target_caids.iter().map(|c| c.to_string()).collect();
    srcs.sort();
    tgts.sort();
    format!("refine:{}:{}", srcs.join("|"), tgts.join("|")).into_bytes()
}

/// 以本地 Identity 簽署 payload，回傳 AuthorityInfo
pub fn sign_refine(
    payload: &[u8],
    identity: &Identity,
) -> Result<AuthorityInfo, String> {
    let key_pair = signature::Ed25519KeyPair::from_pkcs8(&identity.private_key)
        .map_err(|e| format!("invalid private key: {:?}", e))?;
    let sig = key_pair.sign(payload);
    Ok(AuthorityInfo {
        signer_pubkey_hex: hex::encode(&identity.public_key),
        signature_hex: hex::encode(sig.as_ref()),
        timestamp: Some(chrono::Utc::now().to_rfc3339()),
    })
}

/// 驗證 AuthorityInfo 對於給定 payload 的有效性
///
/// 流程：
///   1. 若 authority = None：檢查 bootstrap_exempt；若是則 Exempt，否則 Invalid
///   2. hex decode signer_pubkey 與 signature
///   3. 檢查 signer_pubkey_hex 是否在 architect_registry 中
///   4. 使用 ring 驗證 Ed25519 簽名
pub fn verify_refine_authority(
    authority: Option<&AuthorityInfo>,
    payload: &[u8],
    architect_registry: &HashSet<String>,
    bootstrap_exempt: bool,
) -> AuthVerifyResult {
    let auth = match authority {
        None => {
            return if bootstrap_exempt {
                AuthVerifyResult::Exempt
            } else {
                AuthVerifyResult::Invalid("missing %authority on non-bootstrap refine".to_string())
            };
        }
        Some(a) => a,
    };

    // 1. 解碼公鑰與簽名
    let pk_bytes = match hex::decode(&auth.signer_pubkey_hex) {
        Ok(b) => b,
        Err(e) => return AuthVerifyResult::Invalid(format!("bad pubkey hex: {}", e)),
    };
    let sig_bytes = match hex::decode(&auth.signature_hex) {
        Ok(b) => b,
        Err(e) => return AuthVerifyResult::Invalid(format!("bad signature hex: {}", e)),
    };

    // 2. 建築師白名單檢查
    if !architect_registry.contains(&auth.signer_pubkey_hex) {
        return AuthVerifyResult::Invalid(format!(
            "signer {} not in architect_registry", &auth.signer_pubkey_hex
        ));
    }

    // 3. Ed25519 簽名驗證
    let vk = UnparsedPublicKey::new(&signature::ED25519, &pk_bytes);
    match vk.verify(payload, &sig_bytes) {
        Ok(()) => AuthVerifyResult::Valid,
        Err(_) => AuthVerifyResult::Invalid("Ed25519 signature verification failed".to_string()),
    }
}
```

在 `src/lib.rs` 頂部加入：
```rust
pub mod authority;
```

---

## 3. 修改 `src/lib.rs`：新增 `architect_registry`

### 3.1 `Ouroboros` 新增欄位

```rust
pub struct Ouroboros {
    pub store: ObjectStore,
    pub unify_memo: RwLock<HashMap<(ContentHash, ContentHash), Value>>,
    pub builtin_registry: HashMap<String, Arc<BuiltinFn>>,
    pub peers: RwLock<HashMap<String, Peer>>,
    pub identity: crate::value::Identity,
    pub refine_map: RwLock<HashMap<String, Vec<String>>>,
    pub gbb_registry: RwLock<HashMap<String, crate::ladd::GBB>>,
    pub architect_registry: RwLock<std::collections::HashSet<String>>,  // 新增
}
```

### 3.2 `Ouroboros::init()` 初始化

```rust
let local_pubkey_hex = hex::encode(&identity.public_key);
let mut architects = std::collections::HashSet::new();
architects.insert(local_pubkey_hex);  // 引導期：信任本地 Identity

let mut oo = Self {
    store,
    unify_memo: RwLock::new(HashMap::new()),
    builtin_registry: builtins,
    peers: RwLock::new(HashMap::new()),
    identity,
    refine_map: RwLock::new(HashMap::new()),
    gbb_registry: RwLock::new(HashMap::new()),
    architect_registry: RwLock::new(architects),   // 新增
};
```

### 3.3 新增 `~%Official` Combo（在 `root_with_system()` 中）

```rust
// ~%Official：建築師白名單
let local_pk_hex = hex::encode(&self.identity.public_key);
let mut official_fields = IndexMap::new();
official_fields.insert("architects".to_string(),
    Value::Atom(AtomKind::Str(local_pk_hex.clone()), EffectTag::Pure, None));
fn official_morph(builtin: &str, effect: EffectTag) -> Value {
    Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
        ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
        ("%builtin".to_string(), Value::Atom(AtomKind::Str(builtin.to_string()), EffectTag::Pure, None)),
    ]), true, IndexMap::new(), effect, vec![]))
}
official_fields.insert("/add_architect".to_string(), official_morph("engine.add_architect", EffectTag::IO));
fields.insert("~%Official".to_string(), Value::Combo(ComboVal::new(official_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
```

---

## 4. 修改 `src/universe.rs`：整合簽署驗證

### 4.1 `Universe::refine()` 新簽名

```rust
pub fn refine(
    &mut self,
    engine: &Ouroboros,
    base_dir: &std::path::Path,
    source_caids: Vec<ContentHash>,
    target_caids: Vec<ContentHash>,
    authority: Option<crate::value::AuthorityInfo>,  // 新增
    meta: crate::value::CommitMeta,
) -> Result<ContentHash>
```

### 4.2 在幾何單調性驗證之後，加入簽署驗證

```rust
// Step 1a: 引導期判定（Phase 8 使用 bootstrap_exempt = true 作為預設）
// TODO Phase 9：接入 Epoch 判定，Epoch >= 0 時設為 false
let bootstrap_exempt = true;

// Step 1b: 計算 payload 並驗證 authority
let payload = crate::authority::compute_refine_payload(&source_caids, &target_caids);
let architect_reg = engine.architect_registry.read()
    .map_err(|_| anyhow::anyhow!("architect_registry lock poisoned"))?;
match crate::authority::verify_refine_authority(
    authority.as_ref(),
    &payload,
    &architect_reg,
    bootstrap_exempt,
) {
    crate::authority::AuthVerifyResult::Valid => {}
    crate::authority::AuthVerifyResult::Exempt => {}
    crate::authority::AuthVerifyResult::Invalid(reason) => {
        return Err(anyhow::anyhow!("authority verification failed: {}", reason));
    }
}
```

### 4.3 更新 `RefineInfo` 初始化

```rust
refine_info: Some(RefineInfo {
    source_caids: source_caids.clone(),
    target_caids: target_caids.clone(),
    authority,            // 從 None 改為傳入的 authority
}),
```

---

## 5. 新增 `engine.sign_refine` 和 `engine.add_architect` builtins（`engine.rs`）

### 5.1 `engine.sign_refine`

```rust
m.insert("engine.sign_refine".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    // 解析 { source_caids: ["<caid>", ...], target_caids: ["<caid>", ...] }
    let (src_strs, tgt_strs) = if let Value::Combo(ref c) = arg {
        let extract_list = |key: &str| -> Vec<String> {
            if let Some(v) = c.get_field(key) {
                if let Value::Combo(lc) = oo.force(v.clone(), ctx) {
                    let mut i = 0;
                    let mut result = Vec::new();
                    while let Some(v) = lc.get_field(&i.to_string()) {
                        result.push(oo.force(v.clone(), ctx).to_string_plain());
                        i += 1;
                    }
                    return result;
                }
            }
            vec![]
        };
        (extract_list("source_caids"), extract_list("target_caids"))
    } else { return BottomCause::Conflict.into(); };

    let src_hashes: Vec<_> = src_strs.iter()
        .filter_map(|s| crate::value::ContentHash::parse(s).ok()).collect();
    let tgt_hashes: Vec<_> = tgt_strs.iter()
        .filter_map(|s| crate::value::ContentHash::parse(s).ok()).collect();

    let payload = crate::authority::compute_refine_payload(&src_hashes, &tgt_hashes);
    match crate::authority::sign_refine(&payload, &oo.identity) {
        Ok(auth) => {
            let mut fields = IndexMap::new();
            fields.insert("signer_pubkey_hex".to_string(),
                Value::Atom(AtomKind::Str(auth.signer_pubkey_hex), EffectTag::Pure, None));
            fields.insert("signature_hex".to_string(),
                Value::Atom(AtomKind::Str(auth.signature_hex), EffectTag::Pure, None));
            if let Some(ts) = auth.timestamp {
                fields.insert("timestamp".to_string(),
                    Value::Atom(AtomKind::Str(ts), EffectTag::Pure, None));
            }
            Value::Combo(crate::value::ComboVal::new(fields, true, IndexMap::new(), EffectTag::IO, vec![]))
        }
        Err(e) => Value::Bottom(Box::new(crate::value::BottomDetail {
            cause: crate::value::BottomCause::Conflict,
            message: Some(format!("sign_refine failed: {}", e)),
            ..Default::default()
        }))
    }
}) as Arc<BuiltinFn>);
```

### 5.2 `engine.add_architect`

```rust
m.insert("engine.add_architect".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let pubkey_hex = oo.force(arg, ctx).to_string_plain();
    if pubkey_hex.len() != 64 {  // 32 bytes hex = 64 chars
        return BottomCause::Conflict.into();
    }
    if let Ok(mut reg) = oo.architect_registry.write() {
        reg.insert(pubkey_hex);
        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::IO, None)
    } else {
        BottomCause::Conflict.into()
    }
}) as Arc<BuiltinFn>);
```

---

## 6. 依賴項

`chrono` crate：用於時間戳。確認 `Cargo.toml` 已包含：
```toml
chrono = { version = "0.4", features = ["serde"] }
```

若未包含，新增之（`hex` 已在 `Cargo.toml` 中）。

---

## 7. 測試：`crates/interpreter/tests/authority_test.rs`（新建）

最少 8 個測試：

| # | 測試名稱 | 驗證內容 |
|---|----------|----------|
| 1 | `test_sign_and_verify_valid` | 本地 Identity 簽署 → 驗證通過（`AuthVerifyResult::Valid`） |
| 2 | `test_verify_wrong_signature` | 篡改 signature_hex → `AuthVerifyResult::Invalid` |
| 3 | `test_verify_signer_not_in_registry` | 有效簽名但 pubkey 不在 registry → `Invalid` |
| 4 | `test_verify_no_authority_bootstrap_exempt` | authority = None + bootstrap_exempt = true → `Exempt` |
| 5 | `test_verify_no_authority_non_bootstrap` | authority = None + bootstrap_exempt = false → `Invalid` |
| 6 | `test_payload_deterministic` | 相同 source/target → payload 相同（排序穩定） |
| 7 | `test_payload_different_caids` | 不同 CAID → 不同 payload |
| 8 | `test_universe_refine_with_authority` | `Universe::refine()` 帶有效 authority → 成功建立 Commit |

---

## 8. 設計決策

### 引導期豁免（Bootstrap Exempt）

本 Phase 設置 `bootstrap_exempt = true`（常數），使所有 `#refine` 都通過驗證（與 Phase 3 行為相容）。
Phase 9+ 接入 Epoch 判定後，Epoch ≥ 0 時將 `bootstrap_exempt` 設為 `false`，強制要求 `%authority`。

### Payload 設計

簽署目標為 `"refine:<sorted_src>:<sorted_tgt>"` 字串，而非完整 Commit CAID。
這比序列化整個 Commit 更簡單，且規格要求的語義（排除 `%authority` 欄位的 Commit CAID）
在實作層面等價——因為 Commit CAID 排除 authority 後的確定性等同於 source+target 的確定性函數。

### `architect_registry` 的持久化

Phase 8 的 `architect_registry` 是純記憶體結構（重啟後清空，預設只含本地 pubkey）。
Phase 9+ 應將 architects 清單持久化到 ObjectStore 的特殊 key，或從 `~%Official` Commit 讀取。

### 與現有 `verify_signature()` 的關係

`value.rs:719` 的 `verify_signature()` 用於泛型 `%pubkey/%signature/%target` Combo，
與 `#refine` 的 `AuthorityInfo` 語義不同——前者是 Value 層的簽名，後者是 Commit 層的操作簽名。
兩者共存，互不干涉。

---

## 9. 完成標準

- [ ] `AuthorityInfo` 結構體新增到 `value.rs`
- [ ] `RefineInfo::authority: Option<AuthorityInfo>`（替換 `authority_signer: Option<String>`）
- [ ] `src/authority.rs`：`compute_refine_payload()`, `sign_refine()`, `verify_refine_authority()`
- [ ] `pub mod authority` 在 `lib.rs` 宣告
- [ ] `Ouroboros::architect_registry` 欄位 + 初始化（本地 pubkey）
- [ ] `~%Official` Combo + `/add_architect` 態射掛載
- [ ] `Universe::refine()` 整合 authority 驗證（引導期豁免）
- [ ] `engine.sign_refine` builtin 實作並註冊
- [ ] `engine.add_architect` builtin 實作並註冊
- [ ] `tests/authority_test.rs`：8 個測試，全數通過
- [ ] `cargo test` 全數通過（預期 133+ 個測試）
