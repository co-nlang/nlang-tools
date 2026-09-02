# 工單：一次覆寫，其實是擲了一次硬幣

**Queue ID**：**Q-016a**（Ready 表第 1 列，Active）。原 Q-016 於 2026-09-03 重量後拆為
a／b 兩卡；**b（並行提交、`§4.1.1`、HEAD 的 CAS）不在本弧**，見 §3。

**基線**：`v0.42.0`，`/home/gali/nlang-baselines/v0.42.0-verify/target/release/oo`
（known-answer `~%Math./add (1,2)` → **`3`**，**對照** `~%Math./add (1,"x")` → **`_|_` (%cause: #conflict)`**）。
**你也要先做一次 known-answer 加對照。**`oo --version` 不知道自己是哪一版（Inbox 四例）⟹ 用行為確認。
**⚠ 取離開碼不得經過管線**（`$?` 在 `cmd | head` 之後是 `head` 的；本專案犯過兩次）。

**裁定依賴**：**D58**（已裁，取候選乙）／**D48**（工作集是集合）／**D49** ＋ `SPEC_10` §2.2.2（並行注入）。
設計全文：`nlang-spec/meta/oo/commit.md` **§1.7.11**（本弧的那一節）與 **§1.1.6 ③**（四週前就拒付的那條路）。

---

## 1. 這一弧修的是一個型別錯誤

**D48 把工作集改成一個不可變注入的「集合」，以 meet 折疊。** meet 是 ACI，
所以「對一個集合 fold」有定義——**順序不可能有影響**。

**`pin` 不是 meet，是 replace，兩者皆非。** 所以「對一個集合以 replace fold」
**根本沒有定義**，而引擎照做了（`universe.rs` `load_staged` 的 `pin_pending` 分支），
於是缺的那個序，由 `injections::paths()` 的 `out.sort()` 補上——而檔名是
**16 bytes 的 OS 熵**。

〔量 2026-09-02，基線二進位〕**兩個指令，一個行程，沒有任何並行**：

```
oo evolve --pin --grant pin  (x: 2)      rc=0，無輸出
oo evolve --pin --grant pin  (x: 3)      rc=0，無輸出
oo status                                x: 2
```

20 次試驗：**晚的贏 12、早的贏 8**；**20／20 與「隨機 id 字典序最大者勝」相符**。

**〔對照組〕**不帶 `--pin` 的兩次衝突 evolve 在**演化邊界**即被拒
（`Error: Evolution Conflict … #conflict at x`、rc=1、**不寫注入**）
⟹ **只有 `pin` 造得出兩筆互相衝突而共存的注入**，序敏感性為 `pin` 獨有。

**⚠ 而今天的實況比 LWW 更差**：兩個運算元**都**具 `pin` 權限 ⟹「誰有權」分不出勝負；
集合無序 ⟹「誰比較晚」也拿不到。**兩個裁決者都落空，裁決者是熵。**
⟹ **我們沒有付 `commit.md` §1.1.6 ③ 拒付的那筆錢，也沒有拿到那筆錢買的東西。**

**D58 取乙**：`pin` 在 **evolve 當下**吸收該座標既有的注入。
把非交換的那一步放回**本來就有序的地方**——**evolve 是一個指令**，它與磁碟上
已存在的注入之間有 happens-before。**集合仍是集合，fold 仍只做 meet。**

---

## 2. 射程（S1–S5）

### S1 — `pin` 在 evolve 當下吸收**該座標**既有的注入

*   **不變式（這是驗收的判準，不是機制）**：
    **對任一座標，工作集的值不得依賴注入的走訪序。**
*   **⚠ 吸收的單位是座標，不是檔案。** 一個注入檔可含多個座標
    （`oo evolve` 一次一個檔，檔裡幾個定義就是幾個座標）。
    **整檔吸收會把同檔裡未被本次 `pin` 指名的座標一併吞掉。** R3 釘這一格。
*   **不得引入序。** 不得替注入排時間、排計數、排檔名。理由不是美學：
    **候選甲（讓注入有序）就是 `commit.md` §1.1.6 ③ 在 2026-08-04 已經拒付的那條 LWW 路**
    ——它把「**誰有權**」換成「**誰比較晚**」。
*   **建議（非裁定，你可以不採）**：吸收以「**指名被吸收的注入 id**」表達。
    那保住不可變性、不需要序，而且形狀與 ○ 的 `parents:` 同構。

### S2 — 吸收之後，**注入與注入之間**不再有 replace

`load_staged` 今天在 `pin_pending` 為真時，把**整個注入集合**改以 `replace_merge`
折疊——**包括本次 `pin` 沒有指名的座標**。S1 成立之後那條分支沒有工作可做：
replace 已經在注入時做完了。

*   **仍然保留的是另一半**：**根與工作集之間**的 replace（`pin_commit_merge`）。
    那一半有序（根是一個，工作集是一個），**不在本 S 的射程內，不得順手改動**。

### S3 — 特權意圖必須與它所修改的值住在同一個不可變物件裡

**`.oo/pin_pending` 是一個被整檔改寫的共享格子**——**D48 為工作集拿掉的正是這個形狀**，
而 `pin` 的意圖還留在裡面。

〔量 2026-09-02，並行兩個**不同座標**的 `pin` ×5〕注入 **2 個**，而 sidecar 只剩一個座標
（`["x"]` ×4、`["y"]` ×1）⟹ 另一個座標**靜默失去 replace 語義**，
於是 `oo status` 逐字顯示 `{ x: 9  y: 9 }`（看起來完好），
而 `oo commit --grant pin` **rc=1**、逐字 **`Error: Commit failed`**。

*   **不變式**：**特權意圖不得住在一個會被另一個寫者整檔覆寫的格子裡。**
*   **本 S 不指定新家。** 但它必須滿足：兩個並行 `pin` 各自的意圖**都**存活。
*   **這與 S1 是同一個類別**（修類別不修個案）：值住不可變物件、意圖住共享格子，
    是同一個病灶的兩半。

### S4 — 並行雙 `pin` **同座標**落回 D49／`SPEC_10` §2.2.2

S1 之後，兩個並行 `pin` 各自吸收當時所見的集合 ⟹ **兩筆同座標且不可比的注入**。
**那正是 D49 已經裁完的情形**：兩筆都留、fold 在該座標回報 ⊥、**離開碼非零**、
由操作者化解。

*   **不得為此新增任何協調機制**（鎖、序列化點、共享計數器）。
    D49 取候選 2 的理由逐字是「**它是唯一不需要序、也不需要協調的**」。
*   **此項無探針**（逐字寫在探針檔頭）：它是競速，探針只能是統計的。
    **§4 Q3 要你手量並附逐字紀錄。**

### S5 — 規格今天對 `pin` 路徑的 fold 是**零條文**

`SPEC_10` §2.2.2 規範了 meet 路徑的並行注入，**而 `pin` 路徑一句都沒有**
——它只在該節末的說明句裡被提到一次（「那屬於另一條線」）。

*   **條文由驗收方於收弧時寫**（規格收尾是驗收方的工作）。
*   **你要交的是實際行為的逐字描述**，見 §4 Q1／Q2／Q3。

---

## 3. 明確不做

*   **Q-016b 的全部**：並行提交、`set_head` 的 compare-and-swap、`SPEC_10` §4.1.1
    的「晚到者自動收斂」。〔量 2026-09-02〕三輪 ×20 並行提交：**值 20／20 全存活**，
    而 20 次逐字 `Commit successful` 只有 **1** 顆進得了 `oo log`。
    **那需要一則本弧沒有的裁定**（收斂還是合法分叉），**碰它就是射程擴大**。
*   **`Error: Commit failed` 沒有座標、沒有錯誤碼**（§2.2.1 家族）。
    **已進 Inbox，本弧不修**——S3 修掉的是它的成因之一，不是那句話。
    **若你的改動讓這條路徑不再觸發，照實說；不要順手改文案。**
*   **`pin` 不傳播**（`commit.md` §2.4：覆寫一個座標不會重算由它導出的座標）。
    既有掛帳，與序無關。
*   **CLI 拼法**（`--pin` ＋ `--grant pin` 的兩步閘）不動。
*   **`Commit.parent`／○ 的接線**（Q-015 已出貨）不動。

---

## 4. 你必須回答的

**Q1 — 吸收怎麼表達，而且不引入序？** 逐字寫出磁碟上長什麼樣。
若採「指名被吸收的注入 id」，說明**兩個並行 `pin` 指名同一批既有注入**時會發生什麼
（預期：兩筆都留，落 S4）。

**Q2 — commit 那一側的 `pin_coords` 從哪來？** 根與工作集之間的 replace 仍需要知道
哪些座標帶特權。S3 之後那份資訊的來源是什麼，**而它如何保證兩個並行 `pin` 的意圖都在**。

**Q3 — 並行雙 `pin` 同座標的實測（無探針，必須手量）。** 附逐字：
`oo status` 的輸出、`oo commit` 的輸出、**不經管線取得的離開碼**。
**要看到的是** ⊥ 落在該座標、rc≠0。

**Q4 — 舊倉相容。** v0.42.0 **會**在磁碟上寫 `.oo/pin_pending`。
一個由 v0.42.0 建立、帶著未提交 `pin` 的倉，被新引擎打開時會發生什麼？
**⚠ Q-017 在這一格付過一次帳**：一個舊引擎讀不懂的新欄位，讓它把整個倉宣告為
`corrupt`——而那句話是假的。**若讀不動，拒絕必須誠實**（說出版本落差，
**不得**報成完整性失效——`REAL_03` §6.8 判例三與其 MUST NOT）。

**Q5 — 動不動編碼版本？** 若注入檔需要新欄位，說明是否需要
`OBJECT_ENCODING_VERSION`／`STORE_LAYOUT_VERSION` 前進，以及
**未遷移的舊倉是否可能被寫進新引擎才看得懂的東西**（Q-017 S7 的教訓：
那句話要寫成不變式——**宣告舊 layout 的儲存不得持有舊引擎驗證不了的東西**）。

---

## 5. 探針

`crates/oo/tests/an_overwrite_that_was_a_coin_flip_probe_test.rs`（驗收方已寫）。
**基線 3 綠 4 紅，五次全跑逐次相同**（同一組 4 紅、同一組 3 綠），
**每一支紅都倒在自己的斷言上**，不是 REACH、不是 helper。

| | 探針 | 基線 | 釘什麼 |
| :-- | :-- | :-- | :-- |
| **R1** | `traversal_order_must_not_decide_the_answer` | **紅** | **不變式本身**：同樣的位元組、不同的檔名，答案不得改變 |
| **R2** | `the_later_pin_wins` | **紅** | 操作者最後說的那一句是他要的那一句 |
| **R3** | `absorption_is_by_coordinate_not_by_file` | **紅** | `x: 3` 是今天的紅；**`y: 7` 是防「整檔吸收」的守衛** |
| **R4** | `privileged_intent_must_not_live_in_a_shared_cell` | **紅（統計，8 回合）** | S3。**不是釘子，照實標示**——它是競速 |
| **G1** | `identity_is_a_red_line` | 綠 | `x: 0` 三物件、標準根 `7038e250…` |
| **G2** | `the_ordinary_path_still_refuses_at_the_evolve_boundary` | 綠 | **對照組**：沒有 `pin` 就沒有兩筆共存的衝突注入 |
| **G3** | `a_single_pin_still_overwrites` | 綠 | 這個操作的用途本身不得壞掉 |

**R1–R3 為什麼要改注入檔名**：自然的缺陷是擲硬幣，而**五次只紅四次的紅不是釘子**。
改名把兩種可能的排序各逼出一次。**那不是造一個不可能的狀態**——任何一對隨機 id
都可能出現，這兩種序正是引擎今天隨機產生的那兩種。**它把 50% 的紅變成 100% 的紅。**

**⚠ `rustfmt` 不得掃這個檔。**
**⚠ 全跑必須 `--no-fail-fast`，且聚合要連失敗的測試名一起留**（只留計數＝把證據丟掉）。

**紅線**：不動身分——`x: 0` 根 `31745ef0…`／**3 物件**／標準根 `7038e250…`。

---

## N. 交付回報（交付方填；本行以上一字不得動）

### N.1 射程逐項對照

* **S1**：注入成員現在於內容內攜帶 intrinsic `id`，並以
  `absorbs: { coordinate: [injection-id...] }` 記錄 evolve 當下逐座標吸收的既有成員；
  檔名只剩本地儲存 key。實作於 `crates/interpreter/src/injections.rs:19-31,69-134,138-173`
  與 `crates/interpreter/src/universe.rs:744-759,814-826`。R1／R2／R3 全綠；實際循序兩次
  pin 的第二筆逐字帶 `absorbs: {"x":[第一筆 id]}`，status 為 `x: 3`。
* **S2**：`load_staged` 已刪除 `pin_pending` 下的 `replace_merge` fold 分支，所有注入一律
  經 absorption 投影後以 meet fold；根與工作集間的 `pin_commit_merge` 保留不動。
  見 `injections.rs:138-173`、`universe.rs:987-1000,1056-1068`。R1 改名兩種走訪序仍同答，
  G3 單一 pin 仍能 commit。
* **S3**：新 pin intent 與值同住該 immutable injection 的 `pin_coords`；load 時對所有成員
  取聯集，commit 的 `pin_coords` 由此而來。新引擎不再建立／改寫 `.oo/pin_pending`，僅讀取
  layout <= 3 留下的 legacy sidecar。見 `injections.rs:23-30,175-212`、
  `universe.rs:346-353,901-954,957-986`。R4 八回合全綠；既有 atomic-write R3 已改釘
  「每個 pin 各自一筆 immutable member、intent 與值同檔、sidecar 不存在」。
* **S4**：未新增鎖、序列化點或共享計數器。以兩個 FIFO 讓兩個行程都在載入相同工作集後
  才同時取得來源，兩個 evolve 均 rc=0；逐字量測見 N.4 Q3，落 D49 衝突。
* **S5**：實際 fold 行為為：先將全體 `absorbs` 按座標取聯集，從被指名成員只投影掉該座標，
  再對所有投影後成員做 meet；無成員吸收另一成員時，同座標不相容值回報
  `#conflict at <coordinate>`。條文仍留給驗收方收弧。

### N.2 順手改動（逐項指名）

* `crates/oo/src/main.rs:1625-1628`：不再丟棄 `load_staged` 的錯誤。這是 Q4 的誠實拒絕
  所必需；否則未知／破損的新注入 frame 會被靜默當成空工作集。
* `crates/interpreter/src/storage.rs:250-269`：已有 `.oo/format` 即視為既有 store，即使尚無
  HEAD／CAS。這是 Q4／Q5 的無 HEAD 舊倉相容所必需；否則僅有未提交 injection 的 layout=3
  倉會在「讀取」時被默默重標 layout=4，繞過 migrate。
* `crates/oo/tests/atomic_write_probe_test.rs:243-275`：舊測試要求 `.oo/pin_pending` 被原子替換，
  與 S3 裁定直接衝突，改釘 immutable injection 內的 intent；不是產品行為順修。
* `crates/oo/tests/atomic_write_probe_test.rs` 與
  `a_commit_that_closes_the_door_probe_test.rs:250-305` 的 layout 守衛由 3 更新為 4，並留下
  Q-016a／D58 理由。object encoding 守衛仍為 5。
* 除上列外無順手改動；未碰 Q-016b、commit HEAD CAS、錯誤文案、pin 傳播、CLI 拼法或 ○。

### N.3 工單哪裡是錯的

沒有發現射程或驗收不變式寫錯。只有一個實作上必須補足的歧義：§2 S1 建議的「注入 id」
不能是今天的檔名，因為 R1 合法地只改檔名而不改位元組；所以 id 必須進入 immutable 內容。
這是把建議具體化，不是推翻裁定。

### N.4 工單指名要你回答的問題

**Q1 — 磁碟表達。** layout=4 的兩筆循序 pin 實際長這樣（body 間空行為 frame 邊界）：

```text
#nlang/store injection
id: "6bb3967958afeca42222204c05aa80b6"
pin_coords: ["x"]
absorbs: {"x":[]}

{ x: 2 }

#nlang/store injection
id: "d1058f1acfad8a1be6c057cc7f579643"
pin_coords: ["x"]
absorbs: {"x":["6bb3967958afeca42222204c05aa80b6"]}

{ x: 3 }
```

`.oo/pin_pending` 不存在。fold 先按座標算被吸收 id 集合，再投影成員，故不使用 paths 排序。
兩個並行 pin 都只指名同一批舊 id，彼此 id 不在對方的 `absorbs` 中；兩筆都留並 meet 成
`#conflict`，正是 S4。

**Q2 — commit 的 `pin_coords`。** 每個 layout=4 injection 自帶 `pin_coords`；load 對全體取
集合聯集（`universe.rs:979-986`）。並行不同座標各寫自己的 immutable member，所以即使寫入
交錯，聯集仍同時含 x、y；沒有最後寫者覆蓋整格。layout<=3 僅為相容而另併入 legacy
`.oo/pin_pending` 的座標。

**Q3 — 並行同座標手量。** 量測目錄 `/tmp/q016a-fifo-current.SOmNEv`，layout=4；FIFO barrier
保證兩行程先各自 load 再同時收到來源。兩個 evolve：`pin_x_rc=0 pin_y_rc=0`。

```text
$ oo status
Standard root dependency: 7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911 (available)
Conflict
#conflict at x
Error: #conflict at x
```

status 離開碼（不經管線）為 **1**。

```text
$ oo commit --grant pin -m same-coordinate
Error: Evolution Conflict: #conflict at x
```

commit 離開碼（不經管線）為 **1**。

**Q4 — 舊倉相容。** 用 v0.42.0 建 layout=3、commit `x:1`、再留下未提交 pin `x:2`
（sidecar `["x"]`）；新引擎 `status` rc=0 顯示 staged `x: 2`，接著
`commit --grant pin` rc=0、`Commit successful`。新 decoder 接受舊 injection body，並以
immutable bytes 的 SHA-256 導出穩定 legacy id（不讓舊檔名重新成為語義）；commit 後移除
sidecar。反向量測：已有 HEAD 的 layout=4 倉給 v0.42.0
開啟時 rc=1，逐字 `store layout declaration "layout=4" is not supported; refusing to open`，
不是 corrupt／integrity failure。

**Q5 — 版本。** `STORE_LAYOUT_VERSION` **3→4**；`OBJECT_ENCODING_VERSION` 維持 **5**。
原因是改的是 `.oo/injections` 容器，不是 CAS object bytes。layout=2/3 仍列入可讀／可 migrate；
舊 layout 的 ordinary evolve 仍寫舊 injection frame，舊引擎可驗證。舊 layout 若要求 `--pin`，
在寫任何 injection 前 rc=1：

```text
Error: this store declares layout=3; pinned injections require layout=4. Run `oo migrate --grant migrate` before evolving with --pin
```

量測 injection_count=0。故不變式成立：**宣告舊 layout 的儲存不得持有舊引擎驗證不了的東西。**

### N.5 探針

`an_overwrite_that_was_a_coin_flip_probe_test`：**7/7**。

* R1 traversal order：綠。
* R2 later pin wins：綠。
* R3 coordinate-not-file absorption：綠。
* R4 immutable privileged intent（8 回合統計）：綠。
* G1 identity、G2 ordinary conflict boundary、G3 single pin：全綠。

**未修改、未 rustfmt 此探針檔。** 修改的是既有 `atomic_write_probe_test` 的一支過期
`pin_pending` 測試，理由逐項見 N.2。

### N.6 數字

全樹 `cargo test --workspace --release --no-fail-fast`（網路測試需非沙箱 bind/connect）：

| 輪 | targets | passed | failed | 失敗測試名 |
| :-- | --: | --: | --: | :-- |
| 1 | 223 | 2139 | 0 | 無 |
| 2 | 223 | 2139 | 0 | 無 |
| 3 | 223 | 2139 | 0 | 無 |

補充誠實紀錄：沙箱內一輪因 `oo node serve` 得 EPERM 造成 18 targets 假紅，未計；第一次
非沙箱嘗試有既有 `r3_a_root_written_into_a_pre_sentinel_repo_stays_self_contained` 偶發紅，
同 binary 精確重跑立即綠，故也未計入上表，之後三輪完整全綠。

* conformance：`162 vectors, 162 pass, 0 fail`。
* `x: 0` root：
  `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。
* `x: 0` CAS objects：**3**。
* standard root：
  `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`（available）。
* 基線 known-answer：`~%Math./add (1,2)` → `3` rc=0；對照
  `~%Math./add (1,"x")` → `_|_ (%cause: #conflict)` rc=0；離開碼均直接取得，未經管線。

### N.7 你認為需要改規格之處

`SPEC_10` §2.2.2 應補 pin 路徑的規範性條文，至少明定：

1. pin 在 evolve 時逐座標吸收當時已存在的工作集成員，不得以檔案為吸收單位；
2. 吸收關係不引入全序，工作集仍是集合，fold 只做 meet；
3. 兩筆並行、同座標且互不可見的 pin 都保留，依 D49 回報該座標 ⊥，不得 LWW；
4. pin intent 必須與它修改的值同住不可變工作集成員，commit 的特權座標取成員聯集；
5. 根與工作集之間仍按 pin 座標做 replace，而非把整個工作集改成 replace fold；
6. layout/version 閘須保證舊 layout 不會收到其宣告的舊引擎無法驗證的 injection frame。
