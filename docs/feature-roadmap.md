# nlang 功能路線圖

> 最後更新：2026-07-12（v0.2.2）  
> 開發模式：由 "project brain" AI 出規劃，執行 AI 實作；Phase 制（1–47）之後轉入
> **規格同步波**（工單＋預置探針驗收制，記錄見 nlang-spec `meta/ENGINE_SYNC.md`
> 與 `docs/worknotes/`）

---

## 1. 已完成功能（Phase 1–47）

### 核心格論

| 功能 | Phase | 位置 |
|:-----|:-----:|:-----|
| Meet/Join/Complement 基礎 | 1a-1c | `unify.rs`, `complement.rs` |
| H¹ phase obstruction（相位阻礙） | 7 | `unify.rs:make_h1_split_bottom` |
| H² MASA obstruction（互補性違規） | 7 | `unify.rs:make_h2_split_bottom` |
| `approximate_phase_diff` 量子距離（arccos） | 40 | `lattice_sketch.rs:phase_diff_between`, `unify.rs` |
| 視界震盪防禦（#semantic_eclipse） | 41 | `disc.rs`, `value.rs:SemanticEclipse`, `lib.rs:EvalContext` |
| `disc.find` 多跳迭代路由 | 42 | `disc.rs`（multi-hop loop + compute_mass/build_query_nerve） |
| Orthomodular Law 驗證 | 7 | `oml.rs` |
| Bohrification Q↔B | 7 | `lib.rs` |
| `Value::Blur(BlurDetail)` 第一類公民 | 9 | `value.rs` |
| Blur 傳播（Blur∧Top/Bottom/X）| 9 | `unify.rs` |

### CAID 基礎設施

| 功能 | Phase | 位置 |
|:-----|:-----:|:-----|
| BN/ 位元流序列化 | 3 | `bn_serial.rs` |
| Lattice Sketch v2 | 3/13 | `lattice_sketch.rs` |
| CAID v2 格式 | 3 | `value.rs` |
| Genesis 種子 CAID | 12/13 | `genesis.rs` |
| 跨架構穩定性測試 | 13 | `lattice_sketch_v2_test.rs` |

### 演化 Commit（#refine）

| 功能 | Phase | 位置 |
|:-----|:-----:|:-----|
| `Universe::refine()` 幾何單調性驗證 | 4 | `universe.rs` |
| Ed25519 Authority 簽署 | 8 | `authority.rs` |
| `bootstrap_exempt` Epoch 判定 | 10 | `universe.rs` |
| `oo refine` CLI 子命令 | 10 | `crates/oo/src/main.rs` |
| Architects 清單持久化（.oo/architects.json） | 11 | `storage.rs` |
| Shadow Refinement（16-commit 回溯掃描） | 12 | `universe.rs` step 1c |
| BFS Cycle Detection（環形 DAG 拒絕） | 15 | `universe.rs` step 1d |

### 標準庫 ~%Math

| 功能 | Phase |
|:-----|:-----:|
| 基礎算術：add/sub/mul/div/rem/abs/pow/bits | 早期 |
| 位元運算：bitAnd/Or/Xor/Not/shl/shr | 早期 |
| 超越函數：exp/ln/sin/cos/sqrt/eml | 早期 |
| NonDet：random | 早期 |
| `ln(0)` → Blur（MathSingularity） | 9 |
| `%branch` Riemann 面（ln/sqrt/eml） | 13/14 |
| min/max/floor/ceil/round/clamp | 19 |
| gcd/lcm/sign/log2/log10 | 27 |
| factorial/choose/is_prime/pow_mod | 35 |
| atan2/hypot/sinh/cosh/tanh/trunc/fract/to_float | 45 |

### 標準庫 ~%List

| 功能 | Phase |
|:-----|:-----:|
| len/at/concat/reverse/slice/zip/sort/map/fold/filter | 早期 |
| flat_map | 17 |
| any/all/find/head/tail/take/drop | 18 |
| count/zip_with | 19 |
| partition/flatten/sum/min_by/max_by | 22 |
| unique/range/reduce；@list genesis seed | 25 |
| group_by/chunk/window | 28 |
| enumerate/sort_by/dedup/intersperse | 35 |
| scan/take_while/drop_while/product/transpose | 45 |

### 標準庫 ~%String

| 功能 | Phase |
|:-----|:-----:|
| concat/split/join/trim/len/replace/to_lower/to_upper/starts_with/ends_with/contains | 早期 |
| parse_int/from_int/repeat | 19 |
| format（位置佔位符 `{}` `{0}`） | 21 |
| char_at/chars | 25 |
| index_of/pad_left/pad_right/trim_start/trim_end | 27 |
| format 命名佔位符 `{name}` | 29 |
| reverse/count/slice/is_empty/parse_float/lines | 32 |
| encode_uri/decode_uri/levenshtein/word_count/title_case | 45 |

### 標準庫 @option / @result

| 功能 | Phase |
|:-----|:-----:|
| option.map / result.map / result.map_err | 15 |
| @option %fmap / @result %fmap %map_err（genesis seed） | 15 |
| option.and_then / result.and_then | 16 |
| option.or/unwrap_or/filter | 17 |
| result.unwrap/expect / option.expect | 18 |
| option.zip/flatten / result.and/or/flatten | 26 |

### 標準庫 ~%Reflection

| 功能 | Phase |
|:-----|:-----:|
| keys/has/is_cocoon/type_of | 早期 |
| is_blur/is_bottom/is_some/is_none/is_ok/is_err/to_str/bottom_cause | 16 |
| get/set/delete/values/entries | 17 |

### 標準庫 新模組

| 模組 | Phase | 態射數 | 說明 |
|:-----|:-----:|:------:|:-----|
| ~%Time | 早期 + 45 | 9 | now/format/diff/add_ms/parse/to_iso8601/add_days/add_hours/weekday |
| ~%Complex | 早期 | 4 | conj/phase/real/imag |
| ~%Cond | 早期 + 14 | 3 | if/cond/match（真實模式匹配） |
| ~%Bytes | 30 + 32 | 12 | 二進位 + sha256/base64/hmac_sha256 |
| ~%Regex | 31 | 4 | match/find/replace/split |
| ~%Json | 33 | 4 | parse/stringify/get/keys |
| ~%Io | 34 | 4 | read_file/write_file/exists/append_file（IO） |
| ~%Env | 36 | 3 | get/args/cwd（IO） |
| ~%Process | 36 | 2 | exit/pid（IO） |
| ~%Path | 37 | 5 | join/dirname/basename/extension/is_absolute（Pure） |
| ~%Query | 43 | 4 | select/where/pluck/deep_merge（巢狀 Combo 讀取） |
| ~%Diff | 44 | 3 | diff/patch/is_compatible（Value 樹差異與修補） |
| ~%Set | 46 | 8 | from_list/union/intersection/difference/is_subset/is_superset/is_disjoint/contains |
| ~%Stat | 46 | 6 | mean/variance/std_dev/median/percentile/histogram |
| ~%Csv | 47 | 4 | parse/parse_with_headers/stringify/read_csv（手寫 RFC 4180） |
| ~%Url | 47 | 5 | parse/encode/decode/join/query_params（url crate） |
| ~%Toml | 47 | 2 | parse/stringify（toml crate） |

### 引擎基礎設施

| 功能 | Phase |
|:-----|:-----:|
| `~%Config` genesis 預設值 | 13 |
| `Ouroboros::eval_context()` | 14 |
| `cond.match` 真實模式匹配 | 14 |
| `%timeout` → `timeout_deadline` 動態設定 | 15 |
| `d_l_approx` cosine similarity | 16 |
| `oo eval` + `oo inspect` CLI 子命令 | 23 |
| NerveEntry.field_keys 精確交集（語義 key 過濾） | 38 |
| `~%Engine.equivalence_map` 動態視圖 | 39 |
| `~%Engine.resolve` CAID 鏈尾追蹤 | 39 |

---

## 1b. 規格同步波（2026-06/07，Phase 制之後）

> 詳表見 `docs/implementation-status.md` §1a；逐案驗收記錄在 nlang-spec
> `meta/ENGINE_SYNC.md` #1–19 與其後之附列。

| 主題 | 狀態 |
|:-----|:-----|
| SPEC_14 權威文法全同步（SYNTAX_01–12 定稿對齊） | ✅ |
| `$` 語義 P1–P5（`#no_context`；超級惰性＝語義要求） | ✅ |
| 惰性引擎 Stage 1–5（call-by-observation → force memo → Route B 每座標失效） | ✅ 增量收斂全線落地 |
| 管道代數律（Kleisli bind、疊加平等演化） | ✅ |
| Parser fuzz／golden-AST 掃描＋EOI 護欄 | ✅ |
| `Atom(Top)`/`Atom(Bottom)` 正規化＋吸收律 | ✅ |
| 比較兩家族極值端（SYNTAX_06 §4.1/§4.2） | ✅ |
| Range／`@{e}` 求值（閉閉區間集合；`Value::Range`） | ✅ |
| Linter Tier 1（`oo lint`：R1/R2/R3＋ω(G)） | ✅ |

## 2. 剩餘 Backlog

### 近期（已立案或裁決明文另案）

| 功能 | 說明 | 出處 |
|:-----|:-----|:-----|
| 步進∩步進區間交集 | 等差數列交集（CRT）；現誠實 ⊥ | Range 工單非目標 |
| Range 子集比較（`1..5 <= 1..10`） | 集合家族 cmp 對 Range 值的自然延伸 | cmp/Range 另案 |
| `<=>` 非嚴格聯集態＋跨容器 ⊥ | 容器歸屬追蹤 | ENGINE_SYNC 附帶事項 |
| `=` 真格論相等（互 `<=`） | 現為非塌縮結構等值 | 同上 |
| `3 <= 5` 數值序 vs 子集語義 | 規格 §4.10 為子集；引擎數值序＝記錄在案的刻意偏離，需獨立裁決＋遷移 | SYNTAX_06 §4.10 |
| field_key path-vs-named 語意 | fuzz 掃描殘留 | ENGINE_SYNC #19 |
| ~~`.n` 語料清理~~ ✅ 2026-07-12 | 11 件舊期望歸零（unit 65/0、integration 7/0、R4 掃描歸零）；曝光缺口 G1 combo 等值／G2 `/` 柯里定義／G4 去重×導航（ENGINE_SYNC 量測） | tests/README.md 缺口清單 |
| cargo-fuzz 外掛 | 可選強化 | 同上 |
| REAL_05 合規測試矩陣 | ✅ 48/48（v0.2.2；含前向引用與聯集冪等向量）——裸核自 v0.2.0 | REAL_05 |

### P3：長期目標

| 功能 | 說明 | 規格章節 |
|:-----|:-----|:---------|
| GPP 幾何概率零知識證明 | 基於 Lattice Sketch | APP_05 §5 |
| CIP 因果完整性證明 | 分布式計算委託 | APP_05 §6 |
| SPEC_17 自我演化 | N-1 自舉算法、%promoter；規格書 Combo 化前提 | SPEC_17 全章 |
| WASM 舊版引擎模組 | 語義虛擬化掛載 | SPEC_17 §1.4.3 |
| Linter Tier 2（真 ω/q） | 需 CAID v2 symplectic fingerprint（SPEC_13 §1.3） | linter_tier1_handover §7 |

---

## 3. 開發速度參考

Phase 1–16：核心格論、CAID、#refine、基礎標準庫（~198 tests）  
Phase 17–24：標準庫大幅擴展（option/result/list/refl 補全、cond.match、eval CLI）→ 274 tests  
Phase 25–34：新模組（Bytes/Regex/Json/Io）+ 字串/數學/列表補全 → ~392 tests  
Phase 35–39：P2 清空（List/Math Round 2、Env/Process/Path 模組、nerve 精確交集、Engine 視圖）→ ~439 tests  
Phase 40：量子相位距離 arccos(Tr(P_A·P_B))（`phase_diff_between`，P3 第一項）→ ~446 tests  
Phase 41：視界震盪防禦（SemanticEclipse + disc.find blacklist/tiebreaker）→ ~452 tests  
Phase 42：disc.find 多跳迭代路由（multi-hop loop + compute_mass/build_query_nerve）→ ~458 tests  
Phase 43：~%Query 模組（select/where/pluck/deep_merge，含 parse_path/get_at_path pub helpers）→ ~466 tests  
Phase 44：~%Diff 模組（diff/patch/is_compatible，set_at_path + collect_diffs 遞歸）→ ~474 tests  
Phase 45：A 組擴充（~%Math +8, ~%List +5, ~%String +5, ~%Time +5）→ ~492 tests  
Phase 46：~%Set（8）+ ~%Stat（6）零 dep 新模組 → ~504 tests  
Phase 47：~%Csv（手寫）+ ~%Url（url crate）+ ~%Toml（toml crate）→ ~514 tests  
語義同步波（2026-06/07）：SPEC_14 全同步 → `$` P1–P5 → 惰性 Stage 1–5＋memo＋Route B
→ fuzz/golden → Top/Bottom 正規化 → cmp 極值 → Range → Range 補完 E1–E3(型別標記×Range/分派鍵/正交補)→ nominal @Name 接線(E4,README 範例端到端)→ L2-17 發散偵測 + ⊥ %cause(裸核收官)→ 前向引用/one-shot 同時性 → Union 冪等去重 + R4 lint → **762 tests（113 套件）**

每 Phase 平均：3–6 個新 builtin，5–14 個新測試。  
零依賴原則：優先使用已有 dep（serde_json/sha2/ring/base64/hex/regex），無需新增。  
語義波起的驗收制：工單附**預置校準紅線探針**（`#[ignore]`，unignore＝驗收）＋活護欄
釘邊界兩側；「根因」宣稱須附量測。範本見 `docs/worknotes/*_handover.md`。
