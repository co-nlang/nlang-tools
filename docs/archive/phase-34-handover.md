# Phase 34 交接文件

> 狀態：待實作  
> 前置：Phase 33 完成（~386 tests passing）  
> 目標：`~%Io` 模組 — 4 個 builtins（io.read_file / write_file / exists / append_file）

---

## 概覽

| 任務 | 位置 | 內容 |
|:-----|:-----|:-----|
| Task 1 | `crates/interpreter/src/builtins/io.rs`（**新建**） | 4 個 IO builtins |
| Task 2 | `crates/interpreter/src/builtins/mod.rs` | 加入 `mod io;` 和呼叫 |
| Task 3 | `crates/interpreter/src/lib.rs` | 加入 `~%Io` 模組（**IO EffectTag** 語義） |
| Task 4 | `crates/interpreter/src/genesis.rs` | 加入 `SEED_IO`，重跑 seed test |
| Tests  | `crates/interpreter/tests/io_p34_test.rs`（新建） | ~6 個測試 |

預期完成後：**~386 + 6 ≈ 392 tests**

### Builtin 語義速查

| builtin | 輸入 | 輸出成功 | 輸出失敗 | EffectTag |
|:--------|:-----|:---------|:---------|:---------|
| `io.read_file` | `{0: path}` | `Str`（UTF-8 內容） | `#none`（不存在或非 UTF-8） | IO |
| `io.write_file` | `{0: path, 1: content}` | `#true`（建立或截斷） | `#none` | IO |
| `io.exists` | `{0: path}` | `#true` \| `#false` | — | IO |
| `io.append_file` | `{0: path, 1: content}` | `#true`（不存在時自動建立） | `#none` | IO |

---

## Task 1：新建 `crates/interpreter/src/builtins/io.rs`

```rust
use std::sync::Arc;
use std::collections::HashMap;
use std::io::Write;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;

pub fn register_io_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // io.read_file: {0: path_str} → Str | #none  (IO)
    m.insert("io.read_file".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(path), _, _) = oo.force(v, ctx).collapse() {
            return match std::fs::read_to_string(path.as_str()) {
                Ok(content) => Value::Atom(AtomKind::Str(content), EffectTag::IO, None),
                Err(_)      => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::IO, None),
            };
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // io.write_file: {0: path_str, 1: content_str} → #true | #none  (IO)
    // Creates or truncates the file.
    m.insert("io.write_file".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vp), Some(vc)) = (c.get_field("0"), c.get_field("1")) {
                let fp = oo.force(vp.clone(), ctx);
                let fc = oo.force(vc.clone(), ctx);
                if let (Value::Atom(AtomKind::Str(path), _, _), Value::Atom(AtomKind::Str(content), _, _)) =
                    (fp.collapse(), fc.collapse())
                {
                    let tag = if std::fs::write(path.as_str(), content.as_bytes()).is_ok() { "true" } else { "none" };
                    return Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::IO, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // io.exists: {0: path_str} → #true | #false  (IO)
    m.insert("io.exists".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Str(path), _, _) = oo.force(v, ctx).collapse() {
            let tag = if std::path::Path::new(path.as_str()).exists() { "true" } else { "false" };
            return Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::IO, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // io.append_file: {0: path_str, 1: content_str} → #true | #none  (IO)
    // Creates file if absent, appends to existing.
    m.insert("io.append_file".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vp), Some(vc)) = (c.get_field("0"), c.get_field("1")) {
                let fp = oo.force(vp.clone(), ctx);
                let fc = oo.force(vc.clone(), ctx);
                if let (Value::Atom(AtomKind::Str(path), _, _), Value::Atom(AtomKind::Str(content), _, _)) =
                    (fp.collapse(), fc.collapse())
                {
                    let result = std::fs::OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open(path.as_str())
                        .and_then(|mut f| f.write_all(content.as_bytes()));
                    let tag = if result.is_ok() { "true" } else { "none" };
                    return Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::IO, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
}
```

---

## Task 2：更新 `mod.rs`

找到：

```rust
mod json;
```

替換為：

```rust
mod json;
mod io;
```

並在 `create_default_builtins()` 中加入（`json::register_json_builtins(&mut m);` 之後）：

```rust
    io::register_io_builtins(&mut m);
```

---

## Task 3：更新 `root_with_system()`（`lib.rs`）

在 `~%Json` 區塊（`fields.insert("~%Json"...` 那行）之後，`~%Discovery` 區塊之前，插入：

```rust
        let mut io_fields = IndexMap::new();
        let io_morphisms = vec![
            ("/read_file",   "io.read_file"),
            ("/write_file",  "io.write_file"),
            ("/exists",      "io.exists"),
            ("/append_file", "io.append_file"),
        ];
        for (n, b) in io_morphisms {
            io_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(),  Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::IO, vec![])));
        }
        fields.insert("~%Io".to_string(), Value::Combo(ComboVal::new(io_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
```

> 每個 morphism ComboVal 的 EffectTag 設為 **IO**，這是唯一與 `~%Math`/`~%String` 等 Pure 模組的差異。`~%Io` 容器本身仍為 Pure（與 `~%Time` 相同模式）。

---

## Task 4：更新 `genesis.rs`

### 加入常數（在 `SEED_JSON` 之後）

```rust
pub const SEED_IO:        &str = "hash:sha256:v1:PLACEHOLDER_run_seed_test";
```

### 更新 `all_seeds()`（在 `"~%Json"` 條目之後）

```rust
        ("~%Io",         SEED_IO),
```

### 重跑 seed test

```bash
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture 2>&1
```

從輸出的 `UPDATE:` 行找到 `~%Io` 的 CAID，更新 `SEED_IO`。其他 seed 不受影響。

---

## 測試（`tests/io_p34_test.rs`）

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use nlang_interpreter::value::ComboVal;
use tempfile::tempdir;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
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
fn is_true(v: &Value)  -> bool { matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "true") }
fn is_false(v: &Value) -> bool { matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "false") }
fn is_none(v: &Value)  -> bool { matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none") }
fn as_str_content(v: &Value) -> &str {
    match v { Value::Atom(AtomKind::Str(s), _, _) => s, o => panic!("expected Str: {:?}", o) }
}

// ── io.write_file + io.read_file ──────────────────────────────────

#[test]
fn test_io_write_and_read_roundtrip() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.txt").to_string_lossy().into_owned();

    let wrote = call(&oo, &mut ctx, "io.write_file", combo2(str_val(&path), str_val("hello nlang")));
    assert!(is_true(&wrote));

    let content = call(&oo, &mut ctx, "io.read_file", combo1(str_val(&path)));
    assert_eq!(as_str_content(&content), "hello nlang");
    // Result carries IO EffectTag
    assert!(matches!(content, Value::Atom(_, EffectTag::IO, _)));
}

#[test]
fn test_io_read_nonexistent_returns_none() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "io.read_file",
        combo1(str_val("/nonexistent/path/that/cannot/exist/file.txt")));
    assert!(is_none(&r));
}

// ── io.exists ─────────────────────────────────────────────────────

#[test]
fn test_io_exists_true_and_false() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let dir = tempdir().unwrap();
    let path = dir.path().join("exists_test.txt").to_string_lossy().into_owned();

    // Before creation
    assert!(is_false(&call(&oo, &mut ctx, "io.exists", combo1(str_val(&path)))));

    // After creation
    call(&oo, &mut ctx, "io.write_file", combo2(str_val(&path), str_val("x")));
    assert!(is_true(&call(&oo, &mut ctx, "io.exists", combo1(str_val(&path)))));
}

// ── io.write_file truncation ───────────────────────────────────────

#[test]
fn test_io_write_truncates_existing() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let dir = tempdir().unwrap();
    let path = dir.path().join("trunc.txt").to_string_lossy().into_owned();

    call(&oo, &mut ctx, "io.write_file", combo2(str_val(&path), str_val("long content here")));
    call(&oo, &mut ctx, "io.write_file", combo2(str_val(&path), str_val("short")));

    let r = call(&oo, &mut ctx, "io.read_file", combo1(str_val(&path)));
    assert_eq!(as_str_content(&r), "short");
}

// ── io.append_file ────────────────────────────────────────────────

#[test]
fn test_io_append_file() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let dir = tempdir().unwrap();
    let path = dir.path().join("append.txt").to_string_lossy().into_owned();

    call(&oo, &mut ctx, "io.write_file",  combo2(str_val(&path), str_val("hello ")));
    let appended = call(&oo, &mut ctx, "io.append_file", combo2(str_val(&path), str_val("world")));
    assert!(is_true(&appended));

    let r = call(&oo, &mut ctx, "io.read_file", combo1(str_val(&path)));
    assert_eq!(as_str_content(&r), "hello world");
}

#[test]
fn test_io_append_creates_if_absent() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let dir = tempdir().unwrap();
    let path = dir.path().join("new_append.txt").to_string_lossy().into_owned();

    let r = call(&oo, &mut ctx, "io.append_file", combo2(str_val(&path), str_val("created")));
    assert!(is_true(&r));

    let content = call(&oo, &mut ctx, "io.read_file", combo1(str_val(&path)));
    assert_eq!(as_str_content(&content), "created");
}
```

### 加入 `Cargo.toml` 的 test 條目

```toml
[[test]]
name = "io_p34_test"
path = "tests/io_p34_test.rs"
```

---

## 設計備忘

### `EffectTag::IO` 放在哪裡？
- **builtin 回傳值**：`Value::Atom(... , EffectTag::IO, None)` — 表示此值攜帶 IO 副作用
- **morphism ComboVal**：`ComboVal::new(..., EffectTag::IO, vec![])` — 表示此 morphism 本身是 IO 操作
- 兩者都設，與 `time.now` 相同模式

### `io.read_file` 只讀 UTF-8 文字
使用 `std::fs::read_to_string`。非 UTF-8 檔案（如二進位）→ `#none`。若需讀取二進位，使用 `bytes.from_str` + 自訂處理（或未來的 `io.read_bytes`）。

### `io.write_file` 截斷行為
使用 `std::fs::write`，等同 `OpenOptions::new().write(true).create(true).truncate(true)`。

### `io.append_file` 的 `create(true)`
使用 `OpenOptions::new().append(true).create(true)`，若路徑不存在則自動建立（含中間目錄若已存在）。

### `tempfile` 已在 dev-dependencies
`tempfile = "3.3"` 已在 `Cargo.toml` 的 `[dev-dependencies]`，測試可直接使用 `tempdir()`，無需新增依賴。

### 只有 `SEED_IO` 是新的
新增 `~%Io` 欄位到 `root_with_system()` 會改變 genesis 結構的 CAID，但只有 `SEED_IO` 是新常數；其他現有 seed 不受影響。

---

## 驗證步驟

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools

# 1. 編譯
cargo build --manifest-path crates/interpreter/Cargo.toml 2>&1

# 2. 新測試
cargo test --manifest-path crates/interpreter/Cargo.toml io_p34_test -- --nocapture

# 3. seed 更新後穩定性
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture

# 4. 全套不退步
cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：~392 tests, 0 failed
```
