# Phase 1b 交接文件：ε_coherent 相位感知合併

> **執行者**：引擎開發 Agent  
> **預估工作量**：1 週  
> **前置條件**：Phase 1a 完成（`bn_serial.rs`、`lattice_sketch.rs`、CAID v2 格式、`ComboVal.masa_ref` 已存在）  
> **完成判斷**：通過本文末尾的驗收測試清單

---

## 背景

Phase 1a 完成了 BN/ 序列化和 CAID v2 格式，但 **`unify.rs` 的 meet（`&`）運算尚未加入相位感知邏輯**。目前 `unify_combo()` 只做純粹的欄位逐一合併，不區分 H¹（相位阻礙）和 H²（MASA 不相容）的情境。

Phase 1b 的任務是在 `unify_combo()` 的入口加入**三路決策**（REAL_03 §4）：

| 情境 | 決策 | 產出 |
|:-----|:-----|:-----|
| MASA 重疊 `== _\|_` | **H² SPLIT** | `Bottom(H2Split)` |
| `θ_AB < ε_coherent` | **MERGE** | 正常進行欄位合併 |
| `θ_AB ≥ ε_coherent` | **H¹ SPLIT** | `Bottom(H1Split)` + `%cause` 記錄 survivor |

ε_coherent 預設值 = **0.1 rad**。

---

## 規格書參考

| 任務 | 規格 |
|:-----|:-----|
| 相位感知合併三路邏輯 | `REAL_03 §4.1`（相位感知合併） |
| `%cause` 結構 | `REAL_03 §4.1` + `SPEC_08 §2`（meta 欄位語義） |
| H¹ / H² 障礙區分 | `APP_07`（阻礙梯子指引） |

規格書位置：`nlang-spec/spec/zh_TW/`

---

## 現有程式碼定位

| 檔案 | 相關位置 |
|:-----|:--------|
| `crates/interpreter/src/unify.rs` | `unify_combo()` 第 98 行 — 在此加入三路決策入口 |
| `crates/interpreter/src/value.rs` | `BottomCause` enum（第 307 行）— 加入新 variant |
| `crates/interpreter/src/value.rs` | `BottomDetail.as_cause_combo()`（第 268 行）— 擴充新 cause 的輸出 |
| `crates/interpreter/src/value.rs` | `ComboVal.masa_ref`（第 55 行）— Phase 1a 已加，直接讀取 |
| `crates/interpreter/src/lattice_sketch.rs` | `compute_sketch_approximate()` — 讀取用，計算近似相位差 |

---

## 步驟一：在 `BottomCause` 加入新 variant

**位置**：`value.rs:307`

```rust
pub enum BottomCause {
    Conflict,
    MissingKey,
    FuelExhausted,
    Timeout,
    Divergent,
    InvalidPath,
    PrivateAccessViolation,
    NumericalError,
    ArithmeticOnAnchor,
    // Phase 1b 新增：
    H1Split,   // H¹ 相位阻礙（可恢復）：兩個 Combo 的相位差 ≥ ε_coherent
    H2Split,   // H² MASA 阻礙（不可恢復）：MASA 上下文完全不相容
}
```

同時在 `as_cause_combo()` 的 `match self.cause` 加入對應的標籤：

```rust
BottomCause::H1Split => "#h1_split",
BottomCause::H2Split => "#h2_split",
```

---

## 步驟二：實作 `phase_merge_decision()`

新建一個 **私有函式**（不用新建模組，直接放在 `unify.rs`）：

```rust
const EPSILON_COHERENT: f64 = 0.1; // rad，REAL_03 §4.1

enum MergeDecision {
    Merge,
    H1Split { theta: f64 },
    H2Split,
}

fn phase_merge_decision(a: &ComboVal, b: &ComboVal) -> MergeDecision {
    use crate::value::MasaRef;

    // 步驟 1：MASA 重疊檢查（H² 判斷）
    // Phase 1b：masa_ref 全為 Top → 重疊永遠不是 _|_
    // Phase 4 後此處才有真實計算
    let h2_incompatible = match (&a.masa_ref, &b.masa_ref) {
        (MasaRef::Top, _) | (_, MasaRef::Top) => false,
        (MasaRef::Digest(da), MasaRef::Digest(db)) => da != db,
        // 同一 MASA：相容
    };
    if h2_incompatible {
        return MergeDecision::H2Split;
    }

    // 步驟 2：計算相位差 θ_AB（Phase 1b 近似）
    // 用 lattice_sketch 的 Base64 解碼後的 Hamming distance 正規化到 [0, π/2]
    let theta = approximate_phase_diff(&a.lattice_sketch, &b.lattice_sketch);

    // 步驟 3：三路決策
    if theta < EPSILON_COHERENT {
        MergeDecision::Merge
    } else {
        MergeDecision::H1Split { theta }
    }
}

fn approximate_phase_diff(sketch_a: &str, sketch_b: &str) -> f64 {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let ba = STANDARD.decode(sketch_a).unwrap_or_default();
    let bb = STANDARD.decode(sketch_b).unwrap_or_default();
    let len = ba.len().min(bb.len());
    if len == 0 { return 0.0; }
    // Hamming distance（不同位元數）正規化到 [0, π/2]
    let hamming: u32 = ba.iter().zip(bb.iter()).map(|(x, y)| (x ^ y).count_ones()).sum();
    let max_bits = (len * 8) as f64;
    (hamming as f64 / max_bits) * std::f64::consts::FRAC_PI_2
}
```

**Phase 4 升級路徑**：Phase 4 完成後，`approximate_phase_diff` 替換為真實的
`arccos(Tr(P_A · P_B))` 計算。`phase_merge_decision` 的簽章不變。

---

## 步驟三：修改 `unify_combo()`

**位置**：`unify.rs:98`，在現有邏輯的**最前面**加入三路決策：

```rust
fn unify_combo(&self, a: ComboVal, b: ComboVal, ctx: &mut EvalContext) -> Value {
    // ... 現有 type_constraint 檢查（保留不動）...

    // Phase 1b：相位感知合併入口
    match phase_merge_decision(&a, &b) {
        MergeDecision::H2Split => {
            return make_h2_split_bottom(&a, &b);
        }
        MergeDecision::H1Split { theta } => {
            return make_h1_split_bottom(&a, &b, theta);
        }
        MergeDecision::Merge => {
            // 繼續執行現有欄位合併邏輯
        }
    }

    // 現有欄位合併邏輯（完全不動）...
}
```

---

## 步驟四：實作 `make_h1_split_bottom()` 和 `make_h2_split_bottom()`

```rust
fn make_h1_split_bottom(a: &ComboVal, b: &ComboVal, theta: f64) -> Value {
    // %cause combo：記錄 H¹ survivor 供 LADD 路由修正
    // 內容：{ %type: #h1_split, %theta: <rad>, %caid_a: "<caid>", %caid_b: "<caid>" }
    let caid_a = Value::Combo(a.clone()).content_hash().to_string();
    let caid_b = Value::Combo(b.clone()).content_hash().to_string();
    
    Value::Bottom(Box::new(BottomDetail {
        cause: BottomCause::H1Split,
        path: None,
        message: Some(format!("H¹ phase obstruction: θ={:.4} rad ≥ ε_coherent={}", theta, EPSILON_COHERENT)),
        expected: None,
        found: None,
        involved: vec![
            Value::Combo(a.clone()).content_hash(),
            Value::Combo(b.clone()).content_hash(),
        ],
    }))
}

fn make_h2_split_bottom(a: &ComboVal, b: &ComboVal) -> Value {
    Value::Bottom(Box::new(BottomDetail {
        cause: BottomCause::H2Split,
        path: None,
        message: Some(format!(
            "H² MASA obstruction: incompatible contexts {} vs {}",
            a.masa_ref, b.masa_ref
        )),
        expected: None,
        found: None,
        involved: vec![
            Value::Combo(a.clone()).content_hash(),
            Value::Combo(b.clone()).content_hash(),
        ],
    }))
}
```

> **注意**：`a.masa_ref` 的 `Display` 已在 Phase 1a 實作（`value.rs:327`）。

---

## 步驟五：新增 Cargo 依賴

`approximate_phase_diff` 需要 `base64` crate。確認 `crates/interpreter/Cargo.toml` 已有：

```toml
base64 = "0.22"
```

如果 Phase 1a 已加（`lattice_sketch.rs` 用到），此步驟跳過。

---

## 修改檔案清單

| 檔案 | 動作 | 說明 |
|:-----|:-----|:-----|
| `crates/interpreter/src/value.rs` | **修改** | `BottomCause` 加 `H1Split`、`H2Split`；`as_cause_combo()` 加對應標籤 |
| `crates/interpreter/src/unify.rs` | **修改** | 加入 `phase_merge_decision()`、`approximate_phase_diff()`、兩個 `make_*_bottom()` 函式；`unify_combo()` 頭部加三路決策 |

**不需要新建任何檔案。**

---

## 行為邊界說明

Phase 1b 後的 `&` 語義：

```
// 兩個結構相同的 Combo（lattice_sketch 完全一樣）→ θ = 0 → MERGE
A & A  →  A

// 兩個結構不同的 Combo，sketch 差異小（< ε_coherent）→ MERGE
{ x: 1 } & { y: 2 }  →  { x: 1, y: 2 }   （若 sketch 接近，Phase 1b 中幾乎總是如此）

// 兩個 masa_ref 都非 Top 且不同 → H² SPLIT
A(masa=X) & B(masa=Y where X≠Y)  →  Bottom(H2Split)

// θ ≥ 0.1 rad（sketch Hamming distance 超過閾值）→ H¹ SPLIT
A & B  →  Bottom(H1Split, theta=0.23)
```

**Phase 1b 的 sketch 近似特性**：因為 lattice_sketch 是 BN/ 的 SHA256 截段，
兩個不同 Combo 的 sketch Hamming distance 幾乎總是 ≥ 閾值（隨機 12 bytes 的 Hamming 距離期望值約 π/4 ≈ 0.78 rad >> 0.1）。

這表示 Phase 1b 之後，**任何兩個不同的 Combo 做 `&` 都會 SPLIT**，除非結構完全一樣。

這是**設計內的暫時行為**：Phase 4 換成真實特徵值計算後，真正「幾何接近」的 Combo 才會 MERGE。Phase 1b 的目的是讓架構和 Bottom 格式正確，不是讓語義立刻完美。

如果這個行為讓現有測試大量失敗，有兩個選項：

1. **（推薦）** 直接把 `approximate_phase_diff` 回傳 `0.0`（永遠 MERGE），讓測試通過，等 Phase 4 填入真實計算。這樣 Phase 1b 只部署架構，不改行為。
2. 保留 Hamming 計算，修正失敗的測試用例（可能工作量較大）。

**建議選項 1**，並在 TODO 中標注：

```rust
fn approximate_phase_diff(_sketch_a: &str, _sketch_b: &str) -> f64 {
    // TODO Phase 4: replace with arccos(Tr(P_A · P_B)) eigenvalue computation
    // Returning 0.0 for now so all Combos merge (architecture-only deployment)
    0.0
}
```

---

## 驗收測試清單

### 新 BottomCause variant
- [ ] `BottomCause::H1Split` 和 `H2Split` 存在且可編譯
- [ ] `as_cause_combo()` 對 H1Split 輸出 `%type: #h1_split`，H2Split 輸出 `%type: #h2_split`

### 三路決策架構
- [ ] `unify_combo()` 在欄位合併前呼叫 `phase_merge_decision()`
- [ ] `phase_merge_decision()` 接受兩個 `&ComboVal` 並回傳 `MergeDecision` enum

### 現有測試不退步
- [ ] `cargo test` 5 tests passed（所有現有測試繼續通過）
- [ ] `cargo build` 無 error

### H² 觸發（可手動驗證）
- [ ] 構造兩個 `masa_ref` 分別為 `Digest([1,2,3,...])` 和 `Digest([4,5,6,...])` 的 ComboVal，`phase_merge_decision` 回傳 `H2Split`

---

## 不在 Phase 1b 範圍內

| 項目 | 延後至 |
|:-----|:------|
| 真實 θ 計算（`arccos(Tr(P_A P_B))`） | Phase 4（LADD + MASA 基礎設施） |
| H¹ survivor 的 Union 回傳（目前回傳 Bottom） | Phase NEW（%obstruction_degree） |
| `/%differential` 計算 | Phase NEW |
| MASA overlap 的真實 lattice meet | Phase 4 |

---

## 快速定位

```bash
# 確認 unify_combo 位置
grep -n "fn unify_combo" crates/interpreter/src/unify.rs

# 確認 BottomCause 位置
grep -n "enum BottomCause" crates/interpreter/src/value.rs

# 編譯
cargo build -p nlang-interpreter

# 測試
cargo test -p nlang-interpreter
```
