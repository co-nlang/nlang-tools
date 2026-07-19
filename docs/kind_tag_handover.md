# 工單:%kind 標籤統一(B3 — #type 勝出,#type_constraint 退場)

**開單**:2026-07-19(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。

## 1. 法源(%kind 超級衝突裁定 B 全套,2026-07-19;B3=本單)

- SPEC_03 §4 角色表:Type 角 `%kind: #type` = 正典。
- SPEC_05 §3.2 實作現況註(B2):`#type_constraint` 為引擎舊拼,
  裁定退場;約束節點載荷欄=引擎內部表示,不入法。
- 規格書全樹 `type_constraint` 字串僅餘 REAL_01 L224($kind=LSP
  協議 JSON 層,不同命名空間,**不動**)。

## 2. 病灶(v0.2.23 量測)

引擎為 Type 角鑄兩種 `%kind`:stdlib 型別節點(@option/@result,
lib.rs)鑄 `#type`+`%name`;nominal 約束 marker(type_constraint.rs
:60、dispatch.rs:101)鑄 `#type_constraint`+載荷欄。讀取點
type_constraint.rs:244(+ :126 inline)按 `"type_constraint"` 字串
判 marker。用戶可見面:`<<@{ @int }>>` 印 `%kind: #type_constraint`。

## 3. 修法方向與位點

- 兩鑄造點標籤改 `"type"`;**讀取點同步遷移**(:244/:126)——
  is-marker 判準改為 `%kind == #type` **且持載荷欄**(或等效),
  **不得**把 stdlib 型別節點(`%kind: #type`+`%name`,無載荷欄)
  誤判成 marker(釘守 @option)。
- 載荷欄(`%type: "Name"` 拼法)**不改名不隱藏**(B2 內部表示,
  本弧不動;釘守顯示)。
- **CAID 註記**:marker 節點 %kind 標籤變更 → 一次性合法位移,
  交付紀錄須記。
- conformance 不加向量(結構面 expect 會把 B2 不承諾的載荷拼法
  釘成法;矩陣 117 不動)。
- 全樹 grep 已做(新紅線):probes/corpus/conformance 無
  `#type_constraint` 期望字串;工單註解與 fn 名之提及非期望值。

## 4. 門(紅)與釘 —— `crates/interpreter/tests/kind_tag_probe_test.rs`

**已預提交+校準**(2 紅全紅、5 釘全綠)。交付=移除 2 個
`#[ignore]`,探針檔**其餘一字不改**(修改權在驗收方)。

紅門:`<<@{ @int }>>` / `<<@{ @str }>>` 含 `%kind: #type` 且不含
`type_constraint`。
釘:builtin 精化雙面(5&@int/"s"&@int ⊥)/nominal 精化雙面/
stdlib @option 非 marker/載荷顯示不動/@{e} 透明+@{@Pos} 解值。

另:genesis_test(TypeConstraint API)/nominal_ref/cmp_extremes
既有測試一顆不得翻紅;語料非 pending 不退。

## 5. 目標與交付紀錄

**目標**(基線實測 2026-07-19,先量後寫):探針 7/7;workspace
**1213/0/3**(基線 1211/0/5);conformance **117/117** 不動;
語料非 pending **74/0**(unit 67 + integration 7)不退。

**交付紀錄**(交付方填;先寫再回報):

- [x] 交付 commit(s): (本交付 commit,見 `git log` kind_tag)
- [x] 根因與修法(is-marker 新判準寫明、CAID 位移記錄):
  - **根因**:Type 角色雙拼 — marker 鑄 `#type_constraint`,stdlib 鑄
    `#type`;讀取按舊字串判 marker。
  - **鑄造**:`type_constraint::marker_value` + `dispatch` `@Name` 臂 →
    `%kind: #type`(與 SPEC_03 §4 正典一致)。
  - **is-marker 新判準**:`%kind == #type` **且**持載荷欄
    `get_type_constraint_name`(`%type: "…"` 字串)為 Some。stdlib 型別
    節點(`%kind: #type`+`%name`,無載荷)不命中。
  - **CAID 位移**:marker 節點 `%kind` 標籤變更 → 約束 marker 繭 content
    hash 一次性合法位移。
  - 載荷欄 `%type: "Name"` **不改名不隱藏**(B2)。
- [x] 探針/workspace/conformance/語料 四數:
  - 探針 **7/7**
  - workspace **1213/0/3**
  - conformance **117/117**
  - 語料 unit+integration **74/0**
  - genesis / nominal_ref / cmp_extremes / cocoon_shape 保綠
- [x] 申報事項(範圍外接觸、歧異記錄):
  - **未碰** %super/%predicate(B5)、載荷欄改名、`{ @int: … }` 派發拼法。
  - 規格書 REAL_01 L224 `$kind` LSP 層不動(不同命名空間)。

## 6. 驗收紀錄(2026-07-19,驗收方)

**PASS——零代修(第二十三例;協議全淨,無單方遷移)**。交付
commit `0d3aded`。

- **Diff 純度** ✓:兩鑄造點標籤改 `"type"`、is-marker 新判準=
  `%kind == #type` ∧ `get_type_constraint_name` Some(:126 inline
  同遷);引擎全樹 `"type_constraint"` 字串歸零(grep 驗證);
  探針檔僅 2 個 `#[ignore]` 移除。
- **獨立重跑** ✓:探針 7/7、workspace **1213/0/3**、conformance
  **117/117**、語料非 pending 74/0。
- **對抗全正**:marker 結構面 `{{%kind: #type, %type: "int"}}`/
  @option 精化 `#none`/nominal 密封模板 `@T` 執法/雙重精化冪等/
  精化後等值 `#true`。`#ok & @result` ⊥ 同訊息=v0.2.23 worktree
  反事實同形(既有行為,bare tag 非 result 形,非回歸)。
- marker 繭 CAID 一次性合法位移入帳(%kind 標籤變更)。
