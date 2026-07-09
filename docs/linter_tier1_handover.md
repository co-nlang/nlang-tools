# Handover：n/ Linter Tier 1（純語法／純圖論層）

> 2026-07-07。交接對象：引擎側 agent（冷啟動，假設只有本文件＋兩個 repo）。
> 發案背景：claims_ledger L11 **Path 2** 的前置件（`nlang-spec/meta/claims_ledger.md` 註 N2）
> ＋ discussion/018–019 累積的三條靜態規則。
> 「Tier 1」的意思：**只做 parser 層與圖論層**，不做求值、不做 CAID v2 辛指紋
> （那是 Tier 2，被 SPEC_13 §1.3 擋住）。

---

## 0. 使命（兩件事，一個工具)

做一個 `nlint` 工具（建議直接擴充 `nlang-tools/crates/oo`，它已有 `static_analyzer.rs`
與 main.rs 骨架），輸入 `.n` 檔（或目錄），輸出兩類東西：

1. **靜態規則診斷**（§2）：三條新規則＋既有 SPEC_15 反模式檢測（後者已實作，保留）。
2. **上下文圖與團數報告**（§3）：建 incidence／context graph，算 ω(G)，列 K₄/K₅ 見證。

第 2 類是本案的深層目的：等「規格書 Combo 化」完成後（ROADMAP §3，另案），把規格自身
餵進去量 Ouroboros nerve 的 ω(G)——ω(G) < 4 ⟹ n=4 宣稱對實際迴圈退役為純數學；
存在 K₅ ⟹ n 首次可測。**本案不執行該實驗，只交付能跑它的工具**；驗收語料用
`nlang-tools/tests/`。

## 1. 必讀（按此順序，全部在 repo 內）

1. `nlang-spec/spec/zh_TW/SPEC_07_Logic_and_Pipe.md` §4（管道語義；§4.1 末的 Kleisli 註記）
2. `nlang-spec/spec/zh_TW/SYNTAX_12`（`$` 規則 P1–P5 表）＋ SYNTAX_04 §2.5（tuple 密封）
3. `docs/discussion/018_pipe_kleisli_decomposition.md`（兩箭頭類；§5.2 密封規則）
4. `docs/discussion/019_refinement_nucleus_criterion.md`（三層分類；§2 語法判準——**本案的規格**）
5. `nlang-tools/crates/interpreter/tests/pipe_laws_test.rs`（定律測試＝語義的可執行版）
6. `nlang-spec/meta/claims_ledger.md` 註 N2（Path 2 的判定語義——報告措辭的紅線來源）

## 2. 交付 D1：三條靜態規則

對每個管道出現處（AST `ExprKind::Pipe`），先做**右值形態靜態分類**：

- RHS 是 combo 字面量 → 轉換器形態；
- RHS 是態射字面量（`->`）或 `/` 前綴路徑 → 態射形態；
- RHS 是原子字面量 → 原子形態；
- 其他（一般路徑、call 結果等）→ **Unknown——不分類、不猜**（誠實近似原則：
  靜態層寧可漏報不可誤報）。

### R1｜rerun-safe 標記（Tier C 判定）

轉換器 RHS 的子樹中**無自由 `$`** ⟹ 標 `rerun-safe`（019 命題 2：恆冪等）。

**自由 `$` 的掃描邊界（本案最容易做錯的一點）**：`$` 在**演化邊界**重綁（SPEC_07 P1），
所以掃描**不得下降**進：(a) 巢狀 Pipe 的 RHS 子樹；(b) 態射字面量（`->`）的 body。
例：`{ b: 2 |> { c: $ } }` 對外層管道而言是 $-free（內層 `$` 屬內層管道）。
內插字串 `${...}` **要**下降（P5：內插不建作用域）。

### R2｜tier 分類（C／M／Q／U）

按 019 §2 的語法判準，對轉換器 RHS 分層：

- **C**：$-free（R1 成立）。
- **M（正片段）**：子樹只含——`$`、路徑／Lens 投影、`&`（Meet）、`|`（Join）、
  字面量原子、combo／list／tuple 構造、spread。白名單制。
- **Q**：出現任何白名單外節點——比較（`==`/`!=` 與格家族布林軌都算）、`!`、
  三元、分派表模式鍵、任意態射應用、算術、內插。**保守方向**：不確定就 Q。
- **U**：RHS 形態 Unknown 者整條 U。

輸出附「降層原因」（第一個把它踢出 M 的節點與位置）——這是使用者體驗的關鍵，
也是之後 GUIDE_03 調度要吃的欄位。

### R3｜密封 × 加鍵 ＝ 靜態 ⊥（018 §5.2）

Tier 1 只做**字面量對字面量**（無型別推導）：管道 LHS 是 tuple 字面量或 cocoon
字面量，且 RHS 是轉換器形態，且轉換器的鍵集 ⊄ LHS 字面量的鍵集 ⟹ 診斷
「靜態可判的 `_|_ #missing_key`；建議改態射演化或 `{ ...t }` 顯式拆封
（SYNTAX_12 §4 #7）」。LHS 非字面量一律放行（不推導、不猜）。

## 3. 交付 D2：上下文圖與 ω(G)

**Tier 1 的圖定義**（近似紀律：全部語法層，逐條寫明近似）：

- **上下文（context）** ＝ 每個 combo／cocoon **字面量出現處**（poset `#{}` 不算；
  list/tuple 不算——只有具名座標的容器才是觀測脈絡）。
- **座標（coordinate）** ＝ 欄位鍵的正準字串。Tier 1 近似：能從 AST 靜態串出的
  root-相對路徑就用它，否則用「局部鍵名」並標 `approx: local-key`。
  **同名即同座標是一個已知的過近似**——報告 metadata 必須攜帶此旗標。
- **incidence graph**（雙部圖）：上下文 × 座標，屬於即連邊。原樣輸出（供人工檢查）。
- **context graph** G：頂點＝上下文；邊＝共享 ≥1 座標。
- **ω(G)**：精確算（Bron–Kerbosch＋pivot 即可，語料尺度下毫秒級；不要引重依賴，
  手寫或用 workspace 既有依賴）。
- **輸出**：ω(G)；全部 K₄ 與 K₅ 極大團的**見證清單**（每團：上下文位置＋共享座標集）；
  連通元件數、度分布摘要。

**報告措辭紅線（防火牆，照抄進工具的字串）**：K₄/K₅ 只能稱
**「candidate site（候選位）」**——Tier 1 沒有 q/ω 資料，**不能**宣稱障礙存在；
確認需 Tier 2（CAID v2 辛指紋，SPEC_13 §1.3，另案）。報告尾註固定一行：
「graph facts only; no obstruction claims at Tier 1」。

## 4. 交付 D3：介面

- CLI：`nlint <path>`（檔或目錄，遞迴 `.n`）；`--json` 機器格式（schema 見下）＋
  預設人讀摘要；exit code：0 乾淨／1 有診斷／2 有 R3 級靜態 ⊥。
- JSON schema（穩定欄位，Tier 2 之後只加不改）：
  ```json
  {
    "version": "tier1-v1",
    "diagnostics": [{ "rule": "R1|R2|R3|SPEC15-*", "severity": "info|warn|error",
                      "loc": {"file","span"}, "tier": "C|M|Q|U", "demotion_reason": "…", "msg": "…" }],
    "graph": { "contexts": N, "coordinates": N, "edges": N, "omega": N,
               "k4_witnesses": [...], "k5_witnesses": [...],
               "approximations": ["local-key-identity", "..."] }
  }
  ```

## 5. 驗收標準

1. 對 `nlang-tools/tests/unit/` 全語料跑通（parse 失敗檔跳過並列報）。
2. 三個 fixture（自建，入 `crates/oo/tests/`）＋ golden 輸出：
   - tier fixture：一檔含 C／M／Q 各一的管道，斷言分類與降層原因；
   - R3 fixture：`(1,2) |> { s: $.0 }` 觸發、`{ ...(1,2) } |> { s: $.0 }` 不觸發;
   - ω fixture：手工構造 4 個兩兩共享座標的 combo → 斷言 ω=4 且見證正確。
3. `$`-掃描邊界測試：`{ b: 2 |> { c: $ } }` 判 C（見 §2 R1 的陷阱）。
4. 全 workspace 既有測試不退（cargo test --workspace）。

## 6. 非目標（越界即打回）

- 不做求值／不依賴 interpreter crate 的 eval（parser AST 即全部輸入）。
- 不做型別推導（R3 字面量限定）。
- 不算 H²、不讀 q/ω、不下任何「障礙存在」結論（Tier 2）。
- 不對 n=4 說任何話——那是 Path 2 實驗（另案）拿本工具的輸出去說的。
- 不加語法、不改 parser。

## 7. 之後的接口（記在心裡，不實作）

- **Tier 2**：CAID v2 辛指紋（SPEC_13 §1.3）落地後，同 schema 加 `q`/`omega_symp` 欄位，
  candidate site 升級為可判定。
- **Path 2 實驗**：規格書 Combo 化完成後，`nlint --json spec.n` 的 `graph.omega` 就是
  L11 的判決輸入（ledger N2 的兩個分支）。
- **GUIDE_03 收斂引擎**：吃 R2 的 tier 欄位做增量重算調度（C 跳過／M 迭代／Q 綁觀測點）。

## 8. 回報格式

完成後：提交於 nlang-tools `local` 分支（使用者 merge review），並回填
`nlang-spec/meta/claims_ledger.md` 帳本 todo（「Path 2 首件」打勾）與
ROADMAP §2 審計行。有規格歧義就停下來記進交接回函，不要自行裁決語義。
