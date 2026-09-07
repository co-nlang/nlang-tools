# Q-002 偵察 — 一道不是你撞上的天花板

> **驗收方自量。** 基線 `v0.45.0` 標籤二進位
> （`/home/gali/nlang-baselines/v0.45.0-verify/target/release/oo`），
> known-answer 已過（`eval '1 + 2'` → `3`；對照 `eval '1 & 2'` → `_|_ (%cause: #conflict)`）。
> 佇列卡 Q-002 的證據取自 **`v0.18.0`**，27 個 minor 版本之前，故本文全部數字重量。

---

## 0. 一句話

**`oo eval '1 + 2'` 在一個 300 MB 位址空間上限下回答
`_|_ (%cause: #stack_overflow)`，離開碼 0。**

而 `TAG_REGISTRY` §2.7.3 逐字告訴操作者，收到這個標籤時該做的是
「攤平結構，或換一個能走更深的實作。**調旋鈕沒有用**」。

**輸入是 `1 + 2`。攤平它不會有任何效果。**

---

## 1. 卡片說的三件事，量到的是四件，而且分界跟卡片畫的不一樣

卡片的完成條件寫「分開 spawn 失敗、join panic 與真正資源界」。
量測之後，那三者**不在同一條路徑上**：

```
parse_expr_only / parse_program
 ├─ parser_nesting_gate(input)        ← 文字巢深閘，上限 256，**在執行緒之外**
 │    └─ 撞到 ⟹ ParserNestingLimitExceeded  ← 真正的 #stack_overflow ✅
 ├─ with_parser_stack(...)            ← crates/parser/src/lib.rs:1173
 │    ├─ builder.spawn_scoped(...).map_err(|_| ParserNestingLimitExceeded)?   ← :1182
 │    └─ parser.join().map_err(|_| ParserNestingLimitExceeded)                ← :1184
 └─ parser_ast_gate_*(...)            ← AST 高度閘，上限 4096，**在執行緒之後**
      └─ 撞到 ⟹ ParserNestingLimitExceeded  ← 真正的 #stack_overflow ✅
```

**「真正資源界」根本不在 join 上。** 它由兩道閘處理，兩道閘都在執行緒之外，
而且兩道今天都答得對（§2 第 1、2 列）。

⟹ **`join()` 的 `Err` 分支只有一個居民：panic。**
`std::thread::JoinHandle::join` 回 `Err` 的充要條件就是該執行緒 panic。
**那一格裡沒有任何一個合法的 `#stack_overflow`。**

而 `spawn_scoped` 的 `Err` 分支也只有一個居民：**宿主拒絕建立執行緒**
（記憶體或執行緒數；量到的 errno 是 `EAGAIN`／`WouldBlock`）。**那也不是深度。**

---

## 2. 量測

### 2.1 位址空間帶（`oo eval '1 + 2'`，離開碼直接取，不經管線）

| `ulimit -v` | rc | 輸出 | 判 |
| :-- | :-- | :-- | :-- |
| 1000000 KB | 0 | `3` | ✅ 正確 |
| 800000 | 0 | `3` | ✅ |
| 700000 | 0 | `3` | ✅ |
| **650000** | **0** | **`_\|_ (%cause: #stack_overflow)`** | ❌ **假話** |
| 620000 / 600000 / 300000 / 150000 | 0 | 同上 | ❌ |
| 100000 / 80000 | **134** | `memory allocation of 7 bytes failed`（SIGABRT） | ❌ 裸宿主中止 |
| 60000 / 40000 | **101** | `thread 'main' panicked at crates/oo/src/main.rs:385:10:`<br>`spawn oo-main thread: Os { code: 11, kind: WouldBlock, … }` | ❌ **完全沒有 n/ 答案** |

**假話帶約 500 MB 寬**（~150 MB–650 MB）。成因：`PARSER_STACK_BYTES = 512 MiB`
（`lib.rs:1163`）的保留失敗，`oo-main` 的 64 MiB 仍成功。

〔對照〕**同一顆二進位、同一個表達式、無限制 ⟹ `3`。**
量測確實碰到了目標，且目標之外的一切不變。

### 2.2 真深度輸入（必須保持正確，本弧不得動）

| 輸入 | rc | 輸出 |
| :-- | :-- | :-- |
| 400 層括號 | 0 | `_\|_ (%cause: #stack_overflow)` ✅ |
| `1 + 1 + …`（1000 項） | 0 | `#blur { %cause: #max_depth_exceeded, … }` ✅ 政策界 |
| 同上 4000 項 | 0 | 同上 ✅ |
| 同上 **4100** 項 | 0 | `_\|_ (%cause: #stack_overflow)` ✅ AST 閘 4096 |
| 同上 6000／8000／20000 項 | 0 | 同上 ✅ **無 SIGSEGV** |

⟹ **兩道閘今天是有效的**，`#stack_overflow` 的真陽性完好。
本弧要拆的不是閘，是**閘以外那兩格的冒名**。

### 2.3 內部 panic 長什麼樣（打補丁量，補丁已還原，樹乾淨）

在 `parse_expr` 開頭注入一個 `panic!`，`cargo build --release -p oo`，
對照組（`eval '1 + 2'` → `3`）先過：

```
$ oo-panicprobe eval 'zzpanicz'
stdout: _|_ (%cause: #stack_overflow)
rc:     0
stderr:
  thread '<unnamed>' (2724029) panicked at crates/parser/src/lib.rs:174:9:
  recon: injected internal parser bug
```

**三件事同時發生**：
1. 操作者拿到的答案是 `#stack_overflow`——一個**指向錯誤補救**的名字；
2. **rc = 0**，腳本看見成功；
3. stderr 逐字洩漏**宿主原始碼路徑與行號**（`crates/parser/src/lib.rs:174:9`）
   ——`SPEC_10` §2.2.1 禁止洩漏實作表示，Q-016b 的 `os error 2` 是同一條的另一個實例。

### 2.4 七個剖析面，五個回報 rc=0（`ulimit -v 300000`）

| 指令 | rc | 輸出 |
| :-- | :-- | :-- |
| `oo eval '1 + 2'` | 0 | `_\|_ (%cause: #stack_overflow)` |
| `oo evolve k.n` | 0 | 同上；**且 `status` 隨後為 `Universe is static`——什麼都沒進工作集** |
| `oo run -o a k.n` | 0 | 同上 |
| `oo fmt k.n` | 0 | 同上 |
| `oo fmt k.n --write` | 0 | 同上；**檔案未被改寫**（已 `diff` 確認） |
| `oo test k.n` | **1** | `FAIL: "k.n" (Parse error: #stack_overflow)` ← 唯一非零 |
| `oo lint k.n` | 0 | `PARSE-SKIP` ＋ `diagnostics: 0 (0 error, 0 warn, 0 info)` |

**`oo evolve` 是其中最貴的一格**：它回報 rc=0，而**演化沒有發生**。
操作者的腳本看見成功並繼續；後續 `commit` 才說 `Nothing to commit`（rc=1）。

**`oo lint` 有陽性對照**〔量〕：`tests/tmp_node_a.n` 無限制時
`diagnostics: 1 (0 error, 1 warn, 0 info)`；**同一個檔在壓力下 `diagnostics: 0`、rc=0**。
⟹ **一道 CI lint 閘會在一個它根本沒讀進來的檔案上變綠。**

### 2.5 而引擎自己的慣例是離開碼 1

| 同一個 `oo evolve` 入口 | rc |
| :-- | :-- |
| 普通語法錯誤（`a: (((`） | **1**，逐字 `Error: Parse Error in "b.n": …` |
| 剖析柵欄（真深度、假 `#stack_overflow` 皆同） | **0** |

⟹ **柵欄路徑是這個入口上唯一一種離開碼為 0 的剖析失敗。**

### 2.6 線上：一台停擺的節點會把責任推給對端

`oodp.rs:203` 把柵欄錯誤轉成 `"#stack_overflow"`，`:357` 再轉成
`(OodpStatus::Rejected, "stack_overflow")`。

同一個 1 行 `#fetch` 請求，只改**伺服器行程**的位址空間上限：

| 伺服器 `ulimit -v` | 回應 |
| :-- | :-- |
| 1200000 KB | `{"%status":"#not_found","%reason":"#not_held", …}` ✅ |
| 700000 | 同上 ✅ |
| **500000** | `{"%status":"#rejected","%reason":"#stack_overflow", …}` ❌ |
| **300000** | 同上 ❌ |

**這一格違反的是 REAL_02 §3.2.3 自己寫下的理由。**
該節「拒絕不得以沉默表達（MUST NOT）」的論證逐字是：

> 否則**發送方無法分辨「我被拒絕了」與「這台掛了」**。

今天發送方收到一個具名拒絕，於是**判定為前者**——而真相是後者。
**具名了，而名字把它放進錯的那一桶。** 沉默被禁掉了，冒名沒有。

且依 §3.2.3＋`TAG_REGISTRY` §1.3，這一類拒絕**依大小／形狀而非內容**，
故**不入完整性紀錄**。⟹ **一台已經不再服務的節點，不留任何痕跡。**

---

## 3. 這是誰的鍋

| 面 | 判 |
| :-- | :-- |
| **join panic ⟹ `#stack_overflow`** | **引擎的鍋。** `lib.rs:1184` 一行；規格從未說 panic 是深度 |
| **stderr 洩漏宿主路徑** | **引擎的鍋。** `SPEC_10` §2.2.1 已管著 |
| **rc=0** | **引擎的鍋。** 同入口的普通剖析錯誤是 rc=1（§2.5） |
| **spawn 失敗 ⟹ `#stack_overflow`** | **一半是規格的鍋。** 見下 |
| **`oo lint` 在讀不到的檔上回 0 診斷** | **引擎的鍋**，但屬 §2.4 那一族（讀不出來不得等於沒有，`REAL_02` §5.1.1，v0.44.0 新設） |
| **「宿主拒絕給資源」沒有名字** | **規格的鍋。** 登記簿全文 grep `internal_error`／`host_resource`：**0 命中** |

### 3.1 spawn 失敗這一格為什麼難

`TAG_REGISTRY` §2.7.4 最後一條 MUST 逐字：

> **上限與其所保護的資源是同一個決定（MUST）**：實作必須將其堆疊配置到
> 足以在其最壞語法形式下清空自己宣告的上限。

那 512 MiB 的保留**就是**這條 MUST 的兌現機制。保留不到時，實作**確實**
到不了它宣告的 256 層。⟹ **「我到不了那裡」這句話字面為真。**

**但補救是假的。** 登記簿把 `#stack_overflow` 的操作者動作寫死為
「攤平結構／換實作／**調旋鈕沒有用**」，而這一格唯一有效的動作是
**調高行程的資源上限**——一個旋鈕。

⟹ 這正是 §2.7.1／§2.7.2／§2.7.3 三處反覆論證的那個病：
**把一個可行的補救換成一個無效的補救。**
**標籤可以為真而仍然是錯的答案，因為原因標籤是一句關於補救的話。**

---

## 4. 待裁的一題（⟹ 依 §8 開弧 3，今天只能開偵察或先取裁定）

卡片寫「依賴：無」。**那是錯的**——量測引出一個設計問題：

> **這三種條件各自叫什麼名字？**

| 條件 | 今天 | 操作者真正該做的 |
| :-- | :-- | :-- |
| ① 形狀超過柵欄 | `#stack_overflow` rc=0 | 攤平／換實作 |
| ② 宿主拒絕給資源（EAGAIN／RLIMIT） | `#stack_overflow` rc=0 | **調高行程上限** |
| ③ 剖析器 panic（實作 bug） | `#stack_overflow` rc=0 ＋ 裸 backtrace | **回報 bug**；操作者做不了任何事 |

**候選**

*   **甲（兩個新名）**：② → `#host_resource_denied`（邊界）／③ → `#internal_error`（邊界）。
    登記簿新增兩列。**分得最乾淨，三種補救三個名字。**
*   **乙（一個新名）**：②③ 併為 `#internal_error`（「這不是你的問題」），
    以訊息區分。登記簿新增一列。**便宜，但把「你調得動」與「你調不動」併了**
    ——正是 §2.7.3 那張表所拆開的那一刀。
*   **丙（不新增標籤）**：只改離開碼與訊息，`%cause` 仍為 `#stack_overflow`。
    **不推薦**：它讓登記簿繼續說一句假話，而 §2.7.3 的四條 MUST 全是關於名字的。

**驗收方推薦甲。** 理由不是完備性，是 §2.7.3 那張表**已經**用「誰決定」
把 `#max_depth_exceeded` 與 `#stack_overflow` 分開了；②③ 在同一根軸上是
**兩個不同的「誰」**（宿主／實作自己的錯誤），把它們併起來會在下一次
重演 §2.7.2 那個更正。

**第二題（可與第一題同時裁）**：**離開碼**。
§2.5 量到柵欄路徑是該入口唯一 rc=0 的剖析失敗。②③ 應否 rc≠0？
（驗收方意見：**應**——`oo evolve` 回報成功而演化沒有發生，是 Q-017 同一族。
但 ① 的 rc 是否一併改動，是相容性問題，須明說。）

---

## 5. 若裁定落地，射程與價（驗收方預估，交付方須自行複價）

*   `crates/parser/src/lib.rs`：把 `ParserNestingLimitExceeded` 拆成三個
    （或兩個＋一個 panic 載體），`with_parser_stack_using` 的兩個 `map_err` 各自改。
    **`catch_unwind` 不需要**——`join()` 的 `Err` 已經帶著 panic payload。
*   **判別函式**：`is_parser_nesting_limit_error` 今天是唯一出口，
    **6 處 `main.rs` ＋ 2 處 `oodp.rs` ＋ 3 處 `lib.rs` 引用**。
*   `crates/oo/src/main.rs`：5 個 `print_parser_stack_overflow()` 呼叫點
    （`evolve` :492／`repl` :1340／`run` :1509／`fmt` :1542／`eval` :1572），
    另 `oo test`、`oo lint` 走 Display 字串。
*   `crates/interpreter/src/oodp.rs`：`:203` 與 `:357`。
*   `main.rs:385` 的 `.expect("spawn oo-main thread")` 是同一族的第四格
    （§2.1 最後兩列，rc=101、無 n/ 答案）——**建議同批**，理由是
    「修類別不修個案」；若排除須明文。
*   **新增 `BottomCause`／邊界載體變體**：`value.rs`（`to_nlang`／`as_bytes`／
    `as_str`）＋ `store_codec.rs:1582/1600` 的解碼字串。
    **不動既有值的位元組** ⟹ **不是紀元、不動身分**。

**紅線（依既往）**：根 `31745ef0…`／3 物件／標準根 `7038e250…`／
`layout=5`／`encoding=5` 皆不動。

**符合性**：語料形式表達不了「宿主拒絕給執行緒」與「剖析器有 bug」，
**預期 0 向量，逐字記帳**（比照 Q-038）。真陽性的 ① 已有向量
（`a_limit_you_cannot_catch_probe_test.rs`／`limit_you_cannot_choose_probe_test.rs`），
**兩者皆為本弧紅線，不得改**。

---

## 6. 本偵察順手發現、已入 Inbox 不在本弧

*   `oo lint` 在 `PARSE-SKIP` 之後仍印 `diagnostics: 0` 與空的 ω(G) 摘要，
    **摘要不說分母**（幾個檔被跳過）。與「任何聚合都要印出分母」同族。
*   `ulimit -v` 100000 帶的 `memory allocation of N bytes failed` + SIGABRT：
    Rust 配置失敗中止，**不是本弧的 join／spawn**，但同樣是裸宿主訊息。
