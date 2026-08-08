# W3′-a 交接:矛盾在哪

**開弧日**:2026-08-08
**基線**:`dev f0ecb21`(= `top 22b1957`,v0.11.1)
**來源**:`nlang-spec meta/oo/STATUS.md` W3′-a、D37–D39;`docs/discussion/029` §3.1
**性質**:**規格語義變更**(新增條文)＋ 引擎。與前一弧 W0′(純符合性、走 patch)不同,**本弧走 minor**。

---

## 0. 一句話

> **引擎算出了矛盾的葉層座標,然後在兩個型別邊界上把它丟掉,再把「你剛剛打的那個字」印出來當作答案。**

---

## 1. 缺陷

### 1.1 今天操作者看到的東西

工作區已提交 `app: { db: { host: "h", port: 5432, opts: { tls: true, retries: 3 } } }`,
接著 evolve 一份只有 `retries` 從 `3` 改成 `9` 的來源:

```
Error: Evolution Conflict in "u.n": Conflict at
  Path(Path { anchor: Bare, segments: ["app"], span: Span { start: 0, end: 3 } })
```

矛盾在 **`app.db.opts.retries`**——四層深、六個葉子裡的一個。訊息說的是 `["app"]`。

三件事同時錯:

1. **座標不是座標**,是 `f.key`——**你剛剛打的那個頂層欄位名**。見 R3:那不是「前綴不足」,是**拿錯了東西**——真正的座標另有其物,而且是絕對的。
2. **內部結構被 `{:?}` 印進操作者的臉**:`Path(Path { … span: Span { start: 0, end: 3 } })`。
   `Span` 是原始碼位元組偏移,是實作細節。
3. **兩個回報點給的資訊不一樣**:
   - `main.rs:338`(`oo evolve FILE`):印 `f.key`。
   - `main.rs:963`(REPL):`println!("Evolution Conflict: {:?}", e)`——**連欄位都沒有**。

### 1.2 而引擎其實算出來了

`unify.rs` 的 `unify_combo` 在回程逐層累積路徑:

```rust
let cp = detail.path.as_ref()
    .map(|p| format!("{}.{}", key, p))
    .unwrap_or_else(|| key.clone());
detail.path = Some(cp);
return Value::Bottom(detail);
```

**〔量 2026-08-08〕** 直接呼 `engine.unify` 合併 `{p:{q:{deep:1}}}` 與 `{p:{q:{deep:2}}}`:

```
cause   = Conflict
path    = Some("p.q.deep")          ← 精確到葉
message = Some("Incompatible types: Atom(Int(1), …) vs Atom(Int(2), …)")
```

**這是本弧最重要的一個量測。** 上一版的偵察結論寫「引擎從來沒算過」,**被這個量測推翻**——
它算了,而且算對了。

### 1.3 丟在哪兩個邊界

```
universe.rs:264   pub fn evolve(…) -> Result<(), BottomCause>
                  ↑ 整個 BottomDetail(含 path)被壓成一個無 payload 的 enum
```

```
Bottom::to_nlang  印 `_|_ (%cause: #conflict)  ;; message`,不印 path
```

第二個**不在本弧範圍**(見 §3.3)。本弧只治第一個。

### 1.4 規格側

**evolve 邊界的回報在規格裡沒有家。** SPEC_10 §4.1.2「回報的內容(MUST)」只治**提交**;
ERROR_CODES 的 `#conflict` 只給人「檢查兩個值是否互斥」的建議,沒有對引擎的要求。

⟹ 依 **D39**,本弧要**新增條文**,故是語義變更。條文由**驗收方**在收尾時寫,交付方不動規格。

---

## 2. 裁定

| # | 裁定 |
| :-- | :--- |
| **R1** | **座標必須到葉**(D38)。`app.db.opts.retries`,不是 `app`。 |
| **R2** | **操作者面的座標必須以 n/ 的拼法表達**,不得是 Rust 結構的 `{:?}`。**不得出現 `Span`、位元組偏移、或任何 `Xxx(Xxx { … })` 形狀。** |
| **R3** | **座標直接用 `detail.path`,不得再加 `f.key` 前綴。** 〔量 2026-08-08〕evolve 的合併是**整個 staged × 整個 incoming**,故 `unify_combo` 在最外層就已補上欄位名:深層案例得 `Some("app.db.opts.retries")`、淺層案例得 `Some("x")`,**兩者都已是絕對座標**。**本條的第一版寫「= f.key ＋ path」,實測推翻**——那樣會印出 `app.app.db.opts.retries`。今天印的 `f.key` 不是前綴不足,是**拿錯了東西**。 |
| **R4** | **兩個回報點必須給同樣的資訊**。REPL 不是次等公民。 |
| **R5** | **拒絕語義不變**:evolve 仍然 exit 1、仍然不寫 `staged`。本弧只治**它說了什麼**,不治**它做了什麼**。 |
| **R6** | **D36 的靜默不得被破壞**:沒有衝突的 evolve 仍然**逐字零輸出**。 |

---

## 3. 要求交付的內容

### 3.1 引擎

1. `Universe::evolve` 的錯誤型別**必須**帶得動 `BottomDetail`(或至少 `cause` ＋ `path`)。
   `BottomCause` 是無 payload 的 enum,壓在那裡的東西救不回來。
   **丟棄點就是這兩行**,`d` 在手上而只取了 `.cause`:
   ```
   universe.rs:396   Value::Bottom(d) => Err(d.cause),
   universe.rs:495   Value::Bottom(d) => Err(d.cause),
   ```
2. `main.rs:338` 與 `main.rs:963` 兩處**都**印出完整座標(R3),形狀一致。
3. 停止 `{:?}` 印任何內部結構(R2)。
4. `path` 為 `None` 時(見 §4)座標退回 `f.key` 單獨一項——**那是全篇唯一該用到 `f.key` 的地方**。
   **且不得印出空的或殘缺的座標**(例如結尾多一個點)。

### 3.2 明確**不在**本弧範圍

| 不做 | 為什麼 |
| :-- | :--- |
| **`Bottom::to_nlang` 印 path** | `to_nlang` 是**值的正準文字形**,動它會碰 **fmt v2 凍結**(v0.2.0 分水嶺承諾)與 conformance 向量。診斷訊息與正準形是兩件事。**另開帳** |
| 「**哪兩個來源**」 | O33 剩下的那一半,需要 meet 保留出處。本弧只答「在哪」,不答「誰跟誰」 |
| 提交時報告 ⊥ 座標 | **W3′-b,阻塞於 W12**。§4.1.2 的射程是並發路徑,而 commit 不重讀 HEAD ⟹ 那個場合到不了 |
| 改變 evolve 的拒絕行為 | R5 |
| `message` 裡的 `Atom(Int(1), EffectTag(0), None)` | 那也是 `{:?}`,但它在**訊息**不在**座標**。**若順手改掉是加分,但不列為要求**——本弧的論旨是座標 |

---

## 4. 可滿足性檢查

| 要求 | 可滿足嗎 |
| :-- | :--- |
| 取得葉層座標 | ✅ **已實測**:`path = Some("p.q.deep")`。不是推論 |
| 座標永遠存在 | ⚠ **兩個實測案例都有**(`"app.db.opts.retries"` / `"x"`),因為 evolve 的合併恆為 Combo × Combo,最外層必補鍵名。**但 `universe.rs:396` 那條路徑的 `incoming` 未必是 Combo**,`path: None` 在型別上仍可達而我**未能造出**。⟹ R4 維持為**防禦性**探針:`None` 時不得印出殘缺座標(結尾的 `.`、空字串)。**「未能造出」記在此處,不假裝它不存在** |
| 兩處回報點都拿得到 `f.key` | ✅ 兩處都在 `for f in &program.fields` 之內 |
| 不破壞 D36 的靜默 | ✅ 成功路徑不經過這些分支;仍以探針釘住 |

---

## 5. 探針(驗收方所有)

新檔 `crates/oo/tests/where_the_conflict_is_probe_test.rs`。

| # | 內容 |
| :-- | :--- |
| **C1** | **control**:一次**淺層**衝突(頂層 `x`)今天就會被回報且 exit 1。若 C1 掛,底下的紅可能只是因為回報整條壞了 |
| **C2** | **control**:D36 的反向——一次成功的 evolve **逐字零輸出**、exit 0 |
| **R1** | 深層衝突(`app.db.opts.retries`)的訊息**必須含 `app.db.opts.retries`** |
| **R2** | 該訊息**不得**含 `Span`、`start:`、`Path(Path`、`segments:` |
| **R3** | **REPL 路徑**的衝突訊息也必須含完整座標 |
| **R4** | 頂層直接衝突(`x: 1` vs `x: 2`)訊息含 `x`,且**不得**以 `.` 結尾或含 `..`(防禦性——見 §4) |
| **P1** | 衝突時 exit code 仍為 **1** |
| **P2** | 衝突時 **`staged` 仍未被寫出**(`oo commit` 仍回「Nothing to commit」) |
| **P3** | 成功的 evolve 仍然零輸出(與 C2 同,但列為釘:交付不得為了「一致」而開始在成功時說話) |

**校準**:R1–R4 在 `dev f0ecb21` 上必須全紅**且紅在對的理由上**;C1、C2、P1–P3 必須全綠。
紅的以 `#[ignore]` 標記,交付方**只得移除 `#[ignore]`**,不得編輯探針。

### 5.1 校準紀錄(2026-08-08,`dev f0ecb21`)

**C1、C2、P1–P3 五支全綠;R1–R4 四支全紅,每一支紅在它自己名字說的理由上。**

| 探針 | 基線實測 |
| :-- | :--- |
| **R1** | `Evolution Conflict in "u.n": Conflict at Path(Path { anchor: Bare, segments: ["app"], … })`——訊息不含 `app.db.opts.retries` |
| **R2** | 五個碎片全中:`segments:` / `Span` / `start:` / `Path(Path` / `anchor:` |
| **R3** | REPL 逐字全文 = `Evolution Conflict: Conflict`。**沒有座標,沒有欄位** |
| **R4** | 淺層案例同樣洩漏五個碎片(`segments: ["x"], span: Span { start: 0, end: 1 }`) |

**workspace 基線:1777 passed / 0 failed / 7 ignored,183 個區塊**(= v0.11.1 的 1772/0/3
加上本檔的 5 綠 4 紅;算術對得上,證明沒有別的東西被動到)。

---

## 6. 驗收量測

1. **diff 純度**:`universe.rs`、`main.rs`,以及必要的錯誤型別定義處。
2. **獨立重跑**:整個 workspace,不只本套件。
3. **重複穩定**:候選樹連跑五次。
4. **對抗**:座標含**引號鍵**、**數字鍵**、以及**含點的鍵名**時,印出來的座標仍須可讀且不歧義
   ——若鍵名本身含 `.`,點分隔法會說謊。**這一項可能會逼出一個裁定。**
5. **跨版本**:v0.11.1 建立的倉庫,交付版讀之行為不變(本弧不動耐久格式,故此為〔讀〕)。

---

## 7. 掛帳(本弧開出,不在本弧解)

| 內容 | 為什麼 |
| :-- | :--- |
| `Bottom::to_nlang` 是否該印 path | 碰 fmt v2 凍結與 conformance 向量,需獨立裁定 |
| `message` 裡的 `Atom(Int(1), EffectTag(0), None)` 是 Rust Debug | 同類病,不同位置 |
| 鍵名含 `.` 時點分隔座標會說謊 | 由 §6.4 的對抗量測決定要不要現在治 |
| O33 後半「哪兩個來源」 | 需要 meet 保留出處 |

---

## 8. 交付紀錄(交付方填)

*(留白)*
