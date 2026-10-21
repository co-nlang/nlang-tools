# 工單：一個記錄，店把它寫了 N 遍（Q-097）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-097（Inbox「同一座標寫兩個大記錄，提交的記憶體按欄位數平方成長」，偵察推寬）／**依據 D114**（2026-10-21，用戶「都甲」＋「照舊內嵌」）
> 探針（已預先提交並校準）`crates/oo/tests/a_record_the_store_wrote_n_times_probe_test.rs`（基線約 55 秒，修好後約 6 秒）
> 預先修訂（本提交）：8 支既有測試共 12 處以 `encoding=7` 或「through v0.94.0」為讀數者，改為 `encoding=8`／「through v0.97.0」，皆標 `AMENDED 2026-10-21 for Q-097 (D114)`。
> 基線：dev（本工單所在提交）／`oo v0.97.0` ⟹ 探針 **2 綠 3 紅**；預先修訂的 8 支在基線上紅（那是它們該紅的地方）；conformance **181／181**。

## 0. 判準（裁定）

*   **D114（`nlang-spec/meta/oo/STATUS.md`）**：耐久形（工作集成員、savepoint、根與值物件）裡，**同一個閉包框架只寫一遍**，thunk 以參照指它，讀回時還原共用——與記憶體裡的形（`Arc` 共用）一致，**語義不變**。**`encoding` 7 → 8**；較早的引擎打開新倉以「encoding 8 不支援」拒絕。**宣告 ≤7 的倉照讀照寫、寫入時照舊逐 thunk 內嵌**（v0.97.0 讀得到），`oo migrate` 升到 8 之後才共用——**不拒絕任何寫入**。**身分位元組（`bn_serial`）不改**——沒有根移動。
*   **不取「只寫讀到的名字」**：偵察原型量到語義會變（兄弟鏈讀 `_`；五格寫入由收下改拒；演化變慢）。

## 1. 缺陷

〔量，標籤建置；腳本 `nlang-baselines/acc/q97/`，6 GB／15 分鐘上限〕

*   **任何一個座標上的 N 欄記錄**：`r: { f0: 0, …, f399: 399 }` 一次演化、一次提交——提交 13.8 秒、**1.78 GB**（N=200 466 MB；翻倍約 ×3.8）；兩個不相交記錄 N=400 3.5 GB；單一來源 1500 欄提交在 6 GB 處 abort。演化線性；頂層字面量無事。
*   **成因**：工作集成員與 savepoint 把記錄每個欄位存成 thunk，**每個 thunk 的 `~%__nlang_closure` 都抄一份整張記錄**（`store_codec.rs` `write_thunk` 逐一 `write_combo(frame)`），字面量 `f0: 0` 也是：400 欄的成員 11.9 MB、savepoint 14.9 MB；讀回解成 N 份獨立的框架。
*   **根也是**：讀宇宙或讀兄弟的配方欄位在根裡同樣抄框架——200 個 `xI: d.b` 的根檔 **2993617 位元組**。身分位元組也逐 thunk 內嵌（本卡不改）。
*   **何時**：v0.35.0 提交 200 欄 54 MB；**v0.36.0（Q-012，店改寫成 n/）408 MB**；之後各版同。

## 2. 依據；位址

*   **D114**（已裁）。
*   **位址：一個都不動**（g1：五個根與 v0.97.0 逐位元組同，含一個「參照排在定義之前」的根；r2 的讀法記錄根 `60d3f35a…`）。新倉宣告 `encoding=8`。

## 3. 射程＝不變式（不是機制）

*   **M1 一個框架在耐久形裡只寫一次**：記錄的大小按欄位數線性——400 欄字面量記錄一次演化後成員 < 200 KB、savepoint < 300 KB，提交成功、讀數不變（r1）；兩個 400 欄記錄同一座標提交並讀得到兩邊（r1）；200 個讀法的根檔 < 100 KB（r2）。**這適用於每一種店文件**：成員、savepoint、暫存、根與值物件。
*   **M2 位址不動**（g1、r2）：根的 CAID 與 v0.97.0 逐位元組同。
*   **M3 讀數與判決不變**（g2）：兄弟鏈讀得穿；兄弟只在自己的字面量裡解；之後寫的兄弟照收、讀不到；點號補欄照收；座標層一致照收。
*   **M4 舊倉照舊、升級才共用**（r3）：新倉宣告 `encoding=8` 並寫共用框架；宣告 7 的倉**不寫**任何共用框架、照讀照寫；`oo migrate` 說出它停用的引擎（`v0.95.0 through v0.97.0`）、宣告升到 8，之後的寫入共用、舊的與新的都讀得到。
*   **另一半——較早的引擎**：本引擎寫進 encoding-7 倉的東西，v0.97.0 讀得到、寫得下去；encoding-8 的倉，v0.97.0 以「encoding 8 不支援」拒絕（探針跑不了舊二進位，驗收方量；你在 8.4 Q3 量一格）。

## 4. 紅線（今天綠，必須保持綠）

*   g1、g2；Q-095（3）；Q-094（10）；Q-093（11，r6 已預先修訂）；Q-092（25）；Q-091（13）；Q-090（4）；Q-089（6）；Q-088（8）；`caused_top`（11）；`static_cycle`（17）；`math_union`（17）；`what_an_observation_leaves`（15）；`blur_spread`（13）；`pin_probe_test`（15）；`a_store_written_in_another_language`；`every_byte_or_none`；`atomic_write`；`an_axis_the_address_forgot`。
*   conformance **181／181**（空目錄裡跑）。
*   **交叉編譯** `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   身分紅線：`x: 0` 根 `b74d7758…`；`v: 1 + 1` 與 `1+1` 根 `df91a8ad…`；`f: (x -> x + 1)` 根 `258a1a08…`；`k: { a: 1, _: 9 }` 根 `420a0c9f…`；`m: (_ -> 1)` 根 `cf11d2ff…`；標準根 `c96c2937…`；新倉 `layout=9`／**`encoding=8`**。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1 文件清單**：列出每一種會寫 thunk 的店文件與它的寫入點（成員、savepoint、暫存、根、值物件、`pin`、`refine`、`gc` 重寫、`migrate`），各一格：寫出的位元組數、讀回讀數。`refs_of_document`（gc 的走訪）在共用框架下還找得到框架裡指名的物件嗎——量一格「只在框架裡被指名的物件」gc 之後還在。
*   **Q2 代價**：同機與 v0.97.0 比——400 欄、1500 欄字面量記錄；兩個 400 欄記錄；200、400、1000 個讀法的記錄：演化與提交的秒數、最大駐留、成員／savepoint／根檔位元組。讀法記錄的提交仍按 N² 算位址（D114 Q2 接受）——給出它在 1000 欄的數字，驗收方據此決定是否另開。
*   **Q3 跨引擎**：本引擎在 encoding-7 倉寫入後，v0.97.0 讀與寫（一格）；`migrate` 之後 v0.97.0 打開的訊息；一個 encoding-7 倉裡 v0.97.0 寫的成員，`migrate` 之後由本引擎提交（讀回內嵌、寫出共用）。

## 6. 明文不在射程內

*   身分位元組（`bn_serial`）裡的閉包——D114 Q2，這張不改。
*   剪掉閉包裡沒讀到的名字——不取（§0）。
*   規格文字（`REAL_03` §2.3、§6.8 的耐久字面）、向量與 `CHANGELOG`：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r3 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。** `rustfmt` 不得掃探針檔。預先修訂的 12 處同樣不得改回。
*   〔驗收方校準〕**兩極皆做**：
    基線 v0.97.0（dev 加本探針）：**2 綠 3 紅**——r1 紅 2 格（成員、savepoint 位元組）、r2 紅 1 格（根檔 2993617 位元組）、r3 紅 5 格（新倉宣告 7、不共用、migrate 停用範圍、升級後仍 7、升級後不共用）；g1、g2 綠。
    參考實作（`q097-ref 287fcff`，相對 dev：`store_codec.rs` 一個文件內的框架表——定義 `{ ~%__nlang_frame: [n, <框架>] }`、其餘處寫 `n`，讀回時先收齊全文件的定義再按需解碼；`storage.rs` encoding 8 與只在宣告 ≥8 時共用；`main.rs` migrate 的停用範圍）⟹ 本探針 **5／5**（約 6 秒）；全樹 **288 target、2754 通過、10 失敗**——10 個全是以 `encoding=7` 為讀數的既有測試，預先修訂後那 8 個目標重跑 **92／0**；conformance **181／181**（空目錄，前後皆空）；交叉 0 error；17 格、位址列、身分紅線（除新倉宣告 8）、展開矩陣、`acc/q93/hunt.sh`、`acc/q92/inv94.sh`、Q-095 的偵察矩陣與 `acc/q97/stale.py` 與 v0.97.0 逐行同；五個根與 v0.97.0 同。跨引擎（`acc/q97/compat.sh`）：encoding-7 倉裡本引擎寫入、v0.97.0 讀 `_.q.w` `1` 並續寫；migrate 說「oo v0.95.0 through v0.97.0」；新倉 v0.97.0 拒開。
    **代價**：400 欄字面量提交 0.59 秒、43 MB（v0.97.0 13.8 秒、1.78 GB）；1500 欄演化 6.3 秒、提交 7.1 秒、73 MB；兩個 400 欄提交 1.3 秒、55 MB；400 個讀法提交 1.2 秒、208 MB（位址仍按內嵌位元組算）。
    **變異**（`acc/q97/mut.py`）：從不共用 ⟹ r1、r2、r3；舊倉也共用 ⟹ r3；不以文字去重 ⟹ r2（讀回的框架不共用指標，根重寫時又拆開）；宣告仍是 7 ⟹ r1、r2、r3。**擋不住而留下的**：不以指標去重——寫出的位元組相同，但 1500 欄演化 6.3 → 14.9 秒（每個閉包重新渲染一次框架）；它是代價機制，不是位元組機制。**第一版參考實作**以「定義在前、編號照定義結束的順序」讀回，混合根那一格提交失敗（`write_combo` 先寫列、後排序，參照排到定義前面）——g1 的「mixed」就是為它留的。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

`store_codec.rs`、`storage.rs`、`main.rs` 與 `q097-ref` `287fcff` 逐位元組相同。一份文件裡同一個閉包框架只寫一次：定義是 `{ ~%__nlang_frame: [n, <框架>] }`，其餘 thunk 寫 `n`；讀的時候先收齊這份文件裡的定義再按需解，還原同一份 `Arc`。只有倉宣告 `encoding ≥ 8` 才這樣寫。宣告 7 的倉照舊逐 thunk 內嵌。`oo` 仍是 `0.97.0`，解譯器 `0.2.2`。探針未改。身分位元組未改。

*   **M1。** 探針 r1。400 欄字面量一次演化：成員 60103 位元組、savepoint 60030 位元組（v0.97.0 是 11943095 與 11943022），提交退出碼 0，`_.r.f399` 是 399。兩個 400 欄記錄同一座標提交，`_.r.f399` 與 `_.r.g399` 都是 399。探針 r2：200 個讀法的根仍是 `60d3f35a5b033490b1f0475e468ce324b28d444bdd793528b9104c4e4a8230f0`，根檔在 100 KB 以內，`_.q.x199` 是 3。同形的 Q2（鍵名 `r`）根檔 30135 位元組，v0.97.0 是 2993617，兩邊根都是 `3dd138529716114bf5fb953de2eb6a358348de32b6c5b0889b7a2b07358bffbd`。
*   **M2。** 探針 g1 五個根與 v0.97.0 逐位元組同，含參照排在定義之前的 mixed。Q2 的字面量 400、兩個 400、讀法 200、讀法 400，根都與 v0.97.0 相同（`4cbecfc6…`、`400e403f…`、`3dd13852…`、`d4d3c62d…`）。`st.sh` 對 `st_97.txt`、`inb.sh` 對 `inb_97.txt`，diff 都是 0 行。
*   **M3。** 探針 g2。`acc/q97/stale.py`、Q-095 的 `m.py`／`m2.py`／`m3.py`／`m4.py`、`acc/q93/hunt.sh`、`acc/q92/inv94.sh` 與 v0.97.0 逐行相同。
*   **M4。** 探針 r3。新倉 `encoding=8`，文件裡有 `__nlang_frame`。宣告 7 的倉這次提交沒有 `__nlang_frame`，`_.r.z` 是 1。`oo migrate --grant migrate` 說 `oo v0.95.0 through v0.97.0`，宣告改成 `encoding=8`。之後的提交寫共用框架，`_.r.z` 仍是 1、`_.u.w` 是 2。
*   **另一半。** 本引擎寫進宣告 7 的倉（沒有 `__nlang_frame`），v0.97.0 讀 `_.r.z` 是 1，並能再提交 `p: { k: 2, m: k }`，`_.p.m` 是 2。`migrate` 之後 v0.97.0 打開是 `object encoding version 8 is not supported by this engine (understands encoding 1 through 7); refusing to open`。新倉同樣這句。

### 8.2 順手改動（逐項指名）

沒有。

### 8.3 工單哪裡是錯的

沒有。

### 8.4 工單指名要你回答的問題

時間是 `/usr/bin/time` 直接 exec `oo` 的秒數與最大駐留（KB）。上限 `ulimit -v 6000000`。v0.97.0 是 `/home/gali/nlang-baselines/v0.97.0-verify/target/release/oo`。

*   **Q1。會寫 thunk 的文件，以及它的寫入點。** 共用只在 `set_share_frames` 為真時包進 `document()`：開倉時宣告 `≥ 8` 才設。定義在文件內按指標與文字各編一次號；讀回時同一個編號解成同一份 `Arc`。
    *   **成員。** `injections.rs` 的 `write` → `encode_injection`。40 欄字面量：5983 位元組，`__nlang_frame` 1 次、thunk 80 次（框架裡 40 個，外面 40 個指它）。`oo status` 看得到 `f0: 0`。讀 `_.r.f39` 要等提交，提交後是 39。400 欄的成員是 60103 位元組。
    *   **savepoint。** `savepoint.rs` → `encode_savepoint`。同一格 5910 位元組，框架 1 次、thunk 80 次。400 欄 60030 位元組。
    *   **暫存。** `encode_staged` 有同一層 `document()`，這一版沒有寫入點會產生 `.oo/staged`。40 欄演化之後該檔不存在。暫存的內容在成員裡。
    *   **根。** `ObjectStore::encode_cas_value`（宣告 `≥ 5`）→ `encode_value`。字面量提交後欄位是值，根裡沒有 thunk：40 欄根檔 415 位元組，400 欄根檔 3895 位元組，與 v0.97.0 同一檔、同一個 CAID。讀法才把 thunk 留在根上：200 欄 30135 位元組（v0.97.0 是 2993617），CAID 相同，`_.r.x199` 是 3。
    *   **值物件。** 同一條 `encode_value`。`~%Engine./save { z: 7 }` 寫出 21 位元組的 `{ z: 7 }`，裡面沒有 thunk。帶 thunk 的值物件就是上面的根。
    *   **pin。** 沒有另一個編碼器。`evolve --pin --grant pin` 仍走成員：`r: { y: 2, z: y }` 的成員 434 位元組，框架 1 次、thunk 4 次。`commit --grant pin` 之後 `_.r.y` 是 2、`_.r.z` 是 2。
    *   **refine。** 新提交走 `encode_commit`，那份文件沒有 thunk 閉包（816 位元組，`__nlang_frame` 0 次）。來源與目標都給原來的根（完整 CAID）時，根物件沒有重寫：仍是 359 位元組、框架 1 次、thunk 3 次，`_.r.z` 是 1。根若要重寫，走的是同一條 `encode_value`。
    *   **gc。** 不重寫物件，只刪走不到的。走訪是 `refs_of_document`：先把文件解回值，再走閉包。解碼會把編號還原成框架，所以框架裡的名字還在。兩格：`~n` 放著 `engine.save` 的 CAID、`f: (x -> ~n)`，這顆 CAID 在根檔裡只出現一次，在 `%closure` 裡，不在別的欄位上；另存一顆沒被指名的 `{ z: 9 }`。`gc --grant gc` 之後被指名的還在，沒被指名的刪了，`_.f 1` 仍是那顆 CAID。另一格只量走訪、沒有留在樹上：一份文件裡摘要只出現在 `{ ~%__nlang_frame: [0, { k: "<64hex>" }] }` 的框架本體，另一個 thunk 的閉包只寫 `0`；`refs_of_document` 仍回這顆摘要。量完把 `store_codec.rs` 還原成與 `287fcff` 相同的位元組。
    *   **migrate。** 不重寫既有物件。宣告 7 的倉提交之後，物件、成員、savepoint 共 17373 位元組，sha256 前綴 `a4384d750134ee62`，`__nlang_frame` 0 次。`migrate` 之後位元組與這顆 sha 不變，框架仍是 0，宣告改成 `encoding=8`，`_.r.z` 仍是 1。之後的寫入才共用。
*   **Q2。** 字面量記錄的根檔兩邊一樣大（值，不是 thunk）；N² 在成員、savepoint，以及 v0.97.0 提交當下的記憶體。讀法記錄的根檔本引擎按欄位線性，提交的記憶體仍按內嵌位元組算位址。
    *   **400 欄字面量。** 本引擎：演化 0.45 秒、25900 KB；提交 0.56 秒、43272 KB；成員 60103、savepoint 60030、根 3895；`_.r.f399` 是 399。v0.97.0：演化 1.20 秒、92732 KB；提交 13.09 秒、1777252 KB；成員 11943095、savepoint 11943022、根 3895。根都是 `4cbecfc6c345f5a639a62abd1a8162e0958673e9e2b274d48f56622de43ba359`。
    *   **1500 欄字面量。** 本引擎：演化 6.38 秒、67576 KB；提交 7.09 秒、75440 KB；成員 228203、savepoint 228130、根 15895；`_.r.f1499` 是 1499。v0.97.0：演化 16.94 秒、774132 KB，成員 170037895；提交要配置 6442450944 位元組，失敗（信號 6，退出碼 134，27.01 秒，駐留 3498324 KB），沒有 HEAD，讀是 `_`。
    *   **兩個 400 欄記錄。** 本引擎：演化 1.55 秒、38432 KB；提交 1.13 秒、55708 KB；成員 120091、savepoint 120018、根 7675；兩邊都讀到 399。v0.97.0：演化 3.00 秒、169240 KB；提交 25.90 秒、3511608 KB；成員 23886075、savepoint 23886002、根 7675。根都是 `400e403f71706228c0708e4bc0c52ca3dea71bf7243f8aff14754a8497596e2d`。
    *   **讀法 200。** 本引擎：演化 0.21 秒、22664 KB；提交 0.36 秒、79620 KB；成員 30299、savepoint 30226、根 30135。v0.97.0：演化 0.36 秒、36332 KB；提交 5.12 秒、570308 KB；成員 2993755、savepoint 2993682、根 2993617。根都是 `3dd13852…`，讀是 3。
    *   **讀法 400。** 本引擎：演化 0.63 秒、26552 KB；提交 1.16 秒、208312 KB；成員 60499、savepoint 60426、根 60335。v0.97.0：演化 1.30 秒、93388 KB；提交 20.84 秒、2167744 KB；成員 11987355、savepoint 11987282、根 11987217。根都是 `d4d3c62d6dcd3fdafa65c5344987b5ea1a814e789ffdb65ca83d81d63081601f`，讀是 3。
    *   **讀法 1000（位址仍按內嵌位元組）。** 本引擎：演化 3.78 秒、50928 KB；提交 6.84 秒、**1121008 KB**；成員 151099、savepoint 151026、根檔只有 150935；讀是 3。根 `65e218d542659cec6d2e5c7209a65ea840102031e610cab72b14e5b74fa4879c`。v0.97.0：演化 8.41 秒、366032 KB，成員 74968155；提交同樣要配置 6442450944 位元組並失敗（信號 6，22.41 秒，駐留 3312568 KB），沒有根。
*   **Q3。**
    *   宣告 7 的倉，本引擎提交 `r: { y: 1, z: y }`（整棵 `.oo` 的 `__nlang_frame` 是 0）。v0.97.0 讀 `_.r.z` 是 1，再提交 `p: { k: 2, m: k }` 成功，`_.p.m` 是 2。本引擎讀同一格也是 2。
    *   `migrate` 的全文：`Migrating this store from layout=9 to layout=9 will make it unopenable by oo v0.95.0 through v0.97.0. Object encoding advances from encoding=7 to encoding=8.` 之後 v0.97.0 的 `status` 是 `Error: object encoding version 8 is not supported by this engine (understands encoding 1 through 7); refusing to open`。
    *   v0.97.0 自己建倉（`encoding=7`）只演化、不提交：成員沒有 `__nlang_frame`。本引擎 `migrate` 之後再提交這份成員：根 359 位元組、框架 1 次、thunk 3 次，`_.r.z` 是 1。新倉 `encoding=8`，v0.97.0 用上面同一句拒絕打開。

### 8.5 探針

`a_record_the_store_wrote_n_times_probe_test` 5／5。閘門前單獨 9.97 秒，`PROBE_EXIT` 0。閘門第一輪 9.01 秒。探針檔未改。預先修訂的 12 處未改回。

### 8.6 數字

*   三輪：2026-10-21 20:56:39 起 1292.73 秒、21:18:12 起 851.84 秒、21:32:24 起 867.71 秒，退出碼都是 0。正規化 3625 行，sha256 `cdb70a2220a7515f3c6fc6c536f213fc4bcb20245c73b1f32968ef521cbd8dae`，三輪相同。`test result:` 288 條，合計 2764 通過、0 失敗。以 `test ` 起算且以 ` ok` 結尾的行 2763。差的那一條是 `a_working_set_that_is_a_set` 的 `g1_a_sequential_conflict_is_still_refused`（`ok` 落在下一行）。`Running ` 285 條。原始日誌 292978／292824／292824 位元組，sha256 前綴 `1769bb67aa6ed192`、`d8ba3cde244a3b68`、`51483c5dc6dbdda4`。`^error` 0，BrokenPipe 0，panicked 0。
*   紅線（閘門第一輪，皆通過）：本探針 5／5 9.01 秒；Q-095 3／3 22.95 秒；Q-094 10／10 201.05 秒；Q-093 11／11 9.21 秒；Q-092 25／25 43.16 秒；Q-091 13／13 7.56 秒；Q-090 4／4 2.03 秒；Q-089 6／6 2.03 秒；Q-088 8／8 4.92 秒；`caused_top` 11／11 1.09 秒；`static_cycle` 17／17 0.66 秒；`math_union` 17／17 0.92 秒；`what_an_observation_leaves` 15／15 3.86 秒；`blur_spread` 13／13 2.84 秒；`pin_probe_test` 15／15 2.18 秒；`a_store_written_in_another_language` 13／13 0.95 秒；`every_byte_or_none` 14／14 1.92 秒；`atomic_write` 8／8 1.46 秒；`an_axis_the_address_forgot` 17／17 6.23 秒（epochs 4／4 0.31 秒、union 2／2 0.00 秒、bytes 1／1 0.23 秒）；`forward_spread` 12／12 0.81 秒；`spread_collision` 25／25 0.86 秒；`spread_privacy` 11／11 0.36 秒；`spread_splice` 9／9 0.15 秒；`import_is_spread` 7／7 0.43 秒；`a_name_spread_in` 15／15 6.09 秒；`where_there_is_no_universe` 13／13 4.11 秒；`a_private_never_committed` 10／10 4.05 秒、R-1 3／3 0.48 秒。
*   conformance 181／181，空目錄，`CONF_EXIT` 0。父目錄 `.oo/format` 前後都是 `32`。
*   Windows：`Finished release in 4.74s`，`WIN_EXIT` 0，`^error` 0。該目標 lib 15 則警告（`cargo fix` 會動 13 處）。linux 建置同一 lib 14 則警告（`cargo fix` 會動 12 處）。未修。
*   釋出 `oo` 11148744 位元組，mtime 2026-10-21 20:59 +0800。閘門第一輪開始時連上的。工作區在編譯時是髒的，所以這裡不引 `--version`。

### 8.7 你認為需要改規格之處

沒有。

## 9. 驗收（驗收方填）


### 9.1 第一輪驗收：**不受理，開 R-1**（一類；**起因在驗收方**）

交付 `eda60b3` 的三檔與參考實作 `q097-ref 287fcff` 逐位元組相同（同形——驗收方因此在探針外另量）；探針、預先修訂的 12 處與 §8 以上一字未動。8.3「沒有」對；8.4 的讀數與驗收方在參考實作上量的相同（400 欄提交 0.56 秒、43 MB；讀法 1000 欄提交 1.12 GB——位址仍按內嵌位元組算，D114 Q2 接受，不另開）。探針外十種形狀（多層作用域、函式造的記錄、帶 context 的開放項、管道造的記錄、兩個成員、表模式、配方遇上寫入、記錄裡的函式、根上展開、再提交）讀數與 v0.97.0 逐格同（`acc/q97/shapes.py`）。

**不受理**：**一個宇宙寫成什麼形，由同一個行程最後開啟的那個倉決定**——M4「宣告 7 的倉不寫共用框架」在探針只開一個倉時成立，換個寫法就不成立。共用與否是一個執行緒層級的開關，設在 `ObjectStore::init`，而 init 不只在綁定自己的宇宙時呼叫：
*   **`~%Discovery./connect` 連本機 peer**（`acc/q97/peer2.sh`）：宣告 7 的宇宙連上 8 的 peer、同一次演化寫 `r: { y: 1, z: y }`——**自己的成員與 savepoint 寫進共用框架**，之後 v0.97.0 提交它：`Error: thunk frame is not a combo: Atom(Int(0), …)`。反過來，8 的宇宙連上 7 的 peer，自己退回內嵌。
*   **以位址字面讀一個根**（`acc/q97/addr.sh`）：`ObjectStore::get_value` 讀根時會建一個 in-memory 引擎，它的暫存倉是新的（encoding 8）。宣告 7 的宇宙裡 `q: _{sha256:…}.x` 旁寫一個記錄——**成員與 savepoint 寫進共用框架**。
*   **起因在驗收方**：參考實作把「這個宇宙寫成什麼形」放在「開一個倉」的地方，而不是「綁定這個宇宙」的地方；探針 r3 只在一個倉裡、只做第一次提交（那時還沒有根可讀）。**驗收方的第一版 R-1 參考實作也漏了第二條**（把 in-memory 引擎也設成依它自己的暫存倉）——補 r5 時才量到。

### 9.2 修補回合 R-1（交付方做；不須新裁定：D114 已寫「宣告 ≤7 的倉照舊內嵌」）

*   **R-1 射程（不變式；M1–M4 照舊）**：
    *   **R1-M1 一個宇宙的店文件寫成什麼形，只由這個宇宙自己的宣告決定**——不隨同一行程開過哪些別的倉（peer、讀根時的暫存倉、任何其他）而變。宣告 7：整棵 `.oo` 沒有 `~%__nlang_frame`；宣告 8：共用。r4（connect 一個不同宣告的 peer，兩個方向）、r5（同一次演化以位址字面讀根，兩種宣告）。
    *   **另一半**：r1–r3、g1、g2 一字不動；本引擎寫進 7 的倉，v0.97.0 照讀照提交（驗收方量）。
*   **探針**（驗收方追加，本提交；r1–r3、g1、g2 一字不動）：r4、r5。
*   **參考實作** `q097-r1ref 243040f`（在 `eda60b3` 上）：`ObjectStore::init` 不再設開關；`Ouroboros::bind` 依它綁定的倉設（`ObjectStore::shares_frames`）。in-memory 引擎**不設**。
*   **必答**：
    *   **R1-Q1 開倉清單**：列出每一處會在同一個行程裡開第二個倉或第二個引擎的地方（`ObjectStore::init` 與 `Ouroboros` 的每個建構子的呼叫點），各一格：在宣告 7 的宇宙裡經過它之後寫一個記錄，`.oo` 裡有沒有 `~%__nlang_frame`。
    *   **R1-Q2 跨引擎**：r4、r5 兩格的「宣告 7」那一邊，只演化不提交，交給 v0.97.0 提交——退出碼與讀數。

### 9.3 R-1 參考實作的數字（驗收方）

`q097-r1ref 243040f` ⟹ 本探針 **7／7**（交付 5 綠 2 紅：r4 紅 2 格——7 的宇宙寫了共用、8 的宇宙沒共用；r5 紅 1 格——7 的宇宙寫了共用）；conformance **181／181**（空目錄，前後皆空）；17 格、位址列、身分紅線（新倉 8）、展開矩陣、`hunt`、五個根、十種形狀與 `q097-ref` 逐行同；跨引擎（`compat.sh`、`peer2.sh`、`addr.sh`）：7 的宇宙經 connect 或位址字面之後 v0.97.0 照提交。全樹 **288 target、2766 通過、0 失敗**（交付 2764 加 r4、r5）。

**變異**（`acc/q97/mut_r1.py`）：`bind` 不設 ⟹ r1–r5；in-memory 引擎也設 ⟹ r5；`init` 又設（即交付）⟹ r4、r5。驗收方試過「寫進一個倉的物件依那個倉的宣告」（`encode_cas_value` 內包一層），探針與 peer 格分不出（peer 的倉在這裡只讀不寫），已從參考實作拿掉。

## 9.4 修補回報（交付方填）

### 9.4.1 射程

`lib.rs`、`storage.rs`、`store_codec.rs` 與 `q097-r1ref` `243040f` 逐位元組相同。共用開關改在 `Ouroboros::bind`（`lib.rs` 第 1021 行）設：`set_share_frames(store.shares_frames())`，`shares_frames` 是這顆倉的 `encoding >= 8`。`ObjectStore::init` 只開倉。`new_in_memory` 走自己的建構，不設這個開關。探針未改。`oo` 仍是 `0.97.0`，解譯器 `0.2.2`。

*   **R1-M1。** 探針 7／7。r4：自己宣告 7、peer 宣告 8，演化退出碼 0，自己的 `.oo` 裡 `__nlang_frame` 是 0；自己宣告 8、peer 宣告 7，演化退出碼 0，自己的 `.oo` 是 2（成員與 savepoint 各一筆定義）。r5：自己宣告 7，以位址字面讀根再寫記錄，演化退出碼 0，`__nlang_frame` 是 0；自己宣告 8，是 2，提交後 `_.q` 是 1、`_.r.z` 是 1。種子 `x: 1` 兩邊的根都是 `c9889219c55cccc569f22c0521d5954b54063702a288cf3e7452677e65749aff`。
*   **另一半。** r1–r3、g1、g2 在這支探針裡通過。宣告 7 的 r4、r5 只演化、交給 v0.97.0 提交，見 9.4.4。

### 9.4.2 順手改動

沒有。

### 9.4.3 工單哪裡是錯的

沒有。

### 9.4.4 必答

開關的預設是關。整個行程裡只有 `bind` 會設它，設的是它正在綁的那顆倉。

*   **R1-Q1。** `ObjectStore::init` 與 `Ouroboros` 建構子的生產呼叫點。每一格都在宣告 7 的宇宙裡經過它，再寫 `r: { y: 1, z: y }`（或該行程本來就要寫的那一份）。計數是整棵自己的 `.oo` 裡 `__nlang_frame` 出現幾次。
    *   **`bind`，hold 為真，`lib.rs` 1015。** 開的是這個宇宙自己的倉，第 1021 行按它設。演化再提交：退出碼都是 0，計數 0，`_.r.z` 是 1。一個行程只綁自己這一次的，都走這裡：`evolve_at`（848）、`commit`（1723，以及拿到鎖之後的 1756）、`migrate`（2273、2306）、`status`（1446）、`log`（1494）、`rollback`（1641）、`squash`（1668）、`refine`（1881）、`gc`（2506）、`inspect`（2742）、有宇宙時的 `engine_for_one_shot`（794）與 `engine_keeping_settings`（799）。`commit` 的第二次 `init` 在寫入之前再設一次；上面那次提交走的就是這兩次。
    *   **`bind`，hold 為假，`lib.rs` 1018。** 開的是暫存倉。暫存倉是新倉，宣告 8，所以第 1021 行把開關設成共用。用在沒有宇宙，或 `--ephemeral`：`without_universe`（792、801），以及 `eval`、`run`、`repl`、`test` 的 `engine_for_one_shot`。它寫的是暫存倉，不是宣告 7 的那棵 `.oo`。宣告 7 的宇宙裡 `eval --ephemeral '1 + 1'` 印 `2`，那棵 `.oo` 的檔案雜湊不變，計數仍是 0。下一個行程再演化記錄，計數仍是 0。
    *   **`new_in_memory`，`lib.rs` 966。** 開暫存倉，不設開關。呼叫點是 `get_value`：`storage.rs` 752（format-3 的根）與 759（encoding 小於 4 才補水）。同一行程裡接著寫的：
        *   位址字面，`lib.rs` 4620。就是 r5。宣告 7 計數 0。宣告 8 計數 2。
        *   `~%Discovery./fetch` 的本機 peer，`disc.rs` 604（578 是自己的倉）。宣告 7 連上宣告 8 的 peer，同一演化先 `fetch` 那顆根再寫記錄：演化退出碼 0，計數 0。`status` 看得到 `g` 是 `{ x: 1 … }`、`p` 是 `#true`、`r` 是那筆記錄。
        *   `refine` 的單調檢查，`universe.rs` 2698。宣告 7，來源與目標都給根的完整 CAID：退出碼 0，寫下 refine 提交，計數前後都是 0。
        *   `eval` 同一條位址字面，另一個行程：印 `1`，計數不變。
        *   `gc`，`gc.rs` 162 與 188。宣告 7 提交之後 `gc --grant gc`：`3 objects, 3 reachable, 0 collectable (0 bytes)`，計數 0。這個行程沒有再寫記錄。下一個行程再演化，計數仍是 0。
        *   同一條 `get_value`、這次沒有另造一格的：`universe.rs` 2847（refine 的 shadow scan；上面那格沒走到）、`lib.rs` 5089（`get_live_value`）、`oodp.rs` 398（`serve`）、`main.rs` 2749（`inspect`）。`new_in_memory` 不設開關。量根的時候跑過 `inspect`，那個行程不寫記錄。
    *   **`~%Discovery./connect` 的本機 peer，`disc.rs` 155。** 再開一顆 `ObjectStore`，`init` 不設開關。宣告 7 連宣告 8：演化退出碼 0，自己的計數 0，peer 的計數 0（peer 這次沒有寫）。宣告 8 連宣告 7：自己的計數 2。
    *   **`evolve_where_absent`，1001。** 先綁一個新的暫存目錄（宣告 8）。Replay 才呼叫 `evolve_at`，寫目的地之前按目的地再設。沒有造出 Replay 那一格。
*   **R1-Q2。** v0.97.0 是 `/home/gali/nlang-baselines/v0.97.0-verify/target/release/oo`。
    *   r4 的宣告 7 那一邊，peer 宣告 8，只演化：計數 0。v0.97.0 `commit -m a` 退出碼 0。v0.97.0 讀 `_.r.z` 是 1，本引擎讀也是 1。
    *   r5 的宣告 7 那一邊，種子 `x: 1` 的根是 `c9889219c55cccc569f22c0521d5954b54063702a288cf3e7452677e65749aff`，只演化位址讀與記錄：計數 0。v0.97.0 提交退出碼 0。v0.97.0 讀 `_.q` 是 1、`_.r.z` 是 1，本引擎讀也是。

### 9.4.5 數字

*   探針 `a_record_the_store_wrote_n_times_probe_test` 7／7。閘門前單獨 10.29 秒，`PROBE_EXIT` 0。閘門第一輪 10.24 秒。探針檔未改。預先修訂的 12 處未改回。
*   三輪：2026-10-21 22:38:12 起 1302.52 秒、22:59:54 起 906.49 秒、23:15:01 起 890.60 秒，退出碼都是 0。正規化 3627 行，sha256 `9efc37b66f5a96589062ca7abd726c92806200475c51312664f66fdd3da25af8`，三輪相同。`test result:` 288 條，合計 2766 通過、0 失敗。以 `test ` 起算且以 ` ok` 結尾的行 2765。差的那一條是 `a_working_set_that_is_a_set` 的 `g1_a_sequential_conflict_is_still_refused`（`ok` 落在下一行）。`Running ` 285 條。原始日誌 293092／292938／292938 位元組，sha256 前綴 `adb4161c1b90d357`、`22c002f4a343ad43`、`afa0ddd436110933`。`^error` 0，BrokenPipe 0，panicked 0。
*   紅線（閘門第一輪，皆通過）：本探針 7／7 10.24 秒；Q-095 3／3 36.03 秒；Q-094 10／10 210.11 秒；Q-093 11／11 9.49 秒；Q-092 25／25 45.61 秒；Q-091 13／13 7.62 秒；Q-090 4／4 2.03 秒；Q-089 6／6 2.03 秒；Q-088 8／8 4.94 秒；`caused_top` 11／11 1.13 秒；`static_cycle` 17／17 0.75 秒；`math_union` 17／17 1.00 秒；`what_an_observation_leaves` 15／15 3.92 秒；`blur_spread` 13／13 2.95 秒；`pin_probe_test` 15／15 2.45 秒；`a_store_written_in_another_language` 13／13 1.21 秒；`every_byte_or_none` 14／14 2.02 秒；`atomic_write` 8／8 1.66 秒；`an_axis_the_address_forgot` 17／17 6.39 秒（epochs 4／4 0.33 秒、union 2／2 0.00 秒、bytes 1／1 0.25 秒）；`forward_spread` 12／12 0.83 秒；`spread_collision` 25／25 0.91 秒；`spread_privacy` 11／11 0.38 秒；`spread_splice` 9／9 0.16 秒；`import_is_spread` 7／7 0.45 秒；`a_name_spread_in` 15／15 6.32 秒；`where_there_is_no_universe` 13／13 4.19 秒；`a_private_never_committed` 10／10 4.29 秒、R-1 3／3 0.62 秒。
*   conformance 181／181，空目錄，`CONF_EXIT` 0。父目錄 `.oo/format` 前後都是 `32`。
*   Windows：`Finished release in 7.06s`，`WIN_EXIT` 0，`^error` 0。該目標 lib 15 則警告（`cargo fix` 會動 13 處）。linux 建置同一 lib 14 則警告（`cargo fix` 會動 12 處）。未修。
*   釋出 `oo` 11148712 位元組，mtime 2026-10-21 22:36:06 +0800。三輪閘門沒有重連這支二進位。工作區在編譯時是髒的，所以這裡不引 `--version`。
