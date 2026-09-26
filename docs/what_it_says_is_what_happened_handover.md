# 工單：說出來的就是發生的事（Q-060）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-060／裁定 `meta/oo/STATUS.md` **D77**
> 探針（已預先提交並校準）`crates/oo/tests/what_it_says_is_what_happened_probe_test.rs`
> 基線：dev `73d3fcb`／`oo v0.61.0` ⟹ **3 綠 14 紅**，三輪一致、零空洞讀數、零 setup 失敗。

## 1. 缺陷：一族，不是一張清單

Q-042、Q-044、Q-055 各修了一族「回報面說的不是真的、或不是用引擎的話說」，**三次都證明是一族，而且比種子寬**。
本次驗收方先量類別〔量，`v0.61.0` 標籤建置〕：`.oo/` 下每一個耐久路徑（加上操作者身分檔、節點金鑰目錄）逐一設為**不可讀**與**不可寫**，
× 17 個本地指令；另量網路路徑（關著的埠、被占用的埠、活的同儕）。**`.oo/` 內的讀取面全數乾淨**（Q-044 修得徹底）；找到 13 個成員、四種形狀：

| 形狀 | 成員〔全部親手重現〕 | 探針 |
| :-- | :-- | :-- |
| **宿主表示** | 身分檔讀不到 ⟹ `Permission denied (os error 13)`（`identity`、`refine --sign`）；節點金鑰讀不到（`node id`）；`discovery.n` 讀不到（`status`）；`node serve` 埠被占 ⟹ `os error 98`；`node advertise` 連不上 ⟹ `os error 111`；`atomic_write temp create …`（引擎函式名）；`discover`／`find-node` 印 `{e:?}` ⟹ `Conflict` | r1–r8、r11、r12 |
| **一個沒發生的理由** | `.oo/format` 不可寫 ⟹ `commit` 說 `working set unreadable`（同一時刻 `status` 讀得到工作集）；循序的 `evolve v: _` 後 `commit` ⟹ `working set consumed by a concurrent commit`（沒有任何並行）；連不上 ⟹ `Conflict` | r9、r10、r7、r8 |
| **各回報面不一致** | `.oo/savepoints` 不可寫 ⟹ `evolve` rc=1，**而注入已落地**、`status` 看得到；`node discover` 寫同儕目錄失敗 ⟹ rc=0、`accepted=1`，**目錄是空的** | r11、r13 |
| **沒記下的欄位當事實印** | `inspect` 對每一筆提交印 `parent: (none)`；`Commit.parent` 自 D18／D52 起刻意不設，祖先在 ○ 圖上 | r14 |

〔讀〕兩個結構性根源：**`map_err(|_| 固定句子)`**——把真正的理由丟掉、換成一句不一定是真的話（`main.rs` 的 `CommitLock::acquire` 三處、`injections.rs` 多處；
`oodp.rs` 約 16 處把一切傳輸失敗映成 `BottomCause::Conflict`；全樹 `map_err(|_|` 35 處，**未逐一判讀**）；以及**身分／節點／網路三條路不經 `operator_io`**。
另有**第二個答案**：`main.rs` `refuse_raw_os` 以 `contains("os error")` 文字比對攔截外洩——**一張靠字串的安全網，本身就是「這件事沒有被構造保證」的證據**。

## 2. 裁定與既有規範

*   **D77（用戶 2026-09-27，選甲）**：一個**根本沒建立起來**的連線是 **`#peer_unreachable`**——`REAL_02` §3.2.2「客戶端可見」表的第五列，與 `#peer_timeout`
    （連上了、不回答）可分；**不是完整性事件**；**不得**用 `#conflict`（該碼意為「修你自己的封包」／完整性裁決）。判例同 2026-07-27 `#peer_timeout` 不得沿用 `#timeout`。
*   既有 MUST（**不需裁定**）：`REAL_01` §1.3 第一條（不得以宿主表示回答）；`SPEC_10` §2.2.1「各回報面一致」；
    `SPEC_10` §4.1.3 最後一款「**誠實的空工作集保留原答覆**，兩者必須可分辨」；D74 ②／D75 的「位址沒承諾的不以事實的口氣說」。

## 3. 射程＝不變式（不是機制）

*   **I1 引擎的話**：任何面向操作者的輸出**不得**含宿主表示——`os error N`、`{:?}` 除錯形、引擎內部函式名。**對每一條路徑成立，不只表中的**。
*   **I2 理由是發生的事**：拒絕說的原因**必須**是實際的原因；**不得**把一個不知道的錯誤換成一句固定的話。沒有並行就不得說並行；
    一個從來沒有東西可提交的工作區（含只 evolve 了不帶資訊的 `v: _`）的答覆**必須**與「從未 evolve」者相同（r10 以對照組比對全文與離開碼）。
*   **I3 三面一致**：文字、離開碼、落地的狀態說同一件事（Q-055 的不變式）。一次寫到一半失敗的動作，**要嘛不留痕跡，要嘛說出留下了什麼**。
    **注入落地而 ○ 沒有**，同時違反 `SPEC_10` §3.1「○ 產生時即為持久」。
*   **I4 沒記的不當事實**：`inspect` 不得把刻意不設的 `parent` 印成「沒有父」——印 ○ 圖上的祖先，或不印那一行，你選。
*   **I5 D77**：連不上 ⟹ `#peer_unreachable`；`advertise`／`discover`／`find-node`，**以及任何其他會建連線的路徑**（例如取物件）。**傳輸失敗不得進完整性紀錄。**
*   **I6 類別，不是清單**：**先做偵察**（§5 Q1），把全樹會把錯誤交到操作者面前的點分類，**再**動手。表中 13 個是種子。
*   **I7 第二個答案**：I1 由構造成立之後，`refuse_raw_os` 的字串比對**不該再需要**。拿掉或說明為何仍需要（§5 Q5）。

## 4. 紅線（今天綠，必須保持綠）

*   **c1** 健康路徑照常答（commit／identity／node id／status）；**c2** 活的 discover 會記下同儕——r13 的量測碰得到目標。
*   **g1** 從未 evolve 的工作區 `commit` 仍 rc≠0、不提並行。
*   **真正的並行**仍說「被並行提交吃掉」（`SPEC_10` §4.1.3 前三款；Q-016b 各探針）。
*   Q-042、Q-044、Q-055 各探針；Q-058 9、Q-059 16＋1；conformance 162／162；`x: 0` 根 `31745ef0…`、`v: 1 + 1` 根 `f4f32e7b…`、標準根 `7038e250…`；`layout=7`／`encoding=5`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1（偵察，先做）** 全樹把錯誤交到操作者面前的點，逐類計數**修前／修後**：`map_err(|_|`（35 處）逐一判為「理由保留／理由丟失而句子為真／理由丟失而句子可能為假」；
    `{:?}`／`{e}` 印宿主錯誤者；不經 `operator_io` 的 IO 點。**是幾個點，還是一族？**
*   **Q2** `commit` 為什麼需要 `.oo/format` 的寫入權？修後在 r9 的狀態下答什麼？
*   **Q3** r11 你選「不留痕跡」還是「說出留下了什麼」？還有沒有別的寫入路徑存在「注入落地而 ○ 沒有」的窗？
*   **Q4** r10：`evolve v: _` 在磁碟上留下了什麼，使得 `commit` 以為被吃掉了？
*   **Q5** `refuse_raw_os` 的去留。
*   **Q6** 取物件（`#fetch`）等其他建連線的路徑，連不上時各答什麼？有沒有任何傳輸失敗進得了完整性紀錄？

## 6. 明文不在射程內

*   唯讀命令在空目錄建出 `.oo/`（Inbox，待裁「沒有倉時該回什麼」）。
*   `oo --version` 在 `dev` 建置上自報 `git describe`（Inbox 另一列）。
*   操作者自己輸入的剖析錯誤所附的行／欄位置（那是關於操作者的輸入，不是引擎的表示）。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r14 基線必須是紅的，理由逐支寫在 doc comment；**你可以讓它們變綠，不得改寫它們的宣稱。**
*   判準只在兩種地方釘拼法：規格定的（`#peer_unreachable`），以及拼法本身就是缺陷的（`os error`、`atomic_write`、`Conflict`、`working set unreadable`、`concurrent`、`parent: (none)`）。
*   **探針用到的儀器不准改**：`oo identity`、`oo node trust add`、`oo node serve`／`advertise`／`discover`／`find-node` 的旗標、`oo status`／`log`／`inspect` 的既有輸出結構、
    `~%Discovery./identify`。**若你認為某支的判準不可能被一個正確的實作滿足，寫在「工單哪裡是錯的」，不要改儀器去對上它。**
*   以非 root 執行（權限注入對 root 無效；探針會報 VOID）。`rustfmt` 不得掃探針檔。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

I1. 操作者看得到的宿主句子改走 `operator_io_reason`：身分檔與節點金鑰的讀寫、`discovery.n`、`atomic_write`（不再把函式名放進句子）、聽埠、`advertise` 的讀寫、工作目錄。驗：r1–r5、r12。
I2. `.oo/format` 鎖不起來時說 `cannot lock …`，不再說工作集讀不到。`evolve v: _` 留下的成員折疊後沒有可提交內容，而且清單沒有少成員、也沒有等待，`commit` 與從未 evolve 的工作區同一句 `Nothing to commit`。真正少了成員或等待過，仍說被並行提交吃掉。驗：r9、r10、g1；Q-016 的 r3 仍在全樹裡。
I3. `save_staged` 在 ○ 寫不進去時刪掉剛寫下的注入。`discover` 先把同儕寫進目錄，寫不進去就 rc≠0，並且不先印 `accepted=`。驗：r11、r13。
I4. `inspect` 印 ○ 上的祖先；沒有那條註記就不印 `parent:`。驗：r14。
I5. 撥號失敗是 `#peer_unreachable`，期限內沒有回答是 `#peer_timeout`。`advertise`／`discover`／`find-node`／`#fetch` 都走這兩個。完整性紀錄仍只在位址不符或解不出的承載上記。驗：r6–r8。
I6. 分類在 §8.4 Q1，先於改動。
I7. `refuse_raw_os` 已刪。它把任何帶 `os error` 的句子改寫成「被並行提交吃掉」。

### 8.2 順手改動（逐項指名）

無。`cargo fmt` 未跑。探針檔未改。`spec/`、`meta/`、`conformance/` 未改。

改動檔：`crates/interpreter/src/operator_io.rs`、`value.rs`、`store_codec.rs`、`storage.rs`、`discovery_config.rs`、`injections.rs`、`universe.rs`、`oodp.rs`、`peers.rs`、`lib.rs`、`builtins/disc.rs`、`crates/oo/src/main.rs`、本工單 §8。

### 8.3 工單哪裡是錯的

無。

### 8.4 工單指名要你回答的問題

Q1. 是一族，不是 13 個點。修前工單點名的 `map_err(|_|` 是 35 處。修後生產碼裡還在的 `map_err(|_|`（測試與註解不算）逐類：

- 理由還在，句子為真：`--to` 不是 `host:port`、CAID 格式、`inspect` 把原錯誤傳回去、PKCS#8 不是那把鑰匙、Ed25519 驗失敗、注入沒有熵、注入 serde 讀不成（Q-055 那句 `working set cannot be read`）、`effect_tags` 讀不成、廣告體 `malformed`、解析器執行緒的 `#host_resource_denied`／內部失敗、熵源 `fill` 只在函式內部變成 `()`。
- 理由換成一個仍然為真的標籤：三處 `addr.parse` 失敗改成 `#peer_unreachable`（這次撥號沒有開始）。操作者金鑰／節點金鑰的 `from_pkcs8` 失敗改成固定的「不是合法 PKCS#8」，不再插 `Debug`。
- 修前是「句子可能為假」、修後不再是 `map_err(|_|`：`CommitLock` 三處「working set unreadable」；`oodp` 把撥號、讀寫、空回應映成 `Conflict` 的那十餘處；`discover`／`find-node` 的 `{e:?}`。
- 留下、句子仍可能為假：`follow_refine` 的鎖中毒與壞的 refine 目標仍是 `#conflict`（`lib.rs`）。沒有操作者路徑在這次的注入矩陣裡走到。沒有改。

`{:?}` 交到操作者面前的，修前是 `discover transport`、`find-node transport`、節點金鑰與操作者金鑰的 `from_pkcs8`。修後這四個都不印 `Debug`。

不經 `operator_io` 的 IO，修前是身分檔、節點金鑰、`discovery.n`、聽埠、`advertise` 的 socket、`atomic_write` 的句子、同儕目錄的失敗被吞掉。這些都改了。`std::env::current_dir` 在 `oo` 裡收成 `cwd()`，失敗是 `cannot read the working directory` 加上同一個理由函數。

Q2. `commit` 要寫入權是因為鎖取在已存在的 `.oo/format` 上，而且是讀寫打開：Windows 的排他鎖要求寫入把手。這次打開不改宣告的位元組。r9（檔案 `0400`）現在答 `cannot lock <路徑>: permission denied`。`status` 仍讀得到工作集。

Q3. 選不留痕跡。注入先落地、○ 寫失敗時，把那個注入檔刪掉，然後把 ○ 的錯誤交回去。r11：rc≠0，`status` 看不到 `d: 4`，句子裡沒有 `atomic_write`。

同一函數裡還有的窗口：模糊片段在注入之前寫進 CAS，注入失敗時那些片段還在，但不是工作集。`effect_pending` 只在 layout≤4 於 ○ 之後寫；layout=7 不走那一步。這次沒有別的「注入在、○ 不在」的路徑。

Q4. `evolve v: _` 在 `.oo/injections` 寫下一個成員。折疊時 `v` 的值是 Top，meet 不把 Top 留在組合裡，所以工作集沒有可提交內容，檔案卻還在。舊的條件把「清單上還有成員」當成被吃掉。修後只有等待過，或兩次清單之間少了成員，才說並行。

Q5. 拿掉。那張網自己把讀不到、鎖不到說成被並行提交吃掉，正是 I2。I1 改由 `operator_io_reason` 的全分支成立之後，不需要再靠 `contains("os error")`。

Q6. `advertise` 對關著的埠答 `#peer_unreachable`（期限是 `#peer_timeout`）。`discover`／`find-node` 答同一個標籤，不再印 `Conflict`。`#fetch`（具名對等點與掃描）把撥不上交成 `#peer_unreachable`，不再掉進「缺席」。空回應（連上了、沒有正文）是 `#peer_timeout`。

撥號與讀寫失敗都不呼叫 `record_integrity`。完整性紀錄仍只在承載解不出或位址不符時寫。連上之後、又不是期限的 IO（例如連線被重置）也答 `#peer_unreachable`：沒有完成一次 OODP 會話，也不是完整性裁決。這比 `#conflict` 接近 D77，但比「根本沒建立」寬一點。沒有第三個標籤可用。

`#fetch` 若在 TCP 已連上之後讀不到自己的節點金鑰，同一函數也回 `#peer_unreachable`。那不是網路。這次的注入矩陣沒有走到。

### 8.5 探針

本弧探針檔未改、未 rustfmt。無 `VOID READING`。17 支皆綠（c1 c2 g1 r1–r14）。

### 8.6 數字

三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`：

- 第 1 輪：`test result:` 245 行，2335 passed，0 failed，`^error` 0，exit 0。
- 第 2 輪：245 行，2334 passed，1 failed，`^error` 2，exit 101。失敗是 `advert_persistence_probe_test::r5_the_rebuilt_index_matches_an_insertion_replay`，panic 原文 `no bucket overflowed with 60 peers, so this probe cannot tell a table rebuilt with the right self id from one rebuilt with zeros`。前一句 `the file lost records` 沒有失敗，60 筆都在檔裡。這是探針自己的武裝條件（隨機節點沒有塞滿桶）。孤立重跑該支：passed。
- 第 3 輪：245 行，2335 passed，0 failed，`^error` 0，exit 0。

conformance：162 vectors，162 pass，0 fail。

`~%Math./add (1, 2)` → `3`，`(1, 3)` → `4`。`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。新鮮倉 `v: 1 + 1` 根 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。新倉 `layout=7`／`encoding=5`。

### 8.7 你認為需要改規格之處

`TAG_REGISTRY` 與 `REAL_02` §3.2.2 加上 `#peer_unreachable`。工單已把這件留給驗收方。引擎這次把新原因加在 `BottomCause` 的尾端。連上之後又不是期限的傳輸失敗，目前也用這個標籤；若要和「根本沒建立」分開，需要另一個原因，這次沒有裁。

---

## 9. 驗收（驗收方填）

### 9.1 已獨立複驗的（交付建置 `694a264`）

*   全樹 ×3（`cargo test --workspace --release --no-fail-fast --jobs 1 -- --test-threads=1`）：**245 target／2335 passed／0 failed，`^error` 0，exit 0**，三輪相同，無失敗測試名
    （交付回報第 2 輪的 `advert_persistence::r5` 間歇紅本次未現；它是 Inbox 既有列）。本弧探針 17／17。
*   Diff 純度：無任何測試檔被改；本工單分隔線以上未動。
*   **重跑偵察矩陣**（同一支腳本，交付二進位，不可讀＋不可寫 × 17 指令 = 340 格）：**零格外洩宿主表示、零格文字說失敗而 rc=0**；`working set unreadable`／`atomic_write` 零命中。I1 對 `.oo/` 內外成立。
*   Q2–Q6 的答案與程式碼一致；`refuse_raw_os` 已刪（I7）。

### 9.2 R-1 第一項：`gc` 把讀不到的根來源當成空的，然後刪掉（**`interrupt-candidate`，既有，驗收方的偵察漏了它**）

〔量，v0.55.0／v0.61.0／交付皆然〕`.oo/HEAD` 不可讀時 `oo gc --grant gc` ⟹ rc=0、「**0 reachable, 5 collectable**」，**全部物件被刪**；恢復權限後 `log` ⟹ `CAID not found`，**歷史沒了**。
`.oo/savepoints` 不可讀 ⟹ 刪掉 2–3 個物件，`log` 同樣壞掉。**`REAL_03` §6.6「打不開的儲存不得換成空的」（Q-055 新增）**，而這裡的後果不是一句錯話，是**無聲的資料遺失**（§2.2 第 2 款）。
**來歷**：驗收方的偵察矩陣只分類了**文字與離開碼**，這兩格 rc=0、無錯字，於是沒被標出；驗收時在矩陣加上**前後狀態比對**（物件數、`log` 還能不能讀）才看見。加上之後全 340 格只有這兩格。
**不變式**：任何會刪除或改寫的動作，其所需的可達性來源（`HEAD`、○、以及任何其他根）**只要有一個讀不到，就必須在動手之前具名拒絕**。**探針** r15、r16。

### 9.3 R-1 第二項：連上之後對端掛斷——**D78（用戶 2026-09-27，選甲）`#peer_closed`**

交付 Q6 自陳：連上之後的非期限失敗答 `#peer_unreachable`，空回應答 `#peer_timeout`——**同一件事兩個名字，兩個都不真**（它連得上；沒有期限到期）。
〔量，對一個接受連線即關閉的同儕〕同一事件依時序（FIN 或 RST）在 `fetch`／`discover`／`find-node` 之間**輪流**得到 `#peer_unreachable` 與 `#peer_timeout`；
**`advertise` 則 rc=0、只印一個換行**（v0.61.0 亦然）——三面不一致的又一個成員。
**D78**：接受了連線、在回答之前關閉 ⟹ **`#peer_closed`**，`REAL_02` §3.2.2 第六列；與 `#peer_unreachable`（沒建立）、`#peer_timeout`（期限到了）三者可分；不是完整性事件。
**不變式**：一個事件一個名字，**不隨時序變**；四條路徑（`fetch`、`discover`、`find-node`、`advertise`）以及任何其他建連線的路徑一致。**探針** r17–r20。

### 9.4 R-1 第三項：`#fetch` 在連上之後讀不到**自己的**節點金鑰，答 `#peer_unreachable`（交付 Q6 自陳）

〔讀 `oodp.rs` `remote_fetch_oodp`〕`oo.node_id()` 失敗 ⟹ `BottomCause::PeerUnreachable`。**那是本地的事，不是對端**——I2。
從外部構造不出那個窗（金鑰在撥號前可讀、之後不可讀），**不設探針，驗收時讀碼**：本地身分讀不到必須以本地的話說出來。

### 9.5 探針校準

r15–r20：交付 `694a264` ×3 紅，各紅在預定斷言上；v0.61.0 亦紅（皆既有缺陷）。原 17 支 ×3 綠。無 `VOID READING`。

### 9.6 不在 R-1

`follow_refine` 鎖中毒與壞的 refine 目標仍答 `#conflict`（交付 Q1 自陳，操作者路徑走不到）⟹ Inbox。

---

## 10. 修補回合 R-1 回報（交付方填；本行以上一字不得動）

### 10.1 射程逐項對照

9.2. `gc` 在標記之前先讀 `HEAD` 與 ○。缺席仍是空的可達集。讀不到（權限、不是找不到）把那句 `cannot read .oo/HEAD` 或 `cannot read .oo/savepoints` 交回去，`run_gc` 在刪除之前返回。走圖時 `previous_commit` 的錯誤同樣中止，不再當成沒有祖先。驗：r15、r16。
9.3. 連線還沒建立：逾時 `#peer_timeout`，其餘 `#peer_unreachable`。連線已經建立：逾時仍是 `#peer_timeout`；對端在回答前結束（乾淨關閉讀到 0 位元組、重置、寫入被切斷）一律 `#peer_closed`。`advertise` 沒有正文時 rc≠0 且印 `#peer_closed`，不再把空讀當成成功。`discover`／`find-node`／`#fetch` 同一規則。驗：r17–r20；r6–r8 仍是 `#peer_unreachable`。
9.4. `remote_fetch_oodp` 在撥號前讀自己的節點金鑰。失敗是 `FetchFail::Local`，句子是 `node_id()` 的 `cannot read …: permission denied`。操作者看到的是這句，不是 `#peer_unreachable`。沒有新的本地標籤（這次沒有裁），所以這個底的 `%cause` 不印出來。

### 10.2 順手改動（逐項指名）

無。`cargo fmt` 未跑。探針檔未改。`spec/`、`meta/`、`conformance/` 未改。

改動檔：`crates/interpreter/src/gc.rs`、`oodp.rs`、`value.rs`、`store_codec.rs`、`lib.rs`、`builtins/disc.rs`、`crates/oo/src/main.rs`、本工單 §10。

### 10.3 工單哪裡是錯的

無。

### 10.4 工單指名要你回答的問題

R1-Q1. 會刪或改、並且要用可達性來源的：
- `gc`：根是 `HEAD` 與 ○。讀不到就拒絕，不刪。見 10.1。
- `squash`：先用 ○ 走祖先（`commit_is_ancestor`）。○ 讀不到時錯誤是 `cannot read .oo/savepoints: …`，發生在 `put_commit` 之前，HEAD 不動。`HEAD` 讀不到則 `init`／`Universe::load` 已經拒絕，進不了 squash。
- `rollback`：不讀 ○ 來決定新的 HEAD，也不刪物件。目標 commit 讀不到時 `get_commit` 的錯誤在 `set_head` 之前。`HEAD` 讀不到同樣在 `init` 就停。
- `migrate`：改的是宣告，不是物件可達性。`.oo/format` 讀不到時是 `cannot read`，在任何宣告寫入之前。
- `commit`：會寫新物件並移動 HEAD，然後才記 ○。注入讀不到時在寫入之前拒絕。它不把讀不到的 ○ 當成空歷史去刪物件。

R1-Q2. 分界是「這次讀寫有沒有碰到期限」，不是 FIN 還是 RST。
- 讀到 0 位元組：`#peer_closed`。
- 連線重置、`Broken pipe`、寫入時對端已掛：`#peer_closed`。
- `TimedOut`／`WouldBlock`：`#peer_timeout`。
- 從未連上（拒絕、地址解析失敗）：`#peer_unreachable`。
- 有正文：照正文。能解出 `%status` 就印那個狀態（那是對端的回答）。解不出的非空正文仍走原來的承載校驗，不改叫 `#peer_closed`。

### 10.5 探針

本弧探針檔未改、未 rustfmt。無 `VOID READING`。23 支皆綠（原 17 支加上 r15–r20）。

### 10.6 數字

三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`，三輪相同：`test result:` 245 行，2341 passed，0 failed，`^error` 0，exit 0。沒有失敗測試名。

conformance：162 vectors，162 pass，0 fail。

`~%Math./add (1, 2)` → `3`，`(1, 3)` → `4`。`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。新鮮倉 `v: 1 + 1` 根 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。

## 11. 驗收 R-1（驗收方填）

**受理。** 交付 `715c6e2`：全樹 ×3 **245 target／2341 passed／0 failed，`^error` 0，exit 0**，三輪相同，無失敗測試名；本弧探針 23／23，無 `VOID READING`。探針檔、分隔線以上未動。
**帶前後狀態比對的偵察矩陣**重跑於交付二進位（340 格）：零外洩、零「文字說失敗而 rc=0」、**零物件遺失、零 `log` 讀不回**。逐格對照 v0.61.0 的離開碼，變化只有兩類：
`gc` 在 `HEAD`／○ 讀不到時拒絕（本回合要的）；**節點金鑰目錄讀不到 ⟹ 具名拒絕開啟**（D73 的延伸——v0.61.0 把「無從判斷金鑰在不在」當成沒有，那正是 D73 禁止的形；已記入 `CHANGELOG`）。
`#fetch` 的本地失敗改為 `FetchFail::Local`，以本地的話說出（讀碼）。身分：`31745ef0…`／`f4f32e7b…`（兩種拼法）／`7038e250…`；known-answer 3／4；conformance 162／162。
**未結、另入 Inbox**：`HEAD` **缺席**（不是讀不到）時 `gc` 仍當成空的可達集而刪光歷史〔量，交付與 v0.61.0 皆然〕——規格沒有規範 `gc` 的根集合，須裁。
