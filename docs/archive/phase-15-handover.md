# Phase 15 Handover: Cycle Detection + %timeout Runtime + %fmap Functor

**Date:** 2026-05-24  
**Status:** Ready for implementation  
**Depends on:** Phase 14 (complete)  
**Spec refs:** SPEC_10 §5.2, SPEC_09 §6, SPEC_09 §1

---

## 目標

三個獨立 P2 項目，方向一致：強化 #refine 安全性、補完 `eval_context()` 配置連接、為標準型別加上 Functor 層。

1. **Cycle detection in `#refine`** — BFS 阻擋形成環路的 refine commit
2. **`%timeout` → `timeout_deadline` runtime** — `eval_context()` 讀取 `%timeout` 設置截止時刻，`check_resources()` 套用
3. **`option.map`/`result.map` + `%fmap` on `@option`/`@result`** — 加入 Functor 操作，更新 genesis seeds

---

## 任務一：Cycle Detection in `#refine`

### 背景

`follow_refine()` 在**查詢**時已有 visited set 防循環（`BottomCause::Divergent`），但 `universe.rs::refine()` 在**寫入** refine commit 時沒有預先檢查。因此可以提交 `A → B` 再提交 `B → A`，雖然後續 `follow_refine` 會捕捉，但 commit 已寫入 DAG。Phase 15 在提交前阻斷。

### 改動位置：`crates/interpreter/src/universe.rs`，`refine()` 函數

在 Step 1c shadow scan 之後、Step 2 build commit 之前，插入 Step 1d：

```rust
// Step 1d: cycle detection — reject if source→target would close a refine cycle
{
    let map = engine.refine_map.read().map_err(|e| anyhow::anyhow!("{:?}", e))?;
    for src in &source_caids {
        let src_str = src.to_string();
        for tgt in &target_caids {
            // BFS from tgt in existing map to see if src is reachable
            let mut stack = vec![tgt.to_string()];
            let mut seen = std::collections::HashSet::new();
            while let Some(current) = stack.pop() {
                if current == src_str {
                    return Err(anyhow::anyhow!(
                        "refine cycle detected: {} → {} would create a cycle",
                        src_str, tgt
                    ));
                }
                if seen.insert(current.clone()) {
                    if let Some(nexts) = map.get(&current) {
                        stack.extend(nexts.iter().cloned());
                    }
                }
            }
        }
    }
}
```

**插入位置（精確）**：在以下這行之前：

```rust
        // Step 2: build Refine Commit
        let current_root_hash = match &self.head {
```

### 測試：新增至 `crates/interpreter/tests/refine_test.rs`

```rust
#[test]
fn refine_cycle_ab_ba_rejected() {
    // A → B then B → A should be rejected
    let oo = Arc::new(Ouroboros::new_in_memory());
    let base_dir = std::env::temp_dir().join("nlang-cycle-ab");
    let _ = std::fs::create_dir_all(&base_dir);
    let mut u = Universe::new(None, oo.root_with_system());

    let val_a = Value::Top;
    let val_b = Value::Atom(nlang_parser::ast::AtomKind::Int(1i64.into()), EffectTag::Pure, None);
    let ca = oo.store.put_value(&val_a).unwrap();
    let cb = oo.store.put_value(&val_b).unwrap();

    // First refine: A → B (OK, head=None → bootstrap_exempt)
    let meta1 = CommitMeta { author: None, timestamp: 0, message: None };
    u.refine(&oo, &base_dir, vec![ca.clone()], vec![cb.clone()], None, meta1).unwrap();

    // Second refine: B → A should fail (cycle: A→B→A)
    let meta2 = CommitMeta { author: None, timestamp: 1, message: None };
    let result = u.refine(&oo, &base_dir, vec![cb.clone()], vec![ca.clone()], None, meta2);
    assert!(result.is_err(), "B→A should be rejected as it closes A→B cycle: {:?}", result);
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("cycle"), "Error should mention cycle: {}", err_str);
}

#[test]
fn refine_same_source_twice_no_cycle() {
    // A → B then A → C should succeed (fan-out, not a cycle)
    let oo = Arc::new(Ouroboros::new_in_memory());
    let base_dir = std::env::temp_dir().join("nlang-cycle-fan");
    let _ = std::fs::create_dir_all(&base_dir);
    let mut u = Universe::new(None, oo.root_with_system());

    let val_a = Value::Top;
    let val_b = Value::Atom(nlang_parser::ast::AtomKind::Int(10i64.into()), EffectTag::Pure, None);
    let val_c = Value::Atom(nlang_parser::ast::AtomKind::Int(10i64.into()), EffectTag::Pure, None); // same value
    let ca = oo.store.put_value(&val_a).unwrap();
    let cb = oo.store.put_value(&val_b).unwrap();
    let cc = oo.store.put_value(&val_c).unwrap();

    let meta1 = CommitMeta { author: None, timestamp: 0, message: None };
    u.refine(&oo, &base_dir, vec![ca.clone()], vec![cb.clone()], None, meta1).unwrap();

    // A → C: A is already a source, but C doesn't lead back to A
    let meta2 = CommitMeta { author: None, timestamp: 1, message: None };
    let result = u.refine(&oo, &base_dir, vec![ca.clone()], vec![cc.clone()], None, meta2);
    assert!(result.is_ok(), "fan-out (A→B, A→C) should be allowed: {:?}", result);
}
```

---

## 任務二：`%timeout` → `timeout_deadline` Runtime

### 背景

`~%Config` 中有 `%timeout: 1000`（毫秒），但 `eval_context()` 目前讀取 fuel、max_branches 等欄位，跳過了 `%timeout`。`check_resources()` 裡 `timeout_deadline` 欄位存在但從未被設置（始終為 `None`），也從未被檢查。

### 改動位置 A：`crates/interpreter/src/lib.rs`，`eval_context()` 方法

在 `strategy` 讀取之後加入：

```rust
// 原本的 strategy 讀取（已存在）：
if let Some(Value::Atom(AtomKind::Tag(s), _, _)) = cfg.get_field("%strategy").cloned() {
    ctx.strategy = match s.trim_start_matches('#') {
        "strict" => ObservationStrategy::Strict,
        "approximate" => ObservationStrategy::Approximate,
        _ => ObservationStrategy::Blur,
    };
}
// 新增 %timeout：
if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("%timeout").cloned() {
    if let Some(timeout_ms) = n.to_u64() {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        ctx.timeout_deadline = Some(now_ms + timeout_ms);
    }
}
```

**需要的 import**：`std::time::SystemTime` 應已在 `lib.rs` 使用（disc/serve 區段），確認頂端有 `use std::time::...`。若無則加入。

### 改動位置 B：`crates/interpreter/src/lib.rs`，`check_resources()` 方法

**原始（約 75–80 行）：**
```rust
pub fn check_resources(&mut self, cost: u64) -> Result<(), ResourceExhausted> {
    if self.fuel < cost { Err(ResourceExhausted::FuelExhausted) }
    else if self.depth > self.max_unification_depth as u32 { Err(ResourceExhausted::StackOverflow) }
    else { self.fuel -= cost; Ok(()) }
}
```

**改後：**
```rust
pub fn check_resources(&mut self, cost: u64) -> Result<(), ResourceExhausted> {
    if self.fuel < cost { return Err(ResourceExhausted::FuelExhausted); }
    if self.depth > self.max_unification_depth as u32 { return Err(ResourceExhausted::StackOverflow); }
    if let Some(deadline) = self.timeout_deadline {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        if now > deadline { return Err(ResourceExhausted::Timeout); }
    }
    self.fuel -= cost;
    Ok(())
}
```

**重要**：`EvalContext::new()` 中 `timeout_deadline: None`（不動），只有 `eval_context()` 才設截止時刻。既有測試全部使用 `EvalContext::new()` → `timeout_deadline = None` → 不受影響。

### 測試：新增至 `crates/interpreter/tests/genesis_test.rs`

```rust
#[test]
fn eval_context_sets_timeout_deadline() {
    let oo = Ouroboros::new_in_memory();
    let ctx = oo.eval_context();
    // ~%Config has %timeout: 1000 → deadline should be set (Some)
    assert!(ctx.timeout_deadline.is_some(),
        "eval_context() should set timeout_deadline from ~%Config %timeout");
    // Deadline should be in the future (now + 1000ms)
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let deadline = ctx.timeout_deadline.unwrap();
    assert!(deadline > now_ms, "timeout_deadline should be in the future");
    assert!(deadline < now_ms + 2000, "timeout_deadline should be within 2 seconds");
}

#[test]
fn eval_context_new_has_no_timeout() {
    let oo = Ouroboros::new_in_memory();
    let ctx = EvalContext::new(oo.root_with_system());
    // EvalContext::new() does NOT set timeout — only eval_context() does
    assert!(ctx.timeout_deadline.is_none(),
        "EvalContext::new() should not set timeout_deadline");
}
```

---

## 任務三：`option.map`/`result.map` + `%fmap` on `@option`/`@result`

### 背景

`@option` 和 `@result` 現在是型別約束標記，但沒有 Functor 操作。SPEC_09 §1 要求代數介面包含 `%fmap`（map over the contained value）。Phase 15 加入：

- `option.map(f, opt)` — `Some(x) → Some(f(x))`；`#none → #none`
- `result.map(f, res)` — `Ok(x) → Ok(f(x))`；`Err(e) → Err(e)` 不動
- `result.map_err(f, res)` — `Ok(x) → Ok(x)` 不動；`Err(e) → Err(f(e))`
- `@option { %fmap: <option.map morphism> }`
- `@result { %fmap: <result.map morphism>, %map_err: <result.map_err morphism> }`

**注意：** 加入 `%fmap` 欄位會改變 `@option`/`@result` 的 CAID，需更新 `SEED_OPTION`/`SEED_RESULT`。

### 改動位置 A：`crates/interpreter/src/builtins/engine.rs`

在 `register_engine_builtins` 函數末尾加入：

```rust
// ── Functor operations ──────────────────────────────────────────

m.insert("option.map".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    // arg = { 0: morphism/fn, 1: option_value }
    if let Value::Combo(ref c) = arg {
        if let (Some(f), Some(opt_v)) = (c.get_field("0"), c.get_field("1")) {
            let f = f.clone();
            let opt = oo.force(opt_v.clone(), ctx);
            return match opt.collapse() {
                // #none → #none
                Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none" => {
                    Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None)
                }
                // { %val: x } → { %val: f(x) }
                Value::Combo(ref cv) => {
                    if let Some(inner) = cv.get_field("%val") {
                        let mapped = oo.apply_morphism(f, inner.clone(), ctx);
                        let mut res_fields = IndexMap::new();
                        res_fields.insert("%val".to_string(), mapped);
                        Value::Combo(ComboVal::new(res_fields, false, IndexMap::new(), EffectTag::Pure, vec![]))
                    } else {
                        Value::Top
                    }
                }
                _ => Value::Top,
            };
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);

m.insert("result.map".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    // arg = { 0: morphism/fn, 1: result_value }
    if let Value::Combo(ref c) = arg {
        if let (Some(f), Some(res_v)) = (c.get_field("0"), c.get_field("1")) {
            let f = f.clone();
            let res = oo.force(res_v.clone(), ctx);
            if let Value::Combo(ref cv) = res.collapse() {
                if let Some(inner) = cv.get_field("%val") {
                    // Ok(x) → Ok(f(x))
                    let mapped = oo.apply_morphism(f, inner.clone(), ctx);
                    let mut res_fields = IndexMap::new();
                    res_fields.insert("%val".to_string(), mapped);
                    return Value::Combo(ComboVal::new(res_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
                }
                if cv.get_field("%cause").is_some() {
                    // Err(e) → Err(e) unchanged
                    return res.collapse().clone();
                }
            }
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);

m.insert("result.map_err".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    // arg = { 0: morphism/fn, 1: result_value }
    if let Value::Combo(ref c) = arg {
        if let (Some(f), Some(res_v)) = (c.get_field("0"), c.get_field("1")) {
            let f = f.clone();
            let res = oo.force(res_v.clone(), ctx);
            if let Value::Combo(ref cv) = res.collapse() {
                if cv.get_field("%val").is_some() {
                    // Ok(x) → Ok(x) unchanged
                    return res.collapse().clone();
                }
                if let Some(cause) = cv.get_field("%cause") {
                    // Err(e) → Err(f(e))
                    let mapped = oo.apply_morphism(f, cause.clone(), ctx);
                    let mut res_fields = IndexMap::new();
                    res_fields.insert("%cause".to_string(), mapped);
                    return Value::Combo(ComboVal::new(res_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
                }
            }
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);
```

**需要的 import**（engine.rs 頂端已有大多數，確認有）：
```rust
use indexmap::IndexMap;
use crate::value::{ComboVal, EffectTag};
use nlang_parser::ast::AtomKind;
```

### 改動位置 B：`crates/interpreter/src/lib.rs`，`root_with_system()` 的 `@option` 段

**找到並替換** `@option` 的構建段（目前約 196–215 行）：

**原始：**
```rust
// @option: @Some { %val: _ } | #none  (SPEC_09 §2.7)
let mut option_fields = IndexMap::new();
option_fields.insert("%kind".to_string(), ...);
option_fields.insert("%name".to_string(), ...);
option_fields.insert("%some".to_string(), ...);
option_fields.insert("%none".to_string(), ...);
fields.insert(
    "@option".to_string(),
    Value::Combo(ComboVal::new(option_fields, true, IndexMap::new(), EffectTag::Pure, vec![])),
);
```

**加入 `%fmap` 欄位**（在 `fields.insert("@option", ...)` 之前）：
```rust
option_fields.insert(
    "%fmap".to_string(),
    Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
        ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
        ("%builtin".to_string(), Value::Atom(AtomKind::Str("option.map".to_string()), EffectTag::Pure, None)),
    ]), true, IndexMap::new(), EffectTag::Pure, vec![])),
);
// 然後 fields.insert("@option", ...) — 不動
```

### 改動位置 C：`crates/interpreter/src/lib.rs`，`root_with_system()` 的 `@result` 段

類似地，在 `fields.insert("@result", ...)` 之前加入：
```rust
result_fields.insert(
    "%fmap".to_string(),
    Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
        ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
        ("%builtin".to_string(), Value::Atom(AtomKind::Str("result.map".to_string()), EffectTag::Pure, None)),
    ]), true, IndexMap::new(), EffectTag::Pure, vec![])),
);
result_fields.insert(
    "%map_err".to_string(),
    Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
        ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
        ("%builtin".to_string(), Value::Atom(AtomKind::Str("result.map_err".to_string()), EffectTag::Pure, None)),
    ]), true, IndexMap::new(), EffectTag::Pure, vec![])),
);
// 然後 fields.insert("@result", ...) — 不動
```

### 改動位置 D：更新 Genesis Seeds

`@option` 和 `@result` 的結構改變 → CAID 改變 → 需更新常數。

**步驟：**
1. 完成上述改動後，執行：
   ```bash
   cargo test -p nlang-interpreter seed_caids_are_stable -- --nocapture 2>&1 | grep "UPDATE:"
   ```
2. 輸出類似：
   ```
   UPDATE: const SEED_OPTION: &str = "hash:sha256:v1:<new_hash>";
   UPDATE: const SEED_RESULT: &str = "hash:sha256:v1:<new_hash>";
   ```
3. 將新值更新至 `crates/interpreter/src/genesis.rs` 中的 `SEED_OPTION` 和 `SEED_RESULT` 常數。

### 測試：新增 `crates/interpreter/tests/functor_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, ComboVal, EffectTag};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;

fn make_int(n: i64) -> Value {
    Value::Atom(AtomKind::Int(n.into()), EffectTag::Pure, None)
}

fn make_some(val: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("%val".to_string(), val);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_none() -> Value {
    Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None)
}

fn make_ok(val: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("%val".to_string(), val);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_err(cause: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("%cause".to_string(), cause);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn get_match_builtin(name: &str, oo: &Ouroboros) -> Value {
    // Retrieve builtin morphism from ~%Engine ... actually construct directly
    let mut f = IndexMap::new();
    f.insert("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
    f.insert("%builtin".to_string(), Value::Atom(AtomKind::Str(name.to_string()), EffectTag::Pure, None));
    Value::Combo(ComboVal::new(f, true, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_map_arg(f: Value, val: Value) -> Value {
    let mut fields = IndexMap::new();
    fields.insert("0".to_string(), f);
    fields.insert("1".to_string(), val);
    Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

#[test]
fn option_map_some() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());

    // option.map(identity_morph, Some(42)) → Some(42)
    let opt = make_some(make_int(42));
    let morph = get_match_builtin("option.map", &oo);
    let arg = make_map_arg(Value::Top, opt); // Top as identity morphism
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    if let Value::Combo(ref c) = result.collapse() {
        assert!(c.get_field("%val").is_some(), "option.map(Some(42)) should return Some: {:?}", result);
    } else {
        panic!("Expected Combo with %val, got {:?}", result);
    }
}

#[test]
fn option_map_none() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());

    // option.map(anything, #none) → #none
    let opt = make_none();
    let morph = get_match_builtin("option.map", &oo);
    let arg = make_map_arg(Value::Top, opt);
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    if let Value::Atom(AtomKind::Tag(t), _, _) = result.collapse() {
        assert_eq!(t.trim_start_matches('#'), "none",
            "option.map(#none) should return #none: {:?}", result);
    } else {
        panic!("Expected #none tag, got {:?}", result);
    }
}

#[test]
fn result_map_ok() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());

    // result.map(identity, Ok(99)) → Ok(99)
    let res = make_ok(make_int(99));
    let morph = get_match_builtin("result.map", &oo);
    let arg = make_map_arg(Value::Top, res);
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    if let Value::Combo(ref c) = result.collapse() {
        assert!(c.get_field("%val").is_some(), "result.map(Ok(99)) should return Ok: {:?}", result);
        assert!(c.get_field("%cause").is_none(), "result.map(Ok) should not have %cause");
    } else {
        panic!("Expected Ok combo, got {:?}", result);
    }
}

#[test]
fn result_map_err_passthrough() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());

    // result.map(anything, Err(#oops)) → Err(#oops) unchanged
    let err_cause = Value::Atom(AtomKind::Tag("oops".to_string()), EffectTag::Pure, None);
    let res = make_err(err_cause);
    let morph = get_match_builtin("result.map", &oo);
    let arg = make_map_arg(Value::Top, res);
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    if let Value::Combo(ref c) = result.collapse() {
        assert!(c.get_field("%cause").is_some(), "result.map(Err) should preserve %cause: {:?}", result);
        assert!(c.get_field("%val").is_none(), "result.map(Err) should not have %val");
    } else {
        panic!("Expected Err combo, got {:?}", result);
    }
}

#[test]
fn result_map_err_maps_cause() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());

    // result.map_err(identity, Err(#oops)) → Err(#oops) (identity applied to cause)
    let err_cause = Value::Atom(AtomKind::Tag("oops".to_string()), EffectTag::Pure, None);
    let res = make_err(err_cause);
    let morph = get_match_builtin("result.map_err", &oo);
    let arg = make_map_arg(Value::Top, res);
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    if let Value::Combo(ref c) = result.collapse() {
        assert!(c.get_field("%cause").is_some(), "result.map_err(Err) should have %cause: {:?}", result);
    } else {
        panic!("Expected Err combo, got {:?}", result);
    }
}

#[test]
fn option_fmap_accessible_from_type() {
    // @option in root should have %fmap field
    let oo = Ouroboros::new_in_memory();
    let root = oo.root_with_system();
    let opt_type = root.get_field("@option").expect("@option should exist");
    if let Value::Combo(ref c) = opt_type {
        assert!(c.get_field("%fmap").is_some(),
            "@option should have %fmap field after Phase 15");
    } else {
        panic!("@option should be a Combo");
    }
}
```

---

## 驗收條件

1. `cargo test -p nlang-interpreter 2>&1 | grep -E "FAILED|passed"` — 全部通過（含新測試）
2. `refine_cycle_ab_ba_rejected` — B→A 被拒絕，錯誤含 "cycle"
3. `refine_same_source_twice_no_cycle` — fan-out 允許
4. `eval_context_sets_timeout_deadline` — `deadline.is_some()` 且在未來
5. `eval_context_new_has_no_timeout` — `EvalContext::new()` 仍為 None
6. `option_map_some` / `option_map_none` / `result_map_ok` / `result_map_err_passthrough` / `result_map_err_maps_cause` / `option_fmap_accessible_from_type` 全部通過
7. `SEED_OPTION` 和 `SEED_RESULT` 已更新為新值，且 `seed_caids_are_stable` 通過
8. `cargo clippy -p nlang-interpreter -- -D warnings` — 無警告

---

## 不在本 Phase 的工作

- **量子距離 `approximate_phase_diff`** — 改變現有 Combo 合併行為，需獨立 Phase 分析測試影響
- **`%bind` (flatMap/chain)** — 自然延伸，`option.and_then` / `result.and_then`；留 Phase 16+
- **`@list { %fmap: list.map }`** — list 已有 `list.map` builtin，加 `%fmap` 欄位留後
- **Equivalence map 合成** — SPEC_17 依賴，P3
