# W8′-a 交接:印出來的東西要能被讀回去

**開弧日**:2026-08-09
**基線**:`dev 6e8beee`(= `top 0328319`,v0.12.0)。workspace **1781 / 0 / 3**
**來源**:`nlang-spec meta/oo/STATUS.md` §2.3.1 W8′ 偵察 M4；§6「現建議」
**性質**:**規格語義變更**(SPEC_10 §2.2.1 的射程升為通則)＋ 引擎。**走 minor**(v0.13.0)
**非破壞性**:不動任何 CAID。見 §2.3 與釘 P4

---

## 0. 一句話

> **`Value::to_nlang` 的收尾是 `_ => format!("{:?}", self)`,而簽章路徑正在把它的輸出當成 n/ 原始碼餵回引擎求值。**

---

## 1. 缺陷

### 1.1 操作者今天看到的東西

一份四行的來源:

```
app: {
  k1: 1
  msg: "hi"
}
```

`oo status`(**最常用的指令**)印出:

```
Staged changes:
{
  app: {
    k1: Thunk { expr: Expr { kind: Atom(Int(1)), span: Span { start: 13, end: 14 } },
         closure: [ComboVal { data: {"k1": Thunk { … }, "msg": Thunk { … }}, types: {},
         rules: {}, meta: {}, system: {}, local: {}, closed: false, effect: EffectTag(0),
         relations: [], masa_ref: Top, pending_spreads: [],
         cache_id: RwLock { data: None, poisoned: false, .. },
         legacy_fields: {}, legacy_local: {} }], context: None, effect: EffectTag(0) }
    msg: Thunk { … }
  }
}
```

裡面有 `span`(原始碼位元組偏移)、`RwLock { poisoned: false }`(Rust 同步原語)、
`legacy_fields`(一個**全樹沒有任何一處寫入**的死欄位)。

結構形也一樣:

```
b: Ref(Path { anchor: Root, segments: ["a"], span: Span { start: 10, end: 13 } })
```

### 1.2 為什麼這不只是清潔問題

`crates/interpreter/src/oodp.rs:442`:

```rust
pub fn identify_caid(engine: &Ouroboros, val: &Value) -> Result<String, String> {
    let src = val.to_nlang(0);
    let id = eval_nlang_value(engine, &format!("~%Discovery./identify {src}"))?;
    Ok(id.to_string_plain())
}
```

**它把 `to_nlang` 的輸出當成 n/ 原始碼,餵回引擎求值。**
⟹「印出來的東西要能被剖析回去」**已經是簽章路徑在依賴的性質**,不是輸出慣例。
今天若值裡含 Thunk／Ref,那段文字剖析不過。

### 1.3 這是本週第三次同一類

| 何時 | 何處 | 洩漏什麼 |
| :-- | :-- | :-- |
| W3′-a(已切 v0.12.0) | evolve 邊界的衝突訊息 | `Path(Path { … span: Span { … } })` |
| **本弧** | **`Value::to_nlang` 本身** | `Thunk { … RwLock { poisoned: false } … }` |
| 本弧(§4 射程內) | `oo log` 的日期 | `SystemTime { tv_sec: 1786202932, tv_nsec: 575000000 }` |

**修的是類別不是個案**——所以規格面要一起升(§5)。

---

## 2. 量測(全部為直接量測,不是讀碼)

### 2.1 漏在哪:`to_nlang` 的十一個變體

`crates/interpreter/src/value.rs:2388` 起,`match self` 明列七個:

`Top`/`TopCaused`、`Atom`、`Combo`、`Union`、`Bottom`、`Blur`、`Range`

收尾是 `_ => format!("{:?}", self)`。**掉進去的是 `Thunk`、`Code`、`Ref` 三個。**

### 2.2 表面掃描(2026-08-08,引擎 v0.12.0)

偵測式:`[A-Z][A-Za-z]+ \{|RwLock|SystemTime|Span \{|tv_sec|EffectTag\(|IndexMap|Path\(Path`

| 表面 | Rust 形跡 |
| :-- | :-- |
| `oo status`(有未提交的暫存) | **3 行** |
| `oo inspect <根值 CAID>` | **3 行 / 共 1151 行** |
| `oo log` | **1 行**(`Date: SystemTime { … }`) |
| `oo eval 1+1`、`oo run` | **0**(**控制:掃描並非全紅**) |

### 2.3 為什麼非破壞性(已查證,不是假設)

| 疑慮 | 查證 | 結論 |
| :-- | :-- | :-- |
| 會不會動 CAID？ | `bn_serial.rs:95` 與 `value.rs:2698` 用的是 **`Expr::to_nlang`(parser 側)**,不是 `Value::to_nlang` | **不動**。由**釘 P4** 機械保證 |
| 會不會動 `oo fmt`(v2 已凍結)？ | `main.rs:1236` 走 `program.to_nlang()`,也是 Expr 側 | **不動**。由**釘 P3** 保證 |
| 會不會踩到既有測試斷言的 Debug 字串？ | 全 tests 掃 `Thunk {`／`ComboVal {`／`RwLock {`／`Span {` 共 **9 處**,逐處看過:**全是 Rust 建構式或 match 型樣,沒有一處斷言 Debug 字串** | 不會假性變紅 |

### 2.4 一個**真的**風險:聯集顯示排序

`value.rs:692` 與 `706` 用 `a.to_nlang(0).cmp(&b.to_nlang(0))` 當**聯集分支的顯示排序鍵**。
若分支是未強制 Thunk,鍵會從 Debug 字串變成 expr ⟹ **順序可能改變**。

- **不是不做的理由**——`display_order_probe_test`、`union_dedupe_probe_test`、
  `union_absorption_probe_test` 是現成的迴歸閘。
- **但交付必須正面面對**:若這三支任何一支變色,**不得改測試**(探針修改權在驗收方),
  須回報並說明順序為何改變、新順序是否仍滿足規格所要求的性質。
- **已查證不受影響**:`builtins/list.rs:899`／`1144` 的去重鍵取自 `oo.force(item, ctx)`
  的結果,**先強制再取鍵**,看不到 Thunk。

---

## 3. 裁定(用戶已批 2026-08-09)

### R-1｜Thunk 印 `expr.to_nlang(indent)`

**未觀測者印出它的來源,不印出它的答案。** `k1: 1` 印 `1`;`k1: 1 + 2` 印 `1 + 2`。

理由:與 call-by-observation 一致;`expr` 本來就已是 CAID 的一部分;**不引入新語法**
(`feedback_syntax_minimalism`:一個概念一種正準拼法)。

代價,明說:**同 expr 異 closure 會顯示相同**。顯示層本來就不是單射(`Top` 印 `_`)。

### R-2｜Ref 印 `<<` ＋ Path 的既有 Display ＋ `>>`

`ast.rs:944` 已經有 `Path` 的 Display:`_.` 根／`^.` 當前／`^^.` 父／裸。

**`<<>>` 不可省。** `Value::Ref` 只由結構形 `<<path>>` 產生(`eval.rs:1545`);
印成裸 `_.a` 再讀進來會**被求值**而不是被持有為引用 ⟹ round-trip 斷裂。
**這一條是 round-trip 準則直接改掉我原本的建議的地方。**

### R-3｜Code 印 `expr.to_nlang(indent)`,且自陳缺口

`Value::Code` 由規則本體產生(`eval.rs:1331`,`%code` 欄)。

**本弧只裁顯示,不宣稱 round-trip**:`%code` 印成裸運算式,再讀進來會被求值而非引述。
**這個缺口要寫進規格的自陳缺口,不要假裝補掉了。**

### R-4｜射程含 `oo log` 的 `Date`,不含 `oo test` 的 `%cause: {:?}`

- **含** `main.rs:819` `println!("    Date: {:?}", date)` ⟹ 改為 RFC-3339／ISO-8601。
  同一條規則的同一種違反,且引擎自己有 `~%Time`。
- **不含** `main.rs:1488` `(%cause: {:?})`——那牽涉 `BottomDetail` 的顯示,
  與 W3′ 系列重疊,應與之一起想。**這是「兩件不同的事」,不是「這樣比較不破壞」。**

### R-5｜規格條文升為通則(⟹ minor)

SPEC_10 §2.2.1 的 MUST「不得洩漏實作表示」**射程只有 evolve 邊界的座標**。
本弧將其升為通則。落腳處與條文由**驗收方**在收尾時寫(§8),交付不碰規格。

---

## 4. 射程

**做:**

1. `crates/interpreter/src/value.rs` 的 `Value::to_nlang`:`Thunk`／`Code`／`Ref`
   三個變體照 R-1／R-2／R-3 實作,**移除 `_ => format!("{:?}", self)` 這個收尾**
   ——改成明列全部十一個變體,使**未來新增變體會編譯失敗而不是靜默退回 Debug**。
   〔這一點是本弧的承重部分:修的是「還會再發生」的機制。〕
2. `crates/oo/src/main.rs:819` 的 `Date`。

**不做:**

- 不強制(forcing)任何值——那會改 CAID,是 W8′-b。
- 不換耐久編碼——那是 W8′-c。
- 不碰 `oo test` 的 `%cause`(R-4)。
- 不碰 `Bottom::to_nlang` 少印路徑那件事(**另掛**:見 §9)。
- **不碰任何規格檔**(spec 收尾一律由驗收方做)。

---

## 5. 探針

檔:`crates/oo/tests/print_what_can_be_read_probe_test.rs`(已隨本工單提交,已校準)

**紅測全部標 `#[ignore]`。#3 只准移除 `#[ignore]`,不得修改探針任何其他字元。**

| # | 類 | 斷言 | 基線 |
| :-- | :-- | :-- | :-- |
| **C1** | 控制 | **偵測器是武裝的**:對一個真的 `format!("{:?}", …)` 字串,偵測器必須命中 | 綠 |
| **C2** | 控制 | **與 R1 同一個指令**,輸入無 thunk(僅頂層原子):`oo status` 無形跡且非空;**且 R3 的區塊萃取在今天就 round-trip 得過** | 綠 |
| **R1** | 紅 | `oo status`(有暫存)無形跡,**且同場含 `k1: 1` 與 `msg: "hi"`** | 紅 |
| **R2** | 紅 | `oo inspect <根值>` 無形跡,**且同場含 `k1: 1`** | 紅 |
| **R3** | 紅 | `oo status` 印出的暫存區塊**寫回檔案後 `oo fmt` 必須成功** | 紅 |
| **R4** | 紅 | `b: <<_.a>>` 的 `oo status` 輸出**含 `<<_.a>>`**,不含 `Ref(Path` | 紅 |
| **R5** | 紅 | `oo log` 的日期列**不含 `SystemTime`／`tv_sec`,且含四位數西元年** | 紅 |
| **R6** | 紅 | 單元層:直接建 `Value::Code`,`to_nlang` 不得含 `Code(` | 紅 |
| **P1** | 釘 | `oo eval` 四個運算式 ＋ 無 thunk 的 `oo status` 輸出**逐位元組不變** | 綠 |
| **P2** | 釘 | 聯集顯示順序 `1 \| 3 \| 2` → 輸出**跨四次執行相同**且無形跡 | 綠 |
| **P3** | 釘 | `oo fmt` 對一份**未排版**輸入的 stdout**逐位元組不變**(v2 凍結) | 綠 |
| **P4** | 釘 | `app: { k1: 1 }` 提交後的**根 CAID 必須以 `6e8eae8b…` 結尾**(v0.2.55–v0.12.0 十週未動的那個值) | 綠 |

### 5.1 校準(2026-08-09,基線 `dev 6e8beee`)

**六支控制／釘綠、六支紅各自紅在自己的理由上:**

| # | 基線訊息(摘) |
| :-- | :-- |
| R1 | `status did not show the staged source \`k1: 1\`` |
| R2 | `inspect did not show \`k1: 1\`` |
| R3 | `the engine could not read back what it printed: Parse Error --> 2:581` |
| R4 | `a structural reference did not print as \`<<_.a>>\`` |
| R5 | `log printed Rust's clock representation: Date: SystemTime { tv_sec: … }` |
| R6 | `quoted code printed Rust's representation: Code(Expr { kind: Atom(Int(2)), span: Span { … } })` |

**校準抓到兩件,兩件都已修:**

1. **R3 原本紅在錯的理由上。** 第一版把 `oo status` 的整個 `{ … }` 區塊寫進檔案,
   `oo fmt` 回 `--> 1:1 expected program`——**因為帶外層大括號的區塊本來就不是合法的
   n/ 程式**(實測:`{ a: 1 }` 也一樣失敗)。那支紅測到的是我的萃取方式,不是印表機。
   已改為剝掉外層大括號與一層縮排,並**把「乾淨輸入時萃取得過」放進 C2**;
   現在 R3 紅在 `2:581`——Thunk 文字的正中間。
2. **P3 原本是空過的。** `oo fmt` 不帶 `-w` **只印到 stdout、不改檔案**(實測),
   而我拿一份**已經是正準形**的輸入去比對檔案 ⟹ 恆真。已改為餵未排版輸入、
   比對 stdout。

### 5.2 這組探針為什麼擋得住「用別的方法變綠」

一個交付若改成**在顯示時強制求值**,R1／R2／R3 會一起變綠而 `to_nlang` 一行沒改。
擋它的是兩支:

- **R4**:`Value::Ref` 被強制之後**仍然是 Ref**(結構形的定義就是「不要求值」),
  所以 R4 只能靠印表機修好。
- **R6**:直接對 `Value::Code` 呼叫 `to_nlang`,**繞不過**。

而若交付改成**在儲存時強制**(那是 W8′-b),**P4 會紅**。

### 5.3 可滿足性:兩件事我先檢查過了

1. **`Value::Ref` 可達**——`b: <<_.a>>`,實測 `oo status` 印
   `Ref(Path { anchor: Root, segments: ["a"], span: Span { start: 10, end: 13 } })`。
   R4 因此是 CLI 層的紅,不是單元層的。
2. **`Value::Code` 我沒能從 CLI 逼出來**——試過 `^.rules./double.%code`
   (回 `⊥ #out_of_horizon`)、`~%Reflection./quote`(無此鍵)。
   **所以 R6 是單元層探針,而且工單如實說它是。**
   〔規則:工單自身須先檢查可滿足性。這一條的答案是「一半」,寫出來。〕

---

## 6. 成功標準

1. §5 六支紅全綠,兩支控制與四支釘不動。
2. workspace **≥ 1793 / 0 / 3**,`conformance` 143/143,`genesis` 11/11。
   〔開弧基線(含本工單的探針)實測 **1787 / 0 / 9**;六支紅解除 `#[ignore]` 後
   9 → 3,1787 → 1793。**未達 1793 即為未交付完。**〕
3. **§2.4 的三支聯集測試若變色,不得改測試**——回報並說明。
4. `oo status` 的輸出**可被 `oo fmt` 剖析**(R3)。

---

## 7. 不變量(交付不得改動)

- `crates/interpreter/src/bn_serial.rs` **一行不得動**(CAID)。
- `Expr::to_nlang` / `to_nlang_prec`(`crates/parser/src/ast.rs`)**一行不得動**(fmt v2 凍結 ＋ CAID)。
- 任何 `crates/*/tests/**` 檔案**只准移除 `#[ignore]`**。
- 不得新增 `.oo/` 下的檔案或新的 op。
  〔已依規則 grep 既有的釘:**沒有任何釘斷言本弧所加之物不存在**——本弧不加持久化物與 op。〕
- 不得 `git add -A`。

---

## 8. 收尾分工

| 誰 | 做什麼 |
| :-- | :-- |
| #3 | §4「做」的兩項 ＋ 移除 `#[ignore]`。**不碰規格。** |
| 驗收方 | 診斷純度／探針完整性證明／獨立全 workspace 重跑／重複穩定 ×5／跨版本;**規格條文(R-5)與 CHANGELOG 由驗收方寫** |

### 8.1 交付紀錄（交付方）

**Delivered** against open `e6ba836` / baseline `6e8beee` (v0.12.0).

* `Value::to_nlang` / `to_string_plain`: **Thunk** → `expr.to_nlang`; **Ref** →
  `<<path>>`; **Code** → `expr.to_nlang`. Exhaustive match — no Debug fallthrough.
* `oo log` Date: RFC-3339 UTC (`%Y-%m-%dT%H:%M:%S%.3fZ`).
* Probe: six `#[ignore]` removed only. Spec/CHANGELOG not touched.

| Measurement | Result |
| --- | --- |
| `print_what_can_be_read_probe_test` | **12/12** |
| `display_order` / `union_dedupe` / `union_absorption` | **17 / 7 / 14** (no colour change) |
| full workspace | **1793 passed / 0 failed / 3 ignored**, 184 blocks |
| conformance | **143/143** |
| genesis | **11/11** |
| fmt / `git diff --check` | pass |

Opening with probe: 1787/0/9 → delivery 1787+6=1793 / 9−6=3.

---

## 9. 本弧**不**處理但已掛帳的相鄰項

| 項 | 內容 |
| :-- | :-- |
| `Bottom::to_nlang` | 印不出 `detail.path`。W3′-a 已在**訊息**面補上座標,但**值的正準列印**仍然丟掉它。與本弧同一個檔案、不同一件事 |
| `%cause: {:?}` | `main.rs:1488`,見 R-4 |
| `legacy_fields` / `legacy_local` | 死欄位,卻在 `bn_serial`(prio 5/6)與 `lattice_sketch` 內被讀。實測填入後 JSON 往返會使 CAID 改變 ⟹ `get_value` 拒收。**目前不可達**(全樹無寫入點)。見 STATUS §2.3.1 M7 |
| `system` 進 CAID | STATUS §2.3.1 M2。歸 W8′-b |
| **本弧達成的不是什麼** | 達成「印出來的能被**剖析**」;**不是**「印出來的再**求值**必得同一個值」——後者需要閉包隨行,超出射程。**這句話要進規格的自陳缺口。** |
