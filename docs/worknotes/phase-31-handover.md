# Phase 31 交接文件

> 狀態：待實作  
> 前置：Phase 30 完成（~354 tests passing）  
> 目標：`~%Regex` 模組 — 4 個 builtins（regex.match / find / replace / split）

---

## 概覽

| 任務 | 位置 | 內容 |
|:-----|:-----|:-----|
| Task 0 | `crates/interpreter/Cargo.toml` | 加入 `regex = "1"` 依賴 |
| Task 1 | `crates/interpreter/src/builtins/regex.rs`（**新建**） | 4 個 regex builtins |
| Task 2 | `crates/interpreter/src/builtins/mod.rs` | 加入 `mod regex;` 和呼叫 |
| Task 3 | `crates/interpreter/src/lib.rs` | 在 `root_with_system()` 加入 `~%Regex` 模組 |
| Task 4 | `crates/interpreter/src/genesis.rs` | 加入 `SEED_REGEX`，重跑 seed test |
| Tests  | `crates/interpreter/tests/regex_p31_test.rs`（新建） | ~9 個測試 |

預期完成後：**~354 + 9 ≈ 363 tests**

---

## Regex builtins 語義速查

| builtin | 輸入 | 輸出 | 說明 |
|:--------|:-----|:-----|:-----|
| `regex.match` | `{0: pattern, 1: str}` | `#true` \| `#false` | 整體是否有匹配；無效 pattern → Top |
| `regex.find` | `{0: pattern, 1: str}` | `{match, start, end}` \| `#none` | 第一個匹配的內容及 char 位置；無效 pattern → Top |
| `regex.replace` | `{0: pattern, 1: repl, 2: str}` | Str | 替換**所有**匹配（支援 `$1`/`$2` 捕獲組引用）；無效 pattern → Top |
| `regex.split` | `{0: pattern, 1: str}` | list of Str | 依 pattern 分割；無效 pattern → Top |

`regex.find` 的 `start`/`end` 是 **Unicode char 索引**（與 `str.index_of`、`str.char_at` 一致）。

---

## Task 0：更新 `Cargo.toml`

在 `[dependencies]` 區塊末尾加入：

```toml
regex = "1"
```

---

## Task 1：新建 `regex.rs`

**建立** `crates/interpreter/src/builtins/regex.rs`，完整內容如下：

```rust
use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;
use regex::Regex;

pub fn register_regex_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // regex.match: {0: pattern, 1: str} → #true | #false  (Top if invalid pattern)
    m.insert("regex.match".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vp), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
                let fp = oo.force(vp.clone(), ctx);
                let fs = oo.force(vs.clone(), ctx);
                if let (Value::Atom(AtomKind::Str(pattern), _, _), Value::Atom(AtomKind::Str(s), _, _)) =
                    (fp.collapse(), fs.collapse())
                {
                    let tag = match Regex::new(pattern.as_str()) {
                        Ok(re) => if re.is_match(s.as_str()) { "true" } else { "false" },
                        Err(_) => return Value::Top,
                    };
                    return Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // regex.find: {0: pattern, 1: str} → {match: Str, start: Int, end: Int} | #none
    // start and end are Unicode char indices (consistent with str.char_at / str.index_of)
    m.insert("regex.find".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vp), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
                let fp = oo.force(vp.clone(), ctx);
                let fs = oo.force(vs.clone(), ctx);
                if let (Value::Atom(AtomKind::Str(pattern), _, _), Value::Atom(AtomKind::Str(s), _, _)) =
                    (fp.collapse(), fs.collapse())
                {
                    let re = match Regex::new(pattern.as_str()) {
                        Ok(r)  => r,
                        Err(_) => return Value::Top,
                    };
                    return match re.find(s.as_str()) {
                        None => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
                        Some(mat) => {
                            let matched   = mat.as_str().to_string();
                            let char_start = s[..mat.start()].chars().count();
                            let char_end   = char_start + matched.chars().count();
                            let mut res = IndexMap::new();
                            res.insert("match".to_string(),
                                Value::Atom(AtomKind::Str(matched), EffectTag::Pure, None));
                            res.insert("start".to_string(),
                                Value::Atom(AtomKind::Int(BigInt::from(char_start)), EffectTag::Pure, None));
                            res.insert("end".to_string(),
                                Value::Atom(AtomKind::Int(BigInt::from(char_end)), EffectTag::Pure, None));
                            Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]))
                        }
                    };
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // regex.replace: {0: pattern, 1: replacement, 2: str} → Str
    // Replaces ALL occurrences. Replacement supports $0, $1, ... for capture groups.
    m.insert("regex.replace".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vp), Some(vr), Some(vs)) = (c.get_field("0"), c.get_field("1"), c.get_field("2")) {
                let fp = oo.force(vp.clone(), ctx);
                let fr = oo.force(vr.clone(), ctx);
                let fs = oo.force(vs.clone(), ctx);
                if let (
                    Value::Atom(AtomKind::Str(pattern), _, _),
                    Value::Atom(AtomKind::Str(replacement), _, _),
                    Value::Atom(AtomKind::Str(s), _, _),
                ) = (fp.collapse(), fr.collapse(), fs.collapse()) {
                    return match Regex::new(pattern.as_str()) {
                        Err(_) => Value::Top,
                        Ok(re) => {
                            let result = re.replace_all(s.as_str(), replacement.as_str()).to_string();
                            Value::Atom(AtomKind::Str(result), EffectTag::Pure, None)
                        }
                    };
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // regex.split: {0: pattern, 1: str} → list of Str
    // Empty strings at boundaries are preserved (raw split behavior).
    m.insert("regex.split".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vp), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
                let fp = oo.force(vp.clone(), ctx);
                let fs = oo.force(vs.clone(), ctx);
                if let (Value::Atom(AtomKind::Str(pattern), _, _), Value::Atom(AtomKind::Str(s), _, _)) =
                    (fp.collapse(), fs.collapse())
                {
                    let re = match Regex::new(pattern.as_str()) {
                        Ok(r)  => r,
                        Err(_) => return Value::Top,
                    };
                    let mut res = IndexMap::new();
                    for (i, part) in re.split(s.as_str()).enumerate() {
                        res.insert(i.to_string(),
                            Value::Atom(AtomKind::Str(part.to_string()), EffectTag::Pure, None));
                    }
                    res.insert("%kind".to_string(),
                        Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
                    return Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]));
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
}
```

---

## Task 2：更新 `mod.rs`

加入（放在 `mod bytes;` 之後）：

```rust
mod regex;
```

並在 `create_default_builtins()` 中加入：

```rust
    regex::register_regex_builtins(&mut m);
```

---

## Task 3：更新 `root_with_system()`（`lib.rs`）

在 `~%Bytes` 區塊之後加入 `~%Regex` 模組：

```rust
        let mut regex_fields = IndexMap::new();
        let regex_morphisms = vec![
            ("/match",   "regex.match"),
            ("/find",    "regex.find"),
            ("/replace", "regex.replace"),
            ("/split",   "regex.split"),
        ];
        for (n, b) in regex_morphisms {
            regex_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(),  Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])));
        }
        fields.insert("~%Regex".to_string(), Value::Combo(ComboVal::new(regex_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
```

---

## Task 4：更新 genesis.rs

### 加入常數

```rust
pub const SEED_REGEX: &str = "hash:sha256:v1:PLACEHOLDER_run_seed_test";
```

### 更新 all_seeds()

```rust
        ("~%Regex",      SEED_REGEX),   // ← 新增（加在 "~%Bytes" 之後）
```

### 重跑 seed test

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture 2>&1
```

從輸出的 `UPDATE:` 行找到 `~%Regex` 的 CAID，更新 `SEED_REGEX`。

---

## 測試（`tests/regex_p31_test.rs`）

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
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
fn is_true(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "true")
}
fn is_false(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "false")
}
fn is_none(v: &Value) -> bool {
    matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none")
}
fn list_len(v: &Value) -> usize {
    match v {
        Value::Combo(c) => c.fields().keys().filter(|k| k.parse::<usize>().is_ok()).count(),
        _ => panic!("expected list"),
    }
}
fn list_str_at(v: &Value, i: usize) -> &str {
    match v {
        Value::Combo(c) => as_str(c.get_field(&i.to_string()).expect("index")),
        _ => panic!("expected list"),
    }
}

// ── regex.match ────────────────────────────────────────────────────

#[test]
fn test_regex_match_true() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "regex.match", combo2(str_val(r"\d+"), str_val("hello123")));
    assert!(is_true(&r));
}

#[test]
fn test_regex_match_false() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "regex.match", combo2(str_val(r"^\d+$"), str_val("hello")));
    assert!(is_false(&r));
}

#[test]
fn test_regex_match_invalid_pattern_returns_top() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "regex.match", combo2(str_val("[invalid"), str_val("test")));
    assert!(matches!(r, Value::Top));
}

// ── regex.find ─────────────────────────────────────────────────────

#[test]
fn test_regex_find_found() {
    // find \d+ in "abc123def" → {match: "123", start: 3, end: 6}
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "regex.find", combo2(str_val(r"\d+"), str_val("abc123def")));
    if let Value::Combo(ref c) = r {
        assert_eq!(as_str(c.get_field("match").unwrap()), "123");
        assert_eq!(as_int(c.get_field("start").unwrap()), 3);
        assert_eq!(as_int(c.get_field("end").unwrap()), 6);
    } else { panic!("expected Combo, got {:?}", r); }
}

#[test]
fn test_regex_find_not_found() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "regex.find", combo2(str_val(r"\d+"), str_val("hello")));
    assert!(is_none(&r));
}

// ── regex.replace ──────────────────────────────────────────────────

#[test]
fn test_regex_replace_all() {
    // Replace all digit sequences with "N"
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "regex.replace",
        combo3(str_val(r"\d+"), str_val("N"), str_val("a1 b22 c3")));
    assert_eq!(as_str(&r), "aN bN cN");
}

#[test]
fn test_regex_replace_no_match_unchanged() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "regex.replace",
        combo3(str_val(r"\d+"), str_val("N"), str_val("hello")));
    assert_eq!(as_str(&r), "hello");
}

// ── regex.split ────────────────────────────────────────────────────

#[test]
fn test_regex_split_whitespace() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "regex.split", combo2(str_val(r"\s+"), str_val("hello world")));
    assert_eq!(list_len(&r), 2);
    assert_eq!(list_str_at(&r, 0), "hello");
    assert_eq!(list_str_at(&r, 1), "world");
}

#[test]
fn test_regex_split_comma() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "regex.split", combo2(str_val(r",\s*"), str_val("a, b, c")));
    assert_eq!(list_len(&r), 3);
    assert_eq!(list_str_at(&r, 0), "a");
    assert_eq!(list_str_at(&r, 1), "b");
    assert_eq!(list_str_at(&r, 2), "c");
}
```

### 加入 `Cargo.toml` 的 test 條目

```toml
[[test]]
name = "regex_p31_test"
path = "tests/regex_p31_test.rs"
```

---

## 注意事項

### 命名衝突
檔名 `regex.rs` 與 `regex` crate 名稱不衝突——Rust 模組系統的檔名不影響 crate 引用。`use regex::Regex;` 在 `regex.rs` 內部完全合法。

### `regex.replace` 替換所有匹配
使用 `replace_all`（非 `replace`）。若只需替換第一個，使用者可先用 `regex.find` 取得位置再手動處理。

### 捕獲組引用
`regex.replace` 的 replacement 字串支援 `$0`（整個匹配）、`$1`、`$2`（捕獲組）—— regex crate 在 `replace_all` 中自動處理。

### 空字串邊界
`regex.split` 在邊界可能產生空字串（例如 `split(\s+, "  hello  ")` → `["", "hello", ""]`）。這是原始 split 行為，不過濾。

### SEED_REGEX 為新常數
只有 `~%Regex` CAID 需要新加；其他既有 seed 不受影響。

---

## 驗證步驟

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools

# 1. 編譯（首次會下載 regex crate，稍慢）
cargo build --manifest-path crates/interpreter/Cargo.toml 2>&1

# 2. 新測試
cargo test --manifest-path crates/interpreter/Cargo.toml regex_p31_test -- --nocapture

# 3. 種子更新後穩定性
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture

# 4. 全套不退步
cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：~363 tests, 0 failed
```
