# 工單:G4 Union 路徑導航 (2026-07-12)

**執行者**:模型 #3
**驗收者**:專案腦(探針已預先提交,紅線不可動)
**探針**:`crates/interpreter/tests/union_nav_probe_test.rs`(6 紅門 + 7 釘;已校準:紅門今日全紅、釘今日全綠)

**驗收 = 紅門全綠 + 釘全綠 + 全套件無退化(基線 800/0/3)+ 語料 72/0 + conformance 全綠(含新增 L1-32,交付時應 52/52)。**

---

## 0. 重要:G4 已重新診斷 — 與去重無關

原帳「union 去重 × 導航(導航拿到去重前 Union)」**範圍錯誤**。反事實:同
向量在 v0.2.2(去重交付後、G2 前)行為與今日完全相同。真相:

- **任何**真多支 Union 的路徑導航——字面量或演化結果——都 ⊥
  `#invalid_path`:`({a:1} | {a:2}).a`、`(({p:{q:5}})|…).p.q` 皆死。
- 病灶單點:`navigate_segments`(lib.rs:1380)的 match 只有
  `Value::Combo` 臂,Union 落 `_ => InvalidPath` catch-all。
  **Union 導航從未被實作**。
- 去重是紅鯡魚:衝突殺支後單倖存(Union 包裝坍縮)導航自然能走,
  全支存活(真雙支)就死——先前誤把相關性當因果。

## 1. 裁定(SPEC_07 平等演化;觀測投影 = 逐支泛函)

Union 之路徑導航 = **逐支導航**,每支行為與單值導航完全一致(單值基準
已釘:combo 開放缺欄 → `_`;原子/不可導航 → ⊥ `#invalid_path`),然後:

1. **⊥ 支剔除**(相容支存活,與 meet 分配同律);
2. **全支 ⊥ → ⊥ `#invalid_path`**(釘 `pin_union_nav_all_bottom_is_invalid_path`
   ——今日 catch-all 恰好同判,修後不得變);
3. 倖存支過 `normalize_union`(結構去重、單支坍縮 Union 包裝);
4. **Top(開放缺欄)支保留**——誠實疊加「可能缺、可能是 2」:
   `({a:1} | {a:1,b:2}).b` → `_ | 2`。鏡照單 combo 的開放世界語義,
   不越權替使用者塌縮。

## 2. 地圖

- `crates/interpreter/src/lib.rs:1335 navigate_segments` —— 迴圈內
  `match current`(:1380)加 `Value::Union(branches)` 臂:逐支
  `navigate_segments(branch, &[seg], …)`(或等價的單段遞推)、依裁定
  1–4 匯總。**注意 current 已 force**;支內值再各自 force(單值路徑
  本就如此)。
- `normalize_union` 已存在(value.rs,聯集冪等交付)。
- pure-wrapper 解包(:1346)與 `%id`/`%rank`/⊥-meta 特殊段(:1354–1378)
  在 Union 臂**之前**已處理 current 非 Union 情況;Union 臂只管分支投影,
  **勿**讓 `%cause`/`%type` 等 meta 段對 Union 分配(⊥ 的 meta 觀測是
  ⊥ 專屬,Union 含 ⊥ 支不可能——⊥ 支在聯集構造時已被 normalize 處理)。

## 3. 邊界與陷阱

1. **效果標籤**:逐支導航的 accumulated_effect 取各支 max(現行單值
   已累積;Union 臂匯總時保持)。
2. **燃料**:逐支各自扣(遞迴呼叫天然如此);大 Union × 深路徑受
   max_branches 既有上限保護,勿自行加新上限。
3. 多段路徑:逐支投影後**續段在匯總結果上進行**(即每段做一次分配-
   匯總;`red_union_nav_multi_segment` 驗 `.p.q`)。等價實作:單段分配、
   結果繼續迴圈——兩者對本探針組不可分,自選,但**別**把整條剩餘路徑
   一次遞迴進支內後再匯總又在外層重跑(雙重投影)。
4. force memo / refine_map:導航在 observe 語境(refine_map_active)——
   逐支遞迴沿用同一 ctx,勿新開 context。
5. 全語料回歸 + conformance L1-32(spec 側已入庫:
   `out: ({ a: 1 } | { a: 2 }).a` → `1 | 2`)。
6. 交付紀錄照舊格式(根因、diff、量測、未動聲明)。

## 4. 非目標

- cmp × Union 分配(`(1|2) == 2` 現 #false——另案,見 ENGINE_SYNC 遺留)。
- 去重規則本身、Range 合併、fmt。
- `<<x>>` 結構態觀測對 Union 的行為(SYNTAX_07,未量測,另議)。

---

## 交付記錄(執行者填)

（待交付)
