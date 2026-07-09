# Phase 37 Handover：~%Path

> 日期：2026-05-25  
> 實作範圍：~%Path（path.join/dirname/basename/extension/is_absolute，5 態射）  
> 預期測試：~418 → ~427（新增 ~9 個測試）

---

## 0. 摘要

新增 `~%Path` 模組：純字串路徑操作，**不碰檔案系統**，全部 `EffectTag::Pure`。

| 態射 | 輸入 | 輸出 |
|:-----|:-----|:-----|
| `/join` | `{0: base_str, 1: seg_str}` | `Str` |
| `/dirname` | `{0: path_str}` | `Str \| #none` |
| `/basename` | `{0: path_str}` | `Str \| #none` |
| `/extension` | `{0: path_str}` | `Str \| #none` |
| `/is_absolute` | `{0: path_str}` | `#true \| #false` |

---

## 1. 新增 `path.rs`

**建立** `crates/interpreter/src/builtins/path.rs`：

```rust
use std::sync::Arc;
use std::collections::HashMap;
use std::path::Path;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;

pub fn register_path_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {

    // path.join: {0: base_str, 1: seg_str} → Str
    // Note: if seg is absolute, it replaces base (std::path behavior)
    m.insert("path.join".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(v0), Some(v1)) = (c.get_field("0"), c.get_field("1")) {
                let f0 = oo.force(v0.clone(), ctx);
                let f1 = oo.force(v1.clone(), ctx);
                if let (Value::Atom(AtomKind::Str(base), _, _), Value::Atom(AtomKind::Str(seg), _, _)) =
                    (f0.collapse(), f1.collapse())
                {
                    let joined = Path::new(base.as_str()).join(seg.as_str());
                    return Value::Atom(AtomKind::Str(joined.to_string_lossy().into_owned()), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // path.dirname: {0: path_str} → Str | #none
    // Returns parent directory string. Root "/" → #none. Relative "foo" → "".
    m.insert("path.dirname".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        if let Value::Atom(AtomKind::Str(path), _, _) = forced.collapse() {
            return match Path::new(path.as_str()).parent() {
                Some(p) => Value::Atom(AtomKind::Str(p.to_string_lossy().into_owned()), EffectTag::Pure, None),
                None    => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
            };
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // path.basename: {0: path_str} → Str | #none
    // Returns last component. "/" → #none. "/foo/" → "foo".
    m.insert("path.basename".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        if let Value::Atom(AtomKind::Str(path), _, _) = forced.collapse() {
            return match Path::new(path.as_str()).file_name() {
                Some(name) => Value::Atom(AtomKind::Str(name.to_string_lossy().into_owned()), EffectTag::Pure, None),
                None       => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
            };
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // path.extension: {0: path_str} → Str | #none
    // Returns extension without dot. "bar.txt" → "txt". ".hidden" → #none. "bar" → #none.
    m.insert("path.extension".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        if let Value::Atom(AtomKind::Str(path), _, _) = forced.collapse() {
            return match Path::new(path.as_str()).extension() {
                Some(ext) => Value::Atom(AtomKind::Str(ext.to_string_lossy().into_owned()), EffectTag::Pure, None),
                None      => Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
            };
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // path.is_absolute: {0: path_str} → #true | #false
    m.insert("path.is_absolute".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        if let Value::Atom(AtomKind::Str(path), _, _) = forced.collapse() {
            let tag = if Path::new(path.as_str()).is_absolute() { "true" } else { "false" };
            return Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::Pure, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);
}
```

**注意**：
- 無需 `num_bigint` 或 `indexmap` —— 只用 `std::path::Path`，import 最精簡
- `forced.collapse()` 模式：先 `let forced = oo.force(v, ctx);`，再 `forced.collapse()`
- `path.join` 的二參數版本和 io.rs 的 `write_file` 同樣模式（`c.get_field("0")` / `c.get_field("1")`）

---

## 2. 修改 `builtins/mod.rs`

在 `mod process;`（最後一行 mod）後加入：

```rust
mod path;
```

在 `process::register_process_builtins(&mut m);` 後加入：

```rust
path::register_path_builtins(&mut m);
```

---

## 3. 修改 `lib.rs`

在 `~%Process` 區塊後、`~%Discovery` 區塊前，插入：

```rust
        let mut path_fields = IndexMap::new();
        let path_morphisms = vec![
            ("/join",        "path.join"),
            ("/dirname",     "path.dirname"),
            ("/basename",    "path.basename"),
            ("/extension",   "path.extension"),
            ("/is_absolute", "path.is_absolute"),
        ];
        for (n, b) in path_morphisms {
            path_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(),  Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])));
        }
        fields.insert("~%Path".to_string(), Value::Combo(ComboVal::new(path_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
```

注意：`~%Path` 的 morphism ComboVal 用 `EffectTag::Pure`（不同於 ~%Io、~%Env、~%Process 的 IO）。

---

## 4. 修改 `genesis.rs`

在 `SEED_PROCESS` 後加入：

```rust
pub const SEED_PATH: &str = "hash:sha256:v1:PLACEHOLDER_PATH";
```

在 `all_seeds()` 的 `("~%Process", SEED_PROCESS)` 後加入：

```rust
        ("~%Path", SEED_PATH),
```

**跑種子測試**：

```bash
cargo test seed_caids_are_stable -- --nocapture
```

將輸出的 `UPDATE:` 行中 `SEED_PATH` 的值複製回 `genesis.rs`，替換 PLACEHOLDER。

---

## 5. 新增測試

### `crates/interpreter/tests/path_p37_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use nlang_interpreter::value::ComboVal;

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
fn as_str(v: &Value) -> &str {
    match v { Value::Atom(AtomKind::Str(s), _, _) => s, o => panic!("expected Str: {:?}", o) }
}
fn is_none(v: &Value) -> bool { matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none") }
fn is_true(v: &Value)  -> bool { matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "true") }
fn is_false(v: &Value) -> bool { matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "false") }
fn is_pure(v: &Value) -> bool { matches!(v, Value::Atom(_, EffectTag::Pure, _)) }

#[test]
fn test_path_join_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "path.join", combo2(str_val("/foo"), str_val("bar")));
    assert_eq!(as_str(&r), "/foo/bar");
    assert!(is_pure(&r));
}

#[test]
fn test_path_join_nested() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "path.join", combo2(str_val("/a/b"), str_val("c/d")));
    assert_eq!(as_str(&r), "/a/b/c/d");
}

#[test]
fn test_path_dirname_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "path.dirname", combo1(str_val("/foo/bar.txt")));
    assert_eq!(as_str(&r), "/foo");
    assert!(is_pure(&r));
}

#[test]
fn test_path_dirname_root_returns_none() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "path.dirname", combo1(str_val("/")));
    assert!(is_none(&r));
}

#[test]
fn test_path_basename_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "path.basename", combo1(str_val("/foo/bar.txt")));
    assert_eq!(as_str(&r), "bar.txt");
    assert!(is_pure(&r));
}

#[test]
fn test_path_basename_root_returns_none() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "path.basename", combo1(str_val("/")));
    assert!(is_none(&r));
}

#[test]
fn test_path_extension_with_ext() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "path.extension", combo1(str_val("/foo/bar.txt")));
    assert_eq!(as_str(&r), "txt");
    assert!(is_pure(&r));
}

#[test]
fn test_path_extension_no_ext() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "path.extension", combo1(str_val("/foo/bar")));
    assert!(is_none(&r));
}

#[test]
fn test_path_extension_dotfile_has_no_ext() {
    // ".hidden" — std::path treats the whole name as stem, no extension
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "path.extension", combo1(str_val(".hidden")));
    assert!(is_none(&r));
}

#[test]
fn test_path_is_absolute_true() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "path.is_absolute", combo1(str_val("/foo/bar")));
    assert!(is_true(&r));
    assert!(is_pure(&r));
}

#[test]
fn test_path_is_absolute_false() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "path.is_absolute", combo1(str_val("foo/bar")));
    assert!(is_false(&r));
}
```

---

## 6. 修改 `Cargo.toml`

在 `process_p36_test` 後加入：

```toml
[[test]]
name = "path_p37_test"
path = "tests/path_p37_test.rs"
```

---

## 7. 完成後驗證

```bash
cargo test
```

預期：~429 tests，0 failed。

重點確認：
- `path.join("/foo", "bar")` → `"/foo/bar"`
- `path.dirname("/")` → `#none`（root 無 parent）
- `path.dirname("/foo/bar")` → `"/foo"`
- `path.basename("/")` → `#none`
- `path.extension("bar.txt")` → `"txt"`（無 dot）
- `path.extension(".hidden")` → `#none`（dotfile 整體是 stem）
- `path.is_absolute("/foo")` → `#true`
- 所有態射回傳值均為 `EffectTag::Pure`
- SEED_PATH 已從 PLACEHOLDER 更新為實際 sha256 值

---

## 8. 實作注意事項

| 事項 | 說明 |
|:-----|:-----|
| 全部 Pure | 路徑操作是純字串運算，不讀寫檔案系統，用 `EffectTag::Pure` |
| `path.join` 絕對路徑行為 | `Path::new("/a").join("/b")` → `"/b"`（std 行為，segment 為絕對路徑時取代 base） |
| `.extension()` 不含 dot | Rust 的 `Path::extension()` 已去掉 `.`，回傳 `"txt"` 而非 `".txt"` |
| `.hidden` dotfile | `Path::new(".hidden").extension()` → `None`（整個名稱是 stem） |
| 無新 dep | 只用 `std::path::Path`，Cargo.toml 不需要改動（除了加 `[[test]]`） |
