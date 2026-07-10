# Phase 16 Handover: Monad bind + LADD cosine distance + ~%Reflection 擴充

**Date:** 2026-05-24  
**Status:** Ready for implementation  
**Depends on:** Phase 15 (complete)  
**Spec refs:** SPEC_09 §1, APP_05 §3.2, SPEC_11 §1

---

## 目標

三個獨立任務：

1. **`option.and_then` / `result.and_then`** — Monad bind，補完 Phase 15 的 `%fmap` Functor 層
2. **`d_l_approx` cosine similarity** — LADD 格論距離從 XOR/Hamming 升級為 cosine，更符合 APP_05 §3.2 的譜幾何語意
3. **`~%Reflection` 擴充** — 新增 7 個反射操作：`/is_blur`, `/is_bottom`, `/is_some`, `/is_none`, `/is_ok`, `/is_err`, `/to_str`, `/bottom_cause`；修正 `/type_of` 遺漏 Blur

---

## 任務一：`option.and_then` / `result.and_then`

### 背景

Monad bind（flatMap/chain/and_then）是 Functor 的延伸：`fmap` 包裝結果，`and_then` 讓 `f` 直接返回 container 型別（避免 `Some(Some(x))`）。

- `option.and_then(f, Some(x)) = f(x)`（f 返回 @option）
- `option.and_then(f, #none) = #none`
- `result.and_then(f, Ok(x)) = f(x)`（f 返回 @result）
- `result.and_then(f, Err(e)) = Err(e)`

**設計決策**：不加到 `@option`/`@result` 型別定義欄位（避免再次更動 SEED_OPTION/SEED_RESULT），作為獨立 builtin 提供。

### 改動位置：`crates/interpreter/src/builtins/engine.rs`

在 Phase 15 新增的 `result.map_err` 之後加入：

```rust
// ── Monad bind (and_then / chain) ───────────────────────────────

m.insert("option.and_then".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    // arg = { 0: f, 1: option_value }
    // f : A → @option B
    if let Value::Combo(ref c) = arg {
        if let (Some(f), Some(opt_v)) = (c.get_field("0"), c.get_field("1")) {
            let f = f.clone();
            let opt = oo.force(opt_v.clone(), ctx);
            return match opt.collapse() {
                // #none → #none (propagate)
                Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none" => {
                    Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None)
                }
                // { %val: x } → f(x)  (f returns @option, no extra wrapping)
                Value::Combo(ref cv) => {
                    if let Some(inner) = cv.get_field("%val") {
                        oo.apply_morphism(f, inner.clone(), ctx)
                    } else {
                        Value::Top
                    }
                }
                _ => Value::Top,
            };
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);

m.insert("result.and_then".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    // arg = { 0: f, 1: result_value }
    // f : A → @result B
    if let Value::Combo(ref c) = arg {
        if let (Some(f), Some(res_v)) = (c.get_field("0"), c.get_field("1")) {
            let f = f.clone();
            let res = oo.force(res_v.clone(), ctx);
            if let Value::Combo(ref cv) = res.collapse() {
                if let Some(inner) = cv.get_field("%val") {
                    // Ok(x) → f(x)
                    return oo.apply_morphism(f, inner.clone(), ctx);
                }
                if cv.get_field("%cause").is_some() {
                    // Err(e) → Err(e) unchanged
                    return res.collapse().clone();
                }
            }
        }
    }
    Value::Top
}) as Arc<BuiltinFn>);
```

### 測試：追加至 `crates/interpreter/tests/functor_test.rs`

```rust
#[test]
fn option_and_then_some_chains() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());

    // and_then(identity_as_some, Some(42)) = Some(42)
    // Use identity: Top as morphism → apply_morphism(Top, x) = x
    // But x must be @option shaped, so use a Some wrapper
    // Simplest test: and_then with identity morphism on Some(42)
    // Since apply_morphism(Top, 42) = 42, result should be 42
    let opt = make_some(make_int(42));
    let morph = get_match_builtin("option.and_then", &oo);
    let arg = make_map_arg(Value::Top, opt); // Top identity → returns inner value directly
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    // Top applied to 42 = 42
    if let Value::Atom(AtomKind::Int(n), _, _) = result.collapse() {
        assert_eq!(n.to_string(), "42", "and_then Some should chain: {:?}", result);
    } else {
        panic!("Expected Int(42), got {:?}", result);
    }
}

#[test]
fn option_and_then_none_propagates() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());

    let opt = make_none();
    let morph = get_match_builtin("option.and_then", &oo);
    let arg = make_map_arg(Value::Top, opt);
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    if let Value::Atom(AtomKind::Tag(t), _, _) = result.collapse() {
        assert_eq!(t.trim_start_matches('#'), "none",
            "and_then None should propagate #none: {:?}", result);
    } else {
        panic!("Expected #none, got {:?}", result);
    }
}

#[test]
fn result_and_then_ok_chains() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());

    let res = make_ok(make_int(99));
    let morph = get_match_builtin("result.and_then", &oo);
    let arg = make_map_arg(Value::Top, res);
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    if let Value::Atom(AtomKind::Int(n), _, _) = result.collapse() {
        assert_eq!(n.to_string(), "99", "result.and_then Ok should chain: {:?}", result);
    } else {
        panic!("Expected Int(99), got {:?}", result);
    }
}

#[test]
fn result_and_then_err_propagates() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());

    let err_cause = Value::Atom(AtomKind::Tag("fail".to_string()), EffectTag::Pure, None);
    let res = make_err(err_cause);
    let morph = get_match_builtin("result.and_then", &oo);
    let arg = make_map_arg(Value::Top, res);
    let result = oo.force(oo.apply_morphism(morph, arg, &mut ctx), &mut ctx);
    if let Value::Combo(ref c) = result.collapse() {
        assert!(c.get_field("%cause").is_some(),
            "result.and_then Err should propagate: {:?}", result);
    } else {
        panic!("Expected Err combo, got {:?}", result);
    }
}
```

**注意**：`functor_test.rs` 已有 `make_some`, `make_none`, `make_ok`, `make_err`, `make_map_arg`, `get_match_builtin`, `make_int` 等輔助函式，直接使用即可。

---

## 任務二：`d_l_approx` cosine similarity

### 背景

`d_l_approx` 目前用 XOR bit count（Hamming 距離），對 sketch bytes 做 bit-level 差異統計。這是一種合理的距離，但 APP_05 §3.2 的譜幾何語意是**投影算子特徵值的 arccos**，在 sketch 層面最接近的近似是 **cosine similarity**：

$$d_L(A, B) = \frac{\arccos(\text{cos\_sim}(s_A, s_B))}{\pi} \in [0, 1]$$

**關鍵特性不變**：
- 完全相同 sketch → cos=1 → arccos(1)=0 → `d_L=0` ✓（現有測試 `test_d_l_approx_identical` 仍過）
- 空 sketch → return 1.0 ✓（`test_d_l_approx_empty` 仍過）
- 結果範圍 [0, 1] ✓（`test_d_l_approx_range` 仍過）

**改善**：Hamming 只看 bit 翻轉，cosine 保留 sketch 振幅方向信息，更能反映格論投影算子的幾何相似性。

### 改動位置：`crates/interpreter/src/ladd.rs`

**原始 `d_l_approx`（約 22–35 行）：**
```rust
/// Approximate spectral distance via sketch Hamming distance.
pub fn d_l_approx(a: &GBB, b: &GBB) -> f64 {
    if a.sketch_bytes.is_empty() || b.sketch_bytes.is_empty() {
        return 1.0;
    }
    let min_len = a.sketch_bytes.len().min(b.sketch_bytes.len());
    let xor_bits: u32 = a.sketch_bytes[..min_len]
        .iter()
        .zip(&b.sketch_bytes[..min_len])
        .map(|(x, y)| (x ^ y).count_ones())
        .sum();
    let max_bits = (min_len * 8) as f64;
    (xor_bits as f64) / max_bits
}
```

**替換為：**
```rust
/// Approximate spectral distance via sketch cosine similarity (APP_05 §3.2).
/// d_L = arccos(cos_sim) / π  ∈ [0, 1].
pub fn d_l_approx(a: &GBB, b: &GBB) -> f64 {
    if a.sketch_bytes.is_empty() || b.sketch_bytes.is_empty() {
        return 1.0;
    }
    let min_len = a.sketch_bytes.len().min(b.sketch_bytes.len());
    // Treat bytes as i8 amplitude samples
    let av: Vec<f64> = a.sketch_bytes[..min_len].iter().map(|&x| x as i8 as f64).collect();
    let bv: Vec<f64> = b.sketch_bytes[..min_len].iter().map(|&x| x as i8 as f64).collect();
    let dot: f64   = av.iter().zip(bv.iter()).map(|(x, y)| x * y).sum();
    let na: f64    = av.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb: f64    = bv.iter().map(|x| x * x).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        // Both zero-vectors → identical in amplitude space
        return if na == nb { 0.0 } else { 1.0 };
    }
    let cos_sim = (dot / (na * nb)).clamp(-1.0, 1.0);
    cos_sim.acos() / std::f64::consts::PI
}
```

### 測試：追加至 `crates/interpreter/tests/ladd_test.rs`

```rust
#[test]
fn test_d_l_approx_cosine_different() {
    // Two non-identical non-empty sketches → distance in (0, 1)
    let a = GBB {
        node_caid: ContentHash::default(),
        mass: 1.0,
        sketch_bytes: vec![1u8, 0, 0, 0],
        masa_ref: MasaRef::Top,
        nerve_structure: vec![],
    };
    let b = GBB {
        node_caid: ContentHash::default(),
        mass: 1.0,
        sketch_bytes: vec![0u8, 1, 0, 0],  // orthogonal to a
        masa_ref: MasaRef::Top,
        nerve_structure: vec![],
    };
    let d = d_l_approx(&a, &b);
    // [1,0,0,0] · [0,1,0,0] = 0 → cos=0 → arccos(0)=π/2 → d=0.5
    assert!((d - 0.5).abs() < 1e-10, "orthogonal sketches → d_L ≈ 0.5, got {}", d);
}

#[test]
fn test_d_l_approx_identical_still_zero() {
    // Verify existing contract preserved after cosine change
    let bytes = vec![42u8, 17, 255, 0, 128];
    let a = GBB { node_caid: ContentHash::default(), mass: 1.0,
        sketch_bytes: bytes.clone(), masa_ref: MasaRef::Top, nerve_structure: vec![] };
    let b = GBB { node_caid: ContentHash::default(), mass: 1.0,
        sketch_bytes: bytes, masa_ref: MasaRef::Top, nerve_structure: vec![] };
    assert_eq!(d_l_approx(&a, &b), 0.0, "identical sketch → d_L = 0");
}
```

**注意：** `ladd_test.rs` 頂端需要 `use nlang_interpreter::value::MasaRef;`（確認已有）。`ContentHash::default()` — 若 `ContentHash` 未實作 `Default`，改用 `ContentHash::parse("hash:sha256:v1:0000000000000000000000000000000000000000000000000000000000000000").unwrap()`。

---

## 任務三：`~%Reflection` 擴充

### 背景

`~%Reflection` 目前有 `/keys`, `/has`, `/is_cocoon`, `/type_of`。Phase 9 加入的 `Value::Blur` 在 `/type_of` 中回傳 "unknown"（遺漏）。Phase 9-15 引入的 @option/@result/@blur 都缺乏直接的 predicate 反射。

### 新增 Builtin：`crates/interpreter/src/builtins/reflection.rs`

在 `register_reflection_builtins` 函數末尾加入：

```rust
// ── Phase 16: predicates + to_str + bottom_cause ─────────────

m.insert("refl.is_blur".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
    let fv = oo.force(v, ctx);
    let is = matches!(fv.collapse(), Value::Blur(_));
    Value::Atom(AtomKind::Tag(if is { "true" } else { "false" }.to_string()), EffectTag::Pure, None)
}) as Arc<BuiltinFn>);

m.insert("refl.is_bottom".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
    let fv = oo.force(v, ctx);
    let is = matches!(fv.collapse(), Value::Bottom(_));
    Value::Atom(AtomKind::Tag(if is { "true" } else { "false" }.to_string()), EffectTag::Pure, None)
}) as Arc<BuiltinFn>);

m.insert("refl.is_some".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
    let fv = oo.force(v, ctx);
    let is = if let Value::Combo(ref cv) = fv.collapse() { cv.get_field("%val").is_some() } else { false };
    Value::Atom(AtomKind::Tag(if is { "true" } else { "false" }.to_string()), EffectTag::Pure, None)
}) as Arc<BuiltinFn>);

m.insert("refl.is_none".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
    let fv = oo.force(v, ctx);
    let is = matches!(fv.collapse(), Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none");
    Value::Atom(AtomKind::Tag(if is { "true" } else { "false" }.to_string()), EffectTag::Pure, None)
}) as Arc<BuiltinFn>);

m.insert("refl.is_ok".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
    let fv = oo.force(v, ctx);
    let is = if let Value::Combo(ref cv) = fv.collapse() {
        cv.get_field("%val").is_some() && cv.get_field("%cause").is_none()
    } else { false };
    Value::Atom(AtomKind::Tag(if is { "true" } else { "false" }.to_string()), EffectTag::Pure, None)
}) as Arc<BuiltinFn>);

m.insert("refl.is_err".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
    let fv = oo.force(v, ctx);
    let is = if let Value::Combo(ref cv) = fv.collapse() { cv.get_field("%cause").is_some() } else { false };
    Value::Atom(AtomKind::Tag(if is { "true" } else { "false" }.to_string()), EffectTag::Pure, None)
}) as Arc<BuiltinFn>);

m.insert("refl.to_str".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
    let fv = oo.force(v, ctx);
    Value::Atom(AtomKind::Str(fv.collapse().to_string_plain()), EffectTag::Pure, None)
}) as Arc<BuiltinFn>);

m.insert("refl.bottom_cause".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
    let fv = oo.force(v, ctx);
    if let Value::Bottom(ref bd) = fv.collapse() {
        Value::Atom(AtomKind::Tag(bd.cause.as_tag().to_string()), EffectTag::Pure, None)
    } else {
        Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None)
    }
}) as Arc<BuiltinFn>);
```

**`bd.cause.as_tag()` 確認**：`BottomCause` 在 `value.rs` 中有 `fn as_tag(&self) -> &str`（回傳 `"#conflict"` 等）。若方法名不同，改用：
```rust
Value::Atom(AtomKind::Tag(format!("{}", bd.cause.as_tag())), EffectTag::Pure, None)
```

### 修正 `refl.type_of`（同檔案）

找到 `refl.type_of` 的 match arm：
```rust
Value::Combo(_) => ...
Value::Union(_) => "union",
_ => "unknown",
```

在 `Value::Bottom(_)` 和 `Value::Union(_)` 之間加入：
```rust
Value::Blur(_) => "blur",
```

**完整修正後的 tag 列表**：
```rust
Value::Top          => "top",
Value::Bottom(_)    => "bottom",
Value::Blur(_)      => "blur",          // 新增
Value::Atom(...)    => match kind { ... },
Value::Combo(c)     => ...,
Value::Union(_)     => "union",
_                   => "unknown",
```

### 擴充 `~%Reflection` 在 `root_with_system()`：`crates/interpreter/src/lib.rs`

找到（約 195 行）：
```rust
let refl_morphisms = vec![("/keys", "refl.keys"), ("/has", "refl.has"), ("/is_cocoon", "refl.is_cocoon"), ("/type_of", "refl.type_of")];
```

替換為：
```rust
let refl_morphisms = vec![
    ("/keys",         "refl.keys"),
    ("/has",          "refl.has"),
    ("/is_cocoon",    "refl.is_cocoon"),
    ("/type_of",      "refl.type_of"),
    ("/is_blur",      "refl.is_blur"),
    ("/is_bottom",    "refl.is_bottom"),
    ("/is_some",      "refl.is_some"),
    ("/is_none",      "refl.is_none"),
    ("/is_ok",        "refl.is_ok"),
    ("/is_err",       "refl.is_err"),
    ("/to_str",       "refl.to_str"),
    ("/bottom_cause", "refl.bottom_cause"),
];
```

### 更新 Genesis Seed

加入新態射後 `~%Reflection` 的 CAID 改變。執行：
```bash
cargo test -p nlang-interpreter seed_caids_are_stable -- --nocapture 2>&1 | grep "UPDATE:"
```

取得新的 `SEED_REFL` 值，更新 `crates/interpreter/src/genesis.rs`。

### 測試：新建 `crates/interpreter/tests/refl_ext_test.rs`

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, ComboVal, EffectTag, BlurDetail, BlurCause, HorizonParams, ObservationStrategy};
use nlang_parser::ast::AtomKind;
use indexmap::IndexMap;

fn get_refl_morph(name: &str, oo: &Ouroboros) -> Value {
    let root = oo.root_with_system();
    let refl = root.get_field("~%Reflection").expect("~%Reflection exists");
    if let Value::Combo(ref c) = refl {
        c.get_field(name).cloned().expect(&format!("{} exists", name))
    } else { panic!("~%Reflection is not a Combo") }
}

fn apply_refl(morph_name: &str, val: Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Value {
    let mut arg_fields = IndexMap::new();
    arg_fields.insert("0".to_string(), val);
    let arg = Value::Combo(ComboVal::new(arg_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
    let morph = get_refl_morph(morph_name, oo);
    oo.force(oo.apply_morphism(morph, arg, ctx), ctx)
}

fn is_true(v: &Value) -> bool {
    matches!(v.collapse(), Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "true")
}
fn is_false(v: &Value) -> bool {
    matches!(v.collapse(), Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "false")
}

#[test]
fn refl_is_blur_on_blur() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let blur = Value::Blur(BlurDetail {
        cause: BlurCause::FuelExhausted,
        horizon: HorizonParams { fuel_remaining: 0, strategy: ObservationStrategy::Blur, salt: Default::default() },
        partial: None,
        effect: EffectTag::Pure,
    });
    let result = apply_refl("/is_blur", blur, &oo, &mut ctx);
    assert!(is_true(&result), "is_blur(Blur) should be #true: {:?}", result);
}

#[test]
fn refl_is_blur_on_non_blur() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let result = apply_refl("/is_blur", Value::Top, &oo, &mut ctx);
    assert!(is_false(&result), "is_blur(Top) should be #false: {:?}", result);
}

#[test]
fn refl_is_bottom_on_bottom() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let bottom: Value = nlang_interpreter::value::BottomCause::Conflict.into();
    let result = apply_refl("/is_bottom", bottom, &oo, &mut ctx);
    assert!(is_true(&result), "is_bottom(Bottom) should be #true: {:?}", result);
}

#[test]
fn refl_is_some_and_is_none() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());

    // Some(42) = { %val: 42 }
    let mut some_fields = IndexMap::new();
    some_fields.insert("%val".to_string(), Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None));
    let some_val = Value::Combo(ComboVal::new(some_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));

    let none_val = Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);

    assert!(is_true(&apply_refl("/is_some", some_val.clone(), &oo, &mut ctx)), "is_some(Some) = #true");
    assert!(is_false(&apply_refl("/is_none", some_val, &oo, &mut ctx)), "is_none(Some) = #false");
    assert!(is_false(&apply_refl("/is_some", none_val.clone(), &oo, &mut ctx)), "is_some(None) = #false");
    assert!(is_true(&apply_refl("/is_none", none_val, &oo, &mut ctx)), "is_none(None) = #true");
}

#[test]
fn refl_is_ok_and_is_err() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());

    let mut ok_fields = IndexMap::new();
    ok_fields.insert("%val".to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None));
    let ok_val = Value::Combo(ComboVal::new(ok_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));

    let mut err_fields = IndexMap::new();
    err_fields.insert("%cause".to_string(), Value::Atom(AtomKind::Tag("fail".to_string()), EffectTag::Pure, None));
    let err_val = Value::Combo(ComboVal::new(err_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));

    assert!(is_true(&apply_refl("/is_ok", ok_val.clone(), &oo, &mut ctx)), "is_ok(Ok) = #true");
    assert!(is_false(&apply_refl("/is_err", ok_val, &oo, &mut ctx)), "is_err(Ok) = #false");
    assert!(is_false(&apply_refl("/is_ok", err_val.clone(), &oo, &mut ctx)), "is_ok(Err) = #false");
    assert!(is_true(&apply_refl("/is_err", err_val, &oo, &mut ctx)), "is_err(Err) = #true");
}

#[test]
fn refl_to_str() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let val = Value::Atom(AtomKind::Int(42.into()), EffectTag::Pure, None);
    let result = apply_refl("/to_str", val, &oo, &mut ctx);
    if let Value::Atom(AtomKind::Str(s), _, _) = result.collapse() {
        assert!(s.contains("42"), "to_str(42) should contain '42': {}", s);
    } else {
        panic!("Expected Str, got {:?}", result);
    }
}

#[test]
fn refl_bottom_cause_on_bottom() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let bottom: Value = nlang_interpreter::value::BottomCause::FuelExhausted.into();
    let result = apply_refl("/bottom_cause", bottom, &oo, &mut ctx);
    if let Value::Atom(AtomKind::Tag(t), _, _) = result.collapse() {
        assert!(t.contains("fuel"), "bottom_cause(FuelExhausted) should contain 'fuel': {}", t);
    } else {
        panic!("Expected Tag, got {:?}", result);
    }
}

#[test]
fn refl_bottom_cause_on_non_bottom() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let result = apply_refl("/bottom_cause", Value::Top, &oo, &mut ctx);
    if let Value::Atom(AtomKind::Tag(t), _, _) = result.collapse() {
        assert_eq!(t.trim_start_matches('#'), "none",
            "bottom_cause(non-Bottom) should return #none: {}", t);
    } else {
        panic!("Expected #none, got {:?}", result);
    }
}

#[test]
fn refl_type_of_blur() {
    let oo = Ouroboros::new_in_memory();
    let mut ctx = EvalContext::new(oo.root_with_system());
    let blur = Value::Blur(BlurDetail {
        cause: BlurCause::Timeout,
        horizon: HorizonParams { fuel_remaining: 100, strategy: ObservationStrategy::Blur, salt: Default::default() },
        partial: None,
        effect: EffectTag::Pure,
    });
    let result = apply_refl("/type_of", blur, &oo, &mut ctx);
    if let Value::Atom(AtomKind::Tag(t), _, _) = result.collapse() {
        assert_eq!(t.trim_start_matches('#'), "blur",
            "type_of(Blur) should return #blur: {}", t);
    } else {
        panic!("Expected #blur tag, got {:?}", result);
    }
}
```

**`use` 清單注意**：`HorizonParams` 的 `salt` 欄位型別為 `ContentHash`，若 `ContentHash` 未實作 `Default`，改用：
```rust
salt: nlang_interpreter::value::ContentHash::parse(
    "hash:sha256:v1:0000000000000000000000000000000000000000000000000000000000000000"
).unwrap()
```

---

## 驗收條件

1. `cargo test -p nlang-interpreter 2>&1 | grep -E "FAILED|passed"` — 全部通過
2. `option_and_then_some_chains` / `option_and_then_none_propagates` / `result_and_then_ok_chains` / `result_and_then_err_propagates` ✓
3. `test_d_l_approx_cosine_different`：`d ≈ 0.5`（[1,0,0,0] ⊥ [0,1,0,0]）✓
4. `test_d_l_approx_identical_still_zero`：不破壞現有合約 ✓
5. 所有 `refl_*` 測試通過（9 個）✓
6. `seed_caids_are_stable` 通過（SEED_REFL 已更新）✓
7. `cargo clippy -p nlang-interpreter -- -D warnings` — 無警告

---

## 不在本 Phase 的工作

- **`approximate_phase_diff` in `unify.rs`** — 改這個會讓 H¹Split 在既有測試中觸發，需獨立分析；留後
- **`@list { %fmap }`** — 加入 ~%List 結構會改 SEED_LIST，留下一 Phase 與 `list.flat_map` 一起處理
- **`list.flat_map`** — Monad bind for lists，留後
- **`option.or` / `option.unwrap_or`** — 常用 combinator，留後
