# 工單:兩種 `_`(裸 Top vs 帶因 Top)與診斷成員豁免

**開單**:2026-07-20(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。
**注意**:本弧修訂 §2.4.2(尚未發版,破壞性 #2 條目就地改寫)。

## 1. 法源(裁定 C,2026-07-20;SPEC_01 §2.4.2 補裁 + ERROR_CODES)

`_` 一個拼法扛兩件事,吸收律必須分辨:

- **裸 `_`** = 格頂,使用者明說「任何」——**照吸收**(`9 | _` 即 `_`)。
- **帶因 `_`**(`#static_cycle`、`#no_coordinate`)= **相位標記**
  「此處尚無答案」,是認識論陳述——**診斷成員**。
- **診斷成員豁免(blur + 帶因 Top)三條一體**:
  1. 永不被吸收;2. 永不吸收他支;
  3. **任何深度含診斷成員之值,不得作為吸收方**(它之所以涵蓋
     對方正因某座標未定;吞掉已知=以未知抹除已知)。
- **開放缺欄**(導航至開放 Combo 未定義座標;非繭之 `#missing_key`)
  自此鑄**帶因** Top `#no_coordinate`(ERROR_CODES 已登記)。顯示
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

## 6. 驗收紀錄(驗收方)
