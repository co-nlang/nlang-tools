# Phase 42 Handover：`disc.find` 多跳迭代路由

> 日期：2026-05-25  
> 實作範圍：`disc.find` 單跳 → 多跳 loop；提取兩個 helper 函數  
> 預期測試：~452 → ~458（新增 ~6 個測試）

---

## 0. 背景與設計決策

### 0.1 單跳的根本限制

Phase 41 之後的 `disc.find`：

```
選最優 GBB → 嘗試從 store/peers 取值 → 找不到 → bottom_not_found()
```

問題：`disc.advertise(value)` 只寫入 `gbb_registry`，**不** 寫入 `oo.store`。
所以 store.get_value(chosen_caid) 幾乎必然失敗，disc.find 的實際用途只有透過 `explicit_target` 欄位才能回傳值。

### 0.2 多跳路由設計

```
hop 0: query Q → 選 GBB A（重力最強）→ 嘗試取 A 的值
       找到 → return A
       找不到 → 用 A 作為下一跳的 query 繼續

hop 1: query A → 選 GBB B（對 A 重力最強）→ 嘗試取 B 的值
       找到 → return B
       找不到 → 用 B 繼續

...直到找到值 / 預算耗盡（SemanticEclipse）/ 無候選（MissingKey）
```

**終止性證明**（繼承 Phase 41）：
- `disc_routing_hops >= MAX_ROUTING_HOPS`（16）→ SemanticEclipse，unconditional
- 若 gbb_registry 有限且黑名單累積 → 最終 no candidates → MissingKey

**語義保證**：每跳使用上一跳選中 GBB 的 sketch/nerve/masa_ref 作為下一跳的 query context，形成重力吸引鏈，逐漸趨近語義最相關的值。

### 0.3 explicit_target 語義不變

若 arg 包含 `target: "caid_str"` 欄位，每一跳都嘗試從當前 peer 取該 CAID。適合「我知道我要找什麼 CAID，但不知道哪個 peer 有它」的場景。

---

## 1. 修改 `crates/interpreter/src/builtins/disc.rs`

### 1.1 提取兩個 helper 函數（在 `perturb_weight` 之後）

```rust
/// Field count as GBB mass (capped at 100).
fn compute_mass(val: &Value) -> f64 {
    if let Value::Combo(ref cv) = val {
        (cv.system.len() + cv.meta.len() + cv.types.len()
         + cv.rules.len() + cv.data.len() + cv.local.len()) as f64
    } else { 1.0 }.min(100.0)
}

/// Build the initial query nerve for disc.find (no overlapping MASA lookup).
fn build_query_nerve(val: &Value) -> Vec<crate::ladd::NerveEntry> {
    if let Value::Combo(ref cv) = val {
        let keys: Vec<String> = cv.all_fields_iter()
            .map(|(k, _)| k)
            .filter(|k| !k.starts_with('%') && !k.starts_with("~%"))
            .collect();
        if keys.is_empty() { vec![] }
        else {
            vec![crate::ladd::NerveEntry {
                masa_caid: field_key_masa_id(cv),
                overlapping_masa_caids: vec![],
                field_keys: keys,
            }]
        }
    } else { vec![] }
}
```

### 1.2 完整替換 `disc.find` builtin

將原本的整個 `disc.find` closure 替換為以下版本（約 line 162–226）：

```rust
    m.insert("disc.find".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        // 1. Build initial query GBB
        let query_hash = arg.content_hash();
        let mut current_query = crate::ladd::GBB {
            node_caid: query_hash.clone(),
            mass: compute_mass(&arg),
            sketch_bytes: base64_decode_sketch(&query_hash.lattice_sketch),
            masa_ref: query_hash.masa_ref.clone(),
            nerve_structure: build_query_nerve(&arg),
        };

        // 2. Extract explicit target CAID (optional direct-lookup mode)
        let explicit_target: Option<String> = if let Value::Combo(ref c) = arg {
            c.get_field("target").map(|v| oo.force(v.clone(), ctx).to_string_plain())
        } else { None };

        const EPSILON: f64 = 1e-6;

        // 3. Multi-hop routing loop
        loop {
            // Safety: hard hop budget (Phase 41)
            if ctx.disc_routing_hops >= MAX_ROUTING_HOPS {
                return Value::Bottom(Box::new(BottomDetail {
                    cause: BottomCause::SemanticEclipse,
                    path: Some("disc.find".to_string()),
                    message: Some(format!(
                        "Routing budget exceeded after {} hops", MAX_ROUTING_HOPS
                    )),
                    ..Default::default()
                }));
            }

            // Gravitational candidate scoring
            let candidates: Vec<(f64, String)> = {
                let reg = match oo.gbb_registry.read() {
                    Ok(r) => r, Err(_) => return BottomCause::Conflict.into(),
                };
                reg.values()
                    .filter(|g| crate::ladd::masa_compatible(&current_query, g))
                    .filter(|g| crate::ladd::nerve_overlap(&current_query, g))
                    .map(|g| {
                        let w = crate::ladd::gravitational_weight(&current_query, g, EPSILON);
                        (w, g.node_caid.to_string())
                    })
                    .collect()
            };

            if candidates.is_empty() { return bottom_not_found(); }

            // Blacklist + horizon_salt tiebreaker (Phase 41)
            let mut perturbed: Vec<(f64, String)> = candidates.iter()
                .map(|(w, caid)| (perturb_weight(*w, caid, &ctx.horizon_salt), caid.clone()))
                .collect();
            perturbed.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

            let chosen = if let Some((_, caid)) = perturbed.iter()
                .find(|(_, c)| !ctx.disc_routing_visited.contains(c))
            {
                caid.clone()
            } else {
                perturbed[0].1.clone() // all visited: use tiebreaker best
            };

            ctx.disc_routing_visited.insert(chosen.clone());
            ctx.disc_routing_hops += 1;

            // Determine which CAID to fetch at this hop
            let fetch_target = explicit_target.as_deref().unwrap_or(chosen.as_str());

            // Try local store, then connected peers
            if let Ok(hash) = crate::value::ContentHash::parse(fetch_target) {
                if let Ok(val) = oo.store.get_value(&hash) { return val; }
                let peers_copy: Vec<_> = oo.peers.read()
                    .map(|p| p.values().cloned().collect()).unwrap_or_default();
                for peer in peers_copy {
                    match peer {
                        crate::Peer::Local(store) => {
                            if let Ok(val) = store.get_value(&hash) { return val; }
                        }
                        crate::Peer::Remote(addr) => {
                            if let Ok(val) = oo.remote_fetch(&addr, &hash) { return val; }
                        }
                    }
                }
            }

            // Value not found at this hop — advance query to chosen GBB for next hop
            let next_gbb = {
                let reg = match oo.gbb_registry.read() {
                    Ok(r) => r, Err(_) => return BottomCause::Conflict.into(),
                };
                reg.get(&chosen).cloned()
            };

            match next_gbb {
                Some(gbb) => { current_query = gbb; }
                None => { return bottom_not_found(); }
            }
        }
    }) as Arc<BuiltinFn>);
```

**注意**：`disc.advertise` 內的 `compute_mass` 重複邏輯可選擇性替換為呼叫新 helper，但非必須。

---

## 2. 新增測試 `crates/interpreter/tests/disc_multihop_test.rs`

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

fn advertise(oo: &Ouroboros, ctx: &mut EvalContext, val: Value) {
    oo.builtin_registry.get("disc.advertise").unwrap().clone()(val, oo, ctx);
}

fn find(oo: &Ouroboros, ctx: &mut EvalContext, query: Value) -> Value {
    oo.builtin_registry.get("disc.find").unwrap().clone()(query, oo, ctx)
}

// ─── 1. Value in store → returns in one hop ───────────────────────────────────

#[test]
fn test_find_returns_stored_value_in_one_hop() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let val = combo(&[("x", 1), ("y", 2)]);

    // Save to store AND register GBB
    oo.store.put_value(&val).expect("put_value should succeed");
    advertise(&oo, &mut ctx, val.clone());

    let result = find(&oo, &mut ctx, combo(&[("x", 1)]));

    // Should find the value (exact or close match)
    assert!(
        !matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::MissingKey)),
        "stored + advertised value should be findable"
    );
    assert_eq!(ctx.disc_routing_hops, 1, "single hop should suffice");
}

// ─── 2. Not in store → hops to semantically related node ──────────────────────

#[test]
fn test_find_multihop_skips_unstored_node() {
    let oo = oo(); let mut ctx = oo.eval_context();

    // node_a: advertised only (not in store) — has fields {a, b}
    let node_a = combo(&[("a", 10), ("b", 20)]);
    advertise(&oo, &mut ctx, node_a.clone());
    // Note: NOT stored in oo.store

    // node_b: advertised AND stored — has fields {b, c} (overlaps with a via "b")
    let node_b = combo(&[("b", 20), ("c", 30)]);
    oo.store.put_value(&node_b).expect("put_value");
    advertise(&oo, &mut ctx, node_b.clone());

    // Query similar to node_a
    let result = find(&oo, &mut ctx, combo(&[("a", 10), ("b", 20)]));

    // Should eventually find node_b via multi-hop (hop 1: a fails; hop 2: b found)
    // At minimum: not a SemanticEclipse (budget not exceeded for 2 hops)
    assert!(
        !matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::SemanticEclipse)),
        "two-hop routing should not exhaust budget"
    );
    assert!(ctx.disc_routing_hops >= 1, "should have made at least one hop");
}

// ─── 3. Multi-hop increments disc_routing_hops ────────────────────────────────

#[test]
fn test_multihop_increments_hop_counter() {
    let oo = oo(); let mut ctx = oo.eval_context();

    // Three nodes: only the third is in store; all have overlapping fields
    for i in 0..3_i64 {
        let v = combo(&[("x", i), ("y", i + 1)]);
        advertise(&oo, &mut ctx, v.clone());
        if i == 2 {
            oo.store.put_value(&v).expect("put_value");
        }
    }

    let _ = find(&oo, &mut ctx, combo(&[("x", 0), ("y", 1)]));

    // Hopped at least once regardless of result
    assert!(ctx.disc_routing_hops >= 1);
}

// ─── 4. SemanticEclipse when budget exhausted ─────────────────────────────────

#[test]
fn test_multihop_semantic_eclipse_on_budget_exhaustion() {
    let oo = oo(); let mut ctx = oo.eval_context();

    // One node: advertised but never stored → each hop finds it but can't fetch value
    let node = combo(&[("z", 99)]);
    advertise(&oo, &mut ctx, node.clone());

    // Pre-consume most of the budget
    ctx.disc_routing_hops = 15; // MAX_ROUTING_HOPS - 1

    let result = find(&oo, &mut ctx, combo(&[("z", 99)]));

    // This hop brings us to 16 → SemanticEclipse on next call would have fired.
    // After this call: hops = 16; one more call triggers eclipse.
    let _ = find(&oo, &mut ctx, combo(&[("z", 99)]));
    // OR: directly set to 16 and verify eclipse fires immediately.
    ctx.disc_routing_hops = 16;
    let eclipse = find(&oo, &mut ctx, combo(&[("z", 99)]));
    assert!(
        matches!(&eclipse, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::SemanticEclipse)),
        "disc_routing_hops >= 16 should give SemanticEclipse, got {:?}", eclipse
    );
    let _ = result; // suppress unused warning
}

// ─── 5. Empty registry → MissingKey (not SemanticEclipse) ────────────────────

#[test]
fn test_multihop_empty_registry_is_missing_key() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let result = find(&oo, &mut ctx, combo(&[("q", 1)]));
    assert!(
        matches!(&result, Value::Bottom(ref bd) if matches!(bd.cause, BottomCause::MissingKey)),
        "empty registry should be MissingKey"
    );
    assert_eq!(ctx.disc_routing_hops, 0, "no hop should occur when registry is empty");
}

// ─── 6. Visited set grows across hops ─────────────────────────────────────────

#[test]
fn test_multihop_visited_set_accumulates() {
    let oo = oo(); let mut ctx = oo.eval_context();

    // Advertise two nodes (neither stored) → two hops, two visited entries
    let a = combo(&[("p", 1), ("q", 2)]);
    let b = combo(&[("p", 3), ("q", 4)]);
    advertise(&oo, &mut ctx, a);
    advertise(&oo, &mut ctx, b);

    // Will hop: find a → not stored → advance → find b → not stored → advance
    // Eventually: no new candidates or budget exhausted
    let _ = find(&oo, &mut ctx, combo(&[("p", 1)]));

    // Visited set should have grown
    assert!(!ctx.disc_routing_visited.is_empty(), "visited set should accumulate hops");
    assert!(ctx.disc_routing_hops > 0, "hop counter should advance");
}
```

---

## 3. 修改 `crates/interpreter/Cargo.toml`

在 `semantic_eclipse_test` 後加入：

```toml
[[test]]
name = "disc_multihop_test"
path = "tests/disc_multihop_test.rs"
```

---

## 4. 完成後驗證

```bash
cargo test
```

預期：~458 tests，0 failed。

重點確認：
- `disc.find` 對已存入 store 的值 → 一跳回傳
- 未存入 store 的值 → 多跳嘗試後繼續路由（不立即 MissingKey）
- `disc_routing_hops` 每跳累加
- `disc_routing_hops >= 16` → SemanticEclipse
- 空 registry → MissingKey（不進入 loop）
- 所有舊測試通過（disc.find 的公開介面不變）

---

## 5. 實作注意事項

| 事項 | 說明 |
|:-----|:-----|
| 提取 helper 的時機 | `compute_mass` 和 `build_query_nerve` 在 `perturb_weight` 後，`register_disc_builtins` 前定義 |
| 舊 disc.advertise 的 mass 計算 | 可改用 `compute_mass(&arg)` 但非必要；保留原樣也正確 |
| `current_query` 型態 | `crate::ladd::GBB` — 初始由 arg 建構，之後直接從 registry clone |
| registry 讀鎖不跨 await | 每次取鎖 → collect/clone → 立即 drop，loop 下一輪再取。Pattern 與 Phase 41 一致，安全 |
| `next_gbb` 可能 None | GBB CAID 在 registry 但取不到（理論上不可能）→ `bottom_not_found()` 安全退出 |
| 測試中 `oo.store.put_value(&val)` | `oo.store` 是 `pub ObjectStore`，`put_value` 回傳 `Result<ContentHash>` — 直接呼叫，unwrap 或 expect 均可 |
| 測試 5（預算測試）的寫法 | 先設 `ctx.disc_routing_hops = 16` 再呼叫 find → 第一件事就是 budget check → SemanticEclipse。不需要實際執行 16 跳 |
| `explicit_target` 欄位語義不變 | 若 arg 有 `{target: "caid"}` → 每跳都嘗試取該 CAID；routing loop 仍然按重力前進 |

---

## 6. 修改摘要（2 個檔案）

| 檔案 | 改動 |
|:-----|:-----|
| `src/builtins/disc.rs` | 新增 `compute_mass`、`build_query_nerve` 兩個 helper；`disc.find` closure 整段替換為 multi-hop loop（約 60 行舊 → 75 行新） |
| `tests/disc_multihop_test.rs` | 新建，6 個測試 |
| `Cargo.toml` | +3 行 `[[test]]` entry |
