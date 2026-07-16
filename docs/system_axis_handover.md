# 工單:系統軸所有權(`~%` 影蓋靜默收帳)

**開單**:2026-07-16(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。

## 1. 裁定(已批 2026-07-16;SPEC_09 所有權條款、ERROR_CODES §1.4)

**Q1 — `~%` 唯引擎鑄造**:stdlib CAID = 宇宙間共享身分基底。使用者對任何
`~%` 座標的 **LHS 寫入**——既有模組(`~%Math: 5`)、路徑鍵(`~%Math.add: 7`)、
novel 名(`~%Mine: …`)——皆違法,**即使單調精化亦然**(所有權判準,非內容
判準;不做 unify 嘗試,同值寫入也擋)。

**Q2 — 違法形雙軌**:
- **root LHS** → **evolve 邊界帶名報錯**(同 G2-S root 單調演化之
  Evolution Conflict 機構;CLI exit 1)。
- **combo 字面量內 `~%` 定義鍵** → 該欄鑄 **⊥ `#system_reserved`**(新
  BottomCause 變體;fmt 凍結只增不刪合規,bn_serial 需相應 tag——留意
  序列化位元組紀律,新變體新 tag 屬「增」合法)。節點級、隨導航/應用合成
  傳播、**不自癒**——詞法鏈不得跳過違法欄改讀真系統模組。

**Q3 — 豁免**:root `~%Config.<裸名欄>` = 視界參數規範家(SPEC_08 §3.1),
照舊合法且必須**繼續生效**(實測 fuel=50 → 300 項鏈 `#fuel_exhausted`)。
combo 內寫 `~%Config` **不豁免**(同 Q2 ⊥)。

**Q4 — RHS 全面保全**:別名(`m: ~%Math`)、交集匯入(`_: ~%Cond`)、路徑
使用(`~%Math.abs x`)照舊。**拼法合法**——parser 不動(parser goldens 的
`~%sys: 1` 斷言解析成功,必須保綠);違法在語義層。

## 2. 病灶(v0.2.16+ 量測)

三種互斥行為並存:
- root `~%Math: 5` / `~%Math.add: 7`:evolve 靜默 Ok、內建照舊(`add 2 3`→5)、
  exit 0——用戶以為定義成功。
- combo `{ ~%Math: 9, v: ~%Math.abs (0 - 3) }`:影蓋生效,`c.v` → `_`
  (詞法鏈先撞局部欄);`c.~%Math` 顯示 `9 ;; %effect: #io`(幻影 io 標籤
  ——預期隨 ⊥ 鑄造消失;若在別處倖存,記錄勿追)。
- `~%Mine: {f}` → `2 |> ~%Mine.f` → `3`(自由鑄造)。

嫌疑位點:root 寫入路徑對 `~%` 系統座標的分流(G2-S 攔截在
`universe.evolve` root unify,`~%` 可能更早繞道);combo 字面量建構
(欄位鍵含 `~%` 前綴時無任何檢查);`~%Config` 的寫入管道(config 收斂弧
實作)——豁免判準沿該管道釘,勿寬(`~%ConfigX` 之類名不豁免,判準是
`~%Config` 整段名,非前綴匹配)。

## 3. 門(紅)與釘 —— `crates/interpreter/tests/system_axis_probe_test.rs`

**已預提交+校準**(6 紅全紅、7 釘全綠)。交付=移除 6 個 `#[ignore]`,
探針檔**其餘一字不改**(修改權在驗收方)。

紅門:
1. `red_root_shadow_module_loud` — `~%Math: 5` evolve 必須 Err
2. `red_root_shadow_path_loud` — `~%Math.add: 7` 同
3. `red_root_novel_loud` — `~%Mine: 5` 同
4. `red_combo_system_key_bottom` — `(c.~%Math).%cause` → `#system_reserved`(L2-60)
5. `red_combo_novel_key_bottom` — `(d.~%Mine).%cause` → `#system_reserved`(L2-61)
6. `red_combo_poison_diagnosable_no_self_heal` — `(c.v).%cause` →
   `#system_reserved`(v 用了被影蓋的 `~%Math`;**不得自癒回 3**)

釘(全數保綠):`pin_config_fuel_write`(豁免家生效)、
`pin_config_write_smoke`(豁免不誤傷)、`pin_rhs_alias`、`pin_rhs_path_use`
(L2-62)、`pin_root_import_merge`(`_: ~%Cond`)、`pin_data_axis_name_free`
(`add: 5` 資料軸自由——保留的是軸不是名)、`pin_private_axis_untouched`
(`~` 私有軸最長匹配邊界)。

另:parser goldens(`golden_ast.rs`/`spec14_sync.rs`/`roundtrip.rs` 之
`~%sys: 1`)必須保綠;全 workspace 一顆不得翻紅;語料零 `~%` LHS 已查核。

## 4. 範圍外(碰到=停,不改)

- `~%Config` 欄位名驗證(未知欄名寫入 ~%Config 的合法性)——另案。
- 幻影 `%effect: #io` 若在非影蓋路徑倖存——記錄,另案。
- `~%` 模組內容本身(SPEC_09 表)、`~%repl`/`~%System` 行為——不動。
- root `~%Config: {...}` 整組替換形——未量測未立法;遇到先記錄。

## 5. 目標與交付紀錄

**目標**:探針 13/13;workspace **1077/0/3**(開單基線 1071/0/9,6 紅
`#[ignore]` 移除後全綠);conformance **101/101**(基線 99/101,L2-60/61
翻綠、62 保綠);語料非 pending **78/0** 不退。

**交付紀錄**(交付方填;先寫再回報):

- [ ] 交付 commit(s):
- [ ] 根因與修法(附量測):
- [ ] 探針 13/13 / workspace / conformance / 語料 四數:
- [ ] 申報事項(範圍外接觸、合法改善、歧異記錄;含幻影 #io 存亡):
