# Q-032 Repair Round 1

> 2026-08-16。交付 `nlang-tools a71a69b` 已提交並驗收。**未通過。**
> 本輪基線 ＝ 該交付；工單本體不變，見 `the_half_that_was_never_written_handover.md`。

---

## 0. 做對了的（不要動它們）

*   探針檔**逐位元組只少了四個 `#[ignore]`** ✓
*   本弧探針 **8/8**，含四個控制組全綠 ✓
*   **歷史列已加**：`from_roots([self.root_with_system(), Self::v0_22_standard_root()])`
    ——一個正規化後的新值，一個原始的舊值。`supports_standard_root("65f52e2d…")` 為真，
    `status` 回報 `(available)` ✓
*   新引擎**讀寫自己的倉完全正常**（新標準根 `2da5b713…`）⟹ 新規則自洽 ✓
*   conformance **143/143** ✓

---

## 1. 未通過之一：舊倉打不開（設計，已裁 O63）

〔量，兩個真二進位、未竄改任何位元組〕

```
舊引擎 oo v0.25.0（標準根 65f52e2d…）建倉並提交
新引擎 oo v0.25.0-611（標準根 2da5b713…）操作它：

  status  → Standard root dependency: 65f52e2d… (available)   ← 歷史列在
  log     → #caid_mismatch
              requested   …:0ebe51f5999f0c3e2b8d9098c7dc9a37…
              recomputed  …:b7025e4f0192dd886383ed0ed5dc2621…
  evolve  → 同上
  commit  → 同上（HEAD 未動——沒有寫壞，這一點是對的）
```

**診斷**：拆開改變了根的**位址規則**——

| | 根的位址算什麼 |
| :--- | :--- |
| 舊 | `hash(標準根 ⊕ 使用者內容)` |
| 新 | `hash(殘差 ＋ 指名依賴)` |

⟹ **持有舊的「值」不夠，還要持有舊的「規則」。**
O55 的表是 `digest → 標準根值`；本弧第一次讓它需要 `digest → 值 ＋ 讀法`。

### 1.1 裁定 O63：走格式閘，不走「規則隨標準根走」

**要做的**：`.oo/objects/` 的**容器宣告**升版；舊倉在容器層被辨識，以**舊讀法**讀取。

**不得**把讀法塞進標準根表。理由逐字：REAL_03 §6.8.2 寫著
「**新增一版＝ `from_roots([…])` 多一個元素，解析邏輯零改動**」——把解碼分支路由到那張表
會直接推翻它，且每加一代就多一條永久維護的分支。

**依據**：O23 已裁「物件編碼由 `.oo/objects/` 這個容器宣告，不由 `.oo/format`，
也不由物件自己」。〔量〕**編碼軸自 2026-07-28 引入以來從未真的被撥動過**，本弧是第一次。

**而 §6.8 第四條 MUST 從另一面預備了這件事**：

> 格式須先宣告（MUST）……使不認得本形式的引擎在**閘上**拒絕，
> 而非按舊讀法解讀出一個不同的根。

**本次失敗正是它的鏡像**：新引擎按**新**讀法，把一個舊根解讀成了一個不同的根。

### 1.2 兩層不得互相取代

*   **歷史標準根值**仍須持有（C3 看守）——那是 O55／O56。
*   **格式閘**另治位址規則——那是 O63。

做完之後，跨版本矩陣必須全綠：舊倉在新引擎上**讀得回、寫得進、位址不動**。

---

## 2. 未通過之二：21 支紅未被回報（紀律）

〔量〕`cargo test --workspace --no-fail-fast`：

```
1943 passed / 21 failed / 0 ignored（201 套件）
基線                    1956 passed /  0 failed / 0 ignored（200 套件）
```

**11 個套件紅**：

```
a_value_not_a_recipe_probe_test      effect_cached_probe_test
every_byte_or_none_probe_test        held_but_unopenable_probe_test
knob_that_does_nothing_probe_test    limit_you_cannot_choose_probe_test
local_gc_probe_test                  name_points_at_remedy_probe_test
print_what_can_be_read_probe_test    slash_shadow_cli_probe_test
snapshot_not_a_reading_probe_test    verdict_must_gate_probe_test
```

回報只說「完成」。**工單 §6.1 要求附原始輸出、§6.4 要求列出受影響測試——兩項都沒有做。**

### 2.1 處置：逐一分類，不是修到綠

**每一支紅必須落入下列三類之一，並在回報中寫明是哪一類與為什麼：**

| 類 | 意義 | 處置 |
| :-- | :--- | :--- |
| **A 預期會變** | 該測試斷言的是**本弧刻意改掉**的行為（例如「根裡有 `~%Math`」、「根位址是某個定值」） | **改測試**，並說明它原本鎖的是哪一個舊行為 |
| **B 真回歸** | 本弧不該碰它 | **改實作** |
| **C 說不清** | 你不確定是 A 還是 B | **回報，不要猜**——「未定」是合法答案 |

⚠ **A 類本身就是本弧的證據**（同 Q-030 的 `bohr_test` 四個短 digest fixture）。
**不得整批改綠而不分類**——那會把「我們刻意改了什麼」這件事一起抹掉。

以下幾支從名字看**很可能是 A**，但**仍須你逐一確認並說明**，不接受我這裡的猜測當結論：
`p1_the_root_caid_does_not_move`、`p4_root_caid_does_not_move`、
`p1_plain_commit_root_is_unchanged`、
`r3_the_root_carries_the_digest_of_system_not_its_body`。

而 `p2_refine_aborts_when_the_shadow_scan_meets_a_root_it_cannot_open`
（Q-031 的探針）**紅得可疑**——那是上一弧剛驗收過的行為，**優先當 B 處理直到你證明它是 A**。

---

## 3. 仍未交的自檢項

工單 §6 七項，回報裡缺：

*   §6.1 全跑的**原始輸出**
*   §6.3 `genesis_test` 的**新舊兩組 seed 數值**〔已知新標準根為 `2da5b713…`，
    但 26 個 seed 的新舊對照未交〕
*   §6.4 受影響測試清單（＝ §2.1）
*   §6.5 四項量測：拆開後根物件的實際形／`#cached` 固化在定義側還是觀測側／
    **`#blur` 的 CAID 是否真的含視界參數**／新舊標準根 digest

---

## 4. 完成條件（本輪）

1.  跨版本矩陣全綠：**舊引擎建的倉，新引擎讀得回、寫得進、位址不動**。
    （驗收方會用兩個真二進位重驗，不看探針。）
2.  `cargo test --workspace --no-fail-fast` **回到 0 failed**，且 §2.1 的分類表已交。
3.  本弧探針仍 **8/8、0 ignored**，該檔仍**只少四個 `#[ignore]`**。
4.  §3 四項自檢補齊，附原始輸出。
5.  `git diff` 不含任何 `spec/` 或 `meta/` 下的檔案。

---

## 5. 紅線（不變）

*   **絕不 `git add -A`**；`git stash` 停用。
*   探針檔的修改權在驗收方。
*   工作區全跑進行中**不得改動樹**。
*   **不確定就回報**。本輪最不希望看到的是 21 支紅被無聲改綠。
