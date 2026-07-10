# Phase NEW 交接文件：%obstruction_degree + %cause Cocycle + /%differential

> **執行者**：引擎開發 Agent  
> **預估工作量**：1 週  
> **前置條件**：Phase 1b 完成（`BottomCause::H1Split`、`H2Split` 已存在；`make_h1_split_bottom()`、`make_h2_split_bottom()` 已在 `unify.rs`）  
> **完成判斷**：通過本文末尾的驗收測試清單  
> **排序理由**：LADD（Phase 4）的 MASA 過濾邏輯需要區分 H¹/H² 障礙；Phase NEW 必須在 Phase 4 之前完成

---

## 背景

Phase 1b 加入了 `H1Split`/`H2Split` 的架構骨架，但 `%cause` 的輸出格式還是簡單的 `%type + %message`。SPEC_06 §1.3.2 定義了完整的**上鏈格式 (Cocycle Format)**：

```nlang
%cause: {
  %degree:      <int>               ;; 上同調維度：1 (H¹) / 2 (H²) / 3 (H³)
  %obstruction: #h1_phase | #h2_sign | #h3_gerbe | #h4_sybil
  %cocycle:     [<masa_1>, <masa_2>, ...]  ;; 形成障礙的 MASA 序列
  %holonomy:    <phase_rad> | #neg_I      ;; H¹ 連續相位 / H² 符號翻轉
  %branches:    <int>               ;; 衝突分支數（H² / H⁴）
}
```

同時，SPEC_07 §2 要求引擎在 `~%Engine` 中暴露 `/%differential.{1,2,3}` 作為收斂狀態的可觀測入口。

Phase NEW 包含三個子任務：
1. **擴充 `BottomDetail`**：加入 cocycle 結構欄位
2. **升級 `as_cause_combo()`**：輸出完整 SPEC_06 §1.3.2 格式
3. **加入 `/%differential.{1,2,3}`**：在 `~%Engine` 中暴露收斂觀測接口

---

## 規格書參考

| 任務 | 規格 |
|:-----|:-----|
| Cocycle 格式 | `SPEC_06 §1.3.2`（上鏈格式） |
| 障礙度標記 | `SPEC_06 §1.3.1`（Obstruction Degree） |
| Differential 態射 | `SPEC_07 §2`（微分態射） |
| `%cause` 對偶性語義 | `SPEC_08 §2.2` |

規格書位置：`nlang-spec/spec/zh_TW/`

---

## 子任務一：擴充 `BottomDetail`

### 現有狀態（`value.rs:250`）

```rust
pub struct BottomDetail {
    pub cause: BottomCause,
    pub path: Option<String>,
    pub message: Option<String>,
    pub expected: Option<Value>,
    pub found: Option<Value>,
    pub involved: Vec<ContentHash>,   // 已存放相關 CAID
}
```

### 需要新增的欄位

```rust
pub struct BottomDetail {
    pub cause: BottomCause,
    pub path: Option<String>,
    pub message: Option<String>,
    pub expected: Option<Value>,
    pub found: Option<Value>,
    pub involved: Vec<ContentHash>,
    // Phase NEW 新增：
    pub obstruction_degree: Option<u8>,   // 1=H¹, 2=H², 3=H³, 4=H⁴
    pub holonomy: Option<Holonomy>,        // H¹ 連續相位 / H² 符號翻轉
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Holonomy {
    Phase(f64),  // H¹：θ 弧度（連續相位，來自 approximate_phase_diff）
    NegI,        // H²：-I（Z₂ 符號翻轉，Kochen-Specker 特徵）
}
```

`cocycle`（MASA 序列）直接複用現有 `involved: Vec<ContentHash>`，不需要新欄位。

### 升級 `bits()` 計算

在 `BottomDetail::bits()` 中加入新欄位的燃料估算：
```rust
if self.obstruction_degree.is_some() { b += 64; }
if self.holonomy.is_some() { b += 64; }
```

---

## 子任務二：升級 `as_cause_combo()`

### 目標輸出格式（SPEC_06 §1.3.2）

**H¹ 相位障礙**：
```nlang
%cause: {
  %degree:      1
  %obstruction: #h1_phase
  %cocycle:     ["hash:sha256:v2:...", "hash:sha256:v2:..."]
  %holonomy:    0.2341   ;; θ 弧度
}
```

**H² 符號障礙**：
```nlang
%cause: {
  %degree:      2
  %obstruction: #h2_sign
  %cocycle:     ["hash:sha256:v2:...", "hash:sha256:v2:...", "_", "_"]
  %holonomy:    #neg_I
  %branches:    2
}
```

### 實作修改（`value.rs:268`）

在 `as_cause_combo()` 中，**在現有的 `%type`/`%message` 輸出之後**，加入 cocycle 結構：

```rust
pub fn as_cause_combo(&self) -> Value {
    let mut fields = IndexMap::new();

    // 現有邏輯：%type, %path, %message, %expected, %found, %involved
    // ...（保留不動）

    // Phase NEW 新增：cocycle 結構欄位
    if let Some(degree) = self.obstruction_degree {
        fields.insert("%degree".to_string(),
            Value::Atom(AtomKind::Int(BigInt::from(degree)), EffectTag::Pure, None));

        let obstruction_tag = match degree {
            1 => "h1_phase",
            2 => "h2_sign",
            3 => "h3_gerbe",
            4 => "h4_sybil",
            _ => "unknown",
        };
        fields.insert("%obstruction".to_string(),
            Value::Atom(AtomKind::Tag(obstruction_tag.to_string()), EffectTag::Pure, None));
    }

    // %cocycle：從 involved 序列化為 List Combo
    if !self.involved.is_empty() {
        let mut cocycle_fields = IndexMap::new();
        cocycle_fields.insert("%kind".to_string(),
            Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
        for (i, h) in self.involved.iter().enumerate() {
            cocycle_fields.insert(i.to_string(),
                Value::Atom(AtomKind::Str(h.to_string()), EffectTag::Pure, None));
        }
        // H²：補齊 4 個 MASA 位置（規格要求四重循環）
        if self.obstruction_degree == Some(2) && self.involved.len() == 2 {
            cocycle_fields.insert("2".to_string(),
                Value::Atom(AtomKind::Tag("_".to_string()), EffectTag::Pure, None));
            cocycle_fields.insert("3".to_string(),
                Value::Atom(AtomKind::Tag("_".to_string()), EffectTag::Pure, None));
        }
        fields.insert("%cocycle".to_string(),
            Value::Combo(ComboVal::new(cocycle_fields, false, IndexMap::new(), EffectTag::Pure, vec![])));
    }

    // %holonomy：H¹ 相位 or H² -I
    if let Some(ref h) = self.holonomy {
        let holonomy_val = match h {
            Holonomy::Phase(theta) =>
                Value::Atom(AtomKind::Float(*theta), EffectTag::Pure, None),
            Holonomy::NegI =>
                Value::Atom(AtomKind::Tag("neg_I".to_string()), EffectTag::Pure, None),
        };
        fields.insert("%holonomy".to_string(), holonomy_val);
    }

    // %branches：H² 分支數
    if self.obstruction_degree == Some(2) {
        fields.insert("%branches".to_string(),
            Value::Atom(AtomKind::Int(BigInt::from(2u8)), EffectTag::Pure, None));
    }

    Value::Combo(ComboVal::new(fields, true, IndexMap::new(), EffectTag::Pure, vec![]))
}
```

---

## 子任務三：更新 `make_h1/h2_split_bottom()`

**位置**：`crates/interpreter/src/unify.rs`

Phase 1b 的兩個函式需要填入新欄位：

```rust
fn make_h1_split_bottom(a: &ComboVal, b: &ComboVal, theta: f64) -> Value {
    Value::Bottom(Box::new(BottomDetail {
        cause: BottomCause::H1Split,
        path: None,
        message: Some(format!("H¹ phase: θ={:.4} rad", theta)),
        expected: None,
        found: None,
        involved: vec![
            Value::Combo(a.clone()).content_hash(),
            Value::Combo(b.clone()).content_hash(),
        ],
        // Phase NEW 新增：
        obstruction_degree: Some(1),
        holonomy: Some(Holonomy::Phase(theta)),
    }))
}

fn make_h2_split_bottom(a: &ComboVal, b: &ComboVal) -> Value {
    Value::Bottom(Box::new(BottomDetail {
        cause: BottomCause::H2Split,
        path: None,
        message: Some(format!("H² MASA: {} vs {}", a.masa_ref, b.masa_ref)),
        expected: None,
        found: None,
        involved: vec![
            Value::Combo(a.clone()).content_hash(),
            Value::Combo(b.clone()).content_hash(),
        ],
        // Phase NEW 新增：
        obstruction_degree: Some(2),
        holonomy: Some(Holonomy::NegI),
    }))
}
```

同時需要更新 **所有其他建立 `BottomDetail` 的地方**（`unify.rs`、`eval.rs`、`dispatch.rs` 等），讓新欄位使用 `None` 預設值（非 H1/H2 障礙不填）：

```rust
// 所有舊有的 BottomDetail 建構，補上兩個 None：
Value::Bottom(Box::new(BottomDetail {
    cause: BottomCause::Conflict,
    path: ...,
    message: ...,
    expected: ...,
    found: ...,
    involved: ...,
    obstruction_degree: None,  // 新增
    holonomy: None,            // 新增
}))
```

> **提示**：`cargo build` 會把所有缺少新欄位的 `BottomDetail { ... }` 標成編譯錯誤，逐一補 `None` 即可。

---

## 子任務四：加入 `/%differential.{1,2,3}` 到 `~%Engine`

### 設計說明（SPEC_07 §2）

| 態射 | 對應 $d_r$ | 回傳值 | 工程語義 |
|:-----|:---:|:---:|:---------|
| `/%differential.1` | $d_1$ | `#d1_converging` | 正常 meet 收斂，無障礙 |
| `/%differential.2` | $d_2$ | `#d2_branching` | H² 衝突，保留聯集分支 |
| `/%differential.3` | $d_3$ | `#d3_horizon` | H³ 障礙，請求更多 `%fuel` |

這些態射由引擎**自動追蹤**，不由使用者顯式呼叫。Phase NEW 的實作目標是：
1. 讓 `~%Engine` 有 `/%differential.1`、`/%differential.2`、`/%differential.3` 三個態射
2. 呼叫時，根據傳入的 `%cause` 結構回傳對應的狀態標籤
3. `~%Engine./state.differential` 路徑可用（回傳當前最後一次收斂的 differential 等級）

### 實作：新建 `builtins/engine.rs` 中加入 differential 判斷

**位置**：`crates/interpreter/src/builtins/engine.rs`（現有檔案）

加入 `engine.differential` builtin：

```rust
m.insert("engine.differential".to_string(), Arc::new(|arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| {
    // arg 是查詢的 differential 等級（1/2/3）或一個 %cause Combo
    match &arg {
        Value::Atom(AtomKind::Int(n), _, _) => {
            match n.to_u8().unwrap_or(0) {
                1 => Value::Atom(AtomKind::Tag("d1_converging".to_string()), EffectTag::Pure, None),
                2 => Value::Atom(AtomKind::Tag("d2_branching".to_string()), EffectTag::Pure, None),
                3 => Value::Atom(AtomKind::Tag("d3_horizon".to_string()), EffectTag::Pure, None),
                _ => Value::Atom(AtomKind::Tag("unknown".to_string()), EffectTag::Pure, None),
            }
        }
        // 傳入 %cause Combo → 解析 %degree 欄位
        Value::Combo(ref c) => {
            if let Some(Value::Atom(AtomKind::Int(d), _, _)) = c.get_field("%degree") {
                match d.to_u8().unwrap_or(0) {
                    1 => Value::Atom(AtomKind::Tag("d1_converging".to_string()), EffectTag::Pure, None),
                    2 => Value::Atom(AtomKind::Tag("d2_branching".to_string()), EffectTag::Pure, None),
                    3 => Value::Atom(AtomKind::Tag("d3_horizon".to_string()), EffectTag::Pure, None),
                    _ => Value::Atom(AtomKind::Tag("d1_converging".to_string()), EffectTag::Pure, None),
                }
            } else {
                Value::Atom(AtomKind::Tag("d1_converging".to_string()), EffectTag::Pure, None)
            }
        }
        _ => Value::Atom(AtomKind::Tag("d1_converging".to_string()), EffectTag::Pure, None),
    }
}) as Arc<BuiltinFn>);
```

### 在 `root_with_system()` 中注冊

**位置**：`lib.rs`，目前 `~%Engine` 不存在（discovery 和 engine morphisms 都在 `~%Discovery` 和根層）。

新建一個 `~%Engine` Combo（或在現有 `~%Discovery` 之後加入），加入 differential 態射：

```rust
let mut engine_fields = IndexMap::new();
let engine_morphisms = vec![
    ("/observe", "engine.observe"),
    ("/save",    "engine.save"),
];
for (n, b) in engine_morphisms {
    engine_fields.insert(n.to_string(), make_builtin_morph(b, EffectTag::IO));
}

// Phase NEW：differential 態射族
for i in 1u8..=3 {
    let name = format!("/%differential.{}", i);
    engine_fields.insert(name, make_builtin_morph("engine.differential", EffectTag::Pure));
}

// state.differential：當前 differential 狀態的捷徑
engine_fields.insert("state".to_string(), Value::Combo({
    let mut s = IndexMap::new();
    s.insert("differential".to_string(),
        Value::Atom(AtomKind::Tag("d1_converging".to_string()), EffectTag::Pure, None));
    ComboVal::new(s, false, IndexMap::new(), EffectTag::Pure, vec![])
}));

fields.insert("~%Engine".to_string(),
    Value::Combo(ComboVal::new(engine_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
```

> **注意**：`make_builtin_morph(b, effect)` 是建立 builtin 態射 Combo 的輔助函式，
> 可以提取現有 `root_with_system()` 中重複的建構模式為一個函式：
> ```rust
> fn make_builtin_morph(builtin_key: &str, effect: EffectTag) -> Value {
>     Value::Combo(ComboVal::new(
>         IndexMap::from_iter(vec![
>             ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
>             ("%builtin".to_string(), Value::Atom(AtomKind::Str(builtin_key.to_string()), EffectTag::Pure, None)),
>         ]),
>         true, IndexMap::new(), effect, vec![]
>     ))
> }
> ```
> 如果覺得重構 `root_with_system()` 範圍太大，也可以直接 inline，不阻斷驗收。

---

## 修改檔案清單

| 檔案 | 動作 | 說明 |
|:-----|:-----|:-----|
| `crates/interpreter/src/value.rs` | **修改** | `BottomDetail` 加 `obstruction_degree`、`holonomy`；新增 `Holonomy` enum；升級 `as_cause_combo()` |
| `crates/interpreter/src/unify.rs` | **修改** | `make_h1_split_bottom()`、`make_h2_split_bottom()` 填入新欄位；所有舊 `BottomDetail` 補 `None` |
| `crates/interpreter/src/builtins/engine.rs` | **修改** | 加入 `engine.differential` builtin |
| `crates/interpreter/src/lib.rs` | **修改** | 建立 `~%Engine` Combo；加入 `/%differential.{1,2,3}`；選做：提取 `make_builtin_morph()` |

---

## 驗收測試清單

### `%cause` Cocycle 格式

- [ ] `BottomDetail` 有 `obstruction_degree: Option<u8>` 和 `holonomy: Option<Holonomy>`
- [ ] H¹ Bottom 的 `as_cause_combo()` 輸出含 `%degree: 1`、`%obstruction: #h1_phase`、`%holonomy: <float>`
- [ ] H² Bottom 的 `as_cause_combo()` 輸出含 `%degree: 2`、`%obstruction: #h2_sign`、`%holonomy: #neg_I`、`%branches: 2`
- [ ] H² 的 `%cocycle` List 長度為 4（前 2 項為真實 CAID，後 2 項為 `#_`）

### `/%differential.{1,2,3}`

- [ ] `~%Engine` 存在於 `root_with_system()` 輸出中
- [ ] `~%Engine./%differential.1` 呼叫回傳 `#d1_converging`
- [ ] `~%Engine./%differential.2` 呼叫回傳 `#d2_branching`
- [ ] `~%Engine./%differential.3` 呼叫回傳 `#d3_horizon`
- [ ] `~%Engine.state.differential` 路徑可讀取

### 回歸測試

- [ ] `cargo build` 無錯誤（所有舊 `BottomDetail` 補好 `None`）
- [ ] `cargo test` 全部通過（73 tests，0 failed）

---

## 不在 Phase NEW 範圍內

| 項目 | 延後至 |
|:-----|:------|
| `state.differential` 動態追蹤（跟隨最近一次 unify 的結果更新） | Phase 4 |
| H³ / H⁴ 障礙的真實觸發邏輯 | Phase 4 / LADD |
| `%cause` 的惰性計算（Lazy evaluation，僅在查詢時才回溯） | SPEC_08 §2.2，Phase 6 |
| `/%differential.2` 在 H² 時自動保留聯集分支（§1.7 非嚴格性）| Phase 4 |
| `cocycle` 的 4 個真實 MASA CAID（目前後兩項為 `#_`） | Phase 4（MASA 基礎設施） |

---

## 快速定位

```bash
# 確認 BottomDetail 結構
grep -n "pub struct BottomDetail" crates/interpreter/src/value.rs

# 確認 as_cause_combo 位置
grep -n "fn as_cause_combo" crates/interpreter/src/value.rs

# 確認 make_h1/h2_split_bottom 位置
grep -n "fn make_h[12]_split_bottom" crates/interpreter/src/unify.rs

# 確認所有 BottomDetail 建構（需補 None 的地方）
grep -rn "BottomDetail {" crates/interpreter/src/

# 編譯（錯誤清單即待修位置）
cargo build -p nlang-interpreter 2>&1 | grep "error\[E"
```
