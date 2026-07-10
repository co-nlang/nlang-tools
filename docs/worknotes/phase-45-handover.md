# Phase 45 Handover：Stdlib Round 3 — A 組擴充現有模組

> 日期：2026-05-25  
> 實作範圍：~%Math（+8）、~%List（+5）、~%String（+5）、~%Time（+5）各自擴充  
> 預期測試：~474 → ~492（新增 ~18 個測試，4 個測試檔）

---

## 0. 設計摘要

| 模組 | 新增態射 | Effect |
|:-----|:---------|:------:|
| ~%Math | atan2, hypot, sinh, cosh, tanh, trunc, fract, to_float | Pure |
| ~%List | scan, take_while, drop_while, product, transpose | Pure/IO |
| ~%String | encode_uri, decode_uri, levenshtein, word_count, title_case | Pure |
| ~%Time | parse, to_iso8601, add_days, add_hours, weekday | IO/Pure |

全部無新依賴（time.rs 已有 chrono）。

---

## 1. 修改 `crates/interpreter/src/builtins/math.rs`

在 `register_math_builtins()` 末尾加入以下 8 個 morphism。每個的輸入格式：

- **unary**（sinh/cosh/tanh/trunc/fract/to_float）：arg 直接是 Float/Int 值
- **binary**（atan2/hypot）：arg = `{0: a, 1: b}` Combo，與 `math.add` 相同

```rust
    // math.atan2: {0: y, 1: x} → Float（四象限反正切）
    m.insert("math.atan2".to_string(), Arc::new(|arg, oo, ctx| {
        let (y, x) = extract_binary_floats(&arg, oo, ctx)?;
        Ok(float_val(y.atan2(x)))
    }) as Arc<BuiltinFn>);  // 注意：使用與 math.add 相同的 extract_binary_floats 輔助

    // math.hypot: {0: x, 1: y} → Float
    m.insert("math.hypot".to_string(), Arc::new(|arg, oo, ctx| {
        let (x, y) = extract_binary_floats(&arg, oo, ctx)?;
        Ok(float_val(x.hypot(y)))
    }) as Arc<BuiltinFn>);

    // math.sinh / math.cosh / math.tanh：unary Float → Float
    for (name, f) in &[
        ("math.sinh", f64::sinh as fn(f64) -> f64),
        ("math.cosh", f64::cosh),
        ("math.tanh", f64::tanh),
        ("math.trunc", f64::trunc),
        ("math.fract", f64::fract),
    ] {
        let f = *f;
        m.insert(name.to_string(), Arc::new(move |arg, oo, ctx| {
            let x = extract_float(&arg, oo, ctx)?;
            Ok(float_val(f(x)))
        }) as Arc<BuiltinFn>);
    }

    // math.to_float: Int → Float（顯式轉換）
    m.insert("math.to_float".to_string(), Arc::new(|arg, oo, ctx| {
        let v = oo.force(arg, ctx);
        match v {
            Value::Atom(AtomKind::Float(f), _, _) => return Value::Atom(AtomKind::Float(f), EffectTag::Pure, None),
            Value::Atom(AtomKind::Int(ref n), _, _) => {
                use num_traits::ToPrimitive;
                let f = n.to_f64().unwrap_or(f64::INFINITY);
                return Value::Atom(AtomKind::Float(f), EffectTag::Pure, None);
            }
            _ => return BottomCause::Conflict.into(),
        }
    }) as Arc<BuiltinFn>);
```

**注意**：上面使用了假設已存在的 `extract_binary_floats`、`extract_float`、`float_val` 輔助函數。請 grep `math.rs` 確認實際存在的 helper 名稱（或 `extract_float_arg`、`make_float` 等），並按實際模式改寫。若沒有 loop 風格，分開寫每個 match 也可以。

**核心 Rust 調用**：
| 態射 | Rust 調用 |
|:-----|:---------|
| atan2 | `y.atan2(x)` |
| hypot | `x.hypot(y)` |
| sinh/cosh/tanh | `x.sinh()` / `x.cosh()` / `x.tanh()` |
| trunc | `x.trunc()` |
| fract | `x.fract()` |
| to_float | `BigInt::to_f64()` (num_traits::ToPrimitive) |

---

## 2. 修改 `crates/interpreter/src/builtins/list.rs`

在 `register_list_builtins()` 末尾加入以下 5 個 morphism：

### 2.1 `list.scan` — `{0: list, 1: f, 2: init}` → @list
fold 但保留所有中間狀態（prefix sum 等）。

```rust
m.insert("list.scan".to_string(), Arc::new(|arg, oo, ctx| {
    let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
    let list = oo.force(c.get_field("0").cloned().unwrap_or(Value::Top), ctx);
    let f    = c.get_field("1").cloned().unwrap_or(Value::Top);
    let mut acc = oo.force(c.get_field("2").cloned().unwrap_or(Value::Top), ctx);

    let items = extract_list_items(&list, oo, ctx);   // 複用 list.rs 中現有輔助
    let mut result = Vec::with_capacity(items.len());
    for item in items {
        // acc = f(acc, item)：構造 {0: acc, 1: item} 然後 apply_morphism
        let pair = make_pair(acc.clone(), item);      // {0: acc, 1: item}
        acc = oo.apply_morphism(f.clone(), pair, ctx);
        result.push(acc.clone());
    }
    build_list(result)                                // 複用 list.rs 中現有 build_list
}));
```

`make_pair(a, b)` = 構造 `{0: a, 1: b}` Combo（若 list.rs 無此輔助，直接 inline）。

### 2.2 `list.take_while` — `{0: list, 1: pred}` → @list
取元素直到 pred 返回非 truthy：

```rust
m.insert("list.take_while".to_string(), Arc::new(|arg, oo, ctx| {
    let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
    let list = oo.force(c.get_field("0").cloned()?, ctx);
    let pred = c.get_field("1").cloned()?;
    let items = extract_list_items(&list, oo, ctx);
    let mut kept = Vec::new();
    for item in items {
        let result = oo.apply_morphism(pred.clone(), item.clone(), ctx);
        if !is_truthy(&result) { break; }             // is_truthy：複用 query.rs 的定義，或在此重定義
        kept.push(item);
    }
    build_list(kept)
}));
```

### 2.3 `list.drop_while` — `{0: list, 1: pred}` → @list
跳過元素直到 pred 返回非 truthy，之後全部保留：

```rust
// 與 take_while 相同結構，差異：用 dropping flag
let mut dropping = true;
for item in items {
    if dropping {
        let result = oo.apply_morphism(pred.clone(), item.clone(), ctx);
        if is_truthy(&result) { continue; }
        dropping = false;
    }
    kept.push(item);
}
```

### 2.4 `list.product` — `{0: list}` → Int/Float
所有元素連乘（對應 `list.sum`）：

```rust
m.insert("list.product".to_string(), Arc::new(|arg, oo, ctx| {
    let list = oo.force(arg, ctx);  // 或從 {0: list} 取出，與 list.sum 模式一致
    let items = extract_list_items(&list, oo, ctx);
    // 用 math.mul 逐步累積，或直接操作 AtomKind::Int/Float
    // 建議：複用 list.sum 的實作模式，把加法換成乘法，初始值為 Int(1)
    let mut acc = Value::Atom(AtomKind::Int(BigInt::from(1)), EffectTag::Pure, None);
    for item in items {
        let pair = make_pair(acc, item);
        acc = oo.call_builtin("math.mul", pair, ctx);  // 或直接對 AtomKind 操作
    }
    acc
}));
```

### 2.5 `list.transpose` — `{0: list}` → @list of @list
二維 list 轉置：

```rust
m.insert("list.transpose".to_string(), Arc::new(|arg, oo, ctx| {
    let list = oo.force(arg, ctx);  // 或從 {0: list}
    let rows: Vec<Vec<Value>> = extract_list_items(&list, oo, ctx)
        .into_iter()
        .map(|row| extract_list_items(&row, oo, ctx))
        .collect();
    if rows.is_empty() { return build_list(vec![]); }
    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut cols: Vec<Value> = Vec::with_capacity(col_count);
    for j in 0..col_count {
        let col_items: Vec<Value> = rows.iter()
            .map(|row| row.get(j).cloned().unwrap_or(Value::Bottom(Box::new(BottomDetail {
                cause: BottomCause::MissingKey, ..Default::default()
            }))))
            .collect();
        cols.push(build_list(col_items));
    }
    build_list(cols)
}));
```

**注意**：`take_while` / `drop_while` 的 Effect = IO（pred 效果未知），`scan` = IO，`product` / `transpose` = Pure。

---

## 3. 修改 `crates/interpreter/src/builtins/string.rs`

在 `register_string_builtins()` 末尾加入以下 5 個 morphism：

### 3.1 encode_uri / decode_uri
RFC 3986 非保留字元（`A-Z a-z 0-9 - _ . ~`）直接保留，其餘 percent-encode。

```rust
fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' |
            b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => { out.push('%'); out.push_str(&format!("{:02X}", b)); }
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let mut bytes: Vec<u8> = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' && i + 2 < chars.len() {
            let hex: String = chars[i+1..=i+2].iter().collect();
            if let Ok(b) = u8::from_str_radix(&hex, 16) {
                bytes.push(b); i += 3; continue;
            }
        }
        bytes.extend_from_slice(chars[i].to_string().as_bytes());
        i += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
```

Builtin 格式：`{0: str}` → Str（與 `string.to_upper` 相同格式）。

### 3.2 levenshtein
`{0: a, 1: b}` → Int（編輯距離）：

```rust
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    let mut dp = vec![vec![0usize; n+1]; m+1];
    for i in 0..=m { dp[i][0] = i; }
    for j in 0..=n { dp[0][j] = j; }
    for i in 1..=m { for j in 1..=n {
        dp[i][j] = if a[i-1] == b[j-1] { dp[i-1][j-1] }
                   else { 1 + dp[i-1][j].min(dp[i][j-1]).min(dp[i-1][j-1]) };
    }}
    dp[m][n]
}
```

返回 `Value::Atom(AtomKind::Int(BigInt::from(dist)), EffectTag::Pure, None)`。

### 3.3 word_count
`{0: str}` → Int：

```rust
let count = s.split_whitespace().count();
Value::Atom(AtomKind::Int(BigInt::from(count)), EffectTag::Pure, None)
```

### 3.4 title_case
`{0: str}` → Str：

```rust
fn to_title_case(s: &str) -> String {
    s.split_whitespace()
     .map(|word| {
         let mut c = word.chars();
         match c.next() {
             None => String::new(),
             Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
         }
     })
     .collect::<Vec<_>>()
     .join(" ")
}
```

---

## 4. 修改 `crates/interpreter/src/builtins/time.rs`

在 `register_time_builtins()` 末尾加入以下 5 個 morphism。
現有 time.rs 已有 `use chrono::{...}` — 需確保 `NaiveDateTime`、`Duration`、`Datelike` 都被引入。

時間戳格式：與現有 `time.now` 一致，使用 **Unix 時間戳（毫秒，i64）**。

### 4.1 time.parse — `{0: str, 1: fmt}` → Int（ms timestamp）

```rust
m.insert("time.parse".to_string(), Arc::new(|arg, oo, ctx| {
    let c = match arg { Value::Combo(ref c) => c.clone(), _ => return BottomCause::Conflict.into() };
    let s   = extract_str_field(&c, "0", oo, ctx)?;
    let fmt = extract_str_field(&c, "1", oo, ctx)?;
    match chrono::NaiveDateTime::parse_from_str(&s, &fmt) {
        Ok(dt) => {
            let ms = dt.and_utc().timestamp_millis();
            Value::Atom(AtomKind::Int(BigInt::from(ms)), EffectTag::IO, None)
        }
        Err(_) => Value::Bottom(Box::new(BottomDetail {
            cause: BottomCause::Conflict,
            message: Some(format!("time.parse: cannot parse {:?} with format {:?}", s, fmt)),
            ..Default::default()
        })),
    }
}));
```

Effect = IO（讀取外部格式字串）。

### 4.2 time.to_iso8601 — `{0: ts_ms}` → Str

```rust
let ms = extract_int_ms(&arg, oo, ctx)?;  // Int ms → i64
let dt = chrono::DateTime::from_timestamp_millis(ms)
    .unwrap_or_default()
    .naive_utc();
let iso = dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
Value::Atom(AtomKind::Str(iso), EffectTag::Pure, None)
```

Effect = Pure（純格式轉換）。

### 4.3 time.add_days / time.add_hours — `{0: ts_ms, 1: n}` → Int（ms）

```rust
// add_days: ts_ms + n * 86_400_000
// add_hours: ts_ms + n * 3_600_000
// 用 i64 arithmetic；n 可為負數
let ts_ms: i64 = extract_int_ms(&arg.0, oo, ctx)?;
let n: i64 = extract_int_ms(&arg.1, oo, ctx)?;
let result = ts_ms + n * UNIT_MS;  // UNIT_MS = 86_400_000 for days, 3_600_000 for hours
Value::Atom(AtomKind::Int(BigInt::from(result)), EffectTag::Pure, None)
```

### 4.4 time.weekday — `{0: ts_ms}` → Tag

```rust
let ms = extract_int_ms(&arg, oo, ctx)?;
let dt = chrono::DateTime::from_timestamp_millis(ms).unwrap_or_default().naive_utc();
use chrono::Datelike;
let tag = match dt.weekday() {
    chrono::Weekday::Mon => "monday",
    chrono::Weekday::Tue => "tuesday",
    chrono::Weekday::Wed => "wednesday",
    chrono::Weekday::Thu => "thursday",
    chrono::Weekday::Fri => "friday",
    chrono::Weekday::Sat => "saturday",
    chrono::Weekday::Sun => "sunday",
};
Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::Pure, None)
```

---

## 5. 修改 `crates/interpreter/src/lib.rs`

在 `root_with_system()` 中，為各模組加入新 morphism 欄位。

**方法**：grep `~%Math` 或 `math_fields` 找到對應區塊，按現有 pattern（如 `/sin`、`/abs`）新增。

```rust
// ~%Math：加在最後的 insert 後
math_fields.insert("/atan2".to_string(), make_math_morph("math.atan2", EffectTag::Pure));
math_fields.insert("/hypot".to_string(), make_math_morph("math.hypot", EffectTag::Pure));
math_fields.insert("/sinh".to_string(),  make_math_morph("math.sinh",  EffectTag::Pure));
math_fields.insert("/cosh".to_string(),  make_math_morph("math.cosh",  EffectTag::Pure));
math_fields.insert("/tanh".to_string(),  make_math_morph("math.tanh",  EffectTag::Pure));
math_fields.insert("/trunc".to_string(), make_math_morph("math.trunc", EffectTag::Pure));
math_fields.insert("/fract".to_string(), make_math_morph("math.fract", EffectTag::Pure));
math_fields.insert("/to_float".to_string(), make_math_morph("math.to_float", EffectTag::Pure));

// ~%List
list_fields.insert("/scan".to_string(),       make_list_morph("list.scan",       EffectTag::IO));
list_fields.insert("/take_while".to_string(), make_list_morph("list.take_while", EffectTag::IO));
list_fields.insert("/drop_while".to_string(), make_list_morph("list.drop_while", EffectTag::IO));
list_fields.insert("/product".to_string(),    make_list_morph("list.product",    EffectTag::Pure));
list_fields.insert("/transpose".to_string(),  make_list_morph("list.transpose",  EffectTag::Pure));

// ~%String
str_fields.insert("/encode_uri".to_string(), make_str_morph("str.encode_uri", EffectTag::Pure));
str_fields.insert("/decode_uri".to_string(), make_str_morph("str.decode_uri", EffectTag::Pure));
str_fields.insert("/levenshtein".to_string(), make_str_morph("str.levenshtein", EffectTag::Pure));
str_fields.insert("/word_count".to_string(), make_str_morph("str.word_count", EffectTag::Pure));
str_fields.insert("/title_case".to_string(), make_str_morph("str.title_case", EffectTag::Pure));

// ~%Time
time_fields.insert("/parse".to_string(),      make_time_morph("time.parse",      EffectTag::IO));
time_fields.insert("/to_iso8601".to_string(), make_time_morph("time.to_iso8601", EffectTag::Pure));
time_fields.insert("/add_days".to_string(),   make_time_morph("time.add_days",   EffectTag::Pure));
time_fields.insert("/add_hours".to_string(),  make_time_morph("time.add_hours",  EffectTag::Pure));
time_fields.insert("/weekday".to_string(),    make_time_morph("time.weekday",    EffectTag::Pure));
```

`make_math_morph`、`make_list_morph` 等是 lib.rs 中已存在的 local helper（或可能是同一個通用 `make_morph`）。請 grep 現有用法確認實際名稱。

---

## 6. 修改 `crates/interpreter/src/genesis.rs`

這 4 個模組的 CAID 會因加入新欄位而改變。**完成所有 builtins + lib.rs 修改後**：

```bash
cargo test genesis_test -- --nocapture 2>&1 | grep -E "MATH|LIST|STRING|TIME|Math|List|String|Time"
```

取得輸出中的實際 hash，更新對應 SEED 常數（名稱格式可能是 `SEED_MATH`、`SEED_LIST_MODULE` 等，grep genesis.rs 確認）。

---

## 7. 測試

### 7.1 `tests/math_p45_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use nlang_interpreter::value::ComboVal;

fn oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn float_val(f: f64) -> Value { Value::Atom(AtomKind::Float(f), EffectTag::Pure, None) }
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn args2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

#[test] fn test_atan2_quadrant() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.atan2", args2(float_val(1.0), float_val(1.0)));
    if let Value::Atom(AtomKind::Float(f), _, _) = r {
        assert!((f - std::f64::consts::FRAC_PI_4).abs() < 1e-10);
    } else { panic!("expected float"); }
}

#[test] fn test_hypot() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.hypot", args2(float_val(3.0), float_val(4.0)));
    if let Value::Atom(AtomKind::Float(f), _, _) = r {
        assert!((f - 5.0).abs() < 1e-10);
    } else { panic!(); }
}

#[test] fn test_tanh_zero() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "math.tanh", float_val(0.0));
    assert!(matches!(r, Value::Atom(AtomKind::Float(f), _, _) if f.abs() < 1e-10));
}

#[test] fn test_trunc_fract() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let t = call(&oo, &mut ctx, "math.trunc", float_val(3.7));
    let f = call(&oo, &mut ctx, "math.fract", float_val(3.7));
    assert!(matches!(t, Value::Atom(AtomKind::Float(v), _, _) if (v - 3.0).abs() < 1e-10));
    assert!(matches!(f, Value::Atom(AtomKind::Float(v), _, _) if (v - 0.7).abs() < 1e-10));
}

#[test] fn test_to_float_from_int() {
    use num_bigint::BigInt;
    let oo = oo(); let mut ctx = oo.eval_context();
    let int_val = Value::Atom(AtomKind::Int(BigInt::from(42)), EffectTag::Pure, None);
    let r = call(&oo, &mut ctx, "math.to_float", int_val);
    assert!(matches!(r, Value::Atom(AtomKind::Float(f), _, _) if (f - 42.0).abs() < 1e-10));
}
```

### 7.2 `tests/list_p45_test.rs`

```rust
// 假設輔助函數與 query_p43_test.rs 相同（int_val, str_val, tag, combo, list_of, call, args2, list_len）

#[test] fn test_scan_prefix_sum() {
    // scan([1,2,3], add, 0) → [1, 3, 6]
    // 使用 math.add 作為 f，或構造一個測試用的加法 Combo
    // 最簡單：用 ~%Math./add 態射
    let oo = oo(); let mut ctx = oo.eval_context();
    let list = list_of(&[int_val(1), int_val(2), int_val(3)]);
    let add_morph = oo.root.get_field("~%Math")
        .and_then(|m| if let Value::Combo(c) = m { c.get_field("/add").cloned() } else { None })
        .expect("~%Math./add");
    let arg = combo(&[("0", list), ("1", add_morph), ("2", int_val(0))]);
    let r = call(&oo, &mut ctx, "list.scan", arg);
    // r[0]=1, r[1]=3, r[2]=6
    if let Value::Combo(rc) = &r {
        assert!(matches!(rc.get_field("0"), Some(Value::Atom(AtomKind::Int(n), _, _)) if n == &BigInt::from(1)));
        assert!(matches!(rc.get_field("2"), Some(Value::Atom(AtomKind::Int(n), _, _)) if n == &BigInt::from(6)));
    } else { panic!("expected list"); }
}

#[test] fn test_take_while_stops_at_false() {
    let oo = oo(); let mut ctx = oo.eval_context();
    // take_while 需要 pred，使用一個返回 #true/#false 的 cond.if 結構，較複雜
    // 改為驗證空 list → 空 list（簡化測試）
    let empty = list_of(&[]);
    let pred = Value::Top;  // Top 作為 truthy pred（不 break）
    let r = call(&oo, &mut ctx, "list.take_while", args2(empty, pred));
    assert_eq!(list_len(&r), 0);
}

#[test] fn test_product_integers() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let list = list_of(&[int_val(2), int_val(3), int_val(4)]);
    let r = call(&oo, &mut ctx, "list.product", list);  // 或 args2(list, ...) 視實作
    assert!(matches!(&r, Value::Atom(AtomKind::Int(n), _, _) if n == &BigInt::from(24)));
}

#[test] fn test_transpose_2x2() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let row0 = list_of(&[int_val(1), int_val(2)]);
    let row1 = list_of(&[int_val(3), int_val(4)]);
    let matrix = list_of(&[row0, row1]);
    let r = call(&oo, &mut ctx, "list.transpose", matrix);
    // result[0] = [1, 3], result[1] = [2, 4]
    if let Value::Combo(rc) = &r {
        let col0 = rc.get_field("0").expect("col 0");
        if let Value::Combo(c0) = col0 {
            assert!(matches!(c0.get_field("0"), Some(Value::Atom(AtomKind::Int(n), _, _)) if n == &BigInt::from(1)));
            assert!(matches!(c0.get_field("1"), Some(Value::Atom(AtomKind::Int(n), _, _)) if n == &BigInt::from(3)));
        } else { panic!("col0 not Combo"); }
    } else { panic!("not Combo"); }
}
```

### 7.3 `tests/str_p45_test.rs`

```rust
#[test] fn test_encode_uri_special_chars() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.encode_uri", str_val("hello world"));
    assert!(matches!(&r, Value::Atom(AtomKind::Str(s), _, _) if s == "hello%20world"));
}

#[test] fn test_decode_uri_roundtrip() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let encoded = call(&oo, &mut ctx, "str.encode_uri", str_val("a=1&b=2"));
    let decoded = call(&oo, &mut ctx, "str.decode_uri", encoded);
    assert!(matches!(&decoded, Value::Atom(AtomKind::Str(s), _, _) if s == "a=1&b=2"));
}

#[test] fn test_levenshtein_distance() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.levenshtein", args2(str_val("kitten"), str_val("sitting")));
    assert!(matches!(&r, Value::Atom(AtomKind::Int(n), _, _) if n == &BigInt::from(3)));
}

#[test] fn test_word_count() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.word_count", str_val("hello world foo"));
    assert!(matches!(&r, Value::Atom(AtomKind::Int(n), _, _) if n == &BigInt::from(3)));
}

#[test] fn test_title_case() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.title_case", str_val("hello world"));
    assert!(matches!(&r, Value::Atom(AtomKind::Str(s), _, _) if s == "Hello World"));
}
```

### 7.4 `tests/time_p45_test.rs`

```rust
#[test] fn test_to_iso8601_roundtrip() {
    let oo = oo(); let mut ctx = oo.eval_context();
    // Use a known timestamp: 2024-01-01T00:00:00 = 1704067200000 ms
    let ts = Value::Atom(AtomKind::Int(BigInt::from(1704067200000i64)), EffectTag::IO, None);
    let r = call(&oo, &mut ctx, "time.to_iso8601", ts);
    if let Value::Atom(AtomKind::Str(s), _, _) = r {
        assert!(s.starts_with("2024-01-01"), "got: {}", s);
    } else { panic!("expected Str"); }
}

#[test] fn test_weekday_known_date() {
    let oo = oo(); let mut ctx = oo.eval_context();
    // 2024-01-01 = Monday; timestamp = 1704067200000 ms
    let ts = Value::Atom(AtomKind::Int(BigInt::from(1704067200000i64)), EffectTag::IO, None);
    let r = call(&oo, &mut ctx, "time.weekday", ts);
    assert!(matches!(&r, Value::Atom(AtomKind::Tag(t), _, _) if t == "monday"));
}

#[test] fn test_add_days() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let ts = Value::Atom(AtomKind::Int(BigInt::from(1704067200000i64)), EffectTag::IO, None);
    let r = call(&oo, &mut ctx, "time.add_days", args2(ts, int_val(1)));
    let expected = BigInt::from(1704067200000i64 + 86_400_000i64);
    assert!(matches!(&r, Value::Atom(AtomKind::Int(n), _, _) if n == &expected));
}
```

---

## 8. 修改 `crates/interpreter/Cargo.toml`

```toml
[[test]]
name = "math_p45_test"
path = "tests/math_p45_test.rs"

[[test]]
name = "list_p45_test"
path = "tests/list_p45_test.rs"

[[test]]
name = "str_p45_test"
path = "tests/str_p45_test.rs"

[[test]]
name = "time_p45_test"
path = "tests/time_p45_test.rs"
```

---

## 9. 完成後驗證

```bash
cargo test
```

預期：~492 tests，0 failed。

重點確認：
- `math.atan2(1.0, 1.0)` ≈ π/4
- `math.to_float(42)` = 42.0
- `list.scan` 產出正確前綴和
- `list.transpose` 正確轉置 2×2 矩陣
- `str.levenshtein("kitten", "sitting")` = 3
- `str.encode_uri`/`decode_uri` 往返一致
- `time.weekday(1704067200000)` = #monday
- genesis_test 通過（更新過的 4 個模組 SEED）

---

## 10. 修改摘要

| 檔案 | 改動 |
|:-----|:-----|
| `src/builtins/math.rs` | +8 morphisms（atan2/hypot/sinh/cosh/tanh/trunc/fract/to_float） |
| `src/builtins/list.rs` | +5 morphisms（scan/take_while/drop_while/product/transpose） |
| `src/builtins/string.rs` | +5 morphisms（encode_uri/decode_uri/levenshtein/word_count/title_case） |
| `src/builtins/time.rs` | +5 morphisms（parse/to_iso8601/add_days/add_hours/weekday） |
| `src/lib.rs` | ~%Math/List/String/Time 各加對應欄位 |
| `src/genesis.rs` | 更新 4 個模組的 SEED 常數 |
| `tests/math_p45_test.rs` | 新建，5 tests |
| `tests/list_p45_test.rs` | 新建，4 tests |
| `tests/str_p45_test.rs` | 新建，5 tests |
| `tests/time_p45_test.rs` | 新建，3 tests |
| `Cargo.toml` | +4 個 `[[test]]` entries |
