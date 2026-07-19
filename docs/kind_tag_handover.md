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

- [ ] 交付 commit(s):
- [ ] 根因與修法(is-marker 新判準寫明、CAID 位移記錄):
- [ ] 探針/workspace/conformance/語料 四數:
- [ ] 申報事項(範圍外接觸、歧異記錄):

## 6. 驗收紀錄(驗收方填)
