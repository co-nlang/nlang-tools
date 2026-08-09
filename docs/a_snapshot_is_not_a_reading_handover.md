# O42 交接:一個快照不是那把尺的讀數

開弧 2026-08-09。基線 `dev`(v0.16.0 定版後),workspace **1822 / 0 / 3**。
探針 `crates/oo/tests/snapshot_not_a_reading_probe_test.rs`(已預先提交並校準)。

---

## 0. 一句話

> **一個快照的身分必須是那個快照,不是量它的那把尺的讀數。**

`#blur` 的 CAID 今天由「剩下多少燃料」與「現在幾點」決定,而不是由「停在哪裡、
在什麼條件下停的」決定。REAL_03 §7.3 早就寫死了它該是什麼,引擎六項強制輸入
一項也沒做對,並且放進了一項該條文明文禁止的東西。

---

## 1. 這不是設計題,是合規題

REAL_03 §7.3 **CHS 封套**:

```
node_content + "#horizon:" + canonical_json([params])
```

| §7.3 要求 | `BlurDetail::blur_caid()` 實際 |
| :-- | :-- |
| `node_content`(即 `partial`) | **缺** |
| `%fuel`,明寫「允許消耗的計算資源**上限**」 | 放的是 `fuel_remaining` **剩餘讀數** |
| `%strategy` | ✓ |
| `%max_branches` | **缺** |
| `%max_unification_depth` | **缺**(連造成該 blur 的那個上限都沒記) |
| `%max_pattern_nodes` | **缺** |
| `%max_lifting_depth`(O43 新增,見 §3) | **缺** |
| **嚴禁** `%timeout`,理由「物理時間不穩定,不具備決定論」 | 放了一個**時鐘鹽** |

鹽的來歷是 `0e8d1f3`(Phase 9-14 整批),早於規格。規格從頭到尾沒有給它位置——
SPEC_01 §2.4.1 提到「實例鹽」只為了**禁止**依賴它。

### 1.1 O37 使這件事承重

`~%Config` **不進提交**(v0.15.0 裁定,`commit` 會印 `note: ~%Config was not
committed`)。所以一個存下來的 blur,**是它那次觀測條件的唯一記錄**。
而它今天沒有記下那些條件。

「視界參數不進歷史(O37)」與「視界參數必須進 CAID(SPEC_08 §3.2.1)」不是張力,
是互補:參數之所以能不進宇宙欄位,正因為它們住在 blur 的身分裡。

---

## 2. 量到的後果

**(a) 同一份原始碼,提交出三個不同的宇宙。** 三個全新倉,evolve + commit,root digest:

```
控制(純值)         4c45e486…  4c45e486…  4c45e486…   相同
控制(_|_ 1/0)      08bb39de…  08bb39de…  08bb39de…   相同
一個 depth #blur   2f559b90…  563566a6…  8a069506…   三次三樣
```

**(b) 一個無關欄位隔空改寫另一個 blur 的身分。** `p` 一字未動:

```
p 單獨                 8b3628d6…
前面多一個同樣的 blur    0132f6dc…
前面多一個無關的加總      8dc07dad…
```

SPEC_01 §2.4.1 早已把這個病判為**違法**(「無關欄位隔空改寫拼法」),而且為此把
`%caid`/鹽排除在顯示排序鍵之外——**卻留下了 `剩餘燃料` 當第 5 項排序鍵**,那是同一個
讀數。該律自己的鍵違反該律自己的目標。

**(c) 機制分裂:今天一個 blur 有沒有身分,取決於它是被哪一種資源攔下的。**

鹽只有一個鑄造點(`storage.rs get_horizon_salt`)與一個呼叫點(`universe.rs`,
evolve 路徑)。實測:

| 鑄造點 | 鹽 | 可重現 |
| :-- | :-- | :-- |
| **燃料側**(observe 期,`observation.rs`) | 固定 `sha256("default")` | ✓ `f41c3b06…` 三倉相同 |
| **合一側**(evolve 期,`unify.rs` / `math.rs`) | **時鐘** | ✗ 每次不同 |

`universe.rs` 自己的註解就寫著「fuel-side blur CAIDs keep their fixed observe salt」
——分裂是知情的,但沒有人問過另一半怎麼辦。

---

## 3. 裁定(用戶已批,2026-08-09)

**R-1 鹽刪除。** `HorizonParams.salt` 與 `Storage::get_horizon_salt()` 移除。

**R-2 `fuel_remaining` 退出身分。** 欄位**保留**(合法的執行期來歷,`unify.rs:414`
在用),但不得進入任何雜湊。身分改用 `%fuel` **上限**。

**R-3 `partial` 進入 CHS**,且**逐視界記錄**進,不是全域一份(見 R-6)。

**R-4 六項強制參數(O43)**:`fuel` / `strategy` / `max_branches` /
`max_unification_depth` / `max_lifting_depth` / `max_pattern_nodes`。
`timeout` 依 §7.3 排除,理由改寫為**它是唯一非離散的視界參數,機器上不可重現**
——比原文的「物理時間不穩定」精確,並說明排除它不是妥協。

**R-5 三處編碼合一。** 今天 blur 有三套不同的雜湊輸入:

| 位置 | 輸入 |
| :-- | :-- |
| `value.rs blur_caid()` | cause + fuel_rem + strategy + salt |
| `bn_serial.rs:113`(身分編碼) | cause + fuel_rem + strategy + salt + partial |
| `value.rs hash_recursive_with_salt` | cause + fuel_rem + salt(**連 strategy 都沒有**) |

三者必須由**同一個 CHS 函數**導出。

**R-6(O46)合併保留兩者,結構為正準排序的記錄集合。**
每筆記錄 = (cause, 六項參數, partial)。合併兩 blur = 記錄集合取聯集。

- **不是 meet**:會靜默改寫存下來那筆的條件,而 O37 之後那是唯一的一份;
  SPEC_08 §3.2.2 第 4 款「穩定性承諾」正在禁止這件事。
- **不是 tuple**:有序 ⟹ `x & y` ≠ `y & x` ⟹ **位置又進了身分**,正是本弧要修的病。
- **不是 `a | b`**:union 是「其一」,而事實是「兩個視界**都**碰到了」。
- **是集合**:§7.3 的封套本來就寫成 `canonical_json([params])`,已經是陣列;
  擴成正準排序集合是格式的最小延伸,免費拿到交換/結合/幂等,同一次觀測內的兩筆
  因參數均勻而自動去重成一筆。

`.%cause` **仍回單一標籤**,依 **REAL_04 §4 主因果優先級**投影;同位階的 tie-break
對 blur 用**集合的正準序**(§4 原文寫「相遇序最左」,那又是位置)。**不動 §4 對 ⊥ 那條。**

**R-7(O47)吸收不得改寫快照。** `unify.rs` 的三個 blur 分支目前把另一側揉進
`partial`。SPEC_03 §90 明文:展開 blur 使目標變為「該 `#blur` **原樣**(cause／
**CAID**／視界參數保全)」,並自陳由 `{b:1, ...big} ≡ {b:1} & unbox(big)` 導出。
今天合規**只因為 CAID 看不見 partial**;R-3 一落地就變成活的違反。
§90 的理由也已寫明:視界後來源的欄位集合**不可知**,揉合是在斷言你不可能知道的事。

---

## 4. 這一弧不能拆

我提過拆成「拿出不該在的」與「放進該在的」,**自己推翻**:

拿掉 `fuel_remaining` 就拿掉了今天那個**意外鑑別器**。partial 不在身分裡 ⟹
同一次觀測內同因的任兩個 blur **撞號** ⟹ SPEC_08 §3.2.2 第 6 款 (a) 判它們 `#true`。
所以 R-2 逼出 R-3,R-3 逼出 R-6/R-7。**一弧。P1 是守這件事的釘。**

---

## 5. 射程

**必動**:
`crates/interpreter/src/value.rs`(`HorizonParams`、`BlurDetail::blur_caid`、
`hash_recursive_with_salt` 的 0xFD 分支、§2.4.1 排序鍵所在的比較器 ~708)、
`crates/interpreter/src/bn_serial.rs`(0xFD 分支)、
`crates/interpreter/src/storage.rs`(刪 `get_horizon_salt`)、
`crates/interpreter/src/universe.rs`(刪鹽指派)、
`crates/interpreter/src/lib.rs`(`EvalContext.horizon_salt`)、
`crates/interpreter/src/observation.rs`、`unify.rs`、`builtins/math.rs`(鑄造點)。

**不得動**:`crates/oo/tests/*probe_test.rs`(探針修改權在驗收方;
`#3` 只能移除 `#[ignore]`)。

**不在射程**:
- **O45** `%partial` 的可觀測性(身分與可見性是兩件事)。
- `observation_result.md` 開放問題 1(Blur 該不該降為標註)——它的配重要等本弧
  做完才重新存在。
- `#incomplete` 的任何事。
- REAL_04 §4 對 ⊥ 的「相遇序最左」。

---

## 6. 探針

`crates/oo/tests/snapshot_not_a_reading_probe_test.rs`,校準於 `dev`(v0.16.0):
**7 綠 / 7 紅**。

| | 斷言 | 基線 |
| :-- | :-- | :-- |
| C0 | 無 blur 的宇宙三倉同 root | 綠 |
| C1 | 含 `_|_` 的宇宙可重現 | 綠 |
| C2 | blur 仍被鑄造,`%cause` 仍是單一標籤 | 綠 |
| **C3** | **燃料側 blur 今天就可重現** | 綠 |
| P1 | 內容不同的兩個 blur 不得撞號 | 綠(**意外地**,見下) |
| P2 | 旋鈕是快照條件的一部分 | 綠(**意外地**) |
| P3 | 無 blur 的宇宙 root 不得改變 | 綠(釘死 `4c45e486…`) |
| R1 | 同源三倉同 root | 紅 |
| R2 | 同源三行程同 `%caid` | 紅 |
| R3 | 無關欄位不得移動 blur | 紅 |
| R4 | 兩個逐字相同的運算式是同一個快照 | 紅 |
| R5 | §3.2.2 第 6 款 (a) 可達(`p == q` → `#true`) | 紅 |
| R6 | 合併交換律(O46) | 紅 |
| R7 | 吸收不得改寫快照(O47) | 紅 |

### 6.1 三件校準期抓到的事,寫下來免得下次重犯

1. **C3 是本弧最重要的控制。** 沒有它,七支紅都可以被讀成「blur 的身分本來就沒救」,
   而實際上有一半今天就是對的。它也是那一半的迴歸守衛。
2. **P1 / P2 今天綠得是別的機制。** 兩者今天靠 `fuel_remaining` 與鹽的漂移而綠,
   交付後必須靠 `node_content` 與六項參數而綠。**斷言不變,底下的機制被換掉。**
   已寫進探針註解——一個沒有說出為什麼會動的釘,下次還是會被人靜靜地改。
3. **R3 / R4 今天紅在兩個缺陷上**(鹽 + 讀數),不是校準不良:兩個成因都在本弧內,
   而且**燃料側 fixture 造不出讀數缺陷**——燃料耗盡時 `fuel_remaining` 恆為 0。
   寫在探針檔頭,免得日後有人「發現」這個糾纏而判定探針失準。

### 6.2 我原本對 O42 的描述有一半是錯的

先前記為「`#blur` 的 CAID 是 `sha256(now_nanos)`」。**observe 路徑不是**——那裡
是固定鹽,三行程逐位元組相同。錯誤來自讀 `storage.rs` 而沒有量呼叫點。
`docs/a_limit_you_cannot_choose_handover.md` 與該弧探針檔頭沿用了這句話,
本弧一併更正(僅註解)。

---

## 7. 成功標準

1. 七支紅全綠,七支綠仍綠(**只准移除 `#[ignore]`**)。
2. workspace **1836 / 0 / 3**。算式:交付前基線(探針已入樹)**1829 / 0 / 10**
   〔= 開弧前 1822/0/3,加 7 支綠、7 支 ignored〕;七支紅轉綠後 1829 + 7 = **1836**,
   ignored 退回 **3**(三支常設)。
3. conformance 143/143、genesis 11/11。
4. **P3 必須綠**——這一弧只准移動含 blur 的宇宙。
5. 五次重跑穩定。
6. 跨版本:v0.16.0 建立的**不含 blur** 的倉,新引擎讀得到且 root 不變;
   含 blur 的舊倉**會**變(破壞性,見 §8),須實測並記錄雙向行為。

---

## 8. 破壞性:條目 #11

每個 blur CAID 與每個含 blur 的 root CAID 都會動。90 天時鐘重啟。

CHANGELOG 必須說清楚兩件事,否則讀者會以為我們弄壞了可重現性:

- **合一側的那些 CAID 今天本來就是隨機數**,本弧是把隨機數換成穩定值;
- **燃料側的 CAID 會改值但不改性質**——今天可重現,之後仍可重現(C3)。

版號走 **minor**(語義變更;major 保留給 v0.500.0 委員會錨點)。

---

## 9. 收尾分工

- **規格條文由驗收方寫**,不進交付範圍。本弧要動:REAL_03 §7.3(六項、
  timeout 排除理由、`#eager/#lazy/#balanced` 筆誤更正為 `#blur/#strict/#approximate`
  ——**O44 已裁為規格側筆誤**,引擎 `universe.rs:98` 白名單與
  `config_validation_probe_test.rs` 都明文拒絕 `#eager`,而全規格只有 §7.3 這一格是孤例)、
  SPEC_01 §2.4.1 第 5 項排序鍵、SPEC_03 §90 按語、SPEC_08 §3.2.2 第 6 款、
  REAL_04 §4 blur tie-back、ENGINE_SYNC。
- **交付方**只動引擎與 `CHANGELOG` 以外的引擎側文件註解。
- **CHANGELOG 分類條目不得進入交付範圍。**

---

## 10. 相鄰項(不在本弧,已掛帳)

- **O40** `oo run` / `oo eval` 看不見倉;REAL_01 §1.1 記載了不存在的 `--load`/`--commit`。
- **O45** `%partial` 不可觀測——引擎全樹沒有 `%partial`,`p.%partial` 依第 5 款吸收。
- **frame size** 每層 ~134 KB,`sub_context` 的 `ctx.clone()` 是第一嫌疑。
- **`observation_result.md` §4 第三列**:`Approximate` 回一個沒有 cause、沒有視界、
  沒有 partial 的**原子**——partial 被當裝飾的第三個實例,本弧不修但同族。

---

## 11. 驗收(驗收方,2026-08-09/10)——**不通過,代修清單如下**

### 11.1 量測

| 項 | 結果 |
| :-- | :-- |
| 全套(獨立重跑) | **1834 / 0 / 3**(工單寫 1836,差額見 11.2) |
| 本弧探針 | 14/14,**五次重跑穩定** |
| 交付範圍 | 引擎 10 檔;本弧探針**只移除 `#[ignore]`**,乾淨 |
| 跨版本 | v0.16.0 舊倉(純值/含 blur)新引擎**皆可讀**;破壞面是新提交的 CAID,不是舊倉可讀性 |

差額算式:1829 + 7(紅轉綠) − 3(被刪的釘) + 1(`blur_test.rs` 新增) = **1834**。

### 11.2 必修一:交付重新打開了 v0.16.0 昨天關上的那一格

`Cargo.toml` 開 `serde_json/unbounded_depth`,`storage.rs:18` 呼叫
`disable_recursion_limit()`。A/B,同機同時同 fixture(**即 v0.16.0 驗收帳所用者**:
`max_unification_depth: 4294967295` + 長鏈):

```
             terms=5000  6000  8000  12000
交付前 ba10853    0      0     0      0
交付後            0      0    134    134     ← overflowed its stack
```

`limit_you_cannot_choose` R1 用 5000 項,**正好在新門檻底下**,所以它綠著。

**交付註解給的正當化理由,在交付前的樹上不成立。** 註解稱沒有該旗標時
「status then lies that the universe is static」。實測交付前同一輸入:

```
big: _|_ (%cause: #stack_overflow)  ;; Implementation recursion limit exceeded
```

`status` 沒有說謊,回的正是 v0.16.0 為此造的名字。⟹ **serde 的 128 層守衛一直在
做 `HARD_RECURSION_LIMIT` 對求值器做的那件事。**

界線劃對之處記一筆:`from_json_deep` 三個呼叫點全屬本地儲存,
**OODP 網路入口仍走有界的 `from_str`** ⟹ 非遠端可觸發。

### 11.3 真正的成因:交付把整棵運算式樹塞進了每一個 blur

同一支程式,`.oo/staged` 的 JSON:

| 項數 | 交付前 | 交付後 |
| :-- | :-- | :-- |
| 200 | 深度 10 / **652 B** | 深度 **517** / 19,874 B |
| 1000 | 深度 10 / **651 B** | 深度 **2,917** / 113,503 B |
| 5000 | 深度 10 / **655 B** | 深度 **14,917** / **591,432 B** |

交付前不論運算式多大,staged 恆為 10 層、~650 B。交付後深度與大小**隨運算式線性成長**
——5000 項時 **900 倍**。serde 的上限當然咬得到:**咬的是引擎自己剛做出來的東西。**

⟹ #3 撞到的是真牆,**但那道牆是本弧自己蓋的**。修法方向因此不是拆守衛。

### 11.4 為什麼「回報 `#stack_overflow` 就好」是錯的(用戶提問導出)

`#stack_overflow` 是**實作上限**,§2.7.3 昨天才裁定它**不進旋鈕表**。所以使用者
**沒有旋鈕可調、沒有補救可做**——倉裡有一個永遠讀不回來的東西。這違反 W4′ 立的
「名字要指向補救」。**一個指向不了補救的名字不是錯誤訊息。**

### 11.5 裁定(用戶 2026-08-10):走 (b) — partial 以 CAID 入 blur

> **裁定原文是「partial 進入身分」。身分 ＝ 雜湊輸入。這不蘊含「partial 必須原地
> 塞進每一個 blur 的 JSON」。**

REAL_03 §7.3 的 `node_content` 要的是內容**參與雜湊**,而本系統處理內容的方式
從第一天起就是內容定址。

**可滿足性已檢查**(工單自身須先過這關):

* `CAID(partial)` 是**純函數**(`content_hash()`),**不需要 store** ⟹ 鑄造點
  (`observation.rs` 的自由函數、`unify.rs`/`math.rs` 的 `impl Ouroboros`)
  **不必被穿線改簽名**,雜湊面零阻力。
* 〔量〕`run` 與 `evolve` **今天寫 0 個物件**,只有 `commit` 寫(2 個)。
  所以「本體何時落 CAS」是一個**真的決定**,見 11.6。

### 11.6 一個仍待裁的分支(本體存哪裡)

`.oo/staged` 不是 CAS。若 blur 只存 partial 的 CAID,而本體在 commit 前沒被寫出,
重載時本體就不見了。兩條:

* **(i) evolve 時把本體寫進 CAS。** evolve 從「寫 0 個物件」變成會寫;未提交即
  成為不可達物件,由 v0.2.53 的本地 GC 收。**保留 O45(`%partial` 可觀測)的可能。**
* **(ii) 本體整個不存,只留 CAID。** 最小。今天零可觀測損失——§3.2.2 第 1 款明文
  「`partial` 快照不參與後續運算」、O47 禁止吸收改寫它、O45 說它不可觀測 ⟹
  **本體今天被雜湊、被儲存,此外從不被讀。** 但這會**關掉 O45**。

驗收方傾向 **(i)**:本弧的論旨是「partial 是內容」,而取不回來的內容就回去當裝飾了。

### 11.7 必修二:探針完整性違規 ×2,根因一半在工單

* `crates/oo/tests/name_points_at_remedy_probe_test.rs` P2 釘值被改寫
  (`e4dc016e…` → `4120193…`)。**值正確**(本弧本就移動每個 blur CAID),
  但改寫權在驗收方。
* `crates/interpreter/tests/blur_display_key_probe_test.rs` 整份重寫,**7 支釘剩 4 支**。
  `pin_blur_caid_still_salted` 與 `red_blur_fuel_orders_adversarial_salts` 隨鹽合法消滅;
  **`pin_blur_after_solid_before_top`(族間序)與 `pin_blur_display_text_untouched`
  (顯示文字不動)不是本弧廢止的** ⟹ 淨損失覆蓋,須復原。

**工單兩處洞(記在我這邊)**:

1. §5 的禁令只寫 `crates/oo/tests/*probe_test.rs`,**未涵蓋 `crates/interpreter/tests/`**。
   **新規則:禁令要按性質寫(「任何 `*probe_test.rs`」),不要按目錄寫。**
2. 我在 §8 自陳「每個 blur CAID 都會動」,卻**沒有 grep 誰正在斷言它不動**。
   這是「新增耐久檔須 grep 既有的釘」同一族的**第三次**。
   **升級為機械檢查:凡工單宣告某量會移動,交付前必須 grep 全樹釘住該量的斷言,
   並在工單裡列成「預定改變」。**

### 11.8 分工

* **回 #3**:11.5 的 partial→CAID(含 11.6 裁定後的形狀)、還原 `Cargo.toml`
  與 `storage.rs` 的 `disable_recursion_limit`、確認 v0.16.0 的當機 fixture
  (`depth: 4294967295` × 8000/12000 項)退出碼回到 0。
* **驗收方**:探針復原(11.7)、SPEC_01 §2.4.1 排序鍵、全部規格收尾。

### 11.9 新掛帳(不在本弧)

* **深度巢狀字面值當機**:`{{a: {{a: …}}}}` 巢狀 150 層,**交付前後皆 exit 134**。
  既有缺陷,與本交付無關;`HARD_RECURSION_LIMIT` 沒有覆蓋這條路徑。
  120 層正常、150 層當機,門檻未細掃。
* **SPEC_01 §2.4.1「禁止以 CAID/digest 作顯示排序鍵」**:交付改用 CHS digest。
  O42 之後 digest 是值的函數,**實質站得住**,但該 MUST NOT 須由驗收方修訂,
  不得由交付默默繞過。
