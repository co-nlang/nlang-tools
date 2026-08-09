# W4‴ 交接:一個你選不了的上限

**開弧日**:2026-08-09
**基線**:`dev d61d010`(= `top 4586223`,v0.15.0)。workspace **1812 / 0 / 3**
**來源**:W4″ 驗收 §11.5;`meta/oo/STATUS.md` W4‴／O41;ENGINE_SYNC 2026-08-09
**性質**:**(d) 是回歸修復**(v0.15.0 已出貨)＋ (a) 缺陷 ＋ O41 規格裁定落地
**破壞性**:否

---

## 0. 一句話

> **操作者把 `max_unification_depth` 調到 490 以上,引擎就 dump core——而那個上限是原生堆疊決定的,n/ 這一側看不到它。**

---

## 1. (d) v0.15.0 出貨了一條當機路徑,是我們自己做出來的

〔量 2026-08-09,5000 項加法鏈,`oo evolve`〕

| `max_unification_depth` | 結果 |
| :-- | :-- |
| **預設 `256`（控制）** | 退出碼 **0**,`#blur { %cause: #max_depth_exceeded }` |
| `488` | 退出碼 0 |
| **`499`／1000／2000／4000／100000** | **退出碼 134** |
| 1000 項鏈 ＋ `depth: 900` | **退出碼 134** |
| **v0.7.0,`depth: 4000`（歸因控制）** | 退出碼 **0** ⟹ **W4″ 之前不會當** |

實際訊息:

```
thread 'oo-main' (16994) has overflowed its stack
fatal runtime error: stack overflow, aborting
```

**沒有 `⊥`、沒有 `#blur`、沒有訊息,而且 dump core。**

### 1.1 為什麼上限只有 ~490:每層 frame ≈ 134 KB

`main.rs:306` 早就把 CLI 跑在 **64 MiB** 的執行緒上,註解寫著
「Eval recursion … can exceed the default main-thread stack **before the engine
depth horizon engages**」——**這個類別以前就被認出來過,而緩解手段是「把堆疊加大」**。

那個緩解**之所以夠用,只因為旋鈕是死的、深度永遠停在 256**。W4″ 讓旋鈕活了,
操作者就能跑贏 64 MiB。

64 MiB ÷ ~490 層 ≈ **134 KB／層**。**那是異常的**——一般直譯器的 frame 是幾十位元組
到幾 KB。**真正的成因是 frame 太肥,而不是堆疊太小**;`sub_context` 的
`ctx.clone()`(F2,註解自陳為 open)是第一個該量的嫌疑。
**本弧不修 frame 大小**(見 §9),但**必須把這個數字記下來**,否則下一個人會
以為「把堆疊調到 512 MiB」是解法。

### 1.2 這不是「政策上限」,是「無能為力」

`max_unification_depth` 是**操作者設的政策**:「我選擇在這裡停」。
堆疊耗盡是**實作的無能**:「我到不了那裡」。

**用政策的名字回報一個無能為力,正是 ERROR_CODES §2.7.1 上週才裁掉的東西**,
只是換了個位置。而且:

> **`§2.7.2` 當時判斷「`#stack_overflow` 與 `#max_depth_exceeded` 說的是同一件事,
> 故不入登記簿」——那個判斷在本弧之後不成立。** 它們是**無能／政策**這一對,
> 而那個當時要被丟掉的名字,正好命名了現在需要的東西。
> **規格側的更正由驗收方寫**(§8)。

---

## 2. (a) 暫存只剩 `~%Config` 時鑄出空提交

〔量〕連按兩次 `oo commit`,兩次都成功,**兩次的根都是 `aa1b70f7…`**——各鑄一個 commit 物件。

成因明確:`main.rs:897` 的閘是 `if !universe.is_dirty`,而 W4″ 的交付在把
`~%Config` 重新暫存時設了 `is_dirty = true`。

**`Nothing to commit` 這條路徑本來就存在且正確**(無暫存、重複提交都會回它)。
〔量〕控制:

```
完全沒有暫存      → Error: Nothing to commit
x:1 提交後再提交  → Error: Nothing to commit
只有 ~%Config     → Commit successful + note   ← 缺陷
```

**這是 W4″ 引入的,而工單缺口在驗收方**:R4 只寫了「Config ＋ `x: 1` 同檔」,
沒寫「只有 Config」。

---

## 3. O41(用戶已裁)＋ 一個由量測導出的判準

**裁定**:`timeout` 的創世預設改為 **`#_`**(正無限／不設限),
旋鈕型別放寬為 **`非負整數 | #_`**。

**拼法依 SPEC_01 §2.6**:`#_` 是**序位**的最大元(正無限),`_` 是**格**的 Top(萬有)
——該節的 WARNING 表正是在防這個混淆;`1..#_` 已是既有的無上界區間寫法。
〔量〕引擎現在會拒:`~%Config.timeout: #_` → `#invalid_config`。

### 3.1 哪些旋鈕可以收 `#_`——判準,不是清單

> **一個旋鈕可以接受 `#_`,當且僅當拿掉它的上限之後,
> 它所治理的每一條路徑上仍有別的界在。**

| 旋鈕 | 收 `#_`? | 理由 |
| :-- | :-- | :-- |
| `timeout` | **✓** | 拿掉之後 fuel 與 depth 仍在 |
| `max_branches` | **✓** | **寬度**界;〔量〕設到 1e8,59 支聯集算術 90 ms 正常完成 |
| `max_pattern_nodes` | **✓** | 節點**計數**界,非深度 |
| `fuel` | **✗** | 觀測期的總界;拿掉則「觀測必定停下」不再成立(SPEC_04 §6) |
| `max_unification_depth` | **✗** | **演化期唯一的界,而且它擋的是原生堆疊**(§1) |
| `max_lifting_depth` | **✗** | 同為**深度**界(管道升寫),同一風險類 |

**三個「不可以」都有理由,所以都不是特例。**
〔`max_pattern_nodes`／`max_lifting_depth` 今天從未被讀(O39),
本弧只裁**型別是否接受 `#_`**,不實作它們的閘。〕

---

## 4. 射程

**做:**

1. **(d) 硬界與政策界分開。**
   * 求值加一個**實作自有的**遞迴預算,**與 `max_unification_depth` 無關**,
     且**永遠小於堆疊能承受的層數**。
   * 撞到硬界:**strict 下為 `⊥`,且 `%cause` 不得是 `#max_depth_exceeded`**
     ——用一個表達「實作到不了」的名字(建議沿用既有的 `stack_overflow` 拼法,
     規格側由驗收方補登記)。
   * **不得鑄 `#blur`**:`#blur` 宣稱一個可定址的快照,而堆疊耗盡產生不出來
     (另見 **O42**:blur 的 CAID 今天本來就是時鐘讀數)。
   * **`max_unification_depth` 仍是政策界**,操作者設任何值都**不得**能夠當掉引擎。
2. **(a)** 提交前的「髒」判準改為看**剝掉 `~%Config` 之後**的暫存;
   只剩旋鈕時回既有的 `Nothing to commit`(**沿用,不要新訊息**),
   且**旋鈕仍須留在暫存區**(O37 不變)。
3. **O41** `timeout` 創世預設改 `#_`;旋鈕驗證依 §3.1 的表放寬;
   `timeout: #_` 之下**不武裝任何期限**。

**不做:**

- 不改 frame 大小、不改 `sub_context`(§9)。
- 不碰 O42(blur 的鹽)。
- 不實作 `max_lifting_depth`／`max_pattern_nodes` 的閘(O39)。
- **不碰任何規格檔。**

---

## 5. 探針

檔:`crates/oo/tests/limit_you_cannot_choose_probe_test.rs`(已隨本工單提交,已校準)

**紅測全部標 `#[ignore]`。#3 只准移除 `#[ignore]`。**

| # | 類 | 斷言 | 基線 |
| :-- | :-- | :-- | :-- |
| **C1** | 控制 | **預設深度下的視界仍在**:5000 項鏈在預設旋鈕下回 `#max_depth_exceeded` 且**退出碼 0** | 綠 |
| **C2** | 控制 | `x: 1` 正常提交;無暫存時回 `Nothing to commit`(既有訊息仍在) | 綠 |
| **R1** | 紅 | **`max_unification_depth` 為 499／4000／100000 時不得使行程異常結束**(不看單一退出碼——實測是**訊號結束**、`code()` 為 `None`;同時檢查 stderr 的 `overflowed its stack`／`fatal runtime error`) | 紅 |
| **R2** | 紅 | 撞到硬界時 `%cause` **不得是 `#max_depth_exceeded`**,且**不得**是 `#blur` | 紅 |
| **R3** | 紅 | 暫存只剩 `~%Config` 時 `oo commit` 回 `Nothing to commit`,**且旋鈕仍在暫存區** | 紅 |
| **R4** | 紅 | `~%Config.timeout: #_` 被接受(不得 `#invalid_config`) | 紅 |
| **R5** | 紅 | `~%Config.max_branches: #_` 被接受;`~%Config.fuel: #_` 與 `max_unification_depth: #_` **被拒** | 紅 |
| **P1** | 釘 | `x: 1` 的提交根 CAID 不變(`aa1b70f7…`) | 綠 |
| **P2** | 釘 | **O37 不變量**:有無 `~%Config`,提交後的根 CAID 相同 | 綠 |
| **P3** | 釘 | W4″ 的三個旋鈕仍生效(depth 兩點、branches 兩點、timeout 兩點) | 綠 |

### 5.1 校準(2026-08-09,基線 `dev d61d010`)

**兩支控制、三支釘綠;五支紅各自紅在自己的理由上:**

| # | 基線訊息(摘) |
| :-- | :-- |
| R1 | `max_unification_depth: 499 ended the process abnormally (code None):`<br>`thread 'oo-main' has overflowed its stack` |
| R2 | `LIVENESS: still crashing, so there is no report to inspect` |
| R3 | `a stage holding only a horizon knob minted a commit: Commit successful: hash:…` |
| R4 | `` `~%Config.timeout: #_` was rejected: … #invalid_config at ~%Config.timeout `` |
| R5 | `` `max_branches: #_` was rejected … #invalid_config at ~%Config.max_branches `` |

**R2 是相依的紅**,與 W4″ 的 R5 同型:它的 LIVENESS 守衛先擋下來,
因為**今天根本沒有一次不當機的執行可供檢查 `%cause`**。
**R1 綠掉之後 R2 才成為獨立的檢查。** 寫出來,免得交付方以為讓 R1 綠就自動有了 R2。

**R1 的退出碼是 `None`**——不是 134,是**被訊號結束**(core dump)。
探針的 `crashed()` 因此同時看 `status.code()` 與 stderr 的
`overflowed its stack`／`fatal runtime error`,不依賴任何單一數字。

### 5.2 R1 是本弧的重點,而它量的是**退出碼**

一支斷言「值是什麼」的探針**抓不到 dump core**——行程根本沒機會回答。
R1 因此直接看 `oo` 的退出狀態與 stderr。

---

## 6. 成功標準

1. §5 五支紅全綠、兩支控制與三支釘不動。
2. workspace **≥ 1822 / 0 / 3**,`conformance` 143/143,`genesis` 11/11。
   〔開弧基線(含本工單探針)實測 **1817 / 0 / 8**;五支紅解除 `#[ignore]` 後
   8 → 3、1817 → 1822。交付前基線(無探針)為 1812 / 0 / 3。〕
3. **交付必須回報硬界的實際層數**,以及它是怎麼得到的(量測?常數?)。

---

## 7. 不變量

- 不得改 CAID 相關碼、不得動列舉既有變體的順序。
- **不得以「把 64 MiB 調大」作為 (d) 的解**——那只是把 ~490 換成另一個數字,
  操作者一樣按得到當機鈕。§1.1 的 134 KB／層才是成因,而它不在本弧射程。
- `crates/*/tests/**` 只准移除 `#[ignore]`;既有測試若撞界,**回報,不要自行改**。
- 不得 `git add -A`。

---

## 8. 收尾分工

| 誰 | 做什麼 |
| :-- | :-- |
| #3 | §4「做」三項 ＋ 移除 `#[ignore]` ＋ 回報 §6.3 |
| 驗收方 | 全套驗收;**規格側**:更正 ERROR_CODES §2.7.2(`#stack_overflow` 與 `#max_depth_exceeded` **不是**同一件事)、登記硬界的因、SPEC_09 §6 旋鈕表改 `timeout` 預設與型別 |

---

## 9. 相鄰項

| 項 | 內容 |
| :-- | :-- |
| **frame 134 KB／層** | 真正的成因。`sub_context` 的 `ctx.clone()`(F2,自陳 open)是第一個該量的嫌疑。**本弧只加界,不減重** |
| **O42** | `#blur` 的 CAID ＝ `sha256(now_nanos)`;§3.2.2 第 6 款 (a) 永遠執行不到。**擋在「堆疊耗盡要不要報成 blur」前面,而本弧用「不得鑄 blur」繞開它** |
| **O40** | `oo run`／`oo eval` 看不見倉;`--load`／`--commit` 記載於 REAL_01 §1.1 而不存在 |
| **W22** | 指令面設計稿 `meta/oo/cli_surface.md` |

---

## 10. 驗收(驗收方,2026-08-09)

**結論:接受交付的程式,但有一項探針完整性違規,而其根因在工單。**
**本弧是破壞性的——條目 #10——而工單寫著「破壞性:否」。**

交付 `89c7f49`,開弧 `47d7b50`,基線 `d61d010`(v0.15.0)。

### 10.1 量測

| 項 | 結果 |
| :-- | :-- |
| 探針 | **10/10**,重複 ×5 全同 |
| 獨立全 workspace | **1822 / 0 / 3**(＝成功標準) |
| conformance | **143/143** · genesis **11/11** |
| **對抗:極端旋鈕值** | `depth` = 401／10000／**4294967295**,8000 項鏈,**退出碼全為 0** |
| **硬界的回報** | `_\|_ (%cause: #stack_overflow) ;; Implementation r…`——**自己的名字、⊥ 而非 blur** |
| **跨版本** | 新引擎讀 v0.7.0 的倉:commit CAID 相同、根解得開、`y: 5` 印得出 |

`HARD_RECURSION_LIMIT = 400`,註解記了推導(實測 488 安全 / 499 dump core,
64 MiB ÷ ~490 ≈ 134 KB/層,400 為安全邊際)。§6.3 的數字**寫在程式裡而不是回報裡**。

### 10.2 破壞性:條目 #10,而工單漏了它

**機制**:O41 改寫創世的 `~%Config`(`timeout: 1000` → `#_`),
而 `~%Config` 住在**系統軸**,`serialize_combo` 把 `cv.system` 折進 CAID
(**W8′ M2,我自己 2026-08-09 量的**)。⟹ **改一個創世預設值,每一個根的 CAID 都會移動。**
證據:`genesis.rs` 的 `SEED_CONFIG` 常數同步改變。

移動的三個被釘住的值:

| 釘 | 舊 | 新 |
| :-- | :-- | :-- |
| `limit_you_cannot_choose` P1 | `aa1b70f7…` | `8698d297…` |
| `knob_that_does_nothing` P1 | `aa1b70f7…` | `8698d297…` |
| **`print_what_can_be_read` P4** | **`6e8eae8b…`** | `16ba5683…` |

**最後那個是 v0.2.55–v0.12.0 十週未動的那個值**,而我在兩個弧裡拿它當「非破壞性」的證據。

**〔量〕新值是決定性的**:同一份 `x: 1` 連建三個倉,根皆 `8698d297…`,與釘一致。
**〔量〕舊倉仍可讀**:見 10.1 跨版本列——動的是「同一份來源重建出的根」,不是既有物件。

### 10.3 探針完整性違規

交付**改了三個釘的期望值**,並改寫了 `config_home`／`config_validation`／`genesis_test`
的斷言(其中一支的斷言被**反轉**:從「deadline 有設」改為「deadline 未武裝」)。

**這些改動的內容全部正確**,但**探針修改權在驗收方**,而工單 §7 明文寫著
「既有測試若撞界,**回報,不要自行改**」。**正確的動作是停下來回報。**

**根因在工單**:我寫了「破壞性:否」,並把 P1 釘在一個 **O41 使其不可能滿足**的值上。
**留著那個釘,交付必然失敗**——這與 W4′ §7 那次是同一種局面,
差別是**那次我事先給了逐項授權,這次我沒有,因為我沒發現會需要**。

**驗收方的處置**:值保留(它們是對的、已獨立驗證),但三個釘的註解由**驗收方**改寫,
把**機制**寫進去(系統軸 → CAID → 改一個創世預設就移動每一個根),
而不是只寫「recalibrated after O41」。**一個沒有說出為什麼會動的釘,下次還是會被人靜靜地改。**

### 10.4 我本來有這個量測,而我沒有把它接上

W8′ M2 的原文就寫著:**「引擎多一個內建 ⟹ 過去每一次提交的根 CAID 都會移動」**,
還補了一句「這是下一次新增內建的風險,不是既成的損害」。

**改一個創世旋鈕的值,是同一個機制。** 我在同一天量過它、寫過它,
然後在下一張工單裡宣告「不動任何 CAID」。
**帳本裡有的東西沒有被讀回來——這不是量測不足,是量測沒有被使用。**
