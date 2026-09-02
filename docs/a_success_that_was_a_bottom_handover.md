# 工單：一次回報成功的提交，其實是一個 `_|_`

**Queue ID**：**Q-017**（Ready 表第 1 列，Active）。
**基線**：`v0.41.0`，`/home/gali/nlang-baselines/v0.41.0-verify/target/release/oo`
（`oo v0.41.0`；known-answer `~%Math./add (1,2)` → **`3`**，**對照** `add (1,"x")` → **`_|_`**）。
**你也要先做一次 known-answer 加對照。**`oo --version` 不知道自己是哪一版（Inbox 四例）⟹ 用行為確認。
**⚠ 取離開碼不得經過管線**（`$?` 在 `cmd | head` 之後是 `head` 的；本專案犯過兩次）。

**裁定依賴（全部已裁）**：
**B1**（`commit.md` §2.1.3，2026-08-06）／**D56**（§4.1.2 射程改軸）／**D57**（審計歸 ●）。
偵察：`a_success_that_was_a_bottom_recon.md`（九題 ＋ 驗收 ＋ D46 對帳）。

---

## 1. 這一弧兌現的是一條**既有的** MUST

`SPEC_10` §4.1.2 **回報的內容（MUST）**逐字：

> 引擎**必須**指出 `_|_` 落在哪些座標，而**不僅回報「成功」**。
> **靜默地固化一個帶矛盾的狀態，等於讓操作者在不知情下做決定。**

**那一句寫於 2026-08-06，逐字描述了引擎今天做的事，而它從未被實作。**

**D56 解掉了「它算不算在射程內」**：§4.1.2 排除「**顯式**地固化含 `_|_` 的內容」，
而**「顯式」是一個關於操作者意圖的主張，只有在他被告知時才為真**——
**而唯一能告知他的就是這條 MUST** ⟹ **排除款無法在不先滿足它所排除的那條 MUST 時進入。**

**D57 解掉了「記在哪」**〔量〕同一段含 ⊥ 的內容在兩個獨立的倉各自提交，**根逐位元組相同**
（`cabe2ee25f29cc9a…`）⟹ **同意不是一個值，進不了根** ⟹ **不單獨記下就永久不可回復**。
歸屬 ● 而非 ○：`SPEC_08` §6.2 ＋ `REAL_01` §7.3——**○ 是工作區裡的檔案，位於斷言層**，
任何能寫 `.oo/` 的人都能偽造。

---

## 2. 射程（S1–S5）

### S1 — 從投影後的根收集 `(葉座標, cause)`

*   **必須到葉**（§2.2.1 的理由句：「衝突揭露的最小單位是座標」）——巢狀結構下不得只報頂層欄位。
*   **不得讀 `message`**。那個欄位是 Rust `Debug`（`Atom(Int(1), EffectTag(0), None)`），
    §2.2.1 **MUST NOT** 禁止宿主語言的除錯輸出格式，且它另有一列 Inbox（服務面）。
    **報告只要座標與 `TAG_REGISTRY` 的 cause 名。**
*   `format_conflict_where`（`main.rs:15`）已是這個形，**可重用**；它已經不印 `message`。

### S2 — `oo commit` 把它們說出來，**然後照樣落地**

*   **rc 維持 0。** 提交沒有失敗——操作者被告知了。
    （`SPEC_10` §2.2.2 的「離開碼不得為零」治的是 **fold 出來的工作集**，那條路今天就對，見 S5。）
*   **不得改成拒絕。** B1 逐字：**要修的不是「中止」，是「報告」**。**探針 G2 是這一格的守衛。**

### S3 — `CommitMeta` 加一個 `Option` 審計欄位

*   **照 `abandoned`／`privileged_effect` 的手寫 `Debug` 模式**（`value.rs:2348`）：
    **`None` 時不進 `debug_struct`** ⟹ **不進 digest** ⟹ **乾淨的提交 CAID 逐位元組不動**。
    **探針 G1 是這一格的守衛**（`x: 0` 根仍 `31745ef0…`、3 物件）。
*   **只有在真的回報過 ⊥ 時才是 `Some`。**

### S4 — 標記只准斷言實際發生的事

`SPEC_08` §6.2 逐字：**「標記必須反映事實而非旗標」**——
不得僅因能力在場或操作被呼叫即標記；**斷言未曾發生之干預，與隱匿已發生之干預，同屬會說謊的審計面**。

⟹ **一次性的 CLI 問不到「同意」。** 實際為真的是「**座標被回報了，而提交繼續**」。
**你的標記不得宣稱比這更多。** 逐字寫進報告它斷言什麼（§4 Q1）。

### S5 — 不加 grant、不加旗標

**D57**：提交 ⊥ **並不被禁止**（§2.3.1 逐字：撞到 ⊥ 者仍**必須**寫入 ⊥ 與其 `%cause`）
⟹ 沒有東西需要被許可 ⟹ **這是審計不是授權**。加旗標還會直接撞上 S4 那條。

---

## 3. 明確不做

*   **不碰 `message`**（Inbox，`interrupt-candidate`，服務面）。報告不得抄它一遍。
*   **不動 `oo status` 的求值深度**。`c: c + 1` 在 status 印未化約原文而根是 ⊥ ——**真的不一致，
    但那是求值深度問題不是回報問題**，且 §2.2.1 的公式到不了它（兩個 Combo 的 meet 恆為 Combo）。
    **本弧不修，驗收方會把它開進 Inbox。**
*   **不動 B 路徑**（fold 出來的工作集撞 ⊥ ⟹ 中止、rc=1）。那是 §2.2.2 的產品承諾。**G3 守它。**
*   **不動身分**：`x: 0` 根 `31745ef0…`／3 物件／標準根 `7038e250…`。
*   不碰 Q-016、Q-018。

---

## 4. 你必須回答的

**Q1.** 你的標記**逐字斷言什麼**？（§6.2 S4）寫出那句話，並說明**它為什麼不多說**。

**Q2.** 一次提交可以有多個 ⊥ 座標（偵察 Q4 已量：`x`／`y` 並存、巢狀 `a.b`／`a.c` 並存）。
**你報幾個？** §4.1.2 說「哪些座標」（複數）、§2.2.1 說「必須到葉」。
**若你只報第一個，逐字說為什麼**——那會是一個要進 Inbox 的缺口。

**Q3.** `c: c + 1` 的 ⊥ **只在算根時才出現**（偵察 Q2 已量：它的 ○ 裡是未化約的 thunk）。
所以報告與標記**在管線的哪一步產生**？`put_commit` 之前還是之後？
**若在之後，那顆 commit 的 digest 是不是已經定了**——標記怎麼進得去？**帶指令說明你的順序。**

**Q4.** **跨版本必須先量，不得推論**：`v0.41.0` 的引擎讀到一顆帶新欄位的 commit 物件會怎樣？
**丟掉、報錯、還是拒絕開倉？**（Q-015 偵察 Q9 丁 逐字把這一項列為「實作前必須先量」，
而當時沒有做。**現在真的要加欄位了，所以現在量。**）
用真的 `v0.41.0` 標籤二進位，**逐字給輸出**。

**Q5.** 實際行數對照偵察 Q7 的報價（走訪 40–60 ＋ CLI 15–30 ⟹ ~50–90）。**差多少、為什麼。**

---

## 5. 探針

`crates/oo/tests/a_success_that_was_a_bottom_probe_test.rs`。
**基線 3 綠 2 紅，三輪皆同**（release）。**兩支紅各倒在自己的斷言上，不是倒在 REACH。**

| | 名字 | 基線 | 釘什麼 |
| :-- | :-- | :-- | :-- |
| **R1** | `r1_a_commit_that_writes_a_bottom_says_where_it_landed` | **紅** | commit 的輸出要有座標與 `#divergent`，**且不得含 `EffectTag`**（不抄 `message`）。**不釘句子** |
| **R2** | `r2_the_commit_records_that_it_was_reported` | **紅** | 帶 ⊥ 的 commit 物件要有一個乾淨 commit 沒有的**鍵**。**比較的是鍵集不是值**，故不釘欄位名 |
| **G1** | `g1_a_clean_commit_keeps_its_identity` | 綠 | **D57 的成本論據**：`x: 0` 根仍 `31745ef0…`、3 物件。紅了代表標記被無條件寫入，**全世界的乾淨 commit 都搬家了** |
| **G2** | `g2_a_reported_bottom_still_lands` | 綠 | **B1／D56**：報告不是中止。rc 仍 0、HEAD 仍在。**讓 R1 過的最省事作法就是拒絕提交，而那正是被拆掉的那一半** |
| **G3** | `g3_the_fold_path_still_refuses` | 綠 | §2.2.2 的 B 路徑仍 rc≠0 且報座標。**以種檔造成，不賽跑，不釘排程器** |

**探針完整性**：可以動的只有 `#[ignore]` 那一行，**其餘一字不得動——`rustfmt` 也不行**。
認為哪一支釘錯了，**寫進報告，不要改它**。

**⚠ 逐字記下沒有探針的一件**：**「操作者同意了」無法被斷言。**
一次性 CLI 問不到同意，實際為真的只有「座標被回報了，而提交繼續」。
依 §6.2 標記不得宣稱比發生的更多，**故本弧要的是 Q1 的一句書面自陳，不是一個斷言**。

---

## N. 交付回報（交付方填；本行以上一字不得動）

### N.1 射程逐項對照

**S1.** `Value::bottom_leaves` 走完投影後的根（六個軸），收集 `(葉座標, #cause)`。不讀 `message`。巢狀是 `a.b`／`a.c`，不是頂層 `a`。排序後去重。

**S2.** `oo commit` 先印每一葉（`#divergent at c`，與 `format_conflict_where` 同形），再印 `Commit successful`。rc=0，HEAD 寫入。G2 綠。不拒絕。

**S3.** `CommitMeta.reported_bottoms: Option<Vec<(String, String)>>`。`None` 不進手寫 Debug、不進 serde、不進 encode。G1 綠：`x: 0` 根仍 `31745ef0…`、3 物件。`pin_commit_meta_debug_omits_absent_fields` 仍是三個普通欄位。

**S4.** 標記只在 `reported` 非空時為 `Some`，內容就是剛剛要印的那份清單。斷言見 Q1。

**S5.** 無 grant、無旗標。B 路徑未動（G3 綠）。

### N.2 順手改動（逐項指名）

*   `Universe::commit` 回傳值多了第三元（報告清單）。呼叫點：`main.rs` `run_commit`、`stage2_open_term_test.rs`。
*   既有 `CommitMeta { … }` 字面補 `reported_bottoms: None`：`refine_test.rs`、`authority_test.rs`、`universe_determinism_probe_test.rs`、`main.rs` squash／refine。否則編不過。Debug 釘 `pin_commit_meta_debug_omits_absent_fields` **沒有**改斷言（None 仍不出現）。
*   rustfmt 只跑本弧動過的檔。探針只拿掉兩行 `#[ignore]`，未 rustfmt。

### N.3 工單哪裡是錯的

沒有射程寫錯。Q4 量出來的跨版本行為見 N.4——工單要我們量，沒有預言「舊引擎會丟掉欄位繼續走」。帶標記的 commit **進入 Debug 故進入 digest**，v0.41.0 在那顆物件上報 `#caid_mismatch`。乾淨 commit 舊引擎照常讀。這是 D57「記在 ● 上且進 digest」的直接後果，不是實作選錯。

### N.4 工單指名要你回答的問題

**Q1.** 標記逐字斷言：**「下列座標上的 ⊥ 已向操作者印出，且這次提交繼續進行。」**
它不多說，因為一次性 CLI 問不到同意（工單 §5 沒探針的那一件／§6.2）。清單就是印出去的那一份，不是「操作者同意把矛盾寫進歷史」。

**Q2.** **全部葉。** §4.1.2「哪些座標」、§2.2.1 到葉。〔量〕`x: 1 & 2` 與 `y: 3 & 4` 同一次會印兩行。不是第一個。

**Q3.** **`project_for_commit` 之後、`put_commit` 之前。** `c: c + 1` 的 ⊥ 在投影時才出現；收集完寫進 `meta.reported_bottoms`，然後 `put_commit` 用含該欄的 Debug 算 digest。若放在 `put_commit` 之後，digest 已定，標記進不去。CLI 的印出在 `commit()` 回傳之後（磁碟已落地），同一條指令、rc=0。

可重跑：

```
c: c + 1
oo evolve
oo commit -m x
# 第一行是 `#divergent at c`，然後 `Commit successful`
# 物件 meta 含 `reported_bottoms: ["c #divergent"]`
```

**Q4.** 〔量，v0.41.0 標籤二進位讀本弧引擎寫的倉〕

帶 ⊥ 的 commit（物件含 `reported_bottoms:`）：

```
oo log     RC=1
Error: #caid_mismatch: object at digest path is corrupt (integrity failure);
requested hash:sha256:v1:c8d95968…, recomputed hash:sha256:v1:6ec2dd11…
oo status  RC=1  （同一則 #caid_mismatch）
oo inspect <HEAD>  RC=1
Error: #object_undecodable: … store document is a commit, not a value
```

**不是丟掉欄位繼續走，也不是拒絕開倉（format 仍認）。** 舊引擎的 `content_hash` 不算新欄位，路徑上的 digest 對不上。

本弧引擎寫的**乾淨** commit，同一支 v0.41.0：`log`／`inspect`／`status` 皆 rc=0，訊息與根都在。

**Q5.** 偵察 Q7 甲報價走訪 40–60 ＋ CLI 15–30 ⟹ ~50–90。產品路徑（`value.rs` 收集＋`CommitMeta`／Debug、`store_codec` 編解碼、`universe.rs`、`main.rs` 印出）大約 **+140／−10** 量級，比報價多出 S3 的框與編解碼（報價沒單獨列）。測試字面補 `None` 另計，不是產品行為。

### N.5 探針

只拿掉 R1／R2 的 `#[ignore]`。未 rustfmt。**5／5**。

### N.6 數字

`cargo test --workspace --no-fail-fast -- --test-threads=1` **exit 0**。
`test result:` 聚合：222 target 皆 ok；**2132 passed／0 failed／0 ignored**。
失敗測試名：**無**。
`^error`：**0**。
conformance：`python3 nlang-spec/scripts/run-conformance.py --engine target/release/oo` → **162／162**。
身分：G1 綠（`x: 0` 根 `31745ef0…`、3 物件）。known-answer `3`，對照 `_|_`。
探針只動 `#[ignore]`。

### N.7 你認為需要改規格之處

沒有本弧必須改的條文。跨版本：帶標記的 ● 對 v0.41.0 是 `#caid_mismatch`，這是「審計欄進入 digest」的後果；若規格要舊引擎仍能核對那顆物件，就得把標記移出 `content_hash`——那會讓 D57 要分辨的兩種紀錄又變成同一個 CAID。不在這裡請裁。

---

## A. 驗收回合 1（驗收方，2026-08-31）

**射程做對了，五題答得誠實，而 Q4 量到的東西需要一則裁定。**

### A.1 通過的

*   **探針 5／5**；純度乾淨——**只拿掉兩行 `#[ignore]`，其餘一字未動、未 rustfmt**。
*   〔複驗〕`c: c + 1` → **`#divergent at c`** 然後 `Commit successful`，**rc=0**、HEAD 寫入。
*   〔複驗〕**乾淨的 commit 對 v0.41.0 完全正常**：`log` rc=0 兩列、`status` rc=0。
    **G1 的成本論據成立**：`x: 0` 根仍 `31745ef0…`、3 物件。
*   **Q1 的自陳是對的形**：「下列座標上的 ⊥ **已向操作者印出**，且這次提交繼續進行。」
    **它沒有宣稱同意**——那正是 §6.2「標記必須反映事實而非旗標」要的節制，
    也正是工單 §5「沒有探針的那一件」所指。
*   **Q3 的順序推理正確**：報告與標記在 `project_for_commit` 之後、`put_commit` 之前產生，
    否則 digest 已定、標記進不去。
*   **Q4 沒有用推論代替量測**——工單指名要量，交付方量了，而且量到的是壞消息，照實寫。

### A.2 Q4 複驗：不是降級，是**完全鎖死**

〔量〕一個倉，新引擎提交一顆 `c: c + 1`（帶標記）。v0.41.0 在那個倉裡：

| 指令 | rc |
| :-- | --: |
| `oo log` | **1** |
| `oo status` | **1** |
| `oo gc --grant gc` | **1** |
| `oo evolve` | **1** |
| `oo commit` | **1** |

**一件都做不了。** 對照組（只有乾淨 commit 的倉）**全部 rc=0**。

⟹ **用新引擎提交一次 `c: c + 1`，就讓 v0.41.0 永久打不開那個倉。**
這正是 **條目 #18／Q-038** 的形狀（「一次成功回報的 `commit`，可以讓一個舊倉打不開」），
而那一次是當成嚴重缺陷開了一整弧的。

**⚠ 這一格有驗收方的份。** 裁 D57 時我寫「只有帶 ⊥ 的 commit 會有新 digest」並把它報成便宜，
**我為 CAID 的移動定了價，卻沒有為讀相容的後果定價**。交付方的 N.7 判斷正確：
**這是 D57「審計進 digest」的直接後果，不是實作選錯。**

### A.3 而它說的那句話是假的——**而正確的說法引擎已經有了**

v0.41.0 逐字說：

```
Error: #caid_mismatch: object at digest path is corrupt (integrity failure)
```

**那個物件並沒有損壞。** 它是被一個更新的引擎寫的。
引擎回報的是**它自己觀測到的內部事實**（重算的 digest 對不上），而不是**對操作者有意義的事實**。

**⚠ 而同一支二進位已經有一條誠實的路徑**〔量，只改宣告不動任何物件〕：

```
$ sed -i 's/layout=2/layout=3/' .oo/format
$ oo log
Error: store layout declaration "layout=3" is not supported; refusing to open
```

**rc 一樣是 1，但這一句是真的。**

⟹ **本弧多出一個選項，而它是一則裁定不是實作題**：
**要不要把 `.oo/format` 由 `layout=2` 升到 `layout=3`。**

| | 不升（今天） | 升 `layout=3` |
| :-- | :-- | :-- |
| 舊引擎對**乾淨**的新倉 | **正常**（rc=0） | **拒絕**（rc=1，但訊息為真） |
| 舊引擎對**帶 ⊥ 標記**的倉 | **鎖死，且說「corrupt」——假的** | **拒絕，訊息為真** |
| 鎖死的範圍 | **窄**（只有帶 ⊥ 的倉） | **寬**（所有新倉） |
| 說的話 | **假** | **真** |

**⚠ 這一題與本弧的主題是同一件事。** 本弧整個在做的，是不讓引擎對操作者說一句
「成功」而其實寫進了矛盾。**而它自己對另一個引擎說「你的儲存損壞了」，其實是「我比你新」。**
**同一個誠實性軸，換一個對話者。**

〔附帶〕Inbox 早有一列記著「`.oo/format` 仍是 `layout=2`，**Q-013 新增目錄時沒有升版**」
⟹ **這個版號欠帳不是本弧造的，但本弧是第一次讓它產生可見後果的地方。**

### A.3a 全跑 ×3：兩次全綠，第三次紅在**另一支探針自己的 REACH 守衛**上

〔量，release，`--no-fail-fast --test-threads=1`，**保留失敗測試名**〕

| 跑 | 結果 |
| :-- | :-- |
| 1 | exit 0／**222 target／2132 passed／0 failed／`^error` 0** |
| 2 | exit 101／2131 passed／**failed=1** ⟹ `r5_the_rebuilt_index_matches_an_insertion_replay` |
| 3 | exit 0／**222／2132／0／0** |

**那支紅與本弧無關，而且不是缺陷。** 逐字訊息：

> **no bucket overflowed with 60 peers**, so this probe cannot tell a table
> rebuilt with the right self id from one rebuilt with zeros

⟹ **它倒在自己的 REACH 守衛上，不是倒在斷言上。**
〔讀 `advert_persistence_probe_test.rs:834`–`:862`〕它廣告 60 個對等節點，
需要**至少一個 Kademlia 桶溢出**，斷言才有意義；而節點自身的 id 每次重新鑄，
桶的分佈因此逐次不同，偶爾沒有任何一個桶溢出。
**那時它拒絕當一個空洞的綠**——**那是對的行為**，與本專案「全綠不等於被檢驗過」同一條紀律。

〔複驗〕單獨跑該支 **8／8 全綠**；本弧的 diff **沒有碰** peers／routing／advert 任何一行。

**⟹ 但它讓全跑不是決定性的**，而依「一支五次只紅四次的紅不是釘子」的反面，
**一次全綠也不能單獨當結論** ⟹ **已開 Inbox 一列**（見 `WORK_QUEUE`）。
**本次的判定用的是三次的名字比對，不是三次的計數。**

**⟹ 而這正是本弧收到的那條常設規則第一次付款**：
若聚合只留計數，這一次會看到「2 綠 1 紅」而**分不出是交付的缺陷還是別人的探針**。

### A.4 待裁

1.  **`layout` 升不升？**（見上表；驗收方傾向**升**，理由是誠實性軸與本弧主題同源，
    且窄而說謊的鎖死比寬而誠實的鎖死更難查。**但這是使用者面的取捨，不由驗收方定。**）
2.  **90 天穩定時鐘要不要重啟？** **本條與 #24 的關鍵差別是「舊引擎有沒有被鎖在門外」**
    ——#24 明文以「**並未被鎖在門外**（仍 inspect、仍 evolve、仍 commit）」為不重啟的理由，
    **而本條逐字相反**。驗收方傾向**重啟**。

**在這兩則裁定之前不切版。** 產品程式碼本身驗收通過，不需要修補回合。

---

## B. 修補回合 1 的射程（驗收方，2026-08-31；用戶已裁「按建議」）

**用戶裁定**：**(1) `layout` 升；(2) 90 天時鐘重啟。**
時鐘那一件是驗收方切版時寫 `CHANGELOG` 的事，**與你無關**。射程只有下面兩項。

### B.1 ⚠ 開單前驗收方先量到的：**單獨升版本號會把每一個既有的倉擱淺**

〔量 2026-08-31，本弧建置〕

| `.oo/format` | `oo migrate --grant migrate` |
| :-- | :-- |
| `layout=2`（現行） | **rc=0** |
| `layout=99`（不被認得的 `layout=N` 形） | **rc=1**：`store layout declaration "layout=99" is not supported; refusing to open` |
| 裸 `1`（legacy 混合形） | **rc=0** |

〔讀 `storage.rs:547`–`:556`〕`migrate_layout` 的**第一件事**是 `Self::ensure_format(base_dir)?`，
而 `ensure_format`（`:189`–`:198`）只認三種：**恰為現行 `layout={N}`** ／ **裸數字（legacy）** ／ 其餘一律 `bail`。

⟹ **把 `STORE_LAYOUT_VERSION` 改成 3 之後，`layout=2` 落進第三支**
⟹ **新引擎不開它，而 `oo migrate` 也拒絕它** ⟹ **既有的倉沒有任何一條出路。**

**⟹ 所以本回合是兩件，不是一件。只改常數是錯的。**

### B.2 射程

**S6.** `STORE_LAYOUT_VERSION` **2 → 3**。

**S7.** **`migrate` 必須能從「本引擎曾經寫過的上一個 layout」前進。**
今天的 `ensure_format` 只有「**現行**」與「legacy 裸數字」兩種認得的形；
需要的是一個**可遷移來源**的概念，與「可直接操作的 layout」分開。
`layout=99`（來自**未來**的宣告）**仍必須被拒絕**——要放行的是**過去**，不是任何非現行值。

**S8.** `run_migrate` 的訊息今天逐字說「The creating engine of a **pre-sentinel** repo will no longer
open this store」。升版後那句話**指錯了對象**（現在被擋在門外的是 `v0.41.0`）。**改成準確的。**

### B.3 明確不做

*   **不動 `reported_bottoms` 那一路**——驗收回合 1 已通過，探針 5／5。
*   不碰 `encoding`（`objects.format` 仍 `encoding=5`）。
*   不動身分：`x: 0` 根 `31745ef0…`／3 物件／標準根 `7038e250…`。
*   不碰 Inbox 那列「advert 探針的 REACH 守衛隨機開火」（**不是本弧造的，也不要順手改**）。

### B.4 你必須回答的

**Q6.** S7 的「可遷移來源」你怎麼表達？**逐字說它為什麼擋得住來自未來的宣告**
（`layout=4`、`layout=99` 必須仍是 rc=1）。

**Q7.** 一個 `v0.41.0` 造的倉（真二進位），在你的引擎下：
(i) 直接開 → 逐字輸出與 rc；(ii) `oo migrate --grant migrate` → 逐字輸出與 rc；
(iii) 遷移後 `oo log`／`oo status`／`oo commit` → 逐字輸出與 rc；
(iv) 遷移後**再用 `v0.41.0` 開它** → 逐字輸出與 rc（**應為那句誠實的拒絕，不是 `#caid_mismatch`**）。
**四步都要逐字。**

**Q8.** 遷移**不得**動 HEAD、不得重寫任何根（O73 逐字）。**怎麼證明？** 帶指令。

### B.5 探針

**⚠ 逐字記下：S6／S7 這一格在 `cargo test` 裡紅不起來。**
理由與 Q-015 那次同型——**測試裡的二進位就是新的**，它寫出來的倉一律是新 layout，
**造不出「上一個 layout 的倉」**（手寫 `layout=2` 進檔案只是把常數抄一遍，
釘住的是那個字串不是那個情境）。
⟹ **由驗收方在驗收時用真的 `v0.41.0` 標籤二進位造倉、你的引擎遷移**，即 Q7 的四步。
**驗收方上一弧在這一格寫過一支假探針並因此逼出一個錯的實作；這次不重蹈。**

既有五支（R1／R2／G1／G2／G3）**必須保持現狀**——可以動的仍然只有 `#[ignore]`，而它們已經沒有了。

---

## R. 交付回報（修補回合 1；交付方填。本行以上一字不得動）

### R.1 射程逐項對照

**S6.** `STORE_LAYOUT_VERSION` **2 → 3**。新倉寫 `layout=3`。`encoding` 仍是 5。

**S7.** 可遷移來源是閉集 `STORE_LAYOUT_MIGRATABLE_FROM = [2]`，與現行常數分開。`ensure_format`／`declared_encoding` 認 `layout=N` 當且僅當 N 是現行或在該閉集；其餘（含未來）走原來那句 `not supported; refusing to open`。`migrate` 仍先走 `ensure_format`，所以同一道閘同時擋 `log` 與 `migrate`。legacy 裸數字未動。

**S8.** `run_migrate` 現在印：`Migrated store layout to layout=3. An engine that only reads layout=2 (oo v0.41.0) will no longer open this store.` 不再提 pre-sentinel。

### R.2 順手改動（逐項指名）

*   `atomic_write` `p2` 的 layout 釘從 `layout=2` 改成 `layout=3`，並在註解裡記下本回合的裁定（該針自己寫著「Whoever moves it next updates this line and says why in the work order」）。
*   Q-038 探針兩處把「現行 layout」釘成字面 `layout=2`：`r5` 的遷移目的地、`g1` 的 REACH。改成 `layout=3`。**未**改 `g4` 手寫的 `layout=2`（那是 encoding=3 的舊倉，必須仍打得開）。
*   未 rustfmt `storage.rs`（整檔會重排不相干函數）。`main.rs` 只動訊息那兩行。
*   **Q-017 探針一字未動。**

### R.3 工單指名要你回答的問題

**Q6.** 可遷移來源是閉集 `STORE_LAYOUT_MIGRATABLE_FROM`，目前只有 `2`。不是 `N < current`：`layout=1` 從未是一種形（那一世代是裸數字），而且現行若再升到 5，`layout=4` 不得因為「比較小」而混進來。未來的宣告（`layout=4`、`layout=99`）不在閉集、也不是現行，故 `log` 與 `migrate` 都是 rc=1，檔案不動。

〔量〕

```
layout=4   oo log                  RC=1
Error: store layout declaration "layout=4" is not supported; refusing to open
           oo migrate --grant migrate  RC=1  （同一句；format 仍是 layout=4）
layout=99  同上兩支，RC=1，format 仍是 layout=99
```

**Q7.** 〔量，真 `v0.41.0` 標籤二進位造倉；本弧 `target/debug/oo` 開它〕倉由 `x: 0` 一次 commit 做成，`.oo/format` 為 `layout=2`。

**(i) 直接開**

```
oo log     RC=0
commit hash:sha256:v1:4666c780e9057108f582c32cede23e5f9118a10efa3ceebb9bb9a1b2cc827856
    message: one
    Date: 2026-09-02T13:45:57.066Z
oo status  RC=0
Standard root dependency: 7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911 (available)
Universe is static (no staged changes).
```

layout=2 仍可直接操作（它是過去，不是未來）。遷移是明示推進宣告，不是開啟的前提。

**(ii) `oo migrate --grant migrate`**

```
RC=0
Migrated store layout to layout=3. An engine that only reads layout=2 (oo v0.41.0) will no longer open this store.
```

`.oo/format` → `layout=3`。`objects.format` 仍 `encoding=5`。

**(iii) 遷移後**

```
oo log     RC=0   （同一顆 4666c780…，message: one）
oo status  RC=0   （同一句 standard root / static）
oo commit  RC=0
Commit successful: hash:sha256:v1:fe3c14eb883559ab402d5f4febf045c24424f24131b48bfc16bfd5a1726eec20
```

（commit 前先 `evolve` 了 `y: 1`，否則無東西可提交。）

**(iv) 遷移後再用 v0.41.0 開**

```
oo log     RC=1
Error: store layout declaration "layout=3" is not supported; refusing to open
oo status  RC=1
Error: store layout declaration "layout=3" is not supported; refusing to open
```

**是那句誠實的拒絕，不是 `#caid_mismatch`。**

新引擎自己造的倉一開始就是 `layout=3`；同一支 v0.41.0 對它也是這句，不必先提交 ⊥。

**Q8.** 遷移前對 `.oo/HEAD`、`.oo/objects.format`、以及 `objects/` 下每一個檔做 `sha256sum`；遷移後 `cmp`。

```
cd <v0.41.0 所造的倉>
cp .oo/HEAD /tmp/head.before
( cd .oo && find objects -type f -print0 | sort -z | xargs -0 sha256sum ) > /tmp/obj.before
oo migrate --grant migrate
cmp .oo/HEAD /tmp/head.before          # identical, rc=0
cmp objects.format 遷移前後            # identical, rc=0
cmp /tmp/obj.before 遷移後的清單雜湊   # identical, rc=0
cmp 遷移前的 format 與 .oo/format      # differ: byte 8, line 1（2 → 3）
```

HEAD 仍是 `hash:sha256:v1:4666c780…`。三個物件路徑與內容未動。只動了容器宣告。

### R.4 探針

Q-017 五支**一字未動**。**5／5**。S6／S7 依工單不在 `cargo test` 裡釘——Q7 用真二進位量。

### R.5 數字

`cargo test --workspace --no-fail-fast -- --test-threads=1` **exit 0**。
`test result:` 聚合：222 target 皆 ok；**2132 passed／0 failed／0 ignored**。
失敗測試名：**無**。
`^error`：**0**。
conformance：`python3 nlang-spec/scripts/run-conformance.py --engine nlang-tools/target/release/oo` → **162／162**。
身分：`x: 0` 根仍 `31745ef0…`、3 物件、標準根 `7038e250…`。known-answer `3`，對照 `_|_`（`#conflict`）。
Q-017 探針未動。

---

## C. 驗收回合 2（驗收方，2026-08-31）

**S6／S7／S8 都做對了。而漏掉的那一格是驗收方射程沒寫好。**

### C.1 通過的（逐項複驗）

*   **探針 5／5**；`a_success_that_was_a_bottom_probe_test.rs` **零改動**。
*   **Q6 的形是對的，而且比要求的更嚴**：`STORE_LAYOUT_MIGRATABLE_FROM: &[u32] = &[2]`
    ——**封閉清單，不是範圍**，註解逐字寫著「**The past is a closed list, not "any value
    other than current"**」，並自己指出「一旦 current 變成 5，未來的 `layout=4` 不得溜過去」。
    〔複驗〕`layout=4` → **rc=1**、`layout=99` → **rc=1**。
*   **Q7 四步全過**〔量，真 `v0.41.0` 標籤二進位造倉〕：
    (i) 新引擎直接開 **rc=0**；(ii) `migrate` **rc=0**，宣告 `layout=2` → `layout=3`；
    (iii) 遷移後 `log`／`commit` 皆 **rc=0**；
    (iv) **`v0.41.0` 再開它 → `Error: store layout declaration "layout=3" is not supported;
    refusing to open`，rc=1** ⟹ **那句話是真的，`#caid_mismatch` 不再出現。**
*   **S8 的訊息改對了**：「An engine that only reads layout=2 (**oo v0.41.0**) will no longer open
    this store」——指名了正確的對象。

### C.2 要修的：**沒有遷移的舊倉，仍然會被寫成那個說謊的鎖死**

〔量〕`v0.41.0` 造的倉（宣告 `layout=2`），**不遷移**，新引擎直接提交一顆 `c: c + 1`：

```
倉的宣告        layout=2
新引擎 commit   rc=0    #divergent at c        ← 帶標記的 commit 寫進去了
提交後宣告      layout=2                        ← 沒有變
v0.41.0 讀它    rc=1    #caid_mismatch: object at digest path is corrupt
```

⟹ **升版擋住的只有「新引擎自己造的倉」與「已遷移的倉」。
最常見的那條路——既有的倉、新引擎、照常使用——仍然產生那個假訊息。**

**成因是一句可以寫成不變式的話**：
**一個仍宣告舊 layout 的儲存，被寫進了那個 layout 表達不了的東西。**
宣告是一個關於裡面有什麼的承諾，而寫標記把承諾破壞了卻沒有更新它。

### C.3 這是驗收方的射程沒寫好，而且是同一種寫法連續第四次

**S7 逐字寫的是機制**：「`migrate` **必須能從上一個 layout 前進**」。
**交付方逐字做到了那句話。** 而**該被守住的不變式**是另一句：
**「宣告舊 layout 的儲存，不得持有舊引擎驗證不了的東西。」**

⟹ **可重用的教訓（與 D55 那條同族）**：

> **射程要寫不變式，不要寫機制。**
> 寫機制，交付方會把機制做得很對，而不變式從旁邊漏掉。

（前三次：Q-015 的 S2「恰有一個父」漏掉兩個關係／S5 的讀者表漏掉 `gc::mark`／
A2.6 的「祖先邊指向提交 ○」漏掉「本弧之前的 commit 沒有 ○」。**四次都是驗收方寫的射程。**）

### C.4 修補回合 2 的射程：一件

**S9.** **一個宣告非現行 layout 的儲存，不得接受一次會寫入標記的提交。**

驗收方建議**窄的那一種**（兩個候選，交付方若認為寬的較好，帶理由）：

*   **甲（建議）**：**只擋帶標記的提交**。偵測到這次提交要寫 `reported_bottoms`
    而 `.oo/format` 非現行 ⟹ **拒絕該次提交**，訊息要**可行動**
    （逐字指出跑 `oo migrate --grant migrate`）。
    **乾淨的提交照常**，於是**沒有踩到 ⊥ 的人完全不受影響**，
    而 `v0.41.0` 對那些倉繼續正常工作。
*   **乙**：**擋掉對非現行 layout 的所有寫入**（唯讀直到遷移）。更好預測，但**強迫所有人遷移**。

**⚠ 這不違反 B1。** B1 治的是「因為有 ⊥ 所以拒絕提交」；**這裡拒絕的理由是儲存的版本，
不是值的內容**——它是一個格式守衛，而且**帶著一條操作者做得到的指示**。
**探針 G2（帶 ⊥ 的提交仍須落地）必須在現行 layout 的倉上保持綠**，那是它的射程。

**Q9.** 你採甲還是乙、為什麼；並逐字給「舊倉 ＋ 未遷移 ＋ `c: c + 1`」的新輸出與 rc。

### C.5 本回合不動

`reported_bottoms` 那一路（回合 1 已過）／`encoding`／身分／Inbox 那列 advert 探針。

---

## R2. 交付回報（修補回合 2；交付方填。本行以上一字不得動）

### R2.1 採甲，為什麼

**甲。** 不變式是「宣告舊 layout 的儲存，不得持有舊引擎驗證不了的東西」。舊引擎驗證不了的是**帶 `reported_bottoms` 的 commit digest**，不是每一次寫入。乙把所有寫入都擋掉，等於強迫每一個既有倉遷移，然後 v0.41.0 對它們全部誠實拒絕——那是修補 1 表上「寬而誠實」的鎖，用戶只把它裁給**新倉**與**已遷移的倉**。乾淨提交的位元組 v0.41.0 本來就讀得懂（修補 1 Q7／Q8）。

不是 B1：拒絕的理由是容器版本。現行 `layout=3` 上帶 ⊥ 的提交仍落地（G2 綠）。

閘在 `project_for_commit` 收集完葉之後、`put_root`／`put_commit` 之前。沒有寫入就沒有標記。

### R2.2 Q9〔量，真 v0.41.0 造倉，未遷移〕

`x: 0` 一次 commit，`.oo/format` = `layout=2`。然後本弧引擎：

```
printf 'c: c + 1\n' > bot.n
oo evolve bot.n
oo commit -m bot
```

```
RC=1
Error: this store declares layout=2; a commit that reports a bottom cannot land until the layout is current. Run `oo migrate --grant migrate`
```

HEAD 未動（仍是 v0.41.0 那顆 `6777e801…`），宣告仍 `layout=2`。同一支 v0.41.0 `oo log` **rc=0**，不是 `#caid_mismatch`。

對照（另一個未遷移的 v0.41.0 倉，工作集沒有 ⊥）：`y: 1` → `Commit successful` **rc=0**，宣告仍 `layout=2`，v0.41.0 `log`／`status` **rc=0**。

遷移之後同一句 `c: c + 1` → `#divergent at c` 然後 `Commit successful`，**rc=0**（G2 的倉）。

### R2.3 順手改動

沒有。Q-017 探針一字未動。未 rustfmt `storage.rs`／`universe.rs`。

### R2.4 探針

**5／5**。G2 在現行 layout 的倉上綠。

### R2.5 數字

`cargo test --workspace --no-fail-fast -- --test-threads=1` **exit 0**。
`test result:` 聚合：222 target 皆 ok；**2132 passed／0 failed／0 ignored**。
失敗測試名：**無**。
`^error`：**0**。
conformance：**162／162**。
身分：G1 綠（`x: 0` 根 `31745ef0…`、3 物件）。known-answer `3`，對照 `_|_`。
Q-017 探針未動。

---

## D. 驗收回合 3（驗收方，2026-08-31）

**通過。** 甲是對的選擇，而它的理由書寫得比工單清楚：
**舊引擎驗證不了的是「帶 `reported_bottoms` 的 commit digest」，不是每一次寫入**
——所以要擋的是那一種提交，不是那個倉。

### D.1 三個情形逐一複驗

| | 情形 | 結果 |
| :-- | :-- | :-- |
| **A** | 舊倉（`layout=2`）未遷移 ＋ `c: c + 1` | **rc=1**，逐字：`this store declares layout=2; a commit that reports a bottom cannot land until the layout is current. Run `oo migrate --grant migrate`` ／ **HEAD 未動** ／ 宣告仍 `layout=2` ／ **`v0.41.0` log rc=0** |
| **B** | 同樣的舊倉，**乾淨**的提交 | **rc=0 落地**，宣告不變，**`v0.41.0` log／status 皆 rc=0（2 列）** |
| **C** | 現行 `layout=3` 的倉 ＋ `c: c + 1` | **rc=0 落地**，印 `#divergent at c` ⟹ **G2 的射程完好** |

### D.2 訊息承諾的復原路徑是真的（驗收方加測，工單沒要求）

那條訊息叫操作者去 `oo migrate`。**所以它必須真的救得回來**：

```
被拒之後   staged 仍有 2 筆（c: c + 1 與 y: 7），HEAD 未動
oo migrate --grant migrate      rc=0   layout=2 → layout=3
oo commit -m retry              rc=0   #divergent at c ＋ Commit successful
之後       staged 0 筆
```

⟹ **拒絕是可回復的，不是破壞性的；一個字都沒有掉。**
**一條叫人去做某件事的錯誤訊息，若那件事做完不管用，它就是另一種說謊**——這一格量過了。

### D.3 ⚠ 驗收方差一步把一個正確的拒絕讀成缺陷

第一次量 B 時得到 **rc=1**，看起來就是「連乾淨的提交都被擋了」。
**那是量測錯了**：前一步被拒的提交把 `c: c + 1` 留在工作集裡（`injections` 2 筆、
`status` 逐字印出兩個欄位），所以那次提交**確實仍帶著 ⊥**，拒絕是對的。
換一個**全新的舊倉**重測才碰到真正要測的那一格。

⟹ 又一次「**先證明你的量測真的碰到了你以為它碰到的東西**」。
本弧這條規則已經救了兩次（另一次是全跑的失敗名字）。

### D.4 其餘

*   **探針 5／5**；弧探針檔**零改動**。
*   **身分紅線**：`x: 0` 物件 **3**、根 `31745ef0…`、標準根 `7038e250…`。
*   **conformance 162／162**。
*   閘的位置對：`project_for_commit` 收集完葉之後、`put_root`／`put_commit` 之前
    ⟹ **沒有寫入就沒有標記**，HEAD 不動是由構造成立的。
