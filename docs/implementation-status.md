# nlang 引擎實作狀態

> 最後更新：2026-05-25（Phase 43 完成後）  
> 測試數量：~466 tests passing（59+ 個測試套件）

---

## 1. 總覽

| 規格章節 | 完整度 | 關鍵剩餘差距 |
|:---------|:------:|:------------|
| SPEC_01（格論基礎） | **95%** | ArithmeticOnAnchor 未在所有算術路徑攔截 |
| SPEC_06（統一化邏輯） | **98%** | `approximate_phase_diff` ✓；視界震盪防禦（P3） |
| SPEC_09（標準庫） | **100%** | — |
| SPEC_10（演化與 Commit） | **99%** | — |
| SPEC_13（OODP） | **80%** | GPP/CIP 零知識證明（P3，研究級） |
| SPEC_17（自我演化） | **0%** | N-1 自舉算法（長期目標） |

---

## 2. SPEC_01 格論基礎

### 已實作 ✓

| 功能 | 位置 |
|:-----|:-----|
| Top `_`（萬有子空間） | `value.rs:Value::Top` |
| Bottom `_\|_`（矛盾） | `value.rs:Value::Bottom(BottomDetail)` |
| Blur `#blur`（視界模糊） | `value.rs:Value::Blur(BlurDetail)` — Phase 9 |
| Meet `&`（格交） | `unify.rs:unify_internal` |
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
| `#_ + 1` → ArithmeticOnAnchor | BottomCause 存在，但未在所有算術路徑攔截 |

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
| `%timeout` → `timeout_deadline` 動態設定 | `lib.rs:eval_context` — Phase 15 |
| `phase_diff_between` 量子距離 arccos(Tr(P_A·P_B)) | `lattice_sketch.rs` — Phase 40 |

### 剩餘 △

| 功能 | 說明 |
|:-----|:-----|
| 剩餘差距暫無（視界震盪防禦已完成 Phase 41） | — |

---

## 4. SPEC_09 標準庫

### 已實作 ✓

#### ~%Math（35 態射）
`/add` `/sub` `/mul` `/div` `/rem` `/abs` `/bits` `/pow` `/sqrt` `/bitAnd` `/bitOr` `/bitXor` `/bitNot` `/shl` `/shr` `/exp` `/ln` `/sin` `/cos` `/eml` `/random` `/min` `/max` `/floor` `/ceil` `/round` `/clamp` `/gcd` `/lcm` `/sign` `/log2` `/log10` `/factorial` `/choose` `/is_prime` `/pow_mod`

特殊：`ln(0)` / `sqrt(-1)` / `eml(0)` → `Value::Blur`（MathSingularity）；`%branch` Riemann 面

#### ~%List（36 態射）
`/map` `/filter` `/fold` `/len` `/concat` `/at` `/sort` `/reverse` `/slice` `/zip` `/flat_map` `/any` `/all` `/find` `/head` `/tail` `/take` `/drop` `/count` `/zip_with` `/partition` `/flatten` `/sum` `/min_by` `/max_by` `/unique` `/range` `/reduce` `/group_by` `/chunk` `/window` `/enumerate` `/sort_by` `/dedup` `/intersperse`

#### ~%String（28 態射）
`/concat` `/split` `/join` `/trim` `/len` `/replace` `/to_lower` `/to_upper` `/starts_with` `/ends_with` `/contains` `/parse_int` `/from_int` `/repeat` `/format`（含命名佔位符）`/char_at` `/chars` `/index_of` `/pad_left` `/pad_right` `/trim_start` `/trim_end` `/reverse` `/count` `/slice` `/is_empty` `/parse_float` `/lines`

#### ~%Bytes（12 態射）
`/from_str` `/to_str` `/len` `/at` `/concat` `/slice` `/to_hex` `/from_hex` `/sha256` `/base64_encode` `/base64_decode` `/hmac_sha256`

#### ~%Regex（4 態射）
`/match` `/find`（char 索引）`/replace`（replace_all，支援捕獲組）`/split`

#### ~%Json（4 態射）
`/parse`（JSON → Value，全型別映射）`/stringify`（Value → JSON）`/get` `/keys`

#### ~%Io（4 態射，IO EffectTag）
`/read_file` `/write_file` `/exists` `/append_file`

#### ~%Env（3 態射，IO EffectTag）
`/get` `/args` `/cwd`

#### ~%Process（2 態射，IO EffectTag）
`/exit` `/pid`

#### ~%Path（5 態射，Pure）
`/join` `/dirname` `/basename` `/extension` `/is_absolute`

#### ~%Query（4 態射）
`/select`（dot-path 導航）`/where`（謂詞過濾，IO）`/pluck`（欄位摘取）`/deep_merge`（遞歸合併）  
共用 helpers：`parse_path`、`get_at_path`（pub，Phase 44 ~%Diff 複用）— Phase 43

#### ~%Time（4 態射，IO EffectTag）
`/now` `/format` `/diff` `/add_ms`

#### ~%Complex（4 態射）
`/conj` `/phase` `/real` `/imag`

#### ~%Cond（3 態射）
`/if` `/cond` `/match`（真實模式匹配）

#### ~%Reflection（17 態射）
`/keys` `/has` `/is_cocoon` `/type_of` `/is_blur` `/is_bottom` `/is_some` `/is_none` `/is_ok` `/is_err` `/to_str` `/bottom_cause` `/get` `/set` `/delete` `/values` `/entries`

#### ~%Discovery（6 態射，IO EffectTag）
`/connect` `/fetch` `/identify` `/identify_and_store` `/advertise` `/find`

#### ~%Engine（10 態射）
`/observe` `/save` `/%differential.{1,2,3}` `/project_down` `/project_up` `/set_strategy` `/check_oml` `/equivalence_map` `/resolve`

#### ~%Official（2 態射，IO EffectTag）
`/sign_refine` `/add_architect`

#### @option（1 %fmap + 7 態射）
`%fmap(option.map)` + `/and_then` `/or` `/unwrap_or` `/filter` `/expect` `/zip` `/flatten`

#### @result（1 %fmap + 1 %map_err + 6 態射）
`%fmap(result.map)` `%map_err` + `/and_then` `/unwrap` `/expect` `/and` `/or` `/flatten`

#### @list（1 %fmap）
`%fmap(list.map)` — Phase 25 genesis seed

#### ~%Config
`%fuel:10000` `%max_branches:64` `%max_depth:256` `%max_pattern_nodes:1024` `%timeout:1000` `%strategy:blur`

---

## 5. SPEC_10 演化與 Commit

### 已實作 ✓

| 功能 | 位置 |
|:-----|:-----|
| `#refine` Commit 類型 | `universe.rs:refine()` |
| 幾何單調性驗證（$ID_{new} \sqsubseteq ID_{old}$） | `universe.rs` step 1a |
| Ed25519 Authority 簽署驗證 | `authority.rs` — Phase 8 |
| `bootstrap_exempt` Epoch 判定 | `universe.rs` — Phase 10 |
| `oo refine` CLI 子命令 | `crates/oo/src/main.rs` — Phase 10 |
| Architects 清單持久化（`.oo/architects.json`） | `storage.rs` — Phase 11 |
| Shadow Refinement（DAG 回溯掃描） | `universe.rs` step 1c — Phase 12 |
| BFS Cycle Detection（環形 DAG 拒絕） | `universe.rs` step 1d — Phase 15 |
| `RefineInfo.shadow_affected` | `value.rs:RefineInfo` — Phase 12 |
| `~%Engine.equivalence_map` 動態視圖 | `builtins/engine.rs` — Phase 39 |
| `~%Engine.resolve` CAID 鏈尾追蹤 | `builtins/engine.rs` — Phase 39 |

---

## 6. SPEC_13 OODP

### 已實作 ✓

| 功能 | 位置 |
|:-----|:-----|
| BN/ 位元流序列化 | `bn_serial.rs`（含 Blur tag 0xFD）— Phase 3/9 |
| Lattice Sketch v2 | `lattice_sketch.rs` — Phase 3/13 |
| CAID v2 格式 | `value.rs:ContentHash` |
| Genesis 種子 CAID（@option, @result, ~%Config, @list + Phase 25–34 新模組） | `genesis.rs` |
| 跨架構穩定性測試 | `lattice_sketch_v2_test.rs` — Phase 13 |
| LADD 引力路由（GBB） | `ladd.rs`, `builtins/disc.rs` — Phase 5/6 |
| nerve_structure MASA（field-key based） | `builtins/disc.rs` — Phase 11 |
| nerve_overlap 前置過濾 | `ladd.rs:nerve_overlap` |
| `d_l_approx` cosine similarity | `ladd.rs` — Phase 16 |
| NerveEntry.field_keys 精確交集（語義 key 過濾） | `builtins/disc.rs` — Phase 38 |
| 視界震盪防禦（SemanticEclipse + blacklist + tiebreaker） | `disc.rs`, `value.rs`, `lib.rs:EvalContext` — Phase 41 |
| `disc.find` 多跳迭代路由（MAX_ROUTING_HOPS = 16） | `disc.rs:multi-hop loop` — Phase 42 |

### 剩餘 △

| 功能 | 說明 |
|:-----|:-----|
| GPP/CIP 零知識證明 | APP_05 §5-6，研究級，P3 |

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
│   ├── TagStart / TagEnd            # 序位錨點
│   └── Bytes(Vec<u8>)              # 二進位資料（Phase 30）
├── Combo(ComboVal)                  # 組合結構
├── Union(Vec<Value>)                # 聯集 A | B
├── Thunk { expr, closure, effect }  # 惰性求值
└── Code(Box<Expr>)                  # 未執行程式碼
```

### 7.2 EffectTag 層級

| 標籤 | 說明 |
|:-----|:-----|
| Pure | 確定性、無副作用、可快取 |
| State | 讀寫程式狀態 |
| IO | 外部 I/O（時間、檔案、網路） |
| NonDet | 非確定性（math.random） |

### 7.3 ComboVal 結構

```rust
pub struct ComboVal {
    data, types, rules, meta, system, local: IndexMap<String, Value>,
    closed: bool, effect: EffectTag,
    relations: Vec<ValRelation>, masa_ref: MasaRef,
}
// 主要 API: get_field(&str) → Option<&Value>
//           insert_field(&str, Value)
//           fields() → IndexMap<String, Value>（clone）
//           fields_iter() → impl Iterator<Item = (&String, &Value)>
```

---

## 8. 測試套件現況（50+ 個測試套件）

### 核心引擎測試

| 測試套件 | 範圍 |
|:---------|:-----|
| `refine_test`, `authority_test` | #refine 流程、Ed25519、shadow |
| `orthomodular_test`, `oml_test` | OML 驗證、Bohrification |
| `lattice_sketch_v2_test` | Sketch 穩定性、跨架構（17 tests） |
| `blur_test` | Value::Blur 傳播（11 tests） |
| `unify_test` | Meet 規則 |
| `type_constraint_test` | @option/@result 驗證 |
| `genesis_test` | 種子 CAID 穩定性 |
| `math_branch_test` | %branch Riemann 面 |
| `ladd_test`, `nerve_routing_test` | LADD/nerve MASA |

### 標準庫測試（Phase 25–39 新增）

| 測試套件 | 覆蓋 |
|:---------|:-----|
| `list_p25_test` | list.unique/range/reduce |
| `str_p25_test` | str.char_at/chars |
| `option_result_p26_test` | option.zip/flatten, result.and/or/flatten |
| `math_p27_test` | math.gcd/lcm/sign/log2/log10 |
| `str_p27_test` | str.index_of/pad_left/pad_right/trim_start/trim_end |
| `list_p28_test` | list.group_by/chunk/window |
| `str_format_p29_test` | str.format 命名佔位符 |
| `bytes_p30_test` | ~%Bytes 全套（8 基礎） |
| `regex_p31_test` | ~%Regex 全套 |
| `bytes_crypto_p32_test` | sha256/base64/hmac |
| `str_p32_test` | str.reverse/count/slice/is_empty/parse_float/lines |
| `json_p33_test` | ~%Json 全套 |
| `io_p34_test` | ~%Io 全套（tempfile 隔離） |
| `list_p35_test` | list.enumerate/sort_by/dedup/intersperse |
| `math_p35_test` | math.factorial/choose/is_prime/pow_mod |
| `env_p36_test` | env.get/args/cwd |
| `process_p36_test` | process.pid（exit 僅驗證已注冊） |
| `path_p37_test` | path.join/dirname/basename/extension/is_absolute |
| `nerve_routing_test`（Phase 38 節）| disc.rs 語義 key 過濾 |
| `engine_p39_test` | engine.equivalence_map / engine.resolve |
| `h1_phase_test` | phase_diff_between、H1Split 觸發、Top-MASA 無回歸 |
| `semantic_eclipse_test` | disc.find blacklist、hop budget、SemanticEclipse 觸發、tiebreaker 確定性 |
| `disc_multihop_test` | 多跳路由、hop counter、store 命中、預算耗盡、空 registry |
| `query_p43_test` | query.select（路徑導航、list index）、query.pluck、query.deep_merge（遞歸）、query.where（空 list）|

**總計：~466 tests, 0 failed**
