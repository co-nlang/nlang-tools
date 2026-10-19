# 工單：根上的展開，每次讀到當下的來源（Q-093）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-093（Ready 第一列）／**依據 D111**（2026-10-18，用戶「ok 甲」）與 **D112**（2026-10-19，用戶「都甲」）
> 探針（已預先提交並校準）`crates/oo/tests/a_spread_at_the_root_probe_test.rs`
> 基線：dev（本工單所在提交）／`oo v0.94.0` ⟹ 探針 **3 綠 6 紅**；conformance **178／178**。

## 0. 判準（規格原文與裁定）

*   **D111（`nlang-spec/meta/oo/STATUS.md`）**：根上的 `...c`（`c` 是宇宙裡的名字）每次讀到當下的 `c`——D110（`SPEC_10` §2.3.1）照字面適用於根層展開：`c` 之後新增的欄位、晚到的 `c`，都出現在根上，與就地重讀同答。
*   **D112**：根上讀宇宙的 `...c` 以三件事落地。
    *   **身分**：新軸位元組 9「展開列」，鍵是來源的正準拼法（`c`、`s.t`），**值為空**；按鍵排序 ⟹ 與書寫先後無關，同一個展開寫兩次冪等。只有寫了讀宇宙的根層展開的根發出這一列；其餘每一個根逐位元組不動。
    *   **編碼宣告**：`encoding` 6 → 7。較早的引擎打開新倉以「encoding 7 不支援」拒絕；舊倉（≤6）照讀照寫，寫根層展開時**具名拒絕**並指 `oo migrate --grant migrate`。
    *   **攤進來的讀法**：根同時存攤進來的讀法（`b: c.b`，Q-091 R-2 的形），每次宇宙變動（evolve 結束、提交投影）重攤；同鍵相撞在 evolve 以座標拒絕。代價明文接受：身分含衍生的讀法；`pin` 換掉 `c` 之後舊讀法留著讀到 `_`。
*   **`SPEC_03` §3.1**：展開是格的合併（同鍵相交）；**D110**：存語法不得讓衝突晚到——與既有定義相撞的寫入在演化時拒絕。

## 1. 缺陷

〔量，v0.94.0 標籤建置；`nlang-baselines/acc/q92/mx.sh` 的 `## root`〕

*   **新增**：`c: { b: 2 }`、`...c` 提交後再提交 `c: { d: 3 }` ⟹ `_.d` 是 `_`（`_.c.d` 是 `3`）。收窄對（`...s` 逐欄存 `j: s.j`，Q-091 R-2）。
*   **晚到，兩次提交**：只寫 `...c` 的提交回答 `no universe here`／`Nothing to commit`——展開不見了；之後提交 `c: { b: 2 }`，`_.b` 是 `_`。
*   **晚到，兩次 evolve 一次提交**：`_.b` 是 `_`，根 `d582ef4e…`（與只寫 `c` 同）。
*   **同一個 `...c` 寫兩次不冪等**：同檔 `c: { b: 2 }`、`...c`、`...c` 根 `8c4ba4f6…`，寫一次 `eff89b3a…`——第二次把讀法 `c.b` 與答案 `2` 相交，存成答案。
*   〔讀〕`evolve_spread`（`universe.rs`）在 evolve 當下逐欄落地；來源還沒有欄位時進 `deferred_spreads`，只活在這一次程序。`ComboVal::pending_spreads` 是 `#[serde(skip)]`；店的編碼器 `write_combo`（`store_codec.rs`）不寫它，身分的 `serialize_combo`（`bn_serial.rs`）不算它。工作集成員（`encode_injection`）與根走同一個編碼器。資料軸的鍵 `"..."` 不能借——座標可以真的叫 `"..."`（Q-036／O72）。

## 2. 依據；位址

*   **不須新裁定**：D111、D112 已裁；本卡是它們的落地。
*   **位址**（破壞性）：〔量，參考實作對 v0.94.0〕寫了讀宇宙的根層展開的根移動——`c: { b: 2 }`、`...c` `eff89b3a…` → **`5d15b3a6…`**（探針 `R0`；同檔兩序、兩次 evolve 兩序、寫兩次、晚到兩次提交、彼此看不見的兩個成員，全是這個根）；`s: { j: 5 }`、`...s` `a77a72fa…` → `126f1785…`；`c: { b: 2 }`、`..._.c` `d582ef4e…` → `e6de02ec…`（列鍵 `_.c`；讀數見 §5 Q3）。**不動**（g1 逐根釘住）：身分紅線；不讀宇宙的根層展開 `...~%Math`、`...{ j: 5 }`；座標層與巢狀展開；親手寫出的讀法 `b: c.b`、`j: s.j`；別名 `q: ~p`。17 格、位址列與 v0.94.0 逐位元組相同。
*   **新倉宣告 `encoding=7`**；`layout=9` 不動。

## 3. 射程＝不變式（不是機制）

*   **M1 根上的 `...c` 讀當下的 `c`**：`c` 之後新增的欄位（含點號寫的 `s.t.k`、私有來源 `~p`）、晚到的 `c`（下一次提交、下一次 evolve、**兩個彼此看不見的 evolve 各寫一半**）都出現在根上，與 `_.c.<k>` 同答；已有的欄位之後收窄照舊讀到收窄後的值（r1、r2、r3）。
*   **M2 一個根**：`c: { b: 2 }` 與 `...c` 不論怎麼寫——同檔兩序、兩次 evolve 兩序、`...c` 寫兩次——根都是 `R0`；之後再寫一次 `...c`，提交回答 `Nothing to commit`、根不動（r4）。`R0` 就是 D112 的身分列：軸位元組 9、鍵 `c`、無值，加上讀法 `b: c.b` 與 `c`。
*   **M3 晚到的來源與根上既有的鍵相撞，在 evolve 以鍵拒絕**：`b: 3`、`...c` 之後寫 `c: { b: 2 }` ⟹ evolve rc=1、訊息含 `#conflict at b`、不留工作集成員；新增欄位撞上既有的鍵同（`#conflict at d`）；同一個工作集兩次 evolve 同；同值（`b: 2` 遇 `c: { b: 2 }`）不是碰撞（r5）。
*   **M4 encoding 7**：新倉宣告 `encoding=7`。宣告 6 的倉：一般寫入照常；寫根層展開 rc=1、訊息指名 `encoding=7` 與 `oo migrate --grant migrate`、**不寫任何成員**；`migrate` 先說它鎖掉誰（`oo v0.85.0 through v0.94.0`）再把宣告推到 7；之後展開落地（r6）。
*   **另一半——沒有展開列的根不動**（g1）；**不讀宇宙的展開不需要列**，宣告 6 的倉收得下 `...~%Math`、`...{ j: 5 }`（g2）；收窄與同檔相撞照舊（g3）。
*   若你找到另一個重建根或成員的 `ComboVal` 而丟掉展開列的入口（`pin` 提交、`refine`、savepoint、`gc`、`diff`、觀測／`run`），一併在射程內，在 8.1 列出。

## 4. 紅線（今天綠，必須保持綠）

*   g1、g2、g3。
*   **驗收方預先修訂的既有測試**（本工單所在提交；每一處標 `AMENDED 2026-10-19 for Q-093 (D112)`；基線紅、參考綠，除非另註）：
    *   兩支守根的守衛，`s: { j: 5 }`、`...s` 的根：`a_recipe_that_met_a_write_probe_test::g2` 改為 `126f1785…`；`a_name_spread_in_probe_test::g3` 多收 `126f1785…`（兩版皆綠）。
    *   新倉宣告 `encoding=7`：`nothing_here_or_nothing_you_can_see::g1`、`atomic_write::p2`、`every_byte_or_none::r5`、`a_type_you_could_not_carry::g3`、`a_store_written_in_another_language::r6`、`an_axis_the_address_forgot::r5`。
    *   `migrate` 把宣告推到 7：`a_store_written_in_another_language::r7`、`a_refusal_written_as_an_answer::r9`、`an_axis_the_address_forgot::r7`、`an_axis_the_address_forgot::r11`（那一支走不到該分支，兩版皆綠）。
    *   `migrate` 鎖掉的範圍延到 v0.94.0：`a_savepoint_that_knows_where_it_stood::r6`、`a_signature_that_signs_the_commit::r8`、`an_axis_the_address_forgot::r9`、`a_refusal_written_as_an_answer::r11`、`names_the_boundary`（加要 `v0.94.0`）與它的校準句 `k1`（兩版皆綠）。
    *   **三支改造真實的舊倉**：`a_refusal_written_as_an_answer::r6`、`r7` 與 `what_an_observation_leaves::r9` 用現行引擎建倉、只改寫 `.oo/format` 成舊 layout——造出的是「舊 layout ＋ 現行編碼」。編碼是 7 時，沒有任何引擎同時打得開舊 layout 與 encoding 7，參考實作因此正確地回答「locks out no engine」，測試量的性質就不見了。改成同時宣告 `encoding=5`（layout ≤8 的倉最多宣告到 5）；`what_an_observation_leaves::r9` 的預期改為 `v0.64.0` 到 `v0.94.0`。
*   Q-092 探針（25，含上面那一支）；Q-091 探針（13）；Q-090（4）；Q-089（6）；Q-088（8）；`forward_spread`（12）；`spread_collision`（25）；`spread_privacy`（11）；`blur_spread`（13）；`spread_splice`（9）；`import_is_spread`（7）；`a_name_spread_in`（15）；`pin_probe_test`（15）；`what_an_observation_leaves_probe_test`（15）；`where_there_is_no_universe`（13）；`a_private_never_committed`（含 R-1）。
*   conformance **178／178**（以 `nlang-spec` 的 `local` 為語料；**在空目錄裡跑 runner**）。
*   **交叉編譯** `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   身分紅線：`x: 0` 根 `b74d7758…`；`v: 1 + 1` 與 `1+1` 根 `df91a8ad…`；`f: (x -> x + 1)` 根 `258a1a08…`；`k: { a: 1, _: 9 }` 根 `420a0c9f…`；`m: (_ -> 1)` 根 `cf11d2ff…`；標準根 `c96c2937…`。`~%Math./add` (1, 2)=3、(1, 3)=4，且不建 `.oo/`。新倉 `layout=9`／**`encoding=7`**。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1 入口清單**：列出每一個重建根或工作集成員的 `ComboVal`、因而可能丟掉展開列的地方——呼叫點、它建的是什麼、你是否改了它、各一格量測。〔驗收方〕參考實作改了：身分（`serialize_combo`）、店的編碼與解碼（`write_combo`／`combo_expr_to_value`）、合併（`unify` 的記錄交會、`replace_merge`）、提交投影與強制的兩處重建（`project_for_commit`、`force_recursive`）、工作集差量的重建（`restamp_thunk_effects`）、「有沒有東西可提交」（`staged_has_committable_content`）。**第一版參考實作在提交投影把列丟了**——成員裡有 `~%__nlang_spreads: ["c"]`，提交出來的根沒有；**第二版只寫 `...c` 的新目錄什麼都不發布**（`session_would_be_listed` 判它沒東西可提交）。沒查過的：`pin` 的提交合併（`replace_merge` 有改）、`refine`、savepoint、`gc`、`diff`。
*   **Q2 印出**：參考實作的 `oo status`、`oo inspect <root>`、`_.` 都不印展開列；`REAL_01` §1.3 (iii)「印出來的必須讀得回去」——`status` 印出的工作集讀回去少了 `...c`。你的實作印什麼？給逐字。（不在射程內，量出來即可。）
*   **Q3 `..._.c`**：〔量，v0.94.0 與參考實作同〕同一次 evolve 裡 `c: { b: 2 }`、`..._.c` ⟹ `_.b` 是 `_`（`w: _.c.b` 讀得到 `2`）——evolve 時展開的 `_.` 只看已提交的根。參考實作記下列 `_.c`，`c` 提交之後的下一次 evolve 才落地。量：同檔、兩次提交、之後一次無關的寫入，各給 `_.b` 與根。（不在射程內；驗收方另開 Inbox。）
*   **Q4 `pin`**：`c: { b: 2 }`、`...c` 提交後，`--pin` 把 `c` 換成 `{ k: 1 }`：`_.b`、`_.k`、根各是什麼？（D112 明文接受舊讀法留著讀到 `_`；量出來。）`--pin` 寫 `...c` 本身呢？
*   **Q5 代價**：每次 evolve 結束、每次提交，對每一列重攤一次。量：根上 1、10、100 個展開列（各來源 200 欄），之後一次無關寫入 `x: 1` 的 evolve 與提交秒數；沒有列的根上同一次寫入；各與 v0.94.0 同機比。〔驗收方，v0.94.0，`acc/q93/cost.sh`〕種子（一個檔寫 N 個來源與 N 個 `...cK`、提交）0 列 3.60 秒、1 列 3.70 秒、**10 列 45.7 秒**；之後 `x: 1` evolve 0.05／0.07／0.84 秒、提交 0.08／0.12／1.17 秒；**10 列時讀 `_.f9_199` 得 `#blur #fuel_exhausted`**（1 列讀得到 `199`）。100 列那一格跑到一半被主機以記憶體不足停掉，沒有讀數；參考實作未量。
*   **Q6 拼法**：`...c` 與 `..._.c` 讀同一個座標，列鍵不同、根不同（`5d15b3a6…`／`e6de02ec…`）——D110 的身分是語法，參考實作不正規化。你怎麼做？`...~p`、`...s.t`、`...(c)`（括號）各給列鍵。

## 6. 明文不在射程內

*   `..._.c` 在同一次 evolve 讀不到暫存的 `c`（§5 Q3；v0.94.0 同，驗收方開 Inbox）。
*   `...` 施於非記錄（Inbox「`...` 施於非 combo 時吸收掉整個 combo」）；不讀宇宙的根層展開照舊落成答案。
*   座標層與巢狀展開（Q-092 已對）；`pin` 留下的舊讀法（D112 明文接受）。
*   規格文字（`REAL_03` §6.2 的表與「前綴僅 8 種」、`SPEC_10` §2.3.1 括號、店的框架鍵）、向量與 `CHANGELOG`：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r6 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。** `rustfmt` 不得掃探針檔。
*   〔驗收方校準〕**兩極皆做**：
    基線 v0.94.0（dev 加本探針與 §4 的預先修訂）：本探針 **3 綠 6 紅**——r1 紅 5 格（新增的欄位全 `_`；收窄的對）、r2 紅 6 格（`...c` 單獨提交 `no universe here`）、r3 紅 4 格、r4 紅 7 格、r5 紅 8 格（全收下）、r6 紅 7 格（`encoding=6`，展開被收下又忘掉）；g1–g3 綠。§4 預先修訂的測試基線紅 17 支（`k1`、兩支守根守衛之一除外）。
    參考實作（`q093-ref`，相對 v0.94.0 引擎八檔：`value.rs`、`bn_serial.rs`、`store_codec.rs`、`unify.rs`、`universe.rs`、`lib.rs`、`storage.rs`、`main.rs`）⟹ 本探針 **9／9**；§4 修訂過的 12 個測試檔全綠；conformance **178／178**（空目錄）；身分紅線除新倉 `encoding=7` 外不動；17 格、位址列逐位元組相同。
    **參考實作的形**：`ComboVal` 多一個 `spread_rows`（來源拼法的集合）；身分在欄位之後寫軸 9 與鍵；店的框架以只在店裡出現的鍵 `~%__nlang_spreads: ["c"]` 帶它，解碼時拿掉；`evolve_spread` 在讀了宇宙、來源是路徑時記下列；evolve 結束（`evolve_files` 之後）與提交開頭，對根與暫存的每一列重攤一次，**只攤還不是這個讀法的鍵**（寫兩次因此冪等），相撞以鍵拒絕；`save_staged` 在宣告 <7 的倉上具名拒絕；`OBJECT_ENCODING_VERSION` 7；`migrate` 的代價句認得 encoding 6 由 v0.85.0 起打得開。
    **變異**（每次一處，還原後觸碰原始碼重建；本探針與 §4 的 12 個測試檔；`nlang-baselines/acc/q93/mut/run.py`）：身分不寫列 ⟹ r2、r3、r4 與兩支守根守衛；店的編碼不寫列 ⟹ r1–r5；evolve 結束不重攤 ⟹ r5；提交開頭不重攤 ⟹ r3（彼此看不見的兩個成員那格）；已是讀法的鍵照樣再攤 ⟹ r2、r3、r4 與兩支守根守衛；拿掉編碼閘門 ⟹ r6；新倉宣告 6 ⟹ 27 支；只有列的成員不算可提交 ⟹ r2、r3、r4；每一個路徑展開都記列（不看讀不讀宇宙）⟹ g1、g2；提交投影丟列 ⟹ r1–r5；記錄交會丟列 ⟹ r1–r6；工作集差量重建時丟列 ⟹ r1、r4、r5、r6。**擋不住而留下的**：(a) 強制求值重建記錄時保留列——拿掉之後全樹 **285 target、2744 通過、0 失敗**、conformance 178／178、探針外的格子（`acc/q93/hunt.sh`）逐行同：強制出來的根不會再被存回去；(b) `migrate` 代價句裡「`oo v0.85.0 through v0.94.0 open encoding=6 and do not open encoding=7.`」那一句——範圍本身已寫到 v0.94.0，這句是說明，拿掉不說錯。
    〔驗收方自陳〕(1) 第一版參考實作的根一格都沒動——列在成員裡，提交投影重建記錄時丟了（強制那一處同形，一併改了，見上 (a)）。(2) 修好之後，新目錄裡只寫 `...c` 的 evolve 什麼都不發布：「有沒有東西可提交」只看欄位。(3) 同一個 `...c` 寫兩次不冪等是 v0.94.0 就有的，第一版參考實作原樣帶著；r4 的「寫兩次」那格擋到。(4) D112 寫了「提交投影時重攤」，第一版只在 evolve 結束重攤——兩個彼此看不見的成員（r3 末格）就是為它補的。(5) 探針初稿 r1 把 `@int` 的印出形寫錯；`names_the_boundary` 的校準句 `k1` 要跟著判準改，初稿漏了。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

引擎八檔（`value.rs`、`bn_serial.rs`、`store_codec.rs`、`unify.rs`、`universe.rs`、`lib.rs`、`storage.rs`、`main.rs`）與 `q093-ref` `10d23b8` 逐位元組相同。探針未改。`oo` 仍是 `0.94.0`。`injections.rs` 未動。

*   **M1 根上的 `...c` 讀當下的 `c`。** 探針 r1、r2、r3 在閘門裡通過。晚到的欄位、點號 `s.t.k`、私有 `~p`、下一次提交、下一次 evolve、兩個彼此看不見的成員，都進這個根。已落地的欄位之後收窄，讀到的是收窄後的值。
*   **M2 一個根。** `c: { b: 2 }` 與 `...c`，同檔、對調順序、寫兩次，根都是 `R0` `5d15b3a675337d6649a2790e5965f9de6644fba7590bf89544e5436542680d43`。物件帶軸列 `~%__nlang_spreads: ["c"]`、讀法 `b` 的式子 `c.b`、以及 `c: { b: 2 }`。之後再寫一次 `...c`，提交回答 `Nothing to commit`，根停在 `R0`（r4）。
*   **M3 同鍵在 evolve 拒絕。** `b: 3`、`...c` 提交後再 evolve `c: { b: 2 }`，退出碼 1，工作集成員數不變，訊息 `Error: Evolution Conflict: #conflict at b`。同值相遇不是碰撞（r5）。
*   **M4 encoding 7。** 新倉 `layout=9`、`encoding=7`。把 `objects.format` 改回 `encoding=6` 之後 evolve `...c`，退出碼 1，成員數不變，訊息是：

```
Error: this store declares encoding=6; a spread of a universe name at the top (`...c`) is kept as a row of the root, which needs encoding=7. Run `oo migrate --grant migrate`
```

    `oo migrate --grant migrate` 退出碼 0，印出：

```
Migrating this store from layout=9 to layout=9 will make it unopenable by oo v0.85.0 through v0.94.0. Object encoding advances from encoding=6 to encoding=7.
Migrated object encoding to encoding=7.
```

    從 encoding 6、layout 9 出發時，最舊打得開的引擎是 v0.85.0，代價句就停在這兩句。之後 `encoding=7`、`layout=9`。展開接著落地（r6）。
*   **沒有展開列的根不動。** g1。`...~%Math` `42eadc23e9f62a7d379800d33ed7b716f0b2471ef86df8f923679d91e020fe14`。`...{ j: 5 }` `2ab0bdf24ff774cc7b769ba9981bb9bc53a2f46912b7c5466aa481b2b7582615`。親手寫的讀法 `b: c.b` `eff89b3a87264fdba1ac9b0ed6a16ef0423645210c93a36b2148adff9146ea85`。別名 `q: ~p` `aabf08268f5a9fddba1aa6308e5c146c188759a7e44a363586a4804bebc00d9b`。座標層 `c: { b: 2 }`、`r: { a: 1, ...c }` `f6be66e9989ce17b864992b6223061d8503f196a74b26c6311f2921d7e30ab13`。十七格與 `st_93.txt` 逐行相同。位址列與 `inb_94.txt` 逐行相同。宣告 6 的倉收得下 `...~%Math` 與 `...{ j: 5 }`（g2）。收窄與同檔相撞照舊（g3）。
*   **會重建 `ComboVal` 的入口，列都留著。**
    *   身分 `serialize_combo`（`bn_serial.rs`）：欄位之後寫軸位元組 9 與鍵，值空。空集合的位元組與以前相同。`R0` 帶列；上面那些沒有列的根停在原位址。
    *   店 `write_combo`／`combo_expr_to_value`（`store_codec.rs`）：非空時寫 `~%__nlang_spreads`，解碼時拿掉，不留在 `system`。`c: { b: 2 }`、`...c` 的成員、evolve 之後的 savepoint、提交後的根物件都有 `["c"]`。
    *   記錄交會（`unify.rs`）：`spread_rows` 取聯集。`...c` 與 `...d` 兩種書寫順序，根都是 `fbf2ee32f9607cd21f11aab0f251bdf6e80e3f81189cd815d2f24a01dc9ac372`，店裡的鍵是 `["c", "d"]`。
    *   `replace_merge`：併進進來的列。`--pin` 把已提交的 `c` 換成 `{ k: 1 }` 之後，新根仍有 `["c"]`（Q4）。
    *   提交投影 `project_for_commit`（`lib.rs`）：把列抄到新記錄。提交後的 `R0` 物件有 `["c"]`。
    *   `force_recursive`（`lib.rs`）：重建時也抄列。`oo eval` 與 `oo run` 不把強制出來的根寫回去。`oo run z.n --observe _.b`（檔案 `z: 1`、`...c`）印 `2`，根仍是 `R0`，injection 數 0。這就是 §7 擋不住的 (a)。
    *   `restamp_thunk_effects`：差量重建時抄 `session_delta.spread_rows`。成員檔有列；之後的提交仍是 `R0`。
    *   `staged_has_committable_content`：列非空就算有東西可提交。新目錄只寫 `...c`，evolve 0、提交 0，根 `b4e45fc45587a917591e85886b8bdadfea116f828688f8ed9302428dadcf1c6d`，物件是 `{ ~%__nlang_spreads: ["c"] ~%__nlang_system_digest: "c96c29378c9395936ea4999db37e9b6a28291b57a47d4c016ca3cdf6833fed3b" }`。
    *   `land_root_spreads`，`evolve_files` 結尾（`main.rs`）：上面的 `#conflict at b` 就是這裡拒絕的，成員沒有增加。
    *   `land_root_spreads`，`Universe::commit` 開頭、交會之前：兩個彼此看不見的成員（r3）在 9／9 裡通過。
    *   `save_staged`：宣告低於 7 時具名拒絕。上面 encoding 6 那一格，成員數不變。
*   **工單點名、這次另外量過的入口。**
    *   savepoint 與 injection 都走 `write_combo`。evolve 之後、提交之前，savepoint `a80f3230085908597f098b1bc3754253` 與 injection `6f89abddb60fad19951f6f3c176f72d8` 都有 `["c"]`。
    *   `refine` 不重建根的值，把當時的根雜湊抄進一筆 refine 提交。`--source` 用 `_.c` 的 CAID（`oo run --observe x`，程式 `x: ~%Engine./save _.c`），退出碼 0，印 `Refine authority: unverified` 與 `Shadow: 1 historical commit(s)…`，根仍是 `R0`，`_.b` 是 `2`。第一次用座標名 `--source c`，回答 `Error: Invalid CAID 'c': Invalid CAID format`，根也沒有動。
    *   `gc` 不改寫可達物件。`oo gc: 3 objects, 3 reachable, 0 collectable`，根仍是 `R0`，`_.b` 是 `2`，框架鍵還在。
    *   沒有一支 `oo diff` 會重建已存的 `ComboVal`。
    *   `oo run` 不寫倉（上面那格，injection 數 0）。
    *   `squash` 會重建根，列留著。`R0` 再提交 `x: 1` 之後 squash，退出碼 0，根 `061226cca9fc1110cd5fd90d7cba0be3e9c1f7e419634e4965f66ad92aeaa1a6`，`_.b` 是 `2`，`_.x` 是 `1`，新物件仍有 `["c"]`。
    *   `capture_free_fields`（`eval.rs`）從 `ComboVal::default()` 起，只抄捕捉到的欄位，不抄 `spread_rows`。它建的是閉包框架。參考實作留著這形，本輪也留著。

### 8.2 順手改動（逐項指名）

沒有。

### 8.3 工單哪裡是錯的

沒有。

### 8.4 工單指名要你回答的問題

*   **Q1。** 清單在 8.1。每一處都改了，量測也在那裡：身分與未動的根、店的三個落點（成員、savepoint、提交後的根）、記錄交會的兩列一根、`replace_merge` 的 pin、提交投影、強制求值不寫回、差量重建、只寫 `...c` 仍提交、evolve 結尾的碰撞、提交開頭的 r3、encoding 閘門。`refine`、`gc`、`run` 不重建值，根留在 `R0`。`squash` 重建之後列還在。沒有 `oo diff` 這條重建。
*   **Q2。** `c: { b: 2 }`、`...c`，evolve 退出碼 0，提交前的 `oo status`：

```
Standard root dependency: current (no committed root yet)
Staged changes:
{
  b: c.b
  c: {
    b: 2
  }
}
Total Logical Entropy: 664 bits
```

    提交退出碼 0，根 `R0`。`oo inspect` 對那筆提交：

```
CAID:   hash:sha256:v2:_:/5+Gz87p7PxHAP/fr4/c3I3zZwD//4aPisXn2RwA/7+E0OXks8gwAIDAwb2a8JWS/QEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA:9793b0b14a6747fc7f63cbfd0f858aef667d561f419936c6798adb31d9184da9
kind:   commit
root:   hash:sha256:v2:_:/5+P3KDApMdOAP/fl5Sl4NXN0gEAv6ynpvSkppzZAQC/07HpxbrfzgUAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA:5d15b3a675337d6649a2790e5965f9de6644fba7590bf89544e5436542680d43
```

    `oo eval _.`：

```
{
  b: 2
  c: {
    b: 2
  }
  ~%__nlang_system_digest: "c96c29378c9395936ea4999db37e9b6a28291b57a47d4c016ca3cdf6833fed3b"
}
```

    `status` 印出讀法 `b: c.b`。`inspect` 只印 CAID、kind、root。`_.` 把 `b` 強制成 `2`，並印系統摘要。三處都沒有印出展開列。列在磁碟上：savepoint 與 `R0` 物件都有 `~%__nlang_spreads: ["c"]`。`_.b` 是 `2`。
*   **Q3。** 同檔 `c: { b: 2 }`、`..._.c`：`_.b` 是 `_`，`w: _.c.b` 是 `2`，根 `e6de02ec1d994ce39dcb607e317abddc72aa1681c005cccb2b6b59c1b793a8bf`。展開先寫、來源後寫的同檔，`_.b` 仍是 `_`，根仍是這個。兩次提交（先 `..._.c`，再 `c: { b: 2 }`）：`_.b` 是 `_`，根仍是 `e6de02ec…`。再寫一次無關的 `x: 1`：`_.b` 是 `2`，`_.x` 是 `1`，根 `2978edb8deb64e24408b4910bf6e3f08d1467eb84503f4545fdbb761a2203756`。
*   **Q4。** `R0` 上 `--pin` 把 `c` 換成 `{ k: 1 }`：evolve 0、提交 0。`_.b` 是 `_`，`_.k` 是 `1`，`_.c` 是 `{ k: 1 }`，根 `b9806e9fb4d8266acd4de9931db6024c909c6eb13a8db5a4c9782c18a87b6a0e`。新物件仍是 thunk `b` 式子 `c.b`、`c: { k: 1 }`、`k: 1`、`~%__nlang_spreads: ["c"]`。只提交過 `c: { b: 2 }` 的倉上 `--pin` 寫 `...c` 本身：evolve 0、提交 0，根 `f3bf36fc0b458751e2f9dc80f5b51e01036c23f62218ba3ba6f3383293c9b440`，`_.b` 是 `2`。物件是 `{ b: 2 c: { b: 2 } ~%__nlang_spreads: ["c"] … }`，`b` 是答案 `2`。這根不是 `R0`。
*   **Q5。** 同一支 `acc/q93/cost.sh`，本機，2026-10-19。種子是一個檔寫 N 個來源（各 200 欄）與 N 個 `...cK` 再提交；之後 `x: 1`。
    *   v0.94.0（`v0.94.0-verify` 的 `oo`）：0 列種子 3.68 秒、rc 0／0，`x: 1` evolve 0.05 秒、提交 0.08 秒、rc 0／0，讀 `_`，最大駐留 464924 KB。1 列種子 3.79 秒、rc 0／0，evolve 0.07 秒、提交 0.13 秒、rc 0／0，讀 `199`，467192 KB。10 列種子 45.35 秒、rc 0／0，evolve 0.78 秒、提交 1.13 秒、rc 0／0，讀 `#blur { %cause: #fuel_exhausted, %caid: "hash:sha256:v1:1738c1aed6b9a9753c51f74305515e5d8082e2321e31a0ac14323dcb1e2232d4" }`，4435716 KB。
    *   本輪：0 列種子 3.48 秒、rc 0／0，evolve 0.05 秒、提交 0.07 秒、rc 0／0，讀 `_`，466496 KB。1 列種子 3.67 秒、rc 0／0，evolve 0.07 秒、提交 0.12 秒、rc 0／0，讀 `199`，468864 KB。10 列種子 52.14 秒、rc 0／0（種子本身提交成功），`x: 1` evolve 2.03 秒 rc 0、提交 0.87 秒 rc 1，stderr 是 `Error: Commit failed`，讀 `#blur { %cause: #fuel_exhausted, %caid: "hash:sha256:v1:dabca8c56a6c34f1b1cd7fb7abc1ab99451e2c442808d5b3621bd3ffebf363fc" }`，4446844 KB。1 列的那次提交是成功的。`Universe::commit` 在根與暫存的交會不是記錄時印這句。
    *   100 列：evolve 跑到 150.1 秒被時間上限停掉（rc −9，`reason=time_cap`）。當時採到的駐留約 200552 KB。沒有 evolve 跑完的秒數，也沒有提交的秒數。這次沒有再放寬上限。10 列那次尖峰已到約 4.4 GB；當時主機約 15 GiB。
*   **Q6。** 列鍵是解析器印出的路徑，`...c` 與 `..._.c` 各記各的。
    *   `...c`：鍵 `c`，根 `R0`，`_.b` 是 `2`。
    *   `..._.c`：鍵 `_.c`，根 `e6de02ec1d994ce39dcb607e317abddc72aa1681c005cccb2b6b59c1b793a8bf`。物件是 `c` 與這一列，沒有落地的 `b`。`_.b` 是 `_`。
    *   `...~p`（`~p: { b: 2 }`）：鍵 `~p`，根 `3c5703aeec2f1f37b5ff3e4c688a0b82697c902337d58d7c33eadff652f4d2fd`，`_.b` 是 `2`。
    *   `...s.t`（`s: { t: { j: 1 } }`）：鍵 `s.t`，根 `d223a908aee9c3ade8c05dc12b053a6dd95d7cef8a45e8efddea449bfd180ea7`。
    *   `...(c)`：解析之後與 `c` 同一條路徑，鍵 `c`，根 `R0`，`_.b` 是 `2`。

### 8.5 探針

`a_spread_at_the_root_probe_test` 9／9。閘門第一輪 6.93 秒。閘門之前單獨跑一次，9 通過、0 失敗，6.99 秒，退出碼 0。探針檔未改。

### 8.6 數字

*   三輪依序、`--jobs 1 --test-threads=1`、`--release --offline`：2026-10-19 13:07:37 起 1003.07 秒、13:24:20 起 622.48 秒、13:34:43 起 617.95 秒，退出碼都是 0。正規化（去掉行首空白與 `finished in …s`，只留 `test result:`、`test `、`Running `、`running ` 開頭的行）3596 行，sha256 `a759f8792b43041fd85315f3ac2d0659ed409c01e4e2192afe66154bdf8c7eb6`，三輪相同。`test result:` 285 條，合計 2744 通過、0 失敗。以 `test ` 起算的通過行 2743。`Running ` 282 條。原始日誌 291137／290983／290983 位元組，sha256 前綴 `f2530e53fea17d39`、`52eb62511a8d5485`、`56e77530d922bd24`。`^error` 0，BrokenPipe 0，panicked 0。
*   紅線（閘門第一輪，皆通過）：本探針 9／9 6.93 秒；Q-092 25／25 43.45 秒；Q-091 13／13 7.54 秒；Q-090 4／4 2.03 秒；Q-089 6／6 2.07 秒；Q-088 8／8 4.92 秒；`forward_spread` 12／12 0.79 秒；`spread_collision` 25／25 0.88 秒；`spread_privacy` 11／11 0.36 秒；`blur_spread` 13／13 2.88 秒；`spread_splice` 9／9 0.15 秒；`import_is_spread` 7／7 0.44 秒；`a_name_spread_in` 15／15 6.12 秒；`pin_probe_test` 15／15 2.22 秒；`what_an_observation_leaves` 15／15 3.95 秒；`where_there_is_no_universe` 13／13 4.11 秒；`a_private_never_committed` 10／10 4.07 秒、R-1 3／3 0.48 秒。同輪還有：Q-084 10／10 8.71 秒；`a_table_that_runs_every_branch_when_looked_at` 7／7 2.83 秒；`a_table_that_stops_being_one_when_printed` 4／4 2.28 秒；`a_table_you_cannot_read` 12／12 3.21 秒、R-1 4／4 1.38 秒；`a_tag_key_read_as_a_string` 7／7 2.91 秒；epochs 4／4 0.32 秒；union 2／2 0.00 秒；bytes 1／1 0.23 秒；`caused_top` 11／11 1.19 秒；`effect_union` 15／15 0.52 秒；`kind_tag` 7／7 0.28 秒；`math_union` 17／17 0.91 秒；`stage2_open_term_test` 3／3 0.11 秒；`stage3_acceptance_test` 6／6 0.12 秒；`stage3_probe_test` 4／4 0.33 秒；`static_cycle` 17／17 0.67 秒。
*   conformance 178／178，空目錄，`CONF_EXIT` 0。父目錄 `.oo/format` 前後都是 `32`。
*   Windows：`Finished release in 4.56s`，`WIN_EXIT` 0，`^error` 0。該目標 lib 15 則警告。linux 建置 lib 14 則（`cargo fix` 會動 12 則）。未修。
*   身分：`x: 0` 根 `b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f`；`v: 1 + 1` 與 `1+1` 根 `df91a8ad239ea8727c3a03a71eb4b0a787312b92306db4d8b124a73faaf40199`；`f: (x -> x + 1)` 根 `258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f`；`k: { a: 1, _: 9 }` 根 `420a0c9f4592b49ace8c8aacec690fd0a70953b1bd2158260e68c3acce710d9e`；`m: (_ -> 1)` 根 `cf11d2ff2e95c11ecbae5ceba00ece7c60f87c7d81149287ffebce1b551a89a6`；標準根 `c96c29378c9395936ea4999db37e9b6a28291b57a47d4c016ca3cdf6833fed3b`，物件在倉裡。`~%Math./add` (1, 2)=3、(1, 3)=4，且不建 `.oo/`。新倉 `layout=9`、`encoding=7`。
*   十七格與 `st_93.txt` 逐行相同。位址列與 `inb_94.txt` 逐行相同。移動的根：`...s` `126f178575b732d567189d84b5253b45e16f44474909ffef0395f0a876d36f3a`；`..._.c` `e6de02ec…`；`...c` 是 `R0`。
*   釋出 `oo` 11096112 位元組，mtime 2026-10-19 12:55:01 +0800。三輪閘門沒有重連這支二進位。工作區在編譯時是髒的，所以這裡不引 `--version`。

### 8.7 你認為需要改規格之處

本卡不改規格。驗收方收尾的文字還在：`REAL_03` §6.2 的前綴仍是 8 種、編碼仍寫 6；本輪的身分軸是位元組 9，店的框架鍵是 `~%__nlang_spreads`。`REAL_01` §1.3 (iii)：`status` 讀回去留著 `b: c.b`，少了 `...c`（Q2 的逐字）。`..._.c` 在同一次 evolve 讀不到暫存的 `c`（Q3），工單已標成另開的 Inbox。

## 9. 驗收（驗收方填）

### 9.1 第一輪驗收：**不受理，開 R-1**（一類；**起因在驗收方**）

交付 `85d865b` 的引擎八檔與參考實作 `q093-ref 10d23b8` 逐位元組相同——**第十四次「交付與參考同形 ⟹ 兩邊同漏」**；探針、§4 的預先修訂與 §8 以上一字未動。8.3「沒有」對；8.4 Q2–Q6 的讀數與驗收方在參考實作上量的相同。

**不受理**：**一筆不碰展開的寫入，改動了展開的鍵**（交付 8.4 Q5 量到、Q4 也量到，兩者同一個成因）。
*   〔交付 Q5，驗收方以 `acc/q93/cost2.sh` 重現〕根上 7 個展開、各 200 欄，種子提交成功；之後一筆 `x: 1`，**evolve rc 0、提交 rc 1 `Commit failed`**。v0.94.0 同一格提交成功。
*   〔量〕`--pin` 寫 `...c` 之後根上的 `b` 是答案 `2`；之後一筆 `x: 1`，它的工作集成員裡多了 `b: c.b`，根 `c8f03993…`；`x: 1` 先寫、`...c` 後 pin，根 `a75ab440…`。同形更便宜的一格：`b: 2`、`...c`、`c: { b: 2 }` 一次提交後再寫 `x: 1` ⟹ `c8f03993…`；四行一次提交 ⟹ `a75ab440…`。v0.94.0 三種寫法同根。
*   **成因**：重攤只跳過「已經是這個讀法」的鍵，把**其他任何值**當成還沒攤進來——展開在同一次寫入遇上的答案、`--pin` 寫下的值、提交時存成的視界——於是每一次無關的 evolve 都再攤一次讀法，提交時讀法與那個值交會。7×200 那格：種子根裡有 408 個鍵存的是 `#blur #fuel_exhausted`（見下），讀法與它交會不是記錄，提交失敗。
*   **起因在驗收方**：D112 寫「每次宇宙變動時重攤」，§3 的射程只寫了「新增、晚到在重攤時出現」與「相撞在 evolve 拒絕」，沒寫另一半——**重攤不得改動根上已經有值的鍵**；參考實作照機制判，交付與它同形。

**另一個發現（不在本卡；另開 Inbox，interrupt-candidate）**：**大的根在提交時把視界寫進歷史**。〔量〕7 個展開各 200 欄，v0.93.0 的種子根存 1400 個讀法，**v0.94.0 只存 992 個，408 個存成 `#blur #fuel_exhausted`**，讀 `_.f6_199` 得視界——Q-092 R-1（`545e4e5`）帶進來的（Q-092 第一次交付、參考實作、`9275acc` 皆 0；R-1 之後每一輪 408）。同類而更早的：**v0.93.0 的 1400 個別名 `bK: aK` 提交後 1201 個存成視界**，種子 35 分鐘；3000 個別名那格吃到 10 GB 以上，驗收方停掉。

### 9.2 修補回合 R-1（交付方做；用戶裁「甲」，2026-10-19）

*   **R-1 射程（不變式；M1–M4 照舊）**：
    *   **R1-M1 一筆不碰展開的寫入，不改動展開的鍵**：根上一個展開的鍵已經有值、而那不是這個展開的讀法時（展開在同一次寫入遇上的答案、`--pin` 寫下的值、提交時存成的視界），重攤只檢查它與來源一致，不再攤；一筆無關的寫入與它的先後不影響根（**r7**）。引擎自己造出的根上，一筆無關寫入 evolve 收下就提交收下（7×200 那格）。
    *   **另一半**：不一致照舊在 evolve 以鍵拒絕（r5 一字不動）；新增與晚到照舊在重攤時出現（r1–r3）。
*   **明文不在射程內**：提交投影把視界寫進歷史（另開 Inbox）；`..._.c`（§5 Q3）。
*   **探針**（驗收方追加，本提交；r1–r6、g1–g3 一字不動）：r7。
*   **參考實作** `q093-r1ref 8ddb0f9`（在 `85d865b` 上，`universe.rs` 一處）：重攤時，鍵上的值不是這個讀法者，以 `unify` 與來源給的值相交；不是 ⊥ 就從這次重攤拿掉，是 ⊥ 就留著，由後面的檢查以鍵拒絕。
*   **必答**：
    *   **R1-Q1 鍵上的值**：列出重攤時會遇到的「不是這個讀法」的值的種類，各一格：根與讀數。
    *   **R1-Q2 規模**：7×200、10×200 之後一筆 `x: 1` 的 evolve 與提交（退出碼、秒數、最大駐留），與 v0.94.0 同機比。

### 9.3 R-1 參考實作的數字（驗收方）

`q093-r1ref 8ddb0f9` ⟹ 本探針 **10／10**（交付 9 綠、r7 紅 2 格：`--pin` 之後與之後再寫 `x: 1` 的根 `c8f03993…`）；7×200 之後 `x: 1` 提交 rc 0（讀 `_.f6_199` 仍是視界——那是另開的那一列）；展開矩陣、探針外的格子（`acc/q93/hunt.sh`）與交付逐行相同；`--pin` 兩種先後同根 `a75ab440…`。全樹 **285 target、2745 通過、0 失敗**；conformance **178／178**（空目錄，前後皆空）；17 格、位址列與 v0.94.0 逐位元組同；身分紅線與交付同（新倉 `encoding=7`）。

**變異**：拿掉這一處 ⟹ r7（即交付）；鍵上有值就不攤、不檢查一致 ⟹ r5（晚到的相撞被收下）。

## 9.4 修補回報（交付方填）

### 9.4.1 射程

`universe.rs` 與 `q093-r1ref` `8ddb0f9` 逐位元組相同。探針未改。`oo` 仍是 `0.94.0`。

*   **R1-M1 不碰展開的寫入，留下展開的鍵。** 鍵上已有的值若與來源一致，重攤把它從這一次拿掉。
    *   同一次寫入遇上的答案。`b: 2`、`...c`、`c: { b: 2 }` 提交後，根 `f3bf36fc0b458751e2f9dc80f5b51e01036c23f62218ba3ba6f3383293c9b440`，`_.b` 是 `2`。再提交 `x: 1`，根 `a75ab440b717bcc169d3b59b7c6d272882f674ba7d47c29bce654e995ee17888`，`_.b` 仍是 `2`，`_.x` 是 `1`。四行一次提交、先寫 `x` 再寫 `b` 與 `...c`，都是這個根。
    *   `--pin` 寫下的值。只提交 `c: { b: 2 }` 之後 `--pin` 寫 `...c`，根也是 `f3bf36fc…`，`_.b` 是 `2`。之後 `x: 1`，與先寫 `x` 再 pin，根都是 `a75ab440…`，`_.b` 是 `2`，`_.x` 是 `1`。
    *   提交時存成的視界。7×200 與 10×200 的種子提交成功；之後 `x: 1` evolve 與提交退出碼都是 0。`_.f6_199`、`_.f9_199` 在這筆之前與之後都是 `#blur #fuel_exhausted`。讀數在 9.4.4。
*   **另一半。** `b: 3`、`...c` 之後 evolve `c: { b: 2 }`，退出碼 1，訊息 `Error: Evolution Conflict: #conflict at b`，工作集成員數 0。r5 在 10／10 裡通過。r1–r3 通過：新增與晚到仍在重攤時出現。

### 9.4.2 順手改動

沒有。

### 9.4.3 工單哪裡是錯的

沒有。

### 9.4.4 必答

*   **R1-Q1 鍵上的值。** 重攤會遇上三種「不是這個讀法」的值。與來源 `unify` 之後不是 ⊥ 的，這一次不攤；是 ⊥ 的留下，由後面的檢查以鍵拒絕。
    *   答案。`b: 2` 與 `...c`、`c: { b: 2 }` 一次提交，根 `f3bf36fc…`，`_.b` 是 `2`。再寫 `x: 1`，四行一次提交，或 `x` 先寫，根都是 `a75ab440…`，`_.b` 是 `2`。
    *   pin。`--pin` 的 `...c` 先寫或後寫，與上面那筆 `x: 1` 合在一起，根都是 `a75ab440…`，`_.b` 是 `2`，`_.x` 是 `1`。pin 當時、還沒寫 `x` 的根是 `f3bf36fc…`。
    *   視界。7 列種子根 `bf1c642b2ca6e43ed96fd72bb161e422f5cefb8cc56ff5ef8d5cead9b7ae2ace`，`_.f6_199` 是 `#blur { %cause: #fuel_exhausted, %caid: "hash:sha256:v1:a8e14d3b1c6a39b53ce1a10958f3cf64da5684fa0f94723df0f4fc8ea43e3445" }`。`x: 1` 提交後根 `b5ebeb5c170f4a59cc858cbb2ade52904e72971ccf18e25ba458059935057f7b`，讀數仍是 `#fuel_exhausted`，caid `hash:sha256:v1:20a14926c22a7760386319ec6a2036230dea00145fcbb6bb8cf0b27f35a1927c`。10 列種子根 `7728675dba5c12b98e7545cc2c72abfd5572833410d531c63680473c72b2366a`，`_.f9_199` 的 caid 是 `dabca8c56a6c34f1b1cd7fb7abc1ab99451e2c442808d5b3621bd3ffebf363fc`；`x: 1` 之後根 `2e29ed616a79dc172cac0f17b6f59b560a79c88b24ae10fda7c72f72585f60e1`，caid 是 `1738c1aed6b9a9753c51f74305515e5d8082e2321e31a0ac14323dcb1e2232d4`。v0.94.0 同一格的視界 caid，種子之後與 `x: 1` 之後，都與上面這兩個相同。
    *   不一致。`b: 3`、`...c` 之後的 `c: { b: 2 }` 在 evolve 拒絕，`#conflict at b`，成員數 0。
*   **R1-Q2 規模。** 各來源 200 欄。本輪與 v0.94.0（`v0.94.0-verify`）同機，2026-10-19。`x: 1` 的 evolve 與提交分開計時。
    *   7 列，本輪：evolve 0.87 秒、rc 0、最大駐留 99916 KB；提交 1.18 秒、rc 0、124676 KB。v0.94.0：evolve 0.42 秒、rc 0、102172 KB；提交 0.67 秒、rc 0、122916 KB。兩邊 `_.f6_199` 在提交後都是上面那個 `20a14926…` 視界。
    *   10 列，本輪：evolve 2.12 秒、rc 0、117644 KB；提交 2.64 秒、rc 0、131048 KB。v0.94.0：evolve 0.79 秒、rc 0、122324 KB；提交 1.22 秒、rc 0、129236 KB。兩邊 `_.f9_199` 在提交後都是 `1738c1ae…` 視界。
    *   種子本身的提交仍是重的一截。本輪 7 列 26.19 秒、3100208 KB；10 列 38.09 秒、4445696 KB。v0.94.0 7 列 25.29 秒、3093744 KB；10 列 36.49 秒、4435988 KB。種子的 evolve／提交退出碼都是 0。

### 9.4.5 數字

*   探針 `a_spread_at_the_root_probe_test` 10／10。閘門第一輪 8.88 秒。閘門前單獨跑一次 8.70 秒，退出碼 0。探針檔未改。
*   三輪：2026-10-19 21:04:14 起 1052.16 秒、21:21:46 起 660.17 秒、21:32:46 起 649.26 秒，退出碼都是 0。正規化 3597 行，sha256 `9e10fa95d3ea35d0b1a7d0d3f43605912701e68132de12357f53ca434efe9a2c`，三輪相同。`test result:` 285 條，合計 2745 通過、0 失敗。以 `test ` 起算的通過行 2744。`Running ` 282 條。原始日誌 291203／291049／291049 位元組，sha256 前綴 `a442e1613a6ba67d`、`003aefd0b44cd24e`、`f6943c382d106189`。`^error` 0，BrokenPipe 0，panicked 0。
*   紅線（閘門第一輪，皆通過）：本探針 10／10 8.88 秒；Q-092 25／25 45.54 秒；Q-091 13／13 7.86 秒；Q-090 4／4 2.07 秒；Q-089 6／6 2.03 秒；Q-088 8／8 5.18 秒；`forward_spread` 12／12 0.92 秒；`spread_collision` 25／25 0.89 秒；`spread_privacy` 11／11 0.36 秒；`blur_spread` 13／13 3.13 秒；`spread_splice` 9／9 0.16 秒；`import_is_spread` 7／7 0.45 秒；`a_name_spread_in` 15／15 6.08 秒；`pin_probe_test` 15／15 2.32 秒；`what_an_observation_leaves` 15／15 4.20 秒；`where_there_is_no_universe` 13／13 4.20 秒；`a_private_never_committed` 10／10 4.13 秒、R-1 3／3 0.51 秒。同輪還有：Q-084 10／10 8.76 秒；`a_table_that_runs_every_branch_when_looked_at` 7／7 2.79 秒；`a_table_that_stops_being_one_when_printed` 4／4 2.28 秒；`a_table_you_cannot_read` 12／12 3.33 秒、R-1 4／4 1.55 秒；`a_tag_key_read_as_a_string` 7／7 2.85 秒；epochs 4／4 0.37 秒；union 2／2 0.00 秒；bytes 1／1 0.26 秒；`caused_top` 11／11 1.17 秒；`effect_union` 15／15 0.54 秒；`kind_tag` 7／7 0.28 秒；`math_union` 17／17 0.98 秒；`stage2_open_term_test` 3／3 0.10 秒；`stage3_acceptance_test` 6／6 0.12 秒；`stage3_probe_test` 4／4 0.34 秒；`static_cycle` 17／17 0.69 秒。
*   conformance 178／178，空目錄，`CONF_EXIT` 0。父目錄 `.oo/format` 前後都是 `32`。
*   Windows：`Finished release in 4.57s`，`WIN_EXIT` 0，`^error` 0。該目標 lib 15 則警告。linux 閘門 lib 14 則。未修。
*   十七格與 `st_93.txt` 逐行相同。位址列與 `inb_94.txt` 逐行相同。
*   釋出 `oo` 11103240 位元組，mtime 2026-10-19 20:58:53 +0800。三輪閘門沒有重連這支二進位。工作區在編譯時是髒的，所以這裡不引 `--version`。


## 9.5 R-1 驗收：**不受理，開 R-2**（一類；**起因在驗收方**）

交付 `332dff8` 的 `universe.rs` 與參考實作 `q093-r1ref 8ddb0f9` 逐位元組相同——**第十五次同形**；9.4 的讀數與驗收方量的相同（7×200 種子根 `bf1c642b…`、之後 `b5ebeb5c…`）。R1-M1 成立：無關寫入三路同根 `a75ab440…`，7×200、10×200 之後的無關提交成功。

**不受理**：**晚到的來源與一個存著讀法的鍵相撞，兩次提交與兩次 evolve 都收下**（`acc/q93/` 下手量；探針外）：

| 先提交 | 再寫 | 同一個檔 | 兩次提交 | 兩次 evolve | `85d865b` 兩次提交 |
| :-- | :-- | :-- | :-- | :-- | :-- |
| `d: { b: 3 }`、`b: d.b`、`...c` | `c: { b: 2 }` | 拒 `#conflict at b` | **收**，`_.b` 是 `3`、`_.c.b` 是 `2` | **收** | 收，`_.b` 存 ⊥ |
| `n: 3`、`b: n`、`...c` | `c: { b: 2 }` | 拒 | **收**，同上 | **收** | 收，`_.b` 存 ⊥ |
| `b: 3`、`...c`（r5） | `c: { b: 2 }` | 拒 | 拒 | 拒 | 拒 |

來源先寫、再寫 `b: d.b` 與 `...c`：拒（對）。違反 M3（「晚到的來源與根上既有的鍵相撞，在 evolve 以鍵拒絕」）：根上留著 `...c` 的列，`_.b` 與 `_.c.b` 不同。

**起因在驗收方**：R-1 參考實作的一致性檢查用 `engine.unify`——**看不到宇宙的上下文**；鍵上存的讀法 `d.b` 在那裡讀成 `_`，與任何值都不衝突。Q-092 R-1 栽過同一種（「驗證不能在看不到被驗的東西的地方做」）。第一輪交付在同一格存 ⊥：重攤後面那個根檢查也是 `engine.unify`，同樣看不到，讀法進暫存後在提交時交會成 ⊥。r5 只量了鍵上存答案的那一形。

### 9.6 修補回合 R-2（交付方做）

*   **R-2 射程（不變式；M1–M4、R1-M1 照舊）**：
    *   **R2-M1 晚到的來源，與鍵上存的任何東西相撞都在 evolve 以鍵拒絕**——答案、別的座標的讀法、別名、配方；與同一個檔寫完同答；兩次提交、兩次 evolve 皆然（**r8**）。判一致要在看得到宇宙的地方判。
    *   **另一半**：一致的照收（r8 末格）；R1-M1 的三路同根、7×200 之後的無關提交照舊（r7）。
*   **明文不在射程內**：提交投影把視界寫進歷史（Inbox）；`..._.c`。
*   **探針**（驗收方追加，本提交；r1–r7、g1–g3 一字不動）：r8。
*   **參考實作** `q093-r2ref d804420`（在 `332dff8` 上，`universe.rs` 一處）：重攤的一致性檢查改用 evolve 的上下文（`unify_internal(…, &mut ctx)`，根與暫存都在）；兩側都不是 ⊥ 而相交是 ⊥ 時，直接以鍵拒絕。
*   **必答**：
    *   **R2-Q1 還有哪些檢查看不到宇宙**：`evolve_spread` 裡與根比的那一處（`engine.unify(root_val, v)`）同樣是盲的——它在你的實作裡還有沒有路徑會把相撞放過？列出 evolve 與提交路徑上每一處以 `engine.unify` 判相撞的地方，各一格。
    *   **R2-Q2 代價**：重攤時每個「不是這個讀法」的鍵要在宇宙裡強制一次。7×200、10×200 之後一筆 `x: 1` 的 evolve 與提交，與 R-1 同機比。

### 9.7 R-2 參考實作的數字（驗收方）

`q093-r2ref d804420` ⟹ 本探針 **11／11**（R-1 交付 10 綠、r8 紅 10／17 格）；全樹 **285 target、2746 通過、0 失敗**；conformance **178／178**（空目錄，前後皆空）；17 格、位址列與 v0.94.0 逐位元組同；身分紅線、展開矩陣、探針外的格子（`acc/q93/hunt.sh`）與交付逐行相同；`--pin` 兩種先後同根 `a75ab440…`；7×200 種子根 `bf1c642b…`、之後 `x: 1` 的根 `b5ebeb5c…`，與 R-1 逐位元組相同。

**變異**：改回 `engine.unify`（R-1 的檢查）⟹ r8；不一致時不在這裡拒絕、留給後面那個根檢查 ⟹ r8（那一處同樣看不到宇宙）。

## 9.8 修補回報（交付方填）

### 9.8.1 射程

`universe.rs` 與 `q093-r2ref` `d804420` 逐位元組相同。探針未改。`oo` 仍是 `0.94.0`。

*   **R2-M1 晚到的來源與鍵上已有的值相撞，在 evolve 以鍵拒絕。** 判一致用的是這次 evolve 的上下文。列已經在根上。
    *   別的座標的讀法。`d: { b: 3 }`、`b: d.b`、`...c` 提交後 evolve `c: { b: 2 }`，退出碼 1，`Error: Evolution Conflict: #conflict at b`，工作集成員數 0。`_.b` 仍是 `3`，`_.c.b` 是 `_`。兩次 evolve 的退出碼是 `0 1`，同一句。
    *   別名。`n: 3`、`b: n`、`...c` 之後同樣拒絕，`_.b` 仍是 `3`。兩次 evolve 也是 `0 1`。
    *   配方。`b: d.b + 0` 之後 evolve `c: { b: 2 }`，拒絕，`_.b` 仍是 `3`。`b: { k: d.b }` 之後 evolve `c: { b: { k: 2 } }`，拒絕，`_.b` 仍是 `{ k: 3 }`。兩次 evolve 都是 `0 1`。
    *   同一個檔。r8 在 11／11 裡通過：讀法與別名各有一次同一個檔的拒絕。
    *   一致的照收。`d: { b: 2 }`、`b: d.b`、`...c` 再提交 `c: { b: 2 }`，`_.b` 是 `2`，根 `c7a8e3204e5a43cb705e1e6a0195298277756b3643eab2ea1bca92237df96ecb`。兩次 evolve 退出碼都是 0。
*   **R1-M1 照舊。** `b: 2`、`...c`、`c: { b: 2 }` 提交後根 `f3bf36fc0b458751e2f9dc80f5b51e01036c23f62218ba3ba6f3383293c9b440`，`_.b` 是 `2`。再提交 `x: 1`，根 `a75ab440b717bcc169d3b59b7c6d272882f674ba7d47c29bce654e995ee17888`，`_.b` 是 `2`，`_.x` 是 `1`。7×200 種子根 `bf1c642b2ca6e43ed96fd72bb161e422f5cefb8cc56ff5ef8d5cead9b7ae2ace`，`x: 1` 之後 `b5ebeb5c170f4a59cc858cbb2ade52904e72971ccf18e25ba458059935057f7b`，evolve 與提交退出碼都是 0。10×200 種子根 `7728675dba5c12b98e7545cc2c72abfd5572833410d531c63680473c72b2366a`，之後 `2e29ed616a79dc172cac0f17b6f59b560a79c88b24ae10fda7c72f72585f60e1`。r7 在 11／11 裡通過。

### 9.8.2 順手改動

沒有。

### 9.8.3 工單哪裡是錯的

沒有。

### 9.8.4 必答

*   **R2-Q1 還有哪些檢查看不到宇宙。** `engine.unify` 自建一份求值上下文，根與暫存都不在裡面。重攤時「不是這個讀法」的那一處已改成 `unify_internal`，上下文就是這次 evolve 的（根與暫存都在）。兩邊都有值、相交是 ⊥，就在這裡以鍵拒絕，然後把這個鍵從這一次拿掉。列已經在根上的晚到來源走這裡，到不了下面那個與根比的檢查。
    *   **`evolve_spread` 與根比的 `engine.unify(root_val, v)` 仍會放過一條路。** 根上還沒有這一列的時候，第一次落地走這個檢查。先提交 `d: { b: 3 }`、`b: d.b`（沒有 `...c`），再 evolve 並提交 `c: { b: 2 }`、`...c`：evolve 與提交退出碼都是 0，根 `82cb04140f094316f3f5dd6e89b07e116ef6718e0b7c016db98f9522b4616f5e`，`_.b` 是 `_|_`，`%cause: #conflict`，`_.c.b` 是 `2`。別名 `n: 3`、`b: n` 同樣，根 `df49d2ffad6b5e4eec268a2b4276dc64bdf6203e3aa01d482e1f69adbd556aef`，`_.b` 是 `#conflict`。`b: { k: d.b }` 對上 `c: { b: { k: 2 } }`，根 `595d32c98dbba4198ffab480194640aeb39913b8a467503aaae7c092540e4ceb`，`_.b` 是 `#conflict`。鍵上是答案 `b: 3` 時，兩個數在這一次就相交成 ⊥：evolve 退出碼 1，`#conflict at b`，`_.b` 仍是 `3`。
    *   **evolve 與提交路徑上，用 `engine.unify` 判相撞的地方，各一格。**
        *   普通欄位對根上的答案：`engine.unify(root_val, val)`。`b: 1` 再 evolve `b: 2`，退出碼 1，`#conflict at b`，`_.b` 仍是 `1`。
        *   座標的值讀了宇宙：先在這次的上下文裡強制，再 `engine.unify(theirs, ours)`。`d: { b: 3 }`、`b: d.b` 再 evolve `b: 2`，退出碼 1，`#conflict at b`，`_.b` 仍是 `3`。
        *   根上的值對這次寫入做座標交會：`engine.unify(existing, val)`，`DEFER_RECIPES` 設為 2。訊息以 `cannot keep` 起頭才在這裡拒絕。v0.93.0 夾具 `v093_dotted_functions_repo` 上 evolve `r: c`，退出碼 1，`#conflict at r`；提交 `Error: Nothing to commit`；`_.r.k` 仍是 `1`。這具引擎自己提交的 `n: 5`、`c: { b: 2 }`、`r: (x -> { z: x + n }) 1` 再 evolve `r: c`，退出碼 0，`_.r` 仍是 `{ z: 6 }`。CLI 印的是原因與座標。
        *   暫存與這次寫入整顆相交：`unify_universe`（裡面是 `engine.unify`）。同一個檔 `b: 1`、`b: 2`，退出碼 1，`#conflict at b`，成員數 0。
        *   模式欄位與根比：`engine.unify(root_val, val)`。`@{ @int }: "a"` 再 evolve `@{ @int }: "b"`，退出碼 0。提交退出碼 0，根 `090f653622d8a3ee49aa6861db3253462776f256886e2b87e3196560ceb8df98`，`(_. ) 1` 是 `_|_`，`%cause: #conflict`，並寫著不相容的型別 `"b"` 與 `"a"`。
        *   `~%Config` 的部分交會：`engine.unify`。`~%Config.fuel: 50` 提交退出碼 1。再 evolve `~%Config.fuel: 40`，退出碼 1，`#conflict at ~%Config.fuel`。
        *   展開與根比：上一格。有列的重攤在這之前已經以鍵拒絕。
        *   提交本體：`unify_universe`。沒有列的那一格 evolve 已收下，提交退出碼 0，鍵上留下 `#conflict`。有列的不一致在 evolve 已經拒絕。`--pin` 的普通部分走 `pin_commit_merge` 裡的 `unify_universe`；本輪沒有另開這一格。
    *   拿摘要比位置的 `head_if_it_holds`、`session_would_be_listed`、`injection_positions_equal`、`proposals_at` 也呼叫 `engine.unify`，比的是摘要等不等。`refine` 的幾何檢查同樣呼叫它，不在 evolve 與 `oo commit` 這條路上。重攤的新檢查與暫存欄位的交會是 `unify_internal`，上下文看得到宇宙。
*   **R2-Q2 代價。** 各來源 200 欄。本輪 2026-10-19，與 R-1 同機。種子根、`x: 1` 之後的根、視界的 caid，都與 R-1 那一輪相同。
    *   7 列。種子 evolve 8.36 秒、最大駐留 184392 KB、rc 0；提交 26.70 秒、3100112 KB、rc 0。R-1 的種子提交是 26.19 秒、3100208 KB。`x: 1` evolve 0.45 秒、103212 KB、rc 0；提交 0.68 秒、124436 KB、rc 0。R-1 同一格是 evolve 0.87 秒、99916 KB，提交 1.18 秒、124676 KB。`_.f6_199` 在種子之後是 `a8e14d3b…`，`x: 1` 之後是 `20a14926…`，都是 `#fuel_exhausted`。
    *   10 列。種子 evolve 13.81 秒、228304 KB、rc 0；提交 42.49 秒、4445816 KB、rc 0。R-1 的種子提交是 38.09 秒、4445696 KB，evolve 13.66 秒。`x: 1` evolve 0.89 秒、123752 KB、rc 0；提交 1.25 秒、131064 KB、rc 0。R-1 同一格是 evolve 2.12 秒、117644 KB，提交 2.64 秒、131048 KB。`_.f9_199` 在種子之後是 `dabca8c5…`，`x: 1` 之後是 `1738c1ae…`。

### 9.8.5 數字

*   探針 `a_spread_at_the_root_probe_test` 11／11。閘門第一輪 9.93 秒。閘門前單獨跑一次 9.49 秒，退出碼 0。探針檔未改。二進位檔名仍是 `a_spread_at_the_root_probe_test-0d67b36161985d2d`；cargo 編譯了 `oo` 並跑了 11 個測試，含 r8。
*   三輪：2026-10-19 22:56:11 起 1095.16 秒、23:14:26 起 653.01 秒、23:25:19 起 657.35 秒，退出碼都是 0。正規化 3598 行，sha256 `8a2d4cfa7f5c1b055a3835cfdf3eea065232a2137c7a2c1068d7d81e1c767679`，三輪相同。`test result:` 285 條，合計 2746 通過、0 失敗。以 `test ` 起算的通過行 2745。`Running ` 282 條。原始日誌 291272／291118／291119 位元組，sha256 前綴 `07fd7294dc1edafc`、`62eef4df1cb725c7`、`3691a6ad2e70834b`。`^error` 0，BrokenPipe 0，panicked 0。
*   紅線（閘門第一輪，皆通過）：本探針 11／11 9.93 秒；Q-092 25／25 46.04 秒；Q-091 13／13 7.94 秒；Q-090 4／4 2.09 秒；Q-089 6／6 2.03 秒；Q-088 8／8 5.10 秒；`forward_spread` 12／12 1.00 秒；`spread_collision` 25／25 1.28 秒；`spread_privacy` 11／11 0.50 秒；`blur_spread` 13／13 3.40 秒；`spread_splice` 9／9 0.20 秒；`import_is_spread` 7／7 0.47 秒；`a_name_spread_in` 15／15 6.18 秒；`pin_probe_test` 15／15 2.60 秒；`what_an_observation_leaves` 15／15 4.29 秒；`where_there_is_no_universe` 13／13 4.29 秒；`a_private_never_committed` 10／10 4.11 秒、R-1 3／3 0.52 秒。同輪還有：Q-084 10／10 8.86 秒；`a_table_that_runs_every_branch_when_looked_at` 7／7 2.82 秒；`a_table_that_stops_being_one_when_printed` 4／4 2.28 秒；`a_table_you_cannot_read` 12／12 3.31 秒、R-1 4／4 1.42 秒；`a_tag_key_read_as_a_string` 7／7 2.93 秒；epochs 4／4 0.35 秒；union 2／2 0.00 秒；bytes 1／1 0.23 秒；`caused_top` 11／11 1.27 秒；`effect_union` 15／15 0.67 秒；`kind_tag` 7／7 0.31 秒；`math_union` 17／17 1.10 秒；`stage2_open_term_test` 3／3 0.13 秒；`stage3_acceptance_test` 6／6 0.15 秒；`stage3_probe_test` 4／4 0.39 秒；`static_cycle` 17／17 0.80 秒。
*   conformance 178／178，空目錄，`CONF_EXIT` 0。父目錄 `.oo/format` 前後都是 `32`。
*   Windows：`Finished release profile [optimized] target(s) in 4.63s`，`WIN_EXIT` 0，`^error` 0。該目標 lib 15 則警告。linux 閘門 lib 14 則。未修。
*   十七格與 `st_93.txt` 逐行相同。位址列與 `inb_94.txt` 逐行相同。
*   釋出 `oo` 11103904 位元組，mtime 2026-10-19 22:50:03 +0800。三輪閘門沒有重連這支二進位。工作區在編譯時是髒的，所以這裡不引 `--version`。

