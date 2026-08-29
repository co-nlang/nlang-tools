# Q-014（W10）偵察交接 — 一個可以被原子交換的物件

**基線**：引擎 `v0.38.0`（標籤）／規格 `v0.38.0-draft.1`。
**分支**：`dev`。**這是偵察，不是實作** — 本文不要求你改任何行為。
**產出**：`docs/an_object_you_can_swap_recon.md` ＋ 一筆 `dev` 提交。

---

## 0. 這一弧是什麼

`.oo/` 今天沒有任何一個「狀態」是可以被一次原子替換的。`save_staged` 一次寫**五個檔**，
每個各自原子（temp＋rename），**合起來不原子**。W10 要的就是把它們變成**一個**物件，
好讓 Q-016 有東西可以做 compare-and-swap。

> **⚠ 縮寫**：本弧與 Q-016 文件裡的 `CAS` 是 **compare-and-swap**。
> `GLOSSARY` §11.1（2026-08-29 裁定）規定 `CAS` **只**指 Content-Addressed Storage，
> compare-and-swap **一律寫全稱**。本文以下一律寫全稱；讀舊文件時請注意這一格。

**裁定依賴**（全部已裁，本弧不需要新裁定就能偵察）：

| | 裁定 |
| :-- | :-- |
| **D26** | **lock-free compare-and-swap ＋ meet 重試**才是 `SPEC_10` §4.1 要的；鎖是「拒絕／阻塞」語義 |
| **D43** | 每個 ○ 都已經是持久的 ⟹ savepoint 必須活過 commit |
| **D47** | ○ 的產生判準：注入 ⟹ 格上位置變；觀測 ⟹ 有 thunk 被真的化約 |

**⚠ 有一個設計岔路尚未裁，而它是本次偵察的主要目的**（見 §3）。

---

## 1. 驗收方已經量過的，不要重量

〔量 2026-08-29，`v0.38.0` 標籤二進位
`/home/gali/nlang-baselines/v0.38.0-verify-target/release/oo`；
known-answer 已過：`~%Math./add (1,2)` → `3`〕

**(a) 40 個並行 `oo evolve`（各加一個不同欄位），四輪：**

| trial | `staged` 存活欄位 | `savepoints/LOG` 記載 | 目錄檔數 |
| :-- | --: | --: | --: |
| 0 | 4／40 | 5 | **6（孤兒 1）** |
| 1 | 4／40 | 4 | 4 |
| 2 | **5**／40 | **6** | 6 |
| 3 | **4**／40 | **6** | 6 |

*   舊缺陷原封不動（`control_plane` §1.4.3 於 2026-08-05 量到 41 → 2，十六個 minor 前）。
*   **40 次位置移動只鑄出 4–6 個 ○** ⟹ D47 的注入條款在並行下不成立。
*   trial 2／3 的 **○ 比存活欄位還多** ⟹ ○ 鏈記載了 `staged` 沒有的血緣。
*   **孤兒只出現一次，後三輪未再現** ⟹ **是競爭窗不是必然**。你的報告若寫成穩定重現，
    那是錯的。

**(b)〔讀〕`save_staged`（`universe.rs:817`–`859`）依序寫五個檔**，
每個各自 `atomic_write`，**沒有任何一步把它們綁在一起**：

| 序 | 檔 | 寫法 |
| --: | :-- | :-- |
| 0 | CAS 物件（`persist_blur_partials`） | 進 `objects/` |
| 1 | `.oo/staged` | `atomic_write` |
| 2 | `.oo/savepoints/<local-id>` | `atomic_write`（`record`） |
| 3 | `.oo/savepoints/LOG` | `atomic_write`（`record`，**整檔重寫**） |
| 4 | `.oo/pin_pending` | `atomic_write` 或 `remove_file` |
| 5 | `.oo/effect_pending` | `atomic_write` 或 `remove_file` |

⟹ **孤兒不只是並行競爭，也是崩潰窗**：死在 2 與 3 之間就得到一個 LOG 不認識的 body。

**(c)〔讀〕`savepoint::record`（`savepoint.rs:52`）是讀-改-寫，
且 `mint_id` 由讀到的計數決定 id**（`format!("{n:016x}")`，`n = ids.len()+1`）
⟹ **身分衍生自一次可能過期的讀**。兩個行程讀到同一個 N 就鑄出同一個檔名。

**(d) ⟹ 最重要的一件：○ 層今天是唯寫的。**
〔量〕`savepoint::load` 標著 `#[allow(dead_code)]`；`recorded_ids` 的唯一呼叫者是
`record` 自己；`crates/oo/src/` 對 `savepoint` **零命中** ⟹ **CLI 沒有任何 savepoint 表面**。
**沒有讀者 ⟹ 沒有相容性約束 ⟹ 這個格式現在可以自由改一次，以後不行。**

**(e) 暫存態與已提交態的版圖不同，而只有後者被釘住：**

```
暫存（未提交）  .oo/format  .oo/objects.format  .oo/staged
                .oo/savepoints/LOG  .oo/savepoints/<local-id>
已提交          .oo/HEAD  .oo/format  .oo/objects.format
                .oo/objects/sha256/<CAS>  .oo/savepoints/LOG  .oo/savepoints/<local-id>
```

`p1_the_layout_is_a_short_and_known_list` 建的是 **committed** 倉 ⟹
`.oo/staged`、`.oo/pin_pending`、`.oo/effect_pending` **不在任何一份宣告清單裡**。
**〔量〕commit 後 ○ 存活**（D43 ✓）。

---

## 2. 要你去量的（逐題答，附 `檔案:行號` 與可重跑指令）

**Q1 — 五個檔的失敗語義。** 第 1–5 步任何一步失敗（磁碟滿、權限、行程被殺），
留下什麼狀態？有沒有任何回滾或修復路徑？`oo` 下次啟動時會不會發現不一致？

**Q2 — 還有誰在寫 `.oo/`。** 逐一列出**不經過 `save_staged`** 的 `.oo/` 寫入路徑
（commit、GC、`init`、adopt、advert、任何遷移碼），帶 `檔案:行號`，並註明各自寫哪些檔。
**這一題決定「一個物件」的邊界畫在哪。**

**Q3 — 兩種編碼是不是同一個。** `encode_staged` 與 `encode_savepoint`
（`store_codec.rs`）差在哪？逐位元組舉一個例子。**若要合成一個物件，兩者能不能共用一個編碼？**

**Q4 — commit 路徑的順序與崩潰窗。** commit today 怎麼刪 `staged`？
寫 `HEAD` 與刪 `staged` 誰先？中間崩掉會怎樣？commit 動不動 `savepoints/`？

**Q5 — `pin_pending` ／ `effect_pending` 的讀者是誰**（帶行號），它們的值**進不進 CAID**？
W10 的原描述是「單一可替換 savepoint 物件（**hash 指標 ＋ 欄位含 pin/effect 意圖**）」
⟹ 這兩個檔本來就該是那個物件的欄位。**量一下把它們變成欄位會不會改變任何身分。**

**Q6 — 誰依賴現在的兩檔形。** 逐一列出今天讀 `.oo/savepoints/` 形狀的測試／探針
（含 §1(e) 那三支版圖釘），並說明合成一個檔之後各自會怎樣。

**Q7 — 合成之後誰會壞。** 若把第 1–5 步合成單一 `.oo/state`（一次 `atomic_write`），
點名會壞的地方：GC 走訪、`oo inspect`、跨版本讀取（舊 `oo` 打開新倉、新 `oo` 打開舊倉）、
`.oo/format` 要不要升版。

**Q8 — 成本。** 今天一次 `oo evolve` 做幾次磁碟寫入（含 temp＋rename 的兩次）？
合成一個物件後是幾次？給實測數字，不要估算。

**Q9 — 這一題是設計岔路的定價**（見 §3）：**兩個候選各自要動幾行、動哪些檔？**
不要選，只要報價。

---

## 3. 未裁的設計岔路（本次偵察的主要目的）

`LOG` 與 body 今天可以互相矛盾。兩個候選：

*   **甲 ── 合成一個物件。** `staged` ＋ ○ body ＋ `LOG` ＋ `pin_pending` ＋ `effect_pending`
    塌成單一 `.oo/state`，一次 `atomic_write` ⟹ **compare-and-swap 的單位自然出現**，
    Q-016 直接有東西可做。代價：○ 的「鏈」要換一個表示法（物件內的前驅指標？）。
*   **乙 ── `LOG` 成為唯一真相，孤兒由 GC 回收。** 改動小，但**跨檔的不一致窗仍在**，
    只是被判為可容忍。Q-016 之後仍需要一個原子單位。

**驗收方傾向甲**，理由是 §1(d)：**沒有讀者，所以現在是這個格式最便宜的一刻**，
而每一個之後才長出來的讀者都讓它更貴——這正是 `STATUS` §335 W8′ 逐字預言的那筆債。
**但這是用戶的裁定，不是我的。** 你的工作是給兩邊報價，不是選邊。

---

## 4. 明確不做

*   **compare-and-swap 與重試**（Q-016）。本弧只給「一個可以被交換的物件」，**不做交換協定**。
*   **`staged` 的並發語義**（同 Q-016）。
*   **觀測邊界寫 ○** ── 那是 `SPEC_10` §3.1 的未兌現列，另案。
*   **CLI 的 savepoint 動詞**（Q-018／W22）。
*   **動身分**。

## 5. 紅線

*   `x: 0` 的根 **`31745ef0…`**、`.oo/objects` **3 個物件**。
*   標準根 **`7038e250…`**。
*   **本弧是偵察，上述紅線在偵察階段只需確認「你沒有動過」。**

## 6. 兩條協定規則，本弧照舊適用

*   **§8 開弧 4**：任何引用內建的讀數，**必須先證明那個呼叫發生了**。
    本弧多為儲存路徑，若你確實沒有引用任何內建，**在報告裡寫「不適用」，不要留白**。
*   **§8 收弧 3c**（2026-08-29 新設，起因就是這一格）：
    凡在 `.oo/` 新增一種耐久檔，既有的並行／損壞量測**必須對新檔重跑一次**。
    ⟹ 你若在偵察中發現還有別的耐久檔沒被那組量測掃過，**寫進報告**。
