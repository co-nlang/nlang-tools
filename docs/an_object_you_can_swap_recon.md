# Q-014（W10）偵察 — 一個可以被原子交換的物件

> **Queue ID**：`WORK_QUEUE` Q-014（Active，偵察）
> **基線**：引擎 `v0.38.0` 標籤二進位
> `/home/gali/nlang-baselines/v0.38.0-verify-target/release/oo`
> （`--version` 印 `oo v0.38.0`）／規格 `v0.38.0-draft.1`／工作樹
> `nlang-tools dev abfe293`（乾淨，本檔是唯一產物）。源碼相對
> `v0.38.0`（`60a4854`）只有 brief 這份文件。
> **這是偵察，不是實作。** 下面若有「一行就能修」的東西，只寫進報告。
> **未重量**驗收方 2026-08-29 已量的 (a)–(e)（brief §1）。
> **未裁的岔路不選邊**（brief §3）：兩邊只報價。
> **附錄（2026-08-29）**：Q10–Q14 身分三候選報價；撤回 Q9 的甲-1／甲-2／甲-3 框架。不選 A／B／C。
>
> **身分**：本輪零改動。〔量，標籤二進位〕`~%Math./add (1,2)` → `3`
> （呼叫發生了：答案不是 `_`、不是原文）。`x: 0` 根
> `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`，
> `.oo/objects` **3** 個檔，標準根
> `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`。
>
> **⚠ 縮寫**：本文 `CAS` 只指 Content-Addressed Storage。compare-and-swap
> 寫全稱。

先讀了 `save_staged`／`savepoint.rs`／`store_codec.rs`／`gc.rs`／`commit`
路徑。裁定 D26／D43／D47 不重開。

---

## 0. 九題各一句

| | 答案 |
| :-- | :-- |
| **Q1** | 任何一步失敗都不回滾。下次啟動**不檢查** ○／`LOG`。不一致窗存在，引擎看不見。 |
| **Q2** | `save_staged` 的邊界是暫存工作集。HEAD／物件／GC／peers／discovery／abandoned 各走別的寫入點。**交換單位應畫在那五個檔，不要把 HEAD 捲進來。** |
| **Q3** | 差在框的第二個詞：`staged` vs `savepoint`。第一個不同位元組是 offset 14 的 `t`／`a`。body 位元組相同，解碼已共用。**可以共用一個編碼。** |
| **Q4** | 先寫 `HEAD`，再 `unlink` `staged`。中間崩 → 新歷史＋舊工作集，下次能再 commit 出第二顆 commit。`savepoints/` 不動。 |
| **Q5** | 讀者是 `load_staged` 與 commit 閘。值**不進根 CAID**。同一 payload，`--pin` 與普通 evolve 根相同（`ca0986d5…`），commit 雜湊不同（kind）。 |
| **Q6** | 兩檔形被三支版圖釘宣告；kademlia 那支**沒宣告**（該情境不 evolve，是潛伏針）。CLI 零讀者。 |
| **Q7** | GC 不走 ○／staged；`inspect` 只讀 CAS。舊引擎容忍未知檔。新引擎讀舊倉必須雙讀或遷移。`.oo/format` **不一定**要升版——升不升是獨立選擇，升了舊 `oo` 會拒。 |
| **Q8** | 有 HEAD 的一次普通 `evolve`：**3** 次 `atomic_write`（temp＋rename＝**6** 次安裝）。甲把工作集塌成一檔之後，同情境是 **1** 次（**2** 次安裝）。 |
| **Q9** | 甲：核心約 200–280 行＋約 10–15 支探針／版圖；另有「鏈怎麼表示」的子岔路。乙：約 50–80 行、版圖不動；五檔窗與 mint 競態都還在。 |

---

## Q1 — 五個檔的失敗語義

`save_staged`（`universe.rs:817`–`859`）依序：

| 步 | 寫什麼 | 失敗時磁碟上已有 | 回滾？ |
| --: | :-- | :-- | :-- |
| 0 | `persist_blur_partials` → `objects/` | 可能已有新的 blur 物件 | 無 |
| 1 | `.oo/staged` | 步 0 | 無 |
| 2 | `.oo/savepoints/<id>` | 步 0–1 | 無 |
| 3 | `.oo/savepoints/LOG`（整檔重寫） | 步 0–2（**孤兒 body**） | 無 |
| 4 | `.oo/pin_pending` 或 `remove_file` | 步 0–3 | 無 |
| 5 | `.oo/effect_pending` 或 `remove_file` | 步 0–4 | 無 |

`atomic_write`（`storage.rs:15`–`41`）單檔是 temp＋`sync_all`＋`rename`：目標要嘛舊內容、要嘛新內容，不會半截。**步與步之間沒有事務。** 函式 `?` 往外傳，呼叫端 `oo evolve`（`main.rs:463`）失敗退出。沒有補償刪除。

下次啟動：`load_universe` → `load_staged`（`universe.rs:862`–`893`）只讀 `staged`／`pin_pending`／`effect_pending`。`savepoint::load` 標 `#[allow(dead_code)]`（`savepoint.rs:79`）。`oo status`（`main.rs:837`）印 staged，不看 `savepoints/`。

〔量，重構窗，不是並行競態，標籤二進位〕

1. **body 在、LOG 不在**（死在 2–3）：`oo status` 仍印 staged。下一次 `evolve` **重鑄同一個 id** `0000000000000001` 並覆寫那個 body（`mint_id`＝`ids.len()+1`，`ids` 來自空 LOG）。孤兒被下一次寫入吃掉，不是被修復。
2. **LOG 在、body 不在**：`status` 正常。下一次 `evolve` 在 LOG 裡**追加** `0000000000000002`，留下一個 LOG 認識、磁碟沒有的 `0000000000000001`。
3. **刪掉 `staged`、留下 ○**：`status` 說 `Universe is static (no staged changes)`，`savepoints/LOG` 與 body 仍在。CLI 與 ○ 層對「有沒有工作集」答案相反。引擎不報不一致。

**沒有修復路徑，沒有啟動時的一致性檢查。** 孤兒（brief §1(a)）是競態也是這個崩潰窗；報告不把它寫成穩定重現。

### 可重跑（重構，不是 kill -9 插在兩次 rename 之間）

```bash
OO=/home/gali/nlang-baselines/v0.38.0-verify-target/release/oo
W=$(mktemp -d); export OO_IDENTITY="$W/id" OO_NODE_HOME="$W/nh"; cd "$W"
printf 'p: 1\n' > p.n && $OO evolve p.n
rm .oo/savepoints/LOG
$OO status          # 仍印 p: 1；不提孤兒 body
printf 'q: 2\n' > q.n && $OO evolve q.n
cat .oo/savepoints/LOG   # 一行 0000000000000001（覆寫，不是追加）
```

**§8 開弧 4**：本題無內建引用。不適用。

---

## Q2 — 還有誰在寫 `.oo/`（決定「一個物件」的邊界）

不經 `save_staged` 的寫入點：

| 檔／目錄 | 誰寫 | 檔案:行號 | 何時 |
| :-- | :-- | :-- | :-- |
| `.oo/format`、`.oo/objects.format` | `ObjectStore::init` | `storage.rs:216`–`219` | **新倉**（無 HEAD 且無 CAS 物件）。〔量〕第一次 commit **之前**的每一次 `oo` 都當成新倉，**重寫這兩個標籤**。有 HEAD 之後改走 `ensure_format`（唯讀，`storage.rs:180`）。 |
| 同上 | `migrate_layout` | `storage.rs:547`–`565` | `oo migrate --grant migrate`（`main.rs:1304`） |
| `.oo/objects/sha256/…` | `write_object` | `storage.rs:528`–`541` | `put_root`／`put_commit`／`put_value`／`persist_blur_partials`。內容定址，已存在則跳過。 |
| 同上（刪） | `remove_digest` | `storage.rs:277`–`280` | `gc::run_gc`（`gc.rs:282`–`324`；CLI `main.rs:1322`）。**只掃 `objects/`。** |
| `.oo/HEAD` | `set_head` | `storage.rs:516`–`523` | commit（`universe.rs:978`）、rollback（`:1084`）、squash（`:1163`）、refine（`:1488`） |
| `.oo/staged`（刪或重寫） | `Universe::commit` | `:981`–`994` | 成功 commit：有殘留 `~%Config` 則 `save_staged`，否則 `unlink` |
| `.oo/pin_pending`、`.oo/effect_pending`（刪） | `Universe::commit` | `:998`–`1008` | commit 成功之後，**HEAD 已經寫完** |
| `.oo/abandoned` | `append_abandoned_file`／`clear_abandoned_file` | `:1036`–`1054`、`:1009` | rollback 寫；commit／squash 清 |
| `.oo/discovery.n` | `DiscoveryConfig::write` | `discovery_config.rs:60`–`63` | `oo node trust …`，不是 evolve |
| `.oo/peers/directory` | `peers::append`（**append，不是 `atomic_write`**）／`compact` | `peers.rs:411`–`431`、`:450`–`472` | advertise／serve |
| `.oo/architects.json` | `save_architects` | `storage.rs:587`–`597` | CLI **零呼叫**（探針註解：函式對產品路徑已死） |

**不在工作區 `.oo/`**：`~/.oo/identity`、`~/.oo/nodes/`、node key 旁的 `.affiliation`（`oodp.rs:627`–`631`）。

**邊界。** 五個檔（staged／○ body／LOG／pin／effect）是**同一份工作集的五個投影**，由一次 `evolve` 寫出。HEAD 是已提交歷史的指標；CAS 物件不可變；peers／discovery 是另一個平面。

把「一個物件」畫成 `.oo/state` ＝ 那五個檔，Q-016 的 compare-and-swap 單位自然出現。把 HEAD 捲進去會讓每一次 commit 與每一次 evolve 搶同一把鎖，且把已提交歷史與暫存工作集變成同一個可交換位元組。**偵察建議邊界停在工作集**；這不是選甲／乙。

**§8 開弧 4**：不適用。

---

## Q3 — `encode_staged` 與 `encode_savepoint` 是不是同一個

```159:167:crates/interpreter/src/store_codec.rs
pub fn encode_staged(combo: &ComboVal) -> String {
    format!("{FRAME} staged\n{}", write_combo(combo, 0))
}

/// Same combo body as staged; the kind marks a local savepoint (○), whose
/// identity is the filename, not a CAID.
pub fn encode_savepoint(combo: &ComboVal) -> String {
    format!("{FRAME} savepoint\n{}", write_combo(combo, 0))
}
```

解碼把 `savepoint` 與 `staged` 收成同一個 `StoreDocument::Staged`（`:179`–`:181`）。

〔量〕`oo evolve` `a: 1` `b: 2`：

```
.oo/staged     = 23 6e 6c 61 6e 67 2f 73 74 6f 72 65 20 73 74 61 67 65 64 0a 7b 20 61 3a 20 31 20 62 3a 20 32 20 7d
                 #  n  l  a  n  g  /  s  t  o  r  e     s  t  a  g  e  d  \n {     a  :     1     b  :     2     }
.oo/savepoints/0000000000000001
               = 23 6e 6c 61 6e 67 2f 73 74 6f 72 65 20 73 61 76 65 70 6f 69 6e 74 0a 7b 20 61 3a 20 31 20 62 3a 20 32 20 7d
                 #  n  l  a  n  g  /  s  t  o  r  e     s  a  v  e  p  o  i  n  t  \n {     a  :     1     b  :     2     }
```

第一個不同位元組：**offset 14**，`73 74 61 67 65 64`（`staged`）對 `61 76 65 70 6f 69 6e 74`（`savepoint`）。第一行之後的 body **位元組相等**（`{ a: 1 b: 2 }\n` 的有無尾隨視 `write_combo`；本例 staged 33 B、savepoint 36 B，差在那一個詞）。

**合成一個物件可以共用一個編碼。** 框的 kind 是標籤，不是第二種 combo 形。甲若寫 `.oo/state`，合理的是一個新 kind（或沿用 `savepoint`）加上 pin／effect／前驅欄位；combo 本體不必再發明一種 `write_combo`。

### 可重跑

```bash
OO=/home/gali/nlang-baselines/v0.38.0-verify-target/release/oo
W=$(mktemp -d); export OO_IDENTITY="$W/id" OO_NODE_HOME="$W/nh"; cd "$W"
printf 'a: 1\nb: 2\n' > ab.n && $OO evolve ab.n
python3 -c '
from pathlib import Path
s=Path(".oo/staged").read_bytes(); b=Path(".oo/savepoints/0000000000000001").read_bytes()
print(s.split(b"\n",1)[0], b.split(b"\n",1)[0])
print("body equal", s.split(b"\n",1)[1]==b.split(b"\n",1)[1])
print("first diff", next(i for i,(x,y) in enumerate(zip(s,b)) if x!=y))
'
```

**§8 開弧 4**：不適用。

---

## Q4 — commit 的順序與崩潰窗

`Universe::commit`（`universe.rs:899`–`1010`）：

1. meet／`project_for_commit`／`put_root`／`put_commit`（CAS 新物件）
2. **`set_head`（`:978`）** — 歷史指標已指向新 commit
3. 若保留 `~%Config`：`save_staged`（又寫五檔）；否則 **`unlink .oo/staged`（`:991`–`994`）**
4. 清記憶體裡的 pin／effect，然後 **`unlink` 那兩個檔（`:1001`–`1008`）**
5. `clear_abandoned_file`

〔量，strace，標籤二進位，`a: 1` 然後 `b: 2` 再 commit〕rename 順序：三個 CAS 物件 → **`HEAD`** → 然後 `unlink(.oo/staged)`。`savepoints/` **零 rename、零 unlink**（D43：○ 活過 commit。✓）

**寫 HEAD 先於刪 staged。** 中間崩：

〔量，重構〕commit 成功後把舊 `staged` 拷回去：

* `oo status` 再度顯示已提交的 `{ a: 1 }` 為 staged。
* `oo commit -m again` **成功**，HEAD 從 `ebdcd6c5…` 換成 `e186afe7…`（新 commit，不是同一顆）。
* 工作集對已提交根做第二次 meet；payload 相同仍因 timestamp／message 鑄出新歷史。

**pin 的對稱窗**（HEAD 已新、`pin_pending` 沒刪）：把 `["x"]` 拷回後再 `evolve y: 2`。`load_staged` 把 leftover 讀回 `pin_pending=true`，`save_staged` **原樣寫回** `["x"]`。下一次 `oo commit`（無 `--grant pin`）拒 `#privileged_required`；帶 grant 則成功。一次崩潰可以把 pin 意圖接到後來無關的欄位上。

commit **不動** `savepoints/`，除非走 Config 殘留那條 `save_staged`。

### 可重跑

```bash
OO=/home/gali/nlang-baselines/v0.38.0-verify-target/release/oo
W=$(mktemp -d); export OO_IDENTITY="$W/id" OO_NODE_HOME="$W/nh"; cd "$W"
printf 'a: 1\n' > a.n && $OO evolve a.n
cp .oo/staged /tmp/staged.save
$OO commit -m a
cp /tmp/staged.save .oo/staged
$OO status          # Staged changes: { a: 1 }
$OO commit -m again # 第二顆 commit，HEAD 變了
```

**§8 開弧 4**：不適用。

---

## Q5 — `pin_pending`／`effect_pending` 的讀者，與身分

**讀者**（產品路徑）：

| 檔 | 讀 | 檔案:行號 |
| :-- | :-- | :-- |
| `pin_pending` | 存在與否 → `Universe.pin_pending`；JSON 座標 → `pin_coords`（壞檔＝空集合，不是「全部」） | `universe.rs:875`–`886` |
| `pin_pending`（閘） | commit 若 `pin_pending` 且無 `privilege.pin` → 拒 | `main.rs:1046` |
| `pin_pending`（合併） | `pin_commit_merge` 只替換列出的座標 | `universe.rs:914`、`:369` |
| `effect_pending` | bits → `EffectTag`，pure 丟掉 | `universe.rs:888`–`892` |
| `effect_pending`（閘） | 必須再出示**覆蓋**那些 tag 的 capability | `main.rs:1062`–`1074` |
| `effect_pending`（歷史） | 若 `Some`，`CommitMeta.privileged_effect = Some(true)` | `universe.rs:972`–`974` |

寫入：`save_staged` `:841`–`:858`。語言層寫入被 store boundary 擋住（`pin_probe`／`store_boundary` 探針）。

**進不進 CAID。** `pin_pending`／`pin_coords`／`effect_pending` 是 `Universe` 欄位（`:323`–`:340`），不是 `ComboVal` 欄位（`value.rs:886`）。`put_root` 只吃根 combo。

〔量〕同一份 `x: 1`：

| | 根 CAID（64 hex） | commit 雜湊 |
| :-- | :-- | :-- |
| 普通 evolve＋commit | `ca0986d5f6331359f34dd1d24a42a33e9a8a375605b78d444ba82e281bcde33c` | `58267bab…` |
| `--pin --grant pin` 再 commit | **同一個** `ca0986d5…` | **不同** `30fc05db…`（`kind: commit` 的 Pin 標記） |

`grep` 三個 CAS 物件：零命中 `pin_pending`／`effect_pending`／`["x"]`。根的 n/ 是 `{ x: 1, ~%__nlang_system_digest: "7038e250…" }`。

`effect_pending` 只把 `true` 寫進 **commit meta**，動的是 commit 物件的位址，不是根。普通 `x: 1` 沒有 discharge，本輪沒有另鑄一顆帶 `#privileged_effect` 的 commit（要跑 `runPure`）。程式路徑是 `:972`，不是值軸。

**變成 `.oo/state` 的欄位（W10 原文）不會動根身分**，前提是它們繼續不進 `ComboVal`、不進 `put_root`。若誤放進根 combo 或標準表，身分會動——那是實作紅線，不是本弧該做的。

**§8 開弧 4**：本題無內建。不適用。

---

## Q6 — 誰依賴現在的兩檔形

今天讀 `.oo/savepoints/` **形狀**的：

| 位置 | 依賴什麼 | 甲（單一 `.oo/state`）之後 |
| :-- | :-- | :-- |
| `a_store_you_did_not_write_probe_test.rs` `p1` `:206`–`:216` | committed 倉必須有 `.oo/savepoints/LOG` 與 `.oo/savepoints/<local-id>` | **紅**，除非驗收方改宣告 |
| 同檔 `oo_files` `:119`–`:124` | 把 `savepoints/` 下非 LOG 的名字折成 `<local-id>` | 無目錄可折 |
| `local_gc_probe_test.rs` `p4` `:852` | 允許名單含目錄名 `savepoints` | 目錄消失／改名 `state` → **紅** |
| `advert_persistence_probe_test.rs` `r2` `:685` | 同上，宣告了即使本情境不 evolve | 同上 |
| `kademlia_table_probe_test.rs` `p4` `:1317` | 允許名單 **沒有** `savepoints`：`objects, format, objects.format, peers` | 今天綠是因為**從不 evolve**。甲若在此情境寫出 `.oo/state`，這支會紅。**今天若有人在這支裡 evolve，savepoints 就會讓它紅。** 潛伏針。 |
| `depth_belongs_to_the_savepoint_probe_test.rs` 檔頭 `:44`–`:50` | 註解仍寫「沒有 ○ 實體」（Q-013 偵察當時） | 斷言不讀目錄；註解過期，不是閘 |
| `crates/oo/src/` | **零命中** `savepoint` | 無 CLI 相容約束（brief §1(d) 仍成立） |

**不讀兩檔形、但讀 `staged`／`pin_pending` 路徑**（甲若連這兩個檔名一起收掉，它們也紅）：

* `atomic_write_probe_test.rs` R1（`.oo/staged` inode）、R3（`.oo/pin_pending` inode）
* `local_gc_probe_test.rs` `p6`（GC 後 `staged` 位元組不變）
* `every_byte_or_none`／`a_value_not_a_recipe`／`a_store_written_in_another_language`／`where_the_conflict_is`（讀 `.oo/staged` 內容或有無）
* `pin_probe_test.rs`、`store_boundary_probe_test.rs`（`.oo/pin_pending` 有無）

乙不改路徑：上表版圖釘全綠。

**§8 開弧 4**：不適用。

---

## Q7 — 合成 `.oo/state` 之後誰會壞

假設甲：`staged`＋○ body＋`LOG`＋pin＋effect → 單一 `.oo/state`，一次 `atomic_write`。

| 面 | 今天 | 甲之後 |
| :-- | :-- | :-- |
| **GC 走訪** | `mark` 只從 HEAD 走 CAS（`gc.rs:199`–`:210`）。staged／○ **不是根**。blur partial 只被 staged 指名時，GC 可以在 commit 前收掉它們（`p6` 靠「staged 內聯、不指名 CAID」才綠）。 | `.oo/state` 若仍不進 `mark`，同一洞還在。要修洞得讓 GC 讀 state——那是額外工作，不是合成的免費後果。○ 若不再是獨立檔，乙那種「孤兒 body」問題消失。 |
| **`oo inspect`** | 只解析 CAID、讀 CAS（`main.rs:1538`）。 | 不動。沒有 savepoint 表面（brief §4 不做 Q-018）。 |
| **舊 `oo` 開新倉** | `advert` P6：未知 `.oo/` 項目不破壞 evolve／commit／log；`.oo/format` 不因此升版。 | 舊引擎看不懂 `.oo/state`，會繼續找 `.oo/staged`。工作集對它是空的。若 **不**升 `layout=`，舊引擎**打得開**、**看不見暫存**、○ 鏈若搬走也看不見。若升到 `layout=3`，舊引擎拒（`ensure_format`）。 |
| **新 `oo` 開舊倉** | — | 必須雙讀（先 `state`，沒有再 `staged`＋`savepoints/`）或遷移寫回。這筆成本在甲，不在乙。 |
| **`.oo/format` 升版** | 現在 `layout=2`（`storage.rs:125`）。Q-013 加 `savepoints/` **沒有**升版，是驗收方改三支針。 | **不是自動要升。** 升版＝舊引擎拒絕新倉。不升＝靠「未知檔可忽略」＋新引擎雙讀。p1 的文案把「加檔」本身當 layout 變化——那是探針契約，不是 `STORE_LAYOUT_VERSION` 加一。兩件事要分開裁。 |

身分：`.oo/state` 不是 CAS 物件，不進 `x: 0` 的 3 個物件。前提是不要把工作集放進 `objects/`。

**§8 開弧 4**：不適用。

---

## Q8 — 一次 `oo evolve` 的磁碟寫入（實測）

方法：標籤二進位 + `strace -e renameat,unlink,…`。每一次 `atomic_write`＝開 `.partial-*`＋rename 到目標。依 brief「含 temp＋rename 的兩次」：1 次 `atomic_write` ＝ **2** 次安裝。`write`／`fsync` 在 Rust 裡不一定以 `write`／`fsync` 出現在同一條 filter；次數以 `renameat` 次數為準（與 `atomic_write` 呼叫 1:1）。

### 今天

| 情境 | `renameat`（即 `atomic_write` 次數） | temp＋rename | 寫了誰 |
| :-- | --: | --: | :-- |
| **第一次 evolve**（無 HEAD、無 CAS） | **5** | **10** | `format`、`objects.format`、`staged`、○ body、`LOG` |
| 第二次 evolve（新欄位，仍無 HEAD） | **5** | **10** | 同上（init 仍當新倉）＋新 ○ id |
| 再 evolve 已有欄位（D47：body 相同，不鑄 ○） | **3** | **6** | `format`、`objects.format`、`staged` |
| **有 HEAD 之後** evolve 新欄位 | **3** | **6** | `staged`、○ body、`LOG` |
| 有 HEAD、`--pin --grant pin` 的第一次 evolve | **6** | **12** | 無 HEAD 的 5 個＋`pin_pending` |
| 有 HEAD 的普通 evolve，**工作集這三檔** | **3** | **6** | 這是 save_staged 的穩態數字 |

`pin_pending`／`effect_pending` 不存在時是 `stat`／`open` ENOENT，**不寫**。

額外發現（brief 五檔表沒列）：**第一次 commit 之前，每次 `oo` 都重寫 `format` 與 `objects.format`**，因為 `init` 的「新倉」判準是「沒有 HEAD 且沒有 CAS 物件」（`storage.rs:215`）。這 2 次不屬於工作集，甲／乙都不會自動拿掉。

### 甲之後（工作集合成一檔；init 行為不變）

| 情境 | 今天 | 甲 |
| :-- | --: | --: |
| 有 HEAD、普通 evolve | 3／6 | **1／2**（`.oo/state`） |
| 有 HEAD、帶 pin | 4／8（再加 `pin_pending`） | **1／2** |
| 第一次 evolve（無 HEAD） | 5／10 | **3／6**（format×2 + state） |
| D47 跳過鑄 ○ | 有 HEAD：1／2（只重寫 staged） | 若仍每次覆寫 state：1／2；若 byte-identical 整檔跳過：0／0 |

乙：穩態仍是 3／6；GC 多的是讀與偶爾 `unlink` 孤兒，不是每次 evolve 的寫入。

### 可重跑

```bash
OO=/home/gali/nlang-baselines/v0.38.0-verify-target/release/oo
W=$(mktemp -d); export OO_IDENTITY="$W/id" OO_NODE_HOME="$W/nh"; cd "$W"
printf 'a: 1\n' > a.n
strace -f -e trace=renameat,renameat2,unlink -o /tmp/ev.st $OO evolve a.n
grep renameat /tmp/ev.st
# 有 HEAD 之後：
$OO commit -m a
printf 'b: 2\n' > b.n
strace -f -e trace=renameat,renameat2,unlink -o /tmp/ev2.st $OO evolve b.n
grep renameat /tmp/ev2.st   # 三行：staged, 0000000000000002, LOG
```

**§8 開弧 4**：不適用。

---

## Q9 — 甲／乙報價（不選）

### 甲 — 合成一個物件

**要動的檔（生產碼）**

| 檔 | 今天 | 甲要做什麼 | 約略行數 |
| :-- | --: | :-- | --: |
| `savepoint.rs` | 83 | 重寫或刪：不再 mint 檔名、不再整檔重寫 LOG | 83 重寫 → 新模組約 80–150 |
| `universe.rs` `save_staged` | `:817`–`:859`（43） | 一次 `atomic_write(.oo/state)` | 43 → ~25 |
| `universe.rs` `load_staged` | `:862`–`:893`（32） | 讀一個檔；雙讀舊形若要開舊倉 | 32 → ~40–80 |
| `universe.rs` `commit` 清場 | `:981`–`:1008`（28） | 更新／刪 state 裡的工作集欄位，不是 unlink 三個檔 | ~20 |
| `store_codec.rs` | `encode_savepoint` 5 行；decode 已收 `savepoint` | 新 kind 或加欄位；combo body 共用 `write_combo` | +15–40 |
| `gc.rs` | 366 | 可選：把 state 當 blur-partial 的根 | 0 或 +30–50 |

生產碼合計：**約 200–280 行**（不含「開舊倉雙讀」的上限；含則再 +50）。不碰 `ComboVal`、不碰 `put_root`、不動身分。

**探針／版圖（必紅、必改宣告或改路徑）**

* 三支已宣告 `savepoints` 的版圖：`p1`、local_gc `p4`、advert `r2`
* 潛伏：kademlia `p4`（今日名單沒有 `savepoints`）
* 七支讀 `.oo/staged` 的探針（若檔名消失）
* `atomic_write` R1／R3、`pin_probe`、`store_boundary`（若 `pin_pending` 檔名消失）

約 **10–15 個測試檔**，多數是路徑／允許名單，不是語義。

**甲內部還有一個未裁的子岔路**（brief 已點到「鏈要換表示法」）：

| | 鏈放哪 | 代價 |
| :-- | :-- | :-- |
| 甲-1 | `.oo/state`＝當前工作集＋id 列表；舊 body **仍是獨立檔** | 交換單位出現了；body 孤兒窗還在（較小） |
| 甲-2 | 整條鏈內嵌進那一個檔 | 真・單檔；每次 evolve 重寫整段歷史，體積隨 ○ 線性長 |
| 甲-3 | 只留當前快照 | 最便宜；**放棄 ○ 鏈**，與 D43「每個 ○ 都已經持久」的歷史讀法衝突 |

報價不含選這個子岔路。甲-2 的寫入成本會吃掉 Q8 看起來省下的次數（1 次 `atomic_write`，但 payload 變大）。

> **⚠ 附錄撤回（2026-08-29）**：上表甲-1／甲-2／甲-3 把「鏈放哪」當成甲的子岔路。那個框架是錯的——四個候選都沒回答「○ 的身分是什麼」。數字仍是當時對「鏈」的報價，不再作為裁定輸入。見文末 §7。

**Q-016**：compare-and-swap 的單位就是 `.oo/state`。commit 的 HEAD-then-clear 窗**還在**（Q4）——那是另一個指標，不是這一個物件。

### 乙 — `LOG` 為唯一真相，孤兒交給 GC

**要動的檔**

| 檔 | 做什麼 | 約略行數 |
| :-- | :-- | --: |
| `gc.rs` | 掃 `.oo/savepoints/`，LOG 沒列的檔 `unlink` | +40–60 |
| `savepoint.rs` | 可選：`record` 結束後順便收；或不改、留給 `oo gc` | 0–10 |
| `main.rs` `run_gc` | 已呼叫 `run_gc`；報表要不要提 ○ 孤兒 | 0–15 |
| 版圖釘 | 兩檔形不變 | **0** |
| `mint_id` | **不動則並行仍撞同一檔名** | 另案；乙的最小形不解它 |

合計：**約 50–80 行**。探針路徑全綠。

**乙解不掉的：**

* `save_staged` 五檔合起來仍不原子（Q1、Q4 的 pin leftover、brief §1(a) 並行）
* 身分仍從一次過期的讀衍生（`savepoint.rs:37`–`39`）
* Q-016 仍然沒有「一個」可交換物件，還得再做一次甲、或給 LOG 做 compare-and-swap（LOG 不含 staged／pin／effect，單位是錯的）

**驗收方傾向甲**（brief §3，理由 §1(d)）。這裡只標價。

**§8 開弧 4**：不適用。

---

## §8 收弧 3c — 這組並行量測沒掃到的耐久檔

brief §1(a) 的 40 並行 `evolve` 看了 `staged` 欄位數、`savepoints/LOG` 行數、`savepoints/` 檔數。**沒掃：**

| 檔 | 為什麼要寫 |
| :-- | :-- |
| `.oo/pin_pending`、`.oo/effect_pending` | 就在 `save_staged` 第 4–5 步。普通加欄位的並行測試用不到 `--pin`／discharge，所以表上永遠像「沒這兩個檔」。 |
| `.oo/format`、`.oo/objects.format` | 無 HEAD 時每次 evolve 都重寫（Q8）。並行 40 個 evolve 若同倉且尚未 commit，這兩個標籤被重寫 40 次。 |
| `.oo/abandoned` | rollback 路徑，evolve 不寫。 |
| `.oo/peers/directory` | append，不是 `atomic_write`。另一個非原子耐久面。 |
| `.oo/discovery.n` | trust 平面。 |
| `.oo/HEAD`、`objects/` | commit／CAS；不是這組 evolve 量測的對象。 |

kademlia `p4` 的允許名單**沒有** `savepoints`（Q6）：不是 3c 的「新檔沒被量」，是「版圖釘沒覆蓋到會鑄 ○ 的情境」。

甲若新增 `.oo/state`，依 3c：**必須對新檔重跑** 40 並行 evolve。乙若只讓 GC 刪孤兒、不加檔，3c 不觸發；但 brief §1(a) 那種 4–6／40 的丟失，乙的 GC 只收檔、不把欄位寫回 staged。

---

## 明確不做（複述，以免被讀成工單）

compare-and-swap 與重試（Q-016）。staged 並發語義。觀測邊界寫 ○。CLI savepoint 動詞。動身分。選甲或乙。選 A／B／C。

---

# 附錄 — 身分的三個候選（2026-08-29）

> 驗收通過後追加（brief 附錄 §7）。**不是新弧。** 甲／乙仍等用戶，不在這裡選。
> 本節**取代**原文 Q9 把「鏈放哪」寫成甲之子岔路的那個框架。
>
> 另：原文 Q3 的 hex 印成 `23 62 79 6e 67`（`#byng`）。那是**無效證據**；結論不動（body 相等、首差 offset 14）。已在 Q3 更正為 `23 6e 6c 61 6e 67`（`#nlang`）。
>
> **身分**：本附錄零改動。known-answer 再過：`~%Math./add (1,2)` → `3`。

## 7.0 一句

`mint_id` 產出的那個字**同時是身分與順序**，且兩者都從一次可能過期的讀而來。先拆開，再報價。C 是 Q-016，列在這裡只為比價，**不是選項**。

| | 一句 |
| :-- | :-- |
| **Q10 A** | `savepoint.rs` 的 `mint_id` ＋框上一個前驅欄位：約 **60–90** 行（`store_codec` 佔一半）。版圖路徑不變。 |
| **Q10 B** | `mint_id` 改成 `O_EXCL` 發放迴圈：約 **25–40** 行。仍是「每 ○ 一個檔」。 |
| **Q10 C** | 不是換 `mint_id`。那是甲＋compare-and-swap 整弧（Q-016）。 |
| **Q11** | brief 寫的不變式（LOG 行都有檔、LOG ≦ 目錄檔數）在 §1(a) 的孤兒上**是綠的**。要釘今天的病，得寫**反方向**。A／B 在 40 並行下仍會丟 LOG 裡的順序，但身分不再撞。 |
| **Q12** | 只有前驅、沒有旁路索引：讀第 n 個是 **O(n)** 次開檔。今天 LOG 是 O(1) 索引。Q-018 就是那個讀者。 |
| **Q13** | **B 與甲不相容。** 甲之後沒有「每個 ○ 一個檔」可 `O_EXCL`。發放若另做 sidecar，甲就不是一個物件；若做進 `.oo/state` 的 compare-and-swap，那是 C。 |
| **Q14** | 40 並行 `oo node advertise`、四輪，皆 **40／40**、0 壞 JSON。產品路徑是單執行緒 serve，與 40 行程寫同一倉的 evolve **不是同類競爭**。`append` 確無 `sync_all`；`compact` 有。已開 Inbox，本弧不修。 |

---

## Q10 — 三個候選各動幾行

今天 `mint_id`（`savepoint.rs:37`–`39`）8 行語意、`record`（`:52`–`:77`）把那個 id 當檔名也當 LOG 的下一行。`encode_savepoint`（`store_codec.rs:165`–`167`）沒有前驅欄位。

### A — 隨機本地 id，順序來自前驅指標

| 檔 | 做什麼 | 約略行數 |
| :-- | :-- | --: |
| `savepoint.rs` `mint_id` | 不讀 LOG。用已有的 `ring::rand::SystemRandom`（`oodp.rs`／`value.rs` 已依賴）鑄本地 id（寬度是實作選擇：維持 16 hex 或加長）。 | 8 → ~15 |
| `savepoint.rs` `record` | 上一顆的 id 寫進新 body 的前驅欄位；LOG 若仍在，只是索引，**不再是身分來源**。 | ~15 |
| `store_codec.rs` | 框加 `predecessor:`（或等價欄位）。必須在 combo 之外——放進 `ComboVal` 會讓「變成欄位」碰到 Q5 那條身分紅線，即使 ○ 不是 CAS。decode 跳過該欄，body 仍走 `write_combo`。 | +30–50 |
| 版圖釘 p1／p4／advert r2 | 路徑仍是 `savepoints/<local-id>`（id 仍非 CAID）。 | **0**，除非檔名寬度變了而有人釘 16 字 |

合計 **約 60–90 行**。不碰 `ComboVal`、不碰 `put_root`。

與甲疊加：隨機 id 與前驅變成 `.oo/state` 裡的兩個欄位，不再鑄檔名。那時 A 的「不讀就能鑄」仍然成立（鑄的是欄位值，不是檔名）。

### B — `O_EXCL` 原子發放單調計數

| 檔 | 做什麼 | 約略行數 |
| :-- | :-- | --: |
| `savepoint.rs` `mint_id`／`record` | 從 `n = ids.len()+1` 改成：`OpenOptions::create_new(true)` 建 `format!("{n:016x}")`，`EEXIST` 則 `n += 1` 重試。LOG 仍整檔重寫。 | +25–40 |
| 其他 | 編碼、版圖、CLI | 0 |

合計 **約 25–40 行**。這是三個裡最便宜的**只修身分撞檔名**。順序仍編碼在 id 裡（兩份工作還黏在一起，只是發放原子了）。**每 ○ 一個檔**——見 Q13。

### C — 把 mint 放進 compare-and-swap（越界，不是選項）

不是 `mint_id` 的補丁。工作集必須先是一個可替換物件（甲），再在那一個物件上 compare-and-swap 含「下一號／前驅／combo」的整份狀態。行數＝甲（Q9：200–280）＋ Q-016 的重試迴圈，不是本表能單獨報的「換鑄造函式」。列在這裡只為了說：**不要拿 B 的 30 行去跟 C 比，單位不同。**

**§8 開弧 4**：不適用。

---

## Q11 — 40 並行 `evolve` 下會得到什麼；那支探針紅不紅

brief §1(a) 今天：40 次位置移動，存活欄位 4–6，LOG 4–6 行，偶爾多一個孤兒檔。成因是（1）身分與順序都從過期的讀衍生（撞檔名、LOG 最後寫入者贏），（2）`staged` 本身也是最後寫入者贏。

| | 身分 | 順序（LOG／鏈） | `staged` 欄位 | brief 那條不變式 |
| :-- | :-- | :-- | :-- | :-- |
| **今天** | 撞檔名（兩行程同一 N） | 錯（過期的 `ids.len()`） | 4–6／40 | **綠**（見下） |
| **A** | 實務上不撞 | LOG 若仍整檔重寫：最後寫入者贏，**順序仍丟**；前驅寫在各自 body 裡，孤兒 body 仍在磁碟上，鏈在檔裡、LOG 對不齊 | 仍 4–6／40（staged 沒改） | 綠（LOG ⊆ 檔案，而且更常 `LOG ≪ 目錄`） |
| **B** | 不撞（`EEXIST` 跳號） | 同上：LOG 最後寫入者贏；檔案可能有 40 個、LOG 4–6 行 | 仍 4–6／40 | 綠，同 A |
| **C** | 由成功的那一次交換決定 | 與 combo 同一原子單位 | 仍可能丟（meet 重試是 Q-016；沒做交換協定就還是最後寫入者） | 交換成功則綠 |

**brief 點名的不變式**：「每個 LOG 行都有對應檔案、且 LOG 行數 ≦ 目錄檔數」。

這條釘的是**懸空指標**（LOG 認識、檔不在）。§1(a) 量到的是**孤兒**（檔在、LOG 不認識）。孤兒讓目錄**大於** LOG，不等式成立；若那 5 個 LOG id 都還在，第一句也成立。trial 0（LOG 5、目錄 6）**不會讓這支探針紅**。

**能不能寫成會紅的探針？能，但不是那兩句。** 要釘今天的病，不變式得是**反方向**的：

* 每個目錄檔（除 `LOG`）都出現在 LOG 裡 ⟹ 孤兒就紅（trial 0）。
* 不要釘「等於 40」——並行下 D47、最後寫入者、失敗的 evolve 都會讓 40 不是常數。

A／B 之後，反方向那支**仍然紅**（LOG 最後寫入者贏，多出來的是身分不撞的孤兒）。C 在交換協定落地前也紅。所以那支探針測的是「LOG 與目錄的雙射」，不是「選了 A 還是 B」。

**§8 開弧 4**：不適用。

---

## Q12 — A 的前驅會不會讓「讀第 n 個」變成 O(n)

今天沒有讀者（brief §1(d)）。順序的索引是 LOG：第 n 行 → 一次 `open` body。**O(1) 定位 + O(1) 讀。**

A 若**只**在每個 ○ 上記前驅、不另存索引：

| 讀法 | 代價 |
| :-- | :-- |
| 已知 id，讀那一顆 | 仍 O(1)。`savepoint::load` 不變。 |
| 第 n 個（從最舊數） | 從無前驅的那顆走 n 步，**n 次開檔**。 |
| 第 n 個（從最新數，Q-018 `--dry-run` 相對 HEAD／當前 ○） | 從尖端往回走，仍 **O(n)**。 |
| 退回到某一顆（W22） | 先找到那顆（O(n) 或 O(1) 若呼叫者已有 id），再讀一次 body。 |

Q-018／W22 的 `--dry-run` 與退回**就是**這個讀者。寫側 A 很便宜（Q10：60–90 行）；讀側在 n 隨使用成長之後，會把「列出歷史／挑第 n 顆」從一次讀 LOG 變成一次鏈走訪。

若為了讀側再留一份 LOG 當索引：定位回到 O(1)，但**順序又有兩個家**（LOG 與前驅），而今天的病正是兩個家可以打架。那不是免費的；那是把拆開的兩份工作又疊回去，只是身分不再從 LOG 衍生。

**§8 開弧 4**：不適用。

---

## Q13 — B 的 `O_EXCL` 與甲相不相容

**不相容。** 照寫。

B 的發放單位是**檔名**：`create_new(".oo/savepoints/{n:016x}")`，撞到就 `n+1`。甲把五檔塌成**一個** `.oo/state`。甲之後沒有「每個 ○ 一個檔」可以 `O_EXCL`。

剩下的掛載點：

| 發放做在哪 | 結果 |
| :-- | :-- |
| sidecar，例如 `.oo/savepoints/NEXT` 或仍留下 per-○ 檔 | 甲不是一個物件；Q-016 的單位又裂開 |
| `.oo/state` 裡的單調欄位，用 compare-and-swap 改它 | 那是 **C**／Q-016 |
| 放棄單調計數，改隨機 id（A）當 state 裡的欄位 | 那是 A＋甲，不是 B |

這是甲**今天不在 Q9 帳上**的代價：選甲就關掉 B。B 的 25–40 行只在「仍是每 ○ 一個檔」（乙、或甲-尚未發生）時有意義。

**§8 開弧 4**：不適用。

---

## Q14 — `.oo/peers/directory` 的並行量測，與斷電語義

### 〔讀〕耐久性

`peers::append`（`peers.rs:411`–`428`）：

```411:428:crates/interpreter/src/peers.rs
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut f) => {
            ...
            if writeln!(f, "{line}").is_ok() {
                let _ = f.flush();
                ...
            }
        }
```

`flush()` 有。**沒有 `sync_all`／`fsync`。** `atomic_write`（`storage.rs:32`–`34`）有 `sync_all`。屬實。

同一檔的另一條路 `peers::compact`（`:450`–`:475`）走 `atomic_write` ⟹ **同一個 `.oo/peers/directory`，append 與 compact 的斷電語義不同**。規格沒有為這個檔開過例外。

`REAL_01` §4.1.1 寫的是「`.oo/` 之下**任何**耐久寫入必須是臨時檔 + `fsync` + `rename`」。附錄把它標成 §2.5（引擎做了規格沒描述的選擇）；同一條也可以讀成 **§2.4**（MUST 未兌現）。Inbox 兩種入口都寫上，不在本弧裁。

### 〔量〕40 並行 advertise，四輪

標籤二進位。一個 `oo node serve`，四十個獨立身分的 `oo node advertise --to 127.0.0.1:$PORT` 同時跑。與 §1(a) 同構的「四十個呼叫」，**不是**四十個行程寫同一個工作集——寫入發生在 **serve 行程**裡，而 `run_serve`（`main.rs:542`–`577`）是**單執行緒**、一個請求做完才接下一個。

| trial | 回 `#success` | serve 記 `append` | `compact` | 目錄 data 行 | 獨特 `node_id` | 壞 JSON |
| :-- | --: | --: | --: | --: | --: | --: |
| 0 | 40／40 | 40 | 0 | 40 | 40 | 0 |
| 1 | 40／40 | 40 | 0 | 40 | 40 | 0 |
| 2 | 40／40 | 40 | 0 | 40 | 40 | 0 |
| 3 | 40／40 | 40 | 0 | 40 | 40 | 0 |

對照 §1(a) 的 evolve：4–6／40。**產品路徑上，peers 目錄沒有表現出那種丟失。** 原因是競爭的位點不同（單行程串列 append vs 四十行程搶 `staged`／LOG），不是因為 append 比較原子。

旁量（**不是**產品路徑；一輪，不寫成穩定重現）：兩個 `oo node serve` 開在**同一個**工作區、四十個 advertise 對半打。回覆 40／40，目錄 40 行獨特、0 壞 JSON。POSIX `O_APPEND` 對 ~900 B 的整行寫入通常一次 `write` 就進檔；這一輪沒撕行。不斷言「雙 serve 永遠安全」。

並行量測**量不到**斷電語義。`flush` 而不 `fsync` 的洞是電源／殺行程時最後幾行仍在頁快取裡——那要崩潰注入，本輪沒做。

### 可重跑

```bash
OO=/home/gali/nlang-baselines/v0.38.0-verify-target/release/oo
S=$(mktemp -d); export OO_IDENTITY="$S/id" OO_NODE_HOME="$S/nh"; cd "$S"
printf 'seed: 1\n' > seed.n && $OO evolve seed.n && $OO commit -m s && $OO node id >/dev/null
$OO node serve --port 0 >serve.log 2>&1 &
sleep 0.3
PORT=$(grep -oP 'serving at port \K[0-9]+' serve.log)
# 四十個平行 advertise（各自獨立 OO_IDENTITY）見本節量測腳本；核對：
# grep -c success ads/*/out  → 40
# python3 讀 .oo/peers/directory 的 JSON 行數 → 40
```

**§8 開弧 4**：本題沒有引用內建的回傳值（`#success` 是線上 `%status`）。不適用。known-answer 的 `~%Math./add` 只用於證明二進位。

⟹ 把 ○ 鏈改成 append-only log，等於採用**產品路徑上靠單執行緒才看起來沒事、斷電語義比 `atomic_write` 弱**的那個面。孤兒類別（懸空指標）可能換成「state 與 log 過時」，不是消失。

Inbox 一列開在 `nlang-spec` `local`（本弧不修程式）。
