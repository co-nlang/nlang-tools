# Phase 9 Handover: `#blur` as First-Class `Value` Variant

**Date:** 2026-05-24  
**Status:** Ready for implementation  
**Depends on:** Phase 8 (complete)  
**Spec refs:** SPEC_08 §4.1–4.3, SPEC_01 §2.7

---

## 目標

將 `#blur` 從 `Value::Combo { %kind: "blur", … }` 的 ad-hoc 表示，升級為第一類 `Value::Blur(BlurDetail)` enum variant，並使其 CAID 可確定性計算（含 horizon 參數）、能進入 Commit 與 LADD discovery。

同時修正 `math.rs` 的 `blur_singularity()` 回傳 `Bottom(NumericalError)` 的錯誤——數學奇點在 Blur 策略下應回傳 `Value::Blur`，而非 Bottom。

---

## 現狀分析

### 問題一：`observation.rs` blur 是假的 Combo

`handle_resource_exhausted()` 在 `ObservationStrategy::Blur` 下回傳：

```rust
Value::Combo(ComboVal::new(
    vec![
        ("%kind", Tag("blur")),
        ("%state", Str(blur_hash)),
        ("%partial", partial_result),
    ], ...
))
```

這個 Combo 的 CAID 只依賴 content_hash（fields），不含 `fuel_remaining`、`strategy` 等 horizon 參數。違反 SPEC_08 §4.2：「blur CAID 必須包含完整 horizon 狀態快照」。

### 問題二：`math.rs` blur 是 Bottom

```rust
fn blur_singularity(cause_tag: &str) -> Value {
    Value::Bottom(Box::new(BottomDetail { cause: BottomCause::NumericalError, ... }))
}
```

數學奇點（`ln(0)`、`eml` 等）在 Blur 策略下不應是 Bottom，應是 Blur（可能有 partial 近似值）。

### 問題三：整個系統不認識 `Value::Blur`

`do_unify()`、`orthocomplement()`、`force()`、`bn_serial.rs`、`content_hash()` 都沒有 Blur arm。

---

## 新增型別（全在 `crates/interpreter/src/value.rs`）

### `BlurCause`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BlurCause {
    FuelExhausted,
    Timeout,
    StackOverflow,
    MathSingularity(String),  // e.g. "log_singularity", "eml_singularity"
}

impl BlurCause {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            BlurCause::FuelExhausted => b"fuel_exhausted",
            BlurCause::Timeout => b"timeout",
            BlurCause::StackOverflow => b"stack_overflow",
            BlurCause::MathSingularity(s) => s.as_bytes(),
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            BlurCause::FuelExhausted => "fuel_exhausted",
            BlurCause::Timeout => "timeout",
            BlurCause::StackOverflow => "stack_overflow",
            BlurCause::MathSingularity(s) => s.as_str(),
        }
    }
}
```

### `HorizonParams`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HorizonParams {
    pub fuel_remaining: u64,
    pub strategy: crate::observation::ObservationStrategy,  // 注意 cfg 循環
    pub salt: ContentHash,
}
```

**注意：** `ObservationStrategy` 在 `observation.rs` 中定義。為避免循環依賴，可以在 `value.rs` 中重新定義一個 `BlurStrategy` mirror enum，或將 `ObservationStrategy` 移到 `value.rs`。**推薦後者**：將 `ObservationStrategy` 及 `ObservationState` 從 `observation.rs` 移到 `value.rs`（或 `types.rs`），因為它們是 value-level 概念。`observation.rs` 只保留行為函數。

### `BlurDetail`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlurDetail {
    pub cause: BlurCause,
    pub horizon: HorizonParams,
    pub partial: Option<Box<Value>>,
    pub effect: EffectTag,
}

impl BlurDetail {
    pub fn blur_caid(&self) -> ContentHash {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(b"blur:");
        hasher.update(self.cause.as_bytes());
        hasher.update(b":fuel=");
        hasher.update(&self.horizon.fuel_remaining.to_le_bytes());
        hasher.update(b":strategy=");
        let strat_byte: u8 = match self.horizon.strategy {
            ObservationStrategy::Blur => 0,
            ObservationStrategy::Strict => 1,
            ObservationStrategy::Approximate => 2,
        };
        hasher.update(&[strat_byte]);
        hasher.update(b":salt=");
        hasher.update(&self.horizon.salt.digest);
        ContentHash::v1(hasher.finalize().to_vec())
    }
}
```

### `Value` enum 新增 Blur arm

```rust
pub enum Value {
    Top,
    Atom(AtomKind, EffectTag, Option<i64>),
    Combo(ComboVal),
    Union(Vec<Value>),
    Code(Box<Expr>),
    Thunk { expr: Box<Expr>, closure: Vec<ComboVal>, effect: EffectTag },
    Bottom(Box<BottomDetail>),
    Blur(BlurDetail),   // ← NEW
}
```

---

## 修改清單

### 1. `crates/interpreter/src/value.rs`

**1a. 新增三個型別**（如上）

**1b. 更新 `PartialEq for Value`**：
```rust
(Value::Blur(b1), Value::Blur(b2)) => b1 == b2,
```

**1c. 更新 `fn effect(&self) -> EffectTag`**：
```rust
Value::Blur(bd) => bd.effect,
```

**1d. 更新 `fn bits(&self) -> u64`**：
```rust
Value::Blur(bd) => {
    128 + bd.partial.as_ref().map(|p| p.bits()).unwrap_or(0)
}
```

**1e. 更新 `fn to_string_plain(&self)`**：
```rust
Value::Blur(bd) => format!("#blur({})", bd.cause.as_str()),
```

**1f. 更新 `fn to_nlang(&self, indent: usize)`**：
```rust
Value::Blur(bd) => {
    let caid = bd.blur_caid().to_string();
    format!("#blur {{ %cause: #{}, %caid: \"{}\" }}", bd.cause.as_str(), caid)
}
```

**1g. 更新 `fn content_hash(&self)`**：
```rust
Value::Blur(bd) => {
    let caid = bd.blur_caid();
    ContentHash {
        algorithm: HashAlgorithm::Sha256,
        version: CaidVersion::V2,
        masa_ref: MasaRef::Top,
        lattice_sketch: String::new(),   // Blur 沒有結構，sketch 留空
        digest: caid.digest.clone(),
    }
}
```

`content_hash()` 的現有 match arm 需要補 Blur case，並保留其他 arm 不變（改成 `_ =>` 最後一個 arm 目前是 `Value::Combo`，直接補即可）。

**1h. 更新 `hash_recursive_with_salt`**：
```rust
Value::Blur(bd) => {
    hasher.update([0xFD]);
    hasher.update(bd.cause.as_bytes());
    hasher.update(&bd.horizon.fuel_remaining.to_le_bytes());
    hasher.update(&bd.horizon.salt.digest);
}
```

---

### 2. `crates/interpreter/src/observation.rs`

**2a. 修改 `handle_resource_exhausted` 簽名**，新增 `fuel_remaining: u64` 參數：

```rust
pub fn handle_resource_exhausted(
    cause: crate::ResourceExhausted,
    strategy: ObservationStrategy,
    horizon_salt: &ContentHash,
    fuel_remaining: u64,          // ← NEW
    partial_result: Option<Value>,
    effect: EffectTag,
) -> Value {
```

**2b. 修改 Blur arm**，回傳 `Value::Blur`：

```rust
ObservationStrategy::Blur => {
    let blur_cause = match cause {
        crate::ResourceExhausted::FuelExhausted => BlurCause::FuelExhausted,
        crate::ResourceExhausted::Timeout       => BlurCause::Timeout,
        crate::ResourceExhausted::StackOverflow => BlurCause::StackOverflow,
    };
    Value::Blur(BlurDetail {
        cause: blur_cause,
        horizon: HorizonParams {
            fuel_remaining,
            strategy,
            salt: horizon_salt.clone(),
        },
        partial: partial_result.map(Box::new),
        effect,
    })
}
```

**2c. 刪除** `fn compute_blur_caid(...)` 整個函數——邏輯已移入 `BlurDetail::blur_caid()`。

**2d. 更新 imports**：加 `use crate::value::{BlurDetail, BlurCause, HorizonParams};`

---

### 3. 更新 `handle_resource_exhausted` 的所有呼叫點

共 5 處，全部補 `ctx.fuel` 參數（在 `horizon_salt` 之後）：

| 檔案 | 行 | 原始 call | 修改後 |
|------|----|-----------|--------|
| `src/lib.rs` | ~382 | `handle_resource_exhausted(e, ctx.strategy, &ctx.horizon_salt, None, accumulated_effect)` | `handle_resource_exhausted(e, ctx.strategy, &ctx.horizon_salt, ctx.fuel, None, accumulated_effect)` |
| `src/eval.rs` | ~96 | `handle_resource_exhausted(e, ctx.strategy, &ctx.horizon_salt, None, EffectTag::Pure)` | `handle_resource_exhausted(e, ctx.strategy, &ctx.horizon_salt, ctx.fuel, None, EffectTag::Pure)` |
| `src/eval.rs` | ~102 | same pattern | same fix |
| `src/eval.rs` | ~211 | same pattern | same fix |
| `src/unify.rs` | ~82 | same pattern | same fix |

---

### 4. `crates/interpreter/src/builtins/math.rs`

**4a. 修改 `blur_singularity` 簽名**：

```rust
fn blur_singularity(cause_tag: &str, ctx: &EvalContext) -> Value {
    Value::Blur(BlurDetail {
        cause: BlurCause::MathSingularity(cause_tag.trim_start_matches('#').to_string()),
        horizon: HorizonParams {
            fuel_remaining: ctx.fuel,
            strategy: ctx.strategy,
            salt: ctx.horizon_salt.clone(),
        },
        partial: None,
        effect: EffectTag::Pure,
    })
}
```

**4b. 更新所有 `blur_singularity(...)` 呼叫**，補 `ctx` 參數：

搜尋 `blur_singularity(` — 有 3 處：
- `ln` closure: `if f == 0.0 { return blur_singularity("#log_singularity", ctx); }`
- `ln(complex(0,0))`: `return blur_singularity("#log_singularity", ctx);`
- `compute_ln(...).unwrap_or_else(|| blur_singularity("#log_singularity", ctx))`
- `eml` closure: `_ => blur_singularity("#eml_singularity", ctx)`

**注意：** `blur_singularity` 是 `register_math_builtins` 內的 nested fn，closures 有 `ctx: &mut EvalContext`，所以傳 `ctx` 沒有問題（Rust 允許在 nested fn 中接受外部型別的引用只要 fn 本身是非閉包）。

**4c. 更新 imports**：
```rust
use crate::value::{Value, EffectTag, BottomCause, BlurDetail, BlurCause, HorizonParams};
use crate::EvalContext;  // 若原本沒有引入
```

---

### 5. `crates/interpreter/src/bn_serial.rs`

在 `fn serialize_value(val: &Value, buf: &mut Vec<u8>)` 新增 Blur arm（在 `Value::Bottom` 之後）：

```rust
Value::Blur(bd) => {
    buf.push(0xFD);
    // cause tag (length-prefixed)
    let cause_bytes = bd.cause.as_bytes();
    write_leb128_u64(buf, cause_bytes.len() as u64);
    buf.extend_from_slice(cause_bytes);
    // horizon params
    buf.extend_from_slice(&bd.horizon.fuel_remaining.to_le_bytes());
    let strat_byte: u8 = match bd.horizon.strategy {
        crate::ObservationStrategy::Blur => 0,
        crate::ObservationStrategy::Strict => 1,
        crate::ObservationStrategy::Approximate => 2,
    };
    buf.push(strat_byte);
    buf.extend_from_slice(&bd.horizon.salt.digest);
    // partial (optional)
    if let Some(partial) = &bd.partial {
        buf.push(0x01);
        serialize_value(partial, buf);
    } else {
        buf.push(0x00);
    }
}
```

若 `write_leb128_u64` 尚不存在，可先用簡單的 u32 length prefix。檢查現有 serialize_combo 中是否有 LEB128 helper。

---

### 6. `crates/interpreter/src/unify.rs`

在 `do_unify()` 中加入 Blur 處理規則（在 Bottom handling 之後）：

```rust
// Blur propagation rules
(Value::Blur(ba), Value::Blur(bb)) => {
    // Two blurs: merge partials if possible
    let merged_partial = match (ba.partial.as_deref(), bb.partial.as_deref()) {
        (Some(pa), Some(pb)) => {
            let unified = oo.unify_internal(pa.clone(), pb.clone(), ctx);
            if matches!(unified, Value::Bottom(_)) {
                Some(Box::new(Value::Union(vec![pa.clone(), pb.clone()])))
            } else {
                Some(Box::new(unified))
            }
        }
        (Some(p), None) | (None, Some(p)) => Some(Box::new(p.clone())),
        (None, None) => None,
    };
    // Use the blur with lower fuel_remaining (more constrained horizon)
    let base = if ba.horizon.fuel_remaining <= bb.horizon.fuel_remaining { ba } else { bb };
    Value::Blur(BlurDetail {
        cause: base.cause.clone(),
        horizon: base.horizon.clone(),
        partial: merged_partial,
        effect: ba.effect.max(bb.effect),
    })
}

(Value::Blur(_), Value::Bottom(_)) | (Value::Bottom(_), Value::Blur(_)) => {
    // Bottom dominates blur (Bottom is a stronger claim than uncertainty)
    if matches!(a, Value::Bottom(_)) { a } else { b }
}

(Value::Blur(_), Value::Top) => a,   // Blur ∧ Top = Blur (Top is meet-identity)
(Value::Top, Value::Blur(_)) => b,

(Value::Blur(bd), other) | (other, Value::Blur(bd)) => {
    // Blur meets concrete: keep blur, record concrete as partial hint
    let new_partial = match bd.partial.as_deref() {
        Some(existing) => {
            let unified = oo.unify_internal(existing.clone(), other.clone(), ctx);
            if matches!(unified, Value::Bottom(_)) {
                Some(Box::new(other.clone()))
            } else {
                Some(Box::new(unified))
            }
        }
        None => Some(Box::new(other.clone())),
    };
    Value::Blur(BlurDetail {
        partial: new_partial,
        ..bd.clone()
    })
}
```

**注意：** Rust 的 `match (a, b)` 語義——arm 順序要對，且 `(Value::Blur(bd), other) | (other, Value::Blur(bd))` 這樣的 or-pattern 在 Rust 中不直接支援（兩個 binding 位置不同）。需拆成兩個 arm 或使用 guard。建議拆開：

```rust
(Value::Blur(bd), other) => { /* bd 是 blur, other 是任何值 */ ... }
(other, Value::Blur(bd)) => { /* 同上，對稱 */ ... }
```

放在所有其他 arm 之後（作為 catch-all 前的 Blur 特例）。

---

### 7. `crates/interpreter/src/complement.rs`

在 `orthocomplement()` 中，在 `Value::Thunk` arm 之前加：

```rust
Value::Blur(bd) => {
    // Cannot compute complement of an unknown value; propagate blur
    Value::Blur(bd)
}
```

邏輯：Blur 的 orthocomplement 也是 Blur（不確定性的補也是不確定性）。這比回傳 Bottom 更合理——沒有資訊但沒有衝突。

---

### 8. `crates/interpreter/src/lib.rs`

**8a. 更新 `pub use` 清單**：
```rust
pub use crate::value::{
    Value, ComboVal, EffectTag, BottomDetail, BottomCause, ContentHash,
    MasaRef, ValRelation, Holonomy, RefineInfo, AuthorityInfo,
    BlurDetail, BlurCause, HorizonParams,   // ← NEW
};
```

**8b. 在 `force()` 中加入 Blur arm**（找到 force 函數，在 Thunk arm 之後或 Bottom arm 之後）：
```rust
Value::Blur(_) => v,  // Blur 是 terminal，不再 force
```

若 force 裡有 `_ => v` 的 catch-all，確認 Blur 被正確覆蓋（catch-all 已處理，但最好明示）。

**8c. 在 unify 相關 morphism 呼叫中**，如有 pattern matching on `Value` variants，補 Blur arm 或確認 `_ =>` catch-all 不會誤處理 Blur。

---

### 9. `crates/interpreter/src/observation.rs`（補充）

`ObservationState::Blur(ContentHash)` 已存在（line 11）。Phase 9 後，當一個 `Value::Blur` 被 observe 時：

```rust
ObservationState::Blur(v.content_hash())
```

`content_hash()` 對 `Value::Blur` 已在步驟 1g 定義為使用 `BlurDetail::blur_caid()`，所以 CAID 確定性已保證。

---

## 測試檔案：`tests/blur_test.rs`

```rust
// 需要的 use
use nlang_interpreter::{Ouroboros, EvalContext, ObservationStrategy};
use nlang_interpreter::{Value, BlurDetail, BlurCause, HorizonParams, EffectTag, ContentHash};

// Test 1: blur from fuel exhaustion has deterministic CAID
#[test]
fn blur_fuel_caid_deterministic() {
    let bd1 = BlurDetail { cause: BlurCause::FuelExhausted, horizon: HorizonParams { fuel_remaining: 42, strategy: ObservationStrategy::Blur, salt: ContentHash::v1(vec![0u8; 32]) }, partial: None, effect: EffectTag::Pure };
    let bd2 = bd1.clone();
    assert_eq!(bd1.blur_caid(), bd2.blur_caid());
}

// Test 2: different fuel → different CAID
#[test]
fn blur_different_fuel_different_caid() {
    let make = |fuel: u64| BlurDetail { cause: BlurCause::FuelExhausted, horizon: HorizonParams { fuel_remaining: fuel, strategy: ObservationStrategy::Blur, salt: ContentHash::v1(vec![0u8; 32]) }, partial: None, effect: EffectTag::Pure };
    assert_ne!(make(10).blur_caid(), make(20).blur_caid());
}

// Test 3: blur ∧ Top = Blur
#[test]
fn blur_unify_top_is_blur() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    let blur = Value::Blur(BlurDetail { cause: BlurCause::FuelExhausted, horizon: HorizonParams { fuel_remaining: 0, strategy: ObservationStrategy::Blur, salt: ctx.horizon_salt.clone() }, partial: None, effect: EffectTag::Pure });
    let result = oo.unify_internal(blur.clone(), Value::Top, &mut ctx);
    assert!(matches!(result, Value::Blur(_)));
}

// Test 4: blur ∧ Bottom = Bottom
#[test]
fn blur_unify_bottom_is_bottom() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    let blur = Value::Blur(BlurDetail { cause: BlurCause::FuelExhausted, horizon: HorizonParams { fuel_remaining: 0, strategy: ObservationStrategy::Blur, salt: ctx.horizon_salt.clone() }, partial: None, effect: EffectTag::Pure });
    let bottom = Value::Bottom(Box::new(Default::default()));
    let result = oo.unify_internal(blur, bottom, &mut ctx);
    assert!(matches!(result, Value::Bottom(_)));
}

// Test 5: blur ∧ concrete records partial
#[test]
fn blur_unify_concrete_records_partial() {
    use nlang_parser::ast::AtomKind;
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    let blur = Value::Blur(BlurDetail { cause: BlurCause::FuelExhausted, horizon: HorizonParams { fuel_remaining: 0, strategy: ObservationStrategy::Blur, salt: ctx.horizon_salt.clone() }, partial: None, effect: EffectTag::Pure });
    let concrete = Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None);
    let result = oo.unify_internal(blur, concrete, &mut ctx);
    if let Value::Blur(bd) = result {
        assert!(bd.partial.is_some());
    } else {
        panic!("expected Blur");
    }
}

// Test 6: math ln(0) returns Blur in Blur strategy
#[test]
fn math_ln_zero_returns_blur() {
    use nlang_parser::ast::AtomKind;
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default()).with_strategy(ObservationStrategy::Blur);
    let arg = Value::Atom(AtomKind::Float(0.0), EffectTag::Pure, None);
    let result = oo.apply_builtin("math.ln", arg, &mut ctx);
    assert!(matches!(result, Value::Blur(_)), "ln(0) should return Blur in Blur mode, got {:?}", result);
}

// Test 7: math ln(0) returns Bottom in Strict strategy
#[test]
fn math_ln_zero_strict_returns_bottom() {
    use nlang_parser::ast::AtomKind;
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default()).with_strategy(ObservationStrategy::Strict);
    let arg = Value::Atom(AtomKind::Float(0.0), EffectTag::Pure, None);
    let result = oo.apply_builtin("math.ln", arg, &mut ctx);
    // In strict mode, still should be bottom or blur? 
    // math singularity always produces Blur for now—strict applies only to resource exhaustion
    // So this test verifies the current design choice: math singularity → Blur always
    assert!(matches!(result, Value::Blur(_)));
}

// Test 8: handle_resource_exhausted → Value::Blur
#[test]
fn handle_resource_exhausted_returns_blur() {
    use nlang_interpreter::{ResourceExhausted, handle_resource_exhausted};
    let salt = ContentHash::v1(vec![0u8; 32]);
    let result = handle_resource_exhausted(
        ResourceExhausted::FuelExhausted,
        ObservationStrategy::Blur,
        &salt,
        77,   // fuel_remaining
        None,
        EffectTag::Pure,
    );
    assert!(matches!(result, Value::Blur(_)));
    if let Value::Blur(bd) = result {
        assert_eq!(bd.horizon.fuel_remaining, 77);
        assert!(matches!(bd.cause, BlurCause::FuelExhausted));
    }
}

// Test 9: blur complement is blur
#[test]
fn blur_complement_is_blur() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(Default::default());
    let blur = Value::Blur(BlurDetail { cause: BlurCause::Timeout, horizon: HorizonParams { fuel_remaining: 0, strategy: ObservationStrategy::Blur, salt: ctx.horizon_salt.clone() }, partial: None, effect: EffectTag::Pure });
    let result = oo.orthocomplement(blur, &mut ctx);
    assert!(matches!(result, Value::Blur(_)));
}

// Test 10: blur content_hash is deterministic and includes horizon
#[test]
fn blur_content_hash_deterministic() {
    let salt = ContentHash::v1(vec![1u8; 32]);
    let bd = BlurDetail { cause: BlurCause::MathSingularity("log_singularity".to_string()), horizon: HorizonParams { fuel_remaining: 100, strategy: ObservationStrategy::Blur, salt: salt.clone() }, partial: None, effect: EffectTag::Pure };
    let v = Value::Blur(bd);
    let h1 = v.content_hash();
    let h2 = v.content_hash();
    assert_eq!(h1, h2);
}

// Test 11: blur BN/ serialization roundtrip (if deserialize is implemented)
// For now, just verify serialization produces non-empty deterministic bytes
#[test]
fn blur_bn_serial_deterministic() {
    use nlang_interpreter::bn_serial::serialize_bn;
    let salt = ContentHash::v1(vec![0u8; 32]);
    let bd = BlurDetail { cause: BlurCause::FuelExhausted, horizon: HorizonParams { fuel_remaining: 0, strategy: ObservationStrategy::Blur, salt }, partial: None, effect: EffectTag::Pure };
    let v = Value::Blur(bd);
    let b1 = serialize_bn(&v);
    let b2 = serialize_bn(&v);
    assert!(!b1.is_empty());
    assert_eq!(b1, b2);
    assert_eq!(b1[0], 0xFD);
}
```

---

## 設計決策說明

### 為何 math singularity 也用 Blur 而非 Bottom？

數學奇點（如 `ln(0)`）在 Blur 策略下代表「計算無法完成，但不是矛盾」。Bottom 代表邏輯矛盾。Blur 代表「horizon 之外，超出可觀察範圍」。`ln(0) = -∞` 超出 floating point horizon，適合 Blur。

在 Strict 策略下，math singularity 目前設計也回傳 Blur（而非 Bottom），因為奇點是 horizon 問題，不是 type error。可在後續 Phase 加入 `ObservationStrategy::Strict` 對 math 的精確語義。

### 為何 Blur ∧ Bottom = Bottom？

Bottom 是格的底元素，代表「已知矛盾」。Blur 代表「未知」。已知矛盾比未知更強——如果已經發現矛盾，增加「不確定性」不改變矛盾。

### 為何 Blur ∧ Concrete = Blur(partial=Concrete)？

在 Blur 策略下，Blur 是「超出 horizon 的值」。與一個具體值 meet，保留具體值為 partial hint，但不能確認為精確值（因為 horizon 之外可能有更多約束）。

### 為何 Blur 的 orthocomplement 是 Blur？

`!Blur = Blur`：不知道一個值，也不知道它的補。這保持了 Blur 的可傳播性（complement of unknown is unknown）。替代方案是 Bottom，但 Bottom 會污染下游計算；Blur 允許繼續傳播不確定性。

---

## 不在本 Phase 的工作

- **`bootstrap_exempt → Epoch`**：仍延後
- **Approximate 策略的 Blur 行為**：`ObservationStrategy::Approximate` 目前仍回傳 `Tag("approximate")`，Phase 9 不改
- **Blur 在 LADD/disc 中的廣播**：Blur 值的 CAID 已確定，但 disc.advertise 是否廣播 Blur 值待 Phase 10 決定
- **BN/ deserialize for Blur**：只做 serialize，deserialize 待後續
- **Blur 在 `#refine` 中的角色**：Blur source CAID 是否可作為 refine source 待討論

---

## 驗收條件

1. `cargo test -p nlang-interpreter 2>&1 | grep -E "FAILED|passed"` — 現有 133 測試仍全過
2. `cargo test -p nlang-interpreter blur 2>&1` — 11 個新測試全過
3. `Value::Blur` 不再出現於 `complement.rs`、`unify.rs` 的 `_ =>` catch-all（有明確 arm）
4. `handle_resource_exhausted` 在 Blur 模式下不再回傳 `Value::Combo`
5. `blur_singularity` 在 math.rs 不再回傳 `Value::Bottom`
6. `serialize_bn(Value::Blur(...))` 第一個 byte 是 `0xFD`
