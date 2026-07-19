# 工單:%effect 元讀 + 診斷註解層(SPEC_08 §4.1 / SPEC_11 §3.4)

**開單**:2026-07-20(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。

## 1. 法源(裁定 C,2026-07-20)

- **SPEC_11 §3.4 診斷註解層(新設)**:引擎得以註解形式(`;;`)
  在正典拼法外附加診斷資訊;parser 不可見、不參與值語義/等值/
  CAID/顯示排序鍵;效果尾註 `;; %effect: <tag>` 為首位法定成員。
  → **顯示現狀即合法,顯示層零改動**。
- **SPEC_08 §4.1 元欄觀測(新設)**:`x.%effect` → 效果標籤原子。
  預設 `#pure`;開放 Combo=傳染 join(§4.2.1);Cocoon=屏蔽
  `#pure`;聯集依投影逐支分配(SPEC_07);顯式 `%effect` 欄位
  (SYNTAX_08 可寫元欄)優先於引擎標籤;`_|_`/`#blur` 元讀白名單
  (`%cause`/`%caid`)**不變**——⊥ 照 F1 傳過、blur 吸收。

## 2. 病灶(v0.2.26 量測)

`.%effect` 讀取在**一切**常規值上回 Top `_`:io 原子/純原子/
combo/cocoon/聯集/nondet 全滅。存儲側健康:`Value::effect()`
取用器現成;combo 構造時 `me = me.max(te)` 已算傳染 join 且
closed(cocoon)跳過=屏蔽已實作——**只缺讀取臂**。
附帶:孤兒 tests/effect_taint.n 的 `combined.%effect == #io`
今日靠 `_ == #io`→`_` 被 harness 判 PASS(空洞真)。

## 3. 修法方向與位點

- 位點=`navigate_segments`(lib.rs)常規值段處理:`%effect` 段
  → `Value::Atom(Tag(<effect>), Pure, None)`。放置順序:**欄位
  查找之後**(spoof 釘=顯式欄位優先)、missing-key/closed-miss
  鑄造**之前**(cocoon 讀 `%effect` 不得 #missing_key)。
- 標籤來源=節點自身 `effect()`(combo 讀 ComboVal effect 欄;
  構造 join 與 closed 跳過**勿動**)。Thunk 面照既有 force 序。
- 聯集分配預期隨既有 union 導航機構自然成立;若機構歧異,照實
  申報勿另寫特例臂。
- `& 合一`面(red_effect_read_unify_join):unify_combo 結果之
  effect 應為兩側 join(§4.1 組合;引擎 max 即單標籤 join)。
  若 unify 現丟 effect,補 join;**勿**改合一語義本體。
- **不動**:⊥/blur 元讀白名單(lib.rs 1607-1641 臂)、顯示層
  (`;; %effect:` 尾註現狀即法)、EffectTag 枚舉(無 #cached=
  效應系統波另案)、CAID/content_hash(參與義務另案)。
- **語料遷移(交付步)**:`git mv tests/effect_taint.n tests/unit/`
  (內容一字不動)——修後 `#io == #io` 真綠,孤兒轉正;語料
  unit 目標 68 檔 75/0。

## 4. 門(紅)與釘

**已預提交+校準**(7 紅全紅正因 `got _`、5 釘全綠)。

- `crates/interpreter/tests/effect_meta_probe_test.rs`(新檔):
  紅=io 原子(L2-83 孿生)/純預設雙面(原子+純 combo)/combo
  傳染/cocoon 屏蔽(L2-84 孿生)/nondet/聯集分配
  (`#io | #pure` 正典序)/& 合一 join。
  釘=io 尾註在/純值無尾註/spoof 欄位優先/⊥ 白名單+傳過/blur
  吸收。

交付=移除全部 7 個 `#[ignore]`,探針檔**其餘一字不改**(修改權
在驗收方)。全 workspace 一顆不得翻紅;語料非 pending 不退。

## 5. 目標與交付紀錄

**目標**(基線實測 2026-07-20,先量後寫):探針 12/12;workspace
**1248/0/3**(基線 1241/0/10);conformance **123/123**(基線
121/123,L2-83/84 翻綠);語料非 pending **75/0**(unit 68 檔含
遷入之 effect_taint + integration 7)。

**交付紀錄**(交付方填;先寫再回報):

- [x] 交付 commit(s): (本交付 commit,見 `git log` — message 含 effect_meta)
- [x] 根因與修法(讀取臂位置、& 合一 effect 處置寫明):
  - **根因**:`Value::effect()` / combo 構造 join+cocoon 屏蔽已健康;
    `navigate_segments` 無 `.%effect` 讀取臂 → 常規值一律 open miss `_`。
  - **讀取臂**(`lib.rs` `navigate_segments`):
    1. Combo:欄位查找**之後**(spoof 顯式 `%effect` 欄優先)、closed-miss
       **之前** → `effect_tag_atom(c.effect)`(純 Tag 原子,無 re-taint)。
    2. 非 Combo/Union/Bottom/Blur:force 後若段=`%effect` →
       `effect_tag_atom(current.effect())` 早返回。
    3. Union:既有 per-branch 投影自然分配;投影後**早返回**
       `normalize_union`(勿 `with_effect(accumulated)` 否則印出
       `#io ;; %effect: #io | …`)。
  - **⊥/blur**:白名單臂未動——`%effect` 非白名單,⊥ F1 傳過、blur 吸收。
  - **& 合一**:`unify_combo` 既有 `a.effect.max(b.effect)` 即 join;未改
    合一語義本體,紅門 `u1.%effect` → `#io` 自然綠。
  - **顯示**:`;; %effect:` 尾註零改動(SPEC_11 §3.4 法定)。
  - **語料**:`git mv tests/effect_taint.n tests/unit/`(內容不動)。
- [x] 探針/workspace/conformance/語料 四數:
  - effect_meta 探針 **12/12**(7 紅解凍 + 5 釘)
  - workspace **1248/0/3**
  - conformance **123/123**(L2-83/84 翻綠)
  - 語料 unit+integration **75/0**(含遷入 effect_taint)
- [x] 申報事項(聯集分配機構歧異、範圍外接觸、其他):
  - 聯集分配:無特例臂,既有 Union 投影 + 早返回純標籤即可;無歧異。
  - **未碰** EffectTag 枚舉、CAID/content_hash、顯示層、⊥/blur 白名單。

## 6. 驗收紀錄(驗收方)
