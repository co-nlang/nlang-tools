# Q-014 工單 — 一個本來就是集合的工作集

> **Queue ID**：`WORK_QUEUE` Q-014（Active）
> **基線**：引擎 `v0.38.0` 標籤二進位
> `/home/gali/nlang-baselines/v0.38.0-verify-target/release/oo`
> （`oo v0.38.0`；known-answer `~%Math./add (1,2)` → `3`，**開單當日重量**）／
> 規格 `v0.38.0-draft.1`。**分支 `dev`。**
> **探針**：`crates/oo/tests/a_working_set_that_is_a_set_probe_test.rs`
> —— **基線 4 綠 2 紅，兩支紅各自倒在自己的斷言上**（驗收方 2026-08-30 校準過）。
>
> **⚠ 縮寫**：本文 `CAS` 只指 Content-Addressed Storage（`GLOSSARY` §11.1）。
> compare-and-swap 一律寫全稱。

---

## 0. 這一弧是什麼

`SPEC_10` §3 第 2 款一直都這樣寫：

> 「**Staged**：存儲自上一個 Commit 以來新注入但未提交的定義**集合**。」

引擎把那個集合存成**一個每次 evolve 都重寫的可變檔案**。那個格子就是全部的缺陷。

〔量 2026-08-30，標籤二進位〕**40 個並行 `oo evolve`，各加一個不同欄位 → 存活 3／40**
（五輪的範圍 3–6），**零錯誤，每個行程都回報成功**。

**D48 取候選丙**：`evolve` **鑄一個不可變的注入**，不再讀-改-寫共享格子；
工作集 ＝ 未提交注入的 fold。**這不是新設計，是第一個與規格既有那句話相符的實作。**

## 0.1 裁定依賴（全部已裁，本弧不需要新裁定）

| | 裁定 |
| :-- | :-- |
| **D48** | 取丙。`evolve` 鑄不可變注入；工作集 ＝ fold。**且 Q-014 拆成兩件**——本弧**只動工作集**，Savepoint 的身分與順序是 **Q-014b** |
| **D49** | 兩個並行注入**各自**驗證通過而**合起來**為 ⊥ 時：**兩筆都接受，fold 在該座標回報 ⊥，由操作者解決**。拒絕較晚者需要序，而本弧沒有序 |
| **D33** | 收斂撞 ⊥ 必須**停下並回報**；⊥ 可入歷史，由操作者決定 |
| `SPEC_10` §2.2.1 | **[Core Requirement]** meet 為 ⊥ 時演化**不得**發生 ＋ 回報必須指出**葉座標**、含錯誤碼、不洩漏實作表示、各面一致 |
| `SPEC_10` §3.1 | **[Core Requirement]** Savepoint 必須活過 commit、必須帶可判定先後序 —— **綁在 Q-014b，不綁本弧**。**注入不是 Savepoint** |

---

## 1. 射程

### S1 — `evolve` 鑄不可變注入

不再讀-改-寫 `.oo/staged`。每次成功的 evolve **建立一個新檔**，內容不可變，
**以 `atomic_write` 寫入**（`REAL_01` §4.1.1 [Core Requirement]：`.oo/` 之下**任何**
耐久寫入必須是臨時檔 ＋ `fsync` ＋ `rename`）。

**本地 id 由你選**，但**不得由一次讀到的計數決定**——那正是 `savepoint.rs::mint_id`
今天的病（`n = ids.len()+1`），而它在並行下同時撞身分與順序。

### S2 — 工作集 ＝ fold

**必須是引擎內對 `unify` 的迴圈。**〔量，偵察 Q16〕把 fold 寫成 n/ 的 `&` 鏈：
N=10 得到十欄 combo，**N=100 → `#blur %cause: #fuel_exhausted`**，N=1000 → `#max_depth_exceeded`。
**N=100 就已經不是答案。**

### S3 — 寫入前驗證（`SPEC_10` §2.2.1）

`fold ⊓ Definition = ⊥` 時**不得**鑄檔，離開碼非零，**回報必須指出葉座標**。

**這不是讀-改-寫一個共享格子**，是**一次容忍過期的驗證讀**：相異欄位永不衝突，
所以 40 並行仍然全數落地。**今天的「先到者贏」是合規行為，探針 `g1` 釘住它。**

### S4 — D49：並行的聯合 ⊥

兩筆各自通過 S3 而合起來為 ⊥ 時：**兩筆都留**，fold **在該座標**報 ⊥，
**且離開碼不得為 0**（D33 的另一半）。

> **這一格今天不存在**——今天有共享格子，最後寫入者贏。**它是本弧新造出來的**，
> 所以它的行為是被裁定的（D49），不是被實作選的。

### S5 — commit

fold、清除工作集注入、**`~%Config` 的 session 壽命必須存活**。
〔量〕今天 commit 把 `~%Config` 從提交 meet 剝掉再 restage，故 `fuel: 12345`
在 commit 之後仍在 `oo status` 裡（O37）。**「fold 完清空目錄」會把它一起殺掉**
——偵察 Q18 已點名這是丙最容易踩的一格。探針 `g3` 釘住它。

### S6 — 版圖宣告

新目錄**必須**進三份 `.oo/` 允許名單（`a_store_you_did_not_write` `p1`、
`local_gc` `p4`、`advert_persistence` `r2`），**並檢查 `kademlia_table` `p4`**
——它的名單今天**沒有** `savepoints`，只因該情境不 evolve 而綠（偵察 Q6 的潛伏針）。

> **§8 收弧 3c（2026-08-29 新設，起因就是上一弧）**：凡在 `.oo/` 新增一種耐久檔，
> 既有的並行／損壞量測**必須對新檔重跑一次**。**這一條是為這一弧寫的。**

---

## 2. 明確不做

* **`.oo/savepoints/` 的一切**（D48 拆分 ⟹ Q-014b）。探針 `g2` 釘住「你沒碰它」。
* **pin 的定序**、**compare-and-swap 與重試**（皆 Q-016）。
* **衍生快照**（把 fold 壓回 O(1)）。它是**衍生的**，之後補不必重做本弧；
  偵察 Q16 已報價。**現在做會把甲的狀態檔請回來，而本弧的主張是不要那個格子。**
* 觀測邊界寫 ○（`SPEC_10` §3.1 的未兌現列，另案）。
* CLI savepoint 動詞（Q-018／W22）。
* **動身分。**

## 3. 紅線

* `x: 0` 的根 **`31745ef0e8bfde3d…`**、`.oo/objects` **3 個物件**〔開單當日量〕。
* 標準根 **`7038e2504b8ef4d4…`**。
* 探針 `g4` 釘住兩者。**注入不得進 `objects/`。**

## 4. 請在交付報告裡回答

* **Q1** — 一筆注入在磁碟上長什麼樣？貼出一個真實檔案的完整位元組，
  並給一個可重跑的指令產生它。本地 id 怎麼鑄的？**證明它不是由一次讀到的計數決定。**
* **Q2** — S4 的「兩筆都留」在儲存上怎麼表示？fold 怎麼知道 ⊥ 落在哪一格？
* **Q3** — 注入有沒有按座標分組？（`SPEC_10` §2.2.1 自己說「衝突揭露的最小單位
  也應當是座標」。**這是建議不是裁定**。）若沒有，fold 找出 ⊥ 葉座標的成本是多少？
* **Q4** — `~%Config` 用了偵察 Q18 三個候選的哪一個？為什麼？
* **Q5** — 貼出 fold 那個迴圈的 `檔案:行號`。**證明它不是 `&` 鏈。**
* **Q6** — 你動過 `.oo/savepoints/` 嗎？（**應為「無」**。若動了，說明為什麼非動不可。）
* **Q7** — 依 §8 收弧 3c：對新的注入檔重跑並行量測，報數字。

## 5. 交付自檢（缺一不可）

1. 全跑 **`--no-fail-fast`**，**逐 target 聚合**（錨在 `test result:`，**含冒號**），報 exit code。
2. conformance 全跑。
3. 三項身分紅線實測值。
4. 探針：**只能拿掉 `#[ignore]`，本檔其餘一字不得動**。認為某支校準錯了，
   **寫進 §N.5，不要改**。
5. **本工單 §N 以上一字不得動。**

## 6. 開單當日基線〔量 2026-08-30，標籤二進位〕

| | 值 |
| :-- | :-- |
| 版本／known-answer | `oo v0.38.0`／`~%Math./add (1,2)` → `3` |
| 40 並行 evolve（相異欄位） | **存活 3／40**（五輪 3–6），零錯誤，全數回報成功 |
| 循序衝突 | 第二筆 **rc=1**、`#conflict at a`、工作集留 `{ a: 1 }` |
| `x: 0` | 根 `31745ef0e8bfde3d…`、**3** 個物件 |
| 標準根 | `7038e2504b8ef4d4…` |
| 探針基線 | **4 綠 2 紅**；r1 於 `:169` 倒（40 宣稱／6 存活）、r2 於 `:212` 倒（2 宣稱／1 有著落） |

## 7. 驗收方已知的缺口（先說，免得讀成隱瞞）

* **未探針：注入的磁碟形。** 本弧可以自由選，探針會釘住一個被發明的格式。改以 Q1／Q3 用文字要。
* **未探針：崩潰窗。** 本週每一則崩潰窗發現都是用 `cp` 重構的，不是真的崩潰注入；
  假崩潰的探針釘住的是那個假動作。
* **`SPEC_10` §2.2.1 要改。** D49 違反該節「演化不得發生」的**字面**（滿足它**實際規範的每一條**）。
  **規格收尾是驗收方的事，你不要改規格**——依 §8 弧中規則，發現要改之處寫進 **§N.7**。

---

## N. 交付回報（交付方填；本行以上一字不得動）

### N.1 射程逐項對照
S1：`evolve` 不再重寫 `.oo/staged`。每次成功 evolve 以 `atomic_write` 鑄 `.oo/injections/<32-hex>`（框 `#nlang/store injection`）。id＝`ring::SystemRandom` 16 位元組，**不讀目錄計數**。探針 r1；Q1。
S2：工作集＝`injections::fold`（`crates/interpreter/src/injections.rs:69`–`:87`）對 `engine.unify` 的迴圈，不是 n/ `&` 鏈。
S3：鑄檔前 `fold ⊓ incoming`；⊥ 不鑄、離開碼非零、回報葉座標（`#conflict at a`）。探針 g1。
S4：兩筆各自過 S3、合起來為 ⊥：兩檔都留；`oo status` 印 `Conflict`／`#conflict at a` 且 exit≠0；`oo commit` 同樣拒。探針 r2。
S5：commit fold、清注入目錄、有 `~%Config` 則寫回一筆 Config-only 注入（O37）。探針 g3。
S6：`injections` 寫進 `a_store_you_did_not_write` p1、`local_gc` p4、`advert_persistence` r2；`kademlia_table` p4 一併宣告（該情境仍不 evolve）。探針 g2：未動 `.oo/savepoints/`。

### N.2 順手改動（逐項指名）
* 既有探針改讀工作集路徑（性質不變）：`every_byte_or_none` p2、`a_value_not_a_recipe` p1、`a_store_written_in_another_language` r4、`local_gc` p6、`where_the_conflict_is` p2（加「拒寫後注入目錄為空」）。
* `atomic_write` C1／R1：可變 `.oo/staged` 的 inode 命題改成「既有注入檔位元組與 inode 不變、每次 evolve 多一檔」（丙沒有被改寫的格子）。R3 `pin_pending` 仍釘 rename。
* 載入後 `restamp_thunk_effects`（否則 `status` 看不到巢狀 spread／forward-miss 的 `%effect`，那是把快照寫成注入時必須補的一步，不是射程外行為）。
* `pin_pending` 存在時 fold 走 `replace_merge` 而非 unify（pin 是覆蓋；Q-016 仍擁有並行 pin 的序）。
* 本弧編過的檔有跑 `rustfmt`（未對 `lib.rs` 整模組 fmt，避免 `disc.rs` 等被重排）。

### N.3 工單哪裡是錯的
無。

### N.4 工單指名要你回答的問題
**Q1** — `.oo/injections/<32 lowercase hex>`，框 `#nlang/store injection`，body 是這一筆的 combo（delta，不是整份 fold）。可重跑：

```
printf 'a: 1\n' > a.n && oo evolve a.n
xxd .oo/injections/$(ls .oo/injections)
```

實測位元組（31 B，無尾隨換行）：

```
23 6e 6c 61 6e 67 2f 73 74 6f 72 65 20 69 6e 6a
65 63 74 69 6f 6e 0a 7b 20 61 3a 20 31 20 7d
```

即 `#nlang/store injection\n{ a: 1 }`。id：`injections.rs:24`–`:30` `SystemRandom` 填 16 位元組再 `hex::encode`。函式不讀目錄、不用 `ids.len()+1`。40 並行檔名皆 32 hex，且不是 `000…1` 計數形。

**Q2** — 兩筆衝突各一檔，目錄裡都在。fold 在第一次 `unify` 得 `Value::Bottom` 時停下，葉座標是 `BottomDetail.path`（`a: 1` ⊓ `a: "x"` → `path = "a"`）。`Universe.workset_bottom` 記住它；`status`／`commit`／下一次 evolve 都報 `#conflict at a`。

**Q3** — 沒有按座標分組。一筆注入＝一次 evolve 的 incoming combo（可含多欄）。找出 ⊥ 葉＝最多 N 次 `unify`（N＝未提交檔數）；第一次撞 ⊥ 就停。

**Q4** — 偵察 Q18 第三個：commit 清空目錄後寫回一筆 Config-only 注入。不留 `.oo/staged`（那是甲的格子），也不給 Config 另做格運算。同一檔裡 Config 與普通欄一起到：注入裡兩欄，commit 剝 Config 再只把 Config 寫回。

**Q5** — `crates/interpreter/src/injections.rs:69`–`:87`（`engine.unify` 迴圈）。呼叫點 `universe.rs:951`。不是 `&` 鏈。

**Q6** — 無。未改 `savepoint.rs`、未改 LOG 形。evolve 仍呼叫既有 `savepoint::record`（Q-014b 的格子，本弧不當它是工作集）。

**Q7** — 40 並行 `oo evolve`、相異欄位 `f{i}: {i}`：注入檔 **40**、`oo status` 欄位 **40**（標籤基線 3–6／40）。零錯誤。

### N.5 探針
拿掉 `r1_every_success_leaves_a_trace`、`r2_a_conflicting_pair_does_not_vanish_silently` 的 `#[ignore]`。本檔其餘一字未動。

### N.6 數字
* `cargo test --workspace --no-fail-fast -- --test-threads=1`：**exit 0**。錨 `test result:`：**219** target 全 ok；**2112** passed／**0** failed／**0** ignored。`^error` 列 **0**。
* conformance：`python3 nlang-spec/scripts/run-conformance.py --engine …/target/release/oo` → **162** vectors, **162** pass, **0** fail。
* 身分：`x: 0` 根 **`31745ef0e8bfde3d…`**、`.oo/objects` **3** 個物件；標準根 **`7038e2504b8ef4d4…`**。known-answer `~%Math./add (1,2)` → `3`。

### N.7 你認為需要改規格之處
工單 §7 已點名：D49（兩筆都留、fold 報 ⊥）違反 `SPEC_10` §2.2.1「meet 為 ⊥ 時演化不得發生」的**字面**，但滿足該節實際規範（循序 S3 仍拒；並行沒有序可拒較晚者）。規格收尾是驗收方的事，未改規格。
