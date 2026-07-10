# Phase 36 Handover：~%Env + ~%Process

> 日期：2026-05-25  
> 實作範圍：~%Env（env.get/args/cwd，3 態射）+ ~%Process（process.exit/pid，2 態射）  
> 預期測試：~407 → ~418（新增 ~11 個測試）

---

## 0. 摘要

新增兩個系統模組：
- **~%Env**：讀取環境變數、命令列參數、當前目錄（全部 IO effect）
- **~%Process**：終止進程、取得 PID（全部 IO effect）

全部 5 個態射均使用 `EffectTag::IO`，沿用 ~%Io 的模式。

---

## 1. 新增 `env.rs`

**建立** `crates/interpreter/src/builtins/env.rs`：

```rust
use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use num_bigint::BigInt;

fn build_str_list(items: Vec<String>) -> Value {
    let mut data = IndexMap::new();
    data.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::IO, None));
    for (i, s) in items.into_iter().enumerate() {
        data.insert(i.to_string(), Value::Atom(AtomKind::Str(s), EffectTag::IO, None));
    }
    Value::Combo(ComboVal::new(data, false, IndexMap::new(), EffectTag::IO, vec![]))
}

pub fn register_env_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // env.get: {0: name_str} → Str(IO) | #none(IO)
    m.insert("env.get".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        if let Value::Atom(AtomKind::Str(name), _, _) = forced.collapse() {
            return match std::env::var(name.as_str()) {
                Ok(val) => Value::Atom(AtomKind::Str(val), EffectTag::IO, None),
                Err(_)  => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::IO, None),
            };
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // env.args: _ → list of Str(IO)  (includes argv[0])
    m.insert("env.args".to_string(), Arc::new(|_arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| {
        let args: Vec<String> = std::env::args().collect();
        build_str_list(args)
    }) as Arc<BuiltinFn>);

    // env.cwd: _ → Str(IO) | #none(IO)
    m.insert("env.cwd".to_string(), Arc::new(|_arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| {
        match std::env::current_dir() {
            Ok(path) => Value::Atom(AtomKind::Str(path.to_string_lossy().into_owned()), EffectTag::IO, None),
            Err(_)   => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::IO, None),
        }
    }) as Arc<BuiltinFn>);
}
```

**注意**：
- `build_str_list` 是模組內的 helper，不能用 list.rs 的 `build_list_value`（module-private）
- `forced.collapse()` 需要先儲存到變數，避免臨時值生命週期問題
- `num_bigint` 已加入 import，備用（build_str_list 用到 `i: usize`，不需要 BigInt；但如需回傳 Int 可用）

---

## 2. 新增 `process.rs`

**建立** `crates/interpreter/src/builtins/process.rs`：

```rust
use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

pub fn register_process_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // process.exit: {0: code_int} → !  (terminates the process, never returns)
    m.insert("process.exit".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        let code = match forced.collapse() {
            Value::Atom(AtomKind::Int(n), _, _) => n.to_i32().unwrap_or(0),
            _ => 0,
        };
        std::process::exit(code);
    }) as Arc<BuiltinFn>);

    // process.pid: _ → Int(IO)
    m.insert("process.pid".to_string(), Arc::new(|_arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| {
        Value::Atom(AtomKind::Int(BigInt::from(std::process::id())), EffectTag::IO, None)
    }) as Arc<BuiltinFn>);
}
```

**注意**：
- `std::process::exit(code)` 回傳型別是 `!`，可自動 coerce 到 `Value`，所以不需要額外的回傳語句
- `std::process::id()` 回傳 `u32`，用 `BigInt::from()` 包裝成 Int Atom

---

## 3. 修改 `builtins/mod.rs`

在現有 `mod io;`（第 12 行）後加入兩行：

```rust
mod env;
mod process;
```

在 `create_default_builtins()` 的 `io::register_io_builtins(&mut m);`（第 33 行）後加入：

```rust
env::register_env_builtins(&mut m);
process::register_process_builtins(&mut m);
```

最終 mod.rs 結構：
```
mod math; mod cond; mod string; mod list; mod disc; mod reflection;
mod engine; mod time; mod bytes; mod regex; mod json; mod io;
mod env; mod process;   // ← 新增

pub fn create_default_builtins() -> HashMap<String, Arc<BuiltinFn>> {
    let mut m = HashMap::new();
    math::register_math_builtins(&mut m);
    math::register_complex_builtins(&mut m);
    cond::register_cond_builtins(&mut m);
    string::register_string_builtins(&mut m);
    list::register_list_builtins(&mut m);
    disc::register_disc_builtins(&mut m);
    reflection::register_reflection_builtins(&mut m);
    engine::register_engine_builtins(&mut m);
    time::register_time_builtins(&mut m);
    bytes::register_bytes_builtins(&mut m);
    regex::register_regex_builtins(&mut m);
    json::register_json_builtins(&mut m);
    io::register_io_builtins(&mut m);
    env::register_env_builtins(&mut m);       // ← 新增
    process::register_process_builtins(&mut m);  // ← 新增
    m
}
```

---

## 4. 修改 `lib.rs`

在 `~%Io` 區塊（約第 373 行）之後、`~%Discovery` 區塊之前，插入：

```rust
        let mut env_fields = IndexMap::new();
        let env_morphisms = vec![
            ("/get",  "env.get"),
            ("/args", "env.args"),
            ("/cwd",  "env.cwd"),
        ];
        for (n, b) in env_morphisms {
            env_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(),  Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::IO, vec![])));
        }
        fields.insert("~%Env".to_string(), Value::Combo(ComboVal::new(env_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));

        let mut process_fields = IndexMap::new();
        let process_morphisms = vec![
            ("/exit", "process.exit"),
            ("/pid",  "process.pid"),
        ];
        for (n, b) in process_morphisms {
            process_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(),  Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::IO, vec![])));
        }
        fields.insert("~%Process".to_string(), Value::Combo(ComboVal::new(process_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
```

---

## 5. 修改 `genesis.rs`

在 `SEED_IO` 常數後加入：

```rust
pub const SEED_ENV:     &str = "hash:sha256:v1:PLACEHOLDER_ENV";
pub const SEED_PROCESS: &str = "hash:sha256:v1:PLACEHOLDER_PROCESS";
```

在 `all_seeds()` 的 `("~%Io", SEED_IO)` 後加入：

```rust
        ("~%Env",     SEED_ENV),
        ("~%Process", SEED_PROCESS),
```

**然後必須重新計算種子**：

```bash
cargo test seed_caids_are_stable -- --nocapture
```

將輸出中的 `UPDATE:` 行複製回 `genesis.rs`，替換 PLACEHOLDER 值。同時 SEED_LIST 和 SEED_MATH 也可能因 Phase 35 而需要更新（如果 Phase 35 executor 還沒做的話，一起做）。

---

## 6. 新增測試

### `crates/interpreter/tests/env_p36_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use nlang_interpreter::value::ComboVal;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn combo1(a: Value) -> Value {
    let mut m = IndexMap::new(); m.insert("0".to_string(), a);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn is_none(v: &Value) -> bool { matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none") }
fn as_str(v: &Value) -> &str { match v { Value::Atom(AtomKind::Str(s), _, _) => s, o => panic!("expected Str: {:?}", o) } }
fn is_list(v: &Value) -> bool {
    if let Value::Combo(c) = v {
        matches!(c.get_field("%kind"), Some(Value::Atom(AtomKind::Tag(t), _, _)) if t == "list")
    } else { false }
}

#[test]
fn test_env_get_existing() {
    // PATH is always set on any Unix/Windows system
    let oo = make_oo(); let mut ctx = oo.eval_context();
    // Use HOME or PATH — PATH is most reliable cross-platform
    std::env::set_var("NLANG_TEST_VAR_P36", "nlang_value_xyz");
    let r = call(&oo, &mut ctx, "env.get", combo1(str_val("NLANG_TEST_VAR_P36")));
    assert!(matches!(&r, Value::Atom(AtomKind::Str(_), EffectTag::IO, _)));
    assert_eq!(as_str(&r), "nlang_value_xyz");
}

#[test]
fn test_env_get_nonexistent_returns_none() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "env.get", combo1(str_val("NLANG_DEFINITELY_NOT_SET_ABCXYZ123")));
    assert!(is_none(&r));
    assert!(matches!(r, Value::Atom(_, EffectTag::IO, _)));
}

#[test]
fn test_env_args_returns_list() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "env.args", Value::Top);
    assert!(is_list(&r));
    assert!(matches!(r, Value::Combo(_)));
    // argv[0] is the test binary name — list has at least 1 element
    if let Value::Combo(ref c) = r {
        assert!(c.get_field("0").is_some(), "env.args must return at least argv[0]");
        assert!(matches!(c.get_field("0").unwrap(), Value::Atom(AtomKind::Str(_), EffectTag::IO, _)));
    }
}

#[test]
fn test_env_args_effect_is_io() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "env.args", Value::Top);
    assert!(matches!(r, Value::Combo(ref c) if c.effect() == EffectTag::IO));
}

#[test]
fn test_env_cwd_returns_str() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "env.cwd", Value::Top);
    assert!(matches!(r, Value::Atom(AtomKind::Str(_), EffectTag::IO, _)));
    let s = as_str(&r);
    assert!(!s.is_empty());
}

#[test]
fn test_env_cwd_effect_is_io() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "env.cwd", Value::Top);
    assert!(matches!(r, Value::Atom(_, EffectTag::IO, _)));
}
```

### `crates/interpreter/tests/process_p36_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}

#[test]
fn test_process_pid_returns_positive_int() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "process.pid", Value::Top);
    assert!(matches!(r, Value::Atom(AtomKind::Int(_), EffectTag::IO, _)));
    if let Value::Atom(AtomKind::Int(n), _, _) = r {
        assert!(n > BigInt::from(0i64), "PID must be positive");
    }
}

#[test]
fn test_process_pid_effect_is_io() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "process.pid", Value::Top);
    assert!(matches!(r, Value::Atom(_, EffectTag::IO, _)));
}

#[test]
fn test_process_exit_registered() {
    // DO NOT call process.exit — it would terminate the test runner.
    // Just verify the builtin is registered.
    let oo = make_oo();
    assert!(oo.builtin_registry.get("process.exit").is_some());
}

#[test]
fn test_process_pid_consistent() {
    // PID should be the same across two calls within the same process
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r1 = call(&oo, &mut ctx, "process.pid", Value::Top);
    let r2 = call(&oo, &mut ctx, "process.pid", Value::Top);
    match (r1, r2) {
        (Value::Atom(AtomKind::Int(n1), _, _), Value::Atom(AtomKind::Int(n2), _, _)) => assert_eq!(n1, n2),
        _ => panic!("expected two Int values"),
    }
}
```

---

## 7. 修改 `Cargo.toml`

在 `[[test]]` 區塊最後（math_p35_test 後）加入：

```toml
[[test]]
name = "env_p36_test"
path = "tests/env_p36_test.rs"

[[test]]
name = "process_p36_test"
path = "tests/process_p36_test.rs"
```

---

## 8. 完成後驗證

```bash
cargo test
```

預期：~418 tests，0 failed。

重點確認：
- `env.get` 對已設定的變數回傳 `Str(IO)`，未設定的回傳 `#none(IO)`
- `env.args` 回傳 `Combo` with `%kind: #list`，`EffectTag::IO`，至少含 `"0"` key
- `env.cwd` 回傳非空 `Str(IO)`
- `process.pid` 回傳正整數 `Int(IO)`，兩次呼叫相同
- `process.exit` 僅驗證已在 registry 中（不實際呼叫）
- SEED_ENV 和 SEED_PROCESS 已從 `PLACEHOLDER_*` 更新為實際 sha256 值

---

## 9. 實作注意事項

| 事項 | 說明 |
|:-----|:-----|
| `build_str_list` | env.rs 的 module-level helper，不可用 list.rs 的私有函式 |
| `process.exit` 不測試實際呼叫 | 會終止整個 `cargo test` 進程 |
| `std::env::set_var` in tests | 測試前設好變數，避免依賴環境 |
| 種子重算 | 加入 ~%Env 和 ~%Process 後必須跑 `seed_caids_are_stable` 並更新常數 |
| `num_bigint` import in process.rs | `BigInt::from(std::process::id())` — `id()` 回傳 `u32`，`BigInt::from::<u32>` 有實作 |
