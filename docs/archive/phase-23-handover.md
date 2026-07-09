# Phase 23 交接文件

> 狀態：待實作  
> 前置：Phase 22 完成（274 tests passing）  
> 目標：`~%Time` 擴展 — 修正 now + format / diff / add_ms

---

## 概覽

| 任務 | 說明 |
|:-----|:-----|
| Fix `time.now` | 從返回 `#now` Tag 改為返回真實 ms 時間戳（Int, EffectTag::IO） |
| `time.format` | `{0: fmt_str, 1: ms}` → Str，使用 strftime 格式 |
| `time.diff` | `{0: t1_ms, 1: t2_ms}` → Int（t1 - t2） |
| `time.add_ms` | `{0: offset_ms, 1: timestamp_ms}` → Int |

**位置**：`crates/interpreter/src/builtins/time.rs`（整個檔案重寫）  
**測試檔**：`tests/time_test.rs`（新建，6 個測試）  
預期完成後：274 + 6 ≈ **280 tests**

---

## 背景：`chrono` 已在依賴中

```toml
chrono = { version = "0.4", features = ["serde"] }
```

可直接使用 `chrono::DateTime<Utc>` 和 `dt.format(&fmt_str)` 支援完整 strftime 格式。

---

## 完整重寫 `time.rs`

```rust
use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use chrono::{DateTime, Utc, NaiveDateTime};

pub fn register_time_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // time.now → current Unix timestamp in milliseconds (Int, IO)
    m.insert("time.now".to_string(), Arc::new(|_arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| {
        let ms = Utc::now().timestamp_millis();
        Value::Atom(AtomKind::Int(BigInt::from(ms)), EffectTag::IO, None)
    }) as Arc<BuiltinFn>);

    // time.format: {0: fmt_str, 1: ms} → Str
    // fmt_str uses strftime specifiers: %Y %m %d %H %M %S %.3f etc.
    // Default format (empty string): "%Y-%m-%dT%H:%M:%S%.3fZ"
    m.insert("time.format".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vfmt), Some(vms)) = (c.get_field("0"), c.get_field("1")) {
                let fmt_forced = oo.force(vfmt.clone(), ctx);
                let ms_forced  = oo.force(vms.clone(), ctx);

                let fmt_str = match fmt_forced.collapse() {
                    Value::Atom(AtomKind::Str(s), _, _) => {
                        if s.is_empty() { "%Y-%m-%dT%H:%M:%S%.3fZ".to_string() }
                        else { s.clone() }
                    }
                    _ => return Value::Top,
                };

                let ms_i64: i64 = match ms_forced.collapse() {
                    Value::Atom(AtomKind::Int(n), _, _)   => n.to_i64().unwrap_or(0),
                    Value::Atom(AtomKind::Float(f), _, _) => *f as i64,
                    _ => return Value::Top,
                };

                let dt: DateTime<Utc> = {
                    let secs  = ms_i64 / 1000;
                    let nanos = ((ms_i64 % 1000).abs() * 1_000_000) as u32;
                    match NaiveDateTime::from_timestamp_opt(secs, nanos) {
                        Some(ndt) => DateTime::from_naive_utc_and_offset(ndt, Utc),
                        None      => return Value::Top,
                    }
                };

                let formatted = dt.format(&fmt_str).to_string();
                return Value::Atom(AtomKind::Str(formatted), EffectTag::Pure, None);
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // time.diff: {0: t1_ms, 1: t2_ms} → Int  (t1 - t2, may be negative)
    m.insert("time.diff".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vt1), Some(vt2)) = (c.get_field("0"), c.get_field("1")) {
                let t1 = oo.force(vt1.clone(), ctx);
                let t2 = oo.force(vt2.clone(), ctx);
                let to_i64 = |v: &Value| -> Option<i64> {
                    match v.collapse() {
                        Value::Atom(AtomKind::Int(n), _, _)   => n.to_i64(),
                        Value::Atom(AtomKind::Float(f), _, _) => Some(*f as i64),
                        _ => None,
                    }
                };
                if let (Some(i1), Some(i2)) = (to_i64(&t1), to_i64(&t2)) {
                    return Value::Atom(AtomKind::Int(BigInt::from(i1 - i2)), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // time.add_ms: {0: offset_ms, 1: timestamp_ms} → Int
    m.insert("time.add_ms".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(voffset), Some(vts)) = (c.get_field("0"), c.get_field("1")) {
                let offset = oo.force(voffset.clone(), ctx);
                let ts     = oo.force(vts.clone(), ctx);
                let to_i64 = |v: &Value| -> Option<i64> {
                    match v.collapse() {
                        Value::Atom(AtomKind::Int(n), _, _)   => n.to_i64(),
                        Value::Atom(AtomKind::Float(f), _, _) => Some(*f as i64),
                        _ => None,
                    }
                };
                if let (Some(off), Some(t)) = (to_i64(&offset), to_i64(&ts)) {
                    return Value::Atom(AtomKind::Int(BigInt::from(t + off)), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
}
```

---

## 注意事項

**`NaiveDateTime::from_timestamp_opt` API**：  
chrono 0.4.23+ 的穩定 API。若編譯器警告 deprecated，可改為：
```rust
// chrono 0.4.27+ 替代方案：
DateTime::from_timestamp_millis(ms_i64)
    .map(|dt| dt.format(&fmt_str).to_string())
    .unwrap_or_default()
```
優先用 `NaiveDateTime::from_timestamp_opt`；若有警告再換。

**`time.format` 的 EffectTag**：  
格式化本身是純計算（Pure），即使輸入的時間戳來自 IO。這與 `str.format` 的設計一致。

**`time.diff` 返回可負值**：  
`t1 - t2` 當 `t1 < t2` 時為負，這是正確語義（表示 t1 比 t2 早幾毫秒）。

**`time.now` 的 EffectTag::IO**：  
讀取系統時鐘是副作用，必須保持 `IO`。

**strftime 格式參考**（chrono 支援）：

| 格式碼 | 含義 | 範例 |
|:------|:-----|:-----|
| `%Y` | 4位年份 | `2026` |
| `%m` | 2位月份 | `05` |
| `%d` | 2位日期 | `24` |
| `%H` | 小時（24h） | `14` |
| `%M` | 分鐘 | `30` |
| `%S` | 秒 | `00` |
| `%.3f` | 毫秒（帶前綴點） | `.123` |
| `%Z` | 時區縮寫 | `UTC` |
| `%%` | 字面 `%` | `%` |

---

## 測試（`tests/time_test.rs`）

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}
fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}

fn make_combo_2(a: Value, b: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), a);
    f.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}

fn as_i64(v: &Value) -> i64 {
    match v {
        Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(),
        other => panic!("expected Int, got {:?}", other),
    }
}

fn as_str(v: &Value) -> String {
    match v {
        Value::Atom(AtomKind::Str(s), _, _) => s.clone(),
        other => panic!("expected Str, got {:?}", other),
    }
}

#[test]
fn test_time_now_is_positive_int() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "time.now", Value::Top);
    let ms = as_i64(&r);
    // Should be > 0 (well past Unix epoch) and plausible (> year 2020 = 1577836800000 ms)
    assert!(ms > 1_577_836_800_000i64, "time.now should return a recent timestamp, got {}", ms);
}

#[test]
fn test_time_diff_basic() {
    // diff(1000, 0) → 1000
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let arg = make_combo_2(int_val(1000), int_val(0));
    let r = call(&oo, &mut ctx, "time.diff", arg);
    assert_eq!(as_i64(&r), 1000);
}

#[test]
fn test_time_diff_negative() {
    // diff(0, 1000) → -1000 (t1 is earlier than t2)
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let arg = make_combo_2(int_val(0), int_val(1000));
    let r = call(&oo, &mut ctx, "time.diff", arg);
    assert_eq!(as_i64(&r), -1000);
}

#[test]
fn test_time_add_ms() {
    // add_ms(500, 1000) → 1500
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let arg = make_combo_2(int_val(500), int_val(1000));
    let r = call(&oo, &mut ctx, "time.add_ms", arg);
    assert_eq!(as_i64(&r), 1500);
}

#[test]
fn test_time_format_epoch_date() {
    // format("%Y-%m-%d", 0) → "1970-01-01"
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let arg = make_combo_2(str_val("%Y-%m-%d"), int_val(0));
    let r = call(&oo, &mut ctx, "time.format", arg);
    assert_eq!(as_str(&r), "1970-01-01");
}

#[test]
fn test_time_format_epoch_time() {
    // format("%H:%M:%S", 0) → "00:00:00"
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let arg = make_combo_2(str_val("%H:%M:%S"), int_val(0));
    let r = call(&oo, &mut ctx, "time.format", arg);
    assert_eq!(as_str(&r), "00:00:00");
}
```

---

## 驗證

```bash
cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：~280 tests, 0 failed

cargo test time_test -- --nocapture
```

## 完成後 `~%Time` 狀態

| builtin | 語義 | EffectTag |
|:--------|:-----|:---------:|
| `time.now` | 當前 Unix 時間戳（ms, Int） | IO |
| `time.format` | 格式化時間戳為字串（strftime） | Pure |
| `time.diff` | t1 - t2（ms, Int，可負） | Pure |
| `time.add_ms` | timestamp + offset（ms, Int） | Pure |
