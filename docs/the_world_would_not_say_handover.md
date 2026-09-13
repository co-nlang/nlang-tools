# 工單 — The world would not say（Q-046）

**日期**：2026-09-13
**引擎起點**：Q-045 收弧後的 `dev`／**規格起點**：v0.49.0-draft.1
**裁定**：**D66 已裁**（`nlang-spec/meta/oo/STATUS.md`）。本弧是它的落地，**不需要新的裁定**。
**探針**：`crates/oo/tests/the_world_would_not_say_probe_test.rs`（驗收方已寫並校準，**基線 2 綠 3 紅**）

---

## 1. 這一弧是什麼

〔量 2026-09-13，Q-045 收弧後的二進位，`oo eval`，每格帶對照〕

| | ENOENT | EACCES | ENOTDIR | ELOOP | store 邊界 |
| :-- | :-- | :-- | :-- | :-- | :-- |
| `~%Io./exists` | `#false` | **`#false`** | **`#false`** | **`#false`** | `_\|_ ;; %cause: #store_boundary` |
| `~%Io./read_file` | `#none` | **`#none`** | **`#none`** | **`#none`** | 同上 |

**四種宿主失敗，一個答案。** 而**模範就在同一個函數裡**：store 邊界那條路給的是
**載體 ＋ 登記過的成因**，只有宿主錯誤那條路塌成一個值。

**這一格是這一族唯一把壓平後的結果當成值交給 n/ 程式的**——其他成員都在引擎內部，
它把答案交出去給程式做決定。

### 1.1 D66 怎麼切

**不是按錯誤切，是按問句切**：

| 情形 | 問句 | 載體 | `%cause` |
| :-- | :-- | :-- | :-- |
| 可回答 | 好 | 值本身（`#true`／`#false`／內容） | — |
| **EACCES／EIO** | **好，世界不肯說** | **`#blur`** | **登記過的成因** |
| **ENOTDIR／ELOOP** | **問句本身壞了** | **⊥** | 登記過的成因 |
| store 邊界 | 你不可以問 | ⊥ | `#store_boundary` **（已經對了，不要動）** |

**ENOENT 不動**：問句沒有毛病，而「它不在」是一個真的答案。

**⚠ 這不是新標準。** 本專案 **v0.2.41** 就判過同一個塌陷，判詞逐字寫在
`store_boundary_probe_test` 裡：「a refusal that renders as `#false` is
**indistinguishable from "the file is not there"**, so it is not an audit face」。
**當時判在 store 邊界那個載體上，本弧是把它套到宿主錯誤這個載體。**

---

## 2. 射程 ＝ 兩條不變式

### I1 `~%Io./exists` 的四種宿主失敗不得共用一個答案

依 §1.1 的表。**「不得共用」是不變式，四個格子各自填什麼由 D66 給定。**

### I2 `~%Io./read_file` 同樣，因為它也是一個觀測

`#none` 保留給 ENOENT。其餘三格依同一張表。

**⚠ 兩條都要求一個全函數的對映，不是逐個 errno 的個案。**
`EIO`、`ENAMETOOLONG`、`ENOMEM`、未來的任何一個——**每一個 errno 都要落進上表某一列**，
且**沒有落點的那一類必須有一個總括的答案**（Q-042 的 `_ => "unreadable"` 是可抄的形狀）。
探針只測四個代表，**但驗收方會另造沒被探針測過的 errno**。

---

## 3. 明確不做

1. **`~%Io./write_file` 與 `~%Io./append_file`。**〔量〕它們在失敗時同樣一律 `#none`
   （父目錄 000、中間段不是目錄，皆然）——**同一個塌陷**。
   **但它們是動作不是觀測**，而 `#blur` 的意思是「一個沒有完成的觀測」；
   一次證明沒有發生的寫入該回什麼，**是另一個問題** ⟹ **已記為 O87，本弧不動。**
2. **store 邊界那條路一字不動。** 它是本弧抄的模範（G2 是紅線）。
3. **`REAL_03` §6.6 第四款**（O86 (i)）與 **`.oo/peers` 的冷啟動**（需裁定）。
4. **`DiscoveryConfig` 洩裸 errno**（Q-044 殘留，已在 Inbox）。

---

## 4. 紅線

**R1 不得造新的 tag 值（D66 逐字）。** 差異記在 `%cause`。今天的答案域是
`#true`／`#false`／`#none`／內容 ⟹ **不得出現第五個**。

**R2 `%cause` 的拼法進位址，一旦出貨就改不動了。**
〔讀〕`BlurCause::as_bytes()` 餵給 `bn_serial.rs:147` 與 `value.rs:1904` 的 hasher
⟹ **`#blur` 的成因是 blur CAID 的一部分**。
**驗收方指定 `#unreadable`**，依據是**引擎已經在用這個字**
（v0.49.0 的 `StoreReadError::Unreadable`；`injection injections: unreadable`）⟹ 不新造詞彙。
**⚠ 若你認為該用別的字，在 §N.3 說，不要自己改**——這是出貨前唯一便宜的時機。

**R3 非紀元：不得移動任何既有位址。** 新的成因是**附加**（今天沒有任何值帶它）。
必須證明：標準根 `7038e250…`、`x: 0` 根 `31745ef0…`、物件 3、`layout=5`、`encoding=5` 皆不動，
且 conformance 162／162。

**R4 成功路徑的語義不變。** 可讀的檔仍 `#true`／內容；不存在仍 `#false`／`#none`（G1）。

**R5 探針不得改動，也不得被 `rustfmt` 掃到。** 認為探針寫錯 ⟹ §N.3 說明並保持它紅。

**R6 離開碼不得在管線之後取。**

---

## 5. 探針（驗收方所寫，基線 **2 綠 3 紅**）

| | 名稱 | 現況 | 釘什麼 |
| :-- | :-- | :-- | :-- |
| G1 | `g1_the_answerable_questions_keep_their_answers` | 綠 | **紅線 R4** |
| G2 | `g2_the_store_boundary_still_answers_the_way_it_already_did` | 綠 | **紅線 R2 的模範**，`_\|_ ;; %cause: #store_boundary` 不得變 |
| **R1** | `r1_exists_separates_absent_from_unreadable` | **紅** | I1，EACCES ≠ ENOENT 且帶 `%cause` |
| **R2** | `r2_a_broken_question_is_not_an_absent_file` | **紅** | I1，ENOTDIR／ELOOP ≠ ENOENT 且帶 `%cause` |
| **R3** | `r3_read_file_separates_absent_from_unreadable` | **紅** | I2 |

**探針比較的是去掉 `;; %effect: #io` 之後的答案**（那一段在每個答案上都一樣，不帶資訊）。
**每一支紅探針都在同一次執行內帶 ENOENT 對照組**，且夾具開頭有 root 守衛
（`chmod 000` 之後證明拒絕真的咬到，否則以 root 執行時全檔皆為空讀數）。

---

## 6. 請在交付報告裡回答（§N.4）

1. **`HorizonParams` 你填了什麼，為什麼？** `#blur` 今天攜帶一組**進位址**的預算參數
   （`fuel`／`max_branches`／`max_unification_depth`…），而 EACCES 與預算無關。
   〔驗收方已查〕**`BlurCause::MathSingularity` 是既有的非預算成員**，先看它怎麼做。
   **兩個相同的 EACCES 在不同 `%fuel` 下會不會得到不同的 CAID？** 明確回答。
2. **ENOTDIR／ELOOP 用哪個 `BottomCause`？** 既有的 `InvalidPath` 看起來就是；
   若你另造，說明為什麼既有的不夠。
3. **你的對映是全函數嗎？** 列出每一個 `io::ErrorKind` 落在上表哪一列，
   以及**沒有落點的那一類**回什麼。
4. **證明 R3**：哪些位址你量過沒有移動。
5. 任何你認為驗收方寫錯的地方（**包含 `#unreadable` 這個字**），§N.3 說，不要自己改。

---

## 7. 驗收會怎麼做

1. diff 純度、探針完整性（`git diff` 為空、`rustfmt` 未掃）。
2. **全工作區 ×3**，`--release --no-fail-fast`，逐標的聚合，**保留失敗的測試名**。
3. **驗收方會另造探針沒測過的 errno**（至少 `ENAMETOOLONG` 與一個目錄形），
   檢查 §6.3 的全函數宣稱。
4. **身分紅線與 conformance**，以真二進位、known-answer 帶對照。
5. **跨版本雙向**。
6. **規格結案由驗收方做**：`TAG_REGISTRY` 的登記是驗收方的事，**不要自己改規格**。

---

## N. 交付回報（交付方填；本行以上一字不得動）

### N.1 做了什麼

### N.2 量測（每一項都要有分母與對照組）

### N.3 我認為驗收方寫錯的地方（探針／射程／量測／`#unreadable` 這個字）

### N.4 §6 五問的回答

### N.5 規格側的發現（不要自己改）

### N.6 我沒做的事
