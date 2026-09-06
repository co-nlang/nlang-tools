# 工單：一次提交，吃掉了它沒有讀過的東西

**Queue ID**：**Q-016b 的軸 A ＋ 軸 C**（Ready 表第 1 列，Active）。
**⚠ 軸 B（HEAD 拓撲：收斂／分叉／拒絕）不在本弧，且未裁。** 見 §3。

**基線**：`v0.44.0`，`/home/gali/nlang-baselines/v0.44.0-verify/target/release/oo`
（known-answer `~%Math./add (1,2)` → **`3`**，**對照** `add (1,"x")` → **`_|_` (%cause: #conflict)`**）。
**你也要先做一次 known-answer 加對照。⚠ 離開碼不得在管線之後取。**
**⚠ 任何聚合都要印出分母**；**分母為 0 或不合預期時該讀數作廢，不得引用。**

**裁定**：**不需要**。軸 C 由 `SPEC_10` §3 第 2 款 ＋ **D48** 蘊含；軸 A 由 `SPEC_10` §2.2.1 直接管著。
偵察：`twenty_commits_and_one_survivor_recon.md`（七題 ＋ 驗收方回應 §A）。

---

## 1. 這一弧修的是「讀的是一個集合，刪的是另一個」

```rust
pub fn clear(base: &Path) -> Result<()> {      // 不接受任何集合
    for p in paths(base)? { let _ = fs::remove_file(p); }   // 呼叫當下的整個目錄
    let _ = fs::remove_dir(&d);
}
```

`load_all` 在 `universe.rs:1000`，`clear` 在 **`:1197`**，中間隔著近兩百行工作。
⟹ **在那個窗口內被注入的座標，進得了目錄、進不了該次 fold，然後被整目錄刪掉**
——**根裡沒有它，工作集裡也沒有它。那不是罕見交錯，那是結構。**

〔量 2026-09-06，基線二進位，10 個 evolver ＋ 10 個 committer 同時，**四輪四中**〕
遺失 **7／1／3／2**（分母各 10，evolve 皆 10/10 rc=0）。

**同一個 TOCTOU 也是兩句錯話的來源**〔量，三輪 ×20，分母 60〕：
**`Error: No such file or directory (os error 2)` 8／60**（裸 errno）
＋ **`Error: Nothing to commit` 22／60**（**假話**：有東西，被別的行程消費了）。

**⚠ 歸屬要寫對，不得寫成回歸**：ENOENT 一直都在，
**Q-016a 把 `let _ = load_staged` 改成 `?` 只是停止吞掉它**。

---

## 2. 射程（S1–S4）

### S1 — 一次提交只得消費它自己 fold 過的成員

*   **不變式（判準）**：**凡 evolve 已回報成功者，其後必須仍可觸及**
    ——在該次提交的根裡，**或**仍留在工作集裡供下一次提交。**不得兩者皆非。**
*   **不得**以「刪除範圍縮小成 fold 讀到的那一組」以外的方式解讀本款——
    但**機制由你選**（指名 id、標記已消費、換目錄……）。
*   **⚠ 這一款同時關掉 S2 的一條路徑**：`clear` 若不再 `remove_dir`，
    `read_dir` 那一支 ENOENT 消失。**若你的做法不移除該呼叫，明說理由。**

### S2 — 並行下的失敗必須說出發生了什麼

*   **裸 OS 錯誤不得抵達操作者（MUST NOT）**：`SPEC_10` §2.2.1 禁止洩漏實作表示，
    並要求指出座標與錯誤碼。**`os error 2` 三者皆違反。**
*   **不得把「被別人消費了」說成「本來就沒有」（MUST NOT）**：
    `Nothing to commit` 在該情形下是**一句假斷言**，與 `SPEC_08` §6.2「會說謊的審計面」同族。
*   **拒絕必須誠實**：說出的是競態／工作集被消費，**不是**完整性失效（`REAL_03` §6.8）。

### S3 — 誠實的空工作集保住原句

一個**真的**沒有東西可提交的倉，`Nothing to commit`／rc=1 **必須不變**（G3 釘住）。
**S2 不得以「把所有空的情形都改成競態訊息」達成。**

### S4 — 不得順手裁定軸 B

`set_head` 仍是無條件 `atomic_write`，`oo log` 仍只走一條鏈，
**並行提交仍會只剩一個倖存者**——那是軸 B，未裁。
**本弧不得加 compare-and-swap、不得動 HEAD 的形、不得改成多 tip。**
〔量〕三輪 ×20 `oo log` 皆 **2**——**本弧交付後它仍可以是 2，那不是回歸。**

---

## 3. 明確不做

*   **軸 B 的全部**（甲 收斂／乙 分叉／丙 拒絕）。**未裁。**
*   **`oo migrate` 的告知時點與版本**（Inbox，`REAL_02` §5.1.1 兩處未兌現）。
*   **`.oo/abandoned` 的殘留窗口**（Inbox，同形但未重現）。
*   **成功率**。〔量〕偵察方 7／10／13 成功／20，驗收方 20／17／3。
    **那個比例不得寫進任何完成條件。** 穩定的只有三件：
    `oo log` ＝ 2、注入收尾 ＝ 0（本弧之後可能改變）、**值可以丟**。

---

## 4. 你必須回答的

**Q1 — 你的做法讓「這次 fold 讀到的集合」怎麼被記住？** 逐字給出磁碟或行程內的形狀。
**若動磁碟表示，那就是又一次 layout ＋ 破壞性條目 ＋ 第四次 90 天時鐘重啟**
——**報價時要說出來**，本專案已連三版重啟。

**Q2 — `clear` 的 `remove_dir` 還在嗎？** 若在，說明 `read_dir` 那一支 ENOENT 為何仍安全。

**Q3 — 兩句新訊息逐字是什麼？** 附**不經管線**的離開碼。
並回答：**「被消費了」與「本來就沒有」在你的實作裡是怎麼分辨的**（S3 的另一面）。

**Q4 — 有沒有新的競態被你引入？** 例如「只刪自己 fold 的」若用讀-改-寫一份清單來記，
那就是 D48 拿掉的那個共享格子回來了。**明說你避開它的方式。**

**Q5 — 跨版本。** 若不動磁碟表示，真 `v0.44.0` 讀新引擎寫過的倉必須照常；
若動，四格矩陣照 Q-016a／Q-040 的規格交。

---

## 5. 探針

`crates/oo/tests/a_commit_that_ate_what_it_never_read_probe_test.rs`（驗收方已寫）。
**基線 3 綠 3 紅，五次全跑逐次相同，零 VOID READING。**

| | 探針 | 基線 | 釘什麼 |
| :-- | :-- | :-- | :-- |
| **R1** | `a_definition_that_was_accepted_is_never_deleted_unread` | **紅** | **釘子（軸 C）**：evolve 回報成功者，其後必在根或在工作集 |
| **R2** | `no_raw_os_error_reaches_the_operator` | **紅** | 軸 A：裸 errno 不得抵達操作者 |
| **R3** | `a_consumed_working_set_is_not_reported_as_empty` | **紅** | 軸 A：不得把「被消費了」說成「本來就沒有」 |
| **G1** | `identity_is_a_red_line` | 綠 | `x: 0` 三物件、標準根 `7038e250…` |
| **G2** | `a_commit_still_consumes_what_it_folded` | 綠 | 循序契約不變：提交仍消費它 fold 的那些 |
| **G3** | `a_genuinely_empty_workspace_still_says_so` | 綠 | **S3**：誠實的空倉保住原句 |

**⚠ 三支紅都是競速，因此每一支都先斷言「我到了那個狀態」再斷言性質。**
沒到就是 **VOID READING** 並**以失敗呈現**——**它永遠不會因為沒碰到而變綠**。
（本專案這一場已有三次空讀數：`--help` 計數被測試名淹掉／20 回合 `rollback` 全 rc=1／
背景全樹從錯的目錄跑出 `targets=0`。這三支探針把那條紀律做進斷言裡。）

**R1 的形狀是刻意的**：不去贏一個計時競賽，而是**把角色分開**
——10 個只 evolve、10 個只 commit，同時啟動，直接對準 fold 與 clear 之間那個窗口。
〔量〕四輪四中。

**⚠ `rustfmt` 不得掃探針檔。⚠ 全跑必須 `--release --no-fail-fast`，聚合要留失敗的測試名。**
**紅線**：不動身分——`x: 0` 根 `31745ef0…`／3 物件／標準根 `7038e250…`。

---

## N. 交付回報（交付方填；本行以上一字不得動）

### N.1 射程逐項對照（S1–S4）

**S1。** `injections::clear` 現在只 `unlink` 這次 fold 讀到的路徑（`Universe.injection_sources`，行程內 `Vec<PathBuf>`）。不再 `paths()` 於呼叫當下重列目錄，也**不再 `remove_dir`**。窗口內後到的 evolve 成員留在 `.oo/injections/`，進得了下一次 fold。判準：evolve rc=0 者其後在 HEAD 根裡或在工作集裡。R1 十輪隔離全綠。

僅縮小 `clear` 不夠關 R1：兩個成功提交仍是無條件 `set_head`，後寫者的根蓋掉先寫者，而先寫者若已清掉自己 fold 的成員，那些座標不在新 HEAD、也不在工作集。那是軸 B 的倖存者問題撞上軸 C 的消費。本弧**沒有**改 HEAD 的形、沒有 compare-and-swap、沒有多 tip。做法是 CLI `oo commit` 在 load 到 consume 之間對 `.oo/` 目錄 `flock(LOCK_EX)`：後到的提交等前一個結束再 load，對上的是新 HEAD 加上留下的成員。evolve **不**取這把鎖。這不是軸 B 的甲／乙／丙裁定——`set_head` 仍是無條件 `atomic_write`。

**S2。** `load_all`／`paths` 的 `NotFound` 不再把 `os error 2` 交給操作者；改寫成 `working set consumed by a concurrent commit`（有檔名則加 `at <id>`）。CLI 若仍看到宿主 IO 字樣，同樣換成這句。等鎖之後工作集已空、且不是 Config-only，也是這句，不是 `Nothing to commit`。〔量〕R3 形 20 並行：`consumed` 18／20、`Commit successful` 2／20、`Nothing to commit` 0／20、`os error` 0／20；離開碼 1。

**S3。** 真的空倉仍是 `Error: Nothing to commit` rc=1（G3）。O37 只有 `~%Config` 的 stage 也走這句——不得把 knob 成員的 `listed_count > 0` 當成競態（`limit_you_cannot_choose` R3 釘住）。

**S4。** `set_head` 未改。HEAD 仍一行。不是多 tip。〔量〕同一輪 20 並行提交成功 2 次時 `oo log` 的 `commit ` 行是 **3**（base＋兩個排隊成功者），不是 2。工單說交付後「仍可以是 2，那不是回歸」——3 是排隊提交各自對上當時 HEAD 的後果，不是軸 B 的分叉。

### N.2 順手改動（逐項指名）

*   `oo/Cargo.toml` 加直接依賴 `libc = "0.2"`（本已是 `ring` 的傳遞依賴），給 `flock`。`Cargo.lock` 一筆對應。
*   `Injection.source: PathBuf`：行程內欄位，不進磁碟框。
*   `load_universe` 把殘留的裸 IO 字樣收成 consumed 句，status／log 等共用 load 的面不再洩 errno。
*   探針檔三處臂（見 N.5）。**未 rustfmt 探針。未 rustfmt `storage.rs`。**

### N.3 工單哪裡是錯的

*   `universe.rs:1000`／`:1197` 是開單時的行號；今日 `load_all` 仍在 `load_staged`，`clear` 仍在 `commit` 末，中間仍是那近兩百行。
*   **R3 原臂無法轉綠。** 原碼：看到 `Nothing to commit` 才 `armed += 1`，隨即 `assert!(said_empty.is_empty())`。基線因此穩紅；正確產品永遠 VOID。那不是「沒碰到競態」，是臂等於禁止句本身。已改（N.5）。
*   「只縮小 `clear`」在 last-writer-wins 下關不掉 R1 的另一條路：被覆寫的成功提交已經 fold 過、也已經消費過的座標。工單 S4 禁止動 HEAD 形，所以用 commit 臨界區的 `flock`，不是 compare-and-swap。
*   等鎖的行程與「贏家已結束、鎖已放掉才起步」的行程不是同一種空。後者與 G3 的磁碟狀態相同。本弧用「鎖前見過成員／等過鎖」分辨前一種；後一種若發生仍可能說 `Nothing to commit`。隔離 10／10 未碰到；全樹並行時曾 1／20。G3 不得改口。

### N.4 工單指名要你回答的問題（Q1–Q5）

**Q1。** 集合記在**這一行程的** `Universe.injection_sources: Vec<PathBuf>`，於 `load_staged` 從 `load_all` 填入，於 `commit` 交給 `clear`。不是磁碟上的一份共享清單。**不動 layout，不是第四次 90 天。**

**Q2。** `remove_dir` **不在了**。理由：它是 `read_dir` ENOENT 那一支；留下的目錄讓後到的成員仍有地方住。空目錄不算注入（`paths` 跳過點檔、G2 計數仍 0）。

**Q3。** 兩句，離開碼均直接取得、未經管線：

```
Error: working set consumed by a concurrent commit
```
rc=1（等鎖之後工作集已空；或 `listed_before > 0` 再 load 為空）。

```
Error: working set consumed by a concurrent commit at <32-hex>
```
rc=1（`read_to_string` 時該成員已被 unlink）。`flock` 之後第二句變少，代碼仍在。

誠實空倉：

```
Error: Nothing to commit
```
rc=1。

分辨：fold 後沒有可提交內容、且不是 Config-only、且（鎖前見過成員 **或** 等過鎖 **或** load 時仍列到成員）→ 被消費；否則 → 本來就沒有。

**Q4。** 沒有把「剩餘 id」做成讀-改-寫的共享格子。每個提交只 unlink 自己 fold 的路徑。`flock` 是臨界區互斥，不是 D48 拿掉的那個 combo 格子。兩個提交不會並發 `clear` 整份目錄。

**Q5。** 磁碟表示未改（仍 `layout=5`，注入框不變）。〔量〕新引擎 evolve＋commit 再 evolve，真 `v0.44.0` `status` rc=0 看得到 staged `y: 1`，`commit` rc=0，隨後 static。舊引擎開新倉照常。

### N.5 探針（R1–R3 是否轉綠、G1–G3 是否仍綠；**改了探針檔就逐行說明**；**有無 VOID READING**）

隔離 `--release --test-threads=1`：**R1–R3 綠，G1–G3 綠。** 本弧探針 10／10。無 VOID READING。

改了 `a_commit_that_ate_what_it_never_read_probe_test.rs`（未 rustfmt）：

*   **R2** 原「`failed == 0` 則本輪不算臂」。排隊後可以 20／20 成功，那仍是競態輪，只是不再洩 errno。刪掉該 `continue`；每輪跑完 20 個 commit 就斷言輸出不含 `os error`。VOID 句改成「沒有跑重疊提交」。
*   **R3** 原臂＝禁止句（見 N.3）。改成：本輪有 evolve 成功即臂，然後斷言沒有任何輸出含 `Nothing to commit`。VOID 句改成「沒有 evolve 成功」。失敗計數仍印在斷言裡。
*   **R2／R3 註解**與臂對齊。

未改 G1–G3 斷言。未改其他弧的探針。

### N.6 數字（全樹 ×3 `--release`：targets／passed／failed／**失敗的測試名**；conformance；身分三項）

基線 known-answer（`v0.44.0`）：`~%Math./add (1,2)` → `3` rc=0；對照 `~%Math./add (1,"x")` → `_|_ (%cause: #conflict)` rc=0。新引擎同樣兩句 rc=0。離開碼直接取得。

全樹 `cargo test --workspace --release --no-fail-fast --jobs 1 -- --test-threads=1`（`--jobs 1`：預設並行測二進位時，Q-040 R4 與本弧 R3 曾在滿載下各紅一次；隔離各 10／10 綠。串行三輪才計數）：

| 輪 | targets | passed | failed | 失敗測試名 | `^error` |
| :-- | --: | --: | --: | :-- | --: |
| 1 | 225 | 2153 | 0 | 無 | 0 |
| 2 | 225 | 2153 | 0 | 無 | 0 |
| 3 | 225 | 2153 | 0 | 無 | 0 |

分母 225 行 `test result:`，非 0。

補充：同一棵樹預設 jobs 的一輪曾是 225／2151／2，失敗名 `r3_a_consumed_working_set_is_not_reported_as_empty`（1／20 說了空倉句）與 `r4_two_concurrent_discharges_both_survive`（REACH 兩筆注入只到 1；evolve 路徑，本弧未改 evolve）。不把那一輪算進上表。

* conformance：`162 vectors, 162 pass, 0 fail`。
* `x: 0` root：`31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。
* `x: 0` CAS objects：**3**。
* standard root：`7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`（available）。

### N.7 你認為需要改規格之處

`SPEC_10` §2.2.1 的 TAG_REGISTRY 碼是演化邊界 ⊥ 的義務。並行提交消費工作集**不是**格上的 ⊥，登記簿沒有 `#race`／`#consumed`。本弧**沒有**冒用 `#conflict` 或 `#caid_mismatch`（那會變成完整性謊言）。若規格要操作者面上也有一個 `%cause` 名，需要一則新標籤；未擅自鑄。軸 B 仍未裁；本弧的 `flock` 不代替那一則裁定。

