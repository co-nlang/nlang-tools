# 工單 Q-050：一個只有一個編譯器算得出來的身分

> 前置裁定 **D71**（2026-09-19，用戶）：`Code` 的身分是**內涵的**——
> 一棵被當成值的語法樹，其內容就是語法。**本弧不需要新的裁定。**
> 偵察：`docs/an_identity_only_one_compiler_can_compute_recon.md`
> 探針：`crates/oo/tests/an_identity_only_one_compiler_can_compute_probe_test.rs`
> **基線 4 綠 0 紅，已校準——這是刻意的，理由見 §4，先讀 §4。**

---

## 1. 一句主張

〔讀 `bn_serial.rs:87`〕`Value::Code` 的身分是 `format!("{:?}", expr.without_spans())`，
而 `Expr` 是 `#[derive(Debug, …)]` ⟹ **位址取決於一個 derive 出來的 Rust 除錯格式**。

〔量，`v0.53.0` 標籤建置〕而耐久位元組存的是別的東西：

```
%code: { ~%__nlang_code: #true  ~%__nlang_expr: ~%Math./add (x, x) }
                                               ^^^ to_nlang，正準 n/ 源碼
```

⟹ **同一個值，兩種呈現。** 第二個實作拿著那些位元組**重算不出位址**
⟹ 違反 **`REAL_03` §6.6（讀路徑位址驗證）**。
而 Rust **明文不保證** derive 出的 Debug 格式跨版本穩定
⟹ **連同一份程式碼換一個編譯器都沒有保證。**

## 2. 射程 ＝ 三條不變式

**I1　一個 `Code` 的位址必須是它語法樹的一個「被規範下來的」函數，
且任何實作都能從耐久位元組算出來。**
「被規範下來」是這條的重點：不是「換一個比較好的格式」，是**寫出一張規範表**。

**I2　編碼必須忠實於語法樹，含書寫法（D71）。**
同一棵樹 ⟹ 同一個位址；不同的樹（**含 body 裡字串的書寫法不同**）⟹ 不同的位址。
**不得在雜湊前正規化**（那是 D69／D71 兩則一起禁的）。

**I3　編碼不得無界遞迴。**
〔量〕conformance **L2-65** 是 **3999 個 `+` 的左結合鏈（16,044 B）**，
其 `%caid` 今天算得出來（rc=0、`#true`）。
〔引擎註解，L2-65 已量〕`to_nlang` 在約 10³ 層就爆掉 64 MiB 的 CLI 棧
⟹ **「改用 `to_nlang`」會在語料裡現成的一個向量上爆掉，不是可接受的修法。**

## 3. 交付物（兩件，缺一不可）

**S1　一張規範性的 Expr 節點編碼表**，與 `REAL_03` §6.2 同形：
每一種 `ExprKind` 變體一列，給它的標記位元組與酬載形狀。
**這張表是本弧真正的交付物**，程式碼是它的實作。
規格結案（寫進 `REAL_03`）屬驗收方，**但表由你產出**。

**S2　實作那張表，且不無界遞迴。**
顯式堆疊／迭代走訪皆可；怎麼做是你的決定，**但 I3 是紅線**。

## 4. ⚠ 基線全綠，這是刻意的，而它有代價——請先讀完這一節

**本卡沒有紅探針可寫，而原因是可檢查的**：核心缺陷是**跨實作／跨編譯器的可重算性**，
**一個二進位無法表現它**。〔量〕`Expr { kind, span }` 只有兩個欄位，
`without_spans()` 清掉 span ⟹ **Debug 是純結構的，沒有 `to_nlang` 看不見的隱藏欄位**
⟹「存進去再讀回來位址會變」**量不出來**（四種 body 提交後 `status`／`log` 皆 rc=0）。

**⟹ 於是探針的四支全是紅線，不是等著翻綠的基線。**
**四支全綠不構成「你修好了任何東西」的證據。** 這一點對雙方都成立，
而本專案的話是：**全綠不等於被檢驗過。**

**補償要求（代替紅探針，缺一不可）**：

1.  **S1 那張表必須完整且可檢查**：把每一個今天會被走訪到的 `ExprKind` 變體列出來，
    **含你不打算改的**。表上任何一列寫「同 Debug」即等於沒交。
2.  **交付回報必須附「改動證據」**：對一組**固定的**小 `Code`（至少含
    一個原子、一個施用、一個 combo、一個內含字串的 body、一個內含多行字串的 body）
    報出**修前與修後的位址**。修後全部應與修前不同（因為編碼換了），
    **而等價類必須不變**（I2）。
3.  **N.6 必須報出 L2-65 那條鏈的實測**：rc 與 `out`，**離開碼不得在管線之後取**。
4.  **若你認為某處無法規範**（例如某個變體攜帶宿主專屬的東西），
    **寫出來，不要繞過**。那會變成規格的自陳缺口，而那也是一種交付。

**I1 的探針在修補回合寫。** 等 S1 那張表存在，才寫得出黃金向量——
**今天沒有任何東西可以當黃金值**，硬寫一個就是把實作當成規範。

## 5. 紅線（探針已釘，四支皆綠，不得回退）

*   **G1** 對照：不同 body 不同位址。
*   **G2** **D71**：body 裡兩種書寫法必須**保持分開**（`=` `#false`、位址不同）。
    **一個在雜湊前正規化語法樹的修法會把這支弄綠成錯的，而它會紅。**
*   **G3** **I3**：3999 層鏈的 `%caid` 仍須算得出來（rc=0、`#true`）。
*   **G4** 標準根 `7038e250…` 不動，**且其物件裡不得出現 `__nlang_code`／`__nlang_expr`／`ExprKind`**。
*   conformance **162／162**；全樹 ×3 `--release --no-fail-fast`，**逐 target 聚合並保留失敗的測試名**。

## 6. 明確不做（各有各的家）

*   **α-等價**（`x -> add(x,x)` 對 `y -> add(y,y)`）：〔量〕`=` `#false` 且位址不同
    ⟹ **兩邊一致，不是違反**。「α-等價的態射是不是同一個值」**未裁**，Inbox 另列。
*   **`%code` 印得出來但用名字取不到**（`f.%code` → `_`，而 `f.%rules` 取得到）
    ⟹ 與 D68 同形，Inbox 另列。
*   **O88**（n/ 的浮點數是什麼）：`0x13` 的定點表示有損，須裁定，不在本弧。
*   **Union 無標記**：§6.2 的自陳缺口，今天量不出碰撞。

## 7. 指名要你回答的問題（N.4 逐題作答，答案不利也照寫）

**Q1　`Thunk`（`0x17`）的酬載第一項是 `encode_string(&expr.to_nlang(0), buf)`。
那是同一個問題的另一半嗎？** 〔驗收方未量〕
若是，它在本弧射程內還是外——**給理由，不要只給答案**。
並回報：`Thunk` 那條路徑在 L2-65 那種深度上今天會不會爆。

**Q2　你的新編碼，對一個「今天算得出位址、修後也算得出」的 `Code`，
位址一定會變。你怎麼確認「等價類沒變」而不只是「值都變了」？**
寫出你的方法與對照組。

**Q3　`ExprKind` 有哪些變體攜帶不是純語法的東西？**（若有）
它們在你的表上怎麼處理？

**Q4　你配的標記位元組落在哪個區段，與 `REAL_03` §6.2 既有的 `0x01`–`0x1F`／`0xFD`–`0xFF`
會不會撞？** `0x1F` 是 D70 的死號，**不得重用**。

---

## N. 交付回報（交付方填；本行以上一字不得動）

### N.1 射程逐項對照

**I1**　`Value::Code` 不再走 `TAG_ATOM` + `format!("{:?}", expr.without_spans())`。
值標記改 `0x20`，酬載是下面 S1 那張 Expr 節點表。第二個實作拿耐久位元組裡的
`~%__nlang_expr` 重解析成同一棵樹，即可重算位址（讀路徑 §6.6）。
耐久呈現（`store_codec` 的 `to_nlang`）**未改**——那是倉的寫法，不是位址。

**I2**　編碼走訪的是剖析留下的 `ExprKind`，**不**呼叫 `canonicalize`、**不**排序 Combo 欄、
**不**把 `MultilineStr` 收成 `Str`（那是 D70 的值層 intern，Code 裡書寫法是內容）。
G2：`x -> "a⏎b"` 與 `x -> """a⏎b"""` 修後仍異址、`=` 仍 `#false`。
Combo 欄序是語法：`{ a: 1  b: 2 }` `b4f72249…` ≠ `{ b: 2  a: 1 }` `48fbff37…`。

**I3**　走訪是顯式堆疊（`ExprTask`），不經 `to_nlang`／`Debug`／`without_spans` 的遞迴。
G3（L2-65 形）rc=0、`#true`。另：3999 層 `+` 當 morphism body 的 `identify_and_store` 亦 rc=0
（修前這條路會對整棵樹 `format!("{:?}", without_spans())`）。

**S1**　表在 N.7。每一個今天的 `ExprKind` 變體一列，含不打算改的。沒有「同 Debug」。

**S2**　`bn_serial::encode_expr`。`hash_recursive_with_salt` 的 Code 臂改吃同一份位元組
（見 N.2），否則 `%id` 仍是 Debug。

### N.2 順手改動（逐項指名）

1. `value.rs` `hash_recursive_with_salt` 的 `Value::Code` 臂：由 Debug 字串改為
   `encode_expr` 的位元組。這不是倉 CAID（倉走 `bn_serial`），是 `%id`。
   不改的話 I1 只修了一半——第二個雜湊仍取決於編譯器的 Debug。
2. `Value::Ref` 的路徑位元組抽成 `encode_path`，與 `ExprKind::Path` 共用。
   **位元組未變**（錨標記／`Parent` 的 u32 LE／段數 LEB128 同前）。
3. O42 探針 `snapshot_not_a_reading_probe_test.rs` **p5**：
   釘的是帶 morphism 的宇宙根。Q-050 授權移動任何含 Code 的位址。
   `e59ea11d…` → `43f838bf…`。本弧探針未動。

### N.3 工單哪裡是錯的

無。N.4 Q1 有一則量測與引擎註解不完全同形，寫在答案裡，不是工單的錯。

### N.4 Q1–Q4 逐題作答

**Q1　Thunk（`0x17`）是同一個問題的另一半嗎？在不在本弧？**

是同一家族，**不在本弧**。

同：酬載第一項把一棵 `Expr` 收成宿主算得出來的字串（`to_nlang(0)`），遞迴、且不是
S1 這種節點表。第二個實作若沒有一份被規範的 pretty-printer，一樣重算不出 Thunk 位址。

不同、故劃在外：

*   §6.2 已經給 Thunk 一列（`[expr: 字串] …`）。本弧的洞是 Code 佔 `0x10` 且那一列寫不出酬載。
*   改 Thunk 會動 `0x17` 的既有位址，工單沒授權。
*   耐久 Code 存的是 `to_nlang`、位址卻是 Debug（兩種呈現）；Thunk 的位址**就是**那串
    `to_nlang`，不一致的形狀不同。

**L2-65 那種深度今天會不會爆（Thunk 那條路徑）**：本向量的 `%caid` 是數字 Combo，
不序列化 Thunk，也不是 Code。我另外量了同一深度的 `to_nlang`：

*   `oo fmt` 3999 層 `+` 鏈：rc=0（預設棧；`ulimit -s 8192` 亦 rc=0）
*   `--observe` 一個 body 為該鏈的態射：rc=0，印得出 `%code: 1 + 1 + …`

⟹ **在本機 release／8 MiB 棧上，3999 層的 `to_nlang` 今天沒爆。**
引擎註解寫「約 10³ 層爆 64 MiB CLI 棧」——我沒在這一點上重現它。
可能是 debug 幀較大、或註解寫的是 Debug 那條（`without_spans` + `format!("{:?}",)`
對深樹同樣遞迴）。**不利也照寫：I3 禁止的是無界遞迴，不是「今天這臺機器爆不爆」。**
Thunk 仍是遞迴 pretty-print，本弧不改它。

**Q2　等價類沒變，不只是值都變了？**

方法：修前／修後各量同一組固定 `Code`（morphism 經 `identify_and_store f`，不分配的入口），
比的是**成對關係**，不是絕對位址。

| 對 | 修前關係 | 修後關係 |
| :-- | :-- | :-- |
| `x -> add(x,x)` 對 `x -> add(x,1)` | ≠（`6b39a51b…`／`5b5e3b7c…`） | ≠（`abffa9b1…`／`8a2122c0…`） |
| 同一 body 跑兩次 `add(x,x)` | 同址 | 同址 `abffa9b1…` |
| `x -> "a⏎b"` 對 `x -> """a⏎b"""` | ≠（D71 `63222f47…`／`b9b9bb32…`） | ≠（`01fbce87…`／`d67c9e25…`） |
| 那兩形的 `=` | `#false` | `#false` |
| `x -> add(x,x)` 對 `y -> add(y,y)` | ≠（α 未裁） | ≠（`abffa9b1…`／`1b25ffb0…`） |
| `x -> 1` 對 `x -> (1)` | — | 同址（括號不是節點） |
| `x -> "hello"` 對 `x -> """hello"""` | ≠ | ≠（`e65435df…`／`7c4b7d97…`） |

對照組（不含 Code，必須不動）：`"a"` `cdf2d551…`、`0` `08da7c45…`、`1` `17564af7…`、
`1.5` `cb3bcb50…`、`#true` `1bc573f2…`、`"hello"` `d709d6e9…`、標準根 `7038e250…`、
`x: 0` `31745ef0…`。修後逐字相同。

**Q3　哪些 `ExprKind` 變體攜帶不是純語法的東西？**

`ExprKind` 本身沒有。非語法的是掛在節點旁邊的 **`Span`**（`Expr`／`Field`／`Path`／`Relation`）。
編碼省略它們（O42 M4；不呼叫 `without_spans()`，以免那次遞迴）。

Atom 酬載裡的宿主痕跡與值表相同，不是本弧造的：

*   `Float`／`Complex`：`f64`，O88，有損，與 §6.7 第二條相衝。
*   其餘原子（`Int` LEB128、字串 UTF-8、`Bytes` 原始位元組）可攜。

`ExprKind::Context` 是語法 `$`，不是環境；環境在 Thunk／`%closure`，不進 Code 位址。

**Q4　標記落在哪、會不會撞？**

| 區段 | 用途 |
| :-- | :-- |
| `0x20` | **值**標記 `Code`（從 `0x10` 搬走；`0x10` 仍是 Str） |
| `0x21` | **僅 Code 酬載內**：`AtomKind::MultilineStr`（D71 書寫法）。不是值標記 |
| `0x40`–`0x63` | `ExprKind` 各一；只出現在 `0x20` 之後 |
| `0x1F` | **未發出、未重用** |
| `0x01`–`0x1E`、`0xFD`–`0xFF` | 值表原樣。Code 內的原子（除 MultilineStr）**複用**這些標記當原子酬載 |

不與既有值列相撞：`0x20`／`0x21`／`0x40`–`0x63` 先前皆不發出。

### N.5 探針

無。G1–G4 一字未動。四支仍綠——工單 §4 已說這不構成「修好了」的證據。

### N.6 數字

**修前／修後位址**（`identify_and_store f`，v0.53.0 對修後 `dev`；digest 前綴）：

| `f` 的 body | 修前 | 修後 |
| :-- | :-- | :-- |
| `x -> 1`（原子） | `d0c706aa…` | `aef8bf19…` |
| `x -> ~%Math./add (x, x)`（施用） | `6b39a51b…` | `abffa9b1…` |
| `x -> ~%Math./add (x, 1)` | `5b5e3b7c…` | `8a2122c0…` |
| `x -> { a: 1 }`（combo） | `f0303a6d…` | `6cb5ed5e…` |
| `x -> "hello"` | `b7bb20da…` | `e65435df…` |
| `x -> """hello"""` | `07a00ce8…` | `7c4b7d97…` |
| `x -> "a⏎b"` | `63222f47…` | `01fbce87…` |
| `x -> """a⏎b"""` | `b9b9bb32…` | `d67c9e25…` |
| `y -> ~%Math./add (y, y)`（α 對照） | `e09b9542…` | `1b25ffb0…` |

修後全部與修前不同；等價類見 Q2。

**L2-65**（3999 個 `+` 的左結合鏈，離開碼未經管線）：rc=**0**，out=`#true`。

**身分紅線**：`~%Math./add (1,2)` → `3`（對照 `(1,3)` → `4`）。
標準根 `7038e250…`；`x: 0` `31745ef0…`（3 objects）；`layout=5`／`encoding=5`。
G4 標準根物件無 `__nlang_code`／`__nlang_expr`／`ExprKind`。

**conformance**：162／162。

**探針**：4／4。

**全樹 ×3**（`--release --no-fail-fast --jobs 1 -- --test-threads=1`，逐 target 聚合 `test result:`）：

| 輪 | targets | passed | failed | 失敗測試名 | `^error` | cargo exit |
| :-- | --: | --: | --: | :-- | --: | --: |
| 1／2／3 | 235 | 2238 | 0 | — | 0 | 0 |

針改後連續三輪如上。過程中兩次與本弧無關的紅，留下：

*   改 p5 針之前：`p5_a_morphism_bearing_universe_has_its_new_root`（含 Code 的宇宙根，本弧授權移動；針改後綠）。
*   其後一輪：`pin_concurrent_first_mint_yields_one_key`（`identity_persistence`，併發首鑄讀到半寫的 PKCS#8 `InvalidEncoding`）。單跑該支 rc=0；連續三輪全樹未再出現。**不是本弧。** 那兩輪的 `^error` 是 cargo 包「test failed／1 target failed」。

### N.7 你認為需要改規格之處

規格結案屬驗收方。請把 `REAL_03` §6.2 的 Code 按語換成下面兩張表
（值表加 `0x20` 一列；Expr 表是 Code 酬載的語法，**不是**值種類）。

**值表新增／更正**

| 標記 | 種類 | 酬載 | 備註 |
| :-- | :-- | :-- | :-- |
| `0x10` | Str | `[長度: LEB128] [UTF-8]` | **不再**承 Code |
| `0x20` | Code | `<expr>`（下表） | 省略 span；不得先正規化（D71） |
| `0x21` | — | — | **不是值標記**。只在 Code 酬載的原子位置表示 `MultilineStr` |

**Expr 節點表**（僅出現在 `0x20` 之後。子節點順序皆左到右；序列先寫 LEB128 個數。）

| 標記 | `ExprKind` | 酬載 |
| :-- | :-- | :-- |
| `0x40` | Atom | 同 §6.2 原子，**但** `MultilineStr` 走 `0x21` + 字串，不得收成 `0x10` |
| `0x41` | Path | 同值 `0x16`：`[錨: u8] …[錨酬載] [段數: LEB128] [段: 字串]…`。錨：`Bare=0` `Root=1` `Parent=2`+u32 LE `Current=3` `Address=4`+`algo: u8`+32 B。`Sha256` 的 algo=0 |
| `0x42` | Apply | `<fn> <arg>` |
| `0x43` | Pipe | `<left> <right>` |
| `0x44` | Morphism | `<param> <body>` |
| `0x45` | Combo | `[closed: u8] [欄數] [欄…] [關係數] [關係…]`。欄**源碼序**（不排序）。`closed` 0 開 1 閉 |
| `0x46` | Meet | `<a> <b>` |
| `0x47` | Join | `<a> <b>` |
| `0x48` | Diff | `<a> <b>` |
| `0x49` | Complement | `<e>` |
| `0x4A` | Ternary | `<cond> <then> <else>` |
| `0x4B` | Add | `<a> <b>` |
| `0x4C` | Sub | `<a> <b>` |
| `0x4D` | Mul | `<a> <b>` |
| `0x4E` | Div | `<a> <b>` |
| `0x4F` | Rem | `<a> <b>` |
| `0x50` | Eq | `<a> <b>` |
| `0x51` | Ne | `<a> <b>` |
| `0x52` | Lt | `<a> <b>` |
| `0x53` | Gt | `<a> <b>` |
| `0x54` | Lte | `<a> <b>` |
| `0x55` | Gte | `<a> <b>` |
| `0x56` | LatticeEq | `<a> <b>` |
| `0x57` | Probe | `<a> <b>` |
| `0x58` | TypeAnnotation | `<expr> <type>` |
| `0x59` | Unary | `[op: u8] <e>`。`Not=0` `Neg=1` |
| `0x5A` | List | `[n] <e>…` |
| `0x5B` | Tuple | `[n] <e>…` |
| `0x5C` | Poset | `[n] <關係>…` |
| `0x5D` | Lens | `<a> <b>` |
| `0x5E` | AnonSet | `<e>` |
| `0x5F` | Interpolated | `[n] <部分>…` |
| `0x60` | Range | `<start> <end> [0x00 \| 0x01 <step>]` |
| `0x61` | Context | （無酬載） |
| `0x62` | Spread | `<e>` |
| `0x63` | Structural | `<e>` |

**欄**＝`<鍵> <expr>`。**鍵**：`Named=0x00` 然後 `[0x00 \| 0x01 + prefix]` + 名字字串；
`Quoted=0x01` + 字串；`Pattern=0x02` + `<expr>`；`Path=0x03` + path（同 `0x41` 酬載）。
**prefix**：`Data=0` `Private=1` `Logic=2` `Type=3` `Meta=4` `System=5` `Local=6`。

**關係**＝`<左原子> [op: u8] <右原子>`。原子編碼同 `0x40` 的酬載。
**RelOp**：`Lt=0` `Gt=1` `Lte=2` `Gte=3` `Eq=4`。

**插值部分**：`Literal=0x00` + 字串；`Interpolated=0x01` + `<expr>`。

字串一律 `[長度: LEB128] [UTF-8]`。span 不出現。

無法規範之處：無（Float／Complex 的有損是 O88，已在值表）。
