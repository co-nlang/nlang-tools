# 工單:靜態守護 / #effect_violation(SPEC_08 §4.3)—— 效應系統波 arc 3

**開單**:2026-07-24(驗收方)。**基線**:dev @ 本工單 commit(v0.2.35 後)。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。探針/向量**修改權在
驗收方**——交付僅移除探針 `#[ignore]`,一字不改其餘。

## 1. 法源(裁定 Model A,2026-07-24,使用者)

SPEC_08 §4.3:「在預設的純粹上下文 (#pure) 中,若意外觸發了 #io 觀測,引擎
將其阻擋並坍縮為 `_|_`(含 `%cause: #effect_violation`)。」

**「純語境」的操作對映(裁定 A:宣告純度被矛盾)**:n/ 唯一的純度宣告機制 =
顯式 `%effect: #pure` 元欄(SYNTAX_08 可寫元欄)。當此宣告被值的**實際活動
傳染效應**(#io/#nondet/#state)**矛盾**時,承諾即謊言,值坍縮為
`_|_ (%cause: #effect_violation)`——「形式化 = 說謊就崩潰的型別系統」。
**非**環境預設語境(那會坍縮每個 io 值、砸掉 L2-83 與 arc-1/2)。**opt-in、
非破壞**。

**逃生門 = 繭壁**(自動豁免):繭 `{{ }}` 真正屏蔽(closed 跳過效應累積
§4.2.1),故其實際 `cv.effect` **就是 #pure**——宣告 #pure 與實際 #pure
**相符**,無矛盾、無違規。(runPure §4.3 = 另一條特權 discharge,掛帳。)

## 2. 病灶(v0.2.35 量測)

引擎**已算出矛盾**卻讓 spoof 靜默勝出:
```
{ %effect: #pure, v: io }.%effect     → #pure  ;; %effect: #io
{ %effect: #pure, v: nondet }.%effect → #pure  ;; %effect: #nondet
```
`.%effect` 讀回**宣告的** #pure;尾註洩漏**實際的** #io。守護把謊言轉 ⊥。

## 3. 修法(建議)

**(A) `BottomCause::EffectViolation` 變體**(value.rs enum,**append-only 尾**,
仿 `InvalidConfig`):`as_tag` → `"effect_violation"`。

**(B) 守護鉤 = combo 終化點**(`eval.rs` `ExprKind::Combo` 臂,**~line 1061**,
在效應已終化之後——即 open 的 `me` 與 closed 的 shield 都已定〔1054-1060〕、
`res` 回傳前 ~1069):
```
if let Value::Combo(ref cv) = res {
    // 宣告純度被實際活動傳染矛盾 → ⊥ #effect_violation
    if declared_pure(cv) && cv.effect.has_active() {
        return Value::Bottom(BottomDetail {
            cause: EffectViolation,
            message: Some("declared #pure but observes <active tag>"),  // 可選,有益
            ..
        });
    }
}
```
- `declared_pure(cv)` = `cv.get_field("%effect")` 存在且解析為 `#pure` tag
  (literal;必要時 force)。宣告 `#io`/其他 → 不觸(誠實/過宣告,非本弧)。
- `has_active()` = `contains(IO) || contains(NonDet) || contains(State)`
  (可加 `EffectTag::has_active()` 或內聯;複用 arc-2 `solidify_active_effect`
  的同一判斷)。**只在活動標籤觸發**;`#cached`/`#pure` 不觸。
- **繭自動豁免**:closed combo 的 `cv.effect` 經 shield 為 #pure(1002 的
  `if !*closed` 跳過 + 1056 union pure)→ `has_active()` false → 不觸。**勿
  對繭特判**。

## 4. 範圍柵欄

**做**:顯式 `%effect: #pure` ∧ 實際活動傳染(io/nondet/state)→ ⊥
`#effect_violation`(整個 combo 坍縮)。繭壁逃生(自動,勿特判)。

**不做(掛帳後續弧,勿實作)**:`~%Effect./runPure` + `%privilege_token`
(§4.3)、`#ext:`(§4.1)、CAID 全集參與(§4.1)、**下宣告**(declared #io
over actual io|nondet = 另一種謊,非「純度」矛盾)。

## 5. 風險與紅線

- **回歸掃描**:交付前 grep 全樹(tests/corpus/conformance)是否有既有
  `%effect: #pure` 疊在**活動內容**上而期望**非 ⊥** 的案例;若有=真曝光
  (那些正是謊),照實申報,勿為過測放寬守護。
- **不動**:`.%effect` 讀取臂(spoof 精度〔effect_meta〕)、arc-1 union、
  arc-2 solidify、⊥/blur 白名單、EffectTag 型別、CAID/bn_serial。守護只在
  combo 終化**新增一條 ⊥ 早返回**。
- 誠實宣告(`%effect: #io` over io)、真純宣告(`%effect: #pure` over 純)、
  未宣告 io、繭壁——四者**皆不得**翻 ⊥(釘覆蓋)。

## 6. 門(紅)與釘 + 目標(先量後寫,基線實測 2026-07-24)

**探針** `crates/interpreter/tests/effect_violation_probe_test.rs`(已預提交+校準):
- **5 紅**(`#[ignore]`,全紅正因現值非 ⊥、謊言被靜默接受):
  `red_violation_io`/`_nondet`/`_state`(宣告 #pure 疊活動 → ⊥)、`_nested`
  (孫層 io 傳染 → ⊥)、`_propagates_through_effect_read`(`.%effect` 讀得 ⊥,
  原 `#pure`)。
- **6 釘**(全綠須守):真純宣告 `{%effect:#pure,v:42}`→#pure、**繭壁逃生**
  `{{%effect:#pure,v:io}}`→#pure、誠實 `{%effect:#io,v:io}` 非 ⊥、未宣告 io
  照流(`{v:io}`/裸 io → #io)、未宣告多活動 `#io|#nondet`、⊥ 白名單。

**交付 = 移除 5 個 `#[ignore]`**,探針其餘一字不改。

**目標**(基線 → 交付後):
- effect_violation 探針 **11/11**(基線 6 綠 + 5 ignore)。
- workspace **1358/0/3**(基線 with-probes 1353/0/8)。
- conformance **141/141**(基線 140/141;L2-100 現紅 got combo;101/102 綠邊界
  =真純宣告 + 繭逃生)。runner:`nlang-spec/scripts/run-conformance.py`。
- 語料非 pending 不退。

全 workspace 一顆不得翻紅(除 §5 回歸掃描曝光之真謊,照實申報)。

## 7. 交付紀錄(交付方填;先寫再回報)

- [x] 交付 commit(s): tools tip (subject: effect_violation arc 3)
- [x] `BottomCause::EffectViolation`(as_tag/append-only)落點:
  - `value.rs` enum 尾加 `EffectViolation`;`as_tag`/`as_cause_combo`/`primary_rank`
    (rank 1 與 system_reserved/invalid_config 同檔)。
  - `EffectTag::has_active()` 抽出(io|nondet|state);solidify 複用。
- [x] 守護鉤位置 + 條件(declared_pure ∧ has_active;繭經 shield 自動豁免):
  - `eval.rs` Combo 終化、`res` 回傳前;`declared_pure_meta` 讀 `%effect`
    force 後 tag == pure;`cv.effect.has_active()`。繭 closed shield 後
    effect pure → 不觸(無特判)。
- [x] §5 回歸掃描結果(既有 `%effect:#pure` over active 案例有無):
  - 全樹:探針/工單/SPEC_09 EML 註解、L2-100/101/102(本弧合規)。
  - **無**既有「宣告 pure 疊活動且期望非 ⊥」的測試/語料/合規向量曝光。
- [x] 探針 11/11 / workspace / conformance / 語料 四數:
  - effect_violation **11/11**(5 ignore 全撤,斷言未改)
  - workspace **1358/0/3**
  - conformance **141/141**(L2-100 翻綠;101/102 邊界綠)
  - 語料 **75/0**(68+7)
- [x] 申報事項(範圍外接觸、繭/⊥ 邊界、其他):
  - 未改 `.%effect` spoof 讀取臂(違規在終化早返,讀臂見 ⊥)。
  - runPure / 下宣告 / #ext / CAID 全集未做(掛帳)。

## 8. 驗收紀錄(驗收方填)
