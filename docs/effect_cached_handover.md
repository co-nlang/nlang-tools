# 工單:#cached 固化(SPEC_08 §4.2.4)—— 效應系統波 arc 2

**開單**:2026-07-24(驗收方)。**基線**:dev @ 本工單 commit(v0.2.34 後)。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。探針**修改權在驗收方**
——交付僅移除探針 `#[ignore]`,一字不改其餘。

## 1. 法源(SPEC_08 §4.2.4,零新裁定)

> 「一旦數據被坍縮並**獲得穩定 CAID**,其 `%effect` 中的**活動標籤**
> (#io/#nondet/#state)必須**在觀測結果中**被轉化(固化)為 **#cached**。」
> 「重新激活:#cached 節點參與新的態射運算且該態射有活動副作用 → 重獲活動標籤。」

**操作對映**(使用者框定 2026-07-24:「不動 CAID,只影觀測結果」):
- **「穩定 CAID」= 由 store 依內容位址取回的值**(store-committed 的歷史已固化)。
  新鮮計算值仍活動(arc-1 釘 / L2-83 守)。
- **觀測時投影,不動 store/CAID**:固化是取回值的**投影**,store 物件與其 CAID
  完全不動(`get_value` 只讀)。
- **重激活 = 免費**:arc-1 集合聯集已成立;#cached 值與新活動效應組合 →
  `{cached} ⊔ {io}` = `#cached | #io`(§4.1 矩陣)。無需代碼。

## 2. 觸發鉤(範圍,關鍵)

`get_value`(storage.rs:27)**不是**純使用者取值口——它也重建 commit root
與 refine 檢查。固化**只掛使用者面「依 CAID 取值觀測」**:

**固化(→cached)**:
- `disc.rs` **disc.fetch**(所有 `return val` / `results.push(val)` 分支:本地
  store、peer、remote)。
- `disc.rs` **disc.find**(~267 `return val`、~273 peer 分支)。
- `main.rs` **inspect**(run_inspect ~399,顯示前固化)。

**保持 RAW(不得固化)**:
- `universe.rs` **commit root 重建**(load 持久宇宙)+ **refine 單調性檢查**
  (~477,用 `content_hash()` 比對——固化會破壞比對)。
- `lib.rs:2030` **refine follow**(refine 內部)。
- `main.rs:127` **NDP wire-serve**(送 RAW,收方 peer 於**自己**的 fetch 固化)。

理由:commit-chain CAID 與 refine 決定性(REAL_04)須不受影響。

**建議實作**:helper `Ouroboros::fetch_observed(hash) = get_value(hash).map(|v|
v.solidify_effects())`,只在 disc.fetch/find + inspect 用;`get_value` 保持 raw。

## 3. 機制:`Value::solidify_effects()`

遞迴把整棵取回子樹的活動標籤 → cached(觀測投影,回傳新值):

```rust
// 單節點效應固化:任一活動標籤 → 單一 Cached;pure 不變;cached 不變。
fn solidify_active(e: EffectTag) -> EffectTag {
    if e.contains(EffectTag::IO) || e.contains(EffectTag::NonDet)
        || e.contains(EffectTag::State) { EffectTag::Cached } else { e }
}
```

- **Atom(k, e, ctx)** → `Atom(k, solidify_active(e), ctx)`(ctx 不動)。
- **Combo(c)** → c.effect 固化 **且** 每欄位遞迴 `solidify_effects()`
  (整棵取回樹皆固化;`red_fetch_nested_field` 覆蓋)。
- **Union(b)** → 各支遞迴。
- **Blur(bd)** → bd.effect 固化。**Range** → 各部遞迴。
- **Bottom/Top** → 不動(無活動效應;⊥ 白名單守)。
- **Thunk**:取回值一般非 thunk;若遇到,固化其 effect 欄即可。

**多活動坍縮**:`{io, nondet}` → `Cached`(單一;`red_fetch_multi_active` 覆蓋)。

## 4. CAID 穩定守則(紅線)

- `get_value` **保持 raw**——store 物件與 commit-chain CAID 完全不動。
- 固化 = 取回**副本**上的投影,不寫回 store。**genesis_test**(commit 鏈)
  與全套須綠。
- refine 單調性(universe.rs content_hash 比對)用 raw 值——**不得**經固化。
- 本弧**不觸** `EffectTag` 型別、`to_serial_byte`、bn_serial、content_hash
  路徑。Cached 位既有(arc 1 保留)。

## 5. 範圍柵欄

**做**:store-fetched 值觀測時活動→cached(遞迴)、多活動坍縮為單 cached、
重激活(免費驗證)。鉤只在 disc.fetch/find + inspect。

**不做(掛帳後續弧)**:靜態守護 `#effect_violation`(§4.3)、`runPure` +
特權(§4.3)、`#ext:` 標籤(§4.1)、效應集合完整參與 CAID(§4.1)。

## 6. 門(紅)與釘 + 目標(先量後寫,基線實測 2026-07-24)

**探針** `crates/interpreter/tests/effect_cached_probe_test.rs`(已預提交+校準):
- **5 紅**(`#[ignore]`,全紅正因取回值仍顯活動標籤):
  `red_fetch_io_solidifies`(got `#io` → `#cached`)、`_multi_active_collapses`
  (`#io | #nondet` → `#cached`)、`_nested_field_solidifies`(`#io` → `#cached`,
  遞迴)、`_reactivation_union`(`#io` → `#cached | #io`,重激活)、
  `_display_tail`(尾註 `#io` → `#cached`)。
- **6 釘**(全綠須守):新鮮 io 仍活動(L2-83 孿生)、新鮮 combo 活動、
  新鮮多活動仍 `#io | #nondet`(arc-1 不退)、fetched 純值仍 `#pure`、
  fetched 內容保全(`42` 原樣)、⊥ 白名單。

**交付 = 移除 5 個 `#[ignore]`**,探針其餘一字不改。

**目標**(基線 → 交付後):
- effect_cached 探針 **11/11**(基線 6 綠 + 5 ignore)。
- workspace **1347/0/3**(基線 with-probes 1342/0/8)。
- conformance **138/138 不變**。**本弧不加合規向量**:觀測對象=store round-trip,
  非 hermetic(純值 CAID 決定性,與持久 `.oo` store 既有物件碰撞致 fetch ⊥;
  io 值時戳唯一才僥倖),不合 conformance 無狀態 `oo run FILE --observe` 契約。
  探針(乾淨 temp store/測)為本弧法定測具。〔另註:持久 store 碰撞回 ⊥ 為
  既有 store 行為,非本弧引入〕
- 語料非 pending 不退。

全 workspace 一顆不得翻紅。

## 7. 交付紀錄(交付方填;先寫再回報)

- [x] 交付 commit(s): tools tip (subject: effect_cached arc 2)
- [x] `solidify_effects()` 落點(遞迴含 combo 欄/union/blur/range):
  - `value.rs`: `solidify_active_effect` (任一活動 → 單 `Cached`; pure/cached 不變);
    `Value::solidify_effects` 遞迴 Atom/Combo(全軸欄位)/Union/Blur/Range/Thunk。
- [x] 鉤點確認(disc.fetch 全分支 + disc.find + inspect;universe/refine/NDP raw):
  - `disc.rs` fetch: local store / peer local / remote / multi-result merge 前 push 皆
    `val.solidify_effects()`; find: local+peer+remote 回傳固化。
  - `oo/main.rs` `run_inspect`: 顯示前固化。
  - `get_value` / universe load / refine / NDP serve 未動(raw)。
- [x] 重激活免費驗證(cached ⊔ 活動 = 聯集,無新碼):
  - `red_reactivation_union` → `#cached | #io`(arc-1 union 自然成立)。
- [x] CAID 穩定(get_value raw、genesis_test 綠、refine content_hash 用 raw):
  - 未改 bn_serial / to_serial_byte / content_hash; workspace 含 genesis 全綠。
- [x] 探針 11/11 / workspace / conformance / 語料 四數:
  - effect_cached **11/11**(5 ignore 全撤,斷言未改)
  - workspace **1347/0/3**
  - conformance **138/138**(不變)
  - 語料 **75/0**(68+7)
- [x] 申報事項(範圍外接觸、store 行為觀察、其他):
  - 固化僅觀測投影,不寫回 store。
  - §4.3 靜態守護 / runPure / #ext / CAID 全集參與未做(掛帳)。

## 8. 驗收紀錄(驗收方填)
