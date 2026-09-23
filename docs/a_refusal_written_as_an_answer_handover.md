# 工單：一次拒絕，被寫成了一個答案（Q-055）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-055／**不需新裁定**（成員全部違反已寫下的 MUST；
> 三態那一條沿用 D73 的形）。
> 探針（已預先提交並校準）`crates/oo/tests/a_refusal_written_as_an_answer_probe_test.rs`
> 基線：dev `b87f226`／`oo v0.57.0` ⟹ **5 綠 9 紅**，三輪一致、零空洞讀數。

## 1. 缺陷

有些指令做不成自己的工作時，會在一個回報面上說出來、在另一個回報面上否認，
或者乾脆說成另一件事。〔量，`v0.57.0` 標籤建置，known-answer `add(1,2)=3`／`add(1,3)=4` 已過；
離開碼一律取自 `oo` 自己的行程，不經管線〕

| # | 指令與情境 | 今天的回答 | 違反 |
| :-- | :-- | :-- | :-- |
| M1 | `oo status`，工作集的注入檔損壞 | `Universe unavailable: expected value at line 1 column 1`，**rc=0**（同一狀態下 `commit` rc=1） | `SPEC_10` §2.2.1 各回報面一致 |
| M2 | `oo inspect <確實存在的 commit>`，儲存**存在但拒絕開啟**（`objects.format` 為 `encoding=99`，或 D73：節點金鑰存在但讀不到） | **`CAID not found in local store`** | `REAL_03` §6.6 三種結果可分；D73 |
| M3 | `oo eval '_{sha256:<根>}.x'`，同上兩種拒絕 | **`_\|_ ;; %cause: #missing_key`，rc=0**——與一個**哪裡都不存在的位址**逐字同形（對照：健康的倉回 `1`） | 同上；拒絕變成了一個值 |
| M4 | `oo eval '~%Math./add (1,2)'`，D73 拒絕 | `3`，rc=0——**而 `run`／`status`／`log` 都遵守 D73 而拒絕** | D73 |
| M5 | `oo migrate --grant migrate`，layout 2 → 5 | 代價句印在 `migrate_layout(&cur)?` **之後**；只點名 `oo v0.41.0`，〔量，真二進位〕**實際鎖在門外的是 v0.40.0／v0.41.0／v0.42.0／v0.43.0**，v0.44.0 打得開 | `REAL_02` §5.1.1「動手前說出代價」「哪些引擎此後將打不開」 |
| M6 | `oo migrate --grant migrate`，**儲存已是最新** | `Migrated store layout to layout=5. …`，rc=0，**兩份宣告逐位元組未變** | §2.2.1（宣稱了一件沒發生的事） |
| M7 | `.oo` 唯讀時 `oo migrate --grant migrate` | 只有宿主錯誤 `atomic_write temp create …: permission denied`——**動手之前代價一個字都沒說** | §5.1.1 |
| M8 | M1 的 `status`／`commit` 文字；D73 本身的拒絕文字 | `expected value at line 1 column 1`（serde_json 原文）；`… (KeyRejected("InvalidEncoding"))`（加密庫錯誤的 Rust `Debug`） | `REAL_01` §1.3 (i) 不得洩漏實作表示 |

**M2–M4 同一行**（`main.rs:1595` `run_eval`、`:1688` `run_inspect`）：

```rust
let engine = Ouroboros::init(&cur).unwrap_or_else(|_| Ouroboros::new_in_memory());
```

它吞掉**每一種** `init` 失敗，換上一個空的記憶體儲存。自 2026-05-24（`befde50`，第一個 CLI）起就在，
比「三態」這條規則早四個月。

**⚠ M8 的第二半是驗收方的漏網**：Q-054 受理時，D73 的拒絕文字就含 `KeyRejected(…)`，驗收方沒看見。

## 2. 那個 fallback 唯一正當的用途（不得被修掉）

〔量〕在唯讀、且沒有 `.oo` 的目錄裡，`oo eval '~%Math./add (1,2)'` → `3`，rc=0，且沒有建出 `.oo`
——`init` 失敗於「建不出儲存」，fallback 讓一個純表達式照樣可算。**這是對的**：那裡**沒有**儲存，
換上空的不會讀錯任何東西。探針 `g2` 釘住它。

⟹ 判準就是 D73 的三態，只是對象從金鑰換成儲存：**不存在的可以代換；存在但打不開的不可以。**

## 3. 射程＝不變式（不是機制）

*   **I1** 一個指令**做不成自己的工作**時，每一個回報面（文字、離開碼、落地的狀態）都說失敗；
    一個指令**沒有改變任何東西**時，不得宣稱改變了。
*   **I2** **存在但打不開的儲存不是不存在**：任何指令都不得以空的（或記憶體中的）儲存代換一個
    存在而打不開的儲存，也不得把那個拒絕回答成「找不到」或 `#missing_key`。
    **D73 的拒絕在每一條路徑上都成立**——`eval`／`inspect` 不得繞過 `run`／`status`／`log` 已遵守的那一個。
*   **I3** **代價在動手之前**：`migrate` 在改動任何一個宣告位元組之前，就把「此後哪些引擎打不開這個儲存」
    完整地說出來；動作失敗時，代價仍然已經在畫面上。
    **形式你選**（先印再做／預覽模式／把代價放進未授權時的拒絕裡……），探針只看**失敗時畫面上有沒有代價**
    與**名單完不完整**（`v0.43.0` 或 `v0.44.0` 那一條邊界，任一寫法皆可）。
*   **I4** 拒絕用引擎的話說，不用宿主的：本弧碰到的出口，不得出現 serde／加密庫的原文或 Rust `Debug`。

## 4. 紅線（探針已釘，今天綠，修復不得拿它們換 I1–I4）

*   **c1** 位址字面真的碰得到儲存（健康倉回 `1`；不存在的位址回 `#missing_key`）
    ——r3／r4 的紅**因此**不能是位址字面本身壞了。
*   **g1** 缺席仍是缺席：健康倉裡 `inspect` 一個不存在的 CAID，仍說 `not found`，rc≠0。
*   **g2** 沒有儲存、也建不出儲存時，純表達式仍可算（§2）。
*   **g3** `migrate` 沒有 grant 仍拒絕且不動宣告；有 grant 仍把 layout 推到 5。
*   **k1** 每一支紅探針的判準，都已由一個**今天就正確**的真實輸出滿足過（`status` 對拒絕的儲存、
    `run` 在 D73 下）——所以紅是引擎的紅，不是判準不可滿足。
*   **身分紅線**：本弧不得碰 `bn_serial`／任何位址。全樹 ×3 不得有任何 digest 移動。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1**（請在交付報告裡回答）列出 `crates/oo/src/main.rs` 裡**所有**「印出失敗文字、卻不改離開碼」的出口，
    以及所有吞掉 `init` 失敗的點。**是 M1–M8 這幾個點，還是一族？** 驗收方讀到但**沒有探針**的三處：
    (a) `Error: Commit failed`（`universe.rs:1113`／`1121`）沒有座標也沒有錯誤碼——**它今天走得到嗎？**
    驗收方沒構造出來；(b) `oo log` 重讀 commit 失敗時 `eprintln!` 然後 rc=0（`main.rs:1029`）；
    (c) `oo node discover` 以 `let _ =` 丟掉 `record_peer_advert` 的回報行，而 `peers::append`
    打不開或寫不進目錄時**靜默返回**——畫面說收了這個同儕，磁碟上沒有。
    **三處你修或不修都要說理由**；修了的，補一支探針並在報告裡寫出它的基線紅。
*   **Q2**（請在交付報告裡回答） I2 你怎麼區分「不存在」與「存在但打不開」？`init` 今天在唯讀目錄失敗的那條路，
    和它在 `encoding=99` 失敗的那條路，**型別上分得開嗎**？
*   **Q3**（請在交付報告裡回答） I3 你選了什麼形式，為什麼？名單從哪裡來（引擎怎麼知道 v0.43.0 是最後一個打不開的）？
    **已量的邊界**：layout 2 → 5 之後 v0.40.0–v0.43.0 拒絕、v0.44.0 開得起來。
    若儲存原本是 layout 3 或 4，名單應該不同——**你的做法對那兩種起點說什麼？**
*   **Q4**（請在交付報告裡回答） `migrate_layout` 以**兩次獨立的 `atomic_write`** 寫 `format` 與 `objects.format`
    （`storage.rs:683`／`692`）——兩次之間當掉，宣告就是半新半舊。這與 Q-045 修掉的初始化窗口同形。
    **不在本弧射程**（I1–I4 都不是它），只要你**量它是否可觀測**並回報。

## 6. 明文不在射程內（沉默不得被當成覆蓋）

*   **網路指令把同儕的拒絕當成答案印出來、rc=0**（`node advertise` 的 `#rejected …`、
    `node discover` 的 `#not_implemented`、`node find-node` 的 `#oversize`）——
    **那是別人的拒絕，不是本指令做不成**；答案本身是不是一個拒絕、要不要進離開碼，是 **O84**，未裁。
*   **`print_integrity_incidents`** 在值正確時仍於 stderr 報完整性事件而 rc=0——那是對第三方的報告，
    值本身是對的（peer-fetch 弧的受理修補，理由記在 `main.rs:1636` 的註解）。不動。
*   **唯讀命令偷偷建 `.oo/`**（Inbox，弧 D-2 已解散後無家）——本弧不改「要不要建」，
    只要求「建不成／打不開」時不說謊。
*   **`REAL_01` §1.3 (i) 的全面清掃**（Inbox：儲存層 25 個 IO 點、兩個設定載入器）不在本弧；
    I4 只治**本弧碰到的出口**。
*   `oo --version` 在 `dev` 建置自報 `v0.57.0-881-g…`——另一列，不在本弧。

## 7. 探針完整性（本探針沒有 `#[ignore]`：紅就是紅，不必拿掉任何一行）

*   紅探針 r1–r8b **基線必須是紅的**，理由逐支寫在各自的 doc comment。
    **你可以讓它們變綠，不得改寫它們的宣稱。** 若你認為某個 pin 是錯的，**在報告裡說，不要改它**。
*   每一次故障注入都先驗證碰到了目標（`status` 必須拒絕、`commit` 必須拒絕、唯讀寫入必須失敗），
    否則探針 panic 並印 `VOID READING`——**那不是失敗也不是通過，回報它**。
    r6 以唯讀目錄讓寫入失敗；**以 root 身分跑會作廢**（權限不擋 root）。
*   `rustfmt` **不得**掃這個檔案。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照
S1 I1：`status` 讀不到工作集時改為 `bail`（文字仍是 `Universe unavailable:`，離開碼不再是 0）。已是最新的 `migrate` 在寫任何位元組之前回「`Store layout is already layout=5. Nothing was changed.`」，不含 `Migrated store layout to`。驗：r1、r5。
S2 I2：`eval`／`inspect` 不再 `unwrap_or_else(|_| new_in_memory())`。只有 `durable_store_present` 為假才換記憶體儲存；`encoding=99` 與 D73 把 `init` 的錯誤原樣交回。驗：r2、r3、r4；g2 仍是 `3` rc=0 且沒有建出 `.oo`。
S3 I3：代價句先印、`stdout` 先 flush，然後才 `migrate_layout`。寫入失敗時代價已在畫面上。成功之後才印 `Migrated store layout to layout=5.`。layout=2 的名單是 `oo v0.40.0 through v0.43.0`。驗：r6、r7；g3 無 grant 仍 `#privileged_required` 且宣告不動，有 grant 仍到 layout=5。
S4 I4：注入檔的 serde 失敗改成 `injection <檔名>: working set cannot be read`。D73 不再把加密庫的 `Debug` 插進句子，留下 `not a valid PKCS#8 Ed25519 key; file left unchanged`。驗：r8a、r8b。

### 8.2 順手改動（逐項指名）
無。`cargo fmt` 未跑。改動檔：`crates/oo/src/main.rs`、`crates/interpreter/src/storage.rs`（只加 `durable_store_present`）、`crates/interpreter/src/injections.rs`、`crates/interpreter/src/value.rs`。

### 8.3 工單哪裡是錯的
`nothing_here_or_nothing_you_can_see` 的 g3 把 `status` rc=0 當成「這個倉打得開」。那倉的唯一注入是 `{}`，本來就讀不成工作集；rc=0 就是 M1。本弧之後 `status` rc=1，文句是 `Universe unavailable: injection deadbeef: working set cannot be read`。另量：`format` 仍是 `layout=4`，`objects.format` 仍是 `encoding=4`，沒有被打開時推進。沒有改那支探針。
`migrate_layout` 的兩次 `atomic_write` 在本交付原始碼是 `storage.rs:728` 與 `:737`（工單寫 683／692，是加了 `durable_store_present` 之前的行號）。兩次寫入本身沒有改。

### 8.4 工單指名要你回答的問題
Q1. `main.rs` 裡「印出失敗、離開碼仍是成功」的出口，以及吞掉 `init` 失敗的點：
- 吞 `init`：只有 `run_eval` 與 `run_inspect` 兩處 `unwrap_or_else(|_| new_in_memory())`。不是更大的一族。這兩處已改。其餘 `Ouroboros::init(...)?` 本來就拒絕。
- `run_status` 的 `Universe unavailable` 然後 `return Ok(())`：M1，已改成 `bail`。
- `run_log` 重讀 commit 失敗時 `eprintln!` 然後繼續，函式最後 `Ok`（約 `main.rs:1029`）。`log()` 對鏈上每一顆已經 `?` 過 `get_commit`，所以一份持久讀不開的 commit 會先讓整條 `log` rc≠0。這行是同一次行程裡的第二次讀。不修：造得出來的只有兩次讀之間的競態，驗收方沒有搭，我也沒有搭，因此沒有補探針。
- `run_refine` 讀回 commit 失敗時 `eprintln!` 然後 `Ok`（約 `main.rs:1326`）。同一形：refine 已經落地，失敗的是影子報告的第二次讀。不在點名的三處裡。不修，理由與 `log` 相同。
- `oo node discover` 的 `#not_implemented`、`oo node find-node` 的 `#oversize`、`oo node advertise` 印出對端 `%status` 之後 `return Ok(())`：工單 §6／O84，別人的拒絕。不修。
- `print_integrity_incidents`：工單 §6。不修。
- REPL 印 `Parse Error` 之後繼續；離開碼是整個會話的，不是那一行的。不修。
- `run_fmt` 對 stdout 的 `writeln` 用 `let _ =` 丟掉寫入錯誤。不在 M1–M8。不修。
- (a) `Error: Commit failed`（`universe.rs` 兩處）今天走不到。`pin_commit_merge` 回 `None` 只在「沒被 pin 的那一半與根 meet 不成 Combo」；非 pin 那臂是 `unify` 不是 Combo。本二進位上：普通衝突在 `evolve` 就 `#conflict at x`／`at y`，接著 `commit` 是 `Nothing to commit`；兩份不同座標的 `--pin` 都保住 `pin_coords`，`commit` rc=0；一份 `--pin` 裡同時改一個衝突欄位，兩欄都算 pin，`commit` rc=0；`~%Config.fuel: 7` 演化成功、`commit` 是 `Nothing to commit`。畫面上沒有出現 `Commit failed`。不改那句話，不補探針。舊的成因（`pin_pending` 只留一個座標）不是 layout=5 現在寫的東西。
- (b) 見上面 `run_log`。不修。
- (c) `oo node discover` 以 `let _ =` 丟掉 `record_peer_advert` 的回報；`peers::append` 在目錄打不開或標頭寫不進時回空的 log。畫面可以說收了這個同儕，目錄裡沒有那一行。不修：要一則 discover 回覆才走得到，而 `append` 也是 serve 的落地路徑。沒有補探針。O84 是線上那句別人的拒絕；這是本地寫入沒落地。沒有基線紅可以出示，所以不動。

Q2. 兩條路今天都是 `anyhow::Error`，型別上分不開。唯讀且沒有 `.oo`：`ObjectStore::init` 在 `create_dir_all` 失敗（`cannot write`）。`encoding=99`：`ensure_supported_encoding` 失敗（`refusing to open`）。沒有依字串分支。`durable_store_present` 是 `init` 裡 `new_store` 的補集：有 `format`、或有 `HEAD`、或有任一 CAS 物件；`.oo` 無法 stat 算存在。不存在才換記憶體；存在就把 `init` 的錯誤交回。D73 發生在儲存已經存在之後，所以 `eval`／`inspect` 拒絕。g2 沒有 `.oo`，fallback 還在。

Q3. 形式是先說代價、再寫。已經是 layout=5 則在任何寫入之前說明沒有改，並且不說 `Migrated`。名單來自各標籤原始碼的 `STORE_LAYOUT_VERSION`／`STORE_LAYOUT_MIGRATABLE_FROM`，不是當場去跑舊二進位：v0.40.0 與 v0.41.0 只開 layout=2；v0.42.0 開 2..=3；v0.43.0 開 2..=4；v0.44.0 是第一個開 layout=5 的標籤。所以起點不同，被鎖在門外的人不同——layout=2 是 `oo v0.40.0 through v0.43.0`；layout=3 是 `oo v0.42.0 through v0.43.0`；layout=4 是 `oo v0.43.0`。裸數字宣告 v0.40.0–v0.43.0 仍開得了，名單與 layout=2 相同。本二進位實測這三句彼此不同，且都含 `v0.43.0` 或 `v0.44.0`。

Q4. 可觀測性取決於第二次寫會不會改位元組。沒有在行程中途殺掉程序；量的是兩次 `rename` 之間會留下的檔案狀態。
- 本引擎新建的倉 `objects.format` 已是 `encoding=5`（v0.40.0 起新倉就寫 5）。第二次寫入再寫一次 `encoding=5`。只做第一次之後，兩份宣告已經是完成態。這個窗口在宣告位元組上看不見。`status` rc=0。
- `encoding>=4` 且還不是 5：先放 `layout=4`／`encoding=4`（`status` rc=0），再只做第一次（`layout=5`／`encoding=4`）。`status` rc=0。位元組與完成態（`encoding=5`）不同，看得到。本引擎接受 encoding 4，所以半新半舊不是拒絕。
- `encoding<4`：第二次寫入保留舊編碼，第一次之後就已經等於完成態。看不見落差。另測：把本引擎以 encoding 5 寫下的物件改宣告成 `encoding=2`，`status` rc=1 `#caid_mismatch`。那是宣告與位元組不符，不是這個當機窗口。

### 8.5 探針
沒有 `#[ignore]` 可拿。本弧探針檔未改、未 rustfmt。無 `VOID READING`。14 支皆綠（c1 g1 g2 g3 k1 r1 r2 r3 r4 r5 r6 r7 r8a r8b）。

### 8.6 數字
全跑三輪相同：`cargo test --workspace --release --no-fail-fast --jobs 1 -- --test-threads=1`。逐 `test result:` 聚合：240 個 target，2275 passed，1 failed，ignored 0。cargo exit 101。以 `^error` 起首的行 2 行，都是 cargo 包裝那一個失敗 target。失敗測試名：`g3_a_declared_store_without_history_is_not_a_new_store`。
conformance：`python3 nlang-spec/scripts/run-conformance.py --engine nlang-tools/target/release/oo` → 162 vectors，162 pass，0 fail。
身分：`oo eval '~%Math./add (1,2)'` → `3` rc=0；`~%Math./add (1,3)` → `4` rc=0。`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`，物件 3，`layout=5` `encoding=5`。標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`（available）。未改 `bn_serial`。

### 8.7 你認為需要改規格之處
無。

---

## 9. 驗收（驗收方填）

**第一輪：不受理，開一個修補回合（R-1）。** I1–I4 在探針上全部兌現，但交付讓一個**原本做得到的動作做不到了**。
diff 純度：五檔（`main.rs`／`storage.rs`／`injections.rs`／`value.rs`／本檔報告），**探針一字未動**、
未動 `bn_serial`、未 `rustfmt`；報告只動了分隔線以下。

### 9.1 R-1：「已是最新」只看了兩份宣告中的一份

`migrate` 推進**兩份**宣告（`storage.rs` `migrate_layout`：`format` 與 `objects.format`）。
交付的「已是最新」判斷是 `layout_declaration_is_current`，**只讀 layout**。
〔量，真建置，同一個倉兩個引擎〕`layout=5`／`encoding=4`——**正是交付自己在 Q4 量到的那個半途狀態**：

| 引擎 | `oo migrate --grant migrate` | 之後的宣告 |
| :-- | :-- | :-- |
| `v0.57.0` | `Migrated store layout to layout=5. …` rc=0 | `layout=5`／**`encoding=5`** |
| 交付 `e2831a3` | `Store layout is already layout=5. Nothing was changed.` rc=0 | `layout=5`／**`encoding=4`** |

那句話字面是真的（確實沒改），**但一個半途的倉從此沒有任何明示的動作能把它完成**——
`migrate` 是推進編碼軸唯一被明示要求的動作（`REAL_02` §5.1.1）。
**這是驗收方寫射程的方式造成的**：I1 寫的是「沒改就不說改了」，交付把它做得字面正確，
而「該做的還是要做」從旁邊漏掉——**射程寫了一半的不變式**。

**修補的不變式**：*`migrate` 在兩份宣告**都**已是本引擎所寫的那一份時，才可以說「沒有改變」；
任一份不是，它就做它在本弧之前做的事（推進到最新），並且代價照 I3 在動手之前說——
若沒有任何引擎因此被鎖在門外，就說沒有。* 探針 **r9**（驗收時新增）：v0.57.0 綠、交付紅。

### 9.2 驗收方的探針修補：一支舊探針的 REACH 一直騎在 M1 上

交付在 8.3 回報 `nothing_here_or_nothing_you_can_see::g3` 由綠轉紅，**沒有去動它——這是對的**。
〔讀〕它寫進去的注入是 `{}`，**不是任何 layout 的注入**（不是 layout 4 的框，也解不成舊式本體）
⟹ 工作集從來讀不成，而它的 REACH `rc == 0` 之所以綠，**正是因為 `status` 說了「Universe unavailable」
還回 0**——那就是本弧的 M1。本弧修好 M1，它的 REACH 就誠實地紅了，而它守的性質（不推進宣告）
交付另量仍成立。

**修補**（驗收方，只動夾具與 REACH，不動斷言）：注入改成一個真的 layout 4 框（無 `effect_tags:` 行），
REACH 另加「必須讀回 `x: 1`」，使它**不能再騎在一個說謊的 `status` 上**。
`init` 判斷新倉的輸入（`format`／`HEAD`／CAS 物件）不變 ⟹ 它能偵測的東西不變。
〔量〕修補後 **v0.57.0 綠、交付綠**（兩個引擎都讀回 `x: 1`、宣告都沒動）。

### 9.3 驗收方另加一支：r10

交付分辨「不存在」與「存在但打不開」的方法，是在 `init` **已經失敗之後**才問「儲存在不在」。
在一個還沒有儲存、而節點金鑰讀不到的目錄裡，這之所以對，**只因為 `init` 先寫下 `.oo/format`、
後讀節點金鑰**〔量：交付 rc=1 具名拒絕；v0.57.0 答 `3` rc=0〕。
那是一個順序，不是一個不變式 ⟹ **r10** 把觀測釘住（v0.57.0 紅、交付綠），
讓將來重排 `init` 不能悄悄把 r4 關上的路重新打開。**不要求改碼。**

### 9.4 已獨立複驗的

*   探針（交付樹＋驗收方修補）：本檔 16 支（原 14 ＋ r9 ＋ r10）⟹ **15 綠 1 紅**（r9），無 `VOID READING`；
    `nothing_here…` **7／7**。
*   全樹 ×1（`--release --no-fail-fast --jobs 1 -- --test-threads=1`，**修補前的交付樹**）：
    **240 個 target**（237 Running＋3 Doc-tests；`test result:` 行 245，多出的 5 行各為 0／0）、
    **2275 passed、1 failed**：`g3_a_declared_store_without_history_is_not_a_new_store`（即 9.2）、
    `^error` 2 行皆 cargo 包裝、cargo exit 101。**與交付 8.6 一致。**
*   conformance **162／162**。身分：`x: 0` → `31745ef0…`；`v: 1 + 1` 與 `v: 1+1` → `f4f32e7b…`；
    標準根 `7038e250…`（available）——**三者未動**。known-answer `3`／`4`。
*   D73 在「無儲存＋金鑰讀不到」下仍拒絕（9.3）。
*   **`oo --version` 對交付建置自報 `v0.57.0-881-gb87f226`**（`build.rs` 沒重跑）——
    Inbox 那一列又收了一次稅；驗收方以行為判準（`migrate` 對最新倉的回答）確認二進位是交付的。

### 9.5 交付的判斷，驗收方同意的

*   Q1：`Commit failed` 今天走不到（四種構造皆未出現）；`log`／`refine` 的第二次讀與 `discover` 的落地失敗
    不修——**理由成立**，後者開 Inbox（落地失敗被丟棄，不是 O84）。
*   Q2：兩種 `init` 失敗型別上分不開，交付沒有依字串分支——**對**。
*   Q3：名單取自各標籤原始碼的常數而非臨場跑舊二進位；三個起點三句不同——**與驗收方的真二進位量測一致**（layout 2）。
*   Q4 的量測正是 R-1 的來源；**R-1 修好之後，半途的倉可由再跑一次 `migrate` 完成**。

---

## 10. 修補回合 R-1 回報（交付方填；本行以上一字不得動）

### 10.1 射程逐項對照

### 10.2 順手改動（逐項指名）

### 10.3 工單哪裡是錯的

### 10.4 工單指名要你回答的問題
（R-1 無新題；若 9.1 的不變式你有不同讀法，寫在這裡。）

### 10.5 探針
本檔與 `nothing_here…` 皆由驗收方修補過（見 9.2／9.3）；**你不得動**。r9 應轉綠，其餘不得轉紅。

### 10.6 數字
全樹 ×3（`--release --no-fail-fast`、逐 target 聚合、**失敗測試名**、`^error` 行數、exit code）／conformance／身分紅線。

### 10.7 你認為需要改規格之處

---

## 11. 驗收 R-1（驗收方填）
