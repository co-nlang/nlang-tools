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
