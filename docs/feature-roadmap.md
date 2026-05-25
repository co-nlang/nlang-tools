# nlang 功能路線圖

> 最後更新：2026-05-25（Phase 40 完成後）  
> 開發模式：由 "project brain" AI 出規劃，執行 AI 實作，逐 Phase 交接

---

## 1. 已完成功能（Phase 1–40）

### 核心格論

| 功能 | Phase | 位置 |
|:-----|:-----:|:-----|
| Meet/Join/Complement 基礎 | 1a-1c | `unify.rs`, `complement.rs` |
| H¹ phase obstruction（相位阻礙） | 7 | `unify.rs:make_h1_split_bottom` |
| H² MASA obstruction（互補性違規） | 7 | `unify.rs:make_h2_split_bottom` |
| `approximate_phase_diff` 量子距離（arccos） | 40 | `lattice_sketch.rs:phase_diff_between`, `unify.rs` |
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
| ~%Time | 早期 + 擴展 | 4 | now/format/diff/add_ms |
| ~%Complex | 早期 | 4 | conj/phase/real/imag |
| ~%Cond | 早期 + 14 | 3 | if/cond/match（真實模式匹配） |
| ~%Bytes | 30 + 32 | 12 | 二進位 + sha256/base64/hmac_sha256 |
| ~%Regex | 31 | 4 | match/find/replace/split |
| ~%Json | 33 | 4 | parse/stringify/get/keys |
| ~%Io | 34 | 4 | read_file/write_file/exists/append_file（IO） |
| ~%Env | 36 | 3 | get/args/cwd（IO） |
| ~%Process | 36 | 2 | exit/pid（IO） |
| ~%Path | 37 | 5 | join/dirname/basename/extension/is_absolute（Pure） |

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

## 2. 剩餘 Backlog

### P2：已全部完成（Phase 35–39）✓

### P3：長期目標

| 功能 | 說明 | 規格章節 |
|:-----|:-----|:---------|
| 視界震盪防禦 | #semantic_eclipse | SPEC_13 §7.2 |
| GPP 幾何概率零知識證明 | 基於 Lattice Sketch | APP_05 §5 |
| CIP 因果完整性證明 | 分布式計算委託 | APP_05 §6 |
| SPEC_17 自我演化 | N-1 自舉算法、%promoter | SPEC_17 全章 |
| WASM 舊版引擎模組 | 語義虛擬化掛載 | SPEC_17 §1.4.3 |

---

## 3. 開發速度參考

Phase 1–16：核心格論、CAID、#refine、基礎標準庫（~198 tests）  
Phase 17–24：標準庫大幅擴展（option/result/list/refl 補全、cond.match、eval CLI）→ 274 tests  
Phase 25–34：新模組（Bytes/Regex/Json/Io）+ 字串/數學/列表補全 → ~392 tests  
Phase 35–39：P2 清空（List/Math Round 2、Env/Process/Path 模組、nerve 精確交集、Engine 視圖）→ ~439 tests  
Phase 40：量子相位距離 arccos(Tr(P_A·P_B))（`phase_diff_between`，P3 第一項）→ ~446 tests

每 Phase 平均：3–6 個新 builtin，5–14 個新測試。  
零依賴原則：優先使用已有 dep（serde_json/sha2/ring/base64/hex/regex），無需新增。
