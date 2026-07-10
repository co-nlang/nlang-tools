# Phase 27 交接文件

> 狀態：待實作  
> 前置：Phase 26 完成（~305 tests passing）  
> 目標：A) Math 擴展（gcd/lcm/sign/log2/log10）C) String 擴展（index_of/pad_left/pad_right/trim_start/trim_end）

---

## 重要發現：~%Math 的 Phase 19 態射缺漏

與 Phase 25 發現 ~%List 缺漏相同，**Phase 19 新增的 math.min/max/floor/ceil/round/clamp 也從未加進 `root_with_system()` 的 `math_morphisms`**。  
Phase 27 Task 3 會一次補齊。

---

## 任務總覽

| # | 位置 | 內容 |
|:--|:-----|:-----|
| Task 1 | `crates/interpreter/src/builtins/math.rs` | 5 個新 math builtins |
| Task 2 | `crates/interpreter/src/builtins/string.rs` | 5 個新 string builtins |
| Task 3 | `crates/interpreter/src/lib.rs` | 補全 `~%Math`（Phase 19 缺漏 + 新）和 `~%String`（新）態射 |
| Task 4 | `crates/interpreter/src/genesis.rs` | 重跑 seed test，更新 SEED_MATH、SEED_STRING |
| Tests  | `tests/math_p27_test.rs` + `tests/str_p27_test.rs`（新建） | ~22 個測試 |

預期完成後：**~305 + 22 ≈ 327 tests**

---

## Task 1：新 math builtins（`math.rs`）

在 `math.clamp` 之後（約 513 行）加入以下 5 個 builtins。

### 輔助函數（加在 `register_math_builtins` 函數內部頂端附近，或直接在第一個 m.insert 之前）

```rust
    // Euclidean GCD for BigInt (used by math.gcd and math.lcm)
    fn bigint_gcd(mut a: BigInt, mut b: BigInt) -> BigInt {
        a = a.abs();
        b = b.abs();
        while !b.is_zero() {
            let t = b.clone();
            b = a % &b;
            a = t;
        }
        a
    }
```

**注意**：這是 `register_math_builtins` 內的 inner function（與現有 `math.clamp` 中的 `to_f` closure 同級），不是模組層級函數。

### `math.gcd`

```rust
    // math.gcd: {0: a, 1: b} → Int  (gcd(|a|, |b|))
    m.insert("math.gcd".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(va), Some(vb)) = (c.get_field("0"), c.get_field("1")) {
                let fa = oo.force(va.clone(), ctx);
                let fb = oo.force(vb.clone(), ctx);
                if let (Value::Atom(AtomKind::Int(a), _, _), Value::Atom(AtomKind::Int(b), _, _)) =
                    (fa.collapse(), fb.collapse())
                {
                    return Value::Atom(AtomKind::Int(bigint_gcd(a.clone(), b.clone())), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
```

### `math.lcm`

```rust
    // math.lcm: {0: a, 1: b} → Int  (lcm(|a|, |b|))
    // lcm = |a * b| / gcd(a, b); lcm(0, _) = 0
    m.insert("math.lcm".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(va), Some(vb)) = (c.get_field("0"), c.get_field("1")) {
                let fa = oo.force(va.clone(), ctx);
                let fb = oo.force(vb.clone(), ctx);
                if let (Value::Atom(AtomKind::Int(a), _, _), Value::Atom(AtomKind::Int(b), _, _)) =
                    (fa.collapse(), fb.collapse())
                {
                    let g = bigint_gcd(a.clone(), b.clone());
                    if g.is_zero() {
                        return Value::Atom(AtomKind::Int(BigInt::from(0)), EffectTag::Pure, None);
                    }
                    let lcm = (a.clone().abs() / &g) * b.clone().abs();
                    return Value::Atom(AtomKind::Int(lcm), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
```

### `math.sign`

語義：正數 → 1（或 1.0），負數 → -1（或 -1.0），零 → 0（或 0.0）。輸出型別與輸入相同。

```rust
    // math.sign: {0: x} → Int (-1/0/1) or Float (-1.0/0.0/1.0)
    m.insert("math.sign".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        match oo.force(v, ctx).collapse().clone() {
            Value::Atom(AtomKind::Int(i), e, _) => {
                let s = if i.is_positive() { 1i64 } else if i.is_negative() { -1 } else { 0 };
                Value::Atom(AtomKind::Int(BigInt::from(s)), e, None)
            }
            Value::Atom(AtomKind::Float(f), e, _) => {
                let s = if f > 0.0 { 1.0f64 } else if f < 0.0 { -1.0 } else { 0.0 };
                Value::Atom(AtomKind::Float(s), e, None)
            }
            _ => Value::Top,
        }
    }) as Arc<BuiltinFn>);
```

### `math.log2`

```rust
    // math.log2: {0: x} → Float; log2(0) or log2(negative) → Blur
    m.insert("math.log2".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let fv = oo.force(v, ctx);
        let x: f64 = match fv.collapse() {
            Value::Atom(AtomKind::Float(f), _, _) => *f,
            Value::Atom(AtomKind::Int(i), _, _) => match i.to_f64() { Some(f) => f, None => return Value::Top },
            _ => return Value::Top,
        };
        if x <= 0.0 {
            return Value::Blur(BlurDetail {
                cause: BlurCause::MathSingularity,
                horizon: HorizonParams::default(),
                partial: None,
                effect: EffectTag::Pure,
            });
        }
        Value::Atom(AtomKind::Float(x.log2()), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);
```

### `math.log10`

```rust
    // math.log10: {0: x} → Float; log10(0) or log10(negative) → Blur
    m.insert("math.log10".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let fv = oo.force(v, ctx);
        let x: f64 = match fv.collapse() {
            Value::Atom(AtomKind::Float(f), _, _) => *f,
            Value::Atom(AtomKind::Int(i), _, _) => match i.to_f64() { Some(f) => f, None => return Value::Top },
            _ => return Value::Top,
        };
        if x <= 0.0 {
            return Value::Blur(BlurDetail {
                cause: BlurCause::MathSingularity,
                horizon: HorizonParams::default(),
                partial: None,
                effect: EffectTag::Pure,
            });
        }
        Value::Atom(AtomKind::Float(x.log10()), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);
```

**注意**：`BlurDetail`、`BlurCause`、`HorizonParams` 均已在 math.rs 的 imports 中（`use crate::value::{..., BlurDetail, BlurCause, HorizonParams};`）。

---

## Task 2：新 string builtins（`string.rs`）

在 `str.chars` 之後（285 行末）加入：

### `str.index_of`

語義：回傳 `needle` 在 `haystack` 中第一次出現的**字元位置**（Unicode char index，0-based）。找不到 → `#none` Tag。

```
str.index_of {0: needle, 1: haystack}  →  Int | #none
```

```rust
    // str.index_of: {0: needle, 1: haystack} → Int (char index) or #none
    m.insert("str.index_of".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vn), Some(vh)) = (c.get_field("0"), c.get_field("1")) {
                let needle   = oo.force(vn.clone(), ctx);
                let haystack = oo.force(vh.clone(), ctx);
                if let (Value::Atom(AtomKind::Str(n), _, _), Value::Atom(AtomKind::Str(h), _, _)) =
                    (needle.collapse(), haystack.collapse())
                {
                    return match h.find(n.as_str()) {
                        None => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
                        Some(byte_idx) => {
                            let char_idx = h[..byte_idx].chars().count();
                            Value::Atom(AtomKind::Int(BigInt::from(char_idx)), EffectTag::Pure, None)
                        }
                    };
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
```

### `str.pad_left`

語義：用空格在左側填充到總寬度 `width`。字串已達或超過 `width` 則原樣回傳。

```
str.pad_left {0: width, 1: str}  →  Str
```

```rust
    // str.pad_left: {0: width, 1: str} → Str (space-pad on left to total width)
    m.insert("str.pad_left".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vw), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
                let fw = oo.force(vw.clone(), ctx);
                let fs = oo.force(vs.clone(), ctx);
                if let (Value::Atom(AtomKind::Int(w), _, _), Value::Atom(AtomKind::Str(s), _, _)) =
                    (fw.collapse(), fs.collapse())
                {
                    if let Some(width) = w.to_usize() {
                        let char_count = s.chars().count();
                        if char_count >= width {
                            return Value::Atom(AtomKind::Str(s.clone()), EffectTag::Pure, None);
                        }
                        let pad = " ".repeat(width - char_count);
                        return Value::Atom(AtomKind::Str(format!("{}{}", pad, s)), EffectTag::Pure, None);
                    }
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
```

### `str.pad_right`

```
str.pad_right {0: width, 1: str}  →  Str (space-pad on right)
```

```rust
    // str.pad_right: {0: width, 1: str} → Str (space-pad on right to total width)
    m.insert("str.pad_right".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vw), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
                let fw = oo.force(vw.clone(), ctx);
                let fs = oo.force(vs.clone(), ctx);
                if let (Value::Atom(AtomKind::Int(w), _, _), Value::Atom(AtomKind::Str(s), _, _)) =
                    (fw.collapse(), fs.collapse())
                {
                    if let Some(width) = w.to_usize() {
                        let char_count = s.chars().count();
                        if char_count >= width {
                            return Value::Atom(AtomKind::Str(s.clone()), EffectTag::Pure, None);
                        }
                        let pad = " ".repeat(width - char_count);
                        return Value::Atom(AtomKind::Str(format!("{}{}", s, pad)), EffectTag::Pure, None);
                    }
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
```

### `str.trim_start`

```
str.trim_start {0: str}  →  Str（移除前置空白）
```

```rust
    // str.trim_start: {0: str} → Str (remove leading whitespace)
    m.insert("str.trim_start".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
            return Value::Atom(AtomKind::Str(s.trim_start().to_string()), EffectTag::Pure, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);
```

### `str.trim_end`

```rust
    // str.trim_end: {0: str} → Str (remove trailing whitespace)
    m.insert("str.trim_end".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
            return Value::Atom(AtomKind::Str(s.trim_end().to_string()), EffectTag::Pure, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);
```

**注意**：`string.rs` 已有 `use num_bigint::BigInt;` import。

---

## Task 3：補全 `root_with_system()` 的模組態射

### 3A：更新 `~%Math`

**找到**（約 160 行）：
```rust
        let math_morphisms = vec![("/sub", "math.sub"), ("/mul", "math.mul"), ..., ("/eml", "math.eml")];
```

**替換為**（完整版）：
```rust
        let math_morphisms = vec![
            ("/sub",    "math.sub"),
            ("/mul",    "math.mul"),
            ("/div",    "math.div"),
            ("/rem",    "math.rem"),
            ("/abs",    "math.abs"),
            ("/bits",   "math.bits"),
            ("/pow",    "math.pow"),
            ("/sqrt",   "math.sqrt"),
            ("/bitAnd", "math.bitAnd"),
            ("/bitOr",  "math.bitOr"),
            ("/bitXor", "math.bitXor"),
            ("/bitNot", "math.bitNot"),
            ("/shl",    "math.shl"),
            ("/shr",    "math.shr"),
            ("/exp",    "math.exp"),
            ("/ln",     "math.ln"),
            ("/sin",    "math.sin"),
            ("/cos",    "math.cos"),
            ("/eml",    "math.eml"),
            // Phase 19 (previously missing from module)
            ("/min",    "math.min"),
            ("/max",    "math.max"),
            ("/floor",  "math.floor"),
            ("/ceil",   "math.ceil"),
            ("/round",  "math.round"),
            ("/clamp",  "math.clamp"),
            // Phase 27
            ("/gcd",    "math.gcd"),
            ("/lcm",    "math.lcm"),
            ("/sign",   "math.sign"),
            ("/log2",   "math.log2"),
            ("/log10",  "math.log10"),
        ];
```

### 3B：更新 `~%String`（加新 5 個）

**找到** 現有的 string_morphisms vec（應已包含 Phase 19+21+25 的項目），在尾端加入：

```rust
            // Phase 27
            ("/index_of",   "str.index_of"),
            ("/pad_left",   "str.pad_left"),
            ("/pad_right",  "str.pad_right"),
            ("/trim_start", "str.trim_start"),
            ("/trim_end",   "str.trim_end"),
```

---

## Task 4：更新 genesis.rs

`~%Math` 和 `~%String` 的結構改變，CAID 需重新計算：

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture 2>&1
```

從輸出的 `UPDATE:` 行複製新值，更新：
```rust
pub const SEED_MATH:   &str = "hash:sha256:v1:...";  // ← 更新
pub const SEED_STRING: &str = "hash:sha256:v1:...";  // ← 更新
```

---

## 測試

### `tests/math_p27_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn int(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
fn float(f: f64) -> Value { Value::Atom(AtomKind::Float(f), EffectTag::Pure, None) }
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn combo2(a: Value, b: Value) -> Value {
    use nlang_interpreter::value::ComboVal;
    use indexmap::IndexMap;
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn as_int(v: &Value) -> i64 {
    match v { Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(), o => panic!("expected Int: {:?}", o) }
}
fn as_float(v: &Value) -> f64 {
    match v { Value::Atom(AtomKind::Float(f), _, _) => *f, o => panic!("expected Float: {:?}", o) }
}

#[test]
fn test_math_gcd_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.gcd", combo2(int(12), int(8)));
    assert_eq!(as_int(&r), 4);
}

#[test]
fn test_math_gcd_zero() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.gcd", combo2(int(0), int(5)));
    assert_eq!(as_int(&r), 5);
}

#[test]
fn test_math_lcm_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.lcm", combo2(int(4), int(6)));
    assert_eq!(as_int(&r), 12);
}

#[test]
fn test_math_lcm_with_zero() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.lcm", combo2(int(0), int(7)));
    assert_eq!(as_int(&r), 0);
}

#[test]
fn test_math_sign_positive() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    assert_eq!(as_int(&call(&oo, &mut ctx, "math.sign", int(42))), 1);
}

#[test]
fn test_math_sign_negative() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    assert_eq!(as_int(&call(&oo, &mut ctx, "math.sign", int(-7))), -1);
}

#[test]
fn test_math_sign_zero() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    assert_eq!(as_int(&call(&oo, &mut ctx, "math.sign", int(0))), 0);
}

#[test]
fn test_math_log2_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.log2", float(8.0));
    let f = as_float(&r);
    assert!((f - 3.0).abs() < 1e-9, "log2(8) should be 3.0, got {}", f);
}

#[test]
fn test_math_log2_zero_is_blur() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.log2", float(0.0));
    assert!(matches!(r, Value::Blur(_)), "log2(0) should be Blur");
}

#[test]
fn test_math_log10_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.log10", float(1000.0));
    let f = as_float(&r);
    assert!((f - 3.0).abs() < 1e-9, "log10(1000) should be 3.0, got {}", f);
}

#[test]
fn test_math_log10_zero_is_blur() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.log10", float(0.0));
    assert!(matches!(r, Value::Blur(_)), "log10(0) should be Blur");
}
```

### `tests/str_p27_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn int(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
fn combo2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn as_str(v: &Value) -> &str {
    match v { Value::Atom(AtomKind::Str(s), _, _) => s.as_str(), o => panic!("expected Str: {:?}", o) }
}
fn as_int(v: &Value) -> i64 {
    match v { Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(), o => panic!("expected Int: {:?}", o) }
}
fn is_none(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none")
}

#[test]
fn test_str_index_of_found() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.index_of", combo2(str_val("lo"), str_val("hello world")));
    assert_eq!(as_int(&r), 3);
}

#[test]
fn test_str_index_of_not_found() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.index_of", combo2(str_val("xyz"), str_val("hello")));
    assert!(is_none(&r));
}

#[test]
fn test_str_index_of_at_start() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.index_of", combo2(str_val("he"), str_val("hello")));
    assert_eq!(as_int(&r), 0);
}

#[test]
fn test_str_pad_left_shorter() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.pad_left", combo2(int(6), str_val("hi")));
    assert_eq!(as_str(&r), "    hi");
}

#[test]
fn test_str_pad_left_already_wide() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.pad_left", combo2(int(2), str_val("hello")));
    assert_eq!(as_str(&r), "hello");
}

#[test]
fn test_str_pad_right_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.pad_right", combo2(int(5), str_val("hi")));
    assert_eq!(as_str(&r), "hi   ");
}

#[test]
fn test_str_trim_start_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.trim_start", str_val("   hello"));
    assert_eq!(as_str(&r), "hello");
}

#[test]
fn test_str_trim_end_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.trim_end", str_val("hello   "));
    assert_eq!(as_str(&r), "hello");
}
```

### 加入 `Cargo.toml` 的 test 條目

```toml
[[test]]
name = "math_p27_test"
path = "tests/math_p27_test.rs"

[[test]]
name = "str_p27_test"
path = "tests/str_p27_test.rs"
```

---

## 注意事項

### `bigint_gcd` 的位置
Rust 不允許在閉包之間共用 inner function，但允許在同一個函數體內的所有閉包使用該 inner function。只要 `bigint_gcd` 定義在第一個 `m.insert` **之前**，即可在 `math.gcd` 和 `math.lcm` 的閉包中直接呼叫。

### `math.sign` 對 0.0 的處理
Rust 的 `f.signum()` 對 `0.0` 回傳 `1.0`（正零），對 `-0.0` 回傳 `-1.0`。我們改用 `if f > 0.0 / f < 0.0 / else` 的比較，確保 `0.0` 正確回傳 `0.0`。

### `str.index_of` 回傳 char index 非 byte index
`h.find(n)` 回傳 byte offset；用 `h[..byte_idx].chars().count()` 轉換為 char index，與 `str.char_at` 的索引語義一致。空字串 `needle = ""` 時，`find("")` 回傳 `Some(0)`，char_idx = 0，行為正確。

### SEED_MATH 必須更新
加入 `/min`、`/max`、`/floor`、`/ceil`、`/round`、`/clamp`（Phase 19 補漏）以及 5 個 Phase 27 新態射，共 11 個新欄位，`~%Math` CAID 會改變。若不更新 `SEED_MATH`，`seed_caids_are_stable` 測試失敗。

---

## 驗證步驟

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools

# 1. 編譯
cargo build --manifest-path crates/interpreter/Cargo.toml 2>&1

# 2. 新測試
cargo test --manifest-path crates/interpreter/Cargo.toml math_p27_test -- --nocapture
cargo test --manifest-path crates/interpreter/Cargo.toml str_p27_test -- --nocapture

# 3. 種子更新後穩定性
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture

# 4. 全套不退步
cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：~327 tests, 0 failed
```
