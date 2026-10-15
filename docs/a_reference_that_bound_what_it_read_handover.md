# 工單：一個把讀到的東西綁成 `$` 的引用（Q-089）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-089（原 Inbox「`<<_.>>` 經 `oo eval` 讀，解析到活的根——含 eval 自己的 `__eval_result` 欄位」，偵察推寬）／**不需裁定**
> 探針（已預先提交並校準）`crates/oo/tests/a_reference_that_bound_what_it_read_probe_test.rs`
> 基線：dev `52a306e`／`oo v0.90.0` ⟹ **1 綠 5 紅**；紅的都只紅在非對照格，零空洞讀數。

## 0. 判準（規格原文；本卡的每一格都由這幾句決定）

*   **`SYNTAX_07` §2 #4**：路徑的結構態 `<<p>>` 是對該**座標**的活引用——「不在建構時定值，**綁定發生於觀測時**。透過引用觀測，看到的是觀測當下的座標內容」；引用的 CAID ＝ 路徑的語法幾何。〔讀〕這裡的「綁定」是**引用綁到哪一份座標內容**，不是 `$`。
*   **`SPEC_07` §4.2.1 P1**：`$` 只在演化邊界重綁——`|>` 與態射施用。
*   **P3**：開放項觀測時由**最近包圍演化**動態綁定；無綁定觀測 ⟹ `_|_ #no_context`。**P3 判例（Q-088）**：存起來的開放項就是它的定義，讀它就在讀的地方求它；其餘照 `GUIDE_03` §11.2 的強制規則（thunk 的 `context` → 觀測方的 `$` → ⊥）。
*   **§4.2.2**：正典向量 `v: <<_.>> |> <<_.>>`、`_: v.w.x.a ;; "Logic"——此框架下 w 的 $ 綁定為根宇宙`；`_: w.x ;; _|_`；「**誰 pipe 開放項，誰的上下文即綁定其 `$`**」。

## 1. 缺陷

〔量，v0.90.0〕

*   **解參照就綁 `$`**（引擎註解稱 F1）：透過活引用觀測時，引擎把 `$` 設成**解出來的值**。正典範例因此「碰巧」對（管道輸入與解出來的值都是根），其餘都錯：
    *   `r: <<_.>>`（沒有管道）：`run --observe r.w.x`、`oo eval 'r.w.x'` ⟹ `"Logic"`——應是 ⊥ `#no_context`（`r.w.x` 就是 `w.x`）。**v0.89.0 是 ⊥：這一格是 v0.90.0 帶出去的回歸**（Q-088 把 `w` 存成語法之後，F1 碰得到它了）。
    *   `m: { s: "Logic" |> { a: $ }, w: { x: $.s } }`、`k: <<_.m>>`：`oo eval '_.k'` 印出 `w: { x: "Logic"… }`。
*   **管道穿過引用不綁輸入**：`t: { a: $.b + 1 }`；`{ b: 1 } |> <<t>>` ⟹ `{ a: ⊥, b: 1 }`；`{ b: 1 } |> <<_.t>>` ⟹ ⊥ `#conflict`（v0.89.0 是 `{ a: ⊥, b: 1 }`）。就地 `{ b: 1 } |> _.t` ⟹ `{ a: 2, b: 1 }`。
*   **管道本身被丟掉**：`v: <<_.>> |> <<_.>>` 存成光禿禿的 `<<_.>>`（`status` 印 `v: <<_.>>`）。因此只有走 F1 的觀測面答 `"Logic"`：`run --observe v.w.x.a`、`oo eval 'v.w.x.a'` 對；`oo eval '_.v.w.x.a'`、`oo eval '(<<_.>> |> <<_.>>).w.x.a'` ⟹ ⊥ `#no_context`。**同一條路徑，觀測面給兩個答案。**
*   **透過引用讀存起來的開放項**：`_.k.w` ⟹ `_`（v0.89.0 `{ x: ⊥ }`——又一格 v0.90.0 的回歸：`m` 存成語法之後，從引用讀進去沒有在讀的地方求它）。
*   **`oo eval` 把問題寫進它問的宇宙**：`x: 1` 提交後 `oo eval '<<_.>>'` 印 `{ __eval_result: { __eval_result: … } }`，`<<_.>>.__eval_result` ⟹ ⊥ `#divergent`。`run`／`test`／`repl` 沒有這個鷹架（〔量〕`run --observe` 注入的檔案本來就是視圖的一部分）。

## 2. 裁定與依據

*   **不需裁定**：§0 四句逐字決定每一格。F1 的程式註解引 `SYNTAX_07` §2.4「binding occurs at observation time」——那句說的是引用綁到哪份內容，不是 `$`；規格沒有任何一處讓解參照綁 `$`。
*   **位址**：〔量，參考實作〕在活引用上做的計算存成語法（它的意義取決於觀測當下座標裡是什麼——身分是語法，與引用、開放項同一原則）：`v: <<_.>> |> <<_.>>` `76c1a2ec…` → `5ce55aa5…`；`t: { a: $.b + 1 }` 加 `q: { b: 1 } |> <<t>>` `ad6a3a0b…` → `64f54c32…`；`k: { a: 1 } & <<_.>>` `308d7f14…` → `81ad547c…`。**光禿禿的引用不動**（`r: <<_.>>` `a92a0478…`），沒有引用的值不動（g1）。

## 3. 射程＝不變式（不是機制）

*   **I1 解參照不綁 `$`**：透過活引用讀、導航、整個印出，都不給任何開放項一個綁定；`r.w.x`（`r: <<_.>>`）與 `w.x` 一樣是 ⊥ `#no_context`，在每一個觀測面（r4、r5）。
*   **I2 管道穿過引用綁它的輸入**（P1）：`X |> <<p>>` 讀 `p` 當下的內容當轉換器，其中的開放項取 `X`：`{ b: 1 } |> <<t>>` 與 `{ b: 1 } |> _.t` 同為 `{ a: 2, b: 1 }`（r3）。
*   **I3 正典範例在每一個觀測面成立**：`_.v.w.x.a`、`v.w.x.a`、`(<<_.>> |> <<_.>>).w.x.a`、`run --observe v.w.x.a` 皆 `"Logic"`（r2）。這要求「這是一次管道、輸入是根」在存放與讀回之後仍在——光禿禿的引用表達不了它。
*   **I4 透過引用讀存起來的開放項，就在讀的地方求它**（P3 判例；r5）。
*   **I5 問問題不把問題寫進它所問的宇宙**：`oo eval` 時 `<<_.>>`、`_.` 是那個宇宙的根（r1）。
*   **I6 引用仍是活的**（`SYNTAX_07` §2 #4）：之後才提交的定義透過引用看得到（r4 的 `r.z`）；`v` 在 evolve 不被快照；`v` 全量觀測在視界誠實截斷、標準庫根上不 OOM（`stage3_probe_test`）；透過引用的觀測是結構視圖（`SYNTAX_07` §4 #6；r5 的對照）。
*   **另一半**：I1 與 I2 是同一條規則的兩半——**只有演化綁 `$`**。任何「在某處多綁一次」的修法，請用 I1 的格子檢查它有沒有綁過頭；任何「拿掉 F1」的修法，請用 I2、I3 的格子檢查管道的綁定有沒有跟著消失。
*   若你找到另一個「解參照時給了 `$`」或「管道的輸入沒傳到引用的內容」的入口，一併在射程內，並在 8.1 列出。

## 4. 紅線（今天綠，必須保持綠）

*   g1；Q-088 探針（8）、`stage2_open_term_test`（3）；`stage3_probe_test`（4）、`stage3_acceptance_test`（6）；Q-087（7）、Q-086（4）與 Q-077（12）、R-1（4）；Q-085（7）；Q-084（11）；Q-083（17）、bytes（1）、epochs（4）、union（2）、`pin_probe_test`（15）；`a_name_spread_in_probe_test`（15）；`where_there_is_no_universe`（13）；`a_private_never_committed`（含 `r4_a_one_shot_evaluation_is_inside_the_root`）。conformance **162／162**。
*   **交叉編譯** `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   身分紅線：`x: 0` 根 `b74d7758…`；`v: 1 + 1` 與 `1+1` 根 `df91a8ad…`；`f: (x -> x + 1)` 根 `258a1a08…`；`k: { a: 1, _: 9 }` 根 `420a0c9f…`；`m: (_ -> 1)` 根 `cf11d2ff…`；標準根 `c96c2937…`。`~%Math./add` (1, 2)=3、(1, 3)=4，且不建 `.oo/`。新倉 `layout=9`／`encoding=6`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** 列出引擎裡每一個設定 `$` 的地方（`context_value` 與 thunk 的 `context`）。每一處是哪一種演化（`|>`、施用、分派），還是別的？改完之後，還有沒有任何一處不是演化？
*   **Q2** 哪些定義因為在活引用上計算而存成語法？判準怎麼寫（光禿禿的引用、組合裡的引用、箭頭本體裡的引用、`&`、展開）？§2 三個根，以及 `status` 怎麼印 `v`。
*   **Q3** 一次性求值的每個入口（`oo eval`、`run --observe`、`oo test`、`oo repl`、`--universe`、`--ephemeral`）：`<<_.>>` 看到的是什麼？與 v0.90.0 比。
*   **Q4** 你的修法有沒有改變 `oo eval` 的其他讀數（效應標記、`#divergent` 與軟重入、視界的形）？〔驗收方量〕參考實作初版改成「直接求值式子」，`{ ...{ w: <寫檔> }, q: 1 }.q` 多出 `;; %effect: #io`、`{ j: k, k: j }.j` 由 `_` 變 ⊥ `#divergent`（`a_name_spread_in_probe_test` 兩支紅）——那是 evolve 那條路自己的語義，已退回。
*   **Q5** 標準庫根上 `v` 的全量觀測：耗時與記憶體，與 v0.90.0 比。

## 6. 明文不在射程內

*   **衍生值裡的開放項**（Inbox，須裁）：`g: { m: _.t }` 存的是值。
*   **`5 |> { a: $ + 1 }` ⟹ `5`**（Inbox，須裁）。
*   `=` 對 Thunk 外延、位址內涵（Inbox）。
*   規格文字與 `CHANGELOG`：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r5 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。** `rustfmt` 不得掃探針檔。
*   **驗收方預先修訂的既有測試（你不得再改）**：
    *   `stage3_probe_test::probe_evolve_stores_live_ref_not_snapshot` 原斷言「`v` 是 `Value::Ref`」——表示法；宣稱是「活的，不是 evolve 時的快照」。修訂為：Ref，或沒有綁定、尚未求值的 Thunk 皆收；Combo（快照）仍紅。
    *   `stage3_acceptance_test::stage3_pipe_with_ref_late_binding` 原斷言「直接求值管道得 `Value::Ref`」——表示法；宣稱是「後期綁定：管道被求值的當下才讀根」。修訂為：在根含 `z: 1` 的上下文裡求值，結果是 Ref，或是讀到的那個根（含 `z`）。
    兩支在 v0.90.0 與參考實作皆綠。
*   〔驗收方校準〕**兩極皆做**：
    基線 v0.90.0：**1 綠 5 紅**（逐支讀過失敗行；r1 一格、r2 兩格、r3 兩格、r4 兩格、r5 三格，各支對照皆綠）。
    參考實作（`q089-ref 6709313`，相對 v0.90.0 四檔 +91／−43）⟹ 本探針 **6／6**；同一參考實作全樹 **286 target、2693 通過、0 失敗**（= v0.90.0 的 285／2687 加本探針 6）；交叉編譯 0 error；conformance 162／162；§4 身分紅線逐根不動。
    **變異**（每次拿掉一處，量完還原；以不變式描述）：鷹架留在根裡 ⟹ r1；完整印出時解參照又綁 `$` ⟹ r5；導航時解參照又綁 `$` ⟹ r4；管道不在施用當下讀引用 ⟹ r2、r3；在活引用上的計算不存成語法 ⟹ r2；光禿禿的引用也存成語法 ⟹ g1（及 r1、r4、r5）；透過引用讀存起來的開放項不在讀的地方求 ⟹ r5；`oo eval` 透過引用時丟掉結構視圖 ⟹ r5（對照格）。
    〔驗收方自陳〕(1) r1 初版斷言 `oo eval '<<_.>>'` 的整份印出不含 `__eval_result`——在 v0.90.0 上那份印出巢到視界，子程序塞滿輸出管道後被 60 秒逾時殺掉，逾時字串裡當然沒有它：**綠得空洞**；改成小宇宙上的 `<<_.>>.__eval_result`，幾十個位元組。(2) 參考實作曾有三處死碼（提交時保留活計算、讀回後剝 thunk、以及一份與 `observe` 重複的函式），變異量出前兩者擋不住，已刪；第三者重構成共用的觀測框。(3) 參考實作初版改走「直接求值」，打壞效應標記與軟重入（見 Q4）；初版的觀測函式先強制 Ref 才判斷結構視圖，`_.k` 印成收合態——r5 的對照格由此而來。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

I1：解參照不綁 `$`。`r: <<_.>>` 之後 `r.w.x` 在 `oo eval`、`run --observe`、`_.r.w.x` 都是 ⊥ `#no_context`。r4、r5 綠。把 F1 只放回導航，只有 r4 紅（`oo eval 'r.w.x'` 與 `run --observe r.w.x` 變成 `"Logic"`；`_.r.w.x` 仍是 ⊥，根錨那條不走那個蓋章）。把 F1 只放回完整印出的強制，只有 r5 紅（`_.k` 印出 `w: { x: { %val: "Logic" a: "Logic" } }`）。

I2：管道穿過引用綁它的輸入。`{ b: 1 } |> <<t>>` 與 `{ b: 1 } |> <<_.t>>` 都是 `{ a: 2, b: 1 }`，與就地 `{ b: 1 } |> _.t` 相同。r3 綠。管道在施用當下不讀引用，r2 與 r3 一起紅：r2 的 `_.v.w.x.a` 是 `_`，其餘三個觀測面是 ⊥ `#no_context`；r3 兩條引用管道都是 `{ a: ⊥ #no_context, b: 1 }`（成因是 `#no_context`，不是 `#conflict`），就地 `_.t` 仍是 `{ a: 2, b: 1 }`。

I3：正典 `v: <<_.>> |> <<_.>>` 在 `_.v.w.x.a`、`v.w.x.a`、`(<<_.>> |> <<_.>>).w.x.a`、`run --observe v.w.x.a` 都是 `"Logic"`。r2 綠。管道被存成語法，提交前 `status` 印 `v: <<_.>> |> <<_.>>`，提交後 `inspect` 仍印這句。拿掉「活引用上的計算存成語法」，只有 r2 紅：三個從存放讀回的觀測面是 `_`，就地 `(<<_.>> |> <<_.>>).w.x.a` 仍是 `"Logic"`。r3 在這個變異下仍綠。

I4：透過引用讀存起來的開放項，在讀的地方求它。`_.k.w` 開頭是 `{ x: _|_ ;; %cause: #no_context`。r5 綠。這條強制拿掉時，r5 的 `_.k.w` 與 `_.k.w.x` 是 `_`，而且 r3 只有 `{ b: 1 } |> <<_.t>>` 變成 ⊥ `#no_context`。`{ b: 1 } |> <<t>>` 仍綠：合併會在管道的 `$` 底下強制那個光禿名字的 thunk。工單的變異句只寫 r5；多紅的這一格在這裡。這條強制留著。

I5：問問題不把問題寫進它所問的宇宙。`x: 1` 提交後 `<<_.>>.__eval_result` 是 `_`，`<<_.>>.x` 是 `1`。r1 綠。鷹架留在根裡，只有 r1 紅：`<<_.>>.__eval_result` 是 ⊥ `#divergent`。對照 `_.r.s` 仍是 `"Logic"`。

I6：引用仍是活的。之後才提交的 `z: 5`，`_.r.z` 與 `r.z` 都看得到。r4 綠。`v` 在 evolve 是沒有綁定的 thunk，不是快照。標準庫根上 `v` 的全量觀測停在 `#fuel`，沒有 OOM（`stage3_probe_test` 4／4）。r5 的對照 `s: { %val: "Logic" a: "Logic" }` 仍是結構視圖。光禿引用也存成語法，g1、r1、r4、r5 一起紅：除錯建置的 g1 根移到 `52843aba1c483e278e9af85a2a97cac3a015c0d71b288ee55b0762fa3f4cd036`（除錯位址，不是釋出釘）；`_.r.s`、`_.r.w.x`、之後提交的 `_.r.z`、r5 的路徑格都是 `_`。`oo eval` 在結構視圖之前強制 Ref，只有 r5 的對照紅：`_.k` 印成 `{ s: "Logic" w: { x: ⊥ #no_context } }`，沒有 `%val`。開放項那幾格仍綠。

另外一個入口，探針看不見、位址看得見：提交投影若不再保留「表達式含活引用、且沒有綁定」的 thunk，本探針仍 6／6，但 `q` 與 `k` 提交成 `#conflict`。保留這條。根見 8.3、8.4 Q2。

### 8.2 順手改動（逐項指名）

兩處，都沒有改行為：

1. `has_live_ref` 放在 `narrow_deferred_body` 的文件註解之前，註解仍掛在那個函式上。參考實作把新文件接在舊文件後面，D109 的註解被帶走。
2. 導航拿掉 F1 之後，`let is_ref = matches!(&v, Value::Ref(_))` 沒有人讀。這份綁定刪了。linux 釋出建置 `nlang-interpreter` lib 仍是 14 條警告。參考實作 `6709313` 留著它，lib 15 條。

沒有 `cargo fix`，沒有 `rustfmt` 探針，沒有改 `parser/src/tier.rs`、`predict_effect`、`refuse_lying_pure`。

### 8.3 工單哪裡是錯的

§2 的三個根，與工單指名的參考實作 `q089-ref 6709313` 不一致。那一版刪了提交時保留活計算的那條（工單 §7 自陳：變異量出它擋不住，已刪）。在那顆二進位上量：

*   `v: <<_.>> |> <<_.>>` 仍是 `5ce55aa56a6260f0a38232802ce05cd798e7ec8b250c873b7d15c2b22022d168`。純 thunk 的舊規則在強制結果不是 ⊥ 時會把語法留回來，所以這一根不必靠那條。
*   `t: { a: $.b + 1 }` 加 `q: { b: 1 } |> <<t>>` 提交成 `#conflict`，根 `002ef8ef8d8945d96ccf7f435a918873be57e598b4a537ba22273e0cf407e324`。提交前 `status` 仍印那句語法。
*   `k: { a: 1 } & <<_.>>` 同樣是 `#conflict`，根 `114f0042c0b8893ac6091046966abc22545b816efda125d9aaa3a466bd9b909a`。提交前 `status` 仍印語法。

強制發生在提交投影裡，讀的是這次提交要換掉的那份根，所以衝突蓋過語法。本探針 6／6 分不出這兩種存放。§2 寫的是計算存成語法，交付照 §2：兩根是 `64f54c3248b91413aa9f1d48911c2fbe61318f97e18687068a3e90ab59d50cf6` 與 `81ad547ce7d81d4c337e8d343cfc5894b350d95c68d188d05646933954be9385`。那條保留留著。

### 8.4 工單指名要你回答的問題

**Q1** 設定 `$` 的地方。

`context_value` 的賦值剩三處：

1. `eval.rs` `pipe_apply`：`call_ctx.context_value = Some(lv.clone())`。這是 `|>`。
2. `dispatch.rs` `apply_single_rule`：`call_ctx.context_value = Some(arg.clone())`。這是態射施用。
3. `lib.rs` `force` 強制 thunk：`call_ctx.context_value = effective_context`。`effective_context` 是這個 thunk 自己存著的 `context`，沒有才沿用觀測方當下的 `$`。這是把已有的綁定裝進這次求值，不是新的演化。

thunk 的 `context: Some(...)`，會寫進一個新值的：

1. `bind_open_terms`（`eval.rs`）。管道右值在合併前，把沒有綁定且有自由 `$` 的 thunk 蓋上這次輸入。右值整段是態射則不走。
2. `dispatch.rs` `read_branch` 的 `%val`，以及 `apply_single_rule` 的 `%val`。分派選中分支時蓋上被匹配的輸入（D109）。

其餘造 thunk 的地方是把當下的 `context_value` 抄進去，有就帶著、沒有就是空：組合欄位（`combo_field_from_expr`）、展開來源、引號欄、圖樣本體、位址路徑欄、頂層前向未命中（`universe.rs`）、算術答案改回配方（`recipe_instead_of_pure_answer`）、頂層展開的來源 thunk。evolve 把開放項或活引用上的計算存起來時，寫的是 `context: None`。

F1 的兩處賦值已刪：`force_recursive` 解引用時不再把 `$` 設成解出來的值；多段導航也不再。解引用剩下的是讀座標。

**Q2** 判準在 evolve、而且當下沒有 `$`：式子是開放項，或式子含活引用且最外層不是 `Structural`、也不是 `Combo`。滿足就存成那個式子的 thunk（`context` 空），不在 evolve 求它。

*   光禿 `<<path>>` 是 `Structural(Path)`，仍是 Ref。`r: <<_.>>` 根仍是 `a92a04780fd6bd7e3227a1b2b8fecff8debef28f1703290b21a8c5b748854071`。
*   組合整段排除，所以記錄裡放一個引用不會把整筆記錄存成語法。`c: { r: <<_.>> }` 根仍是 `1bf9d477cefb987c08537c8f40603d3b1e0eee6cb498ab539927e0670f282677`，欄位仍是 `r: <<_.>>`。展開寫在組合裡會被求：`u: { ...<<_.>>, a: 1 }` 兩邊都存 `{ a: 1 }`，根 `2a4b4603325ec7f7047972848cea754b1f40d9f856883ee944fc0ba3ee44340b`。
*   箭頭本體不掃描。`f: (x -> <<_.>>)` 根仍是 `40c8e316cd872f3ecfa3e28456e84e98d74a63a1423a1fc688df7e03f9e706eb`，`%code` 仍是 `<<_.>>`。
*   `&`、管道、清單、其餘二元運算都掃兩邊。

§2 三個根（v0.90.0 → 交付，皆釋出二進位）：

*   `v: <<_.>> |> <<_.>>`：`76c1a2ec07590b3eb7a618fc177c8af0fb9dea95085f7281fe6f432cbac81086` → `5ce55aa56a6260f0a38232802ce05cd798e7ec8b250c873b7d15c2b22022d168`。v0.90.0 提交前 `status` 印 `v: <<_.>>`。交付提交前印 `v: <<_.>> |> <<_.>>`。提交後 `status` 只說宇宙是靜態的；`inspect` 印 `v: <<_.>> |> <<_.>>`。
*   `t: { a: $.b + 1 }` 加 `q: { b: 1 } |> <<t>>`：`ad6a3a0b766db668c08c533562d76e80c373a00fa4b76c114464f9890d2447c4` → `64f54c3248b91413aa9f1d48911c2fbe61318f97e18687068a3e90ab59d50cf6`。`inspect` 印 `q: { b: 1 } |> <<t>>` 與 `t: { a: $["b"] + 1 }`。
*   `k: { a: 1 } & <<_.>>`：`308d7f14787b5a16a9d63c61be1fd3eff851771c148b281a39f31c9774433f2c` → `81ad547ce7d81d4c337e8d343cfc5894b350d95c68d188d05646933954be9385`。v0.90.0 的 `inspect` 收成 `{ a: 1 }`。交付留著 `k: { a: 1 } & <<_.>>`。

同一條規則、工單沒有點名的根也動了：

*   `m: <<_.>> & { a: 1 }`：`ea55cad69ba7fd20b88b82b2d2398e6d69a4ac8fffeab695db97edfe75f949f7`（收成 `{ a: 1 }`）→ `bf4e814672c4dd7960de2f66d40f440387f8e1fa62a945105db167c74f182290`（語法留下）。
*   `l: [<<_.>>, 2]`：`132e6a8bf4759d5bdb9355e7fb8536df83cd229ed6bc1a4b0001bee9727680f3` → `35cde16729695d836a276e7a2da6444d0206afb1141325f7626c5bf0a1940089`。兩邊都印 `[<<_.>>, 2]`。
*   `t` 加 `h: { q: { b: 1 } |> <<t>> }`：`ca84e693dccf911a9f80561e521caf47b8340a8567ce652628e3d7839eb9cdc7`（提交說 `#conflict at h.q`）→ `678a5d804c270839eb33191b7d568c47fca268d378bdbfdfa0c8fc6ee91dc6c1`（內層管道留下）。
*   含 `s`、`w`、`v`、`r`、`t` 的正典檔：`7c60aa257a2f079980588f48fcb6823a8d831b74f0d5545fcd718293edf2c329` → `a96f5aa39a6d231d3bb7125fc7d3d3b410c1944ca1855c7f6b9fbc1eab5b011a`。動的是 `v` 那條管道。

沒有動：`n: <<_.>>.x`（旁邊 `x: 1`）`d0b1b0d8bdff6d6f15880e55b8ab348baa5ca8c967ac41b715770f37a08f295b`，兩邊都已存 `n: <<_.>>["x"]`。開放項 `t: { a: $.b + 1 }` `3d28862809c08c1356d8ed093871cc8ea270988a4c44fccd3ec20d700044d720`。頂層來源 `...<<_.>>`：兩邊 `evolve` rc 0，`status` 與 `commit` 都說這裡沒有宇宙，沒有 HEAD。

**Q3** 宇宙已提交 `x: 1`。v0.90.0 二進位是 `oo v0.90.0-1093-g52a306e`（10876064 bytes）。交付是本輪閘門的釋出 `oo`（10899992 bytes）。

*   `oo eval`。交付：`<<_.>>.x` 是 `1`，`<<_.>>.__eval_result` 是 `_`，`_.x` 是 `1`，`_.__eval_result` 是 `_`。整份 `<<_.>>` 與整份 `_.` 都是 105 位元組，形如 `{ x: 1, ~%__nlang_system_digest: "c96c2937…" }`，裡面沒有 `__eval_result`。v0.90.0：`<<_.>>.x` 仍是 `1`，`<<_.>>.__eval_result` 是 ⊥ `#divergent`，`_.x` 是 `1`，`_.__eval_result` 是 `_`（路徑 `_.` 本來就不注入問題）。整份 `<<_.>>` 是 292667 位元組，裡面巢著 `__eval_result`。整份 `_.` 與交付同為那 105 位元組。
*   `run --observe`。兩邊相同：空檔觀察 `x` 是 `1`；檔案寫 `extra: 9` 時觀察到 `9`（這份檔案本來就是視圖的一部分）；檔案寫 `r: <<_.>>` 時 `r.x` 是 `1`、`r.extra` 是 `_`。`run --observe '<<_.>>'` 兩邊都是 `Error: Not a path`，rc 1。
*   `oo test`。檔案含 `test_x: <<_.>>.x = 1` 與 `test_no_q: <<_.>>.__eval_result = _`。兩邊都是 2 通過、0 失敗。測試本來沒有這個鷹架。
*   `oo repl`。`q: <<_.>>` 然後退出。兩邊都是 289653 位元組，沒有 `__eval_result`，`q` 自我巢到視界（活引用看得到剛剛 evolve 的那條定義）。
*   `--ephemeral`。從已提交的宇宙求 `<<_.>>.x`，兩邊都是 `_`（ephemeral 是空的）。
*   `--universe` 指到那個目錄、從別的工作目錄跑：`.x` 兩邊都是 `1`；`__eval_result` 交付是 `_`，v0.90.0 是 ⊥ `#divergent`。
*   兩個旗標一起：兩邊都是 `Error: --ephemeral and --universe name two universes; pass one`，rc 1。

**Q4** 這四個讀數交付與 v0.90.0 相同，而且 `oo eval` 沒有為它們建 `.oo/`：

*   `{ ...{ w: ~%Io./append_file ("log.txt", "a") }, q: 1 }.q` 是 `1`，日誌空。
*   `{ ...{ w: ~%Io./append_file ("log.txt", "a") }, q: w }.q` 是 `#true  ;; %effect: #io`，日誌是 `a`。
*   `{ j: k, k: j }.j` 是 `_`。
*   `{ ...{ j: k }, ...{ k: j } }.j` 是 `_`。

`oo eval` 仍是先把問題 evolve 成欄位 `__eval_result`，再把那個名字從 staged 拿掉，在同一個觀測框裡看剩下的值。拿住的 Ref 仍以 Ref 交回去，所以結構視圖還在。沒有改成直接求值式子。

**Q5** 標準庫根上，檔案是 `s: "Logic" |> { a: $ }`、`w: { x: $.s }`、`v: <<_.>> |> <<_.>>`。`/usr/bin/time -f`：

*   交付 `run --observe v`（空的觀察檔）：rc 0，wall 0.36s（time 0.35），290693 位元組，標記 `#fuel`，MAXRSS 247628 KB。開頭是巢狀的 `s: "Logic"`／`v: { s: "Logic", v: … }`。
*   交付 `oo eval '_.v'`：wall 0.36s，290693 位元組，`#fuel`，MAXRSS 247596 KB。
*   v0.90.0 `run --observe v`：wall 0.77s，896032 位元組，標記 `#max_depth`，MAXRSS 565344 KB。開頭是結構態 `s: { %val: "Logic" a: "Logic" }`。
*   v0.90.0 `oo eval '_.v'`：wall 16.69s（time 16.68），5257990 位元組，標記 `#fuel` 與 `#max_depth`，MAXRSS 6511404 KB。開頭是 `__eval_result: #blur { %cause: #fuel_exhausted, … }`，然後才是那份結構態。

`~%Math./add` (1, 2)=3、(1, 3)=4，rc 0，兩邊都不建 `.oo/`。

### 8.5 探針

`crates/oo/tests/a_reference_that_bound_what_it_read_probe_test.rs` 未改，未 `rustfmt`。`stage2_open_term_test.rs`、`stage3_probe_test.rs`、`stage3_acceptance_test.rs` 未改。釋出閘門裡本探針 6／6，2.01s。

變異都在除錯探針上，每次一處，量完還原。還原後再跑本探針 6／6，5.23s。

*   鷹架留在根裡（`oo eval` 改走 `observe` 路徑 `__eval_result`）：只有 r1。
*   完整印出時解參照又綁 `$`（F1 只放回 `force_recursive`）：只有 r5。
*   導航時解參照又綁 `$`（F1 只放回 `navigate`）：只有 r4。
*   管道在施用當下不讀引用（`pipe_apply_value` 開頭那段拿掉）：r2、r3。
*   活引用上的計算不存成語法（evolve 的 `live` 拿掉）：只有 r2。
*   光禿引用也存成語法（拿掉 `Structural` 排除）：g1、r1、r4、r5。
*   透過引用讀存起來的開放項不在讀的地方求（Ref 臂只 `resolve_path_internal`）：r5，外加 r3 的 `{ b: 1 } |> <<_.t>>`。
*   `oo eval` 透過引用時先強制再看結構視圖：只有 r5 的對照格。

提交時保留活引用這條拿掉，探針仍 6／6，`q` 與 `k` 變成 8.3 的兩個 `#conflict` 根。留著。

### 8.6 數字

1. `oo` 仍是 `0.90.0`。閘門用的釋出二進位 `/home/gali/nlang/nlang-tools/target/release/oo` 是 10899992 bytes，時間 2026-10-15 13:24。提交前的 `--version` 帶工作樹髒標記，不拿來當版本釘。
2. 三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`，各 `EXIT:0`。牆鐘 917.48s、551.55s、548.50s。第一輪含測試裝具的釋出編譯。`test result:` 281 行，通過 2693，失敗 0，`^error` 0，沒有 `BrokenPipe`，沒有 `panicked`。正規化（去掉行首空白與 `finished in …s` 之後，留下 `test result:`、`test `、`Running `、`running ` 開頭的行）各 3533 行，sha256 `b5236840ed6a7b59a71c5bed910a037da56e34137d27f3990420f107f6a01ba9`，三輪相同。2693 是 v0.90.0 的 2687 加上本探針的 6。`Running ` 行是 278；工單的 286 target 是另一種目標計數，通過數與那句相同。
3. conformance 162／162，exit 0。前後 `/home/gali/nlang/.oo/format` 都是一個位元組 `0x32`（ASCII `2`；`xxd -p` 印 `32`）。
4. `~%Math./add` (1, 2)=3、(1, 3)=4，rc 0，不建 `.oo/`。euid 1000。
5. 身分：`x: 0` 根 `b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f`。`v: 1 + 1` 與 `1+1` 根 `df91a8ad239ea8727c3a03a71eb4b0a787312b92306db4d8b124a73faaf40199`。`f: (x -> x + 1)` 根 `258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f`。`k: { a: 1, _: 9 }` 根 `420a0c9f4592b49ace8c8aacec690fd0a70953b1bd2158260e68c3acce710d9e`。`m: (_ -> 1)` 根 `cf11d2ff2e95c11ecbae5ceba00ece7c60f87c7d81149287ffebce1b551a89a6`。新倉物件裡標準根仍是 `c96c29378c9395936ea4999db37e9b6a28291b57a47d4c016ca3cdf6833fed3b`。`.oo/format` 文字是 `layout=9`，`.oo/objects.format` 文字是 `encoding=6`。
6. §4 紅線，第一輪計時，皆通過：本探針 6／6，2.01s。Q-088 8／8，4.90s。`stage2_open_term_test` 3／3，0.11s。`stage3_probe_test` 4／4，0.31s。`stage3_acceptance_test` 6／6，0.11s。Q-087 7／7，2.79s。Q-086 4／4，2.30s。Q-077 12／12，2.95s；R-1 4／4，1.27s。Q-085 7／7，2.85s。Q-084 11／11，2.08s。Q-083 17／17，6.21s；bytes 1／1，0.23s；epochs 4／4，0.31s；union 2／2，0.00s；`pin_probe_test` 15／15，2.12s。`a_name_spread_in_probe_test` 15／15，6.10s。`where_there_is_no_universe` 13／13，4.11s。`a_private_never_committed` 10／10，4.05s，其中 `r4_a_one_shot_evaluation_is_inside_the_root` 通過；R-1 3／3，0.47s。同輪也綠：`kind_tag_probe_test` 7／7，0.28s。`a_table_that_forgot_its_patterns` 17／17，2.36s；R-1 6／6，0.59s；R-2 10／10，1.62s。`forward_spread` 12／12，0.85s；`spread_collision` 25／25，0.86s；`spread_privacy` 11／11，0.37s；`blur_spread` 13／13，3.00s；`import_is_spread` 7／7，0.40s；`spread_splice` 9／9，0.15s。
7. `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` exit 0，`^error` 0，4.31s。動手之前先刪了該 target 裡四個零長度檔（兩份 cargo lock、`libnlint` 與 `liboo` 的 `.rmeta`）。該 target 上 `nlang-interpreter` lib 15 條警告。linux 上 lib 14 條。都沒有修。
8. 未 push。

### 8.7 你認為需要改規格之處

無。§0 那幾句已經寫對。F1 的註解把「綁定發生於觀測時」讀成 `$`。規格文字與 `CHANGELOG` 留給驗收方。

## 9. 驗收（驗收方填）

### 9.1 第一輪驗收：**受理**

〔量，交付 `3433237`，自己的 target〕
*   與參考實作 `q089-ref 6709313` 語義相同，**多一條**：提交投影保留「含活引用、沒有綁定」的 thunk。**§8.3 屬實**：驗收方在參考二進位上重量，`t` 加 `q: { b: 1 } |> <<t>>` 提交成 `002ef8ef…`、提交後 `q` 讀成 ⊥ `#conflict`；`k: { a: 1 } & <<_.>>` 提交成 `114f0042…`、`k.a` 讀成 ⊥。交付是 `64f54c32…`（`q` ⟹ `{ a: 2, b: 1 }`）與 `81ad547c…`（`k.a` ⟹ `1`）。**錯在驗收方**：§7 的變異只看探針，「探針擋不住」被當成死碼；§2 的根是刪之前量的。
*   8.2 兩處順手改動成立（D109 文件註解回到 `narrow_deferred_body`；`is_ref` 沒人讀）。
*   全樹 `cargo test --workspace --release --offline --no-fail-fast -- --test-threads=4`：**286 target、2693 通過、0 失敗**，rc=0。`cells89` 全格對。身分紅線、17 格、位址列與 v0.90.0 逐位元組相同。conformance 162／162，rc=0（不經管線取），`.oo/format` 前後 `32`。交叉 rc=0、`^error` 0。
*   §2 三根與 8.4 Q2 另報的四根（`m`、`l`、`h`、正典檔）逐根同；不動的五根（`r`、`c`、`f`、`u`、`n`）逐根同；驗收方另量 `x: 1` 加 `n2: <<x>> + 1` `7a7fc0a2…` → `088ccd72…`（同一條規則）。Q5 重量：`oo eval '_.v'` v0.90.0 14.0 s／6513336 KB／5257990 B，交付 0.40 s／248024 KB／290693 B。
*   **對抗補量**（照條文兩半；交付與參考同形，所以主動找兩邊同漏）：引用鏈 `r1: <<t>>` 經管道 ⟹ `{ a: 2, b: 1 }`；`5 |> <<f>>` ⟹ `6`（v0.90.0 ⊥ `#conflict`）；引用到存著的管道 `p2: <<p>>` ⟹ `p2.a` 為 `2`（v0.90.0 ⊥）；組合裡的正典管道 `c: { a: <<_.>> |> <<_.>> }` ⟹ `c.a.w.x.a` 為 `"Logic"`；恆等律 `(y -> y) _.r`、`_.r |> (y -> y)` 原樣；跨提交活性：先提交 `n3: <<_.y>> + 1`、`q: { b: 1 } |> <<_.t>>`，再提交 `y`、`t`，讀得 `5` 與 `{ a: 2, b: 1 }`。
*   **唯一與就地不同的格**：轉換器裡巢著的引用——`{ b: 1 } |> { inner: <<t>> }`、`{ b: 1 } |> <<u>>`（`u: { inner: <<t>> }`）、`{ b: 1 } |> _.h`（`h: (y -> <<t>>)`）的 `a` 是 ⊥ `#no_context`，就地 `_.t` 是 `2`。P3 寫的是「觀測時由最近包圍演化**動態**綁定」：引用是值，在被觀測的地方才讀，那裡沒有演化。照字面，不是缺陷；規格收尾寫成 `SYNTAX_07` §2 #4 判例。
*   規格收尾：`SYNTAX_07` §2 #4 判例、`GUIDE_03` §11.2 第 6 條、ENGINE_SYNC（16b 列標 F1 撤除；Q-089 條）、CHANGELOG [Unreleased]。Inbox 新開「活引用沒有任何 conformance 向量」。
