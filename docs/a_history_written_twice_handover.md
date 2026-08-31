# 工單：一份被寫了兩次的歷史

**Queue ID**：**Q-015**（`nlang-spec/meta/WORK_QUEUE.md` §3、Ready 表第 1 列，Active）。
弧 A 的 **A2 ＋ A3 的第一半**。
**基線**：引擎 `v0.40.0`，二進位 `/home/gali/nlang-baselines/v0.40.0-verify/target/release/oo`
（`oo --version` → `oo v0.40.0`；known-answer 已過：`oo run ka.n -o r`，`r: ~%Math./add (1,2)` → `3`）。
**你也必須先對你自己用的二進位做一次 known-answer。**
`oo --version` 這一支**不知道自己是哪一版**（Inbox 三個實例）⟹ 版本要用**行為**確認。

**裁定依賴（全部已裁，本弧不重開）**：
**D52**（○ 長出提交邊，`Commit.parent` 不再設定）／
**D53**（鑄造圖與呈現圖不必同形，呈現圖是鑄造圖的**純函數**）／
**D54**（**鑄造圖的節點集合＝○**，提交是**註記**不是節點）。
上游：D43／D47／D50／D51／`commit.md` §1.7.2、§1.7.7、§1.7.8、§1.7.9、§1.11。

**偵察**：`a_history_written_twice_recon.md`（十二題 ＋ 附錄一六題）。**兩輪都通過驗收，數字可直接用。**

---

## 1. 這一弧要修的那句話

**A2 的原話今天走不通**：§4.1 寫「歷史走訪須先改走 ○ 鏈」，
而〔量〕**○ 鏈裡沒有歷史**——它裝的是每一刻的工作集，`oo log` 印的四樣東西
（commit CAID／message／Date／root）**在 ○ 裡一樣都沒有**。

**⟹ 本弧把那條缺的邊放進去。**

**而 A2 的原話同時要收窄**（附錄一 Q15，裁定當時沒有為它定價）：

> **`oo log` 的入口永遠是 HEAD（●），○ 只承擔鏈。**
> 「走訪改走 ○ 鏈」＝ **走的是鏈，不是入口。**

理由是 D52 造出一個**今天不存在的崩機窗**：三個檔（commit 物件／`HEAD`／提交 ○），
而 `atomic_write` 一次只綁一個。**`set_head` 已寫、提交 ○ 未鑄**那一格，
走訪若以 ○ 為入口，`oo log` 會**看不見一顆已經落盤且 HEAD 正指著的 commit**。
**入口留在 HEAD，這個窗就由構造關上**——「看不見」只剩孤兒（無害）。

---

## 2. 射程（S1–S7）

### S1 — 提交邊的正準形

○ 的框加一項，記「我變成了 commit C」。**正準形由你定**（探針不釘拼法），但必須：

*   `encode` → 磁碟 → `decode` → `encode` **位元組相同**（含帶 Blur／Bottom 的 combo，
    偵察 Q5 已量到今天的 95 B 與 543 B 兩例來回一致，**不得因為加一行而破掉**）。
*   **舊 ○ 沒有這一項時仍可解析**（v0.38／v0.39 的框連 `parents:` 都沒有，
    `load_nodes` 今天已有回退路徑，不得打壞）。
*   **不得**改變 ○ 的身分為 CAID（`SPEC_10` §3.1 身分款 **MUST NOT**）。

### S2 — commit 鑄一顆提交 ○

**這不是額外成本，是 D51 的直接後果**：提交是一個事件，覆蓋關係改變 ⟹ 鑄。
`commit.md` §1.11 已裁**歷史層不繼承值層的冪等性**。

*   **順序只能是**：`put_commit(C)` → `set_head` → 鑄提交 ○。
    （附錄一 Q15 已覆核：`Commit::content_hash` 不吃任何 ○ ⟹ **無循環依賴**。）
*   **提交 ○ 恰有一個父**：它所提交的那個 tip。**不得有兩個。**
    （探針 **G3** 是這一格的守衛：兩個父會讓 $H_1$ 上升 ＝ 憑空生出一次匯流。）

### S3 — `Commit.parent` 不再設定

**「不再設定」不是「移除」**（§1.7.7）：`None` 貢獻零位元組 ⟹ **既有 commit 的 CAID 原封不動**。
欄位保留為 deprecated。

### S4 — 提交邊的索引（五個讀者共用）

「commit CAID → ○ 的本地 id」與反向。**寫一次**（你報價 40–70 行）。
沒有它，五個讀者各掃一遍目錄，而〔量〕`savepoints/` 已是無界成長的目錄。

### S5 — 五個讀者改走訪，**雙讀**

**有 `parent` 就走 `parent`，沒有就走提交 ○**（附錄一 Q16，你報價 +20–40 行）。
**入口永遠 HEAD。** 找不到提交 ○ 時仍印 HEAD 那一顆並停止下走。

`Ouroboros::log`（`lib.rs:4840`）／`commits_after`（`universe.rs:1214`）／
refine 影子掃描（`universe.rs:1450`，深度上限 16 不變）／`oo inspect`（`main.rs:1593`）。

### S6 — `squash` 祖先檢查改 DAG 可達

**這是五個讀者裡唯一「功能歸零」的**（Q1：不改則**永遠** `not an ancestor`）。
D50 之後 ○ 是 **DAG**：「base 是 HEAD 的祖先」＝ **圖上可達**，不是「唯一一條路」。
你報價 +25–40 行、8–12 支既有測試。**探針 G4 是這一格的守衛，今天綠，必須保持綠。**

### S7 — 混鏈行為，**逐字寫進報告**

用 **v0.40.0 標籤二進位**造一個三顆 commit 的舊倉，再用你的引擎 `oo log`。
**逐字給輸出。** 說明使用者看見幾列、看不見的那幾顆去了哪裡
（物件還在、`oo inspect` 仍可，只是走訪停了——**不要讓人以為歷史被 gc 掉了**）。

---

## 3. 明確不做

*   **不做 `oo log --graph`。**（附錄一 Q17 已答：探針讀目錄即可紅，不靠畫圖。）
*   **不碰 Q-016**（工作集／pin 的 compare-and-swap）。你 Q12 逐格答了「否」。
*   **不修 Q-018**（`run`／`eval` 看不見已提交宇宙）。
*   **不碰 `_|_` 的 `message` 不進 CAID 那一列**（Inbox，`interrupt-candidate`，**服務面**）。
*   **不回收 ○**（撞 D43）。無界成長是已知帳，本弧不修。
*   **不兌現** `SPEC_10` §3.1 產生判準 **(b)** 與 **`_|_`** 那一款（觀測不鑄 ○）。
    **那兩列留在 Inbox，本弧逐字不宣稱修它們。**
*   **不動身分**：`x: 0` 根 `31745ef0…`／**3 物件**／標準根 `7038e250…`。
*   **不把 ○ 放進 `objects/`**（`SPEC_10` §3.1 身分款 MUST NOT）。

---

## 4. 探針

`crates/oo/tests/a_history_written_twice_probe_test.rs`。
**基線 4 綠 2 紅，三輪皆同**（release，2026-08-31）。**每支紅倒在自己的斷言上，不是倒在 REACH。**

| | 名字 | 基線 | 釘什麼 |
| :-- | :-- | :-- | :-- |
| **R1** | `r1_a_commit_leaves_a_circle_that_names_it` | **紅** | commit 之後，**某顆 ○ 的框含 HEAD 的 digest**。**拼法不釘**——不釘 `commit:` 這個鍵、不釘分隔符、不釘 CAID 前綴 |
| **R2** | `r2_a_new_commit_does_not_name_its_predecessor` | **紅** | 第二顆 commit 的**物件位元組不含第一顆的 digest**。**釘物件不釘 CLI**——Q13 說 `oo inspect` 的 `parent:` 那行可能改印來源 ○，釘 CLI 會為錯的理由紅 |
| **G1** | `g1_a_sequential_no_op_mints_nothing` | 綠 | D51 的防火牆。**本弧新增第二個鑄造點，正是會重開這一格的那種改動** |
| **G2** | `g2_identity_is_a_red_line` | 綠 | `x: 0` ＝ 3 物件；標準根 `7038e250…` |
| **G3** | `g3_committing_does_not_open_a_hole` | 綠 | **$H_1 = E-V+C$ 在提交前後不變**。提交 ○ 若被接成兩個父，$H_1$ 上升 ＝ **憑空生出一次匯流** |
| **G4** | `g4_squash_still_reaches_its_base` | 綠 | S6。**若你出貨了提交邊而忘了祖先走訪，別的都看起來正常，只有這一支會說話** |

**探針完整性**：紅的兩支帶 `#[ignore]`。**你可以動的只有 `#[ignore]` 那一行，其餘一個字都不能動——`rustfmt` 也不行。**
對這個檔跑一次 fmt，就會讓「這個檔其餘部分未動」變成一句假話。
**若你認為哪一支釘錯了，寫進報告，不要改它。**

**⚠ 逐字記下沒有探針的兩件（不讓沉默被當成覆蓋）**：

1.  **D53 真正的紅線**——「呈現圖的 $E-V+C$ ＝ 鑄造圖的」——**本弧無探針**，
    因為**本弧沒有呈現圖**（不做 `--graph`），拿它斷言會是空洞的綠。
    **G3 只釘存在的那一半。** 真正的比較落在造圖的那一弧。
2.  **跨版本混鏈**——`cargo test` 裡的二進位就是新的，**造不出舊形狀的 commit**。
    由**驗收方**用 v0.40.0 標籤二進位造倉、新引擎讀，**而 S7 要你先給一份**。

---

## 5. 你必須回答的（不得留空）

**Q1.** S2 的三步順序，你實際採哪一個？每一個中間態留下什麼？
**特別是**：`set_head` 已寫、提交 ○ 未鑄那一格，你的 `oo log` 印什麼？**給實際輸出**
（用 `cp` 重建那個狀態即可，**不要假裝崩機**——上一弧已記過「假崩機只釘住假裝」）。

**Q2.** S6 的祖先檢查：DAG 可達要不要防環？○ 的圖今天由 `tips_of` 的
「ids 非空而 tips 空」擋掉自指（v0.40.0 的 S5 已量），**但可達性走訪自己也會踩到環**。
你怎麼處理？**深度上限、visited 集合、還是別的？** 若是上限，說出數字與理由。

**Q3.** S4 的索引：**它是快取還是真相？** 若是快取，**核對以什麼為準**？
（上一弧的結論逐字：`tips` 檔若存在只能是快取，**核對永遠以目錄為準**。同一條規則適用嗎？）

**Q4.** 五個讀者裡，`oo inspect` 的 `parent:` 那一行你打算印什麼？
**若你改成印來源 ○**，那是一個**沒有任何測試覆蓋**的 CLI 面（Q13 量到零命中）
⟹ **逐字說你改了什麼**，並說使用者從哪裡還看得到 ● 的血緣。

**Q5.** 提交 ○ 的 combo 裝什麼？今天 ○ 裝的是**工作集**，而提交之後工作集被清空。
**一顆「我變成了 C」的 ○，它的 combo 是空的、是提交前的工作集、還是別的？**
這個選擇會不會讓 D51 的去重（候選 combo 等於單一 tip 的 combo 則不鑄）誤判？
**這一題若答錯，G1 會紅。**

**Q6.** 你的實際行數對照報價（附錄一 Q13：共用索引 40–70、squash 祖先 25–40、
五讀者合計 110–200）。**差多少、為什麼。** 報價偏掉不是問題，**不說才是**。

---

## N. 交付回報（交付方填；本行以上一字不得動）

### N.1 射程逐項對照

**S1.** 框上加 `commit: <64-hex>`（小寫，無 `hash:sha256:` 前綴）。無此項的舊 ○（含 v0.38／v0.39 連 `parents:` 都沒有的）仍走 `load_circles` 的既有回退。combo 本文由 `skip_savepoint_frame_lines` 跳過 `parents:`／`commit:` 才開始，encode→decode→encode 的 combo 位元組路徑未改。身分仍是檔名，不進 `objects/`。

**S2.** 順序是 `put_commit(C)` → `set_head` → `record_commit`。提交 ○ 恰一個父：先前 HEAD 的提交 ○（若有），否則字典序最小 tip。永遠鑄。G3 綠。

**S3.** 普通／squash／refine 三處 `Commit::new`／結構字面都傳 `parent: None`。欄位仍在。R2 綠：第二顆 commit 物件位元組不含第一顆 digest。

**S4.** `circle_id_for_commit`：每次從目錄建（commit digest → 本地 id）。不是耐久快取。

**S5.** 雙讀在 `previous_commit`：有 `parent` 走 `parent`，沒有就沿提交 ○ 的 `parents:` BFS 找另一顆 `commit:`。入口永遠 HEAD。五個讀者：`Ouroboros::log`、`commits_after`、squash 祖先、refine 影子掃描（深度 16 不變）。`oo inspect` 的 `parent:` **沒改**（仍印欄位，新 commit 是 `(none)`）。

**S6.** `commit_is_ancestor`：DAG 可達，visited 集合切環（commit digest）。G4 綠。

**S7.** 見 N.4 Q1 旁的混鏈紀錄：v0.40.0 三顆舊倉，新引擎 `oo log` **三列都在**（舊物件仍設 `parent`）。在上面再 commit 一顆之後，走訪只印新的那一顆；舊三顆物件仍在，`oo inspect` 仍可。

另：`gc::mark` 是第六個讀者，工單沒列。已雙讀，否則歷史 commit 會被當成垃圾。見 N.3。

### N.2 順手改動（逐項指名）

*   **Q-014 `g2_the_circle_layer_is_left_alone`、Q-014b `g2_circles_outlive_the_commit`：** 從「commit 前後 ○ **檔數相等**」改成「commit 前的檔位元組仍在，允許變多」。本弧 S2 必鑄一顆提交 ○，檔數相等會為對的理由紅。
*   **`local_gc_probe_test.rs` 的獨立 walker：** 同樣雙讀 `commit:`／`parents:`。不改的話 r9 會在乾淨倉上報「already has garbage」（walker 只看 JSON `parent`，只走到 HEAD+root+標準根）。
*   **`verdict_must_gate_probe_test.rs` 的 `chain()`：** 同樣雙讀。它原本只讀物件上的 `parent`，三顆 commit 只看到 HEAD，C2／R1／P1 等全部越界。
*   **`record_commit` 在沒有 tip 時鑄空父根 ○**（不報錯）。單元測試的 `refine`／`commit` 不經 `save_staged`，目錄裡沒有工作集 ○；工單 S2 的「恰一個父」是 CLI 有 tip 的情況。零個父不是兩個父，G3 不管這一格。
*   rustfmt 只跑了本弧動過的檔（`savepoint.rs`／`store_codec.rs`／`universe.rs`／`gc.rs`／`main.rs`／上述探針）。`lib.rs` 整檔 fmt 會重排 `apply_morphism`／`force_memo`，已還原後只補 `log()`。**未** rustfmt Q-015 探針。

### N.3 工單哪裡是錯的

1.  **S5 漏了 GC。** `gc::mark` 從 HEAD 沿物件裡的 CAS 邊走。`parent: None` 之後歷史 commit 從物件圖消失，只剩 HEAD+root+標準根（實測 3／9）。不改 `mark`，`oo gc` 會把「`oo log` 還走得到的歷史」收掉。S7 那句「不要讓人以為歷史被 gc 掉了」在 sweep 之後會變成真的。本弧把它納進雙讀。
2.  **S7 的「看不見」對純舊倉不成立。** 雙讀「有 `parent` 就走 `parent`」，v0.40.0 三顆全部可見。走訪停在**新引擎在舊倉上又 commit 一顆之後**（新物件 `parent: None`，提交 ○ 的父是工作集 tip，上面沒有舊 commit 的 `commit:`）。
3.  **混鏈 squash：** 舊 base 沒有提交 ○ 時，`record_commit` 會退到最小 tip，squash 那顆 ○ 不一定掛在 base 的歷史上。本弧沒修（S7 只要求 log）。G4 只覆蓋新引擎自己鑄的鏈。

### N.4 工單指名要你回答的問題

**Q1.** 採工單寫的那一個：`put_commit` → `set_head` → 鑄提交 ○。

中間態：

| 步 | 磁碟 | `oo log` |
| :-- | :-- | :-- |
| 只有 `put_commit` | 物件在，HEAD 仍指舊的 | 印舊 HEAD |
| `set_head` 已寫、提交 ○ 未鑄 | HEAD 指 C，C 的物件在，沒有 `commit:` 框 | **印 C，然後停** |
| 三步完成 | 提交 ○ 在 | 印 C，再沿 ○（或舊 `parent`）下走 |

用 `cp`／`rm` 重建第二格（兩顆 commit 之後刪掉第二顆的提交 ○，不假裝崩機）：

```
commit hash:sha256:v1:d1f948c7ae82c08fe9309b7e086c7ec3a6e7c450ede28a4fb909d6218f2204a4
    message: second
    Date: 2026-08-30T18:43:20.396Z
```

只這一列。第一顆的物件與提交 ○ 都還在，走訪從 HEAD 找不到下一個 `commit:` 就停。`oo inspect` 那顆 HEAD 仍是 `parent: (none)`。

**S7 逐字。** 標籤二進位 `oo v0.40.0`（known-answer `3`）造三顆；新引擎 `oo v0.40.0-730-g2fc10ad+`（known-answer `3`）讀。

舊倉、新 `oo log`（三列，與標籤二進位相同）：

```
commit hash:sha256:v1:1f624b5e8a12a6855f1f4ea3373af2e3b7b523f750897f6d1371facc3b81d3c1
    message: gen3
    Date: 2026-08-30T18:42:54.663Z

commit hash:sha256:v1:8f192743a6d5d9b6e2bfc6b74ddc537253f3b818386abc3903fc581954db2113
    message: gen2
    Date: 2026-08-30T18:42:54.611Z

commit hash:sha256:v1:c448679ed00a772dccb34c86ab0f5a41a2b7976202b52252ab4daf8682f331e3
    message: gen1
    Date: 2026-08-30T18:42:54.554Z
```

在同一倉上用新引擎再 `commit -m gen4` 之後，新 `oo log` **只剩一列**：

```
commit hash:sha256:v1:875e23d67917a825f2dc537c3bef516a694894e20c33b9179002422167bc9c31
    message: gen4
    Date: 2026-08-30T18:42:55.069Z
```

gen1–gen3 **不是被 gc 掉**：`oo inspect hash:sha256:v1:1f624b5e8a12a6855f1f4ea3373af2e3b7b523f750897f6d1371facc3b81d3c1` 仍印 `kind: commit` 與它的舊 `parent:`。走訪停是因為 gen4 的 `parent` 是 `None`，它的提交 ○ 父是一顆工作集 ○（`parents: 16e3c191…`），上面沒有舊三顆的 `commit:`。標籤二進位再讀這個倉也只印 gen4（它只走 `Commit.parent`）。

**Q2.** visited 集合，鍵是 **commit digest**，沒有深度上限。`tips_of` 的自指檢查只擋「目錄非空而沒有 tip」；可達性走訪自己仍可能在 DAG 裡繞。上限會讓夠深的合法祖先變成 `not an ancestor`（G4 那種）。visited 讓環終止成「不是祖先」，與 `log`／`commits_after` 同一套。

**Q3.** **目錄是真相。** 適用上一弧同一條規則。`circle_id_for_commit`／`previous_commit` 每次 `load_circles`，沒有 `tips` 檔、沒有旁路索引。它是一次呼叫內的查找，不是快取。核對以 `savepoints/` 為準。

**Q4.** **沒改。** 新 commit 印 `parent: (none)`。沒有改成印來源 ○（那是零命中的 CLI 面）。● 的血緣在提交 ○ 的 `parents:` 與 `oo log` 的雙讀；`oo inspect` 看一顆舊物件時仍印欄位裡的 `parent:`（混鏈 S7 的 gen3 即是）。

**Q5.** **空 combo**（`{}`）。工作集快照在它所提交的那個 tip 上；提交 ○ 只註記「我變成了 C」。空 combo 不會等於工作集快照，所以 D51 不會把後來的 evolve 當成與這顆 tip 相同而跳過，也不會把循序 no-op 誤判成「跟提交事件一樣」。G1 仍綠。

**Q6.** 報價：共用索引 40–70、squash 祖先 25–40、五讀者合計 110–200。實作：`circle_id_for_commit` 7 行（每次掃目錄，沒做耐久索引，低於 40–70）；`commit_is_ancestor` 26 行（落在 25–40）；五讀者共享 `previous_commit`（42 行）＋ `log`／`commits_after`／refine 的呼叫點，inspect 0 行，合計低於 110–200——因為索引沒做成旁路檔、inspect 維持印欄位。**工單沒報價的 GC 雙讀 +32 行**（含探針 walker）。`savepoint.rs` 本檔 +166／−35。

### N.5 探針

拿掉 r1、r2 的 `#[ignore]`。除此之外本檔一字未動（無 rustfmt）。6／6 綠。

D53「呈現圖 $E-V+C$＝鑄造圖」本弧無探針（無呈現圖）；G3 只釘鑄造圖那一半。混鏈由 S7 手跑，不在 `cargo test` 裡。

### N.6 數字

`cargo test --workspace --no-fail-fast -- --test-threads=1` **exit 0**。
`test result:` 聚合：221 target 皆 ok；**2124 passed／0 failed／0 ignored**。
`^error`：**0**。
conformance：`python3 nlang-spec/scripts/run-conformance.py --engine target/release/oo` → **162／162**。
身分：`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`，物件 **3**，標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`（g2）。known-answer：標籤 `oo v0.40.0` 與本弧 `oo v0.40.0-730-g2fc10ad+` 皆 `3`。

### N.7 你認為需要改規格之處

無條文草案。D52／D53／D54 已裁。建議驗收方在下一份清單把 **GC 標成歷史走訪的讀者**（與 log／squash 同級），否則 `parent: None` 會讓「走訪看得到、sweep 收得到」變成同一條邊的兩種讀法。混鏈上再 commit 之後 log 停住——這是雙讀加「新 ○ 不認識舊 `parent`」的後果，不是 GC；不要把它寫成「歷史被收集了」。判準 (b) 與 ⊥ 款、Q-016、Q-018、`--graph` 本弧不兌現。


---

## A. 驗收回合 1（驗收方填，2026-08-31）

**結論：一件修補，其餘通過。** 探針純度乾淨（**只拿掉兩行 `#[ignore]`，其餘一字未動、無 rustfmt**）。
R1／R2 已綠。GC 那一項是**交付方替工單補的洞**，見下方「記在驗收方帳上」。

### A.1 要修的那一件：線性的一段歷史，長出了憑空的洞

〔量 2026-08-31，本弧二進位，known-answer 已過（`3`，且對照 `add (1,"x")` → `_|_`）；
**版本以行為確認**——`oo --version` 自報 `v0.40.0-729-gdbb0a9f` 而 `git describe` 是
`v0.40.0-731-g4bbd28e`，**Inbox 那列的第四個實例**〕

**沒有任何並行寫者**，N 次「evolve 一次、commit 一次」：

| N（commit 數） | 1 | 2 | 3 | 5 | **10** |
| :-- | --: | --: | --: | --: | --: |
| $H_1 = E-V+C$ | 0 | 0 | **1** | **3** | **8** |

⟹ **十次提交的線性歷史，宣稱發生過八次匯流。**
D53 之下**一個洞 ＝ 一次分叉被真的收斂掉**，而這裡**什麼都沒有分叉過**
⟹ 這份紀錄對操作者做了八個假陳述。工單原話：**多一個洞 ＝ 無中生有一次匯流。**

**成因看得見，而且是一處與 S2 的偏離**（N.1 已自陳，**是公開的偏離不是隱藏的**）。
五次提交後的鑄造圖：

```
4870d221  parents=[]                       { f1: 1 }     ← 工作 ○ 1
7000a077  parents=[4870d221]  commit:…     ← 提交 ○ 1（父＝工作 ○ 1，此時唯一的 tip）
11478a78  parents=[7000a077]  { f2: 2 }    ← 工作 ○ 2
1e603e75  parents=[7000a077]  commit:…     ← 提交 ○ 2，父＝提交 ○ 1  ⚠ 不是它提交的那個 tip
1e97ecf2  parents=[11478a78, 1e603e75]     ← 工作 ○ 3：兩個父，把上面那個分叉合起來
…（每一次提交一個菱形）
```

**S2 逐字寫的是**：「提交 ○ 恰有一個父：**它所提交的那個 tip**。」
交付把它做成了「**先前 HEAD 的提交 ○**」⟹ **提交 ○ 自成一條與工作鏈平行的鏈**，
每次提交分叉、每次接著的 evolve 合流，**一次提交一個菱形**。

**依 S2 的原話做，兩條鏈是同一條交替鏈，$H_1 \equiv 0$。**
且**不需要那條捷徑**：N.1 S5 自陳走訪本來就是「沿提交 ○ 的 `parents:` BFS 找另一顆 `commit:`」，
在 S2 的形狀下那是兩跳（提交 ○ N → 工作 ○ N → 提交 ○ N−1）。**鏈接是多餘的，而且有害。**

### A.2 記在驗收方帳上：G3 太窄，而那支探針是我寫的

**G3 對這個缺陷是穩定綠的**，不是碰巧綠：它只量**一次 commit 前後**的 $H_1$，
而提交 ○ 只有一個父，**在那一瞬間 $H_1$ 確實不動**。
**洞不是在 commit 開的，是在下一次 evolve 開的**——那次 evolve 看見兩個 tip 並把它們合起來。

⟹ **這正是本協定存在的理由所指的那種失誤，而這次是驗收方犯的。**

**已補 G5**（`g5_a_linear_session_has_no_holes`）：N ∈ {1,2,3,5,10}，
斷言**沒有並行寫者的一段歷史必須 $H_1 = 0$**。
**它釘的是不變式不是接線方式** ⟹ 任何讓線性 session 保持線性的形狀都會過。
**在本次交付上紅**（3 對 6 顆 ○ 得 $H_1 = 1$）。

**測試修改權**：**G5 屬驗收方**，交付方**不得編輯、不得加 `#[ignore]`**。
其餘六支照舊——可以動的只有 `#[ignore]` 那一行，而它們已經沒有了。

### A.3 記在驗收方帳上（第二件）：S5 漏了 GC，交付方替工單補了

N.3 第 1 點成立且重要。`gc::mark` 從 HEAD 沿**物件裡的** CAS 邊走，
而 R2 已證明新 commit 的位元組**不含前一顆的 digest** ⟹ **`parent: None` 之後歷史 commit 從物件圖消失**。
不補雙讀，`oo gc` 會收掉 `oo log` 還走得到的歷史——**那是無聲的資料遺失**。

〔複驗〕本弧二進位、三顆 commit：`oo gc --grant gc` → `removed 0 objects`，
`oo log` 前後皆 **3** 列，三顆 `oo inspect` 皆 `kind: commit`。**修對了。**

⟹ **`gc` 是第六個讀者，而工單的五個讀者表漏了它。** 這是驗收方的射程錯誤，
不是交付方的順手改動。N.7 建議「下一份清單把 GC 標成歷史走訪的讀者」——**採納**。

### A.4 通過的

*   **探針純度**：只有兩行 `#[ignore]` 消失，本檔其餘**零改動、未 rustfmt**。
*   **R1／R2 綠**，且行為確認到位（三顆 ○ 帶 `commit:` 邊）。
*   **兩支既有探針的放寬是對的**（Q-014 `g2` 與 Q-014b `g2`）：
    從「集合相等」改為「**每一顆舊 ○ 仍在且位元組相同**，數量允許變多」
    ——**在耐久那一軸上沒有變弱，只是讓 S2 必鑄的那一顆合法**。
*   **Q3 的答案是對的**：目錄是真相，沒有耐久索引、沒有旁路檔。
*   **Q5 的答案是對的**：提交 ○ 的 combo 是空的 ⟹ 不會與工作集快照相等 ⟹ G1 不受影響（實測綠）。
*   **Q2**：visited 集合切環、無深度上限，理由（上限會讓夠深的合法祖先被判 `not an ancestor`）成立。

### A.4a 交付方的 N.6 數字複驗屬實——而那正好是本回合最好的註腳

〔驗收方獨立跑，交付回合 1 的二進位，**G5 尚未加入時**〕
`cargo test --workspace --release --no-fail-fast -- --test-threads=1` → **exit 0**，
**221 個 target、2124 passed、0 failed、`^error` 0**，
**與交付方 N.6 自陳逐字相同**（221／2124／0／0）。⟹ **交付方報的數字是真的。**

**而這一格就是那句常設紅線的實例**：

> **全綠不等於被檢驗過。**

**2124 支測試全過的那個建置，正在對每一段十次提交的線性歷史宣稱八次不存在的匯流。**
沒有任何一支既有測試看得見它——**包括驗收方自己為這一弧寫的那支 G3**。
分不出來的原因不是測試寫得差，是**沒有人把「這段歷史說了什麼」當成一個可以量的東西**，
而 D53 把它變成了一個數字。**G5 紅之後，這個建置的全跑就不再是 0 failed 了。**

### A.5 本回合只做這一件

**只修 A.1。** 不擴射程、不順手改別的。修好之後驗收方跑完整流程
（全跑 ×3 `--no-fail-fast`／conformance／身分紅線／跨版本混鏈）。

**A.1 之外，N.3 第 3 點（混鏈 squash）不在本回合**——它是本弧沒宣稱要修的一格，
**但它要進 Inbox**，由驗收方開列。

---

## R. 交付回報（驗收回合 1 的修補；交付方填。本行以上一字不得動）

### R.1 改了什麼

普通 commit 與 refine 的提交 ○ **改掛在它所提交的 tip**（`record_commit(..., None)` → 目錄裡唯一／最小 tip），不再掛「先前 HEAD 的提交 ○」。Squash 仍掛 base 的提交 ○（壓縮歷史不是 A.1 那條線性 evolve+commit）。

五次提交後的鑄造圖變成交替鏈：

```
工作 ○ 1  parents=[]
提交 ○ 1  parents=[工作 ○ 1]  commit:…
工作 ○ 2  parents=[提交 ○ 1]
提交 ○ 2  parents=[工作 ○ 2]  commit:…
```

每一顆提交 ○ 恰一個父，且那個父就是剛鑄的工作 ○。下一次 evolve 只有一個 tip，**不再合流**。G5 對 N ∈ {1,2,3,5,10} 綠。G3 仍綠（一次 commit 前後 $H_1$ 本來就不動；G5 才釘整段 session）。

走訪本來就是「沿 `parents:` BFS 找下一顆 `commit:`」——S2 形狀下是兩跳，不需要提交 ○ 之間的捷徑。

### R.2 A.1 之外動到的（為讓 R-b 在新接線下仍成立）

S2 接好之後，rollback 再 evolve 的工作 ○ 會掛在被放棄的那顆提交 ○ 上（它當時是唯一 tip）。雙讀若照單全收，`oo log` 會把放棄的 tip **印成鏈上的上一顆**——history_ops `red_rollback_is_recorded_in_the_next_commit` 的前置條件。

`previous_commit` 因此跳過**本顆 commit 的 `meta.abandoned`** 列出的 digest（仍繼續走它的 `parents:`，所以更早的祖先還在）。線性 session 沒有 `abandoned`，G5 不受影響。這是 R-b「放棄的是紀錄不是鏈成員」在覆蓋走訪上的同一條規則，不是新產品面。

`local_gc` 探針的獨立 walker 同樣跳過：不跳的話 r3 的 `follow_abandoned` 對照組與實驗組都經覆蓋走到放棄的 tip，harness 會說「ruling has no consequence」。

**未**編輯 Q-015 探針（G5 屬驗收方）。**未** rustfmt 該檔。**未**動混鏈 squash。

### R.3 探針

Q-015 **7／7**（含 G5）。history_ops 15／15。local_gc 17／17。

### R.4 數字

`cargo test --workspace --no-fail-fast -- --test-threads=1` **exit 0**。
`test result:` 聚合：221 target 皆 ok；**2125 passed／0 failed／0 ignored**（比交付回合 1 多 1，就是 G5）。
`^error`：**0**。
conformance：`python3 nlang-spec/scripts/run-conformance.py --engine target/release/oo` → **162／162**。
身分：G2 綠（`x: 0` 三物件、標準根 `7038e250…`）。Q-015 探針一字未動。

---

## A2. 驗收回合 2（驗收方填，2026-08-31）

**A.1 修好了，而且修得乾淨。** 但它換出了另一半，**而那一半是工單的鍋不是交付的鍋。**

### A2.1 A.1 已解決

〔量，本弧二進位，known-answer 已過（`3`，對照 `add (1,"x")` → `_|_`）；
版本以行為確認（○ 帶 `commit:`）〕**沒有並行寫者，N 次 evolve＋commit：**

| N | 1 | 2 | 3 | 5 | 10 | **20** |
| :-- | --: | --: | --: | --: | --: | --: |
| $H_1$ | 0 | 0 | 0 | 0 | 0 | **0** |

$V = 2N$、$E = 2N-1$、$C = 1$ ⟹ **乾淨的交替鏈，一個洞都沒有。** **G5 綠。**
探針檔**自 G5 進檔後零改動**（`git diff` 空），未 rustfmt。

### A2.2 但祖先關係壞了，而且不是顯示問題

〔量〕五顆 commit、rollback 回 gen1、再 commit 一顆：

| | `oo log` |
| :-- | :-- |
| **v0.40.0 基線** | `final gen1` — **2 列** ✅ |
| **交付 2** | `final gen4 gen3 gen2 gen1` — **5 列** ❌ |

**被放棄的整段又回到 log 裡了**，只少了 `abandoned` 逐字點名的那一顆（gen5）。
成因：**`CommitMeta.abandoned` 依 R1 只記「被放棄的 HEAD」一顆**，
所以照它過濾只濾掉一顆，**區間裡其餘各顆沿覆蓋走回來**。

`SPEC_08` §6.2 R1 逐字：「回溯後歷史自新 HEAD 沿 parent 走，**被放棄的整段離開歷史**。」
⟹ **這是規格違反，不只是回歸。**

**而且不是顯示問題**〔量〕：`squash` 用同一條走訪證明祖先
⟹ **`oo squash` 接受一顆被回溯掉的 commit 當 base**，並且**真的產出了 squash commit**
（`Squash commit: 6a060e9c…`，log 變成 `compressed gen3 gen2 gen1`）。
**一個特權的歷史改寫，作用在操作者明確回溯掉的那條鏈上。**

### A2.3 這是工單的鍋——S2 把兩個關係壓成一句

**兩次交付都做了被要求的事。被要求的事本身漏了一半。**

S2 逐字只說「提交 ○ **恰有一個父**」，**沒有說那個父代表哪一個關係**。**有兩個**：

| 關係 | 是什麼 | rollback 在它上面 |
| :-- | :-- | :-- |
| **時間覆蓋** | 誰在誰之後被鑄 | **不留痕，而且正確**——gen2..gen5 真的發生過 |
| **祖先** | 現在的 HEAD 從誰下來 | **rollback 就是這條關係上的一次跳躍** |

*   **交付 1**：父＝先前 HEAD 的提交 ○ ⟹ **祖先對、拓撲錯**（A.1 的洞）。
*   **交付 2**：父＝所提交的 tip ⟹ **拓撲對、祖先錯**（本節）。

⟹ **沒有任何單一條邊能同時當這兩者。**

**而這正是 §1.7.1 那兩張圖，在本地圖之內又出現了一次。**
外層是「本地時間序 vs 全球特化序」；這裡是**本地圖自己內部**的
「時間覆蓋 vs 祖先」。**同一個形狀，低一階。**
（與 D50 同型：§3.1 的「順序」也是把**函數**與**關係**壓成一句話。
**這是本弧第二次踩同一個坑，而兩次都是驗收方寫的條文。**）

### A2.4 已補 G6

`g6_a_rolled_back_segment_stays_out_of_the_log`：五顆、rollback、再一顆，
斷言 gen2–gen5 **都不在** log 裡。**在交付 2 上紅。**
**釘性質不釘做法**——任何讓被放棄區段離開 HEAD 祖先的形狀都會過。
**測試修改權屬驗收方**，交付方不得編輯、不得加 `#[ignore]`。

### A2.5 下一步要一則裁定，不是一次修補

**交付方請先停在這裡。** 修法是設計選擇，不是實作選擇：

*   **甲**：提交 ○ 帶**兩條邊**——`parents:`（時間覆蓋，給拓撲／$H_1$）
    ＋一條**祖先邊**（鑄造當下 HEAD 的那顆提交 ○，給 `oo log`／squash）。
*   **乙**：只留時間覆蓋，`oo log` 改以**整個被放棄區間**過濾
    ⟹ 要改 `abandoned` 記什麼（**而 R1 是刻意只記 HEAD 一顆的**）。
*   **丙**：只留祖先邊（＝交付 1），接受洞
    ⟹ **$H_1$ 不再表示「一次分叉被收斂」，等於廢掉 D53。**
*   **丁**：`Commit.parent` 繼續設定（＝Q9 的甲）⟹ **D52 部分收回。**

**驗收方建議甲**，而且理由不是折衷：**甲就是 A2 本來的樣子。**
§4.1 原話「parent 是**搬家**不是刪除」、D50 改成「**換形**」——
**搬過去的正是祖先那條邊**，而時間覆蓋是 ○ **本來就有**的另一條。
**一顆 ○ 上兩條邊，因為那裡本來就有兩個關係。** 不動身分、不動 CAID。

### A2.6 裁定已下 ＝ D55 ＝ 甲。修補回合 2 的射程

**用戶 2026-08-31 裁甲。** 全文 `STATUS` D55，設計推導 `commit.md` §1.7.10。

> **一顆 ○ 上兩條邊，因為那裡本來就有兩個關係。**

**S2 就地修訂**（原文「提交 ○ 恰有一個父：它所提交的那個 tip」保留，但**那句話只管時間覆蓋**）：

| 邊 | 指向 | 誰用 |
| :-- | :-- | :-- |
| **時間覆蓋** `parents:` | **它所提交的那個 tip**（＝交付 2 現在的行為，**不要動**） | $H_1$／鑄造圖 |
| **祖先**（新增） | **鑄造當下 HEAD 的那顆提交 ○**（＝交付 1 的規則） | `oo log`／squash 祖先／`commits_after`／refine 影子／`gc::mark` |

**射程只有三件：**

**R2-1.** 提交 ○ 的框加一項記祖先邊。**正準形由你定**（探針不釘拼法），
但**必須與 `commit:` 及 `parents:` 三者可分辨**，且 encode↔decode 位元組相同。
第一顆 commit 沒有祖先 ⟹ 該項缺席（不是空字串）。

**R2-2.** 六個讀者（含 `gc::mark`）的走訪改走**祖先邊**。
`Commit.parent` 仍有就仍優先（混鏈雙讀不變）。**入口仍是 HEAD。**

**R2-3.** **重量 `abandoned` 過濾還需不需要。**
在甲之下祖先邊指向鑄造當下 HEAD 的提交 ○，**回溯段自然不在鏈上**
⟹ 交付回合 1 為救 R-b 加的那段過濾**應已多餘**。
**量完再決定移不移除，不要留成遮蔽用的死碼**；`local_gc` 探針的 walker 同理。
**若量出來仍需要，逐字說為什麼**——那會是甲沒完全解決問題的證據，要回頭請裁。

### A2.7 ⚠ 這一格做錯就等於白修

**祖先邊不得進入鑄造圖的邊集。**

D54 已裁：鑄造圖節點＝○、邊＝`parents:`。**祖先邊與 `commit:` 同屬註記**，
**不計入邊數**。把它算成邊，菱形就回來了——**A.1 原樣復發，G5 會紅**。

探針 `h1()` 只讀 `parents:`（逐字寫在函式註解裡），所以 **G5 就是這一格的守衛**。

### A2.8 驗收方會跑的完整流程（本回合之後）

全跑 ×3 `--no-fail-fast`／conformance／身分紅線／**跨版本混鏈（v0.40.0 造倉、新引擎讀）**／
**rollback 與 squash 的基線對照**（本回合抓到的那兩組，逐字並排）。

---

## R2. 交付回報（修補回合 2；交付方填。本行以上一字不得動）

### R2.1 射程

**R2-1.** 框上加 `ancestor: <本地 id>`（與 `parents:` 同一名字空間，與 64-hex 的 `commit:` 可分辨）。第一顆 commit 缺席這一項，不是空字串。`skip_savepoint_frame_lines` 連 `parents:`／`commit:`／`ancestor:` 一併跳過，combo encode↔decode 路徑未改。祖先邊**不是** `parents:`，`h1()` 不計（G5 綠）。

**R2-2.** `record_commit` 兩條邊分開鑄：`parents:` ＝所提交的 tip（交付 2，未改）；`ancestor:` ＝鑄造當下 `self.head` 的提交 ○（交付 1 的規則）。squash 的 covering 改為唯一 tip，祖先掛 base 的提交 ○（否則 covering 掛 base 會讓 HEAD 的提交 ○ 仍是 tip，下一次 evolve 再開洞）。六個讀者走祖先：`previous_commit` 有 `Commit.parent` 走 parent，否則讀 `ancestor:` 那顆 ○ 的 `commit:`。入口仍是 HEAD。`oo inspect` 的 `parent:` 沒改。

**R2-3.** 量完：**不再需要 `abandoned` 過濾。** 祖先邊指向回溯後的 HEAD 提交 ○，被放棄區段不在鏈上。已從 `previous_commit` 與 `local_gc` walker 拿掉；不是死碼。history_ops 那支「abandoned 離開 parent chain」與 G6 皆綠。

### R2.2 探針

未編輯 Q-015 探針（G5／G6 屬驗收方）。未 rustfmt。**8／8**（含 G6）。

### R2.3 數字

`cargo test --workspace --no-fail-fast -- --test-threads=1` **exit 0**。
`test result:` 聚合：221 target 皆 ok；**2126 passed／0 failed／0 ignored**（比修補 1 多 1，就是 G6）。
`^error`：**0**。
（第一次全跑 `identity_persistence::pin_concurrent_first_mint_yields_one_key` 因並行 mint 撞到半截 PKCS#8 紅了一次；重跑該檔 16／16，第二次全跑 exit 0。與本弧無關。）
conformance：`python3 nlang-spec/scripts/run-conformance.py --engine target/release/oo` → **162／162**。
身分：G2 綠。Q-015 探針一字未動。

---

## A3. 驗收回合 3（驗收方填，2026-08-31）

**R2-1／R2-2／R2-3 三項都做對了。但混鏈那一格，量出來是資料遺失。**

### A3.1 通過的（逐項複驗）

*   **探針 8／8**（含 G5、G6），**探針檔自 G6 進檔後零改動**、未 rustfmt。
*   **$H_1$**：N ＝ 3／10／20 皆 **0**（$V=2N$、$E=2N-1$、$C=1$）。祖先邊確實**沒有**進邊集。
*   **rollback 與基線逐字相同**〔量，五顆＋回溯到第一顆＋再一顆〕：
    基線 `final gen1`、交付 `final gen1`，**兩邊都恰好一行 `abandoned`**。**A2.2 已關。**
*   **squash 對被放棄的 commit 現在拒絕**〔量〕：`Error: squash base is not an ancestor of HEAD`，
    **rc=1**，log 不變。**回合 2 那個「特權改寫作用在回溯掉的鏈上」已關。**
*   **R2-3 答得對且做得對**：`abandoned` 過濾量完確認多餘、已移除，**沒有留成死碼**。
*   行為版本確認：○ 帶 `ancestor:`。known-answer `3`，對照 `add (1,"x")` → `_|_`。

### A3.1a 全跑複驗：交付方的數字屬實，而我差點把自己的探針讀成別人的缺陷

〔驗收方獨立跑四次，release，`--no-fail-fast --test-threads=1`〕

| 跑 | 結果 |
| :-- | :-- |
| 1（G7 尚未進檔） | **221 target／2126 passed／0 failed／`^error` 0** |
| 2、3（G7 已進檔） | 2126 passed／**failed=1** |
| 4（指名抓失敗名字） | 2126 passed／failed=1 ⟹ **唯一失敗是 `g7_…`** |

⟹ **交付方 R2.3 自陳的 221／2126／0／0 屬實。** 2、3 的那支紅是**驗收方自己在背景跑到一半
加進去的 G7**，不是交付的缺陷。

**⚠ 但這一格差點被我讀反，而讀反的方式值得記**：聚合腳本只留數字、丟掉名字，
於是三次跑出「1 綠 2 紅」時，手上**沒有任何東西能分辨「交付有偶發缺陷」與「我自己剛加了一支紅」**。
**「一支五次只紅四次的紅不是釘子」的反面同樣成立——一支三次只綠一次的綠也不能當綠**，
而兩者都要**先知道紅的是誰**才判得出來。
⟹ **常設**：全跑的聚合**必須連失敗的測試名一起留**，只留計數等於把證據丟掉。

**另**：交付方 R2.3 自陳的那支偶發紅
（`identity_persistence::pin_concurrent_first_mint_yields_one_key`，半截 PKCS#8）
**在驗收方這四次全跑裡一次都沒有重現** ⟹ 比 1／4 更罕見。已據實記進 Inbox 該列。

### A3.2 要修的：在既有的倉上提交一次，然後 `gc` 會把歷史刪掉

〔量，**真二進位**：`oo v0.40.0` 標籤建置造倉（known-answer `3`），本弧建置讀〕

三顆 commit 的舊倉 → **本弧引擎 `oo commit` 一次** → `oo gc --grant gc`：

| | |
| :-- | :-- |
| 物件數 | **7 → 3**（`removed 6 objects, freed 1614 bytes`） |
| `oo log` | 3 列 → **1 列** |
| `oo inspect <old1>` | **`Error: CAID not found in local store`** |
| `oo inspect <old2>` | **`Error: CAID not found in local store`** |
| v0.40.0 二進位再讀 | 也只看得到 `new4` |

**⟹ 五秒前還在的那段歷史，永久不見了。** 觸發路徑是**最普通的兩步：commit，然後 gc。**

**這不是「走訪提早停下」**——那個 §1.7.7 預言過、工單也接受。
**這是無聲的、不可回復的資料遺失。**
而工單 S7 逐字寫著「**不要讓人以為歷史被 gc 掉了**」；**現在它真的被 gc 掉了。**

**成因**：`ancestor:` 記的是**一顆 ○ 的本地 id**，而**本弧之前寫下的 commit 沒有 ○**。
於是 `new4` 的祖先邊落空 ⟹ 走訪在 `new4` 就停 ⟹ `gc::mark` 走同一條邊
⟹ old1–old3 判為不可達 ⟹ **sweep 掉**。

### A3.3 這一格也有驗收方的份

A2.6 的表逐字寫「祖先邊 → **鑄造當下 HEAD 的那顆提交 ○**」。
**對一顆本弧之前的 commit，那顆 ○ 不存在**，而工單沒有說那時候該怎麼辦。

**建議的方向（一句話，仍請你自己量）**：**祖先邊改記「前一顆 commit 的 digest」**，
不是 ○ 的本地 id。兩種引擎寫的 commit 就統一了——
以 digest 直接搆到 `old3`，而 `old3` 自己還帶著 `Commit.parent`，雙讀接得下去。

### A3.4 已補 G7

`g7_committing_on_an_older_repo_does_not_let_gc_eat_it`：造三顆、**把本弧的註記從框上剝掉**
（做出本弧之前的形狀：commit 帶 `parent`、○ 沒有 `commit:`）、再 commit 一次、`gc`，
斷言最舊那顆仍 `inspect` 得到。**在修補 2 上紅。**
**釘存活不釘拼法。** 它用剝註記的方式在 `cargo test` 裡重現，
**上面 A3.2 的數字則是用真的 v0.40.0 二進位量的**。
**測試修改權屬驗收方。**

### A3.5 本回合只做這一件

**只修 A3.2。** 不擴射程。修好之後驗收方跑完整流程並收弧。
