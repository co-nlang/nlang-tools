# 工單:cocoon 本徵態預設(SPEC_03 §1.2/§1.3 既有法追法)(2026-07-16)

**執行者**:模型 #3
**驗收者**:專案腦(探針已預先提交,紅線不可動——**全部**探針檔皆紅線。
若交付中發現任何既有釘因新法必紅:**停下報驗收方**;單方遷移計代修。)
**探針**:`crates/interpreter/tests/cocoon_eigenstate_probe_test.rs`
(7 紅門 + 10 釘;已校準:紅門今日全紅、釘今日全綠)

**驗收 = 紅門全綠 + 釘全綠 + 全套件無退化(基線 1022/0/3 + 本探針
17 測 = 應 1039)+ 語料非 pending 全零敗 + conformance 全綠(含
新增 L2-50~52,交付時應 91/91)。**

**紀錄義務**:交付紀錄先寫進本檔再回報。

---

## 0. 法(既有,零新裁定)

- §1.2 #1:讀取 Cocoon 未定義欄位 → **立即 ⊥**(#missing_key,
  §1.2.1 自例拼法「在本徵態中找不到」)。
- §1.2 #2:合併拒絕限「**非 Top 欄位**」——Top 欄=無約束,放行。
- §1.3:$Cocoon.k = \bot$ 本徵態預設(vs Combo 的 $\top$ 疊加態)。

## 1. 量測(v0.2.14,三病六健)

| 面 | 今日 | 法定 |
|---|---|---|
| `{{a:1}}.b` / `{{}}.x` / 巢內 | `_` | ⊥ #missing_key |
| cause-cocoon `.zz`(REAL_04 §1 同法) | `_` | ⊥ #missing_key |
| `cc & {a:1, b:_}` | ⊥ #missing_key | 放行(r.a=1) |
| `({{a:1}} \| {b:2}).b` | `_ \| 2` | `2`(⊥ 支剔) |

健康(釘):合併拒絕自例 ✓、同鍵衝突 #conflict ✓、已定義鍵、
meta 讀開放、詞法提升 6、裸名 miss `_`、開放 combo/原子開放律、
spread 解封。

## 2. 地圖與實作建議

1. **存取面**(lib.rs navigate_segments Combo 臂):get_field miss
   且 `c.closed` → `BottomCause::MissingKey`(BottomDetail 慣例;
   unify 已有同因鑄造點可參照拼法)。位置:在 F4 開放 catch-all
   (`_ => val = Top; continue`)**之前**分流 closed;%-meta 段與
   `~%` 段的既有臂在前不受影響。
2. **合併面**(unify.rs Cocoon 臂):鍵集檢查跳過**值為 Top** 的
   欄(force 後判 Top;惰性 Thunk 須先看穿——`b: _` 在字面量是
   Thunk(Atom Top))。
3. **聯集面**:navigate 的 Union 逐支投影已剔 ⊥(G4 機構)——
   存取面修好後應自動變綠;若未剔,查 per-branch 投影是否把
   MissingKey 當非 ⊥ 對待。
4. cause-cocoon:`as_cause_combo` 產物 closed=true 即自動吃到
   存取面;確認其 `_: Top` 墊欄鍵名 `_` 不與真鍵混淆。

## 3. 邊界與陷阱

1. **軸邊界三條**(釘守):%-meta 讀開放(%kind/%cause `_`)、
   `~%` 系統段照舊、**裸名解析 miss ≠ 座標存取**(`{{d: zz+1}}.d`
   → `6`? 不——`zz` miss 續外溯,d=`_`;詞法弧機構勿動)。
2. **開放世界不動**:開放 combo `.b` → `_`、原子 `.name` → `_`
   (F4/L2-31)。
3. **⊥ 效果標籤**照 BottomDetail 慣例;effect 傳遞照舊。
4. **雙拼法**:內聯 `cc.b` 與綁定拆分 `g: cc.b; out: g` 兩形皆釘
   (紅門內雙 assert)。
5. **display**:⊥ 顯示走既有 `_|_ (%cause: ...)` 形,勿新拼。
6. **G6/observe**:cocoon 值語境塌縮(純包裝剝殼)只對 %val 純
   包裝;`{{a:1}}` 非純包裝,不受影響——若實作中發現交互,停下
   報驗收方。
7. 全語料回歸(語料有 cocoon 用例:effect_cocoon/horizon_test
   之 `{...cocoon}` 解封——解封不走存取面,應無感)+
   conformance L2-50~52(今日三紅)。
8. 交付紀錄:根因、diff、量測、未動聲明。

## 4. 非目標

- 互指/自指語義(另案裁定候選)。
- %kind 材料化(佇列)。
- Cocoon 密封後拒新欄之其他面(evolve 層,如量測到現況記錄即可)。
- Blur 展開源、~% 影蓋(各自另案)。

---

## 5. 交付紀錄(2026-07-16,模型 #3)

### 根因
1. **存取**:`navigate_segments` Combo 臂 miss 一律 `Top`(疊加態開放),
   未分流 `closed` → cocoon 未定義座標與 open combo 同形 `_`。
2. **合併**:閉鍵拒絕用 `vb.is_top()` 未看穿 `Thunk(Top)`/`b: _` →
   Top 欄被誤拒 `#missing_key`。
3. **聯集**:⊥ 支剔機構已在(G4);存取修好後自動綠。

### Diff
1. **lib.rs `navigate_segments`**:`c.closed && get_field miss &&
   !seg.starts_with('%')` → `⊥ #missing_key`(path/message 慣例);
   %-meta 仍開放(F 系列軸邊界)。
2. **unify.rs `unify_combo`**:閉鍵拒絕前 `force`+`collapse` 判
   Top 無約束(§1.2 #2);真非 Top 外來鍵仍 MissingKey。

### 釘衝突
上弧 `pin_cocoon_closed_miss_frozen`(期望 `_`)與本法定 ⊥ 衝突——
停下報驗收方;驗收方已遷移該釘(探針檔現 16 測)。全套件基線
修正:**1022 −1 遷移 +17 = 1038**。

### 量測
| 項 | 結果 |
|---|---|
| 本探針 7 紅+10 釘 | **17/17** |
| 上弧 lexical_completion | **16/16**(遷移後) |
| workspace | **1038/0/3** |
| 語料 | **74/0** |
| conformance | **91/91**(L2-50~52) |
| 軸邊界釘(meta/lexical miss/open/atom) | **綠** |

### 未動聲明
- 互指/自指、%kind 材料化、evolve 層拒新欄、Blur 展開、~% 影蓋。
- 開放 combo/原子 F4 開放律;裸名詞法 miss ≠ 座標存取。
- 解封 spread 不走存取面(corpus effect_cocoon 無感)。

---

## 6. 驗收紀錄(2026-07-16,驗收方)

**判定:通過——零代修(第十四例);釘衝突停報=協議正確執行首例
(遷移由驗收方補辦,開單漏遷共責已記)。**

獨立重測:純度乾淨(僅 7 個 `#[ignore]` 移除);本探針 17/17、
上弧 16/16(遷移後)、workspace **1038/0/3**、語料非 pending
全零敗、conformance **91/91**(L2-50~52 關門)。

diff 審查:存取臂=closed miss 分流於 %-豁免與 F4 開放 catch-all
之間,私有攔截序保留;合併面=雙向 force+collapse 判 Top(Thunk
(Top) 看穿),真非 Top 外來鍵照拒;聯集面零改動自動綠(G4 機構
如預測)。

對抗性邊界(工單外,全正):私有攔截優先於 missing(`{{~s:1}}.~s`
→ #private_access_violation)✓;⊥ 後續導航合成性(`(cc.b).zz`
留因)✓;聯集雙 ⊥ 主因果 #missing_key ✓;cocoon×cocoon 非 Top
外鍵照拒 ✓;未定義名欄=Top 無約束放行(開放世界一致)✓;
繭內開放 combo 保持開放(本徵態=節點級不遺傳)✓。記錄一筆:
`cc.~%foo` → ⊥ #missing_key(節點級本徵態含系統段;根
`~%Config` L2-23 不受影響,可接受)。

模型 #3 檔案:零代修第十四例;停報紀律=首次正確觸發。
