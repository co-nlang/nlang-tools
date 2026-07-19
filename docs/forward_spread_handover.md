# 工單:前向引用 × spread(收斂時序,SPEC_03 §3.1 增訂)

**開單**:2026-07-19(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。

## 1. 法源(零新裁定;SPEC_03 §3.1 時序條款 2026-07-19 收帳)

- **欄位同時性/交換律**(SPEC_03,L1-26/27 前向引用既有法):
  展開源的檔內定義位置**不得**影響結果。
- **§3.1 新條款**:展開於**觀測收斂時**擴張;碰撞交集/⊥ 傳因/
  Blur 吸收諸律於擴張時一體適用;未定義源=開放 `_` → Top no-op
  (既有行);循環展開照 `#divergent`(C4 機構)。

## 2. 病灶(v0.2.25 量測)

展開於**構造時急切**執行——前向源五面全靜默貢獻零:基本欄
`q.a`→`_`(法 7)/碰撞 `w.a`→`1..5`(法 1)/⊥ 源 no-op(法
#conflict,倒序對照今日正確)/blur 源 no-op(法吸收原樣)/別名
鏈 `_`(法 7)。循環面被病遮蔽(源永不擴張 → `_`)。

## 3. 修法方向與位點

- 病灶=combo 構造時 spread 立即讀源(未定源得 Top → no-op 臂)。
- 修法方向自選:(a) 未解析源保留 pending 展開項,force 時擴張
  (碰撞交集走 merge_field_into 既有機構);或 (b) 源 thunk 化 +
  force 時擴張。**擴張時**適用全部既有律(交集/⊥/blur/私有
  排除),**勿**另寫前向特例臂。
- **C4 循環守衛必須在擴張路徑上仍然武裝**(紅門釘死:別名繞道
  自展開 `#divergent`,不得懸掛不得靜默)。
- **不動**:倒序展開行為、Top no-op 分界、私有排除
  (spread_privacy 釘)、碰撞交集語義本體(spread_collision 釘)。
- **CAID/時序記錄義務**:含 pending 展開之 combo 於 store 落定
  時點的形(thunk vs solid)如實記錄交付紀錄;harness/CLI 兩語境
  若異,照實申報(勿為齊一而改機構)。
- **記錄義務(非門)**:cross-dep `q: {...src, b: 1}\nsrc: {a:
  q.b}` 今日 `_`——理想=逐座標惰性下 `1`,但循環守衛粒度可容
  合法差異;交付行為照實記錄 §5,驗收方另案裁量。

## 4. 門(紅)與釘

**已預提交+校準**(7 紅全紅、3 釘全綠 + 1 遷移紅)。

- `crates/interpreter/tests/forward_spread_probe_test.rs`(新檔):
  紅=基本欄(L2-81 孿生)/碰撞交集/⊥ 傳因(L2-82 孿生)/blur
  吸收/別名鏈/交換律等值 `q1 = q2` `#true`/循環展開 `#divergent`
  (校準發現:今日 `_` 因病遮蔽,修後 C4 必接手)。
  釘=倒序三面/未定義源 no-op 雙拼/私有排除雙面。
- **開單遷移紅**:spread_collision_probe_test
  `pin_forward_ref_spread_frozen` 解凍遷移(`_`→`7`,MIGRATED
  標記)。

交付=移除全部 8 個 `#[ignore]`,探針檔**其餘一字不改**(修改權
在驗收方)。全 workspace 一顆不得翻紅;語料非 pending 不退。

## 5. 目標與交付紀錄

**目標**(基線實測 2026-07-19,先量後寫):探針 11/11 + 遷移紅
綠;workspace **1234/0/3**(基線 1226/0/11);conformance
**121/121**(基線 119/121,L2-81/82 翻綠);語料非 pending
**74/0**(unit 67 + integration 7)不退。

**交付紀錄**(交付方填;先寫再回報):

- [x] 交付 commit(s): (本交付 commit,見 `git log` — message 含 forward_spread)
- [x] 根因與修法(pending/thunk 形制、C4 武裝點、CAID 時序寫明):
  - **根因**:combo 構造急切 force 展開源;前向未定源=Top → no-op。
  - **形制(b)**:`ComboVal.pending_spreads: Vec<Value>` 存源 Thunk
    (serde skip,預設空);構造只入佇列;擴張在
    `expand_combo_pending`(與急切臂同律:交集/`⊥`/blur/Top no-op/私有)。
  - **擴張觸發**:`force`/`force_recursive`、`navigate_segments`(段前)、
    `force_coord`(仍在 computing 內擴張,C4 可見)、`unify_combo` 前。
  - **關鍵補丁(交付期病灶)**:`engine.unify`/`eval_context` 用系統 root
    (`memo_enabled=false`)——evolve 合併 staged 與 observe 入口 unify 若
    急切 expand,前向名解析為 Top 並 `mem::take` 清空 pending,後續觀測
    永久靜默。修法:(1) 引擎內部 expand 遇 Top **回佇**源 Thunk;
    觀測 ctx(`memo_enabled`)才真 Top no-op。(2) `unify_combo` 結果
    **保留**殘餘 `pending_spreads`(原 `ComboVal::new` 丟棄)。
  - **C4**:構造時 `spread_path_is_under_construction`;擴張時再檢 +
    展開 force 前 `chain_transform_taint=true`(別名繞道→`#divergent`
    非 static Top);`force_coord` 對 pending Combo 亦入 computing。
  - **CAID/時序**:in-session evolve 可暫存含 pending 的 combo(thunk 形);
    observe/force 後 solid。pending 不進 serde / 不進 content_hash
    (落庫前應 force solid;本 harness 路徑均 expand 後觀測)。harness 與
    CLI 語境行為一致(見 cross-dep)。
- [x] 探針/workspace/conformance/語料 四數:
  - forward_spread 探針 **10/10** + 遷移紅 pin_forward_ref_spread 綠
    (= 工單 11 紅門合計)
  - workspace **1234/0/3**
  - conformance **121/121**(L2-81/82 翻綠)
  - 語料 unit+integration **74/0**
- [x] 申報事項(cross-dep 記錄義務、範圍外接觸、歧異記錄):
  - **cross-dep** `q: {...src, b:1}; src: {a: q.b}` → 實測 **`1`**
    (harness observe 與 `oo run --observe` 一致;達理想逐座標惰性)。
  - **未碰** spread_privacy 本體、collision 交集本體、Top no-op 分界
    (僅改 expand 時序與 unify 保留 pending)。

## 6. 驗收紀錄(2026-07-19,驗收方)

**PASS——一件代修(驗收代修;交付漏 cocoon 目標面)**。交付
commit `6822ff3`、代修 `f072e6b`。

- **Diff 純度** ✓:形制 (b)(`pending_spreads` thunk 佇列+四觸發
  點擴張+unify 保留 pending+交付期自抓 root-unify 消耗病)按單;
  PartialEq 含 pending、serde/content_hash 排除;探針僅 ignore
  移除。交付紀錄品質高(關鍵補丁自申報)。
- **獨立重跑** ✓:探針 10/10+遷移紅綠、workspace 1234/0/3、
  conformance 121/121、語料 74/0(代修後 1236/0/3 含代修釘×2)。
- **對抗**:聯集支顯示擴張後正典序/CAID 前向 vs 實心孿生
  `#true`(身分收斂!)/雙前向鏈/eq 前向 vs 實心/去重坍縮單支
  ——全正。**抓漏:cocoon 目標面**——`{{...later, b: 1}}` 前向
  `#missing_key` vs 倒序 `7`(顯示 `{{b: 1}}` 證 pending 未擴):
  繭構造 force_recursive 於 evolve 期跑,觀測類 ctx 把「尚未定義」
  當「永不定義」消耗 no-op;且 force_recursive 重建丟 pending。
- **代修**(反事實:未修=cocoon 面 ⊥、修後六面全綠):
  (1) `EvalContext.in_evolve` 相位旗標(Universe::evolve 置位),
  evolve 期 **closed combo** 之 Top 源回佇(開放 combo 保基線
  消耗——never-eq 面 `q = {b:1}` `#true` 守住);(2)
  force_recursive combo 重建保留 pending。代修釘×2(cocoon 雙序
  /never eq+本徵態)。
- **共責**:紅門只釘 `{}` 目標——**容器雙形({}/{{}})與拼法
  雙形同級,門要雙釘**(教訓入紅線)。
- **既有債記帳**:evolve 期急切計算×pending 開放 combo(如
  `out: q = q2` 寫在源定義之前)=基線同形(v0.2.25 急切展開
  同樣答錯),前向計算通用族另案。
- **cross-dep 記錄**:`src: {a: q.b}` → `1`(交付申報,驗收
  複測 ✓;達逐座標惰性理想)。
