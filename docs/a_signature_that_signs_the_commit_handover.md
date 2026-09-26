# 工單：簽的是這筆提交（Q-059）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-059／裁定 `meta/oo/STATUS.md` **D76**（甲／②甲／③甲）
> 探針（已預先提交並校準）`crates/oo/tests/a_signature_that_signs_the_commit_probe_test.rs`
> 夾具 `crates/oo/tests/fixtures/layout6_signed_repo/`（真 `oo v0.60.0` 所造：新式提交上的一個舊式簽章）；
> 沿用 `layout5_signed_repo/`（真 `oo v0.58.0`）
> 基線：dev `b0472d7`／`oo v0.60.0` ⟹ **5 綠 10 紅**，三輪一致、零空洞讀數。
> **另有 5 個檔共 8 支既有探針已由驗收方修訂**（§4 末），基線上刻意為紅。

## 1. 缺陷

`SPEC_10` §2.5 第 4 款逐字：「**簽署對象**：該 Commit 的 CAID（計算雜湊時排除 `%authority` 欄位本身）」。
引擎簽的是 `authority::compute_refine_payload`：`refine:` ＋ 排序後的來源 CAID（`|` 相接）＋ `:` ＋ 排序後的目標 CAID——**不綁這筆提交**。
〔量 2026-09-26，dev `b0472d7`，以值解碼器重鑄位址自洽的提交〕同一個簽章在下列三種情況下**仍驗得過，`oo log` 仍點名簽署者**：

*   **m1** 把寫者的字 `"unverified"` 改成 `"verified"` ⟹ `refine authority: <簽署者> (writer recorded: verified)`。
    **這一例最重**：D75 ② 把寫者的字並列在簽署者旁邊，而簽署者**從沒簽過那個字**。
*   **m2** 改掉提交訊息 ⟹ 簽署者照舊。
*   **m3** 把簽章搬進**另一個倉**一筆來源／目標相同、原本未簽的精煉 ⟹ 簽署者照舊。

另：`AuthorityInfo.timestamp` 簽署時在記憶體裡，`store_codec::write_refine` 從未寫它，讀回恆為 `None`（Q-057 交付 Q4）。

## 2. 裁定（D76，用戶 2026-09-26，三題皆選甲）

*   **甲**：一個**讀得過的舊式簽章**（只簽來源與目標）照它實際簽了什麼呈現——**與簽了整筆提交者可區分**。舊簽章不作廢（D75 延續）。
*   **②甲**：拿掉 `authority.timestamp`。簽署時間就是提交自己的 `meta.timestamp`，它**在被簽的值裡**。它從未落地 ⟹ 沒有位元組要動。
*   **③甲**：推進到 **`layout=7`**。整筆提交的簽章**只寫進 `layout=7` 的倉**；宣告較舊的倉**照舊收到舊式簽章**，直到被明示遷移。
    理由是既有 MUST NOT（`REAL_02` §5.1.1「宣告未推進者不得收到它宣告不了的東西」）：否則 v0.59／v0.60 開一個 `layout=6` 倉，
    會對一個真簽章說「did not verify」——**倉裡躺著造它的引擎驗證不了的東西**。**破壞性條目，90 天時鐘重啟。**

## 3. 釘死的簽署對象（這是互通的定義，探針照它驗）

```
V       = 這筆提交的值，其 refine.authority 欄位不存在——恰是同一筆提交若不簽時的本文
payload = "refine-commit:v1:" ++ CAID(V)       （CAID 用它的字串形；位址依 D74：store 的值解碼器 ＋ content_hash）
sig     = Ed25519(操作者私鑰, payload)
```

與 `REAL_02` §4.2.1 廣告簽章同形（`oodp-advert:v1:` ＋ 移除 `signature` 後的 `CAID(本體)`）。域前綴使新舊 payload **互不相交**
（舊的以 `refine:` 起頭）⟹ 讀者兩種都試，不需要任何標記欄位。舊式（只供讀取）：`"refine:" ++ 排序來源 "|" 相接 ++ ":" ++ 排序目標 "|" 相接`。

## 4. 射程＝不變式（不是機制）

*   **I1 寫**：在 `layout=7` 的倉，`refine --sign` 存下的簽章**依 §3 簽整筆提交**（r1）。
    **V 裡的每一個欄位在簽之前就必須定案**——包括 `authority_status`：它在 V 裡，所以它**不能**是「簽完再驗一次才決定」的結果。
    寫入當下對登記的判定照舊（成員資格＋密碼學），不在非空登記內者照舊被拒（Q-058 g2）。
*   **I2 讀**：一筆精煉的授權，**只呈現讀取當下驗得過的**，而且照它簽了什麼說：
    整筆提交的簽章 ⟹ 簽署者；舊式簽章 ⟹ 簽署者，**且看得出它只簽了來源與目標**（r6 新式提交、r7 舊式提交）；
    都驗不過 ⟹ 與前兩者不同（r2–r4）。一個你沒寫過、但依 §3 簽的簽章，也要驗得過（r5）——**第二個實作簽的東西，你要認得**。
    提交是新式還是舊式都一樣；舊式提交上存下的字仍不是事實（D74 ②），新式提交上寫者的字照 D75 ② 並列。
    **措辭你選**；探針比較的是**把公鑰與寫者的字抹掉之後的形**，所以可區分性**不得**只靠寫者的字。
*   **I3 宣告**：新倉宣告 `layout=7`。宣告較舊的倉：`refine --sign` 照舊寫舊式簽章、**不動宣告**，並**在輸出裡點名那個明示的遷移**（r10）。
    明示遷移把 `layout=6` 推進到新倉的宣告、不動 `HEAD`，遷移後讀得到舊簽章（r8）、之後的簽章簽整筆提交（r9）。
*   **I4 遷移的代價**：**對每一個起始狀態**（`layout=2`…`6` × 各編碼），代價句點名**遷移前打得開、遷移後打不開**的全部參考引擎，
    **最老與最新兩端都要對**。最新一端現在是 `v0.60.0`；`layout=6` 起始 ⟹ `v0.59.0` 到 `v0.60.0`（r8）。
    `main.rs` `migrate_cost` 今天把 `v0.58.0` 寫死、並寫死一句「every engine that opens layout=5 (oo v0.44.0 through v0.58.0)」；
    `first_engine_that_opens` 對 `layout=6` 回 `None`。**這是一整張表，不是加一列**（Q-055 R-1 的教訓）。
*   **I5**：`AuthorityInfo` 不再有 `timestamp`；每一處構造與讀取一併拿掉。沒有位元組要動。
*   **I6**：值的位址、既有提交的位址一個都不動；讀取不寫入、不鑄造。

## 5. 紅線（今天綠，必須保持綠）

*   **c1／c2** 兩個 oracle 各自兩極：依 §3 簽的被接受、改一個位元組即被拒；真 v0.60.0 的舊式簽章被接受、換目標即被拒。**不經引擎。**
*   **g1** 真簽章在 `refine` 與 `log` 裡仍點名簽署者；**g2** `layout6_signed_repo` 的舊式簽章仍點名簽署者；**g3** `layout5_signed_repo` 同。
*   Q-058 `a_verdict_only_a_signature_can_carry` 9 支、Q-057 `a_commit_that_is_a_value` 13 支、Q-055 `a_refusal_written_as_an_answer` 17 支；
    `identity_persistence`、`universe_determinism` 各支；conformance 162／162；`x: 0` 根 `31745ef0…`、`v: 1 + 1` 根 `f4f32e7b…`、標準根 `7038e250…`。
*   **驗收方修訂的 8 支（基線刻意為紅，屬本弧）**——`layout=6` → `layout=7`，代價的最新一端 `v0.58.0` → `v0.60.0`：
    `a_refusal_written_as_an_answer` r6／r7／r11（及 `names_the_boundary`）、`a_commit_that_is_a_value` r6、
    `nothing_here_or_nothing_you_can_see` g1、`a_commit_that_closes_the_door` r5 與 g1、`atomic_write` p2。
    基線上 8 支都只因 `layout=6`／`v0.58.0` 而紅（已逐支讀過失敗訊息）。

## 6. 必答（請在交付報告裡回答，逐題）

*   **Q1** V 在哪裡組出來？你怎麼保證簽完之後 V 裡沒有任何欄位再變（尤其 `authority_status`、`meta`）？
*   **Q2** 哪些讀取面會印出授權（`log`、`refine`，還有嗎）？三種情況（整筆／舊式／驗不過）各印什麼，逐字列出。
*   **Q3** 〔量〕以真 `v0.60.0`（`/home/gali/nlang-baselines/v0.60.0-verify/target/release/oo`）開一個本弧建出的新倉：
    `status`／`log` 的離開碼與逐字回覆。**打得開就是 I3 沒做到。**
*   **Q4** 寫入當下對非空登記的驗證，現在驗的是哪一個 payload？
*   **Q5** 全樹還有誰呼叫 `compute_refine_payload`（網路面、測試、conformance）？逐一列出，並說明各自該用哪一個形。

## 7. 明文不在射程內

*   `oo inspect` 對每一筆提交印 `parent: (none)`（Inbox 另一列）。
*   登記的歷史（寫入當時誰是架構師）——D75 已裁不宣稱。
*   `oo node discover` 寫目錄失敗不說（Inbox 另一列）。
*   舊式簽章的**重簽**或任何改寫既有物件的路——`REAL_02` §5.1.1 禁止。

## 8. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r10 基線必須是紅的，理由逐支寫在 doc comment；**你可以讓它們變綠，不得改寫它們的宣稱。**
*   **探針用到的儀器不准改**：`store_codec::FRAME`／`decode_value`／`content_hash`（鑄造與定址）、`oo identity`、`oo log`、`oo refine`、`oo inspect`
    的既有輸出結構、`.oo/format` 的形式。§3 的定義是釘死的——**若你認為某支的判準不可能被一個正確的實作滿足，寫在「工單哪裡是錯的」，不要改儀器去對上它。**
*   夾具 `layout6_signed_repo`、`layout5_signed_repo` 不得用現行引擎重建。`rustfmt` 不得掃探針檔。

---

## 9. 交付回報（交付方填；本行以上一字不得動）

### 9.1 射程逐項對照

### 9.2 順手改動（逐項指名）

### 9.3 工單哪裡是錯的

### 9.4 工單指名要你回答的問題

### 9.5 探針

### 9.6 數字

### 9.7 你認為需要改規格之處
