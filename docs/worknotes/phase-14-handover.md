# Phase 14 Handover: `eml %branch` + `~%Config` eval_context + `cond.match` 真實模式匹配

**Date:** 2026-05-24  
**Status:** Ready for implementation  
**Depends on:** Phase 13 (complete)  
**Spec refs:** SPEC_09 §3.4, SPEC_09 §6, SPEC_09 §4

---

## 目標

三個獨立任務，全部小到中型：

1. **`math.eml` + `%branch`** — 完成 Phase 13 遺留項目，讓 `eml(x,y) { %branch: n }` 正確選取 Riemann 面
2. **`~%Config` → `Ouroboros::eval_context()`** — 連接 genesis 配置到 runtime，建立單一 EvalContext 構建入口
3. **`cond.match` 真實模式匹配** — 把 stub 補完成正式的結構式模式匹配，補完控制流三角

---

## 任務一：`math.eml` + `%branch`

### 背景

`eml(x, y) = e^x − ln(y)` 。`ln(y)` 有分支切割：`ln_n(y) = ln(y) + 2πni`。
因此 `eml_n(x, y) = e^x − ln_n(y) = (e^x − ln(y)) − 2πni`。

Phase 13 實作了 `math.ln` 和 `math.sqrt` 的 `%branch`，但 `math.eml` 的 `compute_ln` 是直接呼叫私有函式，沒有走 `%branch` 路徑。

### 改動位置：`crates/interpreter/src/builtins/math.rs`

**原始 `math.eml` closure（約 326–350 行）：**
```rust
m.insert("math.eml".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
            let x = oo.force(vx.clone(), ctx).collapse().clone();
            let y = oo.force(vy.clone(), ctx).collapse().clone();
            // eml(x, y) = exp(x) - ln(y)
            let exp_x = compute_exp(&x);
            let ln_y = compute_ln(&y);
            return match (exp_x, ln_y) {
                (Some(ex), Some(ly)) => {
                    // sub(ex, ly) — inline the Complex subtraction
                    let eff = ex.effect().max(ly.effect());
                    match (ex, ly) {
                        (Value::Atom(AtomKind::Complex(r1, i1), _, _), Value::Atom(AtomKind::Complex(r2, i2), _, _)) =>
                            Value::Atom(AtomKind::Complex(r1 - r2, i1 - i2), eff, None),
                        _ => BottomCause::Conflict.into(),
                    }
                }
                _ => blur_singularity("#eml_singularity", &*ctx),
            };
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);
```

**改後：**
```rust
m.insert("math.eml".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let branch: i64 = if let Value::Combo(ref c) = arg {
        c.get_field("%branch")
            .and_then(|v| if let Value::Atom(AtomKind::Int(n), _, _) = v { n.to_i64() } else { None })
            .unwrap_or(0)
    } else { 0 };
    if let Value::Combo(ref c) = arg {
        if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
            let x = oo.force(vx.clone(), ctx).collapse().clone();
            let y = oo.force(vy.clone(), ctx).collapse().clone();
            let exp_x = compute_exp(&x);
            let ln_y = compute_ln(&y);
            return match (exp_x, ln_y) {
                (Some(ex), Some(ly)) => {
                    let eff = ex.effect().max(ly.effect());
                    let base = match (ex, ly) {
                        (Value::Atom(AtomKind::Complex(r1, i1), _, _), Value::Atom(AtomKind::Complex(r2, i2), _, _)) =>
                            Value::Atom(AtomKind::Complex(r1 - r2, i1 - i2), eff, None),
                        _ => return BottomCause::Conflict.into(),
                    };
                    // %branch: eml_n(x,y) = base - 2πni
                    if branch != 0 {
                        let offset_imag = 2.0 * std::f64::consts::PI * (branch as f64);
                        match base {
                            Value::Atom(AtomKind::Complex(r, i), e, rank) =>
                                Value::Atom(AtomKind::Complex(r, i - offset_imag), e, rank),
                            other => other,
                        }
                    } else {
                        base
                    }
                }
                _ => blur_singularity("#eml_singularity", &*ctx),
            };
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);
```

**變更摘要：**
- 在 closure 最上方插入 `%branch` 提取（與 `math.ln` 完全相同的模式）
- `base` 計算不變
- `branch != 0` 時把 `base` 的虛部減去 `2πni`（`ln_n` 增加 `2πni`，因此 `eml_n = base - 2πni`）

### 測試：新增至 `crates/interpreter/tests/math_branch_test.rs`

檔案若不存在則新建；存在則追加。

```rust
#[test]
fn eml_branch_0_principal() {
    // eml(0, 1) = exp(0) - ln(1) = 1 - 0 = Complex(1, 0)
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let arg = build_combo(vec![
        ("0", Value::Atom(AtomKind::Int(0.into()), EffectTag::Pure, None)),
        ("1", Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None)),
        ("%branch", Value::Atom(AtomKind::Int(0.into()), EffectTag::Pure, None)),
    ]);
    let result = oo.force(call_builtin("math.eml", arg, &oo, &mut ctx), &mut ctx);
    if let Value::Atom(AtomKind::Complex(r, i), _, _) = result.collapse() {
        assert!((r - 1.0).abs() < 1e-10);
        assert!(i.abs() < 1e-10);
    } else { panic!("Expected Complex, got {:?}", result); }
}

#[test]
fn eml_branch_1_shifts_imag() {
    // eml(0, 1) { %branch: 1 } = 1 - 2πi
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let arg = build_combo(vec![
        ("0", Value::Atom(AtomKind::Int(0.into()), EffectTag::Pure, None)),
        ("1", Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None)),
        ("%branch", Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None)),
    ]);
    let result = oo.force(call_builtin("math.eml", arg, &oo, &mut ctx), &mut ctx);
    let two_pi = 2.0 * std::f64::consts::PI;
    if let Value::Atom(AtomKind::Complex(r, i), _, _) = result.collapse() {
        assert!((r - 1.0).abs() < 1e-10);
        assert!((i + two_pi).abs() < 1e-10, "Expected -2π, got {}", i);
    } else { panic!("Expected Complex, got {:?}", result); }
}
```

**注意：** 如果 `math_branch_test.rs` 尚未有 `build_combo` / `call_builtin` 輔助函式，參考 Phase 13 測試中的寫法（直接構造 `Value::Combo` 並呼叫 `oo.apply_morphism`）。

---

## 任務二：`~%Config` → `Ouroboros::eval_context()`

### 背景

`EvalContext::new(root)` 的 `fuel`, `max_branches`, `max_unification_depth`, `max_pattern_nodes`, `strategy` 全是硬編碼。`~%Config` 雖然在 `root_with_system()` 中定義，但 runtime 完全沒有讀取它。Phase 14 在 `Ouroboros` 加一個 `eval_context()` 方法，作為唯一構建 EvalContext 的入口，讓 `~%Config` 真正連接到 runtime。

### 改動位置：`crates/interpreter/src/lib.rs`

在 `Ouroboros` impl block 中加入（放在 `root_with_system()` 之後）：

```rust
pub fn eval_context(&self) -> EvalContext {
    let sys_root = self.root_with_system();
    let mut ctx = EvalContext::new(sys_root.clone());
    // Read ~%Config defaults
    if let Some(Value::Combo(ref cfg)) = sys_root.get_field("~%Config").cloned() {
        if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("%fuel").cloned() {
            if let Some(f) = n.to_u64() { ctx.fuel = f; }
        }
        if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("%max_branches").cloned() {
            if let Some(v) = n.to_u64() { ctx.max_branches = v as usize; }
        }
        if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("%max_depth").cloned() {
            if let Some(v) = n.to_u64() { ctx.max_unification_depth = v as usize; }
        }
        if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("%max_pattern_nodes").cloned() {
            if let Some(v) = n.to_u64() { ctx.max_pattern_nodes = v as usize; }
        }
        if let Some(Value::Atom(AtomKind::Tag(s), _, _)) = cfg.get_field("%strategy").cloned() {
            ctx.strategy = match s.trim_start_matches('#') {
                "strict" => ObservationStrategy::Strict,
                "approximate" => ObservationStrategy::Approximate,
                _ => ObservationStrategy::Blur,
            };
        }
    }
    ctx
}
```

**依賴確認：** `BigInt` 的 `to_u64()` 方法在 `num_bigint` 中需要 `use num_traits::ToPrimitive;`。若 `lib.rs` 頂端尚無此 import，加入：
```rust
use num_traits::ToPrimitive;
```
（`num-traits` 已在 `Cargo.toml` 中，Phase 13 已使用）

### 改動位置：`crates/interpreter/src/unify.rs`，第 80 行

```rust
// 原始
let mut ctx = EvalContext::new(self.root_with_system());

// 改為
let mut ctx = self.eval_context();
```

### 改動位置：`crates/oo/src/main.rs`

找到所有 `EvalContext::new(engine.root_with_system())` 或等效呼叫，替換為 `engine.eval_context()`。

搜尋指令：
```bash
grep -n "EvalContext::new\|root_with_system" crates/oo/src/main.rs
```

預期有 2–3 個 `Universe::new(None, engine.root_with_system())` 呼叫不需更動（Universe 的 root 是作用域，不是 EvalContext），以及可能有 1–2 個直接建立 EvalContext 的地方需要替換。

**判斷原則：**
- `Universe::new(None, engine.root_with_system())` → **不動**（這設定 universe 的作用域根，不是 EvalContext）
- `EvalContext::new(engine.root_with_system())` 或類似 → **改為 `engine.eval_context()`**

### 測試：新增至 `crates/interpreter/tests/genesis_test.rs`

```rust
#[test]
fn eval_context_reads_config_fuel() {
    let oo = Ouroboros::new_in_memory();
    let ctx = oo.eval_context();
    // ~%Config has %fuel: 10000
    assert_eq!(ctx.fuel, 10000, "eval_context() should read fuel from ~%Config");
}

#[test]
fn eval_context_reads_config_max_branches() {
    let oo = Ouroboros::new_in_memory();
    let ctx = oo.eval_context();
    assert_eq!(ctx.max_branches, 64, "eval_context() should read max_branches from ~%Config");
}

#[test]
fn eval_context_reads_config_strategy() {
    let oo = Ouroboros::new_in_memory();
    let ctx = oo.eval_context();
    assert!(matches!(ctx.strategy, ObservationStrategy::Blur),
        "~%Config %strategy: #blur should map to ObservationStrategy::Blur");
}
```

**`use` 清單確認：** `genesis_test.rs` 已有 `use nlang_interpreter::{Ouroboros, EvalContext};`，加入：
```rust
use nlang_interpreter::ObservationStrategy;
```

---

## 任務三：`cond.match` 真實模式匹配

### 背景

`cond.match` 目前是 stub：`|arg, _oo, _ctx| { arg }`，直接回傳輸入，沒有任何匹配邏輯。

`/if` 和 `/cond` 已完整，但 `/match` 是控制流三角的最後一塊。真實語意：

```
match(value, patterns_list)
```

- `patterns_list` 是 positional-indexed Combo（list），每個元素是 `{ 0: pattern, 1: action }`
- 依序嘗試 `unify_internal(value, pattern_i)`
- 若結果不是 `Bottom` → `apply_morphism(action_i, unified_result)`，立即返回
- 若全部不匹配 → `Top`（無信息，呼叫者可選擇繼續）

這與 Haskell `case`/ML `match` 的語意一致，但使用 n/lang 的格序統一而非結構相等。

**呼叫慣例（與 `cond.cond` 一致）：**
```
arg = { 0: <value_to_match>, 1: <patterns_list> }
patterns_list = { %kind: #list, 0: {0: pat1, 1: action1}, 1: {0: pat2, 1: action2}, ... }
```

### 改動位置：`crates/interpreter/src/builtins/cond.rs`

**原始（第 48 行）：**
```rust
m.insert("cond.match".to_string(), Arc::new(|arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| { arg }));
```

**替換為：**
```rust
m.insert("cond.match".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(val_v), Some(pats_v)) = (c.get_field("0"), c.get_field("1")) {
            let val = oo.force(val_v.clone(), ctx);
            let pats = oo.force(pats_v.clone(), ctx);
            if let Value::Combo(ref pc) = pats.collapse() {
                let mut i = 0usize;
                while let Some(pair_v) = pc.get_field(&i.to_string()) {
                    let pair = oo.force(pair_v.clone(), ctx);
                    if let Value::Combo(ref pair_c) = pair.collapse() {
                        if let (Some(pat), Some(action)) = (pair_c.get_field("0"), pair_c.get_field("1")) {
                            let unified = oo.unify_internal(val.clone(), pat.clone(), ctx);
                            if !matches!(unified, Value::Bottom(_)) {
                                return oo.apply_morphism(action.clone(), unified, ctx);
                            }
                        }
                    }
                    i += 1;
                }
            }
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);
```

**Signature 還原：** 把 `_oo: &Ouroboros, _ctx: &mut EvalContext` 改為 `oo: &Ouroboros, ctx: &mut EvalContext`（移除 `_` 前綴）。

### 測試：新增 `crates/interpreter/tests/cond_match_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, ComboVal, EffectTag};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use std::sync::Arc;

fn make_atom_tag(t: &str) -> Value {
    Value::Atom(AtomKind::Tag(t.to_string()), EffectTag::Pure, None)
}

fn make_atom_int(n: i64) -> Value {
    Value::Atom(AtomKind::Int(n.into()), EffectTag::Pure, None)
}

fn make_list(items: Vec<Value>) -> Value {
    let mut fields = IndexMap::new();
    for (i, v) in items.into_iter().enumerate() {
        fields.insert(i.to_string(), v);
    }
    fields.insert("%kind".to_string(), make_atom_tag("list"));
    Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_pair(pat: Value, action: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), pat);
    f.insert("1".to_string(), action);
    Value::Combo(ComboVal::new(f, true, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_match_arg(value: Value, patterns: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), value);
    f.insert("1".to_string(), patterns);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

// Identity morphism: always returns its argument unchanged
fn identity_morphism() -> Value {
    // A closure-like morphism via Top (identity in n/lang: force(Top) = Top)
    // Use a Tag morphism that resolves via apply_morphism
    // Simplest: use a Combo with %morphism and %builtin pointing to identity
    // Actually, for test purposes, use a raw closure wrapped in Arc...
    // Better: use a Combo that when applied returns the arg
    // Simplest test: action is Top → apply_morphism(Top, arg) = arg (no-op)
    Value::Top
}

#[test]
fn match_first_pattern_wins() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());

    // value = #foo
    // patterns = [ {pat: #foo, action: Top}, {pat: #bar, action: Top} ]
    // First pattern matches → returns unified value (#foo)
    let value = make_atom_tag("#foo");
    let pat1 = make_atom_tag("#foo");
    let pat2 = make_atom_tag("#bar");
    let patterns = make_list(vec![
        make_pair(pat1, Value::Top),
        make_pair(pat2, Value::Top),
    ]);
    let arg = make_match_arg(value, patterns);

    let result = oo.force(
        oo.apply_morphism(
            // call via ~%Cond /match
            {
                let sys = oo.root_with_system();
                if let Some(Value::Combo(ref cond_combo)) = sys.get_field("~%Cond") {
                    if let Some(match_morph) = cond_combo.get_field("/match") {
                        match_morph.clone()
                    } else { panic!("/match not found in ~%Cond") }
                } else { panic!("~%Cond not in root") }
            },
            arg,
            &mut ctx,
        ),
        &mut ctx,
    );
    // unified = #foo (tag match), action = Top, apply_morphism(Top, #foo) = #foo
    assert_eq!(result.collapse().to_string_plain().trim_start_matches('#'), "foo",
        "First matching pattern should win: {:?}", result);
}

#[test]
fn match_skips_non_matching() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());

    // value = #baz; patterns = [{#foo, Top}, {#baz, Top}]
    // #foo doesn't match #baz (Bottom), #baz matches
    let value = make_atom_tag("#baz");
    let patterns = make_list(vec![
        make_pair(make_atom_tag("#foo"), Value::Top),
        make_pair(make_atom_tag("#baz"), Value::Top),
    ]);
    let arg = make_match_arg(value, patterns);

    let sys = oo.root_with_system();
    let match_morph = sys.get_field("~%Cond")
        .and_then(|v| if let Value::Combo(ref c) = v { c.get_field("/match").cloned() } else { None })
        .expect("/match in ~%Cond");

    let result = oo.force(oo.apply_morphism(match_morph, arg, &mut ctx), &mut ctx);
    assert_eq!(result.collapse().to_string_plain().trim_start_matches('#'), "baz",
        "Should skip #foo and match #baz: {:?}", result);
}

#[test]
fn match_no_pattern_returns_top() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());

    // value = #qux; patterns = [{#foo, Top}] — no match
    let value = make_atom_tag("#qux");
    let patterns = make_list(vec![
        make_pair(make_atom_tag("#foo"), Value::Top),
    ]);
    let arg = make_match_arg(value, patterns);

    let sys = oo.root_with_system();
    let match_morph = sys.get_field("~%Cond")
        .and_then(|v| if let Value::Combo(ref c) = v { c.get_field("/match").cloned() } else { None })
        .expect("/match in ~%Cond");

    let result = oo.force(oo.apply_morphism(match_morph, arg, &mut ctx), &mut ctx);
    assert!(result.collapse().is_top(),
        "No match should return Top: {:?}", result);
}

#[test]
fn match_top_pattern_catches_all() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());

    // Top pattern unifies with everything (Top ∧ X = X, not Bottom)
    let value = make_atom_int(42);
    let patterns = make_list(vec![
        make_pair(Value::Top, Value::Top),  // catchall
    ]);
    let arg = make_match_arg(value.clone(), patterns);

    let sys = oo.root_with_system();
    let match_morph = sys.get_field("~%Cond")
        .and_then(|v| if let Value::Combo(ref c) = v { c.get_field("/match").cloned() } else { None })
        .expect("/match in ~%Cond");

    let result = oo.force(oo.apply_morphism(match_morph, arg, &mut ctx), &mut ctx);
    // unify(42, Top) = 42; apply_morphism(Top, 42) = 42
    if let Value::Atom(AtomKind::Int(n), _, _) = result.collapse() {
        assert_eq!(n.to_string(), "42", "Top pattern should match 42: {:?}", result);
    } else {
        panic!("Expected Int(42), got {:?}", result);
    }
}
```

**注意：** `ComboVal::get_field` 若回傳 `Option<&Value>` 而非 `Option<Value>`，在 `sys.get_field("~%Cond")` 那幾行需要 `.cloned()`。依照實際 API 調整（參考 Phase 13 genesis_test 的寫法）。

---

## Cargo.toml 確認

無新依賴。`num-traits` 已在 `crates/interpreter/Cargo.toml`。

---

## 驗收條件

1. `cargo test -p nlang-interpreter 2>&1 | grep -E "FAILED|passed"` — 全部通過
2. `eml_branch_0_principal` 與 `eml_branch_1_shifts_imag` 通過
3. `eval_context_reads_config_fuel` / `_max_branches` / `_strategy` 通過
4. `match_first_pattern_wins` / `match_skips_non_matching` / `match_no_pattern_returns_top` / `match_top_pattern_catches_all` 通過
5. `grep "bootstrap_exempt = true" crates/interpreter/src/universe.rs` — 無輸出（Phase 10 確認）
6. `cargo clippy -p nlang-interpreter -- -D warnings` — 無警告

---

## 不在本 Phase 的工作

- **`%timeout` → `timeout_deadline`** — 需要 `SystemTime::now()` 才能算截止時刻；留 Phase 15+
- **`%branch` 在 Universe 作用域動態更新** — 使用者通過 #refine 修改 `~%Config` 後 `eval_context()` 應重新讀取；目前 `root_with_system()` 是靜態的
- **量子距離 `approximate_phase_diff` 實作** — Phase 4 TODO in `unify.rs`；需要 sketch cosine similarity；留 Phase 15+
- **`cond.match` + 變數綁定** — 例如 `{ x: Top }` 作為 pattern 捕捉 `x` 欄位；目前 unified 就是完整 unify 結果，未做特殊捕捉語意；留後
