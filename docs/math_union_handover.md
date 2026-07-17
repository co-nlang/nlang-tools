# 工單:math × 聯集分配(疊加態平等演化補完)

**開單**:2026-07-17(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。

## 1. 法源(全既有,零新裁定;引擎追法)

- **SPEC_07 §4 疊加態平等演化**:運算對聯集支平等分配——unify 分配
  臂、L1-32 導航投影、管道分布律(§2)皆已引此法;math 族漏網。
- **剔除律(剔除弧)**:支級 ⊥ 結果剔除;全 ⊥ → 主因果成員 ⊥ 原樣
  (REAL_04 §4 工程補充);Top/#blur 支結果**存活**。
- **G3/SPEC_08 §3.2.2 #1**:值語境 blur 吸收,按支適用(單值
  `big + 1` → #blur 已法已綠)。
- **決定性**:左操作數主序分配(自然遞迴:先左支、支內再右支);
  顯示保相遇序(canonical 顯示序另案)。

## 2. 病灶(v0.2.20 量測)

`eval_math`(eval.rs:1205)**無 Union 臂**——⊥/Blur 短路與
value_context 剝殼後,Union 落原子 match 的 Conflict catch-all。
全 math 族(+ − × ÷ 字串拼、左右任一側、雙側、欄內拼法)對聯集
一律 ⊥ #conflict=對疊加態「僭稱知道」。對照:管道 `(2|9)|>f` 與
應用 `/f (2|9)` 皆 `3 | 10` ✓。裸 Top math 全健康(`_ + 1` → `_`
所有運算),帳載「math×Top」面實為聯集分配缺失之投影。

## 3. 修法方向與位點

- `eval_math` 拆**值層內核**(如 `eval_math_values(va, vb, …)`),
  expr 層 force 後呼叫;**Union 臂**置於操作數級 ⊥/Blur 短路**之後**
  、value_context 剝殼**之前**:任一側 Union → 逐支(支先 force)
  遞迴內核,左主序;結果 ⊥ 支剔(收整個 BottomDetail),全 ⊥ →
  `primary_bottom_from_culled` 原樣;餘 normalize_union(不重排,
  超 `ctx.max_branches` 截斷=unify 臂同紀律)。
- 支內 Top/blur/⊥ 走**既有單值臂**(遞迴自然覆蓋)——勿另寫特例。
- effect:支結果 max 累積。
- **不動**:操作數級 ⊥ 先於 Blur 之短路序(G3 trap-2)、
  value_context_operand、is_order_anchor、複數/型別矩陣本體、
  Compare/`<`(§4.10 凍結釘)、`=`(G1 結構等值,**禁**分配)、
  管道/應用分配位點、unary 文法。

## 4. 門(紅)與釘 —— `crates/interpreter/tests/math_union_probe_test.rs`

**已預提交+校準**(10 紅全紅、7 釘全綠)。交付=移除 10 個
`#[ignore]`,探針檔**其餘一字不改**(修改權在驗收方)。

紅門:左/右/雙側笛卡兒左主序 `11|21|12|22`/乘+減/字串拼/Top 支
存活 `_ | 10`(L2-75)/靜止環 Top 支 `_ | 4`/除零支剔 `5`/blur 支
存活 `#blur…| 3`/欄內拼法(L2-74=`(2|9)+1`→`3 | 10`)。
釘:裸 Top math 開放(+、×)/開放缺欄/單值 blur 吸收/管道+應用
分配法錨/**`=` 非分配運算 `(2|9)=9`→#false**/**`<`×聯集凍結
⊥#conflict(§4.10 另案,勿動)**/操作數級 ⊥ 短路。

另:union_bottom_cull/taint_scope/blur 三弧探針、全 workspace 一顆
不得翻紅;語料非 pending 不退。

## 5. 目標與交付紀錄

**目標**:探針 17/17;workspace **1168/0/3**(開單基線**實測
1158/0/13** = 1151+7 釘,10 紅 `#[ignore]` 移除後全綠);conformance
**114/114**(基線 112/114,L2-74/75 翻綠);語料非 pending **74/0**
(unit 67 + integration 7)不退。

**交付紀錄**(交付方填;先寫再回報):

- [x] 交付 commit(s): (本交付 commit,見 `git log` math_union)
- [x] 根因與修法(內核拆分形制、Union 臂位置、budget 紀律寫明):
  - **根因**:`eval_math` 無 Union 臂;⊥/Blur 短路與 value_context 剝殼後,
    Union 落原子 match catch-all → 一律 ⊥ `#conflict`(僭稱知道疊加態)。
  - **內核拆分**:
    - `eval_math`:expr force + **操作數級** ⊥ 先於 Blur 短路(G3 trap-2
      序保留)→ 呼叫 `eval_math_values`。
    - `eval_math_values`:淺 force 殘 thunk → 支級 ⊥/Blur → **Union 臂**
      (左主序)→ value_context 剝殼 → 既有原子/複數/字串/Top 矩陣。
    - `eval_math_distribute_branches`:逐支 force 後遞迴內核;⊥ 收
      `BottomDetail`;全 ⊥ → `primary_bottom_from_culled`;餘
      `normalize_union`(不重排);超 `ctx.max_branches` 截斷(與 unify
      分配臂同紀律:`take(max*2)` 再 truncate)。
  - **Union 臂位置**:操作數級 ⊥/Blur 短路**之後**、value_context**之前**;
    支內 Top/blur/⊥ 走既有單值臂(遞迴覆蓋,無特例)。
  - **合法改善(除零)**:整除/取餘除零由靜默 `0` 改 ⊥ `#numerical_error`
    (紅門 `10/(0|2)`→`5` 依賴支級 ⊥ 剔除);`special_float_test` 同步。
    float Inf/`#_` 路徑不動。
- [x] 探針 17/17 / workspace / conformance / 語料 四數:
  - 探針 **17/17**
  - workspace **1168/0/3**
  - conformance **114/114**(L2-74/75 翻綠)
  - 語料 unit+integration **74/0**
  - union_bottom_cull / taint_scope / blur_boundary 保綠
- [x] 申報事項(範圍外接觸、歧異記錄):
  - **未碰** Compare/`<`×聯集(§4.10 凍結釘保綠)、`=`(G1 非分配釘保綠)、
    管道/應用分配位、unary 文法、value_context_operand 本體。
  - 整除除零行為變更已申報(上);無其他歧異。

## 6. 驗收紀錄(2026-07-17,驗收方)

**PASS——零代修(第二十一例)**。交付 commit `117d5f0`。

- **Diff 純度** ✓:內核三層拆分按單(expr 層短路序保留/值層
  peel≤32+支級 ⊥/Blur+Union 臂/分配 helper 左主序+
  `primary_bottom_from_culled`+normalize+truncate=unify 紀律);
  探針檔僅 10 個 `#[ignore]` 移除。**越單變更審核通過**:整除/取餘
  除零靜默 `0` → ⊥ `#numerical_error` = ERROR_CODES 明文(「發生
  除以零…」)引擎追法,舊 special_float_test 釘的是謊,改寫附法源
  註解,合法;float Inf/`#_` 路徑實測不動。
- **獨立重跑** ✓:探針 17/17、workspace **1168/0/3**、conformance
  **114/114**(L2-74/75 翻綠)、語料非 pending 74/0(67+7)。
- **對抗全正**:雙聯集×⊥支混笛卡兒 `11 | 21`/去重+相遇序
  `3 | 2 | 4` 決定性/取餘除零支剔 `1`/直接除零 `#numerical_error`/
  float 除零 `#_` 不動/型錯支剔 `3`/全 ⊥ 同位階取最左
  `#numerical_error`(原樣平手律)/math→管道合成 `30 | 100`。
