# 工單：一次一個寫者（Q-069）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-069／裁定 `meta/oo/STATUS.md` **D87**（甲）／同族 D61（Q-016b 的提交臨界區）、D84（`HEAD` 是唯一的提交點）
> 既有 MUST：`SPEC_10` §4.1（`#commit` 強原子性、晚到者收斂）；`SPEC_08` §6.2.1（`#gc` 須聲明預期獨佔——**本弧不動**）
> 探針（已預先提交並校準）`crates/oo/tests/one_writer_at_a_time_probe_test.rs`
> 基線：dev `2d052cc`／`oo v0.70.0` ⟹ **4 綠 6 紅**，三輪一致；六支紅的都紅在預定斷言，零空洞讀數。

## 1. 缺陷

會移動 `HEAD` 的指令有四個：`commit`、`refine`、`squash`、`rollback`。**只有 `commit` 取提交臨界區**（`.oo/format` 上的排他鎖，Q-016b）。〔量，v0.70.0〕兩個寫者並行，兩邊 rc=0，一方回報的提交不在歷史、下一次 `gc` 刪掉它：

| 並行 | 遺失 |
| :-- | :-- |
| `commit` ＋ `refine`（工作集非空） | **30／30**；內容不在 `HEAD` 的根、注入已被消費、`status` 說 static |
| `squash` ＋ `refine` | **10／10** |
| `squash` ＋ `squash` | 3／10 |
| `refine` ＋ `refine` | 1／10 |
| 依序（對照組） | 0 |

〔量，決定性〕**`migrate` 以 rename 換掉 `.oo/format`（inode 608175 → 608190）且自己不取鎖** ⟹ 有人持鎖時跑過 `migrate`，之後的 `commit` 鎖住新檔、在持鎖期間落地；不跑 `migrate` 的對照組被擋住。**臨界區在 `migrate` 之後不再互斥——連 `commit` 之間亦然。**

## 2. 裁定與依據

*   **D87（用戶，甲）**：會移動 `HEAD` 的操作**彼此可序列化**——每一個都在同一個臨界區內讀 `HEAD`，結果等同依某個順序逐一執行。
*   **`#gc` 不在本裁定內**（否決乙）：它維持 `SPEC_08` §6.2.1 的獨佔聲明。
*   **`migrate` 納入射程**（驗收方開卡時加入，已向用戶明說）：它不移動 `HEAD`，但它換掉臨界區所在的檔；不處理它，甲的不變式不成立。
*   機制不限（鎖、CAS 皆可），**但必須與既有 `commit` 的臨界區互斥**；**不得新增鎖檔**（那是佈局變更，Q-016b 為此選了 `.oo/format`）。

## 3. 射程＝不變式（不是機制）

*   **I1** **四個寫者彼此可序列化**：任兩個並行，結果等同依某個順序逐一執行——每一個回報落地的提交都在歷史裡（r5、r6）；每一個在讀 `HEAD` 之前已在臨界區內（r1–r3：別人持有臨界區時，它不移動 `HEAD`）。
*   **I2** **臨界區不因 `.oo/` 裡的檔被換掉而失效**：`migrate` 換掉 `.oo/format` 的前後，臨界區照樣互斥（r4）；**一個在舊檔上等到鎖的寫者，不得與在新檔上取得鎖的寫者同時在臨界區內**（探針造不出這一格，請在 Q2 回答你怎麼保證）。
*   **I3** **不等的照樣不等**：讀（`status`／`log`／`inspect`）、`evolve`、`gc` 在別人持有臨界區時照常作答（g1–g3）。
*   **I4** **取不到臨界區時具名拒絕、不寫**，與 `commit` 今天的答法同族（`what_it_says_is_what_happened::r9`：取不到鎖不是讀不到工作集）。
*   **I5** **原本對的仍對**：依序執行的四個寫者照常落地（g4）；`commit` 的晚到者語義（D61、D86）不變；**所有既有探針**。
*   **I6** 不新增耐久檔、不改磁碟格式、不推進佈局；值位址與提交位址不動。

## 4. 紅線（今天綠，必須保持綠）

*   g1–g4；`already_in_head` 7、`a_commit_that_ate_what_it_never_read` 6、`what_it_says_is_what_happened` 23、Q-067 11、Q-066 10、Q-065 12。
*   conformance 162／162；`x: 0` 根 `31745ef0…`、`v: 1 + 1` 根 `f4f32e7b…`、標準根 `7038e250…`；新倉 `layout=8`／`encoding=5`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** 每一個寫者：取臨界區在哪一步、讀 `HEAD` 在哪一步（指名函式與順序）；`refine` 若在取臨界區之前做了任何讀取，那些讀取的結果是否在臨界區內重驗。
*   **Q2** I2 的後半：一個在舊 `.oo/format` 上排隊的寫者，在 `migrate` 換檔之後拿到鎖時，你怎麼知道它鎖的是舊檔、之後怎麼做。
*   **Q3** `.oo/format` 唯讀（0400）時，`refine`／`squash`／`rollback`／`migrate` 今天與改後各答什麼。
*   **Q4** 事實陳述：`gc` 與 `commit`、`rollback` 並行，你讀程式碼看得出的遺失路徑（若有）。**不要改 `gc`**。

## 6. 明文不在射程內

*   `#gc` 的並行（D87 否決乙）；`evolve` 的並行（D49）。
*   新增鎖檔或任何佈局變更。
*   規格文字（`SPEC_10` §4.1 一款、`SPEC_08` §6.2.1 措辭）：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r6 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。**
*   r1–r4 以**引擎自己的臨界區**為儀器：探針對 `.oo/format` 取與引擎同一種獨佔檔案鎖，啟動指令，等 1.5 秒，看 `HEAD` 動了沒有。**若你改了臨界區的機制（例如換成 CAS），r1–r4 會量不到——請在報告裡說，不要改探針。**
*   r5、r6 是真的並行，只看結局（每一個回報的提交都在歷史裡），各 5 次；基線幾乎每次遺失，正確的實作永不遺失。
*   探針用到的儀器（CLI 輸出、`.oo/HEAD`、`.oo/format`、`log`、`inspect`）不准改。夾具不准改。`rustfmt` 不得掃探針檔。
*   〔驗收方校準〕**兩極皆做**：
    基線 4 綠 6 紅（三輪一致）。
    一個最小參考實作（`refine`／`squash`／`rollback`／`migrate` 在讀任何東西之前取同一個 `CommitLock`；取到鎖之後比對鎖住的檔與路徑上的檔是同一個 inode，不是就放開重取）⟹ 本探針 10／10。
    **四個變異各自讓一支轉紅**：`status` 取臨界區 ⟹ g1；`evolve` 取臨界區 ⟹ g2；`gc` 取臨界區 ⟹ g3；`migrate` 不取臨界區 ⟹ r4。（`evolve` 的變異第一版寫錯——新工作區還沒有 `format`，一取就失敗、九支全倒——改為「`format` 存在時才取」重做。）
    **參考實作的 inode 比對用了 `std::os::unix`，不要照抄**：本專案要跨平台（`CommitLock` 的註解寫了 Windows 的 `LockFileEx`）。
    同一參考實作跑全樹：**254 target／2441 passed／0 failed，exit 0**（= v0.70.0 的 253／2431 加上本探針 10 支）——**沒有既有測試衝突，不需預先修訂**。
    〔驗收方另掃，上一弧的教訓〕以 `.oo/format` 權限或位元組為儀器、又跑寫者的既有測試：`a_refusal_written_as_an_answer`（比對 `format` 位元組；取鎖只開檔不寫）、`what_it_says_is_what_happened::r9`（只跑 `commit`）——皆不受影響。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

1. **I1** `commit`、`refine`、`squash`、`rollback` 在同一把 `CommitLock`（既有 `.oo/format` 排他鎖）裡讀 `HEAD`，再移動它。r1–r3、g4。r5 的五次、r6 的五次，每個回報的提交都在 `oo log` 裡。另量 r6 的兩種啟動順序各 20 次，遺失 0。
2. **I2** `migrate` 換掉 `.oo/format` 的那一步在同一把鎖裡。r4。`flock` 返回後比對打開的檔與路徑上的檔；不是同一個就放開再取。量到：在舊檔上排到鎖的 `commit`，新檔仍被別人鎖著時不落地；新檔放開之後才 `Commit successful`。
3. **I3** `status`／`log`／`inspect`、`evolve`、`gc` 不取這把鎖。g1–g3。
4. **I4** 取不到鎖時具名拒絕、不寫。句型與今天的 `commit` 相同：`cannot lock <路徑>: permission denied`。`what_it_says_is_what_happened::r9` 仍綠。
5. **I5** 依序的四個寫者仍落地（g4）。`commit` 仍在鎖前列出並讀工作集，鎖後重讀 `HEAD`（D86）。紅線：`already_in_head` 7、`a_commit_that_ate_what_it_never_read` 6、`what_it_says_is_what_happened` 23、Q-067 11、Q-066 10、Q-065 12，皆 0 failed。範圍裡沒有 refine 的 squash 仍把中間提交壓出 log（依序三筆壓成 2 行）。
6. **I6** 沒有新耐久檔，沒有佈局號，值位址與提交位址沒有動。版本仍是 `oo v0.70.0`。

### 8.2 順手改動（逐項指名）

1. `CommitLock::acquire`（`crates/oo/src/main.rs`）在鎖到手之後核對檔案身份。Unix 是裝置號加 inode，Windows 是磁碟區序號加檔案索引。平台報不出身份時，這一關視為同一檔，互斥只靠鎖本身。
2. `run_refine`／`run_squash`／`run_rollback` 在 `require_universe` 之後、`Ouroboros::init` 之前取鎖。
3. `run_migrate`：宣告已經是這次會寫下的 layout 與 encoding 時不取鎖，仍印 `Nothing was changed`。否則取鎖，鎖內重新 `init`、重讀宣告，再 `migrate_layout`。
4. `run_commit` 在取鎖之後再 `Ouroboros::init` 一次，讓佈局旗標和 `HEAD` 屬於鎖住的那個檔。鎖前的 `paths`／`load_all` 沒有搬。
5. `Universe::squash`（`universe.rs`）與 `savepoint::rewrite_commit_ancestor`（`savepoint.rs`）：範圍裡的 refine 留在 log 走訪上。squash 那顆的 `ancestor:` 指向最新的 refine；`set_head` 之後把這些 refine 的 ○ `ancestor:` 改成下一顆 refine，最舊的改成 base。改寫失敗會把已改的 ○ 改回，並把 `HEAD` 放回移動前。沒有 refine 時 `ancestor:` 仍是 base，不改舊 ○。`commits_after` 不把留下的 refine 算進 `compressed` 的數目。提交物件本身不改寫。

### 8.3 工單哪裡是錯的

工單說只取同一把鎖、再做 inode 重取，r6 就永不遺失，全樹 254／2441／0。只取鎖時，refine 先做完再 squash，結果等於依序「先 refine 再 squash」：squash 把那顆 refine 壓出祖先走訪，`oo log` 的 `commit` 行沒有它的 digest。量到 refine 先啟動 20／20 這樣；squash 先啟動仍有 1／20。探針用 `contains(digest)`，那一序會紅。全樹數字在加上「refine 留在走訪上」之後成立：254 行、2441 passed、0 failed。

### 8.4 工單指名要你回答的問題

**Q1**

| 寫者 | 取鎖 | 讀 `HEAD` |
| :-- | :-- | :-- |
| `commit` | `run_commit` 裡，`injections::paths` 與 `load_all` 之後，`CommitLock::acquire` | 鎖前：`refuse_lost_context` → `get_head`，只決定要不要在進鎖前拒絕。鎖後：再次 `init`、`refuse_lost_context`、`Universe::load` → `get_head`、`head_if_it_holds`、`Universe::commit` 裡的 `refuse_lost_context`。落地用的是鎖後這幾次。 |
| `refine` | `run_refine`，`require_universe` 之後、`Ouroboros::init` 之前 | 鎖後：`refuse_lost_context`、`Universe::load`、`Universe::refine`（用剛載入的 `self.head`，並再 `refuse_lost_context`）。`--sign` 的身分檔在鎖內讀；探針不簽。 |
| `squash` | `run_squash`，同一位置 | 鎖後：`Universe::load`、`commits_after`（用 `self.head`）、`Universe::squash` 的 `get_commit`／`commit_is_ancestor`。 |
| `rollback` | `run_rollback`，同一位置 | 鎖後：`Universe::load`，`Universe::rollback` 用 `self.head`，然後 `set_head`。 |
| `migrate` | 宣告尚未現行時，`migrate_layout` 之前 | 不讀 `HEAD`。鎖前讀過的宣告只用來跳過「已經現行」。鎖內重讀，寫的是重讀的結果。 |

`require_universe` 只看見證檔在不在，不讀 `HEAD`，四個寫者都在取鎖之前做這一步。`refine` 在取鎖前沒有別的倉讀取，沒有要在鎖內重驗的倉讀結果。

**Q2** `acquire` 的迴圈：打開路徑、`try_lock`／`lock`，然後 `still_the_directory_entry`。打開的 fd 做 `metadata`，路徑再 `metadata`。Unix 比 `(dev, ino)`，Windows 比 `(volume_serial_number, file_index)`。不相等，或路徑已經不在，就 `unlock` 再打開路徑。排在舊 inode 上的寫者要等舊 fd 的 `lock` 返回才做這次比對；比對失敗就不會帶著那把鎖去 `set_head`。它改去鎖路徑上的新檔。新檔上已經有人通過比對並持鎖時，它堵在新檔的 `flock` 上。量（euid 1000）：持有舊檔的鎖、把 `.oo/format` rename 走、寫入新檔並鎖住新檔、放開舊檔之後，排隊的 `commit` 仍在跑，`HEAD` 未動；放開新檔之後 rc=0，`Commit successful`，`HEAD` 已動。

**Q3** `.oo/format` 模式 0400，euid 1000。今天（v0.70.0 二進位）與改後：

| 指令 | 今天 | 改後 |
| :-- | :-- | :-- |
| `commit` | rc=1 `Error: cannot lock <路徑>: permission denied`，`HEAD` 與 format 不動 | 同一句，不動 |
| `refine` | rc=0 `Refine commit: …`，`HEAD` 移動 | rc=1 同一句 `cannot lock`，不動 |
| `squash` | rc=0 `Squash commit: …`，`HEAD` 移動 | rc=1 同一句，不動 |
| `rollback` | rc=0 `Rolled back to …`，`HEAD` 成為目標 | rc=1 同一句，不動 |
| `migrate`（已是 layout=8、encoding=5） | rc=0 `Store declarations are already layout=8 and encoding=5. Nothing was changed.` | 同一句，不動 |
| `migrate`（layout=7） | rc=0，先印成本，再 `Migrated store layout to layout=8.`，`HEAD` 不動 | rc=1 `cannot lock`，format 仍是 `layout=7`，`HEAD` 不動 |

**Q4** 沒有改 `gc`。`gc` 不取這把鎖（g3）。`Universe::commit` 在 `set_head` 之前寫根、寫提交物件、寫 ○。`gc::run_gc` 用較早的 `list_digests`，再用當時的 `HEAD` 做 `mark`（`follow_abandoned == false`），刪掉清單裡走不到的 digest。視窗：新物件已經在那份清單裡，`mark` 讀到的仍是舊 `HEAD`，物件被刪，隨後 `set_head` 指向被刪的那顆。`rollback` 不鑄新提交：`append_abandoned` 然後 `set_head`。目標是移動前 `HEAD` 的祖先，`mark` 讀到移動前或移動後的 `HEAD` 都走得到它。移動之後，只有舊尖端才走得到的物件變成可收集，與 rollback 完成之後再 `gc` 同一件事。

### 8.5 探針

`crates/oo/tests/one_writer_at_a_time_probe_test.rs` 未改，未 rustfmt。夾具未改。臨界區仍是 `.oo/format` 上的同一種排他鎖，r1–r4 量得到。本探針 10／10（紅線那輪 17.51s）。

### 8.6 數字

1. 三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`：每輪 rc=0，`^error` 0，`test result:` 254 行，2441 passed，0 failed。去掉 ` finished in ` 之後三輪 `cmp` 相同。
2. conformance 162／162，rc=0。
3. `~%Math./add (1, 2)` → 3，`(1, 3)` → 4，rc=0。這兩次 `eval` 沒有建立 `.oo/`。
4. `x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。`v: 1 + 1` 根 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`。新倉 `layout=8`／`encoding=5`。
5. 依序先 refine 再 squash 到第一筆：log 為 3 行（squash、那顆 refine、base），中間的一般提交不在 log。依序只 squash 三筆一般提交：仍是 2 行。

### 8.7 你認為需要改規格之處

`SPEC_10` §4.1 與 `SPEC_08` §6.2.1 仍由驗收方收尾。這次沒有改規格檔。建議補一句：squash 壓過的範圍裡，refine 提交留在 log 走訪上，其餘提交離開走訪。

---

## 9. 驗收（驗收方填）

### 9.1 第一輪驗收：**不受理，開修補回合 R-1（一項；起因在驗收方的探針）**

**起因在驗收方**：r6 原本要求「squash 與 refine 回報的提交都在歷史裡」，**那不是可序列化給的性質**——依序先 refine 再 squash，squash 會把那顆 refine 壓掉，這是對的。〔驗收方複驗，v0.70.0，依序〕refine → squash：refine 不在 `log`、`log` 2 行。驗收方的參考實作通過舊 r6，**是因為舊 r6 永遠先啟動 squash**（交付量到：refine 先啟動 20／20 紅）。交付在 §8.3 指出了這件事——**這部分交付是對的**。
**但交付的修法不受理**：§8.2 第 5 項改了 `squash` 的語義（範圍裡的 refine 留在走訪上）並**改寫既有 ○ 的 `ancestor:` 行**。前者是需要裁定的設計變更，不在 D87 射程；後者違反 ○ 是事件紀錄、不得事後改寫（`SPEC_08` §6.2.1 D81「遷移不得事後補造 ○ 註記」；`layout=8` 遷移條「既有 ○ 一顆都不改寫」）。**探針錯了的時候，該做的是在 §8.3 說、不改探針、也不改語義去遷就它**——前半交付做到了。
**驗收方修訂 r6**（標 `AMENDED 2026-10-03 for Q-069 R-1`）：只要求 **squash 自己回報的提交在歷史裡**——每一種依序它都在（它是 `HEAD`，或是其後 refine 的父）；〔量〕基線上被蓋掉的**正是 squash，10／10**。試驗**輪流先啟動 squash 或 refine**，兩種依序都會被量到。
**兩極重校**：基線（`609fc78`）4 綠 6 紅三輪一致，r6 紅在預定斷言；只取鎖、**不動 squash** 的參考實作：本探針 10／10，r6 單跑 10 次（60 次試驗、30 次 refine 先啟動）全綠。
**已知限度（記帳）**：squash 讀到舊 `HEAD` 而蓋掉 refine 的那種遺失，與「依序先 refine 再 squash」在磁碟上逐位元組不可分（refine 不改根、○ 的分叉數相同），只有 squash 訊息裡的計數不同——**探針不量它**；r2（squash 在別人持有臨界區時不移動 `HEAD`）與 I1 的文字覆蓋這一側。

**R-1 射程（交付方）**：
1. **撤回 §8.2 第 5 項**：`Universe::squash`、`savepoint::rewrite_commit_ancestor`、`commits_after` 回到交付前的行為；**不改寫任何既有 ○**。
2. §8.2 第 1–4 項保留。
3. 修訂後的本探針 10／10；全樹 ×3；紅線同 §4。
4. 在 §9.2（交付方填，新開）回答：撤回之後，r6 的兩種啟動順序各 20 次的結果。

### 9.2 R-1 交付回報（交付方填；本行以上一字不得動）

1. **撤回 §8.2 第 5 項。** `savepoint::rewrite_commit_ancestor` 刪除。`Universe::squash` 回到交付前：新 ○ 的 `ancestor:` 是 base 的 digest，`set_head` 之後不改任何既有 ○。`commits_after` 再次把範圍裡的每一顆都算進 `compressed`。這兩個檔與 `609fc78` 的對應內容相同。
2. **§8.2 第 1–4 項留著。** `CommitLock` 鎖到手之後仍核對檔案身份並重取；`refine`／`squash`／`rollback` 仍在 `require_universe` 之後取鎖；會改寫宣告的 `migrate` 仍在鎖內重讀再寫；`commit` 仍在鎖後再 `init`，鎖前的 `paths`／`load_all` 沒有搬。探針這次沒有再改（r6 的修訂是 `4cbaff7`）。版本仍是 `oo v0.70.0`。
3. **修訂後的本探針 10／10**（17.65s）。`history_ops` 15／15，含全部 `red_squash_*`：三筆一般提交壓成 2 行。
4. **撤回之後，r6 的兩種啟動順序各 20 次**（release 二進位，euid 1000）。兩邊都是 rc=0，squash 回報的 digest 都在 `oo log`，遺失 0。
   - squash 先啟動 20／20：refine 回報的 digest 也在 log，`commit` 行 3。
   - refine 先啟動 20／20：refine 回報的 digest 不在 log，`commit` 行 2。這就是依序先 refine 再 squash。
5. **紅線。** `a_commit_that_ate_what_it_never_read` 6（8.43s）、Q-067 11（5.19s）、Q-066 10（1.94s）、`already_in_head` 7（7.47s）、Q-065 12（1.85s）、`what_it_says_is_what_happened` 23（1.77s），皆 0 failed。g1–g4 在第 3 項的本探針裡。
6. **三輪** `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`：每輪 rc=0，`^error` 0，`test result:` 254 行，2441 passed，0 failed。去掉 ` finished in ` 之後三輪相同。
7. conformance 162／162，rc=0。`~%Math./add (1, 2)` → 3，`(1, 3)` → 4，rc=0，這兩次沒有建立 `.oo/`。`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。`v: 1 + 1` 根 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`。新倉 `layout=8`／`encoding=5`。
8. 規格檔沒有改。§8.7 那句「refine 留在走訪上」隨這次撤回一起收回。`SPEC_10` §4.1 與 `SPEC_08` §6.2.1 仍由驗收方收尾。
