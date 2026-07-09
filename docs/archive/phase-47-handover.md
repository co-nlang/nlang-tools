# Phase 47 Handover：Stdlib Round 3 — C1+C2+C3（~%Csv + ~%Url + ~%Toml）

> 日期：2026-05-25  
> 實作範圍：~%Csv（4 態射，手寫解析器）、~%Url（5 態射，`url` crate）、~%Toml（2 態射，`toml` crate）  
> 新增依賴：`url = "2"`, `toml = "0.8"`（Cargo.toml `[dependencies]`）  
> 預期測試：~504 → ~514（新增 ~10 個測試，3 個測試檔）

---

## 0. 設計摘要

| 模組 | 態射 | 依賴策略 |
|:-----|:-----|:---------|
| ~%Csv | parse / parse_with_headers / stringify / read_csv | 手寫（無新 dep） |
| ~%Url | parse / encode / decode / join / query_params | `url = "2"` |
| ~%Toml | parse / stringify | `toml = "0.8"` |

**~%Csv 手寫解析器**：支援 RFC 4180（逗號分隔、雙引號括住欄位、`""` 轉義引號、`\r\n` 或 `\n` 換行）。
不支援：自定分隔符、unicode 以外的編碼（保留為未來擴充）。

---

## 1. 修改 `crates/interpreter/Cargo.toml`

### 1.1 新增依賴

```toml
[dependencies]
# ... 現有依賴 ...
url = "2"
toml = "0.8"
```

### 1.2 新增測試

```toml
[[test]]
name = "csv_p47_test"
path = "tests/csv_p47_test.rs"

[[test]]
name = "url_p47_test"
path = "tests/url_p47_test.rs"

[[test]]
name = "toml_p47_test"
path = "tests/toml_p47_test.rs"
```

---

## 2. 新建 `crates/interpreter/src/builtins/csv.rs`

```rust
use std::sync::Arc;
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, BottomCause, BottomDetail, ComboVal};
use nlang_parser::ast::AtomKind;

// ── CSV parser ───────────────────────────────────────────────────────────────

/// Parse CSV string → Vec<Vec<String>> (rows of fields).
/// Handles RFC 4180: quoted fields, "" escape, \r\n or \n line endings.
fn parse_csv(input: &str) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut current_field = String::new();
    let mut chars = input.chars().peekable();
    let mut in_quotes = false;

    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                if in_quotes {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        current_field.push('"');
                    } else {
                        in_quotes = false;
                    }
                } else {
                    in_quotes = true;
                }
            }
            ',' if !in_quotes => {
                current_row.push(current_field.clone());
                current_field.clear();
            }
            '\r' if !in_quotes => {
                if chars.peek() == Some(&'\n') { chars.next(); }
                current_row.push(current_field.clone());
                current_field.clear();
                rows.push(current_row.clone());
                current_row.clear();
            }
            '\n' if !in_quotes => {
                current_row.push(current_field.clone());
                current_field.clear();
                rows.push(current_row.clone());
                current_row.clear();
            }
            _ => current_field.push(ch),
        }
    }
    // Flush last field/row (handle missing final newline)
    if !current_field.is_empty() || !current_row.is_empty() {
        current_row.push(current_field);
        if !current_row.is_empty() { rows.push(current_row); }
    }
    rows
}

/// Escape a CSV field: add quotes if contains comma, quote, or newline.
fn escape_csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ── Value builders ───────────────────────────────────────────────────────────

fn str_atom(s: impl Into<String>) -> Value {
    Value::Atom(AtomKind::Str(s.into()), EffectTag::Pure, None)
}

fn build_list(items: Vec<Value>) -> Value {
    let mut m = IndexMap::new();
    m.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    for (i, v) in items.iter().enumerate() { m.insert(i.to_string(), v.clone()); }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn extract_list_items(v: &Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Vec<Value> {
    if let Value::Combo(c) = v {
        let mut out = Vec::new();
        for i in 0u32.. {
            match c.get_field(&i.to_string()) {
                Some(v) => out.push(oo.force(v.clone(), ctx)),
                None => break,
            }
        }
        out
    } else { vec![] }
}

// ── Builtins ─────────────────────────────────────────────────────────────────

pub fn register_csv_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // csv.parse: {0: str} → @list of @list（每 row 為 @list of Str）
    m.insert("csv.parse".to_string(), Arc::new(|arg, oo, ctx| {
        let v = oo.force(arg, ctx);
        // arg may be {0: str} or the string directly
        let s = match &v {
            Value::Atom(AtomKind::Str(s), _, _) => s.clone(),
            Value::Combo(c) => match c.get_field("0") {
                Some(v) => match oo.force(v.clone(), ctx) {
                    Value::Atom(AtomKind::Str(s), _, _) => s,
                    _ => return BottomCause::Conflict.into(),
                },
                None => return BottomCause::Conflict.into(),
            },
            _ => return BottomCause::Conflict.into(),
        };
        let rows = parse_csv(&s);
        let row_vals: Vec<Value> = rows.into_iter()
            .map(|row| build_list(row.into_iter().map(|f| str_atom(f)).collect()))
            .collect();
        build_list(row_vals)
    }) as Arc<BuiltinFn>);

    // csv.parse_with_headers: {0: str} → @list of Combo（key = 欄位名，value = Str）
    // 第一行為 header，後續每行生成 {field_name: value} Combo
    m.insert("csv.parse_with_headers".to_string(), Arc::new(|arg, oo, ctx| {
        let v = oo.force(arg, ctx);
        let s = match &v {
            Value::Atom(AtomKind::Str(s), _, _) => s.clone(),
            Value::Combo(c) => match c.get_field("0") {
                Some(v) => match oo.force(v.clone(), ctx) {
                    Value::Atom(AtomKind::Str(s), _, _) => s,
                    _ => return BottomCause::Conflict.into(),
                },
                None => return BottomCause::Conflict.into(),
            },
            _ => return BottomCause::Conflict.into(),
        };
        let mut rows = parse_csv(&s);
        if rows.is_empty() { return build_list(vec![]); }
        let headers = rows.remove(0);
        let record_vals: Vec<Value> = rows.into_iter().map(|row| {
            let mut fields = IndexMap::new();
            for (i, header) in headers.iter().enumerate() {
                let val = row.get(i).cloned().unwrap_or_default();
                fields.insert(header.clone(), str_atom(val));
            }
            Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]))
        }).collect();
        build_list(record_vals)
    }) as Arc<BuiltinFn>);

    // csv.stringify: {0: list_of_lists} → Str（@list of @list → CSV 字串）
    m.insert("csv.stringify".to_string(), Arc::new(|arg, oo, ctx| {
        let v = oo.force(arg, ctx);
        let outer = match &v {
            Value::Combo(_) => v.clone(),
            _ => return BottomCause::Conflict.into(),
        };
        let rows = extract_list_items(&outer, oo, ctx);
        let mut lines: Vec<String> = Vec::new();
        for row in rows {
            let fields = extract_list_items(&row, oo, ctx);
            let escaped: Vec<String> = fields.iter().map(|v| {
                match v {
                    Value::Atom(AtomKind::Str(s), _, _) => escape_csv_field(s),
                    _ => escape_csv_field(&format!("{:?}", v)),
                }
            }).collect();
            lines.push(escaped.join(","));
        }
        Value::Atom(AtomKind::Str(lines.join("\n")), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);

    // csv.read_csv: {0: path} → @list of @list（IO，從檔案讀取）
    m.insert("csv.read_csv".to_string(), Arc::new(|arg, oo, ctx| {
        let v = oo.force(arg, ctx);
        let path = match &v {
            Value::Atom(AtomKind::Str(s), _, _) => s.clone(),
            Value::Combo(c) => match c.get_field("0") {
                Some(v) => match oo.force(v.clone(), ctx) {
                    Value::Atom(AtomKind::Str(s), _, _) => s,
                    _ => return BottomCause::Conflict.into(),
                },
                None => return BottomCause::Conflict.into(),
            },
            _ => return BottomCause::Conflict.into(),
        };
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let rows = parse_csv(&content);
                let row_vals: Vec<Value> = rows.into_iter()
                    .map(|row| build_list(row.into_iter().map(|f| str_atom(f)).collect()))
                    .collect();
                // Set effect to IO
                let inner_list = build_list(row_vals);
                // Rebuild with IO effect
                if let Value::Combo(mut c) = inner_list {
                    c = ComboVal::new(c.fields(), false, IndexMap::new(), EffectTag::IO, vec![]);
                    Value::Combo(c)
                } else { inner_list }
            }
            Err(e) => Value::Bottom(Box::new(BottomDetail {
                cause: BottomCause::Conflict,
                message: Some(format!("csv.read_csv: {}", e)),
                ..Default::default()
            })),
        }
    }) as Arc<BuiltinFn>);
}
```

**注意**：`ComboVal::fields()` 需要回傳可用於重建的 IndexMap。若 API 不同，使用 `all_fields_iter()` 重建。

---

## 3. 新建 `crates/interpreter/src/builtins/url.rs`

```rust
use std::sync::Arc;
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, BottomCause, BottomDetail, ComboVal};
use nlang_parser::ast::AtomKind;

fn str_atom(s: impl Into<String>) -> Value {
    Value::Atom(AtomKind::Str(s.into()), EffectTag::Pure, None)
}

fn conflict() -> Value { BottomCause::Conflict.into() }

fn extract_str(v: &Value) -> Option<String> {
    match v { Value::Atom(AtomKind::Str(s), _, _) => Some(s.clone()), _ => None }
}

// ── URL percent-encoding（手寫，避免依賴 url crate 的內部函數）────────────────

fn url_encode(s: &str) -> String {
    // Encode all chars except RFC 3986 unreserved: A-Z a-z 0-9 - _ . ~
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

fn url_decode(s: &str) -> String {
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

pub fn register_url_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // url.parse: {0: str} → Combo {scheme, host, path, query, fragment}
    m.insert("url.parse".to_string(), Arc::new(|arg, oo, ctx| {
        let v = oo.force(arg, ctx);
        let s = match &v {
            Value::Atom(AtomKind::Str(s), _, _) => s.clone(),
            Value::Combo(c) => match c.get_field("0") {
                Some(v) => match oo.force(v.clone(), ctx) {
                    Value::Atom(AtomKind::Str(s), _, _) => s,
                    _ => return conflict(),
                },
                None => return conflict(),
            },
            _ => return conflict(),
        };
        match url::Url::parse(&s) {
            Ok(u) => {
                let mut fields = IndexMap::new();
                fields.insert("scheme".to_string(), str_atom(u.scheme()));
                fields.insert("host".to_string(),   str_atom(u.host_str().unwrap_or("")));
                fields.insert("path".to_string(),   str_atom(u.path()));
                fields.insert("query".to_string(),  str_atom(u.query().unwrap_or("")));
                fields.insert("fragment".to_string(), str_atom(u.fragment().unwrap_or("")));
                Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]))
            }
            Err(e) => Value::Bottom(Box::new(BottomDetail {
                cause: BottomCause::Conflict,
                message: Some(format!("url.parse: {}", e)),
                ..Default::default()
            })),
        }
    }) as Arc<BuiltinFn>);

    // url.encode: {0: str} → Str（percent-encode）
    m.insert("url.encode".to_string(), Arc::new(|arg, oo, ctx| {
        let v = oo.force(arg, ctx);
        let s = match &v {
            Value::Atom(AtomKind::Str(s), _, _) => s.clone(),
            Value::Combo(c) => match c.get_field("0") {
                Some(v) => match oo.force(v.clone(), ctx) {
                    Value::Atom(AtomKind::Str(s), _, _) => s,
                    _ => return conflict(),
                },
                None => return conflict(),
            },
            _ => return conflict(),
        };
        str_atom(url_encode(&s))
    }) as Arc<BuiltinFn>);

    // url.decode: {0: str} → Str（percent-decode）
    m.insert("url.decode".to_string(), Arc::new(|arg, oo, ctx| {
        let v = oo.force(arg, ctx);
        let s = match &v {
            Value::Atom(AtomKind::Str(s), _, _) => s.clone(),
            Value::Combo(c) => match c.get_field("0") {
                Some(v) => match oo.force(v.clone(), ctx) {
                    Value::Atom(AtomKind::Str(s), _, _) => s,
                    _ => return conflict(),
                },
                None => return conflict(),
            },
            _ => return conflict(),
        };
        str_atom(url_decode(&s))
    }) as Arc<BuiltinFn>);

    // url.join: {0: base, 1: relative} → Str（解析相對 URL）
    m.insert("url.join".to_string(), Arc::new(|arg, oo, ctx| {
        let c = match arg { Value::Combo(ref c) => c.clone(), _ => return conflict() };
        let base_s = extract_str(&oo.force(c.get_field("0").cloned().unwrap_or(Value::Top), ctx))
            .unwrap_or_default();
        let rel_s  = extract_str(&oo.force(c.get_field("1").cloned().unwrap_or(Value::Top), ctx))
            .unwrap_or_default();
        match url::Url::parse(&base_s).and_then(|base| base.join(&rel_s)) {
            Ok(u) => str_atom(u.as_str()),
            Err(e) => Value::Bottom(Box::new(BottomDetail {
                cause: BottomCause::Conflict,
                message: Some(format!("url.join: {}", e)),
                ..Default::default()
            })),
        }
    }) as Arc<BuiltinFn>);

    // url.query_params: {0: str} → Combo（key → Str value）
    // 解析 URL 的 query string 為 Combo：{key1: val1, key2: val2}
    m.insert("url.query_params".to_string(), Arc::new(|arg, oo, ctx| {
        let v = oo.force(arg, ctx);
        let s = match &v {
            Value::Atom(AtomKind::Str(s), _, _) => s.clone(),
            Value::Combo(c) => match c.get_field("0") {
                Some(v) => match oo.force(v.clone(), ctx) {
                    Value::Atom(AtomKind::Str(s), _, _) => s,
                    _ => return conflict(),
                },
                None => return conflict(),
            },
            _ => return conflict(),
        };
        match url::Url::parse(&s) {
            Ok(u) => {
                let mut fields = IndexMap::new();
                for (k, v) in u.query_pairs() {
                    fields.insert(k.into_owned(), str_atom(v.into_owned()));
                }
                Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]))
            }
            Err(_) => Value::Combo(ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]))
        }
    }) as Arc<BuiltinFn>);
}
```

---

## 4. 新建 `crates/interpreter/src/builtins/toml.rs`

```rust
use std::sync::Arc;
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, BottomCause, BottomDetail, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

fn conflict() -> Value { BottomCause::Conflict.into() }

// ── TOML Value → nlang Value ─────────────────────────────────────────────────

fn toml_to_value(t: &toml::Value) -> Value {
    match t {
        toml::Value::String(s) =>
            Value::Atom(AtomKind::Str(s.clone()), EffectTag::Pure, None),
        toml::Value::Integer(i) =>
            Value::Atom(AtomKind::Int(BigInt::from(*i)), EffectTag::Pure, None),
        toml::Value::Float(f) =>
            Value::Atom(AtomKind::Float(*f), EffectTag::Pure, None),
        toml::Value::Boolean(b) =>
            Value::Atom(AtomKind::Tag(if *b { "true" } else { "false" }.to_string()), EffectTag::Pure, None),
        toml::Value::Datetime(dt) =>
            Value::Atom(AtomKind::Str(dt.to_string()), EffectTag::Pure, None),
        toml::Value::Array(arr) => {
            let mut m = IndexMap::new();
            m.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
            for (i, v) in arr.iter().enumerate() { m.insert(i.to_string(), toml_to_value(v)); }
            Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
        }
        toml::Value::Table(tbl) => {
            let mut fields = IndexMap::new();
            for (k, v) in tbl { fields.insert(k.clone(), toml_to_value(v)); }
            Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]))
        }
    }
}

// ── nlang Value → TOML Value（盡力轉換）─────────────────────────────────────

fn value_to_toml(v: &Value) -> Option<toml::Value> {
    match v {
        Value::Atom(AtomKind::Str(s), _, _) => Some(toml::Value::String(s.clone())),
        Value::Atom(AtomKind::Int(n), _, _) => {
            use num_traits::ToPrimitive;
            n.to_i64().map(toml::Value::Integer)
        }
        Value::Atom(AtomKind::Float(f), _, _) => Some(toml::Value::Float(*f)),
        Value::Atom(AtomKind::Tag(t), _, _) => {
            if t == "true" { Some(toml::Value::Boolean(true)) }
            else if t == "false" { Some(toml::Value::Boolean(false)) }
            else { None }
        }
        Value::Combo(c) => {
            // Check if it's a @list
            if let Some(Value::Atom(AtomKind::Tag(kind), _, _)) = c.get_field("%kind") {
                if kind == "list" {
                    let mut arr = Vec::new();
                    for i in 0u32.. {
                        match c.get_field(&i.to_string()) {
                            Some(v) => match value_to_toml(v) {
                                Some(tv) => arr.push(tv),
                                None => return None,
                            },
                            None => break,
                        }
                    }
                    return Some(toml::Value::Array(arr));
                }
            }
            let mut tbl = toml::value::Table::new();
            for (k, v) in c.all_fields_iter() {
                // Skip internal meta fields (start with % / @ ~%)
                if k.starts_with('%') || k.starts_with('@') || k.starts_with('~') { continue; }
                match value_to_toml(&v) {
                    Some(tv) => { tbl.insert(k.clone(), tv); }
                    None => return None,
                }
            }
            Some(toml::Value::Table(tbl))
        }
        _ => None,
    }
}

pub fn register_toml_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // toml.parse: {0: str} → Value（TOML 字串 → Combo）
    m.insert("toml.parse".to_string(), Arc::new(|arg, oo, ctx| {
        let v = oo.force(arg, ctx);
        let s = match &v {
            Value::Atom(AtomKind::Str(s), _, _) => s.clone(),
            Value::Combo(c) => match c.get_field("0") {
                Some(v) => match oo.force(v.clone(), ctx) {
                    Value::Atom(AtomKind::Str(s), _, _) => s,
                    _ => return conflict(),
                },
                None => return conflict(),
            },
            _ => return conflict(),
        };
        match toml::from_str::<toml::Value>(&s) {
            Ok(tv) => toml_to_value(&tv),
            Err(e) => Value::Bottom(Box::new(BottomDetail {
                cause: BottomCause::Conflict,
                message: Some(format!("toml.parse: {}", e)),
                ..Default::default()
            })),
        }
    }) as Arc<BuiltinFn>);

    // toml.stringify: {0: val} → Str（Combo → TOML 字串）
    m.insert("toml.stringify".to_string(), Arc::new(|arg, oo, ctx| {
        let v = oo.force(arg, ctx);
        let target = match &v {
            Value::Combo(c) => match c.get_field("0") {
                Some(inner) => oo.force(inner.clone(), ctx),
                None => v,
            },
            _ => v,
        };
        match value_to_toml(&target) {
            Some(tv) => match toml::to_string_pretty(&tv) {
                Ok(s) => Value::Atom(AtomKind::Str(s), EffectTag::Pure, None),
                Err(e) => Value::Bottom(Box::new(BottomDetail {
                    cause: BottomCause::Conflict,
                    message: Some(format!("toml.stringify: {}", e)),
                    ..Default::default()
                })),
            },
            None => conflict(),
        }
    }) as Arc<BuiltinFn>);
}
```

**注意**：`toml::value::Table` 在 toml 0.8 中為 `toml::Table`（別名）。若編譯錯誤，改用 `toml::map::Map<String, toml::Value>`。

---

## 5. 修改 `crates/interpreter/src/builtins/mod.rs`

```rust
mod csv;
mod url;
mod toml;
```

在 `create_default_builtins()` 末尾：

```rust
    csv::register_csv_builtins(&mut m);
    url::register_url_builtins(&mut m);
    toml::register_toml_builtins(&mut m);
```

---

## 6. 修改 `crates/interpreter/src/lib.rs`

在 `~%Stat` 區塊之後加入 3 個新模組：

```rust
        // ~%Csv module
        let mut csv_fields = IndexMap::new();
        csv_fields.insert("/parse".to_string(),              make_morph("csv.parse",              EffectTag::Pure));
        csv_fields.insert("/parse_with_headers".to_string(), make_morph("csv.parse_with_headers", EffectTag::Pure));
        csv_fields.insert("/stringify".to_string(),          make_morph("csv.stringify",          EffectTag::Pure));
        csv_fields.insert("/read_csv".to_string(),           make_morph("csv.read_csv",           EffectTag::IO));
        let csv_module = Value::Combo(ComboVal::new(csv_fields, true, IndexMap::new(), EffectTag::Pure, vec![]));
        root.insert_field("~%Csv", csv_module);

        // ~%Url module
        let mut url_fields = IndexMap::new();
        url_fields.insert("/parse".to_string(),        make_morph("url.parse",        EffectTag::Pure));
        url_fields.insert("/encode".to_string(),       make_morph("url.encode",       EffectTag::Pure));
        url_fields.insert("/decode".to_string(),       make_morph("url.decode",       EffectTag::Pure));
        url_fields.insert("/join".to_string(),         make_morph("url.join",         EffectTag::Pure));
        url_fields.insert("/query_params".to_string(), make_morph("url.query_params", EffectTag::Pure));
        let url_module = Value::Combo(ComboVal::new(url_fields, true, IndexMap::new(), EffectTag::Pure, vec![]));
        root.insert_field("~%Url", url_module);

        // ~%Toml module
        let mut toml_fields = IndexMap::new();
        toml_fields.insert("/parse".to_string(),     make_morph("toml.parse",     EffectTag::Pure));
        toml_fields.insert("/stringify".to_string(), make_morph("toml.stringify", EffectTag::Pure));
        let toml_module = Value::Combo(ComboVal::new(toml_fields, true, IndexMap::new(), EffectTag::Pure, vec![]));
        root.insert_field("~%Toml", toml_module);
```

---

## 7. 修改 `crates/interpreter/src/genesis.rs`

加入 3 個新模組的 seed：

```rust
pub const SEED_CSV:  &str = "hash:sha256:v2:_:<lattice_sketch>:<digest>";
pub const SEED_URL:  &str = "hash:sha256:v2:_:<lattice_sketch>:<digest>";
pub const SEED_TOML: &str = "hash:sha256:v2:_:<lattice_sketch>:<digest>";
```

在 `all_seeds()` 中：

```rust
seeds.push(("~%Csv",  SEED_CSV));
seeds.push(("~%Url",  SEED_URL));
seeds.push(("~%Toml", SEED_TOML));
```

---

## 8. 測試

### 8.1 `tests/csv_p47_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use nlang_interpreter::value::ComboVal;

fn oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn list_len(v: &Value) -> usize {
    if let Value::Combo(c) = v {
        (0u32..).take_while(|i| c.get_field(&i.to_string()).is_some()).count()
    } else { 0 }
}

#[test] fn test_csv_parse_basic() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "csv.parse", str_val("a,b,c\n1,2,3"));
    assert_eq!(list_len(&r), 2, "2 rows");
    if let Value::Combo(c) = &r {
        let row0 = c.get_field("0").expect("row 0");
        assert_eq!(list_len(row0), 3, "3 fields in row 0");
    }
}

#[test] fn test_csv_parse_with_headers() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "csv.parse_with_headers", str_val("name,age\nAlice,30\nBob,25"));
    assert_eq!(list_len(&r), 2, "2 records");
    if let Value::Combo(c) = &r {
        let rec0 = c.get_field("0").expect("record 0");
        if let Value::Combo(rc) = rec0 {
            assert!(matches!(rc.get_field("name"), Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "Alice"));
        }
    }
}

#[test] fn test_csv_stringify_roundtrip() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let original = "a,b\n1,2";
    let parsed  = call(&oo, &mut ctx, "csv.parse",     str_val(original));
    let stringified = call(&oo, &mut ctx, "csv.stringify", parsed);
    assert!(matches!(&stringified, Value::Atom(AtomKind::Str(s), _, _) if s == original));
}

#[test] fn test_csv_quoted_field() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "csv.parse", str_val("\"hello, world\",two"));
    if let Value::Combo(c) = &r {
        let row0 = c.get_field("0").expect("row");
        if let Value::Combo(rc) = row0 {
            assert!(matches!(rc.get_field("0"), Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "hello, world"));
        }
    }
}
```

### 8.2 `tests/url_p47_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use nlang_interpreter::value::ComboVal;

fn oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn args2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

#[test] fn test_url_parse_components() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "url.parse", str_val("https://example.com/path?key=val#frag"));
    if let Value::Combo(c) = &r {
        assert!(matches!(c.get_field("scheme"), Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "https"));
        assert!(matches!(c.get_field("host"),   Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "example.com"));
        assert!(matches!(c.get_field("path"),   Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "/path"));
    } else { panic!("expected Combo"); }
}

#[test] fn test_url_encode_decode_roundtrip() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let encoded = call(&oo, &mut ctx, "url.encode", str_val("hello world!"));
    let decoded = call(&oo, &mut ctx, "url.decode", encoded);
    assert!(matches!(&decoded, Value::Atom(AtomKind::Str(s), _, _) if s == "hello world!"));
}

#[test] fn test_url_query_params() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "url.query_params", str_val("https://x.com/?foo=1&bar=2"));
    if let Value::Combo(c) = &r {
        assert!(matches!(c.get_field("foo"), Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "1"));
        assert!(matches!(c.get_field("bar"), Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "2"));
    } else { panic!("expected Combo"); }
}
```

### 8.3 `tests/toml_p47_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;

fn oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}

#[test] fn test_toml_parse_basic() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let toml_str = "name = \"Alice\"\nage = 30\n";
    let r = call(&oo, &mut ctx, "toml.parse", str_val(toml_str));
    if let Value::Combo(c) = &r {
        assert!(matches!(c.get_field("name"), Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "Alice"));
    } else { panic!("expected Combo"); }
}

#[test] fn test_toml_parse_nested_table() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let toml_str = "[server]\nhost = \"localhost\"\nport = 8080\n";
    let r = call(&oo, &mut ctx, "toml.parse", str_val(toml_str));
    if let Value::Combo(c) = &r {
        let server = c.get_field("server").expect("server table");
        if let Value::Combo(sc) = server {
            assert!(matches!(sc.get_field("host"), Some(Value::Atom(AtomKind::Str(s), _, _)) if s == "localhost"));
        } else { panic!("server not Combo"); }
    } else { panic!("expected Combo"); }
}

#[test] fn test_toml_parse_invalid_returns_bottom() {
    let oo = oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "toml.parse", str_val("invalid = ==="));
    assert!(matches!(&r, Value::Bottom(_)));
}
```

---

## 9. 完成後驗證

```bash
cargo test
```

預期：~514 tests，0 failed。

重點確認：
- `csv.parse("a,b\n1,2")` → 2 rows × 2 fields
- `csv.parse_with_headers` 正確對應 header 欄位名
- `csv.parse("\"hello, world\",two")` → 第一欄為 `"hello, world"`（含逗號）
- `url.parse("https://example.com/path?k=v")` → 正確分解 scheme/host/path/query
- `url.encode`/`decode` roundtrip
- `toml.parse` 解析巢狀 table
- `toml.parse("invalid = ===")` → Bottom
- genesis_test 通過（SEED_CSV/SEED_URL/SEED_TOML 正確填入）

---

## 10. 實作注意事項

| 事項 | 說明 |
|:-----|:-----|
| `toml::value::Table` vs `toml::Table` | toml 0.8 中 `Table = Map<String, Value>`；用 `toml::map::Map<String, toml::Value>` 最穩定 |
| `url` crate 的 `query_pairs()` | 回傳 `impl Iterator<Item = (Cow<str>, Cow<str>)>`，需 `.into_owned()` |
| CSV 空行處理 | `parse_csv` 最後的 flush logic 會生成一個空 row 若輸入以 `\n` 結尾；可在最後過濾空行：`rows.retain(\|r\| !r.is_empty() \|\| r.iter().any(\|f\| !f.is_empty()))` |
| `csv.read_csv` Effect | 回傳的 @list 應標記 IO effect；若 `ComboVal::new` 不接受 effect 欄位重寫，可在最後包一層 Thunk 或直接回傳 list（讓呼叫端的 apply 決定 effect） |
| `mod url` 衝突 | `mod url;` 可能與 crate `url` 衝突。若 `use url::Url;` 出現命名衝突，在 `url.rs` 頂部加 `extern crate url as url_crate;` 或直接用 `::url::Url::parse(...)` |
| `mod toml` 衝突 | 同上，`extern crate toml as toml_crate;` 或 `::toml::from_str(...)` |

---

## 11. 修改摘要

| 檔案 | 改動 |
|:-----|:-----|
| `Cargo.toml` (interpreter) | `[dependencies]` 加 `url = "2"`, `toml = "0.8"`；+3 個 `[[test]]` entries |
| `src/builtins/csv.rs` | 新建：手寫 RFC 4180 解析器 + 4 builtins |
| `src/builtins/url.rs` | 新建：url crate 封裝 + 手寫 percent encode/decode + 5 builtins |
| `src/builtins/toml.rs` | 新建：toml crate 封裝 + toml↔Value 雙向轉換 + 2 builtins |
| `src/builtins/mod.rs` | `mod csv/url/toml;` + 3 個 register 呼叫 |
| `src/lib.rs` | `~%Csv`（4）+ `~%Url`（5）+ `~%Toml`（2）模組定義 |
| `src/genesis.rs` | `SEED_CSV` + `SEED_URL` + `SEED_TOML` |
| `tests/csv_p47_test.rs` | 新建，4 tests |
| `tests/url_p47_test.rs` | 新建，3 tests |
| `tests/toml_p47_test.rs` | 新建，3 tests |
