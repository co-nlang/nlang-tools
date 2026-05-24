# nlang 功能路線圖

> 最後更新：2026-05-24（Phase 14 完成後）  
> 開發模式：由 "project brain" AI 出規劃，執行 AI 實作，逐 Phase 交接

---

## 1. 已完成功能（Phase 1–14）

### 核心格論

| 功能 | Phase | 位置 |
|:-----|:-----:|:-----|
| Meet/Join/Complement 基礎 | 1a-1c | `unify.rs`, `complement.rs` |
| H¹ phase obstruction（相位阻礙） | 7 | `unify.rs:make_h1_split_bottom` |
| H² MASA obstruction（互補性違規） | 7 | `unify.rs:make_h2_split_bottom` |
| Orthomodular Law 驗證 | 7 | `oml.rs` |
| Bohrification Q↔B | 7 | `lib.rs` |
| `Value::Blur(BlurDetail)` 第一類公民 | 9 | `value.rs` |
| Blur 傳播（Blur∧Top/Bottom/X）| 9 | `unify.rs` |

### CAID 基礎設施

| 功能 | Phase | 位置 |
|:-----|:-----:|:-----|
| BN/ 位元流序列化 | 3 | `bn_serial.rs` |
| Lattice Sketch v2（Delta→ZigZag→LEB128→Base64） | 3/13 | `lattice_sketch.rs` |
| CAID v2 格式（`hash:sha256:v2:<masa>:<sketch>:<digest>`） | 3 | `value.rs` |
| Genesis 種子 CAID（@option, @result, ~%Config） | 12/13 | `genesis.rs` |
| 跨架構穩定性測試（5 個固定 EXPECTED_SKETCH_*） | 13 | `lattice_sketch_v2_test.rs` |

### 演化 Commit（#refine）

| 功能 | Phase | 位置 |
|:-----|:-----:|:-----|
| `Universe::refine()` 幾何單調性驗證 | 4 | `universe.rs` |
| Ed25519 Authority 簽署 | 8 | `authority.rs` |
| `bootstrap_exempt` Epoch 判定 | 10 | `universe.rs:117` |
| `oo refine` CLI 子命令 | 10 | `crates/oo/src/main.rs` |
| Architects 清單持久化（.oo/architects.json） | 11 | `storage.rs` |
| Shadow Refinement（16-commit 回溯掃描） | 12 | `universe.rs` step 1c |

### 標準庫

| 功能 | Phase | 位置 |
|:-----|:-----:|:-----|
| ~%Math 全套（含 exp/ln/sin/cos/sqrt/eml） | 2/前期 | `builtins/math.rs` |
| 分支切割 `ln(0)` → Blur | 9 | `builtins/math.rs` |
| `%branch` Riemann 面（ln, sqrt, eml） | 13/14 | `builtins/math.rs` |
| `@option`/`@result` 標準型別 | 12 | `type_constraint.rs` |
| `~%Config` genesis 預設值 | 13 | `lib.rs:root_with_system` |
| `Ouroboros::eval_context()` | 14 | `lib.rs` |
| `cond.match` 真實模式匹配 | 14 | `builtins/cond.rs` |

### LADD / OODP

| 功能 | Phase | 位置 |
|:-----|:-----:|:-----|
| GBB（Geometric Bounding Box）廣告/查詢 | 5/6 | `ladd.rs`, `builtins/disc.rs` |
| 引力路由權重（W = mass / d_L² + ε） | 5/6 | `ladd.rs` |
| nerve_structure MASA（field-key based） | 11 | `builtins/disc.rs:field_key_masa_id` |

---

## 2. 剩餘 Backlog

### P2：有明確設計，下一批實作候選

| 功能 | 說明 | 規格章節 |
|:-----|:-----|:---------|
| `%timeout` → `timeout_deadline` | `eval_context()` 讀取 `%timeout` 設置截止時刻 | SPEC_09 §6 |
| `%fmap`/`%fold` Functor 介面 | List/Option/Result 上的代數元欄位 | SPEC_09 §1 |
| Cycle detection in #refine | Commit DAG 循環阻斷 | SPEC_13 §5.2 |
| Equivalence map 合成 | `~%Engine.equivalence_map` 動態視圖 | SPEC_17 §1.3 |
| 量子距離 `approximate_phase_diff` | `unify.rs` Phase 4 TODO：sketch cosine similarity → arccos → θ | Phase 4 殘留 |
| `/find` 引力導航完整路徑 | disc.find 走完整引力權重篩選 | SPEC_13 §6.2 |
| `nerve_structure` overlapping_masa_caids | 動態計算 MASA 交集 | Phase 11 deferred |

### P3：長期目標

| 功能 | 說明 | 規格章節 |
|:-----|:-----|:---------|
| 視界震盪防禦 | 防語義日蝕攻擊（#semantic_eclipse） | SPEC_13 §7.2 |
| GPP 幾何概率零知識證明 | 基於 Lattice Sketch | APP_05 §5 |
| CIP 因果完整性證明 | 分布式計算委託 | APP_05 §6 |
| SPEC_17 自我演化 | N-1 自舉算法、%promoter、退化封套 | SPEC_17 全章 |
| WASM 舊版引擎模組 | 語義虛擬化掛載 | SPEC_17 §1.4.3 |

---

## 3. 技術依賴圖（剩餘部分）

```
量子距離(arccos) ──→ H¹ Split 真實觸發 ──→ LADD 精確路由
                                              │
                   /find 完整路徑 ───────────→ LADD 引力導航

%timeout 動態設定 ──→ eval_context() 完整

%fmap/%fold ──→ Functor 代數介面

Cycle detection ──→ #refine 安全強化

Equivalence map ──→ SPEC_17 自我演化 (長期)
```

---

## 4. 測試策略

### 現有測試覆蓋

- **格論**：unify, complement, oml, orthomodular（42+ tests）
- **CAID**：lattice_sketch_v2（17 tests，含跨架構固定向量）
- **#refine**：refine_test（13 tests）
- **Blur**：blur_test（11 tests）
- **模式匹配**：cond_match_test（4 tests）
- **LADD**：ladd_test, nerve_routing_test（~20 tests）
- **型別**：type_constraint_test（~10 tests）
- **數學分支**：math_branch_test（~6 tests）
- **Authority**：authority_test（~8 tests）

### 下一批測試需求

- 量子距離：兩個 sketch 的 cosine similarity 數值回歸測試
- `/find` 引力導航：end-to-end 路由結果驗證
- Cycle detection：環形 refine DAG 拒絕測試

---

## 5. 開發速度參考

截至 Phase 14，單次 Phase 平均涵蓋 2–4 個功能模組，每 Phase 新增 3–13 個測試。  
整體 test suite 從 Phase 1（~10 tests）成長到 Phase 14（173 tests）。  
主要架構突破：Phase 9（Blur 第一類），Phase 11（MASA field-key），Phase 13（跨架構穩定性）。
