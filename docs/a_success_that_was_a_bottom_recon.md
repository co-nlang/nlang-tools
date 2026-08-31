# Q-017 偵察 — 一次回報成功的提交，其實是一個 `_|_`

> **Queue ID**：`WORK_QUEUE` Q-017（Active，偵察）
> **基線**：引擎 `v0.41.0` 標籤二進位
> `/home/gali/nlang-baselines/v0.41.0-verify/target/release/oo`
> （`--version` 印 `oo v0.41.0`；行為：`oo run ka.n -o r`，
> `r: ~%Math./add (1,2)` → `3`，對照 `add (1,"x")` → `_|_ (%cause: #conflict)`）。
> 工作樹 `nlang-tools` `dev`（`--version` 印 `oo v0.40.0-743-gfc17cee+`，
> **同一組 known-answer**；版本用行為確認）。
> **這是偵察，不是實作。** 甲／乙／丙不選邊。
> **未推翻** brief §1.1–§1.3；覆核帶指令。
>
> **⚠ 縮寫**：本文 `CAS` 只指 Content-Addressed Storage。
> 離開碼一律不經管線（`set +e; out=$("$OO" … 2>&1); rc=$?`）。
> 引用內建先換引數換答案。

先讀了 `universe.rs`（`workset_bottom`／`evolve`／`load_staged`／`commit`／`project_for_commit`）、
`injections.rs` `fold`、`main.rs` `run_evolve`／`run_status`／`run_commit`／`format_conflict_where`、
`SPEC_10` §2.2.1／§2.2.2／§2.3／§4.1.2、`commit.md` §2.1.3。
B1／B2／D49 不重開。一次性腳本跑完已刪，樹乾淨。

---

## 0. 九題各一句

| | 答案 |
| :-- | :-- |
| **Q1** | `workset_bottom` **只**在 `load_staged` 的 fold 失敗時設定。A 類到不了它，因為欄位裡的 ⊥ 仍是一個 Combo，fold／staged-meet 回 `Ok`。A 的 ⊥ 有的在 evolve 求值時就寫進欄位，有的遲到 `project_for_commit`。 |
| **Q2** | `c: c + 1`：**commit 時**才變成 ⊥。`x: 1 & 2` 與 `add (1,"x")`：**evolve 求值時**已是 ⊥，status／○／根同一份。三形 `commit` 皆 rc=0、`Commit successful`。 |
| **Q3** | `status` 印未觀測的 staged（Thunk 印原文，Bottom 印 `_|_`）。`commit` 走 `project_for_commit`（強制純 thunk，⊥ 留下）。**不是同一個求值深度。** 這就是 `c: c + 1` 兩面兩個答案的成因。 |
| **Q4** | Combo 裡可以有任意多個 ⊥ 座標（〔量〕`x` 與 `y` 兩個、巢狀 `a.b`／`a.c` 兩個，都進了根）。`workset_bottom` 是單數，表達不了這個。§2.2.1 說必須到葉 ⟹ **報告該列全部**；B 路徑的 fold 今天在第一個 ⊥ 停下（§2.2.2 自陳的留白）。 |
| **Q5** | `#divergent`／`#conflict`／`#missing_key` 都會進根。`#unprovided_builtin` 不是使用者源碼能鑄的（標準根點名、引擎交不出）。`#incomplete` 不是 BottomCause；fuel=0 預設 `#blur`，進根的是 blur／thunk，不是 ⊥。 |
| **Q6** | **看哪一種 ○。** 工作集 ○：A2／A3／`#missing_key` 的 combo **已有** `~%__nlang_bottom`；A1 只有 thunk。提交 ○ 的 combo 是 `{}`（D52 空快照），⊥ 在 CAS 根物件上。B 路徑的兩顆 ○ 是 `k: 1`／`k: 2`，**沒有** ⊥——⊥ 是 fold 算出來的。 |
| **Q7** | 甲 ~50–90 行、乙 ~70–110、丙 ~90–140。都不選。丙與 B1「要修的不是中止」正面衝突；若把旗標讀成「操作者決定固化」，那是回頭請裁的材料，不是本偵察能選的。 |
| **Q8** | 兩條條文今天靠「⊥ 是整顆 meet 還是欄位值」共存。把 B 改成 A＝放寬 §2.2.2「提交必須失敗」；把 A 改成 B＝違反 B1。不選。 |
| **Q9** | 基線必紅：`c: c + 1` 之後 `oo commit` 的輸出**不含** `#divergent` 也不含座標 `c`（今天逐字只有 `Commit successful: hash:…`、rc=0）。可在本引擎鑄出，不剝註記、不造不可能狀態。 |

---

## Q1 — `workset_bottom` 誰設定，A 為什麼到不了

**設定點只有一處。** 欄位宣告 `universe.rs:347`。寫入：

| 位置 | 做什麼 |
| :-- | :-- |
| `universe.rs:371` | `new` 時 `None` |
| `universe.rs:921` | `load_staged` **先清掉** |
| `universe.rs:959` | `injections::fold` 回 `Err(d)` 時 `Some(d)`，staged 清空、injections **留在磁碟** |
| `universe.rs:465` | `evolve` 開頭若已是 `Some`，直接 `Err`（不再寫入） |
| `universe.rs:997`／`main.rs:1037`／`main.rs:863` | 讀者（commit／CLI commit／status） |

`injections.rs:69`–`:87` 的 `fold`：兩個 Combo 做 `unify`，得到 `Value::Bottom` 才 `Err`。
得到帶 ⊥ **欄位**的 Combo 是 `Ok`。

**會觸發設定的最小序列**（不需要真並行；複製第二份注入即可）：

```
k: 1          →  oo evolve p.n          # rc=0，一顆 injection
把另一個倉裡 k: 2 的 injection 檔拷進 .oo/injections/
oo status     # RC=1  Conflict / #conflict at k
oo commit     # RC=1  Error: Evolution Conflict: #conflict at k
              # 無 HEAD、兩份 injection 仍在、○ 仍在
```

真並行同一形：兩個行程同時 `evolve`，兩邊 rc=0，然後 status／commit 同上。
〔量，標籤二進位。離開碼不經管線。〕

循序對照（§2.2.1 仍綠）：`k: 1` 然後 `k: 2` → 第二次 evolve
`Error: Evolution Conflict in "q.n": #conflict at k`、rc=1、只留第一份 injection。
這條路走 `evolve` 的 staged-meet／G2-S（`universe.rs:664`／`:757`），**從不碰** `workset_bottom`。

**A 類到不了它，兩個原因疊在一起：**

1. **資料結構裝的是「整份工作集 meet 為 ⊥」**，不是「某個座標的值是 ⊥」。
   `{ x: _|_ }` 對 fold 與 `unify(staged, incoming)` 都是 Combo。
2. **有的 ⊥ 根本還沒誕生。** `c: c + 1` 在 evolve 時是 Thunk（`universe.rs:535`–`:541`，
   `computing` 把自指收成 thunk 而不是當場 ⊥）。⊥ 要到 `project_for_commit`
   （`lib.rs:3778`–`:3784`：純 thunk 強制後若是 Bottom 就留下）才寫進根。

不是判斷寫錯——`run_commit:1037` 的檢查對它看得到的那一格是對的。A 從不進那一格。

---

## Q2 — A 類三形各自何時變成 ⊥

〔量，標籤二進位。`add` 先證明呼叫：`add (1,2)` → `3`，`add (1,"x")` → `_|_ (%cause: #conflict)`。〕

| 形 | evolve | status（evolve 後） | 工作集 ○ | commit | 根物件 |
| :-- | :-- | :-- | :-- | :-- | :-- |
| `c: c + 1` | 靜默 rc=0 | `{ c: c + 1 }` | thunk，**無** `~%__nlang_bottom` | `Commit successful` rc=0 | `{ c: { ~%__nlang_bottom: #divergent } }` |
| `x: 1 & 2` | 靜默 rc=0 | `x: _|_ (%cause: #conflict)` ＋ Rust `Debug` message | **已有** `~%__nlang_bottom: #conflict` ＋同一則 message | 同上 rc=0 | 同一份 ⊥ 進根 |
| `y: ~%Math./add (1, "x")` | 靜默 rc=0 | `y: _|_ (%cause: #conflict)`（無 message） | **已有** `~%__nlang_bottom: #conflict` | 同上 rc=0 | 同一份 ⊥ 進根 |

**⟹ 報告不能只掛在 commit。** 後兩形的 ⊥ 在 evolve 求值就進了工作集；
第一形的 ⊥ 是提交觀測造出來的。三形今天 commit 面都是成功謊。

可重跑（離開碼不經管線）：

```bash
OO=/home/gali/nlang-baselines/v0.41.0-verify/target/release/oo
D=$(mktemp -d); cd "$D"
printf '%s\n' 'c: c + 1' > u.n
"$OO" evolve u.n; echo evolve:$?
"$OO" status; echo status:$?
cat .oo/savepoints/*
"$OO" commit -m a1; echo commit:$?
grep -R '__nlang_bottom' .oo/objects/sha256 --exclude-dir=. | grep -v '~%Math'
```

---

## Q3 — 為什麼 `status` 對 `c: c + 1` 印原文、對 `x: 1 & 2` 印 `_|_`

**不是兩個 status。是 staged 裡放了兩種值，而 status 不觀測。**

`run_status`（`main.rs:837`–`:878`）在沒有 `workset_bottom` 時印

```
Value::Combo(universe.staged.clone()).to_nlang(0)
```

`to_nlang`（`value.rs:3189`–`:3194`）：Thunk **印源表達式**（「print_what_can_be_read」）；
Bottom（`:3164`）印 `_|_ (%cause: #…)`，若有 `message` 就附上。

`commit` 另走 `project_for_commit`（`universe.rs:1049`，本體 `lib.rs:3760`）：
純 thunk 強制一遍，結果是 Bottom 就留下，否則 **把原文 thunk 放回**（D46：`#pure` 不得用答案取代定義）。

所以：

*   `c: c + 1` 在 staged 是 thunk → status 印 `c + 1`；commit 強制得到 `#divergent` → 根是 ⊥。
    **同一個倉，提交前／提交後兩個答案。** 這就是 §2.2.1「各回報面一致」在本卡上的成因。
*   `x: 1 & 2` 在 evolve 的 `engine.eval`（`universe.rs:516`）已經是 Bottom，
    再放進 incoming Combo（`:696`），與空 staged meet 仍是 Combo → status 與根一致（都是 ⊥），
    只是誰也不報告。

求值深度：**不是同一個。** status 零強制；commit 對純 thunk 強制一次。
`workset_bottom` 那條 status 路徑（`:863`，印 `Conflict` 然後 bail）A 類走不到。

巢狀對照〔量〕`a: { b: 1 & 2, c: 3 & 4 }`：status 印未化約的 `1 & 2`（內層被收成 thunk），
commit 後根裡 `a.b`／`a.c` 都是 `#conflict`。同一條深度差，換了一層。

---

## Q4 — 一次提交能有多少個 ⊥ 座標

**欄位值：無上界，Combo 裝得下。** 〔量〕

```
x: 1 & 2
y: 3 & 4
```

evolve rc=0；status 兩行 ⊥；工作集 ○ 兩個 `~%__nlang_bottom`；commit rc=0；
根物件兩個座標都在。巢狀兩葉同樣兩個都進根。

**`workset_bottom`：恰好 0 或 1。** 它是 `Option<BottomDetail>`。
三個座標各自撞 ⊥ 而工作集仍是 Combo 時，它維持 `None`。
B 路徑 fold 在**第一個** `Value::Bottom` 停下（`injections.rs:77`），
座標隨本地 id 遍歷序變——這正是 §2.2.2 自陳留白〔原文見 Q8〕。

§2.2.1「必須到葉」指的是**實際發生矛盾的那一個座標，不得只是頂層欄位**。
一次提交裡有三個葉，列一個是漏報。A 類的材料已經在 Combo 樹上；
要列全部，是一次走訪，不是新的資料結構。B 類今天結構上列不出全部。

---

## Q5 — 還有哪些 `%cause` 到得了提交邊界

逐個試（標籤二進位）。內建皆先換引數：

| `%cause` | 源 | evolve／status | 根 |
| :-- | :-- | :-- | :-- |
| `#divergent` | `c: c + 1` | thunk／原文 | **是**，`~%__nlang_bottom: #divergent` |
| `#conflict` | `x: 1 & 2`、`add (1,"x")` | 已是 ⊥ | **是** |
| `#missing_key` | `out: {{ a: 1 }}.b` | 已是 ⊥（`run` 對照：`.a` → `1`，`.b` → `_|_ #missing_key`） | **是** |
| `#missing_key` | `~%NoSuchModule./foo 1`、`~%Math./nope (1,2)` | 已是 ⊥ | **是**（閉包模組缺鍵，不是 `#unprovided_builtin`） |
| `#unprovided_builtin` | 使用者源碼 | **鑄不出來** | 該 cause 的唯一寫入是 `lib.rs:3190`：「標準根點了這個 builtin，本引擎交不出」。要一顆與引擎不一致的標準根，不是本卡三指令能造的 |
| `#incomplete` | 不是 `BottomCause` 成員 | fuel=0 預設 `#blur`：`out` 變成 blur（`1..1000`）或留下 thunk（`c: 1` / `out: c + 1`，D46） | **不是 ⊥**。blur／thunk 進了根；Config 依 O37 未進 commit |

⟹ 提交邊界今天放行的 ⊥ 至少有 `#divergent`／`#conflict`／`#missing_key`。
報告面若只認 `#conflict`（`universe.rs:1001` 對 `workset_bottom` 就是這樣分的），
A 類的 `#divergent`／`#missing_key` 連那條錯的路也走不進去。

---

## Q6 — 撞 ⊥ 的那次觀測／注入，○ 上有沒有記下

B1：「固化的最小單位是 ○」。今天每次注入與每次提交都鑄 ○（D43／D52）。

〔量 `.oo/savepoints/`〕

| 事件 | 那顆 ○ 的 combo |
| :-- | :-- |
| evolve `c: c + 1` | `{ c: { ~%__nlang_thunk … expr: c + 1 } }` — **沒有** bottom |
| evolve `x: 1 & 2` | `{ x: { ~%__nlang_bottom: #conflict message: "Incompatible types: Atom(Int(1), …" } }` — **有** |
| evolve `add (1,"x")` | `{ y: { ~%__nlang_bottom: #conflict } }` — **有** |
| 隨後的提交 ○ | `parents: <tip>`、`commit: <digest>`、combo `{}` — **空的**。⊥ 在 CAS 根物件，不在這顆 ○ |
| B 路徑兩次 evolve | `{ k: 1 }` 與 `{ k: 2 }` — **沒有** ⊥。⊥ 只存在 fold 的記憶體結果裡 |

**報告材料並不都在磁碟上的 ○。** A2／A3／`#missing_key` 的工作集 ○ 已經夠印座標與錯誤碼
（且不要抄 `message` 裡的 Rust Debug——brief §1.3／Inbox）。A1 的工作集 ○ 只有 thunk，
要到 commit 觀測才有 ⊥。B 的 ○ 是兩筆合法注入，⊥ 是它們的 fold，不寫在任何一顆 combo 裡。

這會改報價：若實作只掃 `savepoints/` 的 `~%__nlang_bottom`，會漏掉 A1 與整個 B。
A 類要掃的是 **commit 即將寫入的根**（`project_for_commit` 之後的 Combo）。
B 類的材料是 `workset_bottom`，已經在記憶體。

---

## Q7 — 甲／乙／丙逐一報價（不選）

共同前置（三種都要）：從 Combo 收集 `(座標, BottomCause)`，**不讀 `message`**
（§2.2.1 不得洩漏實作表示；`format_conflict_where` 已不印 message，`main.rs:15`）。
走訪約 40–60 行（`value.rs` 或 `universe.rs`，形狀接近 `project_for_commit` 的分軸迴圈）。
`format_conflict_where` 可重用。

`Universe::commit` 的產品呼叫者：`main.rs:1096` 一處，外加
`crates/interpreter/tests/stage2_open_term_test.rs:102`。
CLI 改了、庫沒改，則 `oo` 之外的呼叫者仍是今天的沉默成功。

| | 甲 報告後落地，rc=0 | 乙 報告後落地，rc≠0 | 丙 預設拒絕，`--allow-bottom` 才落地 |
| :-- | :-- | :-- | :-- |
| **一句話** | 印葉座標＋錯誤碼，提交仍成功 | 歷史進了，離開碼說帶著 ⊥ | 預設中止；旗標才是「操作者決定」 |
| **行數** | 走訪 40–60 ＋ CLI 印 15–30 ⟹ **~50–90** | 甲 ＋ 落地後讓 CLI 回 `Err`（或 `commit` 回多一個「帶 ⊥」）⟹ **~70–110** | 甲 ＋ clap 旗標／help ＋ 預設在 `put_commit` 前 return Err ⟹ **~90–140** |
| **檔** | `universe.rs`、`main.rs`，可能 `value.rs` | 同左 | 同左 ＋ `Commands::Commit` 的 clap |
| **既有測試** | 產品測試幾乎不 commit 含 ⊥ 的宇宙（`1 & 2` 在 interpreter 單元測試裡，不經 `oo commit`）。`contains("Commit successful")` 的探針仍綠。估 **0–2 紅**（若有人釘「stdout 恰好一行 hash」） | 另加：以 `status.success()` 當 commit 成功的腳本／測試，在「根裡有 ⊥」時會紅。產品探針仍少。估 **0–4 紅** | 甲的沉默成功全部變成拒絕，除非測試加上旗標。行為面最大。估 **數支**，但不是現有紅探針——是今天綠的「commit 含 ⊥ 的程式」會開始失敗 |
| **庫呼叫者** | 若只改 CLI：庫仍 `Ok`、根仍有 ⊥、沒有報告 | 若只改 CLI rc：庫仍 `Ok`。要一致就得改 `commit` 的 `Result`／回傳值 | 若只改 CLI：庫仍落地。旗標不是語言面 |

**丙與 B1。** B1 原話「要修的不是中止，是報告」。丙的預設就是中止。
把旗標讀成「操作者決定要不要固化」在字面上像 B1 的後半，
但「沒給旗標＝引擎代決不准固化」正是 §4.1.2 拆掉的那一半。
**若認為丙其實相容，那是回頭請裁的材料，不是報價能消掉的衝突。** 本偵察不選。

---

## Q8 — B 路徑要不要改成同一個形狀（不選）

條文原文怎麼共存：

**§2.2.1 開頭（循序、單一寫者）：**

> $Staged_{old} \sqcap Definition = \bot$ 時，演化**不得**發生。

**§2.2.2（D49）把那句話收窄為「預設了單一寫者」，然後拆兩半：**

> **循序情形不變（MUST）**：注入前與當前集合的 meet 為 ⊥ 時，該次演化**不得**發生，回報依 §2.2.1 各條。**這一半不因並行而放寬。**
>
> **並行情形（MUST）**：兩筆注入**各自**通過上述檢查而其**聯合** fold 為 ⊥ 時，**兩筆都必須保留**，且工作集**必須**回報 ⊥ **落在哪一個座標**……**離開碼不得為零**。
>
> **由操作者化解**：⊥ 的工作集依 D33 由操作者處置。**提交必須失敗**（§4.1 的原子性保證），而非把 ⊥ 靜默寫入歷史。

**§2.2.2 自陳（多重衝突）：**

> 本節要求回報「哪一個座標」，但**未要求**多重衝突時回報**哪一個**。〔量，參考實作〕兩組獨立衝突並存時，回報的座標隨執行變動——因為 fold 在第一個 ⊥ 停下……

**§4.1.2／B1（`commit.md` §2.1.3）：**

> 收斂撞到 `_|_` → 觀測**必須停下並回報**；要不要固化**不由本條代決**。
> **要修的不是「中止」，是「報告」。**
> §4.1.2 射程：**自動收斂**路徑；**顯式地固化一個含 `_|_` 的內容不在本條射程內。**

**今天引擎怎麼對上：**

*   循序座標碰撞（`k: 1` 然後 `k: 2`）＝ §2.2.1／§2.2.2 第一款。evolve 拒絕。綠。
*   並行兩筆注入 fold 為 ⊥ ＝ §2.2.2 第二款 ＋「提交必須失敗」。status rc=1、commit rc=1、不落地。這是 brief 的 **B**。
*   欄位值是 ⊥、整顆仍是 Combo ＝ §2.2.1 的公式在型別上沒觸發（meet 不是 `Value::Bottom`）。
    §2.3「若合併結果產生 `_|_`，則提交失敗」被同一件事穿透——兩個 Combo 的 meet 恆為 Combo。
    這是 brief 的 **A**，也是 B1 量到的沉默成功。

**若 B 改成 A 的形狀**（報告後落地）：放寬的是 §2.2.2 逐字「提交必須失敗」與「離開碼不得為零」。
那是 D49 的產品承諾，不是沒寫清的空缺。

**若 A 改成 B 的形狀**（中止、不落地）：brief 已禁止這樣讀——「B 自己就違反 B1」。

兩者要同一個形狀，先要一則裁定處理「§2.2.2 的提交必須失敗」對 A 類（顯式固化含 ⊥）還算不算。
§4.1.2 說顯式固化含 ⊥ **不在它射程**；§2.2.2 的失敗句寫的是 fold 出來的工作集。
**本偵察不選邊。**

---

## Q9 — 探針怎麼寫才不會釘住不可能的狀態

上一弧的錯：用本引擎造 commit，再剝 `commit:`／`ancestor:`，得到
「`parent: None` 且沒有 ○ 點名」——真實世界產生不出來。工單 §4 已寫過
混鏈造不出來。

本卡**可以**用本引擎造出的狀態：

**R1（基線必紅）** `r1_a_commit_that_lands_a_bottom_does_not_say_so`

```
c: c + 1
oo evolve     # REACH: rc=0
oo commit -m x
```

斷言：**commit 的 stdout／stderr 含 `#divergent`，且含座標 `c`。**
拼法不釘（不釘 `Evolution Conflict:`、不釘 `at`、不釘 hash 那一行還在不在）。

**為什麼今天一定紅：** 〔量〕commit 全文是

```
Commit successful: hash:sha256:v1:7bb9b9d0…
```

rc=0。字串裡沒有 `divergent`，沒有單獨的座標 `c`。
這是本引擎自己走 `project_for_commit` 造出來的根，不是剝註記。

**不要釘 rc。** 甲落地 rc=0、乙落地 rc≠0、丙拒絕——未裁。釘 rc 就是在探針裡選邊。
**不要釘「commit 必須失敗」。** 同上。
**不要再造一條「剝掉 ○ 上的 bottom 再 commit」的路徑。** 那又是不可能狀態。

建議的綠守衛（本卡會重開的那一格）：

*   **G-seq**：`k: 1` 然後 `k: 2` 仍是 evolve 拒絕、rc≠0（§2.2.1 循序 MUST NOT）。
*   **G-id**：`x: 0` 三物件／標準根（本卡不該動身分）。
*   **G-surfaces（可選，A1 專用）**：evolve 之後、commit 之前，`status` 含 `c + 1`；
    這支若拿來當「必須與根一致」會在 commit 前就紅——那是 Q3 的釘，
    與「commit 說出它寫進去的 ⊥」不是同一件事。不要兩支探針共用一個斷言。

跨版本混鏈、`--allow-bottom` 的有無，都不是 `cargo test` 裡這支二進位能造的；
與上一弧相同，留給驗收手量。

---

## 對 brief §1

§1.1 三形覆核成立（指令見 Q2）。§1.2 兩條路徑覆核成立（Q1）。
§1.3 `x: 1 & 2` 的 `message` 確是 Rust `Debug`，出現在 status、工作集 ○、根；
本弧報告**不得抄它**。未修該 Inbox 列。

---

## A. 驗收（驗收方，2026-08-31）

**通過。** 九題全答、帶 `檔案:行號` 與可重跑指令、甲／乙／丙三邊報價且逐字不選邊。

### A.1 獨立複驗六項

| 題 | 複驗了什麼 | 結果 |
| :-- | :-- | :-- |
| **Q2／Q6** | 三形各在哪一步變 ⊥、○ 上有沒有 ⊥ | **逐字屬實**〔量〕`c: c + 1` 的 ○ combo 是**未化約的 thunk**（`~%__nlang_thunk`），○ **不帶 ⊥**，根才是 `#divergent`；`x: 1 & 2` 與 `add (1,"x")` 的 ○ combo **已經帶 `~%__nlang_bottom: #conflict`** ⟹ **這兩形的報告材料今天就在磁碟上** |
| **Q5** | `#missing_key` 到不到得了根 | **屬實**〔量〕`~%Math./nosuchmorph (1)` → 根 `m: _\|_ (%cause: #missing_key)`。**但要補一句**：自由名字（`z: nosuch.field`、`b: a.q`）**不會**變 ⊥，它們**原樣留在根裡當 thunk**（D46 的修正仍成立） |
| **Q4** | 一次提交可有多個 ⊥ 座標 | **屬實**（`x`／`y` 兩個並存） |
| **Q1** | `workset_bottom` 只有一個設定點 | **屬實**（`universe.rs:959`，`fold` 回 `Err` 時） |
| **§8 開弧 4** | 呼叫有沒有發生 | **有**——同一個欄位名、只換引數：`add (1,2)` → `3`，`add (1,"x")` → `_\|_ (%cause: #conflict)` |
| 交付方自陳的樹乾淨 | `git status` | **屬實** |

### A.2 ⚠ 驗收方分診時說錯了一句，就地更正

分診那則逐字寫「**`SPEC_10` §2.2.1 三條 MUST 同時未兌現**」。**那句話是錯的。**

〔讀〕§2.2.1 的射程被它自己的第一句閘住：

> $Staged_{old} \sqcap Definition = \bot$ 時，演化**不得**發生。……**本節規範的是它必須說出什麼。**

A 類的 meet **不是 ⊥**（兩個 Combo 的 meet 恆為 Combo，⊥ 在欄位裡）
⟹ **演化沒有被拒絕 ⟹ §2.2.1 的五款從頭到尾沒有被引動。**
**引擎沒有違反那五條，是那五條夠不到這一格。** 交付方 Q8 的分析是對的，驗收方的分診講得太快。

### A.3 而真正的成因比「有個洞」更難看：**規格在這一格自己打架**

〔讀〕**`SPEC_10` §2.3「原子性保證」**：

> 若合併結果產生 `_|_`，則**提交失敗**，HEAD 維持不變，且 Staged 區自動回滾。

〔讀〕**同一章 §2.3.1（2026-08-28 新增，裁定 D46）的 `_|_` 款**：

> **`_|_`（MUST）**：撞到 `_|_` 者仍**必須**寫入 `_|_` 與其 `%cause`（§4.1.2）。
> 一個誠實標記的矛盾是事實，不是待重算的配方。

⟹ **一節說提交必須失敗，下一小節說 ⊥ 必須被寫進去。** 兩者同章相鄰，
**而 §2.3.1 是後寫的、有裁定的（D46），引擎照它做。**

**交付方對 §2.3 的讀法與驗收方不同，而那個歧義本身就是要裁的東西**：
§2.3 的「合併結果產生 `_|_`」是指
**(a) 結果就是 ⊥**（交付方的讀法，於是 A 不觸發），
還是 **(b) 結果裡某個座標是 ⊥**（則 A 觸發，引擎違反 §2.3）？
**§2.2.1 自己的理由句偏向 (b)**——「演化的最小單位是座標，衝突揭露的最小單位也應當是座標」；
**而 §2.3.1 的存在偏向 (a)**，否則它自己那一款永遠無法執行。

### A.4 ⟹ 待裁的其實是兩件，不是一件

**驗收方原本只列了 CLI 那三個候選（甲／乙／丙）。偵察做完之後，它們是第二順位的。**

1.  **（先）§2.3 與 §2.3.1 哪一個說了算，以及「合併結果產生 `_|_`」是 (a) 還是 (b)。**
    這一則裁完之前，甲／乙／丙**都沒有立足點**——它們全都預設「⊥ 可以落地」，
    而那正是 §2.3 逐字禁止的事。
2.  **（後）「要不要固化由操作者決定」在 CLI 上的形**（甲／乙／丙，報價已在 Q7）。

**而 B1 缺的那一半，兩條條文都沒有補上**：§2.3 說失敗，§2.3.1 說寫進去，
**沒有任何一條說「必須告訴操作者」**。B1 逐字「**要修的不是中止，是報告**」
——**那句話至今沒有進過規格。**

### A.5 交付方做對而值得記的

*   **Q8 沒有給一個假的和解。** 它把四段條文原文並排，指出 A 與 B 住在**不同的條文底下**，
    然後說「兩者要同一個形狀，先要一則裁定」。**那正是本卡該有的答案。**
*   **Q7 的共同前置點名「不讀 `message`」**，理由給的是 §2.2.1 的 MUST NOT
    ——**主動避開驗收方 brief §1.3 標為「本弧不修」的那一格**。
*   **Q5 沒有含糊帶過**：`#unprovided_builtin` 使用者鑄不出、`#incomplete` 根本不是 `BottomCause`、
    fuel=0 走的是 blur 不是 ⊥。**三個否定各自帶理由。**
