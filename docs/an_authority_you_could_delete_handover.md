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

* **S1**：discharge 標籤集與它所描述的值同住不可變注入成員。layout=5 的框在 `absorbs:` 之後多一行 `effect_tags: <u8 bits>`；提交側對全體成員取聯集。每個寫者只把自己這次 discharge 的集合寫進自己鑄的那一筆，所以兩個並行 discharge 各自存活，聯集在 load／commit 時才做。新引擎在現行 layout **不再建立或改寫** `.oo/effect_pending`。見 `injections.rs` 的 `Injection.effect_tags`／`write`／`load_all`，與 `universe.rs` 的 `session_effect_tags`。**照抄**：意圖搬進成員不使它免於竄改——改寫成員仍在斷言層。得到的是不再可分離：移除授權事實必須改寫承載值本身的那個物件。
* **S2**：成員上的 `effect_tags` 讀不出來（JSON 不是 u8）→ `effect_tags unreadable`，rc=1，不落地。未知 metadata 行仍 `unknown metadata`，**未放寬** `injections.rs` 對多餘行的 `bail`。兩種都不會變成「沒有 discharge」。標記仍只在 `effect_pending` 聯集非空時寫入 `privileged_effect`，不因 grant 在場而標記。
* **S3**：`STORE_LAYOUT_VERSION` **4→5**；可遷移來源閉集改為 `[2, 3, 4]`。layout=4 收到需要新欄位的寫入（新引擎的 discharged evolve）在寫下任何成員之前拒絕，注入數 0，指名 `oo migrate --grant migrate`。layout=4 上的普通 evolve 仍寫 layout=4 框（`id`／`pin_coords`／`absorbs`，**沒有** `effect_tags:`），真 `v0.43.0` `status` rc=0、`commit` rc=0。`--pin` 在 layout=4 仍寫舊 pin 框，真 `v0.43.0` 讀得到並提交得了。`v0.43.0` 開 layout=5：`store layout declaration "layout=5" is not supported; refusing to open`，不是 corrupt。
* **S4**：未碰 `.oo/abandoned`、未示範 `~%Io./write_file` 進 `.oo/`、未碰 Q-016b／HEAD CAS／`pin` 的吸收語義。為了在 current 變成 5 之後 layout=4 仍能表達 pin，把 pin 的寫入閘從 `layout_declaration_is_current` 改成 `layout_writes_pin_frame`（已知且 ≥4）。那是讓 S3「舊 layout 不得收到它宣告不了的東西」在 pin 這一側保持為真，不是改 pin。

### N.2 順手改動（逐項指名）

* `crates/oo/tests/atomic_write_probe_test.rs` `p2`：現行 layout 釘 `layout=4`→`layout=5`，註解補 Q-040／D60。該針自己寫著「Whoever moves it next updates this line」。
* `crates/oo/tests/a_commit_that_closes_the_door_probe_test.rs`：`r5` 遷移目的地與 `g1` REACH 的現行 layout 字面由 4 改 5。**未**改 `g4` 手寫 encoding-3／舊 layout 的倉。
* `crates/oo/src/main.rs`：註解把 discharge 意圖的住處從單一 sidecar 改成成員 `effect_tags` ＋ layout≤4 遺留 sidecar。行為閘仍讀 `universe.effect_pending`（現為聯集）。
* 未 rustfmt `storage.rs`（整檔會重排不相干函數）。**Q-040 探針一字未動。**

### N.3 工單哪裡是錯的

沒有發現射程或釘子寫錯。S3「普通 discharge 仍須寫舊形式」按字面會與「需要新欄位的寫入必須在寫成員之前拒絕」打架：新引擎若在 layout=4 再寫 `.oo/effect_pending`，就是把本弧要拿掉的可分離格子寫回去。實作取 Q-016a 的類比——**新** discharged evolve 在 layout=4 拒絕（注入 0）；v0.43.0 自己寫下的 sidecar 與 layout=4 注入框仍是舊形式，真二進位讀得到、提交得了（Q3／Q4）。這不是推翻裁定，是讓兩句同時為真時優先「不得寫它宣告不了的欄位、也不得把洞寫回去」。

### N.4 工單指名要你回答的問題（Q1–Q4）

**Q1 — 磁碟形狀。** layout=5、一次 `runPure` 讀時鐘，成員逐字：

```text
#nlang/store injection
id: "93f8b671428bb14de7b67edd4a80790b"
pin_coords: []
absorbs: {}
effect_tags: 1

{ p: 1788664707017 }
```

`.oo/` 下列沒有 `effect_pending`。`effect_tags: 1` 是 `EffectTag::IO` 的 bits（與舊 sidecar 同一個 u8）。普通 evolve 仍寫同一框、`effect_tags: 0`。兩次循序 discharge（`io` 再 `nondet`）是兩筆成員，分別 `effect_tags: 1` 與 `effect_tags: 2`；無 grant 的提交 rc=1，逐字 `discharged #io | #nondet`。未知欄位仍 `unknown metadata`，未放寬。

**Q2 — 讀不出標籤。** 把 `effect_tags: 1` 改成 `effect_tags: not-a-tag-set` 之後，`oo commit --grant effect_override:io -m x`：

```text
Error: injection 224c6687ebabd475d970588568093812: effect_tags unreadable
```

離開碼（不經管線）**1**。HEAD 未動。另測在 `effect_tags:` 後加 `extra: 1`：

```text
Error: injection 5454e5bd77517c1377c0b902e2c96b63: unknown metadata
```

離開碼 **1**。兩者都不是「沒有 discharge」。

**Q3 — 跨版本矩陣（真 `/home/gali/nlang-baselines/v0.43.0-verify/target/release/oo`）。**

1. 新倉：`.oo/format` 為 `layout=5`。
2. `v0.43.0` 造 `layout=4` 倉，新引擎普通 `evolve y: 2` 寫出**沒有** `effect_tags:` 的 layout=4 框；宣告仍是 `layout=4`。真 `v0.43.0` `status` rc=0 顯示 `y: 2`，`commit -m from-new` rc=0 `Commit successful`。
3. 同一類 `layout=4` 倉，新引擎 `evolve --grant effect_override:io`（`runPure` 讀時鐘）→ **rc=1**，注入數 **0**，sidecar 不存在，宣告仍 `layout=4`。逐字：

```text
Error: this store declares layout=4; a discharged injection cannot land until the layout is current. Run `oo migrate --grant migrate`
```

4. `v0.43.0` 開 layout=5：`log`／`status` 皆 rc=1，逐字 `store layout declaration "layout=5" is not supported; refusing to open`。不是 `#caid_mismatch`、不是 corrupt。

**Q4 — 遺留 sidecar。** `v0.43.0` 在 layout=4 留下 `.oo/effect_pending` 內容 `1`。新引擎無 grant 提交 rc=1，逐字 `discharged #io`（閘不比今天鬆）。帶 `--grant effect_override:io` 則落地且 log 有 `privileged_effect`。刪掉該 sidecar 之後無 grant 提交 rc=0——與今天 v0.43.0 的洞相同，未遷移的倉沒有被收得更鬆，也沒有被收得更緊。遷移之前的舊注入沒有 per-member 標籤，閘只靠這份 sidecar。

### N.5 探針（R1–R4／G1–G3；**若你改了探針檔，逐行說明為什麼**）

`an_authority_you_could_delete_probe_test`：**7／7**。未修改、未 rustfmt 此檔。

* R1 無 grant 仍拒：綠。
* R2 刪 sidecar 仍拒：綠（layout=5 成員帶 `effect_tags: 1`，`.oo/` 根下沒有可刪的格子）。
* R3：R2 已拒，早退；綠。
* R4 十回合並行兩種 discharge、只持 `io` 的提交皆 rc=1：綠。
* G1 身分、G2 普通提交無標記、G3 出示能力仍落地且有 `privileged_effect`：全綠。

### N.6 數字（全樹 ×3：targets／passed／failed／**失敗的測試名**；conformance；身分三項）

全樹 `cargo test --workspace --no-fail-fast -- --test-threads=1`：

| 輪 | targets | passed | failed | 失敗測試名 | `^error` |
| :-- | --: | --: | --: | :-- | --: |
| 1 | 224 | 2146 | 0 | 無 | 0 |
| 2 | 224 | 2146 | 0 | 無 | 0 |
| 3 | 224 | 2146 | 0 | 無 | 0 |

補充誠實紀錄：WSL 在第一次全樹中途被關掉，`target/debug/deps` 留下 29 個截斷的 interpreter 測試二進位（`file` 報 `data`，執行是 `Exec format error`）。刪掉那些檔後重編，上表三輪才算。那 29 支不是本弧的產品紅。

* conformance：`162 vectors, 162 pass, 0 fail`。
* `x: 0` root：`31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。
* `x: 0` CAS objects：**3**。
* standard root：`7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`（available）。
* 基線 known-answer：`~%Math./add (1,2)` → `3` rc=0；對照 `~%Math./add (1,"x")` → `_|_ (%cause: #conflict)` rc=0；離開碼均直接取得，未經管線。新引擎同樣兩句。新倉宣告 **`layout=5`**。

### N.7 你認為需要改規格之處

`REAL_02` §5.1.1 已有「宣告未推進者不得收到它宣告不了的東西」；本弧是那一款的第二次行使，條文不必因本弧重寫。收弧時 CHANGELOG／SPEC_00 需記 `layout=5` 與 90 天時鐘重啟（D60；時鐘是切版的事，不是本交付）。`oo migrate` 成功句仍說「只讀 layout=2 的引擎（oo v0.41.0）此後打不開」——對 layout=5 **仍真**（v0.41.0 也打不開），但不完整（擋在門外的還有 v0.43.0）。建議切版時改成指名最新被鎖在門外的那一版，與 Q-017 S8 同一類誠實性。身分未動，標準根表不必改。
