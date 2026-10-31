# 一段讀回來變短的文字（Q-114）

探針：`crates/oo/tests/a_text_read_back_short_probe_test.rs`（不得修改）。
佇列：`nlang-spec/meta/WORK_QUEUE.md` §3「2026-10-31 — 開 Q-114」。

---

## 0. 判準（規格原文）

*   **`SPEC_09` §5.1.2 第 1 款**（Q-113 收尾新設，MUST）：寫出去再以同一模組的 `parse` 讀回來，是它自己。
*   **`SPEC_09` §5.1.2 第 5 款**：巢狀照寫；超過 `max_unification_depth` 是視界 `#max_depth_exceeded`。
*   **`SPEC_08` §3.2.2 第 1 款（G3）與第 3 款（成因誠實）**：值語境讀到的視界就是答案；成因要說出真正撞到的那個上限。`#none` 說的是「這不是 JSON」，⊥ `#conflict`「TOML parse error」說的是「這不是 TOML」——一段合法而深的文字兩者皆非。
*   **深度的旋鈕是引擎的**：`max_unification_depth`（`SPEC_09` §6 旋鈕表，預設 256）是政策；`HARD_RECURSION_LIMIT`（400，`lib.rs`，W4‴）是實作的原生堆疊天花板，不是政策。`serde_json` 的 128 層與 `toml_edit` 的 80 層**不是規格宣告過的任何旋鈕**——一個值能多深，不得由解析器 crate 的常數決定。
*   **不須裁**：上面三條已經定了答案。〔另一條路量過而不取〕讓 `stringify` 也在 80 層停下來，第 1 款字面上就成立，但那個 80 不對應任何宣告過的旋鈕，同一個值在 JSON 寫得出、在 TOML 寫不出。

## 1. 缺陷

〔量，`v0.114.0` 標籤建置；空目錄；`acc/q114/recon.py`、`deep.py`、`g.py`〕

*   **JSON 從 128 層起讀不回來**：127 層的 `[…]` 來回長度 254（對）；128 層起 `~%Json./parse` 答 `#none`——與不合法的 JSON（`"{"`）同一個答案。字面記錄 `{ a: { a: … } }` 128、129、200、250 層，`(~%Json./parse (~%Json./stringify (v))) = v` 皆 `#false`；`stringify` 寫得出（200 層 1201 字）。
*   **TOML 從 80 層起讀不回來**：表、陣列、inline table、點號表頭、點號鍵，80 層起 `~%Toml./parse` 答 ⊥ `#conflict`「TOML parse error … column 84」——79 層對。
*   **再深的文字**：JSON 300、5000、五萬層答 `#none`；TOML 一萬層答 ⊥ `#conflict`。不崩潰——是 crate 的上限先擋下來的，拿掉上限就要自己擋。
*   **經坐標同**：`x: { a: …200 層… }`、`r: (~%Json./parse (~%Json./stringify (x))) = x` 一檔、兩次演化、兩次提交都 `#false`；`r: ~%Json./parse ("""[…300 層…]""")` 存成 `#none`。
*   〔讀〕`json.rs` `serde_json::from_str`（預設遞迴上限 128，錯誤一律 `#none`）；`toml.rs` `::toml::from_str`（`toml_edit` `RecursionCheck::LIMIT = 80`，錯誤一律 ⊥ `#conflict`）。兩個 crate 都有拿掉上限的 feature（`serde_json/unbounded_depth`、`toml/unbounded`），不需新相依。

## 2. 依據；位址

*   **不須裁**（§0）。
*   **位址**：今天讀錯的格子有新根；**今天讀對的單次寫入根一顆都不動**（g2 六顆，v0.114.0 釘）。**破壞性**：深的 JSON 由 `#none`、深的 TOML 由 ⊥ 變成值；太深的文字由 `#none`／⊥ 變成視界。收尾記 `CHANGELOG`。

## 3. 射程＝不變式（不是機制）

*   **M1 寫出的讀得回**（r1）：`stringify` 寫得出的每一個值，`parse` 讀回來是它自己——記錄、清單、表、陣列，1 到 250 層；一個深表旁邊一個深陣列（混合文件的深度是較深的那一支）；**一個數的小數點不是一層**（240 層陣列裡兩百個 `1.5`）；在 `stringify` 寫得出的**最深那一層**（探針往深寫到它答視界為止，在來回裡它所在的那個呼叫深度上找——深度預算算的是值被寫在幾層呼叫底下）。
*   **M2 太深是視界**（r2）：比一個值能有的深度還深的文字是視界 `#max_depth_exceeded`——不是 `#none`、不是衝突、**不是崩潰，多深都一樣**（三百層到五萬層）。JSON 的清單與記錄；TOML 的陣列、inline table、點號表頭、點號鍵；**一個鍵的每一段都是一層，不論怎麼拼**（數字段 `1.1.1…`、引號段 `"b"."b"…`、inline table 裡的點號鍵、陣列裡的 inline table）。
*   **M3 三條路**（r3）：一檔、兩次演化、兩次提交同讀數、同根。
*   **另一半——今天讀對的照舊**（g1、g2）：不是 JSON 的仍是 `#none`、不是 TOML 的仍是 ⊥ `#conflict`；淺的文字照讀；**字串與註解裡的括號、點、大括號不是深度**（含跳脫引號 `\"`、TOML 的 `'''…'''`）；127 層；**寬不是深**（一千個並列的小陣列、三百張表是一層）；六顆根不動。
*   **上一張卡學到的，逐條過**：鍵序——`parse` 讀回的記錄與字面記錄同根（r1 以 `=` 比、g2 釘根）；列舉不重做——`parse` 不碰聯集，無涉。若你找到另一支把巢狀文字讀成值、而深度由 crate 決定的內建（`csv`／`url`／`time` 的 `parse` 是平的），一併在射程內，在 8.1 列出。

## 4. 紅線（今天綠，必須保持綠）

*   g1、g2；`json_p33_test`、`toml_p47_test`、`csv_p47_test`；Q-113 探針（`a_field_written_as_null_probe_test`，r5 的深度與自我參照）。
*   全樹、conformance（`nlang-spec` 的 `local`，**新的空目錄裡跑**，前後印 `ls -A | wc -l`）、交叉編譯 `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   身分紅線：`x: 0` 根 `b74d7758…`；`v: 1 + 1` 根 `df91a8ad…`；`f: (x -> x + 1)` 根 `258a1a08…`；標準根 `c96c2937…`；新倉 `layout=9`／`encoding=10`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1 深度在哪裡算**：讀成值時每一層在哪裡數、對哪個旋鈕比；為什麼 `parse` 讀得回 `stringify` 寫得出的每一層（兩邊數法差在哪）。
*   **Q2 原生堆疊**：解析器 crate 拿掉上限之後，什麼擋住五萬層的文字不崩潰；那道擋是精確的還是上界；若是上界，**有沒有 `stringify` 寫得出、卻被它擋掉的文件**——你怎麼確定沒有（TOML 的點號鍵、字串、註解、浮點數、表頭與底下的宣告）。
*   **Q3 成本**：五萬層的文字答多久；一份十萬個元素的淺 JSON 讀多久，與 v0.114.0 比。
*   **Q4 根**：哪些根是新的、g2 為什麼不動；`json_p33`／`toml_p47` 有沒有斷言舊讀數。

## 6. 明文不在射程內

*   **不是 JSON 的文字答 `#none`、不是 TOML 的答 ⊥ `#conflict`**——兩個模組兩種慣例，規格沒寫，須裁；照今天。
*   **又深又不合法的文字**（例如一千個沒有閉合的 `{`）答 `#none` 或視界都說得通（視界是「讀到上限就停了」，不知道後面合不合法）；探針不釘。
*   **n/ 剖析器讀 254 層以上的字面**以具名的 `#stack_overflow` 拒絕（D62）——那是原始碼的上限，不是 `parse` 的。
*   **深度預算算的是呼叫的層數**：`stringify` 被包在另一個呼叫裡時寫得出的層數少一層——全引擎的語義，不是本卡。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r3 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。** `rustfmt` 不得掃探針檔。每個 `oo` 行程 1 GB 位址空間上限、10 秒即殺。
*   〔驗收方校準〕**兩極皆做**：
    基線 v0.114.0（dev `df5f64e` 加本探針）：**2 綠 4 紅**——r1 錯 26／36（JSON 128 層起、TOML 80 層起，混合與浮點格皆錯；1 層與 79 層以內對）、r2 25／25（JSON 答 `#none`、TOML 答 ⊥ `#conflict`）、r3 12／16（經坐標的深記錄 `#false`、深文字存成 `#none`）、r4 9／9；g1、g2 綠；整支約 45 秒。
    參考實作（`q114-ref c7df8be`，相對 dev 引擎四個檔 `Cargo.toml`、`builtins/{mod,json,toml}.rs`，+173／−13）：探針 **6／6**；全樹 **307 個目標、2895 通過、0 失敗**（`acc/q114/tree_ref.log`，含本探針）；conformance **208／208**（空目錄前後 0）；交叉編譯 0 error。〔這輪全樹與驗收方另一份量測同時有六支孤兒 `oo` 各吃滿一核，計時數字不可用；通過數不受影響。〕
    參考的做法（只是一種）：開 `serde_json/unbounded_depth`、`toml/unbounded` 拿掉 crate 的上限；`builtins/mod.rs` 的 `read_deeper` 在轉成值時每進一個容器就 `ctx.depth += 1` 並 `check_resources(0)`——與 `written_of` 同一個旋鈕、同一條視界路徑（`handle_resource_exhausted`）；`text_too_deep` 在剖析之前拿文字深度的上界與 `HARD_RECURSION_LIMIT` 比，超過就直接答視界。JSON 的上界是精確的（括號計數，跳過字串與跳脫）；TOML 的上界逐筆宣告算：鍵裡的點（表頭、`=` 之前、inline table 的鍵；值裡的點是數字的一部分，不算）＋開了幾層容器＋1，鍵值宣告再加上它所屬的表頭；跳過字串（四種引號）與註解。
    **變異**（`acc/q114/mut.py`）：十七個，十六個被擋下——留著 crate 的上限 ⟹ r1 r2 r3（JSON）、r1 r2 r3（TOML）；剖析前不擋 ⟹ r4（JSON 一百萬層、TOML 十萬層崩潰；五萬層與一萬層**不會**崩潰，所以 r4 用檔案寫入）；轉值時不數深度 ⟹ r2（JSON 另 r3）；JSON 的上界數進字串裡的括號、不認跳脫 ⟹ g1；TOML 的上界數進字串、註解 ⟹ g1；表頭成本加到每一筆宣告上、數字的小數點算一層 ⟹ r1；鍵裡的點不算 ⟹ r4；太深答 `#none` ⟹ r2 r3；出了一層不把深度減回去 ⟹ g1 r1（JSON、TOML 各一）。**一個存活、為等價變異**：剖析前的門檻改用 `max_unification_depth` 而不是 `HARD_RECURSION_LIMIT`——TOML 的上界至多高估約兩層，而轉值時的深度檢查本來就落在 `max_unification_depth` 減去呼叫深度的地方，`stringify` 寫得出的每一份文件兩個門檻都放行；參考取 400，留著對高估的餘裕。
    〔驗收方做參考時自己踩到的錯〕(1) TOML 上界第一版把所有表頭的最大成本加到每一筆宣告上，一張 200 層的表旁邊一個 210 層的陣列被誤判太深——混合格看見；(2) 第二版把值裡的點也算一層，240 層陣列裡兩百個 `1.5` 被誤判——邊角格看見；只在鍵的位置數點，數字鍵 `1.1.1…` 照樣算；(3) 探針儀器錯：來回格在 253 層錯，視界來自被包深一層的 `stringify` 自己而不是 `parse`——改 250 層，並在來回所在的呼叫深度上找 `stringify` 寫得出的最深一層；一格「跳脫引號」其實寫成了不合法的文字；TOML 字串格的結尾 `"` 貼著 n/ 的 `"""`；(4) 第一輪變異的判讀把 `error: test failed` 當成編譯錯誤；(5) 拿掉剖析前的擋，五萬層 JSON 與一萬層 TOML 仍不崩潰——探針的 r2 碰不到原生堆疊，補 r4（檔案寫入，一百萬層與十萬層）。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

`Cargo.toml`、`builtins/mod.rs`、`json.rs`、`toml.rs` 與 `q114-ref c7df8be` 逐位元組相同（+173／−13）。探針未改。版本未動（`oo` 0.114.0，interpreter 與 parser 0.2.2）。

開 `serde_json/unbounded_depth`、`toml/unbounded`，crate 的 128 層與 80 層不再擋。`json.parse` 先用括號計數（跳過字串與跳脫）跟 `HARD_RECURSION_LIMIT`（400）比，過了才關掉 `serde_json` 的遞迴上限來讀。`toml.parse` 先用上界跟同一個 400 比，再 `toml::from_str`。轉成值時，JSON 的每個陣列與物件、TOML 的每個陣列與每張表走 `read_deeper`：`ctx.depth` 加一，`check_resources(0)`，超過 `max_unification_depth`（預設 256）走 `handle_resource_exhausted`，與 `written_of` 同一條視界。出去時把深度減回來。

*   **M1** r1 綠。記錄、清單、表、陣列，1 到 250 層，寫出再讀回來是它自己。一張深表旁邊一個深陣列，深度是較深的那一支。240 層陣列裡兩百個 `1.5`，小數點不是一層。`stringify` 寫得出的最深那一層，在來回的那個呼叫深度上，讀得回。
*   **M2** r2、r4 綠。三百層到五萬層是視界 `#max_depth_exceeded`，不是 `#none`、不是衝突。鍵的每一段都是一層：數字段、引號段、inline table 裡的點號鍵、陣列裡的 inline table。檔案裡的一百萬層 JSON、三十萬層記錄、十萬層 TOML，演化並提交，答視界，不崩潰。
*   **M3** r3 綠。一檔、兩次演化、兩次提交同讀數、同根。兩百層記錄讀回來是它自己；三百層文字存成視界。
*   **另一半** g1、g2 綠。不是 JSON 的仍是 `#none`，不是 TOML 的仍是 ⊥ `#conflict`。字串與註解裡的括號、點、大括號不是深度，含 `\"` 與 `'''…'''`。127 層照讀。一千個並列的小陣列、三百張表是一層。六顆根都在。
*   `csv`、`url`、`time` 的 `parse` 是平的。沒有另一支把巢狀文字的深度交給 crate。

### 8.2 順手改動（逐項指名）

沒有。探針、版本、`CHANGELOG`、規格、向量、`Cargo.lock` 都沒動。`rustfmt` 未跑。

### 8.3 工單哪裡是錯的

沒有。工單寫全樹 307 個目標、2895 通過、conformance 208。這棵樹印出 307 個 `test result:`、加總 2895 通過、0 失敗。conformance **208／208**。

### 8.4 工單指名要你回答的問題

**Q1。** 讀成值的深度在 `read_deeper`。JSON 每個陣列、每個物件進一層；TOML 每個陣列、每張表進一層。原子、字串、數字不加。比的是 `ctx.depth > max_unification_depth`（預設 256），`check_resources(0)`，與 `written_of` 同一個旋鈕。`eval` 每個算式再加一層，包在別的呼叫裡時預算少一層。

數法有兩處差。`written_of` 連葉子也進一層；`read_deeper` 只數容器，所以寫得出來的文字，容器層數比撞到旋鈕的那一步少。來回式裡 `stringify` 是 `parse` 的引數，比 `parse` 再深一層呼叫，它先停。讀的那邊用同一個旋鈕，剩下的預算夠把那些容器再數一次。

剖析前另有一道門，比的是 `HARD_RECURSION_LIMIT`（400），不是 256。那是文字深度的上界，用來讓剖析器不要走原生堆疊。JSON 的括號數等於容器層數。TOML 的上界可以比值深大約兩層。兩道都放行 `stringify` 寫得出的文件。

**Q2。** 擋住五萬層的是剖析前的 `text_too_deep`：上界大於 400 就答視界，剖析器沒有往下遞迴。JSON 的上界是精確的括號計數，字串與 `\"` 不算。TOML 的上界是上界：鍵的位置數點（表頭、`=` 之前、inline table 的鍵），加上開著的容器，再加一；鍵值宣告再加上它所屬的表頭。四種引號的字串與註解跳過。值裡的小數點不是鍵上的點，不算。高估大約兩層：兩百層的表寫成 `[a.a.…]\nv = 1\n`，長度 408；表頭成本是段數加一，底下那筆 `v = 1` 再加一。

沒有 `stringify` 寫得出、卻被這道門擋掉的文件。頂層呼叫，253 層清單寫得出，長度 507；254 層已是視界 `#max_depth_exceeded`。預設旋鈕 256，寫得出的容器到不了 400，TOML 高估兩層也到不了。點號鍵、字串、註解、浮點、表頭與底下的宣告在 r1、g1：浮點的點不算層，字串與註解裡的括號不算，表頭的每一段算。一百萬層與十萬層在 r4，答視界，不崩潰。

**Q3。** 五萬層 JSON，0.035 秒答視界 `#max_depth_exceeded`，CAID `b5de23854b5ca45cdb4db4848ab7fd09d0a8caf33a0ca9f5f38da4b3c468c13c`。v0.114.0 同一段 0.047 秒答 `#none`。十萬個 `1` 的淺陣列，兩邊都是燃料視界，2.772 秒，同一顆 CAID `2cfa31660844654d7be115dea0be851cd5cd87d0dbed6a3bb4e7bd332e78d568`。讀得完的寬陣列：一千個 0.046 秒對 0.043 秒，五千個 0.083 對 0.078，八千個 0.117 對 0.116，長度都對。`read_deeper` 扣的燃料是 0。

**Q4。** 今天讀錯的深文字，根是新的。128 層 JSON 清單是 `25bf5661198caacc54cd600d1225083e9dc6f0ef9523838cac61cd5d3c96793a`。80 層 TOML 陣列是 `ceab4ae6308e1c92ea2a296745f48e191d9479667aad2f7f480f392f65116888`。300 層 JSON 清單存成視界，根 `d42fecb7c015b5303f1cfa5703de39b6c5822bb3a5321d67db9c95a8e1b3918a`。g2 不動，因為那些格子本來就讀對：`[[1], {"a": 2}]` `bdaacbd1f3857d746c8eaccd92c8e45f9aee4486fb756ca0e4dc56602e86dd79`；`[t]\nb = [1, [2]]` `c0fd230f4dd166d1217679c07fcede28fbab9ea9b2e976b7c6d1e539e8e985b6`；不合法的 `{` `0c8242bb8570326595b0b1dda5f1c8ac3b4e58d5f7b08c75ed1d59bf73935978`；100 層清單 `352bbb92f1e4f0b147a047c3d2b58e563aeefdbf89a59c0594aecb7141529623`；`a.b.c = 1` `d5c0fd6859e4b5e8aa002f32631c4a53bd235c820bc0e3c2d621ddeac871825d`；寫回去 `6f8569af46be09a5adae97fac4dc993c492218da718f6c772c1fca1dffe1268f`。`json_p33` 斷言淺的物件、陣列、`null` 與 `{bad json}` 答 `#none`，沒有斷言 128 層。`toml_p47` 斷言淺的表與 `invalid = ===` 答 ⊥，沒有斷言 80 層。

### 8.5 探針

`a_text_read_back_short_probe_test` **6／6**，0 失敗。單獨一輪 40.83 s（編譯 Finished 1 m 28 s）。閘門內 41.49 s。檔沒有改。六支：g1、g2、r1、r2、r3、r4。

### 8.6 數字

閘門：GATE 1 2026-10-31T13:32:57+08:00 WALL 1787 s；GATE 2 14:02:44 1422 s；GATE 3 14:26:26 1374 s。正規化 3813 行，sha `32e50f7f8dcead0bed9cd5ab83a96dff8c0f7a123209537221f32380e321d78c`，307 個結果，加總 **2895** 通過、0 失敗，`test ` 行 `ok` 2894，`Running` 304，`running` 307。三輪正規化相同。原始 sha 前綴 `194814d29b795b0f`、`5a6c1c4a47300133`、`02ecf7392a28a4ff`。位元組 306020／305865／305865。`^error` 0，BrokenPipe 0，panicked 0。

Linux 這輪編譯在閘門裡 Finished 7 m 00 s，interpreter lib 15 則警告，`cargo fix` 13，留著。Windows `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` 2026-10-31T14:49:20+08:00 Finished 6.84 s，EXIT 0，`^error` 0，target lib 16 則、`cargo fix` 14，留著。

conformance **208／208**，空目錄 `/home/gali/tmp/q114/conf.empty`，前後 `ls -A | wc -l` 都是 0，23 s。`oo` 11407248 位元組，mtime 2026-10-31 13:32:11.286313011 +0800。閘門沒有重編它。

身分：`x: 0` `b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f`；`v: 1 + 1` `df91a8ad239ea8727c3a03a71eb4b0a787312b92306db4d8b124a73faaf40199`；`f: (x -> x + 1)` `258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f`。標準根 `c96c29378c9395936ea4999db37e9b6a28291b57a47d4c016ca3cdf6833fed3b` 在 `c9/6c29378c…`（16029 位元組）。新倉 `layout=9`，`encoding=10`。

### 8.7 你認為需要改規格之處

`SPEC_09` §5.1.2 已寫：寫出去再讀回來是它自己；超過 `max_unification_depth` 是視界 `#max_depth_exceeded`。crate 的 128 與 80 不是旋鈕。剖析前的門是實作天花板 `HARD_RECURSION_LIMIT`（400），上界用來不走原生堆疊；值的深度仍是 `max_unification_depth`。規格文字、新向量與 `CHANGELOG` 未改。

## 9. 驗收（驗收方填）
