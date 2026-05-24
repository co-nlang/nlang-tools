# Phase 29 交接文件

> 狀態：待實作  
> 前置：Phase 28 完成（~338 tests passing）  
> 目標：`str.format` 支援命名佔位符 `{name}`

---

## 概覽

這是一個**最小改動**的 Phase：只修改 `crates/interpreter/src/builtins/string.rs` 的 `str.format` 實作中的一個分支。不需要改 `lib.rs`（態射已登錄）、不需要改 `genesis.rs`（CAID 不變，builtin 邏輯不影響模組結構 CAID）。

| 任務 | 位置 | 內容 |
|:-----|:-----|:-----|
| Task 1 | `crates/interpreter/src/builtins/string.rs` | 修改 `str.format` 的 `Err(_)` 分支 |
| Tests  | `crates/interpreter/tests/str_format_p29_test.rs`（新建） | 6 個測試 |

預期完成後：**~338 + 6 ≈ 344 tests**

---

## 新語義

### 現有行為（保留，不改）

```
str.format {0: "{} and {0}", 1: ["hello", "world"]}
→ "hello and hello"
```

- `{}` → 自動遞增索引，從 list 的 "0","1",... 取值
- `{N}` → 明確 Int 索引，從 list 的 "N" key 取值
- `{{` / `}}` → 字面 `{` / `}`

### 新增行為

```
str.format {0: "Hi {name}, you are {age}!", 1: {name: "Alice", age: 30}}
→ "Hi Alice, you are 30!"
```

- `{name}`（非數字 inner）→ 查詢 args Combo 的 `name` 欄位
- key 不存在 → 原樣保留 `{name}`（不報錯，原有行為）
- **同時支援 list 與 named Combo**，甚至混用：

```
str.format {0: "{0} likes {thing}", 1: {0: "Alice", thing: "pizza"}}
→ "Alice likes pizza"
```

---

## Task 1：修改 `string.rs`

**找到**（約 222–231 行）：

```rust
                                    match inner.trim().parse::<usize>() {
                                        Ok(idx) => {
                                            result.push_str(args.get(idx).map(|s| s.as_str()).unwrap_or(""));
                                        }
                                        Err(_) => {
                                            result.push('{');
                                            result.push_str(&inner);
                                            result.push('}');
                                        }
                                    }
```

**替換為**：

```rust
                                    match inner.trim().parse::<usize>() {
                                        Ok(idx) => {
                                            result.push_str(args.get(idx).map(|s| s.as_str()).unwrap_or(""));
                                        }
                                        Err(_) => {
                                            let name = inner.trim();
                                            if let Value::Combo(ref nc) = list_forced {
                                                if let Some(v) = nc.get_field(name) {
                                                    result.push_str(&oo.force(v.clone(), ctx).to_string_plain());
                                                } else {
                                                    result.push('{');
                                                    result.push_str(&inner);
                                                    result.push('}');
                                                }
                                            } else {
                                                result.push('{');
                                                result.push_str(&inner);
                                                result.push('}');
                                            }
                                        }
                                    }
```

### 為什麼這樣改是安全的

1. `list_forced` 已在 closure 頂端用 `oo.force(vlist.clone(), ctx)` 計算完畢，在此借用時 `ctx` 的 `&mut` 借用已釋放，不會衝突。
2. 新分支只在 `inner.trim().parse::<usize>()` 失敗（即 inner 不是純數字）時才執行，不影響現有的 numeric index 路徑。
3. key 不存在時回退為原有的「原樣保留」行為，完全向後相容。
4. `{}` 的 auto-index 路徑（`Some(&'}')` 分支）完全不變，仍只從 numeric keys 取值。

---

## 測試（`tests/str_format_p29_test.rs`）

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
fn int(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}

fn named_combo(pairs: Vec<(&str, Value)>) -> Value {
    let mut m = IndexMap::new();
    for (k, v) in pairs { m.insert(k.to_string(), v); }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn format_arg(fmt: &str, args: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), str_val(fmt));
    m.insert("1".to_string(), args);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call_format(oo: &Ouroboros, ctx: &mut EvalContext, arg: Value) -> String {
    let r = oo.builtin_registry.get("str.format").unwrap().clone()(arg, oo, ctx);
    match r {
        Value::Atom(AtomKind::Str(s), _, _) => s,
        other => panic!("expected Str, got {:?}", other),
    }
}

// ── 命名佔位符基本功能 ────────────────────────────────────────────

#[test]
fn test_str_format_named_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let args = named_combo(vec![("name", str_val("Alice"))]);
    let r = call_format(&oo, &mut ctx, format_arg("Hi {name}!", args));
    assert_eq!(r, "Hi Alice!");
}

#[test]
fn test_str_format_named_multiple() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let args = named_combo(vec![("a", str_val("foo")), ("b", str_val("bar"))]);
    let r = call_format(&oo, &mut ctx, format_arg("{a} + {b}", args));
    assert_eq!(r, "foo + bar");
}

#[test]
fn test_str_format_named_int_value() {
    // Int value formatted via to_string_plain
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let args = named_combo(vec![("age", int(30))]);
    let r = call_format(&oo, &mut ctx, format_arg("age={age}", args));
    assert_eq!(r, "age=30");
}

#[test]
fn test_str_format_named_key_not_found_passthrough() {
    // Missing key → pass through literally
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let args = named_combo(vec![("name", str_val("Alice"))]);
    let r = call_format(&oo, &mut ctx, format_arg("{name} {missing}", args));
    assert_eq!(r, "Alice {missing}");
}

#[test]
fn test_str_format_mixed_named_and_numeric() {
    // Named Combo that also has numeric keys → both work
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let args = named_combo(vec![("0", str_val("Alice")), ("thing", str_val("pizza"))]);
    let r = call_format(&oo, &mut ctx, format_arg("{0} likes {thing}", args));
    assert_eq!(r, "Alice likes pizza");
}

#[test]
fn test_str_format_existing_list_still_works() {
    // Backward compat: list args with {} auto-index still works
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let mut m = IndexMap::new();
    m.insert("0".to_string(), str_val("hello"));
    m.insert("1".to_string(), str_val("world"));
    m.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    let list = Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]));
    let r = call_format(&oo, &mut ctx, format_arg("{} and {}", list));
    assert_eq!(r, "hello and world");
}
```

### 加入 `Cargo.toml` 的 test 條目

```toml
[[test]]
name = "str_format_p29_test"
path = "tests/str_format_p29_test.rs"
```

---

## 驗證步驟

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools

# 1. 編譯（應無警告）
cargo build --manifest-path crates/interpreter/Cargo.toml 2>&1

# 2. 新測試
cargo test --manifest-path crates/interpreter/Cargo.toml str_format_p29_test -- --nocapture

# 3. 全套不退步（seed 不需更新）
cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：~344 tests, 0 failed
```
