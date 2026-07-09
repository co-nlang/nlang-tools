# Phase 18 交接文件

> 狀態：待實作  
> 前置：Phase 17 完成（211 tests passing）  
> 目標：標準庫完整化 — list 查詢/結構操作 + result/option 解包

---

## 概覽

| 任務 | 位置 | 新增測試數 |
|:-----|:-----|:---------:|
| Task 1：`list.any` / `list.all` / `list.find` | `builtins/list.rs` | 7 |
| Task 2：`list.head` / `list.tail` / `list.take` / `list.drop` | `builtins/list.rs` | 6 |
| Task 3：`result.unwrap` / `result.expect` / `option.expect` | `builtins/engine.rs` | 6 |

預期完成後：211 + 19 ≈ **230 tests**

---

## 背景知識（程式碼模式）

### List 表示法

List 是一個 Combo，key 為 `"0"`, `"1"`, ... + `%kind: #list` + `%len: n`（Phase 17 加入）。

### 既有 helper（`list.rs` 內部函數，Phase 17 加入）

```rust
fn extract_list_items(list: &Value) -> Vec<Value> {
    // 讀 %len，找 "0".."n-1"，返回 Vec<Value>
}
fn build_list_value(items: Vec<Value>) -> Value {
    // 建 Combo with numeric keys + %kind + %len
}
```

**Task 1 和 Task 2 的所有新 builtins 必須加在 `register_list_builtins` 函數內部（`list.flat_map` 之後，最後的 `}` 之前），才能用到這兩個 inner function。**

### Boolean 判斷

```rust
value.to_string_plain().trim_start_matches('#') == "true"
```

（參照 `list.filter` 的做法）

### Option 格式

- None: `Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None)`
- Some: `Value::Combo(cv)` where `cv.get_field("%val").is_some()`

### Result 格式

- Ok: `Value::Combo(cv)` where `cv.get_field("%val").is_some()`
- Err: `Value::Combo(cv)` where `cv.get_field("%cause").is_some()`

### Bottom 格式

```rust
Value::Bottom(Box::new(BottomDetail {
    cause: BottomCause::Conflict,
    message: Some("訊息".to_string()),
    ..Default::default()
}))
```

---

## Task 1：`list.any` / `list.all` / `list.find`

### 位置

`crates/interpreter/src/builtins/list.rs`，加在 `list.flat_map` 之後（`}` 之前）。

### 語義

```
list.any  : {0: pred_fn, 1: list} → #true | #false
  空 list → #false
  任一元素 pred_fn(e) = #true → #true
  全部不滿足 → #false

list.all  : {0: pred_fn, 1: list} → #true | #false
  空 list → #true（vacuously true）
  任一元素 pred_fn(e) ≠ #true → #false
  全部滿足 → #true

list.find : {0: pred_fn, 1: list} → Option
  找到第一個 pred_fn(e) = #true 的元素 → Some({%val: e})
  找不到 → #none
```

### 實作

```rust
m.insert("list.any".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(pred_f), Some(list_v)) = (c.get_field("0"), c.get_field("1")) {
            let pred_f = pred_f.clone();
            let list = oo.force(list_v.clone(), ctx);
            let items = extract_list_items(&list);
            for item in items {
                let result = oo.apply_morphism(pred_f.clone(), item, ctx);
                if result.to_string_plain().trim_start_matches('#') == "true" {
                    return Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None);
                }
            }
            return Value::Atom(AtomKind::Tag("false".to_string()), EffectTag::Pure, None);
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);

m.insert("list.all".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(pred_f), Some(list_v)) = (c.get_field("0"), c.get_field("1")) {
            let pred_f = pred_f.clone();
            let list = oo.force(list_v.clone(), ctx);
            let items = extract_list_items(&list);
            for item in items {
                let result = oo.apply_morphism(pred_f.clone(), item, ctx);
                if result.to_string_plain().trim_start_matches('#') != "true" {
                    return Value::Atom(AtomKind::Tag("false".to_string()), EffectTag::Pure, None);
                }
            }
            return Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None);
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);

m.insert("list.find".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let none_val = Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
    if let Value::Combo(ref c) = arg {
        if let (Some(pred_f), Some(list_v)) = (c.get_field("0"), c.get_field("1")) {
            let pred_f = pred_f.clone();
            let list = oo.force(list_v.clone(), ctx);
            let items = extract_list_items(&list);
            for item in items {
                let result = oo.apply_morphism(pred_f.clone(), item.clone(), ctx);
                if result.to_string_plain().trim_start_matches('#') == "true" {
                    let mut fields = IndexMap::new();
                    fields.insert("%val".to_string(), item);
                    return Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
                }
            }
            return none_val;
        }
    }
    none_val
}) as Arc<BuiltinFn>);
```

### 必要 import 確認

`list.rs` 已有：`use indexmap::IndexMap;`, `use crate::value::{Value, ComboVal, EffectTag};`, `use nlang_parser::ast::AtomKind;`。  
全部 Task 1/2 都在現有 import 範圍內，不需要新增。

### 測試

測試檔：`tests/list_query_test.rs`（新建）

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}

fn make_list(oo: &Ouroboros, items: Vec<Value>) -> Value {
    // 複製 flat_map_test.rs 中的 make_list helper
    // {0: items[0], 1: items[1], ..., %kind: #list, %len: n}
    use indexmap::IndexMap;
    use nlang_interpreter::value::{ComboVal};
    let mut fields = IndexMap::new();
    for (i, v) in items.iter().enumerate() { fields.insert(i.to_string(), v.clone()); }
    fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    fields.insert("%len".to_string(), int_val(items.len() as i64));
    Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    let f = oo.builtin_registry.get(name).expect("builtin not found").clone();
    f(arg, oo, ctx)
}

fn is_tag(v: &Value, t: &str) -> bool {
    if let Value::Atom(AtomKind::Tag(s), _, _) = v { s.trim_start_matches('#') == t } else { false }
}

#[test]
fn test_list_any_true() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    // 需要一個 pred_fn：任何 Value → #true
    // 用 list.filter 的模式：建立 Code 值，但測試中最簡單的是用已有的 builtin 構造
    // 替代方案：直接用 list.find 的結果 is_some 來驗證
    // 此測試用「非空 list + 永遠返回 #true 的 pred」→ #true
    // pred_fn 用 Value::Top（apply_morphism(Top, x) 回傳 Top，不等於 #true）
    // 改用 list 包含 #true 元素 + pred = identity 的等效驗證
    // 實際上最簡單：list.any 的謂詞接受一個函數，在測試中最易建立的是用 list.filter 等已知能工作的 pred 格式
    // 所以此測試套件應放在 flat_map_test.rs 那種模式下（見下面 Note）
    let _ = oo; // placeholder — 見 Note
}
```

**Note**: 測試中需要建立謂詞函數值。參照 `flat_map_test.rs` 中既有的 `make_fn_*` helper，或使用以下模式：

```rust
// 建立「永遠返回 #true」的 builtin 包裝：
// 在 Ouroboros 中暫時插入一個測試用 builtin
// 或：用 oo.eval() 搭配 nlang 語法字串（如果有 eval API）

// 最務實的方式（參照 cond_match_test.rs 模式）：
// 用 oo.eval_str("...") 寫整個表達式字串然後 assert 結果
```

如果引擎有 `oo.eval_str(expr: &str) -> Value` 或類似接口，直接用：

```rust
#[test]
fn test_list_any_true() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(&oo, vec![int_val(1), int_val(2), int_val(3)]);
    // pred: 永遠返回 #true — 用 refl.is_bottom 的反向，或直接造一個返回 #true 的 Thunk
    // 最簡單：查看現有測試裡怎麼建立 pred，照搬
    // 如果只能用 Value 而非字串 eval，可以用：
    // refl.type_of 的 builtin 作為 pred（返回字串），然後 list.any 看 == "int"？
    // 但這樣 pred 返回值不是 #true/#false
    // 最可靠：用 list.filter 作為參照，filter 能工作代表 any/all/find 的 pred 機制相同
    let _ = (list, ctx);
}
```

**務實建議**：讓執行 AI 參照 `tests/flat_map_test.rs` 和 `tests/functor_test.rs` 中現有的謂詞建立方式，自行決定測試 helper 的實作，確保：
- `test_list_any_true`：非空 list，有元素滿足謂詞 → `#true`
- `test_list_any_false`：非空 list，全部不滿足 → `#false`
- `test_list_any_empty`：空 list → `#false`
- `test_list_all_true`：全部滿足 → `#true`
- `test_list_all_false`：有一個不滿足 → `#false`
- `test_list_find_found`：找到第一個滿足的 → `Some({%val: v})`，v 正確
- `test_list_find_not_found`：找不到 → `#none`

---

## Task 2：`list.head` / `list.tail` / `list.take` / `list.drop`

### 位置

同上，加在 Task 1 的 builtins 之後。

### 語義

```
list.head : list → Option
  非空 list → Some({%val: 第一個元素})
  空 list → #none

list.tail : list → list
  [a, b, c, ...] → [b, c, ...]
  [] → []（空 list）
  [a] → []

list.take : {0: n, 1: list} → list
  取前 n 個元素（n 大於長度時取全部）

list.drop : {0: n, 1: list} → list
  跳過前 n 個元素，返回剩餘（n 大於長度時返回 []）
```

### 實作

```rust
m.insert("list.head".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let none_val = Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
    let target = if let Value::Combo(ref c) = arg {
        c.get_field("0").cloned().unwrap_or_else(|| arg.clone())
    } else { arg.clone() };
    let list = oo.force(target, ctx);
    let items = extract_list_items(&list);
    if items.is_empty() {
        return none_val;
    }
    let mut fields = IndexMap::new();
    fields.insert("%val".to_string(), items[0].clone());
    Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]))
}) as Arc<BuiltinFn>);

m.insert("list.tail".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let target = if let Value::Combo(ref c) = arg {
        c.get_field("0").cloned().unwrap_or_else(|| arg.clone())
    } else { arg.clone() };
    let list = oo.force(target, ctx);
    let items = extract_list_items(&list);
    if items.len() <= 1 {
        return build_list_value(vec![]);
    }
    build_list_value(items[1..].to_vec())
}) as Arc<BuiltinFn>);

m.insert("list.take".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(vn), Some(vlist)) = (c.get_field("0"), c.get_field("1")) {
            let n_forced = oo.force(vn.clone(), ctx);
            let list = oo.force(vlist.clone(), ctx);
            if let Value::Atom(AtomKind::Int(ref n), _, _) = n_forced {
                let n = n.to_usize().unwrap_or(0);
                let items = extract_list_items(&list);
                let taken = items.into_iter().take(n).collect();
                return build_list_value(taken);
            }
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);

m.insert("list.drop".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(vn), Some(vlist)) = (c.get_field("0"), c.get_field("1")) {
            let n_forced = oo.force(vn.clone(), ctx);
            let list = oo.force(vlist.clone(), ctx);
            if let Value::Atom(AtomKind::Int(ref n), _, _) = n_forced {
                let n = n.to_usize().unwrap_or(0);
                let items = extract_list_items(&list);
                let dropped = items.into_iter().skip(n).collect();
                return build_list_value(dropped);
            }
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);
```

### 注意事項

- `list.head` 和 `list.tail` 接受直接的 list 值（不是 `{0: list}` 包裝）。  
  使用 `c.get_field("0").unwrap_or(arg)` 相容兩種呼叫方式（直接傳 list，或傳 `{0: list}`）。
- `list.take` / `list.drop` 使用 `{0: n, 1: list}` 格式（兩個參數）。
- `n.to_usize()` 如果 n 是負數返回 `None` → 取 0，行為同 take(0) = []。

### 測試

測試檔：`tests/list_structural_test.rs`（新建）

```rust
#[test]
fn test_list_head_some() {
    // head([10, 20, 30]) → Some({%val: 10})
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(&oo, vec![int_val(10), int_val(20), int_val(30)]);
    let result = call(&oo, &mut ctx, "list.head", list);
    // result should be Some
    if let Value::Combo(ref cv) = result {
        let inner = cv.get_field("%val").expect("should have %val");
        assert_eq!(inner.to_string_plain(), "10");
    } else { panic!("expected Some, got {:?}", result); }
}

#[test]
fn test_list_head_empty() {
    // head([]) → #none
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(&oo, vec![]);
    let result = call(&oo, &mut ctx, "list.head", list);
    assert!(is_tag(&result, "none"), "expected #none, got {:?}", result);
}

#[test]
fn test_list_tail_normal() {
    // tail([1, 2, 3]) → [2, 3]
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(&oo, vec![int_val(1), int_val(2), int_val(3)]);
    let result = call(&oo, &mut ctx, "list.tail", list);
    if let Value::Combo(ref cv) = result {
        assert_eq!(cv.get_field("0").unwrap().to_string_plain(), "2");
        assert_eq!(cv.get_field("1").unwrap().to_string_plain(), "3");
        assert!(cv.get_field("2").is_none());
    } else { panic!("expected list combo"); }
}

#[test]
fn test_list_take_n() {
    // take(2, [10, 20, 30, 40]) → [10, 20]
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(&oo, vec![int_val(10), int_val(20), int_val(30), int_val(40)]);
    let arg = make_combo_2(&oo, int_val(2), list);
    let result = call(&oo, &mut ctx, "list.take", arg);
    if let Value::Combo(ref cv) = result {
        assert!(cv.get_field("0").is_some());
        assert!(cv.get_field("1").is_some());
        assert!(cv.get_field("2").is_none());
    } else { panic!("expected list"); }
}

#[test]
fn test_list_drop_n() {
    // drop(2, [10, 20, 30, 40]) → [30, 40]
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(&oo, vec![int_val(10), int_val(20), int_val(30), int_val(40)]);
    let arg = make_combo_2(&oo, int_val(2), list);
    let result = call(&oo, &mut ctx, "list.drop", arg);
    if let Value::Combo(ref cv) = result {
        assert_eq!(cv.get_field("0").unwrap().to_string_plain(), "30");
        assert_eq!(cv.get_field("1").unwrap().to_string_plain(), "40");
        assert!(cv.get_field("2").is_none());
    } else { panic!("expected list"); }
}

#[test]
fn test_list_tail_empty() {
    // tail([]) → []
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let list = make_list(&oo, vec![]);
    let result = call(&oo, &mut ctx, "list.tail", list);
    if let Value::Combo(ref cv) = result {
        assert!(cv.get_field("0").is_none(), "tail of empty should be empty");
    } else { panic!("expected empty list combo"); }
}
```

---

## Task 3：`result.unwrap` / `result.expect` / `option.expect`

### 位置

`crates/interpreter/src/builtins/engine.rs`，加在 `result.and_then` 之後。

### 語義

```
result.unwrap : result → Value
  Ok({%val: v}) → v
  Err({%cause: c}) → Bottom(Conflict, "called unwrap on Err: <c>")

result.expect : {0: msg, 1: result} → Value
  Ok({%val: v}) → v
  Err({%cause: c}) → Bottom(Conflict, "<msg>: <c>")

option.expect : {0: msg, 1: option} → Value
  Some({%val: v}) → v
  None → Bottom(Conflict, "<msg>")
```

### 實作

```rust
// result.unwrap: result → Value (or Bottom on Err)
m.insert("result.unwrap".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let res = oo.force(arg, ctx);
    match &res {
        Value::Combo(ref cv) => {
            if let Some(inner) = cv.get_field("%val").cloned() {
                return inner;
            }
            if let Some(cause) = cv.get_field("%cause") {
                return Value::Bottom(Box::new(BottomDetail {
                    cause: BottomCause::Conflict,
                    message: Some(format!("called unwrap on Err: {}", cause.to_string_plain())),
                    ..Default::default()
                }));
            }
        }
        _ => {}
    }
    Value::Top
}) as Arc<BuiltinFn>);

// result.expect: {0: msg_str, 1: result} → Value (or Bottom with message)
m.insert("result.expect".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(msg_v), Some(res_v)) = (c.get_field("0"), c.get_field("1")) {
            let msg = oo.force(msg_v.clone(), ctx).to_string_plain();
            let res = oo.force(res_v.clone(), ctx);
            match &res {
                Value::Combo(ref cv) => {
                    if let Some(inner) = cv.get_field("%val").cloned() {
                        return inner;
                    }
                    if let Some(cause) = cv.get_field("%cause") {
                        return Value::Bottom(Box::new(BottomDetail {
                            cause: BottomCause::Conflict,
                            message: Some(format!("{}: {}", msg, cause.to_string_plain())),
                            ..Default::default()
                        }));
                    }
                }
                _ => {}
            }
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);

// option.expect: {0: msg_str, 1: option} → Value (or Bottom on None)
m.insert("option.expect".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(msg_v), Some(opt_v)) = (c.get_field("0"), c.get_field("1")) {
            let msg = oo.force(msg_v.clone(), ctx).to_string_plain();
            let opt = oo.force(opt_v.clone(), ctx);
            match &opt {
                Value::Atom(AtomKind::Tag(ref t), _, _) if t.trim_start_matches('#') == "none" => {
                    return Value::Bottom(Box::new(BottomDetail {
                        cause: BottomCause::Conflict,
                        message: Some(msg),
                        ..Default::default()
                    }));
                }
                Value::Combo(ref cv) => {
                    if let Some(inner) = cv.get_field("%val").cloned() {
                        return inner;
                    }
                }
                _ => {}
            }
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);
```

### 注意事項

- `result.unwrap` 接受單一參數（直接是 result 值），不是 `{0: result}` 包裝格式。  
  因此 `oo.force(arg, ctx)` 直接對 arg 求值，不需要先 `c.get_field("0")`。
- `to_string_plain()` 用來把 cause 轉成人類可讀字串放入 message。
- `BottomDetail` 需要 `..Default::default()` 填充其他欄位。確認 `BottomDetail` 有實作 `Default` trait（Phase 7 時已加）。
- 這三個函數都是 `#[allow(dead_code)]` 的潛在候選，但實際上它們是公開 builtins，不會有 dead code 警告。

### 測試

測試檔：`tests/unwrap_test.rs`（新建）

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal, BottomCause};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use num_bigint::BigInt;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn int_val(n: i64) -> Value {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
}

fn str_val(s: &str) -> Value {
    Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None)
}

fn make_ok(v: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("%val".to_string(), v);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_err(cause: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("%cause".to_string(), cause);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_some(v: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("%val".to_string(), v);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn make_none() -> Value {
    Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None)
}

fn make_combo_2(oo: &Ouroboros, a: Value, b: Value) -> Value {
    let mut f = IndexMap::new();
    f.insert("0".to_string(), a);
    f.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(f, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    let f = oo.builtin_registry.get(name).expect("builtin not found").clone();
    f(arg, oo, ctx)
}

#[test]
fn test_result_unwrap_ok() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let ok = make_ok(int_val(42));
    let result = call(&oo, &mut ctx, "result.unwrap", ok);
    assert_eq!(result.to_string_plain(), "42");
}

#[test]
fn test_result_unwrap_err() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let err = make_err(str_val("bad_input"));
    let result = call(&oo, &mut ctx, "result.unwrap", err);
    match result {
        Value::Bottom(ref detail) => {
            assert!(matches!(detail.cause, BottomCause::Conflict));
            assert!(detail.message.as_deref().unwrap_or("").contains("unwrap"));
        }
        _ => panic!("expected Bottom, got {:?}", result),
    }
}

#[test]
fn test_result_expect_ok() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let ok = make_ok(int_val(99));
    let arg = make_combo_2(&oo, str_val("parse error"), ok);
    let result = call(&oo, &mut ctx, "result.expect", arg);
    assert_eq!(result.to_string_plain(), "99");
}

#[test]
fn test_result_expect_err() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let err = make_err(str_val("timeout"));
    let arg = make_combo_2(&oo, str_val("fetch failed"), err);
    let result = call(&oo, &mut ctx, "result.expect", arg);
    match result {
        Value::Bottom(ref detail) => {
            let msg = detail.message.as_deref().unwrap_or("");
            assert!(msg.contains("fetch failed"), "msg was: {}", msg);
            assert!(msg.contains("timeout"), "msg was: {}", msg);
        }
        _ => panic!("expected Bottom"),
    }
}

#[test]
fn test_option_expect_some() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let some = make_some(int_val(7));
    let arg = make_combo_2(&oo, str_val("should be present"), some);
    let result = call(&oo, &mut ctx, "option.expect", arg);
    assert_eq!(result.to_string_plain(), "7");
}

#[test]
fn test_option_expect_none() {
    let oo = make_oo();
    let mut ctx = oo.eval_context();
    let none = make_none();
    let arg = make_combo_2(&oo, str_val("expected a value"), none);
    let result = call(&oo, &mut ctx, "option.expect", arg);
    match result {
        Value::Bottom(ref detail) => {
            assert!(detail.message.as_deref().unwrap_or("").contains("expected a value"));
        }
        _ => panic!("expected Bottom"),
    }
}
```

---

## 執行順序

Task 1、2、3 互相獨立，可以任意順序進行：

1. Task 1 + Task 2：都在 `list.rs`，加在 `list.flat_map` 之後。
2. Task 3：在 `engine.rs`，加在 `result.and_then` 之後。

最後驗證：

```bash
cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：~230 tests, 0 failed

cargo test list_query -- --nocapture    # Task 1
cargo test list_structural -- --nocapture  # Task 2
cargo test unwrap -- --nocapture        # Task 3
```

## 完成後狀態

Phase 18 完成後，`~%List` 的核心操作集完整：

| 分類 | builtins |
|:-----|:---------|
| 基礎 | len, at, concat, reverse, slice, zip, sort |
| 函子 | map, flat_map |
| 折疊 | fold, filter |
| 查詢 | **any, all, find** ← Phase 18 |
| 結構 | **head, tail, take, drop** ← Phase 18 |

`~%Engine` option/result 組合子完整：map, map_err, and_then, or, unwrap_or, filter, **unwrap, expect** ← Phase 18
