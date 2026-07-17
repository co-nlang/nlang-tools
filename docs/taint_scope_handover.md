# 工單:靜止環染色作用域(taint 鏈作用域化)

**開單**:2026-07-17(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。

## 1. 法源(全既有,零新裁定;引擎追法)

- **SPEC_12 §1.1 兩級線**:純引用環(裸名+純路徑)→ 帶因 Top
  `#static_cycle`;任何變換跳 → ⊥ `#divergent`。
- **Q2「非純引用即變換」**:判的是**環自身的跳**——與環無關的兄弟
  求值不得參與分類。
- **Q4 守欄「不傳播」**:分類狀態不得洩漏到無關求值(運算消費即
  蒸發之同族精神)。

## 2. 病灶(v0.2.19+cull 量測;儀器化 worktree 直讀確認)

**根因**:`lib.rs` force 的染色寫回
`ctx.chain_transform_taint = ctx.chain_transform_taint || call_ctx.chain_transform_taint`
(「once transform, always transform」)把**鏈狀態全域化到整個觀測
ctx**。force 任何非純引用 thunk(儀器實測:字面量 `9` 即觸發
`TAINT_SET expr_kind=atom`)即永久毒染 ctx;之後同觀測內的靜止環
再入(`cycle_reentry`)讀到 taint=true → 誤判變換 → ⊥ `#divergent`
→(剔除弧上崗後)被依法剔,`_` 支**靜默消失**。

量測面(序依賴=變因):
- `u: {v:9}|p` 之 `.v` → `9`(p 支被抹);反序 `p|{v:9}` → `_ | 9` ✓
  (再入先於字面量 force,儀器:taint=false)。
- alias(`al: p`)、互指環(`a1↔b1`)成員同病 → `9`。
- **twin-eq 謊**:`u1: p|{v:9}`、`u2: p|{v:9}` → `u1 = u2` → `#false`。
- 欄內 join `w: {q: p.v | 9}`:CLI 綠(evolve 先分類)/harness 紅
  (觀測期 force 踩毒 ctx)——語境依賴本身即病。
- 兄弟欄面(`w: {a:1+1, b:p.v}`)今日健康=**偶然時序**(p.v 在
  evolve 期先分類),非正確作用域。

**帳載修正第九次**:上弧「CLI vs harness 語境分歧」框架=量測
误差;真變因=**同觀測內先行 force 的非純引用兄弟**(染色汙染)
+分類時點(evolve 期 vs 觀測期再入)。

## 3. 修法方向與位點

- **拆除向上寫回**:force 收尾的
  `ctx.chain_transform_taint ||= call_ctx.chain_transform_taint`
  (lib.rs ~1170)——鏈狀態隨鏈框架死亡,**不回寫**。
- **下傳繼承保留**(`sub_context` 全克隆):真變換環的跳在自身鏈內
  染色,再入點在鏈內讀到 → 分類安全(儀器已證:`m.a+1` 的
  taint 自鏈內設)。**勿**改成子鏈清零。
- `ctx.cycle_chain = call_ctx.cycle_chain` 寫回:同屬鏈狀態;健康
  路徑 push/pop 平衡=寫回恆等。量測後擇一(保留=最小 diff/移除
  =衛生),兩案皆須四數全綠;若移除,互指環成員名單(SPEC_12
  `%members`)照舊正確=static_cycle 弧探針看守。
- **不動**:`in_flight`/`computing`/`fuel`/`lexical_forcing` 寫回
  (直譯器狀態,正確);`expr_is_pure_ref` 判準;cycle_reentry 本體;
  四再入點位置;剔除弧機構(union_bottom_cull_probe_test 全綠)。

## 4. 門(紅)與釘 —— `crates/interpreter/tests/taint_scope_probe_test.rs`

**已預提交+校準**(6 紅全紅、8 釘全綠)。交付=移除 6 個
`#[ignore]`,探針檔**其餘一字不改**(修改權在驗收方)。

紅門:C 形 `9 | _`(L2-72)/三支居中 `9 | _ | 8`/alias 形/互指環
成員/欄內 join `_ | 9`(harness 紅)/twin-eq `#true`。
釘:直接觀測 `_`/`%cause` `#static_cycle`/靜止環首支序 `_ | 9`/
root join `_ | 9`/變換環成員依法剔 `1`(L2-73 綠法釘)/變換環直接
`#divergent`/兄弟欄 `_`/剔除弧 thunk-⊥ 門 `1`。

另:static_cycle_probe_test(L2-53~56)、union_bottom_cull、
cycle_test、詞法雙弧、全 workspace 一顆不得翻紅;語料非 pending
不退。**紅了停下勿弱化**——若寫回移除翻紅任何 static_cycle 釘,
報驗收方,勿自行改判準。

## 5. 範圍外(碰到=停,不改)

- math×Top 聯集值語境(`(_|9)+1` → 今日 ⊥ #conflict)——另案記帳,
  勿修。
- TopCaused vs Top 之 normalize 去重判等——遇歧異記錄勿裁。
- canonical 顯示序(帳)。`<`/`<=`×union(§4.10 帳)。
- 剔除弧機構本體(cull/原樣主因果)。

## 6. 目標與交付紀錄

**目標**:探針 14/14;workspace **1151/0/3**(開單基線**實測
1145/0/9** = 1137+8 釘,6 紅 `#[ignore]` 移除後全綠);conformance
**112/112**(基線 111/112,L2-72 翻綠、73 保綠);語料非 pending
**74/0**(unit 67 + integration 7)不退。

**交付紀錄**(交付方填;先寫再回報):

- [x] 交付 commit(s): (本交付 commit,見 `git log` taint_scope)
- [x] 根因與修法(cycle_chain 寫回擇案附量測):
  - **根因**:`force` 收尾
    `ctx.chain_transform_taint ||= call_ctx.chain_transform_taint`
    把鏈染色全域化到整個觀測 ctx。force 任何非純引用 thunk(字面量
    `9` 亦觸發)永久毒染;同觀測內後續靜止環再入讀 taint=true → 誤判
    變換 → ⊥ `#divergent` → 剔除弧依法剔 → `_` 支靜默消失。序依賴
    `{v:9}|p` 壞 / `p|{v:9}` 好 = 先行兄弟 force 是否踩毒。
  - **修法**:拆除 **taint 向上寫回**;下傳繼承(`sub_context` 全克隆)
    **保留**——真變換環在自身鏈內染色,再入點仍讀到。
    `in_flight`/`computing`/`fuel`/`lexical_forcing` 寫回不動;
    `expr_is_pure_ref` / cycle_reentry / 四再入點 / 剔除弧不動。
  - **cycle_chain 寫回擇案**:量測後 **保留** `ctx.cycle_chain =
    call_ctx.cycle_chain`(最小 diff)。健康路徑 push/pop 平衡 → 寫回
    恆等;static_cycle 互指環 `%members` 探針保綠,無需衛生性移除。
  - 量測:修前 6 紅全紅;修後探針 14/14;變換環釘仍 `#divergent`/剔 `1`。
- [x] 探針 14/14 / workspace / conformance / 語料 四數:
  - 探針 **14/14**
  - workspace **1151/0/3**
  - conformance **112/112**(L2-72 翻綠、73 保綠)
  - 語料 unit+integration **74/0**
  - static_cycle / union_bottom_cull / cycle_test / 詞法雙弧 保綠
- [x] 申報事項(範圍外接觸、歧異記錄):
  - **未碰** math×Top 聯集值語境、TopCaused vs Top 去重、canonical 顯示序、
    `<`/`<=`×union、剔除弧機構本體。
  - 無歧異需裁;cycle_chain 未移除(量測保留方案已四數全綠)。
