# Q-015 偵察 — 一份被寫了兩次的歷史

> **Queue ID**：`WORK_QUEUE` Q-015（Active，偵察；弧 A 的 A2＋A3）
> **基線**：引擎 `v0.40.0` 標籤二進位
> `/home/gali/nlang-baselines/v0.40.0-verify/target/release/oo`
> （`--version` 印 `oo v0.40.0`；行為確認：`oo run ka.n -o r`，
> `r: ~%Math./add (1,2)` → `3`，答案不是 `_`、不是原文）。
> 工作樹 `nlang-tools` `dev`（本檔是唯一產物）。
> **這是偵察，不是實作。** 未裁的四個候選不選邊。
>
> **身分**：本輪零改動。〔量〕`x: 0` 根
> `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`，
> `.oo/objects` **3** 個檔，標準根
> `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`。
>
> **⚠ 縮寫**：本文 `CAS` 只指 Content-Addressed Storage。compare-and-swap 寫全稱。

未重量 brief §1.1–§1.3 已給的 40 並行／平方曲線／「兩張圖不相連」；
下列若覆核，帶指令。裁定 D43／D47／D50／D51 不重開。

---

## 0. 十二題各一句

| | 答案 |
| :-- | :-- |
| **Q1** | `oo log` 三列靠 `parent` 走；`parent` 恆 `None` 之後只剩 HEAD 一列。`squash` 今天成功；之後永遠「not an ancestor」。`inspect` 今天印真實父；之後恆 `(none)`。refine 影子掃描今天走完整條 `parent` 鏈（深度 16）；之後只看得見當時的 HEAD。 |
| **Q2** | `.oo/HEAD` 只有一行、沒有 reflog。時間戳是 **Unix 毫秒**（`as_millis`）。相鄰兩次提交可同秒（實測差 54 ms）。**沒有「不新增任何東西」還能走訪歷史的路。** |
| **Q3** | 影子掃描要的是**祖先集合（可達性）**，不是畫哪一條覆蓋邊。換成集合它還做得對。 |
| **Q4** | §578 原文「放棄邊使對象永久可達」**今天不成立**：放棄邊不是 GC 根，rollback 之後不必 squash，`gc` 就收掉了被放棄的 commit。squash 另外把 `abandoned` 從新 commit 上拿掉。這是規格 MUST 與引擎的已知落差，不一定要跟 A2 綁在一起。 |
| **Q5** | 框裡裝得下 Bottom（`~%__nlang_bottom`）、`#incomplete` 標籤、帶視界的 Blur、化約後的 `%effect` 值。Top 作為欄位值會被 meet 吃掉，快照裡看不見 `_`。encode↔decode **位元組相同**（含 Blur）。A3 不再卡在「沒有 CAID 就不能有產生判準」——D51 已經用覆蓋關係回答了何時鑄；剩下的是條目要不要為三軸分開欄位。 |
| **Q6** | **拆成兩家。** 「`run`／`eval` 看不見已提交宇宙」屬 **Q-018**。「即使 REPL 已 `load_universe`，observe 也不鑄 ○」屬 **Q-015／A3**（觀測作為日誌事件沒有入口）。不要寫成同一件事。 |
| **Q7** | 每 10 次 evolve 提交一次，到 n＝300：○ **300** 個檔、**35 661 B**（近線性）。`gc` 之後仍 300／35 661。檔數全域無界；單檔平方只在提交區間內。 |
| **Q8** | 有家：commit 的 kind／timestamp／author／parent／root／`abandoned`／`privileged_effect`；○ 的 `parents:` 與 combo 快照（可含 Blur 視界與 `%effect`）。沒有家：事件種類（注入／觀測／commit 沒有寫在 ○ 上）、○ 的時間／作者、○↔● 的邊。 |
| **Q9** | 甲最便宜（A2 不做）。乙最貴（第三份紀錄）。丙要在不可變 ○ 上長出提交邊（不能改寫舊檔）。丁對**舊** commit 零成本，對**新** commit 是紀元。不選。 |
| **Q10** | 要畫圖最少要：節點 id、覆蓋邊、（可選）標籤。甲缺接縫；乙自帶圖；丙／丁各補一條跨層邊。今天兩張圖都畫得出來，但不是一張。 |
| **Q11** | `atomic_write` 綁不住兩個檔。真正的跨檔窗是 `put_commit` 然後 `set_head`、以及 HEAD 已寫但注入未清。孤兒 commit 無害；「新歷史＋舊工作集」能再 commit 一顆。這些窗**不是**單檔 rename 解得掉的；是否非要 WAL，是乙要不要做的裁定，不是物理必然。 |
| **Q12** | Q-016 是工作集／pin 的 compare-and-swap。乙／丙／丁都不取代它。不得合併。 |

---

## Q1 — `parent` 六個讀者的行為

〔量，標籤二進位〕三次 `evolve`＋`commit`（`x: 1`／`y: 2`／`z: 3`）。

### `Ouroboros::log`（`lib.rs:4840`）→ `oo log`（`main.rs:883`）

今天印 **3** 列 commit，各有 message／Date／（HEAD 那顆的）inspectable parent。走訪是 `curr = commit.parent`（`:4848`）。

`parent` 恆 `None` 之後：迴圈第一步就把 `curr` 設成 `None`，**只印 HEAD 一列**。

### `commits_after`（`universe.rs:1214`）／`squash`（`:1227`、祖先檢查 `:1250`）

今天 `oo squash <第一顆> --grant squash` **rc＝0**，log 變成 2 列（squash 標記 ＋ base），訊息 `compressed 2 commit(s) onto …`。

`parent` 恆 `None` 之後：HEAD 的 `parent` 是 `None`，對任何不是 HEAD 自己的 base，`found` 維持 false，**永遠** `squash base is not an ancestor of HEAD`。空區間（base＝HEAD）今天就已經拒絕。

### `oo inspect`（`main.rs:1593`–`:1603`）

HEAD 那顆：`parent: hash:sha256:v1:9326a427…`（上一顆）。第一顆：`parent: (none)`。

`parent` 恆 `None` 之後：每一顆都印 `(none)`。

### refine 影子掃描（`universe.rs:1450`–`:1542`）

今天 `oo refine -s <第一顆的 root> -t <第三顆的 root> -m shadow` **rc＝0**，產出 Refine commit（`Refine authority: unverified`）。這次 **沒有**印 `Shadow: N`——來源是**整顆 root 的 CAID**，掃描比的是各歷史 commit 裡**欄位**的 `content_hash`（`:1533`），對不上。掃描仍然發生：從 HEAD 沿 `parent` 走，上限 `SHADOW_SCAN_DEPTH = 16`（`:1450`）。

`parent` 恆 `None` 之後：只看當時 HEAD 那一顆（refine 自己寫入之前），更舊的 commit 不進 `shadow_affected`。

要的是可達集合還是覆蓋邊 → Q3。

### `store_codec.rs:556`–`:560`／`value.rs:2406`／`:3627`

編碼：有父寫 `parent: <hash>`，無父寫 `parent: _`。雜湊：`if let Some(p) = &self.parent { buf.extend(p.digest) }`——**`None` 貢獻零位元組**（`commit.md` §1.7.7 仍對）。

### 可重跑

```bash
OO=/home/gali/nlang-baselines/v0.40.0-verify/target/release/oo
W=$(mktemp -d); export OO_IDENTITY="$W/id" OO_NODE_HOME="$W/nh"; cd "$W"
printf 'x: 1\n' > x.n; $OO evolve x.n; $OO commit -m c1
printf 'y: 2\n' > y.n; $OO evolve y.n; $OO commit -m c2
printf 'z: 3\n' > z.n; $OO evolve z.n; $OO commit -m c3
$OO log
$OO inspect "$(cat .oo/HEAD)"
$OO squash "$(python3 -c "print(open('.oo/HEAD').read())")" --grant squash  # 用第一顆，不是 HEAD
```

（squash 的引數必須是**第一顆**的 CAID，從第一次 commit 記下。）

**§8 開弧 4**：本題無內建。不適用。

---

## Q2 — 磁碟上還有沒有提交先後？

〔量〕

| 來源 | 今天 | `parent` 消失後能不能當序 |
| :-- | :-- | :-- |
| `.oo/HEAD` | **一行**，79 B，無換行，一個 CAID | 只認得最新。不是鏈 |
| reflog | **沒有**。`find .oo` 只有 `HEAD`／`format`／`objects`／`objects.format`／`savepoints/` | 無 |
| `CommitMeta.timestamp` | `u64` 毫秒（`main.rs:1017` `as_millis`；`format_commit_date_ms` `:968`） | 相鄰兩次提交實測 **54 ms**（`…57.724Z` 與 `…57.778Z`），**同秒**。同一毫秒理論上可撞。不能當全序 |
| 物件檔 mtime | POSIX 有 | 不是規格、不是跨機器、精度差 |

○ 的 `parents:` 是**工作集快照**的覆蓋邊，裡面**沒有 commit CAID**（brief §1.2，覆核成立）。

⟹ **A2 沒有「不新增任何東西」的路。** 「不再設定 `parent`」對雜湊免費（§1.7.7），對走訪不是。要把歷史走訪從 `parent` 搬走，必須有另一份序：○ 上的提交邊（丙）、● 上的 ○ 邊（丁）、或第三份紀錄（乙）。

**§8 開弧 4**：不適用。

---

## Q3 — 影子掃描要可達性還是覆蓋關係？

〔讀 `universe.rs:1450`–`:1542`〕從 `self.head` 沿 `commit.parent` 走到最多 16 顆。對每一顆，載入 `commit.root`，掃欄位 `content_hash`，命中 `source_caids` 就把該 **commit CAID** 推進 `shadow_affected`。

它要的是「哪些歷史 commit 的根裡出現過這個來源值」——**祖先集合**。不讀邊的方向、不畫 Hasse 圖、不管兩顆之間有沒有被 squash 抽掉的中間點（那些點若不在 `parent` 鏈上就本來看不見）。

換成「祖先集合」而不存覆蓋邊：**還做得對**，只要那個集合與今天沿 `parent` 走到的集合相同。這正是 `commit.md` §1.7.5 那組判別器裡「可達性」的那一半，不是「畫圖」的那一半。

**§8 開弧 4**：不適用。

---

## Q4 — `SPEC_08` §578 今天兌現嗎？

〔量〕`a` → `b` → `c` 三顆 commit，`rollback` 到 `a`，再 `evolve d`＋`commit`。

* 新 HEAD 的 `parent` 是 `a`（覆蓋邊跳過了 `b`／`c`）。
* 新 commit 的 meta 有 `abandoned <c的CAID>`。`oo log` 印那一行。
* `b`／`c` 此時**仍是物件**（`inspect` 得到 `kind: commit`）。
* **`oo gc --grant gc` 不等 squash**：`9 objects, 5 reachable, 4 collectable`，其中 4 個「content of heads abandoned by #rollback」。之後 `inspect c` → `CAID not found`；log 改印 `abandoned … (content collected)`。

再 `squash` 到 `a`：新 squash commit **沒有** `abandoned` 行（`universe.rs:1267` 故意不從中間 commit 拷貝）。細節（哪一顆被放棄）從鏈上消失，只留下 `squash` 標記。

**§578 原文**：「否則被放棄的 Commit 因放棄邊而永久可達」。**今天這句是假的。** `gc.rs:206` 從 HEAD 走 digest；`follow_abandoned` 預設 false。放棄邊不是根。這正是 §6.2.1（2026-07-29）已經寫下的推翻——brief 引的 §578 那句 MUST，同一節下面已經改了理由。

**與 A2 分開：** 這是「MUST 的字面 vs 引擎 vs 已寫下的修訂」的 Inbox 材料，不是「`parent` 退場會不會弄壞 squash」的同一題。A2 若不再設定 `parent`，squash 的祖先檢查會先壞（Q1），與放棄邊無關。

**§8 開弧 4**：不適用。

---

## Q5 — ○ 的框裝得下什麼？A3 還卡在 `observation_result.md` §1 嗎？

〔量，標籤二進位，一次 evolve 四個欄位；另一次 `~%Config.fuel: 1` 加長加法〕

| 注入 | 快照裡出現的 |
| :-- | :-- |
| `t: _`（Top） | 欄位**消失**（單獨 evolve 得到 `{}`）。Top 是 meet 的單位元，不是一個可記下的「值事件」 |
| `b: _\|_` | `{ b: { ~%__nlang_bottom: #conflict } }` |
| `i: #incomplete` | `i: #incomplete`（標籤，不是 Blur 包裝） |
| `e: ~%Math./add (1, 1)` | `e: 2`（呼叫發生了：改成 `(1, "x")` 得到 Bottom，不是原文） |
| `k: ~%Math./add (1, "x")` | `{ ~%__nlang_bottom: #conflict }` |
| 低燃料加法 | **Blur**：`~%__nlang_blur: #true` ＋ `cause`／`fuel`／`fuel_remaining`／`strategy`／四個視界上限 ＋ `partial` 的 CAID |

`encode_savepoint` → 磁碟 → `decode_staged` ＋ `parse_savepoint_parents` → `encode_savepoint`：

* 含 Bottom／`#incomplete`／已化約加法的那顆：**95 B，位元組相同**。
* 含 Blur＋視界的那顆：**543 B，位元組相同**。

（一次性 `/tmp` 程式呼叫與引擎相同的 `store_codec`，跑完已刪。）

**A3 還卡在 §1 嗎？** 卡的那句是「`#incomplete` 沒有 CAID，產生判準不能用 digest」。**D51 已經用覆蓋關係回答了「何時鑄」**，不再需要 digest。框**已經**裝得下 Blur 的視界參數與 Bottom 的 `%cause`。剩下的不是「裝不下」，是「一條日誌條目要不要把狀態／精度／效果拆成三個欄位，而不是塞在 combo 裡」——那是 A3 的詞彙表，不是 §1 的表示障礙。

**§8 開弧 4**：`~%Math./add` 改引數改答案，呼叫發生了。

---

## Q6 — 觀測條款那一列的家

Q-014b 偵察 Q6 的兩個洞仍然在 v0.40.0：

1. **沒有寫者。** `Universe::observe` 是 `&self`（`universe.rs:1288`）。`savepoint::record` 的唯一呼叫者仍是 `save_staged`（`:894`）。`oo run`／`eval`／`test`／`repl` 在 observe 之後都不 `record`。
2. **倉庫。** `run_one_shot`（`main.rs:1385`）明文「no local staged load, no durable store writes」，根是 `None`。REPL 走 `load_universe`（`:1185`）——看得見已提交的宇宙，仍然不鑄 ○。

〔覆核，不必新數字〕`oo run s.n -o r` 在已 commit 的 `a: 1` 上對 `r: ~%Math./add (a, 1)` 得 `_\_|__`；對字面 `(1, 1)` 得 `2`。savepoints 數不變。

**家：**

* 「`run`／`eval` 看不見 HEAD」→ **Q-018**。本卡指名不修。
* 「觀測化約了 thunk／坍成 ⊥ 也不鑄 ○」→ **Q-015／A3**，**當且僅當**本卡把「觀測」列為一種日誌事件（`commit.md` §1.10.7 的三個字）。若 A3 縮成只談 commit 條目，這一列繼續留 Inbox，不要假裝 Q-015 結掉它。
* 不要把兩洞寫成同一個「零兌現」。

**§8 開弧 4**：改 `add` 的引數改答案。

---

## Q7 — 有提交時 ○ 怎麼長；有沒有回收？

〔量，標籤二進位〕每 10 次 `evolve` 一次 `commit`，到 n＝300。

| n | ○ 檔數 | `savepoints/` 總位元組 | 最後一顆 |
| --: | --: | --: | --: |
| 10 | 10 | 979 | 130 |
| 100 | 100 | 11 061 | 150 |
| 200 | 200 | 23 361 | 168 |
| 300 | 300 | **35 661** | 168 |

牆鐘 9.54 s（對照 brief §1.3：不提交時 322 次就 >2 min）。`oo gc --grant gc` 之後仍 **300 檔／35 661 B**。

與 §1.3 一致、且把那句限定量出來了：**檔數全域線性無界；單檔大小的平方只發生在兩次 commit 之間**（這裡每個區間 10 個欄位，最後一顆停在 ~168 B）。沒有任何路徑刪 ○（`gc.rs:282` 只掃 `objects/`；commit 清的是 `injections/`）。

**§8 開弧 4**：不適用。

---

## Q8 — 一條「日誌條目」的欄位，今天在哪裡？

| 欄位 | 今天的家 | 沒有家 |
| :-- | :-- | :-- |
| 事件種類（注入／觀測／commit） | commit 有 `CommitKind`（Standard／Pin／Squash／Refine）。○ **沒有**種類欄——每一顆都是 `savepoint` 框 | 觀測事件 |
| 時間 | `CommitMeta.timestamp`（ms） | ○ |
| 作者 | `CommitMeta.author`（CLI 寫 `"oo-cli"`） | ○ |
| 前驅 | ○：`parents:`（本地 id，複數）。●：`parent`（CAID，單數） | 跨層的前驅 |
| 產生的值或位址 | ○：整個工作集 combo。●：`root` CAID | ○ 不記 commit／root CAID（§1.2） |
| 視界 | 若值是 Blur，視界參數**已經在 combo 裡**（Q5） | 不是 ○ 框的獨立欄 |
| `%effect` | 值上的 `~%__nlang_effect`；commit 上的 `privileged_effect`（放電審計） | 觀測當下的 effect 不另存 |

§1.10.7 的三個字，只有 **commit** 有完整的 ● 紀錄；**注入**有 ○ 快照但沒有「這是注入」的標籤（與觀測共用同一個框）；**觀測**沒有紀錄。

**§8 開弧 4**：不適用。

---

## Q9 — 四個候選報價（不選）

共同約束：○ 身分不得為 CAID（§3.1 MUST NOT）；`x: 0` 根與 3 物件不動。不得在報價裡替 D50 挑邊。

### 甲 — A2 結案為「不做」

* **一句話：** `Commit.parent` 留下，本卡縮成 A3。
* **身分：** 不動。
* **磁碟：** ● 不變。A3 若改 ○ 框，那是另一筆。
* **`layout=`：** 不必因 A2 升（A2 沒發生）。
* **檔：** 0（A2）；A3 另計。
* **既有測試：** log／squash／inspect／refine 影子全綠。
* **跨版本：** 舊倉照走 `parent`。
* **解不掉的：** 用戶「一張圖」的直覺；Q2 的「沒有第二份序」。

### 乙 — 第三份紀錄（append-only 事件日誌；● 與 ○ 都是衍生視圖）

* **一句話：** 新增一份不可變事件檔（名字不一定叫 `wal/`）。走訪只讀它。
* **身分：** 事件 id 必須是本地的，不可進 `objects/`。
* **磁碟：** 新目錄或新檔。`REAL_01` §4.2 的 `wal/` 目前零條文、零實作。
* **`layout=`：** 加檔是否升版仍是獨立選擇（Q-013／Q-014b 加目錄都沒升）。升＝舊引擎拒新倉。
* **檔：** 新模組 ~150–300 行 ＋ `log`／squash／inspect 改走事件；○／● 寫入改成「先附事件」。
* **測試：** `oo log` 全套、squash 祖先、inspect parent、版圖 `p1`、advert／kademlia 允許名單。約 **15–25** 支要改路徑或宣告。
* **跨版本：** 舊倉無事件檔 ⟹ 必須從 `parent`＋○ **重建**一次，或雙讀。這筆比甲貴一個數量級。
* **D50：** 事件的前驅可以是複數，不與 ○ 的 `parents:` 搶形。不必挑邊。
* **Q-016：** 不順便解掉（Q12）。

### 丙 — ○ 長出提交邊

* **一句話：** 某顆 ○ 記錄「我變成了 commit C」；`parent` 不再設定；走訪把兩張圖接起來。
* **身分：** C 仍是 CAID，○ 仍是本地 id。邊是「本地 id → CAID」，**不是**共用識別碼空間。若有人把 ○ 的身分改成 CAID，MUST NOT。
* **磁碟：** ○ 檔今天不可變。**不能改寫**已落地的 ○ 去加欄。必須（1）commit 時再鑄一顆帶 `commit: <CAID>` 的 ○，或（2）旁路索引（又是一份真相）。（1）會讓 D51 再鑄——覆蓋關係變了。
* **`layout=`：** 框加欄，與 Q-014b 加 `parents:` 同類；當時沒升。
* **檔：** `store_codec` ＋ `record`／commit 交界 ~80–150 行。`log` 改走 ○ 上的提交邊再跳到 ●。
* **測試：** log 列數／順序；squash 祖先改讀這條邊。inspect 的 `parent:` 會變 `(none)`——那支針要改性質。約 **8–15** 支。
* **跨版本：** 舊 ○ 沒有提交邊 ⟹ 舊歷史仍只靠 `parent`。混鏈：切換點之前走 `parent`，之後走 ○。§1.7.7 已預告。
* **D50：** 提交邊是另一種邊，不必把 `parents:` 改成單數。

### 丁 — ● 長出 ○ 邊（`CommitMeta` 加 `Option`）

* **一句話：** meta 記「我來自哪些 ○」。§1.6 的 Debug 模式：`None` 不進雜湊。
* **身分（跨版本，brief 點名的那題）：**
  * 舊 commit、欄位 `None`：**digest 不動**（與 `abandoned`／`privileged_effect` 同一招）。
  * 新 commit、欄位 **永遠 `Some`**：同一個宇宙、舊引擎提交 vs 新引擎提交，**commit CAID 不同**。根物件可以不變（`x: 0` 仍 `31745ef0…`、仍 3 個物件），**commit 物件的位址變了**。這是**新歷史的紀元**，不是舊歷史的。
  * 若新欄位只在「有來源 ○」時為 `Some`、普通路徑省略——第一顆 `x: 0` 仍可與舊引擎同 CAID。這是實作選擇，報價裡兩種都列上。
* **磁碟：** encoding-5 的 commit 正文多一個可選欄。舊 decode 若遇見未知欄位：要量——今天 `decode` 對 commit meta 是具名字段，**未知欄可能被丟或失敗**。這支必須在實作前列進探針。
* **`layout=`：** 不一定。物件編碼軸是 `objects.format`（encoding=5），不是 `layout=`。
* **檔：** `CommitMeta` ＋手寫 `Debug` ＋ `write_commit_meta`／decode ~40–80 行。`log --graph` 才用得上這條邊。
* **測試：** 釘死**新** commit 雜湊的會紅；釘根 CAID 的（g4）在「第一顆 meta 仍 None」時綠。約 **5–12** 支視「是否永遠 Some」。
* **D50：** meta 裡可以是 id **陣列**，與 ○ 的複數前驅對齊，不必裁回單數。

**§8 開弧 4**：不適用。

---

## Q10 — 誰能讓 `oo log --graph` 畫得出來？

最少欄位：

1. **節點 id**（印得出來的名字）
2. **覆蓋邊**（畫哪一條，不是只給可達性）
3. 可選：**標籤**（message／Date／kind），否則只是匿名 DAG

| 候選 | 能畫什麼 | 缺什麼 |
| :-- | :-- | :-- |
| 今天 | ● 只能畫一條**鏈**（`parent` 單數）；○ 能畫 **DAG**（`parents:` 複數）。兩張圖不相連 | 接縫；○ 無 message／Date；沒有 CLI |
| **甲** | 與今天相同 | 接縫。用戶的「一張圖」沒有 |
| **乙** | 事件日誌自己就是圖 | 若事件不記覆蓋邊，又退回可達性 |
| **丙** | ○ DAG ＋ 若干節點上的「變成了 C」 | 舊歷史沒有這條邊；C 的 `parent` 若不再設定，● 自己不再成鏈 |
| **丁** | ● 鏈（或點）＋ 指向來源 ○ | 舊 commit 沒有這條邊；單數 `parent` 若留下，圖上會有兩種 ● 邊 |

用戶 2026-08-30 的直覺要的是**一張**含分叉與匯流的圖。今天 ○ 已經能表達分叉／匯流（D50），只是 **CLI 不讀它**（`savepoint::load` 仍 `dead_code`；`oo --help` **18** 個子命令，沒有一個看 ○——brief §1.5 寫 17，本輪數到 18，多的是 `lint`）。畫得出來 ≠ 已經在畫。

**§8 開弧 4**：不適用。

---

## Q11 — `wal/` 還需要嗎？

`atomic_write`（`storage.rs:15`–`:41`）最後一步是 `tmp.persist` ＝ **覆寫式 rename**。單檔：舊或新，沒有半截。**兩個檔之間沒有事務。**

今天寫超過一個耐久檔、中間崩潰會留下中間態的位置：

| 序 | 位置 | 中間態 | 後果 |
| :-- | :-- | :-- | :-- |
| 1 | `put_commit` 然後 `set_head`（`universe.rs:1083`–`:1084` commit；`:1272`–`:1273` squash；`:1597`–`:1598` refine） | 新 commit 物件在、HEAD 仍舊 | **孤兒物件**。下次 commit 的 parent 仍是舊 HEAD。GC 可收。不是第二份歷史 |
| 2 | `set_head` 然後 `injections::clear`（`:1084` 然後 `:1090`） | 新歷史 ＋ **舊工作集還在** | 下次 `commit` 能再交一顆（Q-014 偵察 Q4 已量）。**真的不一致** |
| 3 | `save_staged`：注入（`:886`）然後 ○（`:894`）然後 pin／effect | 工作集有、○ 沒有（或相反） | 跨層：status 與 ○ 數對不上。單檔 rename 解不掉 |
| 4 | rollback：`append_abandoned_file`（`:1191`）然後 `set_head`（`:1194`） | abandoned 已寫、HEAD 未動 | 下一顆 commit 可能標記一個**仍是 HEAD** 的位址為放棄。髒，少見 |
| 5 | `ObjectStore::init` 寫 `format` 與 `objects.format` | 只寫了一個哨兵 | 舊引擎／新引擎對「這是不是新倉」答案可能相反 |
| 6 | commit 成功後 Config 再 `save_staged`（`:1101`） | HEAD 已新、Config 注入未寫 | 下一觀測掉回 genesis 燃料（O37）。已知 |

**哪一個是單檔 rename 解不掉、非 WAL 不可的？**
2、3、4、5、6 都跨檔。**沒有一個是物理上非 WAL 不可**——也可以：接受孤兒（1）、接受「再 commit 一顆」並用探針釘住（2）、把工作集與 ○ 收成一個物件（那是 Q-014 沒做的甲）、或乙的事件日誌把「commit 發生了」變成**一筆** append。

`wal/` 在規格裡是 §4.2 的一張圖、零條文。乙若做第三份紀錄，它**可以**順便把 2 綁成一筆事件；那是乙的報價，不是「必須先做 wal 目錄」。不要把 `REAL_01` §4.2 的檔名當成完成條件。

**§8 開弧 4**：不適用。

---

## Q12 — 與 Q-016 的分界

Q-016：meet 路徑上的 compare-and-swap ＋ 重試；pin 路徑序列化。工作集在 Q-014 之後，相異欄位已經不需要協調（40／40）。剩下的是**同一座標**的並行與 **pin**。

| 候選 | 順手解掉 Q-016？ |
| :-- | :-- |
| 甲 | 否 |
| 乙 | **否。** 日誌是事後紀錄，不是 compare-and-swap |
| 丙 | 否。提交邊在 commit 時寫，evolve 當下的競爭還在 |
| 丁 | 否。meta 在 commit 物件上，更晚 |

若有人把乙做成「先 append 事件再 fold 工作集」，那是把 WAL 當成工作集的寫入協定，**那才踏進 Q-016**。本偵察不把兩張卡合併。合併要裁定。

**§8 開弧 4**：不適用。

---

## 明確不做（複述）

實作 A2／A3。選甲／乙／丙／丁。動產品程式碼。動身分。把 ○ 放進 `objects/`。合併 Q-016。修 Q-018。實現 `oo log --graph`。以 `.oo/audit.log` 取代 Commit 審計（MUST NOT）。
