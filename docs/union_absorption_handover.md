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

## 6. 驗收紀錄(驗收方)
