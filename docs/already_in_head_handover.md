# 工單：已在 HEAD 裡（Q-068）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-068／裁定 `meta/oo/STATUS.md` **D86**（乙）／同族 D61（晚到者的工作集被消費）、D63（離開碼由載體決定）、D84 (i)（`HEAD` 已持有的注入不是提議）
> 既有 MUST：`SPEC_10` §4.1.3（必須具名回報被消費；不得說成本來就沒有；**誠實的空工作集保留原答覆，兩者必須可分辨**；自陳缺口：未說出內容已落地）
> 探針（已預先提交並校準）`crates/oo/tests/already_in_head_probe_test.rs`
> 基線：dev `ac8714e`／`oo v0.69.0` ⟹ **4 綠 3 紅**，三輪一致；三支紅的都紅在預定斷言（點名 `HEAD`／點名 `HEAD`／兩種答案相同），零空洞讀數。

## 1. 缺陷

一次提交可能發現它被給的東西已經在歷史裡了，兩種途徑〔量，v0.69.0；並行以探針持有提交鎖做成決定性〕：

| | 情形 | 今天 |
| :-- | :-- | :-- |
| **C1** | 兩個提交等同一把鎖、同一份工作集；一個落地，另一個醒來發現工作集被拿走 | rc=1 `working set consumed by a concurrent commit`——**真但不完備**：沒說內容已在 `HEAD` 裡（§4.1.3 自陳缺口） |
| **C2** | 重新 evolve 一份 `HEAD` 已有的 `x: 1`（D84 (i)：不是提議），然後提交 | rc=1 `Nothing to commit`——**與從未有東西的工作區同一句**，§4.1.3 要求兩者可分辨 |
| 對照 | 提交後再提交（誠實的空）；`HEAD` 之上 evolve `v: _`；只有 `~%Config` | rc=1 `Nothing to commit` |
| 對照 | 等鎖時成員被手動刪掉（沒落地） | rc=1 `working set consumed …` |

## 2. 裁定與依據

*   **D86（用戶，乙）**：**仍是拒絕**（rc≠0；D63：邊界的載體）；**C1 與 C2 回同一句話，並點名持有那份內容的 `HEAD`**。
*   **只在驗證過時說**：`HEAD` 確實持有那份內容（D84 (i) 的同一個判準：與 `HEAD` 的根合起來位置不變）才說；驗證不到就不得這樣說。
*   **誠實的空不變**（§4.1.3 末款）：從未有東西的工作區、**沒有可提交內容的工作集**（`v: _`、只有 `~%Config`）——後兩者 D84 (i) 也判為「已持有」，那是平凡的持有，**不得**因本裁定改答。
*   否決甲（rc=0）、丙（只收斂文字不點名）。

## 3. 射程＝不變式（不是機制）

*   **I1** **C1**：等鎖的提交，若它等鎖前看到的工作集裡有可提交的內容，而那些內容此刻全在 `HEAD` 裡 ⟹ 回 D86 那句話、點名 `HEAD`（r1）。
*   **I2** **C2**：工作集裡有 `HEAD` 已持有、且有可提交內容的注入，而沒有任何提議 ⟹ 同一句話、點名 `HEAD`；rc≠0；**不寫任何東西**；與誠實的空可分辨（r2）。
*   **I3** **一句話**：C1 與 C2 的輸出，各自把自己的 `HEAD` 換成佔位符之後**相同**（r3）。「點名」＝ `HEAD` 的 64-hex 摘要出現在輸出裡。
*   **I4** **不說沒驗證過的話**：拿走了卻不在 `HEAD` 裡的（g2）、等鎖前讀不到內容的——不得回那句話；照原答。
*   **I5** **原本對的仍對**：誠實的空、`v: _`、只有 `~%Config` 的答覆逐字不變（g1、既有 `what_it_says_is_what_happened::g1／r10`）；已持有旁邊有新提議 ⟹ 照常提交（g3）；並行的勝者照常落地（g4）；**所有既有探針**。
*   **I6** 不新增耐久檔、不改磁碟格式、不推進佈局；值位址與提交位址不動。

## 4. 紅線（今天綠，必須保持綠）

*   g1–g4；`what_it_says_is_what_happened_probe_test`（含 g1、r10）、`a_commit_that_ate_what_it_never_read_probe_test`、Q-067 11、Q-066 10、Q-065 12。
*   conformance 162／162；`x: 0` 根 `31745ef0…`、`v: 1 + 1` 根 `f4f32e7b…`、標準根 `7038e250…`；新倉 `layout=8`／`encoding=5`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** C1 你怎麼知道「等鎖前看到的工作集」是什麼（讀了什麼、何時讀）；在列出與讀取之間成員消失時答什麼。
*   **Q2** 「點名」印的是完整 CAID 還是摘要；`HEAD` 讀不到時（權限）C1／C2 各答什麼。
*   **Q3** 事實陳述：C1 的勝者若是 `squash` 或 `refine`（不是 `commit`），輸家今天答什麼。不要為此擴大射程。
*   **Q4** 事實陳述：C2 之後的 `status` 答什麼；它跟這句話是否一致。不要改 `status`。

## 6. 明文不在射程內

*   `status` 的措辭；`evolve` 對已持有內容的回答。
*   rc 的值（只要求 ≠0）。
*   CLI 的確切拼法（D64：行為規範、拼法不規範）。
*   規格文字（`SPEC_10` §4.1.3 自陳缺口改寫）：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r3 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。**
*   **C1 以探針持有提交鎖做成決定性**：探針對 `.oo/format` 取與引擎同一種獨佔檔案鎖，啟動兩個提交，等 1.5 秒（兩者都已列出工作集並阻塞），再放鎖。**不要改鎖的位置或種類**（那是 Q-016b 的佈局決定）；若你的實作改變了「列出工作集」與「取鎖」的先後，r1 的 `VOID READING` 會開火——那是探針在說它量不到，請在報告裡說。
*   探針用到的儀器（CLI 輸出、`.oo/` 的檔、`inspect`）不准改。`rustfmt` 不得掃探針檔。
*   〔驗收方校準〕**兩極皆做**：
    基線 4 綠 3 紅（三輪一致）。
    一個最小參考實作（`proposals_at` 記下「被判為已持有的成員裡有可提交內容」；提交在取鎖前讀一次成員，取鎖後若被拿走，以同一判準問 `HEAD` 是否持有它們）⟹ 本探針 7／7。
    **三個變異各自讓一支守衛轉紅**：不看可提交內容 ⟹ g1；被拿走就一律說已落地 ⟹ g2；有已持有成員就拒絕 ⟹ g3。g4 是 r1 場景的對照（勝者確實落地），不是守衛。
    同一參考實作跑全樹：**253 target／2429 passed／2 failed**——兩支都在 `a_landing_that_head_decides_probe_test`（Q-066）：`r1_a_folded_injection_left_behind_is_not_a_proposal`、`r4_an_injection_identical_to_head_is_not_a_proposal`。
    **兩支都正是 C2 的實例**（殘留的已持有注入；帶 pin 的相同注入），用 `Nothing to commit` 這幾個字當「不是提議」的儀器——**D86 刻意改的正是這句回答**，不是參考實作越過裁定。
    **驗收方預先修訂**：改量「rc≠0 且 `HEAD` 沒移動」（HEAD 一移動就紅，仍量得到它原本防的缺陷），標 `AMENDED 2026-10-03 for Q-068 (D86)`；**兩邊驗證**：v0.69.0 基線 10／10、參考實作 10／10。**你不得再改它。**
    其餘以 `Nothing to commit` 為儀器的既有測試（`what_did_not_land`、`limit_you_cannot_choose`、`where_the_conflict_is`、`a_commit_that_ate_what_it_never_read`）在參考實作上全綠——它們量的都是誠實的空。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

I1. 等鎖前讀到的成員裡有可提交內容，而且那些內容此刻都在 `HEAD` 裡：rc=1，一句 `Error: already in HEAD <HEAD 檔裡的完整 CAID>`。r1。

I2. 工作集裡有 `HEAD` 已持有、且有可提交內容的注入，沒有提議：同一句。rc=1。`.oo/` 不寫。與 `Error: Nothing to commit` 可分辨。r2。

I3. C1 與 C2 把各自的 `HEAD` 換成佔位符之後，輸出相同。r3。

I4. 成員被拿走、內容不在 `HEAD` 裡：`Error: working set consumed by a concurrent commit`。g2。等鎖前讀不到成員：權限拒絕是 `Error: cannot read injection <檔名>: permission denied`。列出與讀取之間成員消失：同一句 consumed，`HEAD` 留在原處。

I5. 誠實的空、`v: _`、只有 `~%Config`，仍是 `Error: Nothing to commit`。g1；`what_it_says_is_what_happened` 的 g1、r10。已持有旁邊有新提議，照常提交。g3。並行的勝者照常落地。g4。Q-067 11、Q-066 10、Q-065 12、`a_commit_that_ate_what_it_never_read` 6，皆 0 failed。

I6. 沒有新的耐久檔，沒有改磁碟格式，沒有推進佈局。值位址與提交位址沒動。版本仍是 `oo v0.69.0`。

### 8.2 順手改動（逐項指名）

`proposals_at` 記下已持有、而且自己有可提交內容的成員。`v: _` 與只有 `~%Config` 的成員留在已持有名單，不進這份。

`commit` 在取鎖前先 `paths` 再 `load_all`。取鎖之後，成員還在就用 D84 (i) 的同一判準對當下的 `HEAD` 再核對一次。被拿走、而且核對得過，回同一句。

改動檔：`crates/interpreter/src/universe.rs`、`crates/oo/src/main.rs`、本工單 §8。

探針、Q-066 預先修訂的兩支、版本、規格、`TAG_REGISTRY` 維持原檔。沒有跑 rustfmt。

### 8.3 工單哪裡是錯的

工單的判準與探針一致。`squash` 與 `refine` 不拿 `.oo/format` 這把鎖，也不刪注入。見 Q3。

### 8.4 工單指名要你回答的問題

Q1. 等鎖前看到的工作集是取鎖之前的兩次讀：`injections::paths` 列出 `.oo/injections`，接著 `load_all` 讀每個成員。有可提交內容的成員（與空 combo 合一之後，去掉 `~%Config` 仍有內容）留在這個行程裡。

列出與讀取之間成員消失時，`load_all` 回 consumed。這次讀標記為未核對，然後仍去取鎖。鎖後成員已不在：rc=1，`Error: working set consumed by a concurrent commit`。量到的那一次，注入目錄被清空，`HEAD` 仍是取鎖前那一筆 `f0ce3cb9610360bf986ed75dd6222761a36c64fa43f7938eec56a07cf2e9a97b`。

Q2. 點名印的是 `HEAD` 檔裡的完整 CAID（`hash:sha256:v2:…:<64-hex>`）。64-hex 在這串裡面。

`HEAD` 檔權限 000：

1. C2：rc=1。`Error: cannot read .oo/HEAD: permission denied`。恢復權限後 `HEAD` 仍是提交 `x: 1` 的那一筆。
2. C1，等鎖期間把 `HEAD` 改成 000，注入刪掉或留著，兩次都是同一句：rc=1，`Error: cannot read .oo/HEAD: permission denied`。

Q3. `squash` 與 `refine` 不刪工作集。有一份尚未落地的 `x: 1` 時：

1. `squash` rc=1。`Error: dirty worktree: commit or discard staged changes before squash`。等鎖的 `commit` 隨後 rc=0，`Commit successful:`。
2. `refine --source <根> --target <根> -m r` rc=0。`Refine commit:`，下一行 `Refine authority: unverified`。等鎖的 `commit` 隨後 rc=0，`Commit successful:`。

工作集已是持有（重 evolve 已提交的 `a: 1`）時，`squash` 與 `refine` 各自落地。等鎖的 `commit` rc=1，`Error: already in HEAD <剛落地的那筆完整 CAID>`。注入檔還在，判準對上的是新的 `HEAD`。這兩個命令的路徑沒有改。

Q4. C2 之後 `status` rc=0。兩行：

`Standard root dependency: 7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911 (available)`

`Universe is static (no staged changes).`

與那句話一致：沒有新的提議。`status` 沒有改。

### 8.5 探針

`already_in_head_probe_test`：7 passed、0 failed，7.19 s。無空洞讀數。

`what_it_says_is_what_happened_probe_test` 23、`a_commit_that_ate_what_it_never_read_probe_test` 6、Q-067 11、Q-066 10、Q-065 12，皆 0 failed。

### 8.6 數字

三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`：每輪 253 行 `test result:`、2431 passed、0 failed、`^error` 0 行、exit 0。去掉 `finished in` 之後三輪 `cmp` 相同。沒有失敗測試名。

conformance：162 vectors、162 pass、0 fail。

`~%Math./add (1, 2)` 是 3，`(1, 3)` 是 4，rc=0。這兩次 eval 沒有建立 `.oo/`。

`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。`v: 1 + 1` 根 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`。新倉 `layout=8`、`encoding=5`。

### 8.7 你認為需要改規格之處

沒有改規格文字。`SPEC_10` §4.1.3 自陳缺口仍由驗收方收尾。

---

## 9. 驗收（驗收方填）
