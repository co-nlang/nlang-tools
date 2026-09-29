# 工單：比它的存檔點還老的歷史（Q-063）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-063／裁定 `meta/oo/STATUS.md` **D81**（甲）；前一弧 D79（Q-061）／設計筆記 `meta/oo/commit.md` §1.12（候選）
> 探針（已預先提交並校準）`crates/oo/tests/a_history_older_than_its_savepoints_probe_test.rs`；新夾具 `crates/oo/tests/fixtures/layout2_framed_repo/`（真 v0.40.0），沿用 `encoding4_repo/`（真 v0.35.0）
> 基線：dev `8bf0e5c`（探針提交於 `b0bed79`）／`oo v0.64.0` ⟹ **6 綠 7 紅**；r3 矩陣 **14／14 格紅**，理由逐格皆為缺陷本身、零空洞讀數。

## 1. 缺陷

D79 讓「丟了的 context」具名拒絕，**而它認的證據只有一種**：○ 上的 `commit:` 註記（D52 起，v0.41.0）。更早寫下的儲存沒有這種註記。
〔量，v0.64.0 標籤建置〕把 `.oo/HEAD` 移開：

| 儲存 | `status` | `log` | `evolve` | `gc --grant gc` | `squash` | `repl` |
| :-- | :-- | :-- | :-- | :-- | :-- | :-- |
| 真 v0.40.0（`layout=2`，有框，兩筆提交） | rc=0「no committed root yet」 | rc=0 空 | rc=0，寫入 | rc=0「0 reachable」**刪光 5／5** | rc=1「no HEAD to squash」 | rc=0 |
| 真 v0.35.0（`encoding=4`，JSON，一筆提交） | 同上 | 同上 | 同上 | **刪光 3／3** | 同上 | 同上 |
| 真 v0.40.0，先 `migrate` 到 `layout=8` | — | — | — | **仍刪光 5／5**（遷移不補註記） | — | — |
| 現行引擎兩筆提交，`savepoints/` 也移開 | — | — | — | **刪光 5／5** | — | — |

`commit`（在 `HEAD` 還在時先 `evolve`，再移開 `HEAD`）⟹ rc=0 `Commit successful`，**長出新的創世鏈**。
`rollback <舊提交> --grant rollback` ⟹ rc=0，裝回 `HEAD`，歷史恢復——**○ 裡沒有記載的提交也裝得回去**。這是回頭路，必須保留。
〔讀〕`savepoint.rs` `records_a_commit` 只讀 ○；其註解逐字「Presence of a commit object in CAS is not this declaration」——**D81 推翻的正是這一句**。

## 2. 裁定與依據

*   **D81（用戶 2026-09-29，甲）**：「丟了的 context」的證據是**儲存宣告過提交**——○ 上的提交註記，**或**引擎讀得出來是 Commit 的物件；兩者有一即成立。
*   **Commit 物件是「宣告」不是「推測」的理由**：物件的類別由**引擎寫下的框**（`#nlang/store commit`）或 JSON 年代的 Commit 結構宣告。
    **由內容判讀會誤判**〔量〕：沒提交過的倉可以 `oo eval '~%Engine./save { kind: #Standard, parent: "p", root: "r", s: "#nlang/store commit" }'`，
    存進一個**內容**含那串字、欄位也像提交的值物件（g4 就是這個陷阱；驗收方已確認「看內容」的實作會讓 g4 轉紅）。
*   **已揭露並接受的代價**：首次提交若在 `put_commit` 與 `set_head` 之間崩潰，會留下沒有 `HEAD`、沒有註記的 Commit 物件 ⟹ 本裁定下判為丟了 context；
    回頭路 `rollback <它>` 即完成那次提交。
*   Q-061 工單 Q1 曾要求「不是儲存裡剛好有 commit 物件的推測」——**那句在 D81 之下改讀為「不是由內容推測」**；由框宣告的類別是證據。

## 3. 射程＝不變式（不是機制）

*   **I1** `HEAD` 缺席，而儲存宣告過提交（○ 註記，**或**任何一個引擎讀得出是 Commit 的物件，**本引擎讀得了的每一種編碼**）⟹ D79 的每一個相對於 context 的指令
    （`status`、`log`、`evolve`、`commit`、`gc`、`refine`、`squash`、`repl`）**具名拒絕**，**儲存逐位元組不變**，不長出 `HEAD`（r1–r5、r7）。
    **證據不只住在 ○**：○ 目錄不在了，儲存仍宣告它的提交（r5）。
*   **I2** 物件的類別是**引擎寫下的那個宣告**，不是內容；**所有入口一致**（CLI 與函式庫 API 上凡用到「丟了 context」判準者）（g4）。
*   **I3** 拒絕**對它拒絕的那個儲存是真的**：不得宣稱這個儲存沒有的證據（r6——這些儲存的 ○ 一個註記都沒有）；**點名回頭路** `rollback`（r3）。
*   **I4** **原本做得到的仍做得到**：`HEAD` 在時舊倉照常開、照常讀歷史（g2）；`rollback` 在兩種舊倉上都是回頭路（g3）；誠實的空 context 照常作答、照常開始歷史（g4）；
    與 context 無關的指令照常（g5）；D79 的 ○ 註記路徑不變（g6，以及 Q-061 全部 10 支）。
*   **I5** 值位址、提交位址、佈局宣告一個都不動；不加新的耐久檔；**讀取不寫入**。

## 4. 紅線（今天綠，必須保持綠）

*   g1–g6；Q-061 10、Q-062 14、Q-060 23、Q-059 16＋1、Q-058 9、Q-057 13、Q-055 17；既有 `gc`／`rollback`／`squash`／`migrate` 探針。
*   conformance 162／162；`x: 0` 根 `31745ef0…`、`v: 1 + 1` 根 `f4f32e7b…`、標準根 `7038e250…`；新倉 `layout=8`／`encoding=5`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** 你怎麼判一個物件「是 Commit」？有框的年代讀哪一行、JSON 年代用哪個解碼器？為什麼一個內容像提交的值物件不會觸發？
*   **Q2** 這個判準**什麼時候跑**？`HEAD` 在的時候有沒有付任何新的代價（掃描、讀檔）？`HEAD` 缺席的誠實空倉每個指令付多少（量一個數字即可）？
*   **Q3** 掃描途中遇到**讀不到的物件檔**（權限、截斷、壞框）你答什麼？依據哪一條（`REAL_03` §6.6「打不開的儲存不得換成空的」）？**事實陳述題，照你做的寫**。
*   **Q4** 拒絕句現在說什麼？對「只有 ○ 註記」「只有 Commit 物件」「兩者都有」三種儲存，各自是真的嗎？
*   **Q5** 你有沒有找到第三種「儲存宣告過提交」的形式（本引擎讀得了、而上兩種都不涵蓋）？有就列出，**不要自行擴大射程**。

## 6. 明文不在射程內

*   `SPEC_08` §6.2.1 的文字（MUST 括號裡的證據、自陳盲區註記）：驗收方收尾。
*   `migrate` **不得**為舊倉補造 ○ 註記——註記記的是「這一次提交發生時」的事件，事後寫出來就是捏造；本弧也不需要它（r7）。
*   `HEAD` 的恢復要不要專門的動作（`commit.md` §1.12.7 ③）：未裁。
*   `follow_refine` 的兩條 `#conflict` 臂（Inbox，無操作者路徑）：若你本來就碰 `lib.rs` 可順手，**須在 §8.2 指名**；不碰也行。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r7 基線必須是紅的，理由逐支寫在 doc comment；**你可以讓它們變綠，不得改寫它們的宣稱。**
*   **夾具不准動、不准重建**：g1 以摘要釘住兩個夾具的每一個位元組。
*   判準只在兩處釘拼法：回頭路的指令名（`rollback`），與現行拒絕句自己的宣稱（`savepoint records a commit`——對這些儲存是假的）。
*   **探針用到的儀器不准改**：`oo log`／`status`／`inspect`（`root:` 行）／`gc`／`rollback`／`migrate`／`eval` 與 `~%Engine./save` 的既有輸出結構與旗標；「丟失」一律以移開 `.oo/HEAD` 構造。
    **若你認為某支的判準不可能被一個正確的實作滿足，寫在「工單哪裡是錯的」，不要改儀器去對上它。** `rustfmt` 不得掃探針檔。
*   〔驗收方校準〕兩極皆做：基線 6 綠 7 紅；一個約 30 行、只改 `records_a_commit` 的參考實作 ⟹ 本探針 13／13、Q-061 10／10 全綠
    ⟹ **沒有正確實作也過不了的設置**。把判準換成「看內容」⟹ g4 紅 ⟹ **陷阱有效**。
    同一參考實作跑全樹：**248 target／2378 passed／0 failed，exit 0** ⟹ **沒有既有測試與 D81 衝突**，本弧不預先修訂舊探針。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

I1. `HEAD` 缺席，而 ○ 有 `commit:` 註記，或物件被本引擎讀成 Commit（有框的 `#nlang/store commit`，以及 JSON 的 `Commit` 解碼），`status`、`log`、`evolve`、`commit`、`gc`、`refine`、`squash`、`repl` 具名拒絕。儲存位元組不變，不長出 `HEAD`。r1–r5、r7。註記不在（○ 目錄被移走）仍認得出提交物件（r5）。

I2. 類別是框或 JSON 解碼器，不是內文。g4 的值物件含 `"#nlang/store commit"`，仍是誠實空 context。CLI 與函式庫（`Universe`、`Ouroboros::log`、`gc`）走同一個 `records_a_commit`。

I3. 拒絕句不宣稱這些倉沒有的 ○ 註記（r6），並點名 `rollback`（r3）。

I4. `HEAD` 在的舊倉照常讀歷史（g2）。兩種舊倉的 `rollback` 仍是回頭路（g3）。誠實空 context 照常作答、照常開始歷史（g4）。與 context 無關的指令照常（g5）。D79 的註記路徑不變（g6，Q-061 10 支）。

I5. 值位址、提交位址、佈局宣告未動。沒有新的耐久檔。掃描只讀目錄與物件，不建目錄、不寫檔。`migrate` 不補註記。

### 8.2 順手改動（逐項指名）

`gc.rs` 的註解原寫「沒有 `commit:` 註記就是空走訪」。那句是 D81 推翻的規則，改成與判準同一句話。沒有碰 `follow_refine`。`cargo fmt` 未跑。探針檔與夾具未改。

改動檔：`crates/interpreter/src/savepoint.rs`、`crates/interpreter/src/gc.rs`、本工單 §8。

### 8.3 工單哪裡是錯的

無。

### 8.4 工單指名要你回答的問題

Q1. 有框的年代看 `#nlang/store` 後面那一段：是 ` commit` 才交給 `decode_commit`（與 `decode_document` 同一條前綴）。JSON 年代用 `serde_json` 解成 `Commit`，與 `get_commit` 對無框位元組用的是同一個解碼器。值物件的框是 `#nlang/store` 換行，內文裡的那串字不會變成這個前綴，所以 g4 不觸發。

Q2. 判準在既有的「丟了 context」檢查裡，而且那些檢查都是 `get_head` 為 `None` 才呼叫。〔量〕已有 `HEAD` 的 `oo status`：沒有開啟 `.oo/savepoints`，也沒有把 `.oo/objects/sha256` 當目錄掃。誠實空倉的 `oo status`：兩次目錄開啟，都是 ENOENT（`.oo/savepoints`、`.oo/objects/sha256`），物件檔 0。有註記時先返回，不再讀物件。

Q3. 權限與其他讀取錯誤（`NotFound` 除外）答 `cannot read store objects: ` 接 `operator_io_reason`。這不是空 context，也不是丟了 context 那句。依據 `REAL_03` §6.6：打不開的儲存不得換成空的。讀得到、框是 `#nlang/store commit`、但 `decode_commit` 失敗：答 `cannot read store object: commit frame does not decode`（宣告了而打不開，不是缺席）。截斷或壞框若不是這個提交框，以及 JSON 解不成 `Commit`，都不是提交宣告，掃描繼續。物件目錄 `NotFound` 是沒有物件。

Q4. 拒絕句是 `lost context: HEAD is absent and the store records a commit; restore it with rollback <commit> --grant rollback`。只有 ○ 註記：這句是真的（儲存記過提交）。只有 Commit 物件：這句是真的，而且不含 `savepoint records a commit`（r6）。兩者都有：這句是真的。

Q5. 沒有第三種。本引擎的雜湊演算法只有 Sha256。提交只寫成有框的 `#nlang/store commit`（encoding ≥ 5）或 serde JSON 的 `Commit`（encoding ≤ 4）。`ancestor:` 不是「發生過一次提交」的宣告。沒有自行擴大。

### 8.5 探針

本弧探針檔未改、未 rustfmt。無 `VOID READING`。13 支皆綠（g1–g6、r1–r7）。Q-061 10 支皆綠。

### 8.6 數字

三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`，三輪相同：`test result:` 248 行，2378 passed，0 failed，`^error` 0，exit 0。沒有失敗測試名。各行的耗時不同，通過與失敗的數目相同。

conformance：162 vectors，162 pass，0 fail。

`~%Math./add (1, 2)` → `3`，`(1, 3)` → `4`。`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。新鮮倉 `v: 1 + 1` 根 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`。新倉 `layout=8`／`encoding=5`。

### 8.7 你認為需要改規格之處

`SPEC_08` §6.2.1 的證據仍只寫 ○ 註記。D81 加上 Commit 物件。工單 §6 把該段留給驗收方，此處未改。

---

## 9. 驗收（驗收方填）

**受理，零修補回合。** 交付 `06d18aa`：全樹 ×3 **248 target／2378 passed／0 failed，`^error` 0，exit 0**，三輪相同，無失敗測試名；本弧探針 13／13，無 `VOID READING`；Q-061 10、Q-062 14、Q-060 23、Q-059 16 全綠。探針檔、夾具、分隔線以上未動；改動只在 `savepoint.rs` 與 `gc.rs`（註解）。
**狀態矩陣**（6 種儲存 × 17 指令，逐格比對 `.oo/` 全檔雜湊，並與 v0.64.0 逐格 diff）：兩種舊倉與「○ 被移走的現行倉」上，`status`／`log`／`evolve`／`commit`／`gc`／`squash`／`repl` 具名拒絕、儲存不變；`rollback` 裝回 `HEAD`；`run`／`eval`／`test`／`fmt`／`lint`／`inspect`／`identity`／`migrate`／`node id` 照答。誠實空倉（含內容像提交的值）逐格與 v0.64.0 相同。
**Q3 的新行為成立且已入規格**：`HEAD` 缺席的誠實空倉裡有讀不到的物件 ⟹ 相對於 context 的指令拒絕（`REAL_03` §6.6）。〔strace〕`HEAD` 在時不開物件目錄（Q2）。
身分：`31745ef0…`／`f4f32e7b…`（`1 + 1` 與 `1+1`）／標準根 `7038e250…`；known-answer 3／4；conformance 162／162；新倉 `layout=8`／`encoding=5`。
