# 工單 Q-030：一個不在那裡的 digest —— a digest that was not there

> 開單 2026-08-16。**基線實測於開單當下**：`crates/oo/Cargo.toml` → `0.24.0`；
> 由 tag 之後的 `dev` 建置，`oo --version` → `oo v0.24.0-597-g37679ec`。
>
> **探針已預先寫好並校準**：`crates/oo/tests/a_digest_that_was_not_there_probe_test.rs`
> ——2 綠（控制組）／5 紅（`#[ignore]`）。**交付方只得移除 `#[ignore]`，該檔其他一個字都不得改。**
>
> **本弧中斷 Q-029**（第一層已交付並驗收；第二、三層未做）。判準：
> `WORK_QUEUE` §2.2「可由輸入觸發宿主 abort」，且**遠端、未認證、單一封包**。

---

## 1. 一句話

```
{ %op: #fetch, %hash: "hash:sha256:v1:", %from: "x" }
```

四十七個位元組，一台 `oo node serve` 行程消失。

`ContentHash::parse`（`value.rs:1987`）只檢查三件事：冒號段數 ≥ 4、前綴 `hash:sha256`、
digest 可 hex 解碼。**`hex::decode("") == Ok(vec![])`**，所以空 digest 是一個合法 CAID，
而**全程沒有任何一處檢查長度**。`storage.rs:476` 隨後切它的前兩個字元：

```rust
self.root.join(algo_dir).join(&hex[0..2]).join(&hex[2..])
//                             ^^^^^^^^^ end byte index 2 is out of bounds
//                                       for string of length 0
```

**非本次交付所致**——同一輸入對交付前的 `v0.24.0` 基線二進位逐字重現。

---

## 2. 修在哪：`parse` 驗長度〔用戶裁定 2026-08-16〕

**sha256 的 digest 是 32 bytes ＝ 64 hex 字元。`parse` 必須拒絕其他長度**，v1 與 v2
兩支都要（兩支是**兩個各自的 `hex::decode` 呼叫**，只修 v1 會留下 v2——探針 **P5** 看守）。

**不採**「只擋空」——〔量〕`hash:sha256:v1:ab` 今天被當成合法的 sha256 CAID 收下並
拿去查倉（探針 **P4**）。
**不採**「在 `hash_to_path` 加防禦」——那是修個案：`digest_path` 那一家仍需各自防禦，
且「`parse` 接受了不該接受之物」這件事沒有被回答。

### 2.1 正確的線上答案已經寫好了，只是到不了

`oodp.rs:371`：

```rust
let hash = match (&req.hash, &req.hash_raw) {
    (Some(h), _) => h.clone(),
    (None, Some(raw)) => {                      // ← 這一支
        let body = refuse(OodpStatus::Conflict, "unparseable_caid", source_id);
        return (body, format!("OODP #fetch unparseable %hash: {raw}"));
    }
    (None, None) => { … "missing_field" … }
};
```

`parse` 一旦正確拒絕，`hash` 為 `None`、`hash_raw` 為 `Some("hash:sha256:v1:")`
⟹ **`#conflict %reason: unparseable_caid` 自己就會出現**。

⚠ **探針 P1 逐字斷言 `unparseable_caid`。** 發明一個新答案不會通過——那是改協定，
不是修剖析器。

### 2.2 射程外的孿生（讀一下，不要動）

`storage.rs:193` `digest_path` 有**同樣的 `[0..2]`**。三個呼叫點裡只有
`object_exists_digest`（`:203`）有 `len() < 4` 防護；`remove_digest`／`read_raw_digest`
沒有。〔量〕**兩者今天不可達**——輸入由磁碟目錄結構重建，恆為 2+62 字元，且 `gc.rs`
另有 `is_hex64` 過濾。

**本弧不改它們。** 但若你的修法是「在 `parse` 保證長度」，請在回報裡說明
**為什麼那個保證到不了 `digest_path`**（它收的是 `&str` 不是 `ContentHash`）。

---

## 3. 射程：逐處列出，探針逐處對應

### 3.1 到得了（要修）

| 路徑 | 今天 | 探針 |
| :--- | :--- | :--- |
| **線上 `#fetch`**（`%hash` 空） | **行程死亡 rc=101，無回應** | **P1** |
| 線上：死後的下一個呼叫者 | 連不上 | **P2** |
| CLI `oo inspect` | panic | **P3** |
| CLI `oo rollback` | panic | **P3** |
| CLI `oo refine` | panic | **P3** |
| 長度不驗（`…v1:ab`） | 收下並查倉 | **P4** |
| v2 形（`…v2:_:AAA:`） | panic | **P5** |

### 3.2 到不了（已量，不要順手改）

| | 今天 | |
| :--- | :--- | :--- |
| 線上 `#discover` | 正常回應 | 不碰 `hash_to_path` |
| 線上 `#find_node` | `#malformed` | **已經是對的** |
| CLI `oo squash` | `squash base is not an ancestor of HEAD` | 先被祖先檢查擋下 |
| CLI `node discover`／`find-node` | `transport: Conflict` | 不碰倉 |

### 3.3 明文不在本弧

*   **節點的 panic 隔離**〔用戶裁定 2026-08-16：另一弧〕。今天全樹**沒有**
    `catch_unwind`、**沒有** `thread::spawn`、**沒有** `panic::set_hook`
    （`crates/oo/src/` 與 `oodp.rs` 皆零），`main.rs:521` 的
    `for stream in listener.incoming()` 使 handler 跑在 accept 迴圈自己的執行緒上
    ⟹ 任何一個 panic 都是整台節點的死因。**本弧只讓這一個輸入不再抵達。**
*   **Q-029 的第二、三層**（五個 `| None` 分類點、`not_held`）。
*   規格條文。〔量〕`ERROR_CODES` 已有 `#malformed`、`unparseable_caid` 已在用
    ⟹ **本弧預期零規格變更**。若你認為需要，**回報，不要改**。

---

## 4. 一件值得知道的事

〔量〕`oodp.rs` 全檔 `unwrap()`／`expect(`／切片／`panic!`／`unreachable!`
**共 0 處**。協定層是防禦性寫成的；這個 panic 來自它底下的儲存層。

⟹ **修法不應該把防禦加回協定層。** 讓 `parse` 履行它名字的承諾，協定層那一支就會生效。

---

## 5. 交付方自檢：跑完這五項才算做完，探針全綠不是完成訊號

1.  **`cargo test --workspace --no-fail-fast`**，記下 passed／failed／ignored 與 suite 數。
    **不得省略 `--no-fail-fast`**。基線為 **1943/0/0（198 套件）**（本探針檔 7 支尚未計入）。
2.  **符合性向量**：`python3 scripts/run-conformance.py --engine <你的 oo>`，回報 x/143。
    （`--engine` 不可省——省了它腳本會直接以參數錯誤退出，而外層仍可能 exit 0。）
3.  **`cargo test -p nlang-interpreter --test genesis_test`**——標準根 26 個 seed CAID
    不得移動。長度驗證若不慎改到 `ContentHash` 的序列化，這裡會先紅。
4.  **列出所有因本次改動而需要調整的既有測試或 fixture**，逐一說明它原本用的是哪一種
    不合法長度的 CAID。**若有測試在用短 digest 當 fixture，那本身就是本弧的證據，要單獨列出。**
5.  **`oo --version`** 確認你測的是你改的那個。

回報時附上第 1、2 項的**原始輸出**，不要只給結論。

---

## 6. 完成條件

*   探針檔 5 紅全綠、2 控制組仍綠，且該檔**逐位元組只少了五個 `#[ignore]`**。
*   §3.1 七列逐一有交代；§3.2 四列**仍為原樣**（不得順手改）。
*   §2.2 的問題有回答。
*   §5 五項全部完成並附輸出。
*   `git diff` 不含任何 `spec/` 或 `meta/` 下的檔案。

---

## 7. 常設紅線

*   **絕不 `git add -A`**；`git stash` 停用。
*   探針檔的修改權在驗收方；交付方**只得移除 `#[ignore]`**。
*   commit message 走檔案 `-F`，不走 `-m "…"`。
*   工作區全跑進行中**不得改動樹**。
*   不確定的事**回報，不要猜**。
