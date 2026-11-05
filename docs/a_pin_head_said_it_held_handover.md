# 一個說已經在 HEAD 的覆寫（Q-123）

探針：`crates/oo/tests/a_pin_head_said_it_held_probe_test.rs`（不得修改）。沒有預先修訂。
佇列：`nlang-spec/meta/WORK_QUEUE.md` §3「2026-11-05 — 開 Q-123」。不須裁（規格已定）。

---

## 0. 判準

*   **D84 (i)**：「內容已在 `HEAD` 根裡（**不移動位置**，D80 ②）的注入**不是提議**。」
*   **`SPEC_08` §6.2**：`#pin`「直接覆蓋節點值，跳過收斂邏輯（唯一動及格本體的特權操作）」。**`SPEC_10` §2.2.3**：pin 的單位是坐標；「根與工作集之間依特權座標做覆寫」。
*   **D33**：⊥ 由操作者處置。一般寫入不能蓋過 ⊥（單調；⊥ 交什麼都是 ⊥），所以保留歷史的修法只有 pin。
*   一句話：**一筆 pin 的位置是提交會做出的那個覆寫，不是交會**；pin 讓一個被釘的坐標換了值，它就是提議，提交後那個坐標讀 pin 的值；只有提交會存下的根就是 HEAD 的根時，才是「已在 HEAD」。

## 1. 缺陷

〔量，`v0.123.0` 標籤建置，`v0.122.0` 同；`acc/q123/recon.py`、`widen.sh`〕

*   **pin 修不了已提交的 ⊥**：`r: 5 & 7` 提交（寫的人自己的 ⊥，D56 收下並回報）之後 `evolve --pin --grant pin` 寫 `r: 3`：演化 rc=0，`status` 答 `Universe is static (no staged changes).`，`commit --grant pin` 答 `already in HEAD`，`_.r` 仍是 ⊥，成員檔留著。記錄欄位（`h: { a: 1, r: 5 & 7 }` pin `h: { a: 1, r: 3 }`）、清單元素（`l: [5 & 7, 1]` pin `l: [3, 1]`）、存成 ⊥ 的配方（`n: 5; r: n & 7` pin `r: 3`）同。
*   **pin 寬化、少欄位同**：`r: 3` pin `r: @int`、pin `r: 1..10`；`r: { a: 1, b: 2 }` pin `r: { a: 1 }`——都答 `already in HEAD`，讀原值。
*   **起因**：`proposals_at`（以及提交答「已在 HEAD」的 `head_if_it_holds`）一律以「根與成員交會的雜湊等於根的」判已持有。pin 的值與 ⊥、與較窄的值、與欄位較多的記錄交會，都是根。
*   **對照**：pin `r: 4` 蓋 `r: 3` 落地；pin 與 HEAD 存的相同值答 `already in HEAD`；一般寫入 `r: 3` 蓋 ⊥ 被拒 `#conflict at r`。
*   **其他路**：一般寫入拒（單調，對）；`rollback` 可以，但整段歷史跟著退；移動配方讀的名字（pin `n: 7`）提交成功而 `r` 仍是 ⊥——提交是固化邊界（O35），根裡存的是 ⊥ 本身，不是配方，所以只能覆寫那個坐標。

## 2. 依據；位址

*   §0 各款。不須裁。
*   **位址：小的寫入不動**（g2 四顆）；一般寫入的「已在 HEAD」判法不動。改變的只有：pin 由「被當成已持有、丟掉」改為落地——落地的提交是新提交（pin 本來就會產生的那種）。
*   破壞性（pin 的結果改變），收尾記 `CHANGELOG`。

## 3. 射程＝不變式（不是機制）

*   **M1 pin 讓被釘的坐標換值，就落地**（r1，九格）：pin rc=0；`status` 列出它；`commit --grant pin` 成功；讀 pin 的值；成員檔收走。蓋 ⊥（頂層、記錄欄位、清單元素、存成 ⊥ 的配方、在別的提交之後提交的 ⊥）、寬化（型別、區間）、少欄位（頂層、巢狀）。
*   **M2 與其他成員並存時同**（r2）：一般寫入在前或在後；同一坐標兩次 pin（後者吸收前者，D58）；兩個坐標兩次 pin；同一筆 pin 裡一個坐標已持有、一個換值；落地之後再寬化一次，再 pin 相同的值是已持有。
*   **另一半**（g1）：
    *   pin 與 HEAD 存的相同（值、記錄、型別、⊥ 本身、讀名字的記錄、箭頭、表、讀根的配方）：`status` 靜止、提交答 `already in HEAD`、根不動——**比的是提交會存下的根，不是成員的寫法**；
    *   pin 收窄照舊落地，再 pin 一次是已持有；
    *   一般寫入蓋 ⊥ 照拒；一般寫入 HEAD 已持有（`q: @int` 蓋 `q: 3`）照答 `already in HEAD`、根不動；
    *   pin 的提交仍須出示能力（`#privileged_required`）。
*   g2：小的根不動。
*   **上一張卡學到的，逐條過**：不變式要在所有路徑都經過的那一處判——「已在 HEAD」有兩處：`proposals_at`（載入工作集）與 `head_if_it_holds`（提交答「已在 HEAD」，含等鎖時被別的提交拿走的 C1 那條）；別照抄舊判法的例外（Q-122 R-1）——pin 不是「整筆換判法」，成員裡沒被釘的坐標照交會；成員的寫法與存形不同（字面 vs 固化後），比雜湊前要照提交的方式觀測。

## 4. 紅線（今天綠，必須保持綠）

*   g1、g2；所有既有探針與 conformance——**特別指名**：`already_in_head_probe_test`、`a_landing_that_head_decides_probe_test`（D84、D86）、`pin_probe_test`、`an_overwrite_that_was_a_coin_flip_probe_test`（D58 吸收）、`a_recipe_that_met_a_write_probe_test`、`a_spread_at_the_root_probe_test`、`atomic_write_probe_test`、`privileged_effect_audit_probe_test`、`a_refusal_that_built_a_universe_probe_test`、`an_axis_the_address_forgot_probe_test`、`a_name_that_arrived_late_probe_test`（Q-122 的 pin 格）。
*   全樹（`--no-fail-fast`）、conformance（`nlang-spec` 的 `local`，**新的空目錄裡跑**，前後印 `ls -A | wc -l`）、交叉編譯 `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   身分紅線：`x: 0` 根 `b74d7758…`；`v: 1 + 1` 根 `df91a8ad…`；標準根 `c96c2937…`；新倉 `layout=9`／`encoding=11`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1 兩處**：你在哪裡判 pin 成員「已在 HEAD」；`proposals_at` 與 `head_if_it_holds`（已持有的答句、等鎖後的 C1）各怎麼判；還有沒有別處用交會判一個 pin 成員的位置（`status`、D92 的 `session_would_be_listed`、`evolve` 端）。
*   **Q2 比什麼**：同一個值寫成字面與從根讀回的存形不同，你怎麼讓「pin 相同的值」答已持有；讀名字的記錄、箭頭、表、讀根的配方各一格（根、`status`、提交答句）。
*   **Q3 成員裡沒被釘的坐標**：一筆 pin 成員可帶未被釘的坐標嗎（`pin_coords` 與 `combo` 的關係）；若可以，那些坐標照交會判——量一格。
*   **Q4 成本**：每個 pin 成員多做幾次觀測；`r: { 8000 欄 }` 之後 pin `r: { 7999 欄 }` 的演化與提交時間，對照 v0.123.0（v0.123.0 會答 `already in HEAD`）。〔驗收方量，`acc/q123/cost.sh`〕2000／8000 欄：v0.123.0 演化 0.35／1.33 秒、提交 0.73／2.65 秒（答 `already in HEAD`，`k7999` 仍讀 `7999`）；參考 0.33／1.26 秒、0.62／2.47 秒（落地，`k7999` 讀 `_`）。

## 6. 明文不在射程內

*   **pin 路徑鍵把兄弟欄位抹掉**（`h.k: 5` 的 pin 把 `h` 換成 `{ k: 5 }`；Inbox）——那是 pin 的坐標粒度，不是「已在 HEAD」的判法。
*   **並行注入做出的晚到 ⊥**（Inbox；成員沒記看過誰）。
*   **Q-122 走訪的代價**（Inbox）。
*   `rollback` 的語義、一般寫入蓋 ⊥ 被拒（單調，對）。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1、r2 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。** `rustfmt` 不得掃探針檔。每個 `oo` 行程 3 GB 位址空間上限、90 秒即殺；子行程輸出邊跑邊讀。
*   〔驗收方校準〕**兩極皆做**：
    基線 v0.123.0（標籤 `c480382` 加本探針）：**2 綠 2 紅**——r1 紅（九格裡八格：蓋 ⊥ 五形、寬化兩形、少欄位；**巢狀記錄少欄位那一格在基線上是綠的**——巢狀字面與存形雜湊不同，交會判法湊巧把它當提議，留作守衛）、r2 紅（六格全紅）；g1、g2 綠。
    參考實作（`q123-ref` `df3a376`）：探針 **4／4**；全樹 **313 個目標（`Running` 行）、2947 通過、0 失敗**（v0.123.0 的 312／2943 加本探針一個目標四支）；conformance **213／213**（空目錄前後 0）；交叉編譯 0 error。
    參考的做法（只是一種）：`Universe::pin_moves`——以 `pin_commit_merge`（提交的覆寫）做出提交會交會的根，照提交的方式觀測（`project_for_commit`、`canonical_bodies`），與 HEAD 的根同樣觀測後比雜湊；`proposals_at` 與 `head_if_it_holds` 對帶 `pin_coords` 的成員都用它，其餘照交會。`held_committable` 與 C1 讀到的成員帶著 `pin_coords`。
    **變異**（`acc/q123/mut.py`）：六個，四個擋下——不觀測直接比雜湊 ⟹ g1（同一個記錄被當提議、做出同根的空提交）；pin 成員一律是提議 ⟹ g1、r2；`proposals_at` 照交會（v0.123.0）⟹ r1、r2；只覆寫根上已有且非 ⊥ 的坐標 ⟹ r1、r2。**兩個存活**：(a)「`head_if_it_holds` 照交會」——已持有清單是 `proposals_at` 判過的；C1 那條是成員被別的提交拿走，而拿走它的提交存下了 pin 的值，那時交會與覆寫答案相同；(b)「觀測後的覆寫與**未觀測**的根比」——倉裡的根就是提交觀測過的形，再觀測一次不動（讀名字的記錄、箭頭、表、讀根的配方四格都量過）。兩者一起套上，全樹 313／2947／0、conformance 213／213、交叉編譯 0，與參考同。
    〔驗收方的錯〕(1) 第一版參考把 pin 的覆寫結果直接與根比雜湊（沒有觀測）——pin 同一個記錄被當成提議，做出同根的空提交；g1 抓到。(2) 第一版探針把「之後 pin `r: @int`」與「沒有能力的提交」放在 g1，而兩者都依賴本修補——基線紅；分別移進 r2、改用收窄的 pin。(3) 「兩次提交做出的 ⊥」一格的設定（`r: 7` 再 `r: 7 & 8`）第二筆本來就被拒，根裡沒有 ⊥——改為先提交別的欄位。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

`universe.rs`、`main.rs` 與 `q123-ref` `df3a376` 逐位元組相同。探針是 `0625f13`，未改。

*   **M1。** pin 讓被釘的坐標換值就落地：演化 rc=0，`status` 列出它，`commit --grant pin` 成功，讀 pin 的值，成員收走。
    *   蓋頂層 ⊥：`r: 5 & 7` 之後 pin `r: 3`。根從 `7674df6e0cf1aadcedf26afd44b62186155cc717799a1864be4e90d634611f97` 到 `02992e9f54b9b33f3fd414d81156ba336d797e268cc15cab7b0881dcf127521a`，`_.r` 讀 `3`。
    *   記錄欄位：`h: { a: 1, r: 5 & 7 }` 之後 pin `h: { a: 1, r: 3 }`。根從 `7d5ed99ad4c49653482c76b52b48ea433121069c0320d7389c044bd271f8d2a1` 到 `5fced5bbfe410640dfcb6a1fd4602ec311e703f75bc8c20e65849403be58d3b6`，`_.h` 讀 `{ a: 1 r: 3 }`。
    *   清單元素：`l: [5 & 7, 1]` 之後 pin `l: [3, 1]`。根從 `9895dca97d966ce48136530d7f2cdb5581157125075161e2fa6a8b185345958b` 到 `caf4f24c463c12e48f92a356458309196a53cbda54b16b6bb39db30fc28f93aa`，`_.l` 讀 `[3, 1]`。
    *   存成 ⊥ 的配方：`n: 5`、`r: n & 7`（根 `fba672c829ec60c00b70cb6090e8b95ba38b16c96cd99eb6773bad45a146822f`）之後 pin `r: 3`。根 `4bd16f89349c2819f98d373750a2d745568c4f61997813581c635baff247874d`，`_.r` 讀 `3`。
    *   別的欄位先提交：`a: 1` 再 `r: 5 & 7`（根 `4448ffaefbba3f33ab0f0c37610fcd561d1bc7ec2654e5dba2b16c28fbbb53ab`）之後 pin `r: 3`。根 `62090c9830f0a64a63b3c97cdd8d65f097bf184109799bc7748e88e920a53a37`，`_.r` 讀 `3`。
    *   寬化成型別：`r: 3` 之後 pin `r: @int`。根從 `02992e9f…` 到 `f15584157b0530941f782a00ef28f6281d4c5c994c3be716d1dfa03675307821`，`_.r` 讀 `{{ %kind: #type %name: "int" }}`。
    *   寬化成區間：pin `r: 1..10`。根 `1b2fbd0a51af36c47baf85682a08354adf20d61ff68292a06ad2ba1baa66b1b7`，`_.r` 讀 `1..10`。
    *   少欄位：`r: { a: 1, b: 2 }`（根 `2604502f258bb162157b59571bbbd56fb6534003e9ba25c7db12f01230b64727`）之後 pin `r: { a: 1 }`。根 `0123237164ecb12f7558d886a0d8baceb29777fe30d8e254d282fc217bda08a2`，`_.r` 讀 `{ a: 1 }`。
    *   巢狀少欄位：`h: { k: { a: 1, b: 2 }, z: 0 }`（根 `25706a771a901f4bf365db0e86891c1832bb1088b8fba0c7ce38c84623af3425`）之後 pin `h: { k: { a: 1 }, z: 0 }`。根 `16338586ecc66422c32c4e00f6350f5e14b5952a6d238ef437417c45af639de2`，`_.h.k` 讀 `{ a: 1 }`。
*   **M2。** 一般寫入在前或在後，與 pin `r: 3` 一起提交，根都是 `9b456cc4bc2e6a209190eb72dcfa1df74cf8206c700b27f81e845ef8a06b9def`，`_.r` 讀 `3`，`_.s` 讀 `2`。同一坐標兩次 pin（`@int` 再 `1..10`），後者落地，根 `1b2fbd0a…`（與直接 pin `1..10` 同一顆），`_.r` 讀 `1..10`。兩個坐標兩次 pin，根 `1b68970f49d587a3b72dc01ace5d658943c071a50e95a5d818996e121bd5a4cf`，`_.r` 讀 `3`，`_.s` 讀 `{ a: 1 }`。同一筆 pin 裡 `r: 3` 已持有、`s: 4` 換值，根 `28cbf2a51549999a83f6d2a9fb5bbb982840ac922ad8967fec36ce1ca87cb460`，讀 `3` 與 `4`。落地 `r: 4`（根 `560ac5cd003ad1d37f9e8b398d06a2a88ba7ace348efa70fe98672dcef584aaa`）再 pin `r: @int`，根 `f1558415…`，再 pin 一次 `status` 靜止、提交答 `already in HEAD`、根不動。
*   **另一半。** pin 與 HEAD 存的相同（值、記錄、型別、⊥、讀名字的記錄、箭頭、表、讀根的配方）：`status` 靜止，提交答 `already in HEAD`，根不動。讀名字的記錄根 `2284caf43b29be83c7b90aee891418b43fcd91b8495819d95f70ff43f54a0e5e`；箭頭 `258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f`；表 `3218de24658ad19158c38e50b714ef7b7eba8e85fc8a296a791f4f9823a2c66f`；`n: 5`、`r: n + 1` 根 `f6dc16ae774bef4a5fb10d214847b2c84718d416654e9e11f3570fd92ef10db9`。收窄 pin `r: 4` 落地，根 `560ac5cd…`，再 pin 一次已持有。一般寫入 `r: 3` 蓋 ⊥ 拒 `#conflict at r`。一般寫入 `q: @int` 蓋 `q: 3` 答 `already in HEAD`，根不動，`_.q` 讀 `3`。沒有 `--grant pin` 的提交答 `#privileged_required`，根不動。
*   **g2。** `x: 0` `b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f`；`v: 1 + 1` `df91a8ad239ea8727c3a03a71eb4b0a787312b92306db4d8b124a73faaf40199`；`f: (x -> x + 1)` `258a1a08…`；`h: { a: 1, b: a + 1 }` `2284caf4…`。

### 8.2 順手改動（逐項指名）

沒有。本回合只動 `universe.rs` 與 `main.rs`，而且是參考實作的位元組。探針、版本、`CHANGELOG`、規格、向量都沒動。`rustfmt` 未跑。

### 8.3 工單哪裡是錯的

沒有。

### 8.4 工單指名要你回答的問題

**Q1。** pin 成員「已在 HEAD」只在兩處改判，都走 `pin_moves`：`proposals_at`（載入工作集；`status`、提交、下一筆演化都從這裡看），以及 `head_if_it_holds`（提交答「已在 HEAD」）。後者有兩條路。工作集折完沒有可提交內容時，C2 把 `held_committable` 交進去——那份清單是 `proposals_at` 判過、而且帶著 `(combo, pin_coords)` 的。等鎖之後成員被別的提交拿走（C1），`seen_committable` 是鎖前讀到的成員，同樣帶 `pin_coords`，再交同一支。`pin_coords` 空的成員仍用交會雜湊。

`pin_moves` 用 `pin_commit_merge` 做出提交會存的那顆，再與 HEAD 的根一起走提交的觀測（`project_for_commit`、`canonical_bodies`）比雜湊。相等就是已持有。

`evolve` 寫成員的時候不判這句。判在下一次載入。蓋 ⊥ 的 pin，演化 rc=0，成員檔在，接著 `status` 列出 `Staged changes`。

`session_would_be_listed`（D92，還沒有倉的那一條）仍用交會，對的是空根。空目錄裡 pin `r: 3`：演化 rc=0，倉出現，`status` 列出它。空根上，交會與覆寫落在同一位置，這條不會把「已提交的 ⊥」誤判成已持有——那時候還沒有 HEAD。

`injection_positions_equal` 仍用交會，記的是存點圓（D80）工作集前後的位置，不決定「已在 HEAD」那句，也不把成員丟掉。

**Q2。** 比的不是成員正文與倉裡那份字。兩邊都先照提交觀測再比雜湊，所以同一個值寫成字面、倉裡已是觀測過的形，pin 那份字面答已持有，根不動。

*   讀名字的記錄 `h: { a: 1, b: a + 1 }`：根 `2284caf43b29be83c7b90aee891418b43fcd91b8495819d95f70ff43f54a0e5e`。`inspect` 仍印 `b: a + 1`。pin 同一句：`status` 靜止，提交 `already in HEAD`，根不動。
*   箭頭 `f: (x -> x + 1)`：根 `258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f`。同上。
*   表 `t: { @{ @int }: 0, _: 1 }`：根 `3218de24658ad19158c38e50b714ef7b7eba8e85fc8a296a791f4f9823a2c66f`。同上。
*   讀根的配方 `n: 5`、`r: n + 1`：根 `f6dc16ae774bef4a5fb10d214847b2c84718d416654e9e11f3570fd92ef10db9`。`inspect` 仍印 `r: n + 1`。pin `r: n + 1`：`status` 靜止，提交 `already in HEAD`，根不動。

**Q3。** 可以。`combo` 是這筆的 delta，`pin_coords` 是釘下的坐標，分開記。`pin_commit_merge` 先把 `pin_coords` 從 combo 拿掉，剩下的與根交會，再把釘的坐標覆寫上去。

一道 `--pin` 寫下的每個頂層坐標都會進 `pin_coords`。pin `r: 3` 與 `s: 4`，成員是 `pin_coords: ["r","s"]`，正文 `{ r: 3 s: 4 }`。

同一筆裡不在 `pin_coords` 的坐標照交會。HEAD 是 `r: 3`、`q: 3`，成員正文改成 `{ r: 3 q: 4 }`、`pin_coords` 仍只有 `r`：不是把 `q` 覆寫成 `4`。提交印 `#conflict at q` 然後成功，倉裡 `q` 是 `_|_ #conflict`，`r` 仍是 `3`，根 `e79fecdc1d891cdb75f791deeb80745eefe9251a037045ab324edbc1cf1672c6`。對照：同一道 pin 把 `q: @int` 也釘上，`_.q` 讀型別 `int`，根 `2d2eaed731b2aef97c8bab1b6c40cc797e12a7d88176cc16903904fb767981cc`。

另一格是命令自己做出的：pin `r: 3` 與一般寫入 `q: @int` 分兩筆成員，HEAD 原是 `r` 的 ⊥ 與 `q: 3`。`status` 只列出 `{ r: 3 }`（`q` 交會之後已持有）。提交後 `_.r` 讀 `3`，`_.q` 仍是 `3`，根 `224fc22c9148aeed8cc1f25717d46a459d86d9d7f7c84563eec3d0ad2af561b3`。

**Q4。** 每個帶 `pin_coords` 的成員，一次 `pin_moves` 做兩次提交觀測：覆寫後的根一次，HEAD 的根一次。`proposals_at` 在載入時對每個這樣的成員叫一次。`head_if_it_holds` 在提交要答已持有時再叫一次；會落地的成員不進這條。沒有 `pin_coords` 的成員仍是一次交會雜湊。

`r: { N 欄 }` 之後 pin `r: { N-1 欄 }`：N=2000 演化 0.34 秒、提交 0.63 秒；N=8000 演化 1.31 秒、提交 2.40 秒。都落地，`k1999`／`k7999` 讀 `_`。驗收方量的 v0.123.0 是演化 0.35／1.33 秒、提交 0.73／2.65 秒，答 `already in HEAD`，`k7999` 仍讀 `7999`。本回合沒有重編那個標籤。

### 8.5 探針

`a_pin_head_said_it_held_probe_test` **4／4**。單獨一輪 `finished in 3.84s`，EXIT 0。閘門裡 `finished in 3.76s`（`a_pin_head_said_it_held_probe_test-82a91dbbab569c2d`）。探針檔未改。`rustfmt` 未掃。

### 8.6 數字

`universe.rs` 與 `main.rs` 相對 `0625f13`：+91／−21。加上本節，交付是這兩檔與這份交接。

探針 4／4。閘門：GATE 1 2026-11-05T12:32:21+08:00 WALL 1995 s（含編譯 7 m 11 s，結束 13:05:36）；GATE 2 13:05:36 1561 s（結束 13:31:37）；GATE 3 13:31:37 1549 s（結束 13:57:26）。正規化 3892 行，sha `c521d5674e1cb91fb28c79588373a7faeac157dfda4cdb0618ad3c56280e4121`，316 個 `running`、313 個 `Running`，加總 **2947** 通過、0 失敗，`test ` 行 `ok` 2946。三輪正規化相同。原始 sha 前綴 `42cb03ab5b45ba98`、`46feced62096414f`、`1ac036b0b5fa4f9e`。位元組 311442、311285、311285。`^error` 0，BrokenPipe 0，panicked 0。

conformance **213／213**，空目錄 `/home/gali/tmp/q123/conf.empty`，前後 `ls -A | wc -l` 都是 0，9 s。

Linux 編譯在閘門裡 Finished 7 m 11 s，interpreter lib 16 則警告，`cargo fix` 13，留著。Windows `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` 2026-11-05T13:57:26+08:00 Finished 4.86 s，腳本 wall 5 s，EXIT 0，`^error` 0，target lib 17 則、`cargo fix` 14，留著。

`oo` 11521320 位元組，mtime 2026-11-05 12:27:55.688391488 +0800。三輪閘門沒有再連結（`cmp` 相同，mtime 未動）。

身分：`x: 0` `b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f`；`v: 1 + 1` `df91a8ad239ea8727c3a03a71eb4b0a787312b92306db4d8b124a73faaf40199`；`f: (x -> x + 1)` `258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f`；`h: { a: 1, b: a + 1 }` `2284caf43b29be83c7b90aee891418b43fcd91b8495819d95f70ff43f54a0e5e`。標準根 `c96c29378c9395936ea4999db37e9b6a28291b57a47d4c016ca3cdf6833fed3b`（`c9/6c…`，16029 位元組）。新倉 `layout=9`，`encoding=11`。

### 8.7 你認為需要改規格之處

規格已寫 pin 的位置是覆寫、單位是坐標，已在 HEAD 是不移動那個位置。本回合把「已在 HEAD」從交會改成提交會存下的那顆。破壞性（pin 的結果改變），收尾時記 `CHANGELOG`。本回合未改規格、向量、`CHANGELOG`。

路徑鍵把兄弟欄位抹掉、並行注入做出的晚到 ⊥、Q-122 的走訪代價、`rollback`，都不在本回合。

## 9. 驗收（驗收方填）

### 9.1 驗收（2026-11-05，交付 `2bde335`）：**受理，零修補回合**

交付的 `universe.rs`、`main.rs` 與參考實作 `q123-ref` `df3a376` 逐位元組相同，探針未改。8.4 的讀數與驗收方在參考上量的相同。同形交付，所以驗收方在探針外另量。

**探針外**〔`acc/q123acc/out.sh`，v0.123.0 建置對參考建置，九格〕：

| HEAD | pin | v0.123.0 | 本版 |
| :-- | :-- | :-- | :-- |
| `h: { k: 5, r: 6 }` | `h.k: 5` | 已持有，`_.h` 讀 `{ k: 5 r: 6 }` | **落地，`_.h` 讀 `{ k: 5 }`** |
| `h: { k: 5, r: 6 }` | `h.k: 7` | 落地，`{ k: 7 }` | 同 |
| `h: { k: 5, r: 6 }` | `h: { k: 5 }` | 已持有 | 落地，`{ k: 5 }`（r1 少欄位） |
| `r: 5 & 7` | `r: 5 & 7`、`r: 5 & 8` | 已持有 | 已持有（⊥ 照提交觀測相同） |
| `r: @int` | `r: @int` | 已持有 | 已持有 |
| `r: 3`、`s: 4` | 同兩欄 | 已持有 | 已持有 |
| `r: [1, 2]` | `r: [1]` | 已持有，讀 `[1, 2]` | 落地，讀 `[1]` |
| `r: "a"` | `r: @string` | 落地 | 落地 |

第一列變寬，但成因不在本卡判準：路徑鍵 `h.k: 5` 釘的是頂層欄位 `h`、值是 `{ k: 5 }`。覆寫後的根確實不是 HEAD 的根，這與 r1「少欄位就落地」一致。壞的是 Inbox 既有列「pin 路徑鍵把兄弟欄位抹掉」：它原本是「換值」才抹，現在「寫同一個值」也抹。這一格記進 `CHANGELOG` 破壞性，也補在 Inbox 那一列。

**驗收方的漏**：g1「與 HEAD 存的相同」八形都用欄位的完整寫法，沒有一格只差寫法。Q-113 R-2 的教訓是「相等寫成同一段文字，每格要配一個只差寫法、根相同的對照」，開卡時沒有逐條過。

**收尾**（spec `local`）：
*   `SPEC_10` D84 款補句：pin 的「合起來」是覆寫。已持有指提交會存的根照提交觀測後與 HEAD 相同。
*   `CHANGELOG` [Unreleased]：兩條破壞性、代價、已知留白。
*   ENGINE_SYNC Q-123。
*   Inbox：「pin 修不了已提交的 ⊥」已結。
