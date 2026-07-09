# Phase 25 交接文件

> 狀態：待實作  
> 前置：Phase 24 完成（280 tests passing）  
> 目標：A) 新增 5 個 list/string builtins，B) `@list` genesis seed，C) 修復 `root_with_system()` 缺漏的模組態射

---

## 重要發現：root_with_system() 的態射缺漏

**Phase 17–22 新增的 builtins 都只在 `builtin_registry` 中，沒有加進 `root_with_system()` 的模組欄位。**  
這意味著測試通過（測試直接呼叫 registry），但使用者無法用 `~%List/flat_map` 等語法存取。

Phase 25 會一次補齊所有缺漏，作為任務 3。

---

## 任務總覽

| # | 位置 | 內容 |
|:--|:-----|:-----|
| Task 1 | `crates/interpreter/src/builtins/list.rs` | `list.unique`, `list.range`, `list.reduce` |
| Task 2 | `crates/interpreter/src/builtins/string.rs` | `str.char_at`, `str.chars` |
| Task 3 | `crates/interpreter/src/lib.rs` | 補全 `~%List`/`~%String`/`~%Time` 態射 + 新增 `@list` 型別定義 |
| Task 4 | `crates/interpreter/src/genesis.rs` | 加入 `SEED_TYPE_LIST`，重跑 seed test 更新所有變動的 SEED |
| Tests  | `crates/interpreter/tests/list_test.rs` + `str_test.rs`（新建）| ~11 個測試 |

預期完成後：**280 + 11 ≈ 291 tests**

---

## Task 1：新 list builtins（`list.rs`）

在 `list.max_by` 之後（566 行末尾前）加入以下三個 builtins。

### `list.unique`

語義：保留第一次出現，移除重複值（用 `to_nlang(0)` 作去重鍵）。

```
list.unique {0: list}  →  list
```

```rust
m.insert("list.unique".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
    let list = oo.force(v, ctx);
    let items = extract_list_items(&list);
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let forced = oo.force(item, ctx);
        let key = forced.to_nlang(0);
        if seen.insert(key) {
            out.push(forced);
        }
    }
    build_list_value(out, EffectTag::Pure)
}) as Arc<BuiltinFn>);
```

### `list.range`

語義：產生整數序列 `[start, start+1, ..., end-1]`（不含 end，Python 慣例）。

```
list.range {0: start, 1: end}  →  list of Int
```

```rust
m.insert("list.range".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(vs), Some(ve)) = (c.get_field("0"), c.get_field("1")) {
            let fs = oo.force(vs.clone(), ctx);
            let fe = oo.force(ve.clone(), ctx);
            if let (Value::Atom(AtomKind::Int(start), _, _), Value::Atom(AtomKind::Int(end), _, _)) =
                (fs.collapse(), fe.collapse())
            {
                let mut items = Vec::new();
                let mut i = start.clone();
                while i < *end {
                    items.push(Value::Atom(AtomKind::Int(i.clone()), EffectTag::Pure, None));
                    i += 1;
                }
                return build_list_value(items, EffectTag::Pure);
            }
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);
```

**注意**：`start >= end` 時回傳空 list（`build_list_value(vec![], EffectTag::Pure)`，不是 Top）。

### `list.reduce`

語義：用第一個元素作初始值，依序套用函數。空 list → Top。

```
list.reduce {0: fn, 1: list}  →  value
fn 接受 {0: acc, 1: item}
```

```rust
m.insert("list.reduce".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(vf), Some(vl)) = (c.get_field("0"), c.get_field("1")) {
            let func = vf.clone();
            let list = oo.force(vl.clone(), ctx);
            let items = extract_list_items(&list);
            if items.is_empty() { return Value::Top; }
            let mut acc = oo.force(items[0].clone(), ctx);
            for item in items.into_iter().skip(1) {
                let item_forced = oo.force(item, ctx);
                let mut pair = IndexMap::new();
                pair.insert("0".to_string(), acc);
                pair.insert("1".to_string(), item_forced);
                let pair_val = Value::Combo(ComboVal::new(pair, true, IndexMap::new(), EffectTag::Pure, vec![]));
                acc = oo.apply_morphism(func.clone(), pair_val, ctx);
            }
            return acc;
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);
```

**注意**：`list.rs` 的 imports 已包含 `IndexMap`、`ComboVal`、`EffectTag` 等，直接使用即可。

---

## Task 2：新 string builtins（`string.rs`）

在 `str.format` 之後（string.rs 末尾前 `Value::Top` return 之前）加入：

### `str.char_at`

語義：取得字串第 N 個 Unicode 字元（0-indexed）。超出範圍 → Top。

```
str.char_at {0: idx, 1: str}  →  Str（單一字元）
```

```rust
m.insert("str.char_at".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(vi), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
            let fi = oo.force(vi.clone(), ctx);
            let fs = oo.force(vs.clone(), ctx);
            if let (Value::Atom(AtomKind::Int(idx), _, _), Value::Atom(AtomKind::Str(s), _, _)) =
                (fi.collapse(), fs.collapse())
            {
                if let Some(n) = idx.to_usize() {
                    if let Some(ch) = s.chars().nth(n) {
                        return Value::Atom(AtomKind::Str(ch.to_string()), EffectTag::Pure, None);
                    }
                }
            }
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);
```

### `str.chars`

語義：將字串拆成單一字元的 list。

```
str.chars {0: str}  →  list of Str
```

```rust
m.insert("str.chars".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
    if let Value::Atom(AtomKind::Str(s), _, _) = oo.force(v, ctx).collapse() {
        let mut res = IndexMap::new();
        for (i, ch) in s.chars().enumerate() {
            res.insert(i.to_string(), Value::Atom(AtomKind::Str(ch.to_string()), EffectTag::Pure, None));
        }
        res.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
        return Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]));
    }
    Value::Top
}) as Arc<BuiltinFn>);
```

**注意**：`string.rs` 已有 `use indexmap::IndexMap; use crate::value::{..., ComboVal, ...};`，確認有 `ComboVal` import。

---

## Task 3：補全 `root_with_system()` 的模組態射 + 加入 `@list`

**位置**：`crates/interpreter/src/lib.rs`

這是最重要的任務。分為三部分：

### 3A：補全 `~%List` 態射

**找到**（約 172 行）：
```rust
let list_morphisms = vec![("/map", "list.map"), ("/filter", "list.filter"), ("/fold", "list.fold"), ("/len", "list.len"), ("/concat", "list.concat"), ("/at", "list.at"), ("/sort", "list.sort"), ("/reverse", "list.reverse"), ("/slice", "list.slice"), ("/zip", "list.zip")];
```

**替換為**：
```rust
let list_morphisms = vec![
    ("/map",       "list.map"),
    ("/filter",    "list.filter"),
    ("/fold",      "list.fold"),
    ("/len",       "list.len"),
    ("/concat",    "list.concat"),
    ("/at",        "list.at"),
    ("/sort",      "list.sort"),
    ("/reverse",   "list.reverse"),
    ("/slice",     "list.slice"),
    ("/zip",       "list.zip"),
    // Phase 17
    ("/flat_map",  "list.flat_map"),
    // Phase 18
    ("/any",       "list.any"),
    ("/all",       "list.all"),
    ("/find",      "list.find"),
    ("/head",      "list.head"),
    ("/tail",      "list.tail"),
    ("/take",      "list.take"),
    ("/drop",      "list.drop"),
    // Phase 19
    ("/count",     "list.count"),
    ("/zip_with",  "list.zip_with"),
    // Phase 22
    ("/partition", "list.partition"),
    ("/flatten",   "list.flatten"),
    ("/sum",       "list.sum"),
    ("/min_by",    "list.min_by"),
    ("/max_by",    "list.max_by"),
    // Phase 25
    ("/unique",    "list.unique"),
    ("/range",     "list.range"),
    ("/reduce",    "list.reduce"),
];
```

### 3B：補全 `~%String` 態射

**找到**（約 177 行）：
```rust
let string_morphisms = vec![
    ("/concat", "str.concat"), ("/split", "str.split"), ("/join", "str.join"), ("/trim", "str.trim"), ("/len", "str.len"),
    ("/replace", "str.replace"), ("/to_lower", "str.to_lower"), ("/to_upper", "str.to_upper"), 
    ("/starts_with", "str.starts_with"), ("/ends_with", "str.ends_with"), ("/contains", "str.contains")
];
```

**替換為**：
```rust
let string_morphisms = vec![
    ("/concat",      "str.concat"),
    ("/split",       "str.split"),
    ("/join",        "str.join"),
    ("/trim",        "str.trim"),
    ("/len",         "str.len"),
    ("/replace",     "str.replace"),
    ("/to_lower",    "str.to_lower"),
    ("/to_upper",    "str.to_upper"),
    ("/starts_with", "str.starts_with"),
    ("/ends_with",   "str.ends_with"),
    ("/contains",    "str.contains"),
    // Phase 19
    ("/parse_int",   "str.parse_int"),
    ("/from_int",    "str.from_int"),
    ("/repeat",      "str.repeat"),
    // Phase 21
    ("/format",      "str.format"),
    // Phase 25
    ("/char_at",     "str.char_at"),
    ("/chars",       "str.chars"),
];
```

### 3C：補全 `~%Time` 態射

**找到**（約 185-187 行）：
```rust
let mut time_fields = IndexMap::new();
time_fields.insert("/now".to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![...IO effect...]), ...)));
fields.insert("~%Time".to_string(), ...);
```

**在 `fields.insert("~%Time"...)` 之前插入**：
```rust
let time_morphisms = vec![
    ("/format", "time.format"),
    ("/diff",   "time.diff"),
    ("/add_ms", "time.add_ms"),
];
for (n, b) in time_morphisms {
    time_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
        ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
        ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
    ]), true, IndexMap::new(), EffectTag::Pure, vec![])));
}
```

### 3D：加入 `@list` 型別定義

**在 `@result` 的 `fields.insert("@result"...)` 之後**，加入：

```rust
// @list: Combo with %kind: #list  (SPEC_09 §2.x)
let mut list_type_fields = IndexMap::new();
list_type_fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("type".to_string()), EffectTag::Pure, None));
list_type_fields.insert("%name".to_string(), Value::Atom(AtomKind::Str("list".to_string()), EffectTag::Pure, None));
list_type_fields.insert(
    "%fmap".to_string(),
    Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
        ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
        ("%builtin".to_string(), Value::Atom(AtomKind::Str("list.map".to_string()), EffectTag::Pure, None)),
    ]), true, IndexMap::new(), EffectTag::Pure, vec![])),
);
fields.insert(
    "@list".to_string(),
    Value::Combo(ComboVal::new(list_type_fields, true, IndexMap::new(), EffectTag::Pure, vec![])),
);
```

---

## Task 4：更新 genesis.rs

**位置**：`crates/interpreter/src/genesis.rs`

### 4A：加入 SEED_TYPE_LIST 常數

在現有常數列表末尾加入：

```rust
pub const SEED_TYPE_LIST: &str = "hash:sha256:v1:PLACEHOLDER_run_seed_test_to_get_real_value";
```

### 4B：更新 all_seeds()

```rust
pub fn all_seeds() -> Vec<(&'static str, &'static str)> {
    vec![
        ("~%Math",       SEED_MATH),
        ("~%List",       SEED_LIST),      // ← 值會改變！
        ("~%Cond",       SEED_COND),
        ("~%Discovery",  SEED_DISCOVERY),
        ("~%String",     SEED_STRING),    // ← 值會改變！
        ("~%Complex",    SEED_COMPLEX),
        ("~%Reflection", SEED_REFL),
        ("~%Time",       SEED_TIME),      // ← 值會改變！
        ("@option",      SEED_OPTION),
        ("@result",      SEED_RESULT),
        ("@list",        SEED_TYPE_LIST), // ← 新增
        ("~%Config",     SEED_CONFIG),
    ]
}
```

### 4C：重新計算所有變動的 SEED 值（必做！）

執行：
```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture 2>&1
```

輸出中的 `UPDATE:` 行會列出所有需要更新的常數。  
**將 SEED_LIST、SEED_STRING、SEED_TIME、SEED_TYPE_LIST 的值全部從輸出中複製回 genesis.rs。**

---

## 測試（新建檔案，接在既有 test 套件之後）

### `tests/list_p25_test.rs`

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
fn make_list(items: Vec<Value>) -> Value {
    let mut m = IndexMap::new();
    for (i, v) in items.into_iter().enumerate() { m.insert(i.to_string(), v); }
    m.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn morph(builtin: &str) -> Value {
    let mut m = IndexMap::new();
    m.insert("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
    m.insert("%builtin".to_string(), Value::Atom(AtomKind::Str(builtin.to_string()), EffectTag::Pure, None));
    Value::Combo(ComboVal::new(m, true, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn make_combo2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a);
    m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn list_len(v: &Value) -> usize {
    match v {
        Value::Combo(c) => c.fields().keys().filter(|k| k.parse::<usize>().is_ok()).count(),
        _ => panic!("expected list"),
    }
}

#[test]
fn test_list_unique_dedup() {
    // [1, 2, 1, 3, 2] → [1, 2, 3]
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let list = make_list(vec![int_val(1), int_val(2), int_val(1), int_val(3), int_val(2)]);
    let r = call(&oo, &mut ctx, "list.unique", list);
    assert_eq!(list_len(&r), 3);
}

#[test]
fn test_list_unique_empty() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.unique", make_list(vec![]));
    assert_eq!(list_len(&r), 0);
}

#[test]
fn test_list_range_basic() {
    // range(2, 5) → [2, 3, 4]
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let arg = make_combo2(int_val(2), int_val(5));
    let r = call(&oo, &mut ctx, "list.range", arg);
    assert_eq!(list_len(&r), 3);
    if let Value::Combo(c) = &r {
        assert_eq!(c.get_field("0").unwrap().to_string_plain(), "2");
        assert_eq!(c.get_field("2").unwrap().to_string_plain(), "4");
    }
}

#[test]
fn test_list_range_empty_when_start_ge_end() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let arg = make_combo2(int_val(5), int_val(3));
    let r = call(&oo, &mut ctx, "list.range", arg);
    assert_eq!(list_len(&r), 0);
}

#[test]
fn test_list_reduce_sum() {
    // reduce(math.add, [1, 2, 3, 4]) → 10
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let list = make_list(vec![int_val(1), int_val(2), int_val(3), int_val(4)]);
    let arg = make_combo2(morph("math.add"), list);
    let r = call(&oo, &mut ctx, "list.reduce", arg);
    match r {
        Value::Atom(AtomKind::Int(n), _, _) => assert_eq!(n, BigInt::from(10)),
        other => panic!("expected Int(10), got {:?}", other),
    }
}

#[test]
fn test_list_reduce_empty_returns_top() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let arg = make_combo2(morph("math.add"), make_list(vec![]));
    let r = call(&oo, &mut ctx, "list.reduce", arg);
    assert!(matches!(r, Value::Top));
}
```

### `tests/str_p25_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn int_val(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
fn make_combo2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn as_str(v: &Value) -> String {
    match v { Value::Atom(AtomKind::Str(s), _, _) => s.clone(), other => panic!("expected Str, got {:?}", other) }
}

#[test]
fn test_str_char_at_first() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.char_at", make_combo2(int_val(0), str_val("hello")));
    assert_eq!(as_str(&r), "h");
}

#[test]
fn test_str_char_at_last() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.char_at", make_combo2(int_val(4), str_val("hello")));
    assert_eq!(as_str(&r), "o");
}

#[test]
fn test_str_char_at_oob_returns_top() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.char_at", make_combo2(int_val(5), str_val("hello")));
    assert!(matches!(r, Value::Top));
}

#[test]
fn test_str_chars_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.chars", str_val("hi!"));
    if let Value::Combo(c) = &r {
        assert_eq!(c.get_field("0").unwrap().to_string_plain(), "h");
        assert_eq!(c.get_field("1").unwrap().to_string_plain(), "i");
        assert_eq!(c.get_field("2").unwrap().to_string_plain(), "!");
    } else { panic!("expected list"); }
}

#[test]
fn test_str_chars_empty() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "str.chars", str_val(""));
    if let Value::Combo(c) = &r {
        assert_eq!(c.fields().keys().filter(|k| k.parse::<usize>().is_ok()).count(), 0);
    } else { panic!("expected empty list"); }
}
```

### 加入 `Cargo.toml` 的 test 條目

在 `crates/interpreter/Cargo.toml` 的 `[[test]]` 區塊末尾加入：

```toml
[[test]]
name = "list_p25_test"
path = "tests/list_p25_test.rs"

[[test]]
name = "str_p25_test"
path = "tests/str_p25_test.rs"
```

---

## 注意事項

### `str.char_at` 的 `to_usize()`
`BigInt::to_usize()` 在負數時回傳 `None`，天然防止負索引，不需額外檢查。

### `str.chars` 的返回格式
純粹用數字 key + `%kind: #list`，不設 `%len`（現有所有 list builtins 都不設 `%len`，保持一致）。

### `list.reduce` 使用 `math.add`
`math.add` 接受 `{0: a, 1: b}` 且回傳 `a + b`，已在多個測試中驗證，可安全使用。

### SEED 更新是必須的
`root_with_system()` 的結構改變後，`SEED_LIST`、`SEED_STRING`、`SEED_TIME` 的 CAID 都會不同。  
若不更新，`seed_caids_are_stable` 測試會失敗。**Task 4 不可省略。**

### `~%Time` 的 `/now` 需保留
`/now` 有 `EffectTag::IO`，不能加進 `for (n, b) in time_morphisms` 迴圈（那個迴圈統一用 `EffectTag::Pure`）。  
只把 `/format`、`/diff`、`/add_ms` 加進 `time_morphisms`，`/now` 保持原樣。

---

## 驗證步驟

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools

# 1. 編譯
cargo build --manifest-path crates/interpreter/Cargo.toml 2>&1

# 2. 新測試
cargo test --manifest-path crates/interpreter/Cargo.toml list_p25_test -- --nocapture
cargo test --manifest-path crates/interpreter/Cargo.toml str_p25_test -- --nocapture

# 3. 種子穩定性（Task 4 完成後）
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture

# 4. 全套測試
cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：~291 tests, 0 failed
```
