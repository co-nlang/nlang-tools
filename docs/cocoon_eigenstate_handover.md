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
