# 工單：一個沒有被提交的私有欄位（Q-079）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-079／裁定 `meta/oo/STATUS.md` **D103**（用戶 2026-10-11，選甲）／依據 `SPEC_04` §3.1（#1 向內可見；#4 剝除只在顯示層，CAID／`=` 含 local 軸；#5 裸名＝作用域鏈＝內部人，`.~key` 含 `_.~key` ＝外部定位）、`REAL_03` §6（`~`／`~@`／`~/` 是身分的第 6–8 軸）、`SPEC_03` §3.1（展開源在所在容器解析，位置不得影響結果；§1 的範例）、`REAL_01` §1.1（D88：一次性求值以 HEAD 的根為起點）
> 探針（已預先提交並校準）`crates/oo/tests/a_private_never_committed_probe_test.rs`
> 基線：dev `82d19de`／`oo v0.80.0` ⟹ **4 綠 6 紅**；六支紅的都紅在一般斷言，零空洞讀數。

## 1. 缺陷

〔量，v0.80.0；v0.72.0、v0.76.0、v0.79.0 逐格相同；v0.71.0 以前一次性求值跑在空白宇宙，量不到〕

*   **根上的 `~` 欄位沒有進宇宙**：只有 `~h: 3` 一行的檔，`evolve` 回成功而**沒有建出 `.oo/`**；與別的欄位同檔時，工作集成員裡沒有它；提交後的根上沒有它，下一次 evolve 的 `k2: h + 1` 得 `_`。
*   **身分**：`~h: 3; k: 1`、`~h: 4; k: 1`、`k: 1` **提交出同一個根** `ced4d79f…`。同樣的 `~` 欄位放在 Combo 裡，三者三個根。
*   **靜默錯答案**：根上的子容器、更深的子孫、根上與子容器裡的態射讀 `h`，提交後全得 `_`；同一個檔 `run --observe` 全得 `3`。同層 `m: h` 讀得到 `3`，只因提交當下已算好。
*   **一次性求值**：已提交 `~h: 3` 的宇宙裡 `oo eval 'h'` 得 `_`（D103 裁定應得 `3`）。
*   **根上的欄位展開一個根上的名字，提交後那次展開不見**：`c: { b: 2 }` 加 `r: { a: 1, ...c }`，提交後 `_.r` 得 `{ a: 1 }`（`r: { ...c }` 得 `{}`；`c` 寫在前或後皆然，`c` 為 `~c` 亦然）。同一個檔 `run --observe r` 得 `{ a: 1 b: 2 }`。`SPEC_03` §1 自己的範例 `result_spread: { a: 1, ...~c }` 因此跑不出 `{ a: 1, b: 2 }`。Q-078 宣稱涵蓋提交面，**它的探針沒有量這個形狀**（驗收方的漏）。

## 2. 裁定與依據

*   **D103（甲）**：一次性求值在宇宙裡站在根的內部——根就是它的工作目錄，裸名 `h` 是取工作目錄內的節點 `h`；`_.~h` 是從外面指名，仍 `#private_access_violation`。與 2026-07-15 私有軸實施裁定 Q1（`SPEC_04` §3.1 #5）同一條判準。〔量〕同一個檔 `run --observe h`／`~h` 今天就得 `3`。
*   其餘皆已在規格：根上的 `~` 欄位是根的欄位（`REAL_03` §6、`SPEC_04` §3.1 #4），向內可見（#1）；展開源在所在容器解析、位置不得影響結果（`SPEC_03` §3.1）。

## 3. 射程＝不變式（不是機制）

*   **I1 根上的 `~` 欄位是根的欄位**：只有它的檔也是一次提議；它進工作集、進提交後的根；下一次 evolve、提交後的讀取都看得到它（r1）。
*   **I2 向內可見，提交前後一樣**：根上的子容器、任意深度的子孫、根上與子容器裡的態射，提交後讀得到根上的 `~` 欄位，兩種拼法（`h`、`~h`）皆然（r2）。
*   **I3 身分含 local 軸**：只差一個根上 `~` 欄位的宇宙，提交出不同的根（r3）。
*   **I4（D103）**：一次性求值在宇宙裡以裸名讀得到根上的 `~` 欄位（r4）；外部定位仍被擋、顯示仍不呈現 local 軸（g1）。
*   **I5 根上的 `~` 欄位單調演化**：與已提交的值不同就具名拒絕，相同就落地；`--pin --grant pin` 照舊可覆寫（r5）。
*   **I6 根上的欄位展開根上的名字，跨 evolve 與提交仍成立**：源寫在前或後、`~` 或不帶 `~`、分兩次 evolve 寫（同一個工作集、提交前），皆得同一個答案；碰撞在兩種順序下都是欄位 ⊥，與字面 `{ a: 1, ...{ a: 2 } }` 相同（r6）。從未定義的源仍是空操作（g4）。
*   **I7 Combo 內的 `~` 欄位與沒有根上 `~` 欄位的根不動**（g2 四根）；展開不帶出源的 `~` 欄位（g3）；同一個程序裡（`run --observe`）的讀數不變（g4）。
*   **I8 效應時點不變**：帶效應的展開源，今天在 evolve 時執行一次、提交時不再執行（〔量，v0.80.0〕`log.txt` 兩階段皆 `a`）；你的交付必須相同。
*   **I9 不改磁碟格式**：不推進佈局或編碼；工作集成員與提交物件的編碼不得因本卡多出新的列形。含根上 `~` 欄位、或根上欄位展開根上名字的值，位址會移動（破壞性，驗收方記帳，請在 Q3 逐根列出）。

## 4. 紅線（今天綠，必須保持綠）

*   g1–g4。Q-078 探針（15）、Q-077 兩支、Q-076 三支、`forward_spread_probe_test.rs`（12）、`a_field_outside_identity_probe_test.rs`（8）、`a_field_outside_identity_crate_test.rs`（3）。conformance **162／162**。
*   **交叉編譯** `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   `x: 0` 根 `31745ef0…`；`v: 1 + 1` 與 `1+1` 根 `f4f32e7b…`；`f: (x -> x + 1)` 根 `039d0735…`；`k: { a: 1, _: 9 }` 根 `24fe01b6…`；`m: (_ -> 1)` 根 `c23f8a56…`；標準根 `7038e250…`。`~%Math./add` (1, 2)=3、(1, 3)=4。新倉 `layout=9`／`encoding=5`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** 每一處「走過一個容器的欄位、卻只走公開軸」的地方：在哪裡、是否該含 local 軸、改了什麼或為什麼不必改。至少包括工作集成員的建立與重建、注入的折疊、提交、`replace_merge`／pin、squash／rollback 的 shadow 掃描（`universe.rs` 有一處 `all_fields_iter()` 走訪），以及 `status`／`inspect`／`log` 的顯示。
*   **Q2** 根上欄位裡未落地的展開，在 evolve 與提交之間怎麼保住（I6、I9）：在哪一刻落地、看得到哪些名字、為什麼兩種順序得同一個答案。
*   **Q3**〔量，陳述〕下列每一個檔案提交後的根，v0.80.0 與你的交付各是什麼，哪些移動了；每一個移動的，讀出來的答案有沒有變——**答案沒變而位址移動的，就是 I7／I9 的違反**，若有，說明為什麼。檔案：`~h: 3` 加 `k: 1`、`~h: 4` 加 `k: 1`、`k: 1`、`~h: 3`、`~h: 3` 加 `t: { k: h }`、`t: { ~h: 3, k: 1 }`、`c: { b: 2 }` 加 `r: { a: 1, ...c }`、`r: { a: 1, ...c }` 加 `c: { b: 2 }`、`c: { b: 2 }` 加 `r: { q: c.b }`、`r: { a: 1, ...nothing }`。
*   **Q4**〔量，陳述，不要求改〕`r: { a: 1, ...c }` 在一次 `oo evolve a.n`、`c: { b: 2 }` 在下一次 `oo evolve b.n`（同一個工作集、提交前），提交後 `_.r` 得什麼、為什麼。
*   **Q5**〔量，陳述〕`oo status`、`oo inspect <HEAD>`、`oo log` 對一個只有根上 `~` 欄位的宇宙各顯示什麼；`SPEC_04` §3.1 #4 說外部觀測者的顯示投影不呈現 local 軸——你把操作者算成哪一邊，為什麼。

## 6. 明文不在射程內

*   `_.` 在程式內印 `{}`、`q: j` 寫在 `j: 1` 前或後根不同（Thunk 對結果）、`{ ...5 }`／`{ %val: _, z: 1 }` 吸收兄弟、表 `&` 兩讀：Inbox 既有列。
*   `~%Math./add` 長鏈的深度上限報成 `#conflict`、規格四處範例把 `<`／`>` 當數值：Inbox 既有列。
*   規格文字與 `CHANGELOG`：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r6 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。** `rustfmt` 不得掃探針檔。
*   〔驗收方校準〕**兩極皆做**：
    基線 v0.80.0：**4 綠 6 紅**（g1–g4 綠、r1–r6 紅）。
    一個參考實作 ⟹ 本探針 **10／10**。
    同一參考實作跑全樹（含本探針）：**267 target／2582 passed／0 failed，exit 0**（＝ v0.80.0 的 266／2572 加上本探針 10 支，沒有任何既有測試撞到）；交叉編譯 0 error；conformance 162／162；§4 身分紅線全數不動；Q-078 g3 的七根不動。
    **守衛的變異**（每一個只換一處；參考實作上量，每次限 8 GB 記憶體、15 分鐘，量完還原）：讓 `.~key` 段讀得到欄位 ⟹ g1；提交時丟掉每個容器的 local 軸 ⟹ g2（與 r1、r2）；源的 `~` 欄位一律隨展開帶過去 ⟹ g3；從未定義的展開源改答 ⊥ ⟹ g4。還原後 10／10。
    〔驗收方自陳〕參考實作第一版在 evolve 當下讓欄位的展開落地、源未定義就延後重試，兩種順序給了兩種答案——I6 的「兩種順序」一句由此而來。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照
I1：工作集成員重建時把 local 軸一併留下。只有 `~h: 3` 的檔會建出 `.oo/` 並提交；下一次 `k2: h + 1` 讀到 4。r1 綠。
I2：提交後的根帶著這個 `~` 欄位，子容器、更深的子孫、根上與子容器裡的態射用 `h` 讀得到。r2 綠。
I3：`~h: 3; k: 1`、`~h: 4; k: 1`、`k: 1` 三個根。r3 綠。三個摘要在 Q3。
I4：一次性求值的裸名走根的 local 軸。`h`、`h + k`、`{ q: h }.q` 得 3、4、3。`_.~h` 仍是 `#private_access_violation`，`_.` 不印 `h`。r4、g1 綠。
I5：已提交的 `~h: 3` 再 evolve `~h: 4` 以 `#conflict` 拒絕，`h` 仍是 3。r5 綠。`--pin --grant pin` 覆寫後 `h` 是 4。
I6：寫成員前，staged 裡還帶未落地展開的欄位先展開；當時整份 staged 都在，所以同一個檔裡源在前或在後是同一個值。r6 綠。兩次 `oo evolve` 見 Q4。從未定義的源仍落地成沒有那個欄位。g4 綠。
I7：只把容器交給讀到自己外面的源這條沒動。g2 四根與 g3 綠。根上沒有 `~` 欄位、展開也不會多帶出源的 `~` 欄位。
I8：`w: ~%Io./append_file ("log.txt", "a")` 再 `r: { ...{ q: w } }`，v0.80.0 與交付都是 evolve 後 `log.txt` 為 `a`、提交後仍是 `a`。
I9：佈局與編碼沒改，沒有新的列形。`pending_spreads` 仍不進成員。意義變了的根在 Q3，讀出來的答案都變了。

### 8.2 順手改動（逐項指名）
無。

### 8.3 工單哪裡是錯的
I6 寫「分兩次 evolve、兩種順序同一個答案」。量到的是：源先寫、展開後寫，提交後 `_.r` 是 `{ a: 1, b: 2 }`，根 `e2ea63e3…`；展開先寫、源後寫，`_.r` 是 `{ a: 1 }`，根仍是 v0.80.0 的 `2f88a514…`。Q4 寫明不要求改，展開先寫的那次就維持這樣。

### 8.4 工單指名要你回答的問題

**Q1**

1. 工作集成員的建立。`note_session_incoming` 第一次整份複製，之後的 unify 本來就合併 local。緊接著 `restamp_thunk_effects` 只按 `all_fields_iter` 重建，local 被丟掉，只有 `~` 欄位的檔因此不算一筆提議，`.oo/` 不會出現。改了：重建時把 `session_delta.local` 裡的鍵從 `staged.local` 抄回去。
2. 工作集的重建。`load_staged` 用注入的 `fold`。unify 已經走 local 軸。成員裡有，載回來就有。沒另改。
3. 注入的折疊。同上，`fold` 的 unify 含 local。沒改。
4. 提交。`unify(root, staged)` 含 local。寫成員之前多了 `land_staged_spreads`：公開軸與 local 上還帶 `pending_spreads` 的欄位，在這次程序的整份 staged 都在時展開，展開結果再寫進成員。
5. `replace_merge`。公開軸之後已經把 `incoming.local` 覆上。pin 走這條。沒改。
6. `pin_commit_merge`。釘住的座標若在 local，已經寫回 `out.local`。沒改。
7. squash／rollback 不掃欄位。rollback 用 `get_root` 載整顆根，local 在裡面。工單點名的那處 `all_fields_iter` 在 `refine` 的 shadow 掃描，只走公開軸。只活在 local 軸上的來源不會被標成 shadow。沒改：`shadow_affected` 進 refine 提交的摘要，這張卡沒有要動那些位址。
8. 顯示。`observe` 走顯示投影，剝掉 local。`status` 有 staged 時印 `to_nlang`，`to_nlang` 含 `~` 欄位。`inspect` 一個物件也是 `to_nlang`。`inspect` HEAD 與 `log` 只印提交。見 Q5。

**Q2** 同一個 `oo evolve` 在 `save_staged` 寫成員之前展開。查找用的 staged 是當時整份工作集，裸名也查 local，所以源寫在前或後、帶 `~` 或不帶，都看得到。展開後 `pending_spreads` 清空才寫進成員，成員編碼沒有新列。`c: { b: 2 }` 與 `r: { a: 1, ...c }` 兩種文字順序提交後都是 `_.r` = `{ a: 1, b: 2 }`，根都是 `e2ea63e33f5ca3b81afced21d66d04c163019093c10338f020b179522c0d7ff7`。

**Q3** 基線是 `82d19de` 的 release 二進位。交付的根是這次 release 二進位。

移動了，而且讀出來的答案變了：

1. `~h: 3` 加 `k: 1`：`ced4d79f3c4ae4d6cbf797089541b9b751e0fd6f6d61286624dde71be96a47d9` → `645e1938328e23840554d1bf52952ad83e41fabf76cee7fb88d1ae92b97a34e2`。`h` 由 `_` 變 `3`。
2. `~h: 4` 加 `k: 1`：`ced4d79f3c4ae4d6cbf797089541b9b751e0fd6f6d61286624dde71be96a47d9` → `02c18f3b22589ee18a3422f94d7f86e8f024d1927d1f17607424907cb1ee35c6`。`h` 由 `_` 變 `4`。
3. 只有 `~h: 3`：v0.80.0 的 evolve 成功但沒有 `.oo/`，commit rc=1，沒有根。交付的根是 `1154db1cdabe1153a146560a045fb2caef8f9bacf5597a8cdd854d412638a4e8`。`h` 由 `_` 變 `3`。
4. `~h: 3` 加 `t: { k: h }`：`9b8e626df4d0d0617c21bcbee18e1e50110064511899ae047edab2dae77bb79a` → `7cc380fd4c9bb40e1e926f6fa11ab6c65f8763bf3319ce3a3c70a4ecc708b277`。`_.t.k` 由 `_` 變 `3`。
5. `c: { b: 2 }` 加 `r: { a: 1, ...c }`：`2f88a514c4ca23fb818d26375e9360e868241cb560f143cb4bcdbe9f34d41cba` → `e2ea63e33f5ca3b81afced21d66d04c163019093c10338f020b179522c0d7ff7`。`_.r` 由 `{ a: 1 }` 變 `{ a: 1, b: 2 }`。
6. `r: { a: 1, ...c }` 加 `c: { b: 2 }`：與第 5 條同一個基線根、同一個交付根、同一個答案。

沒有移動：`k: 1` 為 `ced4d79f3c4ae4d6cbf797089541b9b751e0fd6f6d61286624dde71be96a47d9`（`_.k` 是 `1`）；`t: { ~h: 3, k: 1 }` 為 `05dc9846f0de0fa238ae44c23d1cc8c4545d275aaa2583afa1b12dc9d6dba051`（`_.t.k` 是 `1`）；`c: { b: 2 }` 加 `r: { q: c.b }` 為 `94d96d72d96b6c944f6bce50156fa5941f5b0feda16b283a8df29523b8f28cbf`（`_.r.q` 是 `2`）；`r: { a: 1, ...nothing }` 為 `5165149dffaf7d145ac33c6e6561dba5379f0fd707998709e363168710a1b9e5`（`_.r` 是 `{ a: 1 }`）。答案沒變而位址移動的，沒有。

**Q4** 基線與交付相同。`r: { a: 1, ...c }` 這次 `oo evolve`、`c: { b: 2 }` 下次 `oo evolve`，提交後 `_.r` 是 `{ a: 1 }`，`_.r.b` 是 `_`，根是 `2f88a514c4ca23fb818d26375e9360e868241cb560f143cb4bcdbe9f34d41cba`。第一次寫成員時 `c` 還不在工作集，展開留在 `pending_spreads`，而它不進成員；第二次載入的 `r` 已經沒有那個展開。反過來先寫 `c` 再寫 `r`，交付提交後 `_.r` 是 `{ a: 1, b: 2 }`，根是 `e2ea63e3…`。見 8.3。

**Q5** 只有 `~h: 3`、已經提交的宇宙：

`oo status` 印標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`（`available`），然後 `Universe is static (no staged changes).` 沒有欄位。

`oo inspect <HEAD>` 印提交的 CAID、`kind: commit`、`root:` 那一行。沒有欄位本體。

`oo log` 印 `commit`、`message: a`、`Date:`。沒有欄位。

`_.` 是只有 `~%__nlang_system_digest` 的投影，沒有 `~h`。裸名 `h` 是 `3`。

提交前的 `oo status` 印 staged 的 `to_nlang`，那裡有 `~h: 3`。`inspect` 根物件（不是 HEAD）的 `to_nlang` 也有 `~h: 3`。

操作者看 `_.` 時算外部：顯示投影不呈現 local 軸。看 staged／根物件的 `to_nlang` 時看到的是物件原文，local 軸還在。#4 剝的是顯示投影，不是倉裡的物件。

### 8.5 探針
無 `#[ignore]` 可拿。探針檔未改，未 rustfmt。

全樹第三輪：本探針 10／10，2.80s。Q-078 探針 15／15，3.80s。`forward_spread_probe_test` 12／12，0.72s。`a_field_outside_identity_probe_test` 8／8，0.22s。`a_field_outside_identity_crate_test` 3／3，0.03s。Q-077 原探針 12／12，2.09s；R-1 4／4，0.89s。Q-076 原探針 17／17，1.64s；R-1 6／6，0.41s；R-2 10／10，1.14s。

交叉編譯 `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu`：`Finished` 4.64s，`^error` 0。該 lib 15 則警告，含既有的 `unused variable: pre_existing`，未 cargo-fix。

### 8.6 數字
1. 三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`：每輪 rc=0，`^error` 0，`test result:` 267 行，2582 passed，0 failed。`Running`／`running`／`test `／`test result:` 三輪各 3380 行，去掉耗時後相同。原始日誌的警告順序不必相同。linux 上該 lib 14 則警告，未 cargo-fix。
2. conformance 162／162，rc=0。跑者只印這一行總結。cwd 是 `/home/gali/nlang`。引擎是 `nlang-tools/target/release/oo`。
3. 空目錄：`~%Math./add (1, 2)` → 3，`(1, 3)` → 4，rc=0。這兩次 `eval` 沒有建立 `.oo/`。euid 1000。`/tmp/oo-ephemeral-*` 為 0。
4. `oo inspect <HEAD>` 的 `root:`：`x: 0` 為 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。`v: 1 + 1` 與 `v: 1+1` 為 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。`f: (x -> x + 1)` 為 `039d07351a998261d3150af05a84bd0fcf0ad4133b24d644d35ab1e59517d2b4`。`k: { a: 1, _: 9 }` 為 `24fe01b6001567e4b03edc5e0715cb4cf53eadea4945994d7a963b495382662a`。`m: (_ -> 1)` 為 `c23f8a56608b9aa32b3fa089aa5d54ec4b0385b990d0cc2fe695970db114fb85`。標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`（`status` 該行帶 `(available)`）。新倉 `.oo/format` 為 `layout=9`，`.oo/objects.format` 為 `encoding=5`。套件仍是 `oo` 0.80.0。未推送。

### 8.7 你認為需要改規格之處
無。


---

## 9. 驗收（驗收方填）

### 9.1 第一輪驗收：**受理**（零修補回合；驗收代修一項 R-1）

交付 `4d177ae`。§8 以上一字未動；探針、`Cargo.toml`、`Cargo.lock` 未動；無重排。與驗收方參考實作逐行相同（`universe.rs` 兩處，差在註解）。

**重量**：全樹 ×3 **268 target／2585 passed／0 failed，每輪 rc=0，去耗時後 md5 三輪相同（`9c5d9166…`）**；本探針 10／10、R-1 3／3；Q-078 探針 15、`forward_spread_probe_test` 12、`a_field_outside_identity_probe_test` 8、`a_field_outside_identity_crate_test` 3、Q-077 兩支、Q-076 三支全綠；交叉編譯 0 error；conformance 162／162；§4 身分紅線全數不動（`x: 0` `31745ef0…`、`1+1` 與 `1 + 1` `f4f32e7b…`、`f` `039d0735…`、`k` `24fe01b6…`、`m` `c23f8a56…`、標準根 `7038e250…`、`add` 3／4 不建 `.oo/`、新倉 `layout=9`／`encoding=5`）。
**對抗量測**（v0.80.0 對交付逐格比對）：Q3 十根逐根複量，與 §8.4 逐字同——移動的六根答案皆變，答案不變的四根不動。探針沒碰的形狀：串接的展開（`c: { ...d }`、`d: { b: 2 }`、`r: { a: 1, ...c }`，三種文字順序皆 `{ a: 1, b: 2 }`、同一個根）；巢狀欄位裡的展開（v0.80.0 已對）；以態射結果為源的展開（`r: { a: 1, ...(f 2) }`，由 `{ a: 1 }` 變 `{ a: 1, b: 2 }`）；根上 `~` 欄位裡的展開（`~r: { a: 1, ...c }`，`t: { k: r.b }` 得 2，`_.r` 照舊 `_`）；源讀根上 `~` 欄位的展開（`r: { ...{ q: h } }` 得 `q: 3`）；分兩次 evolve 寫 `~h` 與讀它的子容器；跨三次提交（`~h` 提交、子容器提交、再 evolve 相同的 `~h` 與讀它的態射——`_.u.m 1` 得 4）；先提交源、下一次提交展開。皆合規格。單調碰撞 v0.80.0 為「evolve 照收、提交答 `Nothing to commit`」，交付為 `#conflict at ~h`。Q4 複量同交付報告。
**§8.3 成立**：I6 的「兩種順序」寫過了頭——同一個檔兩種順序、先源後展開的兩次 evolve 皆同；先展開後源的兩次 evolve 不同，與 Q4 的「不要求改」互相矛盾。錯在驗收方；交付照 Q4 處理是對的。入 Inbox 待裁。

### 9.2 驗收代修 R-1：refine 的 shadow 掃描

§8.4 Q1 #7 自陳：shadow 掃描只走歷史根的公開軸，理由是不動已記錄的位址。〔量，交付〕來源為 `{ z: 7 }` 的 CAID：`p: { z: 7 }` 的提交列入 `Shadow: 1 historical commit(s)`；`~p: { z: 7 }` 加 `k: 1` 的提交不列。**此前沒有根持有自己的 `~` 欄位**（v0.80.0 上 `~h: 3; k: 1` 與 `k: 1` 同一個根；唯一的根上 local 是系統摘要），所以現在把 local 軸加進掃描不移動任何已記錄的 `shadow` 清單；以後再改就會。驗收方代修（`ab64a04`），一處：掃描的欄位走訪接上 `cv.local`。
探針 `crates/oo/tests/a_private_never_committed_r1_probe_test.rs`（3）：s1 公開欄位列出（守衛）、r1 根上 `~` 欄位列出、s2 持有別的值的 `~` 欄位不列（守衛）。**兩極**：交付 2 綠 1 紅（r1）；代修 3／3。**變異**：只走 local 軸 ⟹ s1 紅；有 local 就標記 ⟹ s2 紅（r1 同紅）。還原後 3／3，二進位重建。

**規格收尾**（spec `local`）：`SPEC_04` §3.1 #5 補根上 `~` 欄位與 D103；`REAL_01` §1.1 補一款；`CHANGELOG` [Unreleased] 破壞性兩條（十根記帳、shadow）＋行為兩條；`ENGINE_SYNC` Q-079；`STATUS` D103 已落地；`WORK_QUEUE` Inbox 結一列、開一列。
