# 工單:⊥ meta 觀測整流 + `#invalid_path` 廢止(2026-07-14)

**執行者**:模型 #3
**驗收者**:專案腦(探針已預先提交,紅線不可動——**全部**探針檔皆紅線。
若交付中發現任何既有釘因新法必紅:**停下報驗收方**,由驗收方修釘;
單方遷移直接計代修。)
**探針**:`crates/interpreter/tests/bottom_meta_probe_test.rs`
(13 紅門 + 10 釘;已校準:紅門今日全紅、釘今日全綠)

**驗收 = 紅門全綠 + 釘全綠 + 全套件無退化(基線 907/0/3 + 本探針 23 測)
+ 語料 74/0 + conformance 全綠(含新增 L2-28~31,交付時應 70/70)。**

註:G4 檔 `union_nav_probe_test.rs` 三處(`red_union_nav_bottom_branch_dropped`
/`pin_union_nav_all_bottom_is_invalid_path`/`pin_atom_nav_invalid_path`)
已由**驗收方**於本開單 commit 遷移(#invalid_path 廢止使其前提失效),
後繼紅門在本探針檔。該檔其餘測試原封,照常紅線。

---

## 0. 裁定(已批;全引擎追法,法源俱在)

- **F1 合成性**:⊥ 之座標導航是左摺(`x.a.b ≡ (x.a).b`)。⊥ 臂
  跳出段迴圈與 Blur 代修同蟲異臂——內聯 meta 讀被丟棄。
- **F2 `%cause` 對偶**(REAL_04 §1 + SYNTAX_08 §4 #3):`%cause` 是
  Cocoon 含 `%val: #Tag`;直接觀測坍縮為標籤、`<<路徑>>` 保全因果
  鏈。引擎診斷 combo 有 `%type` 無 `%val`——**補 `%val` 即可**,
  G6 值語境投影自動完成兩態。
- **F3 無因開放**(SYNTAX_08 §4 #2):未坍縮節點 `%cause` → `_`。
  combo 缺欄早已回 `_`;只有原子走 catch-all 中毒——與 F4 同一刀。
- **F4 `#invalid_path` 廢止**(未立法之引擎誤鑄;ERROR_CODES/REAL_04
  已加廢止注記):
  - 導航 catch-all(原子/Top)→ **`_`**(原子資料軸可 `&` 混血擴欄
    =開放世界);
  - `^` 溢出 → **`#out_of_horizon`**(ERROR_CODES §1 正典);
  - 聯集全 ⊥ 存活 → **REAL_04 §4 主因果**(優先級:#divergent >
    #effect_violation > #conflict > 資源邊界 > #not_found);
  - G4 條款修訂:原子支=開放缺欄,**保留**如 Top-miss 支
    (`({a:1}|7).a` → `1 | _`)。

## 1. 地圖與實作建議

1. **F1**(lib.rs Bottom 臂 ≈1429 `return current`):改 `val = current;
   continue;`(與 Blur 代修同形——見 blur_boundary 弧 `ceaef12`)。
   `%cause`/`%type` 特殊段檢查保持在前。
2. **F2**(`as_cause_combo`,value.rs):補 `%val: Tag(cause)` 欄。
   勿動其餘診斷欄(%expected/%found/%involved/%message/%type 照舊)。
   顯示坍縮由 observe 出口投影自動處理(G6 機制,勿另寫)。
3. **F3+F4a**(lib.rs:1508 `_ => return InvalidPath`):改
   `_ => { val = Value::Top }` 續迴圈(合成性:`(7).a.b` 兩段皆開)。
4. **F4b**(lib.rs:1368 Parent 錨定):InvalidPath → `OutOfHorizon`
   (**BottomCause 新變體,附尾新增**)。
5. **F4c**(lib.rs:1497 聯集空存活):蒐集被剔支之 cause,依 REAL_04
   §4 優先級選主因果鑄 ⊥。量測義務:F4a 落地後此臂可能不可達
   (即時 ⊥ 支已於建構期 normalize 剔除、原子支不再鑄 ⊥)——交付
   紀錄報告可達性;不可達也**保留防禦臂**(勿刪)。
6. **BottomCause enum**:變體**只增不刪**(fmt v2 凍結)。InvalidPath
   保留(存量宇宙讀取;顯示/`as_str` 照舊)、引擎停鑄。交付紀錄附
   fmt 序列化判別值附尾安全性查核一筆。

## 2. 邊界與陷阱

1. **`%type` 於 ⊥ 回標籤不變**(釘);F2 只動 `%cause` 的 Cocoon 形。
2. **Blur 臂勿動**(blur_boundary 弧已法;釘守)。
3. **私有軸勿動**:`p.~s` → `1` 是**現況凍結釘**(未實施=另案;
   catch-all 改造時勿順手擋)。
4. **`^` 有效形今日也落溢出臂**(scopes 僅態射派發時填,觀測語境
   未接=另案):改標後有效形回 ⊥ #out_of_horizon 仍誠實;**勿**
   在本單接 scopes。
5. **建構期 normalize 勿動**(⊥ 支建構剔除+空聯集 #conflict 照舊,
   釘 `pin_union_bottom_build_dropped` 守)。
6. **1543 CAID parse 勿動**(cause 正典審計另案)。
7. 全語料回歸 + conformance L2-28~31(今日四紅)。
8. 交付紀錄照舊格式(根因、diff、量測、未動聲明;含 F4c 可達性
   量測與 fmt 附尾查核)。

## 3. 非目標

- `^` 錨定於觀測語境之解析補全(另案)。
- 私有軸實施(SPEC_04 §61,另案)。
- G4 惰性 ⊥ 支剔除(thunk 漏,另案)。
- cause 正典審計(BottomCause 全表 vs REAL_04,另案)。
- REAL_04 完整 Cocoon 欄位集(message/path/line/trace…調和,另案)。
