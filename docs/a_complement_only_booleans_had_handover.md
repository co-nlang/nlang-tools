# 工單：一個只對布林定義的補（Q-100）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-100（Inbox「補集模式鍵在表裡永不匹配」；「整數原子的正交補 `!0` 未定義」併入）／**依據 D116**（2026-10-22 用戶裁定「乙」）
> 探針（已預先提交並校準）`crates/oo/tests/a_complement_only_booleans_had_probe_test.rs`（約 4 秒）、`crates/interpreter/tests/a_complement_only_booleans_had_bytes_probe_test.rs`（位元組級）
> **預先修訂**：15 支既有測試 27 處，標 `AMENDED 2026-10-22 for Q-100 (D116)`——新倉 `encoding=10`、`migrate` 的停用範圍到 v0.100.0（10 支 18 處）；釘著舊補的語義（`!5`／`!#red` 是 ⊥、`!#true` 是 `#false`、`!#_|_` 是 `#_`、`!(..0)` 是 ⊥、開放 Combo 的補是欄位補的聯集）與窮舉值種類的一處（5 支 9 處）。
> 基線：dev（本工單所在提交）／`oo v0.100.0` ⟹ CLI 探針 **2 綠 6 紅**、位元組探針 **1 綠 1 紅**；conformance **182／182**。

## 0. 判準（規格原文）

*   **D116**：`!` 對全域取補——`!C` 是「不在 `C` 裡的一切」。`#true` 沒有預設型別：它是只含自己的單點；布林的「非」是 `@bool & !x`（交會得 `#false`，不是特例）。補是值；宣告不到 `encoding=10` 的倉遇到補值具名拒絕，`oo migrate` 推進並說出鎖掉的版本。
*   **`SPEC_01` §2.5**：補集運算子 `!` **必須**滿足：對合 `!!A = A`；`A & !A = _|_`；`A | !A = _`；反單調 `A <= B ⟹ !B <= !A`；德摩根兩式。差集 `A \ B = A & !B`。範例：`!#true & #false` ⟹ `#false`、`!#true & (#true | #false)` ⟹ `#false`、`@int \ @{0}` ⟹ 所有非零整數、`!(@int & 1..)`（收尾時改寫為「不是正整數的一切」）。
*   **`SYNTAX_12` §3 #3**：`cond ? a : b` ≡ `cond |> { #true: a, #false: b, _: _|_ }`——分派糖。
*   **`REAL_03` §6 值編碼表**：一個標記位元組恰對應一種值（MUST），表必須窮舉。§6.8.1：編碼還必須夠用——容器宣告編碼。
*   **先例**：D112（展開列在 `encoding` 7 以下沒有形 ⟹ 演化具名拒絕）、D114、D115（新形 ⟹ `encoding` 推進、`migrate` 說出鎖掉的版本）。

## 1. 缺陷

〔量，`v0.100.0` 標籤建置；`WORK_QUEUE` §3「開 Q-100」〕

*   〔讀〕`complement.rs` `orthocomplement` 只對 `_`、`_|_`、`#true`／`#false`、`#_|_`／`#_` 給答案。其他原子 ⟹ ⊥「Orthocomplement not defined for this atom type」；區間 ⟹ ⊥「… value type」；封閉 Combo 逐欄補、補不了的欄丟掉；開放 Combo 取欄位補的德摩根聯集（鍵丟了）。`eval.rs` `try_meet_not_range`（E3）只在字面 `x & !(區間)` 時改寫成成員否定，且部分重疊一律 ⊥。
*   **靜默錯答案**：型別是封閉 Combo，`!@int` ⟹ `{{ }}`，於是 `5 & !@int` ⟹ `5`、`"s" & !@str` ⟹ `"s"`；`{ b: 1 } & !{ a: #true }` ⟹ `#false`；`#a | !#a` ⟹ `#a`；`!!(..0)` ⟹ `_`。
*   **⊥**：`!0`、`!"a"`、`!#a`、`!(..0)`、`!(1 | 2)`；`@int & !0`（`QUICK_REFERENCE` §7 的 `@NonZero`）、`@int \ @{0}`、`!(@int & 1..)`。
*   **表鍵**：`{ @{ !(..0) }: "pos", _: "no" }` 的鍵在建表時求成 ⊥，分支永不匹配。
*   **能提交**：上面那些演化 0、提交 0，讀回 `{{ }}`、⊥、⊥ 鍵。
*   **原列的對照作廢**：箭頭形 `(x @{ … } -> 1)` 的模式是 `_`（`x @{ … }` 剖析成施用，另一列），從來沒有對過。

## 2. 依據；位址

*   **D116**（裁定已下）。
*   **位址**：補是新的值種類，標記 `0x06`：`[within] [without]`；`!c` 的 `within` 是 Top（`0xFF`）。`!0` 的原像 `80 06 FF 12 00`、`!#true` 的原像 `80 06 FF 11 04 "true"`（r7）。補先前不是值（是 ⊥、`{{ }}` 或 `#false`），**不帶補的值位址不動**——兩種宣告都不動（g1、g3）。會移動的是先前把 `!` 算成別的東西的值：`b: !#true` 先前存 `#false`，現在存補。
*   **存檔形**：`{ ~%__nlang_complement: #true within: <值> without: <值> }`（與 `__nlang_top_cause` 同一種包裝）。表層印法沒有新拼法：`!c`、`w & !c`（運算元不是原始式時加括號），讀回去交會出同一個補——**不動 fmt v2**。

## 3. 射程＝不變式（不是機制）

*   **M1 補依成員關係判**：`x & !C` 在 `C` 外是 `x`、在 `C` 內是 ⊥、部分重疊留下 `x & !C`——`C` 是區間、原子、字串、標籤、型別、Combo、聯集、另一個補，都一樣；聯集逐成員判（r1）。
*   **M2 `#true` 是單點不是型別**：`5 & !#true` ⟹ `5`；`!#true` 印成 `!#true`；布林的「非」是 `@bool & !x`；`? :` 的條件是補時取它與 `@bool` 的交會（`SYNTAX_12` 的分派）；`SPEC_01` §2.5 的範例照讀（r2）。
*   **M3 五條公設**：對合、`A & !A = ⊥`、`A | !A = _`、反單調、德摩根，以及補與補的交會與先後無關（r3）。
*   **M4 補用在表鍵與型別裡**：`@{ !(..0) }`、`@{ @int & !0 }`、`@{ !@str }`、具名的 `@NonZero: @int & !0` 都依成員關係分派（r4）。
*   **M5 寫得下、讀得回**：根上、欄位裡、表裡的補，提交後讀數與就地同；新倉宣告 `encoding=10`；`n: !0` 一個根；位址標記 `0x06`（r5、r7）。
*   **M6 宣告不到 10 的倉具名拒絕，什麼都不寫**：訊息說出 `encoding=10` 與 `oo migrate`；工作集、HEAD、宣告不動；`oo migrate --grant migrate` 推到 10 並說出 `oo v0.100.0`，之後同一筆寫入落地（r6）。
*   **另一半——不帶補的不動**：根逐位元組同（g1，兩種宣告）、位元組同（g3）；`_`、`_|_`、布林條件、L1-16、L1-17、一般的區間與型別交會照舊（g2）。
*   若你找到另一處「依語法而不是依值判補」（像 E3）、「補過不去的值種類」或「寫補而不看倉的宣告」，一併在射程內，在 8.1 列出。

## 4. 紅線（今天綠，必須保持綠）

*   g1、g2、g3；預先修訂的 15 支（`a_hidden_facet_taken_for_a_name`、`a_limit_you_cannot_catch`、`a_record_the_store_wrote_n_times`、`a_refusal_written_as_an_answer`、`a_spread_at_the_root`、`a_store_written_in_another_language`、`a_type_you_could_not_carry`、`an_axis_the_address_forgot`、`atomic_write`、`every_byte_or_none`、`nothing_here_or_nothing_you_can_see`、`memo_soundness_test`、`oml_test`、`orthomodular_test`、`range_gaps_probe_test`）；`range_eval` 族、`cond_match_test`、`bohr_test`；Q-099、Q-098、Q-097、Q-095 探針；表族（`a_table_*`）。
*   conformance **182／182**（`nlang-spec` 的 `local`；**空目錄裡跑 runner**）。
*   **交叉編譯** `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   身分紅線：`x: 0` 根 `b74d7758…`；`v: 1 + 1` 與 `1+1` 根 `df91a8ad…`；`f: (x -> x + 1)` 根 `258a1a08…`；`k: { a: 1, _: 9 }` 根 `420a0c9f…`；`m: (_ -> 1)` 根 `cf11d2ff…`；標準根 `c96c2937…`；新倉 `layout=9`／**`encoding=10`**。
*   `first_engine_that_opens_before` 的 `encoding=9` 列寫 **`v0.100.0`**、其後寫 **`v0.101.0`**（下一個切版號；`oo --version` 由標籤決定，不在程式裡）。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1 產生與用到補的入口**：列出每一處產生補或拿補做事的地方——`!`、`\`、`? :`、分派（模式與極小元）、聯集正規化、`=`、`<=`、`~%Logic` 以外的內建（`check_oml`、`~%Set` 等）、導航（`(!0).x`）、展開（`{ ...!0 }`）、型別交會、`project_for_commit`、存檔寫讀、gc 的引用掃描、`~%Engine./save`、`~%Discovery./identify`——各一格量測，說明結果是什麼、你改了沒有。
*   **Q2 跨引擎**：(a) v0.100.0 開本引擎寫的 10 倉：訊息；(b) 本引擎開 v0.100.0 寫的 9 倉——裡面存著舊 `!` 算出的 `{{ }}`、⊥、`#false`、表的 ⊥ 鍵：`log`、讀數、`gc`、遷移前後；(c) 9 倉裡存著式子 `n: !p`（配方，不是補值）：本引擎與 v0.100.0 各讀什麼。
*   **Q3 印得回**：每一種補的印出形（`oo eval` 的輸出、`status`、存檔）餵回去，是不是同一個值；寫出你量過的格子。

## 6. 明文不在射程內

*   `~%Logic`（`SPEC_09` §5.1 的 `/and`、`/or`、`/not`、`/xor`）：加模組會移動標準根，另開卡。
*   `x @{ … } -> …` 剖析成施用、模式成了 `_`（Inbox 既有列）。
*   箭頭本體裡建的表，模式鍵 `@{ @Pos }` 解不到外層的型別名、匹配一切（`{ @Pos: 1.., /f: a -> { @{ @Pos }: "y", _: "n" }, r: /f 1 0 }.r` ⟹ `"y"`，v0.100.0 同；驗收方另開 Inbox）。所以 `QUICK_REFERENCE` §7 的 `/safe_div 6 0` 今天仍走到除以零，不在探針裡。
*   `5 ? 1 : 2` 讀 `_`，而 `SYNTAX_12` 的分派糖寫 `_: _|_`：本卡只讓補作條件時取它與 `@bool` 的交會，非布林條件照舊。
*   表鍵裡的 `$`（`@{ $ & !(..0) }` ⟹ ⊥ `#no_context`）。
*   開頭不限的區間印成 `#_|_..0`（既有印法；讀得回）。
*   非分配性（`SPEC_01` §2.5.1 的 H¹／H² 障礙）對補的影響；`check_oml` 的讀數變化只回報，不規定。
*   規格文字、向量與 `CHANGELOG`：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r7 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。** `rustfmt` 不得掃探針檔與預先修訂的 27 處。
*   〔驗收方校準〕**兩極皆做**：
    基線 v0.100.0（dev 加兩支探針與預先修訂）：CLI **2 綠 6 紅**——r1 紅 12／24（區間、原子、字串、標籤、型別、Combo、聯集、差集、留下的部分）、r2 紅 3／11（`5 & !#true`、`!#true` 的印法、補作條件）、r3 紅 8／14、r4 紅 6／11（表鍵求成 ⊥）、r5 紅 7／14（提交後讀數、新倉宣告、根）、r6 紅 7／13（9 倉照收補值）；g1、g2 綠。位元組 **r7 紅、g3 綠**。全樹（release、單工）**2761 通過、27 失敗**——紅的只有兩支探針的 7 項與預先修訂的 20 支（逐支對過，沒有別的）。
    參考實作（`q100-ref 070b5f0`，相對 dev 引擎 十二個檔：`complement.rs`（補對全域、依成員關係交會、`@bool` 展開成兩個成員）、`unify.rs`（交會分支）、`eval.rs`（拿掉 E3、`? :` 的補條件、聯集的 `A | !A`）、`value.rs`（值種類、印出、各處走訪）、`bn_serial.rs`（標記 `0x06`）、`store_codec.rs`（存檔包裝與 gc 引用）、`storage.rs`（`encoding=10`、具名拒絕）、`universe.rs`（演化時拒絕）、`lib.rs`、`dispatch.rs`、`builtins/reflection.rs`（走訪）、`oo/src/main.rs`（`migrate` 說明））⟹ 兩支探針 **8／8、2／2**；全樹 **2788 通過、0 失敗**（`--release --offline --jobs 1 -- --test-threads=1`）；conformance **182／182**（空目錄，前後皆空）；盤點腳本（十七格、位址列、展開矩陣、`hunt`、`inv94`、Q-095 偵察七支、`hunt5`、`acc/q98` 兩支、`acc/q99` 兩支）與 v0.100.0 逐行同，只差十七格的第 2 格 `2 & (@int & !0)`（⊥ → `2`）；身分紅線只差新倉 `encoding=10`；Windows 交叉檢查 0 error；跨引擎：v0.100.0 開 10 倉以「encoding version 10 is not supported」具名拒絕，本引擎讀 v0.100.0 寫下的舊值照原樣（`{{ }}`、`#false`、⊥ 鍵的表——舊版寫錯的不回來），遷移前後同。
    **變異**（各被至少一格擋下）：布林留閉形 `!#true` ⟹ `#false` ⟹ r2、r5、r7；`@bool` 不展開 ⟹ r2、r5；`? :` 不與 `@bool` 交會 ⟹ r2；沒有 `A | !A = _` ⟹ r3；「整個落在被拿掉的部分」用未強制的摘要比 ⟹ r1（字面 Combo 的欄位還是 thunk）；合併被拿掉的部分改用聯集吸收 ⟹ r3（`<=` 的探測把吸收關掉）；不拒絕 ⟹ r6；`encoding` 留在 9 ⟹ r5、r6；部分重疊一律 ⊥（E3 的形）⟹ r1、r3、r4、r5；存檔寫表層語法 ⟹ r5、r6；演化時不拒絕 ⟹ r6；標記改成別的位元組 ⟹ r5、r7；`migrate` 說錯版本 ⟹ r6。**存活而從參考拿掉**：交會分支裡「另一邊是聯集就交給 `do_unify`」的守衛（`meet_complement` 自己逐成員判，拿掉後讀數全同）。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

十二個引擎檔與 `q100-ref 0d67b0d`（引擎本體同 `070b5f0`）逐位元組相同。`oo` 仍是 `0.100.0`，interpreter 仍是 `0.2.2`。

*   **補是值** `Value::Complement { within, without }`（`value.rs:295`）。`!c` 的 `within` 是 Top。`orthocomplement`（`complement.rs:15`）對 `_`、`_|_` 仍給 ⊥、`_`；對已經是補的值做對合（`!(w & !c)` 是 `!w | c`）；其餘（整數、字串、標籤、區間、型別、Combo、浮點）留下 `{ within: _, without: 該值 }`。`#true` 不再收成 `#false`。`@bool` 在交會時展開成 `#true | #false`（`bool_members`）。
*   **依成員關係交會** `meet_complement`（`complement.rs:84`），`unify.rs:637` 在區間交會之前接上。完全在外是自己、完全在內是 ⊥ `#conflict`、各佔一部分留下補。聯集逐成員。兩補相交出「被拿掉的部分」用 `removed_union`（落在另一個裡面的成員丟掉），不用聯集吸收。`eval.rs` 拿掉 E3（`try_meet_not_range`）：`x & !(區間)` 走同一個交會。
*   **聯集** `normalize_union_absorbing`（`eval.rs:933`）：`within` 是 Top、而且被拿掉的那一邊落在另一個成員裡時，`A | !A` 收成 `_`。
*   **`? :`**（`eval.rs:2270`）條件是補時，先與 `@bool` 交會再分派。非布林條件仍是 `_`。
*   **差集** `ExprKind::Diff`（`eval.rs:2325`）本來就是 `A & !B`，臂沒有改；補變成值之後 `@int \ @{0}` 印成 `{{ %kind: #type %name: "int" }} & !0`。
*   **印出** `value.rs:3739`：`!c`，或 `w & !c`。運算元不是原始式時加括號（`!(1.5)`、`!(#_|_..0)`、`!(1 | 2)`）。開頭不限的區間仍印 `#_|_..0`。
*   **位址** `bn_serial.rs:86` 標記 `0x06`，接著 `within`、`without`。`!0` 的原像 `80 06 FF 12 00`。不帶補的位元組不動（g3）。
*   **存檔** `store_codec.rs:42` 包裝 `{ ~%__nlang_complement: #true within: <值> without: <值> }`。gc 的 `refs_of_value`（`:1899`）走兩邊。
*   **宣告** `OBJECT_ENCODING_VERSION` 與 `COMPLEMENT_ENCODING` 都是 10（`storage.rs:210`、`:220`）。`put_value`／`put_root` 在寫之前 `refuse_complement`（`:939`）。演化落地前 `universe.rs` 對工作集再拒一次。訊息：`this store declares encoding=9; a complement (`!0`) has no form below encoding=10. Run `oo migrate --grant migrate``。
*   **migrate** `main.rs` `migrate_cost`（`:2355`）：目標到 10 時，鎖掉的最新引擎是 `v0.100.0`。最舊能開的引擎早於 v0.100.0 時再加一句「oo v0.100.0 opens encoding=9 and does not open encoding=10.」。`first_engine_that_opens_before`（`:2487`）encoding 9 是 `v0.100.0`，其後是 `v0.101.0`。
*   **走訪** 補加進 `project_for_commit`、`rekey_identity_names`、`refresh_thunk_effects`、`solid_combo_expansion_cost`、`first_complement`、效果／雜湊／等式。`~%Reflection` 的種類名是 `complement`。`hash_recursive_with_salt` 用前綴 `0x09`，這是 thunk／強制的雜湊，不是倉的位址。

*   **M1** `5 & !(..0)` 是 `5`，`(0 - 3) & !(..0)` 是 ⊥，`"s" & !(..0)` 是 `"s"`，`2 & !0` 是 `2`，`0 & !0` 是 ⊥，`"b" & !"a"` 是 `"b"`，`#b & !#a` 是 `#b`，`5 & !@int` 是 ⊥，`5 & !@str` 是 `5`，`{ a: #false } & !{ a: #true }` 是 `{ a: #false }`，`{ a: #true, b: 1 } & !{ a: #true }` 是 ⊥，`(1 | 2 | 3) & !2` 是 `1 | 3`，`(1 | 2) & !(..1)` 由探針綠，`(-5..5) & !(..0)` 留下 `(-5..5) & !(#_|_..0)`。
*   **M2** `5 & !#true` 是 `5`，`!#true` 印 `!#true`，`@bool & !#true` 是 `#false`，`@bool & !#false` 是 `#true`，`!#true & #false` 是 `#false`，`!#true & (#true | #false)` 是 `#false`，`(0 - 2) & !(@int & 1..)` 是 `-2`，`"s" & !(@int & 1..)` 是 `"s"`，`(!#true) ? 1 : 2` 是 `2`，`(!#false) ? 1 : 2` 是 `1`。`5 ? 1 : 2` 仍是 `_`。
*   **M3** `!!(..0) = (..0)`、`!!0 = 0`、`!!#true = #true` 都是 `#true`。`#a & !#a` 與 `(..0) & !(..0)` 是 ⊥。`#a | !#a`、`(..0) | !(..0)`、`@int | !@int` 都是 `_`。`!(..10) <= !(..0)` 是 `#true`，反過來是 `#false`。`!(1 | 2) = (!1 & !2)` 與 `(!0 & !1) = (!1 & !0)` 由探針綠。
*   **M4** `{ @{ !(..0) }: "pos", _: "no" } 12` 是 `"pos"`，`(0 - 3)` 是 `"no"`。重疊時取極小元：`{ @{ !(..0) }: "wide", @{ 1.. }: "narrow", _: "no" }` 對 `5` 是 `"narrow"`、對 `"s"` 是 `"wide"`、對 `(0 - 1)` 是 `"no"`。`{ @NonZero: @int & !0, k: 5 & @NonZero }.k` 是 `5`，對 `0` 與 `"s"` 是 ⊥。
*   **M5** `n: !0` 提交後根 `24fb6a7619d84f718af10ba9ff93f2939cf9d56796b4e0ab72ccc0d929604f42`，`_.n` 是 `!0`。存檔是 `{ n: { ~%__nlang_complement: #true within: _ without: 0 } }`。新倉 `layout=9`、`encoding=10`。位元組探針 r7 綠。
*   **M6** 預先宣告 encoding=9，先提交 `a: 1`，再演化 `n: !0` 退出碼 1，文句見上，`status` 是 `no staged changes`，HEAD 與宣告不動。`migrate --grant migrate` 退出碼 0，文句是「Migrating this store from layout=9 to layout=9 will make it unopenable by oo v0.100.0. Object encoding advances from encoding=9 to encoding=10.」。最舊能開的引擎就是 v0.100.0，所以沒有再附加「opens encoding=9 and does not open encoding=10」。從 encoding=8 遷時那句會出現，主句是 `oo v0.98.0 through v0.100.0`。`v0.101.0` 是 encoding=10 那一列，這次從 9 遷沒有把它印出來。遷移後 `_.a` 仍是 `1`，再提交 `n: !0` 讀 `5 & _.n` 是 `5`。
*   **另一半** 新倉身分：`x: 0` 根 `b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f`；`v: 1 + 1` 與 `1+1` 根 `df91a8ad239ea8727c3a03a71eb4b0a787312b92306db4d8b124a73faaf40199`；`f: (x -> x + 1)` 根 `258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f`；`k: { a: 1, _: 9 }` 根 `420a0c9f4592b49ace8c8aacec690fd0a70953b1bd2158260e68c3acce710d9e`；`m: (_ -> 1)` 根 `cf11d2ff2e95c11ecbae5ceba00ece7c60f87c7d81149287ffebce1b551a89a6`；標準根 `c96c29378c9395936ea4999db37e9b6a28291b57a47d4c016ca3cdf6833fed3b`。g1 的 `x: 0`、`b: #true`（`b5f945e2…`）、`r: 1..3`（`06254b66…`）、`t: { a: 1, b: [1, 2] }` 在宣告 9 與宣告 10 是同一條根。`!_` 是 ⊥，`!_|_` 是 `_`，`#true ? 1 : 2` 是 `1`，`5 & @int` 是 `5`。

### 8.2 順手改動（逐項指名）

沒有。

### 8.3 工單哪裡是錯的

沒有。

### 8.4 工單指名要你回答的問題

**Q1** 產生補或拿補做事的入口。舊引擎是 `/home/gali/nlang-baselines/v0.100.0-verify/target/release/oo`。

| 入口 | 讀數 | 改了 |
|---|---|---|
| `!` `orthocomplement` | `!0` 印 `!0`，`!!0` 是 `0`，`!#true` 印 `!#true`，`!5` 印 `!5`，`!1.5` 印 `!(1.5)`，`!(..0)` 印 `!(#_|_..0)`。v0.100.0 的 `!0` 是 ⊥「Orthocomplement not defined」，`!#true` 是 `#false`，`!@int` 是 `{{ }}` | 補臂改成留下補 |
| `\` `eval.rs:2325` | `3 & (@int \ @{0})` 是 `3`，`0 & (@int \ @{0})` 是 ⊥，`@int \ @{0}` 印 `{{ %kind: #type %name: "int" }} & !0` | 臂沒有改 |
| `? :` `eval.rs:2270` | `(!#true) ? 1 : 2` 是 `2`，`(!#false) ? 1 : 2` 是 `1`，`#true ? 1 : 2` 是 `1`，`5 ? 1 : 2` 是 `_` | 補條件先與 `@bool` 交會 |
| 分派模式與極小元 | `{ @{ !(..0) }: "wide", @{ 1.. }: "narrow", _: "no" }` 對 `5` 是 `"narrow"`、對 `"s"` 是 `"wide"`、對 `(0 - 1)` 是 `"no"` | `filter_minimal_branches` 沒有改；交會決定誰更細 |
| 聯集正規化 `eval.rs:933` | `#a \| !#a`、`(..0) \| !(..0)`、`@int \| !@int` 都是 `_`。v0.100.0 的 `#a \| !#a` 是 `#a`，`!!(..0)` 是 `_` | 加了 `A \| !A` |
| `=` | `!!0 = 0`、`!0 = !0`、`!(..0) = !(..0)` 都是 `#true` | 比較臂沒有改 |
| `<=` | `!(..10) <= !(..0)` 是 `#true`，反過來 `#false`，`!@int <= !1` 是 `#true` | 沒有改 |
| `~%Engine./check_oml` | `{ a: 0, b: 0 }` 兩邊都是 `#oml_valid`；`{ a: 0, b: @int }` 兩邊都是 `#oml_approximate`；`{ a: #true, b: (#true \| #false) }` 兩邊都是 `#oml_valid`；`{ a: !0, b: !0 }` 是 `#oml_valid`；`{ a: 0, b: !0 }` 是 `#oml_vacuous` | `oml.rs` 沒有改 |
| `~%Set` | `~%Set./difference { 0: [1, 2, 3], 1: [2] }` 兩邊都是 `[1, 3]`。`{ 0: [!0, 1], 1: [!0] }` 是 `[1]`，`~%Set./contains { 0: [!0], 1: !0 }` 是 `#true` | `set.rs` 沒有改 |
| 導航 `(!0).x` | `_`。v0.100.0 在取欄位之前就是 ⊥「Orthocomplement not defined」 | 導航沒有改 |
| 展開 | `{ ...!0 }` 與 `[...!0]` 兩邊都是剖析錯誤 `expected primary`。`{ ...(!0) }` 是 `{}`，`[...(!0)]` 是 `[]`。v0.100.0 的 `{ ...(!0) }` 是 ⊥：⊥ 的展開會拆掉整個容器，補不是 Combo，所以什麼都不帶 | 展開沒有改 |
| 型別交會 | `5 & !@int` 是 ⊥，`"s" & !@str` 是 ⊥，`5 & !@str` 是 `5`。v0.100.0 的 `5 & !@int` 是 `5` | 走 `meet_complement` |
| `project_for_commit` | `n: !0` 存成補的包裝，不是 thunk。`p: 0` 與 `n: !p` 強制出來是補、不是 ⊥，所以留下配方 `~%__nlang_expr: !p`（見 Q2c） | 加了補臂 |
| 存檔寫讀 | 宣告 10 寫包裝、讀回 `!0`。宣告 9 演化 `n: !0` 具名拒絕，什麼都不寫 | 加了 |
| gc | `n: !0` 提交後 `gc --grant gc`：`3 objects, 3 reachable, 0 collectable (0 bytes)`，之後 `_.n` 仍是 `!0`。`refs_of_value` 走 `within` 與 `without` | 加了走訪 |
| `~%Engine./save` | 宣告 10 的 `!0` 摘要 `fdf3f684ad111065214acf767d28278cafb59d8491e5043e7b9ce1c3deca1f7c`。沒有宇宙是 `#no_universe`。宣告 9 走 `put_value` 的拒絕，`engine.save` 把這個錯誤收成 `#conflict`（既有的臂），倉裡沒有補的物件 | 拒絕跟著 `put_value`；收成 `#conflict` 的那臂沒有改 |
| `~%Discovery./identify` | 同一個 `!0` 摘要 `fdf3f684…`，沒有宇宙也是。不看倉的宣告 | 沒有改 |

`~%Logic` 沒有動。

**Q2** 舊引擎同上。

*   **(a)** 本引擎寫 `n: !0`，根 `24fb6a7619d84f718af10ba9ff93f2939cf9d56796b4e0ab72ccc0d929604f42`，宣告 encoding=10。v0.100.0 的 `log` 與 `eval '_.n'` 都是退出碼 1：`Error: object encoding version 10 is not supported by this engine (understands encoding 1 through 9); refusing to open`。
*   **(b)** v0.100.0 寫宣告 9。本引擎 `log` 退出碼 0，`gc` 都是 `3 objects, 3 reachable, 0 collectable`，`migrate` 退出碼 0、宣告變 encoding=10、文句點名 `oo v0.100.0`。遷移前後讀數相同：
    *   `t: !@int` 根 `3c86afa0c4f3f24e280380131ceb098d0241c468a9d9eb5f5c6ba8c9ca961cf9`，讀 `{{ }}`。
    *   `n: !0` 根 `2518f993faa1feed738ae6e75c271b4d9bb98b4474314becca535a7da45e6dfc`，讀 `_|_ #conflict`。
    *   `b: !#true` 根 `ace1e233b786742c3385026f30ad9441ec76046cc5e431fdf8f402ab83d1a89b`，讀 `#false`。
    *   `k: { @{ !(..0) }: "pos", _: "no" }` 根 `c6141a677a493c9de79aad94620fec05bffea8ea6bf3ffb35f5e6e970e6d6ce4`，讀 `{ @{ _|_ #conflict }: "pos" _: "no" }`，`_.k 12` 是 `"no"`。
    *   `u: !!(..0)` 在 v0.100.0 求成 `_`，演化退出碼 0，提交是 `no universe here`，沒有可遷移的倉。
*   **(c)** 配方 `p: 0` 與 `n: !p`。v0.100.0 在提交時把 `!p` 收成 ⊥（`#conflict at n`），兩邊讀 `_.n` 都是 ⊥，倉裡沒有補的包裝，也沒有 thunk。本引擎在宣告 9 與宣告 10 都把欄位留成配方 `{ ~%__nlang_thunk: #true ~%__nlang_expr: !p ~%__nlang_closure: [] }`（強制結果是補，不是 ⊥，所以不改寫成 ⊥）。宣告 10 的根是 `dc2616009a0ef33d03f722523217ebdca1dca56a503c551703c297a619426ad5`。本引擎讀 `_.n` 是 `!0`，`_.n = !0` 是 `#true`。v0.100.0 開這個宣告 9 的倉：`log` 退出碼 0，讀 `_|_ #conflict`，訊息是「Orthocomplement not defined for this atom type」。箭頭 `f: (p -> !p)` 兩邊根都是 `a4d022657ea5ebd6fd97eb15a501edba9b48af8d2900960b0cb4d1291a153e9e`（式子，不是補值）；本引擎 `_.f 0` 是 `!0`，v0.100.0 是 ⊥。

**Q3** 印出再交回去，`印出 = 原式` 都是 `#true`。

| 原式 | `oo eval` 印出 |
|---|---|
| `!0` | `!0` |
| `!#true` | `!#true` |
| `!#false` | `!#false` |
| `!5` | `!5` |
| `!#red` | `!#red` |
| `!1.5` | `!(1.5)` |
| `!(..0)` | `!(#_|_..0)` |
| `!(#_|_)` | `!(#_|_)` |
| `!(#_)` | `!(#_)` |
| `!(1 \| 2)` | `!(1 \| 2)` |
| `!{ a: #true }` | `!{ a: #true }` |
| `!@int` | `!{{ %kind: #type %name: "int" }}` |
| `@int & !0` | `{{ %kind: #type %name: "int" }} & !0` |
| `(-5..5) & !(..0)` | `(-5..5) & !(#_|_..0)` |
| `(1 \| 2) & !1` | `2`（已經求出；與原式相等） |

提交 `n: !0`、`b: !#true`、`z: @int & !0` 之後，`_.n` 是 `!0`，`_.b` 是 `!#true`，`_.z` 是 `{{ %kind: #type %name: "int" }} & !0`。存檔與 savepoint 都是 `~%__nlang_complement` 包裝（`within: _ without: 0`、`without: #true`、`within` 為型別）。`status` 只報標準根與「no staged changes」，不重印這些欄位。

### 8.5 探針

`crates/oo/tests/a_complement_only_booleans_had_probe_test.rs` 與 `crates/interpreter/tests/a_complement_only_booleans_had_bytes_probe_test.rs` 未改，`rustfmt` 未掃它們，也未掃預先修訂的 15 支 27 處。單獨一輪 **2／2**（0.04s）與 **8／8**（5.03s），`PROBE_EXIT` 0，該輪 `cargo` `Finished` `release` 在 1m 08s。全樹第一輪裡這兩支是 **2／2**（0.04s）與 **8／8**（5.05s）。

### 8.6 數字

*   三輪皆 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`，`EXIT` 0。2026-10-22 21:16:36 牆鐘 1269.62s；21:37:46 牆鐘 921.46s；21:53:08 牆鐘 884.58s。
*   歸一化 3664 行，三輪 sha256 同為 `68326a26fa53dae9ef967644643c933e1d10b6ba57fd6c1831e3b2a7b3b4811f`。`test result:` 293 行，加總 **2788** 通過、**0** 失敗。以 `test ` 起、以 ` ok` 收的行 2787 行：`g1_a_sequential_conflict_is_still_refused` 把 `Error: Evolution Conflict…` 印在測試名那一行，`ok` 在下一行。`Running ` 290。原始位元組 295971／295816／295815。原始 sha256 前綴 `f7e699f2d9ddada2`、`95712ca98fa2e3e7`、`c9aba135a4b1dae3`。`^error` 0，`BrokenPipe` 0，`panicked` 0。
*   第一輪紅線（皆 ok）：本探針 CLI 8／8 5.05s、位元組 2／2 0.04s；Q-099 5／5 4.05s、位元組 2／2 0.00s；Q-098 5／5 8.81s；Q-097 7／7 10.29s；Q-095 3／3 24.11s；`a_limit_you_cannot_catch` 15／15 2.29s；`orthomodular_test` 18／18 0.24s；`oml_test` 10／10 0.19s；`memo_soundness_test` 2／2 0.04s；`range_gaps_probe_test` 26／26 0.60s；`range_eval_probe_test` 8／8 0.32s；`cond_match_test` 4／4 0.08s；`bohr_test` 8／8 0.16s；`a_refusal_written_as_an_answer` 17／17 4.16s；`a_spread_at_the_root` 11／11 9.37s；`a_store_written_in_another_language` 13／13 0.96s；`a_type_you_could_not_carry` 5／5 1.93s；`an_axis_the_address_forgot` 17／17 6.27s（epochs 4／4 0.31s，union 2／2 0.00s，bytes 1／1 0.23s）；`atomic_write` 8／8 1.48s；`every_byte_or_none` 14／14 1.96s；`nothing_here_or_nothing_you_can_see` 7／7 21.73s；`spread_privacy` 11／11 0.35s；`a_private_never_committed` 10／10 4.09s、R-1 3／3 0.48s；`type_super_probe_test` 14／14 0.49s。同輪還有：`a_working_set_that_is_a_set` 6／6 1.35s；`a_savepoint_that_knows_where_it_stood` 14／14 1.43s；`pin_probe_test` 15／15 2.29s；`a_table_you_cannot_read` 12／12 3.09s、R-1 4／4 1.32s；`a_table_that_stops_being_one_when_printed` 4／4 2.30s；`a_table_that_runs_every_branch_when_looked_at` 7／7 2.79s；`a_table_read_before_its_spread_landed` 11／11 2.12s；`a_table_that_forgot_its_patterns` 17／17 2.47s、R-1 6／6 0.67s、R-2 10／10 1.77s；`a_table_that_lost_its_patterns` 10／10 8.73s；`caused_top` 11／11 1.09s；`static_cycle` 17／17 0.75s；`math_union` 17／17 0.93s；`what_an_observation_leaves` 15／15 3.93s；`blur_spread` 13／13 2.85s；`forward_spread` 12／12 0.82s；`spread_collision` 25／25 0.88s；`spread_splice` 9／9 0.16s；`import_is_spread` 7／7 0.45s；`a_name_spread_in` 15／15 6.07s；`where_there_is_no_universe` 13／13 4.13s。
*   身分與新倉宣告見 8.1。
*   conformance **182／182**，空目錄 `/home/gali/tmp/q100/conf.iumb`，`CONF_EXIT` 0。跑完該目錄仍是空的。父倉 `/home/gali/nlang/.oo/format` 在閘門前、Windows 後、conformance 後都是位元組 `32`。父倉 HEAD 仍是 `311ae7dce700bf1ed31235e5503d99563dc1b243`。
*   Windows：`cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu`，`Finished release in 4.69s`，`WIN_EXIT` 0，`^error` 0。該目標 lib 15 則警告（`cargo fix` 會動 13 則）。Linux 同一 lib 14 則（`cargo fix` 12 則），見第一輪與 `/home/gali/tmp/q100/probe.log`。都沒有修。
*   `target/release/oo` 11170280 位元組，mtime 2026-10-22 21:12:35 +0800。三輪與 Windows 都沒有重新連結它。編譯時工作區是髒的，此處不報 `--version`。

### 8.7 你認為需要改規格之處

沒有。


## 9. 驗收（驗收方填）
