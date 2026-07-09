# Phase 39 Handover：~%Engine.equivalence_map 動態視圖

> 日期：2026-05-25  
> 實作範圍：`engine.equivalence_map` + `engine.resolve`（2 個新態射，State effect）  
> 預期測試：~434 → ~439（新增 ~5 個測試）

---

## 0. 設計摘要

`refine_map` 是 `Ouroboros` 上的 `RwLock<HashMap<String, Vec<String>>>`，記錄每次 `#refine` commit 建立的 CAID → 後繼 CAID 關係。

本 Phase 在 `~%Engine` 新增兩個態射，將這個 map 暴露給 nlang 程式碼：

| 態射 | 輸入 | 輸出 | Effect |
|:-----|:-----|:-----|:------:|
| `/equivalence_map` | `_` | `{%kind:#equivalence_map, %count:Int, entries:list}` | State |
| `/resolve` | `{0: caid_str}` | `Str`（鏈尾 CAID） | State |

`/equivalence_map`：掃描所有 refine 鏈，建立「已精煉 CAID → 最終 CAID」的合成視圖（跳過無精煉的直達情形）。  
`/resolve`：對單一 CAID 字串跟蹤 `follow_refine()` 到鏈尾，若 CAID 不在 map 中則回傳原字串。

---

## 1. 修改 `engine.rs`

### 1.1 新增 import（第 8 行附近）

```rust
// 在現有的 use num_traits::ToPrimitive; 後加入：
use num_bigint::BigInt;
```

### 1.2 新增兩個 builtin（在 `result.flatten` 後、閉括號 `}` 前）

```rust
    // ── Phase 39: equivalence_map + resolve ───────────────────────

    // engine.equivalence_map: _ → {%kind:#equivalence_map, %count:Int, entries:list}  (State)
    // 回傳所有已知 refine 鏈的合成視圖：每個 from_caid 對應其鏈尾 to_caid。
    m.insert("engine.equivalence_map".to_string(), Arc::new(|_arg: Value, oo: &Ouroboros, _ctx: &mut EvalContext| {
        // 1. 取出所有 key（持鎖極短，立即釋放）
        let all_from: Vec<String> = match oo.refine_map.read() {
            Ok(map) => map.keys().cloned().collect(),
            Err(_)  => return BottomCause::Conflict.into(),
        };

        // 2. 對每個 key 跟蹤鏈尾（follow_refine 內部自己取讀鎖，安全）
        let mut entries: Vec<Value> = Vec::new();
        for from_str in &all_from {
            if let Ok(from_hash) = ContentHash::parse(from_str) {
                if let Ok(to_hash) = oo.follow_refine(&from_hash) {
                    let to_str = to_hash.to_string();
                    if to_str != *from_str {   // 只收錄有實際精煉的項目
                        let mut entry = IndexMap::new();
                        entry.insert("from".to_string(), Value::Atom(AtomKind::Str(from_str.clone()), EffectTag::State, None));
                        entry.insert("to".to_string(),   Value::Atom(AtomKind::Str(to_str),          EffectTag::State, None));
                        entries.push(Value::Combo(ComboVal::new(entry, false, IndexMap::new(), EffectTag::State, vec![])));
                    }
                }
            }
        }

        // 3. 包裝成 list
        let mut list_fields = IndexMap::new();
        list_fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::State, None));
        for (i, e) in entries.iter().enumerate() {
            list_fields.insert(i.to_string(), e.clone());
        }
        let entries_list = Value::Combo(ComboVal::new(list_fields, false, IndexMap::new(), EffectTag::State, vec![]));

        // 4. 建立結果 Combo
        let mut result = IndexMap::new();
        result.insert("%kind".to_string(),  Value::Atom(AtomKind::Tag("equivalence_map".to_string()), EffectTag::Pure, None));
        result.insert("%count".to_string(), Value::Atom(AtomKind::Int(BigInt::from(entries.len() as i64)), EffectTag::State, None));
        result.insert("entries".to_string(), entries_list);

        Value::Combo(ComboVal::new(result, true, IndexMap::new(), EffectTag::State, vec![]))
    }) as Arc<BuiltinFn>);

    // engine.resolve: {0: caid_str} → Str(State)
    // 跟蹤 refine 鏈到鏈尾，若 CAID 不在 map 中則回傳原字串。
    m.insert("engine.resolve".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        if let Value::Atom(AtomKind::Str(caid_str), _, _) = forced.collapse() {
            if let Ok(h) = ContentHash::parse(caid_str.as_str()) {
                return match oo.follow_refine(&h) {
                    Ok(resolved) => Value::Atom(AtomKind::Str(resolved.to_string()), EffectTag::State, None),
                    Err(_)       => Value::Top,
                };
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
```

---

## 2. 修改 `lib.rs`

在 `engine_fields.insert("/check_oml"...)` 後、`state_inner` 區塊前，插入：

```rust
        engine_fields.insert("/equivalence_map".to_string(), engine_morph("/equivalence_map", "engine.equivalence_map", EffectTag::State));
        engine_fields.insert("/resolve".to_string(),         engine_morph("/resolve",         "engine.resolve",         EffectTag::State));
```

完成後 `~%Engine` 共有 10 個態射（原 8 + 2 新）：  
`/observe` `/save` `/%differential.{1,2,3}` `/project_down` `/project_up` `/set_strategy` `/check_oml` `/equivalence_map` `/resolve`

---

## 3. genesis.rs

**不需要修改**。genesis.rs 沒有 `SEED_ENGINE`，`~%Engine` 的 ComboVal 不在種子穩定性測試範圍內。

---

## 4. 新增測試

### `crates/interpreter/tests/engine_p39_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;
use nlang_interpreter::value::ComboVal;
use num_bigint::BigInt;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }

fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }

fn combo1(a: Value) -> Value {
    let mut m = IndexMap::new(); m.insert("0".to_string(), a);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}

/// Compute a real CAID string from an integer (deterministic content hash).
fn caid_of(n: i64) -> String {
    Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None)
        .content_hash()
        .to_string()
}

fn is_list(v: &Value) -> bool {
    matches!(v, Value::Combo(c) if matches!(c.get_field("%kind"), Some(Value::Atom(AtomKind::Tag(t), _, _)) if t == "list"))
}

#[test]
fn test_equivalence_map_empty_returns_kind_tag() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "engine.equivalence_map", Value::Top);
    assert!(matches!(r, Value::Combo(_)), "should return a Combo");
    if let Value::Combo(ref c) = r {
        assert!(
            matches!(c.get_field("%kind"), Some(Value::Atom(AtomKind::Tag(t), _, _)) if t == "equivalence_map"),
            "%kind should be #equivalence_map"
        );
        assert!(
            matches!(c.get_field("%count"), Some(Value::Atom(AtomKind::Int(n), _, _)) if n == &BigInt::from(0i64)),
            "%count should be 0 when refine_map is empty"
        );
        let entries = c.get_field("entries").expect("should have entries field");
        assert!(is_list(entries), "entries should be a list");
    }
}

#[test]
fn test_equivalence_map_effect_is_state() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "engine.equivalence_map", Value::Top);
    assert!(matches!(r, Value::Combo(ref c) if c.effect() == EffectTag::State));
}

#[test]
fn test_resolve_unknown_caid_returns_itself() {
    // A CAID not in refine_map → follow_refine returns it unchanged → engine.resolve returns same string
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let caid_str = caid_of(9999);
    let r = call(&oo, &mut ctx, "engine.resolve", combo1(str_val(&caid_str)));
    match &r {
        Value::Atom(AtomKind::Str(s), EffectTag::State, _) => {
            assert_eq!(s, &caid_str, "unrefined CAID should resolve to itself");
        }
        other => panic!("expected Str(State), got {:?}", other),
    }
}

#[test]
fn test_resolve_follows_one_hop() {
    // Insert A → B in refine_map, then resolve(A) should return B
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let caid_a = caid_of(1001);
    let caid_b = caid_of(2001);

    {
        let mut map = oo.refine_map.write().unwrap();
        map.insert(caid_a.clone(), vec![caid_b.clone()]);
    }

    let r = call(&oo, &mut ctx, "engine.resolve", combo1(str_val(&caid_a)));
    match &r {
        Value::Atom(AtomKind::Str(s), EffectTag::State, _) => {
            assert_eq!(s, &caid_b, "resolve should follow A → B");
        }
        other => panic!("expected Str(State), got {:?}", other),
    }
}

#[test]
fn test_equivalence_map_shows_refined_entry() {
    // After inserting A → B, equivalence_map should have one entry {from:A, to:B}
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let caid_a = caid_of(3001);
    let caid_b = caid_of(4001);

    {
        let mut map = oo.refine_map.write().unwrap();
        map.insert(caid_a.clone(), vec![caid_b.clone()]);
    }

    let r = call(&oo, &mut ctx, "engine.equivalence_map", Value::Top);
    if let Value::Combo(ref c) = r {
        assert!(
            matches!(c.get_field("%count"), Some(Value::Atom(AtomKind::Int(n), _, _)) if n == &BigInt::from(1i64)),
            "%count should be 1"
        );
        let entries = c.get_field("entries").expect("entries field");
        if let Value::Combo(ref lc) = entries {
            let entry = lc.get_field("0").expect("entries[0]");
            if let Value::Combo(ref ec) = entry {
                let from = ec.get_field("from").expect("entry.from");
                let to   = ec.get_field("to").expect("entry.to");
                assert!(matches!(from, Value::Atom(AtomKind::Str(s), _, _) if s == &caid_a), "from should be caid_a");
                assert!(matches!(to,   Value::Atom(AtomKind::Str(s), _, _) if s == &caid_b), "to should be caid_b");
            } else { panic!("entries[0] should be a Combo"); }
        } else { panic!("entries should be a Combo"); }
    } else { panic!("result should be a Combo"); }
}
```

---

## 5. 修改 `Cargo.toml`

在 `path_p37_test` 後加入：

```toml
[[test]]
name = "engine_p39_test"
path = "tests/engine_p39_test.rs"
```

---

## 6. 完成後驗證

```bash
cargo test
```

預期：~439 tests，0 failed。

重點確認：
- `engine.equivalence_map` 空 refine_map → `{%kind:#equivalence_map, %count:0, entries:[]}`
- `engine.equivalence_map` 有 A→B → `%count:1, entries:[{from:A, to:B}]`
- `engine.resolve` 未知 CAID → 回傳原字串（`EffectTag::State`）
- `engine.resolve` A→B 在 map 中 → 回傳 B
- 兩個態射的回傳值都是 `EffectTag::State`

---

## 7. 實作注意事項

| 事項 | 說明 |
|:-----|:-----|
| `use num_bigint::BigInt;` | engine.rs 原本只有 `use num_traits::ToPrimitive`，需加入 BigInt |
| 鎖的順序 | 先取讀鎖收集 keys → drop 鎖 → 再對每個 key 呼叫 `follow_refine`（它內部自取讀鎖）。不能持著讀鎖再呼叫 `follow_refine`（避免平台特定的死鎖） |
| `to_str != *from_str` 過濾 | 若 CAID 不在 map（follow_refine 回傳自身），跳過它；equivalence_map 只收錄有實際精煉的項目 |
| 無 SEED_ENGINE | genesis.rs 不追蹤 ~%Engine，不需跑 seed test |
| `caid_of(n)` in tests | `Value::Atom(AtomKind::Int(n.into()), Pure, None).content_hash().to_string()` — 真實 CAID，`ContentHash::parse` 可解析 |
| follow_refine 鏈尾判斷 | 若 from == to（自指），等同無精煉，過濾掉。若 A→B 但 B→A（cycle），`follow_refine` 回傳 `Err(Divergent)` → 跳過 |
