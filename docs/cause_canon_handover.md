# 工單:cause 正典審計(引擎追法三件)

**開單**:2026-07-17(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。

## 1. 裁定(已批 2026-07-17;REAL_04 §2/§4、ERROR_CODES 正典地位)

法典側(REAL_04 類別法重寫、§4 優先級五階、ERROR_CODES 正典登記簿)由
驗收方完成,**不在本單**。本單=引擎追既有法三件,**零新裁定**:

**T1 — 二源展開 blur 摺疊律**:spread 之 blur 早退臂(blur_spread 弧交付)
跳過剩餘源——`{...big, ...bot}` 回 `#fuel_exhausted`,依導出鏈
`{t} & unbox(s1) & unbox(s2)` 與既有 unify 臂 `Blur×Bottom = Bottom`
(雙向)應為 ⊥ `#conflict`。修法:blur 源**不早退**,對剩餘源/欄位繼續
unify 摺疊(⊥ 早退**合法保留**——⊥ 吸收一切含 blur)。單源 blur 吸收
語義不變(釘住)。

**T2 — 效果預測誠實**:`predict_effect`(eval.rs ~209)一刀切
`first.starts_with("~%") → EffectTag::IO`,謊報純態射——SPEC_09 §4
效果表:`~%Math` 等純、`~%Env`/`~%repl` 等真 IO/state。修法:拆毯,
走既有查找鏈讀**實際儲存效果**(root_with_system 之模組/態射自帶標籤)。
真 IO 模組(env_p36 等測試)不得誤純化——執行側鑄造已正確,勿動。

**T3 — `#invalid_path` 末活鑄點退役**:`lib.rs` `follow_refine` 之
`ContentHash::parse(...).map_err(|_| BottomCause::InvalidPath)` 是 F4
廢止後全引擎**最後一個**活鑄點(⊥-meta 整流弧承諾 minting stops)。
修法:改鑄 `BottomCause::Conflict`;同時 `get_live_value` 上游錯誤訊息
「Refinement cycle detected」對 parse 失敗是謊——分流誠實訊息。
交付後 `BottomCause::InvalidPath` 鑄造點 = 0(serde 解碼讀取除外),
驗收方將 grep 確認。

## 2. 病灶(v0.2.17 量測)

- `{...big, ...bot}` → `#fuel_exhausted`;`{...big, x:1, ...bot}` 同
  (bot: 1&2 先定義;⊥ 在前 `#conflict` 正確)。
- `c: { v: ~%Math.abs (0 - 3) }` → `3 ;; %effect: #io`(apply 與 pipe
  兩拼法同);root 級同式乾淨(僅 combo 欄位建構走 predict_effect)。
- 反事實已做:幻影 #io 為既有債(系統軸弧交付前同形)。
- 注意:`caid_recheck` 形(bot 定義於觀測**之後**)之 `{...bot,...big}`
  回 `#fuel_exhausted` 屬**前向引用×spread 凍結案**交互——非本單變因,
  碰到記錄勿修。

## 3. 門(紅)與釘 —— `crates/interpreter/tests/cause_canon_probe_test.rs`

**已預提交+校準**(4 紅全紅、5 釘全綠)。交付=移除 4 個 `#[ignore]`,
探針檔**其餘一字不改**(修改權在驗收方)。

紅門:
1. `red_two_source_bottom_after_blur` — `({...big, ...bot}).%cause` →
   `#conflict`(L2-63)
2. `red_blur_fold_continues_through_fields` — `({...big, x:1, ...bot})` 同
3. `red_combo_system_apply_effect_clean` — `c.v` → `3` 淨(L2-64)
4. `red_combo_system_pipe_effect_clean` — pipe 拼法同淨

釘(全數保綠):`pin_two_source_bottom_first`(⊥ 早退合法)、
`pin_blur_merge_caid_verbatim`(L2-65,`&`×blur CAID 原樣雙序)、
`pin_unify_blur_bottom_both_orders`(T1 依賴的既有 unify 臂)、
`pin_root_system_use_clean`(root 面不得過矯)、
`pin_single_source_blur_absorb_still`(blur_spread 弧法不退)。

另:blur_spread/system_axis/spread_collision 全釘、effect_flow/env_p36
(真 IO 執行側)、全 workspace 一顆不得翻紅。

## 4. 範圍外(碰到=停,不改)

- 前向引用×spread(凍結)、`<`/`<=`×blur(§4.10)。
- REAL_04/ERROR_CODES 文本(驗收方已完成)。
- Blur×Blur 二源合併語義(unify 既有臂,未在本單重審)。
- `%effect` 顯示格式本身(`;; %effect:` 疣另案)。

## 5. 目標與交付紀錄

**目標**:探針 9/9;workspace **1087/0/3**(開單基線**實測 1083/0/7**,
4 紅 `#[ignore]` 移除後全綠);conformance **104/104**(基線 102/104,
L2-63/64 翻綠、65 保綠);語料非 pending **78/0** 不退;
`InvalidPath` 活鑄點 = 0。

**交付紀錄**(交付方填;先寫再回報):

- [x] 交付 commit(s): (本交付 commit,見 `git log` cause_canon)
- [x] 根因與修法(附量測):
  - **T1 根因**:Combo spread 臂對 `Value::Blur` **早退** → `{...big, ...bot}`
    在 big 處回 `#fuel_exhausted`,跳過 bot;導出鏈
    `{t} & unbox(s1) & unbox(s2)` 與既有 unify `Blur×Bottom = Bottom`(雙向)
    應為 ⊥ `#conflict`。
  - **T1 修法**:blur 源改為 `blur_absorb` 累積(Blur×Blur 走既有 unify 臂),
    **不早退**、繼續摺疊剩餘源/欄位;⊥ 早退**保留**;迴圈結束若仍持有
    blur absorb → 回該 `#blur` 快照(單源 blur 吸收法釘住)。
  - **T2 根因**:`predict_effect` Path 臂一刀切 `first.starts_with("~%") → IO`,
    combo 欄位 thunk 標幻影 `#io`;force 再把預測效果抬到結果上
    (`3  ;; %effect: #io`)。
  - **T2 修法**:拆毯;scopes/root/staged 查找首段後**沿剩餘段靜態走欄位**
    (`seg` / `/seg` / `@seg`),讀葉值 `effect()`——`~%Math.abs` Pure、
    真 IO 態射(env 等)仍 IO。執行側鑄造未動。
  - **T3 根因**:`follow_refine` `ContentHash::parse` 失敗鑄 `InvalidPath`
    (F4 後末活鑄點);`get_live_value` 一律「Refinement cycle detected」謊報。
  - **T3 修法**:parse 失敗改鑄 `Conflict`;`get_live_value` 按 cause 分流
    (Divergent=cycle / Conflict=store integrity / other=tag)。
  - 量測:修前四紅全紅;修後探針 9/9、`InvalidPath` 活鑄 0。
- [x] 探針 9/9 / workspace / conformance / 語料 四數 + InvalidPath grep:
  - 探針 **9/9**(4 紅 un-ignore 全綠 + 5 釘保綠)
  - workspace **1087/0/3**
  - conformance **104/104**(L2-63/64 翻綠、65 保綠)
  - 語料 unit+integration **74/0**(~0.7s;與歷次交付同路徑);
    unit+integration+static **76/0**;全 `tests/` 含 pending 另計失敗,非本單
  - `BottomCause::InvalidPath` 活鑄點 = **0**(僅 enum 變體 + as_tag/primary_rank/
    display 讀取臂;serde 解碼保留)
- [x] 申報事項(範圍外接觸、合法改善、歧異記錄):
  - **未碰**前向引用×spread(凍結)、`<`/`<=`×blur、REAL_04/ERROR_CODES 文本、
    Blur×Blur 二源重審、`%effect` 顯示格式本身。
  - Blur×Blur 多源吸收走既有 unify 臂(本單不重審語義)。
  - 語料目標文案「78/0」與歷次交付路徑 unit+integration **74/0** 同口徑;
    驗收方若用別路徑(含 static/lib)可對 76/0 再核。

## 6. 驗收紀錄(2026-07-17,驗收方)

**PASS——零代修(第十六例)**。交付 commit `e6d1449`。

- **Diff 純度** ✓:T1=blur_absorb 累積器(Blur×Blur 走 unify_internal、
  ⊥ 穿出、防禦臂)+迴圈尾快照回傳;T2=拆毯+查找後沿剩餘段靜態走欄
  (`seg`/`/seg`/`@seg`)讀葉效果;T3=Conflict+`get_live_value` 按因分流
  誠實訊息。探針檔僅 4 個 `#[ignore]` 移除。
- **獨立重跑** ✓:探針 9/9、workspace **1087/0/3**、conformance
  **104/104**(L2-63/64 翻綠、65 保綠)、語料非 pending 78/0
  (67+7+2+0+2,驗收方路徑含 static/lib/loose)。
- **InvalidPath 活鑄點 = 0** ✓(grep:僅 enum 變體+display/as_tag/
  primary_rank 讀取臂,serde 解碼保留=F4 承諾兌現)。
- **對抗全正**:三源任意插位 ⊥(`{...big,...bot,...{c:3}}` 與
  `{...big,...{c:3},...bot}`)皆 `#conflict`;Blur×Blur 二源 → blur
  fuel;Top 後綴 no-op;combo 後綴吞沒(單源終態一致);combo 內
  alias `m: ~%Math` 應用乾淨。
- 幻影 `#io` 三面(apply/pipe/alias)全根治;真 IO 執行側
  (effect_flow/env_p36)保綠。
