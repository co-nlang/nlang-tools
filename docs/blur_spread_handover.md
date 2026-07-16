# 工單:Blur 展開源吸收(SPEC_03 §3.1 Blur 行)

**開單**:2026-07-16(驗收方)。**基線**:dev @ 本工單 commit(v0.2.16 之後)。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**——上上弧的紀錄缺席
成因是回報當口觸發上下文壓縮;先寫單,壓縮就吃不掉紀錄。

## 1. 裁定(已批 2026-07-16)

**Q1 — Blur 行入異質展開規則**(SPEC_03 §3.1;原列 List/Atom/Top/Bottom,
Blur 缺席):展開=**全座標讀取**;視界後來源的欄位集合不可知 → 目標容器
立即變為**該 `#blur` 原樣**。不得鑄新因、不得靜默 no-op(=視界抹除)。
導出鏈:`{b:1, ...big}` ≡ `{b:1} & unbox(big)`;視界後解封即 `#blur` 本身,
再依既有合併吸收律收官。與 Bottom 條款同構(⊥ 傳因/blur 傳快照)、與
SPEC_08 §3.2.2 #5 座標吸收同律。**序盲**(合併交換律)、**逐節點**(巢內
只吸收該節點,外層照常)、**目標種類無關**(`{}`/`{{}}` 皆吸收)。

**Q2 — 快照原樣性**:吸收結果保全來源 **cause/CAID/視界參數**(§3.2.2 #1
「原樣傳出」字面)——是同一顆快照,不是展開點新鑄。目標既有欄位被吞沒,
循 ⊥ 展開強制坍縮先例;不造「partial 記錄目標」新機器。

**分界(不得越線)**:Top 展開 no-op **不動**——`{x:1, ..._}` → `{x:1}`。
Blur 行與 Top 行的邊界正是「不可知」vs「無約束」。

## 2. 病灶(v0.2.16 量測)

引擎將 `#blur` 展開源路由到 Top 臂(「無效操作」):
- `{b:1, ...big}` → `{b:1}`,`%cause` = `_`(雙序同)
- `{...big}` → `{ }`;巢內 `{a:{...big}}` → `{a:{}}`;`{{...big}}` → `{{}}`
- 鄰居守法:`big & {b:1}` / `{b:1} & big` → `#blur`;⊥ 展開傳因(v0.2.13)

嫌疑位點:spread 求值臂的來源分流(⊥/Top/Atom/List 已各有臂,Blur 落入
Top/default)。修法方向自定;吸收應發生在來源判定處,早於欄位搬運與
目標屬性處理(所以 cocoon 目標自動同綠)。

## 3. 門(紅)與釘 —— `crates/interpreter/tests/blur_spread_probe_test.rs`

**已預提交+校準**(7 紅全紅、6 釘全綠)。交付=移除 7 個 `#[ignore]`,
探針檔**其餘一字不改**(修改權在驗收方)。

紅門:
1. `red_blur_spread_absorbs_cause` — `p.%cause` → `#fuel_exhausted`(L2-57)
2. `red_blur_spread_form` — 結果為 `#blur` 形+fuel cause
3. `red_blur_spread_order_blind` — `{...big, b:1}` 同吸收
4. `red_blur_spread_empty_target` — `{...big}` 同吸收
5. `red_blur_spread_caid_preserved` — `p.%caid == big.%caid` → `#true`(Q2)
6. `red_blur_spread_nested_per_node` — `(w.a).%cause` → `#fuel_exhausted` 且
   外層 `w.%cause` → `_`(逐節點)(L2-58)
7. `red_blur_spread_cocoon_target` — `{{...big}}` 同吸收

釘(全數必須保綠):`pin_top_spread_noop`(L2-59 分界)、
`pin_bottom_spread_collapse`(⊥ 鄰居)、`pin_blur_merge_absorbs`(`&` 鄰居)、
`pin_blur_nav_absorbs`(§3.2.2 #5)、`pin_blur_cause_meta`(配方健全)、
`pin_atom_spread_val`(Atom 鄰居行)。

另:全 workspace 既有測試(含 blur_boundary/blur_horizon/spread_collision/
spread_privacy 各釘)一顆不得翻紅。

## 4. 範圍外(碰到=停,不改)

- **`&`×blur 快照非原樣**:同程式 `big.%caid`、`(big&{b:1}).%caid`、
  `({b:1}&big).%caid` 三 CAID 各異——疑違 §3.2.2 #1,已記帳歸 cause 正典
  審計弧。本單**只要求 spread 路徑**保 CAID;若你的實作順手讓 `&` 也保了,
  屬合法改善但**須在交付紀錄申報**;不得為過門而改 `&` 的既有釘。
- `<`/`<=`×blur(§4.10 凍結釘)、前向引用×spread(凍結釘)、循環展開
  (既有 ⊥ #divergent 釘)——皆非本單變因。
- Blur 展開**至聯集/作二源**等未量測面:遇到歧異先記錄,不擴權裁定。

## 5. 目標與交付紀錄

**目標**:探針 13/13;workspace **1064/0/3**(開單基線 1057/0/10,7 紅
`#[ignore]` 移除後全綠);conformance **98/98**(基線 96/98,L2-57/58 翻綠、
L2-59 保綠);語料非 pending **78/0** 不退。

**交付紀錄**(交付方填;先寫再回報):

- [x] 交付 commit(s): (本交付 commit,見 git log `blur spread`)
- [x] 根因與修法(附量測):
  - **根因**:Combo 字面量 spread 臂對非 Combo/Atom/Top/Bottom 源落入
    `_ => {}` 靜默 no-op;Blur 與 Top 同路「無效操作」,視界抹除。
  - **修法**:spread 源 force 後若 `Value::Blur(bd)` → **立即
    `return Value::Blur(bd)`**(同 Bottom 早退),先於欄位搬運/合併/
    closed 屬性;目標既有欄被吞,快照原樣(cause/CAID/視界參數隨
    `BlurDetail` 走,不新鑄)。Top / TopCaused 仍 no-op。
  - 量測:修前 `{b:1,...big}` → `{b:1}` `%cause _`;修後 `#blur`
    `%cause #fuel_exhausted`,`p.%caid == big.%caid` → `#true`。
- [x] 探針 13/13 / workspace / conformance / 語料 四數:
  - 探針 **13/13**
  - workspace **1064/0/3**
  - conformance **98/98**(L2-57/58 綠,L2-59 保綠)
  - 語料 unit+integration **74/0**(~0.70s;與歷次交付同路徑,無退化)
- [x] 申報事項(範圍外接觸、合法改善、歧異記錄):
  - **未動** `&`×blur CAID 非原樣(仍三異;歸 cause 正典審計弧)。
  - 未碰 `<`/`<=`×blur、前向×spread、循環展開。
  - 未擴權裁定 Blur 展開至聯集/二源。

## 6. 驗收紀錄(2026-07-16,驗收方)

**PASS——零代修(第十五例)**。交付 commit `9aaf425`(§5 勾選框留白處
補釘於此)。

- **Diff 純度** ✓:實作=spread 源 force 後單一 `Value::Blur` 早退臂,
  位於欄位搬運之前(三種目標同臂,cocoon 自動同綠);探針檔僅 7 個
  `#[ignore]` 移除。附帶 `TopCaused` 併入 Top no-op 臂=行為等同
  (原落 `_ => {}` 同為 no-op),純申明性,合規。
- **獨立重跑** ✓:探針 13/13、workspace 1064/0/3、conformance 98/98
  (L2-57/58 翻綠、59 保綠)、語料非 pending 78/0(67+7+2+0+2)。
- **對抗全正**:`{b:1,...big} = {c:2,...big}` → `#true`(吸收×#6a 同
  CAID 決定論合成)、`{..._, ...big}` → blur(Top 讓位)、雙重展開
  CAID 原樣、態射體內展開吸收、cocoon `%cause`/後續導航吸收、巢內
  顯示 `a: #blur{...}` 全節點保因。
- **新曝光(另案記帳)**:二源 ⊥×blur **序依賴**——`{...bot, ...big}`
  → `#conflict`、`{...big, ...bot}` → `#fuel_exhausted`(逐源早退,
  相遇序決定勝者;數學上 ⊥ 為格底,兩序應同答)。無法可依、非本單
  變因;歸 REAL_04 調和/cause 正典審計弧候選。交付方未量此面故無
  申報義務,驗收方補量入帳。
