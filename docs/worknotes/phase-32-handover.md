# Phase 32 交接文件

> 狀態：待實作  
> 前置：Phase 31 完成（~363 tests passing）  
> 目標：B+C — Bytes 加密擴展（4 builtins）+ str Round 2（6 builtins）

---

## 概覽

| 任務 | 位置 | 內容 |
|:-----|:-----|:-----|
| Task 1 | `crates/interpreter/src/builtins/bytes.rs` | 新增 4 個加密 builtins |
| Task 2 | `crates/interpreter/src/builtins/string.rs` | 新增 6 個字串 builtins |
| Task 3 | `crates/interpreter/src/lib.rs` | 更新 `~%Bytes`（+4）和 `~%String`（+6）morphism 列表 |
| Task 4 | `crates/interpreter/src/genesis.rs` | 重跑 seed test → 更新 SEED_BYTES、SEED_STRING |
| Tests  | `crates/interpreter/tests/bytes_crypto_p32_test.rs`（新建） | ~6 個測試 |
| Tests  | `crates/interpreter/tests/str_p32_test.rs`（新建） | ~8 個測試 |

預期完成後：**~363 + 14 ≈ 377 tests**

### 已存在的 str builtins（不需重複實作）

以下均已在 `string.rs` 中實作完畢：
`str.to_lower`、`str.to_upper`、`str.starts_with`、`str.ends_with`、`str.repeat`、`str.contains`

---

## Task 1：bytes.rs 新增 4 個 builtins

在 `register_bytes_builtins` 末尾（`bytes.from_hex` 之後）加入：

```rust
use sha2::{Sha256, Digest};
use base64::{engine::general_purpose, Engine as _};
use ring::hmac;
```

> **注意**：這三個 `use` 語句放在函式外部頂層（檔案的 use 區塊）。bytes.rs 現有的 use 區塊在第 1–7 行，直接追加進去。

---

### 完整新增程式碼（貼在 `bytes.from_hex` 的 `}) as Arc<BuiltinFn>);` 之後）

```rust
    // bytes.sha256: {0: bytes} → Bytes (32-byte SHA-256 hash)
    m.insert("bytes.sha256".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Bytes(b), _, _) = oo.force(v, ctx).collapse() {
            let mut hasher = Sha256::new();
            hasher.update(b);
            return Value::Atom(AtomKind::Bytes(hasher.finalize().to_vec()), EffectTag::Pure, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // bytes.base64_encode: {0: bytes} → Str (standard base64, with padding)
    m.insert("bytes.base64_encode".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Bytes(b), _, _) = oo.force(v, ctx).collapse() {
            return Value::Atom(AtomKind::Str(general_purpose::STANDARD.encode(b)), EffectTag::Pure, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // bytes.base64_decode: {0: str} → Bytes | #none (invalid base64 → #none)
    m.insert("bytes.base64_decode".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
            return match general_purpose::STANDARD.decode(s.trim()) {
                Ok(bytes) => Value::Atom(AtomKind::Bytes(bytes), EffectTag::Pure, None),
                Err(_)    => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
            };
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // bytes.hmac_sha256: {0: key_bytes, 1: msg_bytes} → Bytes (32-byte HMAC-SHA256 tag)
    m.insert("bytes.hmac_sha256".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vk), Some(vm)) = (c.get_field("0"), c.get_field("1")) {
                let fk = oo.force(vk.clone(), ctx);
                let fm = oo.force(vm.clone(), ctx);
                if let (Value::Atom(AtomKind::Bytes(key), _, _), Value::Atom(AtomKind::Bytes(msg), _, _)) =
                    (fk.collapse(), fm.collapse())
                {
                    let signing_key = hmac::Key::new(hmac::HMAC_SHA256, key);
                    let tag = hmac::sign(&signing_key, msg);
                    return Value::Atom(AtomKind::Bytes(tag.as_ref().to_vec()), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
```

### 更新 bytes.rs 頂端的 use 區塊

找到（第 1–7 行）：

```rust
use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
```

替換為：

```rust
use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use sha2::{Sha256, Digest};
use base64::{engine::general_purpose, Engine as _};
use ring::hmac;
```

---

## Task 2：string.rs 新增 6 個 builtins

在 `str.trim_end` 的 `}) as Arc<BuiltinFn>);` 之後貼入：

```rust
    // str.reverse: {0: str} → Str (Unicode-safe char reversal)
    m.insert("str.reverse".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
            return Value::Atom(AtomKind::Str(s.chars().rev().collect::<String>()), EffectTag::Pure, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // str.count: {0: needle, 1: haystack} → Int (number of non-overlapping occurrences)
    m.insert("str.count".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vn), Some(vh)) = (c.get_field("0"), c.get_field("1")) {
                let needle   = oo.force(vn.clone(), ctx);
                let haystack = oo.force(vh.clone(), ctx);
                if let (Value::Atom(AtomKind::Str(n), _, _), Value::Atom(AtomKind::Str(h), _, _)) =
                    (needle.collapse(), haystack.collapse())
                {
                    let count = h.matches(n.as_str()).count();
                    return Value::Atom(AtomKind::Int(BigInt::from(count)), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // str.slice: {0: start, 1: end, 2: str} → Str (Unicode char indices, clamped silently)
    // Consistent with str.char_at / str.index_of (char-based, not byte-based).
    m.insert("str.slice".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vs), Some(ve), Some(vstr)) = (c.get_field("0"), c.get_field("1"), c.get_field("2")) {
                let fs  = oo.force(vs.clone(), ctx);
                let fe  = oo.force(ve.clone(), ctx);
                let fst = oo.force(vstr.clone(), ctx);
                if let (
                    Value::Atom(AtomKind::Int(s), _, _),
                    Value::Atom(AtomKind::Int(e), _, _),
                    Value::Atom(AtomKind::Str(st), _, _),
                ) = (fs.collapse(), fe.collapse(), fst.collapse()) {
                    let chars: Vec<char> = st.chars().collect();
                    let len   = chars.len();
                    let start = s.to_usize().unwrap_or(0).min(len);
                    let end   = e.to_usize().unwrap_or(0).min(len);
                    let sliced: String = if start <= end { chars[start..end].iter().collect() } else { String::new() };
                    return Value::Atom(AtomKind::Str(sliced), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // str.is_empty: {0: str} → #true | #false
    m.insert("str.is_empty".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
            return Value::Atom(AtomKind::Tag(if s.is_empty() { "true" } else { "false" }.to_string()), EffectTag::Pure, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // str.parse_float: {0: str} → Float | Bottom (parse error)
    m.insert("str.parse_float".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
            return match s.trim().parse::<f64>() {
                Ok(f)  => Value::Atom(AtomKind::Float(f), EffectTag::Pure, None),
                Err(_) => Value::Bottom(Box::new(BottomDetail {
                    cause: BottomCause::Conflict,
                    message: Some(format!("parse_float: invalid float {:?}", s)),
                    ..Default::default()
                })),
            };
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // str.lines: {0: str} → list of Str (split by line endings, no trailing empty from final \n)
    m.insert("str.lines".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
            let mut res = IndexMap::new();
            for (i, line) in s.lines().enumerate() {
                res.insert(i.to_string(), Value::Atom(AtomKind::Str(line.to_string()), EffectTag::Pure, None));
            }
            res.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
            return Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]));
        }
        Value::Top
    }) as Arc<BuiltinFn>);
```

---

## Task 3：更新 `lib.rs` 的 morphism 列表

### `~%Bytes`（約第 290–306 行）

找到：

```rust
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
```

替換為：

```rust
        let bytes_morphisms = vec![
            ("/from_str",      "bytes.from_str"),
            ("/to_str",        "bytes.to_str"),
            ("/len",           "bytes.len"),
            ("/at",            "bytes.at"),
            ("/concat",        "bytes.concat"),
            ("/slice",         "bytes.slice"),
            ("/to_hex",        "bytes.to_hex"),
            ("/from_hex",      "bytes.from_hex"),
            // Phase 32
            ("/sha256",        "bytes.sha256"),
            ("/base64_encode", "bytes.base64_encode"),
            ("/base64_decode", "bytes.base64_decode"),
            ("/hmac_sha256",   "bytes.hmac_sha256"),
        ];
```

### `~%String`（約第 248–275 行）

找到：

```rust
            // Phase 27
            ("/index_of",    "str.index_of"),
            ("/pad_left",    "str.pad_left"),
            ("/pad_right",   "str.pad_right"),
            ("/trim_start",  "str.trim_start"),
            ("/trim_end",    "str.trim_end"),
        ];
```

替換為：

```rust
            // Phase 27
            ("/index_of",    "str.index_of"),
            ("/pad_left",    "str.pad_left"),
            ("/pad_right",   "str.pad_right"),
            ("/trim_start",  "str.trim_start"),
            ("/trim_end",    "str.trim_end"),
            // Phase 32
            ("/reverse",     "str.reverse"),
            ("/count",       "str.count"),
            ("/slice",       "str.slice"),
            ("/is_empty",    "str.is_empty"),
            ("/parse_float", "str.parse_float"),
            ("/lines",       "str.lines"),
        ];
```

---

## Task 4：重跑 seed test → 更新 genesis.rs

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture 2>&1
```

從輸出找到 `UPDATE:` 行，將 `SEED_BYTES` 和 `SEED_STRING` 更新為新值。其他 seed 不受影響。

---

## 測試：`tests/bytes_crypto_p32_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn bytes_val(b: Vec<u8>) -> Value {
    Value::Atom(AtomKind::Bytes(b), EffectTag::Pure, None)
}
fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}
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

// ── bytes.sha256 ───────────────────────────────────────────────────

#[test]
fn test_sha256_output_is_32_bytes() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "bytes.sha256", combo1(bytes_val(b"hello".to_vec())));
    if let Value::Atom(AtomKind::Bytes(b), _, _) = r { assert_eq!(b.len(), 32); }
    else { panic!("expected Bytes"); }
}

#[test]
fn test_sha256_deterministic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r1 = call(&oo, &mut ctx, "bytes.sha256", combo1(bytes_val(b"abc".to_vec())));
    let r2 = call(&oo, &mut ctx, "bytes.sha256", combo1(bytes_val(b"abc".to_vec())));
    assert_eq!(r1, r2);
}

// ── bytes.base64_encode / decode ───────────────────────────────────

#[test]
fn test_base64_encode_decode_roundtrip() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let original = b"hello world!".to_vec();
    let encoded = call(&oo, &mut ctx, "bytes.base64_encode", combo1(bytes_val(original.clone())));
    let decoded = call(&oo, &mut ctx, "bytes.base64_decode", combo1(encoded));
    if let Value::Atom(AtomKind::Bytes(b), _, _) = decoded { assert_eq!(b, original); }
    else { panic!("expected Bytes"); }
}

#[test]
fn test_base64_decode_invalid_returns_none() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "bytes.base64_decode", combo1(str_val("not!!valid!!")));
    assert!(matches!(r, Value::Atom(AtomKind::Tag(t), _, _) if t == "none"));
}

// ── bytes.hmac_sha256 ──────────────────────────────────────────────

#[test]
fn test_hmac_sha256_output_is_32_bytes() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let key = bytes_val(b"secret".to_vec());
    let msg = bytes_val(b"message".to_vec());
    let r = call(&oo, &mut ctx, "bytes.hmac_sha256", combo2(key, msg));
    if let Value::Atom(AtomKind::Bytes(b), _, _) = r { assert_eq!(b.len(), 32); }
    else { panic!("expected Bytes"); }
}

#[test]
fn test_hmac_sha256_different_keys_differ() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let msg = bytes_val(b"data".to_vec());
    let r1 = call(&oo, &mut ctx, "bytes.hmac_sha256", combo2(bytes_val(b"key1".to_vec()), msg.clone()));
    let r2 = call(&oo, &mut ctx, "bytes.hmac_sha256", combo2(bytes_val(b"key2".to_vec()), msg));
    assert_ne!(r1, r2);
}
```

---

## 測試：`tests/str_p32_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn int(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
fn combo1(a: Value) -> Value {
    let mut m = IndexMap::new(); m.insert("0".to_string(), a);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
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
fn as_str(v: &Value) -> &str {
    match v { Value::Atom(AtomKind::Str(s), _, _) => s, o => panic!("expected Str: {:?}", o) }
}
fn as_int(v: &Value) -> i64 {
    match v { Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(), o => panic!("expected Int: {:?}", o) }
}
fn list_len(v: &Value) -> usize {
    match v { Value::Combo(c) => c.fields().keys().filter(|k| k.parse::<usize>().is_ok()).count(), _ => panic!("expected list") }
}
fn list_str_at(v: &Value, i: usize) -> &str {
    match v { Value::Combo(c) => as_str(c.get_field(&i.to_string()).expect("index")), _ => panic!() }
}

// ── str.reverse ────────────────────────────────────────────────────

#[test]
fn test_str_reverse_ascii() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.reverse", combo1(str_val("hello")));
    assert_eq!(as_str(&r), "olleh");
}

#[test]
fn test_str_reverse_empty() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.reverse", combo1(str_val("")));
    assert_eq!(as_str(&r), "");
}

// ── str.count ──────────────────────────────────────────────────────

#[test]
fn test_str_count_occurrences() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.count", combo2(str_val("ab"), str_val("ababab")));
    assert_eq!(as_int(&r), 3);
}

#[test]
fn test_str_count_zero() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.count", combo2(str_val("xyz"), str_val("hello")));
    assert_eq!(as_int(&r), 0);
}

// ── str.slice ──────────────────────────────────────────────────────

#[test]
fn test_str_slice_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.slice", combo3(int(1), int(4), str_val("hello")));
    assert_eq!(as_str(&r), "ell");
}

#[test]
fn test_str_slice_clamped() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.slice", combo3(int(2), int(100), str_val("hi")));
    assert_eq!(as_str(&r), "");
}

// ── str.is_empty ───────────────────────────────────────────────────

#[test]
fn test_str_is_empty_true() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.is_empty", combo1(str_val("")));
    assert!(matches!(r, Value::Atom(AtomKind::Tag(t), _, _) if t == "true"));
}

// ── str.lines ──────────────────────────────────────────────────────

#[test]
fn test_str_lines_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.lines", combo1(str_val("a\nb\nc")));
    assert_eq!(list_len(&r), 3);
    assert_eq!(list_str_at(&r, 0), "a");
    assert_eq!(list_str_at(&r, 2), "c");
}
```

---

## Cargo.toml：新增兩個 test 條目

```toml
[[test]]
name = "bytes_crypto_p32_test"
path = "tests/bytes_crypto_p32_test.rs"

[[test]]
name = "str_p32_test"
path = "tests/str_p32_test.rs"
```

---

## 設計備忘

### str.slice 使用 char 索引
與 `str.char_at`、`str.index_of` 保持一致（char 索引，非 byte 索引）。
注意：現有的 `str.len` 回傳 byte 長度（已知 inconsistency，Phase 32 不修改以免破壞現有測試）。

### str.lines 不產生多餘尾空行
Rust 的 `str::lines()` 對末尾的 `\n` 不產生空字串，行為與 Python 的 `str.splitlines()` 一致。

### bytes.base64 使用 STANDARD engine
即帶 padding 的標準 Base64（RFC 4648）。

### sha2/ring/base64 均為既有依賴
`Cargo.toml` **無需新增**任何依賴。

---

## 驗證步驟

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools

# 1. 編譯
cargo build --manifest-path crates/interpreter/Cargo.toml 2>&1

# 2. 新測試
cargo test --manifest-path crates/interpreter/Cargo.toml bytes_crypto_p32_test -- --nocapture
cargo test --manifest-path crates/interpreter/Cargo.toml str_p32_test -- --nocapture

# 3. seed 更新後穩定性
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture

# 4. 全套不退步
cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：~377 tests, 0 failed
```
