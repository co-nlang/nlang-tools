# Phase 1a 交接文件：BN/ 序列化 + CAID v2 + 複數譜量化

> **執行者**：引擎開發 Agent  
> **預估工作量**：2–3 週（BN/ 5–7 天、Lattice Sketch 3–5 天、CAID v2 2 天、Phase-aware merge 3 天）  
> **前置條件**：引擎可編譯（v0.1 基礎可運行）  
> **完成判斷**：通過本文末尾的驗收測試清單

---

## 背景與目標

引擎目前（v0.1）的 CAID 是 `hash:sha256:<content_digest>`（v1 格式），內容摘要由 **JSON 序列化 + SHA256** 計算。這有兩個問題：

1. **JSON 非決定論**：欄位排序不穩定，跨平台結果可能不同。
2. **缺少幾何資訊**：無法支援 LADD 的「氣味搜尋」（譜距離比對）。

Phase 1a 的目標是將引擎升級到 **CAID v2**：

```
hash:sha256:v2:<masa_ref>:<lattice_sketch>:<content_digest>
```

三個步驟依序完成：

1. **BN/ 序列化**：用決定論位元流取代 JSON → 產生新的 `content_digest`
2. **Lattice Sketch**：複數譜量化 → 產生 `lattice_sketch`（Base64）
3. **CAID v2 格式 + Phase-aware merge**：組合新格式，更新 meet 運算

---

## 規格書參考

| 任務 | 主要規格 | 輔助參考 |
|:-----|:--------|:--------|
| BN/ 序列化 | `REAL_03 §6`（Binary-n/ 序列化格式） | `REAL_03 §5`（規範化規則） |
| 複數譜量化 | `REAL_03 §3.2`（幾何摘要） | `APP_05 §3.5`（Lattice Sketch） |
| CAID v2 格式 | `REAL_03 §1–2` | — |
| Phase-aware merge | `REAL_03 §4`（相位感知合併） | `SPEC_06 §1.3.1` |
| 創世種子 CAID | `REAL_03 §2.1` | `SPEC_13 §3` |

規格書位置：`nlang-spec/spec/zh_TW/`

---

## 步驟一：BN/ 序列化

### 目標

實作一個 `fn serialize_bn(value: &Value) -> Vec<u8>` 函式，
將 n/ 值序列化為決定論位元流，供 SHA256 計算使用。

### 實作位置

新建 `crates/interpreter/src/bn_serial.rs`，並在 `lib.rs` 加入 `mod bn_serial;`。

### BN/ 欄位排序（REAL_03 §6.2）

序列化 Combo 欄位前必須按以下優先順序排序：

| 優先級 | 前綴 | ComboVal 欄位 |
|:------:|:-----|:-------------|
| 1 | `~%` | `system` |
| 2 | `%` | `meta` |
| 3 | `@` | `types` |
| 4 | `/` | `rules` |
| 5 | (無) | `data` |
| 6 | `~` | `local` |

同優先級內，按欄位名稱 Unicode 代碼點遞增排序。

### 值類型標記（REAL_03 §6.2）

```rust
const TAG_COMBO:  u8 = 0x01;   // Combo {}
const TAG_COCOON: u8 = 0x02;   // Cocoon {{}}
const TAG_LIST:   u8 = 0x03;   // List []
const TAG_TUPLE:  u8 = 0x04;   // Tuple ()
const TAG_ATOM:   u8 = 0x10;   // Atom (generic string bytes)
const TAG_TAG:    u8 = 0x11;   // Tag (#foo)
const TAG_INT64:  u8 = 0x12;   // Int64 (LEB128 signed)
const TAG_FLOAT:  u8 = 0x13;   // Float (i64_LEB128 + u64)
const TAG_COMPLEX: u8 = 0x14;  // Complex (Float + Float)
const TAG_BOOL:   u8 = 0x15;   // Bool (0x00=false, 0x01=true)
const TAG_REF:    u8 = 0x16;   // CAID reference (UTF-8 string)
```

### 編碼規則

**LEB128（整數）**：
- 有符號（`i64`）：Signed LEB128
- 無符號（`u32` 欄位數、字串長度）：Unsigned LEB128

**字串**：`[長度: u32 LEB128][UTF-8 bytes]`

**浮點數（128-bit 定點）**：
```
[整數部: i64 signed LEB128][小數部: u64 小端 8 bytes]
```
小數部：`frac * 2^64` 取整（代表 [0, 1) 範圍）

**複數**：序列化實部，再序列化虛部（各自用浮點數格式）

**Combo**：
```
[TAG_COMBO or TAG_COCOON: u8]
[欄位數: u32 LEB128]
對每個欄位（按上述排序）：
  [欄位名長度: u32 LEB128][欄位名 UTF-8]
  [值: 遞迴值編碼]
```

**Top（`_`）**：單字節 `0xFF`

**Bottom（`_|_`）**：單字節 `0xFE`（僅序列化結構，不含 cause 細節）

**Tag（`#foo`）**：`[TAG_TAG][名稱長度 LEB128][UTF-8]`（不含 `#` 前綴）

**Union**：序列化前按每個分支的 CAID 字典序排序（見規格 §5.2.2），再依序序列化各分支

### 規範化前處理（REAL_03 §5）

序列化前對字串執行 NFC Unicode 正規化，整數不含前導零，浮點數用小寫 `e`。

### API 簽章

```rust
// crates/interpreter/src/bn_serial.rs

pub fn serialize_bn(value: &Value) -> Vec<u8>

// 計算 content_digest（SHA256 of BN/ bytes）
pub fn content_digest(value: &Value) -> [u8; 32] {
    use sha2::{Sha256, Digest};
    let bytes = serialize_bn(value);
    Sha256::digest(&bytes).into()
}
```

### 驗收標準

- `{ x: 3, y: 4 }` 和 `{ y: 4, x: 3 }` 產生**相同** BN/ 位元流
- 跨測試重複呼叫同一值，輸出完全相同（決定論）
- 通過 `REAL_03 §6.3` 的範例（`@point: { x: 3, y: 4 }`）

---

## 步驟二：Lattice Sketch（複數譜量化）

### 目標

計算投影算子 $P_A$ 的複數譜特徵摘要，編碼為 Base64 字串，成為 CAID v2 的 `<lattice_sketch>` 欄位。

### 背景

Lattice Sketch 的數學意義：對一個 Combo（子空間）$A$，它的投影算子 $P_A$ 的特徵值 $\lambda_i$ 帶有相位 $\theta_i$（相對於所在的 MASA 參考系）。這些資訊壓縮成 Base64 摘要，讓 LADD 可以做「氣味搜尋」——無需下載完整內容即可估算兩個 Combo 的幾何距離。

### Phase 1a 的簡化策略

完整的 Lattice Sketch 需要實際做投影算子的特徵值分解，這依賴 LADD 的 MASA 基礎設施（Phase 4）。Phase 1a 採用**佔位實作**：

1. 對於**無 MASA 上下文的 Combo**（masa_ref = `_`）：
   - `lattice_sketch` = 對 Combo 的 BN/ bytes 做 SHA256，取前 12 bytes，Base64 編碼
   - 這是一個**結構近似**，足以讓 CAID v2 格式運作，LADD 路由時再替換為真實值

2. 對於**已知複數特徵值** $(\lambda_i, \theta_i)$ 的情境（未來 Phase 4 才有）：

```
for each (λ_i, θ_i):
    amp_fixed = to_fixed128(λ_i)         // i64 整數部 + u64 小數部
    phase_fixed = to_fixed128(θ_i / π)   // 歸一化到 [-1, 1]
    
    delta_encode(amp_fixed)    // 差分編碼
    zigzag_encode(...)         // ZigZag 映射到無符號
    leb128_encode(...)         // LEB128 壓縮

最終：Base64(LEB128 bytes)
```

### 實作位置

新建 `crates/interpreter/src/lattice_sketch.rs`

### API 簽章

```rust
// crates/interpreter/src/lattice_sketch.rs

pub struct SpectralPoint {
    pub amplitude: f64,   // 特徵值 λ_i ∈ [0, 1]
    pub phase: f64,       // 相位 θ_i ∈ [-π, π]
}

/// Phase 1a 佔位實作：由 BN/ 結構近似
pub fn compute_sketch_approximate(bn_bytes: &[u8]) -> String {
    use sha2::{Sha256, Digest};
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let hash = Sha256::digest(bn_bytes);
    STANDARD.encode(&hash[..12])
}

/// Phase 4+ 完整實作（留空，未來填入）
pub fn compute_sketch_full(points: &[SpectralPoint]) -> String {
    todo!("Phase 4: requires MASA eigenvalue decomposition")
}
```

---

## 步驟三：CAID v2 格式

### 目標

將 `ContentHash` 更新為 v2 格式，同時保留 v1 向後相容性（創世 Commit 用）。

### 修改位置：`crates/interpreter/src/value.rs`

#### 更新 `ContentHash` 結構體

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContentHash {
    pub algorithm: HashAlgorithm,
    pub version: CaidVersion,
    pub masa_ref: MasaRef,
    pub lattice_sketch: String,   // Base64，v1 時為空字串
    pub digest: Vec<u8>,          // SHA256 of BN/ bytes
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CaidVersion { V1, V2 }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MasaRef {
    Top,              // "_"：MASA 自身，或無父脈絡
    Digest(Vec<u8>),  // MASA 的 content_digest（64 hex chars = 32 bytes）
}
```

#### 更新 `Display`（格式化輸出）

```rust
impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let algo = match self.algorithm { HashAlgorithm::Sha256 => "sha256", };
        let digest_hex: String = self.digest.iter().map(|b| format!("{:02x}", b)).collect();
        match self.version {
            CaidVersion::V1 => write!(f, "hash:{}:v1:{}", algo, digest_hex),
            CaidVersion::V2 => {
                let masa = match &self.masa_ref {
                    MasaRef::Top => "_".to_string(),
                    MasaRef::Digest(d) => d.iter().map(|b| format!("{:02x}", b)).collect(),
                };
                write!(f, "hash:{}:v2:{}:{}:{}", algo, masa, self.lattice_sketch, digest_hex)
            }
        }
    }
}
```

#### 更新 `parse`（字串解析）

```rust
impl ContentHash {
    pub fn parse(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.splitn(4, ':').collect();
        // "hash" : algo : "v1" or "v2" : rest
        match parts[2] {
            "v1" => { /* 解析 digest */ }
            "v2" => {
                // rest = "<masa_ref>:<lattice_sketch>:<digest>"
                let rest_parts: Vec<&str> = parts[3].splitn(3, ':').collect();
                // rest_parts[0] = masa_ref, [1] = lattice_sketch, [2] = digest
            }
            _ => Err(anyhow!("Unknown CAID version"))
        }
    }
}
```

#### 更新 `content_hash()` 計算

舊實作（JSON + SHA256）替換為：

```rust
impl Value {
    pub fn content_hash(&self) -> ContentHash {
        use crate::bn_serial::content_digest;
        use crate::lattice_sketch::compute_sketch_approximate;
        
        let bn_bytes = crate::bn_serial::serialize_bn(self);
        let digest = content_digest(self);
        let sketch = compute_sketch_approximate(&bn_bytes);
        
        ContentHash {
            algorithm: HashAlgorithm::Sha256,
            version: CaidVersion::V2,
            masa_ref: MasaRef::Top,      // Phase 1a：全部用 Top
            lattice_sketch: sketch,
            digest: digest.to_vec(),
        }
    }
    
    // v1 格式（僅供創世 Commit 使用）
    pub fn content_hash_v1(&self) -> ContentHash {
        use crate::bn_serial::content_digest;
        let digest = content_digest(self);
        ContentHash {
            algorithm: HashAlgorithm::Sha256,
            version: CaidVersion::V1,
            masa_ref: MasaRef::Top,
            lattice_sketch: String::new(),
            digest: digest.to_vec(),
        }
    }
}
```

### 注意：創世種子 CAID

`storage.rs` 中的創世 Commit（`$C_0$`）應呼叫 `content_hash_v1()`，而非一般的 `content_hash()`。
確認 `storage.rs:CommitEntry::root` 的初始化邏輯是否需要調整。

---

## 步驟四：Phase-aware Merge

### 目標

在 `unify.rs` 的 meet（`&`）運算中，當合併兩個帶有 CAID v2 的 Combo 時，
根據 MASA 重疊和幾何相位差做三路決策（REAL_03 §4）。

### 決策邏輯（REAL_03 §4.1）

```
合併 Combo A（masa_ref=X）和 Combo B（masa_ref=Y）：

1. 計算 MASA 重疊：
   overlap = X & Y
   if overlap == _|_:
     → SPLIT（H² obstruction，MASA 上下文不相容）
     → 記錄 Bottom，cause = #h2_split

2. 計算幾何相位差：
   θ_AB = phase_difference(A, B)
          = arccos(Tr(P_A · P_B) / (||P_A|| · ||P_B||))
   
   // Phase 1a: masa_ref 全是 Top，直接 MERGE
   // TODO Phase 4: 引入真實 MASA 後，用 Hamming distance 做估算
   // TODO Phase 5: 完整特徵值分解後，用 Frobenius 範數 d_L^ℂ

3. 決策：
   if θ_AB < ε_coherent (0.1 rad):
     → MERGE（相干疊加，H¹ 小到可忽略）
   if θ_AB ≥ ε_coherent:
     → SPLIT，%cause 記錄 H¹ survivor
   if θ_AB ≈ π/2 (±0.05 rad):
     → H¹ orthogonal survivor
```

### Phase 1a 簡化實作

Phase 1a 的 masa_ref 全部是 `Top`，所以 MASA 重疊永遠 ≠ `_|_`（步驟 1 不觸發）。
重點是**架構正確**：讓 unify 可以接受並傳遞 masa_ref，Phase 4 再填入真實計算。

```rust
// crates/interpreter/src/unify.rs
// 在 unify_internal 的 Combo+Combo 分支中加入：

fn phase_merge_decision(a: &ComboVal, b: &ComboVal, epsilon: f64) -> MergeDecision {
    // Phase 1a：兩者 masa_ref 都是 Top → 直接 MERGE
    if a.masa_ref() == MasaRef::Top && b.masa_ref() == MasaRef::Top {
        return MergeDecision::Merge;
    }
    // TODO Phase 4：實作真實的相位差計算
    MergeDecision::Merge
}

enum MergeDecision { Merge, Split(String) }

const EPSILON_COHERENT: f64 = 0.1; // rad
```

需要在 `ComboVal` 加入 `masa_ref: MasaRef` 欄位（預設 `MasaRef::Top`）。

---

## 修改檔案清單

| 檔案 | 動作 | 說明 |
|:-----|:-----|:-----|
| `crates/interpreter/src/bn_serial.rs` | **新建** | BN/ 序列化實作 |
| `crates/interpreter/src/lattice_sketch.rs` | **新建** | Lattice Sketch 計算 |
| `crates/interpreter/src/value.rs` | **修改** | ContentHash 結構、content_hash()、ComboVal 加 masa_ref |
| `crates/interpreter/src/unify.rs` | **修改** | phase_merge_decision() 架構 |
| `crates/interpreter/src/lib.rs` | **修改** | 加入 mod bn_serial; mod lattice_sketch; |
| `crates/interpreter/src/storage.rs` | **檢查** | 創世 Commit 改用 content_hash_v1() |

---

## 驗收測試清單

以下測試全部通過才算 Phase 1a 完成：

### BN/ 決定論
- [ ] `{ x: 3, y: 4 }` 和 `{ y: 4, x: 3 }` 產生相同 BN/ bytes 和相同 CAID
- [ ] 同一個 Combo 多次呼叫 `content_hash()` 結果完全相同
- [ ] Cocoon `{{ x: 1 }}` 的 BN/ 類型標記為 `0x02`（非 `0x01`）

### CAID v2 格式
- [ ] `display()` 輸出格式為 `hash:sha256:v2:_:<sketch>:<64hex>`
- [ ] `parse()` 可正確解析 v1 和 v2 兩種格式
- [ ] 創世 Commit 的 CAID 格式為 v1（`hash:sha256:v1:<64hex>`）

### 相容性
- [ ] 現有 `oo test` 測試全部通過（舊測試用例不應因格式變更而失敗）
- [ ] `oo repl` 可正常使用，`~%Engine./observe` 回傳的 CAID 格式正確

### Cargo
- [ ] `cargo build` 無 warning（不含 `#[allow(dead_code)]` 遮蓋的警告）
- [ ] `cargo test` 通過

---

## 不在 Phase 1a 範圍內

以下項目**刻意延後**，不要在此階段實作：

| 項目 | 延後至 |
|:-----|:------|
| 真實的特徵值分解（投影算子計算） | Phase 4（LADD） |
| 非 Top 的 masa_ref（來自真實 MASA） | Phase 1c |
| H² SPLIT 觸發（MASA 上下文不相容） | Phase NEW（%obstruction_degree） |
| Lattice Sketch 的真實幾何距離計算 | Phase 4（APP_05 §3.5.4） |
| `oo caid-test-suite`（官方標準套件） | Phase 1a 末期或 Phase 2 |

---

## 引擎整體路線圖（供參考）

```
Phase 1a（本文）：BN/ + CAID v2 + Lattice Sketch（近似） + Phase-aware merge（架構）
Phase 1b：ε_coherent 相位感知合併邏輯（真實計算）
Phase 1c：MASA 創世種子 CAID 確定（genesis defaults）
Phase NEW：%obstruction_degree + %cause cocycle + /%differential.{1,2,3}
Phase 2：StdLib（EML 派生函數、創世預設值）—— 可與 1b/1c 並行
Phase 3：#refine 精煉機制
Phase 4：LADD 基礎路由（需要 %obstruction_degree 先完成）
Phase 5：LADD nerve-aware 路由 + 真實 Lattice Sketch
Phase 6：%project_down / %project_up（Bohrification 視角）
```

---

## 快速定位

```bash
# 查看當前 ContentHash 實作
grep -n "ContentHash\|content_hash" crates/interpreter/src/value.rs

# 查看 Combo meet 邏輯
grep -n "Combo.*Combo\|unify_internal" crates/interpreter/src/unify.rs

# 編譯測試
cargo build -p nlang-interpreter
cargo test -p nlang-interpreter

# 快速 REPL 驗證
cargo run -p oo -- repl
```
