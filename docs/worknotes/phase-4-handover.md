# Phase 4 交接文件：LADD 引力路由基礎 (APP_05 §2–4)

## 目標

實作 LADD（Lattice-Aware Distributed Discovery）協議的 L3-L5 基礎架構。使用
Phase 1a 已有的近似量（BN/ sketch、masa_ref）建立完整的協議骨架；Phase 5 再以
真實特徵值分解替換近似值。

**依賴前置**：Phase 1a ✅、Phase 1b ✅、Phase NEW ✅（H¹/H² obstruction 已實作）

---

## 規格對應

| 實作項目 | 規格章節 |
|:---------|:---------|
| GBB 結構（幾何包圍盒）  | APP_05 §2.2 |
| AdvertiseGeometry      | APP_05 §2.2 |
| DiscoverRequest        | APP_05 §2.3 |
| MASA 前置過濾           | APP_05 §4.1 |
| 引力路由權重 W          | APP_05 §4.2 |
| `/find` 態射           | APP_05 §4、SPEC_13 §6.2 |
| 視界震盪               | APP_05 §4.4 |

---

## 1. 新增模組：`crates/interpreter/src/ladd.rs`

```rust
use crate::value::{ContentHash, MasaRef};

/// 幾何包圍盒 (Geometric Bounding Box)
/// 代表一個節點所承載之子空間的近似幾何描述
#[derive(Debug, Clone)]
pub struct GBB {
    pub node_caid: ContentHash,
    pub mass: f64,             // ≈ Tr(P)；Phase 4：用 BN/ 位元組數正規化
    pub sketch_bytes: Vec<u8>, // lattice_sketch Base64 解碼後的位元組
    pub masa_ref: MasaRef,
}

/// Phase 4 近似：hamming 距離作為譜距離代理
/// Phase 5 替換為真正的 Chordal Distance（Frobenius 範數）
pub fn d_l_approx(a: &GBB, b: &GBB) -> f64 {
    if a.sketch_bytes.is_empty() || b.sketch_bytes.is_empty() {
        return 1.0;
    }
    let min_len = a.sketch_bytes.len().min(b.sketch_bytes.len());
    let xor_bits: u32 = a.sketch_bytes[..min_len]
        .iter()
        .zip(&b.sketch_bytes[..min_len])
        .map(|(x, y)| (x ^ y).count_ones())
        .sum();
    let max_bits = (min_len * 8) as f64;
    (xor_bits as f64) / max_bits
}

/// 引力路由權重：W = mass / (d_L² + ε)
pub fn gravitational_weight(query: &GBB, peer: &GBB, epsilon: f64) -> f64 {
    let d = d_l_approx(query, peer);
    peer.mass / (d * d + epsilon)
}

/// MASA 相容性前置過濾（H² obstruction）
/// 返回 false 代表 H² 不相容，跳過此節點（W = 0）
pub fn masa_compatible(query: &GBB, peer: &GBB) -> bool {
    match (&query.masa_ref, &peer.masa_ref) {
        (MasaRef::Top, _) | (_, MasaRef::Top) => true,
        (MasaRef::Digest(a), MasaRef::Digest(b)) => a == b,
    }
}
```

**說明**：
- `mass`：Phase 4 使用 BN/ 位元組數除以 256.0 作為近似值，上限 1.0
- `d_l_approx`：Phase 5 替換為 `sqrt(Tr(P_A + P_B - 2 P_A ⊓ P_B))`（真實複數 Chordal Distance）
- `masa_compatible`：重用 Phase 1b/NEW 的語義——MASA 不相容即 H² obstruction，路由跳過

---

## 2. 修改 `Ouroboros` 結構體（`src/lib.rs`）

### 2.1 新增欄位

```rust
// 加在現有欄位後面
pub gbb_registry: RwLock<HashMap<String, crate::ladd::GBB>>,
// key = node_caid.to_string()
```

### 2.2 修改 `Ouroboros::init()` 初始化

```rust
let mut oo = Self {
    store,
    unify_memo: RwLock::new(HashMap::new()),
    builtin_registry: builtins,
    peers: RwLock::new(HashMap::new()),
    identity: crate::value::Identity::new_random(),
    refine_map: RwLock::new(HashMap::new()),
    gbb_registry: RwLock::new(HashMap::new()),   // 新增
};
```

### 2.3 在 `root_with_system()` 的 `~%Discovery` Combo 新增態射

```rust
let disc_morphisms = vec![
    ("/connect",           "disc.connect"),
    ("/fetch",             "disc.fetch"),
    ("/identify",          "disc.identify"),
    ("/identify_and_store","engine.save"),
    ("/advertise",         "disc.advertise"),   // 新增
    ("/find",              "disc.find"),         // 新增
];
```

---

## 3. 修改 `builtins/disc.rs`

### 3.1 `disc.advertise`

輸入：`arg` = 任意 Value（代表本節點承載的子空間）  
行為：計算 GBB，儲存到 `oo.gbb_registry`  
輸出：`#true`（IO 效果）

```rust
m.insert("disc.advertise".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let hash = arg.content_hash();
    // Phase 4 近似質量：BN/ 位元組數 / 256.0，上限 1.0
    let bn_bytes = crate::bn_serial::serialize_bn(&arg);
    let mass = (bn_bytes.len() as f64 / 256.0).min(1.0);
    // 從 content_hash 取得 sketch 與 masa_ref
    let sketch_bytes = base64_decode_sketch(&hash.lattice_sketch);
    let masa_ref = hash.masa_ref.clone();
    let gbb = crate::ladd::GBB { node_caid: hash.clone(), mass, sketch_bytes, masa_ref };
    if let Ok(mut reg) = oo.gbb_registry.write() {
        reg.insert(hash.to_string(), gbb);
    }
    Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::IO, None)
}) as Arc<BuiltinFn>);
```

輔助函式（加在 disc.rs 頂部，非 pub）：
```rust
fn base64_decode_sketch(s: &str) -> Vec<u8> {
    use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
    STANDARD_NO_PAD.decode(s).unwrap_or_default()
}
```

### 3.2 `disc.find`

輸入：`arg` = `{ target: "<CAID 字串>" }` 或任意 Value（作為查詢目標的幾何代理）  
行為：
1. 解析查詢目標，計算查詢 GBB
2. MASA 前置過濾（APP_05 §4.1）
3. 計算各 peer 引力權重（APP_05 §4.2）
4. 視界震盪（APP_05 §4.4）——以 10% 機率隨機選擇
5. 從最高權重 peer fetch 目標
6. 若所有 peer 均不相容或找不到，返回 `Bottom(#not_found)`

```rust
m.insert("disc.find".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    // 1. 建立查詢 GBB
    let query_hash = arg.content_hash();
    let query_bn = crate::bn_serial::serialize_bn(&arg);
    let query_mass = (query_bn.len() as f64 / 256.0).min(1.0);
    let query_sketch = base64_decode_sketch(&query_hash.lattice_sketch);
    let query_gbb = crate::ladd::GBB {
        node_caid: query_hash.clone(),
        mass: query_mass,
        sketch_bytes: query_sketch,
        masa_ref: query_hash.masa_ref.clone(),
    };

    // 2. 從 gbb_registry 取得候選節點，MASA 過濾 + 計算引力權重
    const EPSILON: f64 = 1e-6;
    let mut candidates: Vec<(f64, String)> = {
        let reg = match oo.gbb_registry.read() { Ok(r) => r, Err(_) => return BottomCause::Conflict.into() };
        reg.values()
            .filter(|peer_gbb| crate::ladd::masa_compatible(&query_gbb, peer_gbb))
            .map(|peer_gbb| {
                let w = crate::ladd::gravitational_weight(&query_gbb, peer_gbb, EPSILON);
                (w, peer_gbb.node_caid.to_string())
            })
            .collect()
    };

    if candidates.is_empty() { return bottom_not_found(); }
    candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // 3. 視界震盪：10% 機率隨機跳躍
    let chosen_caid_str = if ctx.horizon_salt[0] % 10 == 0 {
        // 隨機選一個（用 horizon_salt 做簡單偽隨機）
        let idx = (ctx.horizon_salt[1] as usize) % candidates.len();
        candidates[idx].1.clone()
    } else {
        candidates[0].1.clone()
    };

    // 4. 從最佳候選節點 fetch 目標
    let target_caid_str = if let Value::Combo(ref c) = arg {
        if let Some(v) = c.get_field("target") { oo.force(v.clone(), ctx).to_string_plain() }
        else { chosen_caid_str.clone() }
    } else { chosen_caid_str.clone() };

    if let Ok(hash) = crate::value::ContentHash::parse(&target_caid_str) {
        // 先查本地
        if let Ok(val) = oo.store.get_value(&hash) { return val; }
        // 再查 peers
        let peers_copy: Vec<_> = oo.peers.read().map(|p| p.values().cloned().collect()).unwrap_or_default();
        for peer in peers_copy {
            match peer {
                crate::Peer::Local(store) => { if let Ok(val) = store.get_value(&hash) { return val; } }
                crate::Peer::Remote(addr) => { if let Ok(val) = oo.remote_fetch(&addr, &hash) { return val; } }
            }
        }
    }
    bottom_not_found()
}) as Arc<BuiltinFn>);
```

輔助函式：
```rust
fn bottom_not_found() -> Value {
    use crate::value::{BottomDetail, BottomCause};
    Value::Bottom(Box::new(BottomDetail {
        cause: BottomCause::Conflict,
        message: Some("#not_found: no compatible peer".to_string()),
        ..Default::default()
    }))
}
```

---

## 4. `src/ladd.rs` 需要加入 `mod` 宣告

在 `src/lib.rs` 頂部加入：
```rust
pub mod ladd;
```

---

## 5. 測試：`crates/interpreter/tests/ladd_test.rs`

請新增最少 8 個測試，涵蓋：

| # | 測試名稱 | 驗證內容 |
|---|----------|----------|
| 1 | `test_d_l_approx_identical` | 相同 sketch → d_L = 0 |
| 2 | `test_d_l_approx_empty` | 空 sketch → d_L = 1.0 |
| 3 | `test_d_l_approx_range` | 結果在 [0.0, 1.0] 之間 |
| 4 | `test_masa_compatible_top` | MasaRef::Top × anything → true |
| 5 | `test_masa_compatible_same_digest` | 相同 Digest → true |
| 6 | `test_masa_incompatible` | 不同 Digest → false |
| 7 | `test_gravitational_weight_positive` | W > 0 當 d_L < 1 |
| 8 | `test_disc_advertise_and_find` | `disc.advertise` 後 `gbb_registry` 有條目 |

---

## 6. 設計決策與 Phase 5 橋接

### 近似值對應

| Phase 4 近似 | Phase 5 真實實作 |
|:-------------|:----------------|
| `mass ≈ BN/ 位元組數 / 256.0` | `mass = Tr(P)` 特徵值之和 |
| `d_L ≈ hamming(sketch_A XOR sketch_B)` | `d_L = sqrt(Tr(P_A + P_B - 2 P_A ⊓ P_B))` |
| lattice_sketch = SHA256 前綴（Phase 1a） | lattice_sketch = 真實複數譜（Phase 5） |
| 視界震盪：`horizon_salt[0] % 10 == 0` | 可改為可設定 `ε_horizon` 參數 |

### 未實作（Phase 5 待辦）

- `nerve_structure`：GBB 的 Čech 神經位置（路由複雜度從 O(n²) 降至 O(n·k̄)）
- 真實複數譜廣播（AdvertiseGeometry TCP 封包）
- `d_L^ℂ`：複數 Chordal Distance（含 MASA 相位校正）
- `lattice_sketch_test_suite v2` 跨架構穩定性驗證

### MASA 前置過濾的重用

`masa_compatible()` 的語義與 `unify.rs:phase_merge_decision()` 的 H² 判斷完全一致：
```
MasaRef::Digest(da) != MasaRef::Digest(db) → H² obstruction → W = 0（跳過）
```
Phase 4 不重複造輪子——直接呼叫 `ladd::masa_compatible()` 即可，其輸出等同於
`MergeDecision::H2Split` 的前置條件。

---

## 7. 依賴事項

- `base64` crate：已在 `lattice_sketch.rs` 中使用，`STANDARD_NO_PAD` 即可
- `crate::bn_serial::serialize_bn`：Phase 1a 已實作
- `crate::value::{MasaRef, ContentHash, BottomCause, BottomDetail, EffectTag}`：均已存在
- `ctx.horizon_salt`：`EvalContext` 現有欄位，`[u8; 32]`

---

## 8. 完成標準

- [ ] `crates/interpreter/src/ladd.rs` 新增：`GBB`、`d_l_approx()`、`gravitational_weight()`、`masa_compatible()`
- [ ] `Ouroboros::gbb_registry` 欄位及初始化
- [ ] `disc.advertise` builtin 實作並註冊
- [ ] `disc.find` builtin 實作並註冊（含 MASA 過濾 + 視界震盪）
- [ ] `~%Discovery` Combo 新增 `/advertise` 和 `/find` 態射
- [ ] `tests/ladd_test.rs` 最少 8 個測試，全數通過
- [ ] `cargo test` 全數通過（預期 88+ 個測試）
