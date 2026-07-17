# 工單:聯集惰性 ⊥ 支剔除(G4 收帳)

**開單**:2026-07-17(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。

## 1. 法源(全既有,零新裁定;引擎追法 + 一筆已入法工程補充)

- **SPEC_07 L1-32**:聯集路徑導航=逐支投影(平等演化)。
- **SPEC_08 §3.2.2 #5**:「僅 `_|_` 支剔除」——⊥ 支自聯集結果剔除;
  `#blur` 支**存活**(永久探針 blur_boundary
  `red_union_nav_blur_branch_survives` 看守);Top 支存活(誠實疊加
  `1 | _`)。
- **REAL_04 §4**:聯集全 ⊥ → 單一 ⊥ 攜主因果(五階:#divergent >
  違規類 > 格論族 > 資源族 > #missing_key)。
- **REAL_04 §4 工程補充(2026-07-17 已入法)**:全 ⊥ 坍縮=主因果
  位階最高**成員的 `_|_` 原樣傳出**(訊息/座標/涉入項保全;同位階
  取相遇序最左)——不得改鑄僅存標籤的新 `_|_`。

## 2. 病灶(v0.2.19 量測)與位點提示

剔除律現無單一居所——只住 unify 分配臂(root evolve 路徑,
`(1&2)|5`→`5` ✓)與導航臂**即時** Bottom 比對(cocoon miss ✓)。三漏:

- **T1 導航投影不 force**:`lib.rs` `navigate_segments` 之
  `Value::Union(branches)` 臂,`projected` 是未 force 的欄位 thunk
  (Stage 2),`match Value::Bottom` 接不到——
  `({a:1}|{a:(2&3)}).a` → `1 | ⊥` 兩序、全 ⊥ 雙裸曬、`%cause` 對
  漏網聯集投影成**因果聯集** `#divergent | #conflict`、`u.a = 1` →
  #false。**修法**:投影後 force 至非 Thunk 再比對(淺 force 迴圈
  即可,**勿** force_recursive 整支——combo 支內層欄位保持惰性);
  effect 以 force 後為準。剔除收集**整個 BottomDetail**(非僅
  cause);全 ⊥ → min primary_rank(平手=相遇序最左)之成員 ⊥
  **原樣** `with_effect` 傳出(取代現行 `primary.into()` 標籤鑄)。
- **T2 force_recursive Union 臂不剔**:`lib.rs` force_recursive 之
  `Value::Union` 臂只 normalize——欄內直接 `|` ⊥ 成員觀測面裸曬:
  `{v:(1&2)|5}`.v → `⊥ | 5`(**新面,比帳載寬**)。**修法**:force
  後分流,⊥ 支剔、全 ⊥ 同上原樣主因果,餘走 normalize_union。
- **T3 全 ⊥ 鑄造丟誠實訊息**:`unify.rs` Union 分配臂剔 ⊥ 只留
  nondistrib 旗標,空倖存漏到 `normalize_union` 的
  「empty union after normalize」行話鑄(value.rs)。**修法**:分配
  臂保留被剔 `BottomDetail`,空倖存 → 主因果成員 ⊥ 原樣(**不動**
  排序/cap/nondistrib 邏輯——臂序蟲族雷區,最小 diff);
  `normalize_union` 本體**不改**(value 層無 ctx,空鑄留作其他
  呼叫者的防禦臂)。

倖存序=相遇序**不得重排**(顯示序釘 `2 | 1` 在案)。

## 3. 門(紅)與釘 —— `crates/interpreter/tests/union_bottom_cull_probe_test.rs`

**已預提交+校準**(8 紅全紅、7 釘全綠)。交付=移除 8 個
`#[ignore]`,探針檔**其餘一字不改**(修改權在驗收方)。

紅門:thunk ⊥ 剔除雙序(L2-69)/三支倖存 `5`/全 ⊥ 單一原樣(含
「Incompatible types」訊息保全)/`%cause` 單一 `#divergent`(L2-70,
五階優先)/`= 1` → `#true`/欄內 `|` 觀測出口剔除(L2-71)/root 全
⊥ 原樣訊息(非 normalize 行話)。
釘:`1 | _` 誠實疊加/全 Top 去重 `_`/cocoon miss 即時剔 +
`#missing_key`/混階 `#conflict` 勝 `#missing_key`/二段自癒/root
直接 `|` 剔/聯集交換律 `=`。

另:blur_boundary `red_union_nav_blur_branch_survives`(blur 支存活)、
bottom_meta `1 | _`/`(1|2).a→_`/顯示相遇序釘、eq_thunk 九釘、全
workspace 一顆不得翻紅;語料非 pending 不退。

## 4. 範圍外(碰到=停,不改)

- `normalize_union` 簽名與空鑄防禦臂(value 層保持無 ctx)。
- dispatch/apply/membership/complement 之分配剔除位點(健康,他法)。
- `<`/`<=` × union/blur 序關係(§4.10 帳)。canonical 顯示序(帳)。
- **靜止環×聯集投影語境分歧**(校準曝光另案):`p:{v:p.v}` 於
  `{v:9}|p` 之 `.v`——CLI(evolve 固化)=`9 | _`、harness(惰性
  投影)=`9 | ⊥ #divergent`;SPEC_12 家族裁定候選,兩形皆勿釘勿修;
  若你的修法改變其中一形,**申報量測值即可,不追**。
- 詞法鏈/seal/lexical_forcing 本體。

## 5. 目標與交付紀錄

**目標**:探針 15/15;workspace **1137/0/3**(開單基線**實測
1129/0/11** = 1122+7 釘,8 紅 `#[ignore]` 移除後全綠);conformance
**110/110**(基線 107/110,L2-69~71 翻綠);語料非 pending **74/0**
(unit 67 + integration 7)不退。

**交付紀錄**(交付方填;先寫再回報):

- [x] 交付 commit(s): (本交付 commit,見 `git log` union_cull / lazy bottom)
- [x] 根因與修法(T1/T2/T3 各附量測;force 深度約定寫明):
  - **共用**:`value::primary_bottom_from_culled` — 全 ⊥ 時
    `min_by_key(primary_rank)`(平手=相遇序最左)之 `BottomDetail` **原樣**
    包成 `Value::Bottom`(訊息/path/involved 保全)。
  - **T1 導航**(lib.rs `navigate_segments` Union 臂):投影後 **淺 force
    迴圈**(`while Thunk` → `force`, cap 32;**不** `force_recursive` 整支,
    combo 內層欄位保持惰性);再比對 Bottom。收集完整 `BottomDetail`(非僅
    cause);倖存空 → `primary_bottom_from_culled` + `with_effect`。
    量測:修前 `({a:1}|{a:(2&3)}).a` → `1 | ⊥`;修後 `1`。
  - **T2 觀測出口**(lib.rs `force_recursive` Union 臂):各支
    `force_recursive` 後分流 — ⊥ 剔入 culled、餘 normalize_union;全 ⊥ →
    原樣主因果。量測:修前 `{v:(1&2)|5}.v` → `⊥ | 5`;修後 `5`。
  - **T3 分配臂**(unify.rs Union 分配):剔 ⊥ 時保留 `BottomDetail`;
    `results.is_empty()` → 原樣主因果(**不動** sort/cap/nondistrib 臂序)。
    `normalize_union` 本體**未改**(空鑄防禦仍給其他呼叫者)。
    量測:修前 root `(1&2)|(3&4)` → `empty union after normalize` 行話;
    修後 `#conflict` + `Incompatible types`。
  - 倖存序=相遇序(不重排);Top/blur 支繼續存活。
- [x] 探針 15/15 / workspace / conformance / 語料 四數:
  - 探針 **15/15**
  - workspace **1137/0/3**
  - conformance **110/110**(L2-69~71 翻綠)
  - 語料 unit+integration **74/0**
  - blur_boundary `red_union_nav_blur_branch_survives`、bottom_meta、
    union_nav 保綠
- [x] 申報事項(範圍外接觸、靜止環分歧形若受擾之量測、歧異記錄):
  - **未碰** `normalize_union` 簽名/空鑄、dispatch/apply 分配位、
    `<`/`<=`×union、canonical 顯示序、詞法鏈本體。
  - 靜止環×聯集投影分歧(範圍外):本單未釘未追;未另做 CLI/harness
    對照重測(修法僅在 force/nav/unify 之 ⊥ 剔除,不改 SPEC_12 循環分類)。
