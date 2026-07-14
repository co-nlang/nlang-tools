# 工單:Blur 邊界律(座標語境吸收 + `=` 二段律 + `%caid` meta)(2026-07-14)

**執行者**:模型 #3
**驗收者**:專案腦(探針已預先提交,紅線不可動——**全部**探針檔皆紅線。
若交付中發現任何既有釘因新法必紅:**停下報驗收方**,由驗收方修釘;
單方遷移直接計代修。)
**探針**:`crates/interpreter/tests/blur_boundary_probe_test.rs`
(9 紅門 + 11 釘;已校準:紅門今日全紅、釘今日全綠)

**驗收 = 紅門全綠 + 釘全綠 + 全套件無退化(基線 887/0/3 + 本探針 20 測)
+ 語料 74/0 + conformance 全綠(含新增 L2-24~27,交付時應 66/66)。**

註:G3 釘 `pin_lattice_eq_blur_current_behavior`(凍結 `big = 1` → #false)
已由**驗收方**於本開單 commit 遷移(其凍結條款明文「另案解凍」,本案
即該案);後繼紅門 = `red_eq_blur_vs_value_absorbs`。

---

## 0. 裁定(已批;SPEC_08 §3.2.2 #5/#6 + #4 擴充已入法)

G3 只立值語境。量測收口三謊:導航 `big.name` → ⊥ #invalid_path
(僭稱知道路徑無效)、`bigA = bigB` 同文異綁定 → #false(真值皆
4000,可證同值)、聯集導航靜默剔 blur 支(視界痕跡消失)。

- **#5 座標語境吸收**:非 meta 段導航遇 `#blur` → 原樣傳出
  (cause/CAID 保全,效果 max)。聯集逐支導航下 blur 支投影仍為
  blur 支**存活**;僅 ⊥ 支剔除(SPEC_07/G4 投影律不變)。
- **#6 `=` 二段律**:固化後任一側 `#blur`:(a) 雙 blur 且 **CAID
  相同** → `#true`(觀測決定論;與聯集去重同一關係——G1 唯一等值
  之延伸);(b) 其餘 → 左優先吸收(**不得** `#false`)。
- **#4 擴 `%caid`**:`#blur` 之 `%caid` meta 觀測回快照 CAID 字串
  (與 `%cause`/`%type` 同白名單;快照身分全可判比較
  `x.%caid == y.%caid` 由此免費獲得,無新語法)。

## 1. 地圖與實作建議

1. **navigate_segments**(lib.rs):meta 特殊段(`%cause`/`%type`)的
   Blur 臂**旁**補 `%caid` 臂(回 `Value::Atom(Str(caid))`);非 meta
   段遇 `Value::Blur` → 原樣回(在鑄 InvalidPath 之前短路)。既有
   結構標記/純包裝解殼迴圈不動。
2. **聯集導航**:G4 逐支投影臂——量測先行:#5 落地後 blur 支投影
   應自然回 blur;確認剔除邏輯**只**剔 Bottom(若 blur 支被剔,最小
   修該臂,勿改 normalize_union)。紅門
   `red_union_nav_blur_branch_survives` 斷言 `1 | #blur` 形。
3. **LatticeEq**(eval.rs ≈694,G1 固化比對處):force_recursive 之後
   加二段律臂——雙 Blur 且 `detail.caid` 相等 → #true;任一側 Blur
   (含異 CAID 雙 Blur)→ 左優先原樣傳出。**⊥ 短路順序不動**(⊥ 先、
   Blur 次,與 G3 各吸收點同序;交付紀錄註明)。
4. **效果標籤**:吸收傳出照 max 合併(BlurDetail.effect 既有)。

## 2. 邊界與陷阱

1. **本體不可互鑄**:Blur ≠ ⊥;吸收=原樣傳出,不得改 cause、不得
   升級成 ⊥。
2. **`==`/math 勿動**:G3 法已覆蓋(釘 `pin_eqeq_blur_absorbs_g3_law`
   守住)。雙 blur `==` 只傳左側 cause = 既有法,不在本單擴充。
3. **`<`/`<=` 勿動**:序判定於 combo/union 全域未實作(§4.10 另案,
   blur 非變因);釘 `pin_lt/lte_blur_frozen_conflict` 凍結 #conflict。
4. **%caid 是字串**:顯示含引號(`"hash:sha256:v1:…"`);紅門只斷言
   含 `hash:sha256:` 前綴(CAID 逐引擎鹽化,不可釘全值)。
5. **`big = big` 同快照 #true 依賴 force memo**:交付若動到 memo
   路徑,釘 `pin_eq_self_same_snapshot_true` 會抓;勿為湊 #true 而
   特判綁定名。
6. **勿動**:blur 顯示形/CAID 公式、`normalize_union` 去重判準、
   Union×`=` 分配語義(cmp×Union 另案)、fuel 量級。
7. 全語料回歸 + conformance L2-24~27(24/26/27 今日紅;25 今日綠=
   法釘)。
8. 交付紀錄照舊格式(根因、diff、量測、未動聲明)。

## 3. 非目標

- `<`/`<=` 序判定實作(§4.10 弧)。
- cmp×Union 分配、`%kind`/`%id` meta 實體化(G6 鄰接另案)。
- 同引數 #divergent 偵測升級、`~%` 影蓋靜默(各另案)。

---

## 交付記錄(2026-07-14, implementer)

### 根因 / 修復

| 面 | 根因 | 修復 |
|---|---|---|
| **#5 座標導航** | 非 meta 段遇 Blur 落 `_ => InvalidPath` | `navigate_segments` Blur 臂:非 meta 原樣傳出(effect max);meta 旁補 `%caid` → 快照 CAID 字串 |
| **聯集導航** | blur 支投影曾鑄 InvalidPath(=⊥)→被剔 | #5 落地後投影回 blur,G4 剔除邏輯仍只剔 Bottom——零改 normalize_union |
| **#6 `=`** | 異 CAID 雙 blur / blur×值 走 PartialEq → #false | LatticeEq:⊥ 臂先(set 家族布林);雙 blur 同 CAID → #true;其餘含 blur → 左優先吸收 |

### 順序

LatticeEq / 各吸收點:**⊥ 先、Blur 次**(與 G3 同序)。

### 未動

- `==`/math(G3 釘綠)、`<`/`<=` 凍結 #conflict、blur 顯示/CAID 公式、normalize_union 去重、fuel 量級、memo 路徑(無特判綁定名)

### 量測終態

| 項目 | 結果 |
|------|------|
| blur_boundary probes | **20/20** |
| workspace | **906 過 0 敗 3 ignored**(887 基線 −1 G3 已遷釘 +20 本探針) |
| conformance | **66/66**(L2-24~27) |
| 語料 | **74/0** |

nlang-spec 帳:驗收方記。

---

## 驗收紀錄(2026-07-14,驗收方)

**判定:通過——一件代修;無協議違規。**

獨立重測:探針純度乾淨(diff 僅 9 個 `#[ignore]` 移除);交付時
探針 20/20、workspace 906/0/3、語料 74/0、conformance 66/66
(L2-24~27 關門)全數吻合。

diff 逐條:LatticeEq 二段律照序(⊥ 段先且語義原封、雙 blur 同
CAID → #true、其餘左優先吸收 effect max);navigate_segments
%caid 臂回快照字串、非 meta 段吸收;聯集導航零改 normalize_union
(#5 落地後 blur 支投影自然存活——與工單「量測先行」指示相符)。

**驗收代修**:交付的吸收臂 `return` 跳出段迴圈,丟棄剩餘路徑段——
內聯 `big.name.%cause` 回整個 #blur,而綁定拆兩步回 #fuel_exhausted:
**導航合成性(x.a.b ≡ (x.a).b)被破壞**(紅門只測了綁定形,設計
盲點在驗收方)。代修:吸收改 `val = blur; continue`,後續 meta 段
照答;代修釘 `pin_nav_blur_compositional`(內聯 `big.name.%cause` →
#fuel_exhausted)。修後全套件 **907/0/3**、語料 74/0、66/66。

對抗性邊界(工單外):`⊥ = blur` 兩向皆 #false(照工單預裁 ⊥ 先序,
與 G3 同;註:blur 加燃料後可能收斂為 ⊥,此 #false 屬「⊥ 為空集
運算元」讀法,重開候選記帳)、`(1).%caid` → ⊥ #invalid_path(白名單
無過度擴權)、雙 blur 聯集兩支皆存活不誤去重、`bigA.%caid == bigB.%caid`
→ #false(異快照全可判)、`= @IO` 混效果吸收帶效果。

**既有債曝光**(非本交付引入,不擋驗收):⊥ 之非 meta 導航同樣
跳出段迴圈——內聯 `bad.name.%cause` 回 ⊥ 原樣、綁定拆步回 cause
combo,同一合成性歧異(另案候選:nav×⊥ 合成性;且 ⊥.%cause 兩形
回覆形制不一)。

模型 #3 檔案:一件代修(合成性;吸收語義本體正確)。
