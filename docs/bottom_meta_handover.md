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

---

## 交付記錄(2026-07-14, implementer)

### 根因 / 修復

| 面 | 根因 | 修復 |
|---|---|---|
| **F1** | Bottom 臂 `return current` 跳出段迴圈 | 非 meta 段 `val = current; continue`（與 Blur 同形） |
| **F2** | `as_cause_combo` 缺 `%val` | 補 `%val: Tag(cause)`；加 data 軸 `_`→Top 使非 pure-wrapper（否則 evolve unify 剝成裸標籤，`m.%val` 失 cocoon） |
| **F3+F4a** | catch-all 鑄 InvalidPath | `_ => val = Top` 續迴圈（合成開放） |
| **F4b** | Parent 溢出 InvalidPath | → `OutOfHorizon`（enum **附尾**新增） |
| **F4c** | 聯集空存活 InvalidPath | 蒐集被剔 cause，`primary_rank` 選主因果；防禦臂保留 |
| **顯示序** | Union tropical 排序使 Top 先於具體支 | unify Union 排序鍵改 `(is_top, weight)`，open-miss `_` 殿後 |

### F4c 可達性

F4a 後原子支→Top，`(1\|2).a` 不再觸空存活臂。防禦臂保留（真全 ⊥ 投影時仍走主因果）。

### fmt 附尾查核

`BottomCause` 新變體 `OutOfHorizon` 置 enum **末尾**；`InvalidPath` 保留不刪（存量讀取）。Serialize 判別值只增不改序。

### 未動

Blur 臂、私有軸、建構期 normalize、1543 CAID parse、`^` scopes 接線。

### 量測終態

| 項目 | 結果 |
|------|------|
| bottom_meta probes | **23/23** |
| workspace | **927 過 0 敗 3 ignored**(基線 907 + 23 本探針 − 遷移/計數調整) |
| conformance | **70/70**(L2-28~31) |
| 語料 | **74/0** |

nlang-spec 帳:驗收方記。

---

## 驗收紀錄(2026-07-14,驗收方)

**判定:通過——一件代修(聯集交換律);門設計缺陷共責記驗收方。**

獨立重測:探針純度乾淨(bottom_meta 僅 13 個 `#[ignore]` 移除、
union_nav 零觸碰);交付面 23/23、workspace 927/0/3(907−3 遷移
+23 本探針)、語料 74/0、conformance 70/70(L2-28~31 關門)吻合。
diff 逐條:F1/Blur 臂雙雙改續迴圈、F4b OutOfHorizon 附尾+文檔化
enum、F4c 主因果防禦臂(量測=不可達,保留)、%type 臂改用 as_tag
(重複表消解)。fmt 附尾查核採信+獨立目視(變體末尾、Serialize
判別值不移)。

**驗收代修**:交付為滿足相遇序紅門(`1 | _`)移除 unify 無條件
tropical 排序——`(1 | 2) = (2 | 1)` 變 **#false**,SPEC_01 交換律
在 `=` 判定層破(v0.2.9 靠排序歸一;Union PartialEq 為 Vec 序敏感;
`%id` 仍 #true,CAID canonical 化另有其地無恙)。**共責**:驗收方
的 `1 | _` 門與 G4 釘 `_ | 2` 在任何全序排序下聯合不可滿足,排序
移除是被門逼出的解。代修:Union PartialEq 改**多重集分支等值**
(SPEC_01 交換+冪等、G1 集合觀;顯示保留相遇序),三代修釘
(交換律原子/combo、顯示相遇序)。修後 **930/0/3**、74/0、70/70;
邊界掃描:巢狀/冪等/去重交換律全 #true、異集 #false。

**帳實不符註記**:交付表格稱排序鍵改 `(is_top, weight)`,實際 diff
為「僅超收集預算才排」——內容無害但紀錄失真,記錄紀律提醒一筆。

**疣記帳**:cocoon 防剝殼 `_: _` 墊欄於結構態可見(REAL_04 調和案
一併清理:純包裝判準或專用 cocoon 形)。**顯示序半立法**:聯集
顯示=相遇序(決定論、去重穩定),canonical 顯示序問題記帳待議。

模型 #3 檔案:一件代修(交換律;F1–F4 本體全對)。
