# Repair 1：歷史列存了建構器的形，不是出貨的形

> 2026-08-20 驗收。交付 `nlang-tools dev 23d6ec3`。
> **本弧不退回，射程與實作方向都對；一處資料錯誤使所有既存倉打不開。**

---

## 1. 缺陷

〔量，兩個真二進位〕v0.26.1 建倉並提交，交付版引擎讀之：

```
Standard root dependency: 2da5b713… (unavailable)
Error: refusing root: standard root digest 2da5b713… is unavailable
```

`evolve`／`commit`／`status` 皆拒。**每一個本弧之前寫下的倉都打不開。**

反方向**正確**：舊引擎讀新倉具名拒絕、不以自身代入（REAL_03 §6.8 第三條 MUST
逐字兌現）。**壞的只有舊→新。**

---

## 2. 成因：`for_cas_storage()` 夾在建構器與出貨物之間

〔讀〕交付的 `shipped_standard_roots`：

```rust
StandardRootSet::from_roots([
    self.root_with_system(),        // 新的，過 for_cas_storage
    Self::v0_22_standard_root(),    // 建構器原形
    Self::current_standard_root(),  // 新的原形
])
```

而 v0.26.1 真正**出貨**的標準根是 `for_cas_storage(v0_22_standard_root())`
——〔量〕digest `2da5b713…`。清單裡有**建構器的形**（`v0_22_standard_root()` 原形），
**沒有出貨的形**。

> **教訓（常設，本次賺到）：歷史列必須存「出貨的形」，不是「建構器的形」。**
> `root_with_system()` 自 v0.26.0 起在建構器與出貨物之間插入了 `for_cas_storage()`
> ⟹ **建構器的回傳值不再是任何引擎寫出去過的東西**。
> 判別法：一列歷史值若不是某個版本的 `root_with_system()` 回傳過的東西，它就不是歷史。

**附帶疑點（一併處理）**：清單同時列了 `current_standard_root()` 的**原形**與
`root_with_system()` 的**正規化形**。若兩者 digest 不同，引擎就宣稱支援一個
**它從不寫出**的位址；若相同，那一列是冗餘。**兩種情況都應消除**——
`shipped_standard_roots` 應只含**這個與先前各版引擎實際出貨過的**那些 digest，
每個都是出貨的形。

---

## 3. 要做的

1.  **把 v0.26.1 出貨的標準根加回 `shipped_standard_roots`**，以出貨的形
    （即 `for_cas_storage` 之後）。`2da5b713…` 必須 `supports_standard_root` 為真。
2.  **釐清清單語義**（§2 附帶疑點）：移除既非出貨形亦非歷史出貨形的條目，
    或說明為何需要。**回報你的判斷與理由。**
3.  **不得**改動 `v0_22_standard_root()` 建構器本身——它是歷史值的來源。
4.  **不得**為了讓 C5 綠而把新舊 digest 湊成一樣。C5 釘的是「舊的仍被支援」，
    不是「digest 不動」。

---

## 4. 新增控制組 C5（驗收方於本次補上）

`crates/oo/tests/a_name_the_printer_could_not_write_probe_test.rs` 末尾新增
`c5_control_the_previously_shipped_standard_root_stays_supported`。

**這是驗收方的漏**：本弧移動標準根 digest，而原探針檔**沒有任何一支看守歷史支援**
——Q-032 有 C3 做這件事，本弧漏了。C5 把跨版本的失敗縮進測試套件內，
交付方不必自己搭真二進位矩陣就看得到。

〔量〕C5 於交付版 **紅**；其餘 10 支綠。

⚠ **C5 不能證明舊倉真的打得開**——那需要一個標準根不同的二進位，樹內造不出來。
真證據是驗收時的跨版本矩陣。

---

## 5. 其餘驗收結果（本次通過，不必重做）

| 項 | 結果 |
| :--- | :--- |
| diff 純度 | ✅ 未碰 `spec/`／`meta/` |
| 探針完整性 | ✅ **恰好六行 `#[ignore]`**，其他一字未動 |
| 六紅 | ✅ 全綠 |
| 四控制組 | ✅ 仍綠 |
| 符合性 | ✅ 143/143 |
| genesis | ✅ 11/11（Engine 非 seed，與回報一致） |
| 根物件形狀 | ✅〔量〕`app: { k1: 1 }` 的根**只有 sentinel 變**（`2da5b713…`→`229be911…`），使用者內容逐字相同 |
| 反方向跨版本 | ✅ 舊引擎讀新倉具名拒絕、不代入 |
| 射程外的第四處（`unify.rs`） | ✅ **接受**——同一缺陷在合一路徑的重演（`field_keys()`＋`insert_field` 會把資料軸的 `@t` 重新路由到型別軸），且附了理由。修類別不修個案 |

---

## 6. 已知會紅、**不是**你的責任（驗收方處理）

〔量〕以下 6 支是**舊弧的字面 CAID 釘樁**，因裁定 B 移動 digest 而合法過期：

```
every_byte_or_none::p1_the_root_caid_does_not_move
knob_that_does_nothing::p1_plain_commit_root_is_unchanged
limit_you_cannot_choose::p1_plain_commit_root_is_unchanged
print_what_can_be_read::p4_root_caid_does_not_move
snapshot_not_a_reading::p3_a_universe_without_a_blur_keeps_its_root
snapshot_not_a_reading::p5_a_morphism_bearing_universe_has_its_new_root
```

**探針修改權在驗收方，你回報而不動它們是對的。** 驗收方將於 repair 通過後更新。
**不得為了讓它們綠而改動產品碼。**

---

## 7. 完成條件

*   C5 綠，且 §3 第 2 項有明確回報。
*   其餘 10 支仍綠。
*   `cargo test --workspace --no-fail-fast`：除 §6 那 6 支外無其他紅。
    ⚠ **失敗不一定出現在 `test result:` 行**——同時看輸出結尾的 `targets failed`。
*   符合性 143/143、genesis 11/11。
*   `git diff` 不含 `spec/`／`meta/`。
