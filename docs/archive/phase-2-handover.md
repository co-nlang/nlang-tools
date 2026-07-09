# Phase 2 交接文件：StdLib 完善

> **執行者**：引擎開發 Agent  
> **預估工作量**：1–2 週  
> **前置條件**：Phase 1a 完成（複數 `AtomKind::Complex` 可用）；Phase 1b、1c 不阻斷本 Phase  
> **完成判斷**：通過本文末尾的驗收測試清單

---

## 背景

Phase 2 完善標準庫的兩個面向：

1. **EML 派生函數**：加入 `/eml`、`/exp`、`/ln`、`/sin`、`/cos` 到 `~%Math`，含奇異點處理
2. **創世預設值**：讓 `EvalContext` 攜帶 SPEC_09 §6 規定的全域參數，確保跨引擎 CAID 決定論

兩個子任務相互獨立，可並行或依序完成。

---

## 規格書參考

| 任務 | 規格 |
|:-----|:-----|
| EML 核心算子定義 | `SPEC_09 §3.1` |
| 奇異點與分支切割 | `SPEC_09 §3.1`（表格） |
| 派生函數清單 | `SPEC_09 §3.2`、`§3.4` |
| 創世預設值表格 | `SPEC_09 §6` |
| `#blur` 狀態語義 | `SPEC_06 §1.3` |

規格書位置：`nlang-spec/spec/zh_TW/`

---

## 子任務 A：EML 派生函數

### A1. 現有狀況

`lib.rs` 的 `root_with_system()` 目前 `~%Math` 已有：
`/add`, `/sub`, `/mul`, `/div`, `/rem`, `/abs`, `/bits`, `/pow`, `/sqrt`,
`/bitAnd`, `/bitOr`, `/bitXor`, `/bitNot`, `/shl`, `/shr`, `/random`

**缺少**：`/eml`, `/exp`, `/ln`, `/sin`, `/cos`，以及規格要求的 `one: 1` 常數。

### A2. 需要新增的態射

| 態射 | 定義 | 回傳型別 | 奇異點 |
|:-----|:-----|:--------:|:------:|
| `/eml` | `exp(x) - ln(y)` | `@num`（複數域） | `y == 0` → `#blur` |
| `/exp` | $e^x$ | `@num` | 無 |
| `/ln`  | $\ln(x)$（主分支） | `@num`（虛部非零時保留複數） | `x == 0` → `#blur` |
| `/sin` | $\sin(x)$（弧度） | `@num` | 無 |
| `/cos` | $\cos(x)$（弧度） | `@num` | 無 |

以及在 `~%Math` Combo 加入欄位：`one: 1`（`SPEC_09 §3.1`）。

### A3. 實作位置

**`crates/interpreter/src/builtins/math.rs`**：在現有 `register_math_builtins()` 中加入新函式。

```rust
// /ln 實作示意
m.insert("math.ln".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let v = oo.force(arg, ctx).collapse().clone();
    match v {
        // ln(0) → #blur
        Value::Atom(AtomKind::Int(ref n), _, _) if n.is_zero() => blur_singularity("#log_singularity"),
        Value::Atom(AtomKind::Float(f), _, _) if f == 0.0       => blur_singularity("#log_singularity"),
        Value::Atom(AtomKind::Complex(0.0, 0.0), _, _)           => blur_singularity("#log_singularity"),
        
        // ln(負實數) → 複數結果（主分支）
        Value::Atom(AtomKind::Float(f), e, _) if f < 0.0 => {
            // ln(-|f|) = ln(|f|) + iπ  (主分支 arg ∈ (-π, π])
            Value::Atom(AtomKind::Complex(f.abs().ln(), std::f64::consts::PI), e, None)
        }
        
        // ln(正實數)
        Value::Atom(AtomKind::Float(f), e, _) => Value::Atom(AtomKind::Float(f.ln()), e, None),
        Value::Atom(AtomKind::Int(n), e, _) => {
            let f = n.to_f64().unwrap_or(f64::NAN);
            if f > 0.0 { Value::Atom(AtomKind::Float(f.ln()), e, None) }
            else if f < 0.0 { Value::Atom(AtomKind::Complex(f.abs().ln(), std::f64::consts::PI), e, None) }
            else { blur_singularity("#log_singularity") }
        }
        
        // ln(複數 r·e^{iθ}) = ln(r) + iθ  （r = |z|, θ = arg(z)，主分支）
        Value::Atom(AtomKind::Complex(re, im), e, _) => {
            let r = (re * re + im * im).sqrt();
            if r == 0.0 { return blur_singularity("#log_singularity"); }
            let theta = im.atan2(re);
            Value::Atom(AtomKind::Complex(r.ln(), theta), e, None)
        }
        
        _ => BottomCause::Conflict.into()
    }
}) as Arc<BuiltinFn>);
```

**`blur_singularity` 輔助函式**（放在 `math.rs` 頂部）：

```rust
fn blur_singularity(cause_tag: &str) -> Value {
    use crate::value::{BottomDetail, BottomCause};
    // 回傳 Bottom(Conflict)，message 帶有奇異點標籤
    // 未來 Phase NEW 換成真正的 #blur Value 後再重構
    Value::Bottom(Box::new(BottomDetail {
        cause: BottomCause::NumericalError,
        path: None,
        message: Some(cause_tag.to_string()),
        expected: None,
        found: None,
        involved: vec![],
    }))
}
```

> **注意**：SPEC_09 §3.1 說奇異點應回傳 `#blur`（`Value::Top` 帶 `%cause`），
> 但 `#blur` 的完整語義（`Value` 層的 `#blur` 狀態）是 Phase 3 的工作。
> Phase 2 先用 `Bottom(NumericalError)` 帶 cause 標籤暫代，Phase 3 替換。

### A4. `/eml` 實作

```rust
m.insert("math.eml".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    if let Value::Combo(ref c) = arg {
        if let (Some(vx), Some(vy)) = (c.get_field("0"), c.get_field("1")) {
            let x = oo.force(vx.clone(), ctx).collapse().clone();
            let y = oo.force(vy.clone(), ctx).collapse().clone();
            
            // eml(x, y) = exp(x) - ln(y)
            // 先計算 exp(x)，再計算 ln(y)，最後相減
            // 複用已有的 math.exp 和 math.ln 邏輯（或直接展開）
            let exp_x = compute_exp(&x);
            let ln_y  = compute_ln(&y);
            match (exp_x, ln_y) {
                (Some(ex), Some(ly)) => compute_sub(ex, ly),   // 複用 /sub 邏輯
                _ => blur_singularity("#eml_singularity"),
            }
        } else { Value::Top }
    } else { Value::Top }
}) as Arc<BuiltinFn>);
```

`compute_exp`、`compute_ln` 是把 `/exp`、`/ln` 邏輯提取為內部函式，避免重複。

### A5. 在 `root_with_system()` 中注冊新態射

**位置**：`lib.rs:118`，`math_morphisms` 向量中加入：

```rust
let math_morphisms = vec![
    ("/sub", "math.sub"), ("/mul", "math.mul"), ("/div", "math.div"),
    ("/rem", "math.rem"), ("/abs", "math.abs"), ("/bits", "math.bits"),
    ("/pow", "math.pow"), ("/sqrt", "math.sqrt"),
    ("/bitAnd", "math.bitAnd"), ("/bitOr", "math.bitOr"), ("/bitXor", "math.bitXor"),
    ("/bitNot", "math.bitNot"), ("/shl", "math.shl"), ("/shr", "math.shr"),
    // Phase 2 新增：
    ("/eml", "math.eml"),
    ("/exp", "math.exp"),
    ("/ln",  "math.ln"),
    ("/sin", "math.sin"),
    ("/cos", "math.cos"),
];
```

同時在建立 `math_builtins` 後加入 `one: 1` 常數：

```rust
math_builtins.insert("one".to_string(),
    Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None));
```

---

## 子任務 B：創世預設值

### B1. 現有狀況

`EvalContext` 目前已有（`lib.rs:54`）：
- `fuel: 10000` ✅（對應 `%fuel`）
- `strategy: ObservationStrategy::Blur` ✅（對應 `%strategy: #blur`）

**尚未有**（`EvalContext` 缺少的欄位）：
- `max_branches: 64`（對應 `%max_branches`）
- `max_unification_depth: 256`（對應 `%max_unification_depth`）
- `max_pattern_nodes: 1024`（對應 `%max_pattern_nodes`）
- `max_lifting_depth: 32`（對應 `%max_lifting_depth`）

### B2. 修改 `EvalContext`

**位置**：`crates/interpreter/src/lib.rs`

```rust
pub struct EvalContext {
    pub root: ComboVal,
    pub scopes: Vec<ComboVal>,
    pub fuel: u64,
    pub depth: u32,
    pub horizon_salt: ContentHash,
    pub strategy: ObservationStrategy,
    // Phase 2 新增：
    pub max_branches: usize,
    pub max_unification_depth: usize,
    pub max_pattern_nodes: usize,
    pub max_lifting_depth: usize,
}
```

更新 `EvalContext::new()` 建構子加入預設值：

```rust
pub fn new(root: ComboVal) -> Self {
    // ... 現有程式碼 ...
    EvalContext {
        // ... 現有欄位 ...
        max_branches: 64,
        max_unification_depth: 256,
        max_pattern_nodes: 1024,
        max_lifting_depth: 32,
    }
}
```

### B3. 連接預設值到實際行為

加入這些欄位之後，在相應的執行路徑中使用它們：

**`max_unification_depth`** → `unify.rs`：在 `unify_internal` 的遞迴入口加深度檢查：

```rust
// 在 unify_internal 開頭：
if ctx.depth >= ctx.max_unification_depth as u32 {
    return handle_resource_exhausted(
        ResourceExhausted::FuelExhausted, ctx.strategy, &ctx.horizon_salt, None, EffectTag::Pure
    );
}
```

**`max_branches`** → `unify.rs`：Union 疊加分支數量限制（`do_unify` 的 Union 分支）：

```rust
// 在 Union 分支中：
if results.len() > ctx.max_branches {
    results.truncate(ctx.max_branches);
}
```

**`max_pattern_nodes`** → `dispatch.rs`：態射分支數量限制（`dispatch_morphism`）：目前可先加欄位但暫不強制限制，Phase 3 再接入。

### B4. `%fuel` / `%strategy` 的讀取機制（選做）

SPEC_09 §6 規定使用者可以在 Combo 中宣告 `%fuel`、`%strategy` 等來覆蓋預設。Phase 2 可以做最基本的讀取：當 `EvalContext` 初始化時，掃描 root Combo 的 `%fuel` 欄位並覆蓋預設值。這是 P2 優先度，如果時間不夠可跳過。

---

## 修改檔案清單

| 檔案 | 動作 | 說明 |
|:-----|:-----|:-----|
| `crates/interpreter/src/builtins/math.rs` | **修改** | 加入 `math.eml`、`math.exp`、`math.ln`、`math.sin`、`math.cos`；加入 `blur_singularity()` 輔助函式 |
| `crates/interpreter/src/lib.rs` | **修改** | `math_morphisms` 加入 5 個新態射；`math_builtins` 加 `one: 1`；`EvalContext` 加 4 個新欄位 |
| `crates/interpreter/src/unify.rs` | **修改** | 使用 `ctx.max_unification_depth` 和 `ctx.max_branches` |

---

## 驗收測試清單

### EML 派生函數

- [ ] `~%Math./exp` 可呼叫：`~%Math./exp 1.0` 回傳約 `2.718`
- [ ] `~%Math./ln` 可呼叫：`~%Math./ln 1.0` 回傳 `0.0`
- [ ] `~%Math./ln 0` 回傳 `Bottom(NumericalError, "#log_singularity")`
- [ ] `~%Math./ln -1` 回傳複數結果（虛部 ≈ π）
- [ ] `~%Math./sin 0` 回傳 `0.0`；`~%Math./cos 0` 回傳 `1.0`
- [ ] `~%Math./eml` 可呼叫：`~%Math./eml(1, 1)` = `exp(1) - ln(1)` ≈ `2.718`
- [ ] `~%Math./eml(1, 0)` 回傳 `Bottom(NumericalError, "#eml_singularity")`
- [ ] `~%Math` 有 `one: 1` 常數欄位

### 創世預設值

- [ ] `EvalContext::new()` 的 `max_branches` 為 `64`
- [ ] `EvalContext::new()` 的 `max_unification_depth` 為 `256`
- [ ] `EvalContext::new()` 的 `max_pattern_nodes` 為 `1024`
- [ ] `EvalContext::new()` 的 `max_lifting_depth` 為 `32`
- [ ] 現有 Union 合併不超過 64 分支（`max_branches` 有實際作用）

### 回歸測試

- [ ] `cargo test` 全部通過（68 tests，不得有新失敗）
- [ ] `cargo build` 無錯誤

---

## 不在 Phase 2 範圍內

| 項目 | 延後至 |
|:-----|:------|
| `#blur` 作為真正的 `Value` 狀態（非 Bottom 暫代） | Phase 3（#refine 機制） |
| `%branch` Riemann 面層級選擇 | Phase 3 / SPEC_09 §3.1 |
| `@option`、`@result` 型別完整定義 | Phase 3（型別系統） |
| `%fmap`、`%fold` 代數介面 | Phase 4（StdLib P2） |
| EML 的 CAID 靜態語義（結構雜湊非執行結果） | Phase 1a 已處理（BN/ 對 Thunk/Code 有處理） |
