# Phase 21 交接文件

> 狀態：待實作  
> 前置：Phase 20 完成（256 tests passing）  
> 目標：`str.format` — 基本字串插值

---

## 概覽

單一任務：在 `builtins/string.rs` 新增 `str.format`，並新建 `tests/str_format_test.rs`（6 個測試）。

預期完成後：256 + 6 ≈ **262 tests**

---

## 語義定義

```
str.format : {0: fmt_str, 1: args_list} → Str

格式化規則：
  {}      → 下一個自動索引的 args_list 元素（0, 1, 2, ...）
  {N}     → args_list 中第 N 個元素（0-based explicit index）
  {{      → 字面 {（轉義）
  }}      → 字面 }（轉義）
  其他內容 → 原樣保留

值轉換：所有 args_list 元素透過 to_string_plain() 轉成字串
索引越界 → 補空字串（silent empty, no error）
```

範例：
```
str.format("{} + {} = {}", [1, 2, 3])      → "1 + 2 = 3"
str.format("Hello, {}!", ["Alice"])         → "Hello, Alice!"
str.format("{1} then {0}", ["A", "B"])      → "B then A"
str.format("{{literal}}", [])              → "{literal}"
str.format("val: {}", [42])                → "val: 42"
```

---

## 位置

`crates/interpreter/src/builtins/string.rs`，加在 `str.repeat` 之後（`}` 之前）。

---

## Import 確認

`string.rs` 已有（Phase 19 加入）：
```rust
use num_bigint::BigInt;
use std::str::FromStr;
```
`str.format` 不需要額外 import。

---

## 實作

```rust
// ── Phase 21: str.format ──────────────────────────────────────

m.insert("str.format".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(vfmt), Some(vlist)) = (c.get_field("0"), c.get_field("1")) {
            let fmt_forced = oo.force(vfmt.clone(), ctx);
            let list_forced = oo.force(vlist.clone(), ctx);

            let fmt_str = match fmt_forced.collapse() {
                Value::Atom(AtomKind::Str(s), _, _) => s.clone(),
                _ => return Value::Top,
            };

            // Extract args_list items into Vec<String>
            let args: Vec<String> = {
                let mut items = Vec::new();
                let mut i = 0usize;
                loop {
                    match &list_forced {
                        Value::Combo(ref lc) => {
                            match lc.get_field(&i.to_string()) {
                                Some(v) => {
                                    items.push(oo.force(v.clone(), ctx).to_string_plain());
                                    i += 1;
                                }
                                None => break,
                            }
                        }
                        _ => break,
                    }
                }
                items
            };

            // Scan format string
            let mut result = String::with_capacity(fmt_str.len());
            let mut chars = fmt_str.chars().peekable();
            let mut auto_idx = 0usize;

            while let Some(ch) = chars.next() {
                match ch {
                    '{' => {
                        match chars.peek() {
                            Some(&'{') => {
                                // {{ → literal {
                                chars.next();
                                result.push('{');
                            }
                            Some(&'}') => {
                                // {} → auto-indexed arg
                                chars.next();
                                result.push_str(args.get(auto_idx).map(|s| s.as_str()).unwrap_or(""));
                                auto_idx += 1;
                            }
                            _ => {
                                // {N} or unknown — collect until '}'
                                let mut inner = String::new();
                                loop {
                                    match chars.next() {
                                        Some('}') => break,
                                        Some(c)   => inner.push(c),
                                        None      => break,
                                    }
                                }
                                match inner.trim().parse::<usize>() {
                                    Ok(idx) => {
                                        // {N} explicit index
                                        result.push_str(args.get(idx).map(|s| s.as_str()).unwrap_or(""));
                                    }
                                    Err(_) => {
                                        // Unknown placeholder — pass through literally
                                        result.push('{');
                                        result.push_str(&inner);
                                        result.push('}');
                                    }
                                }
                            }
                        }
                    }
                    '}' => {
                        if chars.peek() == Some(&'}') {
                            // }} → literal }
                            chars.next();
                            result.push('}');
                        } else {
                            // Lone } — pass through literally
                            result.push('}');
                        }
                    }
                    other => result.push(other),
                }
            }

            return Value::Atom(AtomKind::Str(result), EffectTag::Pure, None);
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);
```

---

## 注意事項

1. **args 的提取方式**：與 `list.rs` 不同，`string.rs` 沒有 `extract_list_items` helper，所以用 inline loop 逐個讀取 `"0"`, `"1"`, ... 直到 `get_field` 返回 `None`。

2. **越界靜默**：`args.get(idx).unwrap_or("")` — 索引越界時補空字串，不報錯。這讓格式字串可以「超訂」args 而不崩潰。

3. **to_string_plain()**：所有值統一轉字串。Int `42` → `"42"`，Float `3.14` → `"3.14"`，Tag `#true` → `"true"`，Str `"hello"` → `"hello"`（不帶引號），Combo → `"{...}"`。

4. **`{{` / `}}`**：遵循 Python / Rust `format!` 的轉義慣例，讓 `{{` 輸出字面 `{`，`}}` 輸出字面 `}`。

5. **未知佔位符 `{key}`**：非數字內容的 `{...}` 原樣保留，方便未來擴充具名佔位符而不破壞現有行為。

6. **`EffectTag::Pure`**：格式化本身沒有副作用，即使 args 來自 IO 來源，此處保守地使用 Pure（args 已在提取時被 force，effect 可從 args 的實際值追蹤）。如果需要傳播 effect，可改為計算 args 中最大 effect：`args_effect.max(...)` — 但目前暫不處理，保持簡單。

---

## 測試（`tests/str_format_test.rs`）

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}
fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}

fn make_list(items: Vec<Value>) -> Value {
    let mut f = IndexMap::new();
    for (i, v) in items.iter().enumerate() { f.insert(i.to_string(), v.clone()); }
    f.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    f.insert("%len".to_string(), int_val(items.len() as i64));
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_fmt_arg(fmt: &str, args: Vec<Value>) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), str_val(fmt));
    f.insert("1".to_string(), make_list(args));
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call_format(oo: &Ouroboros, ctx: &mut EvalContext, fmt: &str, args: Vec<Value>) -> String {
    let f = oo.builtin_registry.get("str.format").unwrap().clone();
    let arg = make_fmt_arg(fmt, args);
    match f(arg, oo, ctx) {
        Value::Atom(AtomKind::Str(s), _, _) => s,
        other => panic!("expected Str, got {:?}", other),
    }
}

#[test]
fn test_str_format_single_placeholder() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call_format(&oo, &mut ctx, "Hello, {}!", vec![str_val("Alice")]);
    assert_eq!(r, "Hello, Alice!");
}

#[test]
fn test_str_format_multiple_placeholders() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call_format(&oo, &mut ctx, "{} + {} = {}", vec![int_val(1), int_val(2), int_val(3)]);
    assert_eq!(r, "1 + 2 = 3");
}

#[test]
fn test_str_format_explicit_index() {
    // {1} before {0} — reverse order
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call_format(&oo, &mut ctx, "{1} then {0}", vec![str_val("first"), str_val("second")]);
    assert_eq!(r, "second then first");
}

#[test]
fn test_str_format_escape_braces() {
    // {{ → {, }} → }
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call_format(&oo, &mut ctx, "{{literal}}", vec![]);
    assert_eq!(r, "{literal}");
}

#[test]
fn test_str_format_mixed_types() {
    // Int arg in format
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call_format(&oo, &mut ctx, "val: {}", vec![int_val(42)]);
    assert_eq!(r, "val: 42");
}

#[test]
fn test_str_format_out_of_range() {
    // More {} than args → silent empty string for missing slots
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call_format(&oo, &mut ctx, "{} {}", vec![str_val("only")]);
    assert_eq!(r, "only ");
}
```

---

## 驗證

```bash
cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：262 tests, 0 failed

cargo test str_format -- --nocapture
```
