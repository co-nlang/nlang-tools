# 工單：只有簽章承載得了的裁決（Q-058）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-058／裁定 `meta/oo/STATUS.md` **D75**
> 探針（已預先提交並校準）`crates/oo/tests/a_verdict_only_a_signature_can_carry_probe_test.rs`
> 夾具 `crates/oo/tests/fixtures/layout5_signed_repo/`（真 `oo v0.58.0` 所造，含一筆有簽章的舊式精煉）
> 基線：dev `0d50fef`／`oo v0.59.0` ⟹ **4 綠 4 紅**，三輪一致、零空洞讀數。

## 1. 缺陷

精煉提交帶一個字：`authority_status: "verified"` 或 `"unverified"`，**由寫這筆提交的人寫下**。
v0.59.0（D74）之後這個字在新式提交的位址內，所以**事後改不了**；但任何能寫 `.oo` 的人都能**鑄一筆全新、位址自洽**
而那個字寫著 `verified` 的提交——**雜湊擋得住竄改，擋不住偽造**。〔量，v0.59.0，探針 r1〕把一筆真的未簽精煉本文的
`"unverified"` 改成 `"verified"`、以值解碼器重算位址、寫入並指 `HEAD`：`oo log` rc=0，逐字 `refine authority: verified`。

而真正可以查驗的東西**已經在磁碟上**：`--sign` 的精煉存了簽署者公鑰與 Ed25519 簽章（探針 c1），
簽的 payload（`compute_refine_payload`：排序後的來源與目標 CAID 全字串）完全由位址內的欄位決定。

## 2. 裁定

**D75（用戶選「甲 只信簽章」）**：讀者呈現的授權**只有**讀取當下以存下的簽章對 payload 重驗的結果：
驗得過 ⟹ 說出**是誰簽的**（公鑰）；存下的那個字**不呈現**。
「寫入當時簽署者是不是架構師」是讀者驗不回來的歷史事實 ⟹ **不宣稱**。
舊式提交（D74 ②）上，重驗一個存下的簽章是**讀者自己算出來的新事實**，不是那個未受擔保的字 ⟹ **可以呈現**。

## 3. 射程＝不變式（不是機制）

*   **I1** 讀取面（`oo log`，以及任何印出一筆既有精煉之授權的地方）**不得**把存下的 `authority_status` 當成事實呈現；
    **沒有簽章、或簽章驗不過**的精煉，不得呈現為已驗證（r1、r3）。
*   **I2** 簽章驗得過 ⟹ 呈現簽署者的公鑰（r2、r4）；驗不過 ⟹ 呈現得**與驗得過不同**（r3）。**措辭你選。**
*   **I3** 新舊提交一律如此：舊式提交的簽章同樣在讀取時重驗（r4），而舊式提交上存下的字仍不是事實（g1，D74 ②）。
*   **I4** 寫入時的行為不動：`oo refine --sign` 在寫入當下對**當時的**登記驗成員資格，那是一個活的事實，
    它的輸出**可以**繼續說 `verified`／`unverified`（既有探針 `identity_persistence` R3／P2／P3 釘著它），**但要點名簽署者**（r2）；
    不在非空登記內的簽署者照舊被拒（g2）。
*   **I5** 讀取不得寫入、不得鑄造、不得碰任何位址；重驗只用存下的位元組。

## 4. 紅線（探針已釘，今天綠）

*   **c1** 有簽章的精煉存下簽署者與簽章；**c2** 鑄出的自洽提交打得開、讀得到——r1／r3 的量測碰得到目標。
*   **g1** 舊式提交上被改成 `verified` 的字仍不以事實呈現（D74 ②）。
*   **g2** 不在非空登記內的簽署者，`refine --sign` 照舊被拒。
*   Q-057 的 `a_commit_that_is_a_value` 13 支、Q-055 的 17 支；`identity_persistence`、`universe_determinism` 既有各支；
    conformance 162／162；值與提交的位址一個都不動。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1**（請在交付報告裡回答）你把重驗放在哪裡？有哪些讀取面會印出授權（`log`、`refine` 之外還有嗎）？逐一列出。
*   **Q2**（請在交付報告裡回答）payload 只綁來源與目標的集合，**不綁這筆提交本身**（父、根、訊息）——
    一個有效簽章可以被抄進另一筆來源／目標相同的提交而照樣驗得過。你的呈現有沒有讓讀者誤以為「這筆提交」被簽了？
    **這是事實陳述題，不要修**；修它是另一張卡。
*   **Q3**（請在交付報告裡回答）`authority.timestamp` 從未落地（Inbox）——本弧的呈現用不用得到它？用不到就別碰。

## 6. 明文不在射程內

*   **payload 不綁提交**（Q2）與**簽章重放**。
*   **架構師登記的歷史**（D75 候選丁：把當時的登記快照入址）——未採。
*   `authority.timestamp` 的去留（另一列 Inbox）。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r4 基線必須是紅的，理由逐支寫在 doc comment；**你可以讓它們變綠，不得改寫它們的宣稱。**
*   **探針用到的儀器也不准改**：`store_codec::FRAME`／`decode_value`／`content_hash`（鑄造與定址）、`oo identity`、`oo log`、`oo refine`
    的既有輸出結構。**若你認為某支的判準不可能被一個正確的實作滿足，寫在「工單哪裡是錯的」，不要改儀器去對上它**（Q-057 的教訓）。
*   夾具 `layout5_signed_repo` 不得用現行引擎重建。`rustfmt` 不得掃這個檔。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照
S1 I1：`oo log` 不印存下的 `authority_status`。沒有簽章，或簽章驗不過，都不印 `refine authority: verified`。驗：r1、r3。
S2 I2：簽章對來源／目標 payload 驗得過，那一行是簽署者公鑰。驗不過則印 `refine authority: signature did not verify`，與驗得過不同。驗：r2、r3、r4。
S3 I3：舊式提交的簽章同樣在讀取時重驗，驗過就印公鑰（r4）。沒有簽章的舊式提交仍印 `unattested`，被改成 `verified` 的字不呈現（g1；Q-057 r5）。
S4 I4：`oo refine --sign` 仍印寫入當下的 `verified`／`unverified`，並多一行 `Refine signer:` 點名公鑰（r2）。不在非空登記內的簽署者仍被拒（g2）。`identity_persistence` 的 R3／P2／P3 仍綠。
S5 I5：重驗只解存下的公鑰、簽章與來源／目標，不算位址、不寫磁碟。

### 8.2 順手改動（逐項指名）
`cargo fmt` 未跑。`bn_serial` 未改。探針檔未改。
`oo refine` 在原有的 `Refine authority:` 那一行之外，多印 `Refine signer: <公鑰>`。寫入當下的 `verified`／`unverified` 沒改。

### 8.3 工單哪裡是錯的
無。

### 8.4 工單指名要你回答的問題
Q1. 重驗在 `authority::signature_signer`：用存下的來源、目標組 payload，只做 Ed25519，不查登記。讀取面裡印授權的只有 `oo log`，它呼叫這個函式。`oo inspect` 不印授權。`oo refine` 印的是剛寫入那一次的檢查結果（`verified`／`unverified`）加上簽署者，不是把一筆舊提交裡存下的字再讀出來當事實。

Q2. 會。`oo log` 把那一行放在這一筆提交下面，讀者可以把它讀成「這筆提交是這把鑰匙簽的」。實際對上的只有來源與目標的集合。父、根、訊息都不在 payload 裡。把同一組簽章抄進另一筆來源／目標相同的提交，仍驗得過。本弧不修。

Q3. 用不到。`authority.timestamp` 沒讀、沒寫。

### 8.5 探針
沒有 `#[ignore]` 可拿。本弧探針沒改。8 支皆綠。無 `VOID READING`。

### 8.6 數字
全跑三輪相同：`cargo test --workspace --release --no-fail-fast --jobs 1 -- --test-threads=1`。逐 `test result:` 聚合：242 行，2300 passed，0 failed。`^error` 0 行。cargo exit 0。沒有失敗測試名。
conformance：162 vectors，162 pass，0 fail。
身分：`add (1, 2)` → `3` rc=0；`add (1, 3)` → `4` rc=0。`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。

### 8.7 你認為需要改規格之處
無。Q2 那條是給下一張卡的，這裡不動規格。

---

## 9. 驗收（驗收方填）
