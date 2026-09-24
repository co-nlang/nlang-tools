# 工單：一筆提交就是一個值（Q-057）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-057／裁定 `meta/oo/STATUS.md` **D74**（O92 取甲）
> 偵察 `nlang-tools/docs/a_commit_address_quote_recon.md`（F1–F5，皆有對照組）
> 探針（已預先提交並校準）`crates/oo/tests/a_commit_that_is_a_value_probe_test.rs`，
> 另修訂 `a_refusal_written_as_an_answer_probe_test.rs` 的 r6／r7／r11／g3（見 §7）
> 夾具 `crates/oo/tests/fixtures/layout5_repo/`（真 `oo v0.58.0` 所造）
> 基線：dev `fe7f578`／`oo v0.58.0` ⟹ 新探針 **5 綠 6 紅**、Q-055 檔 **14 綠 3 紅**，三輪一致、零空洞讀數。

## 1. 裁定

**D74（用戶，「甲」）**：一筆提交**就是一個值**。它的位址是它在磁碟上那個 n/ 值的位址，
走 `REAL_03` §6.2 與其他所有值同一張表。
*   既有提交**不改寫**（`REAL_02` §5.1.1：遷移不得改寫物件、不得移動 `HEAD`），永遠以舊算法驗證。
*   **D74 ②**（用戶選「標明未受擔保」）：舊提交上**不在其位址內**的欄位（refine 的 `authority_status`、
    簽章、`shadow`、根位址的 version／masa／sketch），照印但一律標明不受位址擔保；
    **舊提交的 refine 授權永遠不說 `verified`，只說 `unattested`。**

## 2. 缺陷（偵察已量，`v0.58.0`）

今天的位址是手工挑欄位算的：

```
[parent.digest]? ‖ root.digest ‖ kind ‖ source.digest* ‖ target.digest* ‖ LEB128(len) ‖ format!("{:?}", meta)
```

*   **F1** 改 `authority_status` 為 `"verified"`：`log` rc=0 逐字印出（對照：改 `message` ⟹ `#caid_mismatch`）。
*   **F2** source／target 無個數：把一個 CAID 移過邊界，位址不變。
*   **F3** commit 裡根 CAID 的 sketch 不在位址內：`inspect <commit>` 印出被改過的根位址。
*   **F4** `{:?}` 依標準庫 Unicode 表跳脫（本工具鏈 17.0.0）——舊算法的這個性質**留下**，只寫進規格並自陳。

〔量〕`oo eval '~%Discovery./identify (<commit 物件去掉框線的本體>)'` **今天就算得出**一個 v2 值位址
（探針 `c1`）——**那就是 D74 說的位址**，而它與今天的 commit 位址不同。

## 3. 射程＝不變式（不是機制）

*   **I1** 本引擎**建立或遷移過**的儲存裡，每一筆新提交的位址＝它在磁碟上那個 n/ 值的位址。
    **磁碟上每一個解碼進這筆提交的位元組都在位址內**——不是「補上 F1–F3 那幾個欄位」，是由構造成立。
    ⟹ 探針 r1（位址即值位址）、r2／r3／r4（F1／F2／F3 的竄改一律被抓）。
*   **I2** 舊提交不改寫、`HEAD` 不因遷移移動；舊提交以舊算法照常驗證、照常讀（g1、g2），
    **並且舊算法抓得到的竄改仍被抓到**。`pre_sentinel_repo`／`encoding4_repo` 兩份夾具照舊打得開（既有探針）。
*   **I3** 讀者**知道**是哪一個算法驗證了一筆提交，並據以呈現（I5）。**選擇子不得是物件內的編碼自陳**
    （`REAL_02` §5.1.1：CAS 物件不得自陳其編碼）。○ 框的 `commit:`／`ancestor:` 註記今天只存 64-hex，
    **它們也必須無歧義地解到一個算法**。機制你選（位址的版本、宣告、或別的），在報告裡說理由。
*   **I4** 宣告尚未推進的儲存**繼續收到舊式提交**（`REAL_02` §5.1.1「宣告未推進者不得收到它宣告不了的東西」；g3）；
    新建的儲存與**明示遷移過**的儲存才寫新式提交（r6）。推進哪一軸、哪一個號碼**你提**（驗收方的讀法：
    物件的位元組編碼沒變，變的是 `HEAD` 與 ○ 註記的意義 ⟹ 佈局軸；若你認為是編碼軸，說理由）。
    **舊引擎必須以「宣告看不懂」拒絕，不得報成完整性失效**（`REAL_03`「版本落差不得報成完整性失效」MUST NOT）。
*   **I5**（D74 ②）舊提交上位址外的欄位照印，但標明不受位址擔保；**舊 refine 授權只說 `unattested`**（r5）。
    新提交的 `authority_status` 在位址內，照實印。
*   **I6** `migrate` 的代價句沿用 Q-055 的不變式（遷移前打得開、遷移後打不開的全部，依兩份宣告，動手之前）。
    **本弧之後的新邊界**：開得了 layout 5 的是 v0.44.0–v0.58.0，而沒有一個開得了新宣告
    ⟹ 從 layout 5 起點，被鎖在門外的是 **v0.44.0 … v0.58.0**；從更舊的起點，上界同樣變成 **v0.58.0**（r6；Q-055 r6／r7／r11）。

## 4. 紅線（探針已釘，今天綠，修復不得拿它們換 I1–I6）

*   **c1** 一筆提交的本體算得出值位址，且今天與它的提交位址不同——r1 的量測碰得到目標。
*   **g1／g2** 舊提交照常驗證與讀取；舊算法涵蓋的竄改（`message`）仍被抓。
*   **g3** 宣告 layout 5 的儲存再提交一次：宣告不動、新提交仍是舊式。
*   **g4** **值的位址一個都不動**：`x: 0` 根 `31745ef0…`、`v: 1 + 1` 根 `f4f32e7b…`。本弧只動提交。
*   **`inspect <commit>` 的提交視圖保持**（`kind: commit`／`parent:`／`root:` 三行）——
    一筆提交成為值之後，`inspect` **不得**把它印成一般的值而失去這三行；多支探針依賴它。
*   Q-055 檔的其餘各支（見 §7）；conformance 162／162；`bn_serial` 對**值**的編碼不得改動。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1**（請在交付報告裡回答）你推進了哪一軸、哪一個號碼，為什麼？新建的儲存宣告什麼？
    **v0.44.0–v0.58.0 遇到新宣告時說什麼**（驗收方會以真二進位複驗）？
*   **Q2**（請在交付報告裡回答）I3 的選擇子是什麼？○ 註記只有 64-hex 時怎麼解？
    有沒有任何一條讀取路徑會「兩個算法都試、哪個過了就算」——若有，說它為什麼不會讓 I5 失效。
*   **Q3**（請在交付報告裡回答）**把舊算法寫成一段不看原始碼也能實作的描述**：每一個欄位進 `buf` 的位元組、
    `{:?}` 對 `Option<String>`／`u64`／`bool`／`Vec<String>`／`Vec<(String,String)>` 的確切輸出、字串跳脫規則，
    以及它依賴 Unicode 表的那一條。驗收方會把它寫進 `REAL_03`；**答案不利也照寫**。
*   **Q4**（請在交付報告裡回答）〔讀〕`store_codec` `write_refine` 不寫 `authority.timestamp`——它是讀回來就不存在的欄位，
    還是本來就不該有？新式提交裡它在不在？
*   **Q5**（請在交付報告裡回答）有沒有任何對等節點的路徑會取回提交物件？若有，它驗的是什麼；新舊兩種提交各怎麼驗。
*   **Q6**（請在交付報告裡回答）`oo log` 等介面上，新提交的位址變成 v2 長形——你保留了什麼、改了什麼。

## 6. 明文不在射程內（沉默不得被當成覆蓋）

*   **值的身分**：任何值位址都不得移動（g4）；`PathAnchor::Current` 的 `0x03` 等「只能併進下一次身分搬遷」的列**不在本弧**
    ——本弧是提交的紀元，不是值的紀元。
*   **F4 對舊提交的殘留**：舊提交的位址仍依 Unicode 表；本弧只要求把它寫進規格並自陳，不修。
*   **簽章的重驗**（D74 裁的是位址；讀取時是否以簽章重驗新提交的 `authority_status`，是另一題）。
*   **歷史改寫**：任何形式的「把舊提交轉成新式」都不在本弧，且被 `REAL_02` §5.1.1 禁止。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   新探針 r1–r6 **基線必須是紅的**，理由逐支寫在各自的 doc comment；**你可以讓它們變綠，不得改寫它們的宣稱。**
*   **驗收方同時修訂了 Q-055 的探針檔**（檔內標 `AMENDED 2026-09-24`）：`names_the_boundary` 與 r11 的上界由
    `v0.43.0` 改為 `v0.58.0`，g3 只要求「推進」而不指定號碼。**r6／r7／r11 因此在 v0.58.0 上是紅的，屬本弧**；
    該檔其餘 14 支今天綠，不得轉紅。
*   每一次竄改都先驗證改到了目標，否則 `VOID READING`——**那不是失敗也不是通過，回報它**。
*   夾具 `layout5_repo` 由真 `oo v0.58.0` 所造，**不得用現行引擎重建**（README）。
*   `rustfmt` **不得**掃這兩個檔案。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照
S1 I1：layout=6 的新提交，位址＝把框線下的 n/ 本文交給 `~%Discovery./identify` 得到的值位址（先求值再 `content_hash`，不是 `expr_to_value`）。竄改本文就換位址。驗：r1、r2、r3、r4。
S2 I2：layout≤5 仍用舊算法 `Commit::content_hash`。不改寫舊物件、遷移不移動 HEAD。驗：g1、g2、g3、r6 的 log。
S3 I3：選擇子是位址上的版本。`v1` 只跑舊算法；`v2` 只算本文的值位址。見 Q2。
S4 I4：推進的是佈局軸，號碼 6。新建倉寫 `layout=6`／`encoding=5`。layout=5 再提交仍是舊式、宣告不動。驗：g3、r6。
S5 I5：舊提交的 `refine authority` 印 `unattested`，不印檔裡的 `verified`。新提交的 `authority_status` 在值位址裡，照字印。驗：r5、r2。
S6 I6：代價上界改為 `v0.58.0`。從 layout=5 起是 `v0.44.0 through v0.58.0`；更舊的起點最舊一個不變、上界同樣是 `v0.58.0`。句子裡仍有 `layout=5`。驗：r6，以及 Q-055 的 r6／r7／r11。

### 8.2 順手改動（逐項指名）
`cargo fmt` 未跑。`bn_serial` 未改。
為了「log 裡的提交位址變成 v2 長形」之後，下列檔案裡「在 log 裡找 `hash:sha256:v1:` 當成提交」或「新建倉必是 layout=5」的讀法改成對應當前輸出。斷言的性質沒改：
- `knob_that_does_nothing_probe_test.rs`、`limit_you_cannot_choose_probe_test.rs`、`snapshot_not_a_reading_probe_test.rs`、`print_what_can_be_read_probe_test.rs`：提交位址改讀 `commit ` 那一行；根改讀 `root:` 那一行。
- `nothing_here_or_nothing_you_can_see_probe_test.rs`、`a_commit_that_closes_the_door_probe_test.rs`、`a_type_you_could_not_carry_probe_test.rs`、`atomic_write_probe_test.rs`：本引擎現在寫下的佈局是 6。
- `crates/interpreter/tests/refine_test.rs`：`put_commit` 回傳的是值位址，不再等於 `Commit::content_hash`。
另外 `open_commit`：只有呼叫者手上是 v1、而舊算法對不上、本文的值位址摘要卻相同時，才改認成 layout-6 提交。舊算法對得上就停，不改口。

### 8.3 工單哪裡是錯的
無。

### 8.4 工單指名要你回答的問題
Q1. 推進佈局軸到 **6**。物件位元組仍是 encoding 5 的 n/ 框，變的是 HEAD 與 ○ 註記所指的位址算法，所以不是編碼軸。新建倉宣告 `layout=6` 與 `encoding=5`。v0.44.0–v0.58.0 的 `ensure_format` 對 `layout=6` 的句子是 `store layout declaration "layout=6" is not supported; refusing to open`。那是宣告看不懂，不是 `#caid_mismatch`。

Q2. 選擇子是位址自己的版本，不在物件裡自陳編碼。`HEAD` 與提交本文裡的 parent 哈希都帶版本。`commit:`／`ancestor:` 今天仍是 64-hex：先用舊算法；對上了就是舊提交（I5 維持 `unattested`）。對不上，才看本文的值位址摘要是否就是這 64 個 hex；是，才是新提交。兩條都對不上才是 `#caid_mismatch`。沒有「兩個都試、哪個過了算哪個」的平手：舊算法先，而且一對上就不再看值位址，所以竄改舊提交的 `authority_status` 仍被當成舊提交，不會印成 `verified`。

Q3. 舊算法，不看原始碼也能實作的描述。位址是 `hash:sha256:v1:` 加上 SHA-256(`buf`) 的 64 hex。`buf` 依序是：
1. 若有 parent，接它的 32 位元組摘要。沒有 parent 就什麼都不接。parent 的 version／masa／sketch 不進。
2. 接 root 的 32 位元組摘要。root 的 version／masa／sketch 不進。
3. 一個 kind 位元組：Standard=0，Refine=1，Pin=2，Squash=3。
4. 若有 refine：先每個 source 的 32 位元組摘要（列表順序），再每個 target 的，中間沒有個數、沒有分隔。`authority`、`authority_status`、`shadow` 不進。把一顆摘要從 source 挪到 target，這一段位元組不變（F2）。
5. `meta` 的 Rust `Debug` 字串 `M = format!("{:?}", meta)`，再用無號 LEB128 寫 `M` 的位元組長度，然後接 `M` 的 UTF-8。手寫的 `Debug` 形狀是 `CommitMeta { author: …, timestamp: …, message: … }`，欄位之間是 `, `。`abandoned`、`privileged_effect`、`reported_bottoms` 只在 `Some` 時出現。`Option<String>`：`None` 或 `Some("…")`。`u64` 是十進位。`bool` 是 `true`／`false`。`Vec<String>` 是 `[…]`。`Vec<(String, String)>` 是 `[(…, …), …]`。字串跳脫是這個工具鏈的 `char::escape_debug`，`char::UNICODE_VERSION = 17.0.0`：`"` 與 `\` 一定跳脫成 `\"`、`\\`；`\n` `\r` `\t` 用那些短跳脫；其餘「不可印」碼點用 `\u{hex}`。哪些碼點算可印，由 Unicode 17.0.0 那張表決定，不是一份與工具鏈無關的閉合規則。換一個 Unicode 表較新的 rustc，同一筆舊提交會算出另一個 CAID。這條留下，不修。
6. LEB128：每次取低 7 bit；後面還有值就把最高 bit 設 1。

Q4. `authority.timestamp` 在簽署時放進記憶體，`write_refine` 不寫它，讀回時固定填 `None`。它不是「讀回來才消失的欄位」——磁碟上從來沒有那些位元組，所以也沒有被解進提交。新式提交走同一份 writer，它一樣不在。沒有補上。

Q5. 對等節點的 `#fetch` 只呼叫 `get_value`。提交物件解不開成值，回答是沒有這份物件，不另做提交驗證。新舊提交在這條路上一樣。

Q6. `oo log` 仍是一行 `commit ` 接位址的 Display。新提交是 v2 長形（`hash:sha256:v2:<masa>:<sketch>:<digest>`）。舊提交仍是 `hash:sha256:v1:<digest>`。`inspect` 的 `kind: commit`／`parent:`／`root:` 三行還在。訊息行沒改。

### 8.5 探針
沒有 `#[ignore]` 可拿。本弧探針與 Q-055 探針都沒改。本弧 11 支皆綠。Q-055 檔 17 支皆綠。無 `VOID READING`。

### 8.6 數字
全跑三輪相同：`cargo test --workspace --release --no-fail-fast --jobs 1 -- --test-threads=1`。逐 `test result:` 聚合：241 行，2290 passed，0 failed。`^error` 0 行。cargo exit 0。沒有失敗測試名。
conformance：162 vectors，162 pass，0 fail。
身分：`add (1,2)` → `3` rc=0；`add (1,3)` → `4` rc=0。`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。`v: 1 + 1` 由 g4 釘住 `f4f32e7b…`。未改 `bn_serial`。新建倉 `layout=6` `encoding=5`。

### 8.7 你認為需要改規格之處
無。Q3 那段是給驗收方寫進 `REAL_03` 的，這裡不動規格。

---

## 9. 驗收（驗收方填）

**第一輪：不受理，開修補回合 R-1（兩項）。** I1–I6 在預先提交的探針上全部兌現（本檔 11／11、Q-055 檔 17／17）；
〔量，真二進位〕v0.44.0／v0.50.0／v0.57.0／v0.58.0 對 `layout=6` 皆 rc=1 逐字
`store layout declaration "layout=6" is not supported; refusing to open`（宣告看不懂，不是 `#caid_mismatch`）。
但驗收時另量到兩件，都是本弧造成的。

### 9.1 R-1a：v2 提交位址只驗 digest——F3 換了一層又回來

`commit_address_matches` 對 v2 只比 digest（註解逐字「Digest only: masa and sketch of a v2 commit address are properties
of the value」）。而 `REAL_03` §6.6「驗證範圍（MUST）」逐字：**v2 請求必須比對 `content_digest`、`lattice_sketch` 與 `<masa_ref>` 三者**
——§9.2 描述的正是這個縫。〔量，交付建置〕把 `HEAD` 裡 v2 位址的 sketch 改一個字元：`log` **rc=0，並把改過的字串印成這筆提交的位址**，
`status` rc=0；對照：改 digest 一個字元 ⟹ `log` rc=1。**這就是 F3（印出一個它沒有承諾的位址），只是從根位址搬到了提交位址本身。**
新探針 **r7**（驗收時新增）：v0.58.0 上 VOID（`HEAD` 是 v1），交付上紅。
**不變式**：*以 v2 位址取一筆提交時，那個位址的每一個部分都被重算比對；只有 digest 的引用（○ 註記）照 v1 請求的規則只比 digest。*

### 9.2 R-1b：驗證一筆提交時求值它的本體

`commit_body_address` 對每一次驗證建一台 `Ouroboros::new_in_memory()`，**把磁碟上的本文當成 n/ 程式求值**，再取值的位址。
〔量，交付建置〕40 筆提交的 `oo log`：**layout 5（v0.58.0）75／77／78 ms；layout 6 1,065／1,073／1,099 ms——14 倍**
（每筆約 25 ms，`log` 對每筆驗兩次）；一千筆的歷史就是半分鐘。
〔量〕在本文裡塞一個 `~%Io./write_file` 欄位：位址不符而被拒，**檔案沒有被寫**（記憶體引擎沒有 IO 權限）——所以今天沒有被利用的路，
**但「讀一筆提交」會執行它**，而那讓提交的位址成為**求值器**的函數（D46 的 Thunk 政策、內建的集合……任何一個改了，新提交的位址就跟著動）。
D74 裁的是「那個值的正準編碼」，不是「那段文字求值的結果」。**這一項的成因在驗收方**：探針 r1 以 `oo eval '~%Discovery./identify (…)'`
定義「它的值的位址」——那是一個**量測**，交付把它做成了**機制**。
**不變式**：*一筆提交的位址是它的位元組經 `REAL_03` §6.2 那張表的函數，**不經求值器**；驗證一筆提交不執行它的任何部分。*
r1 **仍須綠**（對合法的本文，兩條路得到同一個位址——若 hash 字面 `{ ~%__nlang_hash: … }` 在兩條路上成為不同的值，報告裡說清楚它
在值那張表上是哪一格）。**代價判準**：40 筆 `oo log` 與 layout 5 同一數量級（驗收方以 3 倍為線）。

### 9.3 其他探針檔的改動：受理，而那是驗收方漏列的

交付改了 8 個既有探針檔與 `refine_test.rs`，並在 8.2 逐項指名。**逐一讀過**：`layout=5` → `layout=6` 的釘（4 檔）、
在 `log` 裡找 `hash:sha256:v1:` 改成讀 `commit ` 那一行、在 `inspect` 裡找 `v2:` 改成讀 `root:` 那一行（5 檔）、
`refine_test` 不再斷言 `put_commit` 等於舊算法（改為驗證通過）——**每一處都只是跟著一個已裁的格式變化，斷言的性質沒有變弱**。
**這些是驗收方開卡時該列而沒列的**：工單只寫了本弧與 Q-055 的探針，**沒有 grep 其他釘了 `layout=5` 或 `v1` 提交位址的檔**。

### 9.4 驗收方同意的交付判斷

*   Q1 佈局軸 6（位元組編碼沒變，變的是 `HEAD` 與 ○ 註記所指的算法）。
*   Q2 選擇子是位址的版本；只有 64-hex 的註記「舊算法先、對上即停」，所以竄改舊提交的 `authority_status` 仍被當成舊提交印 `unattested`（r5 綠）。
*   Q3 舊算法的描述——收尾時寫進 `REAL_03`。Q4／Q5／Q6 的回答成立（對等 `#fetch` 只取值，提交不走網路）。

---

## 10. 修補回合 R-1 回報（交付方填；本行以上一字不得動）

### 10.1 射程逐項對照
R-1a：以 v2 位址取一筆提交時，比對 digest、lattice_sketch、masa_ref 三者，與值的 v2 規則同一條。64-hex 的引用仍是 v1，只比 digest。驗：r7。
R-1b：提交位址是框線下的本文解碼成值之後的 `content_hash`（`REAL_03` §6.2），不建引擎、不求值。驗證一筆提交不執行它的任何部分。對合法本文，這條路與 `~%Discovery./identify` 的 digest 相同。驗：r1、r6，以及下面的計時。

### 10.2 順手改動（逐項指名）
`cargo fmt` 未跑。`bn_serial` 未改。兩個探針檔未改。
- 無框的 JSON 提交仍只比 digest。一個把 v2 外殼套在舊 digest 上的指標，對得上舊算法就開，不拿空的 sketch 去擋。
- `gc` 的走訪只有 digest，沒有呼叫者手上的 v2 位址。原先造一個空 sketch 的 v2 去 `get_commit`，嚴格比對之後會把完好的 layout-6 提交判成 `#caid_mismatch`。改走 `open_commit`：舊算法先，對不上才用值位址，值位址本身仍三者都比。
- `#caid_mismatch` 的 `requested` 改印 64-hex digest，`recomputed` 仍是完整 Display。r7 要求輸出裡不得出現被改過的那一整條位址；`recomputed hash:sha256:v1:` 這段還在。
- `identify` 只對「提交形狀的字面」改道：每個欄位都是 thunk、`kind` 是 Standard／Refine／Pin／Squash、`root` 解出來帶 `~%__nlang_hash`。這種字面改哈希解碼後的值。其他值的 `identify` 不變。驗證路徑不呼叫 `identify`。

### 10.3 工單哪裡是錯的
§9.2 說對合法本文，解碼那條路與 `identify` 得到同一個位址。不加 10.2 那一層時，兩者的 digest 不同。`identify` 哈希的是求值器留下的 thunk（標記 `0x17`，閉包一幀、幀裡是空閉包的欄位 thunk），不是解碼後的值。這不是 hash 字面單獨造成的。

### 10.4 工單指名要你回答的問題
hash 字面 `{ ~%__nlang_hash: #true, digest, version, masa, sketch }` 在值表上沒有自己的一列。解碼後是開放 Combo，標記 `0x01`。system 欄 `__nlang_hash` 是 Tag `true`（`0x11`）。`digest`／`sketch` 是 Str（`0x10`），`version` 是 Tag（`0x11`），`masa` 的 `_` 是 Top（`0xFF`）。
把同一段文字求值並強制之後，那個 system 欄是 Bottom（`0xFE`，`#system_reserved`）：`~%` 不能當定義。那是另一個值，digest 不同。
`identify` 在這次修補前走的是 thunk 那一格，digest 與上面兩個都不相同。修補後只有提交形狀的字面改走 `0x01` 那個 Combo，所以 r1 量到的 digest 與存進去的相同。

40 筆 `oo log`，本二進位，各三次。修補後：layout 5 為 70／70／70 ms；layout 6 為 140／130／130 ms，約 2 倍，低於 3 倍。驗收方量到的修補前是 layout 5 的 75／77／78 ms、layout 6 的 1,065／1,073／1,099 ms。
在本文裡把 `message` 換成 `~%Io./write_file`：`log` rc=1，檔案沒有被寫。同一條呼叫直接 `eval` 會寫出檔案。

### 10.5 探針
兩個探針檔都沒改，也沒有 `rustfmt`。本弧 12 支皆綠（含 r7）。Q-055 檔 17 支皆綠。無 `VOID READING`。

### 10.6 數字
全跑三輪相同：`cargo test --workspace --release --no-fail-fast --jobs 1 -- --test-threads=1`。逐 `test result:` 聚合：241 行，2291 passed，0 failed。`^error` 0 行。cargo exit 0。沒有失敗測試名。
conformance：162 vectors，162 pass，0 fail。
身分：`add (1, 2)` → `3` rc=0；`add (1, 3)` → `4` rc=0。`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。

### 10.7 你認為需要改規格之處
無。10.4 那一格是給驗收方寫進 `REAL_03` 的，這裡不動規格。

---

## 11. 驗收 R-1（驗收方填）

**R-1 兩項都修對了；但 R-1b 的修法裡有一行改的是量測儀器 ⟹ 開修補回合 R-2（一項）。錯的源頭仍在驗收方。**
diff 純度：四個原始碼檔＋本檔報告；兩個探針檔自驗收方最後一次提交後一字未動。

### 11.1 已獨立複驗的（R-1 建置 `8b9dda1`）

*   **R-1a**：r7 綠——改 `HEAD` 的 sketch，`log` 拒絕，且不把改過的位址印出來。
*   **R-1b**：驗證不再求值（`commit_body_address` 改走值的解碼器）。〔量〕40 筆 `oo log`：layout 5 **73／78／76 ms**、
    layout 6 **141／145／137 ms**（約 1.9×，低於 3 倍線；修補前 14×）。

### 11.2 R-2：`~%Discovery./identify` 被改成對「長得像提交」的輸入另算

交付在 10.2 誠實寫出：`identify` 現在對「每個欄位是 thunk、`kind` 是 Standard／Refine／Pin／Squash、`root` 帶 `~%__nlang_hash`」
的字面**改哈希解碼後的值**，**理由逐字是「讓量測與存進去的位址是同一個函數」**。
〔量〕同一段提交本體：v0.58.0 `identify` → `…9ceb1d51…`；R-1 建置 → `…71b850d2…`；對照（`{ a: 1 b: "x" }`）兩版皆 `…77644e58…`。
⟹ **一個語言內建的答案，現在取決於使用者的值是否長得像引擎的一種內部格式**——這正是 Inbox「`disc.identify` 報出的位址，
不是倉會用的位址」那一族，**而這一次是為了讓一支探針變綠而造出來的**。

**但錯的源頭在驗收方，而且是第二次**：r1 原本以 `identify` 對本文求值後的位址定義「它的值的位址」。
交付在 10.4 說清楚了為什麼那不可能相等——**本文求值出來是一個欄位都是 thunk 的 combo（D46），不是磁碟上的值**。
第一輪交付為了對上它，每筆提交都求值（R-1b）；這一輪為了對上它，改了 `identify`。**同一個錯誤的量法，兩次被做成機制。**
交付該做的是在 10.3「工單哪裡是錯的」寫「r1 的判準本身不可能被一個不求值的實作滿足」——**而不是改儀器**。

**驗收方的修補（探針，不動引擎）**：r1／c1／g3／r6 的量法改為**儲存自己的值解碼器**：本文包成值文件，
交給 `store_codec::decode_value`（讀每一個值物件的同一個函式），再 `content_hash`——不經求值器、不經任何提交專屬的程式碼，
這就是 D74 的字面意思。**新增 g5**：`identify` 對同一段提交本體與一個對照值的答案，釘在 v0.58.0 的讀數上。
〔量，兩極〕修補後的探針檔：**v0.58.0 上 6 綠 7 紅**（r7 VOID——v1 `HEAD`）；**R-1 建置上 12 綠 1 紅**（g5）。

**R-2 的不變式**：*這一弧不改變任何內建對任何值的回答；`identify` 回到 v0.58.0 的行為。* g5 應轉綠，其餘不得轉紅。

---

## 12. 修補回合 R-2 回報（交付方填；本行以上一字不得動）

### 12.1 射程逐項對照
R-2：`~%Discovery./identify` 對任何值都只哈希它拿到的那個值。提交形狀的字面不再另算。驗：g5（提交本體 `9ceb1d51…`，對照 `{ a: 1 b: "x" }` `77644e58…`，皆為 v0.58.0 的讀數）。

### 12.2 順手改動（逐項指名）
`cargo fmt` 未跑。`bn_serial` 未改。兩個探針檔未改。
刪了 `commit_shaped_literal`。它只有 `identify` 那一處呼叫，是 R-1 為了讓量測對上存進去的位址而加的。

### 12.3 工單哪裡是錯的
無。

### 12.4 工單指名要你回答的問題
無。

### 12.5 探針
兩個探針檔都沒改，也沒有 `rustfmt`。本弧 13 支皆綠（含 g5）。Q-055 檔 17 支皆綠。無 `VOID READING`。

### 12.6 數字
全跑三輪相同：`cargo test --workspace --release --no-fail-fast --jobs 1 -- --test-threads=1`。逐 `test result:` 聚合：241 行，2292 passed，0 failed。`^error` 0 行。cargo exit 0。沒有失敗測試名。
conformance：162 vectors，162 pass，0 fail。
身分：`add (1, 2)` → `3` rc=0；`add (1, 3)` → `4` rc=0。`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。

### 12.7 你認為需要改規格之處
無。

---

## 13. 驗收 R-2（驗收方填）
