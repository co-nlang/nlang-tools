# 工單：互動式的 eval（Q-071）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-071／裁定 `meta/oo/STATUS.md` **D89**（甲）／同族 D88（一次性求值看所在宇宙）、D82（沒有宇宙）、D79（丟了的 context）
> 既有規格：`REAL_01` §1.1（D88 兩款）、§1.3、§1.4（F2）；`SPEC_11` §1.2（`~%Repl`：會話層級、不入歷史）
> 探針（已預先提交並校準）`crates/oo/tests/an_interactive_eval_probe_test.rs`
> 基線：dev `dbd7dc6`／`oo v0.72.0` ⟹ **4 綠 7 紅**，三輪一致；七支紅的都紅在一般斷言，零空洞讀數。

## 1. 缺陷

〔量，v0.40.0 … v0.72.0 每一個標籤建置〕`oo repl` 對任何合法欄位（`z: 1`、`z: 1 + 1`、`z: _.a`）**都沒有輸出**；只有剖析錯誤有輸出。
〔量，除錯建置〕裸鍵 `z` 剖析為 `FieldKey::Path(Bare, ["z"])`；`run_repl` 只在鍵為 `Named`／`Quoted` 時觀測並列印，其餘 `_ => continue` **靜默跳過**——每一行都被演化、沒有一行被觀測。
另：`repl` 今天要求宇宙（D82 歸為「需要宇宙」），並載入工作集。

## 2. 裁定與依據

*   **D89（用戶，甲）**：`repl` ＝**互動式的 `eval`**：起點是這個宇宙 `HEAD` 的根，工作集不算（D88 C2）；每一行只在這次會話的記憶體裡累積、**不注入不提交**（C4）；接受 `--universe`／`--ephemeral`（C3，與 `eval` 同一份說明）；沒有宇宙時以空根作答、不建宇宙（D82）；丟了 context 時具名拒絕（D79，同 `eval`）。
*   **用戶的補充**：其他模式（例如把會話注入工作集）的家是 `~%Repl`（`SPEC_11` §1.2），**不是 `repl` 的預設**——本弧不做。

## 3. 射程＝不變式（不是機制）

*   **I1** **每一行都被觀測並印出它的值**，不論欄位鍵在剖析器裡是哪一種形（r1）。**修類別不修個案**：凡是「演化成功卻什麼都不印」的路徑都是本條的違反——請在 Q1 列出你檢查過的每一種鍵形。
*   **I2** **起點是 `HEAD` 的根**（r2）；**工作集不算**（g1）；**會話內的行彼此看得見**（r3）。
*   **I3** **不注入、不提交**：會話結束後 `HEAD` 與工作集逐位元組不變（g2）；明示 `~%Engine./save` 只動物件儲存（g4）。
*   **I4** **沒有宇宙時作答、不建**（r4）；`--universe`（r5）、`--ephemeral`（r6）與 `eval` 同義、同說明（r7）；丟了 context 具名拒絕、不寫（g3）。
*   **I5** **`repl` 的自述為真**：今天說「against this workspace」，D89 之後它讀的是 `HEAD` 的根、不寫工作區。
*   **I6** 不新增耐久檔、不改磁碟格式、不推進佈局；值位址與提交位址不動。

## 4. 紅線（今天綠，必須保持綠）

*   g1–g4；`where_the_conflict_is`（含 r3：`repl` 回報衝突時說出座標）、`a_history_older_than_its_savepoints`（丟了 context 時 `repl` 拒絕）、`where_there_is_no_universe`（含預先修訂）、`the_universe_you_are_in` 14、`one_writer_at_a_time` 10。
*   **交叉編譯** `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   conformance 162／162；`x: 0` 根 `31745ef0…`、`v: 1 + 1` 根 `f4f32e7b…`、標準根 `7038e250…`；新倉 `layout=8`／`encoding=5`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** I1：剖析器的每一種 `FieldKey`（`Named`／`Quoted`／`Pattern`／`Path` 及 `Path` 的各種錨與段數）在 `repl` 裡各答什麼；哪些鍵形「觀測什麼」沒有自然答案，你怎麼答（不得靜默）。
*   **Q2** 一行裡有兩個欄位（如 `a: 1 b: 2`）時印什麼；一行與前面的行衝突時印什麼、會話是否繼續。
*   **Q3** `repl` 改後的自述（逐字）。
*   **Q4** 事實陳述：`repl` 裡呼叫 `~%Repl./…`（任何一個）今天答什麼。**不要實作 `~%Repl`**。

## 6. 明文不在射程內

*   `~%Repl` 家族（用戶補充：其他模式的家，本弧不做）。
*   `repl` 的行編輯、歷史、多行輸入。
*   規格文字（`REAL_01` §1.1 補 `repl`、`REAL_02` §5.1.1 的 D82 清單把 `repl` 移到「不需要宇宙」）：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r7 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。**
*   輸入經 stdin 餵入、以 `exit` 結束；「印出 v」＝輸出中有 `oo eval` 對該值會印的那段文字，值取 42／4242 以免撞到橫幅。`.oo/` 逐位元組比對（`savepoints/` 除外——`SPEC_10` §3.1 的觀測缺口不是本弧的事）。`rustfmt` 不得掃探針檔。
*   〔驗收方校準〕**兩極皆做**：
    基線 4 綠 7 紅（三輪一致）。**初版有三處錯，已修**：r3、r6 在基線上觸發了空洞讀數守衛（它們本該紅在一般斷言）；r8（明示 `save`）在基線上是綠的——基線演化那一行時 `save` 已經執行——改為守衛 g4。
    一個最小參考實作（`run_repl` 改走 Q-070 的 `select_universe`／`engine_for_one_shot`／`one_shot_view`；鍵為 `Path` 時觀測那條路徑；`Repl` 加兩個選擇器）⟹ 本探針 11／11，交叉編譯 0 error。
    **四個變異各自讓一支守衛轉紅**：載入工作集 ⟹ g1；會話往工作集寫東西 ⟹ g2；跳過 lost-context 檢查 ⟹ g3；一律用臨時宇宙（`save` 寫不進）⟹ g4。
    **預先修訂 `where_there_is_no_universe_probe_test`**（D82）：`repl` 由「需要宇宙」移到「不需要宇宙」清單——r2 不再要求它拒絕，r1 仍要求它不寫任何東西；新行為（作答）由本探針 r4 量。標 `AMENDED 2026-10-04 for Q-071 (D89)`；**兩邊驗證**：v0.72.0 基線 13／13、參考實作 13／13。**你不得再改它。**
    同一參考實作跑全樹（含上述預先修訂）：**256 target／2466 passed／0 failed，exit 0**（＝ v0.72.0 的 255／2455 加上本探針 11 支）。
    **參考實作只補了 `Path` 這一種鍵形——那是個案修法，不要照抄**（I1、Q1）。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

1. **I1** 一行演化成功之後一定印出答案。有座標的鍵印 `=>` 與其觀測值；沒有座標的鍵印 `no coordinate to observe:` 與鍵的寫法。剖析錯誤印 `Parse Error:`。r1。各種鍵形見 Q1。
2. **I2** 有宇宙時 `repl` 走 `Universe::load`（`HEAD` 的根，不載工作集）。r2、g1。同一會話裡後面的行看得到前面的行。r3。
3. **I3** 會話只改記憶體裡的宇宙，不注入、不提交。g2。明示 `~%Engine./save` 只動物件儲存。g4。
4. **I4** 沒有宇宙時以空根作答，目錄裡不出現 `.oo/`。r4。`--universe`、`--ephemeral` 與 `eval` 同一條路、同一句說明。r5、r6、r7。丟了 context 時在印橫幅之前具名拒絕，輸出含 `rollback`，不寫。g3。
5. **I5** 自述改為讀這個宇宙 `HEAD` 的根、會話內累積、不注入不提交。逐字見 Q3。
6. **I6** 沒有新耐久檔，佈局與編碼仍是新倉的 `layout=8`／`encoding=5`。三個根位址與工單相同。版本仍是 `oo v0.72.0`。

### 8.2 順手改動（逐項指名）

只動 `crates/oo/src/main.rs`。

1. `Repl` 加上與 `eval` 相同的 `--universe <DIR>`、`--ephemeral`。兩個旗標同時出現時，沿用既有句子：`--ephemeral and --universe name two universes; pass one`。說明裡沒有 clap 的衝突註記。
2. `run_repl` 改走 `select_universe`、`engine_for_one_shot`、`one_shot_view`。不再 `require_universe`，不再 `load_universe`。
3. 演化成功後的印出改為窮盡 `FieldKey`：有座標則 `observe`，展開（`"..."`）則印新出現的座標，其餘印 `no coordinate to observe:`。衝突仍印 `Evolution Conflict:`，會話繼續。

探針、規格、夾具、`Cargo.lock`、版本都未改。`where_there_is_no_universe_probe_test.rs` 上驗收方已寫的 `AMENDED 2026-10-04 for Q-071 (D89)` 未再改。

### 8.3 工單哪裡是錯的

無。工單寫的全樹是 256 target、2466 passed、0 failed。這次三輪的 `test result:` 與此相同。

### 8.4 工單指名要你回答的問題

**Q1** 剖析器的 `field_key` 把 `path` 排在 `named_key` 前面。下列形狀是對這支剖析器逐條量的。

有座標、印 `=>` 與該座標的觀測值：

| 輸入 | 鍵形 | 這一行的答案 |
| :-- | :-- | :-- |
| `z: 40 + 2` | `Path` Bare 一段 | `=> 42` |
| `"z": 40 + 2`、`"""z""": 40 + 2` | `Quoted` | `=> 42` |
| `#t: 40 + 2`、`#_|_: 40 + 2`、`#_: 40 + 2` | `Path` Bare 一段（`tag` 規則排在 `path` 之後，到不了 `Pattern`） | `=> 42` |
| `1: 40 + 2` | `Path` Bare 一段，段為 `1` | `=> 42` |
| `_: 40 + 2` | `Path` Bare 一段，段為 `_` | `=> 42`。`resolve_path` 把裸的 `_`、`_|_`、`#_|_`、`#_` 當字面，所以這四個拼法改從根 `_.…` 讀回剛寫下的欄位 |
| `~secret: 40 + 2`、`/f: 40 + 2`、`@T: 40 + 2`、`%m: 40 + 2`、`~@Schema: 40 + 2`、`~/helper: 40 + 2` | `Path` Bare 一段，前綴在段文字裡 | `=> 42`，下一行用同一條路徑讀回也是 `42` |
| `~%Config.fuel: 50` | `Path` Bare 兩段，且是 `~%Config.<knob>` | `=> 50` |

`Named`（含 `Data`／`Private`／`Logic`／`Type`／`Meta`／`System`／`Local` 與無前綴）這次沒有任何使用者輸入能產生它。臂仍在：若出現，觀測的是它存進去的那條拼法（`/name`、`@name`、`%name`、`~%name`、`~name`，其餘用名字本身）。

沒有自然的單一座標，不靜默：

| 輸入 | 鍵形 | 答案 |
| :-- | :-- | :-- |
| `@{1}: 40 + 2` | `Pattern` | `no coordinate to observe: @{1}`。演化接受這一行，值不落在任何座標 |
| `@{}: 1` | 剖析失敗 | `Parse Error: Empty anon_set` |
| `a.b: 40 + 2`、`a.b.c: …` | `Path` Bare 兩段以上，且不是 `~%Config.<knob>` | `no coordinate to observe: a.b` |
| `_.a: 40 + 2`、`_.a.b: …`、`_.: 40 + 2` | `Path` Root（段數 1、2、0） | `no coordinate to observe: _.a`／`_.` |
| `...{ k: 40 + 2 }` | `Quoted("...")`，展開 | 每個新座標各一行 `=>`。這裡 `k` 是 `=> 42` |
| 已有 `k: 1` 再 `...{ k: 1 }` | 同上，沒有新座標 | `no coordinate to observe: "..."` |
| `~%Sys: 40 + 2` | `Path` Bare，段以 `~%` 起頭 | `Evolution Conflict: #system_reserved at ~%Sys`。這是拒絕，不是靜默 |
| `^.a: 1`、`^.: 1`、`_|_: 1`、`_{sha256:<64 hex>}.a: 1` | 不是合法的欄位鍵（`Current`／`Parent`／`Address` 不在 `field_key`） | `Parse Error:`，會話繼續 |

**Q2** 一行兩個欄位，每個欄位各答一次，然後才出現下一個提示。沒有宇宙的目錄裡 `m: 1 n: 2` 印：

```
=> 1
=> 2
```

rc 0。與前面的行衝突時印 `Evolution Conflict: #conflict at <座標>`，不印該欄位的值，會話繼續，後面的行照答。量過：`p: 1`、`p: 2`、`q: 3` 依序是 `=> 1`、`Evolution Conflict: #conflict at p`、`=> 3`。同一行 `m: 1 m: 2 n: 3` 是 `=> 1`、衝突、`=> 3`。已提交 `a: 40` 時，`a: 1 b: 2` 先報 `a` 的衝突，再印 `=> 2`。

**Q3** `oo repl --help` 的自述，逐字：

```
Read-eval-print loop from this universe's committed root. Each line is printed and kept in this session only; it is not staged or committed. The working set does not count. Where there is no universe, the root is empty. An explicit ~%Engine./save writes this universe's object store; with no universe it answers #no_universe
```

`--universe <DIR>` 與 `--ephemeral` 的說明與 `eval --help` 相同：`Use the universe in this directory instead of the one where this command is run`、`Evaluate against an anonymous temporary universe and leave this universe untouched`。

**Q4** 未實作 `~%Repl`。在有宇宙與沒有宇宙兩處，`repl` 裡下列四行都是 rc 0、`=> _|_  ;; %cause: #missing_key`：`~%Repl`、`~%Repl./commit { message: "m" }`、`~%Repl./start`、`~%Repl./nope`。`oo eval '~%Repl./commit { message: "m" }'`、`oo eval '~%Repl'`、`oo eval '~%Repl./start'` 同一句，rc 0。

### 8.5 探針

`crates/oo/tests/an_interactive_eval_probe_test.rs` 未改，未 rustfmt。全樹第三輪 11／11，0.82s。

`crates/oo/tests/where_there_is_no_universe_probe_test.rs` 未改。全樹第三輪 13／13，3.72s。

紅線，全樹第三輪皆 `test result: ok`、0 failed：`where_the_conflict_is` 9（0.60s，含 r3）、`a_history_older_than_its_savepoints` 13（0.97s）、`the_universe_you_are_in` 14（1.24s）、`one_writer_at_a_time` 10（17.79s）。

交叉編譯 `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu`：`Finished` 1.49s，`^error` 0。

### 8.6 數字

1. 三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`：每輪 rc=0，`^error` 0，`test result:` 256 行，2466 passed，0 failed。`running`／`test `／`test result:` 三輪共 2978 行，去掉耗時後相同。編譯警告在前言裡的先後順序三輪不同，所以整份日誌的 `cmp` 不相同。
2. conformance 162／162，rc=0。cwd 是 `/home/gali/nlang`。
3. `~%Math./add (1, 2)` → 3，`(1, 3)` → 4，rc=0。這兩次 `eval` 沒有建立 `.oo/`。euid 1000。
4. `x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。`v: 1 + 1` 根 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`。新倉 `.oo/format` 為 `layout=8`，`.oo/objects.format` 為 `encoding=5`。

### 8.7 你認為需要改規格之處

規格檔這次沒有改。建議驗收方把 Q3 的自述寫進 `REAL_01` §1.1，並在 `REAL_02` §5.1.1 的 D82 清單把 `repl` 放到不需要宇宙的一側。`~%Repl` 仍無實作，本弧沒有加。

---

## 9. 驗收（驗收方填）
