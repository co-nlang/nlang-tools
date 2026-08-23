# 工單:cause cocoon 調和(REAL_04 §1 重寫 + %type 化石退役 + 鷹架不可見)

**開單**:2026-07-19(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。

## 1. 法源(裁定 2026-07-19,A 案,已入法)

- **REAL_04 §1 重寫**:正典核=`%val`(唯一必備、對偶核);診斷欄
  全部可選、`%`-前綴、依 TAG_REGISTRY 類別變形(%message/%expected/
  %found/%involved/%members);舊表 11 裸名欄(path/line/…)降級
  非規範性附註。
- **%type 廢止**(法 2):設計考古=舊代節點模型殘欄(型別內容曾
  放 %type,後由 SPEC_03 §4 %kind+%super/%predicate 取代);cause
  繭 %type 恆與 %val 同值=化石雙帳。`.%type` 讀法退役:⊥ 上依
  **⊥ 合成性**原樣傳出;#blur 上依**座標吸收律**吸收。SPEC_08
  §3.2.2 #4 已同步(blur meta 觀測僅 %cause/%caid)。
- **鷹架不可見**(法 3):引擎內部墊欄(`_: _` 或後繼機構)
  **不得出現在任何用戶可見投影**(含結構視圖 `<<x>>`);用戶自定
  `_` 欄不受影響(可合法定義,照常顯示)。

## 2. 病灶(v0.2.22 量測)

- `<<e.%cause>>` 印出 `_: _` 墊欄與 `%type` 化石欄(三鑄造點:
  value.rs:811 ⊥ 繭、value.rs:65 static_cycle_cause_combo、blur
  路徑);`e.%type` 走 lib.rs:1589/1608 別名回 tag。
- conformance 8 條向量原用 `.%type` 讀 cause,開單時已改拼
  `.%cause`(全保綠);新 L2-78 紅=門(`e.%type` 應 ⊥ 原樣)。

## 3. 修法方向與位點

- **%type 欄移除**:三個 cause 繭鑄造點刪 `%type` insert。
- **別名退役**:lib.rs:1589/1608 的 `|| seg == "%type"` 刪除;
  lib.rs:1573 一帶(%type 單獨臂)審視同斷。⊥ 上 `.%type` 自然
  落 ⊥ 傳出臂;blur 上自然落吸收臂——**勿寫特例**。
- **墊欄機構**:自選——(a) 墊欄改內部保留名並於一切顯示出口
  剝除,或 (b) 重構掉墊欄需求(cocoon closed 本身防剝?須驗
  G6 值語境塌縮與格合併兩面)。**G6 教訓**:新表示標記須審計
  全消費者(nav/math/cmp/顯示/dedupe)。用戶 `_` 欄與鷹架必須
  可區分(釘守)。
- **不動**:type_constraint.rs/dispatch.rs 的 `%type`(@Name
  約束存放=SPEC_02 §1.2 超級衝突另案,釘守)、%kind 鑄造、
  %cause/%caid 白名單、%val 對偶核、%members。
- **CAID 註記**:cause 繭欄位變動 → 繭 CAID 一次性合法位移,
  交付紀錄須記。

## 4. 門(紅)與釘 —— `crates/interpreter/tests/cocoon_shape_probe_test.rs`

**已預提交+校準**(6 紅全紅、8 釘全綠)。交付=移除 6 個
`#[ignore]`,探針檔**其餘一字不改**(修改權在驗收方)。

紅門:結構視圖無墊欄(保 %val)/無 %type 欄/divergent 繭淨/
static_cycle 繭淨(保 %members)/⊥.%type 原樣傳出/blur.%type
吸收。
釘:%cause 直讀塌 tag/%val 核可讀/%message 可讀/blur %cause
白名單/用戶 `_: 5` 欄保全/二源 ⊥×blur 兩序同答(已癒面照記)/
nominal @Name 執法不動(超級衝突另案圍欄)/#missing_key 類。

另:全 workspace 一顆不得翻紅;語料非 pending 不退;conformance
L2-78 翻綠、8 條改拼向量保綠。

## 5. 目標與交付紀錄

**目標**(基線實測 2026-07-19,先量後寫):探針 14/14;workspace
**1206/0/3**(基線 1200/0/9);conformance **117/117**(基線
116/117,L2-78 翻綠);語料非 pending **74/0**(unit 67 +
integration 7)不退。

**交付紀錄**(交付方填;先寫再回報):

- [x] 交付 commit(s): (本交付 commit,見 `git log` cocoon_shape)
- [x] 根因與修法(墊欄機構選項寫明、CAID 位移記錄):
  - **%type 廢止**:`as_cause_combo`、`static_cycle_cause_combo` 刪
    `%type` insert;lib.rs navigate 刪 ⊥/TopCaused/Blur 上
    `seg == "%type"` 別名臂——⊥ 落 F1 傳出、blur 落 #5 吸收(無特例)。
  - **墊欄(選 a)**:保留 data 軸 `_`→Top 防 pure-wrapper 剝殼(格合併
    語義);`to_nlang` 顯示出口以 `is_engine_scaffold_field` 剝除
    (`key=="_"` 且值為 Top/TopCaused)。用戶 `_: 5` 等非 Top 值照印。
  - **CAID 位移**:cause 繭移除 `%type` 欄 → 繭 content hash 一次性合法
    位移(預期);墊欄仍參與內部 hash(僅顯示不可見)。
- [x] 探針/workspace/conformance/語料 四數:
  - 探針 **14/14**
  - workspace **1206/0/3**
  - conformance **117/117**(L2-78 翻綠)
  - 語料 unit+integration **74/0**
- [x] 申報事項(範圍外接觸、歧異記錄):
  - 遷移舊 `%type` 別名期望(新法合法):bottom_meta×3、cocoon_eigenstate
    ×1、private_axis×2、blur_horizon L2-21 改讀 `%cause`;語料
    `tests/unit/test_causal_chain.n` 三處 `.%type`→`.%cause`。
  - **未碰** type_constraint/dispatch 之 @Name `%type` 存放、%kind 鑄造、
    %cause/%caid 白名單、%val/%members 核。

## 6. 驗收紀錄(2026-07-19,驗收方)

**PASS——代碼零修;協議代修一筆(第二連,見下)**。交付 commit
`e337095`。

- **Diff 純度** ✓:別名臂乾淨移除(⊥ 自然落 F1 傳出、blur 自然落
  #5 吸收、TopCaused %type 臂刪、白名單縮 %cause/%caid)、兩鑄造點
  刪 %type、`is_engine_scaffold_field`(key=="_" 且值 Top/TopCaused)
  於 to_nlang 雙欄位迴圈剝除;探針檔僅 6 個 `#[ignore]` 移除。
- **獨立重跑** ✓:探針 14/14、workspace **1206/0/3**、conformance
  **117/117**(L2-78 翻綠)、語料非 pending 74/0。
- **對抗全正**:conflict 繭全形(%expected/%found/%involved/
  %message/%val,無 %type 無墊欄)、missing_key 繭類變形(%message/
  %path/%val)、`{_: 5}` 照印、`{_: _, a: 1}` 隱 `_` 欄、
  `{_: _} = {}` → `#false`(語義保欄)。
- **角落記帳**:用戶顯式 `_: _` 欄與鷹架**表示同一不可區分**——
  顯示隱藏、語義面(eq/merge)完整保全;法 3「照常顯示」對此角落
  之字面不可滿足(同表示無從分辨),記錄為已知角落非違規。
- **協議記帳**:交付方單方遷移 probe 樹 6 處+語料 3 處 `.%type`
  期望(內容全數=法定後果忠實改寫、申報誠實)→ 預告條款續用,
  **計協議代修一筆(第二連)**。共責:驗收方開單掃描只掃
  conformance 未掃 probe 樹+語料之退役拼法=漏遷第二型再犯。
  **新紅線:退役拼法/別名開單時全樹 grep(probes+corpus+
  conformance 三家)**。
- **編輯性補筆**:REAL_04 §1 示例列補 `%path`(missing_key 類實鑄,
  %-前綴合法;驗收方閉環時補)。
