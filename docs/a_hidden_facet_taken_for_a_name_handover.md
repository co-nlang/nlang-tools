# 工單：一個隱藏的面向，被當成了一個名字（Q-099）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-099（Inbox「隱藏型別鍵 `~@x:` 與隱藏邏輯鍵 `~/x:` 被無聲丟掉」）／**依據 D115**（2026-10-22 用戶裁定「甲」）
> 探針（已預先提交並校準）`crates/oo/tests/a_hidden_facet_taken_for_a_name_probe_test.rs`（約 2 秒）、`crates/interpreter/tests/a_hidden_facet_taken_for_a_name_bytes_probe_test.rs`（位元組級）
> **預先修訂**：9 支既有探針 15 處（新倉 `encoding=9`、`migrate` 的停用範圍到 v0.99.0），標 `AMENDED 2026-10-22 for Q-099 (D115)`。
> 基線：dev（本工單所在提交）／`oo v0.99.0` ⟹ CLI 探針 **2 綠 3 紅**、位元組探針 **1 綠 1 紅**；conformance **181／181**。

## 0. 判準（規格原文）

*   **D115**：隱藏型別 `~@a` 與隱藏邏輯 `~/a` 的位址照 `REAL_03` §6.2——**軸位元組 7、8，名字不帶前綴**。舊倉照宣告：宣告 `encoding≤8` 的倉照 v0.99.0 的規則寫與驗；`encoding` 8 → 9 的新倉用軸 7、8；讀先新後舊；`oo migrate` 推進並說出鎖掉的版本。
*   **`REAL_03` §6.2**：序列化前排序表第 7 列 `~@`（hidden Type）、第 8 列 `~/`（hidden Logic）。
*   **`SYNTAX_05` §4 #1**：「`a`、`@a`、`/a`（及其 `~` 隱藏版）可在同一容器中**並存**——同一座標的不同面向，索引不同表，**不**互相覆寫。」§2 #2：`~` 只影響路徑可達性。
*   **`SPEC_04` §3.1**：向內可見、向外 `#private_access_violation`、外部顯示剝除私有軸。
*   **`REAL_02` §5.1.1**（D106 第一輪更正）：寫入照容器的宣告，不因寫入而推進宣告。

## 1. 缺陷

〔量，`v0.99.0` 標籤建置；腳本 `nlang-baselines/acc/q99/recon.sh`、`cross.sh`〕

*   〔讀〕文法收 `~@a`／`~/a`（`n.pest` `prefix`），`parse_field_key` 把 `~` 之後整串當名字：`~@T` ⟹ 私有鍵 `"@T"`、`~/f` ⟹ 私有鍵 `"/f"`。就地多半能用：`{ ~/inc: (x -> x + 1), r: /inc 2 }.r` ⟹ `3`、`.~/inc` ⟹ `#private_access_violation`。
*   **一、同名的面向互相串**：`{ ~a: 2, ~@a: 3, k: a }.k` ⟹ `3`（先後倒過來同）、`k: @a` ⟹ `2`；`{ ~a: 2, ~/a: (x -> 0), k: a }.k` ⟹ 箭頭、`k: /a 5` ⟹ ⊥。〔讀〕作用域框的精確查找寫成 `scope.local_fields().get(name)`，而 `local_fields()` 的鍵帶 `~`，永遠查不到；接著去前綴 fallback 依 `/`、`@`、`~` 的順序試，先找到 `"@a"`。暫存與根的查找用 `get_local_field(name)`，是對的。v0.50.0 同。
*   **二、存不進倉**：`~@T` 存檔印成 `~"@T"`，自己讀不回——`t: { ~@T: @int, r: 5 & @T }`、根上的 `~@T`、根上的 `~/f` 演化 0、提交 `store n/ parse` 拒；`SPEC_05` §5.1 的泛型範例本身提交即拒。欄位裡的 `~/f` 提交成功（存成配方，因為它的答案印不回）。
*   **三、位址**：私有軸 6、名字 `"@T"`／`"/f"`；規格要軸 7、8。

## 2. 依據；位址

*   **D115**（裁定已下）。
*   **位址**：宣告 9 的倉裡，帶隱藏型別或隱藏邏輯的值位址移動（`{ ~@a: 1 }` 的原像 `80 01 01 07 01 61 12 01`）。**宣告 ≤8 的倉照 v0.99.0 的規則算**——v0.99.0 驗得過、讀得開；那裡的根與 v0.99.0 寫的不同只在一種格子：v0.99.0 因為答案印不回而存配方（`t: { ~/f: … }` `879c3df4…`），修好印出之後存答案（`f42079d4…`），v0.99.0 照樣驗過、讀 `3`。**不帶這兩種鍵的值，兩種宣告下都不動**（g1）。

## 3. 射程＝不變式（不是機制）

*   **M1 同名的面向各讀各的**：`a`／`~a` 讀資料（公開先、隱藏後），`@a`／`~@a` 讀型別，`/a`／`~/a` 讀邏輯——不論先後、就地或提交後（r1、g2）。
*   **M2 隱藏面向寫得下、讀得回**：根上、欄位裡、`SPEC_05` §5.1 的範例，提交成功、讀數與就地同；外部照舊擋（`#private_access_violation`）、顯示照舊剝除（r2）。
*   **M3 位址照宣告**：宣告 9 的倉寫軸 7、8、裸名（位元組級，r4）；宣告 ≤8 的倉照 v0.99.0 的規則寫，v0.99.0 開得了、驗得過；讀先新後舊——遷移前寫的物件遷移後照讀（`log`、讀數、gc、對等點抓取）（r3）。
*   **M4 宣告**：新倉 `encoding=9`；`oo migrate` 由 8 推到 9 並說出 `oo v0.98.0 through v0.99.0`；v0.99.0 開 9 以「encoding version 9 is not supported」具名拒絕（r3）。
*   **另一半——不碰隱藏型別與隱藏邏輯的不動**：根逐位元組同（g1，兩種宣告）、私有資料鍵 `~a` 照舊軸 6（g3）。
*   若你找到另一處「依私有軸的名字判斷、而名字帶 `@`／`/` 時判錯」或「算位址／驗位址而不照倉的宣告」，一併在射程內，在 8.1 列出。

## 4. 紅線（今天綠，必須保持綠）

*   g1、g2、g3；預先修訂的 9 支（`a_record_the_store_wrote_n_times`、`a_refusal_written_as_an_answer`、`a_spread_at_the_root`、`a_store_written_in_another_language`、`a_type_you_could_not_carry`、`an_axis_the_address_forgot`、`atomic_write`、`every_byte_or_none`、`nothing_here_or_nothing_you_can_see`）；`spread_privacy`、`a_private_never_committed`（含 R-1）、`type_super_probe_test`；Q-098、Q-097、Q-095 探針；表族（`a_table_*`）。
*   conformance **181／181**（`nlang-spec` 的 `local`；**空目錄裡跑 runner**）。
*   **交叉編譯** `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   身分紅線：`x: 0` 根 `b74d7758…`；`v: 1 + 1` 與 `1+1` 根 `df91a8ad…`；`f: (x -> x + 1)` 根 `258a1a08…`；`k: { a: 1, _: 9 }` 根 `420a0c9f…`；`m: (_ -> 1)` 根 `cf11d2ff…`；標準根 `c96c2937…`；新倉 `layout=9`／**`encoding=9`**。
*   `first_engine_that_opens_before` 的 `encoding=9` 列寫 **`v0.100.0`**（下一個切版號；`oo --version` 由標籤決定，不在程式裡）。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1 算位址與驗位址的入口**：列出每一處算一個值的位址或驗一個存著的位址的地方——`put_value`、`put_root`、`put_commit`、`value_matches_stored_address`（讀、gc、對等點抓取）、`open_commit`、`StandardRootSet`、`~%Engine./save`、`~%Discovery./identify`、分派表的分支名（`rekey_identity_names`）、`content_hash` 的 memo——各在宣告 8 與 9 的倉裡用哪一條規則、你改了沒有，各一格量測（或說明為什麼走不到隱藏面向）。
*   **Q2 名字解析的入口**：列出每一處以裸名或路徑找欄位的地方（作用域框、暫存、根、標準根、路徑投影、分派表的鍵），各自的精確查找是否先於去前綴 fallback；每處量 `{ ~a: 2, ~@a: 3, k: a }` 形的一格與外部 `.~@T` 一格。
*   **Q3 跨引擎**：(a) v0.99.0 開本引擎寫的 8 倉——`t: { ~/f: … }`、根上的 `~@T`、一張模式裡含 `~@` 鍵的表：`log`、讀數；(b) v0.99.0 開 9 倉：訊息；(c) 本引擎開 v0.99.0 寫的 8 倉，遷移前後：`log`、讀數、`gc`。

## 6. 明文不在射程內

*   表達式節點表（`REAL_03` 欄位鍵 `Named` 的前綴位元組）：`~@T` 在表達式裡照舊是 `Private` 加名字 `"@T"`——那是原始碼的拼法，`~"@T"` 不是合法拼法，不會撞名。
*   `SPEC_05` §5.1 範例的 `bad: { value: "hi" } & @IntBox` 讀 `{ value: "hi" }` 而非 ⊥：連公開的 `@T` 也一樣（型別參數在 `@Box` 定義時就綁住），是泛型的另一件事，另開 Inbox。
*   v0.99.0 寫壞的工作集（成員裡存著 `~"@T"`）：v0.99.0 自己就打不開（`Universe unavailable`）；本卡不讓存檔剖析器接受 `~"…"`（那是第二種拼法）。
*   跨框的去前綴 fallback（Inbox「去前綴 fallback 使模糊比對贏過精確比對」，待裁）：本卡只修同一框裡精確查找失效的那一處。
*   規格文字、向量與 `CHANGELOG`：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r4 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。** `rustfmt` 不得掃探針檔與預先修訂的 15 處。
*   〔驗收方校準〕**兩極皆做**：
    基線 v0.99.0（dev 加兩支探針與 15 處修訂）：CLI **2 綠 3 紅**——r1 紅 15／18（六組同名配對，就地與提交後；含 `~@` 的提交即拒）、r2 紅 10／14、r3 紅 7／15（新倉宣告、根、遷移）；g1、g2 綠。位元組 **r4 紅、g3 綠**。
    參考實作（`q099-ref e49b150`，相對 dev 引擎五個檔：作用域框精確查找、存檔與值兩處印出、身分軸 7／8 與「照私有軸算」模式、倉宣告 9 與讀先新後舊、`migrate` 說明）⟹ 兩支探針 **5／5、2／2**；全樹 **2778 通過、0 失敗**；conformance **181／181**（空目錄，前後皆空）；17 格、位址列、展開矩陣、`hunt`、`hunt5`、Q-095 偵察七支、`acc/q98` 兩支與 v0.99.0 逐行同，`inv94` 只差 `~@int` 那一格（提交由拒變收）；身分紅線只差新倉 `encoding=9`；跨引擎（`acc/q99/cross.sh`）：v0.99.0 開本引擎的 8 倉 `log` 0、讀數對，開 9 倉具名拒絕，本引擎開 v0.99.0 的 8 倉遷移前後讀數對。
    **變異**（各被至少一格擋下）：不修作用域框查找 ⟹ r1；不修存檔印出 ⟹ r1–r3；不修值的印出 ⟹ r3（8 倉存配方、根不同）；身分一律舊規則 ⟹ r3、r4；8 倉也寫新規則 ⟹ r3；讀不試舊規則 ⟹ r3；`content_hash` 的 memo 不避開 ⟹ r3。**存活而從參考拿掉**：`open_commit` 的舊規則退路（提交本體不帶使用者的值，走不到）。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

引擎六個檔與 `q099-ref e49b15055269efb785c79130b563f5c2739d014f` 逐位元組相同（`git diff --exit-code` 退出碼 0）。`oo` 仍是 `0.99.0`，interpreter 仍是 `0.2.2`。兩支探針與預先修訂的 15 處未改。

*   **位址** `bn_serial.rs` `local_entry`（`:58`）：舊身分與「照私有軸」都關著時，`@` 開頭是軸 7、`/` 開頭是軸 8，名字去掉那一個前綴；其餘（含私有資料 `~a`）仍是軸 6、名字原樣。`with_hidden_facets_as_local` 重算 v0.99.0 的規則（軸 6、名字帶 `@`／`/`）。
*   **寫** `storage.rs`：`OBJECT_ENCODING_VERSION` 與 `HIDDEN_FACET_ENCODING` 都是 9。`put_value`（`:624`）、`put_root`（`:653`）、`put_commit`（`:803`）在宣告 `< 9` 而且旗標還沒設時，包一層 `with_hidden_facets_as_local` 再寫。宣告 `≤5` 仍先走 `with_legacy_identity`（`put_value`／`put_root` 在那一層裡 `rekey_identity_names`）。寫入不推進宣告。
*   **讀** `value_matches_stored_address`（`:150`）：先當前的 `content_hash`，再隱藏面向當私有軸，再舊身分。`get_value` 走這裡，所以讀、gc、對等點抓取都先新後舊。`open_commit`（`:882`）沒有隱藏面向的退路；v1 對不上時只試舊身分。
*   **印出** `store_codec.rs` `quote_key`（`:953`）與 `value.rs` `quote_nlang_field_key`（`:660`）：前綴是 `~` 而且 `hidden_facet_name` 成立時印 `~@a`／`~/a`。`ComboVal::content_hash`（`:3930`）在舊身分或隱藏當私有軸時不讀、不寫 memo。
*   **查找** `lib.rs:4224`：同一作用域框的精確查找由 `scope.local_fields().get(name)` 改成 `scope.get_local_field(name)`（`value.rs:1272` 去掉一個前導 `~` 再查 local 表）。前綴 fallback 的順序仍是 `/`、`@`、`~`、`~%`，跨框的那一條沒有動。
*   **migrate** `main.rs` `migrate_cost`（`:2365`）：目標到 9 時，鎖掉的最新引擎是 `v0.99.0`。`first_engine_that_opens_before`（`:2475`）在 encoding 不是 0 到 8 時寫 `v0.100.0`。

*   **M1** 就地 `{ ~a: 2, ~@a: 3, k: a }.k` 是 `2`。提交同一組（根 `faa4854d5c1b1df8fc40ae0d02dabdfa4c237230b2c6c2319296c043d8f40b51`）之後 `_.k` 是 `2`。
*   **M2** 宣告 9 的 `t: { ~/f: (x -> x + 1), r: /f 2 }` 根 `21e6e90ff86296bc70ceb297610288322aa287b970f69a79ffa97879a6298621`，`_.t.r` 是 `3`。根上的 `~a`／`~@a` 提交成功。外部 `_.~@a`、`_.t.~@a`、`{ ~@T: @int }.~@T` 都是 `#private_access_violation`。`_.t` 印出 `{ k: 2 }`。
*   **M3** 預先宣告 encoding=8 的同一份 `~/f` 來源，根 `f42079d4bcc8f79ce6a4b4802cdc87cd352233cff5558cd4c24ce525daa77b15`（答案，不是 v0.99.0 的配方 `879c3df4…`），`_.t.r` 是 `3`。v0.99.0 對這個 8 倉 `log` 退出碼 0、讀 `3`。本引擎 `gc --grant gc`：`4 objects, 3 reachable, 1 collectable (23 bytes)`，移除 1、釋出 23，之後 `_.t.r` 仍是 `3`。另一個沒有 `.oo` 的工作區先 `~%Discovery./connect` 到這個倉的目錄，再 `~%Discovery./fetch` 該根的完整位址，`.t.r` 是 `3`（走對等點的 `get_value`）。
*   **M4** 新倉 `layout=9`、`encoding=9`。上面那個 8 倉 `migrate --grant migrate` 退出碼 0，文句是「Migrating this store from layout=9 to layout=9 will make it unopenable by oo v0.98.0 through v0.99.0. Object encoding advances from encoding=8 to encoding=9.」宣告變成 encoding=9，`_.t.r` 仍是 `3`。最舊能開的引擎就是 v0.98.0，所以沒有再附加那句「open encoding=8 and do not open encoding=9」；主句已經點名 `oo v0.98.0 through v0.99.0`。`v0.100.0` 是 `first_engine_that_opens_before` 的 encoding=9 列，這次遷移沒有把它印出來。v0.99.0 開宣告 9 的倉：`log` 退出碼 1，`Error: object encoding version 9 is not supported by this engine (understands encoding 1 through 8); refusing to open`。
*   **另一半** 新倉身分：`x: 0` 根 `b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f`；`v: 1 + 1` 與 `1+1` 根 `df91a8ad239ea8727c3a03a71eb4b0a787312b92306db4d8b124a73faaf40199`；`f: (x -> x + 1)` 根 `258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f`；`k: { a: 1, _: 9 }` 根 `420a0c9f4592b49ace8c8aacec690fd0a70953b1bd2158260e68c3acce710d9e`；`m: (_ -> 1)` 根 `cf11d2ff2e95c11ecbae5ceba00ece7c60f87c7d81149287ffebce1b551a89a6`；標準根 `c96c29378c9395936ea4999db37e9b6a28291b57a47d4c016ca3cdf6833fed3b`。位元組探針 g3 綠（`~a` 仍是軸 6）。

另外看過、沒有改的兩處：

*   `~%Discovery./identify`（`builtins/disc.rs:398`）對施用後的值做 `content_hash`，旗標關著，不看倉的宣告。`{ ~@a: 1 }` 在宣告 9、宣告 8、沒有宇宙，摘要都是 `afe6e09ba85c1d9afe59451c3005aa7548f185aa1787159830f154285ebb8660`。同一值經 `~%Engine./save`（`engine.rs:150` 的 `put_value`）在宣告 9 是 `82b61edaab80c7300b3d318334be05aeaa5ee0f2f4dc88dd49f092f2310b7dfa`，在宣告 8 是 `3e6db63393e58483b9b7ba8e2169038cc12979286e764c5495cfd56cae811bca`。沒有宇宙時 save 是 `#no_universe`。
*   `hash_recursive_with_salt` 雜湊的是 `local_fields()` 的鍵（帶 `~`），不是倉的位址。`eval.rs` 展開內部 local 時去掉一個前導 `~`，`~@a` 落到 local 表的 `"@a"`。兩處都維持原樣。

### 8.2 順手改動（逐項指名）

沒有。

### 8.3 工單哪裡是錯的

沒有。

### 8.4 工單指名要你回答的問題

**Q1** 算位址與驗位址。格子都是這份來源 `t: { ~/f: (x -> x + 1), r: /f 2 }`，除非另註。

| 入口 | 宣告 8 | 宣告 9 | 改了 |
|---|---|---|---|
| `put_value` `:616` | 包進隱藏當私有軸再寫。這份來源的根是 `f42079d4…`，`_.t.r` 是 `3` | 旗標關，軸 7／8、裸名。根 `21e6e90f…`，`_.t.r` 是 `3` | 加了 `< 9` 那層 |
| `put_root` `:642` | 同上。根位址就是上面兩條 | 同上 | 加了 |
| `put_commit` `:799` | 同樣包一層。提交本體不帶使用者的隱藏鍵；巢狀的值在 `put_value`。兩倉 `commit` 退出碼都是 0 | 同 | 加了 |
| `value_matches_stored_address` `:150` | 讀先試當前規則，再試軸 6。v0.99.0 `log` 0、讀 `3`；本引擎 gc 之後仍讀 `3` | 當前規則就中。`log` 0、讀 `3` | 中間加了隱藏當私有軸 |
| 對等點抓取 `disc.rs:210`／`:604` 的 `get_value` | 另一工作區（自己沒有 `.oo`）抓宣告 8 的根 `hash:sha256:v2:_:/5+B3pCm9a8EAMDzssfW4NT+CQC/07HpxbrfzgUAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA:f42079d4…`，`.t.r` 是 `3` | 同一條先新後舊。這格量的是 8 倉的物件 | 跟著 `get_value` |
| `open_commit` `:882` | 兩倉的 `oo log` 退出碼 0，列出該提交。沒有隱藏面向退路 | 同 | 沒有加退路 |
| `StandardRootSet` | 標準表沒有使用者的 `~@`／`~/`。`oo status` 的標準根是 `c96c2937…` | 同，六個新倉都是這條 | 沒有 |
| `~%Engine./save` | `{ ~@a: 1 }` 摘要 `3e6db633…` | 摘要 `82b61eda…`。沒有宇宙是 `#no_universe` | 跟著 `put_value` |
| `~%Discovery./identify` | `{ ~@a: 1 }` 摘要 `afe6e09b…` | 同一條 `afe6e09b…`，沒有宇宙也是 | 沒有 |
| `rekey_identity_names` `:853` | 只在宣告 `≤5` 的 put 路徑。local 鍵的拼法不動，重寫的是 `%bodies` 的摘要鍵與值。宣告 8 上 `t: { ~@a: 3, @{ ~@a }: 1 }` 的根本引擎與 v0.99.0 都是 `2fae183cf876fefd8f312dcfb9b966291cd4010516e2cf407aa3ef7f8550a101` | 隱藏面向的 put 不呼叫它 | 沒有 |
| `content_hash` memo `:3927` | 旗標開著時不讀不寫 memo。同一來源在 8 倉與 9 倉的根不同（`f42079d4…` 對 `21e6e90f…`） | 旗標關，用當前世代的 memo | 跳過條件加了 `hidden_facets_as_local` |

**Q2** 名字解析。精確查找都在前綴 fallback（`/`、`@`、`~`、`~%`）之前。形狀格是 `{ ~a: 2, ~@a: 3, k: a }`，外部格是 `.~@T`。

| 地點 | 精確查找 | 形狀格 | 外部 |
|---|---|---|---|
| 作用域框 `lib.rs:4221` `get_field`，`:4224` `get_local_field` | 這次改的是 `:4224`。`local_fields()` 的鍵帶 `~`，直接 `.get(name)` 永遠錯過 | `.k` 是 `2` | `{ ~@T: @int }.~@T` 是 `#private_access_violation` |
| 暫存 `:4259`／`:4262` | 原本就是 `get_local_field`，在前綴迴圈之前。沒有改 | 演化後、提交前，`status` 列 `k: a`、`~@a: 3`、`~a: 2`。`oo eval '_.k'` 與 `oo eval 'a'` 都是 `_`。只有 `~a: 2` 與 `k: a` 的暫存也是 `_`。提交後見下一列 | `_.~@a` 是 `#private_access_violation` |
| 根 `:4280` `get_field`，`:4285` `get_local_field` | 原本就先精確。沒有改 | 根 `faa4854d…`，`_.k` 是 `2` | `_.~@a` 是 `#private_access_violation` |
| 標準根 `:4307`／`:4312` | 原本就先 `get_local_field`。沒有改。標準表沒有使用者的 `~@` | `{ k: @int }.k` 讀到 `{{ %kind: #type %name: "int" }}`。形狀那格的 `k` 在紀錄自己裡就是 `2`，沒有落到標準根 | 外部那格在紀錄上就被私有擋下 |
| 路徑 `resolve_path_internal` `:4397` 起 | `get_field` 然後 `get_local_field`，然後才是前綴。暫存、根、標準根在這條路徑上同樣先精確（`:4458`、`:4482`、`:4514`）。沒有改 | `t: { ~a: 2, ~@a: 3, k: a }` 根 `5e847c30063069f0df76a36492ec6b9806d08351061f9fbfd717a603a80317c2`，`_.t.k` 是 `2`，`_.t` 印 `{ k: 2 }` | `_.t.~@a` 是 `#private_access_violation` |
| 分派表的鍵 | 分支體裡的名字走作用域框（上面那一列）。`rekey_identity_names` 不改 local 鍵的拼法 | `{ ~a: 2, ~@a: 3, @{ @int }: a } 4` 是 `2`。模式裡的 `~@` 見 Q1 的 `2fae183c…` | `{ ~@T: @int, @{ @int }: 1 }.~@T` 是 `#private_access_violation` |

跨框的去前綴 fallback 維持原順序。暫存上 `oo eval` 在提交前讀到 `_`，與只有 `~a` 時相同：這張卡修的是同一框裡精確查找失效、因而讀到另一個面向；就地那格已經是 `2`。

**Q3** 舊引擎是 `/home/gali/nlang-baselines/v0.99.0-verify/target/release/oo`。

*   **(a)** 本引擎寫宣告 8，v0.99.0 開：
    *   `t: { ~/f: (x -> x + 1), r: /f 2 }`：根 `f42079d4…`，本引擎讀 `3`，v0.99.0 `log` 0、讀 `3`。
    *   根上 `~@T: @int` 與 `r: 5 & @T`：根 `7d7e128df32786cb14272a0bcf54196e7764926b8bc92c0c7cc20961b776fb0a`，本引擎讀 `5`，v0.99.0 `log` 0、讀 `5`。
    *   `t: { ~@a: 3, @{ ~@a }: 1 }`：根 `2fae183c…`，兩邊都讀 `{ @{ _ }: 1 }`，v0.99.0 `log` 0。
*   **(b)** v0.99.0 開本引擎的宣告 9：`log` 退出碼 1，訊息見 8.1 M4。
*   **(c)** v0.99.0 寫宣告 8，本引擎開：
    *   `~/f` 欄位：v0.99.0 的根是配方 `879c3df498bd6a0f4b2b7d1dd881b358f1a357da822b469b1e85896206257c4b`。本引擎 `log` 0，遷移前 `_.t.r` 是 `3`，`gc` 是 `3 objects, 3 reachable, 0 collectable`，`migrate` 退出碼 0、宣告變 encoding=9、文句點名 `oo v0.98.0 through v0.99.0`，遷移後讀 `3`，`log` 0。
    *   根上的 `~@T`：v0.99.0 演化退出碼 0，提交退出碼 1，沒有 HEAD。拒絕的是 `store n/ parse`，成員印成 `~"@T"`。本引擎 `log` 0（空倉）。沒有一筆 v0.99.0 提交過的 `~@T` 根可以遷移。
    *   模式表 `t: { ~@a: 3, @{ ~@a }: 1 }`：v0.99.0 的根同樣是 `2fae183c…`。本引擎 `log` 0，遷移前後都讀 `{ @{ _ }: 1 }`，`gc` 是 `3 objects, 3 reachable, 0 collectable`，宣告變成 encoding=9。

### 8.5 探針

`crates/oo/tests/a_hidden_facet_taken_for_a_name_probe_test.rs` 與 `crates/interpreter/tests/a_hidden_facet_taken_for_a_name_bytes_probe_test.rs` 未改，`rustfmt` 未掃它們，也未掃預先修訂的 15 處。單獨一輪 **2／2**（0.00s）與 **5／5**（4.11s），`PROBE_EXIT` 0，該輪 `cargo` `Finished` `release` 在 22.06s。全樹第一輪裡這兩支是 **2／2**（0.00s）與 **5／5**（4.09s）。

### 8.6 數字

*   三輪皆 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`，`EXIT` 0。2026-10-22 12:25:25 牆鐘 1277.87s；12:46:43 牆鐘 875.81s；13:01:19 牆鐘 890.60s。
*   歸一化 3648 行，三輪 sha256 同為 `0febaac9f08e8b9fdad5c49a83d626df9464a4d217bd9f604d50dbf25e995334`。`test result:` 291 行，加總 **2778** 通過、**0** 失敗。以 `test ` 起、以 ` ok` 收的行 2777 行：`g1_a_sequential_conflict_is_still_refused` 把 `Error: Evolution Conflict…` 印在測試名那一行，`ok` 在下一行。`Running ` 288。原始位元組 294509／294355／294355。原始 sha256 前綴 `2fcfd169da6557d9`、`9404ab9ece8129f4`、`d8df7f74120e9cb7`。`^error` 0，`BrokenPipe` 0，`panicked` 0。
*   第一輪紅線（皆 ok）：本探針 CLI 5／5 4.09s、位元組 2／2 0.00s；Q-098 5／5 9.01s；Q-097 7／7 10.00s；Q-095 3／3 23.50s；`a_refusal_written_as_an_answer` 17／17 4.16s；`a_spread_at_the_root` 11／11 9.35s；`a_store_written_in_another_language` 13／13 0.96s；`a_type_you_could_not_carry` 5／5 2.20s；`an_axis_the_address_forgot` 17／17 6.45s（epochs 4／4 0.31s，union 2／2 0.00s，bytes 1／1 0.23s）；`atomic_write` 8／8 1.52s；`every_byte_or_none` 14／14 1.99s；`nothing_here_or_nothing_you_can_see` 7／7 21.51s；`spread_privacy` 11／11 0.35s；`a_private_never_committed` 10／10 4.05s、R-1 3／3 0.53s；`type_super_probe_test` 14／14 0.49s。同輪還有：`a_working_set_that_is_a_set` 6／6 1.64s；`a_savepoint_that_knows_where_it_stood` 14／14 1.44s；`pin_probe_test` 15／15 2.28s；`a_table_you_cannot_read` 12／12 3.32s、R-1 4／4 1.38s；`a_table_that_stops_being_one_when_printed` 4／4 2.33s；`a_table_that_runs_every_branch_when_looked_at` 7／7 2.91s；`a_table_read_before_its_spread_landed` 11／11 2.13s；`a_table_that_forgot_its_patterns` 17／17 2.40s、R-1 6／6 0.61s、R-2 10／10 1.67s；`a_table_that_lost_its_patterns` 10／10 8.70s；`caused_top` 11／11 1.11s；`static_cycle` 17／17 0.64s；`math_union` 17／17 0.94s；`what_an_observation_leaves` 15／15 4.25s；`blur_spread` 13／13 2.85s；`forward_spread` 12／12 0.81s；`spread_collision` 25／25 0.89s；`spread_splice` 9／9 0.16s；`import_is_spread` 7／7 0.45s；`a_name_spread_in` 15／15 6.08s；`where_there_is_no_universe` 13／13 4.19s。
*   身分與新倉宣告見 8.1。
*   conformance **181／181**，空目錄 `/home/gali/tmp/q099/conf.iumb`，`CONF_EXIT` 0。跑完該目錄仍是空的。父倉 `/home/gali/nlang/.oo/format` 在閘門前、Windows 後、conformance 後都是位元組 `32`。父倉 HEAD 仍是 `e275ccdb1049c1b72463041fcc72477d0c8d30c2`。
*   Windows：`cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu`，`Finished release in 4.55s`，`WIN_EXIT` 0，`^error` 0。該目標 lib 15 則警告（`cargo fix` 會動 13 則）。Linux 同一 lib 14 則（`cargo fix` 12 則），見第一輪與 `/home/gali/tmp/q099/probe.log`。都沒有修。
*   `target/release/oo` 11150656 位元組，mtime 2026-10-22 12:18:57 +0800。三輪與 Windows 都沒有重新連結它。編譯時工作區是髒的，此處不報 `--version`。

### 8.7 你認為需要改規格之處

沒有。

## 9. 驗收（驗收方填）

### 9.1 第一輪驗收：**受理**

交付 `914c64d` 的六個引擎檔與參考實作 `q099-ref e49b150` 逐位元組相同（同形——驗收方因此在探針外另量）；兩支探針、15 處預先修訂與 §8 以上一字未動。8.3「沒有」對。
*   **探針外量了**（`acc/q99/outside.sh`，v0.99.0 對交付）：**公開面向與私有資料同名的讀數也變了**——`{ @a: @int, ~a: 2, k: a }.k` 由 `@int` 變 `2`、`{ /a: (x -> 1), ~a: 2, k: a }.k` 由箭頭變 `2`；這是 M1（`a` 讀資料面向）的射程，收尾時 CHANGELOG 列為破壞性。交會（`{ ~@a: @int } & { ~@a: @str }` ⊥）、展開（隱藏面向照舊不帶、公開的照帶）、模式、`=` 兩版同；`status` 印 `~@T: @int`。交付記的「提交前 `_.k` 讀 `_`」連公開欄位 `a: 2; k: a` 也是——`oo eval` 讀已提交的宇宙，不是缺陷。
*   **Q1** 收下：十一個入口逐一量過；`identify` 與 `save` 對 `{ ~@a: 1 }` 摘要不同是既有的（字面量的欄位是 thunk，Inbox「`disc.identify` 報出的位址，不是倉會用的位址」）。**Q2** 收下：只有作用域框那一處。**Q3** 收下：三組跨引擎；v0.99.0 寫壞的 `~@T` 工作集沒有可遷移的提交，§6 明文不追回。
*   **數字**（驗收方）：參考 `q099-ref` 全樹 2778／0（與交付同碼；交付三輪同）；兩支探針 5／5、2／2；conformance 182／182（含收尾新增的 L2-143，空目錄）；盤點腳本與 v0.99.0 逐行同（`inv94` 只差 `~@int`）；身分紅線只差新倉 `encoding=9`。
