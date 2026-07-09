# Phase 12 Handover: `@option`/`@result` 標準型別 + Shadow Refinement

**Date:** 2026-05-24  
**Status:** Ready for implementation  
**Depends on:** Phase 10 (refine complete), Phase 11 (complete)  
**Spec refs:** SPEC_09 §2.7–2.8, SPEC_10 §1.1

---

## 目標

完成兩個剩餘 P2 項目：

1. **`@option` / `@result` 標準型別** — 在 `root_with_system()` 預置標準容器型別，並在 `TypeConstraint` 加入驗證邏輯，讓 n/lang 表達式可用 `@option` / `@result` 做型別標注
2. **Shadow Refinement** — 當執行 `#refine(A → B)` 時，掃描歷史 Commit DAG（最多 16 層），收集哪些歷史 Commit 直接引用了 source CAID，記錄為 `RefineInfo::shadow_affected`

---

## 現狀

### `genesis.rs` 已有 8 個 SEED CAID

```rust
pub const SEED_MATH:      &str = "hash:sha256:v1:22a5...";
pub const SEED_LIST:      &str = "hash:sha256:v1:45574...";
// ... 共 8 個
```

`@option` / `@result` 尚未在 `root_with_system()` 中定義，也無對應 SEED。

### `TypeConstraint` 尚無 `Option` / `Result`

`type_constraint.rs` 有 `Any`, `Num`, `Complex`, `Float`, `Int`, `Str`, `Bool`, `List`, `Combo`, `Morphism`, `Unknown` — 缺 `Option` 和 `Result`。

### `RefineInfo` 無歷史影響記錄

```rust
pub struct RefineInfo {
    pub source_caids: Vec<ContentHash>,
    pub target_caids: Vec<ContentHash>,
    pub authority: Option<AuthorityInfo>,
    // ← shadow_affected 不存在
}
```

---

## 任務一：`@option` / `@result` 標準型別

### 1a. `value.rs`：`RefineInfo` 加 `shadow_affected`（提前做，配合任務二）

```rust
pub struct RefineInfo {
    pub source_caids: Vec<ContentHash>,
    pub target_caids: Vec<ContentHash>,
    pub authority: Option<AuthorityInfo>,
    pub shadow_affected: Vec<ContentHash>,   // ← NEW
}
```

在所有 `RefineInfo { ... }` 構造處加 `shadow_affected: vec![]`（現有構造：`universe.rs:134`、`refine_test.rs` 驗證 commit 的那段）。

### 1b. `lib.rs`：在 `root_with_system()` 中加入 `@option` 和 `@result`

在 `~%Complex` 欄位之後（`fields.insert("~%Complex", ...)` 之後）加入：

```rust
// @option: @Some { %val: _ } | #none  (SPEC_09 §2.7)
let mut option_fields = IndexMap::new();
option_fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("type".to_string()), EffectTag::Pure, None));
option_fields.insert("%name".to_string(), Value::Atom(AtomKind::Str("option".to_string()), EffectTag::Pure, None));
option_fields.insert(
    "%some".to_string(),
    Value::Combo(ComboVal::new(
        IndexMap::from_iter(vec![("%val".to_string(), Value::Top)]),
        false, IndexMap::new(), EffectTag::Pure, vec![],
    )),
);
option_fields.insert(
    "%none".to_string(),
    Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
);
fields.insert(
    "@option".to_string(),
    Value::Combo(ComboVal::new(option_fields, true, IndexMap::new(), EffectTag::Pure, vec![])),
);

// @result: @Ok { %val: _ } | @Err { %cause: _ }  (SPEC_09 §2.8)
let mut result_fields = IndexMap::new();
result_fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("type".to_string()), EffectTag::Pure, None));
result_fields.insert("%name".to_string(), Value::Atom(AtomKind::Str("result".to_string()), EffectTag::Pure, None));
result_fields.insert(
    "%ok".to_string(),
    Value::Combo(ComboVal::new(
        IndexMap::from_iter(vec![("%val".to_string(), Value::Top)]),
        false, IndexMap::new(), EffectTag::Pure, vec![],
    )),
);
result_fields.insert(
    "%err".to_string(),
    Value::Combo(ComboVal::new(
        IndexMap::from_iter(vec![("%cause".to_string(), Value::Top)]),
        false, IndexMap::new(), EffectTag::Pure, vec![],
    )),
);
fields.insert(
    "@result".to_string(),
    Value::Combo(ComboVal::new(result_fields, true, IndexMap::new(), EffectTag::Pure, vec![])),
);
```

**欄位設計說明：**
- `%kind: #type` — 標記此 Combo 是型別定義（與 `~%Engine.type_of` 對應）
- `%name: "option"` / `"result"` — 可讀名稱
- `%some: { %val: Top }` — `@Some` 分支的結構（`%val` 存在即為 Some）
- `%none: #none` — `@None` 分支
- `%ok: { %val: Top }` — `@Ok` 分支
- `%err: { %cause: Top }` — `@Err` 分支，`%cause` 與 `BottomDetail.cause` 對應

### 1c. `type_constraint.rs`：新增 `Option` 和 `Result` variant

**TypeConstraint enum:**
```rust
pub enum TypeConstraint {
    Any, Num, Complex, Float, Int, Str, Bool, List, Combo, Morphism,
    Option,   // ← NEW
    Result,   // ← NEW
    Unknown(String),
}
```

**`from_name()`:**
```rust
"option" => TypeConstraint::Option,
"result" => TypeConstraint::Result,
```

**`validate_value()`（在 `Morphism` arm 之後加）:**
```rust
TypeConstraint::Option => match value {
    // #none — the zero-dimensional projection (absence)
    Value::Atom(AtomKind::Tag(t), _, _) if t == "none" => ValidationResult::Pass,
    // @Some { %val: _ } — any Combo with %val field
    Value::Combo(cv) if cv.get_field("%val").is_some() => ValidationResult::Pass,
    // Top subsumes @option
    Value::Top => ValidationResult::Pass,
    _ => ValidationResult::Fail(
        "Value is not @option (expected #none or Combo with %val)".to_string()
    ),
},
TypeConstraint::Result => match value {
    // @Ok { %val: _ }
    Value::Combo(cv) if cv.get_field("%val").is_some() => ValidationResult::Pass,
    // @Err { %cause: _ }
    Value::Combo(cv) if cv.get_field("%cause").is_some() => ValidationResult::Pass,
    // Top subsumes @result
    Value::Top => ValidationResult::Pass,
    _ => ValidationResult::Fail(
        "Value is not @result (expected Combo with %val or %cause)".to_string()
    ),
},
```

### 1d. `genesis.rs`：新增 `SEED_OPTION` 和 `SEED_RESULT`

```rust
pub const SEED_OPTION: &str = "PLACEHOLDER_RUN_TESTS_TO_GET_HASH";
pub const SEED_RESULT: &str = "PLACEHOLDER_RUN_TESTS_TO_GET_HASH";
```

在 `all_seeds()` 加入：
```rust
("@option", SEED_OPTION),
("@result", SEED_RESULT),
```

**取得真實 CAID 的方式：**
```bash
cargo test seed_caids_are_stable -- --nocapture 2>&1 | grep "UPDATE:"
```

確認有輸出 `@option` 和 `@result` 的 hash 之後複製到常數。若 `seed_caids_are_stable` 測試目前只驗證現有 8 個 seed，需先擴充測試（見下方測試段落），再執行取得 hash，再填入常數。

---

## 任務二：Shadow Refinement

### 2a. `universe.rs`：在 `refine()` 加入 Step 1c 掃描

**位置：** Step 1b（authority verification）之後、Step 2（build Commit）之前。

```rust
// Step 1c: Shadow scan — identify historical commits that directly reference source CAIDs
const SHADOW_SCAN_DEPTH: usize = 16;
let mut shadow_affected: Vec<ContentHash> = Vec::new();
{
    let mut current = self.head.clone();
    let mut depth = 0;
    while let Some(ref ch) = current.clone() {
        if depth >= SHADOW_SCAN_DEPTH { break; }
        depth += 1;
        let commit = match engine.store.get_commit(ch) {
            Ok(c) => c,
            Err(_) => break,
        };
        let root_val = match engine.store.get_value(&commit.root) {
            Ok(v) => v,
            Err(_) => { current = commit.parent; continue; }
        };
        // Check if this commit's root directly contains any source CAID as a field value
        if let Value::Combo(ref cv) = root_val {
            'field_scan: for (_, fv) in cv.all_fields_iter() {
                let fh = fv.content_hash();
                for src in &source_caids {
                    if &fh == src {
                        shadow_affected.push(ch.clone());
                        break 'field_scan;  // one match per commit is enough
                    }
                }
            }
        }
        current = commit.parent;
    }
}
```

**語義說明：** Shadow scan 是純資訊性的——它找出哪些歷史 Commit 的根 Combo 直接把 source CAID 作為欄位值。這些 Commit 在語義上受到此 refinement 的「影」覆蓋：未來對這些 Commit 的 `get_live_value()` 查詢將透過 refine_map 重定向到 target。Shadow scan 本身**不阻塞** refine（Step 1 的單調性已保證幾何正確性）。

### 2b. `universe.rs`：將 `shadow_affected` 傳入 `RefineInfo`

Step 2 的 `refine_info` 構造改為：
```rust
refine_info: Some(RefineInfo {
    source_caids: source_caids.clone(),
    target_caids: target_caids.clone(),
    authority,
    shadow_affected,        // ← NEW (populated by Step 1c)
}),
```

### 2c. CLI 回報（`crates/oo/src/main.rs`，`run_refine()`）

在 `println!("Refine commit: {}", hash);` 之後加：

```rust
// We can't access refine_info from here directly, but we can load the commit
if let Ok(engine_ref) = Ouroboros::init(&cur) {
    if let Ok(commit) = engine_ref.store.get_commit(&hash) {
        if let Some(ri) = commit.refine_info {
            if !ri.shadow_affected.is_empty() {
                println!("Shadow: {} historical commit(s) reference source CAID(s):", ri.shadow_affected.len());
                for ch in &ri.shadow_affected {
                    println!("  - {}", ch);
                }
            }
        }
    }
}
```

實際上更簡單的做法：`run_refine` 傳回 `universe.refine()` 後，在同一個 universe 上再讀取 head commit：

```rust
let hash = universe.refine(&engine, &cur, source_caids, target_caids, authority, meta)?;
println!("Refine commit: {}", hash);

// Report shadow-affected commits
if let Ok(commit) = engine.store.get_commit(&hash) {
    if let Some(ri) = commit.refine_info {
        if !ri.shadow_affected.is_empty() {
            println!("Shadow: {} historical commit(s) will be semantically updated:", ri.shadow_affected.len());
            for ch in &ri.shadow_affected {
                println!("  {}", ch);
            }
        }
    }
}
```

---

## 測試

### 新增至 `crates/interpreter/tests/refine_test.rs`

```rust
// Test: shadow_affected is empty when universe has no history
#[test]
fn shadow_affected_empty_on_fresh_universe() {
    let (mut u, oo, base_dir) = setup();
    let base_dir = base_dir.join("shadow_empty");

    let val_a = Value::Top;
    let val_b = Value::Atom(nlang_parser::ast::AtomKind::Int(42.into()), EffectTag::Pure, None);
    let ca = oo.store.put_value(&val_a).unwrap();
    let cb = oo.store.put_value(&val_b).unwrap();

    let meta = CommitMeta { author: None, timestamp: 0, message: None };
    let ch = u.refine(&oo, &base_dir, vec![ca], vec![cb], None, meta).unwrap();

    let commit = oo.store.get_commit(&ch).unwrap();
    let ri = commit.refine_info.unwrap();
    assert!(ri.shadow_affected.is_empty(), "fresh universe → no shadow history");
}

// Test: shadow_affected is non-empty when a prior commit references source CAID
#[test]
fn shadow_affected_detects_historical_usage() {
    let (mut u, oo, base_dir) = setup();
    let base_dir = base_dir.join("shadow_detect");

    // Put a value A into store
    let val_a = Value::Top;
    let ca = oo.store.put_value(&val_a).unwrap();

    // Commit a universe state where ca appears as a field value in root
    let val_b_precise = Value::Atom(nlang_parser::ast::AtomKind::Int(99.into()), EffectTag::Pure, None);
    let cb = oo.store.put_value(&val_b_precise).unwrap();

    // Do a normal refine first (sets self.head)
    let meta1 = CommitMeta { author: None, timestamp: 0, message: None };
    u.refine(&oo, &base_dir, vec![ca.clone()], vec![cb.clone()], None, meta1).unwrap();
    // Now u.head is Some(...)

    // Now do a SECOND refine with a different pair — the first commit has ca as source
    let val_c = Value::Top;
    let val_d = Value::Atom(nlang_parser::ast::AtomKind::Int(1000.into()), EffectTag::Pure, None);
    let cc = oo.store.put_value(&val_c).unwrap();
    let cd = oo.store.put_value(&val_d).unwrap();
    let meta2 = CommitMeta { author: None, timestamp: 1, message: None };
    let ch2 = u.refine(&oo, &base_dir, vec![cc], vec![cd], None, meta2).unwrap();

    let commit2 = oo.store.get_commit(&ch2).unwrap();
    let ri2 = commit2.refine_info.unwrap();
    // The first refine commit's root contains ca (as source CAID in RefineInfo) —
    // shadow scan should find it. Note: shadow scan checks root Combo fields, not refine_info.
    // The root of the first refine commit is the universe's root at that time.
    // Since ca = Top and the root may or may not contain ca as a direct field,
    // this test mainly verifies shadow_affected Vec exists and doesn't panic.
    assert!(ri2.shadow_affected.len() <= 16, "shadow scan bounded to 16 commits");
}
```

**注意：** 第二個測試較難構造「root Combo 直接含有 source CAID 的 field」，因為 `universe.refine()` 不會把 source CAID 寫進 root Combo（root 是整個 universe 狀態）。若要觸發 shadow hit，需要：用 `universe.evolve()` 先把一個 field 設為某個值，commit 後再 refine 那個值。

**更清晰的 shadow hit 測試：**

```rust
#[test]
fn shadow_scan_finds_field_in_committed_root() {
    use nlang_parser::{parse_program};
    
    let oo = Arc::new(Ouroboros::new_in_memory());
    let base_dir = std::env::temp_dir().join("nlang-shadow-hit");
    let _ = std::fs::create_dir_all(&base_dir);
    let mut u = Universe::new(None, oo.root_with_system());

    // Evolve: set field "myval" to a value, then commit
    let val_to_evolve = Value::Atom(nlang_parser::ast::AtomKind::Int(77.into()), EffectTag::Pure, None);
    let caid_77 = oo.store.put_value(&val_to_evolve).unwrap();
    
    // Manually set root to contain a field whose content_hash == caid_77
    let mut root_fields = IndexMap::new();
    root_fields.insert("tracked_field".to_string(), val_to_evolve.clone());
    u.root = nlang_interpreter::value::ComboVal::new(root_fields, false, IndexMap::new(), EffectTag::Pure, vec![]);
    
    // Commit this root
    let root_hash = oo.store.put_value(&Value::Combo(u.root.clone())).unwrap();
    let commit0 = nlang_interpreter::value::Commit {
        parent: None,
        root: root_hash,
        meta: CommitMeta { author: None, timestamp: 0, message: None },
        kind: nlang_interpreter::value::CommitKind::Commit,
        refine_info: None,
        cache_id: nlang_interpreter::value::default_cache_id(),
    };
    let ch0 = oo.store.put_commit(&commit0).unwrap();
    oo.store.set_head(&base_dir, &ch0).unwrap();
    u.head = Some(ch0);

    // Now refine: caid_77 → something more precise (77 & 77 = 77 ✓ trivially)
    // Actually need: tgt & src = tgt. If src = 77 and tgt = 77, meet = 77 = tgt ✓
    // But a more interesting case: src = Top, tgt = 77
    let val_top = Value::Top;
    let caid_top = oo.store.put_value(&val_top).unwrap();
    // The root's "tracked_field" has content_hash == caid_77
    // But our source should be caid_top (Top) and our tracked_field's hash == caid_77
    // so shadow scan won't find it unless we use caid_77 as source.
    // Let's use caid_77 directly as source, refine to something more precise:
    let val_precise = Value::Atom(nlang_parser::ast::AtomKind::Int(77.into()), EffectTag::Pure, None); // same val = trivial
    let caid_precise = oo.store.put_value(&val_precise).unwrap();
    
    let meta = CommitMeta { author: None, timestamp: 1, message: None };
    let ch_refine = u.refine(&oo, &base_dir, vec![caid_77.clone()], vec![caid_precise.clone()], None, meta).unwrap();
    
    let commit = oo.store.get_commit(&ch_refine).unwrap();
    let ri = commit.refine_info.unwrap();
    // The root commit (ch0) has a field with content_hash == caid_77 → should appear in shadow_affected
    assert!(!ri.shadow_affected.is_empty(), "shadow scan should find the historical commit with tracked_field == val_77");
    assert!(ri.shadow_affected.contains(&ch0), "shadow_affected should include ch0");
    
    let _ = std::fs::remove_dir_all(&base_dir);
}
```

### 新增至 `crates/interpreter/tests/genesis_test.rs`（或現有 caid_test.rs）

```rust
// Test: @option is in root_with_system
#[test]
fn at_option_in_root_with_system() {
    let oo = Ouroboros::new_in_memory();
    let root = oo.root_with_system();
    assert!(root.get_field("@option").is_some(), "@option should be in root_with_system");
}

// Test: @result is in root_with_system
#[test]
fn at_result_in_root_with_system() {
    let oo = Ouroboros::new_in_memory();
    let root = oo.root_with_system();
    assert!(root.get_field("@result").is_some(), "@result should be in root_with_system");
}

// Test: TypeConstraint::Option validates #none
#[test]
fn type_constraint_option_accepts_none() {
    use nlang_interpreter::type_constraint::{TypeConstraint, ValidationResult};
    use nlang_parser::ast::AtomKind;
    let v = Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
    assert!(matches!(TypeConstraint::Option.validate_value(&v), ValidationResult::Pass));
}

// Test: TypeConstraint::Option validates { %val: _ }
#[test]
fn type_constraint_option_accepts_some() {
    use nlang_interpreter::type_constraint::{TypeConstraint, ValidationResult};
    use nlang_interpreter::value::ComboVal;
    use indexmap::IndexMap;
    let mut fields = IndexMap::new();
    fields.insert("%val".to_string(), Value::Atom(nlang_parser::ast::AtomKind::Int(42.into()), EffectTag::Pure, None));
    let v = Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
    assert!(matches!(TypeConstraint::Option.validate_value(&v), ValidationResult::Pass));
}

// Test: TypeConstraint::Result validates { %val: _ } and { %cause: _ }
#[test]
fn type_constraint_result_accepts_ok_and_err() {
    use nlang_interpreter::type_constraint::{TypeConstraint, ValidationResult};
    use nlang_interpreter::value::ComboVal;
    use indexmap::IndexMap;

    let mut ok_fields = IndexMap::new();
    ok_fields.insert("%val".to_string(), Value::Top);
    let ok = Value::Combo(ComboVal::new(ok_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
    assert!(matches!(TypeConstraint::Result.validate_value(&ok), ValidationResult::Pass));

    let mut err_fields = IndexMap::new();
    err_fields.insert("%cause".to_string(), Value::Atom(nlang_parser::ast::AtomKind::Tag("timeout".to_string()), EffectTag::Pure, None));
    let err = Value::Combo(ComboVal::new(err_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
    assert!(matches!(TypeConstraint::Result.validate_value(&err), ValidationResult::Pass));
}
```

若 `type_constraint` 模組非 pub，在 `lib.rs` 加：
```rust
pub mod type_constraint;
```

---

## 驗收條件

1. `cargo test --workspace 2>&1 | grep FAILED` — 零失敗
2. `oo.root_with_system().get_field("@option").is_some()` == true
3. `oo.root_with_system().get_field("@result").is_some()` == true
4. `TypeConstraint::from_name("@option")` == `TypeConstraint::Option`
5. `TypeConstraint::from_name("@result")` == `TypeConstraint::Result`
6. `SEED_OPTION` / `SEED_RESULT` 填入真實 hash（非 PLACEHOLDER）
7. `RefineInfo::shadow_affected` 欄位存在且型別為 `Vec<ContentHash>`
8. 新建 universe、commit 含 source 欄位、再 refine → `shadow_affected` 非空
9. `oo refine` CLI 在有 shadow-affected 時印出歷史 Commit 清單

---

## 不在本 Phase 的工作

- **`overlapping_masa_caids` 動態計算** — P3，需節點互相知曉 field key 集合
- **Shadow refinement 非同步化** — 目前同步掃描 16 commit，非同步化留 Phase 14+
- **`@list` / `@morphism` SEED 對齊** — 已有對應 system fields，型別標注驗證留後
- **lattice_sketch_test_suite v2 跨架構測試向量** — 仍 P2，Phase 13 考慮
- **自我演化（SPEC_17）** — P3，長期目標
