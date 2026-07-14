# 工單:~%Config 收斂(視界參數劃家追法)(2026-07-13)

**執行者**:模型 #3
**驗收者**:專案腦(探針已預先提交,紅線不可動——**全部**探針檔皆紅線。
若交付中發現任何既有釘因新法必紅:**停下報驗收方**,由驗收方修釘;
單方遷移直接計代修。)
**探針**:
- `crates/interpreter/tests/config_home_probe_test.rs`(7 紅門 + 3 釘)
- `crates/oo/tests/config_hint_lint_probe_test.rs`(R5;3 紅門 + 3 釘)
(皆已校準:紅門今日全紅、釘今日全綠)

**驗收 = 紅門全綠 + 釘全綠 + 全套件無退化(基線 871/0/3 + 本兩檔 16 測)
+ 語料 74/0 + conformance 全綠(含新增 L2-23,交付時應 62/62)。**

---

## 0. 裁定(已批;SPEC_08 §3.1 已入法)

視界屬**觀測行為**:規範家 = 系統軸 **`~%Config`**,欄位**裸名**、
單次觀測全域生效、引擎必實作。節點級 `%fuel` 等 = 參考性提示非硬性
(本引擎不採納 → **R5 lint 警告**,靜默忽略的配置是陷阱)。

## 1. 施工面

1. **欄位裸名化**(lib.rs:820 一帶 genesis + :838 一帶 eval_context 讀):
   `%fuel`→`fuel`、`%timeout`→`timeout`、`%strategy`→`strategy`、
   `%max_branches`→`max_branches`、`%max_depth`→**`max_unification_depth`**
   (對齊 SPEC_09 §6 字典名)、`%max_pattern_nodes`→`max_pattern_nodes`。
   乾淨斷開,不留 `%` 拼法 fallback(語料/測試零依賴已掃;系統軸
   運行時注入,無存檔宇宙依賴;blur CAID 走 HorizonParams 結構,
   不受欄名影響)。
2. **補 `max_lifting_depth`**:genesis 值 32 + eval_context 讀線
   (EvalContext 欄位既有,從未可配置)。
3. **策略三家併一**:
   - `~%Config.strategy` = **初始值**(genesis + eval_context 讀,既有);
   - `/set_strategy` = **運行時 ctx 覆蓋**(保留,語義不變——它改的是
     本次觀測的活參數,與 ~%Config 為初始值不矛盾;函式註解補明);
   - `~%Engine.state.strategy` = **死展示值,移除**(它永遠顯示 #blur,
     ctx 改了也不跟——說謊欄)。移除前 grep 語料/測試引用;若有引用,
     交付紀錄列出並改寫(引用死值的測試本身在測謊言)。
4. **R5 lint**(oo/src/nlint.rs,R4 為樣板):節點欄名 ∈
   {`%fuel`,`%timeout`,`%strategy`,`%max_branches`,`%max_unification_depth`,
   `%max_lifting_depth`,`%max_pattern_nodes`} → Warn,msg 含欄名 +
   「參考性提示,本引擎不採納;觀測參數之家 = ~%Config」指向。
   任意巢深都掃(紅門 `red_r5_nested_hint_warns`);
   **寧漏勿誤**:`%kind`/`%fmap`/`%bind`/`%termination_proof` 等
   真特徵永不誤報(釘)。

## 2. 邊界與陷阱

1. **`~%Config` 的 closed 旗與觀測形**:裸名欄位須可被路徑觀測
   (`~%Config.fuel` → `10000`;今日 `_` 因欄名帶 `%` 被 meta 軸吞)。
   注意 ComboVal 欄位軸分配:裸名入 data 軸,`%` 名入 meta 軸——
   裸名化後自然可導航,勿另開特殊通道。
2. **fuel 佈線活性**:釘 `pin_fuel_wiring_alive_flat_exhaustion_blurs`
   守住 rename 後 eval_context 讀線沒斷(4000 項鏈仍 #blur)。
3. **`/set_strategy` 語義勿動**:它是 State 態射改活 ctx;本單只併
   死展示欄,不改覆蓋行為。
4. **想法 D 記帳**:R5 是第二件 lint 儀器(R4 後);nlint 診斷結構
   照舊(rule/severity/loc/msg),勿改既有規則輸出形。
5. **勿動**:`%config` 使用者覆蓋機制(evolve 進系統軸 = `~%` 影蓋
   靜默案的地盤,另單)、fuel 量級、fmt/CAID、SPEC_09 §6 字典本身。
6. 全語料回歸 + conformance L2-23(今日紅:`_`)。
7. 交付紀錄照舊格式(根因、diff、量測、未動聲明;含
   `~%Engine.state.strategy` 引用掃描結果)。

## 3. 非目標

- 使用者程式內覆蓋 ~%Config(語法/影蓋語義,另案)。
- 節點提示的實際採納(子樹沙箱燃料帽,RFC 地盤)。
- `~%` 系統軸影蓋靜默(另案,勿順手修)。
- oo CLI `--fuel` 旗標之類的介面糖(另議)。

---

## 交付記錄(2026-07-13, implementer)

### 根因 / 修復

| 面 | 根因 | 修復 |
|---|---|---|
| **~%Config 欄名** | `%fuel` 等入 meta 軸 → `~%Config.fuel` 導航 `_` | genesis + `eval_context` 改裸名 data 軸;乾淨斷開無 `%` fallback |
| **字典名** | `%max_depth` ≠ SPEC_09 | → `max_unification_depth` |
| **max_lifting_depth** | EvalContext 有欄位未可配置 | genesis 32 + 讀線 |
| **策略三家** | `~%Engine.state.strategy` 死展示 | 移除該欄(僅留 `differential`);`/set_strategy` 保留並註「活 ctx 覆蓋」 |
| **R5** | 節點 `%fuel` 等靜默忽略 | nlint Warn(七名);combo 內鍵常為 `Path(["%fuel"])` 非 Named+Meta——兩形皆掃 |

### `~%Engine.state.strategy` 引用掃描

語料/測試 **零引用**(僅本 handover/探針註解)。無需改寫。

### 既有期望修正

| 檔 | 調整 |
|----|------|
| `genesis.rs` SEED_CONFIG | 欄名變更 → 新 CAID |
| `genesis_test.rs` | `get_field("fuel")` 等裸名 |

### 未動

- `%config` 使用者覆蓋/`~%` 影蓋、fuel 量級、fmt/CAID、`/set_strategy` 覆蓋行為
- R1–R4 診斷輸出形

### 量測終態

| 項目 | 結果 |
|------|------|
| config_home probes | **10/10** |
| R5 lint probes | **6/6** |
| workspace | **887 過 0 敗 3 ignored**(871 + 16 本探針) |
| conformance | **62/62**(L2-23) |
| 語料 | **74/0** |

nlang-spec 帳:驗收方記。

---

## 驗收紀錄(2026-07-14,驗收方)

**判定:通過——零代修(第九例);無協議違規。**

獨立重測:兩探針檔 diff 僅 10 個 `#[ignore]` 移除、斷言原封;
config_home **10/10**、R5 lint **6/6**;workspace **887/0/3**
(基線 871 + 16 本探針,吻合);語料 **74/0**;conformance
**62/62**(L2-23 關門)。

diff 逐條:genesis 七欄裸名(含 `max_unification_depth` 改名 +
`max_lifting_depth` 新欄 32)、eval_context 讀線同步(timeout 塊
縮排一併歸位,行為不變)、SEED_CONFIG CAID 隨欄名更新(系統軸
運行時注入,無存檔宇宙依賴——工單預裁);`~%Engine.state` 僅留
`differential`,死展示欄移除;`/set_strategy` 僅補註解(bohr_test
既有覆蓋綠);R5 掃描器三鍵形皆接(Named+Meta / Path / Quoted,
交付紀錄之 `Path(["%fuel"])` 發現與紅門綠互證)、遞迴走全表達式形。

對抗性邊界(工單外):`~%Config.%fuel` → `_`(乾淨斷開確認)、
`~%Engine.state.strategy` → `_`、`~%Config` 全形 = 七裸欄 closed;
R5:`%max_branches`(非紅門名)報、態射體內 combo `%fuel` 報、
`%kind`/`%fmap` 靜默。src 無殘留 `%` 配置拼法。

期望遷移審查:`genesis_test.rs` 為一般測試非探針檔,遷移已於交付
紀錄申報——合規(停下報驗收方規則管的是探針檔;本單探針零觸碰)。

模型 #3 檔案:零代修第九例。
