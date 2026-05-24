# Phase 19 交接文件

> 狀態：待實作  
> 前置：Phase 18 完成（230 tests passing）  
> 目標：Math 比較/取整 + String 轉換 + List 聚合

---

## 概覽

| 任務 | 位置 | 新增 builtins | 新增測試數 |
|:-----|:-----|:------------|:---------:|
| Task 1：Math 取整與比較 | `builtins/math.rs` | `math.min`, `math.max`, `math.floor`, `math.ceil`, `math.round`, `math.clamp` | 8 |
| Task 2：String 轉換 | `builtins/string.rs` | `str.parse_int`, `str.from_int`, `str.repeat` | 6 |
| Task 3：List 聚合 | `builtins/list.rs` | `list.count`, `list.zip_with` | 5 |

預期完成後：230 + 19 ≈ **249 tests**

---

## Task 1：Math 取整與比較

### 位置

`crates/interpreter/src/builtins/math.rs`，加在現有 builtins 之後（`register_math_builtins` 函數末尾，`}` 之前）。

### Import 確認

`math.rs` 已有：
```rust
use num_traits::{Signed, Zero, ToPrimitive};
use num_bigint::BigInt;
```
`ToPrimitive` 提供 `.to_f64()` 轉換。無需新增 import。

### 語義

```
math.min   : {0: a, 1: b} → a 或 b（較小者）
math.max   : {0: a, 1: b} → a 或 b（較大者）
  支援 Int×Int, Float×Float, Int×Float（混合 → Float）
  兩個值相等時返回 a

math.floor : n → Float（向下取整，例 3.7 → 3.0, -1.2 → -2.0）
  Int 輸入返回原值（Int 已是整數）

math.ceil  : n → Float（向上取整，例 3.2 → 4.0, -1.7 → -1.0）
  Int 輸入返回原值

math.round : n → Float（四捨五入，例 3.5 → 4.0, 3.4 → 3.0）
  Int 輸入返回原值

math.clamp : {0: lo, 1: hi, 2: x} → x clamped to [lo, hi]
  若 x < lo → lo；若 x > hi → hi；否則 → x
  支援 Int 和 Float（混合時全部轉 Float）
```

**注意**：floor/ceil/round 對 Float 輸入返回 **Float**（不轉 Int），避免 f64→BigInt 的精度問題。例如 `math.floor(3.7)` → `3.0`（Float），不是 `3`（Int）。

### 實作

```rust
// ── Phase 19: Math comparison and rounding ────────────────────

m.insert("math.min".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(va), Some(vb)) = (c.get_field("0"), c.get_field("1")) {
            let a = oo.force(va.clone(), ctx).collapse().clone();
            let b = oo.force(vb.clone(), ctx).collapse().clone();
            let res_e = a.effect().max(b.effect());
            return match (&a, &b) {
                (Value::Atom(AtomKind::Int(ia), _, _), Value::Atom(AtomKind::Int(ib), _, _)) => {
                    Value::Atom(AtomKind::Int(if ia <= ib { ia.clone() } else { ib.clone() }), res_e, None)
                }
                (Value::Atom(AtomKind::Float(fa), _, _), Value::Atom(AtomKind::Float(fb), _, _)) => {
                    Value::Atom(AtomKind::Float(fa.min(*fb)), res_e, None)
                }
                (Value::Atom(AtomKind::Int(ia), _, _), Value::Atom(AtomKind::Float(fb), _, _)) => {
                    let fa = ia.to_f64().unwrap_or(0.0);
                    Value::Atom(AtomKind::Float(fa.min(*fb)), res_e, None)
                }
                (Value::Atom(AtomKind::Float(fa), _, _), Value::Atom(AtomKind::Int(ib), _, _)) => {
                    let fb = ib.to_f64().unwrap_or(0.0);
                    Value::Atom(AtomKind::Float(fa.min(fb)), res_e, None)
                }
                _ => BottomCause::Conflict.into(),
            };
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);

m.insert("math.max".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(va), Some(vb)) = (c.get_field("0"), c.get_field("1")) {
            let a = oo.force(va.clone(), ctx).collapse().clone();
            let b = oo.force(vb.clone(), ctx).collapse().clone();
            let res_e = a.effect().max(b.effect());
            return match (&a, &b) {
                (Value::Atom(AtomKind::Int(ia), _, _), Value::Atom(AtomKind::Int(ib), _, _)) => {
                    Value::Atom(AtomKind::Int(if ia >= ib { ia.clone() } else { ib.clone() }), res_e, None)
                }
                (Value::Atom(AtomKind::Float(fa), _, _), Value::Atom(AtomKind::Float(fb), _, _)) => {
                    Value::Atom(AtomKind::Float(fa.max(*fb)), res_e, None)
                }
                (Value::Atom(AtomKind::Int(ia), _, _), Value::Atom(AtomKind::Float(fb), _, _)) => {
                    let fa = ia.to_f64().unwrap_or(0.0);
                    Value::Atom(AtomKind::Float(fa.max(*fb)), res_e, None)
                }
                (Value::Atom(AtomKind::Float(fa), _, _), Value::Atom(AtomKind::Int(ib), _, _)) => {
                    let fb = ib.to_f64().unwrap_or(0.0);
                    Value::Atom(AtomKind::Float(fa.max(fb)), res_e, None)
                }
                _ => BottomCause::Conflict.into(),
            };
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);

// Helper macro pattern for floor/ceil/round — all single-arg Float ops
m.insert("math.floor".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
    match oo.force(v, ctx).collapse().clone() {
        Value::Atom(AtomKind::Float(f), e, _) => Value::Atom(AtomKind::Float(f.floor()), e, None),
        Value::Atom(AtomKind::Int(i), e, _)   => Value::Atom(AtomKind::Int(i), e, None),
        _ => BottomCause::Conflict.into(),
    }
}) as Arc<BuiltinFn>);

m.insert("math.ceil".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
    match oo.force(v, ctx).collapse().clone() {
        Value::Atom(AtomKind::Float(f), e, _) => Value::Atom(AtomKind::Float(f.ceil()), e, None),
        Value::Atom(AtomKind::Int(i), e, _)   => Value::Atom(AtomKind::Int(i), e, None),
        _ => BottomCause::Conflict.into(),
    }
}) as Arc<BuiltinFn>);

m.insert("math.round".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
    match oo.force(v, ctx).collapse().clone() {
        Value::Atom(AtomKind::Float(f), e, _) => Value::Atom(AtomKind::Float(f.round()), e, None),
        Value::Atom(AtomKind::Int(i), e, _)   => Value::Atom(AtomKind::Int(i), e, None),
        _ => BottomCause::Conflict.into(),
    }
}) as Arc<BuiltinFn>);

m.insert("math.clamp".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(vlo), Some(vhi), Some(vx)) = (c.get_field("0"), c.get_field("1"), c.get_field("2")) {
            let lo = oo.force(vlo.clone(), ctx).collapse().clone();
            let hi = oo.force(vhi.clone(), ctx).collapse().clone();
            let x  = oo.force(vx.clone(),  ctx).collapse().clone();
            let res_e = lo.effect().max(hi.effect()).max(x.effect());
            // Normalize all to f64 for comparison, then return in original type of x
            let to_f = |v: &Value| -> Option<f64> {
                match v {
                    Value::Atom(AtomKind::Float(f), _, _) => Some(*f),
                    Value::Atom(AtomKind::Int(i), _, _) => i.to_f64(),
                    _ => None,
                }
            };
            if let (Some(flo), Some(fhi), Some(fx)) = (to_f(&lo), to_f(&hi), to_f(&x)) {
                let clamped = fx.clamp(flo, fhi);
                // Preserve Int type if x was Int and result is unchanged
                return match &x {
                    Value::Atom(AtomKind::Int(ix), _, _) => {
                        if (clamped - fx).abs() < f64::EPSILON {
                            Value::Atom(AtomKind::Int(ix.clone()), res_e, None)
                        } else {
                            Value::Atom(AtomKind::Float(clamped), res_e, None)
                        }
                    }
                    _ => Value::Atom(AtomKind::Float(clamped), res_e, None),
                };
            }
            return BottomCause::Conflict.into();
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);
```

### 測試

測試檔：`tests/math_rounding_test.rs`（新建）

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;
use nlang_interpreter::value::ComboVal;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn int_val(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
fn float_val(f: f64) -> Value { Value::Atom(AtomKind::Float(f), EffectTag::Pure, None) }

fn make_combo_2(a: Value, b: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), a); f.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn make_combo_3(a: Value, b: Value, c: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), a); f.insert("1".to_string(), b); f.insert("2".to_string(), c);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    let f = oo.builtin_registry.get(name).unwrap().clone();
    f(arg, oo, ctx)
}

fn assert_float(v: &Value, expected: f64) {
    match v { Value::Atom(AtomKind::Float(f), _, _) => assert!((f - expected).abs() < 1e-9, "expected {}, got {}", expected, f), _ => panic!("expected Float, got {:?}", v) }
}
fn assert_int(v: &Value, expected: i64) {
    match v { Value::Atom(AtomKind::Int(n), _, _) => assert_eq!(n, &BigInt::from(expected)), _ => panic!("expected Int, got {:?}", v) }
}

#[test]
fn test_math_min_ints() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.min", make_combo_2(int_val(3), int_val(7)));
    assert_int(&r, 3);
}

#[test]
fn test_math_max_floats() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.max", make_combo_2(float_val(1.5), float_val(2.5)));
    assert_float(&r, 2.5);
}

#[test]
fn test_math_floor() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.floor", float_val(3.7));
    assert_float(&r, 3.0);
    let r2 = call(&oo, &mut ctx, "math.floor", float_val(-1.2));
    assert_float(&r2, -2.0);
}

#[test]
fn test_math_ceil() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.ceil", float_val(3.2));
    assert_float(&r, 4.0);
}

#[test]
fn test_math_round() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.round", float_val(3.5));
    assert_float(&r, 4.0);
    let r2 = call(&oo, &mut ctx, "math.round", float_val(3.4));
    assert_float(&r2, 3.0);
}

#[test]
fn test_math_clamp_in_range() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    // clamp(0, 10, 5) → 5
    let r = call(&oo, &mut ctx, "math.clamp", make_combo_3(int_val(0), int_val(10), int_val(5)));
    assert_int(&r, 5);
}

#[test]
fn test_math_clamp_below() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    // clamp(0.0, 10.0, -3.0) → 0.0
    let r = call(&oo, &mut ctx, "math.clamp", make_combo_3(float_val(0.0), float_val(10.0), float_val(-3.0)));
    assert_float(&r, 0.0);
}

#[test]
fn test_math_clamp_above() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    // clamp(0.0, 10.0, 15.0) → 10.0
    let r = call(&oo, &mut ctx, "math.clamp", make_combo_3(float_val(0.0), float_val(10.0), float_val(15.0)));
    assert_float(&r, 10.0);
}
```

---

## Task 2：String 轉換

### 位置

`crates/interpreter/src/builtins/string.rs`，加在現有 builtins 之後。

### Import 確認

`string.rs` 目前沒有 `num_bigint` import。需要在文件頂部加入：
```rust
use num_bigint::BigInt;
use std::str::FromStr;
```

### 語義

```
str.parse_int : str → Int | Bottom(Conflict)
  "42"  → 42
  "-7"  → -7
  "abc" → Bottom(Conflict, "parse_int: invalid integer \"abc\"")

str.from_int  : Int | Float → str
  42    → "42"
  3.14  → "3.14"
  其他值 → to_string_plain() 的字串

str.repeat : {0: n, 1: s} → str
  repeat("ab", 3) → "ababab"
  n = 0 → ""
  n < 0 → ""（防禦）
```

### 實作

```rust
// ── Phase 19: String conversions ─────────────────────────────

m.insert("str.parse_int".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
    if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
        match BigInt::from_str(s.trim()) {
            Ok(n) => return Value::Atom(AtomKind::Int(n), EffectTag::Pure, None),
            Err(_) => return Value::Bottom(Box::new(crate::value::BottomDetail {
                cause: crate::value::BottomCause::Conflict,
                message: Some(format!("parse_int: invalid integer {:?}", s)),
                ..Default::default()
            })),
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);

m.insert("str.from_int".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
    let forced = oo.force(v, ctx);
    match forced.collapse() {
        Value::Atom(AtomKind::Int(n), e, _) => Value::Atom(AtomKind::Str(n.to_string()), *e, None),
        Value::Atom(AtomKind::Float(f), e, _) => Value::Atom(AtomKind::Str(format!("{}", f)), *e, None),
        other => Value::Atom(AtomKind::Str(other.to_string_plain()), EffectTag::Pure, None),
    }
}) as Arc<BuiltinFn>);

m.insert("str.repeat".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(vn), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
            let fn_v = oo.force(vn.clone(), ctx).collapse().clone();
            let fs   = oo.force(vs.clone(), ctx).collapse().clone();
            if let (Value::Atom(AtomKind::Int(n), _, _), Value::Atom(AtomKind::Str(s), e, _)) = (fn_v, fs) {
                let count = n.to_usize().unwrap_or(0);
                return Value::Atom(AtomKind::Str(s.repeat(count)), e, None);
            }
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);
```

### 注意事項

- `str.parse_int` 需要 `BigInt::from_str`（來自 `std::str::FromStr` trait）和 `num_bigint::BigInt`，必須在文件頂部新增這兩個 import。
- `str.from_int` 命名偏向 Int→Str，但也處理 Float 和其他值（通過 `to_string_plain()`）。
- `str.repeat` 的參數順序：`{0: n, 1: s}`（n 在前），與 `list.take` / `list.drop` 一致（n 在前）。
- `BottomDetail` 需要 `crate::value::BottomDetail`/`BottomCause` 的完整路徑，或確認 import 已存在。查看 `string.rs` 是否已有相關 import；若無，參照 `math.rs` 的 import 模式補充。

### 測試

測試檔：`tests/string_conversion_test.rs`（新建）

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, BottomCause, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn int_val(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
fn float_val(f: f64) -> Value { Value::Atom(AtomKind::Float(f), EffectTag::Pure, None) }

fn make_combo_2(a: Value, b: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), a); f.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}

#[test]
fn test_str_parse_int_ok() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.parse_int", str_val("42"));
    match r { Value::Atom(AtomKind::Int(n), _, _) => assert_eq!(n, BigInt::from(42)), _ => panic!("expected Int") }
}

#[test]
fn test_str_parse_int_negative() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.parse_int", str_val("-7"));
    match r { Value::Atom(AtomKind::Int(n), _, _) => assert_eq!(n, BigInt::from(-7)), _ => panic!("expected Int") }
}

#[test]
fn test_str_parse_int_invalid() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.parse_int", str_val("not_a_number"));
    assert!(matches!(r, Value::Bottom(_)), "expected Bottom on invalid parse");
}

#[test]
fn test_str_from_int() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.from_int", int_val(99));
    match r { Value::Atom(AtomKind::Str(s), _, _) => assert_eq!(s, "99"), _ => panic!("expected Str") }
}

#[test]
fn test_str_from_int_float() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.from_int", float_val(3.14));
    match r { Value::Atom(AtomKind::Str(s), _, _) => assert!(s.contains("3.14"), "got: {}", s), _ => panic!("expected Str") }
}

#[test]
fn test_str_repeat() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.repeat", make_combo_2(int_val(3), str_val("ab")));
    match r { Value::Atom(AtomKind::Str(s), _, _) => assert_eq!(s, "ababab"), _ => panic!("expected Str") }
}
```

---

## Task 3：List 聚合

### 位置

`crates/interpreter/src/builtins/list.rs`，加在 `list.drop` 之後（`}` 之前）。

### 語義

```
list.count : {0: pred_fn, 1: list} → Int
  計算 list 中滿足 pred_fn 的元素個數
  空 list → 0

list.zip_with : {0: f, 1: list_a, 2: list_b} → list
  對 list_a[i] 和 list_b[i] 套用 f({0: a_i, 1: b_i})
  結果長度 = min(len(list_a), len(list_b))（較短者截斷）
```

### 實作

```rust
m.insert("list.count".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(pred_f), Some(list_v)) = (c.get_field("0"), c.get_field("1")) {
            let pred_f = pred_f.clone();
            let list = oo.force(list_v.clone(), ctx);
            let items = extract_list_items(&list);
            let mut count: usize = 0;
            for item in items {
                let result = oo.apply_morphism(pred_f.clone(), item, ctx);
                if result.to_string_plain().trim_start_matches('#') == "true" {
                    count += 1;
                }
            }
            return Value::Atom(AtomKind::Int(BigInt::from(count)), EffectTag::Pure, None);
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);

m.insert("list.zip_with".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(f), Some(va), Some(vb)) = (c.get_field("0"), c.get_field("1"), c.get_field("2")) {
            let f = f.clone();
            let list_a = oo.force(va.clone(), ctx);
            let list_b = oo.force(vb.clone(), ctx);
            let items_a = extract_list_items(&list_a);
            let items_b = extract_list_items(&list_b);
            let min_len = items_a.len().min(items_b.len());
            let mut result: Vec<Value> = Vec::with_capacity(min_len);
            for i in 0..min_len {
                let mut pair_fields = IndexMap::new();
                pair_fields.insert("0".to_string(), items_a[i].clone());
                pair_fields.insert("1".to_string(), items_b[i].clone());
                let pair = Value::Combo(ComboVal::new(pair_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
                let mapped = oo.apply_morphism(f.clone(), pair, ctx);
                result.push(mapped);
            }
            return build_list_value(result);
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);
```

### 注意事項

- `list.count` 用 `extract_list_items`（Phase 17 定義的 inner function）。
- `list.zip_with` 的 `f` 接收 `{0: a, 1: b}`（Cocoon 格式的 pair），而不是兩個獨立參數。這與 `list.zip` 建立的 tuple 格式一致，方便在 `zip_with(f, zip(la, lb))` 的場景中使用相同的 f。
- `list.count` 在空 list 時返回 `0`（Int）。

### 測試

測試檔：`tests/list_aggregate_test.rs`（新建）

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn int_val(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }

fn make_list(items: Vec<Value>) -> Value {
    let mut f = IndexMap::new();
    for (i, v) in items.iter().enumerate() { f.insert(i.to_string(), v.clone()); }
    f.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    f.insert("%len".to_string(), int_val(items.len() as i64));
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_combo_3(a: Value, b: Value, c: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), a); f.insert("1".to_string(), b); f.insert("2".to_string(), c);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}

fn assert_int(v: &Value, expected: i64) {
    match v { Value::Atom(AtomKind::Int(n), _, _) => assert_eq!(n, &BigInt::from(expected)), _ => panic!("expected Int, got {:?}", v) }
}

#[test]
fn test_list_count_empty() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    // count(always_true_pred, []) → 0
    // 用 list.filter 模式的 pred：這裡用 list.any 的測試 helper 建立 pred
    // 測試：空 list 一定是 0
    let pred = oo.builtin_registry.get("refl.is_bottom").unwrap().clone();
    // 任意 pred，空 list count 都是 0
    use indexmap::IndexMap as IM;
    let mut combo = IM::new();
    combo.insert("0".to_string(), Value::Atom(AtomKind::Tag("refl.is_bottom".to_string()), EffectTag::Pure, None));
    combo.insert("1".to_string(), make_list(vec![]));
    // 直接用 arc fn
    let pred_val = Value::Top; // placeholder — count 是 0 不管 pred
    let mut f = IM::new();
    f.insert("0".to_string(), pred_val);
    f.insert("1".to_string(), make_list(vec![]));
    let arg = Value::Combo(ComboVal::new(f, false, IM::new(), EffectTag::Pure, vec![]));
    let r = call(&oo, &mut ctx, "list.count", arg);
    assert_int(&r, 0);
}

#[test]
fn test_list_zip_with_add() {
    // zip_with(math.add, [1,2,3], [10,20,30]) → [11, 22, 33]
    // f = math.add builtin（接受 {0: a, 1: b}）
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let la = make_list(vec![int_val(1), int_val(2), int_val(3)]);
    let lb = make_list(vec![int_val(10), int_val(20), int_val(30)]);
    // math.add 接受 {0: a, 1: b}，這正好是 zip_with 給 f 的 pair 格式
    // 建立一個 Code 值指向 math.add — 實際上在測試中可以直接用 builtin Arc
    // 參照 flat_map_test.rs 的方式建立 f
    // 此處用 Value::Top 作為 f 的佔位，測試 zip_with 的結構正確性
    // 實際的 math.add 整合測試留給執行 AI 根據現有 helper 模式完成
    let _ = (la, lb);
    // → 見 Note
}

#[test]
fn test_list_zip_with_truncates() {
    // zip_with(f, [1,2,3], [10,20]) → 2 elements (shorter list wins)
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let la = make_list(vec![int_val(1), int_val(2), int_val(3)]);
    let lb = make_list(vec![int_val(10), int_val(20)]);
    // 用 Value::Top 作為 f（apply_morphism(Top, pair) → pair，結果是 pair 值）
    let arg = make_combo_3(Value::Top, la, lb);
    let r = call(&oo, &mut ctx, "list.zip_with", arg);
    // 結果應該只有 2 個元素
    if let Value::Combo(ref cv) = r {
        assert!(cv.get_field("0").is_some());
        assert!(cv.get_field("1").is_some());
        assert!(cv.get_field("2").is_none(), "should truncate to min length");
    } else { panic!("expected list combo"); }
}
```

**Note for `test_list_count` 和 `test_list_zip_with_add`**：執行 AI 應參照 `list_query_test.rs` 中建立謂詞函數的方式，使用現有的 helper 模式完成這些測試。若 `make_fn_*` helpers 已在 `flat_map_test.rs` 定義，可以在新測試文件中複用或重寫相同 helpers。

---

## 執行順序

Task 1、2、3 互相獨立：

```bash
# Task 1: math.rs 末尾加 6 個 builtins + 新建 math_rounding_test.rs
# Task 2: string.rs 頂部加 import，末尾加 3 個 builtins + 新建 string_conversion_test.rs  
# Task 3: list.rs 末尾加 2 個 builtins + 新建 list_aggregate_test.rs

cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：~249 tests, 0 failed

cargo test math_rounding -- --nocapture
cargo test string_conversion -- --nocapture
cargo test list_aggregate -- --nocapture
```

## 完成後狀態

| 模組 | builtins 數量 |
|:-----|:------------|
| ~%Math | add, sub, mul, div, rem, abs, pow, bit*, random, exp, ln, sin, cos, sqrt, eml, **min, max, floor, ceil, round, clamp** = 21 |
| ~%String | concat, len, trim, split, join, replace, to_lower, to_upper, starts_with, ends_with, contains, **parse_int, from_int, repeat** = 14 |
| ~%List | at, len, concat, reverse, slice, zip, sort, map, fold, filter, flat_map, any, all, find, head, tail, take, drop, **count, zip_with** = 20 |
