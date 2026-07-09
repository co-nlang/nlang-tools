# Phase 30 交接文件

> 狀態：待實作  
> 前置：Phase 29 完成（~344 tests passing）  
> 目標：`~%Bytes` 模組 — 8 個 builtins

---

## 概覽

| 任務 | 位置 | 內容 |
|:-----|:-----|:-----|
| Task 1 | `crates/interpreter/src/builtins/bytes.rs`（**新建**） | 8 個 bytes builtins |
| Task 2 | `crates/interpreter/src/builtins/mod.rs` | 加入 `mod bytes;` 和呼叫 |
| Task 3 | `crates/interpreter/src/lib.rs` | 在 `root_with_system()` 加入 `~%Bytes` 模組 |
| Task 4 | `crates/interpreter/src/genesis.rs` | 加入 `SEED_BYTES`，重跑 seed test |
| Tests  | `crates/interpreter/tests/bytes_p30_test.rs`（新建） | ~10 個測試 |

預期完成後：**~344 + 10 ≈ 354 tests**

---

## Bytes builtins 語義速查

| builtin | 輸入 | 輸出 | 說明 |
|:--------|:-----|:-----|:-----|
| `bytes.from_str` | `{0: str}` | Bytes | UTF-8 編碼字串為位元組序列 |
| `bytes.to_str` | `{0: bytes}` | Str \| `#none` | UTF-8 解碼；無效 UTF-8 → `#none` |
| `bytes.len` | `{0: bytes}` | Int | 位元組數量 |
| `bytes.at` | `{0: idx, 1: bytes}` | Int (0–255) | 取第 idx 個位元組值；越界 → Top |
| `bytes.concat` | `{0: bytes_a, 1: bytes_b}` | Bytes | 串接兩個 Bytes |
| `bytes.slice` | `{0: start, 1: end, 2: bytes}` | Bytes | 切片 [start..end]；越界靜默 clamp |
| `bytes.to_hex` | `{0: bytes}` | Str | 轉為小寫十六進位字串（無 `0x` 前綴） |
| `bytes.from_hex` | `{0: str}` | Bytes \| `#none` | 解析十六進位字串；無效格式 → `#none` |

---

## Task 1：新建 `bytes.rs`

**建立** `crates/interpreter/src/builtins/bytes.rs`，完整內容如下：

```rust
use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

pub fn register_bytes_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // bytes.from_str: {0: str} → Bytes (UTF-8 encoded)
    m.insert("bytes.from_str".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
            return Value::Atom(AtomKind::Bytes(s.as_bytes().to_vec()), EffectTag::Pure, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // bytes.to_str: {0: bytes} → Str | #none (UTF-8 decode)
    m.insert("bytes.to_str".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Bytes(b), _, _) = oo.force(v, ctx).collapse() {
            return match String::from_utf8(b.clone()) {
                Ok(s)  => Value::Atom(AtomKind::Str(s), EffectTag::Pure, None),
                Err(_) => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
            };
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // bytes.len: {0: bytes} → Int
    m.insert("bytes.len".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Bytes(b), _, _) = oo.force(v, ctx).collapse() {
            return Value::Atom(AtomKind::Int(BigInt::from(b.len())), EffectTag::Pure, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // bytes.at: {0: idx, 1: bytes} → Int (0–255), Top if out of range
    m.insert("bytes.at".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vi), Some(vb)) = (c.get_field("0"), c.get_field("1")) {
                let fi = oo.force(vi.clone(), ctx);
                let fb = oo.force(vb.clone(), ctx);
                if let (Value::Atom(AtomKind::Int(idx), _, _), Value::Atom(AtomKind::Bytes(b), _, _)) =
                    (fi.collapse(), fb.collapse())
                {
                    if let Some(i) = idx.to_usize() {
                        if let Some(&byte_val) = b.get(i) {
                            return Value::Atom(AtomKind::Int(BigInt::from(byte_val)), EffectTag::Pure, None);
                        }
                    }
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // bytes.concat: {0: bytes_a, 1: bytes_b} → Bytes
    m.insert("bytes.concat".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(va), Some(vb)) = (c.get_field("0"), c.get_field("1")) {
                let fa = oo.force(va.clone(), ctx);
                let fb = oo.force(vb.clone(), ctx);
                if let (Value::Atom(AtomKind::Bytes(ba), _, _), Value::Atom(AtomKind::Bytes(bb), _, _)) =
                    (fa.collapse(), fb.collapse())
                {
                    let mut out = ba.clone();
                    out.extend_from_slice(bb);
                    return Value::Atom(AtomKind::Bytes(out), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // bytes.slice: {0: start, 1: end, 2: bytes} → Bytes
    // Indices are clamped silently (no error on out-of-range)
    m.insert("bytes.slice".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vs), Some(ve), Some(vb)) = (c.get_field("0"), c.get_field("1"), c.get_field("2")) {
                let fs = oo.force(vs.clone(), ctx);
                let fe = oo.force(ve.clone(), ctx);
                let fb = oo.force(vb.clone(), ctx);
                if let (
                    Value::Atom(AtomKind::Int(s), _, _),
                    Value::Atom(AtomKind::Int(e), _, _),
                    Value::Atom(AtomKind::Bytes(b), _, _),
                ) = (fs.collapse(), fe.collapse(), fb.collapse()) {
                    let len = b.len();
                    let start = s.to_usize().unwrap_or(0).min(len);
                    let end   = e.to_usize().unwrap_or(0).min(len);
                    let sliced = if start <= end { b[start..end].to_vec() } else { vec![] };
                    return Value::Atom(AtomKind::Bytes(sliced), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // bytes.to_hex: {0: bytes} → Str (lowercase hex, no 0x prefix)
    m.insert("bytes.to_hex".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Bytes(b), _, _) = oo.force(v, ctx).collapse() {
            return Value::Atom(AtomKind::Str(hex::encode(b)), EffectTag::Pure, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // bytes.from_hex: {0: str} → Bytes | #none (invalid hex → #none)
    m.insert("bytes.from_hex".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
            return match hex::decode(s.trim()) {
                Ok(bytes) => Value::Atom(AtomKind::Bytes(bytes), EffectTag::Pure, None),
                Err(_)    => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
            };
        }
        Value::Top
    }) as Arc<BuiltinFn>);
}
```

---

## Task 2：更新 `mod.rs`

**找到** `crates/interpreter/src/builtins/mod.rs`，加入：

```rust
mod bytes;
```

（放在 `mod time;` 之後）

並在 `create_default_builtins()` 函數內加入：

```rust
    bytes::register_bytes_builtins(&mut m);
```

（放在 `time::register_time_builtins(&mut m);` 之後）

---

## Task 3：更新 `root_with_system()`（`lib.rs`）

在 `~%Time` 區塊（約 185–187 行）之後，加入 `~%Bytes` 模組：

```rust
        let mut bytes_fields = IndexMap::new();
        let bytes_morphisms = vec![
            ("/from_str", "bytes.from_str"),
            ("/to_str",   "bytes.to_str"),
            ("/len",      "bytes.len"),
            ("/at",       "bytes.at"),
            ("/concat",   "bytes.concat"),
            ("/slice",    "bytes.slice"),
            ("/to_hex",   "bytes.to_hex"),
            ("/from_hex", "bytes.from_hex"),
        ];
        for (n, b) in bytes_morphisms {
            bytes_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(),  Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])));
        }
        fields.insert("~%Bytes".to_string(), Value::Combo(ComboVal::new(bytes_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
```

---

## Task 4：更新 genesis.rs

### 4A：加入常數

```rust
pub const SEED_BYTES: &str = "hash:sha256:v1:PLACEHOLDER_run_seed_test";
```

### 4B：更新 all_seeds()

```rust
        ("~%Bytes",      SEED_BYTES),   // ← 新增（加在 "~%Time" 之後）
```

### 4C：重跑 seed test

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture 2>&1
```

從輸出的 `UPDATE:` 行找到 `~%Bytes` 的 CAID，更新 `SEED_BYTES`。

---

## 測試（`tests/bytes_p30_test.rs`）

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn bytes_val(v: Vec<u8>) -> Value {
    Value::Atom(AtomKind::Bytes(v), EffectTag::Pure, None)
}
fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}
fn int(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}
fn combo2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn combo3(a: Value, b: Value, c: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b); m.insert("2".to_string(), c);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn as_bytes(v: &Value) -> &[u8] {
    match v { Value::Atom(AtomKind::Bytes(b), _, _) => b, o => panic!("expected Bytes: {:?}", o) }
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

#[test]
fn test_bytes_from_str_and_len() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let b = call(&oo, &mut ctx, "bytes.from_str", str_val("hello"));
    assert_eq!(as_bytes(&b), b"hello");
    let l = call(&oo, &mut ctx, "bytes.len", b);
    assert_eq!(as_int(&l), 5);
}

#[test]
fn test_bytes_to_str_roundtrip() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let b = call(&oo, &mut ctx, "bytes.from_str", str_val("nlang"));
    let s = call(&oo, &mut ctx, "bytes.to_str", b);
    assert_eq!(as_str(&s), "nlang");
}

#[test]
fn test_bytes_to_str_invalid_utf8_returns_none() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let b = bytes_val(vec![0xFF, 0xFE]);
    let r = call(&oo, &mut ctx, "bytes.to_str", b);
    assert!(is_none(&r), "invalid UTF-8 should return #none");
}

#[test]
fn test_bytes_at_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let b = bytes_val(vec![10, 20, 30]);
    let r = call(&oo, &mut ctx, "bytes.at", combo2(int(1), b));
    assert_eq!(as_int(&r), 20);
}

#[test]
fn test_bytes_at_out_of_range_returns_top() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let b = bytes_val(vec![1, 2, 3]);
    let r = call(&oo, &mut ctx, "bytes.at", combo2(int(5), b));
    assert!(matches!(r, Value::Top));
}

#[test]
fn test_bytes_concat() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let a = bytes_val(vec![1, 2]);
    let b = bytes_val(vec![3, 4]);
    let r = call(&oo, &mut ctx, "bytes.concat", combo2(a, b));
    assert_eq!(as_bytes(&r), &[1u8, 2, 3, 4]);
}

#[test]
fn test_bytes_slice() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let b = bytes_val(vec![10, 20, 30, 40, 50]);
    let r = call(&oo, &mut ctx, "bytes.slice", combo3(int(1), int(4), b));
    assert_eq!(as_bytes(&r), &[20u8, 30, 40]);
}

#[test]
fn test_bytes_to_hex() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let b = bytes_val(vec![0xDE, 0xAD, 0xBE, 0xEF]);
    let r = call(&oo, &mut ctx, "bytes.to_hex", b);
    assert_eq!(as_str(&r), "deadbeef");
}

#[test]
fn test_bytes_from_hex_valid() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "bytes.from_hex", str_val("deadbeef"));
    assert_eq!(as_bytes(&r), &[0xDEu8, 0xAD, 0xBE, 0xEF]);
}

#[test]
fn test_bytes_from_hex_invalid_returns_none() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "bytes.from_hex", str_val("xyz!"));
    assert!(is_none(&r));
}
```

### 加入 `Cargo.toml` 的 test 條目

```toml
[[test]]
name = "bytes_p30_test"
path = "tests/bytes_p30_test.rs"
```

---

## 注意事項

### `hex` crate 的大小寫
`hex::decode` 接受大小寫混合的十六進位字元（`"DEADBEEF"` 和 `"deadbeef"` 都可以）。`hex::encode` 輸出小寫。

### `bytes.slice` 越界靜默 clamp
`start > end` 時（包括 `start` 或 `end` 超出 bytes 長度）回傳空 Bytes，不回傳 Top。行為與 Python bytes 切片一致，適合安全的子序列操作。

### `bytes.at` 的負數索引
`BigInt::to_usize()` 對負數回傳 `None`，天然防止負索引攻擊 → 回傳 Top。

### `bytes.from_str` 的單一 arg 形式
與 `str.trim_start`、`bytes.len` 等其他單一 arg builtins 一致：`c.get_field("0").cloned().unwrap_or(arg.clone())`，可接受裸值或 `{0: value}` Combo。

### SEED_BYTES 為新常數
只有 `~%Bytes` CAID 需要新加，其他既有 seed 不受影響。

---

## 驗證步驟

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools

# 1. 編譯
cargo build --manifest-path crates/interpreter/Cargo.toml 2>&1

# 2. 新測試
cargo test --manifest-path crates/interpreter/Cargo.toml bytes_p30_test -- --nocapture

# 3. 種子更新後穩定性
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture

# 4. 全套不退步
cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：~354 tests, 0 failed
```
