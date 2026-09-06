# 工單：一個你刪得掉的授權

**Queue ID**：**Q-040**（Ready 表第 1 列，Active）。由 Inbox 的 `interrupt-candidate` 升格。
**基線**：`v0.43.0`，`/home/gali/nlang-baselines/v0.43.0-verify/target/release/oo`
（known-answer `~%Math./add (1,2)` → **`3`**，**對照** `add (1,"x")` → **`_|_` (%cause: #conflict)`**）。
**你也要先做一次 known-answer 加對照。⚠ 取離開碼不得經過管線。**

**裁定（皆已裁）**：**D58**（形狀：特權意圖與它所修改的值同住不可變成員）／
**D60**（價格：**現在就花掉 `layout=5`**，不併批）／**D57**（為什麼審計不住工作區）。

---

## 1. 這一弧修的是一個「已經修過一次、但只修了一半」的洞

2026-07-26 為 `pin` 做過一次修補。`main.rs:1050` 的註解逐字：

> the capability must be presented **HERE, through the trusted channel** — not inferred
> as authority from the durable intent … Trusting durable intent once let an
> **unprivileged program obtain `#pin` semantics and falsely mark its commit**.

同一段註解指出那個目錄 **any n/ program can write (`~%Io./write_file`)**。
**那個論證從未被套用到 `effect`。**

〔量 2026-09-06，基線二進位〕**決定性，不需要並行**：

```
oo evolve --grant effect_override:io  (runPure 包住一次時鐘讀取)
oo commit                             → rc=1  「discharged #io」          ← 閘是對的

rm .oo/effect_pending
oo commit                             → rc=0  Commit successful
                                      → 而該 commit 連 privileged_effect 標記都沒有
```

⟹ **特權 discharge 過的內容以零能力、零審計痕進入歷史。**

〔量，同批〕**競速版本**（這是它最初被發現的樣子）：兩個並行
`evolve --grant effect_override:{io,nondet}` ⟹ 注入 **2 個**而該格子**從不取到聯集**；
**10 次試驗，只持 `io` 的提交成功 5 次**，落地的根含兩個座標。
**循序對照組**：該格子取聯集 `1` → `3`，同一個提交 **rc=1**（`discharged #io | #nondet`）。

---

## 2. 射程（S1–S4）

### S1 — discharge 的意圖與它所描述的值同住不可變成員

*   **不變式**：**特權事實不得住在一個與它所描述的值可分離的位置。**
*   **形狀由 D58 給定，與 `pin_coords` 同構**：每個成員宣告自己的 discharge 標籤集；
    提交側取**成員聯集**。
*   **⚠ 不得過度宣稱**（照抄進你的回報）：意圖搬進成員**不使它免於竄改**——
    改寫成員仍在斷言層。得到的是**不再可分離**：移除授權事實必須改寫**承載值本身的那個物件**。

### S2 — 提交側的閘寧可拒絕，不可放行

*   標籤集**讀不出來**（成員損壞、未知形式）時**必須拒絕**，**不得**當成「沒有 discharge」。
    今天的失效模式正是後者：讀不到 ⟹ `None` ⟹ 閘不開火。
*   **不得**因能力在場而標記（`SPEC_08` §6.2「標記必須反映事實而非旗標」）。

### S3 — `layout=5`，且舊 layout 不得收到它宣告不了的東西

**D60 已裁：現在就花。** 依 `REAL_02` §5.1.1（本專案上一版剛寫進去的那一款）：

*   `layout=4` 的儲存收到需要新欄位的寫入時，**必須在寫下任何成員之前拒絕**，並指名 `oo migrate`。
*   `layout=4` 上的**普通** evolve 與**普通** discharge 仍須寫舊形式，
    且 **真 `v0.43.0` 二進位讀得到、提交得了**——**這一項要你手量並附逐字紀錄**。
*   舊引擎打開 `layout=5` 必須是**誠實拒絕**（版本落差），**不得**報成完整性失效（`REAL_03` §6.8）。

### S4 — 明確不做

*   **`.oo/abandoned`**（`append_abandoned_file`，`universe.rs:1224`）。**同一個形狀**
    （load→push→整檔寫），但曝露面不同：`rollback` 對髒工作區拒絕，且兩個並行 rollback
    放棄**同一顆** digest 並去重。**殘留窗口未重現**（commit 先清注入、稍後才清它）。
    **本弧不碰**；若你順手動到它，逐項說明。
*   **`~%Io./write_file` 到不到得了 `.oo/`** ——威脅模型是 `REAL_01` §7.3 的，不需要那個示範；
    但本弧也**不提供**那個示範，別在回報裡宣稱它。
*   Q-016b、HEAD 的 CAS、`pin` 的任何部分。

---

## 3. 你必須回答的

**Q1 — 標籤集在磁碟上長什麼樣？** 逐字。以及**未知欄位的拒絕行為有沒有被弱化**
（`injections.rs:105` 今天對任何未知 metadata 行 `bail`，**那個嚴格性是對的，不得放寬**）。

**Q2 — 讀不出標籤時發生什麼？** S2 的核心。附逐字輸出與**不經管線的離開碼**。

**Q3 — 跨版本矩陣，真二進位。** 四格逐字：新倉 `layout=5`／
`layout=4` 倉被新引擎普通寫入後**真 `v0.43.0` 仍讀得到並提交得了**／
`layout=4` ＋ 需要新欄位的寫入 ⟹ 寫下任何成員之前拒絕（附**注入數為 0** 的證據）／
`v0.43.0` 開 `layout=5` ⟹ 誠實拒絕且**不是 corrupt**。

**Q4 — `v0.43.0` 留下的 `.oo/effect_pending`（layout ≤ 4）怎麼讀？**
與 Q-016a 的 legacy sidecar 同型。**遷移之前的舊倉，其閘不得比今天更鬆。**

---

## 4. 探針

`crates/oo/tests/an_authority_you_could_delete_probe_test.rs`（驗收方已寫）。
**基線 4 綠 3 紅，五次全跑逐次相同**，每一支紅都倒在自己的斷言上。

| | 探針 | 基線 | 釘什麼 |
| :-- | :-- | :-- | :-- |
| **R1** | `the_gate_refuses_without_the_capability` | **綠** | **對照組**：不動任何東西時閘是對的。**它若變紅，其餘全部在量錯的東西** |
| **R2** | `deleting_a_sidecar_must_not_delete_the_authority_requirement` | **紅** | **釘子**。刪掉 `.oo/` 下的可分離格子後，無 grant 的提交仍須被拒 |
| **R3** | `a_discharged_commit_is_never_unmarked` | **紅** | 即使落地也不得無標記。**與 R2 各自獨立失效** |
| **R4** | `two_concurrent_discharges_both_survive` | **紅（統計，10 回合）** | 競速版本。**不是釘子，照實標示** |
| **G1** | `identity_is_a_red_line` | 綠 | `x: 0` 三物件、標準根 `7038e250…` |
| **G2** | `an_ordinary_commit_needs_nothing` | 綠 | 沒有 discharge 就沒有閘、沒有標記 |
| **G3** | `presenting_the_capability_still_lands` | 綠 | 這個操作的用途本身不得壞掉 |

**R2 為什麼用「刪檔」而不是競速**：這一格的威脅模型是 `REAL_01` §7.3
（**斷言層對能寫入它的對手不提供任何保證**），而那正是 **D57** 把同意標記歸 ● 的理由。
**刪一個工作區檔案不是在造不可能的狀態，那就是那條規則所設想的對手。**
且它把一個統計紅變成決定性紅。**R2 不指名檔案**——它移除 `.oo/` 下所有非耐久結構的
平常檔，也就是「值旁邊的格子」這個**類別**。若意圖與值同住，那裡就沒有東西可刪，斷言自動成立。

**⚠ 全跑必須 `--no-fail-fast`，聚合要連失敗的測試名一起留。**
**⚠ `rustfmt` 不得掃探針檔**（樹裡既有的探針普遍不 fmt，那是刻意的）。
**紅線**：不動身分——`x: 0` 根 `31745ef0…`／3 物件／標準根 `7038e250…`。

---

## N. 交付回報（交付方填；本行以上一字不得動）

### N.1 射程逐項對照
### N.2 順手改動（逐項指名）
### N.3 工單哪裡是錯的
### N.4 工單指名要你回答的問題（Q1–Q4）
### N.5 探針（R1–R4／G1–G3；**若你改了探針檔，逐行說明為什麼**）
### N.6 數字（全樹 ×3：targets／passed／failed／**失敗的測試名**；conformance；身分三項）
### N.7 你認為需要改規格之處
