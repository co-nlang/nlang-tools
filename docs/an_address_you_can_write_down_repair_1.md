# Repair 1：一次完整性失效不得被報告成「找不到」

> 開單 2026-08-22。交付 `nlang-tools dev f04784d`（驗收通過，見 §1）。
> 探針 `crates/oo/tests/an_address_you_can_write_down_probe_test.rs`
> ——**現為 10 綠 1 紅**；新增的是驗收方寫的 **R7**，紅在對的理由上。
> **除 R7 外，探針檔與 §1 的驗收數字皆不重跑。**

---

## 1. 先說清楚：主體已經驗收通過

| | |
| :--- | :--- |
| 本弧探針 | 10/10、0 ignored，三輪皆穩 |
| 全跑 ×5 | **2012 passed / 0 failed / 0 ignored**，五輪全同，`error` 行 0 |
| 目標覆蓋 | 206 個 target（203 Running ＋ 3 Doc-tests）**全部回報** |
| conformance ／ genesis | 143/143 ／ 11/11 |
| `golden_ast.rs` | diff **0 行** |
| **身分紅線** | 四個宇宙根位址與標準根 digest，兩個真二進位**逐字元相同** |
| 探針完整性 | 乾淨——只少六行 `#[ignore]` |

**本 repair 不是「工單沒做到」**。工單只要求「缺位址是具名拒絕」，那一項做到了。
本 repair 治的是它**順帶重新打開的一個已關閉類別**。

---

## 2. 論旨

**位址解析的失敗必須說出是哪一種失敗。**

〔量 2026-08-22，同一個倉、同一個物件，只把 `[1,[7]]` 改成 `[1,[9]]`
——JSON 仍良構、仍可解碼，故必須由**重算位址**抓到（REAL_03 §6.6）〕：

| 路徑 | 答案 |
| :--- | :--- |
| `oo inspect <同一個 CAID>` | `#caid_mismatch: object at digest path is corrupt (integrity failure)` |
| `_{位址}.k` | **`⊥ #missing_key`** |

物件**在**磁碟上，只是壞了。而位址路徑回報「找不到」。

**成因是一個 `_`**〔讀 `lib.rs:4066`〕：

```rust
match self.store.get_value(&hash) {
    Ok(v) => v,
    Err(_) => Value::Bottom(… cause: BottomCause::MissingKey …),
}
```

`StoreReadError` 的四個變體——`NotFound`／`CaidMismatch`／`ObjectUndecodable`／
`StandardRootUnavailable`——被壓成同一個答案。

---

## 3. 為什麼這一格擋收弧

**這是 Q-031（v0.24.0）關過的類別。** 該弧的論旨逐字：

> 「閘裝在宇宙上，閘底下**五個呼叫點**仍把『持有但打不開』折進另一個答案。」

而它的解法**正是為了防止這件事再發生**：新增 `StoreReadError::StandardRootUnavailable`
這個變體，**讓編譯器找齊那五處**。

`Err(_)` 從那個檢查旁邊走過去了——**它是唯一一種編譯器看不見的寫法**。
現在是第六處，而且是新寫的。

**代價不是理論的**：引擎對一次完整性失效說了一句不真的話。這正是
REAL_03 §6.6 在 v0.28.0 才剛加強過的地方（讀路徑必須重算比對並具名回報），
也是本紀元反覆出現的同一形狀。

---

## 4. 射程（一項）

**S1**：把 `lib.rs:4066` 的 `Err(_)` 換成對四個變體的 match，
各自映射到**既有的**原因碼。**探針 R7。**

〔量〕四個碼**全部已在 `TAG_REGISTRY`**，本 repair **不得新增任何狀態**：
`#not_found`／`#caid_mismatch`／`#object_undecodable`／`#standard_root_unavailable`。

**不要新造一個「位址解析專用」的錯誤碼。** 這條路徑讀的就是同一個 store，
它欠使用者的答案與 `inspect` 欠的是同一個。

---

## 5. 不做

*   **不要改 `inspect` 或任何既有讀取路徑**——它們是對的，本 repair 以它們為對照組。
*   **不要動 R1–R6 與 C1–C4**，它們已綠。
*   **不要碰身分**：本 repair 只改一個 `match` 的臂，BN/ 編碼（`0x04` ＋ algo ＋ 32 位元組）
    一個位元不動。
*   **不要順手改 `~%Discovery./fetch`**——它有同族問題，但那是 O70 ⑤ 的紀元弧。

---

## 6. 完成條件

1.  探針 **11/11、0 ignored**。
2.  全跑 `--no-fail-fast` **×5 全同**，基線 **2012 passed / 0 failed**。
3.  conformance 143/143、genesis 11/11。
4.  **身分未動**：四個宇宙根位址與標準根 digest 仍與 §1 逐字元相同。
5.  `golden_ast.rs` diff 仍為 0 行。

---

## 7. 交付方自檢

*   [ ] `git diff` 只含 `lib.rs` 那一處 `match` 與探針檔**少一行 `#[ignore]`**。
*   [ ] 沒有新增任何原因碼——`TAG_REGISTRY` 未變。
*   [ ] 四個變體**逐一**有映射，沒有任何一個仍走 `_`。
*   [ ] 完成條件第 4 項是**跑出來的**，附位址逐字比對。
*   [ ] **凡順手改動之處，逐項在報告中指名**——包含你認為是明顯改善的。
      上一輪 `static_analyzer.rs` 的重構修掉了三個潛伏差異（`Current` 的
      `~.` vs `^.`、`Parent` 少一個 `^`、`Parent` 的 segments 被整段丟掉），
      三個都是好的修正，**但工單沒要、報告沒提**，驗收方是逐行讀 diff 才發現的。
