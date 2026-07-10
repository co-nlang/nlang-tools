# Phase 7 交接文件：正交模律驗證與非分配性偵測 (SPEC_01 §2.5.1)

## 目標

1. **非分配性事件追蹤**：在 `unify.rs` 的 Union 分支計算中，當 H¹/H² obstruction
   導致分支被過濾時記錄非分配性事件（`EvalContext::had_nondistrib_event`）。
2. **正交模律驗證**：新增 `src/oml.rs`，實作 `verify_oml(A, B)` — 對於 `A ⊑ B`，
   驗證 `B = A ∨ (B ∧ !A)` 是否成立（SPEC_01 §2.5.1 正交模律）。
3. **對外暴露**：`~%Engine./check_oml` 態射供 n/ 程式呼叫。

**依賴前置**：Phase NEW ✅（H¹/H² BottomDetail + `obstruction_degree`）、Phase 6 ✅（`orthocomplement`、`~%Engine`）

---

## 規格對應

| 實作項目 | 規格章節 |
|:---------|:---------|
| 正交模律 `B = A ∨ (B ∧ !A)` | SPEC_01 §2.5.1 |
| 非分配性（H¹/H² 障礙等級區分） | SPEC_01 §2.5.1、SPEC_06 §1.3 |
| `%obstruction_degree` 讀取 | Phase NEW（已實作） |

---

## 1. 修改 `EvalContext`（`src/lib.rs`）

### 1.1 新增欄位

```rust
pub struct EvalContext {
    // ...（現有欄位）
    pub refine_map_active: bool,
    pub had_nondistrib_event: bool,   // 新增：本次求值是否遭遇非分配性事件
}
```

### 1.2 修改初始化

```rust
Self { 
    // ...（現有欄位）
    refine_map_active: false,
    had_nondistrib_event: false,   // 新增
}
```

---

## 2. 修改 `src/unify.rs`：非分配性事件追蹤

### 2.1 修改 `do_unify` 中的 Union 分支

找到 `do_unify` 的 `(Value::Union(mut branches), other) | (other, Value::Union(mut branches))` 分支，
替換 filter 邏輯：

**現行：**
```rust
let results: Vec<Value> = branches.into_iter().map(|branch| {
    self.unify_internal(branch, other.clone(), ctx)
}).filter(|v| !matches!(v, Value::Bottom(_))).take(max_branches).collect();
```

**Phase 7 替換為：**
```rust
let mut results: Vec<Value> = Vec::new();
for branch in branches.into_iter().take(max_branches * 2) {
    let r = self.unify_internal(branch, other.clone(), ctx);
    match &r {
        Value::Bottom(detail) => {
            // H¹/H² obstruction 被過濾 → 非分配性事件
            if matches!(detail.cause, BottomCause::H1Split | BottomCause::H2Split) {
                ctx.had_nondistrib_event = true;
            }
            // Bottom 不加入結果（現有行為不變）
        }
        _ => {
            results.push(r);
            if results.len() >= max_branches { break; }
        }
    }
}
```

**說明**：
- H¹/H² Bottom 被過濾時，表示 `A ∧ (B₁ | B₂)` 的某個分支不可合併
- 這恰好是分配律 `A ∧ (B₁ | B₂) ≠ (A ∧ B₁) | (A ∧ B₂)` 的工程表現
- `ctx.had_nondistrib_event` 提供給上層呼叫者（如 `engine.check_oml`）使用
- 現有語義完全不變（Bottom 仍然被過濾，結果仍為非 Bottom 分支的 Union）

---

## 3. 新增 `src/oml.rs`

```rust
use crate::{Ouroboros, EvalContext};
use crate::value::Value;

/// OML 驗證結果
#[derive(Debug, Clone)]
pub enum OMLResult {
    /// A ⊄ B — OML 無意義（空洞真）
    Vacuous,
    /// A ⊑ B 且 B = A ∨ (B ∧ !A) — OML 成立
    Valid,
    /// A ⊑ B 但 B ≠ A ∨ (B ∧ !A) — 偵測到真正的 OML 違反
    Violation { rhs: Value, expected: Value },
    /// A ⊑ B 但引擎無法精確比對（Combo 值的譜近似限制）
    Approximate,
}

/// 驗證 A ⊑ B（A 是 B 的子空間）
/// 使用 unify(A, B) 的 content_hash == A.content_hash() 作為代理
pub fn verify_subspace(a: &Value, b: &Value, oo: &Ouroboros, ctx: &mut EvalContext) -> bool {
    let a_and_b = oo.unify_internal(a.clone(), b.clone(), ctx);
    a_and_b.content_hash().digest == a.content_hash().digest
}

/// 正交模律驗證：對於 A ⊑ B，驗證 B = A ∨ (B ∧ !A)
///
/// 演算法：
///   1. 若 A ⊄ B → Vacuous
///   2. 計算 !A = orthocomplement(A)
///   3. 計算 B ∧ !A = unify(B, !A)
///   4. 計算 RHS = A ∨ (B ∧ !A)（以 content_hash 比較）
///   5. 若 RHS.digest == B.digest → Valid；否則 Violation 或 Approximate
pub fn verify_oml(a: Value, b: Value, oo: &Ouroboros, ctx: &mut EvalContext) -> OMLResult {
    // Step 1: A ⊑ B?
    if !verify_subspace(&a, &b, oo, ctx) {
        return OMLResult::Vacuous;
    }

    // Step 2: !A
    let not_a = oo.orthocomplement(a.clone(), ctx);
    if let Value::Bottom(_) = not_a {
        return OMLResult::Approximate; // 無法計算補集
    }

    // Step 3: B ∧ !A
    let b_meet_not_a = oo.unify_internal(b.clone(), not_a, ctx);

    // Step 4: RHS = A | (B ∧ !A)
    // 在 n/ 中，格論 join 以 Union 代理；若兩者不相交則 Union 即是 join
    let rhs = match b_meet_not_a {
        Value::Bottom(_) => {
            // B ∧ !A = ⊥ → RHS = A ∨ ⊥ = A
            a.clone()
        }
        ref bna => {
            // RHS = A ∨ (B ∧ !A) — 用 Union 代理格論 join
            join_values(a.clone(), bna.clone())
        }
    };

    // Step 5: RHS == B?
    let rhs_digest = rhs.content_hash().digest;
    let b_digest = b.content_hash().digest;

    if rhs_digest == b_digest {
        OMLResult::Valid
    } else {
        // 區分：是真正違反還是近似誤差
        match (&a, &b) {
            // 對於簡單 Atom/Tag 值，比對是精確的
            (Value::Atom(_, _, _), Value::Atom(_, _, _))
            | (Value::Atom(_, _, _), Value::Union(_))
            | (Value::Union(_), _) => {
                OMLResult::Violation { rhs, expected: b }
            }
            // 對於 Combo 值，譜指紋近似可能導致假陽性
            _ => OMLResult::Approximate,
        }
    }
}

/// 格論 Join 的工程代理：A ∨ B = Union(A, B)（或若其一為 Top 則返回 Top）
fn join_values(a: Value, b: Value) -> Value {
    match (&a, &b) {
        (Value::Top, _) | (_, Value::Top) => Value::Top,
        (Value::Bottom(_), _) => b,
        (_, Value::Bottom(_)) => a,
        _ => Value::Union(vec![a, b]),
    }
}
```

**在 `src/lib.rs` 頂部加入：**
```rust
pub mod oml;
```

---

## 4. 修改 `src/builtins/engine.rs`：新增 `engine.check_oml`

```rust
m.insert("engine.check_oml".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let (a, b) = if let Value::Combo(ref c) = arg {
        let a = c.get_field("a").cloned().unwrap_or(Value::Top);
        let b = c.get_field("b").cloned().unwrap_or(Value::Top);
        (oo.force(a, ctx), oo.force(b, ctx))
    } else {
        return BottomCause::Conflict.into();
    };

    let result = crate::oml::verify_oml(a, b, oo, ctx);

    match result {
        crate::oml::OMLResult::Vacuous => {
            Value::Atom(AtomKind::Tag("oml_vacuous".to_string()), EffectTag::Pure, None)
        }
        crate::oml::OMLResult::Valid => {
            Value::Atom(AtomKind::Tag("oml_valid".to_string()), EffectTag::Pure, None)
        }
        crate::oml::OMLResult::Approximate => {
            Value::Atom(AtomKind::Tag("oml_approximate".to_string()), EffectTag::Pure, None)
        }
        crate::oml::OMLResult::Violation { rhs, expected } => {
            let mut fields = indexmap::IndexMap::new();
            fields.insert("%kind".to_string(),
                Value::Atom(AtomKind::Tag("oml_violation".to_string()), EffectTag::Pure, None));
            fields.insert("rhs".to_string(), rhs);
            fields.insert("expected".to_string(), expected);
            // 是否伴隨非分配性事件
            if ctx.had_nondistrib_event {
                fields.insert("%nondistributive".to_string(),
                    Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
            }
            Value::Combo(crate::value::ComboVal::new(
                fields, true, indexmap::IndexMap::new(), EffectTag::Pure, vec![]
            ))
        }
    }
}) as Arc<BuiltinFn>);
```

---

## 5. 修改 `src/lib.rs`：掛載 `/check_oml` 到 `~%Engine`

在 `engine_fields` 區塊加入：

```rust
engine_fields.insert("/check_oml".to_string(),
    engine_morph("/check_oml", "engine.check_oml", EffectTag::Pure));
```

---

## 6. 測試：`crates/interpreter/tests/oml_test.rs`（新建）

最少 10 個測試：

| # | 測試名稱 | 驗證內容 |
|---|----------|----------|
| 1 | `test_oml_vacuous_not_subspace` | A ⊄ B → `OMLResult::Vacuous` |
| 2 | `test_oml_valid_tag_true_in_union` | `#true ⊑ (#true \| #false)` → `OMLResult::Valid` |
| 3 | `test_oml_valid_bottom_in_union` | `_\|_ ⊑ A`（任意 A）→ `Vacuous`（`_\|_` 是 Top 的 subspace？需確認） |
| 4 | `test_oml_nondistrib_flag_set` | H² Bottom 在 Union 過濾後 `ctx.had_nondistrib_event == true` |
| 5 | `test_oml_nondistrib_flag_clear` | 純 Conflict Bottom 過濾後 flag 不被設置 |
| 6 | `test_involution_true` | `!!#true = #true` |
| 7 | `test_involution_false` | `!!#false = #false` |
| 8 | `test_de_morgan_union` | `!(A \| B) = !A & !B`（Tag 值） |
| 9 | `test_de_morgan_meet` | `!(A & B) = !A \| !B`（Tag 值） |
| 10 | `test_check_oml_builtin` | 透過 `Ouroboros` 呼叫 `engine.check_oml { a: #true, b: #true \| #false }` → `#oml_valid` |

---

## 7. 設計決策與限制

### OML 驗證精度

OML 驗證對 Atom/Tag 值精確，對 Combo 值回傳 `Approximate`（因為
`content_hash().digest` 比較受 Lattice Sketch v2 雜湊影響，兩個語義相等但由不同路徑
建構的 Combo 可能有不同 digest）。完整精確的 OML 驗證需要 Phase 8+ 的真實 Hilbert
空間。

### Join 的工程代理

`join_values(A, B) = Union(A, B)` 是格論 join 的保守近似：
- 對正交值（`A & B = _|_`）精確
- 對非正交值可能高估（Union 包含兩者，但格論 join 是最小上界）

在 Tag 值的情境下（`#true`、`#false`、`#true | #false`），Union 與格論 join 完全一致，
OML 驗證精確。

### 非分配性事件的語義

`ctx.had_nondistrib_event = true` 表示本次求值路徑上，某次 Union 分支被 H¹/H²
Bottom 過濾——這是 `A ∧ (B₁ | B₂) ≠ (A ∧ B₁) | (A ∧ B₂)` 的工程表現。該旗標
不持久化（每次 `EvalContext::new()` 重設為 `false`），僅供當次求值診斷使用。

### De Morgan 律的現狀

`complement.rs` 中的 De Morgan 律（`!(A | B) = !A & !B`）對 Union 路徑：
```
orthocomplement(Union(A, B))
→ complements: [!A, !B]
→ unify_internal(!A, !B)   ← 這才是 !A & !B
```
這與 De Morgan 一致。反向（`!(A & B) = !A | !B`）對 Combo 路徑：
```
complement_combo(Combo{a:Va, b:Vb})
→ all_complements: [!Va, !Vb]
→ unify_internal(!Va, !Vb)  ← 這是 !Va & !Vb，不是 !Va | !Vb
```
現有實作在 open Combo 路徑對 De Morgan `!(A & B)` 的計算**不正確**（應為
`!Va | !Vb`，實際計算為 `!Va & !Vb`）。Phase 7 測試（test 9）將揭示此問題。

**修正方向**（同樣是 Phase 7 的工作）：
在 `complement_combo` 的 open Combo 分支，將：
```rust
let mut acc = all_complements[0].clone();
for c in all_complements.into_iter().skip(1) {
    acc = self.unify_internal(acc, c, ctx);  // 現在：meet
}
```
改為：
```rust
// De Morgan: !(A & B) = !A | !B → join（Union）
Value::Union(all_complements)
```

注意：Closed Combo（Cocoon）的 De Morgan 行為不同（meet 仍然正確），
只需修改 open Combo 分支。

---

## 8. 完成標準

- [ ] `EvalContext::had_nondistrib_event: bool` 新增並初始化為 `false`
- [ ] `do_unify` Union 分支：H¹/H² Bottom 過濾時設置 `ctx.had_nondistrib_event = true`
- [ ] `src/oml.rs` 新建：`OMLResult`、`verify_subspace()`、`verify_oml()`、`join_values()`
- [ ] `pub mod oml` 在 `lib.rs` 宣告
- [ ] `engine.check_oml` builtin 實作並註冊
- [ ] `~%Engine./check_oml` 態射掛載（`EffectTag::Pure`）
- [ ] `complement.rs` open Combo 分支 De Morgan 修正（`!Va & !Vb` → `Union(!Va, !Vb)`）
- [ ] `tests/oml_test.rs`：10 個測試，全數通過（含 De Morgan 修正後的 test 9）
- [ ] `cargo test` 全數通過（預期 125+ 個測試）
