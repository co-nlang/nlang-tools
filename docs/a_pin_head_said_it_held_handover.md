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

### 8.2 順手改動（逐項指名）

### 8.3 工單哪裡是錯的

### 8.4 工單指名要你回答的問題

### 8.5 探針

### 8.6 數字

### 8.7 你認為需要改規格之處

## 9. 驗收（驗收方填）
