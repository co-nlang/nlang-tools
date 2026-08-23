# Q-035 repair 1 —— 舊格式宇宙的標準庫停止運算

> 開單 2026-08-23，驗收 `4d047f4` 之後。**射程只有一項。**
> 原工單 `a_name_is_no_longer_a_credential_handover.md`。裁定 O68 Q3.B 不變。

---

## 0. 交付方揭露了它，而它成立

交付報告的「格式 1/2」那段是對的，而且比它自己說的重一級——
**倒下的不只是內嵌偽造，是標準庫本身。**

〔量 2026-08-23，三個真二進位，同一個倉〕

| 引擎 | `lib: ~%Math./add (3,4)` | `out: adder (1,2)`（內嵌 `%builtin`） |
| :--- | :--- | :--- |
| **v0.20.0**（造這個倉的那個） | **7** | **3** |
| **PRE**（`ebc0a5a`，交付前） | **7** | **3** |
| **POST**（`4d047f4`，交付後） | **⊥ `#unprojected_builtin`** | **⊥ `#unprojected_builtin`** |

`lib` 是**合法的標準庫呼叫**，不是偽造。⟹ 這是交付造成的回歸。

**在野不是空集合。**〔量，全機普查〕15 個有 Combo 物件的倉中，10 個的根帶
`__nlang_system_digest`、**5 個沒有**；其中 4 個是本次量測自己造的，
**第 5 個是 `/home/gali/nlang/.oo`——超專案自己的倉，HEAD 日期 2026-08-14，根物件 67,494 B**。
（該倉以複製方式測試，原倉 HEAD 量測前後逐字元相同，未被寫入。）

**輕重**：歷史**打得開**、名字**解析得到**、`oo status` 正常印出，**但什麼都算不出來**。
依 REAL_03 §6.8.1 的區分，這落在**可讀性**那一半，是重的那一半。

---

## 1. 成因（已定位，不必再找）

〔讀 `crates/interpreter/src/universe.rs:159–170`，**既有程式碼，非本次交付所寫**〕

```rust
fn standard_for_root(engine: &Ouroboros, root: &ContentHash) -> Result<ComboVal> {
    match engine.store.root_standard_digest(root)? {
        Some(digest) => …,
        // Formats 1/2 were self-contained; keeping the standard layer empty
        // preserves that shape rather than adding today's library to history.
        None => Ok(ComboVal::default()),
    }
}
```

閘之前，這個空表無害——派送**根本不查表**。閘之後，
`Universe::load` → `new_with_standard(…, 空表)` → EvalContext `.with_standard_root(空表)`
⟹ `standard_root_installed = true` 而 `projected_builtins` 為空
⟹ **該宇宙的每一個 `%builtin` 都得 `#unprojected_builtin`**。

**「self-contained」這個詞就是答案**：舊格式的自足，意思是**庫在根裡**。
〔量，那個 67,494 B 的根〕`data` 鍵 = `['x']`（**0 個 builtin**）；
`system` 鍵 = `Bytes/Complex/Cond/Config/Csv/Diff/Discovery/Effect/Engine/Env/Io/Json…`；
全根 256 處 `builtin`，**238 處在 `system` 軸底下，`data` 軸 0 處**。

---

## 2. 射程（唯一一項）

**S1 —— 沒有摘要的根，它的憑證表就是它自己的 `~%` 軸。**

O68 Q3.B 的規則是「憑證是**這個宇宙的**標準根」。格式 3 的那份表是一個獨立 CAS 物件；
格式 1/2 的那份表**內嵌在根裡**。**兩者是同一句話的兩種存放方式**，不是「有表」與「沒有表」。

⟹ 當 `root_standard_digest` 回 `None` 時，該宇宙的投影名集合**必須**取自根本身的
標準庫軸（`system`，以及 `rules` —— 〔量〕舊根的 `rules` 有 `add`，即 O65 後來移除的那個頂層 `/add`），
**不得**取自 `data`。

**這樣做同時保住兩件事**：
1.  合法標準庫呼叫恢復（`lib` 回到 `7`）。
2.  **閘不變弱**：使用者寫在 `data` 裡的偽造，其授權程度與格式 3 **完全一致**
    ——名字在庫裡就過、不在就擋。〔量〕`out: adder (1,2)` 恢復成 `3`，
    正如格式 3 的 `process.exit` 偽造仍然 exit 7（C3）。**這是一致，不是漏洞放大。**

### 明確不做

*   **不得**改成「舊格式宇宙一律不設閘」——那會讓 `#unprojected_builtin` 在整類宇宙上消失，
    探針第二支就是為了擋這條路。
*   **不得**把今天的標準庫塞進舊宇宙的表（`standard_for_root` 那句註解拒絕的正是這件事，它是對的）。
*   **不得**動 `ComboVal` 欄位、標準根內容、任何位址。
*   **不得**碰 Q1／Q2a／Q5。

---

## 3. 基線（開單當下實測，2026-08-23，`dev 4d047f4`）

*   全跑 **×5 逐字全同：`passed=2024 failed=0 ignored=0 targets=208 err=0`**
*   conformance **143/143**（`nlang-spec/scripts/run-conformance.py`）
*   主探針 `a_name_is_no_longer_a_credential_probe_test` **9/9、0 ignored**（×3）
*   S2 crate 測試 **2/2**
*   標準根 digest `7038e250…`，與交付前逐字元相同（**身分未動**）

⚠ 加入本 repair 探針檔之後，交付方看到的「交付前」應是
**`passed=2025`、`ignored=1`、`targets=209`**（新檔 1 綠 1 紅）。

---

## 4. 探針（**已由驗收方在開單當下校準**）

`crates/interpreter/tests/a_library_that_lives_in_the_root_test.rs` —— **1 綠 1 紅，已實跑確認**。
探針修改權在驗收方；認為校準錯了就**回報，不要改**。可以動的只有 `#[ignore]` 那一行。

| ID | 基線 | 斷言 |
| :--- | :--- | :--- |
| `a_legacy_root_still_dispatches_its_own_library` | **紅** | 空標準層＋庫在使用者根 ⟹ `lib: ~%Math./add (3,4)` 必須是 `7`。**先斷言 REACH**（`lib` 不得是 `<absent>`），再斷言值 |
| `the_gate_still_holds_for_a_name_that_root_does_not_have` | **綠** | 同一種宇宙裡，憑空發明的名字**仍須**得 `#unprojected_builtin` ——**這支是用來擋「修法＝拿掉閘」的** |

紅的那支基線實測值逐字：`_|_ (%cause: #unprojected_builtin)  ;; standard root does not project math.add`。

**crate 層而非 CLI 層的理由**：重現只需要
`Universe::new_with_standard(None, oo.root_with_system(), ComboVal::default())`
——那正是本次交付把 63 個測試檔**改離**的那個形狀。不需要 fixture，不需要舊二進位。

---

## 5. 交付方自檢

1.  **凡順手改動之處，逐項指名**，包含你認為明顯是改善的。
2.  **只見證「沒有報錯」的斷言什麼都沒有見證。**
3.  全跑一律 `--no-fail-fast`，失敗計數**不得只看 `test result:` 行**；
    若出現 `error` 行，**先保留完整輸出**再判斷是產物問題還是缺陷。
4.  **本項無規格變更**——`TAG_REGISTRY` 三個碼已於 `nlang-spec 6da71f7` 登記，
    語義不因本 repair 改變（`#unprojected_builtin` 的定義本來就是「這個宇宙的表沒有這個名字」，
    而舊宇宙的表不是空的，是內嵌的）。**若你認為需要改規格，先回報再動。**
5.  交付前先提交；驗收在提交上做。
