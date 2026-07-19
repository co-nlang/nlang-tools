# 工單:態射體內 `^` 綁定(定義側全鏈,SPEC_07 §4.2.3 增訂)

**開單**:2026-07-19(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。

## 1. 法源(裁定 2026-07-19,A 案,已入法)

- **SPEC_07 §4.2.3 增訂**:`^` 是**定義當下的路徑縮寫**(非觀測時
  判讀)——態射體內 `^` 鏈=**定義側容器鏈到底**([持有容器 →
  … → 根宇宙];體視同持有容器內一層,與巢內字面量同構),定義處
  捕獲,**不隨呼叫點改變**;超深 → 觀測 ⊥ `#out_of_horizon`
  (既有法)。呼叫點資料唯一通道=`$`(P1–P5)。三通道各一法:
  裸名=詞法鏈/`$`=動態輸入/`^`=定義側嚴格座標。

## 2. 病灶(v0.2.24 量測=嵌合鏈)

體求值 `^` 鏈=[定義閉包 frames(持有→上溯,**不含 root**)] ++
[**呼叫點**容器鏈(呼叫容器→…→root)]。第 1 跳詞法、第
(frames+1) 跳起洩入呼叫鏈:同字面量 `^^` 於 h 呼叫=9、於 root
呼叫=5;root 持有態射第 1 跳即動態(`^.k` 於 h → 9)。管道與
`/f` 應用同病。九面量測全吻合(工單開單 commit 附測)。

## 3. 修法方向與位點

- 病灶=體求值 ctx 的 `^`-hop 鏈**來源**:frames 之後接的是呼叫者
  scopes。修=frames 耗盡後接**定義側祖先鏈到 root**(定義處捕獲
  ——lexical_forcing/seal frame 機構已捕 frames,補足尾鏈或改
  hop 走定義鏈;實作形自選)。
- **不動**:裸名解析(frames 照舊服務名字解析——只換 `^`-hop
  的鏈源)、`$` 綁定(P1–P5)、欄位 RHS `^` 求值(hops=count+1
  路徑,caret_probe_test 守)、overshoot 鑄 `#out_of_horizon`
  機構(復用)。
- 全樹 grep 已做:probes/corpus/conformance 無態射體 `^` 期望
  (既有 caret 探針全為欄位 RHS 形),零遷移債。

## 4. 門(紅)與釘 —— `crates/interpreter/tests/caret_body_probe_test.rs`

**已預提交+校準**(6 紅全紅、5 釘全綠)。交付=移除 6 個
`#[ignore]`,探針檔**其餘一字不改**(修改權在驗收方)。

紅門:定義側尾鏈 `^^`→root 5(L2-79 孿生)/root 持有 `^`→5
(L2-80 孿生)/同字面量雙呼叫點同值(字面量局部性面)/`^^^`
超深誠實 `#out_of_horizon`/深定義雙 frame 尾鏈/`/f` 應用形。
釘:第 1 跳=持有容器(雙呼叫點)/深定義 `^`=d、`^^`=c/裸名
詞法通道/`$` 動態通道/欄位 RHS `^` 同構錨。

另:caret_probe_test(欄位 RHS 全家)一顆不得翻紅;全 workspace
不退;語料非 pending 不退。

## 5. 目標與交付紀錄

**目標**(基線實測 2026-07-19,先量後寫):探針 11/11;workspace
**1224/0/3**(基線 1218/0/9);conformance **119/119**(基線
117/119,L2-79/80 翻綠);語料非 pending **74/0**(unit 67 +
integration 7)不退。

**交付紀錄**(交付方填;先寫再回報):

- [x] 交付 commit(s): (本交付 commit,見 `git log` caret_body)
- [x] 根因與修法(鏈源改法、捕獲時點寫明):
  - **根因**:`apply_single_rule` 以 `sub_context(ctx)` 繼承**呼叫點**
    scopes,再 `push` 定義 `%closure` frames + 參數 frame → 嵌合鏈
    [呼叫…|定義 frames|param];`^` hop 耗盡 frames 後洩入呼叫容器。
  - **修法**:體求值前 `call_ctx.scopes.clear()`,只裝載定義時捕獲的
    `%closure` frames(態射建立時 `ctx.scopes` 快照,含 seal 持有鏈),
    再 push 參數 frame 作「體當前層」。`^` hop 仍走既有
    `Parent` 臂(`hops=count+1`;`hops==len`→root;`hops>len`→
    `#out_of_horizon`)。`$` 仍經 `context_value`;裸名仍經定義 frames。
  - **捕獲時點**:定義側——`ExprKind::Morphism` 建 `%closure` 時讀
    當下 scopes(欄位 force/seal 已置持有鏈);呼叫點不再改寫 `^` 鏈。
- [x] 探針/workspace/conformance/語料 四數:
  - 探針 **11/11**
  - workspace **1224/0/3**
  - conformance **119/119**(L2-79/80 翻綠)
  - 語料 unit+integration **74/0**
  - caret_probe_test(欄位 RHS) **13/13** 保綠
- [x] 申報事項(範圍外接觸、歧異記錄):
  - **未碰** 欄位 RHS `^` 求值、裸名詞法機構本體、`$` P1–P5、overshoot
    鑄造機構(復用)。

## 6. 驗收紀錄(2026-07-19,驗收方)

**PASS——零代修(第二十四例;協議全淨)**。交付 commit `e6e6331`。

- **Diff 純度** ✓:單點修——`apply_single_rule` 於載入 %closure
  frames 前 `call_ctx.scopes.clear()`(嵌合鏈源頭=sub_context 繼承
  呼叫點 scopes);Parent hop/裸名/`$`/overshoot 機構全復用;探針檔
  僅 6 個 `#[ignore]` 移除。
- **獨立重跑** ✓:探針 11/11、caret RHS 13/13、workspace
  **1224/0/3**、conformance **119/119**(L2-79/80 翻綠)、語料
  74/0。
- **對抗全正**:別名呼叫 `al: c.f` 定義側不變(5)/遞迴 /fact 5
  =120/體內 `^` 與 RHS `^` 雙平面合成(15)/`$`+`^` 同體共存
  (10)/**巢內態射梯全自洽**——inner `^`=外層參數世界
  (`^.n`→1,嚴格 miss 誠實 `_`)、`^^`=c(7)、`^^^`=root(5)
  =字面量局部性逐層成立,外層參數 frame 為合法一層。
