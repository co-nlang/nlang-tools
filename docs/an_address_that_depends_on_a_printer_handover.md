# 工單 Q-052：一個取決於印表機的位址

> **本弧不需要裁定。** 偵察 `docs/an_address_that_depends_on_a_printer_recon.md`
> 探針 `crates/oo/tests/an_address_that_depends_on_a_printer_probe_test.rs`
> **基線 4 綠 0 紅，已校準——刻意的，先讀 §5。**

---

## 1. 一句主張

〔讀 `bn_serial.rs:152`〕`Thunk` 酬載第一項是 `encode_string(&expr.to_nlang(0), buf)`
——**位址對一個 pretty-printer 的輸出算雜湊**。
〔讀 `SYNTAX_02` §4.12〕該款只要求輸出**讀得回**且**讀回同一個值**；
〔查〕**規格全文沒有任何一條規定那串字的逐字形** ⟹ 印成 `1+1` 的實作完全合規，而它算出不同的位址。

**而這可以正面示範**〔量，`v0.55.0`〕`v: 1 + 1` 在倉裡逐字是
`{ v: { ~%__nlang_thunk: #true  ~%__nlang_expr: 1 + 1  ~%__nlang_closure: [] } … }`，
而 `v: 1 + 1`／`v: 1+1`／`v: (1 + 1)` **三者同根 `5ab20106…`**
⟹ **位址不取決於你怎麼寫，取決於 `to_nlang` 選了哪些空白與括號。**

## 2. ⚠ 本弧不是在修一個碰撞，是在移除一個依賴

〔量〕`to_nlang` 對結合性**是忠實的**：`v: (1 + 1) + 1` → `20d63171…`（存 `1 + 1 + 1`），
`v: 1 + (1 + 1)` → `2424e668…`（存 `1 + (1 + 1)`）⟹ **沒有兩棵樹被印成同一串字。**
**⟹ 不要去找碰撞，這裡沒有。** 本弧要拿掉的是「身分經過印表機」這件事本身。

**與 `Code` 那一格的差別**（它改變了理由）：`Code` 的病是**兩種呈現**（耐久 `to_nlang`／位址 `Debug`）；
而〔讀 `store_codec.rs:571`〕`Thunk` 的耐久 `EXPR` 欄**也是** `to_nlang`
⟹ **兩者一致，病只剩「那個呈現沒有被規範」。**

## 3. 射程 ＝ 兩條不變式

**I1　`Thunk` 的位址必須是它語法樹的一個被規範下來的函數，不得經過任何 pretty-printer。**
**I2　編碼必須忠實於語法樹**：源碼的空白與冗餘括號**不得**進入位址（G2）；
而**結合性必須**進入（G3）。

## 4. 修法：甲。乙明文否決，而理由是結構性的

*   **甲　身分改用 v0.54.0 已規範的 Expr 節點表**（`REAL_03` §6.2，36 列）：
    把 `encode_string(&expr.to_nlang(0), buf)` 換成 `encode_expr(expr, buf)`。
    **模板現成，這是本弧最便宜的一點。**
*   **乙　把 `to_nlang` 的逐字輸出規範下來**（空白／括號／欄序逐一釘死）。
    **否決，理由不是偏好**：那會讓**每一個排版決定永久成為 Layer 1 破壞性變更**
    ——換一個空白就是換所有 Thunk 的位址。**一個語言不該把排版鎖進身分。**
    **⚠ 若你認為甲做不到而必須走乙，寫在 N.3，不要默默走。**

**耐久形（`write_thunk` 的 `EXPR`）不必改**：它是倉的寫法。
改完之後 `Thunk` 與 `Code` 同形——**耐久是源碼、位址是節點表，第二個實作重解析再編碼即可重算。**

## 5. ⚠ 基線全綠，這是刻意的，代價在這裡

缺陷是**跨實作的可重算性**，**一個二進位無法表現它**（本實作的印表機當然自我一致）。
⟹ **四支探針全是紅線，而四支全綠不構成「你修好了任何東西」的證據。**

**補償要求（代替紅探針，缺一不可）**：

1.  **修前／修後位址**：對一組固定的 Thunk（至少含一個中綴算術、一個右結合括號形、
    一個帶非空 `closure` 的、一個帶 `context` 的）報出兩版位址。
    **修後應全部不同（編碼換了），而等價類必須不變**（G2／G3 的關係）。
2.  **紀元範圍要量出來寫**，至少三個分母（conformance／引擎測試 `.n`／`examples/`）：
    有多少檔會在倉裡留下 Thunk。**⚠ 別數「含中綴算術的檔」**——
    〔驗收方粗量〕conformance 162 檔中含中綴算術者 36 檔，
    **但 conformance 釘的是求值不是儲存 CAID**，那個數字不是紀元。
3.  **標準根含不含 Thunk，要量**（Q-049 的教訓：驗收方曾跳過這一步而推錯了結論）。
4.  **無法規範之處要寫出來**，不要繞過。

## 6. 紅線（探針已釘，四支皆綠）

*   **G1** 對照：不同式子不同位址。
*   **G2** **源碼拼法不得進入位址**：`1 + 1`／`1+1`／`(1 + 1)` 三者同址。
*   **G3** **結合性必須進入位址**：`(1+1)+1` ≠ `1+(1+1)`，且裸鏈 ＝ 左結合形。
    **一個把鏈扁平化的編碼會讓這支紅，而 G1／G2 仍綠。**
*   **G4** `x: 0` 根 `31745ef0…` 與標準根 `7038e250…` 不動 ⟹ **紀元界在「真的含 Thunk 的宇宙」**。
*   conformance **162／162**；全樹 ×3 `--release --no-fail-fast`，**逐 target 聚合並保留失敗的測試名**。

## 7. 明確不做（各有各的家，已入 Inbox）

*   **`=` 對 Thunk 強制求值（外延）而位址是內涵的** ⟹〔量〕`((1+1) = 2)` `#true`、
    `((1+1) = ~%Math./add (1,1))` `#true`，而 `(1+1)+1` 與 `1+(1+1)` 的根不同
    ⟹ **`=` 判相等的兩個東西位址不同**。**不違反任何已寫下的條款**，與 α-等價同族，**待裁**。
*   **`v: 1 + 1` 留成 Thunk，而 `v: ~%Math./add (1,1)` 被換成結果**
    〔量〕後者的根 `b8f0fe91…` **與 `v: 2` 逐字相同** ⟹ D46 ① 只有一部分落地？與 O79／D46 同族。
*   **O88**：`Float`／`Complex` 原子的定點編碼有損，須裁定。

## 8. 指名要你回答的問題（N.4 逐題作答，答案不利也照寫）

**Q1　`%id`（`hash_recursive_with_salt`）的 Thunk 臂用的是什麼？**
Q-050 在那裡找到過**第二個 Debug 出口**，不改的話 I1 只修一半。**去讀，並回報。**

**Q2　`closure` 幀與 `context` 的編碼有同族問題嗎？**
它們走 `serialize_combo`／`serialize_value`，**看起來沒有，但驗收方沒有逐項確認**。

**Q3　你的修補之後，`Thunk` 與 `Code` 的酬載是否共用同一個 `encode_expr`？**
若不是，為什麼——兩處各有一份會是下一個分歧的種子。

---

## N. 交付回報（交付方填；本行以上一字不得動）

### N.1 射程逐項對照

**I1**　`Value::Thunk` 的倉位址第一項由 `encode_string(&expr.to_nlang(0))` 改為
`encode_expr(expr)`（v0.54.0 那張 Expr 節點表）。`%id` 的 Thunk 臂同步（Q1）。
`store_codec::write_thunk` 的 `EXPR` **未改**（仍是 `to_nlang`）——耐久是源碼、位址是節點表。
G1–G3 關係仍綠；修後絕對位址全變。

**I2**　G2：`1 + 1`／`1+1`／`(1 + 1)` 修後仍同址 `f4f32e7b…`。
G3：`(1+1)+1` `2af73e1c…` ≠ `1+(1+1)` `3e166870…`，裸鏈＝左結合。
未扁平化、未把空白／冗餘括號寫進位址。

### N.2 順手改動（逐項指名）

1. `value.rs` `hash_recursive_with_salt` 的 Thunk 臂：由 `to_nlang` 改 `encode_expr`
   （Q1：不改則 I1 只修一半）。
2. O42 探針 `snapshot_not_a_reading_probe_test.rs` **p4**：unify-side `#blur` 的
   `node_content` 是欄位 Thunk 組成的 combo，CHS 隨 Thunk 編碼移動。
   `d0967392…` → `41ec191e…`。本弧探針未動。
3. **未改** Q-038 的 `pre_sentinel_repo` 夾具（v0.20.0 JSON 根，內含 `Thunk{expr: Atom(1)}`）。
   改它會毀掉「前哨兵倉」這件文物。見 N.6。

未 `rustfmt`。`thunk_cycle_id`／force memo 仍雜湊 `to_nlang`（不是倉位址，見 Q1）。

### N.3 工單哪裡是錯的

無。甲做得到，沒走乙。

### N.4 Q1–Q3 逐題作答

**Q1　`%id` 的 Thunk 臂用的是什麼？**

修前是 `hasher.update(expr.to_nlang(0).as_bytes())`，與倉位址同一個 pretty-printer。
已改成 `encode_expr`，與 `bn_serial` 同一份位元組（外層仍是 `%id` 的 `0x05` 前綴，
不是 `TAG_THUNK`）。

另外兩處仍走 `to_nlang`，**不是位址**：`lib.rs` `thunk_cycle_id`、`force` memo 的
`expr_caid`。它們是 force／in_flight 的行程內鍵。本弧 I1 是倉／`%id`。沒動它們。

**Q2　`closure` 與 `context` 有同族問題嗎？**

沒有。〔讀〕幀是 `serialize_combo`（§6.2 Combo／Cocoon），context 是
`0x00`（`#open`）或 `0x01` + `serialize_value`。都不經過印表機。
〔量〕`g: { x: 1  y: x + 1 }` 倉裡 `y` 的 closure 非空（一幀，內含 `x`／`y` 的欄位 Thunk）；
根層 `v: 1 + 1` 的 closure 是 `[]`、context 缺席（`#open`）。
表層 `evolve` 鑄不出 `context=Some` 的宇宙（`v: $ + 1` 是 ⊥ `#no_context`，不是 Thunk）。
那是鑄造缺口，不是編碼缺口。

**Q3　Thunk 與 Code 是否共用 `encode_expr`？**

是。兩處都呼叫 `bn_serial::encode_expr`。沒有第二份表。

### N.5 探針

無。G1–G4 一字未動。四支仍綠——§5 已說這不構成修好的證據。

### N.6 數字

**修前／修後根位址**（`evolve`+`commit`，digest 前綴；耐久 `EXPR` 仍是 `to_nlang`）：

| 源碼 | 修前 | 修後 | 等價類 |
| :-- | :-- | :-- | :-- |
| `v: 1 + 1` | `5ab20106…` | `f4f32e7b…` | G2 三者同址 |
| `v: 1+1` | `5ab20106…` | `f4f32e7b…` | 同上 |
| `v: (1 + 1)` | `5ab20106…` | `f4f32e7b…` | 同上 |
| `v: (1 + 1) + 1` | `20d63171…` | `2af73e1c…` | G3 左結合 |
| `v: 1 + (1 + 1)` | `2424e668…` | `3e166870…` | G3 右結合，≠左 |
| `g: { x: 1  y: x + 1 }`（非空 closure） | `64dc0f91…` | `849a2d5c…` | — |
| `x: 0`（無 Thunk） | `31745ef0…` | `31745ef0…` | G4 不動 |

`context=Some`：表層 evolve 鑄不出來（見 Q2）。編碼路徑未改。

**紀元範圍**（對每個 `.n` 單獨 `evolve`+`commit`，物件裡出現 `__nlang_thunk` 才算；
不是「含中綴算術的檔」）：

| 分母 | 檔數 | 倉裡留下 Thunk | 未能 evolve/commit |
| :-- | --: | --: | --: |
| conformance `.n` | **162** | **44** | 0 |
| 引擎測試 `.n` | **60** | **22** | 3 |
| `examples/` | **1** | **1** | 0 |

**標準根含不含 Thunk**〔量，`x: 0` 提交後讀 `7038e250…` 物件 16,029 B〕：
`__nlang_thunk`／`__nlang_expr`／`__nlang_closure`／`__nlang_code`／`ExprKind` **各 0 次**。
標準根不動。

探針 **4／4**。符合性 **162／162**。
身分：`~%Math./add (1,2)` → `3`（對照 `(1,3)` → `4`）。

全樹 ×3（`--release --no-fail-fast --jobs 1 -- --test-threads=1`，逐 target 聚合 `test result:`）：

| 輪 | targets | passed | failed | 失敗測試名 | `^error` | cargo exit |
| :-- | --: | --: | --: | :-- | --: | --: |
| 1／2／3 | 237 | 2243 | **3** | 下表 | 2 | 101 |

三輪失敗名相同：

*   `r1_a_pre_sentinel_repo_stays_openable_after_a_commit`
*   `r3_a_root_written_into_a_pre_sentinel_repo_stays_self_contained`
*   `r5_a_granted_migrate_moves_the_container_and_not_the_root`

皆 `a_commit_that_closes_the_door_probe_test`（Q-038）。夾具是 v0.20.0 的 JSON 根
`16ba5683…`，內含 `Thunk{expr: Atom(Int 1)}`。新引擎重算得 `cef5e484…` → `#caid_mismatch`。
**這是 Thunk 紀元打在歷史倉上，不是本弧探針。** 未改夾具（N.2）。
`^error` 2 行是 cargo 包「test failed／1 target failed」。

p4 針改後那一支綠。本弧探針未在失敗名裡。
