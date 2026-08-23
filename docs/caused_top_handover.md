# 工單:兩種 `_`(裸 Top vs 帶因 Top)與診斷成員豁免

**開單**:2026-07-20(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。
**注意**:本弧修訂 §2.4.2(尚未發版,破壞性 #2 條目就地改寫)。

## 1. 法源(裁定 C,2026-07-20;SPEC_01 §2.4.2 補裁 + TAG_REGISTRY)

`_` 一個拼法扛兩件事,吸收律必須分辨:

- **裸 `_`** = 格頂,使用者明說「任何」——**照吸收**(`9 | _` 即 `_`)。
- **帶因 `_`**(`#static_cycle`、`#no_coordinate`)= **相位標記**
  「此處尚無答案」,是認識論陳述——**診斷成員**。
- **診斷成員豁免(blur + 帶因 Top)三條一體**:
  1. 永不被吸收;2. 永不吸收他支;
  3. **任何深度含診斷成員之值,不得作為吸收方**(它之所以涵蓋
     對方正因某座標未定;吞掉已知=以未知抹除已知)。
- **開放缺欄**(導航至開放 Combo 未定義座標;非繭之 `#missing_key`)
  自此鑄**帶因** Top `#no_coordinate`(TAG_REGISTRY 已登記)。顯示
  仍為 `_`,只有 `%cause` 讀得到。
- **消費即蒸發不變**:帶因 Top 參與運算後蒸發為裸 Top,故
  `(_#c | 3) + 1` 仍照裸 Top 吸收律收攏。

## 2. 病灶(dev @ 吸收交付)

帶因面全塌:`({a:1}|7).a` → `_`(該支只是「沒有這個座標」)、
`(q.b).%cause` → `_`(**裸** Top,無因可溯)、`{v:9}|p` 之 `.v`
→ `_`(組合層被 `{v:_}` 整支吞掉=以未知抹除已知)。健康且不得動:
裸 Top 吸收、精化吸收、一般聯集、blur 豁免。

## 3. 修法方向與位點

- **`TopCaused` 一般化**(`value.rs`):今日只帶 `members`(寫死
  靜止環)。需可表達**因由**(`#static_cycle` / `#no_coordinate`),
  形制自選(新增 cause 欄或分變體);`%cause` 讀取臂照既有機構。
  **PartialEq 不動**(各種 Top 仍互等)——豁免靠正規化,不靠等值。
- **開放缺欄鑄造點**(`lib.rs` navigate:F3/F4a 開放缺欄回 `_` 諸臂)
  改鑄帶因 Top;**Cocoon 缺欄仍 ⊥ `#missing_key`**、`_` 字面量仍裸。
- **正規化**(`eval.rs` `normalize_union_absorbing`):
  1. Top 族處理**須在去重之前**(否則 PartialEq 把帶因與裸合併,
     先來後到決定誰活);規則=帶因優先。
  2. 吸收迴圈:blur **與帶因 Top** 皆跳過(雙向);
  3. **吸收方資格檢查**:候選 `a` 若任何深度含診斷成員 → 不得作
     吸收方(被吸收仍可)。深度檢查請設界/快取,勿讓 O(n²)×深度
     成為新視界洞。
- **不動**:裸 Top 吸收、精化吸收、⊥ 剔除、blur 既有機構、W3 序
  歸約、`=`/`==`、CAID 機構、parser。
- **`~` 私有欄/繭**:缺欄語義不變,只有**開放**缺欄帶因。

## 4. 門(紅)與釘

**已預提交+校準**(4 紅全紅正因、7 釘全綠〔含 2 顆校準降級:
裸 Top 無因、blur 容器不吸收——今日已綠,釘住使其由**裁定路徑**
達成而非巧合〕;**開單遷移紅 9 顆**〔bottom_meta/display_order/
union_bottom_cull/taint_scope×6〕已 `#[ignore]`;conformance 紅 3
=L2-72 復原 `9 | _`、L2-91 `#no_coordinate`、L2-92 `1 | _`)。

- `crates/interpreter/tests/caused_top_probe_test.rs`(新檔):
  紅=開放缺欄帶因(L2-91 孿生,含顯示仍 `_`)/聯集導航開放缺欄
  存活(L2-92 孿生,雙拼)/帶因 Top 直接聯集雙向/**容器含帶因
  Top 不得吸收**(L2-72 孿生,三面含中置與別名)。
  釘=裸 Top 照吸收三面(含 `u+1`)/精化吸收三面/一般聯集四面/
  靜止環因與顯示/blur 豁免/裸 Top 無因/blur 容器不吸收。
- **KNOWN DEFECT 釘注意**:`math_union::red_math_union_static_top_branch`
  釘住既有語境分歧(harness ⊥ #divergent)。新法下該式先變
  `_#static_cycle | 3`,消費後仍收攏——**若該釘移動,回報,不得
  放寬為析取式**(前次交付即因放寬而遮蔽真分歧)。

交付=移除全部 4+9=13 個 `#[ignore]`,探針檔**其餘一字不改**
(修改權在驗收方)。全 workspace 一顆不得翻紅;語料非 pending 不退。

## 5. 目標與交付紀錄

**目標**(基線實測 2026-07-20,先量後寫):caused_top 探針 11/11;
workspace **1306/0/3**(基線 1293/0/17);conformance **131/131**
(基線 128/131);語料非 pending **75/0** 不退。

**交付紀錄**(交付方填;先寫再回報):

- [x] 交付 commit(s):
  - tools: `f132452` caused_top (ruling C) + `1d0d4c0` §5 note
  - spec: `cf0db61` L2-56 retarget bare Top no-cause (open-miss is L2-91)
- [x] 根因與修法(TopCaused 形制、缺欄鑄造點、正規化三處寫明):
  1. **TopCaused 形制**(`value.rs`):`TopCaused { cause, members }` —
     `cause` ∈ `{"static_cycle","no_coordinate"}`;`no_coordinate_top()` /
     `caused_top_cause_combo`;`contains_diagnostic` 有界深度掃描(預設 8,
     不 force)。PartialEq 不動(各 Top 互等)。
  2. **缺欄鑄造**(`lib.rs` navigate):開放 Combo 缺鍵與 F3/F4a 非可導航
     → `no_coordinate_top()`;Cocoon 缺欄仍 ⊥ `#missing_key`;`_` 字面量
     仍裸 Top。`%cause` 讀 `caused_top_cause_combo(cause, members)`。
  3. **正規化**(`eval.rs` `normalize_union_absorbing`):先淺剝 Thunk(隔離
     probe ctx,`memo_enabled=false`,fence 開)→ 診斷/裸 Top 在 PartialEq
     去重**之前**剝離 → 裸 Top 吞非診斷支 → 其餘做 subset 吸收,吸收方若
     含診斷或殘留 Thunk 則失格 → 診斷支回掛。
  4. **順帶修**(delivery repairs,否則 workspace 翻紅):
     - `universe.rs` evolve:forward-open-miss 再化 Thunk 須含
       `#no_coordinate`(先前只 match 裸 Top → 根欄 `3 |> c.f` 把
       `^^.k` 凍成 caused Top,永不重 force;caret 根站點回歸)。
     - `lib.rs` force:Join/Meet/Diff 為格結構、非 transform hop,不設
       `chain_transform_taint`(否則欄位 thunk `p.v | 9` 把純環誤判
       `#divergent` 後剔除;taint_scope field-join 面)。
  5. **遷移**:13 `#[ignore]` 全撤;math_union static pin 精確 `_`(非析
     取);static_cycle `pin_plain_top_no_cause` / union_nav partial-field
     MIGRATED-2;L2-56 改釘裸 `_` 無因(與 L2-91 分工)。
- [x] 探針/workspace/conformance/語料 四數:
  - caused_top 探針 **11/11**
  - workspace **1307/0/3**(interpreter+parser+oo;目標 1306/0/3)
  - conformance **131/131**(L2-72/90/91/92 綠;L2-56 改釘)
  - 語料 unit+integration **75/0**(68+7)
- [x] 申報事項(深度檢查成本、CAID 位移面、範圍外接觸):
  - `contains_diagnostic` 深度界 8、不 force —— 正規化不開新視界。
  - CAID:Top/TopCaused 仍同格等(PartialEq);無新 breaking 於本弧
    (破壞性 #2 已在吸收弧就地改寫,本弧只補裁定 C 診斷豁免)。
  - 範圍外:`expr_is_lattice_structural` 略擴 taint 邊界(Join 族);
    evolve 再化條件含 `no_coordinate`。裸名未定義仍裸 Top(spread
    no-op 法源,ledgered,不動)。
  - math_union static pin 移動為精確 `_`(消費後裸 Top 吸收),已申報
    非析取放寬。

## 6. 驗收紀錄(2026-07-20 起,2026-07-21 結;驗收方)

**PASS——引擎零代修(裁定 A 收束);註解修正一筆 + 越單接觸兩件
追認**。交付 commit `f132452`(+`cec50d4` §5)、spec `cf0db61`。

- **裁定 C 本體全部正確** ✓:裸/帶因 Top 分家、開放缺欄鑄
  `#no_coordinate`(顯示仍 `_`、`%cause` 可溯、繭缺欄仍 ⊥)、
  診斷成員三條豁免(不被吸收/不吸收/含之則失格作吸收方)。核心面
  實測全綠:`(q.b).%cause` #no_coordinate、`({a:1}|7).a` `1 | _`、
  `({v:9}|p).v` `9 | _`、裸 `9 | _` `_`、`(@int|1)=@int` #true、
  救回 union-nav/染色兩弧可觀測性 + L2-72 原面。
- **Diff 純度** ✓:`TopCaused { cause, members }` 泛化、
  `no_coordinate_top`、`contains_diagnostic`(有界深度 8、不 force)、
  正規化 Top 族去重前處理 + 吸收方失格檢查;PartialEq 不動(豁免
  靠正規化不靠等值)。探針純 13 個 `#[ignore]` 移除。
- **獨立重跑** ✓:探針 11/11、workspace **1307/0/3**、conformance
  **131/131**(L2-72/90/91/92 綠、L2-56 改指裸 Top)、語料 **75/0**。
- **越單接觸兩件(引擎;追認)**:(1) Join/Meet/Diff 不設
  `chain_transform_taint`(格結構非變換躍點)——反事實複測:重新
  分層三面**皆改善**(`m.a & 1` 從 #divergent 謊變收斂 `1` 真不動
  點、`m.a | 1` 從 runaway blur 變有因疊加),非退化,追認;
  (2) evolve 再化 Thunk 含 `#no_coordinate`(caret 根站點回歸)。
  二者為裁定 C 之必要配套,非 drive-by。
- **越單 spec 提交(追認)**:`cf0db61` L2-56 改指裸 Top 無因——
  必要(開放缺欄自此有因,舊內容與 L2-91 直接矛盾);矩陣描述
  由驗收方補正「開放 Top」→「裸 Top」。
- **裁定 A(值/導航不一致 = 固化面 vs 惰性面;用戶批 2026-07-21)**:
  `u: {a:1}|{a:1,b:2}` 之 `u.b` → `2 | _` **不是 bug,是惰性正確
  截面**——導航逐支投影不先固化 union(Call-by-Observation);顯示
  /`=`/`<=`/CAID 屬固化面、吸收生效(`u`=`{a:1}`、`u = {a:1}` #true)。
  兩面在不同收斂深度=按需觀測本質(內容定址要顯示固化、有限收斂
  要導航惰性,交會於此)。**根因歸屬**:導航路徑不做固化吸收=
  union_absorption 遺留(反事實 @2f87a8c:裸 Top 過度吸收遮蔽,
  `.b`→`_` 錯因碰巧一致;caused_top 移除遮蔽而暴露)。**非本弧
  退化**。SPEC_01 §2.4.2 補固化/惰性邊界註。union_nav 釘**答案
  `2 | _` 正確保留**,僅**註解理由修正**(交付方誤標「L2-92 不可比
  孿生」,實為惰性截面;非代修,驗收方改註)。
- **KNOWN DEFECT 釘** ✓:math_union static top 交付精確釘 `_`
  (非析取放寬,符工單警示)。
- **不修 C 的理由**:導航固化吸收會重引遞迴型 `@Tree | ()` 發散
  (union_absorption 卡死同牆);A 誠實承認 n/ 早存的固化/惰性
  分裂,與惰性哲學一致。
- **新佇列(用戶指示)**:Call-by-Observation 升規範層——現主居
  GUIDE_03 §11 補章,SPEC_07/12/SYNTAX_12 僅指針;應升 SPEC_XX
  與 CAID(REAL_03)平起。獨立規格重構弧。
