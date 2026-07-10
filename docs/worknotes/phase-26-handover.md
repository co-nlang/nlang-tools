# Phase 26 交接文件

> 狀態：待實作  
> 前置：Phase 25 完成（~291 tests passing）  
> 目標：Option / Result 進階組合子 + 補全 @option/@result 型別態射

---

## 概覽

| 任務 | 位置 | 內容 |
|:-----|:-----|:-----|
| Task 1 | `crates/interpreter/src/builtins/engine.rs` | 5 個新 builtins |
| Task 2 | `crates/interpreter/src/lib.rs` | 補全 @option/@result 的 `/morphism` 欄位 |
| Task 3 | `crates/interpreter/src/genesis.rs` | 重跑 seed test，更新 SEED_OPTION、SEED_RESULT |
| Tests  | `crates/interpreter/tests/option_result_p26_test.rs`（新建） | ~14 個測試 |

預期完成後：**~291 + 14 ≈ 305 tests**

---

## 新 builtins 語義總表

| builtin | 輸入 | 輸出 | 語義 |
|:--------|:-----|:-----|:-----|
| `option.zip` | `{0: opt_a, 1: opt_b}` | `Option<{0:a, 1:b}>` | 兩個都是 Some 才合併；任一 None → None |
| `option.flatten` | `Option<Option<T>>` | `Option<T>` | 攤平一層巢狀 Option |
| `result.and` | `{0: result_b, 1: result_a}` | `Result` | result_a 是 Ok 則回傳 result_b；否則回傳 result_a（Err） |
| `result.or` | `{0: result_b, 1: result_a}` | `Result` | result_a 是 Ok 則回傳 result_a；否則回傳 result_b |
| `result.flatten` | `Result<Result<T,E>,E>` | `Result<T,E>` | 攤平一層巢狀 Result |

---

## Task 1：新 builtins（`engine.rs`）

在 `option.expect` 之後（約 523 行末）加入：

```rust
    // ── Phase 26: option/result advanced combinators ───────────────

    // option.zip: {0: opt_a, 1: opt_b} → Option<{0:a, 1:b}>
    m.insert("option.zip".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(va), Some(vb)) = (c.get_field("0"), c.get_field("1")) {
                let oa = oo.force(va.clone(), ctx);
                let ob = oo.force(vb.clone(), ctx);
                let is_none = |v: &Value| matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none");
                let inner = |v: &Value| -> Option<Value> {
                    match v { Value::Combo(ref cv) => cv.get_field("%val").cloned(), _ => None }
                };
                if is_none(&oa) || is_none(&ob) {
                    return Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
                }
                if let (Some(a), Some(b)) = (inner(&oa), inner(&ob)) {
                    let mut pair = IndexMap::new();
                    pair.insert("0".to_string(), a);
                    pair.insert("1".to_string(), b);
                    let pair_val = Value::Combo(ComboVal::new(pair, true, IndexMap::new(), EffectTag::Pure, vec![]));
                    let mut res = IndexMap::new();
                    res.insert("%val".to_string(), pair_val);
                    return Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]));
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // option.flatten: Option<Option<T>> → Option<T>
    // Single-arg: arg is the outer Option directly, or {0: outer_opt}
    m.insert("option.flatten".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let outer = oo.force(v, ctx);
        let none = Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
        match &outer {
            Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none" => none,
            Value::Combo(ref cv) => {
                match cv.get_field("%val") {
                    None => Value::Top,
                    Some(inner) => {
                        let inner_forced = oo.force(inner.clone(), ctx);
                        match &inner_forced {
                            Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none" => none,
                            Value::Combo(ref icv) if icv.get_field("%val").is_some() => inner_forced.clone(),
                            _ => Value::Top,
                        }
                    }
                }
            }
            _ => Value::Top,
        }
    }) as Arc<BuiltinFn>);

    // result.and: {0: result_b, 1: result_a}
    // If result_a is Ok → return result_b; if result_a is Err → return result_a
    m.insert("result.and".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vb), Some(va)) = (c.get_field("0"), c.get_field("1")) {
                let ra = oo.force(va.clone(), ctx);
                return match &ra {
                    Value::Combo(ref cv) if cv.get_field("%val").is_some() => vb.clone(),
                    Value::Combo(ref cv) if cv.get_field("%cause").is_some() => ra.clone(),
                    _ => Value::Top,
                };
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // result.or: {0: result_b, 1: result_a}
    // If result_a is Ok → return result_a; if result_a is Err → return result_b
    m.insert("result.or".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vb), Some(va)) = (c.get_field("0"), c.get_field("1")) {
                let ra = oo.force(va.clone(), ctx);
                return match &ra {
                    Value::Combo(ref cv) if cv.get_field("%val").is_some() => ra.clone(),
                    Value::Combo(ref cv) if cv.get_field("%cause").is_some() => vb.clone(),
                    _ => Value::Top,
                };
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // result.flatten: Result<Result<T,E>,E> → Result<T,E>
    // Single-arg or {0: outer_result}
    m.insert("result.flatten".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let outer = oo.force(v, ctx);
        match &outer {
            Value::Combo(ref cv) => {
                if cv.get_field("%cause").is_some() {
                    return outer.clone();
                }
                if let Some(inner) = cv.get_field("%val") {
                    let inner_forced = oo.force(inner.clone(), ctx);
                    match &inner_forced {
                        Value::Combo(ref icv) if icv.get_field("%val").is_some() || icv.get_field("%cause").is_some() => {
                            return inner_forced.clone();
                        }
                        _ => return Value::Top,
                    }
                }
                Value::Top
            }
            _ => Value::Top,
        }
    }) as Arc<BuiltinFn>);
```

### 注意事項

- `result.and` 的 arg 順序：`{0: result_b, 1: result_a}` — result_b 是「條件滿足時回傳的值」，result_a 是「被判斷的主體」。與 Rust 的 `result_a.and(result_b)` 語義相同，只是 nlang 慣例把函數參數放 "0"。
- `option.flatten` / `result.flatten` 用單一 arg 模式（`c.get_field("0").cloned().unwrap_or(arg.clone())`），與 `option.map`/`result.map` 的雙 arg 模式不同。呼叫方式：`option.flatten some_opt` 而不是 `option.flatten {0: fn, 1: opt}`。
- `option.zip` 只有在 outer 值確認是 None 或 `{%val:...}` 時才有效；其他情況回傳 Top（不觸發 panic）。

---

## Task 2：補全 @option/@result 態射（`lib.rs`）

**目的**：讓使用者可以用 `@option/and_then` 語法呼叫，與 `~%List/map` 一致。

目前 @option 只有 `%fmap`；@result 只有 `%fmap` 和 `%map_err`。

### 2A：更新 @option 區塊

**找到**（約 222–247 行）：
```rust
        option_fields.insert(
            "%fmap".to_string(),
            Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(), Value::Atom(AtomKind::Str("option.map".to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])),
        );
        fields.insert(
            "@option".to_string(),
```

**在 `fields.insert("@option"...)` 之前插入**（加在 `%fmap` 之後）：

```rust
        let opt_morphisms = vec![
            ("/and_then",  "option.and_then"),
            ("/or",        "option.or"),
            ("/unwrap_or", "option.unwrap_or"),
            ("/filter",    "option.filter"),
            ("/expect",    "option.expect"),
            ("/zip",       "option.zip"),
            ("/flatten",   "option.flatten"),
        ];
        for (n, b) in opt_morphisms {
            option_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])));
        }
```

### 2B：更新 @result 區塊

**找到**（約 274–284 行）：
```rust
        result_fields.insert(
            "%map_err".to_string(),
            Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(), Value::Atom(AtomKind::Str("result.map_err".to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])),
        );
        fields.insert(
            "@result".to_string(),
```

**在 `fields.insert("@result"...)` 之前插入**：

```rust
        let res_morphisms = vec![
            ("/and_then", "result.and_then"),
            ("/unwrap",   "result.unwrap"),
            ("/expect",   "result.expect"),
            ("/and",      "result.and"),
            ("/or",       "result.or"),
            ("/flatten",  "result.flatten"),
        ];
        for (n, b) in res_morphisms {
            result_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])));
        }
```

---

## Task 3：更新 genesis.rs（重跑 seed test）

`@option` 和 `@result` 結構改變，CAID 必須重新計算。

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture 2>&1
```

輸出的 `UPDATE:` 行中找到 `@option` 和 `@result` 對應的新 CAID，更新 `genesis.rs` 中的：
```rust
pub const SEED_OPTION: &str = "hash:sha256:v1:...";  // ← 更新
pub const SEED_RESULT: &str = "hash:sha256:v1:...";  // ← 更新
```

---

## 測試（`tests/option_result_p26_test.rs`）

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal, BottomDetail};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn some(v: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("%val".to_string(), v);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn none() -> Value {
    Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None)
}
fn ok(v: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("%val".to_string(), v);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn err(cause: &str) -> Value {
    let mut m = IndexMap::new();
    m.insert("%cause".to_string(), Value::Atom(AtomKind::Str(cause.to_string()), EffectTag::Pure, None));
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn int(n: i64) -> Value {
    Value::Atom(AtomKind::Int(n.into()), EffectTag::Pure, None)
}
fn make_combo2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a);
    m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn is_none(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none")
}
fn is_some(v: &Value) -> bool {
    matches!(v, Value::Combo(ref cv) if cv.get_field("%val").is_some())
}
fn is_ok(v: &Value) -> bool {
    matches!(v, Value::Combo(ref cv) if cv.get_field("%val").is_some() && cv.get_field("%cause").is_none())
}
fn is_err(v: &Value) -> bool {
    matches!(v, Value::Combo(ref cv) if cv.get_field("%cause").is_some())
}
fn unwrap_val(v: &Value) -> &Value {
    match v { Value::Combo(ref cv) => cv.get_field("%val").unwrap(), _ => panic!("not Some/Ok") }
}

// ── option.zip ─────────────────────────────────────────────────────

#[test]
fn test_option_zip_both_some() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "option.zip", make_combo2(some(int(1)), some(int(2))));
    assert!(is_some(&r));
    if let Value::Combo(ref pair) = *unwrap_val(&r) {
        assert_eq!(pair.get_field("0").unwrap().to_string_plain(), "1");
        assert_eq!(pair.get_field("1").unwrap().to_string_plain(), "2");
    } else { panic!("inner should be Combo pair"); }
}

#[test]
fn test_option_zip_first_none() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "option.zip", make_combo2(none(), some(int(2))));
    assert!(is_none(&r));
}

#[test]
fn test_option_zip_second_none() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "option.zip", make_combo2(some(int(1)), none()));
    assert!(is_none(&r));
}

// ── option.flatten ─────────────────────────────────────────────────

#[test]
fn test_option_flatten_nested_some() {
    // Some(Some(42)) → Some(42)
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let nested = some(some(int(42)));
    let r = call(&oo, &mut ctx, "option.flatten", nested);
    assert!(is_some(&r));
    assert_eq!(unwrap_val(&r).to_string_plain(), "42");
}

#[test]
fn test_option_flatten_outer_none() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "option.flatten", none());
    assert!(is_none(&r));
}

#[test]
fn test_option_flatten_inner_none() {
    // Some(None) → None
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "option.flatten", some(none()));
    assert!(is_none(&r));
}

// ── result.and ─────────────────────────────────────────────────────

#[test]
fn test_result_and_both_ok_returns_second() {
    // and(ok(2), ok(1)) → ok(2)  [result_a=ok(1), result_b=ok(2)]
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "result.and", make_combo2(ok(int(2)), ok(int(1))));
    assert!(is_ok(&r));
    assert_eq!(unwrap_val(&r).to_string_plain(), "2");
}

#[test]
fn test_result_and_first_err_propagates() {
    // and(ok(2), err("boom")) → err("boom")  [result_a=err, result_b=ok]
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "result.and", make_combo2(ok(int(2)), err("boom")));
    assert!(is_err(&r));
}

// ── result.or ──────────────────────────────────────────────────────

#[test]
fn test_result_or_ok_returns_self() {
    // or(err("fallback"), ok(1)) → ok(1)
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "result.or", make_combo2(err("fallback"), ok(int(1))));
    assert!(is_ok(&r));
    assert_eq!(unwrap_val(&r).to_string_plain(), "1");
}

#[test]
fn test_result_or_err_uses_fallback() {
    // or(ok(99), err("boom")) → ok(99)
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "result.or", make_combo2(ok(int(99)), err("boom")));
    assert!(is_ok(&r));
    assert_eq!(unwrap_val(&r).to_string_plain(), "99");
}

// ── result.flatten ─────────────────────────────────────────────────

#[test]
fn test_result_flatten_ok_ok() {
    // Ok(Ok(42)) → Ok(42)
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "result.flatten", ok(ok(int(42))));
    assert!(is_ok(&r));
    assert_eq!(unwrap_val(&r).to_string_plain(), "42");
}

#[test]
fn test_result_flatten_outer_err() {
    // Err("outer") → Err("outer")
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "result.flatten", err("outer"));
    assert!(is_err(&r));
}

#[test]
fn test_result_flatten_ok_err() {
    // Ok(Err("inner")) → Err("inner")
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "result.flatten", ok(err("inner")));
    assert!(is_err(&r));
}
```

### 加入 `Cargo.toml` 的 test 條目

```toml
[[test]]
name = "option_result_p26_test"
path = "tests/option_result_p26_test.rs"
```

---

## 驗證步驟

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools

# 1. 編譯確認
cargo build --manifest-path crates/interpreter/Cargo.toml 2>&1

# 2. 新測試
cargo test --manifest-path crates/interpreter/Cargo.toml option_result_p26_test -- --nocapture

# 3. 種子更新後穩定性
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture

# 4. 全套不退步
cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：~305 tests, 0 failed
```
