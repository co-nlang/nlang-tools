# 工單：你所在的宇宙（Q-070）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-070／裁定 `meta/oo/STATUS.md` **D88**（＝O40；`meta/oo/cli_surface.md` §6 C1–C4）
> 既有規格：`SYNTAX_03` §47／§143（「`_.` 從**這個宇宙的** root 絕對起算」）；`REAL_01` §1.3／§1.4（F1 每個旗標有說明；F2 一個概念一個拼法一個簽名一份說明）；`SPEC_08`／`REAL_02` §5.1.1 的 D82（沒有宇宙：讀不寫）
> 探針（已預先提交並校準）`crates/oo/tests/the_universe_you_are_in_probe_test.rs`
> 基線：dev `c07dd6b`／`oo v0.71.0` ⟹ **4 綠 10 紅**，三輪一致；十支紅的都紅在預定斷言（r6／r8／r9 紅在「旗標不存在」），零空洞讀數。

## 1. 缺陷

〔量，v0.71.0，已提交 `a: 1` 的工作區〕`oo eval '_.a'` → `_`；`oo run` 觀測 `b: a + 1` 的 `b` → `_`；`test_a: _.a = 1` 失敗。一次性求值器跑在一個空白宇宙裡，**規格沒有替它們寫這個例外**（`SYNTAX_03` §143）。
另外：`REAL_01` §1.1 記載的 `--load`／`--commit` 從未存在；`run` 自述「does not write this workspace」，而它在宇宙裡經 `~%Engine./save` 會寫物件（Inbox 列）。

## 2. 裁定與依據

*   **D88（用戶，C1–C4 照建議，三點澄清照建議）**：
    *   **C1** 在一個宇宙裡，`eval`／`run`／`test` 以這個宇宙 `HEAD` 的根為起點；沒有宇宙時照 D82 為空。
    *   **C2** 工作集不算（看工作集的是 `status`）。
    *   **C3** `--universe <路徑>`：所有需要宇宙的指令與三個一次性求值器；`--ephemeral`（在宇宙裡也用匿名臨時宇宙）：只給三個一次性求值器。同一旗標同一簽名同一說明（F2）。
    *   **C4** `run` 把檔案注入這個宇宙的唯讀視圖並觀測——**不注入、不提交**。
    *   **澄清一**：「唯讀」＝不動 `HEAD`、不動工作集。明示的 `~%Engine./save` 是程式自己的效果，照既有契約（`cas_integrity` R-2）寫進它所在宇宙的物件儲存，`eval` 與 `run` 一致；`--ephemeral` 或沒有宇宙時答 ⊥ `#no_universe`（D82 ③）。⟹ `run` 的自述**改說明**。
    *   **澄清二**：`repl` 不在本弧（它從 v0.40.0 起對任何輸入都沒有輸出，另列 Inbox；且它讀整個工作區）。
    *   **澄清三**：`--ephemeral` 不給寫者。
*   C5：`evolve` 不改名（用戶，依 `SPEC_10` §2.2）。C6：不做 `--commit`／`--dry-run` 複合旗標（驗收方建議，未裁——**本弧不做**）。

## 3. 射程＝不變式（不是機制）

*   **I1** **`oo X --universe <p> …` ≡ 在 `<p>` 裡跑 `oo X …`**——輸出逐位元組相同；**檔案引數照呼叫者所在的位置解析**；呼叫者所在的位置不得出現 `.oo/`（r4、r5）。
*   **I2** **一次性求值器的 `_.` 是 `HEAD` 的根**，工作集不算（r1–r3、g1）。沒有宇宙 ⟹ 照 D82 為空、不建（g2）。**丟了的 context 不是空的宇宙**（D79）：`HEAD` 缺席而儲存宣告過提交 ⟹ 一次性求值器具名拒絕、不寫；與 context 無關的求值用 `--ephemeral`（r10）。
*   **I3** **`--universe` 指向一個沒有宇宙的目錄**：讀取類（一次性求值器、`status` 等）**具名拒絕、rc≠0**，那個目錄不得出現 `.oo/`（r6）。`evolve --universe <p>` 照 D82 在 `<p>` 建宇宙。
*   **I4** **`--ephemeral` ≡ 在一個沒有宇宙的地方跑**：輸出與之相同，所在宇宙的 `.oo/` 逐位元組不變；明示 `save` 答 ⊥ `#no_universe`（r7、r8）。
*   **I5** **一次性求值器不注入、不提交**：`HEAD`、注入、savepoints 逐位元組不變；明示 `save` 只動物件儲存（g3、g4）。
*   **I6** **F2**：`--universe` 在 13 個指令上同一份說明，`--ephemeral` 在 3 個一次性求值器上同一份說明，寫者不接受 `--ephemeral`（r9）。**每一個指令的自述為真**——`run` 不再說「不寫這個工作區」。
*   **I7** **選定的宇宙必須貫穿到每一個讀寫**：〔驗收方的參考實作量到〕`Ouroboros::log()`（`interpreter/src/lib.rs`）直接讀行程的 `current_dir()`，於是 `log --universe` 讀錯地方。**凡是讀行程工作目錄來決定宇宙的地方，都是這條不變式的違反**——請在 Q1 逐一列出。
*   **I8** 不新增耐久檔、不改磁碟格式、不推進佈局；值位址與提交位址不動。

## 4. 紅線（今天綠，必須保持綠）

*   g1–g4；`one_writer_at_a_time` 10、`already_in_head` 7、`where_there_is_no_universe` 13、`what_it_says_is_what_happened` 23、`cas_integrity`、Q-067 11、Q-066 10。
*   **交叉編譯** `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   conformance 162／162；`x: 0` 根 `31745ef0…`、`v: 1 + 1` 根 `f4f32e7b…`、標準根 `7038e250…`；新倉 `layout=8`／`encoding=5`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** I7：引擎裡所有以行程工作目錄（`current_dir`、`"."`）決定宇宙或儲存的地方，逐一列出（檔:行），以及你怎麼處理每一處。
*   **Q2** `--ephemeral` 的臨時宇宙住在哪裡、何時清掉；它與「沒有宇宙的地方」在 D82 之下是否完全同答（含 `node` 設定的讀取）。
*   **Q3** `run`／`eval`／`test` 改後的自述（逐字）。
*   **Q4** 事實陳述：`HEAD` 讀不到（權限）、或 lost context（D79）時，`eval '_.a'` 答什麼。
*   **Q5** 事實陳述：`node` 子指令與 `fmt`／`lint`／`identity` 今天以什麼決定「這個工作區」；**不要為它們加 `--universe`**（不在射程）。

## 6. 明文不在射程內

*   `repl`（澄清二）；`node`／`fmt`／`lint`／`identity` 的選擇器。
*   `--commit`／`--dry-run` 複合旗標（C6）；○ 的動詞（C7）；`init` 與取值原語（C8）。
*   `evolve` 改名（C5：不改）。
*   規格文字（`REAL_01` §1.1 改寫成實況、§1.4 收 D88、`SYNTAX_03` 若需註記）：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r9 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。**
*   探針只釘旗標名（裁定所選）與登記標籤 `#no_universe`；「相同」是輸出逐位元組相同；`.oo/` 逐位元組比對。`rustfmt` 不得掃探針檔。
*   〔驗收方校準〕**兩極皆做**：
    基線 4 綠 10 紅（三輪一致）。**r6 初版在基線是空綠**（旗標不存在 ⟹ clap 用法錯誤 ⟹ rc≠0、什麼都沒建），已補「選擇器先在真的宇宙上可用」的前提；**r9 初版把 clap 的欄位對齊當成說明不同**，已改為正規化空白後比較。
    一個最小參考實作（選擇器改寫 `cwd()` 的回傳；`--ephemeral` 指向一個新的空暫存目錄；三個一次性求值器以 `Universe::load`（不載工作集）起始；`Ouroboros::log()` 改用 `base_dir`）⟹ 本探針 14／14，交叉編譯 0 error。
    **四個變異各自讓一支守衛轉紅**：一次性求值器載入工作集 ⟹ g1；沒有宇宙處建 `.oo/` ⟹ g2；`run` 注入它的檔 ⟹ g3；`run` 一律用臨時宇宙（`save` 寫不進）⟹ g4。
    同一參考實作跑全樹：**255 target／2453 passed／1 failed**——`a_history_older_than_its_savepoints_probe_test::g5_context_free_commands_still_answer_on_a_lost_old_store`（Q-063）：它要求丟了 context 的舊倉上 `eval '~%Math./add (1, 2)'` 照答。**D88 之後 `eval` 從 `HEAD` 的根起算，就不再與 context 無關**；照 D79 它應具名拒絕（否則 `_.a` 會把丟掉的歷史讀成空的）。**這是 C1＋D79 的直接後果，驗收方已向用戶明說。**
    **驗收方預先修訂**：g5 改為「不答錯」——答出已知答案，或具名拒絕（指向 rollback），兩者皆可；新行為（一般 `eval` 拒絕、`eval --ephemeral` 照答）另入本探針 **r10**。標 `AMENDED 2026-10-03 for Q-070 (D88)`；**兩邊驗證**：v0.71.0 基線 13／13、參考實作 13／13。**你不得再改它。**
    **參考實作的臨時目錄從不清掉，不要照抄**（Q2）。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

1. **I1** `--universe <p>` 把儲存目錄換成 `<p>` 的絕對路徑。相對的檔案引數照行程目錄打開。r4、r5：呼叫者旁邊沒有 `.oo/`，`<p>` 裡隨後的 `eval _.c` 是 `3`。
2. **I2** 有宇宙時，`eval`／`run`／`test` 走 `Universe::load`（`HEAD` 的根）。r1–r3、g1。站在沒有宇宙的地方時，`without_universe` 加空白根，目錄裡不出現 `.oo/`。g2。`HEAD` 缺席而儲存宣告過提交時，一次性求值器具名拒絕，輸出含 `rollback`。r10。
3. **I3** `--universe` 指向沒有宇宙的目錄：讀取類在 `Ouroboros::init` 之前拒絕，句子是 `no universe here: start one with evolve`，該目錄不出現 `.oo/`。r6。`evolve --universe <p>` 仍在 `<p>` 建宇宙。
4. **I4** `--ephemeral` 把基底指到一個新的空暫存目錄，再 `without_universe`。與在沒有宇宙的地方跑，輸出逐位元組相同；所在宇宙的 `.oo/` 不變。明示 `save` 答 `#no_universe`，rc 0。r7、r8。
5. **I5** 三個一次性求值器不載工作集、不寫入注入、不提交。明示 `~%Engine./save` 只動物件儲存。g3、g4。
6. **I6** `--universe` 在 13 個指令上同一句說明；`--ephemeral` 只在 `eval`、`run`、`test`，同一句說明。`evolve`、`commit`、`rollback`、`squash`、`refine`、`gc`、`migrate` 的說明只有 `--universe`。r9。兩個旗標同時出現時，程式拒絕：`--ephemeral and --universe name two universes; pass one`。說明文字沒有 clap 的衝突註記，所以 13 句與 3 句仍然各自相同。
7. **I7** `Ouroboros::log` 收 `base_dir`。`run_log` 把 `cwd()` 的結果傳進去。其餘讀行程目錄的地方見 Q1。
8. **I8** 沒有新耐久檔，佈局與編碼仍是新倉的 `layout=8`／`encoding=5`。三個根位址與工單相同。版本仍是 `oo v0.71.0`。

### 8.2 順手改動（逐項指名）

1. `select_universe`／`cwd()`（`crates/oo/src/main.rs`）。`--universe` 與 `--ephemeral` 把執行緒局部的絕對路徑設為儲存目錄。相對的 `--universe` 用 `real_cwd()` 接上，保留呼叫者寫下的路徑，不做 `canonicalize`。
2. `engine_for_one_shot`／`one_shot_view`。有宇宙：`Ouroboros::init` 然後 `Universe::load`。`--ephemeral`，或人站在沒有宇宙的地方：`without_universe` 加空白根。點名一個沒有宇宙的目錄：在 `init` 之前拒絕。
3. 十三個指令先 `select_universe(..., false)`，再走原來的 `require_universe`、`CommitLock`、D86、lost-context 順序。
4. `run_test` 把 `SelectedUniverse` 留在手上，`process::exit(1)` 之前 `drop`。`exit` 不跑解構，暫存目錄要在那之前刪。
5. `Ouroboros::log(&self, base_dir)`（`crates/interpreter/src/lib.rs`）。

探針、規格、夾具、`Cargo.lock`、版本都未改。

### 8.3 工單哪裡是錯的

工單寫的全樹是參考實作、且 `g5` 仍失敗的那一次：255 target、2453 passed、1 failed。驗收方隨後把 `g5` 改成「已知答案，或指向 rollback 的具名拒絕」。本交付跑的是已修訂的那支探針，三輪都是 255 行、2455 passed、0 failed。通過數比「2453 再把那 1 筆算進通過」多 1。這次沒有增刪測試。

### 8.4 工單指名要你回答的問題

**Q1** 以行程工作目錄決定宇宙或儲存的地方：

1. `crates/oo/src/main.rs:241` `real_cwd()`。未給 `--universe`／`--ephemeral` 時，`cwd()` 用它當儲存目錄。相對的 `--universe` 用它接成絕對路徑。檔案引數的 `read_source_file` 仍打開呼叫者給的路徑。
2. `crates/interpreter/src/lib.rs:4943` `Ouroboros::log`。參數改為 `base_dir`。`run_log` 傳入 `cwd()`。
3. `crates/interpreter/src/value.rs:2783` `Identity::node_key_path`。工作區路徑已是絕對路徑時直接採用；相對路徑才接上 `current_dir()`。`node` 子指令傳入的是 `cwd()`，而那些子指令沒有選擇器，所以今天仍是行程目錄。見 Q5。
4. `crates/interpreter/src/builtins/fs_guard.rs:65` 與 `:95`。這是語言路徑的檔案邊界，用來辨認操作者身分檔。儲存目錄不由這裡決定。留著。
5. `crates/interpreter/src/builtins/env.rs:64` `env.cwd`。它把行程目錄交給程式。宇宙不由這裡決定。`--universe` 之下，程式看到的仍是呼叫者站的地方。留著。

`crates/interpreter/src/storage.rs:38` 的 `Path::new(".")` 是原子寫入時目標沒有父目錄的後備目錄，不選宇宙。探針裡的 `Command::current_dir` 是把子行程放進樣本目錄。

**Q2** 臨時宇宙在 `std::env::temp_dir()` 下，目錄名 `oo-ephemeral-<pid>-<nanos>`。`SelectedUniverse` 的 `Drop` 對它 `remove_dir_all`。`oo test` 在 `process::exit(1)` 之前先 `drop`。量過：一次 `eval --ephemeral '_.a'`（rc 0，輸出 `_`）前後，`/tmp/oo-ephemeral-*` 都是 29 個。這 29 個是更早一輪探針留下的空目錄；這次指令的目錄有被刪掉。

求值與「沒有宇宙的地方」在 D82 上同一條路：`without_universe`，`holds_universe` 為假，根是空白的，明示 `save` 答 `#no_universe`。探針的 `away` 沒有節點設定，所以 r7 逐位元組相同。

節點設定讀的是這個基底，不是呼叫者的工作區。`without_universe` 會讀基底上的 `.oo/discovery.n`（缺檔則空集合，不建檔）、`.oo/peers/`（缺檔則空）、`.oo/architects.json`（缺檔則空集合，不建檔）。空的暫存目錄得到這些空值。一個沒有宇宙、目錄裡卻放了這些檔的地方，會讀到那些檔。物件本體另存在 `scratch::ephemeral_store_root`（前綴 `nlang-test-`），隨引擎放下，與上面這個宇宙基底不是同一個目錄。

**Q3** 自述，取自這支 `target/release/oo` 的說明：

```
run
Inject files into a read-only view of this universe's committed root and observe. Does not stage or commit. An explicit ~%Engine./save writes this universe's object store; with no universe it answers #no_universe

eval
Evaluate one n/ expression from this universe's committed root and print it. The working set does not count. Where there is no universe, the root is empty

test
Observe `test_` fields from this universe's committed root and report pass/fail (use lint for a static graph check that does not run)
```

共用的旗標說明：`--universe` 是 `Use the universe in this directory instead of the one where this command is run`（`value_name` 為 `DIR`）。`--ephemeral` 是 `Evaluate against an anonymous temporary universe and leave this universe untouched`。

**Q4** euid 1000。宇宙裡已有一次提交。

| 情況 | `eval '_.a'` |
| :-- | :-- |
| `.oo/HEAD` 模式 000 | rc 1，stdout 空，stderr `Error: cannot read .oo/HEAD: permission denied`。測完改回 644。 |
| `.oo/HEAD` 被改名挪開，儲存裡已有提交 | rc 1，stdout 空，stderr `Error: lost context: HEAD is absent and the store records a commit; restore it with rollback <commit> --grant rollback`。 |

**Q5**

1. `node` 的子指令呼叫 `cwd()`，沒有 `--universe`。`run_node_id` 與 `run_serve`（`main.rs:961`）把這個路徑交給 `engine_keeping_settings` 與 `Identity::node_key_path`。今天「這個工作區」是行程目錄。
2. `fmt` 打開參數上的那個檔（`read_source_file`）。相對路徑相對行程目錄。
3. `lint` 打開參數上的檔或目錄（`nlint::run_cli`）。相對路徑相對行程目錄。
4. `identity` 用 `Identity::resolve_path()`：環境變數 `OO_IDENTITY`（必須是絕對路徑），否則 `~/.oo/identity`。它不讀工作區的 `.oo/`。

這四個指令，以及 `repl`，都沒有 `--universe`。

### 8.5 探針

`crates/oo/tests/the_universe_you_are_in_probe_test.rs` 未改，未 rustfmt。14／14，1.27s。

`crates/oo/tests/a_history_older_than_its_savepoints_probe_test.rs` 未改。13／13，0.98s，含驗收方已修訂的 g5。

紅線，皆 `test result: ok`、0 failed：`a_history_an_old_crash_cut` 11（5.37s）、`a_landing_that_head_decides` 10（2.11s）、`already_in_head` 7（7.42s）、`cas_integrity` 13（1.65s）、`one_writer_at_a_time` 10（17.74s）、`what_it_says_is_what_happened` 23（2.04s）、`where_there_is_no_universe` 13（3.79s）。

交叉編譯 `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu`：`Finished` 4.34s，`^error` 0。

### 8.6 數字

1. 三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`：每輪 rc=0，`^error` 0，`test result:` 255 行，2455 passed，0 failed。去掉 ` finished in ` 之後三輪 `cmp` 相同。
2. conformance 162／162，rc=0。cwd 是 `/home/gali/nlang`。
3. `~%Math./add (1, 2)` → 3，`(1, 3)` → 4，rc=0。這兩次 `eval` 沒有建立 `.oo/`。euid 1000。
4. `x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。`v: 1 + 1` 根 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`。新倉 `.oo/format` 為 `layout=8`，`.oo/objects.format` 為 `encoding=5`。

### 8.7 你認為需要改規格之處

規格檔這次沒有改。`REAL_01` §1.1、§1.4 的 D88、以及 `SYNTAX_03` 若要註記，仍由驗收方收尾。建議把 `run` 的自述，以及 `--universe`／`--ephemeral` 的那兩句說明，收成與 Q3 相同的句子。

---

## 9. 驗收（驗收方填）
