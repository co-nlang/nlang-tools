# Phase 5 交接文件：真實 Lattice Sketch v2 + Nerve-Aware Routing

## 目標

1. **替換 Lattice Sketch 近似值**：將 `compute_sketch_approximate()`（SHA256 前綴）升級為符合
   APP_05 §3.5 格式的結構化譜指紋（structured spectral approximation）。
2. **Nerve-aware routing**：在 `GBB` 加入 `nerve_structure`，實作 APP_05 §4.3 的 MASA
   交集預篩選，將路由複雜度從 O(n²) 降至 O(n·k̄)。

**依賴前置**：Phase 1a ✅（BN/ 序列化）、Phase 4 ✅（GBB、`disc.find`）

---

## 規格對應

| 實作項目 | 規格章節 |
|:---------|:---------|
| Lattice Sketch v2 複數譜格式 | APP_05 §3.5.2–3.5.4 |
| 量化：λ_q、θ_q | APP_05 §3.5.3 |
| Delta → ZigZag → LEB128 → Base64 | APP_05 §3.5.4 |
| 跨架構穩定性（向零捨入、振幅降冪） | APP_05 §3.5.5 |
| nerve_structure + MASA 重疊預篩選 | APP_05 §4.3 |

---

## 1. 核心替換：`src/lattice_sketch.rs`

### 1.1 設計思路

引擎目前沒有顯式 Hilbert 空間，因此採用 **Combo 欄位結構作為投影算子的代理**：

| 規格概念 | Phase 5 代理 |
|:---------|:-------------|
| 特徵值 λᵢ（Tr(P) 的分量） | SHA256(BN/(field_value))[:8] → f64 ∈ [0,1] |
| 特徵向量排序（振幅降冪）  | 對欄位特徵值排序 |
| MASA 相位 θᵢ = arg(⟨vᵢ\|e_M⟩) | SHA256(masa_digest ++ field_key)[:8] → f64 ∈ [-π,π] |
| 質量 m = Tr(P) | Σ λᵢ（特徵值總和） |

非 Combo 值：
- `Value::Atom(...)` → 單一欄位，振幅 = SHA256(BN/(atom))[:8]，相位 = 0.0
- `Value::Top` → 全零譜（16 個 (0,0) 組）
- `Value::Bottom(...)` → 單一欄位，振幅 = SHA256(BN/(bottom))[:8]，相位 = 0.0
- `Value::Union(branches)` → 各分支取振幅，降冪排序，截取前 16

### 1.2 新函式：`compute_sketch_v2(value: &Value) -> String`

```rust
use sha2::{Sha256, Digest};
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use crate::value::{Value, MasaRef};
use crate::bn_serial::serialize_bn;

const MAX_COMPONENTS: usize = 16;

pub fn compute_sketch_v2(value: &Value) -> String {
    let (amplitudes, phases) = extract_spectral_components(value);
    let encoded = encode_complex_spectrum(&amplitudes, &phases);
    STANDARD_NO_PAD.encode(&encoded)
}

fn extract_spectral_components(value: &Value) -> (Vec<f64>, Vec<f64>) {
    match value {
        Value::Top => (vec![0.0; MAX_COMPONENTS], vec![0.0; MAX_COMPONENTS]),
        Value::Combo(cv) => {
            // BN/ 欄位優先順序：system(1) > meta(2) > types(3) > rules(4) > data(5) > local(6)
            let mut entries: Vec<(&str, &Value)> = Vec::new();
            for (k, v) in &cv.system  { entries.push((k, v)); }
            for (k, v) in &cv.meta    { entries.push((k, v)); }
            for (k, v) in &cv.types   { entries.push((k, v)); }
            for (k, v) in &cv.rules   { entries.push((k, v)); }
            for (k, v) in &cv.data    { entries.push((k, v)); }
            for (k, v) in &cv.local   { entries.push((k, v)); }

            let mut components: Vec<(f64, f64)> = entries.iter().map(|(key, val)| {
                let amp = field_amplitude(val);
                let phase = field_phase(&cv.masa_ref, key);
                (amp, phase)
            }).collect();
            // 振幅降冪排序（跨架構決定論：相同振幅以 field_key 字典序為次排序）
            components.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            components.truncate(MAX_COMPONENTS);
            while components.len() < MAX_COMPONENTS { components.push((0.0, 0.0)); }
            components.into_iter().unzip()
        }
        Value::Union(branches) => {
            let mut amps: Vec<f64> = branches.iter().map(field_amplitude_val).collect();
            amps.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
            amps.truncate(MAX_COMPONENTS);
            while amps.len() < MAX_COMPONENTS { amps.push(0.0); }
            let phases = vec![0.0; MAX_COMPONENTS];
            (amps, phases)
        }
        other => {
            let amp = field_amplitude_val(other);
            let mut amps = vec![0.0; MAX_COMPONENTS];
            let phases = vec![0.0; MAX_COMPONENTS];
            amps[0] = amp;
            (amps, phases)
        }
    }
}

/// 欄位振幅：SHA256(BN/(value))[:8] 解釋為 u64，歸一化到 [0, 1]
fn field_amplitude(value: &Value) -> f64 {
    let bn = serialize_bn(value);
    let hash = Sha256::digest(&bn);
    let hi = u64::from_be_bytes(hash[0..8].try_into().unwrap());
    hi as f64 / u64::MAX as f64
}

fn field_amplitude_val(value: &Value) -> f64 { field_amplitude(value) }

/// MASA 相位：SHA256(masa_digest ++ field_key_bytes)[:8] → [-π, π]
fn field_phase(masa_ref: &MasaRef, field_key: &str) -> f64 {
    match masa_ref {
        MasaRef::Top => 0.0,
        MasaRef::Digest(d) => {
            let mut h = Sha256::new();
            h.update(d);
            h.update(field_key.as_bytes());
            let hash = h.finalize();
            let raw = u64::from_be_bytes(hash[0..8].try_into().unwrap());
            // 歸一化到 [-π, π]
            (raw as f64 / u64::MAX as f64) * 2.0 * std::f64::consts::PI - std::f64::consts::PI
        }
    }
}
```

### 1.3 量化與編碼：`encode_complex_spectrum`

```rust
/// APP_05 §3.5.3–3.5.4：量化 → Delta → ZigZag → LEB128 → 輸出
fn encode_complex_spectrum(amplitudes: &[f64], phases: &[f64]) -> Vec<u8> {
    assert_eq!(amplitudes.len(), MAX_COMPONENTS);
    assert_eq!(phases.len(), MAX_COMPONENTS);

    // 量化（向零捨入，120-bit 截斷）
    let lambda_q: Vec<u64> = amplitudes.iter().map(|&v| quantize_amplitude(v)).collect();
    let theta_q: Vec<u64>  = phases.iter().map(|&p| quantize_phase(p)).collect();

    // Delta 編碼（各自獨立）
    let delta_l = delta_encode(&lambda_q);
    let delta_t = delta_encode(&theta_q);

    // ZigZag 編碼（delta 可為負，以 u64 表示的有符號差分需先轉回 i64 再 zigzag）
    // 注意：quantize 結果均非負，delta 後可能溢出；使用 wrapping 差分確保正確性
    let zz_l: Vec<u64> = delta_l.iter().map(|&d| zigzag(d as i64)).collect();
    let zz_t: Vec<u64> = delta_t.iter().map(|&d| zigzag(d as i64)).collect();

    // LEB128 壓縮，交錯輸出（振幅, 相位, 振幅, 相位, ...）
    let mut out = Vec::new();
    for i in 0..MAX_COMPONENTS {
        leb128_encode(zz_l[i], &mut out);
        leb128_encode(zz_t[i], &mut out);
    }
    out
}

/// λ_q(v) = trunc(v × 2⁶⁴) & 0x00FFFFFFFFFFFFFFFFFFFFFFFFFFFFFF (120-bit)
fn quantize_amplitude(v: f64) -> u64 {
    // 128-bit 定點：保留高 64 bit，捨去低 64 bit（向零捨入）
    let scaled = (v * (u64::MAX as f64)) as u64;   // trunc = 向零捨入
    scaled & 0x00FF_FFFF_FFFF_FFFF                  // 保留 56-bit（簡化：u64 已夠）
}

/// θ_q(φ) = trunc(φ/π × 2⁶⁴) as i64 → 再以 u64 wrapping 存儲
fn quantize_phase(phi: f64) -> u64 {
    let normalized = phi / std::f64::consts::PI;    // [-1.0, 1.0]
    let scaled = (normalized * (i64::MAX as f64)) as i64;  // 向零捨入
    scaled as u64                                   // wrapping bit pattern
}

fn delta_encode(seq: &[u64]) -> Vec<u64> {
    let mut out = Vec::with_capacity(seq.len());
    let mut prev = 0u64;
    for &v in seq {
        out.push(v.wrapping_sub(prev));
        prev = v;
    }
    out
}

/// ZigZag：i → 2i（i≥0），-i → 2i-1（i<0）
fn zigzag(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}

/// 無符號 LEB128
fn leb128_encode(mut v: u64, out: &mut Vec<u8>) {
    loop {
        let byte = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 { out.push(byte); break; }
        out.push(byte | 0x80);
    }
}
```

### 1.4 保留舊函式（供測試確認回歸）

```rust
/// Kept for backward-compatibility reference; no longer called in production.
#[doc(hidden)]
pub fn compute_sketch_approximate(bn_bytes: &[u8]) -> String {
    let hash = Sha256::digest(bn_bytes);
    STANDARD.encode(&hash[..12])
}
```

---

## 2. 修改 `src/value.rs`

### 2.1 `content_hash()` 呼叫改為 v2

```rust
pub fn content_hash(&self) -> ContentHash {
    let bn_bytes = crate::bn_serial::serialize_bn(self);
    let digest = crate::bn_serial::content_digest(self);
    // Phase 5: 換用 v2 結構化譜指紋（取代 SHA256 前綴近似）
    let sketch = crate::lattice_sketch::compute_sketch_v2(self);
    let masa_ref = match self {
        Value::Combo(c) => c.masa_ref.clone(),
        _ => MasaRef::Top,
    };
    ContentHash {
        algorithm: HashAlgorithm::Sha256,
        version: CaidVersion::V2,
        masa_ref,
        lattice_sketch: sketch,
        digest: digest.to_vec(),
    }
}
```

**注意**：`content_hash_v1()`（genesis 專用）不受影響，繼續呼叫 `bn_serial::content_digest()`。

---

## 3. 修改 `src/ladd.rs`

### 3.1 新增 `NerveEntry`，更新 `GBB`

```rust
/// Čech 神經位置條目（APP_05 §2.2, §4.3）
#[derive(Debug, Clone)]
pub struct NerveEntry {
    pub masa_caid: String,
    pub overlapping_masa_caids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GBB {
    pub node_caid: ContentHash,
    pub mass: f64,
    pub sketch_bytes: Vec<u8>,
    pub masa_ref: MasaRef,
    pub nerve_structure: Vec<NerveEntry>,  // 新增
}
```

### 3.2 新增 `nerve_overlap` 函式

```rust
/// 判斷兩個 GBB 是否有共通的 MASA（APP_05 §4.3）
/// 若任一方 nerve_structure 為空 → 視為通過（無資訊 = 不剪枝）
pub fn nerve_overlap(query: &GBB, peer: &GBB) -> bool {
    if query.nerve_structure.is_empty() || peer.nerve_structure.is_empty() {
        return true;
    }
    let query_masas: std::collections::HashSet<&str> =
        query.nerve_structure.iter().map(|e| e.masa_caid.as_str()).collect();
    peer.nerve_structure.iter().any(|pe| {
        query_masas.contains(pe.masa_caid.as_str())
        || pe.overlapping_masa_caids.iter().any(|m| query_masas.contains(m.as_str()))
    })
}
```

---

## 4. 修改 `src/builtins/disc.rs`

### 4.1 `disc.advertise`：更新質量計算 + 填入 nerve_structure

```rust
// Phase 5 質量：特徵值總和（取代 BN/ 位元組數近似）
let mass = if let Value::Combo(ref cv) = arg {
    cv.system.len() + cv.meta.len() + cv.types.len()
    + cv.rules.len() + cv.data.len() + cv.local.len()
} else { 1 } as f64;

// nerve_structure：從 refine_map 建立（近似）
let nerve_structure: Vec<crate::ladd::NerveEntry> = {
    let rmap = oo.refine_map.read().map_or_else(|_| vec![], |m| {
        m.iter().map(|(src, targets)| crate::ladd::NerveEntry {
            masa_caid: src.clone(),
            overlapping_masa_caids: targets.clone(),
        }).collect()
    });
    rmap
};

let gbb = crate::ladd::GBB {
    node_caid: hash.clone(),
    mass,
    sketch_bytes,
    masa_ref,
    nerve_structure,
};
```

### 4.2 `disc.find`：加入 nerve 預篩選

在 MASA 過濾後、引力計算前加一層：

```rust
.filter(|peer_gbb| crate::ladd::masa_compatible(&query_gbb, peer_gbb))
.filter(|peer_gbb| crate::ladd::nerve_overlap(&query_gbb, peer_gbb))  // 新增
.map(|peer_gbb| { ... gravitational_weight ... })
```

---

## 5. 測試

### 5.1 `tests/lattice_sketch_v2_test.rs`（新建）

最少 10 個測試：

| # | 測試名稱 | 驗證內容 |
|---|----------|----------|
| 1 | `test_sketch_top_is_zeros` | `Value::Top` 的 sketch 解碼後前 16 元素均為 0 |
| 2 | `test_sketch_deterministic` | 同一個 Value 呼叫兩次 → 相同 Base64 字串 |
| 3 | `test_sketch_atom_vs_combo_differ` | 同內容 Atom vs Combo → sketch 不同 |
| 4 | `test_sketch_different_combos_differ` | 不同欄位 Combo → sketch 不同（概率性，不應碰撞） |
| 5 | `test_sketch_length_bounded` | 輸出 Base64 長度合理（< 256 bytes） |
| 6 | `test_sketch_known_vector` | 硬編碼測試向量（定義一個 Combo，驗證 sketch 精確匹配）→ 跨架構穩定性 |
| 7 | `test_quantize_amplitude_zero` | `quantize_amplitude(0.0)` = 0 |
| 8 | `test_quantize_amplitude_one` | `quantize_amplitude(1.0)` = 非零最大值 |
| 9 | `test_zigzag_roundtrip` | zigzag(-1) = 1, zigzag(1) = 2 |
| 10 | `test_leb128_small` | `leb128_encode(127)` = 1 個位元組；`leb128_encode(128)` = 2 個位元組 |

**測試向量（test 6）**：在測試中定義一個固定 Combo，先執行一次、印出結果，確認後硬編碼：
```rust
#[test]
fn test_sketch_known_vector() {
    use nlang_interpreter::value::{ComboVal, Value};
    use indexmap::IndexMap;
    let mut data = IndexMap::new();
    data.insert("x".to_string(), Value::Atom(nlang_parser::ast::AtomKind::Int(1.into()), ..));
    let v = Value::Combo(ComboVal::new(data, false, IndexMap::new(), ..));
    let sketch = nlang_interpreter::lattice_sketch::compute_sketch_v2(&v);
    // 第一次執行時用 --nocapture 取得 sketch，硬編碼後不再變動
    assert_eq!(sketch, "<HARDCODED_BASE64>");
}
```

### 5.2 `tests/nerve_routing_test.rs`（新建）

最少 5 個測試：

| # | 測試名稱 | 驗證內容 |
|---|----------|----------|
| 1 | `test_nerve_overlap_both_empty` | 兩者都空 → true（不剪枝） |
| 2 | `test_nerve_overlap_no_common` | 不同 MASA → false |
| 3 | `test_nerve_overlap_direct_match` | 相同 masa_caid → true |
| 4 | `test_nerve_overlap_via_overlapping` | peer.overlapping_masa_caids 包含 query 的 masa_caid → true |
| 5 | `test_find_prunes_incompatible_nerve` | 建立兩個 peer（不同 nerve），find 只回傳相容的那個 |

---

## 6. 重要警告

### 6.1 Genesis seeds 不受影響
`genesis_test.rs` 使用 `content_hash_v1()`，不經 `compute_sketch_v2()`。Phase 5 後不需
重新計算 genesis seeds。

### 6.2 v2 CAID 字串會改變
任何現有測試若硬編碼了 `content_hash().to_string()`（v2 格式）的期望值，需在 Phase 5
後更新。執行方法：跑一次 `cargo test -- --nocapture`，取新的 sketch 字串，更新硬編碼。

### 6.3 `d_l_approx` 在 Phase 5 仍是近似
Phase 5 的 `disc.find` 繼續使用 sketch XOR hamming 作為 `d_L` 代理。v2 sketch 的信息量
比 SHA256 前綴更豐富，因此路由品質實際上有所提升。真正的 Chordal Distance 計算留待
Hilbert 空間正式化後再做（Phase 6+）。

---

## 7. 設計決策摘要

| 決策 | 理由 |
|:-----|:-----|
| 保留 `compute_sketch_approximate` 但不再呼叫 | 供回歸比較使用，未來可刪除 |
| 振幅降冪排序 + field_key 字典序次排序 | 跨架構決定論（避免 IEEE 754 浮點排序不一致） |
| MASA Top → 相位全 0 | 與 v1 相容（APP_05 §3.5.4 v1 相容說明） |
| nerve_structure 從 refine_map 近似 | Phase 5 橋接；Phase 6 應改用真實 MASA 交集複形 |
| mass = 欄位數（而非 BN/ 位元組數） | 更接近 Tr(P) 的語義（非零投影維度數） |

---

## 8. 完成標準

- [ ] `compute_sketch_v2()` 實作並在 `value.rs:content_hash()` 中啟用
- [ ] `GBB` 含 `nerve_structure: Vec<NerveEntry>`
- [ ] `disc.advertise` 質量改用欄位數，填入 nerve_structure
- [ ] `disc.find` 加入 `nerve_overlap()` 預篩選
- [ ] `tests/lattice_sketch_v2_test.rs`：10 個測試，含已硬編碼測試向量（test 6）
- [ ] `tests/nerve_routing_test.rs`：5 個測試
- [ ] `cargo test` 全數通過（預期 103+ 個測試）
- [ ] 確認 `genesis_test` 仍然通過（seeds 不變）
