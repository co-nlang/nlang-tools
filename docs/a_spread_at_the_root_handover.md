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

### 8.2 順手改動（逐項指名）

### 8.3 工單哪裡是錯的

### 8.4 工單指名要你回答的問題

### 8.5 探針

### 8.6 數字

### 8.7 你認為需要改規格之處

## 9. 驗收（驗收方填）
