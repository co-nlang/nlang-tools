# 工單：一個拿在欄位裡就看不穿的引用（Q-090）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-090（原 Inbox「活引用沒有任何 conformance 向量」，偵察推寬）／**不需裁定**
> 探針（已預先提交並校準）`crates/oo/tests/a_reference_held_in_a_field_probe_test.rs`；conformance 新向量 `nlang-spec/conformance/L2/124`–`131`（spec `local`）
> 基線：dev `eedf764`／`oo v0.91.0` ⟹ 探針 **1 綠 2 紅**；conformance **167／170**（紅 L2-129、130、131）。

## 0. 判準（規格原文）

*   **`SYNTAX_07` §2 #4**：「透過引用觀測，看到的是觀測當下的座標內容」。**判例（2026-10-15，Q-089）**：解參照只是讀座標，不綁 `$`；活引用不帶框架——它是一個值，在被觀測的地方才讀，那裡沒有包圍演化就是 ⊥ `#no_context`。
*   **`SPEC_07` §4.2.1 P3**：開放項觀測時由**最近包圍演化**動態綁定。
*   **導航合成性**：`x.a.b ≡ (x.a).b`（引擎註解亦稱 F1——⊥ 與 `#blur` 兩臂早已照此實作）。

## 1. 缺陷

〔量，v0.91.0；v0.89.0、v0.90.0 同〕導航穿過**拿在欄位裡**的引用（不是頂層座標本身就是引用）一律得 `_`：

*   `t: { a: 1 }`、`c: { r: <<t>> }`：`c.r` ⟹ `{ a: 1 }`，**`c.r.a` ⟹ `_`**；`(c.r).a` ⟹ `_`；`c.r = t` ⟹ `#true` 而 **`c.r.a = 1` ⟹ `#false`**。
*   `{ inner: <<t>> }.inner.a` ⟹ `_`；`rr: <<c>>` 時 `rr.r.a` ⟹ `_`；`d: { r: <<_.>> }` 時 `d.r.t.a` ⟹ `_`。
*   開放項那一側同樣是 `_`，而不是 P3 的答案：`t: { a: $.b + 1 }`、`c: { r: <<t>> }` 時 `c.r.a` ⟹ `_`（應 ⊥ `#no_context`）、`{ b: 1 } |> { z: c.r.a }` ⟹ `{ b: 1, z: _ }`（應 `z: 2`，與就地 `{ z: _.t.a }` 相同）；`({ b: 1 } |> { inner: <<t>> }).inner.a` ⟹ `_`，而同一個值整份印出是 `{ b: 1, inner: { a: ⊥ #no_context } }`——**同一個值，路徑導向與整份印出兩個答案**。
*   對照：頂層引用 `r: <<t>>` 的 `r.a` ⟹ `1`；箭頭回傳的 `((y -> <<t>>) 0).a` ⟹ `1`。
*   〔讀，參考實作已確認〕`navigate_segments` 每一段先 `force` 目前的值；欄位存的是 `<<t>>` 的 thunk，強制它得到 `Value::Ref` 本身，沒有人再解一次，於是落進「不可導航 ⟹ ⊤ `#no_coordinate`」那一臂。

## 2. 裁定與依據

*   **不需裁定**：§0 三句逐字決定每一格。
*   **位址**：〔量，參考實作〕不動。`c: { r: <<s>> }` 一類的根逐根同（`74a10ddf…`）；正典檔 `a96f5aa3…`；身分紅線、17 格、位址列與 v0.91.0 逐位元組相同。

## 3. 射程＝不變式（不是機制）

*   **H1 透過拿著的引用導航，讀的是座標**：引用拿在欄位、清單、記錄的記錄、引用的引用裡，`x.r.a` 與 `(x.r).a` 與「先觀測 `x.r` 再取 `a`」一致；`=` 與導航同意（r1；L2-129）。
*   **H2 讀在被觀測的地方**（`SYNTAX_07` 判例、P3）：拿著的引用指向開放項時，在管道的轉換器裡被觀測就取那個管道的 `$`（`{ b: 1 } |> { z: c.r.a }` ⟹ `z: 2`；L2-131），被帶出框架後才觀測就是 ⊥ `#no_context`（L2-130）——**路徑導向與整份印出同答**（r2）。
*   **另一半——解參照不綁 `$`**（Q-089 I1）：讀穿拿著的引用不給任何開放項綁定；`d: { r: <<_.>> }` 時 `d.r.w.x` 與 `w.x` 同為 ⊥ `#no_context`（r2）。
*   **另一半——不動引用本身**：路徑停在引用上（`c.r`、`_.c.r`、`<<c>>.r`）時仍是結構視圖（`SYNTAX_07` §4 #6），印出逐字不變；裝著它的記錄的根不動（g1）。
*   若你找到另一個「拿著的引用沒被讀穿」的入口（解構、展開、分派、`~%Reflection`、`=`……），一併在射程內，並在 8.1 列出。

## 4. 紅線（今天綠，必須保持綠）

*   g1；Q-089 探針（6）；Q-088 探針（8）、`stage2_open_term_test`（3）；`stage3_probe_test`（4）、`stage3_acceptance_test`（6）；Q-087（7）、Q-086（4）與 Q-077（12）、R-1（4）；Q-085（7）；Q-084（11）；Q-083（17）、bytes（1）、epochs（4）、union（2）、`pin_probe_test`（15）；`a_name_spread_in_probe_test`（15）；`where_there_is_no_universe`（13）；`a_private_never_committed`。
*   conformance **170／170**（以 `/home/gali/nlang/nlang-spec` 的 `local` 為語料；L2-124–128 今天已綠，必須保持）。
*   **交叉編譯** `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   身分紅線：`x: 0` 根 `b74d7758…`；`v: 1 + 1` 與 `1+1` 根 `df91a8ad…`；`f: (x -> x + 1)` 根 `258a1a08…`；`k: { a: 1, _: 9 }` 根 `420a0c9f…`；`m: (_ -> 1)` 根 `cf11d2ff…`；標準根 `c96c2937…`。`~%Math./add` (1, 2)=3、(1, 3)=4，且不建 `.oo/`。新倉 `layout=9`／`encoding=6`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** 引擎裡還有哪些地方「強制一個值、拿到 `Value::Ref`、然後把它當成不是引用的東西處理」？逐處列出（導航、解構、展開、分派、`=`、`~%Reflection`、算術……），各自今天怎麼答，改完怎麼答。
*   **Q2** 你的修法在**提交投影**（`preserve_refs`）時做什麼？〔驗收方量〕參考實作的護欄 `!ctx.preserve_refs` 拿掉、或在路徑最後一段也解參照，本探針、根、跨提交活性（`c: { r: <<_.y>> }`、`k: c.r`、`k2: c.r.a` 先提交，之後才提交 `y`）全都量不出差別。你的選擇是什麼、為什麼；若你找得到一格分得出來，列出它。
*   **Q3** 引用鏈與自指：`rr: <<c>>`、`d: { r: <<_.>> }` 的 `d.r.d.r.d.r.t.a`（參考實作 `1`）。**引用環**（`a1: <<a2>>`、`a2: <<a1>>`；`e: { r: <<e2>> }`、`e2: <<e>>` 的 `e.r.x`）〔量〕v0.91.0 與參考實作皆 `_`——與別名環 `{ j: k, k: j }.j` ⟹ `_`（Q-089 Q4 已記）同答，是最大不動點，不要改。參考實作在導航處的跳數上限是 64；〔量〕80 跳的無環鏈 `c: { r: <<x1>> }`、`x1: <<x2>>`…`x80: { a: 1 }`，`c.r.a` 參考實作答 `1`、v0.91.0 答 `_`（解參照本身沿鏈讀到底，導航處的上限碰不到）。你的寫法有沒有上限、什麼情況撞得到、撞到時答什麼（不得是假的 `_`）。
*   **Q4** 標準庫根上 `v`（正典檔）的全量觀測與 `oo eval '_.v'`：耗時與記憶體，與 v0.91.0 比。

## 6. 明文不在射程內

*   **衍生值裡的開放項**（Inbox，須裁）；`k: c.r` 存的是引用還是快照屬此列與 ENGINE_SYNC 16b 的 F3，**不改**——〔量〕v0.91.0 與參考實作 `k` 都是活的。
*   `5 |> { a: $ + 1 }`（Inbox，須裁）；`=` 對 Thunk（Inbox）。
*   整份印出時記錄裡的引用印成收合態（`c` ⟹ `{ r: "Logic" }`、`c.r` ⟹ 結構視圖）——兩版同，不在本卡。
*   `f <<x>>` 是剖析錯誤（規格文法 `apply_expr` 的 `!cmp_op` 護欄；`f (<<x>>)` 可）。
*   規格文字、向量索引與 `CHANGELOG`：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1、r2 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。** `rustfmt` 不得掃探針檔。conformance 向量 L2-124–131 由規格擁有，交付不得改。
*   〔驗收方校準〕**兩極皆做**：
    基線 v0.91.0：探針 **1 綠 2 紅**（r1 紅 9 格、r2 紅 4 格；兩支的對照格皆綠）；conformance 167／170（紅 129、130、131）。v0.90.0 上新向量紅 7 支（只有正典 L2-124 綠——它是圍籬）。
    參考實作（`q090-ref 44a2a65`，相對 v0.91.0 一檔 +11）⟹ 本探針 **3／3**；全樹 **286 target、2693 通過、0 失敗**（未含本探針）；交叉 0 error；conformance **170／170**；身分紅線、17 格、位址列不動。
    **變異**：拿掉導航時的解參照 ⟹ r1、r2（＝基線）。拿掉 `preserve_refs` 護欄、或路徑最後一段也解參照 ⟹ **探針、根、跨提交活性都量不出差別**（見 Q2）——它們是等價的寫法，不是漏洞；驗收方據此沒有加格子，也沒有把護欄當死碼刪掉。
    〔驗收方自陳〕(1) 本卡原是「補向量」；偵察時 `({ b: 1 } |> { inner: <<t>> }).inner.a` 在 runner 上得 `_`，而 Q-089 驗收量的是同一個值的**整份印出**（`inner: { a: ⊥ }`）——驗收紀錄與 v0.91.0 CHANGELOG 把整份印出的讀數寫成了路徑的讀數。追下去才發現底下是一個從 v0.89.0 就在的導航缺陷，連封閉值都讀不穿。(2) 抄參考二進位做變異比較時，曾抄到上一輪變異留在 target 裡的二進位（還原原始碼後沒有重建）；`cmp` 不同才發現，已重抄重量。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

只改 `navigate_segments`。欄位裡的 `<<t>>` 存成 thunk，強制一次得到 `Value::Ref`；這圈在下一欄之前把引用再讀穿，最多 64 次。`preserve_refs` 時不讀穿。64 次之後值仍是引用，答深度視界（`DepthExceeded`），不落進 ⊤ `#no_coordinate`。

H1。`t: { a: 1 }`、`c: { r: <<t>> }`：`c.r.a`、`(c.r).a`、`c.r.a = 1` 由 `_`、`_`、`#false` 成為 `1`、`1`、`#true`。`c.r` 仍是 `{ a: 1 }`，`c.r = t` 仍是 `#true`。根仍是 `756830fc5e46327bd60b57e540d449e9c522ab76046718c9e85c9ae795f42817`。巢一層 `c.r.s.a` 由 `_` 成為 `1`，根 `6da1e5fb87c44f7209bda45742f6db153ad2a9d7bf02fbb7e944f941b858c0a1` 不動。`d.r.t.a` 與 `d.r.d.r.d.r.t.a` 由 `_` 成為 `1`，根 `076ae9693854c218d88ecd442c9221ae9b61f9e038fc452669e9d79d05ce958f` 不動。r1 綠。L2-129 在 170／170 裡。

H2。r2 綠：管道裡 `c.r.a` 取該管道的 `$`，帶出框架後是 ⊥ `#no_context`，與整份印出同答。L2-130、L2-131 在同一輪 conformance 裡綠。

解參照不綁 `$`。r2 的 `d.r.w.x` 是 ⊥ `#no_context`。拿掉這圈解參照時，這一格回到 `_`。

路徑停在引用上。g1 綠。`c.r`、80 跳鏈的 `c.r` 兩邊都是 `{ a: 1 }`。

其他入口，這顆釋出二進位對 v0.91.0 釋出二進位，逐格同或只動導航的答案：

*   清單 `l: [<<t>>]` 的 `l.0`、`l.0.a`、`(l.0).a` 兩邊已是 `{ a: 1 }`、`1`、`1`。根 `0718c87efc9b8d35dd8666563aa7b453d02963538b5ba811ac6af2bc1c9b4f25`。元素存的是引用，下一段的 `force` 本來就會讀穿。這圈沒有新修清單。
*   `{ ...c.r }.a` 與 `{ ...<<t>> }.a` 兩邊已是 `1`。
*   `f c.r` 與 `({ @{ { a: @int } }: $.a }) c.r` 兩邊已是 `1`。`f <<t>>` 兩邊都是剖析錯誤（`expected unary_expr`）。
*   `c.r = 1`（`n: 1`）、`c.r = <<t>>`、`<<c.r>> = <<t>>` 兩邊都是 `#true`。
*   `~%Reflection./keys (c.r)` 兩邊 `["a", "b"]`，`type_of` 兩邊 `#combo`，`<<t>>` 同樣。`~%Reflection./has ("a", c.r)` 兩邊都印出還沒施用完的態射。
*   `{ a: $ } |> c.r` 兩邊已是 `{ a: 1 }`。`c.r |> { a: $ }` 兩邊都是衝突，不相容的兩邊是 `<<t>>` 與 `{ a: $ }`。路徑停在引用上；管道強制的是右值。留著。
*   `c.r + 0` 兩邊都是 `_|_ #conflict`。算術只把 thunk 再強制，不把 `Value::Ref` 再讀穿；欄位強制一次就停在引用上。參考實作的 +11 也沒有動算術，探針與 conformance 沒有這一格。留著。
*   `~ c.r`、`~ <<n>>` 兩邊都是剖析錯誤（`expected unary_expr`）。沒有量到合法的補元。

引用環留著：`a1`、`a1.x`、`_.a1` 是 `_`；`e.r.x`、`e.r.r.x` 是 `_`；`_.j` 是 `_`。`e.r` 兩邊都印成一長串巢狀的 `{ r: { r: … } }`。

### 8.2 順手改動（逐項指名）

沒有。改動只有 `crates/interpreter/src/lib.rs` 的 `navigate_segments`，26 行。沒有 `cargo fix`，沒有 `rustfmt` 探針，沒有改 `parser/src/tier.rs`、`predict_effect`、`refuse_lying_pure`。linux 釋出建置 `nlang-interpreter` lib 仍是 14 條警告。

與參考實作 `44a2a65` 的差別寫在 8.4 Q3：64 次之後仍是引用時，這份答深度視界。參考實作那 11 行沒有這個回傳，剩下的引用會落進 `#no_coordinate`，印出來是 `_`。

### 8.3 工單哪裡是錯的

§2 的前綴 `74a10ddf…` 沒有重現。釋出二進位上重新提交、根沒有動的是這些：

*   `t: { a: 1 }`、`c: { r: <<t>> }`：`756830fc5e46327bd60b57e540d449e9c522ab76046718c9e85c9ae795f42817`。
*   `s: "Logic" |> { a: $ }`、`c: { r: <<s>> }`：`69c0ec0f8d990f2dd038e4bed1fe6b65ad4648954cfe60d67591e11d6ffbd65d`（拿掉護欄時寫下的倉；與有護欄的同一份來源同一根）。
*   先寫 `c: { r: <<s>> }`、接著 `s: { a: 1 }`：`6906ff6892baff0360b8161404b362e30c76d2602f4f73aa28a39d25332108b6`。
*   正典檔 `a96f5aa39a6d231d3bb7125fc7d3d3b410c1944ca1855c7f6b9fbc1eab5b011a`，`v.w.x.a` 仍是 `"Logic"`。

### 8.4 工單指名要你回答的問題

**Q1** 強制之後拿到 `Value::Ref`、再把它當成不是引用的地方。

導航是這張卡的洞，見 8.1。解構、展開、分派、`=`、`~%Reflection./keys`、`type_of` 今天就讀得穿，改完同答。算術 `c.r + 0` 與管道左值 `c.r |> { a: $ }` 今天是衝突，改完仍是衝突；兩處都留著，理由在 8.1。補元沒有合法句法可量。

`force` 自己的 Ref 臂本來就讀座標（Q-089）。頂層 `r: <<t>>` 的 `r.a`、箭頭回傳的 `((y -> <<t>>) 0).a` 今天已是 `1`。清單元素也是。

**Q2** 提交投影設 `preserve_refs`。這圈解參照包在 `if !ctx.preserve_refs` 裡，所以投影不讀穿。

三個寫法都在除錯探針上量過，然後把原始碼還原成現在這份：

*   整段拿掉：r1 紅 9 格（要 `1` 或 `#true` 的格子，含 `run --observe c.r.a`，得到 `_` 或 `#false`），r2 紅 4 格（`{ b: 1 } |> { z: c.r.a }` 得到 `z: _`；`c.r.a`、`({ b: 1 } |> { inner: <<t>> }).inner.a`、`d.r.w.x` 得到 `_`，要的是 ⊥ `#no_context`），g1 仍綠。3.35s。這是基線。
*   護欄改成永遠讀穿：探針 3／3，3.31s。當時寫下的四個根與有護欄相同：`x: 0` `b74d7758…`，正典檔 `a96f5aa3…`，`c: { r: <<t>> }` `756830fc…`，`s` 加 `c: { r: <<s>> }` `69c0ec0f…`。沒有分得出來的格子。護欄留著。
*   路徑最後一段如果停在引用也再強制一次：探針 3／3，3.41s。沒有留。這份與參考實作一樣，只在下一段之前讀穿。

跨提交，釋出對照。先提交 `c: { r: <<_.y>> }`、`k: c.r`、`k2: c.r.a`，根兩邊都是 `58bb8f9d323d2a1d2a0fb8a49274b00cded0bca9554e00069c58665d746182d6`，五個觀測都是 `_`。再提交 `y: { a: 1 }`，根兩邊都是 `57373e406bd7f35722d5e052eeabc437a66b6c67a5e0e1507a47a985a27e5ae6`。`k` 兩邊都是 `{ a: 1 }`，`k.a` 與 `_.y.a` 兩邊都是 `1`。`k2` 與 `c.r.a` 由 `_` 成為 `1`。提交物件的位址含時間戳，不拿來當內容釘。

**Q3** 上限是這圈 while：每次 `force` 交回另一個 `Value::Ref` 才算一跳，64 跳。存好的光禿引用鏈在一次 `force` 裡沿鏈讀完，導航的計數碰不到。80 跳 `c: { r: <<x1>> }` … `x80: { a: 1 }`，根兩邊 `bdf9bce76d16fc87ed0a48039e628f9f40464191005f264e131f09d7b59fbd62`，`c.r` 兩邊 `{ a: 1 }`，`c.r.a` 由 `_` 成為 `1`。

`d.r.d.r.d.r.t.a` 是 `1`。引用環與別名環仍是 `_`，見 8.1。

撞到 64 且值仍是引用時，走 `handle_resource_exhausted(DepthExceeded)`。預設策略是 blur，答 `#blur`、成因 `#max_depth_exceeded`；Strict 是 ⊥ `#max_depth_exceeded`。印出來不是 `_`。

這個出口沒有被任何提交得了的程式撞到。`id: (x -> x)` 然後 `p1: id <<p2>>` 在 evolve 就是剖析錯誤（`expected unary_expr`），5 跳與 70 跳都一樣，進不了倉。沒有把那個剖析失敗的 `_` 當成視界的讀數。

**Q4** 正典檔 `s: "Logic" |> { a: $ }`、`w: { x: $.s }`、`v: <<_.>> |> <<_.>>` 先提交進新倉（標準庫在這份根上），再觀測。`/usr/bin/time -f`。交付二進位 10900264 bytes；v0.91.0 是 `/home/gali/tmp/q090-v091-target/release/oo`，10899888 bytes，`eedf764`。

*   交付 `run empty.n --observe v`：rc 0，wall 0.36s，290693 位元組，標記 `#fuel`，MAXRSS 247952 KB。開頭是巢狀的 `s: "Logic"`、`v: { s: "Logic", v: … }`。
*   交付 `oo eval '_.v'`：rc 0，wall 0.36s，290693 位元組，`#fuel`，MAXRSS 247616 KB。開頭相同。
*   v0.91.0 `run empty.n --observe v`：wall 0.37s，290693 位元組，`#fuel`，MAXRSS 247644 KB。
*   v0.91.0 `oo eval '_.v'`：wall 0.36s，290693 位元組，`#fuel`，MAXRSS 247784 KB。

沒有倉、把正典檔直接當 `oo run canon.n --observe v`：兩邊都是 rc 0，wall 0.14s，229372 位元組，一處 `#fuel`，MAXRSS 116952 KB 與 116796 KB，都不建 `.oo/`。

### 8.5 探針

`crates/oo/tests/a_reference_held_in_a_field_probe_test.rs` 未改，未 `rustfmt`。釋出閘門裡 3／3，1.20s：`r1_a_held_reference_reads_the_coordinate`、`r2_a_held_reference_to_an_open_term_is_read_where_observed`、`g1_the_held_reference_itself_is_unchanged`。

變異見 8.4 Q2。還原後 `cmp` 與留下的那份原始碼相同，三輪閘門跑的是這份。

### 8.6 數字

1. `oo` 仍是 `0.91.0`。閘門用的釋出二進位 `/home/gali/nlang/nlang-tools/target/release/oo` 是 10900264 bytes，時間 2026-10-15 19:43:46 +0800。提交前的 `--version` 帶工作樹髒標記，不拿來當版本釘。
2. 三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`，各 `EXIT:0`。牆鐘 993.00s、571.86s、561.13s。第一輪含釋出編譯。`test result:` 282 行，通過 2696，失敗 0，`^error` 0，沒有 `BrokenPipe`，沒有 `panicked`。正規化（去掉行首空白與 `finished in …s` 之後，留下 `test result:`、`test `、`Running `、`running ` 開頭的行）各 3539 行，sha256 `c38ef4bc3cdb41a5b26d44b0d077fbf213e0c942c70e2e742974aec3527e5bea`，三輪相同。原始日誌第一輪 287729 位元組，後兩輪各 287575，不去比位元組。2696 是 v0.91.0 那句 2693 加上本探針的 3。`Running ` 行是 279。工單的 286 target 是參考實作的另一種目標計數，而且未含本探針。
3. conformance 170／170，exit 0。語料是 `/home/gali/nlang/nlang-spec` 的 `local`。前後 `/home/gali/nlang/.oo/format` 都是一個位元組 `0x32`（`xxd -p` 印 `32`）。
4. `~%Math./add` (1, 2)=3、(1, 3)=4，rc 0，不建 `.oo/`。euid 1000。
5. 身分，本輪釋出二進位：`x: 0` 根 `b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f`。`v: 1 + 1` 與 `1+1` 根 `df91a8ad239ea8727c3a03a71eb4b0a787312b92306db4d8b124a73faaf40199`。`f: (x -> x + 1)` 根 `258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f`。`k: { a: 1, _: 9 }` 根 `420a0c9f4592b49ace8c8aacec690fd0a70953b1bd2158260e68c3acce710d9e`。`m: (_ -> 1)` 根 `cf11d2ff2e95c11ecbae5ceba00ece7c60f87c7d81149287ffebce1b551a89a6`。新倉 `oo eval '_.~%__nlang_system_digest'` 是 `c96c29378c9395936ea4999db37e9b6a28291b57a47d4c016ca3cdf6833fed3b`。`.oo/format` 文字是 `layout=9`，`.oo/objects.format` 文字是 `encoding=6`。
6. §4 紅線，第一輪計時，皆通過：本探針 3／3，1.20s。Q-089 6／6，2.02s。Q-088 8／8，5.03s。`stage2_open_term_test` 3／3，0.10s。`stage3_probe_test` 4／4，0.31s。`stage3_acceptance_test` 6／6，0.12s。Q-087 7／7，2.87s。Q-086 4／4，2.34s。Q-077 12／12，3.00s；R-1 4／4，1.30s。Q-085 7／7，2.93s。Q-084 11／11，2.10s。Q-083 17／17，6.30s；bytes 1／1，0.23s；epochs 4／4，0.31s；union 2／2，0.00s；`pin_probe_test` 15／15，2.15s。`a_name_spread_in_probe_test` 15／15，6.28s。`where_there_is_no_universe` 13／13，4.11s。`a_private_never_committed` 10／10，4.14s；R-1 3／3，0.48s。同輪也綠：`kind_tag_probe_test` 7／7，0.29s。`a_table_that_forgot_its_patterns` 17／17，2.40s；R-1 6／6，0.60s；R-2 10／10，1.65s。`forward_spread` 12／12，0.88s；`spread_collision` 25／25，0.89s；`spread_privacy` 11／11，0.39s；`blur_spread` 13／13，3.08s；`import_is_spread` 7／7，0.40s；`spread_splice` 9／9，0.15s。
7. `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` exit 0，`^error` 0，4.47s。動手之前先刪了該 target 裡五個零長度檔（兩份 cargo lock、`oo` 的 build stderr、`libnlint` 與 `liboo` 的 `.rmeta`）。該 target 上 `nlang-interpreter` lib 15 條警告。linux 上 lib 14 條。都沒有修。
8. 未 push。

### 8.7 你認為需要改規格之處

無。缺陷在引擎的導航。§0 那幾句已經寫對。規格文字、向量索引與 `CHANGELOG` 留給驗收方。
