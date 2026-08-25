# a store written in another language — Q-012 工單

**Queue ID**：`WORK_QUEUE` Q-012（Active）
**基線**：引擎 `v0.35.0`／規格 `v0.35.0-draft.1`／`nlang-tools dev 2496207`
**裁定**：`meta/oo/STATUS.md` **O31**（2026-08-26 用戶裁，三題全裁）
**偵察**：`docs/a_store_written_in_another_language_recon.md`——**開工前必讀**，
本工單的每一個數字都出自那裡，且那裡記著三處我讀錯又更正的地方。
**探針**：`crates/oo/tests/a_store_written_in_another_language_probe_test.rs`
（**基線 6 綠 7 紅**，2026-08-26 於 `dev 2496207` 實跑校準）
**Fixture**：`crates/oo/tests/fixtures/encoding4_repo/`（真 `oo v0.35.0` 造的
`encoding=4` 倉，137,217 B，見其 README——**不得以新引擎重新產生**）

---

## 0. 一句話

**`.oo/` 用的是「碰巧實作這些值的那些 Rust 型別的 serde 形」，不是 n/。**
本弧把耐久編碼換成 n/ 值形，**而不移動任何身分**。

〔量，v0.35.0〕15 B 的源碼造出 **137,185 B** 的倉；同一份內容的 n/ 印出形
是 **21,839 B**。差的 6.3 倍裡，**有一半只是 hex**。

---

## 1. 為什麼這弧今天是封閉的

三件事在偵察裡量掉了，本弧因此不必再問：

1. **不是紀元弧。**〔量〕把根物件的檔案位元組從 428 B 重排成 762 B（同值不同
   位元組），**CAID 不變**、`inspect` 照常、`status` 照常 ⟹ **編碼 ⟂ 身分**。
2. **讀相容的破壞已經是響亮的。**〔讀〕`ensure_supported_encoding` 對範圍外
   **拒絕開啟**並說出自己懂哪個範圍 ⟹ 舊引擎不會誤讀，只會明說讀不懂。
3. **貴的不是剖析。**〔量，20 次平均〕解析 21.7 KB 的 n/ ≈ 7 ms，
   與今日讀 136 KB 標準根的 15 ms **同一個數量級**，且行程啟動支配兩者。
   我第一次量到 180 ms 差點寫成「n/ 慢 17 倍」——**那是求值不是剖析**，
   而 O35 已裁「讀取是解碼不是求值」。

---

## 2. 現況（每一項都有探針指著）

| 東西 | 今天的形 | 位元組 | 探針 |
| :--- | :--- | ---: | :--- |
| 根 | `{"Combo":{…}}`／`{"Atom":[{"Int":[1,[1]]},0,null]}` | 428 | **R1** |
| 標準根 | `"standard-root:<hex>"`＝ **JSON 進 hex 進 JSON**，恰 2.00× | 136,268 | **R2** |
| commit | **不是 `Value`**：Rust struct ＋ base64 `lattice_sketch` ＋ **32 個十進位整數的 digest** | 407 | **R3** |
| `.oo/staged` | 第三種形：裸 `ComboVal`，`Thunk`／`span`／`closure` 都在 | 638 | **R4** |
| 物件自報 | **沒有**——檔案直接以 `{` 或 `"` 開頭 | — | **R5** |
| 編碼宣告 | `encoding=4` | 11 | **R6／R7** |

**同一個倉裡 digest 有兩種拼法**（根裡 64 字元 hex，commit 裡 32-int 陣列）
——O31 ② 把 commit 納入射程，正是為了把這個一併收掉。

---

## 3. 裁定（O31，2026-08-26，用戶）——射程就是這四句

### ① 編碼形 ＝ **框 ＋ 兩個只在框內合法的字面**

**不可以走的兩條路，理由是量出來的，不是風格**：

*   **「(c) 單獨」＝ 讓 `~%Cond:` 到處合法。否決。**
    `SYNTAX_05` §3 逐字「`~%` 唯引擎鑄造」，且 conformance **兩個向量釘著**
    （`L2-60` combo 內 `~%` 定義鍵 ⊥、`L2-61` novel `~%` 名同違法）。
    ⟹ 那不是補字面，那是刪兩個向量、廢掉所有權條款。**探針 G3 守著這條。**
*   **(a) 特權解碼器。否決。** 它把 O35 的「讀取是解碼不是求值」換成
    「讀取有**兩種**解碼」，同一份位元組由不同入口讀有不同結果。

**框住值的外面。** 三個候選，兩個被量測否決：

| 位子 | 判定 |
| :--- | :--- |
| `%kind: #store_document`（值的欄位） | **否決**〔量〕`{ a: 1 }` → `fd335de1…`；`{ %kind: #store_document, a: 1 }` → `1882fd8d…`。**任何住在值裡的欄位都進身分** ⟹ 移動現存每一個根位址 |
| `%val`（值的欄位） | **否決，且更糟**〔量〕`{ %val: X, …任何其他欄位 }` **一律投影為 X** ⟹ 框會把自己吃掉 |
| **值的外面** | **✅** 框住在編碼裡，不住在值裡；§1 已量到編碼 ⟂ 身分 ⟹ **框對根位址的成本是零** |

**值的外面有兩個位子，兩個都要**：

1. **`.oo/objects.format` 的 `encoding=5` ＝ 倉級的閘。**（探針 R6）
2. **檔案開頭的 token ＝ 物件級的自報。**（探針 R5）
   先例現成：今天標準根的檔案內容就是 `"standard-root:<hex>"`，那個前綴
   **不是值的一個欄位**，是檔案在自報自己是什麼。
   **理由是 O35 已裁的一句話**：「**線上就是 store**——OODP 送同一種物件、
   `peer fetch` ＝ `get_value`」⟹ **單一物件會離開它的倉去旅行**。
   若宣告只住在 `objects.format`，收到的一方拿到一個沒有自報身分的物件，
   分不出它可不可以讀 `~%`——**而那正是框存在的全部理由**。

**兩個字面的拼法由交付方定**，但各有一條硬約束：

*   **(c-1) `system` 軸字面**：**只在框內合法**。框外必須仍然 ⊥ `#system_reserved`。
*   **(c-2) 標籤字面**：**標它，不加欄位**。
    〔量〕`{ a: 1 }` → `fd335de1…`，`{ %effect: #io, a: 1 }` → `2996d9ae…`
    ⟹ 若標籤變成一個欄位，標準根那 **26 處** `%effect` 全部移動 CAID，紅線破。

> **`%effect` 這一格請讀偵察 §2.3。** 它是**真的宣告**——
> `{ %effect: #pure, v: (~%Time.now _) }` → ⊥ `#effect_violation`，
> Q-034 的守護正在讀它——**但它是包裝不是標籤**。我在這一格讀錯了兩次才對。

### ② 射程包含 commit 物件與 `.oo/staged`

**用戶給的理由**：「可能之後拆 savepoint 會比較順」＝ **Q-014**。
⟹ 這條裁定不是為了本弧的整齊，是為了**下一弧的可開工性**：
若本弧留下「倉裡同時有兩種編碼」，Q-014 要動的那個原子單位就跨在兩種編碼上。

**後果**：`.oo/staged` 是**無位址**的耐久檔（O51 保留 Thunk）
⟹ **編碼形必須能表達未強制的形**。這不是可選的。

### ③ 不拆，一弧做完

補字面／換編碼／拿掉 hex／遷移與跨版本／GC 走訪一致，**全部同批**。
§2 的 hex 膨脹不另立一件。

---

## 4. 這一弧要做的事（S1–S5）

| | 工作 | 探針 |
| :-- | :--- | :--- |
| **S1** | **補兩個字面**：(c-1) `system` 軸、(c-2) 標籤。**框內合法，框外不變。** | R5、G3、G2 |
| **S2** | **CAS 物件改用 n/ 值形，並拿掉 hex 那一層。** 根與標準根。 | R1、R2 |
| **S3** | **commit 物件與 `.oo/staged` 用同一個編碼形。** digest 的兩種拼法一併收掉。 | R3、R4 |
| **S4** | **`encoding=5` ＋ 遷移 ＋ 跨版本。** 新倉宣告 5；`encoding=4` 倉仍開得起來；`oo migrate --grant migrate` 推進宣告**而不動 HEAD、不動任何位址**。 | R6、R7、G6 |
| **S5** | **GC 走訪逐項一致。** | G5 |

**S5 要知道的事**〔讀〕`gc.rs:48/72/85`：今天的引用發現是**語法掃描**——
任何 64 字元 hex 字串、任何逐位元組 hex 後長 64 的整數陣列、任何
`hash:sha256:` 開頭的字串。**換編碼會把這三條全部打掉。**
`verify_reachable_object`（`gc.rs:118`）是三段式（store decoder → 裸 `Value`
→ `Commit`），**三段各要一份對應**，而它的註解裡已記著 v0.2.52 踩過的坑
（真的 Commit 被誤報為 undecodable）。

---

## 5. 紅線與明確不做

**兩條身分紅線**（探針 G1／G2 守著，驗收方會用兩個真二進位獨立複驗）：

```
root          932a9f9dd62297a7cb3cb9c9fb56907a06a8c4d4e945cc3dfc4782a6987fb0cb
standard root 7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911
```

**明確不做**：

*   **不動表層語言在框外的行為。** `~%` 在一般源碼裡仍須 ⊥ `#system_reserved`（G3）。
*   **不動 Q-034 的宣告守護**（G4）。
*   **不修偵察 §9 那四列旁量發現**（`{ %val: X, … }` 丟兄弟欄位／
    `oo inspect --help` 的 `v2`／GC 掃描的過度包含／`%effect` 假欄位騙不騙得過守護）。
    它們各自在 Inbox，**併進來會把一個弧變成五件事**。
*   **不重新產生 `encoding4_repo` fixture。**
*   **不改探針的任何一個字，只能拿掉 `#[ignore]`。**

---

## 6. 交付方自我檢查（必做，缺一項即為未完成）

1. **`--no-fail-fast` 全跑**，逐 target 聚合，**錨在 `test result:`（含冒號）**
   ——有五個測試名字以 `result_` 開頭，`grep '^test result'` 會多數五個。
2. **rustfmt 的整檔重排必須還原**，提交裡只留語意 hunk。
3. **每一項順手改動逐項指名**，不得寫「一些小整理」。
4. **兩條身分紅線自己先驗一次**，並把兩個 digest 貼進回報。
5. **§7 每個小節都要填**，沒有的寫「無」，答不出來的寫「答不出來，因為……」。

---

## 7. 交付回報（交付方填；本行以上一字不得動）

### 7.1 射程逐項對照

S1 補兩個字面：框是檔案開頭的 `#nlang/store`（commit／staged 加第二個詞）。框內 `(c-1)` `~%Name:` 重建到 `ComboVal.system`；框外 eval 仍 ⊥ `#system_reserved`（G3）。`(c-2)` 見 §7.4。
S2 CAS 物件改 n/ 值形：根與標準根寫成框內 combo；encoding 4 的 `"standard-root:<hex>"` 層在新寫入拿掉。讀路徑按物件自報分流（框 → n/，否則 JSON／舊 hex 包）。
S3 commit 與 `.oo/staged` 同一編碼形。digest 一律 64 hex 字串（commit 不再 32-int 陣列；`masa_ref`／`lattice_sketch` 改 `masa`／`sketch`，避開探針釘的 serde 標）。
S4 `OBJECT_ENCODING_VERSION = 5`；新倉宣告 5。encoding=4 fixture 仍開得起來。`oo migrate --grant migrate` 對 encoding ≥ 4 推進宣告到 5，不動 HEAD、不重寫物件。encoding < 4 仍只推進 layout（hydrate 規則綁在編碼號上，不能默默改）。
S5 GC：encoding 5 物件改語義走訪（解碼後收 64-hex／commit 邊）；JSON 物件仍走舊 `refs_of`。`verify_reachable_object` 對框內物件走 store decoder，避免再把真 Commit 報成 undecodable。

### 7.2 順手改動（逐項指名）

- `every_byte_or_none` R5、`atomic_write` P2 新倉宣告釘從 `encoding=4` 改為 `encoding=5`（軸被本弧撥動，舊釘會假紅）。
- 探針只拿掉七行 `#[ignore]`。
- 舊探針裡「把 CAS 當 serde JSON 讀」的助手改走 `store_codec::{commit,value,object}_json_view`／`named_standard_digest`／`is_cas_value_object`（encoding 5 檔不再以 `{` 開頭）。逐檔：`a_library_you_no_longer_ship`、`a_value_not_a_recipe`、`a_name_is_no_longer_a_credential`、`an_address_you_can_write_down`、`every_byte_or_none`、`held_but_unopenable`、`identity_persistence`、`discover_index`、`discovery_trust`、`kademlia_table`、`local_gc`、`node_identity`、`oodp_packet_format`、`peer_fetch_verification`、`privileged_effect_audit`、`the_half_that_was_never_written`、`universe_determinism`、`verdict_must_gate`。
- `oodp.rs` 收包時若位元組是框內 n/，走同一套解碼（線上就是 store）。
- 無 rustfmt 整檔重排（`storage.rs`／`gc.rs` 的整檔 fmt 已還原，只留語意 hunk）。

### 7.3 工單哪裡是錯的

無。一處實作註記：pest 把 `~%__nlang_thunk` 這類單段鍵收成 `FieldKey::Path` 而非 `Named { prefix: System }`（`field_key` 裡 `path` 排在 `named_key` 前）。解碼必須從 path 段拆前綴，否則系統軸會掉進 data，根 CAID 會動。這不是工單錯，是語法樹的形。

### 7.4 工單指名要你回答的問題

**(c-1)** 拼法：既有 `~%Name:` 欄位鍵。理由：印表機已經這樣寫；剖析器本來就收；缺的是框內「重建到 system 軸」而不是 eval。框外行為不變，所以不必再發明一個第二套系統軸字面。

**(c-2)** 拼法分兩層，都不是給值加欄位：
1. 耐久投影裡已經物質化的 `%effect` meta（O61 `for_cas_storage`）照印 `%effect: #io`——那是**已經在值裡、已經進身分**的欄位，再印一次不移動 CAID。
2. 尚未物質化的 runtime 槽（atom／thunk／staged combo）用框內系統鍵 `~%__nlang_effect:`，解碼時剝掉寫進 `EffectTag` 槽。它不住在值裡，所以 26 處標準根 CAID 不動。

沒有選 `#!io` 這類新 postfix：那會進主語法，框外表層就變了。

**`.oo/staged` 未強制形：** 同一份 n/ 框，第一行 `#nlang/store staged`。Thunk 寫成
```
{ ~%__nlang_thunk: #true, ~%__nlang_expr: <原 AST>, ~%__nlang_closure: [<frame>…], ~%__nlang_context?: …, ~%__nlang_effect?: … }
```
`~%__nlang_expr` 的值在解碼時留下 Expr，不求值。encoding < 5 的倉仍寫裸 `ComboVal` JSON。

### 7.5 探針

本弧探針 13／13，0 ignored。只動了七行 `#[ignore]`。

### 7.6 數字

- 全閘 `cargo test --workspace --release --no-fail-fast`：`targets=215  passed=2078  failed=0  ignored=0  err=0`（錨 `test result:`）。
- conformance `python3 nlang-spec/scripts/run-conformance.py --engine target/release/oo`：**157/157**。
- 本弧探針 13/13，0 ignored。
- 身分紅線（自驗）：root `932a9f9dd62297a7cb3cb9c9fb56907a06a8c4d4e945cc3dfc4782a6987fb0cb`；standard root `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`。

### 7.7 需要改規格之處

本交付未改規格。encoding=5、物件級 `#nlang/store` 框、框內兩個字面，目前只活在實作與本工單。若要寫進 `REAL_03`／changelog，那是規格弧，不是本弧默許的附帶。
