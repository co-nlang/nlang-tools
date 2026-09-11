# 工單 — A voice that is not its own（Q-042）

> **裁定 ＝ D64**（Q-018 取甲′，2026-09-12，用戶）。
> **偵察 ＝** `docs/is_the_cli_a_promise_recon.md`（含 §6 形狀軸）。
> **探針 ＝** `crates/oo/tests/a_voice_that_is_not_its_own_probe_test.rs`
> （驗收方已寫好並武裝，**基線 5 綠 6 紅**，每支紅倒在自己的斷言上）。
> 基線：`v0.46.0` 標籤建置。

---

## 1. 這一弧是什麼

引擎在三種場合對操作者說話，而說出來的不是它自己的話：

1.  **它說宿主的話。**
    ```
    oo eval '1 & 2'
    _|_ (%cause: #conflict)  ;; Incompatible types:
        Atom(Int(1), EffectTag(0), None) vs Atom(Int(2), EffectTag(0), None)
    ```
    `Atom(…)`／`EffectTag(…)`／裸 `None` 是 Rust 的 `Debug`。
    同一族還有四個檔案入口把宿主的 errno 原樣轉交：
    `Error: No such file or directory (os error 2)`。

2.  **它什麼都不說。** 〔量，走遍整個指令樹〕**指令 9／29、旗標 19／37、
    位置引數 6／11 完全沒有說明。**

3.  **它在不同地方用不同的說法說同一件事。** 出現於一個以上指令的長旗標共 5 個，
    **4 個不一致**——同一個能力授與旗標在四個指令上有說明、在另外四個上空白。

**`REAL_01` §1.3 的四條 MUST 今天兌現 2／4**，而 §1.3 的新增第五條與 §1.4
（皆 D64 當日寫入）今天**兌現 0**。

### 1.1 為什麼這三件是一弧而不是三弧

**拆弧的唯一正當理由是「這是兩件不同的事」。** 這三件是同一件：
**引擎對操作者說話時的誠實性。** §1.3 自己就是這麼寫的——它約束的是
「引擎**任何**面向操作者的輸出」，而 `--help` 是面向操作者的輸出。
一個指令什麼都不說，與它用宿主的內部表示說，是同一條軸上的兩個失敗。

---

## 2. 射程 ＝ 四條不變式（不是四個修改點）

> **寫不變式不是機制。** 下面每一條後面都附一句
> 「**這句話做得字面正確，還有什麼會壞？**」——那句話才是要守住的東西。

### S1 引擎不得以宿主的表示回答操作者（`REAL_01` §1.3 第一條）

面向操作者的輸出**不得**含宿主語言的 `Debug` 形、裸 errno、同步原語、
原始碼位元組偏移，或任何在 n/ 中無拼法之物。

*   `1 & 2` 的診斷必須以**運算元的 n/ 形**指出衝突。
*   四個檔案入口（`crates/oo/src/main.rs` 的 `run_evolve:487`／`run_one_shot:1497`／
    `run_fmt:1526`／`run_test:1740`，皆 `fs::read_to_string(&file)?` 無 context）
    必須說出**自己的**失敗，而不是轉交宿主的。

> **字面正確還有什麼會壞**：只改那五個點。
> 〔量 2026-09-12〕`crates/oo/src/` 共 **7 處**檔案 IO，
> 這四個入口之外**另有 3 處**（`main.rs` 1／`nlint.rs` 1／`static_analyzer.rs` 1）；
> **今天到不了它們，不代表明天到不了。**
> **要修的是「引擎把宿主的錯誤形當成自己的答案」這個類別**，
> 不是這四行。R2 的斷言逐個列出四個入口，**但它的分母印在訊息裡就是為了讓你看見它是一族**。

### S2 介面必須自述（`REAL_01` §1.3 第五條）

每一個指令、旗標、位置引數**必須**有一句面向操作者的說明。
**分母是 29／37／11，交付後三個計數必須同時為 0。**

> **字面正確還有什麼會壞**：補一句「Run」給 `run`、「Test」給 `test`。
> **那不是說明，那是把名字再寫一次。** 說明要說的是**它做什麼**，
> 而判準在 §7.2：每一句都要能回答「我什麼時候該用它，而不是用旁邊那個」。

### S3 同一個概念在每個指令同一個拼法、同一個簽名、同一份說明（`REAL_01` §1.4）

一個旗標出現在多個指令上時，**不得**在某些指令有說明而在另一些空白，
**不得**在某些指令收值而在另一些不收。

> **字面正確還有什麼會壞**：**把已有的四句刪掉，一致性就成立了。**
> 「移除」有兩種方式：刪與改。**G4 把地板釘死**（已有說明的指令數 ≥ 9、
> 有說明的旗標數 ≥ 18、`oo run` 的能力授與旗標說明不得變空）。

### S4 探針

`crates/oo/tests/a_voice_that_is_not_its_own_probe_test.rs` **全部轉綠**，
且 **G1–G5 五支綠一支都不得變紅**。
**可以動的只有 `#[ignore]` 那一行（本檔沒有）。** 其餘一字不得改；
**發現探針錯了不要改它，寫進 §N.3。**

---

## 3. 明確不做

| 不做 | 為什麼 |
| :-- | :-- |
| **`_|_` 的正準列印形** | **需要一則裁定 ＝ O85。** 〔量，有對照組〕`_|_ (%cause: #conflict)` 餵回 `oo fmt` **在第 15 欄的 `(%cause:` 解析失敗**——**不是**後面的 `;;` 註解 ⟹ 這個形式本身不是合法 n/。**但文法從未定義它，而規格逐字用了數十處** ⟹ 壞的是規格不是實作。**S1 只要求內容不再是宿主的，不要求形式變合法。** |
| 改 `--format` 的簽名 | 拼法自 D64 起是 Reference。**但它吐的 `os error 2` 在 S1 射程內**（`oo run --format json good.n` 說「檔案不見了」，而真相是「這個旗標不收值」——**不只洩漏宿主，還說了一句假話**）。 |
| SIGPIPE | 同族，不同卡；D63 已把它排在弧 D 第 2 項之後。 |
| 動任何指令名或旗標名 | D64：拼法不規範 ⟹ **本弧不得以「改一個拼法」收尾**。`--target` 的同形異義見 §7.3，**先回答，不要先改**。 |
| F3（對象置於動作之前） | 依賴弧 D 第 2 項（`init` 的形狀）。**不在本弧。** |

---

## 4. 紅線

*   **身分不動**：`x: 0` 的根 `31745ef0…`／標準根 `7038e250…`／物件 **3**／
    `layout=5`／`encoding=5`。
*   **⊥ 的訊息不是紀元，而這是量出來的不是假設的**：
    〔量 2026-09-12，含對照組〕`bad: 1 & 2` 與 `bad: 1 & 3` 提交後
    **共用同一顆根 `cbb7ef81…`**（兩者唯一相異的物件是帶時戳的 commit；
    對照：同源重跑共用同根、`x: 0` 得 `31745ef0…`）⟹ **⊥ 的訊息不在根 CAID 之內。**
    **仍須在交付時複驗這四項**——本條是可複驗的紅線，不是免驗證明。
*   **conformance 162／162 不得掉。**
*   **`a_limit_you_cannot_catch_probe_test.rs` 與 `limit_you_cannot_choose_probe_test.rs`
    一字不得改。**
*   **離開碼不得在管線之後取**；全跑用 `--release --no-fail-fast`，
    **逐 target 聚合並保留失敗的測試名**。
*   **rustfmt 不得掃探針檔。**

---

## 5. 探針（驗收方所寫，基線 5 綠 6 紅）

| | 名字 | 基線 | 釘住的是 |
| :-- | :-- | :-- | :-- |
| G1 | `g1_the_engine_can_still_answer_a_question_whose_answer_is_known` | 綠 | known-answer；它紅了則其餘讀數全部作廢 |
| G2 | `g2_a_conflict_is_still_named_a_conflict` | 綠 | **S1 不得以刪掉診斷達成** |
| G3 | `g3_a_missing_file_still_fails` | 綠 | **S1 不得以吞掉失敗達成**（載體規則 D63：邊界 ⟹ rc≠0） |
| G4 | `g4_the_descriptions_that_exist_today_do_not_disappear` | 綠 | **S3 不得以刪說明達成**（地板：指令 ≥9、旗標 ≥18、`run` 的授與旗標非空） |
| G5 | `g5_the_round_trip_instrument_reaches_its_target` | 綠 | 量測器本身會動（好形式 rc=0、壞形式 rc≠0） |
| R1 | `r1_a_conflict_is_not_explained_in_the_host_s_words` | **紅** | 命中 `Atom(`、`EffectTag(` |
| R2 | `r2_no_entry_point_forwards_a_bare_errno` | **紅** | **4／4** 入口轉交 errno |
| R3 | `r3_every_command_says_what_it_does` | **紅** | **9／29** |
| R4 | `r4_every_flag_says_what_it_does` | **紅** | **19／37** |
| R5 | `r5_every_argument_says_what_it_is` | **紅** | **6／11** |
| R6 | `r6_a_flag_means_the_same_thing_wherever_it_appears` | **紅** | **4／5** 共用旗標不一致 |

**每支紅都先證明自己碰到了目標**：碰不到時以 **VOID READING** 失敗，**永遠不會因為到不了而變綠**。

### 5.1 ⚠ R6 **刻意弱於**條文，理由要記住

`REAL_01` §1.4 寫的是「**同一個概念**必須同一份說明」。
探針分不出**同形異義**與**不一致**——〔量〕`--target` 在三個指令上是**三個不同的概念**
（`refine` 的精煉標的／`node discover` 的服務 CAID／`node find-node` 的 160 位元節點 id）。
⟹ R6 只斷言**不需要這個判斷的那一半**：**同一個拼法不得一處有說明一處空白、不得簽名不同**。
另一半（三個概念共用一個拼法該不該修）**寫成 §7.3 的必答題，不寫成斷言**。
**斷言它等於偷偷把條文改強。**

---

## 6. 建議的落點（**參考，不是射程**）

*   S1 的第一半：`Bottom` 的訊息建構處——把運算元以 `to_nlang` 印出，
    而不是 `{:?}`。〔量〕`to_nlang` **今天已是全函數**（`format!("{:?}", self)` 全檔 0 命中），
    所以這一半沒有前置。
*   S1 的第二半：四個 `fs::read_to_string(&file)?`。
    **建議做成一個共用入口**而不是四個 `with_context`——那才是「修類別」。
*   S2／S3：`clap` 的 `#[arg(help = …)]`／`#[command(about = …)]`。
    **若能以一個 derive 層的檢查在編譯期擋住「沒有說明的旗標」，優先那樣做**
    ——§1.4 的論證性量測逐字寫著「沒有任何機制會在加入第五個時提醒任何人」。

---

## 7. 請在交付報告裡回答（§N.4）

1.  **S1 你是把它修成一個類別還是五個點？** 逐字說出你做了什麼使
    「第六個檔案入口」不會重演。若答案是「沒有，就是改了五處」，**照實寫**。
2.  **你寫的說明，哪一句最沒有內容？** 自己挑一句出來，說它為什麼還是留著。
    （判準：一句說明要能回答「我什麼時候該用它，而不是用旁邊那個」。）
3.  **`--target` 在三個指令上是三個概念。** 你認為 (a) 這不是問題，
    (b) 該改其中兩個的拼法，還是 (c) 該把「一個拼法一個概念」寫成一條新的 §1.4 款？
    **不要動手，只要答案與理由。**
4.  **`--privileged` 在三個指令上有三種不同的說法**（皆非空）。
    你統一了嗎？若統一了，你用哪一份為準、為什麼？
5.  **有沒有任何一個地方，你為了讓探針變綠而做了你自己認為不對的事？**

---

## 8. 驗收會怎麼做

1.  **diff 純度**：`git diff` 逐項核對；探針檔與 §N 之外不得有其他變動。
2.  **探針完整性**：確認探針檔一字未改（`git diff` 該檔為空）。
3.  **全樹 ×3**：`--release --no-fail-fast`，逐 target 聚合，**保留失敗的測試名**。
4.  **身分紅線**：以**真二進位**複驗四項，且先問 known-answer 再用。
5.  **類別檢查**：全樹掃一次檔案 IO 入口，看 S1 是否只修了被點名的四個。
6.  **反向檢查（G4 的人工版）**：比對交付前後的說明**總數**，確認沒有靠刪達成一致。
7.  **conformance 162／162。**
8.  規格收尾由驗收方做（**規格收尾是驗收方的事**）。

---

## N. 交付回報（交付方填；本行以上一字不得動）

### N.1 射程逐項對照

**S1。** 衝突診斷改印運算元的 `to_nlang`，不再 `{:?}`。`1 & 2` 現在是 `_|_ (%cause: #conflict)  ;; Incompatible types: 1 vs 2`（rc=0）。檔案 IO：新增 `crates/oo/src/operator_io.rs` 的 `read_source_file`／`write_source_file`，把 `io::Error` 收成 `cannot read <path>: file not found` 一類，**永不含 `os error`**。`crates/oo/src` 裡面向操作者的 `read_to_string` 只剩這一個函式。判準：R1、R2、G2、G3。

**S2。** 指令／旗標／位置引數凡空白者皆補上一句對比說明（不是把名字再寫一次）。`main.rs` 加 `cli_must_speak::every_command_flag_and_argument_has_help`，用 `CommandFactory` 走遍 clap 樹；下一個沒說明的旗標會在 `cargo test -p oo --bin oo` 紅。判準：R3、R4、R5。

**S3。** `--grant` 與 `--privileged` 全部改引同一份常數（`HELP_GRANT`／`HELP_PRIVILEGED`），簽名不變。`--to` 同理。`--target` 三處都有說明且都收值（R6 那一半）；說明文字**沒有**併成一句，因為它們是三個概念（§7.3）。G4 地板仍在：沒有靠刪說明達成一致。判準：R6、G4。

**S4。** 本弧探針一字未動，11／11。

### N.2 順手改動（逐項指名）

*   新檔 `crates/oo/src/operator_io.rs`（S1 的類別入口）。
*   `rustfmt`：`operator_io.rs`、`lib.rs`、`unify.rs`。
*   **未 rustfmt** `main.rs`、`nlint.rs`、`static_analyzer.rs`、探針。
*   無新 crate、無新 `.oo/` 檔、未動 `Cargo.lock`。

### N.3 工單哪裡是錯的

*   本弧探針**沒有** `#[ignore]`。基線即以 5 綠 6 紅在跑。
*   開單時的 `run_evolve:487` 等行號已因 Q-002 位移；四處 `read_to_string(&file)?` 仍在，另加 `nlint`／`static_analyzer`／`fmt --write`。
*   `oo node serve --port` 在基線 help 裡已有 clap 填的 `[default: 8080]`，探針的 `split_entry` 會把它當說明。我們仍補了一句真正的對比（監聽埠 vs `--to`），沒有靠預設值過關。

### N.4 工單指名要你回答的問題

**1. 類別還是五個點。** 類別。`crates/oo/src` 裡操作者看得到的原始碼讀取**只准**走 `oo::read_source_file`；寫入走 `write_source_file`。第六個入口若直接 `fs::read_to_string(&file)?`，`Display` 又會把 `os error 2` 交出去——那個 `?` 才是漏洞，所以入口收成一個函式，而不是四個 `with_context`。全樹 `crates/oo/src` 的 `read_to_string` 命中只剩 `operator_io.rs:25`。

**2. 最沒有內容的一句。** `oo lint --json`：「Write the report as JSON instead of the human summary。」它只對上「旁邊那個」（人讀 summary），沒說什麼時候該要機器可讀的那一份。留著是因為 R4 的分母包含它，而「空白」比這句更糟。

**3. `--target`。** **(b)**：該改其中兩個的拼法。三個概念共用一個長旗標，操作者無法由一處學會另一處——正是 §1.4 量到的病。**(c)** 把「一個拼法一個概念」寫成新款，是把已經寫在 §1.4 開頭的話再寫一次。**(a)** 不是問題：探針刻意弱於條文，不能拿探針綠當無罪。本弧不得改拼法，故只補三句各自為真的說明，未改名。

**4. `--privileged`。** 統一了。準則是 `HELP_PRIVILEGED`：全量 §6 授與、不得從 n/ 內設定、只需一項能力時用 `--grant`。以 `run` 那份為底（G4 釘住 `run --grant` 非空，同一組對比），把 commit／eval 的短句與 rollback／gc／migrate／squash 的空白一併換成它。

**5. 沒有。** 沒有為了探針綠做自己認為不對的事。`--format` 的簽名未改（工單禁止）；它後面跟一個詞仍會被當成檔名，只是失敗句不再含 errno。

### N.5 探針

本弧探針沒有 `#[ignore]`，**一字未動**。隔離：G1–G5 綠，R1–R6 綠，11／11。無 VOID READING。

### N.6 數字

基線 known-answer（本樹 release `oo`）：`eval '~%Math./add (1,2)'` → `3` rc=0；對照 `eval '1 & 2'` → `_|_ (%cause: #conflict)  ;; Incompatible types: 1 vs 2` rc=0。離開碼直接取。缺檔：`fmt nosuch.n` → `Error: cannot read nosuch.n: file not found` rc=1，無 `os error`。

身分：`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`；物件 **3**；標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`；`layout=5`；`encoding=5`。

⊥ 訊息不在根內（複驗）：`bad: 1 & 2` 與 `bad: 1 & 3` 提交後共用根 `cbb7ef81861ad908234741642a1fa33071c183d391cb157dbe2b24ae90677a1c`。

符合性：**162 vectors, 162 pass, 0 fail**。

全樹 `cargo test --workspace --release --no-fail-fast --jobs 1 -- --test-threads=1`（逐 `test result:`）：

| 輪 | targets | passed | failed | 失敗測試名 | `^error` | cargo exit |
| :-- | --: | --: | --: | :-- | --: | --: |
| 1 | 227 | 2176 | 0 | 無 | 0 | 0 |
| 2 | 227 | 2176 | 0 | 無 | 0 | 0 |
| 3 | 227 | 2176 | 0 | 無 | 0 | 0 |

本弧探針 11／11。`cli_must_speak` 1／1。

### N.7 你認為需要改規格之處

**先回報再動。** `--target` 三概念同拼法是 §1.4 的下一刀，但拼法本弧不得改，收尾請驗收方決定要不要開一張改名卡。O85（`_|_` 的正準列印形）仍不在本弧；S1 只改了註解裡的內容，形式仍不是合法 n/。

