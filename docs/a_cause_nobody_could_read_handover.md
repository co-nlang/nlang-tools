# 工單：沒有人讀得懂的成因（Q-075）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-075／裁定 `meta/oo/STATUS.md` **D93**（甲）／依據 `REAL_02` §5.1.1 判例（「一個引擎無法判斷的狀態，不得被報成另一個它判斷得了的狀態」）、D65（成因不入址）
> 探針（已預先提交並校準）`crates/oo/tests/a_cause_nobody_could_read_probe_test.rs`
> 基線：dev `499315f`／`oo v0.76.0` ⟹ **3 綠 3 紅**，三輪一致；三支紅的都紅在一般斷言，零空洞讀數。

## 1. 缺陷

〔量，v0.76.0〕已存的 ⊥ 帶著成因標籤（`~%__nlang_bottom: #conflict`）。把已提交物件裡那個標籤改成本引擎不認得的（`#from_the_future`）、或不是標籤的（`42`、`"conflict"`、`{ x: 1 }`），讀回一律 `_|_  ;; %cause: #conflict`；同一倉的其他欄位照常（`_.a` 為 1），**完整性檢查不察覺**——依 D65 成因不入址，位址不覆蓋它。工作集（注入檔）裡的 ⊥ 同樣。
〔讀 `store_codec.rs` `cause_from_value`〕兩處退路：值不是標籤 ⟹ `return Ok(BottomCause::Conflict)`；標籤不在清單 ⟹ `_ => BottomCause::Conflict`。（`decode_bottom` 的「成因缺席」分支到不了：只有成因鍵存在時才呼叫它。）
〔量〕今天 `BottomCause` 36 個成員、解碼清單 36 個名字、編碼 `as_tag` 36 個名字逐字對齊 ⟹ 退路只在**別的引擎寫的成因**（較新的版本、第二個實作）或竄改時觸發；**但兩側對齊沒有任何東西保證**——將來只改一側，本引擎自己寫下的 ⊥ 讀回就會無聲變成 `#conflict`。
〔讀〕`#blur` 的成因解碼（`blur_cause_from_value`）有同形的退路（不是標籤 ⟹ `Timeout`；不認得 ⟹ `MathSingularity(名字)`），但 `#blur` 的成因**進位址**（`BlurCause::as_bytes`），竄改會被完整性檢查擋下——**本弧不碰**，請在 Q3 確認這個讀法。

## 2. 裁定與依據

*   **D93（用戶，甲）**：本引擎讀不懂的已存 ⊥ 成因 ⟹ 報成 **`#object_undecodable`**，不得報成任何一個讀得懂的成因。
*   **同一裁定的第二半**：編碼（成因 → 標籤）與解碼（標籤 → 成因）兩側**由編譯器保證窮盡**——新增一個成因而只改一側，必須編譯不過；**不得**以 `_ =>` 把未列者收進某個既有成因。
*   否決乙（保留原標籤、標明不認得）。

## 3. 射程＝不變式（不是機制）

*   **I1** 已提交的 ⊥，成因讀不懂（不認得的標籤、不是標籤的任何值）⟹ 讀回 `#object_undecodable`，輸出裡沒有 `#conflict`（r1）。
*   **I2** 工作集裡的 ⊥ 同樣（r2）。**修類別**：凡是從耐久形讀回 ⊥ 成因的路徑（物件、注入、Savepoint 內容、經對等點收到的值——「線上就是 store」）都是本條的範圍；請在 Q1 列出你檢查過的每一條。
*   **I3** 各回報面一致：`eval`、`run --observe`（觀測新欄位或已提交欄位）說同一件事（r3）；`status` 同 r2。
*   **I4** 讀得懂的成因照原樣讀回（g1）；另一個讀得懂的成因換上去，照它讀（g3——D65：成因不入址，本弧不改這件事）。
*   **I5** 只有成因讀不懂：同一個值、同一個根的其他部分照常，不報完整性失敗（g2）。**不得**把整個物件當成解不開。
*   **I6** 兩側窮盡由編譯器保證（見 §2）；驗收方讀碼確認。
*   **I7** 不改磁碟格式、不推進佈局；值位址與提交位址不動（成因本來就不入址）。

## 4. 紅線（今天綠，必須保持綠）

*   g1–g3；`a_success_that_was_a_bottom`、`where_the_conflict_is`、`what_an_observation_leaves` 15（觀測 ○ 的答案含 ⊥ 成因）。
*   **交叉編譯** `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   conformance 162／162；`x: 0` 根 `31745ef0…`、`v: 1 + 1` 與 `1+1` 根 `f4f32e7b…`、標準根 `7038e250…`；`~%Math./add` (1, 2)=3、(1, 3)=4；新倉 `layout=9`／`encoding=5`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** 你檢查過的每一條「從耐久形讀回 ⊥ 成因」的路徑，各在哪裡、改後各答什麼。
*   **Q2** 窮盡怎麼由編譯器保證（給出結構；並說明：若有人新增一個 `BottomCause` 成員而忘了某一側，編譯錯誤長什麼樣）。
*   **Q3** `#blur` 成因：§1 的讀法（它進位址，所以竄改被完整性檢查擋下）是否成立？〔量〕給一次把已存 `#blur` 的成因改掉之後的讀回。**不要修**，陳述事實。
*   **Q4** 一個讀成 `#object_undecodable` 的 ⊥，再被提交（例如之後的 `evolve` 與 `commit` 把它帶進新的根）時，寫下的是什麼成因；那筆提交的根位址與帶原標籤時是否相同（D65）。陳述事實。

## 6. 明文不在射程內

*   `#blur` 成因的解碼（Q3 只陳述）。
*   保留／顯示別的引擎原本寫的標籤（D93 否決乙）。
*   規格文字（`REAL_02` §5.1.1 判例旁補一句或 `TAG_REGISTRY` `#object_undecodable` 的用法擴充）、`CHANGELOG`：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r3 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。**
*   成因標籤是 `TAG_REGISTRY` 的名字，探針讀它們。探針只改已存物件／注入檔裡的成因標籤（依 D65 位址不變），其他不碰。`rustfmt` 不得掃探針檔。
*   〔驗收方校準〕**兩極皆做**：
    基線 3 綠 3 紅（三輪一致）。
    一個最小參考實作（`cause_from_value` 的兩處退路改為 `ObjectUndecodable`）⟹ 本探針 6／6。
    同一參考實作跑全樹：**260 target／2508 passed／0 failed，exit 0**（＝ v0.76.0 的 259／2502 加上本探針 6 支）；交叉編譯 0 error。**無須預先修訂既有測試**。
    **守衛的變異**：`"conflict"` 也解成讀不懂 ⟹ g1（g3 連帶）；讀不懂就讓整個物件解碼失敗 ⟹ g2；`"divergent"` 錯解成 `Conflict` ⟹ g3（g1 連帶）。
    **參考實作沒有做窮盡（§2 第二半）**——那是本弧的一半，不要只照抄兩行退路（I6、Q2）。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 做了什麼

已存 ⊥ 的成因由一張 `cause_tags!` 讀寫。36 個 `BottomCause` 各一行。`as_tag` 是窮盡的 `const fn`。`from_stored_tag` 按字節對這張表；表上沒有的標籤，以及不是標籤的值，讀成 `BottomCause::ObjectUndecodable`。`cause_from_value` 裡的 `const _: () = BottomCause::CAUSE_TAG_ROUND_TRIP` 要求每一行的標籤解回去再編出來仍是自己。

`decode_bottom` 在成因鍵缺席時仍是 `Conflict`。呼叫點只在成因鍵存在時進來。`#blur` 的解碼未改。磁碟格式、`layout=9`、`encoding=5`、套件 `oo` `0.76.0` 留在原處。

### 8.2 順手改動（逐項指名）

1. `crates/interpreter/src/value.rs`：`cause_tags!`、`as_tag`、`from_stored_tag`、`CAUSE_TAG_ROUND_TRIP`。`as_cause_combo` 與 `primary_rank` 仍是各自的窮盡比對。
2. `crates/interpreter/src/store_codec.rs`：`cause_from_value` 改走這張表，並強制那份 const。

探針、規格、`Cargo.toml`、`Cargo.lock` 未改。沒有 rustfmt `value.rs`、`store_codec.rs`、探針。

### 8.3 工單哪裡是錯的

無。工單寫的交付後全樹是 260 target、2508 passed、0 failed。這次三輪的 `test result:` 與此相同。

### 8.4 工單指名要你回答的問題

**Q1** 從耐久形讀回 ⊥ 成因，改後都進 `cause_from_value`：標籤走 `from_stored_tag`，其餘值是 `#object_undecodable`。

1. 已裝框的 CAS 物件：`ObjectStore::decode_cas_value` → `store_codec::decode_value` → `decode_bottom`。量：把已提交 `b: 1 & 2` 物件裡的 `~%__nlang_bottom: #conflict` 改成 `~%__nlang_bottom: #from_the_future` 之後，`eval _.b` rc 0，stdout `_|_  ;; %cause: #object_undecodable`。探針 r1 對 `#from_the_future`、`42`、`"conflict"`、`{ x: 1 }` 同樣，輸出裡沒有 `#conflict`。
2. 注入：`injections::load_all` 把本文框成 injection 再 `decode_staged`。探針 r2：`status` 含 `#object_undecodable`，不含 `#conflict`。
3. Savepoint 的 combo 本文：`decode_document` 對 `savepoint` 跳過框線後 `expr_to_value`，同一條 `decode_bottom`。觀測的 `answer:` 是字串行，不再走 `cause_from_value`。這次改 blur 成因時，物件與 savepoint 兩份檔都含 `cause: #fuel_exhausted`。`oo log` 不印 combo 裡的成因，所以沒有另一句 CLI 句子；combo 本身的答案與第 1 條相同。
4. 線上：`oodp` 對已裝框的 `%result`，以及舊協定的裝框緩衝，呼叫 `decode_value`。文件本身解不開時記完整性事件，回 `#caid_mismatch`。裝框成功而成因標籤是外來的，走 `cause_from_value`。這次沒有架起對等點。未裝框的 JSON 仍用 serde：不認得的變體讓整份值失敗，呼叫端記 `StoreReadError::ObjectUndecodable`。現在的倉是 encoding=5 的裝框文字；g2 那條文字路徑上，只有成因讀不懂時物件的其他欄位仍在。

**Q2** 一張表展開成三件事：`as_tag` 的窮盡 `match`、`from_stored_tag` 的字節鏈、`CAUSE_TAG_ROUND_TRIP`。未知標籤的鏈尾是 `ObjectUndecodable`，沒有把未列成因收進某個讀得懂的成因。另外兩處手寫的窮盡比對是 `BottomDetail::as_cause_combo` 與 `BottomCause::primary_rank`。`oo` 的 `bottom_cause_tag` 也是窮盡比對；下面的編譯只查了 `nlang-interpreter`，所以日誌裡沒有它的診斷。

臨時加上 `BottomCause::CauseProbeOnly` 且不放進表，`cargo check -p nlang-interpreter` 三條 `error[E0004]: non-exhaustive patterns: BottomCause::CauseProbeOnly not covered`，分別在 `as_cause_combo`、`as_tag`、`primary_rank`。樹已復原。

把表上每一行的解碼都改成 `Conflict` 時，`error[E0080]: evaluation panicked: BottomCause::MissingKey does not round-trip through its stored tag`，`evaluation of value::BottomCause::CAUSE_TAG_ROUND_TRIP failed`。註記指向 `cause_from_value` 裡的那份 const。`Conflict` 自己仍對得上，所以第一條失敗的是 `MissingKey`。樹已復原。`let _ = CAUSE_TAG_ROUND_TRIP` 在 rustc 1.96 不會強迫求值；現在用的是 `const _: () = …`。

**Q3** 成立，未修。release 二進位。`~%Config.fuel: 1` 加上 `s: ~%Math./add (1, ~%Math./add (1, ~%Math./add (1, 1)))` 提交後，物件與 savepoint 都寫著 `~%__nlang_blur: #true cause: #fuel_exhausted`。把 `cause: #fuel_exhausted` 改成 `cause: #from_the_future` 之後，`eval _.s` 與 `status` 都是 rc 1：

`Error: #caid_mismatch: object at digest path is corrupt (integrity failure); requested 7ffd588e59282d46ecafdb02fb9032f45652c0eecf7c4ba93050e8b8934d6b9f, recomputed hash:sha256:v2:…:01d69276e9046d47e81a52589158f22abe9d0ebe097ce4916b1c66764ac669d4`

`status` 的句子前面多了 `Universe unavailable:`。

**Q4** release 二進位。先提交 `a: 1` / `b: 1 & 2`，`eval _.b` 是 `_|_  ;; %cause: #conflict`。把物件與 savepoint 裡的 `~%__nlang_bottom: #conflict` 改成 `#from_the_future` 之後，`eval _.b` rc 0，`_|_  ;; %cause: #object_undecodable`。該物件仍在原位址 `d3c78179a67500a8f7fbb8f39ed452715c4e0040f3406d1f3df16a98f3c4867d`。再 `evolve k: _.b` 並 `commit -m k`，新根裡 `b` 與 `k` 都寫成 `#object_undecodable`。這筆根位址是 `4745c4b046c49956944300d06fb2961f46fd151ab90447c2243210bbc35d9def`。對照組全程留著 `#conflict`，第二次提交的根位址相同。兩筆提交物件不同：`reported_bottoms` 是 `["b #object_undecodable", "k #object_undecodable"]` 與 `["b #conflict", "k #conflict"]`，`timestamp` 是 `1791170865253` 與 `1791170865424`，提交雜湊因此是 `5ed800815b45f6f7b81dd145ca2ddd5050886aa75f8690ff4672d9ae557b73f9` 與 `012fe6edcc370d5e8b2388ec6dc93504252edb0e45e29dc2f84f764cdea955fb`。

### 8.5 探針

`crates/oo/tests/a_cause_nobody_could_read_probe_test.rs` 未改，未 rustfmt。全樹第三輪 6／6，0.84s。`g1_known_causes_read_back_as_written`、`g2_only_the_cause_is_unreadable`、`g3_a_known_cause_is_read_as_written`、r1–r3 皆綠。

紅線，全樹第三輪皆 `test result: ok`、0 failed：`a_success_that_was_a_bottom` 5（0.36s）、`where_the_conflict_is` 9（0.65s）、`what_an_observation_leaves` 15（2.90s）。

交叉編譯 `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu`：`Finished` 4.48s，`^error` 0。

### 8.6 數字

1. 三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`：每輪 rc=0，`^error` 0，`test result:` 260 行，2508 passed，0 failed。`Running`／`test `／`test result:` 三輪各 3025 行，去掉耗時後相同。原始日誌的警告順序不必相同。
2. conformance 162／162，rc=0。cwd 是 `/home/gali/nlang`。引擎是 `nlang-tools/target/release/oo`。
3. `~%Math./add (1, 2)` → 3，`(1, 3)` → 4，rc=0。這兩次 `eval` 沒有建立 `.oo/`。euid 1000。`/tmp/oo-ephemeral-*` 為 0。
4. `oo inspect <HEAD>` 的 `root:`：`x: 0` 為 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。`v: 1 + 1` 與 `v: 1+1` 為 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`（`status` 該行帶 `(available)`）。新倉 `.oo/format` 為 `layout=9`，`.oo/objects.format` 為 `encoding=5`。

### 8.7 你認為需要改規格之處

規格檔這次沒有改。建議驗收方依工單第 6 節：`REAL_02` §5.1.1 判例旁補一句，擴充 `TAG_REGISTRY` 裡 `#object_undecodable` 的用法，並收 `CHANGELOG`。Q3 的 `#blur` 成因解碼仍是原樣。

---

## 9. 驗收（驗收方填）

### 9.1 第一輪驗收：**不受理，開修補回合 R-1（一項；起因在驗收方的工單）**

交付 `2e8b23a` 照工單做得完整：`cause_tags!` 一張表展開出編碼的窮盡 `match`、解碼鏈與編譯期往返檢查，並示範了兩種破壞各自編譯不過（漏成員 ⟹ `E0004`；往返不合 ⟹ `E0080`）。全樹 ×3 **260／2508／0**、交叉編譯 0、conformance 162／162、身分未動；驗收方竄改重現：同一物件裡 `_.a` 為 1、`_.b`（不認得的成因）讀不懂、`_.c`（`#divergent`）照讀。

**不受理的理由在驗收方。** 工單 §2 指定的 `#object_undecodable` 是 `REAL_03` §6.6 的**完整性裁決**——「位置上有物件但無法解碼，完整性無法裁定」——而本題的物件**位址驗證通過**（D65：成因不入址），只有成因讀不懂。報它即違反同節「裁決必須為真」：「把完好說成損壞……會教操作者忽略完整性訊息」。驗收方推薦 D93 甲之前沒有查該標籤的定義。**D94（用戶，2026-10-05）**：改為新成因 **`#unrecognized_cause`**（「這裡有一個成因，本引擎認不出來」）。

#### R-1 射程（不變式）

*   **R1-1** 本引擎讀不懂的已存 ⊥ 成因（不認得的標籤、不是標籤的值）讀回 **`#unrecognized_cause`**；輸出裡沒有 `#conflict`，**也沒有 `#object_undecodable`**（r1–r3，已修訂）。`#object_undecodable` 只留給 §6.6 的那一種情況。
*   **R1-2** **每個成因的名字只寫在一處。** 〔讀〕補分支時，名字字串有三份：`cause_tags!` 表、`BottomDetail::as_cause_combo`、`oo` 的 `bottom_cause_tag`——編譯器只保證不漏成員，**不保證三份名字一致**（今天 36 個逐字相同）。這與 D93 第二半是同一類：請讓另外兩處由表導出，或說明為何不能。
*   **R1-3** 其餘不變：D93 的兩側窮盡、g1–g3、§3 的 I4–I7。

#### R-1 必答

*   **R1-Q1** `#unrecognized_cause` 在 `primary_rank`（多分支塌陷時的主成因優先序）排在哪裡，為什麼。
*   **R1-Q2** R1-2 的結構；以及新增一個成員時，若只在表上加了一列，另外兩處會怎樣（編譯錯誤或自動得到名字）。
*   **R1-Q3** 第一輪 Q4 重做一次：讀成 `#unrecognized_cause` 的 ⊥ 被帶進新的根時寫下什麼、根位址是否與對照組相同。

#### 探針修訂（`AMENDED 2026-10-05 at Q-075 R-1 (D94)`；你不得再改它）

r1–r3 改用 `unrecognized(o)`：含 `#unrecognized_cause`、不含 `#conflict`、不含 `#object_undecodable`；g1 亦不得出現 `#unrecognized_cause`。測試函式名未改（仍寫 `undecodable`），以免 diff 噪音。
〔驗收方校準〕v0.76.0 基線 **3 綠 3 紅**（三輪一致）；你的 `2e8b23a` **3 綠 3 紅**（紅在答 `#object_undecodable`）；最小參考實作（`UnrecognizedCause` 加進列舉與表、兩處退路指向它、`as_cause_combo`／`primary_rank`／`bottom_cause_tag` 各補一個分支——**沒有做 R1-2**）**6／6**。守衛變異（繞過表、標籤一律讀不懂 ⟹ g1；讀不懂即整個物件解碼失敗 ⟹ g2；`divergent` 錯解成 `Conflict` ⟹ g3）各自轉紅。同一參考實作跑全樹（含修訂後的探針）：**260 target／2508 passed／0 failed，exit 0**；交叉編譯 0 error。

---

## 10. R-1 交付回報（交付方填；本行以上一字不得動）

### 10.1 做了什麼

讀不懂的已存 ⊥ 成因改為 `BottomCause::UnrecognizedCause`，標籤 `#unrecognized_cause`（D94）。不認得的標籤與不是標籤的值都進這裡。`#object_undecodable` 仍是表上自己的一列，留給物件本身沒有解出來的情況。

成因的裸名字只寫在 `cause_tags!` 那一列。`BottomDetail::as_cause_combo` 的 `%val` 取 `cause.as_tag()`。`oo` 的 `bottom_cause_tag` 是 `"#" + as_tag()`。`primary_rank` 仍是窮盡比對，排的是優先序，不是名字。

套件仍是 `oo` `0.76.0`。`layout=9`，`encoding=5`。探針這回合沒有再改。

### 10.2 順手改動（逐項指名）

1. `crates/interpreter/src/value.rs`：列舉尾端 `UnrecognizedCause`；表上一列；`from_stored_tag` 的鏈尾改為它；`as_cause_combo` 改由表取名字；`primary_rank` 把它放在 1。
2. `crates/interpreter/src/store_codec.rs`：`cause_from_value` 的非標籤退路改為 `UnrecognizedCause`。
3. `crates/oo/src/main.rs`：`bottom_cause_tag` 改為 `"#" + as_tag()`。

規格、`Cargo.toml`、`Cargo.lock`、探針未改。沒有 rustfmt `value.rs`、`store_codec.rs`、`main.rs`、探針。

### 10.3 工單哪裡是錯的

無。上一輪 `2e8b23a` 按當時的 D93 甲報了 `#object_undecodable`。本回合按 D94 改報 `#unrecognized_cause`。

### 10.4 工單指名要你回答的問題

**R1-Q1** `UnrecognizedCause` 的 `primary_rank` 是 **1**。數字小的更優先。0 只有 `Divergent`：那是本引擎已經判定的不終止。2 是 `Conflict` 那一組。讀不懂的成因若排在 2 或更後，多分支塌陷會留下一個叫得出名字的成因。排在 1，它會蓋過 `#conflict`，並留在 `#divergent` 後面。同一層還有 `ObjectUndecodable`：兩者都要留在格子衝突前面，標籤仍然各寫各的。同層並列時，`min_by_key` 留先遇到的那一個。

**R1-Q2** 名字的字串只在表上。`as_cause_combo` 與 `bottom_cause_tag` 沒有自己的清單，所以表上多一列，這兩處就用那一列的名字。`primary_rank` 不導出名字；漏了它的分支，程式庫編譯不過。

量過：臨時加入 `BottomCause::CauseProbeOnly`，只在表上加 `cause_probe_only`，兩處名字函式沒有改。`cargo check -p nlang-interpreter` 只有一條 `error[E0004]: non-exhaustive patterns: BottomCause::CauseProbeOnly not covered`，落在 `primary_rank` 的 `match`。兩處名字函式沒有錯誤。補上 `CauseProbeOnly => 9` 之後，`bottom_cause_tag` 得到 `#cause_probe_only`，`as_cause_combo` 印出的文字含 `#cause_probe_only`。臨時成員與那個測試已從樹上拿掉。

**R1-Q3** debug 二進位，來源與這次提交相同。先提交 `a: 1` / `b: 1 & 2`，`eval _.b` 是 `_|_  ;; %cause: #conflict`。把 `~%__nlang_bottom: #conflict` 改成 `#from_the_future` 之後，`eval _.b` rc 0，`_|_  ;; %cause: #unrecognized_cause`。再 `evolve k: _.b` 並 `commit -m k`，新根寫著 `b` 與 `k` 都是 `~%__nlang_bottom: #unrecognized_cause`。根位址 `4745c4b046c49956944300d06fb2961f46fd151ab90447c2243210bbc35d9def`。對照組全程留著 `#conflict`，第二次提交的根位址相同，根裡仍是 `#conflict`。兩筆提交物件不同：`reported_bottoms` 是 `["b #unrecognized_cause", "k #unrecognized_cause"]` 與 `["b #conflict", "k #conflict"]`，`timestamp` 是 `1791195365471` 與 `1791195365911`，提交雜湊是 `f7f25e278617d6d4b02c81f95b30d5ed4e50e2c84fed36c734d6f323d2476e1b` 與 `3ad34967223d75e0e2fad62d556b7154ddad6fb457236f5e432062702a19655e`。

### 10.5 探針

`crates/oo/tests/a_cause_nobody_could_read_probe_test.rs` 本回合未改。全樹第三輪 6／6，0.87s。g1–g3 與 r1–r3 皆綠。

紅線，全樹第三輪皆 `test result: ok`、0 failed：`a_success_that_was_a_bottom` 5（0.42s）、`where_the_conflict_is` 9（0.64s）、`what_an_observation_leaves` 15（3.05s）。

交叉編譯 `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu`：`Finished` 4.47s，`^error` 0。

### 10.6 數字

1. 三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`：每輪 rc=0，`^error` 0，`test result:` 260 行，2508 passed，0 failed。`Running`／`test `／`test result:` 三輪各 3025 行，去掉耗時後相同。
2. conformance 162／162，rc=0。cwd 是 `/home/gali/nlang`。引擎是 `nlang-tools/target/release/oo`。
3. `~%Math./add (1, 2)` → 3，`(1, 3)` → 4，rc=0。這兩次 `eval` 沒有建立 `.oo/`。euid 1000。`/tmp/oo-ephemeral-*` 為 0。
4. `oo inspect <HEAD>` 的 `root:`：`x: 0` 為 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。`v: 1 + 1` 與 `v: 1+1` 為 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`（`status` 該行帶 `(available)`）。新倉 `layout=9`，`encoding=5`。

---

## 11. R-1 驗收（驗收方填）

**R-1 受理（一個修補回合；成因在驗收方的工單）。** 交付 `ce68844`：探針、分隔線以上、`Cargo.lock`、版本皆未動。
**交叉編譯**（`touch` 後強制重查）0 error。**全樹 ×3**（期間不跑其他量測）：**260 target／2508 passed／0 failed，`^error` 0，exit 0**，三輪逐行相同。
**驗收方矩陣**〔交付二進位〕：已提交 `a: 1`／`b: 1 & 2`／`c: c + 1`，把物件裡 `b` 的 `#conflict` 改成 `#from_the_future` ⟹ `_.a` 1、`_.b` `_|_  ;; %cause: #unrecognized_cause`、`_.b.%cause` `#unrecognized_cause`、`_.c` `#divergent`；`(1 & 2).%cause` 仍 `#conflict`。R1-2：名字只在 `cause_tags!` 表上，`as_cause_combo` 與 `bottom_cause_tag` 由 `as_tag()` 導出（讀碼確認）。
**驗收方的引用錯誤（本工單 §1、§2、§6 皆受影響）**：「一個引擎無法判斷的狀態，不得被報成另一個它判斷得了的狀態」在 `REAL_02` **§4.1.1**（D73 之註），不是 §5.1.1。帳本與規格已改正。
身分：`31745ef0…`／`f4f32e7b…`（`1 + 1` 與 `1+1`）／標準根 `7038e250…`；known-answer 3／4（不建 `.oo/`）；conformance 162／162；新倉 `layout=9`／`encoding=5`。
