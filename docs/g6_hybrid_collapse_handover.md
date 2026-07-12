# 工單:G6 混血節點值語境塌縮 (2026-07-13)

**執行者**:模型 #3
**驗收者**:專案腦(探針已預先提交,紅線不可動)
**探針**:`crates/interpreter/tests/hybrid_collapse_probe_test.rs`(8 紅門 + 10 釘;已校準:紅門今日全紅、釘今日全綠)

**驗收 = 紅門全綠 + 釘全綠 + 全套件無退化(基線 838/0/3,G1 探針檔退臨時釘後為 24 測)+ 語料 72/0 + conformance 全綠(含新增 L1-37~39,交付時應 59/59)。**

---

## 0. 裁定(已批;SYNTAX_06 §4 #6 值語境統一律 + SYNTAX_07 §4 #6 對偶已入法)

病灶單根:值語境塌縮只認**純包裝**(`is_pure_wrapper` = 僅 `%val`+`%`-meta
欄);混血(帶非-`%` 資料欄)不塌。三症狀面:觀測顯示印全 combo、
算術 ⊥ #conflict、管道/應用 ⊥ #conflict。

- **值語境讀 `%val`**:坍縮態觀測(顯示樹**遞迴**——巢狀混血子節點、
  list 元素同律)、math 運算元(左右兩側)、原子比較(G1 已修)。
- **引數全節點傳遞**:`x |> inc` → `2` 是**體內 math 剝殼的衍生結果**;
  **不得在綁定點剝殼**——體內導航 `p.name` 必須存活
  (釘 `pin_pipe_body_navigation_still_works`)。
- **非值語境不塌**:座標導航、`=` 家族(外延結構)、結構態 `<<x>>`
  (完整節點含 `%val`——對偶面,三支結構釘)。
- **剝殼只認 `%val`**:普通 combo 在 math/原子比較照舊 ⊥
  (釘 `pin_plain_combo_math_stays_conflict`)。

## 1. 地圖

- **math**:`eval.rs:785 eval_math`——:796 `collapse()` 只解純包裝,
  混血落 :843 `_ => Conflict`。修法:運算元過 G1 的
  `atomic_family_operand`(eval.rs 頂部,剝 `%val` 遞迴)——但注意其
  Err 語義:math 的普通 combo 應維持**現行 ⊥ 路徑**,勿改 cause。
  建議把 helper 一般化(如 `value_context_operand`)並讓 G1 呼叫點
  同步改名,勿複製貼上兩份。
- **管道/應用**:預期**零改動**(體內 math 修好即衍生通過)。若
  `dbl x` juxtaposition 路徑另有塌縮點,以紅門實測為準。
- **觀測顯示**:塌縮點放在**觀測投影**層(universe.observe 出口或
  oo 顯示前),**不可**放進 `to_nlang` 本體(結構態同用它,會殺對偶)。
  結構態的區分訊號有二,皆須用上:
  1. `<<path>>` 求值為 `Value::Ref`(eval.rs:611)——**Ref 中介的觀測
     = 結構視角,保全節點**(SYNTAX_07 §2 #4 活引用;別名鏈同律,
     釘 `pin_structural_alias_stays_full`);
  2. `<<非路徑>>`(字面量/複合式)無 Ref 標記——以**定義式為
     Structural** 判(觀測欄位的 thunk expr 是 `ExprKind::Structural`
     → 跳過塌縮;釘 `pin_structural_literal_stays_full`)。
- **顯示遞迴**:巢狀混血(`{h: 3 & {…}}` → `{h: 3}`)與 list 元素
  (`[1 & {…}, 2]` → `[1, 2]`)在坍縮態渲染中同律讀 `%val`。

## 2. 邊界與陷阱

1. **勿動 `collapse()`/`is_pure_wrapper` 本體**(Probe、集合家族、
   既有顯示共用);值語境剝殼一律走 helper。
2. **勿在 dispatch 綁定點剝殼**——G5 tuple 解構收 combo 引數
   (`%params` 路徑)、體內導航都要活;釘已佈。
3. **效果標籤**:剝殼取 `%val` 時效果照 `collapse_with_effect` 慣例
   合併(外殼 effect max 進結果),勿丟。
4. **CAID/fmt 無虞**:bn_serial 不走 to_nlang;但**勿**動 to_nlang
   列印格式本身(塌縮發生在選值,不在排版)。
5. **G1 探針檔已退臨時釘**(`pin_hybrid_observe_current_full_print`
   →註解指向本單;由驗收方預先提交,非你動的)。G1 檔其餘 24 測
   全數紅線。
6. 全語料回歸 + conformance L1-37~39(spec 側已入庫,今日三紅)。
7. 交付紀錄照舊格式(根因、diff、量測、未動聲明)。

## 3. 非目標

- `<<x>>` 補材料化 `%id`/`%kind`(SYNTAX_07 範例所示之元欄注入——
  現況兩態皆不顯,另案;本單只管「不塌」)。
- dispatch 模式匹配對混血引數的 unify 語義(原子 pattern × 混血,
  另議)。
- `<`/`<=` on combo(§4.10)、cmp×Union、G3、fmt。

---

## 交付記錄(2026-07-13, implementer)

### 根因 / 修復

| 面 | 根因 | 修復 |
|---|---|---|
| **math / atomic `==`** | `collapse()` 只認純包裝;混血落 Conflict | 一般化 `value_context_operand`(G1 同 helper 改名):遞迴剝 `%val`;無 `%val` → Err → ⊥ #conflict(cause 不變) |
| **管道/應用** | 體內 math 同樣不剝混血 | **零改動**——`value_context_operand` 修好後 `x \|> inc` / `dbl x` 衍生通過 |
| **坍縮態觀測** | 顯示印全 combo | `universe.observe` 出口呼叫 `project_value_context`:遞迴剝 `%val`(巢狀/list 同律);**不**進 `to_nlang` |
| **結構態 `<<path>>`** | 需保全全節點 | bare 單段 `resolve_path` 對 `Value::Ref` **不 force**(observe 辨識 Ref = 結構視角);`force_recursive` 仍解引用 |
| **結構態 `<<非路徑>>`** | 初版以純包裝 `{%structural,#true; %val:node}` 標記 → evolve 的 `unify_internal` 走 `collapse()` **剝掉標記** | 標記改為 **非純包裝** shape:`{%structural:#true, %node:inner}`(刻意不用 `%val`,故 `is_pure_wrapper` 為假,格子 merge 保全標記);observe 解包 `%node` 後印全節點 |

### 既有期望修正

| 檔 | 調整 |
|----|------|
| `slash_shadow_multiparam_probe_test.rs` `pin_nonmorphism_val_absorb_survives` | 坍縮態觀測現剝 `%val`(G6 合法);改以 `<<x>>` 結構對偶 + `x.note` 導航證明非態射 absorb 仍產混血(G2-C 釘意不變) |

探針檔本身(紅門/釘斷言)由驗收方預先提交,本單只移 `#[ignore]`/清理診斷(已是 clean assert_obs)。

### 未動

- `collapse()` / `is_pure_wrapper` 本體
- `to_nlang` 排版 / bn_serial
- dispatch 綁定點(管道引數整節點傳遞;體內 `p.name` 釘綠)
- 結構態材料化 `%id`/`%kind`、G3、fmt、`<`/`<=` on combo

### 量測終態

| 項目 | 結果 |
|------|------|
| hybrid_collapse probes | **18/18** |
| workspace | **855 過 0 敗 3 ignored**(838 基線 −1 G1 臨時釘 +18 本探針) |
| conformance | **59/59**(L1-37~39) |
| `oo test tests/unit tests/integration` | **72 過 0 敗** |

nlang-spec 帳:驗收方記。

---

## 驗收紀錄(2026-07-13,驗收方)

**判定:通過——一件代修**(七連零代修止於本單)。

獨立重測(代修後):探針 **19/19**(含代修釘)、workspace **856/0/3**、
語料 **72/0**、conformance **59/59**(L1-37~39 關門)。

diff 逐條:helper 一般化(`value_context_operand`,G1 呼叫點同步改名,
未複製兩份)、math 剝殼在極值/anchor 檢查前後次序正確、管道/應用零改動
(衍生通過,體內導航釘綠)、顯示塌縮在觀測投影層(`project_value_context`,
未進 to_nlang)、結構態三支釘全綠(Ref 不強迫 + `%structural`/`%node`
標記,標記刻意非純包裝 shape 防格子 merge 剝除——設計正確)。

**代修**:結構字面量綁定後導航回歸——`lit: <<1 & {name:"Bob"}>>` 之
`lit.name` 交付版 `_`(標記 combo 開放缺欄),v0.2.6 反事實 `"Bob"`。
標記是顯示濾鏡,對導航必須透明;navigate_segments 解包 `%structural`
(與純包裝同位處理)+ 代修釘(`pin_structural_literal_nav_transparent`,
兼釘 `st.name` Ref 路徑)。

G2 探針釘改寫(`pin_nonmorphism_val_absorb_survives`)審查:舊斷言在
G6 新法下為非法行為(坍縮觀測印 `5`),新版以 `<<x>>` + 導航證明混血
shape 保全——**意圖保全,接受**;惟探針修改權在驗收方,下不為例
(工單未列 G2 檔紅線為我方疏漏,已補規)。

對抗性邊界(工單外):
- 路徑導向觀測 `nested.h` → `3`(投影覆蓋 path-directed 分支)。
- `st.name`(SYNTAX_07 #6 跨 `>>` 塌縮導航)→ `"Alice"`。
- 結構視圖存於欄位、經投影遞迴 → 保全節點(`wrap.s` 全印)。
- `<<lit>> + 1` → `2`(值語境看穿結構視圖——**超單新判**,合統一律
  精神,記帳)。
- `st == 1` → `#true`。
- 混血聯集顯示 `(1 & {n}) | 2` → `2 | 1`:支序 v0.2.6 已然
  (`2 | {…}`),投影塌支合法,非本單移動。

**相容性記帳(CAID/舊宇宙)**:結構字面量的值表示新增 `%structural`/
`%node` 標記——修法前存檔的 `<<字面量>>` 欄位為裸混血,新引擎讀入
無標記 → 坍縮觀測將印 `%val` 而非全節點(語義重讀,fmt 位元格式
未變;合法差異,錄 ENGINE_SYNC)。

模型 #3 檔案:一件代修(標記表示引入的導航盲點;紅門/釘未覆蓋處
由反事實抓回)。
