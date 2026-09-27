# 工單：丟了的 context 不是空的 context（Q-061）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-061／裁定 `meta/oo/STATUS.md` **D79**（丙′）／設計筆記 `meta/oo/commit.md` §1.12（候選）
> 探針（已預先提交並校準）`crates/oo/tests/a_lost_context_is_not_an_empty_one_probe_test.rs`
> 基線：dev `c8a95ec`／`oo v0.62.0` ⟹ **4 綠 6 紅**，三輪一致、零空洞讀數。

## 1. 缺陷

`SPEC_08` §6.2.1 規定 `gc` 的根：`HEAD` 所指之 Commit、其祖先（遞移）、各 Commit 的 `root` 值樹；**放棄邊不是根（MUST NOT）**。
〔量，v0.61.0／v0.62.0〕兩筆提交的倉，把 `.oo/HEAD` 移走（○ 仍記著兩筆提交）：

| 指令 | 今天的答覆 | 後果 |
| :-- | :-- | :-- |
| `gc --grant gc` | rc=0、「0 reachable, 5 collectable」 | **刪光全部物件**；放回 `HEAD` 後 `log` ⟹ `CAID not found` |
| `commit` | rc=0、`Commit successful` | **長出一條新的創世鏈**；下一次 `gc` 從新 `HEAD` 起算，**合法地刪掉舊歷史**（實測舊 `HEAD` 物件消失） |
| `status` | rc=0、「no committed root yet」 | 說沒有歷史，而有 |
| `log` | rc=0、什麼都不印 | 同上 |
| `evolve` | rc=0 | 在一個不存在的點上提出 |
| `rollback <舊提交> --grant rollback` | rc=0，裝回 `HEAD`，歷史恢復 | **這是唯一的回頭路**——必須保留 |
| `squash` | rc=1、`no HEAD to squash` | 已是拒絕 |

〔讀 `gc.rs` `mark`〕`let Some(head) = store.get_head(..)? else { return Ok((seen, integrity)) }`——`HEAD` 缺席即空的可達集。
〔讀〕根的第 2 條（祖先）引擎**已經**沿 ○ 的 `ancestor:` 走（`previous_commit`）；規格文字仍寫已退役的 `parent`——**那是驗收方收尾的事，不在本工單**。

## 2. 裁定與依據

*   **D79（用戶 2026-09-27，丙′）**：`gc` 照 §6.2.1（放棄邊仍可回收）；**`HEAD` 讀不到或缺席，而 ○ 記著提交 ⟹ 具名拒絕**。
*   **依據**（`commit.md` §1.12，候選）：HEAD 是 context 的「點」那一層；**需要一個「現在」才答得出的指令**（`status`、`log`、`evolve`、`commit`、`gc`）
    在 context 丟了的時候**不得**把它當成空的——與 `REAL_03` §6.6「打不開的儲存不得換成空的」、D73「讀不到的節點金鑰 ⟹ 拒絕開啟」同一條線。
    **HEAD 讀不到**今天已經拒絕開啟（Q-060 矩陣）；本工單使**缺席而有證據**者與它一致。
*   **驗收方依 D79 的前提把射程從 `gc` 延伸到上表各指令**，理由是量測：只修 `gc`，`commit` 那條路照樣刪掉歷史。

## 3. 射程＝不變式（不是機制）

*   **I1** context 丟了（`HEAD` 缺席，而儲存自己宣告過提交存在——○ 的提交註記）⟹ 相對於 context 的指令**具名拒絕**，**不寫任何東西**、**不刪任何東西**（r1–r5）。
*   **I2** 拒絕**點名回頭路**：`rollback <commit> --grant rollback`（r6）。**回頭路本身必須在 `HEAD` 缺席時照常可用**（g3）。
*   **I3** **誠實的空 context 不受影響**：從未提交的工作區照常 `status`／`evolve`／`commit`（g1）。
*   **I4** **§6.2.1 不變**：放棄邊不是根，rollback 之後 `gc` 仍回收被放棄的內容（g2）；健康的 `gc` 不刪任何可達物（g4）。
*   **I5** 與 context 無關的指令不受影響：`inspect <CAID>`、`identity`、`fetch`／`serve`。
*   **I6** 值位址、提交位址、宣告一個都不動；讀取不寫入。

## 4. 紅線（今天綠，必須保持綠）

*   g1–g4；Q-060 23 支（含 `gc` 讀不到根即拒絕的 r15／r16）、Q-059 16＋1、Q-058 9、Q-057 13、Q-055 17；`privileged_effect`／`history_ops` 等既有 `gc`／`rollback`／`squash` 探針。
*   conformance 162／162；`x: 0` 根 `31745ef0…`、`v: 1 + 1` 根 `f4f32e7b…`、標準根 `7038e250…`；`layout=7`／`encoding=5`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** 你怎麼判「○ 記著提交」？讀的是哪一個宣告過的東西（不是「儲存裡剛好有 commit 物件」的推測）？為什麼它不會被一個從未提交的工作區誤觸？
*   **Q2** ○ 層出現之前的舊儲存（沒有提交註記）若 `HEAD` 缺席，你的判準答什麼？那是可以接受的盲區，還是需要別的證據？**事實陳述題，不要擴大射程**。
*   **Q3** 逐一列出你判為「相對於 context」與「與 context 無關」的指令（含 `run`、`test`、`eval`、`inspect`、`refine`、`migrate`、`node *`），以及理由。
*   **Q4** `rollback` 在 `HEAD` 缺席時：`abandoned` 記什麼（沒有前一個 HEAD）？○ 的祖先邊怎麼寫？

## 6. 明文不在射程內

*   規格 §6.2.1 第 2 條的文字（`parent` → 祖先邊）：驗收方收尾。
*   「HEAD 的恢復要不要一個專門的動作、叫什麼」（`commit.md` §1.12.7 ③）：未裁；本弧只要求 `rollback` 照常可用。
*   `commit.md` §1.12 的其餘開放問題（點或濾子、多人共站一個框架）。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r6 基線必須是紅的，理由逐支寫在 doc comment；**你可以讓它們變綠，不得改寫它們的宣稱。**
*   判準只在兩處釘拼法：缺陷自己的句子（`no committed root yet`）與回頭路的指令名（`rollback`）。
*   **探針用到的儀器不准改**：`oo log`／`status`／`inspect`／`gc`／`rollback` 的既有輸出結構與旗標；「丟失」一律以把 `.oo/HEAD` 移開構造。
    **若你認為某支的判準不可能被一個正確的實作滿足，寫在「工單哪裡是錯的」，不要改儀器去對上它。** `rustfmt` 不得掃探針檔。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

### 8.2 順手改動（逐項指名）

### 8.3 工單哪裡是錯的

### 8.4 工單指名要你回答的問題

### 8.5 探針

### 8.6 數字

### 8.7 你認為需要改規格之處
