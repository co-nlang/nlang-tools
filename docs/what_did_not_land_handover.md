# 工單：沒有落地的東西（Q-065）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-065／裁定 `meta/oo/STATUS.md` **D83**（甲）；其餘是既有 MUST：`REAL_03` §6.6、`SPEC_08` §6.2 R1、`REAL_01` §4.1.1、Q-060 I2
> 探針（已預先提交並校準）`crates/oo/tests/what_did_not_land_probe_test.rs`
> 基線：dev `ac2a39d`／`oo v0.66.0` ⟹ **3 綠 5 紅**，三輪一致、零空洞讀數。

## 1. 缺陷

宿主邊界上**寫入**那一側，以及 `.oo/` 裡那些「先讀再改寫或消耗」的耐久記錄。〔量，v0.66.0 標籤建置，除註明〔讀〕者〕：

| | 注入 | 今天 | 違反 |
| :-- | :-- | :-- | :-- |
| **M1** | `.oo/abandoned` 讀不到 | `rollback` rc=0，**覆寫它**（2 行 → 1 行，一筆放棄記錄消失）；`commit` rc=0，新提交**沒有** `abandoned:` 欄，**然後刪檔**（對照組：提交物件逐字帶 `abandoned: [...]`） | `REAL_03` §6.6、`SPEC_08` §6.2 R1 |
| **M2** | `.oo/injections/` 不可寫 | `commit`「Commit successful」rc=0、`HEAD` 前進，**被折進去的注入留著**；`status` 把已提交的 `a: 1` 列為待提交，下一次提交再折一次 | Q-060 I2（說的就是發生的事） |
| **M3** | `objects/` 不可寫 | 宇宙裡 `~%Engine./save` 答 ⊥ `#conflict`（格上的衝突） | D83 |
| **M4** | 宿主不讓寫的目錄 | `~%Io./write_file`、`~%Io./append_file` 答 `#none`（「沒有」；對照組 `#true`） | D83 |
| **M5** | —— | 〔讀〕同儕目錄 append（`peers.rs:414`）只 `flush`、**無 `sync_all`**，且 `flush` 的結果被丟掉；寫入失敗時的內部句子是「cannot write …: unreadable」。對外回報已誠實（`#rejected #cannot_record`，Q-060） | `REAL_01` §4.1.1 |
| **M6** | —— | 〔讀〕提交後清除 `pin_pending`／`effect_pending`／`abandoned`／`staged` 與注入檔時一律 `let _ =`。前四者在 `.oo/` 頂層，權限造不出（`.oo/` 不可寫則提交在寫 `HEAD` 時已先失敗） | Q-060 I2 |

〔讀〕`universe.rs` `load_abandoned_file` 以 `.ok()?` 吞掉讀取錯誤，兩個呼叫者（commit 讀它寫進 meta、rollback 讀它再覆寫）；`injections::clear` 以 `let _ =` 丟掉刪除錯誤。

## 2. 裁定與依據

*   **D83（用戶，甲）**：宿主拒絕一次寫入 ⟹ **⊥，成因 `#unwritable`（新登記）**。寫入是動作不是觀測：「沒有照你要的方式發生」是確定的，不是未知。
    **已揭露的限制**：`write_file` 非原子，失敗時可能留下半截——⊥ 不得讀成「什麼都沒寫」。**不預先決定 EINTR 那一題**：本成因只說沒落地，不宣稱永久。
*   其餘皆既有 MUST，不需裁定：**讀不到不得當成沒有**（`REAL_03` §6.6）；**被放棄的整段確實發生過**（`SPEC_08` §6.2 R1）；**`.oo/` 下任何耐久寫入都要落地**（`REAL_01` §4.1.1）；**說的就是發生的事**（Q-060 I2）。

## 3. 射程＝不變式（不是機制）

*   **I1** `.oo/` 裡一份耐久記錄**讀不到**時，任何要讀它再改寫、或讀它再消耗的指令**具名拒絕、不寫、不刪**——不得當成空的（r1、r2）。**這是一條類別的不變式**：放棄記錄是量到的成員，**凡同形者皆在射程**（見 Q1）。
*   **I2** 一次**回報成功**的提交，已經**消耗了它折進去的提議**：之後 `status` 不再列出它們、下一次提交不再找到它們。做不到就**在落地之前拒絕**，或**落地並說出沒做完的那一半**——**但無論哪一種，被折進去的提議都不得再以提議的身分出現**（r3）。
    **不寫機制**：可以先確認再落地，可以記下已折者，可以別的方式。
*   **I3** 宿主拒絕的寫入答 **⊥ `#unwritable`**：`~%Engine./save`（含別名 `identify_and_store`）、`~%Io./write_file`、`~%Io./append_file`——**不得**是 `#conflict`、`#none`，**不得**回位址（r4、r5）。**凡語言層向宿主寫入者皆在射程**（見 Q2）。
*   **I4** `.oo/` 下的耐久寫入**落地**（`REAL_01` §4.1.1）：同儕目錄 append 要 `sync_all`，寫入失敗的句子說「寫不進」不說「讀不到」。**探針量不到斷電語義**，驗收方以讀碼與 strace 驗。
*   **I5** 丟掉錯誤的清除（M6）：**每一處**要嘛說出失敗，要嘛寫明為何丟掉它是真的（例如「下一步會以別的方式發現」）——**不得無聲**。
*   **I6** **原本做得到的仍做得到**：讀得到的放棄記錄照舊進提交並被清除（g1）；宿主允許的寫入照舊 `#true`／回位址（g2）；一般提交照舊消耗工作集（g3）；**所有既有探針**。
*   **I7** 值位址、提交位址、佈局宣告一個都不動；不加新的耐久檔。

## 4. 紅線（今天綠，必須保持綠）

*   g1–g3；Q-064 13、Q-063 13、Q-062 14、Q-061 10、Q-060 23（含 `#cannot_record` 那幾支）。
*   conformance 162／162；`x: 0` 根 `31745ef0…`、`v: 1 + 1` 根 `f4f32e7b…`、標準根 `7038e250…`；新倉 `layout=8`／`encoding=5`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** 逐一列出 `.oo/` 裡**所有**「讀了再改寫或消耗」的耐久記錄與它們的讀取路徑，每一條注入「讀不到」之後答什麼。**事實陳述題**；放棄記錄之外若有同形者，一併修（I1 是類別）。
*   **Q2** 逐一列出語言層所有**向宿主寫入**的內建（`~%Io` 與其他模組），各自寫入失敗時答什麼。
*   **Q3** I2 你選了哪一種（先確認再落地／落地並說出／別的）。若選「先確認」：確認與寫入之間的時間窗怎麼處理？若選「落地並說出」：rc、句子、以及下一次指令怎麼知道哪些已折？
*   **Q4** M6 每一處 `let _ =` 你改成什麼，或為何留著是真的。
*   **Q5** `#unwritable` 的 `primary_rank`（`TAG_REGISTRY` §0.2 的離開碼類別）與理由。`write_file` 失敗而留下半截時，你的答覆有沒有說出這件事？（事實陳述）

## 6. 明文不在射程內

*   EINTR 與「暫時／永久」之分（Inbox，須裁）。
*   `run` 自述在宇宙裡不真（Inbox，須裁）。
*   `TAG_REGISTRY`、`REAL_01` §4.1.1、`SPEC_08` §6.2 的文字：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r5 基線必須是紅的，理由逐支寫在 doc comment；**你可以讓它們變綠，不得改寫它們的宣稱。**
*   判準只在這些地方釘拼法：成因 `#unwritable`／`#conflict`／`#none`，與提交物件自己位元組上的欄位名 `abandoned:`。
*   **故障一律以權限造**，每一個故障在被依賴之前先確認已生效（以 root 執行即空洞讀數）。**探針用到的儀器不准改**：CLI 既有輸出結構與旗標、提交物件的磁碟形。`rustfmt` 不得掃探針檔。
*   〔驗收方校準〕**兩極皆做**：基線 3 綠 5 紅（三輪一致）；一個最小參考實作（`load_abandoned_file` 回錯誤而非 `None`、提交落地前確認注入目錄可寫、三個寫入內建失敗答 ⊥ `#unwritable`）⟹ 本探針 8／8；同一參考實作跑全樹 **250 target／2399 passed／0 failed，exit 0** ⟹ **沒有既有測試與 D83 衝突**，本弧不預先修訂舊測試。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

I1. `.oo/abandoned` 讀不到時，`rollback`、`commit`、`squash` 在寫入之前拒絕，句子是 `cannot read <路徑>: permission denied`。檔位元組不變。`rollback` 的倉快照不變（r1）。`commit` 不移動 HEAD、不刪檔（r2）。缺檔是空記錄。同形的 `pin_pending`、`effect_pending`、legacy `staged`、注入成員、○，讀取的 IO 錯誤同樣具名拒絕。

I2. 有折進去的注入時，落地前先確認 `injections/` 能建立新檔。確認失敗則 HEAD 不動、rc≠0（r3）。一般提交仍清掉注入，第二次提交是 `Nothing to commit`（g3）。

I3. `~%Engine./save`、`~%Io./write_file`、`~%Io./append_file` 在宿主拒絕寫入時答 `_|_  ;; %cause: #unwritable`，不回位址（r4、r5）。允許的寫入仍是 `#true` 與位址（g2）。沒有宇宙的 `save` 仍是 `#no_universe`。

I4. 同儕目錄 append 在 `flush` 之後 `sync_all`，兩者的錯誤都返回。失敗句子是 `cannot write …: <reason>`。權限的 reason 是 `permission denied`。

I5. 提交後清除 `pin_pending`、`effect_pending`、`abandoned`、`staged` 與注入檔時，刪除錯誤會返回。`NotFound` 是已經不在。

I6. 讀得到的放棄記錄仍進提交並被清掉（g1）。Q-064、Q-063、Q-062、Q-061、Q-060 都在全樹裡。

I7. `x: 0` 根、新鮮 `v: 1 + 1` 根、標準根、`layout=8`／`encoding=5` 都沒動。沒有新的耐久檔名。

### 8.2 順手改動（逐項指名）

`BottomCause::Unwritable` 加在列舉尾巴，`NoUniverse` 之後。`primary_rank` 是 1。

`engine.save`：`put_value` 回 `cannot write` 時答 `#unwritable`。其他 `Err` 仍是 `#conflict`。`holds_universe` 為假時仍是 `#no_universe`，在 `put_value` 之前。

`io.write_file`、`io.append_file`：寫入的任何 `Err` 答 `#unwritable`。儲存邊界檢查仍在寫入之前，仍是 `#store_boundary`。`read_file`、`exists` 沒改。這兩個寫入的機制沒改，`append_file` 仍不 `sync_all`（I4 點名的是同儕目錄）。

`injections::paths` 改為 `read_dir`：`NotFound` 是空工作集，其他錯誤是 `cannot read injection …`。新增 `ensure_directory_writable`。`clear` 回傳第一個真正的刪除錯誤。

`peers::append` 加上 `sync_all`。`create_dir_all`、`writeln`、`flush` 的錯誤都返回。既有檔的 `metadata` 不是 `NotFound` 時先 `cannot read`，再開檔。`load` 的冷啟動沒改。對外仍是 `#rejected #cannot_record`。

`load_staged`：`pin_pending`、`effect_pending`、legacy `staged` 的 IO 錯誤具名拒絕。讀得到但解析不了的內容維持原讀法（空座標、忽略 sidecar、解析錯誤照舊上傳）。

`squash` 在 `put_commit` 之前先讀 `abandoned`。讀不到就不寫。

探針、規格、`TAG_REGISTRY`、版本都沒改。沒跑 rustfmt。沒動 `storage.rs` 的位元組。

改動檔：`crates/interpreter/src/value.rs`、`store_codec.rs`、`builtins/engine.rs`、`builtins/io.rs`、`injections.rs`、`peers.rs`、`universe.rs`、`crates/oo/src/main.rs`、本工單 §8。

### 8.3 工單哪裡是錯的

工單沒有寫錯。探針的宣稱沒改。沒有既有測試與 D83 衝突，舊測試沒有預先改。

### 8.4 工單指名要你回答的問題

Q1. 讀了再改寫或消耗的記錄，與讀不到時的回答：

1. `.oo/abandoned`。讀取是 `Universe::load_abandoned_file`（`read_to_string`）。呼叫者：`commit`（在 `put_root` 之前）、`rollback` 的 `append_abandoned_file`（在 `atomic_write` 之前）、`squash`（在 `put_commit` 之前）。`NotFound` 是空記錄。其他 IO 是 `cannot read <路徑>: <operator_io_reason>`，三個指令都不寫、不刪。消耗發生在讀成功之後，`remove_durable`：`NotFound` 視為已清，其他錯誤把 HEAD 寫回並返回該錯誤。

2. `.oo/injections/<id>`。讀取是 `injections::load_all`。目錄 `NotFound` 是空工作集。列目錄失敗是 `cannot read injection injections: <reason>`。單檔讀取失敗是 `cannot read injection <檔名>: <reason>`。列出之後才 `NotFound` 是 `working set consumed by a concurrent commit`。消耗是 `injections::clear`，只刪這次折進去的路徑。

3. `.oo/staged`（legacy，沒有注入成員時才讀）。`load_staged` 的 `read_to_string`。`NotFound` 是沒有這份暫存。其他 IO 是 `cannot read`。解析錯誤照舊上傳。消耗是 `remove_durable`，`commit` 與 `save_staged` 都會呼叫。`save_staged` 若刪不掉，連同剛寫下的注入一起撤回再返回錯誤。

4. `.oo/pin_pending`。`load_staged`。`NotFound` 是沒有 pin。讀得到但不是座標清單：pinned、座標未知、空集合。其他 IO 是 `cannot read`。消耗：`commit` 結尾 `remove_durable`；新的 evolve 在不是 legacy 殘留時也刪。

5. `.oo/effect_pending`。`load_staged`。`NotFound` 是沒有 sidecar。讀得到但解析不了：略過，閘門維持原讀法。其他 IO 是 `cannot read`。消耗：`commit` 結尾 `remove_durable`。layout 尚未目前時，`save_staged` 會整檔重寫或刪除。

6. `.oo/savepoints/<id>`。`savepoint::load_circles`。目錄 `NotFound` 是沒有 ○。單檔 `NotFound` 略過。其他 IO 是 `cannot read .oo/savepoints: <reason>`。寫的是新圓，不是把讀不到的正文覆寫掉。`commit`／`evolve` 在這個錯誤上不繼續寫。

7. `.oo/HEAD`。`get_head`。`NotFound` 是沒有 HEAD。其他 IO 是 `cannot read .oo/HEAD`。`commit`、`rollback`、`squash` 都先走這條，讀不到就不 `set_head`。

8. `.oo/peers/directory`。`append` 先 `metadata`：`NotFound` 才建檔並寫標頭；其他 IO 是 `cannot read`，不開檔。寫入、`flush`、`sync_all` 失敗是 `cannot write`。`peers::load` 仍把讀不到的快取當成冷啟動，這是既有探針釘住的觀測；它不覆寫那些位元組。隨後的 `append` 在檔案不可讀時停在 `metadata`。

物件目錄的讀取錯誤仍是 `cannot read store objects`（Q-063）。`gc` 的刪除走既有路徑，這次沒改。

Q2. 語言層向宿主寫入的內建只有三個：

1. `~%Io./write_file`。成功 `#true`。宿主 `Err` 是 ⊥ `#unwritable`。路徑越過儲存邊界是 `#store_boundary`，在寫入之前。引數不對是 ⊤。

2. `~%Io./append_file`。與 `write_file` 同一組回答。

3. `~%Engine./save`。別名 `~%Discovery./identify_and_store` 是同一個內建。沒有宇宙是 `#no_universe`。宇宙裡 `put_value` 成功回位址。錯誤字串以 `cannot write` 開頭是 `#unwritable`。其他 `Err` 是 `#conflict`。

`~%Io./read_file`、`~%Io./exists` 是觀測：缺檔是 `#none`／`#false`，權限是 ⊤ 纖維 `unreadable`。其餘內建模組（math、string、list、bytes、path、json、toml、csv、time、effect、stat、url、disc）不寫宿主。csv 只讀檔。

Q3. 選先確認再落地。

有折進去的注入時，`commit` 在 `put_root` 之前呼叫 `ensure_directory_writable`：對目錄 `metadata`，`NotFound` 視為尚無目錄；否則 `create_new` 一個 `.partial-writable-<pid>`，再刪掉。點開頭的名字 `paths` 不折。開檔失敗是 `cannot write <目錄>: <reason>`，HEAD 不動。

確認與 `clear` 之間若權限又變了：`set_head` 與 `record_commit` 已經做完，`clear`（以及其後的 `staged`／`pin_pending`／`effect_pending`／`abandoned` 刪除）返回錯誤。此時把 HEAD 寫回這次 `set_head` 之前的摘要；原本沒有 HEAD 就刪掉 HEAD 檔。函式返回刪除錯誤，CLI 不印 `Commit successful`。那一枚 ○ 已經在盤上，指向沒有留在 HEAD 的提交。HEAD 自己也寫不回去時，句子是 `<刪除錯誤>; HEAD stayed at <新摘要>: <還原錯誤>`。

`record_commit` 自己失敗時，HEAD 留在新提交，注入還沒刪，錯誤原樣返回。這段這次沒改。`~%Config` 的回寫若在注入已刪之後失敗，HEAD 也留在新提交。這是既有的 O37 路徑。

Q4. M6 各處：

1. `injections::clear` 的 `let _ = remove_file` 改成：`NotFound` 略過，其他錯誤 `cannot write <路徑>: <reason>`，並中止。

2. `commit` 的 `pin_pending`、`effect_pending`、`abandoned` 三處 `let _ =` 改成 `remove_durable`。失敗則還原 HEAD 並返回。

3. `unlink_legacy_staged`（`.oo/staged`）改成 `remove_durable`。`commit` 裡失敗同樣還原 HEAD。

4. `save_staged` 在 ○ 寫失敗時刪剛寫下的注入：刪除再失敗時，返回的錯誤同時帶著 ○ 的錯誤與 `the injection remains`。

5. `save_staged` 刪 legacy `pin_pending`、非目前 layout 的 `effect_pending`：改成 `remove_durable`，錯誤向上返回。若此時注入已寫下而 `staged` 刪不掉，先刪掉該注入再返回。

留下的 `let _ =`：

- `savepoint.rs` 鑄新圓之前刪外來名字 `LOG`。那不是這次讀過的記錄。`paths` 跳過這個名字。刪不掉時下一枚圓仍用隨機 id。這次沒改。

- `storage.rs` `remove_digest` 在清空分片目錄時 `remove_dir`。目錄不空或已經不在時失敗，下一趟走訪看得到物件。這不是 M6 的提交清除。檔沒改。

- `scratch.rs` 清的是行程暫存目錄。

- `peers::compact` 的 `create_dir_all`。接下來的 `atomic_write` 失敗會讓 compact 回 `None`，append 記 `compact failed`。這不是 M6。

Q5. `primary_rank` 是 1，與 `StoreBoundary` 同一群。宿主拒絕寫入是儲存邊界上的失敗。rank 2 是 `#conflict` 那一群。同一個 union 裡較低的 rank 先報，所以這條不會被收成 `#conflict`。

離開碼類別：載體是 ⊥ 這個值。`TAG_REGISTRY` §0.2 載體為值本身時離開碼是 0。〔量〕`eval` 對不可寫目錄呼叫 `write_file`：rc=0，stdout `_|_  ;; %cause: #unwritable`。

`write_file` 失敗時，答覆的文字就是這一行。它沒有再寫半截檔留下或沒留下。`std::fs::write` 本身不是原子的。這次量到的情況是目錄不可寫，路徑上沒有留下檔案。

### 8.5 探針

本弧探針檔未改、未 rustfmt。無 `VOID READING`。8 支皆綠（g1–g3、r1–r5）。

### 8.6 數字

三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`，三輪相同：`test result:` 250 行，2399 passed，0 failed，`^error` 0，exit 0。沒有失敗測試名。各行的耗時不同；把 `finished in` 換成同一記號之後，三輪的 `test result:` 行相同。

conformance：162 vectors，162 pass，0 fail。

`~%Math./add (1, 2)` → `3`，`(1, 3)` → `4`。`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。新鮮倉 `v: 1 + 1` 根 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。標準根物件 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`。新倉 `layout=8`／`encoding=5`。

### 8.7 你認為需要改規格之處

`#unwritable` 還沒有寫進 `TAG_REGISTRY`。`REAL_01` §4.1.1、`SPEC_08` §6.2 的文字仍是驗收方收尾，此處未改。

EINTR 與暫時／永久之分、`run` 的說明句，工單列為 Inbox，此處未裁。

---

## 9. 驗收（驗收方填）

### 第一輪驗收（2026-09-30）：**不受理，開修補回合 R-1（一項；探針比不變式窄，一半在驗收方）**

交付 `f4c876f`：探針 8／8；diff 純度成立（探針、分隔線以上未動）。**交付做對的**〔量，9 種記錄 × 4 指令，逐格比 `.oo/` 全檔與權限，並與 v0.66.0 逐格 diff〕：
`pin_pending`／`effect_pending` 讀不到時，v0.66.0 的 `evolve`／`commit`／`rollback` 會寫入或照答，現在具名拒絕、不寫；放棄記錄、注入目錄與成員、`HEAD` 讀不到皆具名拒絕、不寫。
`save` 的 `#unwritable` 以錯誤句開頭 `cannot write` 分派——〔讀〕`put_value` 所有宿主路徑（`create_dir_all`、`atomic_write` 的五處）都經 `cannot_write`；〔量〕已存在的分片目錄不可寫 ⟹ `#unwritable`。**以句子分派是脆的**，r4 釘住它。

**R-1（一項，兩個入口）：回報失敗的提交已經落地。**
〔量，f4c876f 與 v0.66.0 皆然〕`.oo/savepoints` **讀不到或寫不進** ⟹ `commit` rc=1「cannot read/write .oo/savepoints」，**而 `HEAD` 已經移到新提交**——那筆提交沒有 ○，祖先邊斷了：`log` 只剩一筆，**下一次 `gc` 刪掉上一筆提交與它的根（5 → 3）**。
`squash` 同形（寫 ○ 在 `set_head` 之後）；`refine` 同形〔讀〕。另：`.oo/savepoints` 讀不到時 `evolve` rc=1，**剛寫下的注入留在工作集**（位置比對那一步以 `?` 返回，沒有撤回；緊接的 `record` 失敗路徑有撤回）。
**成因的分配**：工單 I1 寫「凡要讀的耐久記錄讀不到 ⟹ 不寫」、I2 寫「回報成功的提交已消耗……做不到就在落地之前拒絕」——**不變式涵蓋這一格，探針沒有**（r1–r5 只量了放棄記錄與注入目錄）。交付 §8.4 Q1 第 6 項寫「`commit`／`evolve` 在這個錯誤上不繼續寫」——**量出來不成立**；Q3 自陳「`record_commit` 自己失敗時，HEAD 留在新提交……這段這次沒改」——**那一句正是這一格**。
**不變式（補寫，I2 的另一半）**：**一次回報失敗的操作沒有移動 `HEAD`**；凡在 `set_head` 之後才做的耐久寫入（提交 ○、清除），失敗時 `HEAD` 回到之前——`commit`、`squash`、`refine` 三處。**一次回報失敗的 `evolve` 沒有留下提議**。
**新探針 r6–r9**（驗收方加）：`commit` × savepoints 讀不到／寫不進、`evolve` × savepoints 讀不到、`squash` × savepoints 寫不進；皆要求「rc≠0 ⟹ `HEAD` 未動」並在之後跑 `gc` 確認上一筆提交仍在。
〔兩極校準〕f4c876f 上 **r6–r9 四紅、r1–r5 仍綠**；一個最小修正（三處 `record_commit` 失敗即還原 `HEAD`、位置比對失敗即撤回注入）⟹ **12／12**；同一修正跑全樹 **250 target／2403 passed／0 failed，exit 0** ⟹ 修補不牽動既有測試。交付的全樹 ×3（f4c876f）：**250／2399／0／exit 0**，三輪相同。
