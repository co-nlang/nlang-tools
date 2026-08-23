# Q-029 偵察：拒絕只蓋住了讀 —— a refusal that only covers reading

> 開弧：2026-08-16。基線**實測於開單當下**：規格 `v0.24.0-draft.1`／引擎 `v0.24.0`。
>
> 兩個二進位，**未竄改任何位元組**：
> *   `/home/gali/nlang-baselines/q025-plus-target/debug/oo` → `oo v0.22.0`，標準根 `a63ef70b…`
> *   `/home/gali/nlang-baselines/v0.24.0-verify-target/debug/oo` → `oo v0.24.0`，標準根 `65f52e2d…`
>
> 由 Q-028 偵察中斷開弧（`WORK_QUEUE` §2.2 interrupt-candidate：資料遺失 ＋ 使同一
> CHS 得不同身分）。Q-028 停在裁定階段、未開工單，故此刻搶佔成本最低。

---

## 1. 主張

**REAL_03 §6.8 第三條的拒絕，只蓋住了讀取路徑。寫入路徑從來沒有問過。**

而在讀取路徑內部，「我不具備這份標準根」在 **5 個地方**被併進了另一個答案——
其中一個是「我沒有這個物件」（對線上），另一個是「這個檢查通過了」（對 refine）。

---

## 2. 判例（可重現，兩個真二進位）

```
以 plus（標準根 a63ef70b…）建倉、提交
以 v0.24.0（不具備 a63ef70b…）操作同一個倉：

  oo status          → Standard root dependency: a63ef70b… (unavailable)   ✅ 誠實
                       …接著照常印出 staged changes
  oo log             → Error: refusing root: … a63ef70b… is unavailable    ✅ §6.8 兌現
  oo inspect <root>  → Error: store read failed …: refusing root: …        ✅
  oo evolve n.n      → （靜默成功，內容進 staged）                          ❌
  oo commit -m x     → Commit successful: 5caf8484…                        ❌
```

提交之後，倉裡有**兩個 `app` 內容相同、標準根摘要不同的根**：

| 根 | `data` | `__nlang_system_digest` | 寫它的引擎 |
| :--- | :--- | :--- | :--- |
| `1b581444…` | `app` | `a63ef70b…` | plus |
| `5aa84c94…` | `app` | **`65f52e2d…`** | v0.24.0 |

`HEAD` 移到後者。**原引擎 `plus` 從此讀不了自己建的倉**
（`refusing root: … 65f52e2d… is unavailable`）。

⟹ REAL_03 §6.8 第三條 MUST NOT 逐字：「**不得**以自身的標準根代入後繼續」。
**代入了，也繼續了。** 而它發生在同一個二進位**一個指令之前才正確拒絕過**。

---

## 3. 射程：6 處，逐處後果不同

### 3.1 五個分類點——同一個 `| None` 寫法

`get_value`／`get_commit` 失敗時，各處以
`e.downcast_ref::<StoreReadError>()` 分類。「標準根不具備」是一個 `anyhow!` 字串，
**不是 `StoreReadError`**，故 `downcast` 得 `None`——而五處都把 `None` 併進了
`NotFound` 那一支：

| # | 位置 | `| None` 落到哪 | 後果 |
| :-: | :--- | :--- | :--- |
| **1** | `universe.rs:989` | `Ok(None)` ⟹ 運算元視為「本地未持有，不透明」 | **`refine` 的幾何單調性檢查被靜默跳過**。⚠ 該處註解逐字寫著「pretending it passed is the fail-open this arc exists to close」——**它描述的正是這一格今天會發生的事** |
| **2** | `universe.rs:1063` | `break` | shadow-scan 讀 commit 失敗 ⟹ **靜默截斷**（對照組：真正的完整性錯誤會 `record_integrity`） |
| **3** | `universe.rs:1082` | `current = commit.parent; continue` | shadow-scan 讀 root 失敗 ⟹ **靜默略過該 commit** |
| **4** | `builtins/disc.rs:200` | `Err(false)` ＝「不在」 | 本地**持有**但接不回來的物件被判為**不存在** ⟹ 轉而去問對等點 |
| **5** | `oodp.rs:388` | `refuse(NotFound, "not_held")` | **對線上說一句假話**：`%reason: #not_held`，而它其實持有那些位元組 |

第 5 格與 Q-027 的 `#peer_refused` 同族：**一句關於自己的假陳述，由一個
`| None` 產生**。第 1 格更重：它把一個安全檢查變成 fail-open。

### 3.2 寫入路徑——從來沒問

`oo evolve`／`oo commit` 不呼叫 `resolve_standard_root`，也不查
`supports_standard_root`。〔量〕`status` **在同一個行程裡已經知道答案**
（它印得出 `(unavailable)`），`evolve` 沒有問。

⟹ 與 D38 同族：**引擎算得出來，然後在邊界上沒有人去讀。**

### 3.3 不在射程（已量，正確或無關）

| | |
| :--- | :--- |
| `oo status` | 誠實回報 `(unavailable)` ⟹ **保持**。它是唯一能告訴你「為什麼其他指令都失敗」的指令 |
| `oo log`／`oo inspect` | 已正確拒絕 |
| `oo run`／`oo eval`／`oo test` | 不載入倉（O40 既有紀錄），與標準根無關 |
| `oo identity`／`oo lint`／`node id`／`node peers`／`node affiliate` | 不碰根 |
| `oo gc` | 走可達性走訪，不 hydrate ⟹ 不受影響〔量：`2 objects, 2 reachable`〕 |
| `oo rollback`／`oo squash` | **待量**——本次只以 `HEAD` 字面測到參數解析就退出（`Invalid CAID format`），未以真 CAID 走到儲存層 |

---

## 4. 這是引擎的鍋

規格那半已經寫得夠清楚：

> §6.8 第三條 MUST：引擎讀到自己不具備的標準根摘要時，**必須拒絕開啟該根**，且訊息
> **必須**指出所缺者為何。**不得**以自身的標準根代入後繼續（MUST NOT）。

`evolve`／`commit` 開啟了該根、代入了自己的標準根、繼續了。**沒有任何規格條文
需要改。**

唯一需要裁的是**線上那一格該回什麼**（§3.1 第 5 格）——`#not_held` 是假的，
但正確的答案不只一個。見 §5。

---

## 5. 待裁：線上那一格

節點持有物件的位元組，但接不回那份標準根，故拿不出一個可驗證的值。

| | 回答 | 代價 |
| :-- | :--- | :--- |
| **(a)** | `#not_found` ＋ 新 `%reason: #standard_root_unavailable` | 誠實（「我拿不出來」），但仍在 `#not_found` 底下，而請求方的補救是「換一台問」——這裡換一台是對的 |
| **(b)** | `#rejected` ＋ 同一個新 `%reason` | 「我理解，但我不辦」。O57-C 已裁狀態集不得增長、理由集得增長，兩者皆合該規則 |
| **(c)** | 服務端改為交出**未 hydrate 的形**，由持有該標準根的請求方自己接回 | 最有用——**這正是丁-b 的雛形**。但需要新的回應形，射程遠大於本弧 |

〔量〕`#not_implemented` 在**求值層不存在**；`#peer_not_implemented` 為 OODP 專用。
新 `%reason` 須登記進 `TAG_REGISTRY`。

---

## 6. 尚未量（不擋裁定，擋驗收）

*   `oo rollback`／`oo squash` 以**真 CAID** 走到儲存層時的行為。
*   `#fetch` 在服務端對「持有但不具備標準根」的實際線上回應（§3.1 第 5 格為**讀碼推得**，
    本次未以真封包實測——CLI 無 `fetch` 子命令）。**工單必須要求交付方實測它。**
*   修好之後，工作區有多少測試依賴「不具備即視為不存在」這個舊行為。
