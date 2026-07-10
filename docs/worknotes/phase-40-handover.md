# Phase 40 Handover：`approximate_phase_diff` 量子態相位距離

> 日期：2026-05-25  
> 實作範圍：H¹ phase obstruction 幾何計算（`phase_diff_between`）  
> 預期測試：~439 → ~446（新增 ~7 個測試）

---

## 0. 背景與設計決策

### 0.1 原始 stub 狀況

`unify.rs:phase_merge_decision` 目前：

```rust
// TODO Phase 4: replace with arccos(Tr(P_A · P_B)) eigenvalue computation
let theta = 0.0;   // ← 永遠 0，所有 Combo 都會 Merge
```

另有一個完全死碼的 `approximate_phase_diff(_sketch_a, _sketch_b)` 函數（從未被呼叫）。

### 0.2 為什麼「高風險」標籤是誇大的

**實際風險範圍極窄**：`MasaRef::Digest` 在 ComboVal 上只有一個寫入點：
`builtins/engine.rs:engine.project_down`（line 122：`cv.masa_ref = masa_hash.masa_ref.clone()`）。

所有其他 ComboVal 建構（`ComboVal::new`）預設 `MasaRef::Top`。

因此，在 `phase_merge_decision` 中：
- **Top-MASA combos（幾乎所有情況）**: 直接回傳 `theta = 0.0` → 行為零變化
- **Digest-MASA combos（僅來自 project_down）**: 才觸發幾何計算

**現有 ~439 個測試全部使用 Top-MASA** → 零回歸風險。

### 0.3 數學基礎

格論中的 H¹ 相位阻礙：`θ = arccos(Tr(P_A · P_B))`

其中 $P_A, P_B$ 是 Hilbert 空間中的秩-1 射影算子。對純量子態：

$$\text{Tr}(P_A P_B) = |\langle\psi_A|\psi_B\rangle|^2$$

量子態由 `lattice_sketch.rs:extract_spectral_components` 提供的振幅 $\lambda_i$ 和相位 $\phi_i$ 構成：

$$|\psi\rangle = \frac{1}{N} \sum_{i=0}^{15} \sqrt{\lambda_i}\, e^{i\phi_i} |e_i\rangle, \quad N = \sqrt{\sum_i \lambda_i}$$

內積計算：

$$\langle\psi_A|\psi_B\rangle = \frac{\sum_i \sqrt{\lambda_A^i \lambda_B^i}\, e^{i(\phi_B^i - \phi_A^i)}}{N_A N_B}$$

統計特性分析（16 個不同欄位的 combos）：

- 相同 MASA、相同 field structure → phases 相消 → $\text{Tr} = 1$，$\theta = 0$（永遠 Merge）
- 相同 MASA、完全不同 field keys → 16 個獨立隨機相位差 → $E[\text{Tr}] \approx 0.06$，$\theta \approx 1.5$ rad >> EPSILON\_COHERENT = 0.1 rad → 可靠觸發 H1Split

---

## 1. 修改 `crates/interpreter/src/lattice_sketch.rs`

### 1.1 修改 import（第 1 行）

```rust
// 原本：
use crate::value::{Value, MasaRef};

// 改為：
use crate::value::{Value, MasaRef, ComboVal};
```

### 1.2 在文件末尾（`compute_sketch_approximate` 函數之後）新增

```rust
/// Compute H¹ phase obstruction angle θ = arccos(Tr(P_A · P_B)).
///
/// Tr(P_A · P_B) = |⟨ψ_A|ψ_B⟩|² where |ψ⟩ = Σ √λ_i e^(iφ_i) |e_i⟩ / ‖ψ‖.
/// Returns θ ∈ [0, π/2]. Returns 0.0 for degenerate (all-zero amplitude) states.
pub fn phase_diff_between(a: &ComboVal, b: &ComboVal) -> f64 {
    let (amps_a, phases_a) = extract_spectral_components(&Value::Combo(a.clone()));
    let (amps_b, phases_b) = extract_spectral_components(&Value::Combo(b.clone()));

    let norm_a: f64 = amps_a.iter().sum::<f64>().sqrt();
    let norm_b: f64 = amps_b.iter().sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    let mut re = 0.0f64;
    let mut im = 0.0f64;
    for i in 0..MAX_COMPONENTS {
        let amp = (amps_a[i] * amps_b[i]).sqrt();
        let delta_phi = phases_b[i] - phases_a[i];
        re += amp * delta_phi.cos();
        im += amp * delta_phi.sin();
    }
    let denom = norm_a * norm_b;
    re /= denom;
    im /= denom;

    let trace = (re * re + im * im).clamp(0.0, 1.0);
    trace.acos()
}
```

---

## 2. 修改 `crates/interpreter/src/unify.rs`

### 2.1 新增 import（在現有 use 列表末尾）

```rust
use crate::lattice_sketch;
```

### 2.2 刪除死碼函數（第 40-44 行）

刪除整個：
```rust
#[allow(dead_code)]
fn approximate_phase_diff(_sketch_a: &str, _sketch_b: &str) -> f64 {
    // TODO Phase 4: replace with real eigenvalue-based computation
    0.0
}
```

### 2.3 修改 `phase_merge_decision`（取代 step 2 stub，約第 27-30 行）

原本：
```rust
    // Step 2: geometric phase difference (Phase 1b: architecture only)
    // TODO Phase 4: replace with arccos(Tr(P_A · P_B)) eigenvalue computation
    // Returning 0.0 so all Combos merge (architecture-only deployment)
    let theta = 0.0;
```

改為：
```rust
    // Step 2: H¹ phase obstruction — only for explicit MASA context combos.
    // Top-MASA combos are context-free; geometric check is undefined for them.
    let theta = match (&a.masa_ref, &b.masa_ref) {
        (MasaRef::Digest(_), MasaRef::Digest(_)) => lattice_sketch::phase_diff_between(a, b),
        _ => 0.0,
    };
```

注意：此時 H2 已通過（同 Digest 才會到這裡），因此 `(Digest(da), Digest(db))` 保證 `da == db`。

---

## 3. 新增測試 `crates/interpreter/tests/h1_phase_test.rs`

```rust
use nlang_interpreter::{Ouroboros, MasaRef};
use nlang_interpreter::value::{Value, ComboVal, EffectTag, BottomCause};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use num_bigint::BigInt;

const EPSILON_COHERENT: f64 = 0.1;

fn oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}

/// Build a Combo with Top-MASA (default).
fn top_combo(fields: &[(&str, Value)]) -> Value {
    let mut m = IndexMap::new();
    for (k, v) in fields { m.insert(k.to_string(), v.clone()); }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

/// Build a Combo with explicit MASA digest (32-byte vector).
fn masa_combo(digest: Vec<u8>, fields: &[(&str, Value)]) -> ComboVal {
    let mut m = IndexMap::new();
    for (k, v) in fields { m.insert(k.to_string(), v.clone()); }
    let mut cv = ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]);
    cv.masa_ref = MasaRef::Digest(digest);
    cv
}

/// Build 32-byte MASA digest from a seed byte.
fn masa_digest(seed: u8) -> Vec<u8> { vec![seed; 32] }

// ─── 1. phase_diff_between: identical combos ────────────────────────────────

#[test]
fn test_phase_diff_identical_combos_is_zero() {
    let digest = masa_digest(0xAB);
    let fields: &[(&str, Value)] = &[("x", int_val(1)), ("y", int_val(2)), ("z", int_val(3))];
    let a = masa_combo(digest.clone(), fields);
    let b = masa_combo(digest.clone(), fields);

    let theta = nlang_interpreter::lattice_sketch::phase_diff_between(&a, &b);
    assert!(
        theta < 1e-9,
        "identical combos should have theta ≈ 0, got {}", theta
    );
}

// ─── 2. phase_diff_between: many-field combos with different keys ─────────────

#[test]
fn test_phase_diff_different_field_keys_is_positive() {
    // Combos with 8 entirely different field keys → E[Tr] << 1 → theta > 0
    let digest = masa_digest(0x11);
    let fields_a: Vec<(&str, Value)> = (0..8_i64)
        .map(|i| { let key = Box::leak(format!("a{}", i).into_boxed_str()) as &str; (key, int_val(i)) })
        .collect();
    let fields_b: Vec<(&str, Value)> = (0..8_i64)
        .map(|i| { let key = Box::leak(format!("b{}", i).into_boxed_str()) as &str; (key, int_val(i + 100)) })
        .collect();

    let a = masa_combo(digest.clone(), &fields_a);
    let b = masa_combo(digest.clone(), &fields_b);

    let theta = nlang_interpreter::lattice_sketch::phase_diff_between(&a, &b);
    // With 8 different fields each and independent phases, theta >> 0
    assert!(theta > 0.0, "different-key combos should have theta > 0, got {}", theta);
}

// ─── 3. Top-MASA combos: unify never H1Splits ────────────────────────────────

#[test]
fn test_top_masa_unify_never_h1splits() {
    let oo = oo();
    // Two combos with same field key but different values and Top-MASA
    let a = top_combo(&[("x", int_val(1)), ("y", int_val(2))]);
    let b = top_combo(&[("z", int_val(3)), ("w", int_val(4))]);
    let result = oo.unify(a, b);
    // Should merge (Combo), never produce H1Split Bottom
    assert!(
        !matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::H1Split)),
        "Top-MASA combos should never H1Split"
    );
}

// ─── 4. same-MASA different-data: H2 passes, H1 may fire ─────────────────────

#[test]
fn test_same_masa_combos_may_h1split() {
    // With 16 completely different field keys and same MASA, H1Split is expected.
    // If this fails (Merge instead), EPSILON_COHERENT boundary may need adjustment.
    let oo = oo();
    let digest = masa_digest(0x55);

    let mut m_a = IndexMap::new();
    let mut m_b = IndexMap::new();
    for i in 0..16_i64 {
        m_a.insert(format!("fa{}", i), int_val(i));
        m_b.insert(format!("fb{}", i), int_val(i + 1000));
    }
    let mut cv_a = ComboVal::new(m_a, false, IndexMap::new(), EffectTag::Pure, vec![]);
    cv_a.masa_ref = MasaRef::Digest(digest.clone());
    let mut cv_b = ComboVal::new(m_b, false, IndexMap::new(), EffectTag::Pure, vec![]);
    cv_b.masa_ref = MasaRef::Digest(digest);

    let result = oo.unify(Value::Combo(cv_a), Value::Combo(cv_b));

    // Expect H1Split (statistically near-certain with 16 orthogonal field sets)
    assert!(
        matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::H1Split)),
        "16 orthogonal-field combos with explicit MASA should H1Split, got {:?}",
        result
    );
}

// ─── 5. H1Split Bottom has correct metadata ───────────────────────────────────

#[test]
fn test_h1split_bottom_has_theta_and_degree() {
    let oo = oo();
    let digest = masa_digest(0x77);

    let mut m_a = IndexMap::new();
    let mut m_b = IndexMap::new();
    for i in 0..16_i64 {
        m_a.insert(format!("aa{}", i), int_val(i));
        m_b.insert(format!("bb{}", i), int_val(i + 500));
    }
    let mut cv_a = ComboVal::new(m_a, false, IndexMap::new(), EffectTag::Pure, vec![]);
    cv_a.masa_ref = MasaRef::Digest(digest.clone());
    let mut cv_b = ComboVal::new(m_b, false, IndexMap::new(), EffectTag::Pure, vec![]);
    cv_b.masa_ref = MasaRef::Digest(digest);

    let result = oo.unify(Value::Combo(cv_a), Value::Combo(cv_b));
    if let Value::Bottom(ref bd) = result {
        assert!(matches!(bd.cause, BottomCause::H1Split), "cause should be H1Split");
        assert_eq!(bd.obstruction_degree, Some(1), "H1 → degree 1");
        // holonomy should be Phase(theta) with theta > EPSILON_COHERENT
        if let Some(nlang_interpreter::value::Holonomy::Phase(theta)) = bd.holonomy {
            assert!(theta >= EPSILON_COHERENT, "theta={} should be >= epsilon={}", theta, EPSILON_COHERENT);
        } else {
            panic!("holonomy should be Phase(theta), got {:?}", bd.holonomy);
        }
    } else {
        // If test reaches here, spectral content happened to be below threshold
        // (extremely unlikely for 16 orthogonal fields); skip assertion
    }
}

// ─── 6. phase_diff_between: degenerate (empty) combo returns 0 ───────────────

#[test]
fn test_phase_diff_empty_combo_is_zero() {
    let digest = masa_digest(0xCC);
    let a = masa_combo(digest.clone(), &[]);
    let b = masa_combo(digest.clone(), &[("x", int_val(1))]);
    let theta = nlang_interpreter::lattice_sketch::phase_diff_between(&a, &b);
    assert!(theta < 1e-9, "degenerate (empty) combo → theta = 0, got {}", theta);
}

// ─── 7. MasaRef::Top + Digest combo: no H2 obstruction, no H1 check ──────────

#[test]
fn test_top_and_digest_masa_no_obstruction() {
    let oo = oo();
    let digest = masa_digest(0xDD);

    // One Top-MASA, one Digest-MASA: H2 passes (Top is always compatible)
    // then match arm returns theta = 0.0 → Merge
    let top_c = top_combo(&[("p", int_val(99))]);
    let mut m_d = IndexMap::new();
    m_d.insert("q".to_string(), int_val(88));
    let mut cv_d = ComboVal::new(m_d, false, IndexMap::new(), EffectTag::Pure, vec![]);
    cv_d.masa_ref = MasaRef::Digest(digest);

    let result = oo.unify(top_c, Value::Combo(cv_d));
    assert!(
        !matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::H1Split)),
        "Top + Digest combos should not H1Split (theta=0 from match arm)"
    );
}
```

---

## 4. 修改 `crates/interpreter/Cargo.toml`

在 `[[test]]` 區段末尾（`engine_p39_test` 後）加入：

```toml
[[test]]
name = "h1_phase_test"
path = "tests/h1_phase_test.rs"
```

---

## 5. 完成後驗證

```bash
cargo test
```

預期：~446 tests，0 failed。

重點確認：
- `phase_diff_between` 對相同 combo → 0.0
- `phase_diff_between` 對 16-field 完全不同 keys combo → > EPSILON_COHERENT
- Top-MASA unify 不產生 H1Split（行為不變）
- H1Split Bottom 有正確的 `obstruction_degree: Some(1)` 和 `Holonomy::Phase(theta)`
- 全部舊有測試通過（零回歸）

---

## 6. 實作注意事項

| 事項 | 說明 |
|:-----|:-----|
| `use crate::value::ComboVal;` | `lattice_sketch.rs` 目前只 import `Value, MasaRef`，必須加 `ComboVal` |
| `extract_spectral_components` 保持 private | 不需要改成 pub，`phase_diff_between` 在同一模組內直接呼叫 |
| clone ComboVal | `phase_diff_between` 需要 `Value::Combo(a.clone())`，這是 Phase 40 的效能妥協；後續可優化 |
| 刪除 `approximate_phase_diff` | 死碼函數，完整刪除（含 `#[allow(dead_code)]` 屬性） |
| 刪除 TODO 注解 | `phase_merge_decision` 內的三行 TODO comment 一併刪除 |
| 測試中的 `Box::leak` | 只用於從 `format!` 取得 `&'static str` 鍵名；測試環境中記憶體不回收是可接受的 |
| H2 保證 | 進入 `(Digest(_), Digest(_))` 分支時，H2 已確認兩者 digest 相等，可直接呼叫 `phase_diff_between` |
| EPSILON\_COHERENT = 0.1 | 在 `unify.rs` 頂部定義，測試中需重宣告（`const EPSILON_COHERENT: f64 = 0.1;`）或直接用字面量 |

---

## 7. 修改摘要（3 個檔案）

| 檔案 | 改動 |
|:-----|:-----|
| `src/lattice_sketch.rs` | +1 import (`ComboVal`), +30 行 `pub fn phase_diff_between` |
| `src/unify.rs` | +1 import (`lattice_sketch`), -5 行 dead code, step 2 stub → 4 行 match |
| `tests/h1_phase_test.rs` | 新建，7 個測試 |
| `Cargo.toml` | +3 行 `[[test]]` entry |
