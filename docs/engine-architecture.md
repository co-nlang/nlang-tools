# nlang 引擎架構概覽

> 最後更新：2026-07-12（v0.2.0 裸核定版；REAL_05 conformance 45/45）  
> 供新貢獻者快速定位切入點。求值模型的施工圖見 nlang-spec
> `GUIDE_03 §11`（Call-by-Observation）；逐案決策背景見 `docs/worknotes/`。

---

## 1. 專案結構

```
nlang-tools/
├── crates/
│   ├── parser/          # AST 與語法解析
│   │   ├── src/lib.rs   # Parser 入口（Pest；expr_toplevel=SOI~expr~EOI 護欄）
│   │   ├── src/ast.rs   # AST 類型定義（含 strip_spans/shape 指紋、to_nlang 正規列印）
│   │   ├── src/tier.rs  # 019 tier 分類器（C/M/Q/U；memo 策略與 linter 共用）
│   │   └── src/n.pest   # Pest 語法定義（SPEC_14 定稿全同步）
│   │
│   ├── interpreter/     # 核心 runtime 引擎
│   │   ├── src/lib.rs            # Ouroboros 引擎、EvalContext、root_with_system()
│   │   ├── src/value.rs          # Value、ComboVal、EffectTag、ContentHash
│   │   ├── src/eval.rs           # 表達式求值
│   │   ├── src/unify.rs          # 統一化（Meet）+ H¹/H² obstruction
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
│   │       ├── mod.rs            # 統一註冊入口
│   │       ├── math.rs           # ~%Math（31 態射）+ ~%Complex（4）
│   │       ├── list.rs           # ~%List（32 態射）
│   │       ├── string.rs         # ~%String（28 態射）
│   │       ├── cond.rs           # ~%Cond（/if /cond /match）
│   │       ├── engine.rs         # @option/@result combinators + ~%Engine
│   │       ├── reflection.rs     # ~%Reflection（17 態射）
│   │       ├── disc.rs           # ~%Discovery（LADD 發現協議）
│   │       ├── time.rs           # ~%Time（/now /format /diff /add_ms）
│   │       ├── bytes.rs          # ~%Bytes（12 態射：基礎 + sha256/base64/hmac）
│   │       ├── regex.rs          # ~%Regex（/match /find /replace /split）
│   │       ├── json.rs           # ~%Json（/parse /stringify /get /keys）
│   │       ├── io.rs             # ~%Io（/read_file /write_file /exists /append_file）
│   │       ├── env.rs            # ~%Env（/get /args /cwd）
│   │       ├── process.rs        # ~%Process（/exit /pid）
│   │       └── path.rs           # ~%Path（/join /dirname /basename /extension /is_absolute）
│   │
│   └── oo/              # CLI 工具入口
│       ├── src/main.rs          # CLI 命令處理
│       ├── src/nlint.rs         # Linter Tier 1（R1/R2/R3 + context graph ω(G)）
│       └── src/static_analyzer.rs # 靜態分析
│
└── docs/                # 文件目錄
    ├── implementation-status.md  # 實作狀態
    ├── engine-architecture.md    # 本文件
    ├── feature-roadmap.md        # 功能路線圖
    └── worknotes/                # 交接與工單存檔（phase-N + *_handover）
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
| `Ouroboros` | `lib.rs` | 主引擎：store、builtin_registry、peers、identity |
| `EvalContext` | `lib.rs` | 求值上下文：root、fuel、depth、strategy |
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
- 新增內建模組 → `builtins/` 新建模組 + `mod.rs` 註冊 + `lib.rs:root_with_system()` + `genesis.rs:all_seeds()`

---

### 2.3 `oo` (CLI)

**CLI 命令**：

| 命令 | 功能 |
|:-----|:-----|
| `oo run <files>` | 單次執行 |
| `oo eval <expr>` | 單次求值（Phase 23） |
| `oo inspect <expr>` | 詳細輸出（Phase 23） |
| `oo evolve <files>` | 演化暫存區 |
| `oo test <files>` | 執行測試 |
| `oo repl` | 互動 REPL |
| `oo status` | 顯示暫存狀態 |
| `oo log` | 顯示 Commit 歷史 |
| `oo commit` | 提交暫存區 |
| `oo refine --source <caid> --target <caid> [--sign]` | 宣告格論精炼 |
| `oo fmt <file>` | 格式化源碼 |
| `oo serve [--port N]` | NDP 網路服務 |
| `oo lint <path> [--json]` | Linter Tier 1：R1/R2/R3 靜態規則＋ω(G)/K4/K5 candidate sites |

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
│   ├── TagStart / TagEnd                # 序位錨點 #_|_ / #_
│   └── Bytes(Vec<u8>)                  # 二進位（Phase 30）
├── Combo(ComboVal)                      # 組合結構 {}
├── Union(Vec<Value>)                    # 聯集 A | B
├── Range { start, end, step }           # 閉閉區間集合 [a,b]（SPEC_02 §3；bn_serial 0x18）
├── Ref(Path)                            # 活引用（<<path>> C 案：觀測期對當下宇宙解引用）
├── Thunk { expr, closure, context, effect }  # 惰性求值（GUIDE_03 §11 四欄）
└── Code(Box<Expr>)                      # 未執行程式碼
```

> 字面量 `_`／`_|_` 於求值端**正規化**為 `Value::Top`／`Value::Bottom`
> （unify 另有忠實別名臂兜底手構形態）；`Atom(TagStart/TagEnd)` 是序位錨點
> （順序極值），與資訊格極值是**不同格**，勿混用。

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
    pub relations: Vec<ValRelation>,
    pub masa_ref: MasaRef,                 // MASA 識別
}

// 常用 API
pub fn get_field(&self, key: &str) -> Option<&Value>
pub fn insert_field(&mut self, key: &str, val: Value)
pub fn fields(&self) -> IndexMap<String, Value>       // clone（開銷較大）
pub fn fields_iter(&self) -> impl Iterator<Item = (&String, &Value)>  // 零拷貝
pub fn collapse(&self) -> &Value  // 穿透 %val 包裝
```

### 3.3 前綴命名空間

| 前綴 | 符號 | 命名空間 | 範例 |
|:-----|:-----|:---------|:-----|
| System | `~%` | 系統內建 | `~%Math./add` |
| Private | `~` | 私有欄位 | `~temp` |
| Logic | `/` | 態射/規則 | `/handler` |
| Type | `@` | 型別約束 | `@int`, `@option` |
| Meta | `%` | 元資料 | `%morphism`, `%branch`, `%kind` |
| Data | 無 | 普通資料 | `name`, `0`, `1` |

### 3.4 Ouroboros 引擎

```rust
pub struct Ouroboros {
    pub store:              ObjectStore,
    pub base_dir:           Option<PathBuf>,
    pub unify_memo:         RwLock<HashMap<(ContentHash, ContentHash), Value>>,
    pub force_memo:         RwLock<HashMap<ForceMemoKey, MemoEntry>>, // Stage 4 觀測 memo
    pub force_memo_rev:     RwLock<HashMap<String, HashSet<ForceMemoKey>>>, // Stage 5 反向索引：coord → keys
    pub builtin_registry:   HashMap<String, Arc<BuiltinFn>>,
    pub peers:              RwLock<HashMap<String, Peer>>,
    pub identity:           Identity,
    pub refine_map:         RwLock<HashMap<String, Vec<String>>>,
    pub gbb_registry:       RwLock<HashMap<String, GBB>>,
    pub architect_registry: RwLock<HashSet<String>>,
}
// ForceMemoKey = (expr CAID, frame CAID, 有效綁定 context CAID | #open, root CAID)
// MemoEntry   = { value, deps: HashSet<String> }；deps=∅ ⇒ C₀ 天然永久
// evolve → invalidate_coords(逐座標＋"*")；commit/load/refine → 全清
```

### 3.5 EvalContext（惰性引擎後）

```rust
pub struct EvalContext {
    pub root:              Arc<ComboVal>,        // F2：sub_context 只 bump 引用計數
    pub root_caid_cache:   Option<ContentHash>,  // Stage 4：root CAID 惰性快取
    pub scopes:            Vec<ComboVal>,
    pub staged:            Option<ComboVal>,     // 暫存區可見性
    pub context_value:     Option<Value>,        // `$` 綁定（pipe/dispatch/deref 框架）
    pub dep_collector:     Option<HashSet<String>>, // Stage 5：Route B 依賴收集
    pub memo_enabled:      bool,                 // Stage 5：engine 內部語境（eval_context）關閉
    pub fuel:              u64,                  // 預設 10000（~%Config）
    pub depth:             u32,
    pub timeout_deadline:  Option<u64>,
    // …（strategy / max_* / had_nondistrib_event 等資源與策略欄位同前）
}
// 建構：Ouroboros::eval_context()（讀取 ~%Config；memo_enabled=false）
// force() 為觀測原語：tier 分類（C/M memo、Q/U 旁路）→ 有效綁定
// （thunk.context ∨ ctx.context_value）一次計算、key 與 call_ctx 共用
```

---

## 4. 核心演算法

### 4.1 統一化（Meet `&`）

**位置**：`unify.rs:unify_internal` → `do_unify`

```
unify(A, B):
  1. hash(A) == hash(B)          → A（memoized；NonDet 旁路）
  2. Top × Thunk/Ref             → 保留 Thunk/Ref（惰性：不解引用——evolve 期
                                    deref = A 案快照，Stage 3 教訓）
  3. Atom(Top)/Atom(Bottom) 別名 → 重入 Value::Top / Value::Bottom（忠實別名）
  4. A == Top                    → B
  5. Bottom                      → Bottom
  6. Blur                        → Blur 傳播規則
  7. Range 臂（僅認 Range×Range、Atom×Range；其餘 DECLINE 放行下游——
     臂序教訓：catch-all 預設讓位，勿搶 Union 分配）
     Atom × Range               → 成員判定（閉端＋步進）
     Range × Range（無步進）     → 交集（空→⊥、單點→塌縮為原子）
  8. Atom × Atom                 → 相等則 A，否則 Bottom
  9. Combo × Combo               → phase_merge_decision
     H² MASA 不相容              → Bottom(H2Split)
     θ >= ε_coherent             → Bottom(H1Split)
     否則遞迴合併欄位
  10. Atom × Combo               → Trinity Isomorphism（{%val: atom}）
  11. Union                      → 逐支分配 + ⊥ 剪枝 + 極小元素篩選
```

### 4.2 求值流程

**位置**：`eval.rs:eval_internal`

| ExprKind | 處理 |
|:---------|:-----|
| `Atom` | 返回 Value（`Top`/`Bottom` 正規化為格極值） |
| `Path` | 在 root/staged/scopes 中觀測路徑（惰性：欄位值為 Thunk，force 時求值） |
| `Structural`（`<<path>>`） | `Value::Ref` 活引用（觀測期對當下宇宙解引用） |
| `Apply` | 呼叫 apply_morphism（Union 入口逐支分配） |
| `Pipe` | `a \|> b`＝疊加 monad Kleisli bind（Union 逐支、⊥ 剪枝、`$` 逐支綁定） |
| `Morphism` | 建立閉包 |
| `Combo` | 構建 ComboVal |
| `Meet/Join/Complement` | 呼叫 unify/eval/complement |
| `Eq/Ne` vs `Lt/Gt/Lte/Gte` | **兩家族分流**：原子家族吸收 ⊥/⊤；集合家族乾淨布林（極值真值表） |
| `LatticeEq`（`=`） | 非塌縮結構等值；⊥ 為空集運算元（不吸收） |
| `Range` | 界求值＋force → `Value::Range`（變數界於觀測期解析） |
| `AnonSet`（`@{e}`） | 透明 ≡ `e`（`@{}` ≡ `_\|_`） |
| `List` | 構建 List Combo（numeric keys + `%kind:#list`） |

### 4.3 態射派發

**位置**：`dispatch.rs:dispatch_morphism`

```
dispatch_morphism(morphism, arg):
  1. 從 %rules 提取所有分支
  2. 對每個分支：unify(pattern, arg)
  3. 篩選非 Bottom 結果
  4. 極小元素規則（移除被包含者）
  5. 單一極小 → 執行 body；多個 → Union
```

### 4.4 CAID v2 計算

格式：`hash:sha256:v2:<masa_ref>:<lattice_sketch>:<content_digest>`

```
content_hash(value):
  1. BN/ 序列化（bn_serial.rs）→ 位元組流
  2. lattice_sketch(value)     → Base64（≤16 分量）
  3. SHA256(BN/ bytes)         → content_digest
  4. 組合 → CAID
```

### 4.5 #refine 流程

**位置**：`universe.rs:refine()`

```
refine(source_caids, target_caids, authority, meta):
  Step 1a: 幾何單調性（ID_new ⊑ ID_old）
  Step 1b: verify_refine_authority（Ed25519）
  Step 1c: Shadow scan（向上掃 16 commits）
  Step 1d: BFS Cycle Detection（Phase 15）
  Step 2:  寫入 RefineCommit → ObjectStore
  Step 3:  更新 refine_map + head
```

---

## 5. 內建模組詳解

### 5.1 現有模組清單（Phase 39 後）

| 模組 | 檔案 | 態射數 | EffectTag | 代表功能 |
|:-----|:-----|:------:|:---------:|:---------|
| ~%Math | `math.rs` | 35 | Pure | 算術、超越函數、gcd/log2/factorial/is_prime |
| ~%Complex | `math.rs` | 4 | Pure | conj/phase/real/imag |
| ~%List | `list.rs` | 36 | Pure | 全 FP 操作、group_by/enumerate/sort_by |
| ~%String | `string.rs` | 28 | Pure | 全字串操作、format 命名佔位符 |
| ~%Bytes | `bytes.rs` | 12 | Pure | 二進位 + sha256/base64/hmac |
| ~%Regex | `regex.rs` | 4 | Pure | match/find/replace/split |
| ~%Json | `json.rs` | 4 | Pure | parse/stringify/get/keys |
| ~%Io | `io.rs` | 4 | **IO** | 檔案讀寫、exists、append |
| ~%Env | `env.rs` | 3 | **IO** | get/args/cwd |
| ~%Process | `process.rs` | 2 | **IO** | exit/pid |
| ~%Path | `path.rs` | 5 | Pure | join/dirname/basename/extension/is_absolute |
| ~%Time | `time.rs` | 4 | **IO** | now/format/diff/add_ms |
| ~%Cond | `cond.rs` | 3 | Pure | if/cond/match |
| ~%Reflection | `reflection.rs` | 17 | Pure | 反射、get/set/delete |
| ~%Discovery | `disc.rs` | 6 | **IO** | LADD 發現協議（Phase 38 精確 key 過濾） |
| ~%Engine | `engine.rs` | 10 | Mixed | observe/save/equivalence_map/resolve |
| @option | `engine.rs` | 8 | Pure | Functor + 全組合子 |
| @result | `engine.rs` | 9 | Pure | Functor + 全組合子 |
| @list | — | 1 | Pure | %fmap → list.map |

### 5.2 新增內建模組的標準步驟

1. 建立 `builtins/my_module.rs`（含 `pub fn register_my_builtins(m: &mut HashMap<...>)`）
2. 在 `builtins/mod.rs` 加入 `mod my_module;` + 呼叫
3. 在 `lib.rs:root_with_system()` 建立 `~%MyModule` Combo
   - Pure 模組：`EffectTag::Pure` on morphism ComboVal
   - IO 模組：`EffectTag::IO` on morphism ComboVal（參考 `~%Time /now` 實作）
4. 在 `genesis.rs` 加入 `SEED_MY_MODULE` 常數 + `all_seeds()` 條目
5. 重跑 seed test：`cargo test seed_caids_are_stable -- --nocapture`

### 5.3 系統根（root_with_system）鍵一覽

```
~%Math, ~%List, ~%String, ~%Bytes, ~%Regex, ~%Json, ~%Io,
~%Env, ~%Process, ~%Path,
~%Time, ~%Complex, ~%Cond, ~%Reflection, ~%Discovery,
~%Engine, ~%Official, ~%Config,
@option, @result, @list,
/add（向後相容快捷鍵）
```

### 5.4 List 格式規範

```
List = Combo { "0": v0, "1": v1, ..., "%kind": Tag("list") }
```

- `build_list_value(items: Vec<Value>) → Value`（list.rs 內部函式）
- 外部模組手動建構：同上格式 + `ComboVal::new`

### 5.5 Option / Result 格式規範

```
Some(x)  = Combo { "%val": x }
None     = Tag("none")
Ok(x)    = Combo { "%val": x }
Err(e)   = Combo { "%cause": e }
```

---

## 6. 效果系統

### 6.1 EffectTag 層級（部分有序）

| 標籤 | 值 | 說明 |
|:-----|:---|:-----|
| Pure | 0 | 確定性、無副作用、可快取 |
| State | 1 | 讀寫程式狀態 |
| IO | 2 | 外部 I/O（檔案、網路、時間） |
| NonDet | 3 | 非確定性（math.random） |

EffectTag 在 Value 傳播時取 `max`（最高效果勝出）。

### 6.2 ObservationStrategy（Phase 9）

| 策略 | 資源耗盡 | 奇點 |
|:-----|:---------|:-----|
| `Blur`（預設） | → `Value::Blur` | → `Value::Blur` |
| `Strict` | → `Value::Bottom` | → `Value::Bottom` |
| `Approximate` | → `Value::Blur` | → approximate |

---

## 7. 測試系統

### 7.1 Rust 整合測試（tests/*.rs）

**718 passed / 0 failed / 3 ignored（109 套件）**。主要覆蓋：

- **格論**：unify, complement, oml, orthomodular；Top/Bottom 正規化格律探針
- **CAID**：caid, lattice_sketch_v2（17 固定向量）、genesis 種子穩定性
- **#refine**：refine, authority, shadow
- **Blur**：blur（11 tests）
- **標準庫**：eval, dispatch, cond_match, math_branch, type_constraint ＋ Phase 25–47 各專項
- **LADD**：ladd, nerve_routing, semantic_eclipse, disc_multihop
- **SPEC_14 同步/`$`**：spec14_sync（parser）、context_dollar_test
- **惰性/增量收斂**：stage1–5（fuel、open term、綁定傳播、memo 紅線、Route B 失效）、memo_soundness
- **語義格律探針（工單預置制）**：atom_top_unify、bottom_spelling、cmp_extremes、range_eval（＋parser range_bounds）
- **parser 防回歸**：golden_ast（SYNTAX_01–12 §4 形狀凍結）、fuzz_roundtrip（種子 800×d3）、roundtrip 冪等
- **linter**：oo::nlint（24 tests）

> 慣例：`*_probe_test` / `*_redline_test` 為工單預置的驗收探針轉永久迴歸守衛——
> **不得弱化**；`#[ignore]` 三項為既存已知議題（深 thunk 堆疊、sibling 解析、
> 隔離語境絕對路徑）。

### 7.2 Genesis 穩定性測試

`seed_caids_are_stable`（在 `genesis_test.rs`）：  
驗證所有 `SEED_*` 常數與 `root_with_system()` 當前計算一致。  
每次修改 `root_with_system()` 後必須重跑並更新常數。

```bash
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture
```

---

## 8. 擴展點索引

| 目標 | 檔案 | 關鍵位置 |
|:-----|:-----|:---------|
| 新增語法 | `parser/n.pest`, `parser/ast.rs` | Rule enum, ExprKind enum |
| 新增值類型 | `interpreter/value.rs` | Value enum, AtomKind enum |
| 新增統一化規則 | `interpreter/unify.rs` | `do_unify()` |
| 新增求值邏輯 | `interpreter/eval.rs` | `eval_internal()` |
| 新增正交補規則 | `interpreter/complement.rs` | `orthocomplement()` |
| 新增 BN/ 序列化 | `interpreter/bn_serial.rs` | `serialize_value()` |
| 新增內建函數 | `interpreter/builtins/*.rs` | 新建模組 + `mod.rs` + `lib.rs` + `genesis.rs` |
| 新增型別約束 | `interpreter/type_constraint.rs` | `TypeConstraint enum` |
| 新增 CLI 命令 | `oo/main.rs` | `Commands enum + match arm` |
| 新增靜態檢查 | `oo/static_analyzer.rs` | `StaticViolation enum` |

---

## 9. 與規格書的對應

| 規格章節 | 引擎對應 |
|:---------|:---------|
| SPEC_01（格論基礎） | `value.rs`, `unify.rs`, `complement.rs`, `oml.rs` |
| SPEC_02/SPEC_14（詞法與權威文法） | `parser/n.pest`, `parser/lib.rs`；防回歸＝`golden_ast`/`fuzz_roundtrip`；同步清單＝nlang-spec `meta/ENGINE_SYNC.md` |
| SPEC_06（統一化邏輯；SYNTAX_06 兩家族） | `unify.rs`, `dispatch.rs`, `observation.rs`, `eval.rs:eval_binary_cmp` |
| SPEC_07（邏輯與管道） | `eval.rs:Pipe`＋`dispatch.rs`（Kleisli/疊加分配；`pipe_laws_test`） |
| SPEC_09（標準庫） | `builtins/*.rs`（21 個模組），`type_constraint.rs` |
| SPEC_10（演化與 Commit） | `universe.rs`, `storage.rs`, `authority.rs` |
| SPEC_13（OODP）| `builtins/disc.rs`, `ladd.rs`, `lattice_sketch.rs`, `bn_serial.rs` |
| REAL_03（CAID 協議） | `value.rs:content_hash()`, `bn_serial.rs`, `lattice_sketch.rs` |
| GUIDE_03 §11（Call-by-Observation／增量收斂） | `lib.rs:force/force_memo/dep_collector`, `universe.rs:invalidate_coords`, `parser/tier.rs` |
| APP_05（LADD） | `ladd.rs`, `builtins/disc.rs` |
| SPEC_17（自我演化） | 未實作（規格書 Combo 化前提） |

---

## 10. 快速入門建議

1. 先讀 `value.rs` — 理解 `Value` enum，尤其 `Thunk`（四欄）、`Ref`、`Range`、`Blur`、`Bottom`
2. 再讀 `unify.rs` — 理解 Meet 運算、臂序（Top/Bottom 別名 → Range → do_unify/Union）、H¹/H² obstruction
3. 讀 `lib.rs:force()` — 觀測原語：tier 分類、有效綁定、memo/依賴收集（GUIDE_03 §11 對照）
4. 讀 `eval.rs` — 求值流程與比較兩家族分流
5. 讀 `builtins/json.rs` 或 `builtins/set.rs` — 最清晰的模組實作範本
6. 看 `genesis.rs` + `lib.rs:root_with_system()` — 理解模組系統結構
7. 用 `cargo test --workspace` 驗證（718 passed / 3 ignored）
8. 參考 `docs/worknotes/` — 各功能的工單、驗收記錄與決策背景
