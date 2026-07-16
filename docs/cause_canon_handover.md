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

- [ ] 交付 commit(s):
- [ ] 根因與修法(附量測):
- [ ] 探針 9/9 / workspace / conformance / 語料 四數 + InvalidPath grep:
- [ ] 申報事項(範圍外接觸、合法改善、歧異記錄):
