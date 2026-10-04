# 工單：一次觀測留下的東西（Q-073）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-073／裁定 `meta/oo/STATUS.md` **D91**（甲，四格照建議）／同族 D80（○ 記站的點）、D88／D89（一次性求值看所在宇宙）、D79（丟了的 context）、D55（前驅與祖先分開）
> 既有規格：`SPEC_10` §3.1（產生判準 (b)、`_|_` 款、前驅款、(c) 的 MUST NOT、自陳缺口 2026-09-27）；`REAL_02` §5.1.1（宣告未推進者不得收到它宣告不了的東西）；`REAL_01` §1.1（一次性求值）
> 探針（已預先提交並校準）`crates/oo/tests/what_an_observation_leaves_probe_test.rs`
> 基線：dev `342d9e6`／`oo v0.74.0` ⟹ **5 綠 10 紅**，三輪一致；十支紅的都紅在一般斷言，零空洞讀數。

## 1. 缺陷

`SPEC_10` §3.1：觀測**真的化約了一個 thunk** ⟹ 產生 Savepoint；觀測坍縮為 `_|_` ⟹ **必須**產生 Savepoint（否則 `%cause` 沒有可指的對象）。規格自 2026-09-27 起自陳參考實作兩者皆未兌現。
〔量，v0.74.0〕已提交 `a: 1`、`v: 1 + 1`（照 D46 存成 thunk）、`b: 1 & 2`（⊥）的倉裡，`eval '_.v'`、`eval '_.b'`、`run --observe`、`repl`、`test` 之後 `.oo/savepoints/` **皆不增**。
〔讀〕寫 ○ 的只有注入（`savepoint::record`）與提交（`record_commit`）；觀測路徑上沒有寫者。
第 4407 列當年分不清「沒實作」與「觀測路徑根本沒有倉可寫」——後者已由 D88 解除，**只剩沒實作**。

## 2. 裁定與依據

*   **D91（用戶，甲）**：在一個宇宙裡的每一次一次性觀測（`eval`、`run`、`test`、`repl` 的每一次作答），**化約了 thunk、或答案為 `_|_`、或觸及視界**（只前進了一半）⟹ 寫**一顆觀測 ○**；只讀到已存的非 thunk 資料 ⟹ 不寫。
    *   **① 葉**：觀測 ○ 掛在它站的那個 context 上（前驅＝當下的末端），**不成為注入或提交的前驅**——不移動主線、不造分叉與匯流。
    *   **② 內容**：站的點（`HEAD`，當下讀取，D80 同一條規則）＋問題（路徑或運算式；`run`／`test`／`repl` 含注入的來源與會話裡先前的行）＋答案（值；`_|_` 連同 `%cause`）。
    *   **③ 冪等**：同一個點、同一個問題、同一個答案已有那顆 ○ ⟹ 不再寫。用戶原話：「**觀測是一個冪等的操作**」——這也是 ① 的理由。
    *   **④ 視界**：觸及視界者算（「看了多深」正是 ○ 的職責）。
*   **直接後果（不需新裁定）**：觀測 ○ 是 `.oo/` 裡新的一種紀錄 ⟹ 依 `REAL_02` §5.1.1 推進 **`layout=9`**。宣告 `layout≤8` 者**不寫**觀測 ○（照答，已知留白）；`migrate` 把 8 推進到 9。

## 3. 射程＝不變式（不是機制）

*   **I1 產生**：化約了 thunk 的觀測恰寫一顆（r1）；`_|_` 恰寫一顆且它帶著成因（r2）；觸及視界恰寫一顆（r3）；只讀到已存原子者不寫（g1）。`run`、`test`、`repl` 同樣成立（r7）。**判準 (b) 的「化約」要由引擎判出**，不得以「消耗了燃料」或「每一次觀測」代替（`SPEC_10` §3.1 已明文禁止前者）。
*   **I2 葉**：觀測 ○ 的前驅是它站的那個末端；之後的注入與提交**仍以那個末端為前驅**，不以觀測 ○ 為前驅（r4）。並行的觀測因此不造分叉，下一次注入不因觀測而記一次匯流。`log`、`status`、`gc --dry-run` 看到的歷史不因觀測而改變（g3、g5）。
*   **I3 內容**：觀測 ○ 記下當下的 `HEAD`（r5）；問題包含注入的來源——觀測同一條路徑、答案相同、來源不同的兩次 `run` 是兩個問題（r8）。
*   **I4 冪等**：同一個點、問題、答案不再寫；換問題寫；`HEAD` 移動後同一個問題寫（r5、r6）。重複的觀測**不得**使紀錄無界成長。
*   **I5 宣告**：新倉宣告 `layout=9`；宣告 `layout=8` 的倉照答而不寫觀測 ○；`migrate --grant migrate` 推進到 9，之後才寫（r9）。**遷移要照實點名被鎖在門外的引擎**（`REAL_02` §5.1.1）：〔量，標籤原始碼〕v0.64.0 起寫 `layout=8`、v0.74.0 仍是 ⟹ 自 8 遷移，被鎖在門外的是 **v0.64.0 … v0.74.0**；自 7 是 v0.61.0 … v0.74.0，更舊的起點上界同為 v0.74.0（r9；預先修訂的 D80 探針 r6）。`layout=9` 之下其餘一切與 `layout=8` 相同（注入 ○、提交 ○、點、簽章、位址）。
*   **I6 寫不下就不回報成功**：觀測 ○ 寫不下時，觀測 rc≠0、具名說出寫不下哪裡；`HEAD` 與工作集不動（r10）。答案可以照印。**與 `SPEC_10` §3.1「一次回報失敗的提交沒有落地」同一個立場：紀錄寫不下而回報成功，比單純失敗更糟。**
*   **I7 不變的**：`--ephemeral` 與沒有宇宙的地方不寫任何宇宙（g2）；丟了 context 照舊具名拒絕、不寫（g4）；觀測不動 `HEAD`、工作集、宣告（g3）。不新增 CAS 物件；值位址與提交位址不動。

## 4. 紅線（今天綠，必須保持綠）

*   g1–g5；`a_savepoint_that_knows_where_it_stood`（D80）、`a_history_older_than_its_savepoints`、`the_universe_you_are_in`、`an_interactive_eval`、`where_there_is_no_universe`、`one_writer_at_a_time`、`what_the_top_dropped`。
*   **交叉編譯** `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   conformance 162／162；`x: 0` 根 `31745ef0…`、`v: 1 + 1` 與 `1+1` 根 `f4f32e7b…`、標準根 `7038e250…`；`~%Math./add` (1, 2)=3、(1, 3)=4；新倉 **`layout=9`**／`encoding=5`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** 觀測 ○ 的磁碟形（逐字給一顆 `eval '_.v'` 與一顆 `eval '_.b'` 的檔案內容），以及它怎麼被辨認為觀測 ○；舊引擎為何讀不到它（`layout=9` 被拒）之外，新引擎的哪些讀者（前驅、提交祖先、`gc`、D85 切斷檢查、`sole_tip`）各自如何不把它當主線節點。
*   **Q2** 「化約了 thunk」怎麼判：在哪裡計、`eval '1 + 41'`（沒有存著的 thunk、但要計算）與 `eval '_.a'`（存著的原子）各寫不寫、為什麼。
*   **Q3** 問題的內容在 `eval`／`run`（含 `--format`）／`test`（每個 `test_` 欄位一顆還是一檔一顆）／`repl`（會話先前的行如何進入問題）各是什麼。
*   **Q4** 並行：兩個終端同時觀測時，冪等的比對與寫入之間有沒有窗口；若有，最壞情況是什麼（重複一顆？還是別的）。**不要求修**，陳述事實。
*   **Q5** `#incomplete`（非 `#blur` 策略下的掛起）今天在一次性觀測上到不到得了；到得了的話寫什麼。

## 6. 明文不在射程內

*   觀測 ○ 的回收（○ 沒有回收機制，本弧不設）。
*   以 ○ 為對象的讀取介面（例如「列出觀測 ○」的指令、`%cause` 經由 ○ 的觀測通道）。
*   一次性觀測以外的觀測入口（`~%Repl`、明示的觀測動作）。
*   規格文字（`SPEC_10` §3.1 移除自陳缺口並寫入 D91 四款、`REAL_02` §5.1.1 `layout=9` 一款與被鎖在門外的版本、`REAL_01` §1.1 一次性求值寫觀測 ○）、`CHANGELOG`：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r10 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。**
*   觀測 ○ 的格式是你的。探針只讀 layout 8 已固定的部分——Savepoint 是 `.oo/savepoints/` 底下的檔（不含 `LOG` 與點檔），有一行 `parents:`——其餘只數檔案、在新檔的位元組裡找 64 位十六進位的 `HEAD` 摘要或成因標籤。`rustfmt` 不得掃探針檔。
*   〔驗收方校準〕**兩極皆做**：
    基線 5 綠 10 紅（三輪一致）。
    一個最小參考實作（`layout=9`；引擎計數強制求值了幾個 thunk；觀測 ○ 與其他 ○ 同目錄、標頭多一行標記，`load_circles` 跳過它；四個入口在作答後寫入，`--ephemeral`／沒有宇宙不寫）⟹ 本探針 15／15。
    **守衛的變異**：一律寫 ⟹ g1；`--ephemeral` 寫進呼叫者所在的宇宙 ⟹ g2；觀測把檢視寫進工作集 ⟹ g3 與 g5；略過丟了 context 的檢查 ⟹ g4。另：觀測 ○ 當成主線節點（不跳過）、或冒充提交 ○ ⟹ **r4** 擋下（g5 不擋：`log` 與 `gc` 走提交物件，結構上與觀測 ○ 隔開——g5 守的是另一件事：觀測 ○ 的格式使 `gc` 讀 Savepoint 時出錯而拒絕）。
    **預先修訂既有測試（十檔十五支，標 `AMENDED 2026-10-04 for Q-073 (D91)`）。你不得再改它們。**
    *   **`layout=8` 絆線改為 `layout=9`（六檔八支）**：`a_commit_that_closes_the_door`（g1、r5）、`a_savepoint_that_knows_where_it_stood`（r5；r6 另把遷移點名的上界由 v0.63.0 改為 v0.74.0）、`a_type_you_could_not_carry`（g3）、`atomic_write`（p2，裁定清單加一列 `layout=9 Q-073 / D91`）、`nothing_here_or_nothing_you_can_see`（g1）、`one_writer_at_a_time`（r4）。**照 Q-062 的先例，這些在基線上是紅的**——它們釘的是「目前的宣告」，改了數字就只在新引擎上綠；〔量〕基線上紅的恰是被改的那幾行，參考實作上全綠。
    *   **遷移點名的上界 v0.63.0 → v0.74.0（三檔五支）**：`a_commit_that_is_a_value`（r6）、`a_signature_that_signs_the_commit`（r8）、`a_refusal_written_as_an_answer`（`names_the_boundary` 與其自測、r6、r7、r11）。理由同上一項；訊息由參考實作照實改寫後才在參考實作上綠——**那句點名是你要改的**（I5）。
    *   **`the_universe_you_are_in`（r2、g4）**：比對 `.oo/` 時排除 `savepoints/`——它們要守的是 `HEAD`、工作集、宣告不動，而觀測現在會寫觀測 ○。〔量〕基線 14／14、參考實作 14／14。
    同一參考實作（含遷移點名改寫）跑全樹（含上述預先修訂）：**258 target／2496 passed／0 failed，exit 0**（＝ v0.74.0 的 257／2481 加上本探針 15 支）；交叉編譯 0 error。**驗收方的全樹跑了三次才到這裡**：第一次揭出十支（`layout` 絆線八支＋語意兩支），第二次揭出遷移點名寫死 v0.63.0 的五支——後者是參考實作改了點名才出現的。
    **參考實作的「化約」判準是一個全域計數器的前後差——那是校準用的捷徑，不是設計**（I1、Q2）。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 做了什麼

在一個宇宙裡的一次性作答（`eval`、`run`、`test`、`repl` 的每一次印出的答案），這一次作答化約了 thunk、或答案是 `_|_`、或觸及視界，寫一顆觀測 ○。只讀到已存的非 thunk 資料不寫。化約由引擎判：`Ouroboros::force` 真正進入 thunk 本體之前記一筆；備忘命中在那之前就返回；燃料消耗不計。

觀測 ○ 與注入 ○、提交 ○ 同在 `.oo/savepoints/`。標頭在 `{` 之前多一行 `observation:`，問題與答案各一行。`load_circles` 看到那一行就略過整檔，所以它不是末端、不是下一顆注入或提交的前驅、也不進 `sole_tip`。

新倉 `.oo/format` 是 `layout=9`。宣告 `layout=8` 的倉照答、目錄不增；`migrate --grant migrate` 推進到 9 之後，同一次觀測才寫。遷移句點名被鎖在門外的引擎：自 8 是 v0.64.0 … v0.74.0。`layout=9` 仍寫 `point:`，提交位址、簽章、物件編碼與 layout 8 相同。編碼仍是 `encoding=5`。套件版本仍是 `oo` `0.74.0`。

寫不下時答案先印，然後 rc≠0。拒絕句是 `cannot write …`，路徑含 `.oo/savepoints`。`HEAD` 與工作集不動。

### 8.2 順手改動（逐項指名）

1. `crates/interpreter/src/observation.rs`：這一次作答的 `AnswerTrace`（化約、視界）。`handle_resource_exhausted` 開頭呼叫 `note_horizon`。沒有作答在進行時兩筆記錄都是空操作。`#incomplete` 仍不構造。
2. `crates/interpreter/src/lib.rs`：進入 thunk 本體的那一次 `eval` 之前呼叫 `note_thunk_reduced`。
3. `crates/interpreter/src/storage.rs`：`STORE_LAYOUT_VERSION` 為 9；`OBSERVATION_LAYOUT` 為 9；可遷移清單加上 8；`layout_records_observations`。`POINT_LAYOUT`、`VALUE_COMMIT_LAYOUT`、`SIGN_COMMIT_LAYOUT`、`OBJECT_ENCODING_VERSION` 未改。
4. `crates/interpreter/src/savepoint.rs`：`observation:` 辨認、`load_circles` 略過、`record_observation`（同一點、問題、答案已在則不寫；前驅是略過之後的主線末端）。
5. `crates/oo/src/main.rs`：四個入口在作答後記錄。`eval` 與 `repl` 的追蹤包住演化與觀測；`test` 只包該 `test_` 欄位的觀測；`run` 包住整份來源的演化與隨後的觀測或格式化。遷移上界改為 v0.74.0，layout 8 的最舊開啟者是 v0.64.0。

探針、標了 `AMENDED 2026-10-04 for Q-073 (D91)` 的十個檔、規格、`Cargo.lock`、版本都未改。沒有 rustfmt 探針、`storage.rs`、`lib.rs`、`main.rs`。

### 8.3 工單哪裡是錯的

無。工單寫的交付後全樹是 258 target、2496 passed、0 failed。這次三輪的 `test result:` 與此相同。

### 8.4 工單指名要你回答的問題

**Q1** 下面兩顆是同一次提交之後量的。識別元是該次提交的末端 `bc2daf0ec4cff7f9cc1731e3444a8b3a`，`point:` 是該次 `HEAD` 最後一欄摘要 `b4d1eb801cdc908c2053f7723c445b646c8f1af3f46fd8efa4884d96e30fb957`。檔名是隨機的，不釘。

`eval '_.v'` 印 `2`，rc 0。檔案：

```
#nlang/store savepoint
parents: bc2daf0ec4cff7f9cc1731e3444a8b3a
point: b4d1eb801cdc908c2053f7723c445b646c8f1af3f46fd8efa4884d96e30fb957
observation:
question: eval\n_.v
answer: 2
{}
```

`eval '_.b'` 印 `_|_  ;; %cause: #conflict`，rc 0。檔案：

```
#nlang/store savepoint
parents: bc2daf0ec4cff7f9cc1731e3444a8b3a
point: b4d1eb801cdc908c2053f7723c445b646c8f1af3f46fd8efa4884d96e30fb957
observation:
question: eval\n_.b
answer: _|_  ;; %cause: #conflict
{}
```

辨認：`{` 之前有一行，去掉空白後恰好是 `observation:`。問題與答案是單行，換行寫成 `\n`。本體是空 Combo `{}`。這顆檔沒有 `commit:`、沒有 `ancestor:`。

舊引擎在開啟時拒絕 `layout=9`（`store layout declaration "layout=9" is not supported; refusing to open.`），檔案進不了它們的讀者。

新引擎：`load_circles` 在剖析前略過整檔。末端、下一顆注入的 `parents:`、`previous_commit` 走的祖先與 `Commit.parent`、`gc` 的 `mark`／`plan_gc`／`push_commit_predecessor`、D85 的提交集合、`sole_tip`，讀的都是略過之後的圓。兩顆觀測 ○ 的 `parents:` 都是那顆提交圓，提交圓仍是唯一末端。`status` 不列出 savepoint 檔；`the_universe_you_are_in` 與 `an_interactive_eval` 的 `state()` 排除 `savepoints/`。

**Q2** 計數在 `Ouroboros::force` 裡、呼叫 thunk 本體的 `eval` 的前一行，而且只有這一次作答的 `AnswerTrace` 推在當前執行緒上時才記。備忘命中在那之前返回。`handle_resource_exhausted` 記視界，不記化約。

〔量〕`eval '1 + 41'` 印 `42`，rc 0，savepoint 數 4→5。純算術的演化結果依 D46 存回 thunk，隨後的觀測進入該本體，所以寫。`eval '_.a'` 印 `1`，rc 0，數 5→5。路徑強迫讀到的是已存原子，沒有進入 thunk 本體，答案也不是 `_|_`、沒有視界。

**Q3** 答案文字是 `to_nlang(0)`，不含 `=>`。

*   `eval`：`eval\n` 接去掉首尾空白的來源。`eval '_.v'` 的問題是 `eval\n_.v`。追蹤包住合成欄位 `__eval_result` 的演化與隨後的觀測。`_.v` 的 thunk 在演化路徑時就被強迫。
*   `run --observe`：`run\nobserve <路徑>\n`，兩者都給時再加 `format\n`，然後每個來源路徑與其位元組（檔尾沒有換行就補一個）。印出的值在兩者都給時仍是 `--observe` 的值；問題裡仍有 `format`。
*   `run --format` 而無 `--observe`：問題以 `run\nformat\n` 起頭，再接來源。印出的是未強迫的暫存 Combo。〔量〕`q: 1 + 41` 印出 thunk 寫法，rc 0，目錄不增（原子相加沒有進入 `force`）。`q: _.v + 40` 在演化時強迫了存著的 thunk，目錄 2→3。問題是 `run\nformat\nq.n\nq: _.v + 40\n`，答案是 `{\n  q: _.v + 40\n}`。
*   `run` 兩個旗標都沒有：不印答案，不記錄。
*   `test`：每個被觀測的 `test_` 欄位一顆，不是一檔一顆。`--static-only` 在觀測前就繼續，不記錄。追蹤只包該欄位的 `observe`。問題是 `test\n<名字>\n<檔案顯示路徑>\n<檔案位元組>`。`test_ok: _.v == 2` 存成比較的 thunk，觀測進入本體。
*   `repl`：該行剖析並演化之後，每一個印出的座標一顆。追蹤包住該欄位的演化與觀測。問題是 `repl\n<座標>\n<到這一行為止的會話，以換行接起>`。剖析失敗、演化衝突、`no coordinate to observe` 不寫。展開或未知鍵印出的多個座標共用該欄位的同一份追蹤，各寫各的座標。會話逐行變長，所以同一行再打一次是更長的問題，會再寫一顆。r6 的冪等是 `eval`：同一個點、同一句 `eval\n_.v`、同一個答案 `2` 不寫第二次。

**Q4** `record_observation` 先掃描再 `write_circle`，不拿提交鎖。兩個終端可以都通過比對、都鑄出一顆。最壞情況是兩顆葉，點、問題、答案相同。`load_circles` 兩顆都略過，主線不分叉，下一次注入不因它們記匯流。這不是遺失的寫入，`HEAD` 也不動。本弧不修。

**Q5** `ObservationState::Incomplete` 今天沒有構造點。預設策略是 Blur。〔量〕探針程式上 `eval '_.big'` 印

`#blur { %cause: #max_depth_exceeded, %caid: "hash:sha256:v1:71a58e16c7a5399b4df8026f5d06bd351960a8c240a32ed56ef7ba857a12c425" }`

rc 0，並寫了一顆 ○（答案就是這一行）。一次性觀測讀的是已提交的根。提交 `~%Config` 被拒（`note: ~%Config was not committed (horizon parameters stay staged as session state)`），所以暫存的 `#strict`／`#approximate` 不改變這一次一次性觀測。`handle_resource_exhausted` 裡嚴格策略的燃料、逾時、深度是 `_|_`，近似策略是帶 `#approximate` 的原子並記視界；那些是那條函式的路徑。本弧不另寫 `#incomplete`。r3 的輸出也接受 `incomplete` 這個字；量到的預設路徑是 `blur`。

### 8.5 探針

`crates/oo/tests/what_an_observation_leaves_probe_test.rs` 未改，未 rustfmt。全樹第三輪 15／15，2.70s。r1–r10、g1–g5 皆綠。

紅線，全樹第三輪皆 `test result: ok`、0 failed：`a_savepoint_that_knows_where_it_stood` 14（0.94s）、`a_history_older_than_its_savepoints` 13（0.93s）、`the_universe_you_are_in` 14（1.23s）、`an_interactive_eval` 11（0.73s）、`where_there_is_no_universe` 13（3.75s）、`one_writer_at_a_time` 10（17.57s）、`what_the_top_dropped` 15（2.21s）。

交叉編譯 `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu`：`Finished` 4.17s，`^error` 0。

### 8.6 數字

1. 三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`：每輪 rc=0，`^error` 0，`test result:` 258 行，2496 passed，0 failed。`running`／`test `／`test result:` 三輪各 3012 行，去掉耗時後相同。原始日誌的警告順序不必相同。
2. conformance 162／162，rc=0。cwd 是 `/home/gali/nlang`。引擎是 `nlang-tools/target/release/oo`。
3. `~%Math./add (1, 2)` → 3，`(1, 3)` → 4，rc=0。這兩次 `eval` 沒有建立 `.oo/`。euid 1000。`/tmp/oo-ephemeral-*` 為 0。
4. `oo inspect <HEAD>` 的 `root:`：`x: 0` 為 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。`v: 1 + 1` 與 `v: 1+1` 為 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`（`status` 該行帶 `(available)`）。新倉 `.oo/format` 為 `layout=9`，`.oo/objects.format` 為 `encoding=5`。

### 8.7 你認為需要改規格之處

規格檔這次沒有改。建議驗收方依工單第 6 節：`SPEC_10` §3.1 移除自陳缺口並寫入 D91 四款；`REAL_02` §5.1.1 寫 `layout=9` 與被鎖在門外的版本（自 8 為 v0.64.0 … v0.74.0）；`REAL_01` §1.1 寫一次性求值留下觀測 ○；並收 `CHANGELOG`。

---

## 9. 驗收（驗收方填）
