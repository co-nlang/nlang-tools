# Q-016b 偵察 — 二十次提交，一個倖存者

> **Queue ID**：`WORK_QUEUE` Q-016b（Active，偵察；§8 開弧 3，未裁）。
> **基線**：`v0.44.0`，`/home/gali/nlang-baselines/v0.44.0-verify/target/release/oo`
> （`oo --version` 逐字 `oo v0.44.0`；known-answer `~%Math./add (1,2)` → **`3`** rc=0；
> 對照 `~%Math./add (1,"x")` → **`_|_ (%cause: #conflict)`** rc=0。離開碼直接取得，未經管線）。
> 工作樹 `nlang-tools` `dev`（本檔是唯一產物）。**這是偵察，不是實作。未改產品程式碼。未寫探針。甲／乙／丙不選。**
>
> **身分**：本輪零改動。〔量〕`x: 0` 根
> `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`，
> `.oo/objects` **3** 個檔，標準根
> `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`。
>
> **⚠ 縮寫**：本文 `CAS` 只指 Content-Addressed Storage。compare-and-swap 寫全稱。

---

## 0. 七題各一句

| | 答案 |
| :-- | :-- |
| **Q1** | 兩個時點：贏家 `load_all` 讀完工作集之後，與 `clear` 的 `paths()` 列出要刪的檔之前。其間另一個 evolve 寫入的座標進得了目錄、進不了該次 fold，然後被整目錄清掉。最小構造（strace 把第一次 `renameat`／`put_root` 延 2s）一次即中：根是 `{ a: 1 x: 0 }`，沒有 `b`，注入 0。 |
| **Q2** | `.oo/injections/<32-hex>` 的 `fs::read_to_string`（`injections.rs` `load_all`），或同目錄在 `exists` 與 `read_dir` 之間被 `rmdir`（`paths`）。`run_commit` → `load_universe` → `load_staged?` 把它浮成 `Error: No such file or directory (os error 2)`。**是 Q-016a 把 `let _ = load_staged` 改成 `?` 才看得見，不是 Q-016a／Q-040 造出 ENOENT。不是回歸。** |
| **Q3** | 今天唯一出口：`main.rs` `run_commit` 在 `!is_dirty` 或沒有可提交內容時 `bail!("Nothing to commit")`。空倉與「有東西被別人 `clear` 掉」走同一句。要分辨，至少得記得「這一行程 load 時見過注入／後來目錄空了」——價在訊息與是否保留被清掉的 id，不是新機制。 |
| **Q4** | **凡把「現在」讀成單一 commit digest、並沿一條 `previous_commit`（`parent` 否則 `ancestor:`）走訪歷史者，在兩個 tip 同時成立時只會回答其中一條。** 判準是「答案裡有沒有另一條仍在磁碟上的提交鏈」，不是那四個名字。 |
| **Q5** | 甲：`set_head` 改 compare-and-swap＋重試，HEAD 仍一行，**不必 layout**；但若不改 `clear` 吃整份工作集，重試會撞空目錄。乙：HEAD 變成 tip 集合 ⟹ **`.oo/HEAD` 形變 ⟹ `layout`＋破壞性條目＋90 天**。丙：偵測 HEAD 已動就拒絕，代碼最便宜，**也不必 layout**；隱藏成本見 Q6。三者都**不動身分**。 |
| **Q6** | **工作集不在。** 贏家 `clear` 掉整個 `injections/`。晚到者再 `commit` 得同一句假的 `Nothing to commit`（rc=1）。來源檔若還在可以再 evolve；只靠工作集則重跑不了。丙若不改 `clear` 的範圍，並不便宜。 |
| **Q7** | v0.44.0 重現：gc **之前** `commit:` 指向的物件都還在（3 輪皆 0 懸空）；gc **之後** 8 顆註記裡 6 懸空／11 裡 9／14 裡 12，`log` 仍 2、`status` rc=0。`gc::mark` 的根寫明從 HEAD 走，**沒有**把每顆 ○ 的 `commit:` 當根。這是「註記不是 GC 根」的後果，不是漏走一條它自以為在走的邊。若規格要「凡 ○ 指名的提交必須仍可解」，那才要一則裁定去加根。 |

---

## Q1 — 值遺失的窗口

### 量

提交的順序（v0.44.0 `universe.rs`）是

`load_staged`／`load_all` → `put_root` → `put_commit` → `set_head` → `record_commit` → **`injections::clear`（先 `paths()` 再逐檔 `unlink`）**。

`clear` 刪的是它**呼叫當下**目錄裡的全部成員，不是「這次 fold 讀到的那些」。

故遺失需要兩個時點交錯：

1. **贏家已經讀完**注入集合（`load_all` 結束）；
2. **輸家的 evolve 已經把新成員寫進** `.oo/injections/`；
3. 然後贏家的 `clear` 列出目錄並刪光。

座標在（2）進得了工作集目錄，進不了（1）的 fold，再被（3）從磁碟拿掉。根裡沒有它，目錄裡也沒有它。

**最小構造**（真 `v0.44.0`，目錄 `/tmp/q016b-q1c.o7f8Wv`，分母 1/1）：

- 已提交 `x: 0`。evolve `a: 1`（1 個注入）。
- `strace -e inject=renameat:delay_enter=2000000:when=1` 包住 `oo commit -m a`：第一次 `renameat` 是 `put_root`，在 `load_all` **之後**、`clear` **之前**，延 2s。
- 看到該 `renameat` 之後立刻 `oo evolve b.n`（`b: 2`）。evolve rc=0，注入 1→**2**。
- 贏家 commit rc=0。`strace` 逐字兩次 `unlink`（兩個注入都刪了）。注入目錄空。
- 根物件逐字：

```text
{ a: 1 x: 0 ~%__nlang_system_digest: "7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911" }
```

沒有 `b`。`oo status`：`Universe is static (no staged changes).` rc=0。

對照：若把延遲放在 `clear` 的 `unlink` 上（`load_all` 與 `paths()` 都已過），後到的 `b` **會留下**——`status` 顯示 `b: 2`，根仍只有 `a`。那是「歷史沒吃到、工作集還在」。Q1 要的遺失是後一種的反面：工作集也被清掉。

三輪 ×20 並行 `evolve`＋`commit`（真 v0.44.0，每輪分母 workers=20）把同一窗口從「構造」變成「自然發生」：

| 輪 | evolve rc=0 | commit rc=0 | 根上的 `k*` | `oo log` | 注入剩餘 |
| --: | --: | --: | --: | --: | --: |
| 1 | 20/20 | 7/20 | **20/20** | 2 | 0 |
| 2 | 20/20 | 10/20 | **16/20**（缺 k13／k17／k18／k19） | 2 | 0 |
| 3 | 20/20 | 13/20 | **19/20**（缺 k19） | 2 | 0 |

第 2 輪 16/20 比 brief 的 19/20 更乾。窗口是結構的，不是「跑很多次總會遇到」。

### 結論

遺失不是 HEAD 被覆寫本身，是 **`clear` 的範圍＝整個目錄**，大於 **fold 讀到的集合**。半 B 的甲／乙／丙若只談 HEAD 拓撲、不談 `clear` 吃誰，不會關上這扇窗。

---

## Q2 — `os error 2` 是哪一個檔

### 量

三輪 ×20 裡這句出現 **2＋5＋1＝8／60** 次，逐字：

```text
Error: No such file or directory (os error 2)
```

離開碼 1。`evolve` 在同一批是 20/20 rc=0，所以不是 evolve 寫注入失敗。

`run_commit` 在 fold／閘之前呼叫 `load_universe` → `load_staged?`。`load_all`（v0.44.0 與今天同一段）是：

```text
paths()        // read_dir(.oo/injections)，目錄不存在則 Ok([])
read_to_string // 對 paths 列出的每一個檔
```

並行下贏家的 `clear` 正在 `unlink`／`rmdir`。兩種 TOCTOU 都會變成同一個 `io::Error`：

| 交錯 | 呼叫 | 路徑 |
| :-- | :-- | :-- |
| `paths` 列出了檔，隨後被 `unlink` | `fs::read_to_string` | `.oo/injections/<32-hex>` |
| `exists()` 為真，隨後 `rmdir`，然後 `read_dir` | `fs::read_dir` | `.oo/injections` |

目錄若在 `exists()` 之前就沒了，`paths` 回空集合，**不會**走 os error 2，會落到 Q3 的 `Nothing to commit`。

用 strace 包住整個 20 並行會把競態變慢，20/20 全成功、抓不到失敗行程的 ENOENT 行。改用延遲 `unlink` 的構造時，另一個 commit 反而先 `clear` 完，延遲的那一方自己的 `unlink` 變成 ENOENT 且被 `let _ = remove_file` 吞掉（`clear` 不 `?` 刪檔）。**會浮到 CLI 的，是 `load_staged?` 那一條。**

Q-016a 把 `let _ = load_staged` 改成 `?`（工單 N.2；v0.44.0 `main.rs` `load_universe` 仍是 `u.load_staged(engine, path)?`）。Q-040 沒有再動這條路徑。ENOENT 一直都在；以前被丟棄，看起來像空工作集。

### 結論

**不是回歸。** Q-016a 讓一個原本被吞掉的競態第一次說出話，而那句話洩了裸 errno（`SPEC_10` §2.2.1 禁止的實作表示），也沒有座標、沒有錯誤碼。半 A 今天就能開：把這句改成誠實的競態／空工作集報告，不必等甲／乙／丙。

---

## Q3 — `Nothing to commit`

### 量

全樹只有一個產品出口：`crates/oo/src/main.rs` `run_commit`，在 `!universe.is_dirty` 或沒有可提交內容時 `bail!("Nothing to commit")`。探針會釘這句還在（`limit_you_cannot_choose` C2）。

空倉（剛提交完、無注入）：

```text
Error: Nothing to commit
```

rc=1。

兩個 evolve 之後贏家提交（`clear` 掉 2 個注入，分母 2），再立刻提交：

```text
Error: Nothing to commit
```

rc=1。注入數 0。**同一句。**

三輪 ×20 裡這句是 11＋5＋6＝**22／60**，比 os error 2 多。它是「load 時目錄已經空了」（`paths` 走不存在→空集合那一支），不是「本來就沒寫過」。

### 結論

是的，這是唯一的「工作集為空」出口。要分辨「從來沒有」與「有過、被另一行程的 `clear` 消費了」，不必新儲存形：commit 開始時若 `paths` 非空而閘前再讀是空，或這一行程自己的 session 剛 evolve 成功但 load 卻是空，就可以換一句話。價：`run_commit` 十數行＋一句新文案（仍要座標／錯誤碼，不得再洩 errno）。半 A。

---

## Q4 — 假設 tip 唯一的不變式

### 量

`.oo/HEAD` 是**一行** digest。`set_head` 是無條件 `atomic_write`，沒有 compare-and-swap。並行誰後寫誰在。

從那一行出發的走訪都是單鏈：

- `Ouroboros::log`：HEAD → `previous_commit`
- `Universe::commits_after`／`squash`：同上，base 不在這條鏈上就 `not an ancestor`
- `gc::mark`：HEAD 入隊，CAS 邊＋`previous_commit`
- `rollback`：把 HEAD **寫成**呼叫者給的那一個 digest（仍是一個）

鑄造側同一假設：`savepoint::record_commit` 在沒有顯式 covering 時 `tips.sort()` 然後 **`tips.into_iter().next()`**——複數 tip 只收字典序最小的那一個當 `parents:`。

三輪 ×20：`Commit successful` 最多 13／20，`oo log` **三輪皆 2**（base＋一個倖存者）。其餘成功提交的物件在 gc 前還在磁碟上，只是進不了這條鏈。

### 結論

不要清單。不變式：

> **凡把「現在」讀成單一 commit digest、並沿一條 `previous_commit` 走訪「歷史」者，當磁碟上同時存在兩個由成功提交鑄出的 tip 時，只會回答其中一條；另一條從它的答案裡消失，即使物件還在。**

判準是答案的**完備性**（漏鏈），不是四個函式名。`record_commit` 的 `next()` 是同一假設在鑄造端的對偶：凡把 tip **集合**收成**一個** covering parent 者，H₁ 也只會接上其中一個。

乙若把 HEAD 改成 tip 集合，所有滿足上一句的讀者都要改；甲／丙保住「HEAD 是一個」，這句不變式可以留。

---

## Q5 — 甲／乙／丙同一把尺

不選。價只報結構，行數是量級。

| | 甲 收斂 | 乙 分叉 | 丙 拒絕 |
| :-- | :-- | :-- | :-- |
| **(a) 檔／量級** | `storage.rs` `set_head` 改成讀－比－寫（compare-and-swap）；`universe.rs` `commit` 失敗則重讀 HEAD＋重 fold＋重試。約百行。 | HEAD 檔從一行變成集合；`log`／`gc::mark`／`squash`／`rollback`／`record_commit` 的 covering 全部從「一個」改成「一組」。數百行，跨 `storage`／`savepoint`／`gc`／`universe`／CLI。 | `run_commit`／`commit` 開頭重讀 HEAD，與 load 時的 digest 不同就 `bail`，具名。數十行。 |
| **(b) 磁碟表示** | HEAD **仍一行**。不必新 layout。重試若要保住輸家的注入，**必須縮小 `clear` 的範圍**（只刪自己 fold 過的成員）——那是工作集契約，不是 HEAD 形，但仍是行為變。 | **要。** `.oo/HEAD` 不再是單一 digest ⟹ 依 `REAL_02` §5.1.1 這是佈局。連三版重啟之後這是第四次 90 天。舊引擎必須誠實拒絕 `layout=6`，不得報 corrupt。 | HEAD 形不變，**不必 layout**。 |
| **(c) 身分** | 不動 `31745ef0…`／3 物件／`7038e250…`。 | 同左。HEAD 不進 CAID。 | 同左。 |
| **(d) 探針／conformance** | 現有 log／G1 身分應仍綠。沒有並行提交探針（本卡規定偵察不寫）。conformance 162 向量不碰 HEAD 檔。 | Q-015 的「log 從 HEAD 進、一條祖先」會變成多 tip；G5／H₁ 若仍假設單一 covering parent 要重寫（那是驗收方的探針，本偵察不改）。conformance 大概仍綠。 | 現有「空工作集 → Nothing to commit」仍綠；需要一句**新的**具名拒絕，不得與 Q3 那句混用。 |

三者都**不會自動修好 Q1 的值遺失**，除非同時改 `clear`：甲的重試若撞上已被清空的目錄，收斂成空工作集；乙的兩個提交各自 fold 的仍是自己 load 到的快照；丙見 Q6。

---

## Q6 — 丙的隱藏成本

### 量

兩個注入（分母 2）→ 贏家 `commit` rc=0 → `injections/` **0** → 立刻再 `commit`：

```text
Error: Nothing to commit
```

rc=1。來源檔 `b.n` 仍在：`evolve b.n` rc=0，`status` 顯示 `b: 2`，再 commit rc=0。

三輪 ×20 結束時注入剩餘皆 **0／（該輪曾 mint 的成員）**。所有 `Nothing to commit` 的工作者都沒有工作集可重跑。

今天 `clear` 在 `set_head`＋鑄 ○ **之後**無條件刪整個目錄。丙若只在閘上拒絕、不碰 `clear`，**贏家仍然會把輸家的成員刪掉**。拒絕發生在輸家行程裡，刪除發生在贏家行程裡。

### 結論

操作者要重跑的不是 `commit`，是 **從來源再 evolve**。來源不在（只靠工作集、或 stdin、或已被覆蓋的檔）就重跑不了——與 Q1 遺失是同一筆帳。丙的代碼價低，**操作者價等於「請你把剛被我清掉的東西再寫一次」**。若要把丙做成真的便宜，拒絕必須發生在**任何一方 `clear` 之前**，且 `clear` 只能刪自己 fold 過的成員；那就不再是「數十行的閘」，而是和工作集契約綁在一起。認真報了：丙可以仍是最誠實的失敗模式，但「最便宜」只在不計工作集時成立。

---

## Q7 — 懸空的 `commit:`

### 量

v0.44.0，三輪 ×20（分母見上表）：

| 輪 | ○ 總數 | 帶 `commit:` 的 ○ | gc 前懸空 | gc 後懸空 | gc 後 `log` | `status` |
| --: | --: | --: | --: | --: | --: | --: |
| 1 | 29 | 8 | **0** | **6／8** | 2 | rc=0 |
| 2 | 32 | 11 | **0** | **9／11** | 2 | rc=0 |
| 3 | 35 | 14 | **0** | **12／14** | 2 | rc=0 |

gc 前物件都在：那些「成功但進不了 log」的提交還躺在 `objects/`。`gc::mark` 從 HEAD 走 `previous_commit`，sweep 其餘。○ 在 `savepoints/`，不是 CAS，gc 不刪 ○，於是註記留下、對象走了。

`gc.rs` 檔頭逐字：根是 HEAD → 前輩＋每顆 commit 的 root 樹。**沒有**「掃描 `savepoints/` 的 `commit:`」。

抽一顆 gc 後的註記（第 1 輪）：circle `a282f581…` 寫著 `commit: 0459e2b2…`，對應 `objects/sha256/04/59e2b2…` 不存在。`oo log`／`oo status` 仍 rc=0。

### 結論

**今天這是「○ 上的提交註記不是 GC 根」，不是「gc 漏走一條它的註解宣稱在走的邊」。** 前者若要改成「凡 ○ 指名的提交必須仍可解」，需要一則裁定（加根 ⟹ 那些競態成功的提交永遠收不掉；不加根 ⟹ 懸空是允許的）。後者不需要裁定，因為現行 mark 沒有把那條邊算進去。v0.42.0 的 18–19 顆與今天 6–12 顆是同一形，只是成功提交的數量從「幾乎全成功」變成「一半失敗」，所以懸空少一些。

---

## 工單哪裡問錯了

1. **半 A／半 B 切在「訊息 vs HEAD 拓撲」，漏了第三軸：`clear` 的範圍。** Q1 的值遺失、Q6 的「重跑不了」、甚至 Q3 的假 `Nothing to commit`，主詞都是「一次 commit 刪掉不是這次 fold 讀到的成員」。甲／乙／丙只裁定歷史怎麼長，不自動關上這扇窗。
2. **Q2 的「是不是 Q-016a／Q-040 造成的」容易讀成歸咎。** 造成 ENOENT 的是 `clear` 與 `load_all` 的 TOCTOU；Q-016a 只是停止吞掉它。歸屬寫「第一次被看見」是對的，請不要寫成那兩弧引入了競態。
3. **Q4 先禁止清單又點了四個名字。** 四個都只是同一不變式的實例；漏掉的是鑄造端 `record_commit` 的 `tips.next()`，它同樣假設唯一 tip。
4. **brief §1 的「約半數失敗」在本輪複驗裡隨調度跳動**（7／10／13 成功／20），不要把 10/10 寫進完成條件。不變的是：`log`＝2，注入收尾＝0，值**可以**丟。

半 A 仍建議可先開工（os error 2 與假的 Nothing to commit 已被現行 MUST 管著）。半 B 未裁。**不要在半 B 的工單裡假裝值遺失會順便修好。**
