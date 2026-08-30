# Q-014b 偵察 — 一份與自己一致的紀錄

> **Queue ID**：`WORK_QUEUE` Q-014b（Active，偵察）
> **基線**：引擎 `v0.39.0` 標籤二進位
> `/home/gali/nlang-baselines/v0.39.0-verify-target/release/oo`
> （`--version` 印 `oo v0.39.0`）／工作樹 `nlang-tools` `dev`（本檔是唯一產物）。
> **這是偵察，不是實作。** 下面若有「一行就能修」的東西，只寫進報告。
> **未重量**驗收方 2026-08-30 已量的 §1.1 三輪 40 並行（注入 40／○ 10–6）。
> **未裁的岔路不選邊**（brief §2）：A／B2／C 只報價。B1 已出局，不報價。
>
> **身分**：本輪零改動。〔量，標籤二進位〕`~%Math./add (1,2)` → `3`
> （呼叫發生了：答案不是 `_`、不是原文）。`x: 0` 根
> `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`，
> `.oo/objects` **3** 個檔，標準根
> `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`。
>
> **⚠ 縮寫**：本文 `CAS` 只指 Content-Addressed Storage。compare-and-swap
> 寫全稱。

先讀了 `savepoint.rs`／`universe.rs` `save_staged`／`observe`／`commit`／
`store_codec.rs`／`gc.rs`／`migrate_layout`／`main.rs` 的 `run`／`eval`／`repl`。
裁定 D43／D47／D48 不重開。不選 A／B2／C。

---

## 0. 十題各一句

| | 答案 |
| :-- | :-- |
| **Q1** | A：約 25–45 行（若連 `LOG` 一起拿掉則 40–70）＋ 1 支跨層並行探針。B2：約 60–110 行；分叉要另購。C：約 80–120 行重寫 `savepoint.rs`；序欄位若由計數而來，就把 `mint_id` 的病從檔名搬進 body。 |
| **Q2** | 「恰好一個 ○」＝每一次成功的位置移動留下一個檔，不是整批塌成一個。A 靠 `O_EXCL` 發名。B2 兩行程讀到同一個前驅 ⟹ **鏈分叉**。我讀 §3.1「可判定的先後序」是**全序**（一個持有者的觀測史），偏序不算——這是讀法，不是裁定。若偏序算，B2 的分叉合法；若不算，B2 需要額外機制（那是 Q-016 或 A 的發名）。 |
| **Q3** | 三種情形下 `LOG` 都是第二份真相。拿掉：`record` 從兩次 `atomic_write` 變成一次，今天的孤兒／覆寫窗消失。列出並排序的價：A ＝ `readdir`＋檔名排序（不開檔）；B2 ＝讀每一顆的前驅、找未被指向的端點、走鏈（有分叉則不是一條）；C ＝讀每一顆的序欄位再排序。 |
| **Q4** | 去重沒壞，基準選錯了（確認 §1.5）。最便宜的**程式**形是 commit 之後也 `record` 一次當新基準，但那一顆 ○ 不在 D47 的 (a)／(b) 裡 ⟹ **可能要動 D47 措辭**。不選。另外兩形：比對改成「已提交的根 ＋ 工作集」／body 分開記這兩份。 |
| **Q5** | 今天的 body **是工作集快照，不是深度讀數**。提交後工作集歸零，鏈往回走，對「快照」是對的，對「看了多深」是錯的。§3.1 標題用了後者。裁定不在這裡。 |
| **Q6** | **兩件獨立的事。** (1) 觀測路徑沒有寫者：`Universe::observe` 是 `&self`（`universe.rs:1288`），`savepoint::record` 的唯一呼叫者是 `save_staged`（`:894`）。(2) `oo run`／`eval` 另有 Q-018：`run_one_shot` `:1385`–`:1395` 明文「no local staged load, no durable store writes」，根是 `None`。REPL **已經** `load_universe`，所以「沒有倉庫可寫」不是全域成因。不做 Q-018 的前提下，**(b) 與 ⊥ 款本弧做不到**：即使在 REPL 接上 `record`，今天比對的是 staged 位元組，觀測不改 staged，去重會跳過。 |
| **Q7** | 逐欄位 evolve：N＝1／10／100／1000 的 `savepoints/` 總位元組是 **49／760／42 790／4 837 990**。`oo gc` **不走訪、不清理** `savepoints/`（連種進去的垃圾檔都還在）。**沒有任何產品路徑會刪 ○。這是一個永遠成長的目錄。** |
| **Q8** | 產品路徑：寫者只有 `universe.rs:894`，讀者零（`load` 仍 `dead_code`）。CLI 零命中。測試釘形：`p1` 要 `LOG` 這個檔名、`g2` 釘 LOG 位元組跨 commit 不變、三支版圖要目錄名 `savepoints`。kademlia `p4` **仍沒宣告** `savepoints`（Q-014 只加了 `injections`）。沒有找到第三處 `savepoint::` 呼叫者。 |
| **Q9** | 驗收方沒看到的洞：**`atomic_write` 的最後一步是 `rename`（可覆寫），與 A 要的 `O_EXCL` 是相反的原語**——「先 `exists` 再 `atomic_write`」仍會互相覆寫。C 的序欄位若讀計數，40 並行會留下 40 個檔、同一個序。commit 的 Config 再暫存會**另外鑄一顆更小的 ○**。 |
| **Q10** | ○ 檔在、`LOG` 不在：下次 evolve **重鑄 `0000000000000001` 並覆寫** body。`LOG` 在、○ 檔不在：下次 evolve **追加 `0000000000000002`，幽靈 id 永遠留在 LOG**。兩種 `status` 都 rc＝0，引擎不報不一致。`recorded_ids` 不檢查檔在不在（`:57` 的 `exists()` 只服務去重）。`load` 對缺檔是 `read_to_string` 失敗；它是死碼。 |

---

## Q1 — 三個候選各自報價

粒度沿用 `an_object_you_can_swap_recon.md` 的 Q9／Q10：生產碼行數、必紅探針、不解掉什麼。不選邊。

今天 `savepoint.rs` 84 行。`mint_id`（`:37`–`:39`）＝ `prev.len() + 1`，`format!("{n:016x}")`。`record`（`:52`–`:77`）兩次 `atomic_write`。`encode_savepoint`（`store_codec.rs:165`–`:167`）沒有前驅、沒有序欄位。

### A — 維持本地識別碼，原子鑄名；序＝檔名

| 檔 | 做什麼 | 約略行數 |
| :-- | :-- | --: |
| `savepoint.rs` `mint_id`／`record` | 不再讀 `ids.len()`。對 `format!("{n:016x}")` 走 `OpenOptions::create_new(true)`（或等價的 `RENAME_NOREPLACE`），`EEXIST` 則 `n += 1`。 | +25–45 |
| 若連 `LOG` 拿掉（Q3） | `record` 只寫 body；列出＝檔名排序 | 再 +15–25，並刪掉重寫 LOG 的那段 |
| `store_codec.rs` | 不動 | 0 |
| `universe.rs` | 仍 `record(&self.staged)` | 0 |
| 版圖 `p1`／local_gc `p4`／advert `r2` | 路徑仍 `savepoints/<local-id>`（仍 16 hex、仍非 CAID） | **0**（拿掉 `LOG` 則 `p1`／`g2` 紅） |

合計 **約 25–45 行**（留 `LOG`）或 **40–70 行**（拿掉 `LOG`）。不碰 `ComboVal`、不碰 `put_root`、○ 不進 `objects/`。

**探針**

* **必新**：一支跨層並行探針——宣稱成功的位置移動數 ＝ ○ 檔數（brief §1.2 紅線）。今天基線必須紅（6–10／40）。只檢查 LOG 自洽的斷言不得當作完成條件。
* 既有版圖：留 `LOG` 則綠；拿掉則 `p1`（宣告 `.oo/savepoints/LOG`）與 `g2`（`a_working_set_that_is_a_set_probe_test.rs:255`，釘 LOG 是一個檔且 commit 不改它的位元組）紅。那兩支釘的是**形**，不是「○ 活過 commit」那個性質——性質可以改讀目錄。

**A 解得掉的**：並行下互相覆寫（§1.1 的 6–10／40），前提是鑄名真的是 `O_EXCL`，見 Q9。
**A 解不掉的**：Q4 比對基準、Q5 鏈往回走、Q6 觀測條款、Q7 永遠成長、`save_staged` 先寫注入再寫 ○ 的兩步窗（Q9）。

**Q-016**：每個 ○ 檔可以原子安裝。工作集仍是注入目錄的 fold，不是這一個檔。本卡給的是「有序的 ○」，不是 compare-and-swap 迴圈。

### B2 — ○ 留在 `savepoints/`，身分仍是本地 id，每個 ○ 記錄前一個本地 id

| 檔 | 做什麼 | 約略行數 |
| :-- | :-- | --: |
| `savepoint.rs` `mint_id` | **不得**再讀計數，否則檔名碰撞還在。重用 `injections.rs:24` 的 `SystemRandom` 16 位元組。 | 8 → ~15 |
| `savepoint.rs` `record` | 把「我讀到的上一顆 id」寫進新 body；LOG 若仍在，只是索引 | ~20 |
| `store_codec.rs` | 框上加 `predecessor:`（或等價）。必須在 combo **之外**——放進 `ComboVal` 會讓一個本地欄位看起來像值。decode 跳過該欄，body 仍走 `write_combo`。 | +30–50 |
| 版圖 | 路徑仍 `savepoints/<local-id>`；id 寬度若改成 32 hex，collapse 規則仍成立 | 0–數行註解 |

合計 **約 60–110 行**。不碰 `ComboVal`、○ 不進 CAS。

**探針**：同 A，一支跨層並行。前驅欄位本身不紅任何現有針。拿掉 `LOG` 則 `p1`／`g2` 紅。

**B2 解不掉的（Q2）**：兩個行程同時 `record`，各自讀到同一個「前一個」⟹ **兩顆 ○ 帶同一個前驅**。那是鏈分叉，不是互相覆寫。身分可以 40／40，序卻不是一條線。要把分叉收成全序，得另買：A 的原子發名、或一個 tip 檔上的 compare-and-swap（**那是 Q-016**）。報價不含那一筆。

### C — 目錄即真相（隨機 id，重用 `injections.rs:24`）＋顯式序欄位

brief 的一句話：把 Q-014 已證明可行的構造，加上一條它刻意沒有的序。

| 檔 | 做什麼 | 約略行數 |
| :-- | :-- | --: |
| `savepoint.rs` | 重寫成與 `injections.rs` 同構：`mint_id` 隨機、`atomic_write`、目錄即集合、無 `ids.len()+1` | 84 重寫 → ~80–120 |
| `store_codec.rs` | 框上加序欄位（同樣必須在 combo 外） | +20–40 |
| `universe.rs` | 呼叫點不變 | 0 |
| 版圖 `p1` | `LOG` 不再存在 | **紅**，除非驗收方改宣告 |
| `g2` | 釘 LOG 這個檔 | **紅** |

合計 **約 80–120 行** 生產碼 ＋ 改 2 支形的針 ＋ 1 支跨層並行。比 A 貴，因為要重寫模組；比「從零發明目錄即真相」便宜，因為 Q-014 付過一半。

**C 解得掉的**：檔名碰撞（隨機 id，40 並行下注入層已量過 40／40）。
**C 不解掉的，除非再買一次原子發序**：序欄位如果是 `len()+1`，兩個行程讀到同一個 N，**鑄出兩個檔、同一個序**。身分對了，§3.1 順序 MUST 仍破。要把序也鑄對，C 必須再做 A（`O_EXCL` 一個序名字）或 Q-016（一個計數物件上的 compare-and-swap）。**那時 C 不是「Q-014 構造 ＋ 一個欄位」那麼便宜**——見 Q9。

**§8 開弧 4**：本題無內建。不適用。

---

## Q2 — 並行下「恰好一個 ○」；B2 的分叉；偏序算不算

完成條件 (ii) 逐字是：每一次 D47 意義下的位置移動留下**恰好一個** ○，並行下不得互相覆寫。
那是 **每筆移動一個檔**，不是「40 筆並行只許存在一顆 ○」。40 個相異欄位應留下 40 個 ○。

| | 每筆移動一個檔？ | 可判定的先後？ |
| :-- | :-- | :-- |
| **A** | 是，若鑄名真是 `O_EXCL`（Q9） | 是：檔名單調。並行兩筆得到相鄰兩個 n，序＝誰搶到較小的名字，**不必**等於牆鐘序，也不必等於注入 fold 的相遇序 |
| **B2** | 是，若 id 隨機（不讀計數） | **不一定**。兩行程讀到同一個前驅 ⟹ 兩顆 ○ 不可比。那是分叉，不是遺漏 |
| **C** | 是（隨機 id） | 序欄位不打結才是。打結＝兩個相同的序 ⟹ 與 B2 分叉同類 |

**規格讀法（不裁）**。`SPEC_10` §3.1 只寫「可判定的先後序」。我讀成**全序**：

1. 中文「先後」是一個觀測者的時間先後，不是格上的 `⊑`。
2. 標題把 ○ 說成「看了多深」——深度若是一個讀數，讀數之間是全序。
3. 今天的 `LOG` 是一條線（行序），S1 落地時就是按這個形交貨的。

若這讀法成立，B2 的分叉**不是**合法的先後序，B2 需要額外機制。
若驗收方／用戶裁定偏序也算，B2 的分叉合法，「恰好一個 ○」只管身分碰撞，不管會不會分叉。

**這句不決定規格。**

**§8 開弧 4**：不適用。

---

## Q3 — `LOG` 還需要存在嗎？

A 讓檔名自帶序，B2 讓每個 ○ 自帶前驅，C 讓 body 自帶序欄位。三種情形下 `LOG` 都是第二份真相。今天 `record` 寫兩次 `atomic_write`（`:67` body、`:75` 整檔重寫 LOG），中間崩潰就是 Q10 的兩種窗。

**拿掉 `LOG` 的價**

| | 價 |
| :-- | :-- |
| 生產碼 | `record` 少一次寫；`recorded_ids`／`parse_ids` 可刪。A 約再 15–25 行淨重寫；C 本來就不需要它 |
| 崩潰窗 | 「body 在 LOG 不在／LOG 在 body 不在」兩個窗一起消失。剩下的是單檔 `atomic_write` 自己的 temp＋rename |
| 探針 | `p1` 宣告 `.oo/savepoints/LOG` → 紅。`g2` 以 LOG 檔的存在與位元組當「沒碰 ○ 層／○ 活過 commit」→ 紅。兩支都是**形**。性質（D43：commit 後 ○ 還在）改成「目錄裡的 body 還在」即可 |
| 列出並排序 | 見下 |

**列出所有 ○ 並排序（沒有 LOG 之後要付什麼）**

| | 怎麼列 | 開檔次數 |
| :-- | :-- | --: |
| **A** | `readdir`，檔名當整數排序 | **0** 次讀 body（名字就是序） |
| **B2** | `readdir` 全部，讀每顆前驅，找沒人指向的端點，走鏈 | **N** 次。有分叉則不是一條鏈，排序函數必須先定義「分叉怎麼排」（Q2） |
| **C** | `readdir` 全部，讀序欄位，排序 | **N** 次。序打結則穩定性／打破平手要另定 |

Q-018 的 CLI 動詞（列出／回到某個 ○）是那個讀者；本卡不開。沒有 LOG 時，那個讀者從 O(1) 讀一行變成 A 的一次目錄排序，或 B2／C 的 N 次開檔。今天 ○ 層唯寫，所以這筆成本還是零——**直到第一個讀者長出來**。brief §1.3 與上一弧 §1(d) 仍成立：現在是這個格式最便宜的一刻。

留 `LOG` 當快取索引也可以，但它必須是可重建的衍生物（從檔名／前驅／序欄位 fold 出來），不能再當鑄名來源。衍生物快照是上一弧 Q-014 偵察標過價、本弧不做的那一格。

**§8 開弧 4**：不適用。

---

## Q4 — 產生判準 (a)：位元組還是位置？

確認 §1.5：去重沒壞，基準選錯了。〔量，標籤二進位，可逐字重跑 brief 的腳本〕

| 情形 | ○ 數 |
| :-- | --: |
| `evolve a` → `evolve a`（不提交） | 1（第二次不鑄） |
| `evolve a` → `commit` → `evolve a` | 1（上一個 body 恰好是 `{ a: 1 }`） |
| `evolve a` → `evolve b` → `commit` → `evolve a` | **3**。○3 body ＝ `{ a: 1 }`，而第二次 commit 只多 **1** 個 CAS 物件（commit 物件；root 被重用） |

第三種：格上的位置沒有移動，○ 照鑄。`record` 比對的是 `ids.last()` 那一個 ○ 的位元組（`:55`–`:63`），不是「已提交的根 ＋ 工作集」。

**三種形，不選。可能要動 D47 措辭，本弧不得順手改。**

| | 形 | 約略行數 | 與 D47 |
| :-- | :-- | --: | :-- |
| **1** | commit 成功之後也 `record` 一次，當作新基準（空工作集，或 Config 再暫存後的 staged） | `universe.rs` commit 清場處 +5–15 | **多鑄的那一顆不是 (a) 也不是 (b)**。iff 被打破。最便宜的程式，最貴的規格 |
| **2** | 比對基準改成「`HEAD` 的根 CAID ＋ 當前工作集位元組」，不再比上一個 ○ | `savepoint.rs` +20–40；`record` 要能讀 HEAD | 對準「格上的位置」。第三種情形不鑄。D47 的 (a) 第一次有可執行的意思。措辭可能仍要寫明「位置＝根＋工作集」 |
| **3** | ○ body 分開記已提交的根與工作集 | `store_codec` +30–50，與 Q5 同一筆 | 之後「深度」與「工作集快照」可以分開走。最貴，也是唯一讓 Q5 的兩種讀法同時活著的形 |

形 1 今天**已經有一半在跑**：commit 若留下 `~%Config`，會走 `save_staged`（`universe.rs:1094`–`:1101`），於是再鑄一顆。〔量〕`~%Config.fuel: 12345` 與 `a: 1` 一次 evolve 再 commit：○1 ＝ `{ a: 1 ~%Config: { fuel: 12345 } }`，○2 ＝ `{ ~%Config: { fuel: 12345 } }`。沒有 Config 的 commit **不**呼叫 `record`。所以「commit 時記一顆當基準」不是新想法，是 Config 再暫存的副作用，而且鑄出的是一顆**更小**的 ○（Q5、Q9）。

**§8 開弧 4**：`~%Math./add` 在 §1.7 的觀測量測裡用過；本題的三種情形是 evolve／commit，無內建。

---

## Q5 — ○ 鏈往回走

〔量〕○2 body ＝ `{ a: 1 b: 2 }`，○3 body ＝ `{ a: 1 }`。後一個比前一個小。Q4 的 Config 再暫存更極端：commit 之後立刻多一顆只有 Config 的 ○。

**讀法。**

今天的 body 是 `encode_savepoint(&self.staged)`（`record` 的第二個參數，`universe.rs:894`）。`staged` 在 commit 時被清空（或收成 Config）。所以 body **是工作集的快照**。它不是「宇宙被看到多深」的讀數：已提交的根不在裡面，觀測化約的值也不在裡面。

§3.1 標題：「Savepoint 記錄的是『看了多深』，Commit 記錄的是『它是什麼』。」
若「深度」是單調的，往回走是矛盾。
若深度可以在提交邊界歸零，那「深度」在這一節用的是工作集的基數，不是觀測深度——**用詞與實作對得上的是後者，與標題對得上的是前者。**

「看了多深」在提交之後歸零：

* 對「工作集快照」這條讀法：**是對的**。工作集確實歸零（D48：commit 清空注入目錄）。
* 對「觀測深度」這條讀法：**是錯的**。已提交的宇宙還在，深度不該跟著工作集一起沒。

兩者都要？那是 Q4 形 3：body 分開記。只要給讀法，不選。

**§8 開弧 4**：不適用。

---

## Q6 — 判準 (b) 與 `_|_`；不做 Q-018 的前提下

§1.7／§1.8 要分清的兩件事，**都在，而且不是同一件事。**

### (1) 沒有寫者 — 全域

`savepoint::record` 在該檔之外**只有一個呼叫者**：`universe.rs:894`，在 `save_staged` 裡。`save_staged` 的產品呼叫者是 `run_evolve`（`main.rs:463`）與 commit 的 Config 再暫存（`universe.rs:1101`）。

`Universe::observe`（`universe.rs:1288`）簽名是 `&self`。它不能寫磁碟，也不能改 `self.staged`。觀測化約發生在回傳值上，staged 位元組不動。

CLI 觀測呼叫點，**沒有任何一個**在 observe 之後呼叫 `save_staged`／`record`：

| 入口 | 檔案:行號 | 有沒有 load 已提交的宇宙 | 寫 ○？ |
| :-- | :-- | :-- | :-- |
| `oo evolve` | `main.rs:426`／`:463` | 有（`load_universe`） | 寫，但是注入路徑，判準 (a) |
| `oo run --observe` | `run_one_shot` `:1376`–`:1426` | **無** | 無 |
| `oo eval` | `:1451`–`:1503` | **無** | 無 |
| `oo repl` | `:1183`–`:1243` | **有**（`:1185` `load_universe`） | 無（`:1227` observe 完只印） |
| `oo test` | `:1628`／`:1654`／`:1717` | **無**（同樣 `new_with_standard(None, …)`） | 無 |

### (2) 觀測路徑沒有倉庫可寫 — 只綁在 `run`／`eval`／`test`

```1385:1395:crates/oo/src/main.rs
    // One-shot: pure universe, no local staged load, no durable store writes.
    let mut universe = Universe::new_with_standard(
        None,
        nlang_interpreter::value::ComboVal::default(),
        engine.root_with_system(),
    );
```

`Ouroboros::init(&cwd)` 會打開 `.oo/`（所以 `engine.base_dir` 有值），但宇宙的根是 `None`，不讀 `HEAD`，也不 `load_staged`。這就是 §1.8：`r: ~%Math./add (a, 1)` 而 `a: 1` 已提交 → `_|_ (%cause: #conflict)`。那是 **Q-018**。

REPL 走另一條路：`:1185` `load_universe`。所以「觀測路徑根本沒有倉庫」**不是**觀察這個動作的內在性質，是 one-shot CLI 的選擇。

〔量，標籤二進位〕commit 過 `a: 1` 之後：

```
oo run s1.n -o r     # r: ~%Math./add (1, 1)     → 2                       LOG 仍 1 行，rc=0
oo run s3.n -o r     # r: ~%Math./add (1, "x")   → _|_ (%cause: #conflict) LOG 仍 1 行，rc=0
oo run s2.n -o r     # r: ~%Math./add (a, 1)     → _|_ (%cause: #conflict) LOG 仍 1 行
oo eval '1 + 1'      → LOG 不動
```

呼叫已證明發生（改引數改答案）。○ 不動。

### 不做 Q-018 的前提下，有沒有辦法產生 ○？

**對 `oo run`／`eval`／`test`：沒有。** 那三條路刻意不載入宇宙、刻意不寫入。把它們改成會寫 ○，就是讓 one-shot 看見並改倉庫，那是 Q-018 的射程。

**對 REPL（以及任何未來「先 `load_universe` 再 observe」的入口）：有倉庫，仍然沒有 (b)。**
即使在 `:1227` 之後接 `savepoint::record(base, &universe.staged)`：

* 觀測不改 `staged`
* `record` 的去重比對上一個 ○ 的位元組
* 結果是 **Ok(None)**，不鑄

所以 (b)「觀測真的化約了一個 thunk」**不能**用今天的 `record(combo)` 表達。⊥ 款同理：`observe` 回傳 `_|_` 時 staged 仍然是舊快照，去重仍跳過；而且 `run` 的 ⊥ 還是 rc＝0（Inbox 那列的第三個面）。

**本弧做不到的（請把射程縮在這裡）：**

* D47 (b) 與 §3.1「`_|_` 亦產生」——需要 (i) 觀測路徑上的寫者，**(ii) 一種不是「工作集快照」的 ○ payload**（化約了哪一條路徑、化約成什麼、是不是 ⊥）。(ii) 與 Q4／Q5 是同一筆裁定。沒有 (ii)，接上寫者只會鑄 (a) 或不鑄。
* `oo run`／`eval` 看見已提交的宇宙 —— Q-018，指名、不修。

**本弧做得到的（若用戶把射程留在身分／順序）：** A／B2／C ＋ 跨層並行探針。那是 brief 標題那件事。

**§8 開弧 4**：`~%Math./add` 是本題用來證明呼叫發生的內建；答案不是 `_`、不是原文。

---

## Q7 — 體積與 GC

每一次位置移動存一份**整個工作集**的快照。〔量，標籤二進位，逐欄位 `oo evolve`，N 個相異欄位 `f{i}: {i}`〕

| N | ○ body 數 | `savepoints/` 檔位元組合計（`du -sb`＝逐檔加總） | 其中 LOG | 最後一顆 body | 逐次 `oo evolve` 牆鐘 |
| --: | --: | --: | --: | --: | --: |
| 1 | 1 | **49** | 17 | 32（`#nlang/store savepoint\n{ f0: 0 }`） | 0.016 s |
| 10 | 10 | **760** | 170 | 86 | 0.247 s |
| 100 | 100 | **42 790** | 1 700 | 806 | 9.95 s |
| 1000 | 1000 | **4 837 990** | 17 000 | 9 806 | 1 980.6 s |

趨勢是 **O(N²)**：第 k 顆 ○ 的 body 長度 ≈ O(k)，N 顆加起來 ≈ O(N²)；LOG 是 17N 位元組（每個 id 16 hex ＋ 換行），可忽略。牆鐘主要不是寫 ○（N＝1000 的目錄只有 4.8 MB），是每一次 evolve 都要 fold 已經累積的注入——那是 Q-014 工作集的成本，本卡改鑄名不會把它削掉。N＝1 的 body hex 以 `23 6e 6c 61 6e 67`（`#nlang`）開頭。

### `oo gc` 不走 `savepoints/`

〔讀〕`gc.rs:282`：「Sweep unreachable objects under `.oo/objects/` only。」`mark` 從 `HEAD` 走 CAS 引用。目錄名 `savepoints` 在 `gc.rs` 零命中。

〔量〕兩個 ○ 存在時 `oo gc --grant gc`：報告 `3 objects, 3 reachable, 0 collectable`（那 3 是 CAS），`savepoints/` 檔數與位元組不變。再種一個 `.oo/savepoints/deadbeef`，再 gc：**`deadbeef` 還在。**

### 有沒有任何路徑會刪 ○？

〔讀〕`savepoint.rs` 沒有 `remove_file`。`universe.rs` 的 `remove_file` 只打 `staged`／`pin_pending`／`effect_pending`／`abandoned`。`injections::clear` 只清 `injections/`。`migrate_layout` 只寫 `format`／`objects.format`。

〔量〕`rollback`：`savepoints/` 檔名與 LOG 位元組不變（abandoned 寫的是 commit CAID，不是 ○）。`squash`：同上。`migrate --grant migrate`：同上（連種進去的 `deadbeef` 都還在）。

**沒有。這是一個永遠成長的目錄。** 本卡若只修鑄名與序，成長曲線不變。GC ○ 或「只留最後一顆」是另一張卡，而且會碰到 D43「每一個 ○ 產生時即為持久」。

**§8 開弧 4**：不適用。

---

## Q8 — 今天誰依賴 ○ 的形？

驗收方讀到「產品路徑只有 `universe.rs:894` 一個寫者、零讀者」。**確認，沒有第三處 `savepoint::` 呼叫者。** `grep savepoint::` 在 `crates/` 底下命中的生產碼就是那一行。`load` 仍 `#[allow(dead_code)]`（`:79`）。`crates/oo/src/` 對 `savepoint` 零命中。

讀 `.oo/savepoints/`、`LOG`、或依賴那個形的**測試**：

| 位置 | 依賴什麼 | 改序之後 |
| :-- | :-- | :-- |
| `a_store_you_did_not_write_probe_test.rs` `p1` `:220`–`:221` | committed 倉必須有 `.oo/savepoints/LOG` 與 `.oo/savepoints/<local-id>` | 拿掉 LOG → **紅（形）**。id 改寬度／改隨機，collapse（`:121`–`:126`）仍把非 LOG 折成 `<local-id>`，綠 |
| 同檔 `oo_files` `:121` | 同上 collapse | 目錄改名才紅 |
| `local_gc_probe_test.rs` `p4` `:868` | 允許名單含目錄名 `savepoints` | 目錄改名 → 紅。A／B2／C 都不必改名 |
| `advert_persistence_probe_test.rs` `r2` `:689` | 同上（該情境不 evolve，宣告的是佈局） | 同上 |
| `kademlia_table_probe_test.rs` `p4` `:1323`–`:1331` | 允許名單 **沒有** `savepoints`：`objects, format, objects.format, peers, injections` | Q-014 加了 `injections`，**沒加** `savepoints`。今天綠是因為從不 evolve。潛伏針仍在。本卡若在此情境鑄 ○，這支會紅 |
| `a_working_set_that_is_a_set_probe_test.rs` `g2` `:255`–`:274` | LOG **是一個檔**、commit 後仍在、**位元組與 commit 前相同** | 拿掉 LOG → 紅。A／B2／C 若留下 LOG 但改寫它（C 的序、B2 的前驅不寫進 LOG 則不一定）→ 視是否改 LOG 內容。這支釘的是「Q-014 沒碰 ○」，不是一個永遠正確的性質；本卡本來就會碰 ○ |
| `depth_belongs_to_the_savepoint_probe_test.rs` 檔頭 | 註解仍寫「沒有 ○ 實體」（Q-013 當時） | 斷言不讀目錄；註解過期，不是閘 |

規格：`SPEC_*` 的 `zh_TW` 裡 `savepoints` **零命中**（只有 `CHANGELOG` 提到磁碟形）。`REAL_01` 參考佈局仍然是 Inbox 那列的失真描述，不含 `savepoints/`。`.oo/format` 是 `layout=2`，Q-013 加目錄時**沒有**升版。`migrate_layout` 不認識這個目錄。

**改序之後會紅的，多數是釘「LOG 是一個檔」這個形。** 釘性質的（D43 活過 commit、本地 id 不是 CAID）可以改讀 body 檔。跨層並行探針今天不存在，必須新建，且基線必須紅。

**§8 開弧 4**：不適用。

---

## Q9 — 三個候選各自在哪裡會壞

比報價重要。先把 brief 點名的各處查完，再指名驗收方沒看到的洞。

### 查了，沒事／有事

| 處 | 結果 |
| :-- | :-- |
| 崩潰落在兩次 `atomic_write` 之間 | **有事，今天就有。** Q10 實測。A 若仍寫 LOG，窗還在；A 拿掉 LOG／B2 拿掉 LOG／C 目錄即真相，這個窗關。 |
| `rollback` 與 `.oo/abandoned` | **查了，○ 沒事。** rollback 寫 abandoned 的是離開的 HEAD（commit CAID）。〔量〕rollback 前後 `savepoints/` 檔名與 LOG 位元組相同。A／B2／C 都不需要為 rollback 改 ○。 |
| `squash` | **查了，○ 沒事。** 〔量〕squash 後兩個 ○ 與 LOG 仍在。 |
| `oo gc` | **查了，○ 沒事也沒被管。** 不走訪。種垃圾也不收。永遠成長（Q7）。三個候選都繼承。 |
| `oo migrate` | **查了：○ 的檔案格式不在任何規格佈局宣告裡。** 與 Q-014 偵察量到的「暫存態三個檔不在宣告裡」是**同一個洞的另一面**——`savepoints/` 有三支**探針**宣告（`p1`／local_gc `p4`／advert `r2`），但 `REAL_01`、`SPEC_*`、`layout=` 哨兵都不提它。migrate 只重寫 `format`／`objects.format`。改 ○ 編碼（B2 前驅、C 序欄位）**舊引擎仍會打開這個倉**（layout 仍是 2），然後 `load`（一旦不再是死碼）會解不開。 |
| 多個宇宙節點共用一個 `.oo/` | **查了，沒有額外隔離。** 兩個行程共用同一目錄時，HEAD／注入／○ 已經在搶。A 的 `O_EXCL` 是這個檔案系統上的 POSIX 原子；NFS 上 `O_EXCL` 傳統上不可信（若有人把 `.oo/` 放上 NFS，A 的保證先破）。C 的隨機 id 在共用目錄上仍不碰撞，序欄位仍打結。這不是本卡獨有，注入層同一事實。 |

### 驗收方沒看到的洞（至少這一處）

**A 不能建立在今天的 `atomic_write` 上。**
`storage.rs:15`–`:41` 的最後一步是 `tmp.persist(path)` ＝ **rename 到目標，目標已存在就覆寫**。這正是 §1.1／§1.2 互相覆寫、不留孤兒的機制。A 若寫成「`exists()` 則 `n+1`，否則 `atomic_write`」，兩個行程仍可同時看見「不存在」然後互相 `persist`——**今天的病原封不動**。A 必須讓**最終檔名**的建立是 `O_EXCL`（`create_new`，或 Linux `renameat2(RENAME_NOREPLACE)`）。那與「內容必須 temp＋fsync＋rename」（`REAL_01` §4.1.1）是兩件原語，要接在一起：先把內容 fsync 進 temp，再以 **不覆寫** 的方式安到單調名字上。brief 寫「`O_EXCL` 或等價」，沒寫今天的 `atomic_write` 不是那個等價。

其餘兩個，順手記下，免得實作時當發現：

1. **C 的序欄位會把 `mint_id` 的病從檔名搬進 body。** 40 並行可以 40 個檔（身分對）且 30 個相同的序（順序 MUST 仍破）。C 要兌現 §3.1 順序，必須再買 A 或 Q-016。不要拿「注入層 40／40」當 C 的序已經免費。
2. **commit 的 Config 再暫存會鑄一顆更小的 ○**（Q4／Q5）。A／B2／C 任何一個若仍呼叫 `record(&self.staged)`，這顆都會在。它讓「上一個 ○ 的 body」在提交邊界變成 Config 碎片，於是 `evolve a → commit → evolve a` 在**有 Config** 時不再去重。
3. **`save_staged` 先寫注入（`:886`）再 `record`（`:894`）。** 中間崩：status 看得到那一筆，○ 沒有。跨層不變式（宣稱成功＝○ 數）在崩潰下仍紅。三個候選都繼承，除非 ○ 的鑄造與注入寫入被收成一步——那會混兩層，本卡沒有這個工單。
4. **今天的 `record(staged)` 表達不了 D47 (b)**（Q6）。改序的實作如果以為「observe 之後呼叫現有 `record`」就能兌現觀測條款，會交一個綠的身分、零的 (b)。

**A 會壞在**：`atomic_write` 被誤當成 `O_EXCL`；並行下仍覆寫。NFS。兩步窗。Config 再暫存。
**B2 會壞在**：分叉（Q2）；若 mint 仍讀計數，分叉之外還覆寫。tip 沒有 compare-and-swap 就不是全序。
**C 會壞在**：序打結；拿掉 LOG 後 `p1`／`g2` 紅（形，不是語義）；列出要 N 次開檔。

**§8 開弧 4**：不適用。

---

## Q10 — 崩潰窗實測（今天，與裁定無關）

重構，不是 `kill -9` 插在兩次 rename 之間。標籤二進位。

### ○ 檔已寫、`LOG` 未更新

```bash
OO=/home/gali/nlang-baselines/v0.39.0-verify-target/release/oo
W=$(mktemp -d); export OO_IDENTITY="$W/id" OO_NODE_HOME="$W/nh"; cd "$W"
python3 -c 'open("p.n","w").write("p: 1\n")'
$OO evolve p.n
rm .oo/savepoints/LOG
$OO status          # 仍印 p: 1；rc=0；不提孤兒 body
python3 -c 'open("q.n","w").write("q: 2\n")'
$OO evolve q.n
cat .oo/savepoints/LOG          # 一行 0000000000000001
cat .oo/savepoints/0000000000000001
# #nlang/store savepoint
# { p: 1 q: 2 }          ← 覆寫，不是追加；舊快照沒了
```

`mint_id` 從空 LOG 得到 n＝1，`atomic_write` rename 到已存在的 `0000000000000001`。孤兒被吃掉，不是被修復。引擎看不見。

### `LOG` 已更新、○ 檔不存在

```bash
# 同上，evolve p 之後：
rm .oo/savepoints/0000000000000001
$OO status          # 仍印 p: 1（status 讀注入，不讀 ○）；rc=0
python3 -c 'open("q.n","w").write("q: 2\n")'
$OO evolve q.n
cat .oo/savepoints/LOG
# 0000000000000001
# 0000000000000002
ls .oo/savepoints   # 只有 LOG 與 0000000000000002；0001 不存在
```

`recorded_ids` 回傳 LOG 裡的兩行，不看檔在不在。`:57` 的 `exists()` 只包住「上一顆拿來去重」：上一顆是 `0001`、檔不在，去重跳過，鑄 `0002`。幽靈 id **永遠**留在 LOG。沒有修復路徑，沒有啟動檢查。

`load(base, "0000000000000001")` 會在 `read_to_string` 失敗。它是死碼，產品路徑走不到。

### 與 v0.38.0 偵察 Q1 的關係

窗的形沒變（當時 staged 還在；現在 status 讀注入 fold）。v0.39.0 之後注入層 40／40，這個窗變得更顯眼：值層說有 40 筆，○ 層在崩潰下可以少一筆或覆寫一筆，而且 **LOG 仍可自洽**（第一種窗：LOG 一行、目錄一檔）。跨層探針會紅；只檢查 LOG 的探針仍綠。

**§8 開弧 4**：不適用。

---

## 明確不做（複述）

compare-and-swap 與重試（Q-016）。觀測路徑的倉庫可見性（Q-018）。logical log（Q-015）。CLI 的 savepoint 動詞。選 A、B2 或 C。動身分。把 ○ 放進 `objects/`（B1，MUST NOT）。順手改 D47 措辭。本弧若只做身分與順序，不要假裝 (b) 與 ⊥ 款已兌現。
