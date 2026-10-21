# 工單：一個表沒收下的寫入（Q-098）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-098（Inbox「⚠ 工作集裡已有頂層模式鍵，之後演化寫進的普通欄位回 0 而丟掉」）／**依據 `SPEC_07` §1 與 D101（已寫；不須新裁定）**
> 探針（已預先提交並校準）`crates/oo/tests/a_write_the_table_did_not_keep_probe_test.rs`（整支約 4 秒）
> 基線：dev（本工單所在提交）／`oo v0.98.0` ⟹ 探針 **2 綠 3 紅**；conformance **181／181**。

## 0. 判準（規格原文）

*   **`SPEC_07` §1**：「有至少一個模式鍵 `@{ … }:` 的 Combo 是一張**分派表**……表內每個非 meta 鍵都是一個**輸入約束**」；「**同一個約束出現多次是同一個分支，所有本體相交**——不論它們寫在同一個字面裡、經 `&`、還是經後續的演化聚在一起」。
*   **`SPEC_07` §1（D101）**：「**一張表只有一個正規形**……字面、`&`（兩向）、`|>`、展開 `...`、根的逐次演化，產生**同一組位元組**」。
*   **`SPEC_07` §1（D100）**：表上的導航讀分支存著的值；根有模式鍵即是表（D97）。
*   **§2.2 第 2 款**（`WORK_QUEUE`）：無聲的資料遺失、回報成功。

## 1. 缺陷

〔量，標籤建置；腳本 `nlang-baselines/acc/q98/tablews.sh`〕

*   **形**：演化 `@{ @int }: 0`，再演化 `x: 3`，提交。兩次演化退出碼 0；第二次演化後 `oo status` 不列 `x: 3`；提交後 `_.x` 讀 `_`。同兩筆寫在同一檔、或中間先提交一次，`_.x` 讀 `3`。
*   **同名**：`@{ "b" }: 2` 再演化 `b: 3` ⟹ `_.b` 讀 `2`（根 `11fb2ba3…`）；同一檔、先後倒過來、中間提交，根皆 `b80f5fb0…`、`_.b` 是 ⊥（同一分支兩個本體相交）。`@{ 4 }: 2` 再演化 `4: 3` ⟹ `_.4` 讀 `2`。
*   **不限同名、不限形**：工作集一旦是表，之後每一次演化寫的資料鍵都丟——具名、引號、點號、記錄、私有欄位旁的讀法、函式、配方。`/f` 這種 Logic 欄位不丟；同一次演化裡另帶一個模式鍵時不丟（成員本身也是表）。
*   **對照，今天對的**：欄位座標裡的表（`t: { @{ "b" }: 2 }` 之後 `t.b: 3`）、模式鍵對模式鍵、提交之後再寫、`--pin`。
*   〔讀〕每次演化之後，`restamp_thunk_effects`（`universe.rs`）依這次演化寫下的鍵（`session_delta`），回到工作集以 `get_field(k)` 取值，重建這次演化的成員。工作集是表時資料鍵是分支（存在 `%rules` 裡），`get_field("x")` 取不到，那個鍵就被跳過，成員寫成 `{}`。〔量，插桩〕合併本身是對的：交會結果有兩個本體，`session_delta` 是 `{ b: 3 }`，重建後成了 `{}`。
*   **v0.79.0 起的回歸**（D100–D102，Q-077：表的資料鍵是分支）；v0.75.0–v0.78.0 該格讀 `3`。

## 2. 依據；位址

*   **不須新裁定**：`SPEC_07` §1、D101（已寫）。
*   **位址**：受影響的格子，根變成同一組寫入寫在同一檔時的根（`b80f5fb0…` 等）；**工作集從來不是表的演化，成員與根逐位元組不動**（g1 六個根、g2 三個成員，與 v0.98.0 同）。沒有新的根形，不動編碼。

## 3. 射程＝不變式（不是機制）

*   **M1 一次退出碼 0 的演化，寫下的每一筆都在它的成員裡。** 同一組寫入，不論分成幾次演化、同一檔或分檔，提交前的工作集（`oo status`）相同、提交後的根逐位元組相同、讀數相同——不論工作集是不是表，不論寫入是什麼形（具名、引號、點號、記錄、私有、函式、配方）（r1、r3）。
*   **M2 同一個約束不論先後都相交**：模式鍵與同名資料鍵、先寫哪一個，根相同（r2）。
*   **另一半——不碰表的不動**：工作集從來不是表的演化，成員與根逐位元組與 v0.98.0 同（g1、g2）。
*   若你找到另一處「依寫下的鍵回到工作集或根取值，而表把那個鍵存成分支、於是取不到」的地方，一併在射程內，在 8.1 列出。

## 4. 紅線（今天綠，必須保持綠）

*   g1、g2；Q-097 `a_record_the_store_wrote_n_times`；Q-095 `a_spread_that_landed_on_a_reading`；`a_working_set_that_is_a_set`；`a_savepoint_that_knows_where_it_stood`；`pin_probe_test`；表族：`a_table_you_cannot_read`（含 R-1）、`a_table_that_stops_being_one_when_printed`、`a_table_that_runs_every_branch_when_looked_at`、`a_table_read_before_its_spread_landed`、`a_table_that_forgot_its_patterns`（含 R-1、R-2）、`a_table_that_lost_its_patterns`；`a_private_never_committed`（含 R-1）。
*   conformance **181／181**（`nlang-spec` 的 `local`；**空目錄裡跑 runner**）。
*   **交叉編譯** `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   身分紅線：`x: 0` 根 `b74d7758…`；`v: 1 + 1` 與 `1+1` 根 `df91a8ad…`；`f: (x -> x + 1)` 根 `258a1a08…`；`k: { a: 1, _: 9 }` 根 `420a0c9f…`；`m: (_ -> 1)` 根 `cf11d2ff…`；標準根 `c96c2937…`；新倉 `layout=9`／`encoding=8`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1 入口清單**：列出演化、存檔、讀回、提交路徑上，每一處「以寫下的鍵（或座標）到工作集或根裡 `get_field`／`get_local_field` 取值」的地方：呼叫點、取的是什麼、工作集或根是表時取得到嗎、取不到時做什麼、你改了沒有，各一格量測。至少包括 `restamp_thunk_effects`；`evolve_field` 的 D110 檢查（`held`）與 G2-S；`proposals_at`（成員是否已在 HEAD）；`session_would_be_listed`；`staged_has_committable_content`；`injection_positions_equal`。取不到而照規格是對的（例如表的分支衝突只屬於該分支，`SPEC_07` §1），寫明依據。
*   **Q2 已經丟掉的寫入**：v0.79.0–v0.98.0 在這一格寫下的成員是 `{}`。以 v0.98.0 寫下這樣的工作集、交給修好的引擎 `status`、`evolve`、`commit`：各是什麼（預期：寫入已不在，沒有錯誤；本卡不追回）。
*   **Q3 還有哪條路**：修好之後，還有哪一條路會讓一次退出碼 0 的演化寫下的東西不進提交的根？至少量 `oo evolve a.n b.n`（一個命令兩個檔，第一個檔讓工作集成為表）在新工作區與既有工作區、以及 `--pin --grant pin`。各一格（或說明找不到）。

## 6. 明文不在射程內

*   表的分支衝突在演化時收下、讀時 ⊥（`b: 3` 之後 `@{ "b" }: 2`）：照 `SPEC_07` §1（衝突只屬於該分支），2026-10-22 盤點已結那一列，不改。
*   成員裡存的是工作集的合併值還是這次演化自己的值（Q-014 的形）：不碰表的照舊（g2）。
*   規格文字、向量與 `CHANGELOG`：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1、r2、r3 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。** `rustfmt` 不得掃探針檔。
*   〔驗收方校準〕**兩極皆做**：
    基線 v0.98.0（dev 加本探針）：**2 綠 3 紅**——r1 紅 24／56（八種寫入形 × 工作集、根、讀數）、r2 紅 11／33（同名相撞、一致、整數鍵、已有欄位的表，各為工作集與根；一致那格讀數同為 `3`、工作集與根不同）、r3 紅 9／21；g1、g2 綠。
    參考實作（`q098-ref 9b2fcf5`，相對 dev 引擎一處：`restamp_thunk_effects` 在工作集取不到某個寫下的資料鍵時，保留這次演化自己的值，不再跳過）⟹ 本探針 **5／5**；全樹 **2771 通過、0 失敗**；conformance **181／181**（空目錄，前後皆空）；17 格、位址列、身分紅線、展開矩陣、`hunt`、`inv94`、`hunt5`、Q-095 偵察矩陣（七支）與 v0.98.0 逐行同；新引擎寫的成員交給 v0.98.0 提交，根與同一檔同（`40a72219…`）。
    **變異**：照舊跳過 ⟹ r1、r2、r3（即 v0.98.0）；一律取這次演化自己的值（不取工作集的）⟹ **只有 g2**；取不到時放進私有軸 ⟹ r1、r2、r3。**存活而從參考實作拿掉的兩處**：私有軸也加退路（表不搬私有欄位，從未走到）、退路上重算 thunk 的效應標記（三種前向引用帶效應的格子，成員位元組逐位元組同；`acc/q98/eff.sh`）。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

引擎與 `q098-ref 9b2fcf511166a1a024db820596602d45b1443890` 的 `universe.rs` 逐位元組相同。改動只有 `restamp_thunk_effects`（`universe.rs:1535`）：工作集取不到這次演化寫下的資料鍵時，成員留下這次演化自己的值（`:1551`）。取得到的仍用工作集上已經刷新過 thunk 的值。私有軸那個迴圈（`:1556`）沒有加退路。`oo` 仍是 `0.98.0`，interpreter 仍是 `0.2.2`。探針未改。

*   **M1** `@{ @int }: 0` 再演化 `x: 3`：兩次退出碼 0。`oo status` 列 `@{ @int }: 0` 與 `x: 3`（熵 2266），與同一檔相同。第二次演化的成員是 `{ x: 3 }`。兩邊提交退出碼 0，根都是 `40a722190555b103fd49d61b5b9cdd82bec17a5e60f15a3be39771d8285d83af`，`_.x` 是 `3`。
*   **M2** `@{ "b" }: 2` 再演化 `b: 3`，與同一檔，根都是 `b80f5fb08691d64667263b37687392cbafc6c313e3a02360bed1124a4e7d848e`。`_.b` 是 ⊥，`#conflict`，2 對 3。探針 r2 同時蓋住倒過來的順序。
*   **另一半** 探針 g1、g2 通過。抽查：`x: 3` 再 `y: 4` 根 `9f4688480a4a9ce2cf664796eaa3d8073ba7dcc656281718d69ac4f7f4389cc1`；`t: { @{ "b" }: 2 }` 再 `t.b: 3` 根 `2fa352ca8b18f80ebc86f640b2483072eeda01879cad92402e9071c0f1e9136d`。
*   其餘「用寫下的鍵回工作集或根取值」的地方見 8.4 Q1。它們不重建成員，這張卡沒有改。`/f: 1` 接在表之後，`get_field` 取得到邏輯軸，成員是 `{ /f: 1 }`，根 `afd68d396c68ba3a3d1a6416cd294a0d07843bae182d8a0d3b673b493bc1acb9`，與 v0.98.0 對同一組寫入提交出的根相同。

### 8.2 順手改動（逐項指名）

沒有。

### 8.3 工單哪裡是錯的

沒有。

### 8.4 工單指名要你回答的問題

**Q1** 取值點。表把資料鍵收成 `%rules` 裡的分支時，`get_field` 在 `data` 裡看不到那個鍵。

| 地點 | 取的是什麼 | 工作集或根是表時 | 取不到時 | 改了 |
|---|---|---|---|---|
| `restamp_thunk_effects` `:1546` | 這次演化寫下的鍵，回到工作集 | 資料鍵取不到；`/f` 這類邏輯鍵取得到 | 成員寫入這次演化自己的值 | 改了 |
| `evolve_field` D110 `held` `:921` | 這次要寫的座標，在工作集與根 | 資料鍵取不到，這次檢查跳過該座標 | 不在這裡拒絕 | 沒有 |
| G2-S `:989` | 這次要寫的座標，只看根 | 根是表時，資料鍵取不到 | 不在這裡拒絕 | 沒有 |
| `proposals_at` `:1636` | 整個成員與 HEAD 根的交會，不按鍵 `get_field` | 成員 `{ x: 3 }` 會移動位置 | 空成員 `{}` 與表的根交在原位，提交時略過 | 沒有 |
| `session_would_be_listed` `:1731` | 整個 `session_delta` 與空根的交會 | 成員裡有 `x: 3` 時 `status` 會列出 | 空成員不增加可列的寫入 | 沒有 |
| `staged_has_committable_content` `:178` | 工作集各軸是否有內容，不按這次的鍵 | 表的 `%rules` 使它可提交 | 空成員自己沒有可提交內容 | 沒有 |
| `injection_positions_equal` `:1773` | 寫入前與寫入後的工作集，各自與 HEAD 根交會的位址 | `x: 3` 讓兩側位址不同，多鑄一圈 | 兩側相同時不鑄圈 | 沒有 |

格子（皆退出碼如所述，修復後的 `oo`）：

*   **restamp** 見 8.1 M1。v0.98.0（`/home/gali/nlang-baselines/v0.98.0-verify/target/release/oo`）對同一組兩次演化，第二次成員是 `{}`，`status` 只列 `@{ @int }: 0`。
*   **D110** `x: 1` 再 `x: 2`：第二次退出碼 1，`Evolution Conflict` at `x`，工作集仍是 `x: 1`。`b: a` 再 `b: 3`：退出碼 0，`status` 為 `b: 3 & a`（與 v0.98.0 相同；這格的工作集不是表）。`@{ "b" }: 2` 再 `b: 3`：退出碼 0，根 `b80f5fb0…`，讀數 ⊥ `#conflict`。分支上的衝突屬於該分支（`SPEC_07` §1），演化收下。
*   **G2-S** 先提交 `x: 1`，再演化 `x: 2`：退出碼 1，`#conflict` at `x`。先提交 `@{ @int }: 0`（根 `b0e1e952dc9b3cd0a57c708de0e09f616eb75b4e49d7fbd1f3c1ef5dbfe94738`），再演化 `x: 3` 並提交：退出碼 0，根 `40a72219…`，`_.x` 是 `3`。先提交 `@{ "b" }: 2`，再演化 `b: 3` 並提交：退出碼 0，根仍是 `b80f5fb0…`，`_.b` 是 ⊥ `#conflict`。
*   **proposals_at／listed／可提交** 已有 `p: 1` 的提交之後，演化表，圈數 2 到 3，`status` 列 `@{ @int }: 0`。再演化 `x: 3`，圈數 3 到 4，`status` 同時列 `x: 3`。提交退出碼 0，根 `de41a7f8bea55eec3ac84a8f3d6badac29f4e80784706c17305e786f79f91cb9`，`_.p` 是 `1`，`_.x` 是 `3`。
*   **邏輯鍵** 見 8.1。`get_field("/f")` 取得到，退路沒有走到。

**Q2** v0.98.0 在空工作區演化 `@{ @int }: 0`、再演化 `x: 3`，兩次退出碼 0，第二次成員本體是 `{}`。把這個 `.oo` 交給修復後的引擎：

*   `status` 退出碼 0，只列 `@{ @int }: 0`（熵 1560）。
*   直接 `commit -m a` 退出碼 0，根 `b0e1e952…`（與只提交那張表相同），`_.x` 是 `_`。
*   另一次：`status` 相同之後演化 `z: 1`，退出碼 0，`status` 列表與 `z: 1`；提交退出碼 0，`_.x` 是 `_`，`_.z` 是 `1`。

已經寫成 `{}` 的那筆沒有被找回。

**Q3** 修復之後量到的這幾條，退出碼 0 的演化都進了提交的根：

*   新工作區 `oo evolve a.n b.n`（`a.n` 是 `@{ @int }: 0`，`b.n` 是 `x: 3`）：退出碼 0，`status` 列兩者，一次命令寫下一個成員。提交根 `40a72219…`，`_.x` 是 `3`。
*   既有工作區（已提交 `p: 1`）同一命令：退出碼 0，`status` 列表與 `x: 3`。提交根 `de41a7f8…`，與上面分成兩次演化的根相同。`_.p` 是 `1`，`_.x` 是 `3`。
*   `--pin --grant pin`：先演化表，再 `evolve --pin --grant pin` 寫 `x: 3`，退出碼 0，成員 `{ x: 3 }`，`status` 列 `x: 3`。`commit --grant pin` 退出碼 0，根 `40a72219…`，`_.x` 是 `3`。

同一張表之後分三次演化 `c: { b: 2 }` 與 `...c`，根 `7f9ebbc043d433ce4b87f801851fb93d449be20df0c2438d92575f02f0b87df9`，與這三行寫在同一檔的根相同。`_.c.b` 是 `2`，`_.b` 是 `_`（同一檔也是 `_`）。v0.98.0 跑這三次時 `status` 只剩那張表。

### 8.5 探針

`crates/oo/tests/a_write_the_table_did_not_keep_probe_test.rs` 未改，`rustfmt` 未掃它。單獨一輪 **5／5**，`finished in 9.11s`，`PROBE_EXIT` 0（該輪 `cargo` `Finished` `release` 在 22.40s）。全樹第一輪裡這支是 **5／5**，8.77s。

### 8.6 數字

*   三輪皆 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`，`EXIT` 0。2026-10-22 01:10:09 牆鐘 1244.85s；01:30:54 牆鐘 871.24s；01:45:25 牆鐘 867.30s。
*   歸一化 3635 行，三輪 sha256 同為 `c8b98c1ee5b24437a0f383a3be3f63e29c9dce36d138947a2e085de496973235`。`test result:` 289 行，加總 **2771** 通過、**0** 失敗。以 `test ` 起、以 ` ok` 收的行 2770 行：`g1_a_sequential_conflict_is_still_refused` 把 `Error: Evolution Conflict…` 印在測試名那一行，`ok` 在下一行。`Running ` 286。原始位元組 293607／293452／293453。原始 sha256 前綴 `60d59ccd0e25f043`、`b3cc65aeacd3977c`、`5fdb7a629545d682`。`^error` 0，`BrokenPipe` 0，`panicked` 0。
*   第一輪紅線（皆 ok）：本探針 5／5 8.77s；Q-097 7／7 10.05s；Q-095 3／3 23.25s；`a_working_set_that_is_a_set` 6／6 1.30s；`a_savepoint_that_knows_where_it_stood` 14／14 1.42s；`pin_probe_test` 15／15 2.22s；`a_table_you_cannot_read` 12／12 3.01s、R-1 4／4 1.42s；`a_table_that_stops_being_one_when_printed` 4／4 2.30s；`a_table_that_runs_every_branch_when_looked_at` 7／7 2.81s；`a_table_read_before_its_spread_landed` 11／11 2.08s；`a_table_that_forgot_its_patterns` 17／17 2.41s、R-1 6／6 0.62s、R-2 10／10 1.65s；`a_table_that_lost_its_patterns` 10／10 8.73s；`a_private_never_committed` 10／10 4.05s、R-1 3／3 0.48s。同輪還有：`caused_top` 11／11 1.12s；`static_cycle` 17／17 0.67s；`math_union` 17／17 0.92s；`what_an_observation_leaves` 15／15 3.89s；`blur_spread` 13／13 2.83s；`a_store_written_in_another_language` 13／13 0.94s；`every_byte_or_none` 14／14 1.92s；`atomic_write` 8／8 1.47s；`an_axis_the_address_forgot` 17／17 6.26s（epochs 4／4 0.32s，union 2／2 0.00s，bytes 1／1 0.23s）；`forward_spread` 12／12 0.82s；`spread_collision` 25／25 0.86s；`spread_privacy` 11／11 0.35s；`spread_splice` 9／9 0.15s；`import_is_spread` 7／7 0.44s；`a_name_spread_in` 15／15 6.10s；`where_there_is_no_universe` 13／13 4.14s。
*   身分：`x: 0` 根 `b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f`；`v: 1 + 1` 與 `1+1` 根 `df91a8ad239ea8727c3a03a71eb4b0a787312b92306db4d8b124a73faaf40199`；`f: (x -> x + 1)` 根 `258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f`；`k: { a: 1, _: 9 }` 根 `420a0c9f4592b49ace8c8aacec690fd0a70953b1bd2158260e68c3acce710d9e`；`m: (_ -> 1)` 根 `cf11d2ff2e95c11ecbae5ceba00ece7c60f87c7d81149287ffebce1b551a89a6`；標準根 `c96c29378c9395936ea4999db37e9b6a28291b57a47d4c016ca3cdf6833fed3b`。新倉 `layout=9`、`encoding=8`。
*   conformance **181／181**，空目錄 `/home/gali/tmp/q098/conf.iumb`，`CONF_EXIT` 0。父倉 `/home/gali/nlang/.oo/format` 在 Windows 前、Windows 後、conformance 後都是位元組 `32`。
*   Windows：`cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu`，`Finished release in 4.37s`，`WIN_EXIT` 0，`^error` 0。該目標 lib 15 則警告（`cargo fix` 會動 13 則）。Linux 同一 lib 14 則（`cargo fix` 12 則），見 `/home/gali/tmp/q098/probe.log`。都沒有修。
*   `target/release/oo` 11149320 位元組，mtime 2026-10-22 01:04:40 +0800。三輪與 Windows 都沒有重新連結它。編譯時工作區是髒的，此處不報 `--version`。

### 8.7 你認為需要改規格之處

沒有。

## 9. 驗收（驗收方填）
