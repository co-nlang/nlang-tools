# nlang 引擎實作狀態

> 最後更新：2026-05-24（Phase 22 完成後）  
> 測試數量：274 tests passing（35+ 個測試套件）

---

## 1. 總覽

| 規格章節 | 完整度 | 關鍵剩餘差距 |
|:---------|:------:|:------------|
| SPEC_01（格論基礎） | **95%** | ArithmeticOnAnchor 自動攔截 |
| SPEC_06（統一化邏輯） | **90%** | approximate_phase_diff（高風險，暫緩） |
| SPEC_09（標準庫） | **97%** | @list 型別定義（genesis seed） |
| SPEC_10（演化與 Commit） | **97%** | equivalence map（SPEC_17 邊界） |
| SPEC_13（OODP） | **65%** | GPP/CIP 零知識證明（P3） |
| SPEC_17（自我演化） | **0%** | N-1 自舉算法（長期目標） |

---

## 2. SPEC_01 格論基礎

### 已實作 ✓

| 功能 | 位置 |
|:-----|:-----|
| Top `_`（萬有子空間） | `value.rs:Value::Top` |
| Bottom `_\|_`（矛盾） | `value.rs:Value::Bottom(BottomDetail)` |
| Blur `#blur`（視界模糊） | `value.rs:Value::Blur(BlurDetail)` — Phase 9 |
| Meet `&`（格交）  | `unify.rs:unify_internal` |
| Join `\|`（格併） | `eval.rs` |
| Orthocomplement `!` | `complement.rs:orthocomplement` |
| Orthomodular Law 驗證 | `oml.rs`, `oml_test.rs` — Phase 7 |
| H¹ Phase Obstruction（相位干涉） | `unify.rs:make_h1_split_bottom` — Phase 7 |
| H² MASA Obstruction（互補性違規） | `unify.rs:make_h2_split_bottom` — Phase 7 |
| 序位錨點 `#_`, `#_\|_` | `value.rs:AtomKind::TagStart/TagEnd` |
| Trinity Isomorphism（Atom ↔ Combo） | `unify.rs`（Atom → `{%val: atom}`） |
| Cocoon 封閉世界（`{{}}`） | `value.rs:ComboVal.closed` |
| Blur 傳播規則 | `unify.rs`（Blur∧Top, Blur∧Bottom, Blur∧X）— Phase 9 |
| Blur orthocomplement | `complement.rs`（Blur → Blur）— Phase 9 |

### 剩餘 △

| 功能 | 說明 |
|:-----|:-----|
| `#_ + 1` → ArithmeticOnAnchor | 部分 BottomCause 存在，但未在所有算術路徑攔截 |

---

## 3. SPEC_06 統一化邏輯

### 已實作 ✓

| 規則 | 位置 |
|:-----|:-----|
| ObservationStrategy Blur/Strict/Approximate | `value.rs:ObservationStrategy` — Phase 9 |
| Blur 策略下資源耗盡 → Value::Blur | `observation.rs:handle_resource_exhausted` — Phase 9 |
| Strict 策略下資源耗盡 → Value::Bottom | `observation.rs` |
| Bohrification（Q→B project_down, B→Q project_up） | `lib.rs`, `bohr_test.rs` — Phase 7 |
| had_nondistrib_event 非分配性旗標 | `EvalContext.had_nondistrib_event` — Phase 7 |
| Unify memoization | `Ouroboros.unify_memo: RwLock<HashMap>` |

### 剩餘 △

| 功能 | 說明 |
|:-----|:-----|
| `%max_pattern_nodes` 組合爆炸保護 | 欄位存在於 EvalContext，但 unify 路徑未完整套用 |

---

## 4. SPEC_09 標準庫

### 已實作 ✓

| 模組/功能 | 位置 |
|:---------|:-----|
| **~%Math** `/add` `/sub` `/mul` `/div` `/rem` `/abs` `/pow` `/bit*` `/random` | `builtins/math.rs` |
| **~%Math** `/exp` `/ln` `/sin` `/cos` `/sqrt` `/eml` | `builtins/math.rs` — Phase 前期 |
| `ln(0)` → `Value::Blur`（#log_singularity） | `builtins/math.rs:blur_singularity` — Phase 9 |
| `%branch` Riemann 面（`math.ln`, `math.sqrt`, `math.eml`） | `builtins/math.rs` — Phase 13+14 |
| **~%Complex** `/conj` `/phase` `/real` `/imag` | `builtins/math.rs` |
| **~%List** `/len` `/at` `/concat` `/reverse` `/slice` `/zip` `/sort` `/map` `/fold` `/filter` | `builtins/list.rs` |
| **~%Str** `/concat` `/len` `/trim` `/split` `/join` `/replace` `/to_lower` `/to_upper` `/starts_with` `/ends_with` `/contains` | `builtins/string.rs` |
| **~%Cond** `/if` `/cond` | `builtins/cond.rs` |
| **~%Cond** `/match`（真實模式匹配） | `builtins/cond.rs` — Phase 14 |
| **~%Time** `/now` | `builtins/time.rs` |
| **~%Refl** `/keys` `/has` `/is_cocoon` `/type_of` | `builtins/reflection.rs` |
| **~%Engine** `/observe` `/save` `/add_architect` `/sign_refine` | `builtins/engine.rs` |
| **~%Disc** `/connect` `/fetch` `/identify` `/advertise` `/find` | `builtins/disc.rs` |
| `@option` 標準型別 | `type_constraint.rs`, genesis seed — Phase 12 |
| `@result` 標準型別 | `type_constraint.rs`, genesis seed — Phase 12 |
| `~%Config` genesis 預設值 | `lib.rs:root_with_system()` — Phase 13 |
| `Ouroboros::eval_context()` | `lib.rs` — Phase 14 |

### 剩餘 △

| 功能 | 說明 |
|:-----|:-----|
| `%fmap`/`%fold` 代數介面元欄位 | 只有 list.map/fold，沒有 Functor 層 |
| `%timeout` → `timeout_deadline` 動態設定 | 需要 SystemTime::now() 計算，未接入 |

---

## 5. SPEC_10 演化與 Commit

### 已實作 ✓

| 功能 | 位置 |
|:-----|:-----|
| `#refine` Commit 類型 | `universe.rs:refine()` |
| 幾何單調性驗證（$ID_{new} \sqsubseteq ID_{old}$） | `universe.rs` step 1a |
| Ed25519 Authority 簽署驗證 | `authority.rs` — Phase 8 |
| `bootstrap_exempt` Epoch 判定 | `universe.rs` `self.head.is_none() \|\| architect_reg.is_empty()` — Phase 10 |
| `oo refine` CLI 子命令 | `crates/oo/src/main.rs` — Phase 10 |
| Architects 清單持久化（`.oo/architects.json`） | `storage.rs`, `lib.rs` — Phase 11 |
| Shadow Refinement（DAG 回溯掃描） | `universe.rs` step 1c — Phase 12 |
| `RefineInfo.shadow_affected` | `value.rs:RefineInfo` — Phase 12 |

### 剩餘 △

| 功能 | 說明 |
|:-----|:-----|
| 循環阻斷檢測 | Commit DAG cycle detection |
| Equivalence map 合成 | `~%Engine.equivalence_map` 動態視圖 |

---

## 6. SPEC_13 OODP

### 已實作 ✓

| 功能 | 位置 |
|:-----|:-----|
| BN/ 位元流序列化 | `bn_serial.rs`（含 Blur tag 0xFD）— Phase 3/9 |
| Lattice Sketch v2 | `lattice_sketch.rs` — Phase 3/13 |
| CAID v2 格式（`hash:sha256:v2:<masa>:<sketch>:<digest>`） | `value.rs:ContentHash` |
| Genesis 種子 CAID（@option, @result, ~%Config） | `genesis.rs` — Phase 12/13 |
| 跨架構穩定性測試（5 個 EXPECTED_SKETCH_* 常數） | `lattice_sketch_v2_test.rs` — Phase 13 |
| LADD 引力路由（GBB, gravitational weight） | `ladd.rs`, `builtins/disc.rs` — Phase 5/6 |
| nerve_structure MASA（field-key based） | `builtins/disc.rs:field_key_masa_id` — Phase 11 |
| nerve_overlap 前置過濾 | `ladd.rs:nerve_overlap` |

### 剩餘 △

| 功能 | 說明 |
|:-----|:-----|
| `/find` 引力導航態射 | disc.find 存在但尚未完整走引力權重路徑 |
| 視界震盪防禦（#semantic_eclipse） | APP_05 §4.2，未實作 |
| GPP/CIP 零知識證明 | APP_05 §5-6，P3 |

---

## 7. 核心資料結構（當前版本）

### 7.1 Value 類型

```
Value (enum)
├── Top                              # 萬有子空間 _
├── Bottom(Box<BottomDetail>)        # 矛盾 _|_ + 原因
├── Blur(BlurDetail)                 # 視界模糊（Phase 9）
├── Atom(AtomKind, EffectTag, Option<i64>)
│   ├── Int(BigInt)                  # 任意精度整數
│   ├── Float(f64)                   # IEEE 754
│   ├── Complex(f64, f64)            # 複數
│   ├── Str(String)                  # 字串
│   ├── Tag(String)                  # #true, #false 等
│   ├── TagStart                     # #_|_
│   ├── TagEnd                       # #_
│   ├── Regex, PathLit, Bytes, Uri, Time
│   └── Unit                         # ()
├── Combo(ComboVal)                  # 組合結構
├── Union(Vec<Value>)                # 聯集 A | B
├── Thunk { expr, closure, effect }  # 惰性求值
├── Code(Box<Expr>)                  # 未執行程式碼
```

### 7.2 BlurDetail（Phase 9 新增）

```rust
pub struct BlurDetail {
    pub cause: BlurCause,       // FuelExhausted/Timeout/StackOverflow/MathSingularity
    pub horizon: HorizonParams, // { fuel_remaining, strategy, salt }
    pub partial: Option<Box<Value>>,
    pub effect: EffectTag,
}
```

### 7.3 BottomCause

```
Conflict, MissingKey, FuelExhausted, Timeout, Divergent,
InvalidPath, PrivateAccessViolation, NumericalError,
ArithmeticOnAnchor, H1Split, H2Split
```

### 7.4 ComboVal 結構

```rust
pub struct ComboVal {
    pub data:    IndexMap<String, Value>,  // 無前綴普通欄位
    pub types:   IndexMap<String, Value>,  // @type 約束
    pub rules:   IndexMap<String, Value>,  // /rule 態射
    pub meta:    IndexMap<String, Value>,  // %meta 元資料
    pub system:  IndexMap<String, Value>,  // ~%system 內建
    pub local:   IndexMap<String, Value>,  // ~local 私有
    pub closed:  bool,                     // Cocoon 模式
    pub effect:  EffectTag,
    pub relations: Vec<ValRelation>,
    pub masa_ref: MasaRef,                 // Phase 3：MASA 識別
}
```

### 7.5 Ouroboros 引擎

```rust
pub struct Ouroboros {
    pub store:             ObjectStore,
    pub base_dir:          Option<PathBuf>,          // Phase 11
    pub unify_memo:        RwLock<HashMap<...>>,
    pub builtin_registry:  HashMap<String, Arc<BuiltinFn>>,
    pub peers:             RwLock<HashMap<String, Peer>>,
    pub identity:          Identity,                 // Ed25519
    pub refine_map:        RwLock<HashMap<String, Vec<String>>>,
    pub gbb_registry:      RwLock<HashMap<String, GBB>>,
    pub architect_registry: RwLock<HashSet<String>>, // Phase 11
}
```

---

## 8. 測試套件現況（24 個 test 檔）

| 測試檔 | 測試數 | 覆蓋範圍 |
|:-------|:------:|:---------|
| `refine_test.rs` | 13 | #refine 完整流程、authority、shadow |
| `orthomodular_test.rs` | ~10 | OML 驗證 |
| `oml_test.rs` | ~8 | Bohrification 非分配性 |
| `lattice_sketch_v2_test.rs` | 17 | Sketch 穩定性、跨架構 |
| `blur_test.rs` | 11 | Value::Blur 傳播 |
| `type_constraint_test.rs` | ~10 | @option/@result 驗證 |
| `cond_match_test.rs` | 4 | /match 模式匹配 |
| `nerve_routing_test.rs` | 10 | LADD/nerve MASA |
| `genesis_test.rs` | 9 | 種子 CAID、~%Config |
| `math_branch_test.rs` | ~6 | %branch Riemann 面 |
| `authority_test.rs` | ~8 | Ed25519 簽署 |
| `unify_test.rs` | ~10 | Meet 規則 |
| 其他 12 個 | ~57 | eval, dispatch, path, storage 等 |

**總計：173 tests, 0 failed**
