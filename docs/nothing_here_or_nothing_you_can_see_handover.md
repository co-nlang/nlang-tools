# 工單 — Nothing here, or nothing you can see（Q-045）

**日期**：2026-09-13
**引擎起點**：v0.49.0（`nlang-tools` `dev`）／**規格起點**：v0.49.0-draft.1（`nlang-spec` `local`）
**偵察**：`nlang-tools/docs/a_store_it_had_not_finished_making_recon.md`
**裁定**：**本弧不需要任何裁定。** `REAL_03` §6.6 與其 v0.49.0 判例已在；O86 兩問皆開著，
**兩問都不擋本弧**（見 §3）。
**探針**：`crates/oo/tests/nothing_here_or_nothing_you_can_see_probe_test.rs`（驗收方已寫並校準，**基線 4 綠 1 紅**）

---

## 1. 這一弧是什麼

`ObjectStore::init`（`crates/interpreter/src/storage.rs:308`–`319`）判別「這是不是別人既有的
store」，讀的是 `.oo/format`——**而那正是它自己接著要寫的兩份宣告中的第一份**：

```rust
let new_store = !oo.join("format").exists() && !HEAD.exists() && !has_cas_objects(…);
if new_store {
    atomic_write(oo/"format",         "layout=5\n");   // (1)
    atomic_write(oo/"objects.format", "encoding=5\n"); // (2)
} else { Self::ensure_format(base_dir)? }              // 讀兩份，缺一即 bail
```

**每一格都是原子的，兩格合起來不是。** 在 (1) 與 (2) 之間抵達的第二個行程看見 `format`，
判定這是別人的 store，然後因為另一份宣告還沒落地而拒絕開啟它。

**兩個並行的唯讀命令就夠**，而 `oo` 沒有 `init` 子命令 ⟹ 操作者**無法事先把 store 蓋好來迴避**。

〔量 2026-09-12／13，v0.49.0 標籤二進位，全新工作區，各 100 回合〕
`status‖status` **6**／`status‖log` **11**／`evolve‖evolve` **5** 次非零離開，
逐字全部同一句 `cannot determine this store's object encoding: `.oo/objects.format` is absent; refusing to open`；
〔對照〕**先跑一次 `status` 把 store 建好，再跑同樣的對 ⟹ 0／200**。

**這一族是 v0.49.0 判例的延伸。** v0.49.0 修的是**一個**這樣的前置檢查
（`read_object_raw` 的 `Path::exists()` 在 EACCES 回 false ⟹ 不透明被壓成不存在），
並把它寫成 `REAL_03` §6.6 的判例。**本弧是去問「還有幾個」——那正是判例存在的意義。**

### 1.1 那是兩條邊界，不是一條（會影響你怎麼修）

〔量 2026-09-13，`rustc -O` 直接問七種情形〕`Path::exists()` 對**六種**非平凡情形一律回 `false`，
而底下的 syscall 給的是三分（`Ok(true)`／`Ok(false)`／`Err(kind)`）：

| 情形 | `exists()` | `try_exists()` | `symlink_metadata()` |
| :-- | :-- | :-- | :-- |
| 真的不存在（ENOENT） | `false` | `Ok(false)` | Err |
| 父目錄不可搜尋（EACCES） | `false` | **`Err(PermissionDenied)`** | Err |
| 中間段不是目錄（ENOTDIR） | `false` | **`Err(NotADirectory)`** | Err |
| 符號連結成環（ELOOP） | `false` | **`Err(FilesystemLoop)`** | **Ok** |
| 斷掉的符號連結 | `false` | `Ok(false)` | **Ok** |

⟹ **壓平發生在標準函式庫**：`Path::exists()` 的回傳型別是 `bool`，它丟掉 `Err` 那一臂
且丟得沒有聲音。全樹 `try_exists` 用了 **0 次**。

**兩條邊界，可救性不同**：

| | 邊界在哪 | 救法 |
| :-- | :-- | :-- |
| **不透明 vs 不存在** | 通道上，API 畫得出來（`Ok`／`Err`） | **就地救得回來**：換一個保留 `Err` 的拼法，逐格決定每個 `Err` 分支怎麼答 |
| **「尚未」vs 不存在** | **不在通道上** | **只能用構造消掉**——競態中的第二個行程拿到的 `Ok(false)` **是真話**，那一瞬間檔案真的不在；「尚未」既非點的性質亦非通道的性質，**它是另一個行程的意圖，而檔案系統不知道什麼叫承諾** |

**⟹ I1 是構造題不是錯誤處理題。** 不要試圖去「偵測」尚未，要讓那個中間狀態**不可觀測**。

**⚠ 這條線也不等於 `Ok`／`Err`**：`ENOTDIR`／`ELOOP` 是 `Err` 卻**不是不透明**，
是**問句本身壞了**（那條路徑上不可能有東西／問題繞回自己）——對到 n/ 這邊是 ⊥ 不是 `#blur`。
斷掉的符號連結是 `Ok(false)` 而 `symlink_metadata` 為 `Ok` ⟹ **那是兩個問句共用一個拼法**，
不是兩個答案。**你的表要分得出這幾種。**

---

## 2. 射程 ＝ 兩條不變式（不是兩個修改點）

### I1（構造）任何時刻，一個並行的觀察者對 `.oo` 的宣告只能看見兩種狀態之一

**「還沒有 store」或「一份完整的宣告」。不得存在一個中間狀態，讓觀察者把
「正在被建立」讀成「既有，但宣告不全」。**

這句話做得字面正確之後還有什麼會壞——請自己先問一遍。至少兩件：
**(a)** 既有 store 的宣告仍須被尊重（§4 紅線 R2／R3）；
**(b)** 這條不變式不得靠「重試到成功」來滿足（§4 紅線 R1）。

### I2（帳）全樹每一個 `.exists()` 都要被問過一次，一格不漏

**判準只有一問**：

> **一個 `.exists()` 是安全的，若且唯若它答錯時，下一步會有別的東西抓到。**

〔量〕全樹（非測試）共 **34 處**：`storage.rs` 11／`universe.rs` 10／`lib.rs` 3／
`value.rs` 2／`savepoint.rs` 2／`peers.rs` 2／`injections.rs` 2／
`builtins/io.rs` 1／`builtins/fs_guard.rs` 1。

驗收方已量過四處，**列出來是為了讓你知道兩端長什麼樣，不是要你只看這四處**：

| 呼叫點 | 判定 | 為什麼 |
| :-- | :-- | :-- |
| `storage.rs:379` `write_object` | **安全** | 答錯就落到 `atomic_write`，會大聲失敗；註解已逐字講明內容定址無 TOCTOU 缺口 |
| `injections.rs:81` `paths` | **安全** | 〔量〕注入目錄 000 時 `.exists()` 仍為 true ⟹ 落到 `read_dir` 具名 `injection injections: unreadable` |
| `storage.rs:310` `new_store` | **不安全** | 那個答案**決定走哪一枝，而那一枝不會回頭** ⟹ 本弧 I1 |
| `builtins/io.rs:75` `io.exists` | **最不安全** | **答案本身就是輸出** ⟹ **但需裁定，不在本弧**（§3） |

**交付要交的是那張 34 列的表**，不是只有被改動的那幾行。**沒病的格子也要留下它為什麼沒病
的一句話**，否則下一次還要再掃一遍——**這張表本身就是本弧一半的產出**。

---

## 3. 明確不做

1. **`~%Io./exists` 的回傳形。**〔量，含對照〕存在且可讀 → `#true`；真的不存在 → `#false`；
   **存在但父目錄 000 → `#false`** ⟹ 後兩者在語言層完全不可分。**這是這一族最鋒利的一格，
   但它要回什麼是開著的裁定（O86 (ii)：`#blur`／⊥／第三個 tag）** ⟹ 本弧**只在表裡記它**，
   **不動它**。
2. **`REAL_03` §6.6 要不要新增第四款**（O86 (i)）。驗收方已量出「尚未」不是通道回得出來的
   答案，故那可能是類別錯誤——**但那是裁定，不是你的事**。
3. **`.oo/peers/directory` 的「不可讀 → 冷啟動」**：它是快取還是紀錄，需要裁定。
   〔告知〕驗收方一次量測是**空讀數**（`oo node discover` 缺 `--to` 而 rc=2，沒碰到目標），
   **不得引用**。
4. **並行提交的語義**（Q-016b：該收斂還是該合法分叉）。本弧**不碰語義**——
   兩個行程對這個 store 的宣告本無分歧，分歧只在「其中一份還沒落地」的那幾微秒。
5. **身分鑄造的競態**（`value.rs:2619`／`2647`）：已於 2026-09-09 查明成因已消失
   （`load_or_mint` 走 `create_new` ＋ `load_after_race`）。表裡記一句即可。
6. **`write_object` 那一格不要動。** 它已經是對的，且它的註解是這一族唯一寫對過的說明。

---

## 4. 紅線

**R1 不得以重試、睡眠、或退避掩蓋 I1。** 那是把窗口變窄不是把它消掉，而**變窄的窗口
在別的機器上會再開**——本弧的整個成因就是「在 ext4 上看不見」。

**R2 不得回退 v0.43.0 那一行買到的保護。** 那一行（`!oo.join("format").exists() &&`）
**不是寫錯的**，理由就在它自己上面的註解：先前的引擎可能 staged 過 injections 而從未寫下
HEAD 或 CAS 物件，把那種倉當成新的會**只因為打開它就偷偷推進 layout**，繞過明示的遷移閘。
**G3 就是這條紅線**；G3 紅 ⟹ 競態是靠重新引入那個 bug「修」掉的。

**R3 legacy conflated store 的宣告不得被改寫。**（`.oo/format` 為裸數字、無 `objects.format`）
**G2 是這條紅線。**

**R4 非紀元。** 宣告的**內容**位元組不得改變（逐字 `layout=5\n` 與 `encoding=5\n`），
`.oo` 的磁碟布局不得改變，**標準根 digest 不得移動**（`x: 0` 仍須為 `31745ef0…`，
標準根 `7038e250…`）。本弧改的是**落地的原子性**，不是宣告本身。

**R5 探針不得被改動，也不得被 `rustfmt` 掃到。** 若你認為某支探針寫錯了，
**在 §N.3 說明並保持它紅**，由驗收方裁決——**不要自己改**。
（前例：Q-043 的 R2 確實寫錯，交付方沒有動它、只在 N.3 指出，驗收方改的。那是對的做法。）

**R6 離開碼不得在管線之後取。** 用 `PIPESTATUS[0]` 或 `wait_with_output`。
（驗收方本輪自己犯過一次並就地重取，記在偵察 §8.2。）

---

## 5. 探針（驗收方所寫，基線 **4 綠 1 紅**）

`crates/oo/tests/nothing_here_or_nothing_you_can_see_probe_test.rs`

| | 名稱 | 現況 | 釘什麼 |
| :-- | :-- | :-- | :-- |
| G1 | `g1_a_fresh_workspace_lands_both_declarations` | 綠 | known-answer 對照組：空工作區得到**恰好一對完整宣告**（逐字 `layout=5\n`／`encoding=5\n`） |
| G2 | `g2_a_legacy_conflated_store_keeps_its_declaration` | 綠 | **紅線 R3** |
| G3 | `g3_a_declared_store_without_history_is_not_a_new_store` | 綠 | **紅線 R2**（有注入、無 HEAD、無 CAS、宣告 `layout=4` ⟹ 開啟後仍是 `layout=4`） |
| G4 | `g4_absent_and_opaque_remain_two_answers` | 綠 | v0.49.0 的判決：不透明具名 `permission denied` 且**不得**說 `not found`；不存在具名 `not found` 且**不得**說 `permission denied` |
| **R1** | `r1_a_concurrent_observer_never_sees_half_a_declaration` | **紅** | **I1** |

**R1 的校準（寫探針前對 v0.49.0 標籤二進位量的）**：每回合至少一個行程被拒的機率——

* **ext4**（`CARGO_TARGET_TMPDIR`），32 個行程：**30／30、30／30 回合**
* **tmpfs**（`std::env::temp_dir()`），4 個行程：11／50、7／100 回合

**兩組都跑，因為窗口寬度是檔案系統的性質**：兩方並行在 ext4 上是 **0／500**，
而 32 方並行在 tmpfs 上只有 1–6／30。**只跑一組，在另一台機器上就是空讀數。**
〔本樹實測〕R1 逐字 `373 of 1080 concurrent oo status processes were refused`，3.2 秒。

**G4 帶 root 守衛**：`seal()` 在 `chmod 000` 之後**證明拒絕真的咬到**，
否則以 root 執行時整個檔案的權限斷言都會是空的。

---

## 6. 建議的落點（**參考，不是射程**）

驗收方不指定機制。以下只是讓你知道驗收方看過什麼，**採不採用都不影響驗收**：

* 標準函式庫有 `Path::try_exists() -> io::Result<bool>`，保留 `Err` 那一臂；全樹 0 次使用。
* I1 有數種構造可滿足（把兩份宣告合成一次落地／改變落地順序使被讀的那一份最後才出現／
  以一次 rename 把蓋好的目錄搬進位）。**每一種都要自己回答 R2 與 R3**——
  例如「改讀 `objects.format` 來判別」會讓 legacy store（有 `format` 無 `objects.format`）
  被誤判為新的，**那正是 R3**。
* `ensure_format` 與 `declared_encoding` 讀同一對檔案而錯誤字串重複三處，
  順手收斂是加分，**不是要求**。

---

## 7. 請在交付報告裡回答（§N.4）

1. **那張 34 列的表。** 每列：位置／缺席在這裡被讀成什麼／**答錯時下一步抓不抓得到**／
   改了沒有／若沒改，為什麼安全。
2. **你選的構造，為什麼讓「尚未」這個答案不存在**，而不是讓它變得可辨識？
   （§1.1 的理由：可辨識是救不回來的，那個差別不在檔案系統裡。）
3. **除了 `new_store` 與 `io.exists`，你找到第幾個不安全的格子？** 一個都沒有也要說，
   並說你是怎麼確認的。
4. **legacy 那一枝（R3）與「已宣告但無歷史」那一枝（R2）在你的構造下各走哪條路？**
5. **你在幾個檔案系統上量過 R1？** 若只有一個，明說。
6. 任何你認為驗收方寫錯的地方（探針、射程、上面的量測），**在 §N.3 說，不要自己改**。

---

## 8. 驗收會怎麼做

1. **diff 純度**：只看射程內的改動；`.oo` 磁碟布局與宣告內容位元組不得動。
2. **探針完整性**：`git diff` 探針檔案為空；`rustfmt` 未掃過它。
3. **全工作區 ×3**，`--release --no-fail-fast`，逐標的聚合，**保留失敗的測試名**。
4. **R1 在兩個檔案系統上各跑**（探針自己就跑兩組，驗收另外手動複驗一次）。
5. **身分紅線**以真二進位跑，每支二進位先問一個已知答案的問題並帶對照組。
6. **conformance 全跑**（今日 162／162），**跨版本雙向**。
7. **標準根與物件數複驗**：`x: 0` → `31745ef0…`、標準根 `7038e250…`、物件 3、layout=5、encoding=5。
8. **規格結案由驗收方做**（§8 收弧）：本弧預期**不改任何規範條文**——
   若你發現某條 MUST 需要改寫，**在 §N.5 說，不要自己改規格**。

---

## N. 交付回報（交付方填；本行以上一字不得動）

### N.1 做了什麼

**I1。** `ObjectStore::init` 仍以 `.oo/format` 判定「這是不是別人的 store」（G3／v0.43.0）。新倉落地時**先寫** `objects.format`（`encoding=5\n`）**再寫** `format`（`layout=5\n`）。看見 `format` 的觀察者已經看見完整的 split-axis 對；還沒看見 `format` 的觀察者走 `new_store`，寫入同一對位元組。沒有重試、沒有睡眠。宣告內容位元組未改。`.oo` 布局未改。

`format`／`HEAD` 的前置檢查改 `try_exists`：`Err` 具名拒絕，不再被 `exists()` 壓成「沒有」。

**I2。** 全樹 34 處 `.exists()` 的表在 §N.4.1。另改一格同族：`object_exists_digest`（GC 走訪）在 `try_exists` 的 `Err` 上改當成「在」，讓讀取路徑命名失敗，而不是把不透明走成不存在。

探針一字未動。`write_object` 那一格未動。`~%Io./exists` 未動。

### N.2 量測（每一項都要有分母與對照組）

本弧探針 R1 自己跑兩組檔案系統（`CARGO_TARGET_TMPDIR` 32 行程 × 30 回合 ＋ `std::env::temp_dir()` 4 行程 × 30 回合 ＝ 1080 個 `oo status`）：**0 拒絕**，約 4.1 s。G1–G4 綠。

對照：工單對 v0.49.0 標籤二進位在同一探針形下為 373／1080 拒絕。

跨版本：真 `v0.49.0-verify` 讀本版寫的 `x: 0` 倉，`status`／`log` rc=0；本版讀真 `v0.49.0` 寫的倉，同樣 rc=0。宣告仍逐字 `layout=5\n`／`encoding=5\n`。

身分：`eval '~%Math./add (1,2)'` → `3` rc=0。`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`；標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`；物件 **3**；`layout=5`；`encoding=5`。

符合性：**162 vectors, 162 pass, 0 fail**。

全樹 `cargo test --workspace --release --no-fail-fast --jobs 1 -- --test-threads=1`（逐 `test result:`）：

| 輪 | targets | passed | failed | 失敗測試名 | `^error` | cargo exit |
| :-- | --: | --: | --: | :-- | --: | --: |
| 1 | 230 | 2203 | 0 | — | 0 | 0 |
| 2 | 230 | 2203 | 0 | — | 0 | 0 |
| 3 | 230 | 2203 | 0 | — | 0 | 0 |

### N.3 我認為驗收方寫錯的地方（探針／射程／量測）

無。R1 兩組都跑到了，不是空讀數。

### N.4 §7 六問的回答

**1. 34 列。** 原點在 `crates/interpreter/src`（非測試）。「答錯」＝ `exists()` 把 `Err` 壓成 `false`。

| # | 位置 | 缺席被讀成 | 答錯時下一步抓不抓得到 | 改了？ | 為什麼安全／為什麼動 |
| :-- | :-- | :-- | :-- | :-- | :-- |
| 1 | `storage.rs` `ensure_format` `oo.exists()` | 沒有 `.oo` → 不檢查宣告 | `declared_encoding` 仍讀 `format` 並具名 | 否 | 抓得到 |
| 2 | `storage.rs` `init` `format.exists()` | 沒有宣告 → 新倉 | **決定走哪一枝且不回頭**（I1） | **是** → `try_exists`；落地順序見 N.1 | I1 用構造消掉「尚未」；`Err` 具名 |
| 3 | `storage.rs` `init` `HEAD.exists()` | 沒有 HEAD | 與 #2 合取；`Err` 曾被壓成新倉 | **是** → `try_exists` | 與 #2 同一前置檢查 |
| 4 | `storage.rs` `init` `objects/.exists()` | 沒有 objects 目錄 → 建 | `create_dir_all` 失敗 | 否 | 抓得到 |
| 5 | `storage.rs` `object_exists_digest` | 沒有物件 → GC 當不存在 | **沒有**：走訪 `continue`（§6.6 不透明變不存在） | **是** → `try_exists`，`Err` 當在 | 同族；讓讀取路徑說話 |
| 6 | `storage.rs` `remove_digest` 物件 | 不刪 | 留下檔案 | 否 | 冪等跳過，不是宣告 |
| 7 | `storage.rs` `remove_digest` 父目錄 | 不 rmdir | 留下空目錄 | 否 | 無害 |
| 8 | `storage.rs` `set_head` `oo.exists()` | 建 `.oo` | `create_dir_all` 失敗 | 否 | 抓得到 |
| 9 | `storage.rs` `write_object` 路徑 | 已在則跳過 | `atomic_write`（註解已寫） | **否（紅線）** | 工單 §3.6 |
| 10 | `storage.rs` `migrate_layout` `oo.exists()` | 「沒有 store」 | 直接 bail | 否 | 不可讀的 `.oo` 本來就不能 migrate |
| 11 | `storage.rs` `load_architects` | 空集合 | 接著的讀已分 NotFound／unreadable，但 `exists` 先短路 | 否 | fail-soft 空集合；不是宣告對 |
| 12 | `universe.rs` `unlink_legacy_staged` | 不刪 | 殘留 sidecar | 否 | 無害 |
| 13 | `universe.rs` evolve 清 `pin_pending` | 不刪 | 殘留 | 否 | 無害 |
| 14 | `universe.rs` evolve 清 `effect_pending` | 不刪 | 殘留 | 否 | 無害 |
| 15 | `universe.rs` `load_staged` `pin_pending` | 不當成 legacy pin | 不讀 sidecar | 否 | 舊 sidecar；不是 I1 |
| 16 | `universe.rs` `load_staged` `staged` | 不當成有 staged | 不讀 | 否 | 舊格子；不是 I1 |
| 17 | `universe.rs` commit 清 `pin_pending` | 不刪 | 殘留 | 否 | 無害 |
| 18 | `universe.rs` commit 清 `effect_pending` | 不刪 | 殘留 | 否 | 無害 |
| 19 | `universe.rs` `load_abandoned` | 沒有 abandoned | `read.ok()` 本會吞 | 否 | 不是宣告對 |
| 20 | `universe.rs` `clear_abandoned` | 不刪 | 殘留 | 否 | 無害 |
| 21 | `universe.rs` `append_abandoned` `oo.exists()` | 建 `.oo` | `create_dir_all` 失敗 | 否 | 抓得到 |
| 22 | `lib.rs` `init` node key `exists` | 不載入 | 之後 `create_new`／AlreadyExists | 否 | §3.5 已關閉 |
| 23 | `lib.rs` `init` 第二處 node key | 同上 | 同上 | 否 | 同上 |
| 24 | `lib.rs` `node_id_if_present` | 回 None | 鑄造路徑走 `create_new` | 否 | 同上 |
| 25 | `value.rs` `create_new_at` 父目錄 | 不 chmod 既有目錄 | 工單 §3.5 | 否 | 不動身分鑄造 |
| 26 | `value.rs` `load_or_mint` | 走 `create_new` | `AlreadyExists` → `load_after_race` | 否 | §3.5 |
| 27 | `savepoint.rs` leftover `LOG` | 不刪 | 殘留 | 否 | 無害 |
| 28 | `savepoint.rs` mint `dest.exists()` | 換下一個 id | `atomic_write` | 否 | 抓得到 |
| 29 | `peers.rs` directory `exists` | 冷啟動 | **沒有**（不可讀也當沒有） | **否** | §3.3 要裁定 |
| 30 | `peers.rs` `need_header` | 寫 header | `open` 失敗 | 否 | 抓得到 |
| 31 | `injections.rs` `paths` | 空清單 | 〔量〕000 時 `exists` 仍 true，落到 `read_dir` 具名 | 否 | 工單已列為安全 |
| 32 | `injections.rs` mint `dest.exists()` | 換 id | `atomic_write` | 否 | 抓得到 |
| 33 | `builtins/io.rs` `io.exists` | `#false` | **答案就是輸出** | **否** | O86 (ii)，§3.1 |
| 34 | `builtins/fs_guard.rs` 最長既有前綴 | 較短的 canonicalize | 可能弱化 symlink 邊界 | 否 | 不是 I1；未動 |

**2.** 構造是「被觀察的那一份最後落地」。觀察者問的是 `format`。`objects.format` 先在，`format` 後在 ⟹ 看見 `format` 時對已經齊；沒看見時兩個行程都當新倉寫同一對位元組（`atomic_write` 同文）。不去「偵測尚未」：那個差別不在檔案系統裡。

**3.** 除 `new_store` 與 `io.exists` 外，找到 **兩格**仍不安全且本弧能動／不能動分得清：`object_exists_digest`（動了，同上族）與 `peers.rs` 冷啟動（§3.3，不動）。其餘「答錯留下殘檔／跳過 sidecar」不是宣告對，也不是通道上的三種結果。

**4.** **R3／G2**（裸數字、無 `objects.format`）：`format` 在 → `new_store=false` → `ensure_format` 走數字枝，不寫 `objects.format`，`format` 仍是 `3\n`。**R2／G3**（`layout=4`、有注入、無 HEAD、無 CAS）：`format` 在 → `new_store=false` → `ensure_format` 走 split-axis 枝，兩份宣告原樣。若改以 `objects.format` 判新倉，G2 會被當成新的並寫上 `layout=5`——所以謂詞仍是 `format`。

**5.** **兩個**：探針 R1 的 `CARGO_TARGET_TMPDIR`（標為 ext4）與 `std::env::temp_dir()`（標為 tmpfs）。沒有另造第三個。

**6.** 無（見 N.3）。

### N.5 規格側的發現（不要自己改）

無 MUST 要改寫。I1 用構造滿足既有 §6.6 判例的延伸；O86 兩問仍開著。

### N.6 我沒做的事（明說，不要讓沉默看起來像覆蓋）

*   `~%Io./exists`（O86 (ii)）。
*   `REAL_03` §6.6 第四款（O86 (i)）。
*   `.oo/peers/directory` 不可讀 → 冷啟動。
*   並行提交語義、身分鑄造競態、`write_object` 的 `exists`。
*   重試／睡眠／退避。
*   宣告內容或 `.oo` 布局。
*   規格正文。
*   本弧探針。
