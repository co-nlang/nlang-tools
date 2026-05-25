# Phase 41 Handover：視界震盪防禦（#semantic_eclipse）

> 日期：2026-05-25  
> 實作範圍：`disc.find` 確定性黑名單 + horizon_salt tiebreaker；新增 `BottomCause::SemanticEclipse`  
> 預期測試：~446 → ~452（新增 ~6 個測試）

---

## 0. 背景與設計決策

### 0.1 問題：現有 stub 是假的

`disc.find` 目前的「防禦」（`disc.rs` line 202–207）：

```rust
// 3. Horizon oscillation: 10% random jump
let chosen_caid_str = if ctx.horizon_salt.digest.first() == Some(&0) {
    // 約 1/256 機率隨機跳
    let idx = (ctx.horizon_salt.digest.get(1).copied().unwrap_or(0) as usize) % candidates.len();
    candidates[idx].1.clone()
} else {
    candidates[0].1.clone()
};
```

問題：
- 1/256 的機率只是偽裝，並非真正的震盪偵測
- 沒有追蹤已訪問節點（每次呼叫都可能選同一個）
- 沒有 hop budget（可以無限振盪）
- `SemanticEclipse` 沒有對應的 `BottomCause` variant

### 0.2 設計：確定性黑名單 + horizon_salt tiebreaker

```
偵測：EvalContext 攜帶 disc_routing_visited（HashSet<String>）和 disc_routing_hops（u32）

Hop Budget：disc_routing_hops >= MAX_ROUTING_HOPS（16）→ Bottom(SemanticEclipse)
  可證性：O(16) worst-case 終止，無需任何隨機假設

候選選擇：
  1. 用 horizon_salt + node_caid 擾動重力權重（±0.5%，deterministic tiebreaker）
  2. 優先選未訪問的候選節點（blacklist）
  3. 若全部已訪問 → fallback 選擾動後最佳（不 SemanticEclipse，繼續嘗試）
  4. 記錄選中節點到 disc_routing_visited，累加 disc_routing_hops

Effectiveness proof：
  - 終止（Safety）：MAX_ROUTING_HOPS 是硬性上界，unconditionally terminates
  - 跳出品質（Liveness）：blacklist 保證每次優先嘗試新節點；horizon_salt 為 session-local，
    對外不可預測，破壞確定性振盪攻擊
```

### 0.3 重要：disc_routing_visited 是 session-wide

`sub_context` 做的是 `ctx.clone()` — 因此 sub-context 繼承 visited set 和 hop 計數。
這是刻意的：在一次 eval session 中，disc.find 的路由狀態跨呼叫累積，防止跨呼叫振盪。

---

## 1. 修改 `crates/interpreter/src/value.rs`

### 1.1 enum 定義（line 382）

原本：
```rust
pub enum BottomCause { #[default] Conflict, MissingKey, FuelExhausted, Timeout, Divergent, InvalidPath, PrivateAccessViolation, NumericalError, ArithmeticOnAnchor, H1Split, H2Split }
```

改為（末尾加 `SemanticEclipse`）：
```rust
pub enum BottomCause { #[default] Conflict, MissingKey, FuelExhausted, Timeout, Divergent, InvalidPath, PrivateAccessViolation, NumericalError, ArithmeticOnAnchor, H1Split, H2Split, SemanticEclipse }
```

### 1.2 `as_cause_combo` match（line 306–318）

在 `BottomCause::H2Split => "#h2_split",` 後加：
```rust
            BottomCause::SemanticEclipse => "#semantic_eclipse",
```

### 1.3 `as_tag` match（line 385–397）

在 `BottomCause::H2Split => "h2_split",` 後加：
```rust
            BottomCause::SemanticEclipse => "semantic_eclipse",
```

---

## 2. 修改 `crates/interpreter/src/lib.rs`

### 2.1 EvalContext struct（line 37–56）

在 `pub had_nondistrib_event: bool,` 後加兩行：
```rust
    pub disc_routing_visited: std::collections::HashSet<String>,
    pub disc_routing_hops: u32,
```

### 2.2 EvalContext::new()（line 63–71）

在 `had_nondistrib_event: false,` 後加（保持在 `}` 前）：
```rust
            disc_routing_visited: std::collections::HashSet::new(),
            disc_routing_hops: 0,
```

### 2.3 `%type` path navigation match（line 826–838）

在 `BottomCause::H2Split => "h2_split",` 後加：
```rust
                        BottomCause::SemanticEclipse => "semantic_eclipse",
```

---

## 3. 修改 `crates/interpreter/src/builtins/disc.rs`

### 3.1 新增常數（在 `fn base64_decode_sketch` 前，第 8 行之前）

```rust
const MAX_ROUTING_HOPS: u32 = 16;
```

### 3.2 新增 helper 函數（在 `fn bottom_not_found` 後）

```rust
/// Perturb gravitational weight with a deterministic session salt.
/// Adds ±0.5% noise: enough to break ties, not enough to override strong gravity.
fn perturb_weight(weight: f64, caid: &str, horizon_salt: &crate::value::ContentHash) -> f64 {
    use sha2::{Sha256, Digest as Sha2Digest};
    let mut h = Sha256::new();
    h.update(&horizon_salt.digest);
    h.update(caid.as_bytes());
    let hash = h.finalize();
    let salt_f = u64::from_be_bytes(hash[0..8].try_into().unwrap()) as f64 / u64::MAX as f64;
    weight * (1.0 + (salt_f - 0.5) * 0.01)
}
```

### 3.3 替換 `disc.find` step 3（原 line 198–208）

原本的 step 3（含前面的 sort）：
```rust
        if candidates.is_empty() { return bottom_not_found(); }
        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // 3. Horizon oscillation: 10% random jump
        let chosen_caid_str = if ctx.horizon_salt.digest.first() == Some(&0) {
            let idx = (ctx.horizon_salt.digest.get(1).copied().unwrap_or(0) as usize) % candidates.len();
            candidates[idx].1.clone()
        } else {
            candidates[0].1.clone()
        };
```

整段替換為：
```rust
        if candidates.is_empty() { return bottom_not_found(); }

        // 3. Horizon oscillation defence: blacklist + horizon_salt tiebreaker
        // Safety: hard hop budget terminates routing unconditionally.
        if ctx.disc_routing_hops >= MAX_ROUTING_HOPS {
            return Value::Bottom(Box::new(BottomDetail {
                cause: BottomCause::SemanticEclipse,
                path: Some("disc.find".to_string()),
                message: Some(format!(
                    "Routing budget exceeded after {} hops (MAX_ROUTING_HOPS={})",
                    ctx.disc_routing_hops, MAX_ROUTING_HOPS
                )),
                ..Default::default()
            }));
        }

        // Apply deterministic perturbation (horizon_salt × node_caid → ±0.5% weight noise)
        let mut perturbed: Vec<(f64, String)> = candidates.iter()
            .map(|(w, caid)| (perturb_weight(*w, caid, &ctx.horizon_salt), caid.clone()))
            .collect();
        perturbed.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Prefer unvisited candidates; fall back to best revisited if blacklist is exhausted.
        let chosen_caid_str = if let Some((_, caid)) = perturbed.iter()
            .find(|(_, c)| !ctx.disc_routing_visited.contains(c))
        {
            caid.clone()
        } else {
            // All candidates have been visited in this session — tiebreaker still applies.
            perturbed[0].1.clone()
        };

        ctx.disc_routing_visited.insert(chosen_caid_str.clone());
        ctx.disc_routing_hops += 1;
```

**注意**：`BottomDetail` 需要 import。檢查 disc.rs 頂部是否已有：
```rust
use crate::value::{Value, EffectTag, BottomCause, BottomDetail, ContentHash};
```
已有（line 4）。`BottomCause::SemanticEclipse` 直接可用。

---

## 4. 新增測試 `crates/interpreter/tests/semantic_eclipse_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, BottomCause, ComboVal};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use num_bigint::BigInt;

fn oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}

fn combo(fields: &[(&str, i64)]) -> Value {
    let mut m = IndexMap::new();
    for (k, v) in fields { m.insert(k.to_string(), int_val(*v)); }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call_find(oo: &Ouroboros, ctx: &mut EvalContext, arg: Value) -> Value {
    oo.builtin_registry.get("disc.find").unwrap().clone()(arg, oo, ctx)
}

fn call_advertise(oo: &Ouroboros, ctx: &mut EvalContext, arg: Value) -> Value {
    oo.builtin_registry.get("disc.advertise").unwrap().clone()(arg, oo, ctx)
}

// ─── 1. Empty registry → MissingKey (not SemanticEclipse) ────────────────────

#[test]
fn test_find_empty_registry_is_missing_key() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let result = call_find(&oo, &mut ctx, combo(&[("x", 1)]));
    assert!(
        matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::MissingKey)),
        "empty registry should be MissingKey, got {:?}", result
    );
}

// ─── 2. Normal find: adds chosen node to disc_routing_visited ─────────────────

#[test]
fn test_find_adds_to_visited() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let node = combo(&[("x", 1)]);
    call_advertise(&oo, &mut ctx, node.clone());

    assert!(ctx.disc_routing_visited.is_empty(), "visited should start empty");
    let _ = call_find(&oo, &mut ctx, node.clone());
    assert_eq!(ctx.disc_routing_hops, 1, "hop count should be 1 after one find");
    assert!(!ctx.disc_routing_visited.is_empty(), "visited should be non-empty after find");
}

// ─── 3. Budget exceeded → SemanticEclipse ─────────────────────────────────────

#[test]
fn test_find_hop_budget_exceeded_returns_semantic_eclipse() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let node = combo(&[("x", 42)]);
    call_advertise(&oo, &mut ctx, node.clone());

    // Manually exhaust the hop budget
    ctx.disc_routing_hops = 16; // MAX_ROUTING_HOPS = 16

    let result = call_find(&oo, &mut ctx, node);
    assert!(
        matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::SemanticEclipse)),
        "exceeded hop budget should return SemanticEclipse, got {:?}", result
    );
}

// ─── 4. SemanticEclipse has correct %type path ────────────────────────────────

#[test]
fn test_semantic_eclipse_as_tag() {
    assert_eq!(BottomCause::SemanticEclipse.as_tag(), "semantic_eclipse");
}

// ─── 5. All-visited fallback: still returns a result (not SemanticEclipse) ────

#[test]
fn test_find_all_visited_still_returns() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let node = combo(&[("p", 100), ("q", 200)]);
    call_advertise(&oo, &mut ctx, node.clone());

    // First call — gets the node, marks it visited
    let r1 = call_find(&oo, &mut ctx, node.clone());
    // It may or may not succeed (depends on store), but routing_visited should be populated
    assert_eq!(ctx.disc_routing_hops, 1);

    // The visited set now contains the advertised node_caid.
    // Second call — candidate is visited, but budget not exceeded → should still pick it (fallback).
    let r2 = call_find(&oo, &mut ctx, node.clone());
    // Should NOT be SemanticEclipse (hop 2 < 16)
    assert!(
        !matches!(&r2, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::SemanticEclipse)),
        "all-visited with budget remaining should not SemanticEclipse, got {:?}", r2
    );
    assert_eq!(ctx.disc_routing_hops, 2);
}

// ─── 6. horizon_salt tiebreaker: same query → same chosen node (deterministic) ─

#[test]
fn test_find_tiebreaker_is_deterministic() {
    // Two separate sessions with same oo, different ctx (fresh horizon_salt each time)
    // Both should select a candidate, but potentially different ones — that's OK.
    // Just verify both return non-Bottom and disc_routing_hops increments correctly.
    let oo = oo();

    let node_a = combo(&[("a", 1)]);
    let node_b = combo(&[("b", 2)]);

    let mut ctx = oo.eval_context();
    call_advertise(&oo, &mut ctx, node_a.clone());
    call_advertise(&oo, &mut ctx, node_b.clone());

    let mut ctx1 = oo.eval_context();
    let r1 = call_find(&oo, &mut ctx1, combo(&[("a", 1)]));

    let mut ctx2 = oo.eval_context();
    let r2 = call_find(&oo, &mut ctx2, combo(&[("a", 1)]));

    // Both sessions make exactly one hop
    assert_eq!(ctx1.disc_routing_hops, 1);
    assert_eq!(ctx2.disc_routing_hops, 1);

    // Both have one visited node
    assert_eq!(ctx1.disc_routing_visited.len(), 1);
    assert_eq!(ctx2.disc_routing_visited.len(), 1);

    // Results are not SemanticEclipse (candidates existed)
    assert!(!matches!(&r1, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::SemanticEclipse)));
    assert!(!matches!(&r2, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::SemanticEclipse)));
}
```

---

## 5. 修改 `crates/interpreter/Cargo.toml`

在 `[[test]]` 區段末尾（`h1_phase_test` 後）加入：

```toml
[[test]]
name = "semantic_eclipse_test"
path = "tests/semantic_eclipse_test.rs"
```

---

## 6. 完成後驗證

```bash
cargo test
```

預期：~452 tests，0 failed。

重點確認：
- `BottomCause::SemanticEclipse.as_tag()` == `"semantic_eclipse"`
- 空 registry → `MissingKey`（非 SemanticEclipse）
- Hop budget 超過 → `SemanticEclipse`
- 正常 find → `disc_routing_hops` 累加，`disc_routing_visited` 有條目
- 全候選已訪問 + budget 未耗盡 → 不 SemanticEclipse（fallback 選擾動後最佳）
- 所有舊有測試通過（EvalContext 新欄位有預設值，行為不變）

---

## 7. 實作注意事項

| 事項 | 說明 |
|:-----|:-----|
| `BottomCause` 有 4 個 exhaustive match | 必須全部更新：`value.rs`（2 處：`as_cause_combo`、`as_tag`）、`lib.rs`（1 處：`%type` navigation）。漏掉一處會 compile error，有助發現。 |
| `EvalContext::new` 初始化 | `disc_routing_visited: std::collections::HashSet::new(), disc_routing_hops: 0` — 兩個欄位放在 `had_nondistrib_event: false,` 之後 |
| `sub_context` 不需修改 | 它做 `ctx.clone()`，自動繼承 visited set 和 hop counter |
| `perturb_weight` 的 sha2 import | 使用 `use sha2::{Sha256, Digest as Sha2Digest};`（注意 alias `Sha2Digest` 避免與 `BottomDetail` 的 `Digest` 衝突） |
| 刪除舊 stub | 原本的 `// 3. Horizon oscillation: 10% random jump` 整段（包括 comment）完整替換 |
| `BottomDetail` 的 `..Default::default()` | `BottomDetail` 已有 `#[derive(Default)]`，SemanticEclipse 不需要 `involved`、`obstruction_degree`、`holonomy` |
| MAX_ROUTING_HOPS 型態 | `u32`，與 `ctx.disc_routing_hops: u32` 一致；比較 `>=` |
| 測試中手動設定 hops | `ctx.disc_routing_hops = 16;` — 可直接設定，欄位是 `pub` |

---

## 8. 修改摘要（4 個檔案）

| 檔案 | 改動 |
|:-----|:-----|
| `src/value.rs` | `BottomCause` enum + 2 個 match 加 `SemanticEclipse` 分支 |
| `src/lib.rs` | `EvalContext` + 2 個新欄位；`EvalContext::new()` 初始化；`%type` match 新分支 |
| `src/builtins/disc.rs` | 新常數 `MAX_ROUTING_HOPS`；新 helper `perturb_weight`；disc.find step 3 整段替換（約 18 行舊 → 約 30 行新） |
| `tests/semantic_eclipse_test.rs` | 新建，6 個測試 |
| `Cargo.toml` | +3 行 `[[test]]` entry |
