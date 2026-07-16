# 工單:互指/自指整流(SPEC_12 §1.1 修訂+帶因 Top)(2026-07-16)

**執行者**:模型 #3
**驗收者**:專案腦(探針已預先提交,紅線不可動——**全部**探針檔皆紅線。
若交付中發現任何既有釘因新法必紅:**停下報驗收方**;單方遷移計代修。)
**探針**:`crates/interpreter/tests/static_cycle_probe_test.rs`
(9 紅門 + 7 釘;已校準:紅門今日全紅、釘今日全綠)

**驗收 = 紅門全綠 + 釘全綠 + 全套件無退化(基線 1038 −3 開單遷移
+ 本探針 16 測 = 應 1051)+ 語料非 pending 全零敗 + conformance
全綠(含新增 L2-53~55 紅、L2-56 綠法釘,交付時應 95/95)。**

**開單時已遷**(驗收方,本 commit):`pin_ref_cycle_still_divergent`
(forward_ref)、`l217_self_identity_divergent`、
`l217_path_cycle_divergent`(divergence_probe)——純引用形轉合法
Top。變換形釘(`a: b+1` 等)全留紅線。

**紀錄義務**:交付紀錄先寫進本檔再回報。

---

## 0. 法(裁定 2026-07-16 已批入法)

SPEC_12 §1.1 修訂+SYNTAX_08 §4 #2 例外+ERROR_CODES #static_cycle:

1. **靜止循環**=閉環每跳皆**純引用**(裸名或純路徑;投影不增
   資訊)→ **Top**(解集=全集)。
2. **變換循環**=任一跳非純引用(算術/比較/應用/管道/字面量
   建構)→ **⊥ #divergent**(解集=∅;§2.1 自例 `a: a+1` =
   L2-17 正典,不動)。
3. **層無關性**:root 與 combo 同律(合成性)。
4. **帶因 Top**:靜止循環之 Top 攜 `%cause: #static_cycle` +
   環成員(自指=環長 1、互指=環長 2,單一標籤);守欄=格律
   中立/不傳播/顯示 `_`。

## 1. 量測(v0.2.15,按層分裂雙向反律)

| 形 | root | combo | 法定 |
|---|---|---|---|
| 靜止互指 `a:b, b:a` | ⊥ ✗ | `_`(無因)✗ | Top+因 |
| 靜止自指 `x: x` | ⊥ ✗ | `_` | Top+因 |
| 純路徑環 `s:{v:s.v}` | ⊥ ✗ | — | Top+因 |
| 變換自指 `a: a+1` | ⊥ ✓ | `_` ✗ | ⊥ #divergent |
| 變換互指 | ⊥ ✓ | `_` ✗ | ⊥ #divergent |
| 帶因 Top 么元 `a & 5` | ⊥ ✗ | — | 5 |
| 帶因 Top `= _` | #false ✗ | — | #true |

根因:root 之 `computing`/`in_flight` 再入一律鑄 ⊥、combo 之
`lexical_forcing` 軟再入一律回 Top——**兩套機構都缺「變換判別」**。
「再入」不是發散,「再入+變換」才是。

## 2. 地圖與實作建議

1. **純引用判準**(共用):一跳的 expr 為 `ExprKind::Path`(任意
   錨,僅段)=純;其他一切=變換。建議 helper `expr_is_pure_ref`。
2. **鏈染色**:force/force_coord 沿當前強制鏈追蹤「自環入口以來
   是否經過變換跳」——可在 ctx 加 taint 計數/集合(仿
   lexical_forcing 手法,零新持久狀態)。
3. **再入分流**(兩處同律):
   - root 再入點(in_flight/computing 命中):鏈純 → 帶因 Top;
     鏈染 → ⊥ Divergent(現行為)。
   - lexical_forcing 軟再入點:同一分流(現行一律 Top 改分流)。
4. **帶因 Top 表示**:工程裁量。守欄(全在紅門/釘):
   - `Value::PartialEq`:帶因 Top == 裸 Top **必須成立**(workspace
     `cycle_test::test_static_cycle` 斷言 `forced_a == Value::Top`,
     非紅線檔但不得破;格律等值紅門同此)。
   - unify:視同 Top(么元律紅門 `a & 5` → 5)。
   - 顯示:`_`(to_nlang 同裸 Top)。
   - `%cause` 導航臂:讀 `#static_cycle`(cocoon 鏡像 ⊥ 之
     as_cause_combo 手法,G6 坍縮為標籤);環成員入因果詳情
     (形制裁量,交付紀錄記)。
   - 消費不傳播:任何運算產物=裸 Top(來歷蒸發)。
5. **fmt**:#static_cycle **非** BottomCause(Top 側來歷)——
   BottomCause 凍結不動;帶因 Top 為觀測期值,正常不進 bn_serial;
   若實作發現必須序列化,新 tag 追加(Blur 前例)並於交付紀錄
   說明。

## 3. 邊界與陷阱

1. **L2-17 正典不動**:`a: a+1` → ⊥ #divergent(釘);變換偵測
   不得因分流變鈍(三顆變換釘+conformance L2-17 守)。
2. **混合環=變換環**:`a: b\nb: a+1`——任一跳染色即全環染
   (釘 `pin_mixed_alias_transform_divergent`)。
3. **前向引用勿破**:本單動的正是前向引用弧的機構;`out: a`
   後定義 `a: 5` 必須照活(釘)——前向引用不是環(從未再入)。
4. **普通 Top 不長因**(釘×2):開放 miss/未定義名之 `%cause`
   仍 `_`——帶因僅限靜止循環判定產出。
5. **靜止環不毒鄰**(釘):環外兄弟照常。
6. **深環**:環長 3+(`a:b, b:c2, c2:a`)同律 Top+因——對抗
   驗收會測,紅門未列但法同;環成員名單應含全環。
7. **G6/blur/⊥ 各臂勿動**:%cause 臂新增 Top-側分支,既有 ⊥/Blur
   讀因不受影響(bottom_meta/blur 探針在檔)。
8. 全語料回歸 + conformance L2-53~55(今日三紅)+ L2-56(綠釘)。
9. 交付紀錄:根因、diff、量測(含環長 3 形、帶因表示形制、
   序列化觸發與否)、未動聲明。

## 4. 非目標

- refine_map 儲存層循環(refine_test 釘,另機構)。
- 態射遞迴/燃料視界(G3 律,勿動)。
- Blur 展開源、~% 影蓋、cause 正典審計(各自另案)。
