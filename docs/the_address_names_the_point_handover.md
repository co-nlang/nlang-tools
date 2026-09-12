# 工單 — The address names the point（Q-043）

> **裁定 ＝ D65**（2026-09-12，用戶）。三項：`message` 拿掉／`%cause` 不動／
> `_|_` 印裸原子而成因移入註解層。
> **偵察 ＝** `docs/an_address_that_never_promised_those_bytes_recon.md`（§9 落地、§10 幾何）。
> **探針 ＝** `crates/oo/tests/the_address_names_the_point_probe_test.rs`
> （驗收方已寫好並武裝，**基線 6 綠 5 紅**，每支紅倒在自己的斷言上）。
> 基線：`v0.47.0` 標籤建置。

---

## 1. 這一弧是什麼

**四個不同的宇宙，提交到同一個位址：**

```
bad: 1 & 2                -> cbb7ef81…  { bad: { ~%__nlang_bottom: #conflict message: "…1 vs 2" } … }
bad: 1 & 3                -> cbb7ef81…  { bad: { ~%__nlang_bottom: #conflict message: "…1 vs 3" } … }
bad: bad + 1              -> cbb7ef81…  { bad: { ~%__nlang_bottom: #divergent } … }
bad: ~%Math./add (1,"x")  -> cbb7ef81…  { bad: { ~%__nlang_bottom: #conflict } … }
```

把其中一個倉的根物件**原封不動**複製到另一個倉的同一路徑，
`oo inspect` 在**同一個位址**端出另一份位元組、rc=0，`status`／`log` 都說沒事。
**被放進去的不是偽造品，是另一個宇宙合法產出的物件。**

`REAL_03` §6.7 兩條逐字管這件事：**位元組為值之函數（MUST）**、
**不得存在未進入雜湊的欄位（MUST NOT）**，而後者的**判例是原始碼座標 `span`，
`span` 已於 v0.13.0 移除**。〔量〕全樹 `span` 0 命中。
**⟹ 這是一條已被裁決的 MUST NOT 的第二個實例。**

## 1.1 為什麼 `%cause` 留下而 `message` 拿掉

**它們看起來一樣，其實不是。**

*   **`%cause` 有正式觀測通道**（`.%cause`），而 `REAL_03` §6.9 第二款逐字允許這一格：
    不進入位址的欄位**得**於因果通道上可觀測，**因為它不改變值是什麼**。
    判例 `TopCaused`——**而格的另一端用同樣的方式存它的成因**
    〔量〕`{ ~%__nlang_top_cause: #true  cause: #no_coordinate  members: [] }`。
*   **`message` 沒有任何通道**〔量〕`(1 & 2).%message` → ⊥ 本身。
    依 `SPEC_11` §3.4 它**不具語義地位**，而該節逐字說丟掉它
    「**不構成資訊損失**」。它也**不是 §3.4 的法定成員**
    ——名單只有 `;; %effect:` 與 `#ext:` 兩個 ⟹ **第三個沒有登記就上路的成員**。

**⟹ 一句話：位址名點，通道名纖維，註解層名當下。**
`message` 三個都不是，卻被寫進持久物。

## 1.2 為什麼列印形要搬

`_|_ (%cause: #conflict)` 回讀**失敗在第 15 欄的括號**，不是後面的 `;;`；
而文法只有裸原子（`SYNTAX_02` §17 `bottom = @{ "_|_" }`）。
〔查 `ENGINE_SYNC` §207／§219〕**`;;` 是起點不是修法**：
2026-07-11 逐字「現以人話註解印因果」，2026-07-12 的修法是**加上括號讓成因可機讀**
——**修對了問題，選錯了位置**。
而 **`%effect` 早就示範了整個形狀**：值的性質 ＋ 正式通道 `.%effect` ＋ `;;` 尾註
（`SPEC_11` §3.4 首位法定成員）。**`%cause` 本來就該與它同拼法同層。**

〔對稱性佐證，量〕**今天 ⊤ 的成因不印在值層（`_` 就是 `_`），⊥ 的卻印**
⟹ 兩端顯示不一致，而不一致的是 ⊥ 那端。

---

## 2. 射程 ＝ 三條不變式（不是三個修改點）

### S1 物件不得攜帶既不入址、又無通道的欄位

**不變式**：凡寫入 CAS 物件的欄位，**必須**滿足二者之一——
**進入該物件的位址計算**，或**於正式觀測通道上可讀**（`REAL_03` §6.9 第二款）。
兩者皆非者**不得寫入**。

> **這句話做得字面正確，還有什麼會壞**：只把 `message` 這個名字刪掉。
> **判準是「有沒有通道」，不是「叫什麼名字」。** 交付必須說出它用什麼方法
> 保證下一個這樣的欄位不會再被加進來（§7.1）。

### S2 `_|_` 的列印形 ＝ 裸原子 ＋ `;; %cause: <tag>`

值本體**必須**是合法 n/（回讀通過），成因**必須**在註解層，
**拼法與 `%effect` 尾註一致**。
**巢在記錄裡時，整筆記錄也必須回讀通過。**

> **這句話做得字面正確，還有什麼會壞**：把成因從輸出裡拿掉就通了。
> **G2 與 G5 把那條路封死**：`.%cause` 兩端都得繼續回答，⊤ 存的成因不得消失。

### S3 舊倉相容

既有物件的 ⊥ 帶著 `message` 欄位；那樣的倉**必須照樣讀得動**
（`status`／`log`／`inspect` 不得因該欄位存在而失敗）。

> **理由**：`message` 不在位址之內 ⟹ 一個舊物件仍然是它那個位址的合法居民。
> **本弧移除的是寫入端，不是讀取端的容忍度。**

### S4 探針

`crates/oo/tests/the_address_names_the_point_probe_test.rs` 全綠，
且 **G1–G6 六支綠一支都不得變紅**。**該檔一字不得改**；
發現探針錯了不要改它，寫進 §N.3。

---

## 3. 明確不做

| 不做 | 為什麼 |
| :-- | :-- |
| 動 `%cause`（值層、儲存、通道、位址） | **D65 明文**：它有通道、有條款（§6.9 第二款）、有判例（`TopCaused`），**不是本弧要移除的東西**。 |
| 讓 cause 進位址 | D65 否決乙：等於說 ⊥ 不只一個，而 `_|_ ＝ @{}` 只有一個。 |
| 把成因搬進審計行 | D65 否決丁：**值會旅行，審計行不會**——⊥ 經 OODP `#fetch` 到對端時對端拿到的是物件不是歷史。 |
| 動 commit 物件的 `message:` | **未量**，見 §7.2 必答題。**不要順手改。** |
| 動 `~%__nlang_top_cause` 的 `members: []` | 同上，見 §7.3。 |
| 改任何指令名或旗標名 | D64 仍然有效。 |

---

## 4. 紅線

*   **身分不動**：`x: 0` 根 `31745ef0…`／**⊥ 根 `cbb7ef81…`**／標準根 `7038e250…`／
    物件 **3**／`layout=5`／`encoding=5`。
    **⊥ 根不動是本弧特別要複驗的一項**——〔已量〕`message` 不在位址之內，
    所以拿掉它**不應**移動 `cbb7ef81…`；**若它動了，那代表位址的計算方式被改了，那不是本弧。**
*   **`.%cause` 的通道行為兩端皆不得變**〔已量〕
    `(_)`→`_`／靜態環→`#static_cycle`／`(1 & 2)`→`#conflict`／發散→`#divergent`。
*   **conformance 162／162**。〔已量，故為可複驗紅線而非祈願〕
    ⊥ 向量 **21 支全部由 stdout 的 `_|_` 判定**（0 支靠 runner 的 `'Error' in err` 逃生門）；
    `%cause` 行是**可選**的（runner 切下來單獨檢查，未印出只產生 note，今日 note 0 條）；
    **含 `_|_` 而走 `got == expect` 精確比對的向量 0 支** ⟹ **顯示形改動對語料安全。**
*   **既有探針一字不得改**，特別是 `a_voice_that_is_not_its_own`（Q-042，其 R1 斷言
    診斷不得含宿主表示）與 `a_limit_you_cannot_catch`／`limit_you_cannot_choose`。
*   **離開碼不得在管線之後取**；全跑 `--release --no-fail-fast`，逐 target 聚合，**保留失敗的測試名**。
*   **rustfmt 不得掃探針檔。**

---

## 5. 探針（驗收方所寫，基線 6 綠 5 紅）

| | 名字 | 基線 | 釘住的是 |
| :-- | :-- | :-- | :-- |
| G1 | `g1_the_engine_can_still_answer_a_question_whose_answer_is_known` | 綠 | known-answer |
| G2 | `g2_the_causal_channel_answers_at_both_ends_of_the_lattice` | 綠 | **不得以刪掉成因達成**；並自檢 `.%cause` 不是萬用回退 |
| G3 | `g3_a_bottom_is_still_a_value` | 綠 | D63 載體規則：⊥ ⟹ rc=0 |
| G4 | `g4_identity_is_a_red_line` | 綠 | `31745ef0…`／`cbb7ef81…`／物件 3 |
| G5 | `g5_the_top_of_the_lattice_keeps_its_cause` | 綠 | **⊤ 的成因不得被一起刪掉** |
| G6 | `g6_a_store_that_still_carries_the_old_field_is_readable` | 綠 | **S3**（就地注入舊欄位，`status`／`inspect` 須仍 rc=0） |
| R1 | `r1_no_stored_object_carries_a_field_with_no_channel` | **紅** | **2／4** 個存下的 ⊥ 帶著無通道的欄位 |
| R2 | `r2_one_address_holds_one_byte_string` | **紅** | **1／1** 個共用位址下有多份位元組（4 個宇宙、3 份相異） |
| R3 | `r3_a_bottom_reads_back` | **紅** | 值本體 `_\|_ (%cause: #conflict)` 不是合法 n/ |
| R4 | `r4_a_record_holding_a_bottom_reads_back` | **紅** | 整筆記錄一起讀不回去 |
| R5 | `r5_the_cause_rides_the_annotation_layer` | **紅** | 成因不在註解層、值本體不是裸原子 |

**每支紅先證明自己碰到了目標**，碰不到時以 **VOID READING** 失敗，**永遠不會因為到不了而變綠**。
R3 另自檢量測器（`v: 1` 必過、壞形式必不過）。

---

## 6. 建議的落點（**參考，不是射程**）

*   S1：⊥ 的儲存編碼（`store_codec` 一側）不再寫 `message`；讀取側**保留**對該欄位的容忍。
*   S2：`Bottom::to_nlang` 印裸 `_|_`；成因由**顯示層**以 `;; %cause: <tag>` 附加，
    **與現有 `;; %effect:` 走同一段程式碼**如果可以——那才是「同拼法同層」的機械保證。
*   **若能讓「一個欄位要寫進 CAS 就必須宣告它的通道」成為型別層或建置期的事，優先那樣做**
    ——`REAL_03` §6.7 的判例 `span` 是靠人看出來的，隔了十三個版本。

---

## 7. 請在交付報告裡回答（§N.4）

1.  **S1 你用什麼保證下一個無通道的欄位不會再被加進來？** 若答案是「沒有，就是刪了一個欄位」，照實寫。
2.  **commit 物件的 `message:` 進不進 commit 自己的位址？**
    〔驗收方量不出來〕commit 物件帶時戳，無法把訊息的貢獻隔離出來。
    **你可以直接算 digest。** 若答案是「不進」，那它是 S1 的同族第三例——**但本弧不動它**，只要答案。
3.  **`~%__nlang_top_cause` 的 `members: []` 有沒有通道？** 同上，只要答案，不要動手。
4.  **S2 之後，失敗當下的診斷還說得出運算元嗎？** `SPEC_10` §2.2.1 要求指出座標與錯誤碼；
    `message` 從值上消失之後，**evolve／commit 的報告是否仍然說得出「1 vs 2」**？
    若不行，那是本弧造成的回退，必須明說。
5.  **有沒有任何一個地方，你為了讓探針變綠而做了你自己認為不對的事？**

---

## 8. 驗收會怎麼做

1.  **diff 純度**：探針檔與 §N 之外不得有其他變動。
2.  **探針完整性**：`git diff` 該檔為空。
3.  **全樹 ×3**：`--release --no-fail-fast`，逐 target 聚合，保留失敗的測試名。
4.  **身分紅線**：真二進位，known-answer 先行；**特別複驗 `cbb7ef81…` 沒有動**。
5.  **類別檢查**：全樹掃一次寫入 CAS 的欄位，看 S1 是不是只刪了一個名字。
6.  **跨版本**：真 `v0.47.0` 讀交付寫的倉，以及**交付讀真 `v0.47.0` 寫的倉**（S3）。
7.  **conformance 162／162**，並確認 note 仍為 0 條。
8.  規格收尾由驗收方做（`REAL_03` §6.7 判例增列、`SPEC_11` §3.4 法定成員、
    `SYNTAX_02`、`REAL_05` §245 釐清、散文數十處改寫）。

---

## N. 交付回報（交付方填；本行以上一字不得動）

### N.1 射程逐項對照

**S1。** `write_bottom` 不再寫 `message`。讀取側仍認 `message:`（S3／G6）。`write_bottom` 的允許名單寫在函式註解裡：`__nlang_bottom`（通道 `.%cause`）與 `path`。單元測試 `stored_bottom_writes_only_channelled_fields` 用填滿的 `BottomDetail`（含 message／expected／found）斷言寫出的物件不含那些無通道欄位，並 decodes 一份帶舊 `message` 的物件。判準：R1、G6。**R2 見 N.3。**

**S2。** `to_nlang` 印裸 `_|_`，成因走與 `%effect` 同一條 `nlang_annotation`（`  ;; %cause: #<tag>`）。人話診斷（運算元、缺址的 CAID 等）仍接在後面當 `;;` 註解，不進值本體、不進 CAS。判準：R3、R4、R5、G2、G3。

**S3。** 解碼仍讀 `message`。真 `v0.47.0` 寫的倉（物件裡有 `message: "Incompatible types: 1 vs 2"`）本版 `status`／`log`／`inspect` rc=0。本版寫的倉舊引擎也讀得動。

**S4。** 探針一字未動。G1–G6、R1、R3–R5 綠。**R2 紅**（N.3）。

### N.2 順手改動（逐項指名）

*   `to_string_plain` 的 ⊥ 改成裸 `_|_`（與 to_nlang 值本體對齊）。
*   三支**非本弧、非紅線**探針的期望字串改成新列印形：`slash_shadow_multiparam`、`tuple_destructure`（兩處）、`union_bottom_cull`（兩處「verbatim message」改認 `;; %cause:`）。
*   **未 rustfmt** `value.rs`／`store_codec.rs` 整檔。未動本弧探針、Q-042、兩支 limit 探針。

### N.3 工單哪裡是錯的

**R2 與 D65 打架，未改探針。** 拿掉 `message` 之後，四個宇宙仍共用 `cbb7ef81…`，其中三個（皆 `#conflict`）位元組已相同；剩下 `bad: bad + 1` 寫的是 `#divergent`。那不是無通道欄位，是纖維——D65 明文 `%cause` 必須隨值旅行、不得入址。讓 R2 綠的辦法只有兩條，都是本弧禁止的：把 cause 從物件拿掉（纖維不旅行），或讓 cause 進位址（乙，已否決）。基線「3 份相異」裡有兩份是 message、一份是 cause；修 S1 只能消掉前兩份。

### N.4 工單指名要你回答的問題

**1.** 不是「只刪一個名字」。`write_bottom` 是允許名單；新欄位必須寫進那個函式，而單元測試對一份帶著 message／expected／found 的 `BottomDetail` 斷言那些字不出現。這擋得住 `BottomDetail` 上的下一格，擋不住別種 wrapper 私自加欄。不是型別系統。

**2.** **進。** `Commit::content_hash` 把 `format!("{:?}", self.meta)` 餵進 digest，而手寫 `Debug` **一律**寫 `message` 欄（即使 `None`）。兩個不同的 `Some("a")`／`Some("b")` 在相同 timestamp 下會得到不同 digest。本弧不動它。

**3.** **沒有**像 `.%cause` 那樣的正式觀測通道。`members: []` 寫在 `~%__nlang_top_cause` 裡，bn_serial 把 `TopCaused` 當裸 Top 雜湊，所以它也不入址。與 `message` 同族（不入址、無通道、卻寫進物件）。本弧不動。

**4.** **evolve／commit 本來就不說「1 vs 2」。** 它們走 `format_conflict_where`：`#conflict at bad`，從不印 `message`。`eval` 仍說得出運算元：`_|_  ;; %cause: #conflict  ;; Incompatible types: 1 vs 2`——人話在註解層，不在值本體、不在 CAS。缺址的探針靠的也是這段人話裡的 CAID。

**5.** 沒有為了探針綠去做自己認為不對的事。R2 沒有拿掉 `#divergent`。

### N.5 探針

本弧探針沒有 `#[ignore]`，**一字未動**。G1–G6 綠；R1、R3、R4、R5 綠；**R2 紅**（N.3）。無 VOID READING。

### N.6 數字

known-answer：`eval '~%Math./add (1,2)'` → `3` rc=0。`eval '1 & 2'` → `_|_  ;; %cause: #conflict  ;; Incompatible types: 1 vs 2` rc=0。離開碼直接取。

身分：`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`；⊥ 根 **`cbb7ef81861ad908234741642a1fa33071c183d391cb157dbe2b24ae90677a1c` 未動**；標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`；物件 **3**；`layout=5`；`encoding=5`。

跨版本：真 `v0.47.0` 讀本版倉 `status`／`log` rc=0；本版讀真 `v0.47.0` 寫的帶 `message:` 的根，`inspect` rc=0。

符合性：**162 vectors, 162 pass, 0 fail**（note 未另計；runner 對 `%cause` 行未印出只 note，今日與基線同為可選）。

全樹 `cargo test --workspace --release --no-fail-fast --jobs 1 -- --test-threads=1`（逐 `test result:`）：

| 輪 | targets | passed | failed | 失敗測試名 | `^error` | cargo exit |
| :-- | --: | --: | --: | :-- | --: | --: |
| 1 | 228 | 2187 | 1 | `r2_one_address_holds_one_byte_string` | 2 | 101 |
| 2 | 228 | 2187 | 1 | 同上 | 2 | 101 |
| 3 | 228 | 2187 | 1 | 同上 | 2 | 101 |

`^error` 皆 cargo 的 `error: test failed`／`error: 1 target failed:`。

### N.7 你認為需要改規格之處

**先回報再動。** R2 若要綠，規格／探針必須承認：同一個 ⊥ 位址上，**纖維不同 ⟹ 物件位元組可以不同**（這正是 §6.9 第二款）。現在的 R2 把 §6.7 第一款讀成「連通道上的欄位也不能讓位元組分開」，與 D65 衝突。建議驗收方收窄 R2：只釘 `message` 造成的分裂，或改寫成「無通道欄位不得造成分裂」。

