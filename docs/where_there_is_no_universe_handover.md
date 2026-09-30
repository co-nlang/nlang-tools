# 工單：沒有宇宙的地方（Q-064）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-064／裁定 `meta/oo/STATUS.md` **D82**（乙）＋ **D82 ②**（甲）＋ **D82 ③**（甲）
> 探針（已預先提交並校準）`crates/oo/tests/where_there_is_no_universe_probe_test.rs`
> **既有測試 26 個檔已由驗收方預先修訂**（每處標 `AMENDED 2026-09-30 for Q-064 (D82)`），見 §7。
> 基線：dev `0442198`／`oo v0.65.0` ⟹ **7 綠 6 紅**，三輪一致、零空洞讀數。

## 1. 缺陷

〔量，v0.65.0 標籤建置，空目錄〕**幾乎每個指令都會順手建出一個宇宙**（`.oo/` 加兩份宣告）：

| 指令 | 今天 | 另外說錯的話 |
| :-- | :-- | :-- |
| `status`／`log`／`gc`／`repl`／`node peers` | rc=0，建出 `.oo/` | `status`「no committed root yet … Universe is static」——前提是有一個宇宙 |
| `migrate --grant migrate` | rc=0，建出 | 「**Nothing was changed.**」——整個倉是它剛建的 |
| `commit` | rc=1「Nothing to commit」，仍建出 | 失敗了還是寫了 |
| `inspect <CAID>` | rc=1，建出 | 「CAID not found in **local store**」——那個倉是它剛建的 |
| `run`／`test` | rc=0，建出 | `run` 自述「**does not write this workspace**」；唯讀目錄下 `run`／`test` 直接失敗（`eval` 不會，它有暫時儲存） |
| `eval '~%Engine./save …'` | 回位址，建出 | —— |
| `node id`／`affiliate`／`discover`／`advertise`／`find-node`／`serve` | 建出宣告 | —— |
| `fmt`／`lint`／`identity`／`node trust list` | 不建 | —— |

**併入本弧、不需新裁定的一項**（`REAL_02` §5.1.1：缺宣告 ⟹ 拒絕開啟〔MUST〕；讀取路徑不得寫宣告〔MUST NOT〕）：
〔量，v0.27.0 起每一版〕**只有提議**（staged／注入／○）而沒有物件的倉，拿掉 `format` 之後被讀成**新倉**，唯讀的 `status` 替它補寫**目前**的佈局。
有提交的倉則每一版都正確拒絕。〔讀〕`storage.rs` `durable_store_present` 與 `ObjectStore::init` 的「新倉」判準是「`format` 或 `HEAD` 或物件」——**與 D82 是同一個述詞**。

## 2. 裁定與依據

*   **D82（用戶，乙）**：沒有宇宙的地方，**讀不寫**；**需要一個宇宙才答得出**的指令**具名拒絕、rc≠0**；**不需要**的照答、不寫；**會建宇宙的只有 `evolve`**。
    理由：`status`／`log` 問的是「這個宇宙」，沒有宇宙就答不出（與 D79 同形）；在錯的目錄下下指令時，rc=0 讓腳本分不出「這裡沒有宇宙」與「這裡有一個空宇宙」（後者依 `SPEC_08` §6.2.1 仍 rc=0）。**不預先決定日後 `init` 的形狀**。
*   **D82 ②（甲）**：D82 管的是**宇宙**，不是 `.oo/` 這個目錄。`.oo/` 放兩層：**宇宙**（宣告、物件、`HEAD`、提議、○）與**節點設定**（`discovery.n`、`peers/`）——`storage.rs` 的既有註解早已把 `discovery.n` 當成「倉之前的設定」。
    `node` 指令在沒有宇宙的地方照常讀寫節點設定，**不得**寫宣告或宇宙內容。
*   **D82 ③（甲）**：沒有宇宙時 `~%Engine./save`（及其別名 `~%Discovery./identify_and_store`）答 **⊥，成因 `#no_universe`（新登記）**，**不回位址**——回位址就是宣稱存下了。

## 3. 射程＝不變式（不是機制）

*   **I1** 沒有宇宙的地方，**只有 `evolve` 建出宇宙**。其他指令**一個宇宙檔都不寫**；`node` 指令最多留下節點設定（r1）。
*   **I2** 需要宇宙的指令——`status`、`log`、`commit`、`gc`、`migrate`、`squash`、`refine`、`rollback`、`repl`、`inspect`——**具名拒絕、rc≠0、不寫**（r2），
    拒絕**點名 `evolve`**（開始一個宇宙的唯一方式；r3）。
*   **I3** 不需要宇宙的指令——`eval`、`run`、`test`、`fmt`、`lint`、`identity`、`node *`——**照答今天的答案、不寫宇宙**（g2、r1）。
    **而且照常讀節點設定**：沒有宇宙的地方，`run` 的 `~%Discovery./fetch` 仍會用已准入的同儕（驗收方校準時量到：只給記憶體引擎會讓 `automatic_admission` 八支紅）。
*   **I4** 在沒有宇宙的地方**說出來的每一句都是真的**：上表四句假話不得再出現（r3）；`save` 不得回一個沒存下的位址（r6）。
*   **I5** **放著宇宙內容的 `.oo/` 就是一個倉**：缺宣告 ⟹ **拒絕開啟、不寫**，`evolve` 也一樣（r4）。**只放節點設定的 `.oo/` 還不是宇宙**：I2 適用、不得被寫上宣告（r5）；`evolve` 可以在那裡建宇宙。
*   **I6** **原本做得到的仍做得到**：`evolve` 之後誠實的空宇宙照常作答、照常提交（g1）；有提交而缺宣告的倉照舊拒絕（g3）；既有倉裡的節點設定照舊（g4、g6）；舊佈局的倉宣告不被改（g5）；宇宙裡 `save` 照舊存下並回位址（g7）；**所有既有探針**。
*   **I7** 值位址、提交位址、佈局宣告一個都不動；不加新的耐久檔。

## 4. 紅線（今天綠，必須保持綠）

*   g1–g7；Q-063 13、Q-062 14、Q-061 10、Q-060 23、Q-059 16＋1；**§7 列出的 26 個已修訂檔**。
*   conformance 162／162；`x: 0` 根 `31745ef0…`、`v: 1 + 1` 根 `f4f32e7b…`、標準根 `7038e250…`；新倉 `layout=8`／`encoding=5`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** 你的「這裡有宇宙」述詞：`.oo/` 裡**哪些檔**算宇宙內容、哪些算節點設定（`discovery.n`、`peers/`、`architects.json`……），理由。**一個空的 `.oo/` 算什麼？**
*   **Q2** 逐一列出每個指令（含 `node *` 各子指令、`test --static`、`run --format`）你歸為：建立者／需要宇宙／不需要宇宙，以及理由。與 §3 不同處寫在 §8.3。
*   **Q3** v0.65.0 在空目錄 `node trust add` 會留下**只有 `discovery.n`** 的 `.oo/`；而 v0.64.0 以前的 `status` 會在同樣的目錄寫宣告。今天之後這種目錄：`status` 說什麼、`evolve` 做什麼、節點設定是否保留？
*   **Q4** `run` 的說明寫「does not write this workspace」，而宇宙裡的 `~%Engine./save` 確實寫進工作區（本弧不改這個行為）。那句自述現在是真的嗎？**事實陳述題**。
*   **Q5** `#no_universe` 的離開碼類別（`TAG_REGISTRY` §0.2）你選了哪一類，理由。另：`save` 在宇宙裡 `put_value` 失敗時今天答 `#conflict`——**你碰到了就指名，不要擴大射程**。
*   **Q6** 程式庫入口（`Ouroboros::init` 等）今天仍會建倉嗎？若會，哪些呼叫者走得到它？

## 6. 明文不在射程內

*   `init` 的形狀（`commit.md` §4.4 弧 D 第 2 項）。
*   `REAL_02` §5.1.1、`REAL_03` §6.6（暫時儲存的正當條件）、`SPEC_08` §6.2.1、`TAG_REGISTRY` 的文字：驗收方收尾。
*   `run` 在宇宙裡 `save` 會寫工作區的既有行為（Q4 只問事實）。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r6 基線必須是紅的，理由逐支寫在 doc comment；**你可以讓它們變綠，不得改寫它們的宣稱。**
*   判準只在這些地方釘拼法：`evolve`（開始宇宙的指令名）、`#no_universe`（D82 ③ 的成因）、以及缺陷自己的四句假話。
*   **既有測試的預先修訂（驗收方，2026-09-30）**：26 個檔，每一處都是同一件事——**原本靠 `status`／`run`／`save` 在空目錄順手建倉的設置，改成先 `evolve` 一個種子檔**
    （`nothing_here` 的並行建倉競賽改成並行 `evolve`；`what_it_says` r10 的「從來沒東西」對照組改成 `evolve` 一個空檔）。
    **兩邊驗證**：26 檔在 v0.65.0 基線上 **344／344 綠**（修訂不改變它們量的東西）；在驗收方的參考實作上 **344／344 綠**（它們只因為這一條而紅）。
    **你不得再改這 26 個檔**；若某處修訂錯了，寫在 §8.3。
*   **探針用到的儀器不准改**：CLI 既有輸出結構與旗標；「沒有宇宙」一律是全新空目錄，身分與節點家目錄在工作目錄之外。`rustfmt` 不得掃探針檔與修訂檔。
*   〔驗收方校準〕**兩極皆做**：基線 7 綠 6 紅；一個粗略的參考實作（指令分派三種引擎：需要宇宙／只讀節點設定＋暫時值儲存／建立者；「有宇宙」述詞＝宣告、`HEAD`、staged、注入、○、物件任一；`save` 無宇宙 ⟹ ⊥）⟹ 本探針 13／13。
    **校準途中參考實作錯了兩次，兩次都被既有測試抓到**：第一次把 `node` 家族歸為「需要宇宙」⟹ 207 紅（⟹ 用戶裁 D82 ②）；第二次讓無宇宙的 `run` 用純記憶體引擎 ⟹ `automatic_admission` 八支紅（⟹ I3 的「照常讀節點設定」）。
    修正後的參考實作跑全樹（含 26 個修訂檔與本探針）：**249 target／2391 passed／0 failed，exit 0**。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

I1. 沒有宇宙時，只有 `evolve` 建倉。其他指令不寫宣告、物件、`HEAD`、提議、○。`node` 最多留下節點設定（r1）。

I2. `status`、`log`、`commit`、`gc`、`migrate`、`squash`、`refine`、`rollback`、`repl`、`inspect` 拒絕，rc=1，句子是 `no universe here: start one with evolve`。不寫（r2、r3）。

I3. `eval`、`run`（含 `--format`、`--observe`）、`test`（含 `--static`）、`fmt`、`lint`、`identity`、全部 `node *` 照答，不建宇宙。沒有宇宙時仍從工作區讀 `discovery.n`、`peers/`、`architects.json`，值在暫時倉（g2、r1）。

I4. 沒有宇宙時不印那四句。`~%Engine./save` 與 `~%Discovery./identify_and_store` 印 `_|_  ;; %cause: #no_universe`，沒有位址，工作區沒有 `.oo/`（r3、r6）。

I5. 已有宇宙內容而缺 `format`：拒絕開啟、不寫，`evolve` 也一樣（r4）。只有 `discovery.n` 或只有 `peers/directory`：`status` 拒絕，位元組不變（r5）。空的 `.oo/` 同樣拒絕、不寫。

I6. `evolve` 之後的誠實空宇宙照常作答、照常提交（g1）。有提交而缺宣告仍拒絕（g3）。既有倉的節點設定仍在（g4、g6）。layout 7 的宣告不變（g5）。宇宙裡的 `save` 仍存下並回位址（g7）。Q-063、Q-062、Q-061 與 26 個修訂檔都在全樹裡。

I7. `x: 0` 根、新鮮 `v: 1 + 1` 根、標準根、`layout=8`／`encoding=5` 都沒動。沒有新的耐久檔。

### 8.2 順手改動（逐項指名）

`engine_or_ephemeral` 刪了。它在可寫的空目錄上會先把倉建出來，這就是本弧的缺陷。

`durable_store_present` 改成跟著 `universe_content`；讀不到算「在」，呼叫者不得換成空倉。CLI 已經不再呼叫它。

拿掉只服務舊述詞的 `has_cas_objects`、`Presence`、`stated`、`objects_hold_anything`。

`BottomCause::NoUniverse` 加在列舉尾巴。`primary_rank` 與 `NoContext` 同為 2。`new_in_memory` 的 `holds_universe` 仍是 true，記憶體裡的 `save` 照舊存進暫時倉。

`run` 的說明句、探針、26 個修訂檔、規格、版本都沒改。沒跑 rustfmt。沒動 `savepoint.rs`、`gc.rs`。宇宙裡 `put_value` 失敗仍答 `#conflict`。

改動檔：`crates/interpreter/src/storage.rs`、`lib.rs`、`builtins/engine.rs`、`value.rs`、`store_codec.rs`、`crates/oo/src/main.rs`、本工單 §8。

### 8.3 工單哪裡是錯的

工單沒有寫錯。26 個修訂檔沒再改。

實作時一度把 `objects.format` 算進「有宇宙」。它寫在 `format` 之前。`nothing_here` r1（驗收方已改成並行 `evolve`）於是在 `format` 尚未落地時拒絕，374／1080，句子是 `cannot determine this store's layout: `.oo/format` is absent`。那個檔因此不進述詞。修正後該檔 7／7，本弧探針 13／13。

### 8.4 工單指名要你回答的問題

Q1. 宇宙內容是這些見證之一：`format`、`HEAD`、`staged`、`effect_pending`、`pin_pending`、`abandoned`，以及 `objects/`、`injections/`、`savepoints/` 底下的任何檔案。`format` 是佈局宣告，後三者是物件與提議，中間三個檔是還沒進 ○ 的提議狀態。

節點設定是 `discovery.n`、`peers/`、`architects.json`。缺檔時讀成空，不創建。〔量〕只有 `architects.json` 的 `.oo/`：`status` rc=1，檔不變。

空的 `.oo/` 沒有宇宙。`.oo` 讀不到時也不是缺席：`universe_content` 回錯誤，`durable_store_present` 把錯誤看成「在」。

`objects.format` 單獨存在時述詞是沒有宇宙。它先寫、`format` 後寫；把它算進去會讓並行的 `evolve` 看見半份宣告（§8.3）。之後的 `evolve` 仍會把這一對寫完。`status` 在這個目錄不寫。空的 `objects/` 目錄（底下沒有檔）也不是見證。

Q2. 與 §3 相同，沒有改歸類。

建立者：`evolve`。它是唯一被允許在沒有宇宙時寫宣告的指令。

需要宇宙：`status`、`log`、`commit`、`gc`（含 `--dry-run`）、`migrate`、`squash`、`refine`、`rollback`、`repl`、`inspect`。它們答的是這個宇宙的工作集、歷史、物件或一次寫入；沒有宇宙就沒有可答的對象。

不需要宇宙：`eval`；`run`（含 `--format`、`--observe`）；`test`（含 `--static`）；`fmt`（`--write` 改的是指名的原始檔）；`lint`；`identity`（`OO_IDENTITY` 或 `~/.oo/identity`，在工作區之外）；`node serve`、`node id`、`node advertise`、`node discover`、`node find-node`、`node affiliate`、`node peers`、`node trust list`、`node trust add`、`node trust remove`。節點指令讀寫的是節點設定與節點家目錄，值在暫時倉。

Q3. 〔量，本交付的 `oo`〕空目錄 `node trust add`：rc=0，`.oo/` 裡只有 `discovery.n`。接著 `status`：rc=1，`Error: no universe here: start one with evolve`，`discovery.n` 位元組不變。接著 `evolve`：rc=0，寫上 `layout=8`、`encoding=5`，`discovery.n` 仍是原來那一份。

Q4. 那句說明現在仍不真。文字沒改，仍是 “does not write this workspace”。〔量〕沒有宇宙時，`run` 一個呼叫 `~%Engine./save` 的程式：rc=0，印 `_|_  ;; %cause: #no_universe`，不建 `.oo/`。已有宇宙時，同一個 `run`：rc=0，印 `1` 的位址 `17564af7…`，物件寫進這個工作區。後者是 I6／g7 要留的行為。

Q5. `#no_universe` 是原因，載體是 ⊥。`TAG_REGISTRY` §0.2：消費者看到的是值本身，離開碼必須為 0。〔量〕`eval` 與 `run` 都是 rc=0，stdout `_|_  ;; %cause: #no_universe`。`status` 那類拒絕是邊界診斷，rc=1，句子裡沒有這個標籤。

`engine.save` 在已有宇宙且 `put_value` 回 `Err` 時，仍是 `BottomCause::Conflict`。這條沒改。

Q6. `Ouroboros::init` 在 `universe_content` 為假時仍會建倉。走得到它、而且真的會建的呼叫者：CLI 的 `evolve`；程式庫與測試裡直接呼叫 `Ouroboros::init` 的那些（interpreter 的探針測試，以及 `oo` 的 `held_but_unopenable`、`what_the_shelf_does_not_hold`、`a_name_the_printer_could_not_write`）。需要宇宙的 CLI 指令先 `require_universe`，那次 `init` 面對的已經是宇宙，不會建。`engine_keeping_settings` 只在已經有宇宙內容時呼叫 `init`。`without_universe` 與 `new_in_memory` 的 `ObjectStore::init` 開在暫時目錄，不寫工作區。

### 8.5 探針

本弧探針檔未改、未 rustfmt。無 `VOID READING`。13 支皆綠（g1–g7、r1–r6）。26 個修訂檔未改。

### 8.6 數字

三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`，三輪相同：`test result:` 249 行，2391 passed，0 failed，`^error` 0，exit 0。沒有失敗測試名。各行的耗時不同；把 `finished in` 換成同一記號之後，三輪的 `test result:` 行相同。

conformance：162 vectors，162 pass，0 fail。

`~%Math./add (1, 2)` → `3`，`(1, 3)` → `4`。`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。新鮮倉 `v: 1 + 1` 根 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。標準根物件 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`。新倉 `layout=8`／`encoding=5`。

### 8.7 你認為需要改規格之處

`#no_universe` 還沒有寫進 `TAG_REGISTRY`。`REAL_02` §5.1.1、`REAL_03` §6.6、`SPEC_08` §6.2.1 的文字仍是驗收方收尾，此處未改。

`run` 的說明句在宇宙裡呼叫 `save` 時與事實不符。工單只要這句的事實、要留下那個寫入，所以說明文字沒動。

---

## 9. 驗收（驗收方填）
