# 工單 — 一道不是你撞上的天花板

| | |
| :-- | :-- |
| **Queue ID** | **Q-002** |
| **基線** | 引擎 `v0.45.0` ／ 規格 `v0.45.0-draft.1`（`nlang-tools dev 3c356f9`／`nlang-spec local 95d9a62`） |
| **裁定依賴** | **D62**（三種條件三個名字）＋ **D63**（離開碼由載體決定）——**兩則皆已裁**（用戶 2026-09-07），§8 開弧 3 條件滿足，**本單是實作不是偵察** |
| **偵察** | `docs/a_ceiling_that_was_not_yours_recon.md`（驗收方自量，含 §7 離開碼 12 格） |
| **探針** | `crates/oo/tests/a_ceiling_that_was_not_yours_probe_test.rs`（**基線 4 綠 6 紅，五次全跑逐次相同，每一支紅都倒在自己的斷言上**） |

---

## 1. 這一弧是什麼

```
$ ulimit -v 300000
$ oo eval '1 + 2'
_|_ (%cause: #stack_overflow)
$ echo $?
0
```

`TAG_REGISTRY` §2.7.3 逐字告訴操作者這個標籤的意思是「**我到不了那裡**」，
補救是「攤平結構，或換一個能走更深的實作。**調旋鈕沒有用**」。

**輸入是 `1 + 2`。攤平它不會有任何效果。而唯一有效的動作就是調一個旋鈕。**

成因是 `crates/parser/src/lib.rs:1182–1184` 兩行：

```rust
let parser = builder.spawn_scoped(s, f).map_err(|_| ParserNestingLimitExceeded)?;
parser.join().map_err(|_| ParserNestingLimitExceeded)
```

`join()` 回 `Err` 的**充要條件**是該執行緒 panic；`spawn_scoped` 回 `Err` 的
**唯一**居民是宿主拒絕建立執行緒。**真正的深度界不在這條路徑上**——它由兩道
執行緒**之外**的閘處理（文字巢深 256 在 spawn 之前、AST 高度 4096 在 join 之後），
而〔量〕兩道今天都答得對。

⟹ **那兩格裡沒有任何一個合法的 `#stack_overflow`。**

---

## 2. 射程 ＝ 五條不變式（不是五個呼叫點）

**寫在這裡的是性質。做到性質，機制由你決定；做到機制而性質沒成立，不算做完。**

*   **S1 ——— 一個原因標籤只得回報它所命名的那件事。**
    `#stack_overflow` 依 `TAG_REGISTRY` §2.7.3 命名的是「**實作的遞迴天花板**」。
    宿主拒絕給資源、以及實作自身的 bug，**都不得**以此名回報。
    〔D62〕三種條件三個名字：① 形狀超過柵欄（`#stack_overflow`，不動）／
    ② 宿主拒絕給資源／③ 剖析器 panic。**②③ 的拼法由你提，驗收方只檢查
    它們互相可分、且都不叫 `#stack_overflow`。**

*   **S2 ——— 邊界的載體不得鑄節點級 `_|_`，且必須給出診斷訊息與非零離開碼。**
    〔D63＋`TAG_REGISTRY` §0.2〕值的載體 ⟹ rc=0；邊界的載體 ⟹ rc≠0。
    §2.7.4 逐字判**剖析期**為邊界（「尚無宇宙可言」），§0.2 逐字禁止邊界
    「鑄節點級 `_|_`」並規定其形為「診斷訊息與退出碼」。
    **①②③ 三者皆為剖析期 ⟹ 三者皆適用，包括真陽性的 ①。**

*   **S3 ——— 同一個標籤的兩個載體必須可判別。**
    〔§0.2「載體必須可判別（MUST）」〕`#stack_overflow` **確實**有兩個載體，
    而**求值期那一個是對的**：它是工作集裡一顆真的 `_|_`，rc=0。
    今天兩者在 stdout 與離開碼上都分不出來。**S2 做到，S3 自動成立**——
    但它是獨立的驗收條件，不得以「反正 S2 做了」代替。

*   **S4 ——— 引擎不得以宿主的 panic 回答一個它有話可說的問題。**
    `crates/oo/src/main.rs:385` 的 `.expect("spawn oo-main thread")` 在同一種
    壓力下 rc=101、逐字印出宿主原始碼路徑、**沒有任何 n/ 答案**。
    這是同一族的第四格，依「**修類別不修個案**」同批處理。
    〔量〕該 panic 的 errno 是 `code: 11, WouldBlock` ＝ `EAGAIN`。

*   **S5 ——— 讀不出來不得等於沒有。**
    〔`REAL_02` §5.1.1，v0.44.0 新設〕`oo lint` 今天在無法讀入的檔案上印
    `PARSE-SKIP` 之後仍以 `diagnostics: 0` ＋ rc=0 收尾
    〔量，陽性對照：同一個檔無限制時 `diagnostics: 1`〕。
    **一道 CI 閘會在一個沒讀進來的檔上變綠。**
    `oo test` 今天已經是 rc=1，**它是對的，照著它做。**

---

## 3. 明確不做

*   **不改兩道閘的門檻**（文字巢深 `PARSER_NESTING_LIMIT = 256`、
    AST 高度 `PARSER_AST_DEPTH_LIMIT = 4096`）。§2.7.4 宣告的
    **保證最小巢狀深度 256** 是規格承諾，不得下降；上調也不在本弧。
*   **不改求值期的 `#stack_overflow`**。它是值的載體、是對的，探針 G4 釘著它。
*   **不動 `#max_depth_exceeded`／`#blur` 那條政策線**。
*   **不碰 SIGPIPE**（`oo log | awk '…exit'` 那一列）。D63 明文把它排在
    CLI 指令的調整之後，錨點為 `commit.md` §4.4 弧 D 第 2 項。**同族但不同弧。**
*   **不做長期的剖析器替換**（卡片自己寫的）。
*   **不動 `#request_too_large`** 與 `REAL_02` §3.2.3 的位元組上限那一款。
*   **不新增 `.oo/` 下的任何耐久檔**（收弧協議 §8 3c 的代價）。

---

## 4. 紅線

*   **身分不動**：`x: 0` 的根 `31745ef0…`／物件數 **3**／標準根 `7038e250…`／
    `layout=5`／`encoding=5`。**新增 cause 變體不得改變任何既有值的位元組。**
*   **既有兩支探針一字不得改**：
    `crates/oo/tests/a_limit_you_cannot_catch_probe_test.rs` 與
    `crates/oo/tests/limit_you_cannot_choose_probe_test.rs`。
    **⚠ 它們會因為本弧而變紅**——`a_limit_you_cannot_catch` §452 逐字寫著
    「exit 0 and `⊥ #stack_overflow`」，而 S2 要求 rc≠0。
    **這是本弧唯一允許動既有探針的地方，而且不得自行決定**：
    請在 §N.4 逐支列出「哪一條斷言與 S2 衝突、你認為該改成什麼」，
    **由驗收方裁決後才改**。擅自改綠視同交付失敗。
*   **本弧探針只能動 `#[ignore]` 那一行**，其餘一字不動。
*   **`rustfmt` 不得掃過探針檔。**
*   **跨平台**：`crates/oo/src/main.rs` 與 `crates/parser/src/lib.rs` 的新增碼
    **必須帶 `#[cfg(unix)]` 或本身跨平台**。v0.45.0 的修補回合成因就是
    六處無守衛的 Unix-only 碼。**驗證方式是編出來**：
    `cargo check --target x86_64-pc-windows-gnu`，**請在 §N.6 貼出前後數字**。

---

## 5. 探針（驗收方所寫，基線 4 綠 6 紅）

| | 判 | 釘什麼 |
| :-- | :-- | :-- |
| **G1** | 綠 | known-answer：`eval '1 + 2'` → `3`、rc=0 |
| **G2** | 綠 | **`⊥` 是一個值**：`eval '1 & 2'` → `_\|_ #conflict`、**rc=0 不得改** |
| **G3** | 綠 | 普通語法錯誤已經是做對的邊界（rc≠0、有診斷、無節點級 `_\|_`）——**目標形狀** |
| **G4** | 綠 | **紅線**：求值期 `#stack_overflow` 仍是工作集裡的 `_\|_`、rc=0 |
| **R1** | 紅 | S2：剖析期柵欄不得鑄節點級 `_\|_`，且 rc≠0 |
| **R2** | 紅 | S1：宿主拒絕給執行緒時不得答 `#stack_overflow`，且 rc≠0 |
| **R3** | 紅 | S2：被拒絕的 `evolve` 不得 rc=0（工作集裡什麼都沒有） |
| **R4** | 紅 | S5：讀不到的檔不得以 `diagnostics: 0` ＋ rc=0 收尾 |
| **R5** | 紅 | S1：受限的節點不得把自己的無能為力說成對端請求太深 |
| **R6** | 紅 | S4：入口不得以宿主 panic 作答 |

**每一支紅都先斷言自己「到達了」再斷言性質**，未到達者以 **VOID READING** 失敗、
**永不通過**。R2–R6 各自**搜尋**自己的資源帶（帶寬因機器而異，探針不釘那個數字）。

**⚠ 探針的 helper 刻意把 stdout 與 stderr 分開**——與鄰近幾支探針不同。
本弧的主題就是「引擎跟操作者說了什麼」，而一段落到 stderr 的診斷與一顆印在
stdout 的 `_|_` **不是同一件事**。（佇列 Inbox 有一列活的帳正是關於 panic 文字
混進被斷言的輸出。）

### 5.1 ⚠ 條件 ③（剖析器 panic）**逐字無探針**

**沒有可攜的辦法從行程外面讓剖析器 panic。** 不要把這句話讀成「已覆蓋」。
改以 **§N.4 兩題必答**（見下）。**驗收方到不了它，所以你必須把它帶回來。**

---

## 6. 建議的落點（**參考，不是射程**）

*   `crates/parser/src/lib.rs`：把 `ParserNestingLimitExceeded` 拆開。
    **`catch_unwind` 不需要**——`join()` 的 `Err` 已經帶著 panic payload。
*   `is_parser_nesting_limit_error` 今天是唯一出口：**`main.rs` 6 處 ＋
    `oodp.rs` 2 處 ＋ `lib.rs` 3 處**引用。
*   `crates/oo/src/main.rs`：5 個 `print_parser_stack_overflow()` 呼叫點
    （`evolve` :492／`repl` :1340／`run` :1509／`fmt` :1542／`eval` :1572），
    另 `oo test`、`oo lint` 走 Display 字串。
*   `crates/interpreter/src/oodp.rs`：`:203` 與 `:357`。
*   `value.rs`（`to_nlang`／`as_bytes`／`as_str`）＋ `store_codec.rs:1582/1600`
    的解碼字串——**新增變體不得改動既有變體的位元組**。

---

## 7. 請在交付報告裡回答（§N.4）

1.  **③ 的逐字紀錄。** 請以一個**暫時的**補丁讓剖析器 panic，跑一次 CLI，
    貼出 **stdout／stderr／離開碼三者分開**的逐字結果，並確認補丁已還原、
    `git status` 乾淨。（驗收方偵察時這樣做過：補丁前的對照組 `1 + 2` → `3` 先過。）
2.  **③ 的常設看守。** `with_parser_stack_using` 是私有的，`crates/oo/tests/`
    到不了。請在 **`crates/parser`** 內加一支單元測試，以一個 **會 panic 的
    closure** 呼叫它，斷言得到的錯誤**不是**深度錯誤。
    （該檔 `:1292` 已有 `parser_thread_spawn_failure_is_a_typed_fence_error`
    以 `stack_size(usize::MAX)` 逼出 spawn 失敗——**同一個位置，第二個孿生**。）
3.  **②③ 的名字你取什麼、為什麼。** D62 只裁「要分」，沒裁拼法。
    請說明它們落在 `TAG_REGISTRY` 的哪一條軸與哪一個載體。
4.  **既有兩支探針的衝突清單。** 見 §4 紅線第二項，逐條列出，**不要自己改**。
5.  **①（真陽性）改成 rc≠0 之後，誰會看見？** 全樹搜尋有哪些測試、腳本或
    文件假設了「柵欄 ⟹ rc=0」。這是 D63 明文記帳的行為變更。
6.  **`oo test` 為什麼今天就是對的？** 它是七個剖析面裡唯一 rc≠0 的。
    請說明它走的是哪一條路徑——如果那條路徑可以直接推廣，本弧會便宜很多。
7.  **`oodp` 那一格：受限的節點應該回什麼？** 依 `REAL_02` §3.2.1，
    `%status`／`%reason` 是線上載體。請說明你選的 `%status` 為何，
    以及它**不入完整性紀錄**這件事是否仍然成立。

---

## 8. 驗收會怎麼做

1.  **diff 純度**：只有 §N 那一節與實作檔變動；探針除 `#[ignore]` 外一字未動。
2.  **探針完整性**：六支紅轉綠，四支綠仍綠，**且每一支都不是靠 VOID 路徑通過**。
3.  **全樹 ×3**（`--release --no-fail-fast`、**逐 target 聚合**、留失敗測試名）。
4.  **身分紅線**：以**真二進位**問一個已知答案的問題並附對照組，再驗五項紅線。
5.  **跨版本**：真 `v0.44.0` 讀本版寫的倉。
6.  **交叉編譯對照**：`x86_64-pc-windows-gnu` 前後數字。
7.  **符合性**：預期 **0 新向量**（語料形式表達不了宿主資源壓力），**逐字記帳**；
    **162 對 162 不得下降**。

---

## N. 交付回報（交付方填；本行以上一字不得動）

### N.1 射程逐項對照

**S1。** `ParserNestingLimitExceeded` 仍只由兩道閘鑄造（文字巢深 256、AST 高度 4096），Display 仍是 `#stack_overflow`。`spawn_scoped` 失敗改鑄 `ParserHostResourceDenied`（`#host_resource_denied`）；`join()` 的 `Err` 改鑄 `ParserInternalError`（`#internal_error`）。三者可分，後兩者都不叫 `#stack_overflow`。判準：本弧 R2、parser 單元測試 `parser_thread_spawn_failure_is_a_typed_fence_error`／`parser_thread_panic_is_not_a_depth_error`。未把新名加進 `BottomCause`（既有值位元組不動）。

**S2。** 刪掉 `print_parser_stack_overflow` 與五處「鑄節點級 `_|_` 然後 `Ok(())`」。剖析期 ①②③ 全部走與普通語法錯誤同一條路：診斷 + `Err` ⟹ rc≠0，stdout 不印 `_|_`。`oo test` 本來就是這條路，本弧把它推廣到 `evolve`／`eval`／`run`／`fmt`／`repl`。判準：R1、R3、G3。

**S3。** 求值期 `#stack_overflow` 仍是工作集裡的 `_|_`、`status` rc=0（G4）。剖析期現在是 stderr `Error: Parse error: #stack_overflow`、rc=1。兩個載體可從形與離開碼判別，不必靠標籤名反推。

**S4。** `main` 的 `oo-main` 執行緒不再 `.expect("spawn oo-main thread")`。宿主拒絕時 `anyhow::bail!("#host_resource_denied")`（不洩 `Os` Debug）。判準：R6。跨平台：`std::thread::Builder::spawn`，無 `cfg(unix)`。

**S5。** `oo lint` 的 `PARSE-SKIP` 計入 summary 的 diagnostics／error，且 `run_cli` 在任一 `parse_error` 上回 rc=1。判準：R4。`oo test` 未改。

### N.2 順手改動（逐項指名）

*   `rustfmt crates/parser/src/lib.rs`（本弧改的檔）。
*   `rustfmt crates/interpreter/src/oodp.rs` 掃到無關函式（`serve_discover` 折行、`FailingEntropy::fill` 折行），**已 `checkout` 撤回**，只重套本弧三處。
*   `rustfmt` 曾碰 `crates/parser/src/ast.rs` 一處 `Address { … }` 折行，**已撤回**。
*   **未 rustfmt** 探針、`main.rs`、`nlint.rs`、`storage.rs`。
*   無新 crate、無新 `.oo/` 檔、未動 `Cargo.lock`。

### N.3 工單哪裡是錯的

*   本弧探針**沒有** `#[ignore]`。基線即以 4 綠 6 紅在跑。N.5  accordingly 一字未動。
*   「`is_parser_nesting_limit_error`：`main.rs` 6 處」今日是 **5** 個 match 臂（evolve／repl／run／fmt／eval）+ 1 個 import；`oodp.rs` 是 parse 1 處 + serve 轉碼 1 處。定位對，計數略高。
*   §7.1 的對照組「補丁後 `1 + 2` → `3` 先過」與「在 `parse_expr` 開頭 `panic!`」不能同一次建置並存——那個開頭對任何輸入都炸。對照組改在**補丁還原後**以同一顆 release 二進位量（known-answer `3`）。③ 的逐字紀錄取自補丁建置的 `eval 'zzpanicz'`。

### N.4 工單指名要你回答的問題

**1. ③ 的逐字紀錄。** 暫時在 `parse_expr` 開頭插入 `panic!("q002: injected internal parser bug")`，`cargo build --release -p oo`。補丁已還原，產品樹無該行。

補丁建置（stdout／stderr／離開碼分開；離開碼直接取）：

```
$ oo eval 'zzpanicz'
stdout: （空）
stderr:
thread '<unnamed>' (620565) panicked at crates/parser/src/lib.rs:237:5:
q002: injected internal parser bug
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
Error: Parse error: #internal_error
rc: 1
```

不是 `#stack_overflow`，不是節點級 `_|_`，不是 rc=0。stderr 仍有宿主預設 panic hook 的原始碼路徑——`join()` 已經把 payload 收成 `#internal_error`，hook 是過程全域的，本弧沒有用 `catch_unwind` 去關它（工單說不需要）。補丁還原後 known-answer `eval '~%Math./add (1,2)'` → `3` rc=0。

**2. ③ 的常設看守。** `crates/parser/src/lib.rs` 的 `nesting_gate_tests::parser_thread_panic_is_not_a_depth_error`：以會 panic 的 closure 呼叫 `with_parser_stack_using`，斷言 `ParserThreadFailure::Internal`、`to_string() == "#internal_error"`、且 `!is_parser_nesting_limit_error`。與既有 `parser_thread_spawn_failure_is_a_typed_fence_error` 孿生（後者改斷言 `HostResourceDenied`，不再等於 `ParserNestingLimitExceeded`）。

**3. ②③ 的名字。**

| 條件 | 拼法 | 軸 | 載體 | 誰決定 | 操作者能做什麼 |
| :-- | :-- | :-- | :-- | :-- | :-- |
| ① | `#stack_overflow`（不動） | 原因 | 剖析期**邊界**／求值期 **⊥** | 實作的遞迴天花板 | 攤平／換實作；調旋鈕沒有用 |
| ② | `#host_resource_denied` | 原因 | CLI **邊界**；線上 `%reason`（`%status` 仍 `#rejected`） | **宿主** | 調高行程資源上限 |
| ③ | `#internal_error` | 原因 | 同上 | **實作自己的 bug** | 回報；操作者做不了任何事 |

取這兩個拼法是因為 D62 已經把它們寫成候選，且與 `#stack_overflow` 在「誰決定」那一軸上可分——併回去會重演 §2.7.2。不是 `BottomCause` 新變體。

**4. 既有兩支探針的衝突清單（未改）。**

`crates/oo/tests/a_limit_you_cannot_catch_probe_test.rs`（量，release，12 綠 **3 紅**）：

*   **`r2_a_deep_nest_is_a_clean_bottom_not_a_crash`**（約 L465）：`assert_eq!(code, Some(0))`，註解逐字「exit 0 and `⊥ #stack_overflow`」。今日輸出 `Error: Parse Error: #stack_overflow`、rc=1。與 S2 衝突的是 **離開碼與載體**（不再是值），不是標籤。建議：rc≠0、輸出含 `#stack_overflow`、不含 `_|_`、不含 `overflowed its stack`。
*   **`assert_deep_input_is_survivable`**（L588–601），**`r5_a_run_of_bangs_does_not_kill_the_parser`** 與 **`r6_a_deep_ast_does_not_kill_the_formatter`** 共用：把 `Some(0)` 當成「活下來」。今日活下來的形是 rc=1 + `#stack_overflow`。建議：信號／native abort 才算死；rc=1 且診斷含 `#stack_overflow` 算活。
*   其餘（r1 24 層、r3 位元組上限、r4 線上 DoS、r7 承諾 256 層）仍綠。

`crates/oo/tests/limit_you_cannot_choose_probe_test.rs`：**無衝突。** 其 R2／C1 走的是求值期天花板（1000 項鏈 < AST 4096）。量：10／10 仍綠。

**5. ① 改成 rc≠0 之後誰會看見。** 全樹（測試，不含規格歷史敘事）：

*   上一則三支斷言（已量紅）。
*   沒有 conformance 向量寫 `#stack_overflow` 或 400 層括號（語料 grep 0；符合性 162／162）。
*   腳本／CI 若把 `oo eval`／`oo fmt`／`oo evolve` 的剖析柵欄當成功（rc=0）會破。這是 D63 明文要記帳的行為變更。
*   求值期路徑（G4、`limit_you_cannot_choose` R2）看不見。

**6. `oo test` 為什麼今天就是對的。** `run_test` 對 `parse_program` 的 `Err` **沒有**特殊臂：一律 `FAIL: … (Parse error: {e})`、`failed += 1`、最後 `failed > 0` ⟹ `exit(1)`。柵欄錯誤的 Display 是 `#stack_overflow`，所以字面是 `Parse error: #stack_overflow`、rc=1——邊界載體該有的兩樣都有。本弧把這條路推到其他入口，而不是另做一套。

**7. `oodp` 那一格。** 受限節點（②）回 `%status: #rejected`、`%reason: #host_resource_denied`。狀態集仍是那五個（MUST NOT 增長）；可分性住在 `%reason`。選 `#rejected` 是因為它已經有「收方做不到」的先例（`#entropy_unavailable`），說的是**我不作答**，不是「你的請求太深」。依 REAL_02 §3.2.2，`#rejected` ⟹ 客戶端 `#peer_refused`，**不入完整性紀錄**——尚未依內容裁決，這一點仍成立。真陽性 ① 仍是 `#rejected` / `#stack_overflow`（形狀，不是宿主）。③ 若落到線上：`#rejected` / `#internal_error`，同樣不入完整性紀錄。

### N.5 探針

本弧探針沒有 `#[ignore]`，**一字未動**（含 rustfmt）。隔離 `--test-threads=1`：G1–G4 綠，R1–R6 綠，10／10。無 VOID READING。

### N.6 數字

基線 known-answer（本樹 release `oo`）：`eval '~%Math./add (1,2)'` → `3` rc=0；對照 `eval '1 & 2'` → `_|_ (%cause: #conflict)` rc=0。離開碼直接取。

身分紅線（`x: 0` 提交後）：根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`；物件 **3**；標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`；`layout=5`；`encoding=5`。

跨版本：真 `v0.44.0` 與真 `v0.45.0` 讀本版寫的倉，`log`／`status` 皆 rc=0。

交叉編譯 `cargo check --target x86_64-pc-windows-gnu --offline -p oo -p nlang-parser -p nlang-interpreter`：前 `Finished`／0 error／`nlang-interpreter` 16 warnings（9.16s）；後 `Finished`／0 error／同 16 warnings（8.26s）。產品碼對 `unix`／`libc::`／`AsRawFd`／`flock` 命中 0。

符合性：`python3 nlang-spec/scripts/run-conformance.py --engine …/target/release/oo` → **162 vectors, 162 pass, 0 fail**。0 新向量（語料表達不了宿主資源壓力）。

全樹 `cargo test --workspace --release --no-fail-fast --jobs 1 -- --test-threads=1`（逐 `test result:` 聚合）：

| 輪 | targets | passed | failed | 失敗測試名 | `^error` | cargo exit |
| :-- | --: | --: | --: | :-- | --: | --: |
| 1 | 226 | 2160 | 4 | 三支 catch ＋ Q-040 R4 flake | 3 | 101 |
| 2 | 226 | 2161 | 3 | 僅三支 catch | 2 | 101 |
| 3 | 226 | 2161 | 3 | 僅三支 catch | 2 | 101 |

三輪皆有、且本弧不得改者（`a_limit_you_cannot_catch_probe_test`，N.4.4）：

*   `r2_a_deep_nest_is_a_clean_bottom_not_a_crash`
*   `r5_a_run_of_bangs_does_not_kill_the_parser`
*   `r6_a_deep_ast_does_not_kill_the_formatter`

第 1 輪多一個非本弧 flake：`r4_two_concurrent_discharges_both_survive`（`an_authority_you_could_delete_probe_test`，Q-040；隔離 10 輪 8／10，REACH 1≠2）。第 2、3 輪未再出現。

`^error` 皆 cargo 的 `error: test failed`／`error: N targets failed:`，不是 rustc `error`。

本弧探針 10／10。parser `nesting_gate_tests` 7／7。`limit_you_cannot_choose_probe_test` 10／10。

### N.7 你認為需要改規格之處

**先回報再動。**

*   `TAG_REGISTRY` 應收 `#host_resource_denied`、`#internal_error`（原因·邊界；線上亦可為 `%reason`）。D62 已裁要分，簿上還是 0 命中。
*   §2.7.4「載體隨階段而異，標籤不變」對 ① 仍真；對 ②③ 標籤必須變。收尾時請不要把 ②③ 寫回 `#stack_overflow`。
*   REAL_02 §3.2.1 共用理由表可加 `#rejected` + `#host_resource_denied`（收方做不到，比照 `#entropy_unavailable`）。**不入完整性紀錄**這句請一起寫清。

