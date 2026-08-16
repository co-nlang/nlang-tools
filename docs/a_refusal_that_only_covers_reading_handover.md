# 工單 Q-029：拒絕只蓋住了讀 —— a refusal that only covers reading

> 開單 2026-08-16。**基線實測於開單當下**：`crates/oo/Cargo.toml` → `0.24.0`；
> `oo --version` → `oo v0.24.0`。偵察全文：`docs/a_refusal_that_only_covers_reading_recon.md`。
>
> **探針已預先寫好並校準**：`crates/oo/tests/a_refusal_that_only_covers_reading_probe_test.rs`
> ——4 綠（控制組）／6 紅（`#[ignore]`）。**交付方只得移除 `#[ignore]`，該檔其他一個字都不得改。**
> 需要新的探針時，寫在別的檔案裡，並在回報時說明。

---

## 1. 要修的是什麼

REAL_03 §6.8 第三條逐字：

> 引擎讀到自己不具備的標準根摘要時，**必須拒絕開啟該根**，且訊息**必須**指出所缺者
> 為何。**不得**以自身的標準根代入後繼續（MUST NOT）。

**規格不用改。** 引擎的讀取路徑做到了，寫入路徑從來沒問過。

### 1.1 成因是一行

`crates/oo/src/main.rs:1538`：

```rust
fn load_universe(engine: &Ouroboros, path: &Path) -> anyhow::Result<Universe> {
    let mut u = match Universe::load(engine, path) {
        Ok(u) => u,
        Err(_) => Universe::new(None, engine.root_with_system()),   // ← 這一行
    };
    let _ = u.load_staged(path);
    Ok(u)
}
```

`Universe::load`（`universe.rs:275`）**是對的**——它呼叫
`engine.store.get_root(&commit.root, &engine.standard_roots)?`，拒絕會往上傳。
然後 `Err(_)` 把它換成**一個以本引擎自己的標準根為根、沒有 HEAD 的全新宇宙**。

⟹ `Universe::new(None, engine.root_with_system())` **就是「自身的標準根」**，
`Err(_) =>` **就是「代入後繼續」**。那條 MUST NOT 被寫成了一個 fallback。

**一個成因，四個症狀**（全部實測，兩個真二進位＋探針各自重現）：

| 症狀 | 為何 |
| :--- | :--- |
| `evolve` 靜默成功 | 對著幻影根 staged |
| `commit` 回報成功、根帶 `65f52e2d…`、`parent: null` | 幻影根被寫進倉 |
| `squash` 回報 `no HEAD to squash` | 幻影宇宙的 `head` 真的是 `None`——**那句話對幻影為真，對倉為假** |
| `refine` 回報 `Refine commit: …`、HEAD 移動 | 同上，且其單調性檢查另有 §2.2 的問題 |

### 1.2 樹裡已經有正確的樣本

`oo rollback` **是對的**，探針 `c3` 綠著看守它。理由可見：`Universe::rollback`
（`universe.rs:796`）自己又呼叫了一次
`engine.store.get_root(&target_commit.root, &engine.standard_roots)?`，
所以拒絕繞過了 `load_universe` 的 fallback。

**要做的是讓其他幾支得到 rollback 已經有的東西，不是發明新東西。**

---

## 2. 射程：逐處列出，探針逐處對應

### 2.1 `load_universe` 的七個呼叫點

**必須逐一標註用途後再動**（不得整批替換）：

| 行 | 函式 | 今天 | 要求 |
| :-- | :--- | :--- | :--- |
| 421 | `run_evolve` | ❌ 靜默成功 | 拒絕並具名 → **P1** |
| 831 | `run_status` | ✅ 誠實回報 `(unavailable)` | **不得改變**（它是唯一能說明為何其他指令失敗的指令）→ **C1** |
| 951 | `run_rollback` | ✅ 正確拒絕 | **不得改變** → **C3** |
| 968 | `run_squash` | ❌ `no HEAD to squash` | 拒絕並具名 → **P5** |
| 998 | `run_commit` | ❌ `Commit successful` | 拒絕並具名 → **P2**／**P3**／**P4** |
| 1074 | `run_refine` | ❌ `Refine commit: …`、HEAD 移動 | 拒絕並具名 → **P6** |
| 1148 | `run_repl` | **未量**（互動式） | **交付方必須自己量並回報**，見 §4 |

⚠ **`run_status` 是最容易做錯的一格。** 若把 fallback 一律改成傳回 `Err`，
`status` 會跟著死掉，而它正是操作者唯一能得知「我缺哪一份標準根」的地方。
探針 **C1** 會抓到這個。

### 2.2 五個 `| None` 分類點

`get_value`／`get_commit` 失敗時以 `downcast_ref::<StoreReadError>()` 分類。
「標準根不具備」是一個 `anyhow!` 字串、**不是 `StoreReadError`**，故落在 `None`——
五處都把 `None` 併進了 `NotFound` 那一支：

| # | 位置 | 今天落到 | 要求 |
| :-: | :--- | :--- | :--- |
| 1 | `universe.rs:989` | `Ok(None)` ⟹ 運算元視為「未持有，不透明」⟹ **`refine` 的幾何單調性檢查被跳過** | **不得**與「未持有」共用同一支。該處註解逐字寫著「pretending it passed is the fail-open this arc exists to close」——**它描述的正是這一格** |
| 2 | `universe.rs:1063` | `break`（shadow-scan 讀 commit） | **中止整個操作並具名**〔用戶裁定 2026-08-16〕 |
| 3 | `universe.rs:1082` | `continue`（shadow-scan 讀 root） | 同上 |
| 4 | `builtins/disc.rs:200` | `Err(false)` ＝「不在」 | **不得**把「持有但接不回來」判為「不存在」 |
| 5 | `oodp.rs:388` | `refuse(NotFound, "not_held")` | 見 §2.3 |

**建議做法**：讓「標準根不具備」成為一個 `StoreReadError` 的變體，
於是這五處的 `match` 都必須顯式處理它，編譯器會替你找齊。
**若採此法，`StoreReadError` 的每一個既有 `match` 都在射程內，必須逐一檢視。**

### 2.3 線上那一格〔用戶裁定 2026-08-16〕

`oodp.rs:388` 今天回 `#not_found %reason: #not_held`——**那是假的**，節點持有那些
位元組。

**要求**：回 `#not_found` ＋ **新** `%reason: #standard_root_unavailable`。

*   **狀態不變**（`#not_found`）：O57-C 已裁「理由集得增長；狀態集不得」，
    且請求方的補救確實是「換一台問」。
*   **不得**改用 `#rejected`——那是「理解了但拒絕受理」，而這裡不是不願意，是做不到。
*   `ERROR_CODES` 的登記由**驗收方**負責，交付方不必動規格。

⚠ **本格為讀碼推得，偵察未以真封包實測**（CLI 無 `fetch` 子命令）。
**交付方必須實測它並回報實際線上回應**，見 §4。

---

## 3. 不在射程

| | |
| :--- | :--- |
| `oo log`／`oo inspect` | 已正確拒絕，**C2** 看守 |
| `oo gc` | 走可達性走訪、不 hydrate〔量：`2 objects, 2 reachable`〕 |
| `oo run`／`eval`／`test` | 不載入倉（O40 既有紀錄） |
| `oo identity`／`lint`／`node id`／`peers`／`affiliate` | 不碰根 |
| **標準根拆為獨立欄位**（甲） | **已裁但不在本弧**（見 `STATUS` O58）。本弧不得順手做 |
| 規格條文 | 一個字都不改 |

---

## 4. 交付方自檢：跑完這六項才算做完，探針全綠不是完成訊號

1.  **`cargo test --workspace --no-fail-fast`**，記下 passed／failed／ignored 與 suite 數。
    **不得省略 `--no-fail-fast`**——沒有它，cargo 會停在第一個失敗的 suite 並給出一個
    看起來合理的假數字。
2.  **`oo repl`（`main.rs:1148`）在標準根不具備的倉裡的行為**——偵察未量。
    實測並回報：它進得去嗎？進去之後看到的是誰的根？
3.  **`#fetch` 的真實線上回應**（§2.3）——起一個節點、對一個持有但不具備標準根的
    物件發 `#fetch`，回報實際封包內容。**不接受讀碼推論。**
4.  **列出所有因本次改動而需要調整的既有測試**，逐一說明它原本依賴的是哪一個舊行為。
    若有測試原本依賴「不具備即視為不存在」，**那本身就是本弧的證據，要單獨列出**。
5.  **符合性向量**：`conformance/L1|L2` 全數，回報 x/143。
6.  **`oo --version`** 確認你測的是你改的那個。

回報時附上第 1、5 項的**原始輸出**，不要只給結論。

---

## 5. 完成條件

*   探針檔 6 紅全綠、4 控制組仍綠，且該檔**逐位元組只少了六個 `#[ignore]`**。
*   §2.1 七個呼叫點逐一有交代（含 `run_repl` 的實測結果）。
*   §2.2 五處逐一有交代。
*   §2.3 有真實封包的量測。
*   §4 六項全部完成並附輸出。
*   `git diff` 不含任何 `spec/` 或 `meta/` 下的檔案。

---

## 6. 常設紅線

*   **絕不 `git add -A`**；`git stash` 停用。
*   探針檔的修改權在驗收方；交付方**只得移除 `#[ignore]`**。
*   commit message 走檔案 `-F`，不走 `-m "…"`（反引號會被命令替換）。
*   工作區全跑進行中**不得改動樹**。
*   不確定的事**回報，不要猜**——「未量」是一個合法的答案，猜錯不是。
