# 工單：由 HEAD 決定的落地（Q-066）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-066／裁定 `meta/oo/STATUS.md` **D84**（丁）／設計筆記 `meta/oo/commit.md` §1.12（HEAD 是 `.oo/` 裡唯一不單調的暫存器）
> 既有 MUST：`SPEC_10` §4.1（`#commit` 強原子性）、`REAL_01` §4.6（Core Requirement：「要麼完全成功，要麼完全不寫入」）、`SPEC_08` §6.2 R1（放棄的整段確實發生過）
> 探針（已預先提交並校準）`crates/oo/tests/a_landing_that_head_decides_probe_test.rs`
> 基線：dev `faa09d0`／`oo v0.67.0` ⟹ **4 綠 6 紅**，三輪一致、零空洞讀數。

## 1. 缺陷

會移動 `HEAD` 的操作是多步的：`commit`（物件、`HEAD`、提交 ○、清注入、清放棄記錄與意圖）、`squash`、`refine`、`rollback`（放棄記錄、`HEAD`）。崩潰可停在任兩步之間。〔量，v0.67.0，以「正常操作後放回崩潰會留下的檔」重構〕：

| | 崩潰停在 | 今天 |
| :-- | :-- | :-- |
| **S2** | `HEAD` 已移、提交 ○ 未寫 | 祖先邊斷（`log` 剩 1 筆），**`gc` 刪掉上一筆提交**；已提交的提議回到工作集 |
| **S3** | ○ 已寫、注入未清（Inbox 第 99 列） | `status` 把已提交的 `x: 2` 列為待提交；**無關的提交被要求 `--grant pin`，給了之後 `log` 在它上面印 `pin`** |
| **S5** | 注入已清、放棄記錄未刪 | 下一筆提交**把同一次放棄再記一次** |
| **S6** | `rollback`：放棄記錄已寫、`HEAD` 未移 | 下一筆提交**宣稱放棄了自己的上一筆** |
| **P** | 提交 ○ 已寫、`HEAD` 未移（D84 要的順序） | **今天就正確**：提議仍在、下一次提交折入、`gc` 回收未落地的提交、歷史完整（g1） |

〔讀〕`commit` 的順序今天是 `put_commit → set_head → record_commit → clear`；`rollback` 是 `append_abandoned → set_head`。

## 2. 裁定與依據

*   **D84（用戶，丁）**：**`HEAD` 是唯一的提交點。** 移動它之前，落地所需的一切（**含提交 ○**）寫到最終位置；移動之後只是回收；**讀取一律以 `HEAD` 推導**：
    **(i)** 內容已在 `HEAD` 位置的注入（與 `HEAD` 的根合起來位置不變，D80 ② 的同一個判準）**不是提議**；
    **(ii)** 已記在 `HEAD` 歷史裡某筆提交放棄欄的放棄**不再記**；
    **(iii)** 指向 `HEAD` 本身或其祖先的「放棄」**無效**。
*   **用戶接受的代價**：與已提交內容完全相同的新注入（含帶 pin 者）是空操作，不是提議。
*   **不做日誌**：原建議的意圖日誌（照 `REAL_01` §4.6 的 WAL）被否決——本專案兩次拒絕第二份真相（`savepoint.rs` 拿掉 `LOG`、D52 否決事件日誌）。**不得新增任何「進行中操作」的記錄檔**。
*   〔偵察〕三條推導與順序變更**皆不改磁碟格式** ⟹ **不推進佈局、非破壞性**。

## 3. 射程＝不變式（不是機制）

*   **I1** **會移動 `HEAD` 的操作，在 `HEAD` 移動之前，已把之後的讀者需要的一切寫到最終位置**：`commit`／`squash`／`refine` 的提交 ○ 在 `HEAD` 之前。
    崩潰停在 `HEAD` 移動之前 ⟹ 那是一次**沒發生的操作**（P，g1）；停在之後 ⟹ 那是一次**發生了的操作**，而 I2–I4 使殘留物不改變任何答案。**探針無法造崩潰**，驗收方以 strace 驗順序。
*   **I2** **`HEAD` 已持有的注入不是提議**——`status` 不列、提交不折、**不帶來 pin 或 discharge 的要求或標記**（r1–r4）。**所有讀工作集的入口一致**。
*   **I3** **一次放棄只記一次**：已記在 `HEAD` 歷史裡的放棄不再記（r5）。**`HEAD` 或其祖先不是被放棄的**（r6）。`commit`、`squash`（它也讀放棄記錄）一致。
*   **I4** **原本做得到的仍做得到**：改變了東西的 pin 仍需 `--grant pin` 且在歷史標 `pin`（g2）；一次 rollback 之後的提交記一次放棄（g3）；新的提議仍是提議（g4）；`~%Config` 的會期注入不因本規則消失；**所有既有探針**。
*   **I5** 不新增耐久檔、不改磁碟格式、不推進佈局；值位址與提交位址不動。

## 4. 紅線（今天綠，必須保持綠）

*   g1–g4；Q-065 12、Q-064 13、Q-063 13、Q-062 14、Q-061 10、Q-060 23。
*   conformance 162／162；`x: 0` 根 `31745ef0…`、`v: 1 + 1` 根 `f4f32e7b…`、標準根 `7038e250…`；新倉 `layout=8`／`encoding=5`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** 每一個會移動 `HEAD` 的操作，改後的耐久步驟順序逐步列出；每一個「移動之後」的步驟崩潰時，I2／I3 哪一條使它無害。
*   **Q2** I2 的判準你放在哪裡（載入工作集時？提交時？）；`evolve` 自己剛寫下一個與 `HEAD` 相同的注入時答什麼；被判為「已持有」的注入檔你刪不刪、何時刪。
*   **Q3** I3 的歷史走訪：走多遠、代價（量一個數字：一千筆提交的倉，一次 rollback 之後的提交多花多少時間）。
*   **Q4** `rollback` 的順序你改了沒有；若沒改，S6 靠哪一條無害。
*   **Q5** 事實陳述：舊引擎在舊順序下留下的 S2 殘留（`HEAD` 指向沒有提交 ○ 的提交），本交付之後的引擎遇到它答什麼。**不要擴大射程**（D84 明列射程外）。

## 6. 明文不在射程內

*   S2 的**舊殘留**（D84 記帳）；S1（首次提交崩潰，D81 已揭露）。
*   `REAL_01` §4.6 的文字重寫（它描述的 `refs/HEAD`、主索引、Blake3 皆不存在）：驗收方收尾。
*   legacy `.oo/staged` 不讀即刪（Inbox 設計題）。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r6 基線必須是紅的，理由逐支寫在 doc comment；**你可以讓它們變綠，不得改寫它們的宣稱。**
*   **崩潰以「正常操作之後放回崩潰會留下的檔」重構**；探針用到的儀器（CLI 輸出、提交物件的磁碟形、注入檔的磁碟形）不准改。`rustfmt` 不得掃探針檔。
*   〔驗收方校準〕**兩極皆做**：基線 4 綠 6 紅（三輪一致）；一個最小參考實作（載入工作集時濾掉與 `HEAD` 根合起來位置不變的注入；提交讀放棄記錄時剔除 `HEAD` 歷史已記者與 `HEAD`／祖先本身）⟹ 本探針 10／10。
    **參考實作自己錯過一次**：用 `get_commit` 讀值定址的提交 ⟹ `#caid_mismatch`；應用 `open_commit`。
    同一參考實作跑全樹：**251 target／2412 passed／1 failed**——`pin_probe_test::pin_intent_file_is_not_authority`。
    它的「無辜提交」是 `y: @int` 疊在已提交的 `y: 5` 上：**不移動位置**，依 D84 (i) 是空操作，提交答「Nothing to commit」而不是 `#privileged_required`——
    **這是用戶接受的代價的一個實例**（已被滿足的型別約束同樣不移動位置），而該測試守的安全性質（偽造的意圖檔不是授權）不受影響。
    **驗收方預先修訂**：無辜提交改為 `z: 1`（真的移動位置），標 `AMENDED 2026-10-01 for Q-066 (D84)`；**兩邊驗證**：v0.67.0 基線 15／15、參考實作 15／15。**你不得再改它。**

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

I1. `commit`、`squash`、`refine` 都在 `set_head` 之前把提交 ○ 寫到最終位置。`record_commit` 失敗時 HEAD 尚未移動，錯誤原樣返回。`set_head` 失敗時 HEAD 檔仍指移動前的那一筆，○ 已在盤上，這是狀態 P。

I2. 載入工作集時逐個注入判斷。HEAD 的根與該注入的合，等於 HEAD 的根與空工作集的合，這份注入就不是提議：不折、不帶 pin、不帶 discharge。`status` 不列它，提交不折它。

I3. `commit` 在 `put_root` 之前讀放棄檔。空檔不走歷史。一條筆記的摘要落在 HEAD 到起點的路徑上，或它的全文／摘要已經記在路徑上某筆的 `abandoned`，就不再記入。`squash` 呼叫同一個函式，結果不寫進 squash 提交。

I4. 會移動位置的 pin 仍要 `--grant pin`，歷史仍標 `pin`。一次 rollback 之後的提交記一次放棄。新的提議仍是提議。`~%Config` 會改變合的位置，留在工作集。本探針 10／10，Q-065 12／12，pin 15／15，全樹 2413 passed。

I5. 沒有新的耐久檔。新倉仍是 `layout=8`、`encoding=5`。提交 ○ 的 `point:` 仍是這一筆的 64-hex。三個根位址沒動。

### 8.2 順手改動（逐項指名）

`record_commit` 在這個佈局要寫 `point:` 時，寫的是這筆提交自己的摘要。圈改在 `set_head` 之前落盤，讀當時的 HEAD 會記成上一筆。沒有 `.oo/format`、或佈局不記 point 的倉，這一行仍省略。evolve 的圈仍讀當下的 HEAD。

`previous_commit_in`：一次歷史走訪把圈目錄讀一次。`previous_commit` 的答案與原先相同。

`Universe` 增加程序內的 `held_sources`，只記「已在 HEAD 位置」的注入路徑。落地成功之後，`set_head` 之後刪掉。它不是磁碟上的清單。

改動檔：`crates/interpreter/src/universe.rs`、`crates/interpreter/src/savepoint.rs`、本工單 §8。

探針、驗收方預先改過的 `pin_intent_file_is_not_authority`、版本、規格、`TAG_REGISTRY` 都維持原檔。沒有跑 rustfmt。沒有改 `storage.rs`。

### 8.3 工單哪裡是錯的

工單的判準與探針一致。走訪用 `open_commit`。預先改成 `z: 1` 的 pin 測試是 15／15，沒有再改。

### 8.4 工單指名要你回答的問題

Q1. 會移動 HEAD 的操作，改後的耐久順序：

1. `commit`。移動之前：讀並過濾放棄檔（空檔不走）、有折進去或已持有的注入時先確認目錄可寫、`put_root`、`put_commit`、`record_commit`。然後 `set_head`。移動之後：刪已持有的注入、刪折進去的注入、刪 legacy `staged`；若有 `~%Config`，再寫回一筆只含它的注入；然後刪 `pin_pending`、`effect_pending`、`abandoned`。這三步的失敗會把 HEAD 寫回移動之前。
    注入檔留在移動之後：I2，內容已在新 HEAD 的位置，下一輪不把它當提議。放棄檔留在移動之後：I3，那條筆記已經在這一筆（現在的 HEAD）裡。`~%Config` 若在「刪除折進去的成員」與「寫回」之間停住，那筆會期注入已經刪掉、尚未寫回；I2／I3 不覆蓋這個縫，這是原先先清再寫回的窗口。沒有注入成員時，留著的 legacy `staged` 下一次載入仍會讀；工單把這條放在 Inbox。

2. `squash`。移動之前：用同一個過濾函式讀放棄檔並丟掉結果、`put_commit`、`record_commit`。然後 `set_head`。移動之後：刪放棄檔。檔留著時，下一筆 `commit` 靠 I3 不再記已經記過的、以及指向 HEAD 或祖先的筆記。squash 提交本身仍不記入這些筆記。

3. `refine`。移動之前：`put_commit`、`record_commit`。然後 `set_head`。移動之後沒有耐久的回收步驟（refine map 在記憶體）。停在 `set_head` 之後就是這次 refine 已經發生。

4. `rollback` 見 Q4。它的放棄記錄寫在 `set_head` 之前。

Q2. 判準在 `load_staged`（`proposals_at`），在 pin、discharge、fold 之前，每個注入各算一次。所有經 `load_universe` 讀工作集的入口走這裡。

`evolve` 剛寫下一個與 HEAD 位置相同的注入時，命令成功返回，檔留在 `injections/`。下一次載入不把它當提議。

載入不刪這些檔。`Nothing to commit` 不刪。一次真正落地的 `commit` 在 `set_head` 之後刪掉 `held_sources` 上的路徑。

Q3. 走訪從 `get_head` 起，`open_commit` 讀每一筆；前一筆是 `parent`（有的話），否則是圈上的 `ancestor`。走到沒有前一筆為止，用摘要集合切環。圈目錄在這一次走訪裡讀一次。放棄檔不存在或是空的：不讀圈、不走。

量到的數字：先建成 1000 筆互不衝突的欄位提交。計時時鏈上是 1002 筆。沒有放棄檔的一筆提交 95 ms。退回上一步再提交 1.80 s。多 1706 ms。

Q4. `rollback` 的順序沒改：先 `append_abandoned`，再 `set_head`。S6（放棄記錄已寫、HEAD 未移）靠 I3（iii）：那條筆記點名的是仍在 HEAD 的這一筆，下一筆提交不記入。

Q5. 舊順序留下的 S2：HEAD 指向一筆提交物件，那筆的提交 ○ 不在，當時還沒清掉的注入還在。本引擎遇到這個殘留時：

- `status` rc=0，印 `Universe is static`。那份注入的內容已在 HEAD 的根上，不列為提議。
- `log` rc=0，只剩 HEAD 這一筆。祖先那一筆不在這條邊上。
- `gc --grant gc`：5 個物件、3 個可達、刪掉 2 個。上一筆不在了。`log` 仍是一筆。
- 其後的新提交成功。`log` 是新的一筆，然後這筆沒有 ○ 的尖端。缺掉的 ○ 沒有補上。被 gc 收掉的上一筆不會回來。

### 8.5 探針

`a_landing_that_head_decides_probe_test`：10 passed、0 failed，1.63 s，無空洞讀數。

`what_did_not_land_probe_test`：12 passed、0 failed。

`pin_probe_test`：15 passed、0 failed，含 `pin_intent_file_is_not_authority`。

### 8.6 數字

三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`：每輪 251 行 `test result:`、2413 passed、0 failed、`^error` 0 行、exit 0。去掉 `finished in` 之後三輪逐行相同。沒有失敗測試名。

conformance：162 vectors、162 pass、0 fail。

`~%Math./add (1, 2)` → `3`，rc=0。`(1, 3)` → `4`，rc=0。這兩次 eval 的目錄沒有 `.oo/`。

`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。新鮮倉 `v: 1 + 1` 根 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`。新倉 `layout=8`，`objects.format` 為 `encoding=5`。

### 8.7 你認為需要改規格之處

這份交付不改規格條文。磁碟格式與佈局宣告沒動。`REAL_01` §4.6 的文字重寫留在驗收收尾。

---

## 9. 驗收（驗收方填）

**受理，零修補回合。** 交付 `62993b7`：全樹 ×3 **251 target／2413 passed／0 failed，`^error` 0，exit 0**，三輪相同，無失敗測試名；本弧探針 10／10，無 `VOID READING`；Q-065 12、Q-064 13、`pin_probe_test` 15（含預先修訂的那支）全綠。探針、修訂檔、分隔線以上皆未動；改動只在 `savepoint.rs`、`universe.rs`。
**I1（探針造不出崩潰）**〔strace，rename 順序〕`commit`、`squash`、`refine` 皆為 **savepoints → HEAD**；對照組 v0.67.0 的 `commit` 為 HEAD → savepoints。
**Q3 的代價**〔驗收方另量〕300 筆提交的倉：一般提交 52 ms、rollback 之後的提交 505 ms（約每筆歷史 1.5 ms，線性），與交付的 1000 筆 +1706 ms 一致。只在放棄紀錄存在時付——**記入 Inbox（效能）**。
**交付自陳的兩處（記帳）**：`~%Config` 在「刪除折入的成員」與「寫回」之間崩潰會遺失會期設定（原本就有的窗，I2／I3 不涵蓋）；舊順序留下的 S2 殘留照舊使 `gc` 刪掉上一筆（D84 射程外，Inbox 已有）。
身分：`31745ef0…`／`f4f32e7b…`（`1 + 1` 與 `1+1`）／標準根 `7038e250…`；known-answer 3／4；conformance 162／162；新倉 `layout=8`／`encoding=5`。
