# Phase 33 交接文件

> 狀態：待實作  
> 前置：Phase 32 完成（~377 tests passing）  
> 目標：`~%Json` 模組 — 4 個 builtins（json.parse / stringify / get / keys）

---

## 概覽

| 任務 | 位置 | 內容 |
|:-----|:-----|:-----|
| Task 1 | `crates/interpreter/src/builtins/json.rs`（**新建**） | 4 個 json builtins |
| Task 2 | `crates/interpreter/src/builtins/mod.rs` | 加入 `mod json;` 和呼叫 |
| Task 3 | `crates/interpreter/src/lib.rs` | 在 `root_with_system()` 加入 `~%Json` 模組 |
| Task 4 | `crates/interpreter/src/genesis.rs` | 加入 `SEED_JSON`，重跑 seed test |
| Tests  | `crates/interpreter/tests/json_p33_test.rs`（新建） | ~9 個測試 |

預期完成後：**~377 + 9 ≈ 386 tests**

### Builtin 語義速查

| builtin | 輸入 | 輸出 | 失敗 |
|:--------|:-----|:-----|:-----|
| `json.parse` | `{0: str}` | Value | `#none`（invalid JSON） |
| `json.stringify` | `{0: value}` | Str | `#none`（不應觸發） |
| `json.get` | `{0: key, 1: combo}` | Value | `#none`（key 不存在）；`%`-前綴 key → `#none` |
| `json.keys` | `{0: combo}` | list of Str | Top（非 Combo） |

---

## JSON ↔ nlang 型別映射

### json.parse（JSON → nlang）

| JSON | nlang Value |
|:-----|:-----------|
| `null` | `Tag("none")` |
| `true` | `Tag("true")` |
| `false` | `Tag("false")` |
| integer | `Int(BigInt)` — 先試 i64，再試 u64 |
| float | `Float(f64)` |
| string | `Str(s)` |
| array | Combo `{"0":v0,"1":v1,...,"%kind":"#list"}` |
| object | Combo `{key: v, ...}`（無 `%kind`） |

### json.stringify（nlang → JSON）

| nlang Value | JSON |
|:-----------|:-----|
| `Tag("none"/"null")` | `null` |
| `Tag("true")` | `true` |
| `Tag("false")` | `false` |
| 其他 `Tag(t)` | `"#t"`（字串） |
| `Int(n)` → 若能轉 i64 | `number`；否則 `"n"` 字串 |
| `Float(f)` | `number`（NaN/Inf → `null`） |
| `Str(s)` | `"s"` |
| `Bytes(b)` | `"<hex>"` |
| Combo + `%kind:#list` | array |
| Combo（其他） | object（跳過 `%`-前綴 key） |
| Top / Bottom / etc. | `null` |

---

## Task 1：新建 `crates/interpreter/src/builtins/json.rs`

完整內容如下：

```rust
use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use indexmap::IndexMap;

// ── 互相遞迴的轉換輔助函式 ───────────────────────────────────────────

fn json_to_nlang(json: serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => {
            Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None)
        }
        serde_json::Value::Bool(b) => {
            Value::Atom(AtomKind::Tag(if b { "true" } else { "false" }.to_string()), EffectTag::Pure, None)
        }
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Atom(AtomKind::Int(BigInt::from(i)), EffectTag::Pure, None)
            } else if let Some(u) = n.as_u64() {
                Value::Atom(AtomKind::Int(BigInt::from(u)), EffectTag::Pure, None)
            } else {
                Value::Atom(AtomKind::Float(n.as_f64().unwrap_or(f64::NAN)), EffectTag::Pure, None)
            }
        }
        serde_json::Value::String(s) => {
            Value::Atom(AtomKind::Str(s), EffectTag::Pure, None)
        }
        serde_json::Value::Array(arr) => {
            let mut res = IndexMap::new();
            for (i, v) in arr.into_iter().enumerate() {
                res.insert(i.to_string(), json_to_nlang(v));
            }
            res.insert("%kind".to_string(),
                Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
            Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]))
        }
        serde_json::Value::Object(obj) => {
            let mut res = IndexMap::new();
            for (k, v) in obj {
                res.insert(k, json_to_nlang(v));
            }
            Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]))
        }
    }
}

fn nlang_to_json(val: &Value) -> serde_json::Value {
    match val.collapse() {
        Value::Atom(AtomKind::Tag(t), _, _) => {
            match t.trim_start_matches('#') {
                "none" | "null" => serde_json::Value::Null,
                "true"          => serde_json::Value::Bool(true),
                "false"         => serde_json::Value::Bool(false),
                other           => serde_json::Value::String(format!("#{}", other)),
            }
        }
        Value::Atom(AtomKind::Int(n), _, _) => {
            n.to_i64()
                .map(|i| serde_json::Value::Number(i.into()))
                .unwrap_or_else(|| serde_json::Value::String(n.to_string()))
        }
        Value::Atom(AtomKind::Float(f), _, _) => {
            serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        Value::Atom(AtomKind::Str(s), _, _) => serde_json::Value::String(s.clone()),
        Value::Atom(AtomKind::Bytes(b), _, _) => serde_json::Value::String(hex::encode(b)),
        Value::Combo(c) => {
            let is_list = c.get_field("%kind").map(|k| {
                matches!(k.collapse(),
                    Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "list")
            }).unwrap_or(false);

            if is_list {
                let mut arr = Vec::new();
                let mut i = 0usize;
                loop {
                    match c.get_field(&i.to_string()) {
                        Some(v) => { arr.push(nlang_to_json(v)); i += 1; }
                        None    => break,
                    }
                }
                serde_json::Value::Array(arr)
            } else {
                let mut obj = serde_json::Map::new();
                for (k, v) in c.fields_iter() {
                    if !k.starts_with('%') {
                        obj.insert(k.clone(), nlang_to_json(v));
                    }
                }
                serde_json::Value::Object(obj)
            }
        }
        _ => serde_json::Value::Null,
    }
}

// ── Builtin 登錄 ───────────────────────────────────────────────────

pub fn register_json_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // json.parse: {0: str} → Value | #none (invalid JSON → #none)
    m.insert("json.parse".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
            return match serde_json::from_str::<serde_json::Value>(s) {
                Ok(json) => json_to_nlang(json),
                Err(_)   => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
            };
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // json.stringify: {0: value} → Str | #none
    m.insert("json.stringify".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        let json_val = nlang_to_json(&forced);
        match serde_json::to_string(&json_val) {
            Ok(s)  => Value::Atom(AtomKind::Str(s), EffectTag::Pure, None),
            Err(_) => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
        }
    }) as Arc<BuiltinFn>);

    // json.get: {0: key_str, 1: combo} → Value | #none
    // Returns #none for: key not found, key starts with %, arg1 not a Combo.
    m.insert("json.get".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vk), Some(vv)) = (c.get_field("0"), c.get_field("1")) {
                let fk = oo.force(vk.clone(), ctx);
                let fv = oo.force(vv.clone(), ctx);
                if let Value::Atom(AtomKind::Str(key), _, _) = fk.collapse() {
                    if key.starts_with('%') {
                        return Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
                    }
                    if let Value::Combo(ref cv) = fv.collapse() {
                        return match cv.get_field(key) {
                            Some(v) => v.clone(),
                            None    => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
                        };
                    }
                    return Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // json.keys: {0: combo} → list of Str (all non-%-prefixed keys, in insertion order)
    m.insert("json.keys".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        if let Value::Combo(ref cv) = forced.collapse() {
            let mut res = IndexMap::new();
            let mut i = 0usize;
            for (k, _) in cv.fields_iter() {
                if !k.starts_with('%') {
                    res.insert(i.to_string(),
                        Value::Atom(AtomKind::Str(k.clone()), EffectTag::Pure, None));
                    i += 1;
                }
            }
            res.insert("%kind".to_string(),
                Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
            return Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]));
        }
        Value::Top
    }) as Arc<BuiltinFn>);
}
```

---

## Task 2：更新 `mod.rs`

找到（第 9 行）：

```rust
mod bytes;
mod regex;
```

替換為：

```rust
mod bytes;
mod regex;
mod json;
```

並在 `create_default_builtins()` 中加入（`regex::register_regex_builtins(&mut m);` 之後）：

```rust
    json::register_json_builtins(&mut m);
```

---

## Task 3：更新 `root_with_system()`（`lib.rs`）

在 `~%Regex` 區塊（`fields.insert("~%Regex"...` 那行）之後，`~%Discovery` 區塊之前，插入：

```rust
        let mut json_fields = IndexMap::new();
        let json_morphisms = vec![
            ("/parse",     "json.parse"),
            ("/stringify", "json.stringify"),
            ("/get",       "json.get"),
            ("/keys",      "json.keys"),
        ];
        for (n, b) in json_morphisms {
            json_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(),  Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])));
        }
        fields.insert("~%Json".to_string(), Value::Combo(ComboVal::new(json_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
```

---

## Task 4：更新 `genesis.rs`

### 加入常數（在 `SEED_BYTES` 之後）

```rust
pub const SEED_JSON:      &str = "hash:sha256:v1:PLACEHOLDER_run_seed_test";
```

### 更新 `all_seeds()`（在 `"~%Bytes"` 條目之後）

```rust
        ("~%Json",       SEED_JSON),
```

### 重跑 seed test

```bash
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture 2>&1
```

從輸出的 `UPDATE:` 行找到 `~%Json` 的 CAID，更新 `SEED_JSON`。其他 seed 不受影響。

---

## 測試（`tests/json_p33_test.rs`）

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn int(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
fn tag(t: &str) -> Value { Value::Atom(AtomKind::Tag(t.to_string()), EffectTag::Pure, None) }
fn combo1(a: Value) -> Value {
    let mut m = IndexMap::new(); m.insert("0".to_string(), a);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn combo2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn as_str(v: &Value) -> &str {
    match v { Value::Atom(AtomKind::Str(s), _, _) => s, o => panic!("expected Str: {:?}", o) }
}
fn as_int(v: &Value) -> i64 {
    match v { Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(), o => panic!("expected Int: {:?}", o) }
}
fn is_none(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none")
}
fn list_len(v: &Value) -> usize {
    match v { Value::Combo(c) => c.fields_iter().filter(|(k,_)| k.parse::<usize>().is_ok()).count(), _ => panic!("not a list") }
}
fn list_str_at(v: &Value, i: usize) -> String {
    match v { Value::Combo(c) => as_str(c.get_field(&i.to_string()).unwrap()).to_string(), _ => panic!() }
}

// ── json.parse ─────────────────────────────────────────────────────

#[test]
fn test_json_parse_object() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "json.parse", combo1(str_val(r#"{"name":"Alice","age":30}"#)));
    if let Value::Combo(ref c) = r {
        assert_eq!(as_str(c.get_field("name").unwrap()), "Alice");
        assert_eq!(as_int(c.get_field("age").unwrap()), 30);
    } else { panic!("expected Combo, got {:?}", r); }
}

#[test]
fn test_json_parse_array() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "json.parse", combo1(str_val("[1,2,3]")));
    assert_eq!(list_len(&r), 3);
    if let Value::Combo(ref c) = r {
        assert_eq!(as_int(c.get_field("1").unwrap()), 2);
    }
}

#[test]
fn test_json_parse_primitives() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r_null  = call(&oo, &mut ctx, "json.parse", combo1(str_val("null")));
    let r_true  = call(&oo, &mut ctx, "json.parse", combo1(str_val("true")));
    let r_false = call(&oo, &mut ctx, "json.parse", combo1(str_val("false")));
    assert!(is_none(&r_null));
    assert!(matches!(r_true,  Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "true"));
    assert!(matches!(r_false, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "false"));
}

#[test]
fn test_json_parse_invalid_returns_none() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "json.parse", combo1(str_val("{bad json}")));
    assert!(is_none(&r));
}

// ── json.stringify ─────────────────────────────────────────────────

#[test]
fn test_json_stringify_int() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "json.stringify", combo1(int(42)));
    assert_eq!(as_str(&r), "42");
}

#[test]
fn test_json_stringify_list() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    // Build a list [1,2]
    let mut m = IndexMap::new();
    m.insert("0".to_string(), int(1));
    m.insert("1".to_string(), int(2));
    m.insert("%kind".to_string(), tag("list"));
    let list = Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]));
    let r = call(&oo, &mut ctx, "json.stringify", combo1(list));
    assert_eq!(as_str(&r), "[1,2]");
}

// ── json.get ───────────────────────────────────────────────────────

#[test]
fn test_json_get_found() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let parsed = call(&oo, &mut ctx, "json.parse", combo1(str_val(r#"{"x":99}"#)));
    let r = call(&oo, &mut ctx, "json.get", combo2(str_val("x"), parsed));
    assert_eq!(as_int(&r), 99);
}

#[test]
fn test_json_get_not_found_returns_none() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let parsed = call(&oo, &mut ctx, "json.parse", combo1(str_val(r#"{"x":1}"#)));
    let r = call(&oo, &mut ctx, "json.get", combo2(str_val("missing"), parsed));
    assert!(is_none(&r));
}

// ── json.keys ──────────────────────────────────────────────────────

#[test]
fn test_json_keys() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let parsed = call(&oo, &mut ctx, "json.parse", combo1(str_val(r#"{"a":1,"b":2}"#)));
    let keys = call(&oo, &mut ctx, "json.keys", combo1(parsed));
    assert_eq!(list_len(&keys), 2);
    // Keys should be "a" and "b" (in insertion order)
    let k0 = list_str_at(&keys, 0);
    let k1 = list_str_at(&keys, 1);
    assert!(k0 == "a" || k0 == "b");
    assert!(k1 == "a" || k1 == "b");
    assert_ne!(k0, k1);
}
```

### 加入 `Cargo.toml` 的 test 條目

```toml
[[test]]
name = "json_p33_test"
path = "tests/json_p33_test.rs"
```

---

## 注意事項

### `serde_json` 無需新依賴
`serde_json = "1.0"` 已在 `Cargo.toml` 的 `[dependencies]`。`json.rs` 直接使用 `serde_json::Value`（與 nlang `Value` 不同的 external crate 型別）。

### `fields_iter()` vs `fields()`
`fields()` 回傳 `IndexMap<String, Value>`（clone 所有值），`fields_iter()` 回傳 `impl Iterator<Item = (&String, &Value)>`（零拷貝）。`nlang_to_json` 和 `json.keys` 均使用 `fields_iter()` 以提高效率。

### `%`-前綴 key 的處理
`json.keys` 和 `json.stringify`（Object 分支）均跳過 `%`-前綴的 meta key（如 `%kind`、`%morphism` 等），確保只輸出使用者語義資料。

### `json.get` 對 `%` key 返回 `#none`
若試圖用 `json.get` 存取 meta key，返回 `#none` 而非 Top，這是有意的保護措施。

### 只有 `SEED_JSON` 是新的
`root_with_system()` 只新增了 `~%Json` 欄位，現有模組（`~%Regex`、`~%Bytes` 等）結構不變，其 seed 不受影響。

---

## 驗證步驟

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools

# 1. 編譯
cargo build --manifest-path crates/interpreter/Cargo.toml 2>&1

# 2. 新測試
cargo test --manifest-path crates/interpreter/Cargo.toml json_p33_test -- --nocapture

# 3. seed 更新後穩定性
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture

# 4. 全套不退步
cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：~386 tests, 0 failed
```
