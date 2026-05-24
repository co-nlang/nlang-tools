# Phase 13 Handover: Lattice Sketch 跨架構向量 + 創世預設值 + `%branch` 元字段

**Date:** 2026-05-24  
**Status:** Ready for implementation  
**Depends on:** Phase 12 (complete)  
**Spec refs:** SPEC_09 §3.3, §6, APP_05 §3.5

---

## 目標

完成三個 SPEC_09 標準庫完整性項目：

1. **lattice_sketch v2 跨架構測試向量** — 為確定性 CAID 保證提供硬編碼期望值，讓引擎能在不同平台/架構上驗證 sketch 輸出一致
2. **創世預設值 (`~%Config`)** — 在 `root_with_system()` 中暴露視界參數（`%fuel`, `%max_branches`, `%timeout` 等）為可讀的 n/lang 值（SPEC_09 §6）
3. **`%branch` 元字段** — `math.ln` 和 `math.sqrt` 支援 Riemann 面層級宣告，讓多值函數有確定性分支選擇（SPEC_09 §3.3）

---

## 現狀

### 問題一：lattice_sketch 測試只驗 non-empty，無確定向量

`tests/lattice_sketch_v2_test.rs`，`test_sketch_known_vector`：
```rust
assert!(!sketch.is_empty(), "sketch should not be empty");
assert!(sketch.len() > 10, "sketch should be at least 10 chars");
```
沒有硬編碼期望 Base64 字串。若演算法被不慎改動，無法立刻偵測。

### 問題二：創世預設值未暴露

`EvalContext::new()` 的預設值（`fuel: 10000`, `max_branches: 64`, ...）是 Rust 常數，n/lang 代碼無法觀測。SPEC_09 §6 要求這些視界參數在根宇宙中可讀。

### 問題三：`math.ln`/`math.sqrt` 不接受 `%branch`

SPEC_09 §3.3：`ln(-1)` 應回傳主分支 `iπ`（已做），但：
- `ln(-1) { %branch: 1 }` 應回傳 `iπ + 2πi = 3πi`（第一層 Riemann 面）
- 目前完全忽略 `%branch`

---

## 任務一：lattice_sketch v2 硬編碼測試向量

### 1a. 計算已知輸入的期望 sketch

執行以下命令取得真實向量：

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools
cargo test -p nlang-interpreter print_known_vectors -- --nocapture
```

在 `tests/lattice_sketch_v2_test.rs` 末端臨時加一個列印測試：

```rust
#[test]
fn print_known_vectors() {
    use nlang_interpreter::lattice_sketch::compute_sketch_v2;
    use nlang_interpreter::value::{Value, ComboVal, EffectTag};
    use nlang_parser::ast::AtomKind;
    use indexmap::IndexMap;

    // Vector 1: Top
    println!("TOP: {}", compute_sketch_v2(&Value::Top));

    // Vector 2: single Int atom
    let atom = Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None);
    println!("ATOM_42: {}", compute_sketch_v2(&atom));

    // Vector 3: single-field Combo {x: 1}
    let mut d = IndexMap::new();
    d.insert("x".to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None));
    let c1 = Value::Combo(ComboVal::new(d, false, IndexMap::new(), EffectTag::Pure, vec![]));
    println!("COMBO_X1: {}", compute_sketch_v2(&c1));

    // Vector 4: two-field Combo {x: 1, y: 2}
    let mut d2 = IndexMap::new();
    d2.insert("x".to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None));
    d2.insert("y".to_string(), Value::Atom(AtomKind::Int(2.into()), EffectTag::Pure, None));
    let c2 = Value::Combo(ComboVal::new(d2, false, IndexMap::new(), EffectTag::Pure, vec![]));
    println!("COMBO_XY: {}", compute_sketch_v2(&c2));

    // Vector 5: String atom
    let s = Value::Atom(AtomKind::Str("hello".to_string()), EffectTag::Pure, None);
    println!("STR_HELLO: {}", compute_sketch_v2(&s));
}
```

### 1b. 將列印結果填入常數，新增確定性測試

執行後取得 5 行輸出，填入常數並新增以下測試（刪除 `print_known_vectors`）：

```rust
// Cross-arch test vectors (generated on x86_64, must match on all platforms)
const EXPECTED_SKETCH_TOP:      &str = "<填入 TOP 輸出>";
const EXPECTED_SKETCH_ATOM_42:  &str = "<填入 ATOM_42 輸出>";
const EXPECTED_SKETCH_COMBO_X1: &str = "<填入 COMBO_X1 輸出>";
const EXPECTED_SKETCH_COMBO_XY: &str = "<填入 COMBO_XY 輸出>";
const EXPECTED_SKETCH_STR_HELLO:&str = "<填入 STR_HELLO 輸出>";

#[test]
fn test_sketch_cross_arch_top() {
    assert_eq!(compute_sketch_v2(&Value::Top), EXPECTED_SKETCH_TOP,
        "Top sketch must be identical across architectures");
}

#[test]
fn test_sketch_cross_arch_atom_int() {
    let v = Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None);
    assert_eq!(compute_sketch_v2(&v), EXPECTED_SKETCH_ATOM_42);
}

#[test]
fn test_sketch_cross_arch_combo_one_field() {
    let mut d = IndexMap::new();
    d.insert("x".to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None));
    let v = Value::Combo(ComboVal::new(d, false, IndexMap::new(), EffectTag::Pure, vec![]));
    assert_eq!(compute_sketch_v2(&v), EXPECTED_SKETCH_COMBO_X1);
}

#[test]
fn test_sketch_cross_arch_combo_two_fields() {
    let mut d = IndexMap::new();
    d.insert("x".to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None));
    d.insert("y".to_string(), Value::Atom(AtomKind::Int(2.into()), EffectTag::Pure, None));
    let v = Value::Combo(ComboVal::new(d, false, IndexMap::new(), EffectTag::Pure, vec![]));
    assert_eq!(compute_sketch_v2(&v), EXPECTED_SKETCH_COMBO_XY);
}

#[test]
fn test_sketch_cross_arch_str() {
    let v = Value::Atom(AtomKind::Str("hello".to_string()), EffectTag::Pure, None);
    assert_eq!(compute_sketch_v2(&v), EXPECTED_SKETCH_STR_HELLO);
}
```

**同時**，將現有 `test_sketch_known_vector` 改為真實驗證：
```rust
#[test]
fn test_sketch_known_vector() {
    let mut d = IndexMap::new();
    d.insert("x".to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None));
    let v = combo_with_data(d);
    // This must match EXPECTED_SKETCH_COMBO_X1
    assert_eq!(compute_sketch_v2(&v), EXPECTED_SKETCH_COMBO_X1,
        "known vector must be stable; if changed, update all EXPECTED_SKETCH_* constants");
}
```

---

## 任務二：創世預設值 (`~%Config`)

### 2a. `crates/interpreter/src/lib.rs`，`root_with_system()` 末端加入

在 `~%Official` 之前（或最後）插入：

```rust
// ~%Config: genesis defaults (SPEC_09 §6)
let mut config_fields = IndexMap::new();
config_fields.insert(
    "%fuel".to_string(),
    Value::Atom(AtomKind::Int(10000i64.into()), EffectTag::Pure, None),
);
config_fields.insert(
    "%max_branches".to_string(),
    Value::Atom(AtomKind::Int(64i64.into()), EffectTag::Pure, None),
);
config_fields.insert(
    "%max_depth".to_string(),
    Value::Atom(AtomKind::Int(256i64.into()), EffectTag::Pure, None),
);
config_fields.insert(
    "%max_pattern_nodes".to_string(),
    Value::Atom(AtomKind::Int(1024i64.into()), EffectTag::Pure, None),
);
config_fields.insert(
    "%timeout".to_string(),
    Value::Atom(AtomKind::Int(1000i64.into()), EffectTag::Pure, None),
);
config_fields.insert(
    "%strategy".to_string(),
    Value::Atom(AtomKind::Tag("blur".to_string()), EffectTag::Pure, None),
);
fields.insert(
    "~%Config".to_string(),
    Value::Combo(ComboVal::new(config_fields, true, IndexMap::new(), EffectTag::Pure, vec![])),
);
```

**語義說明：**
- `~%Config` 是 Cocoon（closed）——只讀，無法從 n/lang 演化覆寫
- 值是靜態 genesis 預設值，不反映 `ctx.fuel` 的動態剩餘量
- 使用者可用 `~%Config.%fuel` 讀取系統初始視界參數

### 2b. `genesis.rs` 新增 `SEED_CONFIG`

```rust
pub const SEED_CONFIG: &str = "PLACEHOLDER";  // 執行 seed_caids_are_stable -- --nocapture 取得
```

在 `all_seeds()` 加：
```rust
("~%Config", SEED_CONFIG),
```

取得真實 hash 後填入（同 Phase 12 的做法）。

### 2c. 測試

```rust
// 新增至 crates/interpreter/tests/genesis_test.rs
#[test]
fn config_in_root_with_system() {
    let oo = Ouroboros::new_in_memory();
    let root = oo.root_with_system();
    let config = root.get_field("~%Config").expect("~%Config should exist");
    if let Value::Combo(cv) = config {
        let fuel = cv.get_field("%fuel").expect("%fuel should exist");
        assert!(matches!(fuel, Value::Atom(AtomKind::Int(n), _, _) if n == &42u64.into() || true),
            "%fuel should be an Int");
        // Verify default value
        if let Value::Atom(AtomKind::Int(n), _, _) = fuel {
            assert_eq!(n.to_u64().unwrap_or(0), 10000, "%fuel default should be 10000");
        }
    } else {
        panic!("~%Config should be a Combo");
    }
}
```

---

## 任務三：`%branch` 元字段

### 3a. `math.rs`：修改 `math.ln` 支援 `%branch`

**設計：** 呼叫者可在傳入值的 Combo 中加 `%branch: n`（整數），選擇 Riemann 面層級：
- `%branch: 0`（預設）→ 主分支（Principal Branch），`arg ∈ (-π, π]`
- `%branch: n` → 主分支結果 + `2πni`（第 n 層 Riemann 面）

**`math.ln` closure 修改：**

```rust
m.insert("math.ln".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    // Extract %branch before forcing (branch is a meta field on the call)
    let branch: i64 = if let Value::Combo(ref c) = arg {
        c.get_field("%branch")
            .and_then(|v| if let Value::Atom(AtomKind::Int(n), _, _) = v { n.to_i64() } else { None })
            .unwrap_or(0)
    } else { 0 };

    let v = oo.force(arg, ctx).collapse();
    let effect = v.effect();

    // ln(0) is singular
    if let Value::Atom(AtomKind::Float(f), _, _) = &v {
        if *f == 0.0 { return blur_singularity("#log_singularity", ctx); }
    }
    if let Value::Atom(AtomKind::Complex(r, i), _, _) = &v {
        if *r == 0.0 && *i == 0.0 { return blur_singularity("#log_singularity", ctx); }
    }

    let base_result = compute_ln(&v).unwrap_or(blur_singularity("#log_singularity", ctx));

    // Apply Riemann surface offset: result + 2πni
    if branch != 0 {
        let offset_imag = 2.0 * std::f64::consts::PI * (branch as f64);
        match base_result {
            Value::Atom(AtomKind::Complex(r, i), e, rank) =>
                Value::Atom(AtomKind::Complex(r, i + offset_imag), e, rank),
            Value::Atom(AtomKind::Float(r), e, rank) =>
                // real result + imaginary offset → becomes complex
                Value::Atom(AtomKind::Complex(r, offset_imag), e, rank),
            other => other,  // blur/bottom propagate unchanged
        }
    } else {
        base_result
    }
}) as Arc<BuiltinFn>);
```

### 3b. `math.sqrt` 支援 `%branch`

數學上：`sqrt(x)` 有兩個分支（正/負）。`%branch: 0` → 主分支（正數平方根）。`%branch: 1` → 負數平方根（乘以 -1）。

```rust
m.insert("math.sqrt".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let branch: i64 = if let Value::Combo(ref c) = arg {
        c.get_field("%branch")
            .and_then(|v| if let Value::Atom(AtomKind::Int(n), _, _) = v { n.to_i64() } else { None })
            .unwrap_or(0)
    } else { 0 };

    let v = oo.force(arg, ctx).collapse();
    let effect = v.effect();
    let result = match &v {
        Value::Atom(AtomKind::Int(n), e, _) => {
            let f = n.to_f64().unwrap_or(0.0);
            if f < 0.0 {
                Value::Atom(AtomKind::Complex(0.0, f.abs().sqrt()), *e, None)
            } else {
                Value::Atom(AtomKind::Float(f.sqrt()), *e, None)
            }
        }
        Value::Atom(AtomKind::Float(f), e, _) => {
            if *f < 0.0 {
                Value::Atom(AtomKind::Complex(0.0, f.abs().sqrt()), *e, None)
            } else {
                Value::Atom(AtomKind::Float(f.sqrt()), *e, None)
            }
        }
        Value::Atom(AtomKind::Complex(r, i), e, _) => {
            let mag = (r * r + i * i).sqrt();
            let new_r = ((mag + r) / 2.0).sqrt();
            let new_i = if *i >= 0.0 { ((mag - r) / 2.0).sqrt() } else { -((mag - r) / 2.0).sqrt() };
            Value::Atom(AtomKind::Complex(new_r, new_i), *e, None)
        }
        _ => BottomCause::Conflict.into(),
    };
    // %branch: 1 → negate (second branch)
    if branch == 1 {
        match result {
            Value::Atom(AtomKind::Float(f), e, r) => Value::Atom(AtomKind::Float(-f), e, r),
            Value::Atom(AtomKind::Complex(re, im), e, r) => Value::Atom(AtomKind::Complex(-re, -im), e, r),
            other => other,
        }
    } else {
        result
    }
}) as Arc<BuiltinFn>);
```

**注意：** 若 `math.sqrt` 目前尚未在 `register_math_builtins` 中獨立存在（只靠 `eml` 派生），則先確認其是否有 `m.insert("math.sqrt", ...)` — 從 `root_with_system()` 的 `math_morphisms` 看到有 `"/sqrt", "math.sqrt"`，因此 builtin 應已存在，只需修改其 closure。

### 3c. 測試

```rust
// 新增至 crates/interpreter/tests/integration_tests.rs 或 special_float_test.rs

#[test]
fn ln_principal_branch_neg_one() {
    use nlang_parser::ast::AtomKind;
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    let arg = Value::Atom(AtomKind::Float(-1.0), EffectTag::Pure, None);
    let result = oo.apply_builtin("math.ln", arg, &mut ctx);
    // ln(-1) = iπ on principal branch
    if let Value::Atom(AtomKind::Complex(r, i), _, _) = result {
        assert!((r).abs() < 1e-10, "real part should be ~0");
        assert!((i - std::f64::consts::PI).abs() < 1e-10, "imag part should be π");
    } else {
        panic!("ln(-1) should return Complex, got {:?}", result);
    }
}

#[test]
fn ln_branch_1_neg_one() {
    use nlang_parser::ast::AtomKind;
    use nlang_interpreter::value::ComboVal;
    use indexmap::IndexMap;
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    // ln(-1) { %branch: 1 } = iπ + 2πi = 3πi
    let mut fields = IndexMap::new();
    fields.insert("0".to_string(), Value::Atom(AtomKind::Float(-1.0), EffectTag::Pure, None));
    fields.insert("%branch".to_string(), Value::Atom(AtomKind::Int(1i64.into()), EffectTag::Pure, None));
    let arg = Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
    let result = oo.apply_builtin("math.ln", arg, &mut ctx);
    if let Value::Atom(AtomKind::Complex(r, i), _, _) = result {
        assert!((r).abs() < 1e-10, "real part should be ~0");
        let expected_i = 3.0 * std::f64::consts::PI;
        assert!((i - expected_i).abs() < 1e-10, "imag should be 3π, got {}", i);
    } else {
        panic!("ln(-1) {{%branch: 1}} should return Complex, got {:?}", result);
    }
}

#[test]
fn sqrt_branch_0_is_positive() {
    use nlang_parser::ast::AtomKind;
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    let arg = Value::Atom(AtomKind::Float(4.0), EffectTag::Pure, None);
    let result = oo.apply_builtin("math.sqrt", arg, &mut ctx);
    if let Value::Atom(AtomKind::Float(f), _, _) = result {
        assert!((f - 2.0).abs() < 1e-10, "sqrt(4) branch 0 = 2.0");
    } else {
        panic!("expected Float, got {:?}", result);
    }
}

#[test]
fn sqrt_branch_1_is_negative() {
    use nlang_parser::ast::AtomKind;
    use nlang_interpreter::value::ComboVal;
    use indexmap::IndexMap;
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    let mut fields = IndexMap::new();
    fields.insert("0".to_string(), Value::Atom(AtomKind::Float(4.0), EffectTag::Pure, None));
    fields.insert("%branch".to_string(), Value::Atom(AtomKind::Int(1i64.into()), EffectTag::Pure, None));
    let arg = Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
    let result = oo.apply_builtin("math.sqrt", arg, &mut ctx);
    if let Value::Atom(AtomKind::Float(f), _, _) = result {
        assert!((f + 2.0).abs() < 1e-10, "sqrt(4) branch 1 = -2.0");
    } else {
        panic!("expected Float(-2.0), got {:?}", result);
    }
}
```

**注意 `%branch` 的 call convention：** 上面 `math.ln` 呼叫的 `arg` 是完整的 Combo（含 `%branch` 和 `"0"` 作為位置參數）。需確認 `math.ln` 現有邏輯如何從 Combo 取出數值參數——目前 `math.ln` 直接 force arg 整體，對 Combo arg 可能已有位置展開。

**請先確認現有 `math.ln` 如何處理 Combo arg（查看 `math.rs` 的 ln closure），再決定 `%branch` 的提取方式是在 force 之前還是之後。** 若 `ln` 直接 force 整體 arg，Combo `{ "0": -1.0, "%branch": 1 }` 會被 force 成 Combo（非數字），需在 force 之前先拆解。

---

## 實作順序建議

1. **先做任務一（cross-arch vectors）** — 純測試工作，不涉及邏輯修改，最安全
2. **再做任務二（創世預設值）** — 只是 `root_with_system()` 新增欄位，零風險
3. **最後做任務三（`%branch`）** — 改動 builtin 邏輯，需要對 ln/sqrt closure 的 call convention 有把握

---

## 驗收條件

1. `cargo test --workspace 2>&1 | grep FAILED` — 零失敗
2. 5 個 `test_sketch_cross_arch_*` 全部是 `assert_eq!`（非 `is_empty()`），且常數非空字串
3. `oo.root_with_system().get_field("~%Config")` — 存在，`%fuel` = 10000，`%max_branches` = 64
4. `SEED_CONFIG` 為真實 hash（非 PLACEHOLDER）
5. `ln(-1)` → `Complex(0, π)`；`ln(-1) { %branch: 1 }` → `Complex(0, 3π)`
6. `sqrt(4)` → `2.0`；`sqrt(4) { %branch: 1 }` → `-2.0`

---

## 不在本 Phase 的工作

- **`%branch` 對 eml 的傳播** — `eml(x, y)` 含 ln(y) 的分支，邏輯更複雜，留 Phase 14+
- **`~%Config` 動態讀取 ctx.fuel** — 目前是靜態預設值；動態版需 `engine.observe` 路徑支援，留後
- **`nerve_structure` overlapping_masa_caids** — P3，留後
- **自我演化（SPEC_17）** — P3，長期目標
