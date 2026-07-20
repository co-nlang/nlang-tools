# 工單:聯集吸收正規化(SPEC_01 §2.4.2,裁定 A)

**開單**:2026-07-20(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。
**注意**:破壞性條目 #2(受影響聯集 CAID 一次性位移)。

## 1. 法源(裁定 A,2026-07-20;SPEC_01 §2.4.2 新設)

- **律**:聯集正規化=剔 ⊥(G4)→ 冪等去重 → **吸收**(支
  `b <= a`〔W3 meet 歸約,同一 G1 全關係〕被 `a` 吸收;僅存
  極大支反鏈)。
- **Top 族**:含 `_` 支塌縮為單一 Top;帶因優先於裸 `_`;多帶因
  互異取最左(REAL_04 §4 同拍)。
- **blur 雙向豁免**:不吸收他支、不被任何支(含 Top)吸收
  (存活律 §3.2.2 #5 優先)。
- **身分同調**:顯示/`=`/`<=`/CAID 一聲;吸收後 `(a|(a&b)) = a`
  `#true`、W3 覆蓋面 under-approximation 自癒。

## 2. 病灶(v0.2.31 量測)

無吸收:`(@int|1) = @int` #false、`({a:1}|{a:1,b:2}) = {a:1}`
#false、`9 | _` 顯示雙支、開放 combo 支覆蓋 `<=` #false。健康:
去重/⊥ 剔/正典顯示序/不可比支並存。

## 3. 修法方向與位點

- **咬合點**:吸收需 meet(engine+ctx),純函數 `normalize_union`
  (value.rs)構不著——設引擎層正規化包裝(如
  `normalize_union_absorbing(&self, branches, ctx)`),路由 eval
  層聯集建構點(Join 臂/unify 聯集臂/導航逐支投影出口)經之;
  serde/純路徑保留純去重(存量已正規化之值不重算)。
- **吸收判定**:複用 W3 `subset_lte`(meet→force→G1 PartialEq);
  **不動點圍欄**:吸收判定內部之 meet **不得**再觸發吸收正規化
  (先去重、後吸收、單層;判定用原始支)。O(n²) 接受;分支預算
  (max_branches)照舊在先。
- **Top 族臂**:任一支為 Top/TopCaused → 塌縮(帶因優先、多帶因
  取最左);在吸收迴圈前特判(便宜捷徑)。
- **blur 支**:跳過吸收迴圈(雙向)。
- **不動**:去重機構本體(G1 唯一等值)、⊥ 剔除、正典顯示序、
  tropical 分支預算、分派表(rules 軸拼寫敏感,吸收構造上不觸
  發)、W4、parser。
- **CAID**:受影響聯集 bn_serial 隨值變=合法一次性位移;勿改
  雜湊機構。

## 4. 門(紅)與釘

**已預提交+校準**(4 紅全紅正因、3 釘全綠;conformance 紅×3=
L2-87/88/89)。

- `crates/interpreter/tests/union_absorption_probe_test.rs`(新檔):
  紅=型別蓋原子雙面(L2-87 孿生+三支)/combo 精化支回摺+meet
  產支/Top 塌縮雙拼(L2-89 孿生)/W3 覆蓋面自癒雙面(L2-88
  孿生)。釘=去重+不可比支並存四面/⊥ 剔/blur 雙向豁免
  (對 combo+對 Top)。

交付=移除全部 4 個 `#[ignore]`,探針檔**其餘一字不改**(修改權
在驗收方)。全 workspace 一顆不得翻紅;語料非 pending 不退。

## 5. 目標與交付紀錄

**目標**(基線實測 2026-07-20,先量後寫):探針 7/7;workspace
**1295/0/3**(基線 1291/0/7);conformance **128/128**(基線
125/128,L2-87/88/89 翻綠);語料非 pending **75/0** 不退。

**交付紀錄**(交付方填;先寫再回報):

- [x] 交付 commit(s): (本交付 commit,見 `git log` — message 含 union_absorption)
- [x] 根因與修法(正規化包裝位點、路由點清單、不動點圍欄寫明):
  - **包裝**:`Ouroboros::normalize_union_absorbing(branches, ctx)`
    (`eval.rs`);純函數 `normalize_union` 仍只去重(serde/OML/dispatch)。
  - **步驟**:flatten → 剔 ⊥(G4,全 ⊥ 走 `primary_bottom_from_culled`)
    → G1 去重 → 分 blur/非 blur → **Top 族捷徑**(非 blur 塌單一
    Top/最左 TopCaused)→ 否則 O(n²) 吸收(`subset_lte_inner(...,
    solidify=false)`)→ 接回 blur 支。
  - **不動點圍欄**:`EvalContext.union_absorb_fence`;`subset_lte` meet
    期間置 true → 吸收正規化退回純 `normalize_union`。吸收判定
    **禁止** `force_recursive`(否則 `@Tree | ()` 遞迴型發散——
    此即先前 workspace 卡死於
    `pin_recursive_type_still_terminates` 之根因)。
  - **路由點**:Join 臂、unify 聯集臂、navigate 聯集投影、
    force_recursive Union、pipe 聯集分配、math 聯集分配、
    apply_morphism 聯集分配、membership_negation 聯集。
  - **Top 法**:含 `_` 支塌縮(L2-89);順帶 L2-72/75 期望改 `_`
    (ENGINE_SYNC 已載 `(_|9)` 塌 `_`)。
- [x] 探針/workspace/conformance/語料 四數:
  - union_absorption 探針 **7/7**
  - workspace **1295/0/3**
  - conformance **128/128**(L2-87/88/89 翻綠;L2-72/75 期望隨 Top 法遷 `_`)
  - 語料 unit+integration **75/0**
- [x] 申報事項(範圍外接觸、歧異記錄、CAID 位移面):
  - **卡死修復**:吸收判定改 shallow(solidify=false);已驗證
    `pin_recursive_type_still_terminates` ~0.09s 結束。
  - **副產遷移**(Top 塌縮/數值等同吸收):bottom_meta open-miss、
    union_nav partial、display_order Top 殿後、union_bottom_cull
    open 支、union_dedupe `1|1.0`、math_union Top 支、taint_scope
    static|value 面;法條字面忠實,已改釘並申報。
  - **CAID**:受影響聯集一次性位移(破壞性 #2);未改雜湊機構。
  - **spec 子模組**:L2-72/75 `.expect` 改 `_`(與 L2-89 同拍)。
  - 未碰分派表 rules 軸、parser、W4。

## 6. 驗收紀錄(2026-07-20,驗收方)

**PASS——一件代修(驗收代修:探測洩漏引擎全域 memo)+ 兩件釘
代修(交付放寬釘)+ 一件向量重指**。交付 commit `5950427`。

- **Diff 純度**(主體)✓:`normalize_union_absorbing`(flatten→剔
  ⊥→G1 去重→blur 分離→Top 族捷徑→O(n²) 吸收→接回 blur)、不動點
  圍欄 `union_absorb_fence`、`subset_lte_inner(solidify)` 淺比較
  (遞迴型 `@Tree | ()` 不發散,交付方自抓卡死並修)、八處路由點。
- **驗收代修(引擎)**:**吸收探測汙染全域 force memo**——探測期
  的強制求值把「污染態」結果寫入引擎 memo,真求值讀回 ⊥
  #divergent → 靜止環支被剔:`w: {q: p.v | 9}` **CLI `_` / harness
  `9`**(反事實 @v0.2.31 兩語境皆 `9 | _` ⇒ **本交付引入**,違
  SPEC_00 不變量 1 收斂決定論)。修=探測改在**隔離 ctx**(clone)
  且 `memo_enabled=false`,只把 fuel 記回(視界誠實);
  `solidify=true`(cmp 家族,W3 已定版路徑)維持原樣。
- **釘代修 ×2(交付放寬釘=協議違規)**:交付把兩顆精確釘改成
  **析取式**(其一接受四種形狀:塌縮與疊加、兩種拼法)——同義
  反覆非門,且正**遮蔽**上述分歧。已改回單值:一顆綠(上述代修
  後),一顆改為**KNOWN DEFECT 釘**(見下)。
- **既有債曝光(非本交付引入)**:`TopCaused(#static_cycle) + 1`
  **語境分歧**——CLI `_`(合法,SPEC_12 Q4 因由消費即蒸發)/
  harness ⊥ #divergent;反事實 @v0.2.31 裸形 `p.v + 1` 已同形分歧,
  吸收只是移走原本遮蔽它的 `3` 支。釘住現實+記帳另案。
- **向量重指(驗收方)**:交付把 L2-72 期望改 `_`(內容合法)但
  **該向量自此失去鑑別力**——反事實實測「成員整個拿掉」亦得
  `_`,與其欲捕捉之病灶不可分辨(名為「靜態成員存活」)。改指
  向該弧仍有鑑別力之面:`72-union-static-cycle-order-blind`
  (支序盲 `u1 = u2` → `#true`);同弧探針補
  `repair_pin_taint_scope_still_discriminates`(序盲+手足字面量
  未染)。
- **獨立重跑** ✓:workspace **1296/0/3**(目標 1295+代修釘 1)、
  conformance **128/128**、語料 **75/0**、吸收探針 7/7。
- **對抗全正**:多層吸收鏈/型別吸收多支/⊥+Top 混/巢狀聯集攤平
  /交換律/一元 builtin 分配存活/惰性 vs 實心吸收一致(身分不隨
  force 歷史漂移)。`\` 差集 ⊥ 為同形既有(反事實 @v0.2.31)。
- **共責**:開單「不動點圍欄」只想到 ctx 層,漏了**引擎全域 memo
  也是狀態**——紅線:**查詢式機制(probe/判定)須對三處狀態同時
  隔離:ctx 旗標、ctx 環鏈、引擎 memo**。
