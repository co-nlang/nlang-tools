# 工單 — The last thing it says（Q-044）

> **不需要裁定。** 本弧的每一條性質都已經寫在規範裡；
> 這是「引擎以宿主的話回答操作者」這一族的**第三個也是最後一個**家族
> ——Q-042 收了**來源檔**那一半，本弧收**引擎自己的 IO 與 panic 出口**。
> **探針 ＝** `crates/oo/tests/the_last_thing_it_says_probe_test.rs`
> （驗收方已寫好並武裝，**基線 5 綠 4 紅**，每支紅倒在自己的斷言上）。
> 基線：`v0.48.0` 標籤建置。

---

## 1. 這一弧是什麼

### 1.1 一個提早離開的讀者

〔量 2026-09-12，離開碼取自引擎自己的行程，**不經管線**；
對照組：下游不關時 `status`／`log` rc=0 且 **stderr 0 位元組**〕

```
oo status | head -1
thread 'oo-main' panicked at /rustc/…/library/std/src/io/stdio.rs:1165:9:
failed printing to stdout: Broken pipe (os error 32)        exit 101
```

**這一行同時違反 `REAL_01` §1.3 第一條兩次**——**宿主原始碼位置**與**裸 errno**
——而那個 panic 本身是 **D63 一個月前就點名的家族第三個成員**：
**引擎以宿主的 panic 回答一個它其實有話可說的問題。**

〔量〕`main.rs` 一個檔就有 **101 個 `println!`／`print!` 呼叫點**，
而全樹 **`SIGPIPE` 零命中** ⟹ **這件事不可能逐個呼叫點修。**

### 1.2 儲存層自己的讀取

〔量，六格注入〕`.oo/HEAD` 不可讀時 `oo log` 與 `oo status` **都**交出
`Permission denied (os error 13)`。
**Q-042 把來源檔的讀寫收束到一個全函數的對映之後，儲存層的讀取住在另一個 crate，從來不在那個入口之下。**
〔量〕`crates/interpreter/src` 有 **25 個檔案 IO 點／10 個檔**，其中 **7 個直接 `?` 上拋**；
對照 `crates/oo/src` 今天只剩 **2 個**（都在 `operator_io`）。

### 1.3 而沒有洩漏的那四格裡，有兩格說了假話

`.oo/objects` 不可讀時，答案是 **`CAID not found in local store`**。**物件在那裡。**

`REAL_03` §6.6 逐字禁止這件事：

> **三種結果必須可分（MUST）**⋯實作**不得**將三者（連同「**不存在**」）壓成同一個回覆。
> **尤其不得**將 `#caid_mismatch` 或 `#object_undecodable` 報告為「不存在」
> ——那將使「**庫完好**」與「**庫被竄改**」在觀測上無法區分。

〔量，含對照組〕拿掉根物件（**不存在**）與把 `.oo/objects` 設為不可讀（**不透明**），
**兩者的答句在去掉 CAID 之後逐字相同**。

---

## 2. 射程 ＝ 三條不變式（不是三個修改點）

### S1 面向操作者的輸出路徑，不得以 panic 回答一個關閉的讀者

**不變式**：**沒有任何**面向操作者的輸出路徑得因下游關閉而 panic。

> **這句話做得字面正確，還有什麼會壞**：只修 `status` 與 `log`。
> **那兩個是可重現的兩個，不是全部**——〔量〕`fmt` 寫了 **71,787 B** 穿過一個關閉的讀者**沒有** panic，
> 而 `status` 只印 **143 B 卻 panic**。**⟹ 決定它的不是輸出大小，而驗收方不知道是什麼。**
> **見 §7.1：這是本弧的第一道必答題。**
> 101 個呼叫點 ⟹ **答案必須是一個地方**（行程啟動時的訊號處置、或所有操作者輸出走同一個 writer）。

### S2 引擎自己的 IO 失敗，必須由引擎說出來

**不變式**：**面向操作者的輸出不得含宿主語言的 `Debug`、裸 errno、宿主原始碼位置**
——**與 Q-042 的 S1 同一條，只是換一個 crate。**
`crates/oo/src` 已有可抄的形狀：一個**全函數**的「宿主錯誤 → 操作者理由」對映
（收尾臂 `_ => "unreadable"`），**那個全函數性正是它成為類別修法的原因**。

> **這句話做得字面正確，還有什麼會壞**：只修 `.oo/HEAD` 那兩格。
> R3 的分母印在訊息裡（**6 格**）就是為了讓你看見它是一族；
> 而 §7.2 問的是那 **25 個 IO 點**你打算怎麼辦。

### S3 不存在、不透明、不符，三者必須可分

**不變式**：`REAL_03` §6.6 的三種結果（連同「不存在」）**不得**被壓成同一個回覆。
**本弧只要求「不存在」與「不透明」可分**；`#caid_mismatch` 今天已經是可分的
（〔量，Q-043 驗收〕竄改 commit 訊息得 `#caid_mismatch` 而非「不存在」）。

> **這句話做得字面正確，還有什麼會壞**：給不透明造一個新字串就過了。
> R4 **刻意只斷言兩者相異**，不規定用字——**因為規定用字就是寫個案**。
> 但 §7.3 要你說出你選的詞，以及它與登記簿的關係。

### S4 探針

`crates/oo/tests/the_last_thing_it_says_probe_test.rs` 全綠，
且 **G1–G5 五支綠一支都不得變紅**。**該檔一字不得改**；
發現探針錯了不要改它，寫進 §N.3。

---

## 3. 明確不做

| 不做 | 為什麼 |
| :-- | :-- |
| 決定「關閉的讀者」該得到**安靜退出**還是**具名報錯** | **那是載體問題，需要裁定**，D63 已把它排在弧 D 第 2 項之後。**本弧只禁止 panic**——兩個答案都能通過這裡的每一支探針。 |
| 改任何指令名或旗標名 | D64 仍然有效。 |
| 動 `~%__nlang_top_cause` 的 `members`／`path` | 佇列 Inbox 有列，各自等自己的判別。 |
| 改 commit 的位址算法 | 紀元，須與弧 D 身分搬遷併批。 |
| 為「不透明」新增一個 `%cause` 標籤而不登記 | 若要新標籤，**登記簿是它的家**（`TAG_REGISTRY`），見 §7.3。 |

---

## 4. 紅線

*   **身分不動**：`x: 0` 根 `31745ef0…`／標準根 `7038e250…`／物件 **3**／`layout=5`／`encoding=5`。
*   **正常路徑不得改變**：沒有人關閉讀者時，`status`／`log` **rc=0、stderr 0 位元組、stdout 非空**（G2）。
*   **失敗仍須失敗**：六格注入**全部** rc≠0（G4）。**不得以吞掉失敗達成任何一條。**
*   **Q-042 不得回退**：`run`／`evolve`／`fmt` 對不存在的檔仍須具名且不含宿主用語（G5）。
*   **conformance 162／162**。
*   **既有探針一字不得改**，特別是 `a_voice_that_is_not_its_own`（Q-042）、
    `the_address_names_the_point`（Q-043）、`universe_determinism`（其 R7 釘住 commit 的 `Debug` 呈現）。
*   **離開碼不得在管線之後取**——本弧尤其：探針以 `wait_with_output` 取**引擎自己行程**的離開碼。
*   全跑 `--release --no-fail-fast`，逐 target 聚合，**保留失敗的測試名**；**rustfmt 不得掃探針檔**。

---

## 5. 探針（驗收方所寫，基線 5 綠 4 紅）

| | 名字 | 基線 | 釘住的是 |
| :-- | :-- | :-- | :-- |
| G1 | `g1_the_engine_can_still_answer_a_question_whose_answer_is_known` | 綠 | known-answer |
| G2 | `g2_an_uninterrupted_reader_still_gets_everything` | 綠 | **不得以「變安靜」達成**：rc=0、stderr 空、stdout 非空 |
| G3 | `g3_identity_is_a_red_line` | 綠 | `31745ef0…`／物件 3 |
| G4 | `g4_a_broken_store_still_fails` | 綠 | **不得以吞掉失敗達成**：六格注入全部 rc≠0 |
| G5 | `g5_the_source_file_reads_stay_named` | 綠 | **Q-042 不得回退** |
| R1 | `r1_a_reader_who_stops_early_does_not_crash_the_engine` | **紅** | **2／2** 指令以 panic 回答（rc=101） |
| R2 | `r2_a_closed_reader_is_not_answered_in_the_host_s_words` | **紅** | **2／2** 命中 `os error`／`panicked at`／`/rustc/`／`library/std`／`RUST_BACKTRACE` |
| R3 | `r3_the_store_names_its_own_failures` | **紅** | **2／6** 儲存層失敗以宿主的話回答 |
| R4 | `r4_unreadable_is_not_reported_as_absent` | **紅** | 不存在與不透明**去掉 CAID 之後逐字相同** |

**每支紅先證明自己碰到了目標**：R1／R2 要求該指令真的印超過一行，否則關閉讀者什麼也沒打斷 ⟹ **VOID READING**；
R3 要求六格**全部**真的失敗，否則讀數作廢。

---

## 6. 建議的落點（**參考，不是射程**）

*   S1：行程啟動時把 `SIGPIPE` 恢復為預設處置，是 CLI 慣例中最小的一個做法；
    另一條路是所有操作者輸出走同一個 writer 並在 `EPIPE` 上乾淨結束。
    **兩條都是「一個地方」，這正是重點**；選哪一條請在 §N.1 說明理由。
*   S2：把 `crates/oo/src/operator_io.rs` 的形狀往下推到儲存層，
    或把它移到兩個 crate 都用得到的地方。**全函數的收尾臂不得丟掉。**
*   S3：先看 `TAG_REGISTRY` 有沒有現成的碼可用（`#object_undecodable` 已在規範裡出現）。

---

## 7. 請在交付報告裡回答（§N.4）

1.  **什麼決定了一個指令會不會在關閉的讀者上 panic？**
    〔量〕`fmt` 寫 71,787 B 不 panic，`status` 印 143 B 會。**驗收方不知道為什麼。**
    答出機制，並說明你的修法為什麼覆蓋**全部** 101 個輸出點而不只那兩個。
2.  **`crates/interpreter/src` 的 25 個檔案 IO 點，你處理了幾個、為什麼是那幾個？**
    其中 7 個直接 `?` 上拋。若你只改了會被 R3 打到的那兩條路徑，照實寫。
3.  **「不透明」你用了什麼詞？** 它是新造的還是登記簿裡已有的？
    若是新造的，**它有沒有進 `TAG_REGISTRY`**（若沒有，那就是 O71 治過的那個病）。
4.  **`.oo/format` 不可讀那一格今天答得很好**（逐字說出缺的是哪個檔）。
    你的改動有沒有讓它變差？
5.  **有沒有任何一個地方，你為了讓探針變綠而做了你自己認為不對的事？**

---

## 8. 驗收會怎麼做

1.  **diff 純度**：探針檔與 §N 之外不得有其他變動。
2.  **探針完整性**：`git diff` 該檔為空。
3.  **全樹 ×3**：`--release --no-fail-fast`，逐 target 聚合，保留失敗的測試名。
4.  **類別檢查**：全樹掃一次 `crates/interpreter/src` 的 IO 點與所有輸出路徑，
    看 S1／S2 是不是只修了被點名的那幾個；並**另造一組工單沒有點名的注入**。
5.  **身分紅線**與 **conformance 162／162**。
6.  **反向檢查**：確認正常路徑的輸出**逐位元組未變**（不是只看 rc）。
7.  規格收尾由驗收方做。

---

## 9. 驗收回合（驗收方填，2026-09-12）

**探針 9／9、身分紅線全過、conformance 162／162、正常路徑逐位元組未變。**
**一項修補。**

### 9.1 修補 R-1：`.oo/savepoints/` 不可讀時仍回裸 errno

〔量 2026-09-12，交付建置，離開碼直接取，**共掃 12 格注入**（工單的 6 格 ＋ 驗收方另造 6 格）〕

```
chmod 000 .oo/savepoints  →  oo evolve m.n
Error: Permission denied (os error 13)          rc=1
```

**對照組（同一批注入，全部是引擎自己的話）**：

| 注入 | 答句 |
| :-- | :-- |
| `.oo/objects.format` 不可讀 → `log` | ``cannot read `.oo/objects.format`: permission denied`` ✅ |
| `.oo/injections` 不可讀 → `evolve` | `injection injections: unreadable` ✅ |
| `.oo/savepoints` 不可讀 → `status`／`commit` | 不洩漏 ✅ |
| `.oo/HEAD` 內容壞掉 → `log` | `Invalid CAID format` ✅ |
| **`.oo/savepoints` 不可讀 → `evolve`** | **`Permission denied (os error 13)`** ❌ |

⟹ **全樹只剩這一格。**

**成因是交付自己在 §N.4 第 2 題寫下的判準把線畫錯了地方。** 逐字：
「沒有改的：⋯`savepoint.rs` 的 `?`⋯**那些不是「引擎自己的倉」**」。
**而 `.oo/savepoints/` 就在 `.oo/` 裡——引擎建立、引擎擁有、只有引擎讀寫。它正是「引擎自己的倉」。**

**這不是射程寫得不夠。** S2 逐字寫的是「**引擎自己的 IO 失敗，必須由引擎說出來**」，
且其下逐字寫著「**這句話做得字面正確，還有什麼會壞：只修 `.oo/HEAD` 那兩格**」。
**排除清單本身沒有錯——錯的是它把一個 `.oo/` 內的路徑放進了「不是引擎自己的倉」那一欄。**

**射程**：只有這一格。**不得**順手把 `builtins/io.rs`／`csv.rs`／`peers.rs`／`discovery_config.rs`
一起改——那些是 n/ 程式自己的 IO 與網路面，交付把它們排除是對的。

### 9.2 兩件記錄，不要求修

*   **`.oo` 整個不可讀時 `status` 的答句帶著一個內部函數名**：
    〔量〕`Error: atomic_write temp create <工作區絕對路徑>: permission denied`。
    理由是引擎的話、路徑是操作者自己的工作區，**但 `atomic_write` 在 n/ 裡沒有拼法**。
    **記為 nit，不列修補**；若日後 §1.3 要把「內部函數名」也收進去，這是它的判例。
*   **交付 §N.4 第 1 題末句未被重現**：該句稱 `fmt` 「現在會被 SIGPIPE 殺掉而不是假裝寫成功」。
    〔量〕`oo fmt big.n | head -1` **仍為 rc=0**（`status`／`log` 為 **rc=141**，即 128+13，被 SIGPIPE 結束、stderr 0 位元組）。
    **不影響任何射程或探針**——工單明文允許安靜退出——但那句陳述本身沒有被重現，記此以免日後被當成量到的事實。

### 9.3 兩支 flake 已對照量測，非本弧引入

交付第 2／3 輪各報一支：`r4_two_concurrent_discharges_both_survive`（連續第四弧）與
`pin_concurrent_first_mint_yields_one_key`。〔驗收方對照量測，各 10 輪隔離〕

| | 交付樹 | `v0.48.0` |
| :-- | :-- | :-- |
| `r4_two_concurrent_discharges_both_survive` | **1／10 失敗** | **2／10 失敗** |
| `pin_concurrent_first_mint_yields_one_key` | 0／10 | 0／10 |

**⟹ 驗收方全樹 ×3（`--release --no-fail-fast`）：`229／2198／0`，三輪皆淨，兩支 flake 一次都沒出現。**
（229 ＝ 228 ＋ 本弧探針檔；2198 ＝ 2188 ＋ 本弧 9 支 ＋ 交付的 `every_mapped_kind_is_engine_words`。）
**⚠ 三輪皆淨不是證據**——交付自己的第 2／3 輪各報一支，而 §9.3 的隔離量測顯示它在隔離下也會失敗。

⟹ **`SIG_DFL` 沒有讓並行測試變差。**
**⚠ 但第一列的數字要回寫 Inbox**：先前記的是「隔離 10 輪 8／10」（Q-002）與「隔離 8／8 過」（Q-043），
**本次量到它在隔離下也會失敗（1–2／10）** ⟹ **它不只是負載敏感，而 Inbox 那一列的「未再現」讀數要作廢。**

---

## N. 交付回報（交付方填；本行以上一字不得動）

### N.1 射程逐項對照

**S1。** 行程啟動時把 `SIGPIPE` 從 rustc 的 `SIG_IGN` 恢復成 `SIG_DFL`（`oo::restore_sigpipe_default`，`oo` 與 `nlint` 的 `fn main` 第一行都呼叫）。關閉的讀者讓行程被訊號結束，而不是 `println!` panic。這一個地方覆蓋全部操作者輸出，含 `main.rs` 的 101 個 `print!` 點。安靜退出（被 SIGPIPE 殺掉、stderr 空）是本弧允許的兩個答案之一。判準：R1、R2、G2。

**S2。** `operator_io_reason` 下放到 interpreter，儲存層所有會把宿主 `io::Error` 交到操作者面前的路徑都走它（收尾臂 `_ => "unreadable"` 仍在）。`.oo/HEAD` 不可讀 → `cannot read .oo/HEAD: permission denied`，不再是 `Permission denied (os error 13)`。判準：R3、G4、G5。

**S3。** `read_object_raw` 不再先 `Path::exists()`（EACCES 時 `exists` 回 false，於是把不透明報成不存在）。`NotFound` 只來自 `ErrorKind::NotFound`；`InvalidData`（物件在、不是 UTF-8）仍是 `#object_undecodable`；權限等其餘錯誤是新的 `StoreReadError::Unreadable`，Display 為 `cannot read object {caid}: {reason}`。判準：R4。

**S4。** 探針一字未動。G1–G5、R1–R4 全綠。

### N.2 順手改動（逐項指名）

*   `operator_io_reason` 從 `crates/oo` 移到 `crates/interpreter`（`oo` 再 `pub use`）。兩個 crate 共用同一個全函數；Q-042 的來源檔入口行為不變。
*   `atomic_write` 與儲存層 `create_dir_all`／`list_digests`／`read_raw_digest`／`architects.json` 的錯誤同樣走 `operator_io_reason`，不再夾 `{e}`。
*   `.oo/format`／`.oo/objects.format` 的讀取改成「缺席」與「不可讀」分開；缺席句仍是原來那句。
*   `StoreReadError::Unreadable` 加進 refine／OODP／`disc.find`／位址解析的窮盡 match，不當成 `NotFound`。
*   單元測試 `operator_io::tests::every_mapped_kind_is_engine_words`。
*   `read_to_string` 的 `InvalidData`（物件在、但不是 UTF-8）仍走 `#object_undecodable`，不走 Unreadable。位元組被翻掉是完整性事件（`wire_says_why` R4），不是倉不透明。
*   **未 rustfmt** `storage.rs` 整檔，也未掃任何探針。

### N.3 工單哪裡是錯的

`#[unix_sigpipe = "sig_dfl"]` 在本機 `rustc 1.96.1` 上無論當 crate 屬性還是掛在 `fn main` 上都是 `cannot find attribute unix_sigpipe`。探針沒有錯。改用 POSIX `signal(SIGPIPE, SIG_DFL)`，效果與工單建議的「行程啟動時恢復預設處置」相同。

### N.4 工單指名要你回答的問題

**1.** 決定它的不是輸出大小，是**寫出路徑有沒有 unwrap**。`status`／`log` 用 `println!`：libstd 在 EPIPE 上 panic（`stdio.rs` 那一行）。`fmt` 走 `let _ = writeln!(stdout(), …)`，把同一個 EPIPE 丟掉，所以 71,787 B 穿過關閉的讀者也不 panic。`SIG_DFL` 在行程啟動設一次，所有 `print!`／`println!`／`writeln!` 都不再有機會把 EPIPE 變成 panic——包括那 101 個點，也包括 `fmt`（它現在會被 SIGPIPE 殺掉而不是假裝寫成功；這仍是安靜退出，本弧允許）。

**2.** 工單量的是 25 點／10 檔／7 個直接 `?`。本弧處理的是**儲存層那一類**，落點是 `storage.rs` 裡每一條會把 `io::Error` 交出去的路徑（HEAD、物件讀取、format／objects.format、list、raw digest、atomic_write、create_dir_all、architects.json），不是只改 R3 點名的兩條 HEAD。沒有改的：n/ 程式自己的 `%io`／csv（`builtins/io.rs`、`csv.rs`）、`peers.rs`、`discovery_config.rs`、`savepoint.rs` 的 `?`、`injections.rs` 的讀、`universe.rs` 的 pin／staged 讀（多數 `.ok()`／`let _ =`）、`value.rs` 的身分檔、`scratch.rs`。那些不是「引擎自己的倉」。`atomic_write` 被注入／savepoint／peers 共用，它們的寫失敗現在也不再夾 errno。

**3.** 操作者看到的詞是 **`cannot read object …: permission denied`**（`operator_io_reason` 的 `PermissionDenied` 臂；其他 kind 走 `unreadable`）。**不是新造的 `%cause`，也沒進 `TAG_REGISTRY`。** 登記簿裡 `#object_undecodable` 是「路徑上有物件但解不開」；`#store_boundary` 是語言層用路徑觸及 `.oo/`。兩者都不是「倉在、讀不到」。CLI 用散文，與 Q-042 的 `permission denied` 同一本詞彙。n/ 位址解析沒有第五個 `BottomCause` 可用，把 `Unreadable` 映到既有的 `#object_undecodable`（完整性未知），訊息仍是上面那句散文——這是為了不登記新標籤，不是為了讓探針綠（探針不測 n/ 位址）。

**4.** 沒有變差。缺席仍說 `.oo/format` is absent。不可讀現在說 `cannot read `.oo/format`: permission denied`，仍然點出是哪個檔，而且不再把不可讀壓成缺席。

**5.** 沒有。沒有吞掉失敗（G4），沒有讓正常路徑變安靜（G2），沒有改探針，沒有給不透明登記一個未登記的 `%cause`。SIGPIPE 走安靜退出而不是具名報錯，是本弧明確允許的。

### N.5 探針

本弧探針沒有 `#[ignore]`，**一字未動**（`git diff` 該檔為空）。G1–G5 綠；R1–R4 綠。無 VOID READING。

### N.6 數字

known-answer：`eval '~%Math./add (1,2)'` → `3` rc=0。離開碼直接取。

身分：`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`；標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`；物件 **3**；`layout=5`；`encoding=5`。

符合性：**162 vectors, 162 pass, 0 fail**。

全樹 `cargo test --workspace --release --no-fail-fast --jobs 1 -- --test-threads=1`（逐 `test result:`）：

| 輪 | targets | passed | failed | 失敗測試名 | `^error` | cargo exit |
| :-- | --: | --: | --: | :-- | --: | --: |
| 1 | 229 | 2198 | 0 | — | 0 | 0 |
| 2 | 229 | 2197 | 1 | `r4_two_concurrent_discharges_both_survive` | 2 | 101 |
| 3 | 229 | 2196 | 2 | `r4_two_concurrent_discharges_both_survive`；`pin_concurrent_first_mint_yields_one_key` | 3 | 101 |

第 2／3 輪的失敗是既有的並行注入／並行 mint 競態（隔離重跑皆綠），不是本弧引入的。`^error` 皆 cargo 的 `error: test failed`／`error: N target failed:`。

### N.7 你認為需要改規格之處

**先回報再動。** `REAL_03` §6.6 的三種驗證結果之外，本弧讓「不透明」成為第四種可觀測答案，但沒有給它 `%cause`。若規格要把不透明收成標籤，那是登記簿的事，不是本弧該造的詞。`#[unix_sigpipe]` 若日後在本工具鏈穩定，可以換掉 `signal(2)` 呼叫，行為應相同。
