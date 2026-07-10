# Phase 28 交接文件

> 狀態：待實作  
> 前置：Phase 27 完成（~327 tests passing）  
> 目標：`list.group_by`、`list.chunk`、`list.window`

---

## 概覽

| 任務 | 位置 | 內容 |
|:-----|:-----|:-----|
| Task 1 | `crates/interpreter/src/builtins/list.rs` | 3 個新 list builtins（加在 `list.unique` 之後） |
| Task 2 | `crates/interpreter/src/lib.rs` | 補入 `~%List` 的 `/group_by`、`/chunk`、`/window` 態射 |
| Task 3 | `crates/interpreter/src/genesis.rs` | 重跑 seed test，更新 SEED_LIST |
| Tests  | `crates/interpreter/tests/list_p28_test.rs`（新建） | ~11 個測試 |

預期完成後：**~327 + 11 ≈ 338 tests**

---

## 語義速查

| builtin | 輸入 | 輸出 | 說明 |
|:--------|:-----|:-----|:-----|
| `list.group_by` | `{0: key_fn, 1: list}` | `Combo`（非 list） | 按 key_fn 結果分組，欄位名 = key 的字串表示 |
| `list.chunk` | `{0: n, 1: list}` | `list of lists` | 切成大小 n 的子列表，最後一塊可能較小 |
| `list.window` | `{0: n, 1: list}` | `list of lists` | 滑動視窗大小 n；list.len < n → 空 list |

---

## Task 1：新 builtins（`list.rs`）

加在 `list.unique` 的閉包之後（約 633 行末 `})`）：

### `list.group_by`

語義：對每個 item 應用 `key_fn`，以結果的 `to_string_plain()` 為鍵分組，回傳 Combo（鍵 = group key，值 = 該 group 的 list）。  
群組順序為第一次出現的順序（IndexMap 保證）。

```rust
    m.insert("list.group_by".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vf), Some(vl)) = (c.get_field("0"), c.get_field("1")) {
                let key_fn = vf.clone();
                let list = oo.force(vl.clone(), ctx);
                let items = extract_list_items(&list);
                let mut groups: IndexMap<String, Vec<Value>> = IndexMap::new();
                for item in items {
                    let item_forced = oo.force(item, ctx);
                    let key = oo.apply_morphism(key_fn.clone(), item_forced.clone(), ctx);
                    let key_str = key.collapse().to_string_plain();
                    groups.entry(key_str).or_insert_with(Vec::new).push(item_forced);
                }
                let mut out = ComboVal::default();
                for (key, group_items) in groups {
                    out.insert_field(&key, build_list_value(group_items));
                }
                return Value::Combo(out);
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
```

### `list.chunk`

語義：把 list 切成每塊大小為 n 的子列表。最後一塊可能小於 n。n ≤ 0 → Top。

```rust
    m.insert("list.chunk".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vn), Some(vl)) = (c.get_field("0"), c.get_field("1")) {
                let fn_ = oo.force(vn.clone(), ctx);
                let list = oo.force(vl.clone(), ctx);
                if let Value::Atom(AtomKind::Int(n), _, _) = fn_.collapse() {
                    let size = match n.to_usize() {
                        Some(s) if s > 0 => s,
                        _ => return Value::Top,
                    };
                    let items = extract_list_items(&list);
                    let chunks: Vec<Value> = items.chunks(size)
                        .map(|chunk| build_list_value(chunk.to_vec()))
                        .collect();
                    return build_list_value(chunks);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
```

### `list.window`

語義：產生所有連續大小為 n 的滑動視窗。`[1,2,3,4]` n=2 → `[[1,2],[2,3],[3,4]]`。  
list.len < n → 空 list。n ≤ 0 → Top。

```rust
    m.insert("list.window".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vn), Some(vl)) = (c.get_field("0"), c.get_field("1")) {
                let fn_ = oo.force(vn.clone(), ctx);
                let list = oo.force(vl.clone(), ctx);
                if let Value::Atom(AtomKind::Int(n), _, _) = fn_.collapse() {
                    let size = match n.to_usize() {
                        Some(s) if s > 0 => s,
                        _ => return Value::Top,
                    };
                    let items = extract_list_items(&list);
                    if items.len() < size {
                        return build_list_value(vec![]);
                    }
                    let windows: Vec<Value> = (0..=(items.len() - size))
                        .map(|i| build_list_value(items[i..i + size].to_vec()))
                        .collect();
                    return build_list_value(windows);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
```

**注意事項**：
- `extract_list_items` 回傳未 force 的 Value；`list.group_by` 需 force 每個 item 再傳入 key_fn，所以有 `oo.force(item, ctx)`。`list.chunk` 和 `list.window` 不強制 force（與 `list.slice` 等行為一致）。
- `items.chunks(size)` 是 Rust 標準庫方法，回傳 `&[Value]` 切片；`.to_vec()` 複製為 `Vec<Value>`（Value 已 derive Clone）。
- `IndexMap::entry()` 與 `HashMap::entry()` 語義相同，但保留插入順序。

---

## Task 2：更新 `root_with_system()`（`lib.rs`）

找到 `~%List` 的 `list_morphisms` vec（應已包含 Phase 25 的 28 個態射），在尾端加入：

```rust
            // Phase 28
            ("/group_by", "list.group_by"),
            ("/chunk",    "list.chunk"),
            ("/window",   "list.window"),
```

---

## Task 3：更新 genesis.rs（重跑 seed test）

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture 2>&1
```

從輸出複製 `~%List` 的新 CAID，更新：
```rust
pub const SEED_LIST: &str = "hash:sha256:v1:...";  // ← 更新
```

---

## 測試（`tests/list_p28_test.rs`）

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn int(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
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
fn combo2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn list_len(v: &Value) -> usize {
    match v {
        Value::Combo(c) => c.fields().keys().filter(|k| k.parse::<usize>().is_ok()).count(),
        _ => panic!("expected list, got {:?}", v),
    }
}
fn list_at(v: &Value, i: usize) -> &Value {
    match v {
        Value::Combo(c) => c.get_field(&i.to_string()).expect("index out of bounds"),
        _ => panic!("expected list"),
    }
}

// ── list.group_by ─────────────────────────────────────────────────

#[test]
fn test_list_group_by_sign() {
    // group_by(math.sign, [-2, 0, 3]) → {"-1": [-2], "0": [0], "1": [3]}
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let list = make_list(vec![int(-2), int(0), int(3)]);
    let r = call(&oo, &mut ctx, "list.group_by", combo2(morph("math.sign"), list));
    if let Value::Combo(c) = &r {
        assert!(c.get_field("-1").is_some(), "missing '-1' group");
        assert!(c.get_field("0").is_some(),  "missing '0' group");
        assert!(c.get_field("1").is_some(),  "missing '1' group");
        assert_eq!(list_len(c.get_field("-1").unwrap()), 1);
        assert_eq!(list_len(c.get_field("0").unwrap()),  1);
        assert_eq!(list_len(c.get_field("1").unwrap()),  1);
    } else { panic!("expected Combo, got {:?}", r); }
}

#[test]
fn test_list_group_by_all_same_key() {
    // group_by(math.abs, [1, 2, 3]) with abs → each item its own group
    // Use a constant: group all into one group by sign(0) = 0 for all? No.
    // Use sign for [1, 2, 3] → all go to "1" group
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let list = make_list(vec![int(1), int(2), int(3)]);
    let r = call(&oo, &mut ctx, "list.group_by", combo2(morph("math.sign"), list));
    if let Value::Combo(c) = &r {
        assert_eq!(list_len(c.get_field("1").unwrap()), 3);
        assert!(c.get_field("-1").is_none());
        assert!(c.get_field("0").is_none());
    } else { panic!("expected Combo"); }
}

#[test]
fn test_list_group_by_empty() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.group_by", combo2(morph("math.sign"), make_list(vec![])));
    if let Value::Combo(c) = &r {
        assert!(c.fields().is_empty() || c.fields().keys().all(|k| k.starts_with('%')));
    } else { panic!("expected Combo"); }
}

// ── list.chunk ────────────────────────────────────────────────────

#[test]
fn test_list_chunk_even() {
    // chunk(2, [1,2,3,4]) → [[1,2], [3,4]]
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let list = make_list(vec![int(1), int(2), int(3), int(4)]);
    let r = call(&oo, &mut ctx, "list.chunk", combo2(int(2), list));
    assert_eq!(list_len(&r), 2, "expected 2 chunks");
    assert_eq!(list_len(list_at(&r, 0)), 2);
    assert_eq!(list_len(list_at(&r, 1)), 2);
}

#[test]
fn test_list_chunk_with_remainder() {
    // chunk(2, [1,2,3,4,5]) → [[1,2], [3,4], [5]]
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let list = make_list(vec![int(1), int(2), int(3), int(4), int(5)]);
    let r = call(&oo, &mut ctx, "list.chunk", combo2(int(2), list));
    assert_eq!(list_len(&r), 3, "expected 3 chunks");
    assert_eq!(list_len(list_at(&r, 0)), 2);
    assert_eq!(list_len(list_at(&r, 1)), 2);
    assert_eq!(list_len(list_at(&r, 2)), 1);
}

#[test]
fn test_list_chunk_larger_than_list() {
    // chunk(10, [1,2,3]) → [[1,2,3]]
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let list = make_list(vec![int(1), int(2), int(3)]);
    let r = call(&oo, &mut ctx, "list.chunk", combo2(int(10), list));
    assert_eq!(list_len(&r), 1);
    assert_eq!(list_len(list_at(&r, 0)), 3);
}

#[test]
fn test_list_chunk_empty() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.chunk", combo2(int(3), make_list(vec![])));
    assert_eq!(list_len(&r), 0);
}

// ── list.window ───────────────────────────────────────────────────

#[test]
fn test_list_window_basic() {
    // window(2, [1,2,3,4]) → [[1,2], [2,3], [3,4]]
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let list = make_list(vec![int(1), int(2), int(3), int(4)]);
    let r = call(&oo, &mut ctx, "list.window", combo2(int(2), list));
    assert_eq!(list_len(&r), 3, "expected 3 windows");
    // First window: [1,2]
    let w0 = list_at(&r, 0);
    assert_eq!(list_at(w0, 0).to_string_plain(), "1");
    assert_eq!(list_at(w0, 1).to_string_plain(), "2");
    // Last window: [3,4]
    let w2 = list_at(&r, 2);
    assert_eq!(list_at(w2, 0).to_string_plain(), "3");
    assert_eq!(list_at(w2, 1).to_string_plain(), "4");
}

#[test]
fn test_list_window_size_equals_list_len() {
    // window(3, [1,2,3]) → [[1,2,3]]
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let list = make_list(vec![int(1), int(2), int(3)]);
    let r = call(&oo, &mut ctx, "list.window", combo2(int(3), list));
    assert_eq!(list_len(&r), 1);
    assert_eq!(list_len(list_at(&r, 0)), 3);
}

#[test]
fn test_list_window_larger_than_list() {
    // window(5, [1,2,3]) → []
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let list = make_list(vec![int(1), int(2), int(3)]);
    let r = call(&oo, &mut ctx, "list.window", combo2(int(5), list));
    assert_eq!(list_len(&r), 0);
}
```

### 加入 `Cargo.toml` 的 test 條目

```toml
[[test]]
name = "list_p28_test"
path = "tests/list_p28_test.rs"
```

---

## 驗證步驟

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools

# 1. 編譯
cargo build --manifest-path crates/interpreter/Cargo.toml 2>&1

# 2. 新測試
cargo test --manifest-path crates/interpreter/Cargo.toml list_p28_test -- --nocapture

# 3. 種子更新後穩定性
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture

# 4. 全套不退步
cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：~338 tests, 0 failed
```
