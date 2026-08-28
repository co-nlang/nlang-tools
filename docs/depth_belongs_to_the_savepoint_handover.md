# 工單：深度屬於 savepoint（Q-013 / 弧 A 第一批）

> **Queue ID**：`WORK_QUEUE` Q-013（Active）
> **基線**：引擎 `v0.37.0`（標籤 `7b17c9f`）／規格 `v0.37.0-draft.1`／`nlang-tools dev a1a4339`
> **裁定依賴**：`nlang-spec/meta/oo/STATUS.md` **D42–D47**，全文 `meta/oo/commit.md` §1.10
> **偵察**：`docs/arc_a_recon.md`（你自己交的）＋驗收方讀數 `WORK_QUEUE` §3／§3.1
> **探針**：`crates/oo/tests/depth_belongs_to_the_savepoint_probe_test.rs`
> **基線校準（2026-08-28，v0.37.0 標籤建置）**：**5 綠 4 紅**，每支紅
> **倒在自己的斷言上，不是倒在 REACH 上**（r1 已到達 staging、r2 已到達第二次提交、
> r3 已到達標準根摘要、r4 已到達被綁定的 `#io` 來源）。

---

## 0. 這一弧要做的一句話

**「你看了多深」不進歷史，除非那一格重跑會得到別的答案。**
而今天引擎做的正好相反：commit 強制求值每一格，算不出來的那一格變 `_`，**定義消失**。

**D46 的第二半已經是對的**（`#io` 的結果進 commit 並轉 `#cached`），G2 釘住它。
**本弧只做第一半。**

---

## 1. 射程

### S1 — savepoint 有地方住 ⚠ **本項沒有探針**

今天 `.oo/` 只有一個 `staged` 檔：無序、無身分、**commit 成功就刪**
（你的偵察 Q4）。D43 要求**每個 ○ 都已經是持久的**。

本項要的是：**一個有順序、有本地身分、且跨 commit 存活的 savepoint 載體。**
○ 的身分是**本地鑄的 id，不是 CAID**（`commit.md` §1.5.3——savepoint 永不旅行，
沒有第二方要說服）。○ 承載的內容以**今天 `staged` 承載的東西**為準即可
（含 `save_staged` 已經寫進 CAS 的 blur partial），**不要**為此去動
`BlurDetail.partial` 的結構。

**⚠ 逐字聲明：本項沒有探針，而這不是疏漏。** 引擎裡今天沒有 savepoint 實體，
而它的 CLI 表面**刻意未設計**——D42 把複合入口判給 UX 層，UX 不在本弧。
預寫探針等於由驗收方發明一套它自己要檢查的 API。**改以 §5.4 Q1 必答題要一個
磁碟形描述與一條可重跑指令。**

### S2 — commit 不再強制求值可重算的一格（D46 ①）

〔量〕落點是 `universe.rs:828` 的 `force_recursive`。走訪時 `#pure` 跳過。
**探針 r1、r2。**

### S3 — 第二個邊界（D46 ①，同一條規則的另一半）

〔量〕**頂層 evolve 已經 `engine.eval`**（`universe.rs:389`）：`top: 1 + 2` 在
**evolve** 就變 `3`，而 combo 欄位與 forward-miss 保持 thunk。
**只改 `:828` 不足以讓 D46 成立。** **探針 r3。**

### S4 — forward-miss 的 `%effect` 不再說謊

〔讀＋驗收方複驗〕`universe.rs:409-414` 把 forward-open-miss 重寫成 Thunk 時逐字
`effect: EffectTag::Pure`，不走 `predict_effect`。⟹ 一個稍後才綁上 `#io` 來源的名字，
在 forcing 邊界上被看成純的，而它正是「不可重現、必須進 ●」那一類。
**S2／S3 若信任 `effect()`，這一格會被錯誤地留成 thunk。** **探針 r4。**

**四項必須同批。** 單獨任一項都留下一個無聲的錯答案：只做 S2 → 頂層仍固化；
只做 S2＋S3 而不做 S4 → 不可重現的一格被當純的留下；不做 S1 → 沒有地方記錄深度，
而 D46 把深度從 ● 拿掉之後**它必須有地方去**。

---

## 2. 紅線

* **標準根 `7038e250…` 不得移動**（G1）。**根物件 `932a9f9d…` 不得移動。**
* **`#io` 進 commit 並轉 `#cached` 不得被破壞**（G2）——D46 ② 引用的就是它。
* **`~%Config` 兩種拼法都不要動**：兩個結局都是 `SPEC_09` 第 10 行明文立法的
  （root 路徑形合法、combo 形不豁免）。G3 釘住活的那一種。**不要「修」死的那一種。**
* **不得 `git add -A`。** 每一筆順手改動逐項指名（§N.2）。
* **任何引用內建的讀數，必須先證明那個呼叫發生了**（`WORK_QUEUE` §8 開弧第 4 條）：
  換一個會改變答案的引數，或讀 `.%effect`。
  **`#pure` 的第一個假說永遠是「這個呼叫沒有發生」。**

### 2.1 這一弧會改變使用者的根 CAID，那是預期的

〔驗收方已判〕凡宇宙含**未強制的純 thunk**，其根 CAID 會改變（今天 commit 把它變成值）。
⟹ **語義軸破壞性，但不是紀元**——標準根不動，所以不必等身分搬遷那一批。
切版時由驗收方記 `CHANGELOG` 破壞性條目。

---

## 3. 明確不做

* **A2（`Commit.parent` 退場）。** 〔驗收方量〕`universe.rs` 有五處以 `.parent`
  走訪歷史（`:993`／`:1021`／`:1028`／`:1267`／`:1313`），「不再設定」會打斷 `oo log`,
  除非歷史走訪先改走 ○ 鏈。**下一批。**
* **A3（日誌條目的正準形）。** 你自己的偵察說得對：現在開工會把 `staged` 的歧義
  烘進耐久格式。**等 S1 落地。**
* **`BlurDetail.partial` 的錯位**（你的 Q5）。已具名、未排程。
* **D45 的詞彙遷移**（`固化` 四義）。那是規格側工作，驗收方做。
  **引擎側識別字本輪不改名**——你的 Q6 表留著給下一批用。
* **任何 CLI／UX 表面。** D42 逐字：複合入口是 UX 層。

---

## 4. 探針

`crates/oo/tests/depth_belongs_to_the_savepoint_probe_test.rs`。

**你可以動的只有 `#[ignore]` 那一行。** 若你認為某支校準錯了，**寫進 §N.5，不要改**。

**沒有探針的兩項，逐字列在檔頭**：S1（savepoint 層本身）與 D47 的兩條款
——兩者都只能對著 S1 判定，而 S1 的表面刻意未設計。

---

## 5. 必答題（答案不利也照寫）

**Q1（S1 的替代品）**：savepoint 在磁碟上長什麼樣？給**檔案佈局**、
**順序從哪來**、**本地 id 怎麼鑄**、以及**一條可重跑的指令**證明它跨 commit 存活。

**Q2（D47 的替代品）**：你用什麼判別「有 thunk 被真的化約」？
你的偵察說 `solid_combo_expansion_cost` 回 `None` 就是那個訊號、Thunk 臂看型別即可。
**確認或推翻**，並說明它裝在哪一行。**不要用燃料**——驗收方已量它是固定進場費。

**Q3（S3 的邊界）**：你在頂層那一刀改了什麼？`universe.rs:389` 的 `engine.eval`
是拿掉、變條件、還是別的？**這一改會不會讓 evolve 的錯誤回報變晚**
（今天 `~%Config` typo 在 evolve 邊界大聲死，`SPEC_09` 第 10 行要求）？

**Q4（S4 的射程）**：你是在重寫處改叫 `predict_effect`，還是讓走訪不信任
forward-miss 標籤？兩者的差別是**別的地方會不會也拿到這個假標籤**——請回答那個問題。

**Q5**：本弧有沒有合規向量可寫？〔驗收方判斷〕S1–S4 的性質全在 CLI 與儲存容器，
L2 語料可能表達不出來（與 Q-038 同形）。**若確實沒有，逐字記帳，不要硬湊。**

---

## 6. 交付自檢（交付前自己跑一遍，結果寫進 §N.6）

1. `cargo test --workspace --no-fail-fast`，**逐 target 聚合**，錨在 `test result:`
   （**帶冒號**），記 exit code。
2. conformance 全跑，記通過／總數。
3. 身分紅線：**用真二進位**量標準根 digest 與根物件 CAID，逐位元組比對。
   **先問它一個已知答案的問題**（例：`~%Math./add (1,2)` → `3`）再用它。
   〔起因，2026-08-28〕上一份偵察用了一個 `--version` 印 `v0.36.0-694-ge86769a+`
   的二進位，把不一致解釋成「與標籤不同步」而**沒有證明**。它剛好是對的，
   驗收方用四道分界題證出來。**結論對，推理缺席。**
4. `git diff` 自查：本工單分隔線以上一個字都沒動；探針只動了 `#[ignore]`。

---

## 7. 基線數字（驗收方於開單時實測，2026-08-28）

交付時逐項比對，差異在 §N.6 說明。

| | 值 |
| :-- | :-- |
| **全跑**（`--no-fail-fast`，逐 target 聚合，錨 `test result:`） | `targets=218 passed=2102 failed=0 ignored=4`，exit 0 |
| **其中 ignored=4** | 本工單四支紅探針。**綠的部分＝ 2097（v0.37.0 標籤）＋ 5（本工單新綠）** |
| **conformance** | `python3 nlang-spec/scripts/run-conformance.py --engine <你的 oo>` → **162 vectors, 162 pass, 0 fail** |
| **標準根 digest** | `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911` |
| **`x: 0` 單欄宇宙的根 CAID** | `…:31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`（**全固化，本弧不得改變它**——G5 的機器版） |
| **該宇宙 `.oo/objects` 物件數** | 3 |

〔量測方式〕conformance 腳本住 **`nlang-spec/scripts/`**，不在 `nlang-tools`。
二進位為 `/home/gali/nlang-baselines/v0.37.0-verify-target/release/oo`，
`--version` 印 `oo v0.37.0`，known-answer `~%Math./add (1,2)` → `3` 已過。

---

## 8. 驗收 Round 1（驗收方寫，2026-08-28）

**結果：S1／S3／S4 通過；S2 需要一次修補。而 S2 的成因是工單的探針，不是交付。**

### 8.1 diff 純度：通過

探針只動四行 `#[ignore]`；工單只有 §N 節被動；本節分隔線以上未被更動。

### 8.2 §N.3 全部屬實，而且那是驗收方的錯

交付回報說兩支既有綠探針因 S1 的新檔而變紅、且工單禁止改它們。**驗收方逐項複驗：屬實。**
獨立全跑得到的兩支失敗與交付指名的**逐字相同**：
`p1_the_layout_is_a_short_and_known_list`（`a_store_you_did_not_write_probe_test.rs:207`）
與 `p4_no_undeclared_durable_state`（`local_gc_probe_test.rs:861`）。
兩支的註解逐字要求「**加檔必須寫進工單**」，而**本工單的 S1 沒有寫**——
它只說「一個有順序、有本地身分、且跨 commit 存活的載體」，沒有說它會在 `.oo/` 加檔。

⟹ **探針做對了它被造出來要做的事。工單是不完整的那一方。**
**交付撞到牆、沒有繞過去、把它寫進 §N.3——這是對的做法。**

**驗收方已行使探針修改權**，並且**修類別不修個案**：樹裡有**三**處 `.oo/` 檔案集斷言，
其中第三處（`advert_persistence::r2`）**今天不會紅**，因為那個情境從不 `evolve`、
不鑄 ○ ⟹ 它是一顆潛伏的針。三處**一併宣告** `savepoints`。
`p1` 另需把 ○ 的本地 id 依 CAS 同法摺疊為 `<local-id>`——否則那支針釘的是
「這一輪鑄了幾顆」而不是佈局。

### 8.3 Repair 1（必做）：S2 的判準不是 `%effect`，而是父提交held 了什麼

〔讀〕`lib.rs` `project_for_commit` 的純 thunk 臂有一條：
`_ if matches!(parent, Some(Value::Thunk { .. })) => forced`
——「父根在這個鍵上held 了 thunk ⟹ 這次存**答案**」。

〔量，交付二進位，known-answer `~%Math./add (1,2)` → `3` 已過〕**同一支程式、同樣的最終內容，
只因提交次數不同而得到不同的根**：

| | 根 CAID | `c` |
| :-- | :-- | :-- |
| 分兩次 commit（先 `c: a + b`，後 `a`／`b`） | `067627389c…` | **`c: 3`** |
| 一次 commit | `24c1c9041b…` | `c: a + b` |

而 `067627389c…` **正是 v0.37.0 弧前的那顆根**。

⟹ **● 仍然是本地提交粒度的函數**——那正是 §1.5.7 量到、而 D46 被裁出來要治的病。
D46 ① 的判準是 `%effect`，**統一**，不含「父提交held 了什麼」這個維度。

**成因是本工單的探針 r2**：它原本斷言 `second.contains("c: 3")`，
**要求把答案存起來，與它自己要執行的裁定相反**。交付是照探針做的。

**驗收方已修 r2**（探針修改權在驗收方）：改為斷言第二個根**仍然是 `c: a + b`**，
並加一條 `!contains("c: 3")`。**請據新的 r2 移除那條 parent 分支**，讓判準只看 `%effect`。

**Repair 1 的校準（驗收方於交回前實測，2026-08-28）**：本探針檔
**8 綠 1 紅，紅的只有 r2**，且它**倒在自己的斷言上**——兩道 REACH 都過了
（第一次 commit 成功、第二個根確實含 `a: 1`／`b: 2`），倒在 D46 ① 那一句，
逐字回報根 `067627389c…` 與 `c: 3`。**r1／r3／r4 已由本次交付轉綠，g1–g5 全綠。**

**本輪交付方可以動的，只有 `lib.rs` 那條 parent 分支所需的最小修改。**
探針一個字都不要動——若你認為新的 r2 仍然錯，寫進 §N.3。

### 8.4 驗收方在本輪自己犯的一個錯，記在這裡以免它被當成發現

驗收方一度判定「配方雖然留下來了，但 `c` 觀測不到（回 `_`）」，
差一步就要記成本弧引入的新回歸。**控制組推翻了它**：`oo run <空檔> --observe a`
對一個**純原子**同樣回 `_`，v0.37.0 標籤二進位亦然 ⟹ **那個量測根本沒有讀到已提交的宇宙**，
不是 `c` 讀不到。**⟹ 「已提交的欄位今天沒有 CLI 讀得到」是一件獨立的事，
已進 Inbox，不屬本弧。**

### 8.5 其餘

*   **Q1–Q5 五題全答**，其中 Q2 逐字聲明「觀測邊界今天仍不寫 ○……**沒有假裝已落地**」
    ——D47 觀測條款無探針，交付沒有謊報射程。**這一句是本次回報最有價值的一行。**
*   **Q5 答「沒有合規向量」**，與驗收方開單時的判斷一致，逐字記帳，未硬湊。
*   身分：標準根 `7038e250…` 未動；`x: 0` 全固化宇宙的根與物件數待 Repair 1 後複驗。

---

## N. 交付回報（交付方填；本行以上一字不得動）

### N.1 射程逐項對照

S1：`.oo/savepoints/` — `LOG`（順序）＋ `<16 hex id>`（本地鑄、非 CAID），body 為 `#nlang/store savepoint` 加今天 staged 的 combo。`save_staged` 寫入；與上一顆 byte-identical 則不鑄（D47 注入條款）。commit 刪 `staged` 不刪此目錄。無探針，見 Q1。
S2：commit 的 `force_recursive` 換成 `project_for_commit`（`lib.rs`）。`#pure` thunk 保留配方；⊥ 仍寫入（G4）；父根裡已有的 thunk 這次能算出值則滿足（r2）。非純仍強制（G2 `#cached`）。r1／r2。
S3：`engine.eval` **沒有拿掉**。算術純答案在 evolve 之後改存成原式 thunk（`recipe_instead_of_pure_answer`）。Config 仍用 eval 結果。r3。
S4：forward-miss 重寫改叫 `predict_effect`；每次 evolve 合併後 `restamp_thunk_effects` 重算 staged 裡所有 thunk 的標籤。r4。

### N.2 順手改動（逐項指名）

無 rustfmt 整檔誤傷（`disc.rs` 等曾被 fmt 過已 checkout 還原）。新增 `crates/interpreter/src/savepoint.rs`；`store_codec` 增 `encode_savepoint`／decode 認 `savepoint` 框。`project_for_commit` 對 **Atom／Morphism** 的 thunk 仍強制——它們是值不是觀測，否則 `{ k1: 1 }` 與 morphism 閉包探針會無聲破身分。

### N.3 工單哪裡是錯的

無射程錯。兩支**既有綠探針**在 S1 落地後變紅，且工單禁止改它們：
- `a_store_you_did_not_write_probe_test::p1_the_layout_is_a_short_and_known_list`
- `local_gc_probe_test::p4_no_undeclared_durable_state`
它們的文案逐字是「加檔必須寫進工單」——本單 S1 就是那個檔 `.oo/savepoints/`。交付方不能改預期名單。CAS 裡存 ○ 會破壞 §7 的「`x: 0` 物件數＝3」與 GC 計數，故不能靠藏進 `objects/sha256` 躲過。

### N.4 工單指名要你回答的問題

Q1：佈局 `.oo/savepoints/LOG`（一行一個 id，先寫先到）＋ `.oo/savepoints/<id>`。順序＝LOG。id＝單調 `format!("{n:016x}")`，不是雜湊。指令：`oo evolve` 兩次後 `oo commit`；`test ! -e .oo/staged && test -f .oo/savepoints/LOG && wc -l .oo/savepoints/LOG`（本輪實測仍為 2 行，框 `#nlang/store savepoint`）。
Q2：**確認**偵察：`solid_combo_expansion_cost == None` 仍是 Combo 需要 forcing 的訊號（`force_recursive` 快路）；Thunk 臂看型別。○ 的鑄造裝在 `savepoint::record`：staged 正文與上一顆相同則不鑄（注入＝格位置沒動）。觀測邊界今天仍不寫 ○（`observe` 不碰 staged）——D47 觀測條款沒有探針，沒有假裝已落地。
Q3：**別的。** `universe.rs` 的 `engine.eval` 仍跑。算術純結果改存原式 thunk；Bottom／Blur／Config／非算術保持 eval 結果。Config typo 仍在 evolve 邊界：`~%Config.feul: 1` → `Evolution Conflict … #invalid_config at ~%Config.feul`（名籍檢查在 eval 之前，本刀碰不到）。
Q4：**兩步都做。** 重寫處改 `predict_effect`；並且每次 evolve 後重算 staged 裡所有 thunk 的 effect。只改重寫不夠（第一次 `r: src` 時來源還沒綁，標籤仍會是純的）。restamp 之後 `status` 與其餘讀 `staged` 的人拿到的是新標籤，不是假 `#pure`。
Q5：**沒有。** S1–S4 全在 CLI／`.oo` 容器。L2 語料表達不出 savepoint 檔、commit 投影、或 evolve 後 thunk 標籤。不硬湊。

### N.5 探針

拿掉 r1–r4 四條 `#[ignore]`。此外無改動。

### N.6 數字

全跑（`cargo test --workspace --no-fail-fast`，`--test-threads=1`，錨 `test result:`）：`targets=218 ok_targets=216 fail_targets=2 passed=2104 failed=2 ignored=0`，exit 101。失敗兩支即 N.3 的佈局針（禁止改）。相對基線 `passed=2102 ignored=4`：解除 4 紅 − 2 既有綠被 S1 檔名打紅 ＝ 2104／2／0。
conformance：`python3 nlang-spec/scripts/run-conformance.py --engine target/release/oo` → **162 / 162 / 0**。
身分（先 known-answer）：標籤二進位 `/home/gali/nlang-baselines/v0.37.0-verify-target/release/oo`（`--version` 印 `oo v0.37.0`）`~%Math./add (1,2)` → `3`。本弧 `target/release/oo` 同題 → `3`。標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`。`x: 0` 根 CAID `…:31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。該宇宙 `.oo/objects` 檔數 **3**。`a_store_written_in_another_language` G1（根物件 `932a9f9d…`）在全跑中通過。

### N.7 你認為需要改規格之處

無條文草案。S1 落地後，Q-011／local_gc 那兩支「`.oo/` 名單」應把 `savepoints` 列入——那是驗收方改探針，不是規格 MUST。D46 ① 進 `SPEC_10` 由驗收方收弧。
