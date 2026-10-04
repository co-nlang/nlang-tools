# 工單：一次拒絕建出來的宇宙（Q-074）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-074／讀法確認 `meta/oo/STATUS.md` **D92**／依據 D82、D82 ②（`REAL_02` §5.1.1）、Q-065（`SPEC_10` §3.1「一次回報失敗的演化沒有留下提議」）
> 探針（已預先提交並校準）`crates/oo/tests/a_refusal_that_built_a_universe_probe_test.rs`
> 基線：dev `e658530`／`oo v0.75.0` ⟹ **2 綠 4 紅**，三輪一致；四支紅的都紅在一般斷言，零空洞讀數。

## 1. 缺陷

〔量，v0.75.0〕在沒有宇宙的地方（空目錄，或只有節點設定 `discovery.n` 的 `.oo/`），一次被拒絕的 `evolve` 留下 `.oo/format`、`.oo/objects.format` 與空的 `objects/`。八種拒絕全部如此：剖析錯誤、檔案不存在、檔內衝突、寫進 `~%` 軸、以 `_.` 起錨的鍵、不認識的旋鈕、不認識的授權、`--pin` 缺授權；兩個檔而第二個剖析失敗亦然。
依 D82 ② 那些是**宇宙內容** ⟹ 該目錄自此是一個空宇宙：`status` 答 `Universe is static (no staged changes).`、`log` rc=0，而原本兩者答 `no universe here: start one with evolve`。
〔量〕沒有欄位的檔（空檔、只有註解）`evolve` rc=0，同樣建出宇宙，**另鑄一顆內容為空的 ○**——沒有寫進任何提議。`v: _`（只有 Top）寫下一個內容為 `{}` 的注入檔，而 `status` 答「沒有暫存的變更」——依 D84 那不是提議，卻同樣建出宇宙。
〔量〕**提議從來不是漏點**：兩個檔第二個失敗，第一個的提議也不留下；已有宇宙時，被拒絕的 `evolve` 逐位元組不動任何東西；已有宇宙時空檔 `evolve` 不鑄 ○。
〔讀〕`run_evolve` 一開始就 `Ouroboros::init`，那時還不知道會不會寫進提議。

## 2. 依據

*   **D82（`REAL_02` §5.1.1）**：「沒有宇宙的地方，**唯一**可以寫下宣告與宇宙內容的是**把提議寫進工作區的那個動作**（`#evolve`）」；D82 ②：宣告是宇宙內容。
*   **D92（用戶確認讀法，2026-10-04）**：一次**沒有寫進提議**的 `evolve`——被拒絕的，或沒有東西可提議的——**不是**那個動作，**不建立宇宙**。
*   **Q-065（`SPEC_10` §3.1）**：一次回報失敗的演化沒有留下提議。本弧把同一條線延伸到宣告。

## 3. 射程＝不變式（不是機制）

*   **I1** 在沒有宇宙的地方，一次被拒絕的 `evolve` 之後，`.oo/` 與之前逐位元組相同（之前沒有 `.oo/` 就仍然沒有）——**每一種拒絕**（r1；請在 Q1 列出你檢查過的每一條拒絕路徑，含 r1 沒列的）。
*   **I2** 只有節點設定的容器，拒絕之後仍只有那些節點設定，位元組不變（r2）。
*   **I3** 拒絕之後，**仍然沒有宇宙**：`status`、`log` 與在一個沒人碰過的目錄裡逐字相同、同 rc（r3）。
*   **I4** 沒有東西可提議的 `evolve`（空檔、只有註解、**以及注入了卻不改變位置者，如 `v: _`**——D84 說它不是提議）在沒有宇宙的地方不建宇宙、不鑄 ○（r4）。「有沒有提議」照 `status` 會不會列出它判，**不是**照有沒有寫下注入檔。它的 rc 與說法由你決定，Q2 陳述。
*   **I5** **寫進提議的 `evolve` 照舊建立宇宙**，含逐欄 `_|_` 者（g1）。
*   **I6** **已有宇宙時一切照舊**：被拒絕的 `evolve` 不動任何東西，宇宙與工作集都還在（g2）。**不得**以「刪掉 `.oo/`」之類的收尾去碰一個本來就在的宇宙或節點設定。
*   **I7** 中途失敗也算：若有一條路徑是寫下了某些東西之後才失敗（例如寫下注入之後讀不到 ○ 鏈，Q-065 那一格），在沒有宇宙的地方，失敗之後同樣沒有宇宙。

## 4. 紅線（今天綠，必須保持綠）

*   g1、g2；`where_there_is_no_universe` 13（D82）、`nothing_here_or_nothing_you_can_see`、`what_did_not_land`（Q-065）、`what_an_observation_leaves` 15、`one_writer_at_a_time` 10。
*   **交叉編譯** `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   conformance 162／162；`x: 0` 根 `31745ef0…`、`v: 1 + 1` 與 `1+1` 根 `f4f32e7b…`、標準根 `7038e250…`；`~%Math./add` (1, 2)=3、(1, 3)=4；新倉 `layout=9`／`encoding=5`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** 你檢查過的每一條 `evolve` 拒絕路徑（含 r1 之外的），各自在沒有宇宙的地方之後留下什麼。
*   **Q2** 沒有東西可提議的 `evolve` 在沒有宇宙的地方答什麼、rc 多少；在已有宇宙的地方答什麼。
*   **Q3** 你的做法是「晚建」（確定要寫進提議才寫宣告）還是「失敗後收回」；若是後者，崩潰在收回之前會留下什麼。
*   **Q4** 事實陳述：除了 `evolve`，還有沒有別的指令會在沒有宇宙的地方寫下宣告（D82 說不得）。**不要修**，列出即可。

## 6. 明文不在射程內

*   已有宇宙時 `evolve` 的任何行為。
*   `--universe <DIR>` 之下的措辭（Inbox 另列）。
*   規格文字（`REAL_02` §5.1.1 D82 款補一句「沒有寫進提議的演化不建立宇宙」）、`CHANGELOG`：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r4 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。**
*   措辭無關：「什麼都沒留下」＝`.oo/` 逐位元組與之前相同；「仍然沒有宇宙」＝`status`／`log` 與沒人碰過的目錄逐字相同。`rustfmt` 不得掃探針檔。
*   〔驗收方校準〕**兩極皆做**：
    基線 2 綠 4 紅（三輪一致）。
    一個最小參考實作（`run_evolve` 先記下這裡有沒有宇宙；被拒絕、或重新載入後工作集為空時，移掉這次建出的宣告與目錄，節點設定不碰）⟹ 本探針 6／6。
    **參考實作第一版判「有沒有提議」看的是注入目錄有沒有檔**——本探針初版 6／6 也綠，**是全樹裡 `what_it_says_is_what_happened` r10 擋下的**：它的兩側（空檔 `evolve` 與 `v: _` 後 `commit`）必須答得一樣，而第一版讓前者沒有宇宙、後者有。**錯在驗收方的探針少了 `v: _` 那一格**；已補進 r4，〔量〕第一版參考實作在補後的 r4 上紅。
    同一參考實作（最終版）跑全樹：**259 target／2502 passed／0 failed，exit 0**（＝ v0.75.0 的 258／2496 加上本探針 6 支）；交叉編譯 0 error。**無須預先修訂既有測試**——r10 的衝突是參考實作的錯，不是 r10 的。
    **守衛的變異**：成功時也收回 ⟹ g1（g2 連帶紅：它要先建出宇宙）；不管原本有沒有宇宙都收回 ⟹ g2。
    **參考實作是「失敗後收回」——那是校準用的捷徑，崩潰在收回之前會留下宇宙；不要照抄**（Q3、I7）。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 做了什麼

沒有宇宙的地方，`evolve` 先在工作區旁邊的 `.nlang-evolve-<id>/` 裡演練。拒絕、或這次會話 `status` 不會列出，就丟掉那個目錄，工作區一個位元組都不動。會列出時，先在旁邊的倉 `save_staged`，成功之後才把那個 `.oo` 換進工作區。工作區還沒有 `.oo/` 時，換進去是一次 `rename`。`.oo/` 裡只有節點設定時，逐項搬進去，原本的名字留著；這次搬進去的若回報失敗，只移走這次搬的名字。

「會不會列出」與 `proposals_at` 對空根的比較相同，再套 `load_staged` 的 `is_dirty`：沒有 delta 的（空檔、只有註解、只有空白）不列；`v: _` 停在原位，不列；逐欄 `_|_` 與 `~%Config.fuel` 會列。

另一個 `evolve` 已經把宇宙發布出去時，這次不覆蓋那個 `.oo`，改走原本已有宇宙的寫入，所以第二份注入還在。已有宇宙的 `evolve` 仍是原來的路徑。

套件版本仍是 `oo` `0.75.0`。`layout=9`，`encoding=5`。

### 8.2 順手改動（逐項指名）

1. `crates/interpreter/src/universe.rs`：`session_would_be_listed`。沒有 HEAD、沒有先前的成員時，`status` 會不會列出這次會話。
2. `crates/oo/src/main.rs`：沒有宇宙時走旁邊的倉；有宇宙時仍是 `evolve_at`。發布時若宇宙已經出現，把這次檔案再演進進那個倉。

探針、規格、`Cargo.toml`、`Cargo.lock` 未改。沒有 rustfmt `main.rs`、`universe.rs`、探針。

### 8.3 工單哪裡是錯的

無。工單寫的交付後全樹是 259 target、2502 passed、0 failed。這次三輪的 `test result:` 與此相同。

### 8.4 工單指名要你回答的問題

**Q1** 沒有宇宙的空目錄，rc 都是 1，stdout 空，`.oo` 不存在，旁邊的 `.nlang-evolve-*` 也不留。euid 1000。

1. `a: (`：`Error: Parse Error in "f.n"`，`expected unary_expr`（箭頭在第 2 列）。
2. 檔案是 `a: 1`，引數是 `nofile.n`：`Error: cannot read nofile.n: file not found`。
3. 同檔 `a: 1` 與 `a: 2`：`Error: Evolution Conflict in "f.n": #conflict at a`。
4. 兩檔 `a: 1` 再 `a: 2`：`Error: Evolution Conflict in "b.n": #conflict at a`。
5. `good.n` 是 `a: 1`，`bad.n` 是 `b: (`：`Error: Parse Error in "bad.n"`，`expected unary_expr`。
6. `~%Foo: 1`：`Error: Evolution Conflict in "f.n": #system_reserved at ~%Foo`。
7. `_.a: 1`：`Error: Parse Error in "f.n": definition key may not anchor at the root: _.a (at byte 0)`。
8. `x: { _.a: 1 }`：同一句，`at byte 5`。
9. `~%Config.bogus: 1`：`Error: Evolution Conflict in "f.n": #invalid_config at ~%Config.bogus`。
10. `~%Config.fuel: "no"` 與 `~%Config.fuel: -1`：`Error: Evolution Conflict in "f.n": #invalid_config at ~%Config.fuel`。
11. `--grant nonsense`，檔案 `a: 1`：`Error: unknown grant SPEC `nonsense` (allowed: effect_override[:tag[+tag]*], pin, rollback, squash, gc, migrate, connect)`。
12. `--pin f.n` 而沒有 `--grant pin`，檔案 `a: 1`：`Error: #privileged_required: --pin requires --grant pin (privilege.pin capability)`。
13. `--pin --grant pin` 而檔案是 `a: (`：同第 1 條的剖析錯誤。
14. 路徑是目錄：`Error: cannot read notafile: is a directory`。
15. 來源模式 000：`Error: cannot read f.n: permission denied`。量完改回 644。

之後的 `status` 與 `log` 都是 rc 1，`Error: no universe here: start one with evolve`，與沒人碰過的目錄相同。

丟了 context 要已有記載提交的 ○，那已經是宇宙內容，不是這一條。

**Q2** 沒有宇宙：空檔、`;; nothing`、只有空白、`v: _`，`evolve` 都是 rc 0，stdout 與 stderr 都空，沒有 `.oo`。接著 `status` rc 1，`Error: no universe here: start one with evolve`。只有節點設定（`node trust add` 那把 64 個 a 的鍵）再 `evolve` `v: _`：rc 0，`.oo` 與演進前 `diff -rq` 相同。

已有宇宙（先 `k: 1` 再 `commit -m k`，○ 數 2）：空檔與只有註解 rc 0，不印字，○ 數仍是 2，沒有注入檔。`v: _` rc 0，不印字，○ 數仍是 2，注入檔有一個；`status` rc 0，仍是

`Standard root dependency: 7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911 (available)`

`Universe is static (no staged changes).`

**Q3** 晚發布。宣告出現在已經知道這次會列出、而且旁邊的 `save_staged` 已經成功之後。拒絕或不會列出的，在 `rename` 之前就丟掉旁邊的目錄。崩潰在 `rename` 之前留下的是 `.nlang-evolve-<id>/`，裡面有它自己的 `.oo`，工作區仍然沒有宣告。`evolve` 不呼叫 `process::exit`。

工作區沒有 `.oo/` 時，發布是同檔案系統上的一次 `rename`。只有節點設定時是逐項搬；崩潰在那個窗口可以在節點設定旁邊留下見證。那次搬移若回報失敗，只移走這次搬進去的名字。

**Q4** 不修。`~%Discovery./connect` 的非 tcp 路徑對目標呼叫 `ObjectStore::init`。

沒有宇宙的地方，`run --observe p p.n`，`p: ~%Discovery./connect { 0: "peer", 1: "../peer" }`：rc 0，stdout `#true  ;; %effect: #io`，呼叫端沒有 `.oo`，`../peer/.oo` 出現 `format`、`objects.format`、`objects/`。`eval '~%Discovery./connect { 0: "peer", 1: "../peer" }'` 同樣：rc 0，呼叫端沒有 `.oo`，目標出現那三項。呼叫端已經 `evolve` 過 `k: 1` 時，目標同樣出現那三項。

### 8.5 探針

`crates/oo/tests/a_refusal_that_built_a_universe_probe_test.rs` 未改，未 rustfmt。全樹第三輪 6／6，0.53s。r1–r4、g1–g2 皆綠。

紅線，全樹第三輪皆 `test result: ok`、0 failed：`where_there_is_no_universe` 13（3.75s）、`nothing_here_or_nothing_you_can_see` 7（14.12s）、`what_did_not_land` 12（2.02s）、`what_an_observation_leaves` 15（3.03s）、`one_writer_at_a_time` 10（17.69s）。

交叉編譯 `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu`：`Finished` 0.88s，`^error` 0。

### 8.6 數字

1. 三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`：每輪 rc=0，`^error` 0，`test result:` 259 行，2502 passed，0 failed。`running`／`test `／`test result:` 三輪各 3020 行，去掉耗時後相同。原始日誌的警告順序不必相同。
2. conformance 162／162，rc=0。cwd 是 `/home/gali/nlang`。引擎是 `nlang-tools/target/release/oo`。
3. `~%Math./add (1, 2)` → 3，`(1, 3)` → 4，rc=0。這兩次 `eval` 沒有建立 `.oo/`。euid 1000。`/tmp/oo-ephemeral-*` 為 0。
4. `oo inspect <HEAD>` 的 `root:`：`x: 0` 為 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。`v: 1 + 1` 與 `v: 1+1` 為 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`（`status` 該行帶 `(available)`）。新倉 `.oo/format` 為 `layout=9`，`.oo/objects.format` 為 `encoding=5`。
5. `b: 1 & 2` rc 0，有 `.oo`，`status` rc 0，列出 `b: _|_  ;; %cause: #conflict`，熵 200 bits。`~%Config.fuel: 20` rc 0，列出 `~%Config: { fuel: 20 }`，熵 229 bits。

### 8.7 你認為需要改規格之處

規格檔這次沒有改。建議驗收方依工單第 6 節：`REAL_02` §5.1.1 補「沒有寫進提議的演化不建立宇宙」，並收 `CHANGELOG`。Q4 的 `Discovery./connect` 仍會在沒有宇宙的目標寫下宣告，本弧未動。

---

## 9. 驗收（驗收方填）

**受理，零修補回合。** 交付 `260be65`：探針、既有測試、分隔線以上、`Cargo.lock`、版本皆未動。
**交叉編譯**（`touch` 後強制重查）0 error。**全樹 ×3**（期間不跑其他量測）：**259 target／2502 passed／0 failed，`^error` 0，exit 0**，三輪逐行相同。
**驗收方矩陣**〔交付二進位〕：`x: 0`／`v: 1 + 1`／`v: 1+1` 三個新倉提交後皆無殘留的 `.nlang-evolve-*`。空目錄裡 `evolve a.n` 與 `evolve b.n` 並行、交替先後 ×6：兩份注入皆在（`status` 列出 `a: 1` 與 `b: 2`，注入檔 2 個）、無殘留、無錯誤輸出——**未以對照組證明撞進發布窗口**，記為「沒看到問題」。
**Q4 重現**：沒有宇宙的地方 `eval '~%Discovery./connect { 0: "peer", 1: "../peer" }'` ⟹ `#true ;; %effect: #io`；呼叫端無 `.oo`；`../peer` 被建出並有 `format`／`objects.format`／`objects/`，在那裡 `status` 答 `Universe is static`。v0.75.0 相同 ⟹ 既有、射程外，入 Inbox。
身分：`31745ef0…`／`f4f32e7b…`（`1 + 1` 與 `1+1`）／標準根 `7038e250…`；known-answer 3／4（不建 `.oo/`）；conformance 162／162；新倉 `layout=9`／`encoding=5`。
