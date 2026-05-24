# nlang 引擎架構概覽

> 最後更新：2026-05-24（Phase 14 完成後）  
> 供新貢獻者快速定位切入點。

---

## 1. 專案結構

```
nlang-tools/
├── crates/
│   ├── parser/          # AST 與語法解析
│   │   ├── src/lib.rs   # Parser 入口（Pest）
│   │   ├── src/ast.rs   # AST 類型定義
│   │   └── src/n.pest   # Pest 語法定義
│   │
│   ├── interpreter/     # 核心 runtime 引擎
│   │   ├── src/lib.rs            # Ouroboros 引擎、EvalContext
│   │   ├── src/value.rs          # Value、ComboVal、EffectTag、ContentHash
│   │   ├── src/eval.rs           # 表達式求值
│   │   ├── src/unify.rs          # 統一化（Meet）運算 + H¹/H² obstruction
│   │   ├── src/dispatch.rs       # 態射模式派發
│   │   ├── src/complement.rs     # 正交補運算（!）
│   │   ├── src/oml.rs            # Orthomodular Law 驗證
│   │   ├── src/observation.rs    # 資源耗盡處理（→ Blur/Bottom）
│   │   ├── src/type_constraint.rs # @option/@result 型別驗證
│   │   ├── src/universe.rs       # Universe 狀態管理（#refine）
│   │   ├── src/storage.rs        # CAID 物件儲存、architects 持久化
│   │   ├── src/authority.rs      # Ed25519 簽署驗證
│   │   ├── src/bn_serial.rs      # BN/ 位元流序列化
│   │   ├── src/lattice_sketch.rs # Lattice Sketch v2
│   │   ├── src/ladd.rs           # LADD 引力路由（GBB）
│   │   ├── src/genesis.rs        # 創世種子 CAID 常數
│   │   └── src/builtins/         # 內建模組
│   │       ├── mod.rs            # 註冊入口
│   │       ├── math.rs           # ~%Math, ~%Complex
│   │       ├── list.rs           # ~%List
│   │       ├── string.rs         # ~%Str
│   │       ├── cond.rs           # ~%Cond（/if, /cond, /match）
│   │       ├── disc.rs           # ~%Disc（LADD 發現協議）
│   │       ├── engine.rs         # ~%Engine（/observe, /save, /add_architect, /sign_refine）
│   │       ├── reflection.rs     # ~%Refl（/keys, /has, /is_cocoon, /type_of）
│   │       └── time.rs           # ~%Time（/now）
│   │
│   └── oo/              # CLI 工具入口
│       ├── src/main.rs          # CLI 命令處理
│       └── src/static_analyzer.rs # 靜態分析
│
└── docs/                # 文件目錄
    ├── implementation-status.md  # 實作狀態（本次更新）
    ├── engine-architecture.md    # 本文件
    ├── feature-roadmap.md        # 功能路線圖（本次更新）
    └── phase-N-handover.md       # Phase 1-14 交接文件
```

---

## 2. Crate 概覽

### 2.1 `nlang-parser`

**用途**：將源文本轉換為 AST

**關鍵公開 API**：
```rust
pub fn parse_program(input: &str) -> Result<Program, Box<dyn Error>>
pub fn parse_expr_only(input: &str) -> Result<Expr, Box<dyn Error>>
```

**切入點**：
- 新增語法規則 → `n.pest`
- 新增 AST 類型 → `ast.rs:ExprKind`
- 新增解析邏輯 → `lib.rs:parse_expr`

---

### 2.2 `nlang-interpreter`

**關鍵類型一覽**：

| 類型 | 檔案 | 說明 |
|:-----|:-----|:-----|
| `Ouroboros` | `lib.rs` | 主引擎：store、registry、peers、identity、GBB、architects |
| `EvalContext` | `lib.rs` | 求值上下文：root、fuel、depth、strategy、max_* |
| `Universe` | `universe.rs` | 狀態管理：head、root、#refine |
| `Value` | `value.rs` | 核心值類型 enum |
| `ComboVal` | `value.rs` | Combo 結構體（含 masa_ref）|
| `BlurDetail` | `value.rs` | #blur 視界（Phase 9）|
| `BottomDetail` | `value.rs` | 矛盾詳情（含 H¹/H² obstruction）|
| `ContentHash` | `value.rs` | CAID（SHA256 v2）|
| `AuthorityInfo` | `authority.rs` | Ed25519 簽署資訊 |
| `GBB` | `ladd.rs` | 幾何邊界盒（LADD 路由用）|
| `Identity` | `value.rs` | Ed25519 身份金鑰對 |

**切入點**：
- 新增 Value 類型 → `value.rs:Value enum + AtomKind`
- 新增統一化規則 → `unify.rs:do_unify`
- 新增求值邏輯 → `eval.rs:eval_internal`
- 新增內建模組 → `builtins/` 新建模組 + `mod.rs` 註冊

---

### 2.3 `oo` (CLI)

**CLI 命令**：

| 命令 | 功能 |
|:-----|:-----|
| `oo run <files>` | 單次執行 |
| `oo evolve <files>` | 演化暫存區 |
| `oo test <files>` | 執行測試 |
| `oo repl` | 互動 REPL |
| `oo status` | 顯示暫存狀態 |
| `oo log` | 顯示 Commit 歷史 |
| `oo commit` | 提交暫存區 |
| `oo refine --source <caid> --target <caid> [--sign]` | 宣告格論精炼 |
| `oo fmt <file>` | 格式化源碼 |
| `oo serve [--port N]` | NDP 網路服務 |

---

## 3. 核心資料結構

### 3.1 Value 類型層次

```
Value (enum)
├── Top                                  # 萬有子空間 _
├── Bottom(Box<BottomDetail>)            # 矛盾 _|_（含 cause + H¹/H² obstruction）
├── Blur(BlurDetail)                     # 視界模糊（Phase 9）
│     ├── cause: BlurCause              # FuelExhausted/Timeout/StackOverflow/MathSingularity
│     ├── horizon: HorizonParams        # fuel_remaining, strategy, salt
│     └── partial: Option<Box<Value>>   # 部分結果
├── Atom(AtomKind, EffectTag, Option<i64>)
│   ├── Int(BigInt)                      # 任意精度整數
│   ├── Float(f64)                       # IEEE 754 雙精度
│   ├── Complex(f64, f64)               # 複數（re + im·i）
│   ├── Str(String)                      # 字串
│   ├── Tag(String)                      # 標籤 #true, #false, #none ...
│   ├── TagStart / TagEnd                # 序位錨點
│   ├── Regex, PathLit, Bytes, Uri, Time
│   └── Unit                             # ()
├── Combo(ComboVal)                      # 組合結構 {}
├── Union(Vec<Value>)                    # 聯集 A | B
├── Thunk { expr, closure, effect }      # 惰性求值
└── Code(Box<Expr>)                      # 未執行程式碼
```

### 3.2 ComboVal 結構

```rust
pub struct ComboVal {
    pub data:    IndexMap<String, Value>,  // 無前綴（普通欄位）
    pub types:   IndexMap<String, Value>,  // @ 前綴（型別約束）
    pub rules:   IndexMap<String, Value>,  // / 前綴（態射）
    pub meta:    IndexMap<String, Value>,  // % 前綴（元資料）
    pub system:  IndexMap<String, Value>,  // ~% 前綴（系統內建）
    pub local:   IndexMap<String, Value>,  // ~ 前綴（私有）
    pub closed:  bool,                     // Cocoon 模式 {{}}
    pub effect:  EffectTag,                // Pure/State/IO/NonDet
    pub relations: Vec<ValRelation>,       // 序位關係 <, >, <=, >=
    pub masa_ref: MasaRef,                 // MASA 識別（Top 或 Digest）
}
```

### 3.3 前綴命名空間

| 前綴 | 符號 | 命名空間 | 範例 |
|:-----|:-----|:---------|:-----|
| System | `~%` | 系統內建 | `~%Math./add` |
| Private | `~` | 私有欄位 | `~temp` |
| Logic | `/` | 態射/規則 | `/handler` |
| Type | `@` | 型別約束 | `@int`, `@option` |
| Meta | `%` | 元資料 | `%morphism`, `%branch` |
| Data | 無 | 普通資料 | `name`, `0`, `1` |

### 3.4 Ouroboros 引擎

```rust
pub struct Ouroboros {
    pub store:             ObjectStore,            // CAID 物件儲存
    pub base_dir:          Option<PathBuf>,        // .oo/ 目錄（Phase 11）
    pub unify_memo:        RwLock<HashMap<...>>,   // 統一化 memoization
    pub builtin_registry:  HashMap<String, Arc<BuiltinFn>>,
    pub peers:             RwLock<HashMap<String, Peer>>,
    pub identity:          Identity,               // Ed25519 金鑰對
    pub refine_map:        RwLock<HashMap<String, Vec<String>>>,
    pub gbb_registry:      RwLock<HashMap<String, GBB>>, // LADD
    pub architect_registry: RwLock<HashSet<String>>,     // Phase 11
}
```

### 3.5 EvalContext

```rust
pub struct EvalContext {
    pub root:                ComboVal,           // 作用域根（含 ~%Config 等）
    pub scopes:              Vec<ComboVal>,
    pub fuel:                u64,                // 預設 10000（~%Config）
    pub strategy:            ObservationStrategy, // Blur/Strict/Approximate
    pub max_branches:        usize,              // 預設 64
    pub max_unification_depth: usize,            // 預設 256
    pub max_pattern_nodes:   usize,              // 預設 1024
    pub had_nondistrib_event: bool,              // OML 非分配性旗標
    // ... 其他欄位
}
// 建構：Ouroboros::eval_context()（讀取 ~%Config）或 EvalContext::new(root)
```

---

## 4. 核心演算法

### 4.1 統一化（Meet `&`）

**位置**：`unify.rs:unify_internal` → `do_unify`

```
unify(A, B):
  1. hash(A) == hash(B)                    → 返回 A（memoized）
  2. A == Top                              → 返回 B
  3. A == Bottom 或 B == Bottom            → 返回 Bottom
  4. A == Blur 或 B == Blur               → Blur 傳播規則
  5. A, B 是 Atom                         → 相等則返回 A，否則 Bottom
  6. A, B 都是 Combo                      → phase_merge_decision
     - H² MASA 不相容                     → Bottom(H2Split)
     - θ >= ε_coherent                    → Bottom(H1Split)（目前 θ 恆為 0.0）
     - 否則遞迴合併欄位
  7. A 是 Atom, B 是 Combo               → Trinity Isomorphism（展開為 {%val: A}）
  8. Union                                → 極小元素篩選
```

### 4.2 求值流程

**位置**：`eval.rs:eval_internal`

| ExprKind | 處理 |
|:---------|:-----|
| `Atom` | 直接返回 Value |
| `Path` | 在 root/scopes 中觀測路徑 |
| `Apply` | 呼叫態射 |
| `Pipe` | `a \|> b = b(a)` |
| `Morphism` | 建立閉包 |
| `Combo` | 構建 ComboVal |
| `Meet/Join/Complement` | 呼叫 unify/eval/complement |
| `Add/Sub/Mul/Div/Rem` | 數學運算 |
| `List` | 構建 List Combo |

### 4.3 態射派發

**位置**：`dispatch.rs:dispatch_morphism`

```
dispatch_morphism(morphism, arg):
  1. 從 %rules 提取所有分支
  2. 對每個分支：unify(pattern, arg) 測試
  3. 篩選非 Bottom 結果
  4. 極小元素規則（移除被包含者）
  5. 單一極小 → 執行 body；多個 → Union
```

### 4.4 CAID v2 計算

**格式**：`hash:sha256:v2:<masa_ref>:<lattice_sketch>:<content_digest>`

**流程**：
```
content_hash(value):
  1. BN/ 序列化（bn_serial.rs）→ 位元組流
  2. lattice_sketch(value)     → Base64 字串（≤16 分量）
  3. SHA256(BN/ bytes)         → content_digest
  4. 組合 → CAID
```

**Blur CAID**：`SHA256("blur:" || cause_bytes || ":fuel=" || fuel_le64 || ":strategy=" || strat_u8 || ":salt=" || salt_32bytes)`

### 4.5 #refine 流程

**位置**：`universe.rs:refine()`

```
refine(source_caids, target_caids, authority, meta):
  Step 1a: 幾何單調性驗證（ID_new ⊑ ID_old）
  Step 1b: bootstrap_exempt = head.is_none() || architect_reg.is_empty()
           → verify_refine_authority（Ed25519 簽署）
  Step 1c: Shadow scan（向上掃 16 commits 找 source_caids 出現處）
  Step 2:  寫入 RefineCommit → ObjectStore
  Step 3:  更新 refine_map + head
```

---

## 5. 內建模組詳解

### 5.1 現有模組清單

| 模組 | 檔案 | 態射 | 行數 |
|:-----|:-----|:-----|-----:|
| Math | `math.rs` | `/add` `/sub` `/mul` `/div` `/rem` `/abs` `/pow` `/bit*` `/random` `/exp` `/ln` `/sin` `/cos` `/sqrt` `/eml` | ~407 |
| Complex | `math.rs` | `/conj` `/phase` `/real` `/imag` | — |
| List | `list.rs` | `/len` `/at` `/concat` `/reverse` `/slice` `/zip` `/sort` `/map` `/fold` `/filter` | ~198 |
| Str | `string.rs` | `/concat` `/len` `/trim` `/split` `/join` `/replace` `/to_lower` `/to_upper` `/starts_with` `/ends_with` `/contains` | ~117 |
| Cond | `cond.rs` | `/if` `/cond` `/match` | ~75 |
| Disc | `disc.rs` | `/connect` `/fetch` `/identify` `/advertise` `/find` | ~203 |
| Engine | `engine.rs` | `/observe` `/save` `/add_architect` `/sign_refine` | ~255 |
| Refl | `reflection.rs` | `/keys` `/has` `/is_cocoon` `/type_of` | ~68 |
| Time | `time.rs` | `/now` | ~10 |

### 5.2 新增內建模組的步驟

1. 建立 `builtins/my_module.rs`
2. 在 `builtins/mod.rs` 引入並呼叫 `register_my_builtins(&mut m)`
3. 在 `lib.rs:root_with_system()` 建立 `~%MyModule` Combo 並暴露態射

### 5.3 系統根（root_with_system）包含

| 鍵 | 內容 |
|:---|:-----|
| `~%Math` | 數學態射 |
| `~%Complex` | 複數態射 |
| `~%List` | 串列態射 |
| `~%Str` | 字串態射 |
| `~%Cond` | 條件控制流（/if, /cond, /match）|
| `~%Disc` | LADD 發現協議 |
| `~%Engine` | 引擎操作 |
| `~%Refl` | 反射 |
| `~%Time` | 時間 |
| `@option` | Option 型別約束 |
| `@result` | Result 型別約束 |
| `~%Config` | Genesis 配置預設值（%fuel: 10000, %max_branches: 64, ...）|

---

## 6. 效果系統

### 6.1 EffectTag 層級

| 標籤 | 值 | 說明 |
|:-----|:---|:-----|
| Pure | 0 | 確定性、無副作用、可快取 |
| State | 1 | 讀寫程式狀態 |
| IO | 2 | 外部 I/O |
| NonDet | 3 | 非確定性（random, time）|

### 6.2 ObservationStrategy（Phase 9）

| 策略 | 資源耗盡行為 | 奇點行為 |
|:-----|:------------|:---------|
| `Blur`（預設） | → `Value::Blur` | → `Value::Blur` |
| `Strict` | → `Value::Bottom` | → `Value::Bottom` |
| `Approximate` | → `Value::Blur` | → approximate result |

---

## 7. 測試系統

### 7.1 測試格式（.n 文件）

```nlang
test_basic: 1 + 1 == 2
test_morph: (x -> x + 1) 5 == 6
test_error: 1 & 2 == _|_
```

### 7.2 Rust 整合測試（tests/*.rs）

24 個測試檔，173 tests 全部通過。主要覆蓋：
- 格論（unify, complement, oml, orthomodular）
- CAID（caid, lattice_sketch_v2）
- 精炼（refine, authority）
- 模糊（blur）
- 標準庫（eval, dispatch, cond_match, math_branch, type_constraint）
- LADD（ladd, nerve_routing）
- 創世（genesis）

---

## 8. 擴展點索引

| 目標 | 檔案 | 關鍵位置 |
|:-----|:-----|:---------|
| 新增語法 | `parser/n.pest`, `parser/ast.rs` | Rule enum, ExprKind enum |
| 新增值類型 | `interpreter/value.rs` | Value enum, AtomKind enum |
| 新增統一化規則 | `interpreter/unify.rs` | do_unify() |
| 新增求值邏輯 | `interpreter/eval.rs` | eval_internal() |
| 新增正交補規則 | `interpreter/complement.rs` | orthocomplement() |
| 新增 BN/ 序列化 | `interpreter/bn_serial.rs` | serialize_value() |
| 新增內建函數 | `interpreter/builtins/*.rs` | 新建模組 + mod.rs 註冊 |
| 新增型別約束 | `interpreter/type_constraint.rs` | TypeConstraint enum |
| 新增 CLI 命令 | `oo/main.rs` | Commands enum + match arm |
| 新增靜態檢查 | `oo/static_analyzer.rs` | StaticViolation enum |

---

## 9. 與規格書的對應

| 規格章節 | 引擎對應檔案 |
|:---------|:-------------|
| SPEC_01（格論基礎） | `value.rs`, `unify.rs`, `complement.rs`, `oml.rs` |
| SPEC_06（統一化邏輯） | `unify.rs`, `dispatch.rs`, `observation.rs` |
| SPEC_09（標準庫） | `builtins/*.rs`, `type_constraint.rs` |
| SPEC_10（演化與 Commit） | `universe.rs`, `storage.rs`, `authority.rs` |
| SPEC_13（OODP）| `builtins/disc.rs`, `ladd.rs`, `lattice_sketch.rs`, `bn_serial.rs` |
| REAL_03（CAID 協議） | `value.rs:content_hash()`, `bn_serial.rs`, `lattice_sketch.rs` |
| APP_05（LADD） | `ladd.rs`, `builtins/disc.rs`（部分）|
| SPEC_17（自我演化） | 未實作 |

---

## 10. 快速入門建議

1. 先讀 `value.rs` — 理解 `Value` enum，尤其 `Blur`、`Bottom`、`Combo`
2. 再讀 `unify.rs` — 理解 Meet 運算、H¹/H² obstruction
3. 讀 `eval.rs` — 理解求值流程和態射派發
4. 用 `oo repl` 或測試 (`cargo test -p nlang-interpreter`) 實驗驗證
5. 看 `docs/phase-N-handover.md` — 了解各功能的實作決策背景
