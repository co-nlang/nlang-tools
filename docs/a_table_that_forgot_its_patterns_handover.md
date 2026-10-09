# 工單：一張忘了自己模式的分派表（Q-076）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-076／裁定 `meta/oo/STATUS.md` **D95–D99**（皆用戶 2026-10-09「ok 按推薦」）／依據 `SPEC_07` §1.1 第 1 步（「Key（輸入約束）」）、§1.1.1（`_:` 只有預設分支一個意義）、頁首（呼叫點資料**唯一**通道是 `$`）；`SPEC_05` §3.3（`%rules` Key 是輸入的 Pattern；`%code` 款 MUST「印出→重讀不得當恆等」）；`REAL_03` §5.2 第 3 條（Pattern + Body 入雜湊）；`SYNTAX_12` §2 #5（分支值的 `$` ＝被匹配的輸入）；`TAG_REGISTRY` `#no_matching_branch`
> 探針（已預先提交並校準）`crates/oo/tests/a_table_that_forgot_its_patterns_probe_test.rs`
> 基線：dev `17c328a`／`oo v0.77.0` ⟹ **7 綠 10 紅**，三輪一致；十支紅的都紅在一般斷言，零空洞讀數。

## 1. 缺陷

〔量，v0.77.0；v0.27.0／v0.36.0／v0.40.0／v0.50.0／v0.60.0／v0.73.0 逐格相同〕模式在**求值時**就被丟掉，留下別的東西：

*   **`@{expr}:` 鍵**（`eval.rs` `FieldKey::Pattern`）：座標＝`eval(pe).to_string_plain()`；分派時 `dispatch.rs` `resolve_pattern` 由字串**猜回**一個模式，猜不出的一律 `Value::Top`。只有區間（E2 特例）、數字、標籤猜得回。⟹ `{ @{ @int }: "A" } "s"` → `"A"`；`{ @{ "s" }: 1 } 5` → `1`；`{ @{ 1 | 2 }: "U", @{ 1 }: "O" } 5` → `"U"`；`@{ @int }` 與 `@{ @str }` 同為 `"{...}"` ⟹ 兩個型別分支的表對任何輸入皆 ⊥；字串 `"4..6"` 與區間 `4..6` 同座標。
*   **`->` 的左手邊**（`eval.rs` `ExprKind::Morphism`）：裸路徑取最後一段**並剝掉前綴**當綁定名（`@int` → 綁定名 `int`），其餘一律 `_`。⟹ `(@int -> 7) "s"` → `7`；`(4.. -> "big") 1` → `"big"`；`(x @int -> x) 4` → `_`（`x @int` 的 `@` 是運算子，右邊剖析成裸名 `int`）；`(#a -> 1) #b` → `1`；`(x @int -> 1) = (x @str -> 1)` 與 `(@int -> 1) = (int -> 1)` 皆 `#true`。
*   **Combo 施用**（`lib.rs` 施用尾端）：以引數的 `to_string_plain()` 查鍵 ⟹ `{ "{...}": "caught" } { x: 1 }` → `"caught"`。
*   **表的其他鍵**：`_:` 被選中後因值不是 `%code` 規則而 `⊥「Rule has no %code」`；分支值裡的 `$` 是 `#no_context`；`resolve_pattern` 把鍵 `"0"`、`"it"` 也當 Top；未登記的 `it` 鍵優先於 `_`，規則本體另有隱含綁定 `it`、`0`；無匹配報 `#conflict`（`#no_matching_branch` 已登記而**零次發出**）。
*   **提交後**：`(_.)` 施用於「頂層有 `@{1}: 42`」的根得 `⊥「Rule has no %code」`，而同一個 Combo 的字面得 `42`。

**合併本身是對的**：`"s" & @int` → ⊥、`5 & @{ @int & 4.. }` → `5`、`"s" & @any` → `"s"`——§1.1 第 1 步要的運算已在，缺的只是模式本身。

## 2. 裁定與依據

*   **D95（甲）**：模式是一個**值**，與它的分支住在一起——分派表與 `->` 的模式皆以其值保存在 `%rules` 的分支裡；模式的印出字串**永不**被讀回成模式；`%rules` 中模式分支的排序（`REAL_03` §5.2 第 3 條）以**模式的位址**為鍵，與聯集分支同法。只有綁定名的態射（`x -> …`、`(x y -> …)`、`((a, b) -> …)`、`_ -> …`）保持 `SPEC_07` §5.2 定義 1 所釘的形，**位元組不變**。否決乙（單射正準文字＋剖析回來）。
*   **D96（甲）**：裸 `@T:` 鍵**永遠**是座標 `T` 的型別面向（引擎今天即如此，不改）；分派約束一律寫 `@{ @T }:`。規格範例改寫由驗收方收尾。
*   **D97（甲）**：有至少一個模式鍵的 Combo 是**分派表**；表內每個非 meta 鍵都是約束——數字鍵是該整數、標籤鍵是該標籤、命名／引號鍵是該字串、`_:` 是預設分支（Top）。沒有模式鍵的 Combo，施用是**依鍵查找**：只有**原子**引數有鍵；非原子引數落到 `_:`，再無則 `#no_matching_branch`。
*   **D98**：`it` 退場（作為鍵、作為隱含綁定）；隱含綁定 `0` 退場；`"0"`／`"it"` 當 Top 的特例移除。
*   **D99（乙）**：舊引擎寫下的分派表（帶 `%morphism`、無 `%rules`／`%builtin`、有非 meta 非數字鍵）**不猜**：施用答 ⊥，成因為新成因 **`#pattern_not_kept`**。

## 3. 射程＝不變式（不是機制）

*   **I1（D95）三個入口都保留模式**：`@{…}:` 鍵（r1–r3）、`->` 左手邊——含 `@T`、`x @T`、標籤、區間、聯集、字面（r4）——、Combo 施用查鍵（r8）。不同的模式是不同的值（r5）。**修類別**：凡是把一個值的**印出形**拿去比對、查鍵或重建成值的地方都在射程內；請在 Q1 逐一列出你找到的每一處（`to_string_plain` 今天在 `crates/interpreter/src` 非測試碼有 23 處呼叫，不是每一處都是這個病，請逐處判）。
*   **I2（D95）位元組**：只有綁定名的態射位元組不變（g4 釘了四個 v0.77.0 的根）；表的書寫順序不進位元組（g5）；同一個模式寫兩次是同一個分支、兩個本體相交（g7，`SPEC_03` §3.1）。模式分支在 `%rules` 中的名字對模式必須是**單射**且**與書寫順序無關**——驗收方的參考實作用模式的 `content_digest`；你可以選別的，在 Q2 說明為什麼它單射、與序無關。
*   **I3（D97）表的語義**：表內數字／標籤／命名鍵是約束、`_:` 是預設分支（r6）；分支值裡的 `$` 是被匹配的輸入（r6，含規格的 `/fib`——改用 `+` 與 D96 拼法）；無匹配（分派或查鍵）一律 `#no_matching_branch`（r2、r8）；極小元素規則照舊（g1）。提交後的表、以及**本身是表的根**，施用結果與字面相同（r9）。
*   **I4（D98）**：`it` 不是鍵也不是綁定名、隱含 `0` 移除（r7）。
*   **I5（D99）**：舊形的表——不論是字面寫出來的還是從倉讀回來的——施用答 `#pattern_not_kept`（r10）。**只有數字鍵**的 `%morphism` Combo 是內建的部分施用（curry slots），**不是**舊形，不得誤判。
*   **I6** 兩個新成員 `NoMatchingBranch`／`PatternNotKept` 進 `cause_tags!` 那張表（Q-075 的結構；名字只寫在表上）與 `primary_rank`。
*   **I7** 不改磁碟格式、不推進佈局或編碼；**值位址會動**——含分派表或模式態射的值（破壞性，驗收方記帳）；標準根 `7038e250…` 不含 `%rules`，不動。

## 4. 紅線（今天綠，必須保持綠）

*   g1–g7。conformance **162／162**（其中 L2/07、08、09、105、109 直接踩在本弧上）。
*   **交叉編譯** `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   `x: 0` 根 `31745ef0…`、`v: 1 + 1` 與 `1+1` 根 `f4f32e7b…`、標準根 `7038e250…`；`~%Math./add` (1, 2)=3、(1, 3)=4；新倉 `layout=9`／`encoding=5`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** 你找到的每一處「以印出形比對／查鍵／重建」（I1 修類別），各在哪裡、判為是或不是本病、改了什麼。
*   **Q2** 模式分支在 `%rules` 中的表示（鍵、欄位），為何單射、為何與書寫順序無關；綁定名與模式並存（`x @T`）時存在哪裡。
*   **Q3** `x @T` 的 `T` 不是裸名時（`x @{ 4.. }`、`x @(1 | 2)`）今天剖析成什麼、你的實作答什麼。陳述事實。
*   **Q4** 〔量，陳述，**不要修**〕`@int & 4..` 在本引擎求值成什麼（驗收方量到 `4..#_`，`@int` 不見了）；`@string`（不存在的型別名）與 `1` 合併得什麼。這兩件會讓某些「看起來不同」的模式其實是同一個值——請舉出你遇到的。
*   **Q5** 一張表印出來（`oo eval`、`run --format`）長什麼樣；把印出的文字貼回去再求值，得到的是不是同一張表。陳述事實。
*   **Q6** 兩張表相交（`t1 & t2`）與兩個態射的聯集施用（`((x @int -> …) | (x @str -> …)) v`）各答什麼。陳述事實。

## 6. 明文不在射程內

*   `@int & 4..` 丟掉 `@int`、未知型別名（`@string`）與任何值合併成功——Q4 只陳述。
*   `SPEC_09` §159–173 的「型別坍縮」義、標籤鍵 `#a:` 與引號鍵 `"#a":` 同座標——另在 Inbox。
*   `oo migrate` 改寫舊形表（D99：不還原）。
*   規格文字（`SPEC_07` §1／§1.1 情境 A／§1.1.1 兌現缺口註記、`SYNTAX_11`、`SPEC_05` §3.3、`REAL_03` §5.2、`SPEC_06`、`SPEC_09` L475、`SYNTAX_12` L78、`TAG_REGISTRY`）、`CHANGELOG`：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r10 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。** `rustfmt` 不得掃探針檔。
*   探針用 `@str`（`@string` 不是已知型別）；不依賴 `@int & 4..` 與 `4..` 是兩個模式（它們今天是同一個值）。
*   〔驗收方校準〕**兩極皆做**：
    基線 7 綠 10 紅（三輪一致）。
    一個參考實作 ⟹ 本探針 17／17：`@{}` 鍵的值以 `{{ %pattern: P, %val: <thunk> }}` 存進 `%rules`、名字為 `"{" + hex(content_digest(P)) + "}"`（整批收齊再放進 `%rules`——逐欄位合併會把帶 `%val` 的規則繭壓成它的 `%val`）；`->` 非裸名左手邊同法、`x @T` 另存 `%param`；分派讀 `%pattern`（缺席 ⟹ Top），表的資料鍵轉成約束；分支值的 thunk 以引數為 context 求值；查鍵只對原子；舊形 ⟹ `PatternNotKept`。
    同一參考實作跑全樹（含下列兩支預先修訂）：**261 target／2525 passed／0 failed，exit 0**（＝ v0.77.0 的 260／2508 加上本探針 17 支）；交叉編譯 0 error；conformance 162／162。
    **預先修訂的既有測試**（`AMENDED 2026-10-09 for Q-076`，基線上能編譯、紅在斷言；參考實作上綠）：
    `crates/interpreter/tests/dispatch_test.rs` `test_dispatch_it_fallback`——`{ it: "default" } 42` 改期待 ⊥ `NoMatchingBranch`（D98）；
    `crates/oo/tests/a_name_is_no_longer_a_credential_probe_test.rs` `c4_no_builtin_key_is_still_a_conflict`——`{{ %kind: #x }} 7` 改期待 `#no_matching_branch`（D97：無模式鍵的施用是查鍵，落空即此因）。
    〔grep〕以舊措辭為儀器的其他測試：`"No matching branch"`／`"Rule has no %code"`／`"{...}"`／`it:` 在 `crates/*/tests` 中**只有**上面那一處；`store_boundary_probe_test.rs:25` 的註解記著另一個以 `to_string_plain()` 得 `"{...}"` 的入口——請在 Q1 判它。
    **守衛的變異**（每一個都只換一處；括號內為連帶轉紅者）：
    取消極小元素篩選 ⟹ g1（r3、r6）；查鍵只對字串原子 ⟹ g2；規則本體不綁綁定名 ⟹ g3（r4）；只有綁定名的態射也寫 `%param` ⟹ g4；模式分支以書寫序號命名 ⟹ g5（g7）；型別模式的分支一律跳過 ⟹ g6（r1、r4、r6、r9）；同一模式寫兩次改為後者覆蓋 ⟹ g7。
    **參考實作沒有做 I1 的修類別盤查**——它只修了三個已知入口。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 做了什麼

裁定日 2026-10-09。模式以值留在 `%rules`。分支名是 `"{" + hex(content_digest(模式)) + "}"`。表裡的普通資料鍵與引號鍵收成約束：`_` 是 Top，整段能解析成整數的鍵是該整數，其餘是該字串。標籤鍵本來就是模式鍵。

只有綁定名的態射保持原位元組：裸名、`x y ->`（剖析器已折成巢狀 curry）、`((a, b) -> …)`、以及 `_ ->`（剖析成 `Atom(Top)`）。這些不寫 `%param`。

其餘 `->` 把模式值放進該分支的 `%pattern`，本體仍走 `%code`。`x @T` 且型別側是裸名時，同一分支另存 `%param` 為該綁定名；求值前把型別側的裸名 `int` 改寫成路徑段 `@int`。

分派讀 `%pattern`，缺席則 Top。`$`：`%val` 的 thunk 在 force 前把 context 換成引數；`%code` 仍把呼叫 context 設成引數。無匹配是 `NoMatchingBranch`（`#no_matching_branch`）。`it` 與隱含綁定 `0` 退場。沒有模式鍵的 Combo 只對原子查鍵；非原子落到 `_`，再無則同一成因。

舊形（有 `%morphism`、無 `%rules`、無 `%builtin`、且有非 meta 非數字鍵）答 `PatternNotKept`（`#pattern_not_kept`）。只有數字鍵的 `%morphism` 仍是內建的部分施用。

兩個新成員接在列舉尾端 `UnrecognizedCause` 之後。名字只寫在 `cause_tags!`。`primary_rank`：`PatternNotKept` 為 1，與 `UnrecognizedCause` 同列；`NoMatchingBranch` 為 2，與 `Conflict` 同列。

表的規則繭是 `{ %pattern, %val, _: Top }`，繭關閉。`%rules` 持有者打開。資料軸 `_ : Top` 讓這顆繭在相交時不被壓成 `%val`。模式箭的規則用 `%code`，沒有這顆 `_`。

套件留在 `oo` 0.77.0。磁碟 `layout=9`、`encoding=5`。

### 8.2 順手改動（逐項指名）

1. `crates/interpreter/src/value.rs`：`NoMatchingBranch`、`PatternNotKept`，以及上面的兩列 rank。
2. `crates/interpreter/src/dispatch.rs`：`resolve_pattern` 已刪。分支名走 `content_digest`。表規則繭與「父資料鍵補成約束」的繭都帶 `_ : Top`。
3. `crates/interpreter/src/eval.rs`：綁定名態射（含 `Atom(Top)`）走原構造；其餘走模式箭。`%rules` 持有者 `closed: false`。`morphism_parameter_names` 走進 `TypeAnnotation`，`x @int -> x + 1` 的 `x` 算綁定名。
4. `crates/interpreter/src/lib.rs`：舊形回 `PatternNotKept`。查鍵只在引數塌成原子時用印出形。落空是 `NoMatchingBranch`。

`_ : Top` 沿用既有腳手架：印出時 `is_engine_scaffold_field` 把它拿掉，與成因繭同一條路。`unify.rs`、`bn_serial.rs`、剖析器、探針未改。沒有 rustfmt 這四個檔與探針。

### 8.3 工單哪裡是錯的

工單寫 `to_string_plain` 在 `crates/interpreter/src` 非測試碼有 23 處呼叫。這次點名：定義 1 處（`value.rs:3172`），呼叫 57 處。`value.rs:3208` 一行裡有起點與終點兩個呼叫。23 與這次的點名對不上。逐處判斷在 Q1。

工單預期的交付後全樹是 261 target、2525 passed、0 failed。三輪的 `test result:` 與此相同。

### 8.4 工單指名要你回答的問題

**Q1** 行號是改完之後的。本病三處已改：

1. `@{expr}:`（`eval.rs` 的 `FieldKey::Pattern`）。座標不再是 `to_string_plain()`。模式值留在分支的 `%pattern`，名字是 digest。這處呼叫已不在清單裡。
2. `->` 的左手邊（`eval.rs` `ExprKind::Morphism`）。只有綁定名的形保持原位元組。其餘左手邊的模式值留下。
3. Combo 施用查鍵（`lib.rs:3282`）。只在 `arg.collapse()` 是原子時用印出形當鍵。非原子沒有鍵。落空是 `NoMatchingBranch`。不再退到欄位 `it`。

`resolve_pattern` 是把字串猜回模式的那一半，唯一呼叫者是舊的 `dispatch_morphism`。它本身不是 `to_string_plain` 呼叫。已刪。

`store_boundary_probe_test.rs:23–27` 的註解：已死的 `~%Official./add_architect` 對 `{0: str}` 做 `force(arg).to_string_plain()`，得到 `"{...}"`。是同一類。該內建仍是死的，探針未改，沒有把它復活。

其餘呼叫判為不是本病，未改：

1. 印表機自身：`value.rs:3200`、`3208`（兩個）、`3210`。`value.rs:3289` 拿來比 `"#list"`。
2. `lib.rs:1419`：force 一個 `%kind` 標籤，再比 `"list"`。
3. `eval.rs:1499`、`1500`、`2191`、`2192`：關係兩側原子自己的拼寫。`1759`：標籤等於 `eager`。`2088`：透鏡沿著使用者寫下的那段鍵走。`2101`：字串插值。
4. `type_constraint.rs:145`、`280`：標籤等於 `list`／`type`。
5. `builtins/diff.rs:183`。`query.rs:116`、`187`。`reflection.rs:55`、`128`、`263`、`298`、`316`、`335`。`engine.rs:210`、`805`、`824`、`834`、`854`。`string.rs:295`、`346`、`395`。`disc.rs:111`、`112`、`178`、`179`、`182`、`187`、`499`。
6. `builtins/list.rs:229`、`230`、`565`、`601`、`768`、`801`、`919`、`1324`、`1374` 比的是 list 標籤或列印。`1030` 是 `list.group_by` 的分組鍵，用的是該鍵的印出形，不是把分派模式重建出來。

**Q2** 分支名 `"{" + hex(content_digest(P)) + "}"`。`content_digest` 是 `serialize_bn` 的 sha256。`serialize_combo` 先按軸、再按鍵排序，所以 IndexMap 的寫入序不進 digest。這個名字不會與數字 curry 槽相撞。同一模式兩次是同一分支、兩個本體相交（g7）。書寫順序不進位元組（g5）。`x @T` 的綁定名在同一分支的 `%param`（字串）。只有綁定名的態射不寫 `%param`，鍵仍是原來的綁定名或元組鍵。

**Q3** 空目錄、release 二進位、`parse_expr_only`：

1. `x @{ 4.. }` 剖析成 `Apply(Path(Bare:x), AnonSet(Range(Int 4, TagEnd)))`。它不是型別註記。`(x @{ 4.. } -> x)` 的 `%pattern` 是 `_`（Top），digest `a8100ae6aa1940d0b663bb31cd466142ebbdbd5187131b92d93818987832eb89`，沒有 `%param`。對 1、對 5、對 9 都答 `_`。單獨求值 `x @{ 4.. }` 也是 `_`：自由的 `x` 是 Top。
2. `x @(1 | 2)` 剖析成 `TypeAnnotation(Path(Bare:x), Join(Int 1, Int 2))`。`(x @(1 | 2) -> x)` 的 `%pattern` 是 `1 | 2`，`%param` 是 `"x"`。對 1 答 `1`，對 3 答 `#no_matching_branch`。
3. 對照：`x @int -> x` 剖析成 `Morphism(TypeAnnotation(Path(Bare:x), Path(Bare:int)), Path(Bare:x))`，型別側是裸名 `int`。`@int -> 7` 的左手邊是 `Path(Bare:@int)`。`_ -> 1` 的左手邊是 `Atom(Top)`。`4.. -> "big"` 的左手邊是 `Range`。

**Q4** 未修。空目錄、release 二進位。`@int & 4..` 印成 `4..#_`，`(@int & 4..) = 4..` 是 `#true`。`@int & 4..6` 印成 `4..6`。`@int & ..6` 印成 `#_|_..6`。`@string & 1` 得 `1`。`@str & 1` 是 `_|_  ;; %cause: #conflict  ;; Value is not a string`。因此 `@int` 與區間寫在一起，和那個區間是同一個值。g1 仍綠：三個區間分支對 5 答 `"C"`。探針不要求 `@int & 4..` 與 `4..` 是兩個模式。

**Q5** 空目錄、release 二進位。`oo eval` 把 `{ @{ @int }: "A", @{ @str }: "B", _: "other" }` 印成剝開的本體：三個 digest 鍵下面是 `"A"`、`"other"`、`"B"`。`oo run --format` 印出 `%pattern` 與 `%val`：`88d2c99cfaaea4df5f1c5d5c43314b9b1361e87f83d522825ff3122bbb490c44` 的模式是 `{{ %kind: #type, %name: "int" }}`，`e935c5cea0c20553e6c7d7fdff175ec0611ef6242816e67ab9991b534e764a13` 是 `@str`，`a8100ae6aa1940d0b663bb31cd466142ebbdbd5187131b92d93818987832eb89` 的模式是 `_`。格式文本裡看不到資料軸 `_`。把 `oo eval` 的文本貼回去，與來源字面比較，是 `#false`。把 `run --format` 的 `t` 本體貼回去再比較，也是 `#false`。來源字面與自己比較是 `#true`。這兩次 `eval`／`run` 都沒有建立 `.oo/`。

**Q6** 空目錄、release 二進位。`({ @{ @int }: "A" } & { @{ @str }: "B" })` 印出一張合併的 `%rules`，兩個 digest 鍵的本體是 `"A"` 與 `"B"`。`({ @{ 4.. }: "A" } & { @{ 4.. }: "B" })` 是 `_|_  ;; %cause: #conflict  ;; Incompatible types: "A" vs "B"`。`({ @{ 4.. }: "A" } & { @{ ..6 }: "B" }) 5` 是 `"A" | "B"`。`((x @int -> "i") | (x @str -> "s"))` 對 4、對 `"s"`、對 2.5 都是 `_|_  ;; %cause: #conflict`。輸出裡沒有 `Incompatible types`。被施用的是一個聯集。

### 8.5 探針

`crates/oo/tests/a_table_that_forgot_its_patterns_probe_test.rs` 未改，未 rustfmt。兩支預先修訂的測試未再改：`dispatch_test.rs` 的 `test_dispatch_it_fallback`，`a_name_is_no_longer_a_credential_probe_test.rs` 的 `c4_no_builtin_key_is_still_a_conflict`。

全樹第三輪該探針 17／17，1.62s。`g1_range_tables_still_dispatch`、`g2_lookup_still_works`、`g3_bare_binders_behave_as_before`、`g4_bare_binders_keep_their_bytes`、`g5_order_does_not_reach_the_bytes`、`g6_any_arrows_still_answer`、`g7_the_same_pattern_twice_meets`、r1–r10 皆 `ok`。測試套件不印單支耗時。

交叉編譯 `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu`：`Finished` 4.27s，`^error` 0。

### 8.6 數字

1. 三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`：每輪 rc=0，`^error` 0，`test result:` 261 行，2525 passed，0 failed。`Running`／`test `／`test result:` 三輪各 3044 行，去掉耗時後相同。原始日誌的警告順序不必相同。
2. conformance 162／162，rc=0。跑者只印這一行總結。cwd 是 `/home/gali/nlang`。引擎是 `nlang-tools/target/release/oo`。
3. 空目錄：`~%Math./add (1, 2)` → 3，`(1, 3)` → 4，rc=0。這兩次 `eval` 沒有建立 `.oo/`。euid 1000。`/tmp/oo-ephemeral-*` 為 0。
4. `oo inspect <HEAD>` 的 `root:`：`x: 0` 為 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。`v: 1 + 1` 與 `v: 1+1` 為 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`（`status` 該行帶 `(available)`）。g4 四個根：`f: (x -> x + 1)` 為 `039d07351a998261d3150af05a84bd0fcf0ad4133b24d644d35ab1e59517d2b4`；`g` 與 `h` 同倉為 `e7793d964930ee884019432971e5082f10f1c5576c8abc89b4ab2f48dc013a27`；`k: { a: 1, _: 9 }` 為 `24fe01b6001567e4b03edc5e0715cb4cf53eadea4945994d7a963b495382662a`；`m: (_ -> 1)` 為 `c23f8a56608b9aa32b3fa089aa5d54ec4b0385b990d0cc2fe695970db114fb85`。新倉 `.oo/format` 為 `layout=9`，`.oo/objects.format` 為 `encoding=5`。套件仍是 `oo` 0.77.0。未推送。

### 8.7 你認為需要改規格之處

規格檔這次沒有改。建議驗收方依工單第 6 節收尾：`SPEC_07` §1／§1.1 情境 A／§1.1.1、`SYNTAX_11`、`SPEC_05` §3.3、`REAL_03` §5.2、`SPEC_06`、`SPEC_09` L475、`SYNTAX_12` L78、`TAG_REGISTRY`（`#no_matching_branch` 與 `#pattern_not_kept`）、`CHANGELOG`。

三件實測留給規格。Q5：兩種印出貼回去都與來源字面比較得 `#false`，來源字面與自己比較是 `#true`。Q6：兩個態射的聯集再施用，答的是 `#conflict`。Q4 照工單留著：`@int & 4..` 與 `4..` 是同一個值，`@string & 1` 得 `1`。

---

## 9. 驗收（驗收方填）

### 9.1 第一輪驗收：**不受理，開修補回合 R-1（一項）**

交付 `6784632` 的三個入口都做對了：模式以值留在 `%rules`、分支名是 `content_digest`、`$`、`_:`、`it` 退場、舊形 `#pattern_not_kept`，字面上的同一模式兩次會相交。驗收方重量：全樹 ×3 **261／2525／0**（rc 皆 0，去掉耗時後排序測試行 md5 三輪相同）、交叉編譯 0 error、conformance 162／162、`x: 0`／`1+1`／`1 + 1`／g4 四根逐位元組與 §8.6 相同、`~%Math./add` 3／4、新倉 `layout=9`／`encoding=5`。差異純度：探針、兩支預先修訂、`Cargo.lock`、版本、本檔 §8 以上皆未動。

**不受理的理由：I2 與 I3 在「鍵事後才到」這條路上不成立。** 〔量，`6784632` release〕鍵在同一個字面裡時，同一約束的兩個本體相交；鍵**後來**才到——經 `&`，或第二次 `evolve` 到一個本身是表的根——而它的約束已經是某個分支的名字時，**到來的值被丟掉**：

| 式子 | 交付 | 字面對照 |
| :--- | :--- | :--- |
| `({ @{ "k" }: 1 } & { k: 5 }) "k"`（反序、引號鍵、`1:` 同） | `1` | `{ @{ "k" }: 1, k: 5 } "k"` ⟹ ⊥ `#conflict` |
| `({ @{ "k" }: { y: 2 } } & { k: { x: 1 } }) "k"` | `{ y: 2 }` | 應為 `{ x: 1, y: 2 }` |
| `({ @{ @int }: 1, _: 2 } & { _: 9 }) "s"` | `2` | `{ @{ @int }: 1, _: 2, _: 9 } "s"` ⟹ ⊥ |
| 根 `@{ "k" }: 1` 提交後 `evolve` `k: 5` | **接受**；`(_.) "k"` ⟹ `1`、`_.k` ⟹ `5` | v0.77.0 拒絕（`#conflict at k`） |

起因是 `dispatch.rs` `rules_with_parent_data` 的 `if out.data.contains_key(&name) { continue; }`——註解寫著「Keys already named by that pattern are left alone」。這是一個**選擇**，與 I2「同一個模式寫兩次是同一個分支、兩個本體相交」、I3「本身是表的根，施用結果與字面相同」直接衝突。v0.77.0 那次拒絕理由是錯的（印出鍵相撞），答案卻是對的；本交付把它變成一次無聲的接受。

**驗收方的缺口**：r9 只量了「同一個檔案裡的根」，沒有量「事後到達的鍵」；I2／I3 的字面涵蓋這條路，探針沒有。

#### R-1 射程（不變式）

*   **R1-1（I2＋I3）** 一個值有模式鍵（因而是表）時，**不論它的鍵是在同一個字面裡、經 `&`、還是經後續 `evolve` 聚在一起**，施用的答案都與「把所有鍵寫進同一個字面」相同：約束相同的分支是同一個分支，所有本體相交；`_` 亦然（ra0–ra3）。新鍵照舊加入（ga1），相等的本體相交成自己（ga2）。第二次 `evolve` 可以接受（然後施用答 ⊥）也可以拒絕（根不變）；**接受而答舊值**不行（ra3）。
*   **R1-2（修類別）** 凡是「同一分支的兩個本體」在程式裡相遇的地方，都必須相交，**不得挑一個**。〔讀〕`eval.rs` `meet_branch_bodies` 的最後一臂 `(left, _) => left` 在右側既非 thunk 也非原子時丟掉右側；兩個 thunk 相交時只留左側的 `closure`／`context`——今天只在同一個字面內被呼叫（同一作用域），所以量不到，但 R-1 不得沿用它去合併**不同作用域**來的本體（`{ @{ "k" }: a } & { k: b }` 的 `b` 必須在它自己的作用域裡求值）。請在 R1-Q2 列出每一個相遇點。
*   **R1-3** 其餘不變：r1–r10、g1–g7、I4–I7、§4 紅線。

#### R-1 必答

*   **R1-Q1** 你在哪一層讓事後到達的鍵與分支相交（建構／合併時、還是施用時），為什麼；`evolve` 第二步你接受還是拒絕，`status` 與 `log` 各顯示什麼。
*   **R1-Q2** R1-2 的每一個相遇點與它怎麼相交；兩個本體來自不同作用域時各自的 `closure` 怎麼保住。
*   **R1-Q3**〔量，陳述，不要求改〕導航與施用是否一致：`{ @{ "k" }: 1, j: 5 }.j` 今天是 `_`，`({ @{ "k" }: 1 } & { j: 5 }).j` 是 `5`；兩者 `=` 是否為 `#true`。
*   **R1-Q4**（補第一輪 Q1）第一輪 Q1 的逐處判斷漏了 `crates/interpreter/src/oodp.rs` 的 7 個呼叫（`identify_caid` 兩處、`field_as_str`、`field_as_i64`、`field_as_str_list` 三處）。請逐處判；其中 `field_as_i64` 把印出形 `parse` 回整數——它是不是本病、會不會讓字串 `"42"` 被當成整數 42。**只判不改**（線上格式不在本卡射程）。另：工單 §3 I1 的「23 處」是驗收方數錯，基線為 58 行；你 §8.3 的 57 是對的。

#### 探針（新增；你不得改它）

`crates/oo/tests/a_table_that_forgot_its_patterns_r1_probe_test.rs`：ra0（字面是儀器：字面上兩個鍵本來就相交，否則下面讀不到東西）、ra1（名字／反序／引號／數字鍵／Combo 本體經 `&` 到來）、ra2（`_` 經 `&` 到來）、ra3（兩次 `evolve`）、ga1（新鍵照舊加入）、ga2（相等本體相交成自己）。原探針與兩支預先修訂不動。
〔驗收方校準〕交付 `6784632` **3 綠 3 紅**（ga1、ga2、ra0 綠；ra1–ra3 紅在一般斷言，三輪一致）。最小參考實作（施用時，對每個事後到達且約束已有分支的資料鍵，force 兩個本體並 `unify_internal`，寫回該分支的 `%val`；**沒有做 R1-2**、沒有處理 `%code` 分支）**6／6**。守衛變異：事後到達的鍵一律不加入 ⟹ ga1（ra1–ra3）；碰到已有分支一律 ⊥ ⟹ ga2（ra1）。
同一參考實作跑全樹（含本探針）：**262 target／2531 passed／0 failed，exit 0**（＝ 261／2525 加上本探針 6 支）；交叉編譯 0 error。

---

## 10. R-1 交付回報（交付方填；本行以上一字不得動）

### 10.1 做了什麼

事後到達的資料鍵在**施用時**與已有分支相交。`rules_with_parent_data` 不再在名字已存在時把到來的值丟掉。兩邊的本體各自用自己的閉包強制（`$` 是這次的引數），再 `unify_internal`。新的約束仍另開一支。

同一個字面裡的相遇仍在建構時做。兩個 thunk 的閉包與 context 相同時，合成一個 `Meet`，留下那一個作用域。其餘的對子先各自強制再相交，右側不再被丟掉。

第二次 `evolve` **接受**。施用答 ⊥ `#conflict`。套件仍是 `oo` 0.77.0。`layout=9`，`encoding=5`。

### 10.2 順手改動（逐項指名）

1. `crates/interpreter/src/dispatch.rs`：`rules_with_parent_data` 改成施用時的方法。已有分支的兩個本體相交後寫回該分支的 `%val`。
2. `crates/interpreter/src/lib.rs`：呼叫帶上引數與 context。
3. `crates/interpreter/src/eval.rs`：`meet_branch_bodies` 的最後一臂改為 `unify_internal`。同作用域的兩個 thunk 仍合成 `Meet`。

探針、兩支預先修訂、規格、`Cargo.toml`、`Cargo.lock` 未改。沒有 rustfmt 這三個檔與探針。`unify.rs` 未改。

### 10.3 工單哪裡是錯的

無。R1-Q4 已寫明第一輪工單的「23 處」是驗收方數錯，§8.3 的 57 是對的。

### 10.4 工單指名要你回答的問題

**R1-Q1** 在施用時相交。到來的 thunk 只有在強制時才用上自己的閉包，`$` 要等引數出現。建構或 `evolve` 當時還沒有這次的引數。

第二次 `evolve` 接受。release 二進位，空倉。先提交 `@{ "k" }: 1`，`status` 是 `Universe is static (no staged changes).`，標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911 (available)`。再 `evolve` `k: 5`，rc 0。`status` 的暫存是 `{ k: 5 }`，`Total Logical Entropy: 75 bits`。`commit -m b` 之後 `log` 兩筆：`b` 為 `7ca94739077de531ee5ef1756bbcc9c128a0d5f65bc42b0cb680ba41fae2c0bc`（2026-10-09T03:53:04.157Z），`a` 為 `31e6f5f6c0a964ef6a1366c8bc93526ef2d1f5934343736f5cbab60c75d4c24f`（2026-10-09T03:53:04.057Z）。`(_.) "k"` 是 `_|_  ;; %cause: #conflict  ;; Incompatible types: 1 vs 5`。

**R1-Q2** 三處相遇：

1. `eval.rs` `acc_pattern_branch`／`meet_branch_bodies`。同一個字面。兩個 thunk 的閉包 `Arc` 相同、context 的雜湊相同時，表達式合成 `Meet`，閉包留那一份。否則 `unify_internal`：每個 thunk 在 `force` 裡換成自己的閉包。舊的 `(left, _) => left` 已不在。
2. `dispatch.rs` `rules_with_parent_data`。施用時，資料鍵的約束已經是某分支的名字。左右都走 `apply_single_rule`：`%val` 的 thunk 保留自己的閉包，context 換成這次的引數；`%code` 在自己的 `%closure` 裡求值。然後 `unify_internal`。新鍵只新增，不跟自己相交。
3. 兩張表的 `&` 走 `unify_internal`。規則繭的 `%val` 在相交前被 `force`，各用各的閉包。這條路原來就這樣，本回合沒有改 `unify.rs`。

不同作用域：release 二進位。`{ left: { a: 10, t: { @{ "k" }: { x: a } } }, right: { b: 7, u: { k: { y: b } } }, q: (left.t & right.u) "k" }.q` 是 `{ x: 10, y: 7 }`。`a` 留在 left，`b` 留在 right。

**R1-Q3** 未改導航。release 二進位，空目錄。`{ @{ "k" }: 1, j: 5 }.j` 是 `_`。`({ @{ "k" }: 1 } & { j: 5 }).j` 是 `5`。兩者 `=` 是 `#false`。

**R1-Q4** 只判不改。`oodp.rs` 這 7 個呼叫都不是分派表把模式從印出形猜回去。

1. `identify_caid`（486）：送進 `identify` 的是 `to_nlang(0)`。`to_string_plain` 用在 identify 的結果上，把 CAID 收成字串。
2. `identify_caid_src`（492）：同一件事，來源是呼叫端給的原文。
3. `field_as_str`（589）：欄位的印出形當成廣告欄位的字串。Combo 會變成 `"{...}"`。
4. `field_as_i64`（594）：印出形再 `parse` 成 `i64`。字串原子的印出形就是內容，所以欄位是字串 `"42"` 時 `parse` 得到 42，整數 42 也得到 42。標籤 `#42` 的印出形是 `#42`，解析失敗。Combo 的印出形是 `{...}`，解析失敗。呼叫點是 `expires`、`listen_port`、`capacity`、`ts`、`ttl`。是「印出形讀回成值」，線上格式不在本卡，未改。
5. `field_as_str_list` 三處（750、756、757）：清單元素、聯集分支、其餘值，各自的印出形收成服務名字。Combo 元素是 `"{...}"`。

### 10.5 探針

`crates/oo/tests/a_table_that_forgot_its_patterns_r1_probe_test.rs` 未改，未 rustfmt。原探針與兩支預先修訂未改。

全樹第三輪：原探針 17／17，1.73s，g1–g7 與 r1–r10 皆 `ok`。R-1 探針 6／6，0.42s，`ga1_a_new_key_still_joins`、`ga2_equal_bodies_still_meet_to_themselves`、`ra0_the_literal_meets`、`ra1_a_key_that_arrives_by_meet_meets_its_branch`、`ra2_a_default_that_arrives_by_meet_meets_the_default`、`ra3_a_root_built_in_two_evolves_answers_like_its_literal` 皆 `ok`。測試套件不印單支耗時。

交叉編譯 `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu`：`Finished` 4.89s，`^error` 0。

### 10.6 數字

1. 三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`：每輪 rc=0，`^error` 0，`test result:` 262 行，2531 passed，0 failed。`Running`／`test `／`test result:` 三輪各 3052 行，去掉耗時後相同。原始日誌的警告順序不必相同。
2. conformance 162／162，rc=0。跑者只印這一行總結。cwd 是 `/home/gali/nlang`。引擎是 `nlang-tools/target/release/oo`。
3. 空目錄：`~%Math./add (1, 2)` → 3，`(1, 3)` → 4，rc=0。這兩次 `eval` 沒有建立 `.oo/`。euid 1000。`/tmp/oo-ephemeral-*` 為 0。
4. `oo inspect <HEAD>` 的 `root:`：`x: 0` 為 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。`v: 1 + 1` 與 `v: 1+1` 為 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`（`status` 該行帶 `(available)`）。g4 四個根仍是 `039d07351a998261d3150af05a84bd0fcf0ad4133b24d644d35ab1e59517d2b4`、`e7793d964930ee884019432971e5082f10f1c5576c8abc89b4ab2f48dc013a27`、`24fe01b6001567e4b03edc5e0715cb4cf53eadea4945994d7a963b495382662a`、`c23f8a56608b9aa32b3fa089aa5d54ec4b0385b990d0cc2fe695970db114fb85`。新倉 `layout=9`，`encoding=5`。套件仍是 `oo` 0.77.0。未推送。

---

## 11. R-1 驗收（驗收方填）

### 11.1 R-1 驗收：**不受理，開修補回合 R-2（一項；起因在驗收方的 R-1 工單與漏讀的 Q6）**

交付 `b187d83` 的 R-1 字面上都做到了：事後到達的鍵與分支相交（ra0–ra3 綠）、`meet_branch_bodies` 的 `(left, _) => left` 拿掉、跨作用域保住各自的閉包（R1-Q2 的例子驗收方重現 `{ x: 10, y: 7 }`）、`oodp.rs` 7 處逐處判了（R1-Q4）。驗收方重量全樹第一輪 **262／2531／0**、rc 0（第二、三輪未跑完即停，理由見下）。差異純度乾淨。

**不受理的理由：一個沒有被選中的分支被求值了，而且兩張表相交時一個分支的衝突塌掉整張表。** 〔量，`b187d83` release〕

1.  **施用時的相交是急的。** `rules_with_parent_data` 在**選分支之前**，對每一個碰撞的分支把兩個本體都 `apply_single_rule`。值是對的，工作不對：

    | 式子（`run --observe`） | 答 | 寫了檔？ | 同樣的鍵寫在一個字面裡 |
    | :--- | :--- | :--- | :--- |
    | `({ @{ "k" }: 1, _: 0 } & { k: ~%Io./write_file ("met.txt", "x") }) "z"` | `0` | **是** | 不寫 |
    | `({ @{ "k" }: ~%Io./write_file ("left.txt", "x"), _: 0 } & { k: 5 }) "z"` | `0` | **是** | 不寫 |
    | `({ @{ @int }: 1, _: 2 } & { _: ~%Io./write_file ("d.txt", "x") }) 4`（匹配但非極小） | `1` | **是** | 不寫 |

2.  **兩張表 `&`、模式相同時，兩個分支在 `unify` 裡當場相交。** 規則繭的 `%val` 被強制（效應執行），兩個本體衝突時**整張表**是 ⊥：`({ @{ "k" }: 1, _: 0 } & { @{ "k" }: 2 }) "z"` ⟹ ⊥ `#conflict`，而 `{ @{ "k" }: 1, _: 0, @{ "k" }: 2 } "z"` ⟹ `0`。模式箭頭同：`((@int -> 1) & (@int -> 2)) "s"` ⟹ ⊥ `#conflict`（應為 `#no_matching_branch`；`SYNTAX_11` §4 #6「衝突在應用時坍縮，定義時不檢查」）。R1-1 寫的是「不論鍵是在同一個字面裡、經 `&`……聚在一起」——**經 `&` 聚在一起的模式鍵**這條路，R-1 探針沒有量。

**起因在驗收方。** 第 1 項：R-1 工單把參考實作寫成「force 兩個本體並 `unify_internal`」，交付照做；驗收方的參考實作有同一個缺陷。第 2 項：第一輪 Q6 交付已經寫了「`({ @{ 4.. }: "A" } & { @{ 4.. }: "B" })` 是 `_|_ #conflict`」——驗收方把它讀成「那個分支是 ⊥」，沒有看出是整張表。

**驗收方另量到的位元組性質**（寫成 R2-3 的理由）：驗收方為第 2 項做的第一版參考實作（相交延後、兩個本體依序存放）讓 `t: { @{ "k" }: 1 } & { @{ "k" }: 2 }` 與 `t: { @{ "k" }: 2 } & { @{ "k" }: 1 }` 提交出**不同的根**。相交的結果若依賴相交的順序，兩個宇宙以不同順序合併就不收斂（`meta/oo/commit.md` §1.1.6：n/ 是 CvRDT）。

#### R-2 射程（不變式）

*   **R2-1（惰性）** 一個分支的本體**只有在分派選中它時**才被求值——不論它的本體們是在同一個字面裡、經 `&`（資料鍵或模式鍵）、還是經後續 `evolve` 聚在一起。匹配而非極小的分支、不匹配的分支，都不求值、不產生效應（rb0–rb4；gb1 是儀器：被選中的分支效應照常執行）。
*   **R2-2（衝突是分支的）** 兩張表或兩個模式箭頭以同一模式相交時，兩個本體的衝突只屬於那個分支：別的引數照常作答；對該模式的引數答 ⊥（rb5、rb6；gb2：相等的本體相交成自己、不同的 Combo 本體合併）。
*   **R2-3（集合）** 同一分支的多個本體是一個**集合**：`a & b` 與 `b & a`、`(a & b) & c` 與 `a & (b & c)`、`(a & b) & b` 與 `a & b` 提交後**根位址相同**；模式箭頭同（rb7，附非空洞檢查：每個宇宙的 `(_.t) "z"` 必須是 `0`）。
*   **R2-4** 其餘不變：r1–r10、g1–g7、ra0–ra3、ga1–ga2、I4–I7、§4 紅線。只有綁定名的兩個態射相交（`(x -> 1) & (y -> 2)`、`(x -> x + 1) & (x -> x * 2)`）**不在射程**——那是 `SPEC_07` §5.3 與 §1.1 兩種讀法的問題，已列 Inbox 待裁。

#### R-2 必答

*   **R2-Q1** 同一分支的多個本體你怎麼表示（欄位、鍵）；為什麼它與聚合順序無關、為什麼冪等。
*   **R2-Q2** 你在哪些地方產生這個表示（字面建構、`unify`、施用時的資料鍵），在哪裡把它們相交；R1-2 的三個相遇點各改成什麼。
*   **R2-Q3**〔量，陳述，不要求改〕同樣的鍵寫在一個字面裡、與經 `&` 聚在一起，提交後根位址是否相同（例：`t: { @{ "k" }: 1, @{ "k" }: 2, _: 0 }` 對 `t: { @{ "k" }: 1, _: 0 } & { @{ "k" }: 2 }`；`t: { @{ "k" }: 1, k: 5 }` 對 `t: { @{ "k" }: 1 } & { k: 5 }`）。不同的話，差在哪裡。
*   **R2-Q4**〔量，陳述〕R1-Q3 的導航再量一次（`{ @{ "k" }: 1, j: 5 }.j` 對 `({ @{ "k" }: 1 } & { j: 5 }).j`）。

#### 探針（新增；你不得改它）

`crates/oo/tests/a_table_that_forgot_its_patterns_r2_probe_test.rs`：gb1（被選中的分支效應照常執行——儀器）、rb0（字面裡沒被選中的分支不執行——對照）、rb1（不匹配分支的到來本體）、rb2（不匹配分支的既有本體）、rb3（匹配而非極小）、rb4（兩張表／兩個箭頭相交）、rb5（衝突是分支的）、rb6（箭頭衝突在應用時）、gb2（相等本體、Combo 本體合併）、rb7（集合；根位址）。原探針、R-1 探針、兩支預先修訂不動。
〔驗收方校準〕交付 `b187d83` **3 綠 7 紅**（gb1、gb2、rb0 綠；rb1–rb7 紅在一般斷言；三輪一致）；同時原探針 17／17、R-1 探針 6／6。參考實作（`/home/gali/nlang-baselines/q076-r2-ref` `c7628a7`：碰撞時不求值，把兩個規則繭收成 `{ %pattern, %bodies: { "{<位址>}": 繭, … } }`，`%bodies` 以本體位址為鍵、只剩一個時就是那個繭；`unify` 遇到兩個規則繭時同法；施用時被選中的分支逐一施用 `%bodies` 再 `unify_internal`；**沒有**改字面建構，故 R2-Q3 的字面對 `&` 根位址不同）**10／10**，原探針 17／17、R-1 探針 6／6。守衛變異（前兩個量在參考實作的前一版 `e2db015`——相交延後、本體依序存放——上，延後相交的路徑相同）：被選中的分支丟掉到來本體 ⟹ gb1、gb2（ra1–ra3、rb5、rb6）；延後的相交一律 ⊥ ⟹ gb1、gb2（ga2、ra1）；本體依序存放（即 `e2db015`）⟹ rb7；集合不去重 ⟹ rb7。
同一參考實作跑全樹（含本探針）：**263 target／2541 passed／0 failed，exit 0**（＝ 262／2531 加上本探針 10 支）；交叉編譯 0 error。

---

## 12. R-2 交付回報（交付方填；本行以上一字不得動）

### 12.1 做了什麼

同一分支的多個本體是一個集合。鍵是該規則繭的 `content_digest`，形如 `"{" + hex + "}"`。集合裡只剩一個本體時，值就是那個繭。鍵按摘要排序，同一摘要再併一次仍是同一格，所以左右對調、分組、重複併入都不進位元組。

本體在分派選中該分支時才求值。選中之後，每個本體用自己的閉包施用（`$` 是這次的引數），結果再 `unify_internal`。沒被選中的分支保持 thunk，不產生效應。

兩張表或兩個模式箭頭以同一模式相交時，衝突留在該分支：別的引數照常作答，對該模式的引數答 ⊥。

套件仍是 `oo` 0.77.0。`layout=9`，`encoding=5`。

### 12.2 順手改動（逐項指名）

1. `crates/interpreter/src/dispatch.rs`：`join_rule_cocoons` 把兩個規則繭收成 `%bodies`。`rules_with_parent_data` 只把到來的資料鍵併進這個集合，不在這裡強制。`apply_single_rule` 見到 `%bodies` 時逐一施用再相交。
2. `crates/interpreter/src/unify.rs`：兩個規則繭相交時走同一個集合，`%val` 不在這一步強制。

字面建構仍是 `eval.rs` 的 `meet_branch_bodies`。探針、規格、`Cargo.toml`、`Cargo.lock` 未改。沒有 rustfmt。

### 12.3 工單哪裡是錯的

無。

### 12.4 工單指名要你回答的問題

**R2-Q1** 一個分支多個本體時，規則繭是 `{ %pattern, %bodies }`。`%bodies` 的資料鍵是 `"{" + hex(content_digest(繭)) + "}"`，值是那個繭。`BTreeMap` 按鍵排序，序列化也按軸與鍵排序，所以 `a & b` 與 `b & a` 的位元組相同。同一摘要寫入會蓋掉同一格，所以 `(a & b) & b` 與 `a & b` 相同。只剩一個本體時不包一層，值就是那個繭，因此兩個摘要相同的表相交之後與單張表同一形。

**R2-Q2** R-1 的三個相遇點現在是：

1. 字面，`eval.rs` `acc_pattern_branch`／`meet_branch_bodies`。同作用域的兩個 thunk 仍合成一個 `Meet`，閉包留那一份。這個 `Meet` 是分支的 `%val`，要等該分支被選中才強制。作用域不同的對子仍在建構時 `unify_internal`。這條路本回合沒有改。
2. 施用時的資料鍵，`dispatch.rs` `rules_with_parent_data`。到來的繭併進 `%bodies`，這裡不強制。相交改到 `apply_single_rule`，而且只在分派選中該分支時：每個本體留自己的閉包，thunk 的 context 換成這次的引數，`%code` 在自己的 `%closure` 裡求值，然後 `unify_internal`。
3. 兩張表（或兩個模式箭頭）的 `&`。`unify.rs` `unify_combo` 在兩個值都是規則繭時呼叫 `join_rule_cocoons`。`%val` 不再在相交當下被 `force`。相交同樣在選中該分支時。

跨作用域再量，release 二進位，空目錄。`{ left: { a: 10, t: { @{ "k" }: { x: a } } }, right: { b: 7, u: { k: { y: b } } }, q: (left.t & right.u) "k" }.q` 是 `{ x: 10, y: 7 }`。`a` 留在 left，`b` 留在 right。

**R2-Q3** 兩對提交後的根都不同。release 二進位，各自的空倉，提交 `t:`。

模式鍵：

- `t: { @{ "k" }: 1, @{ "k" }: 2, _: 0 }` 根 `cbffaaee6fda3a40c12d8f2d081c1101be20e421ecc9803cc4ec44b094290811`
- `t: { @{ "k" }: 1, _: 0 } & { @{ "k" }: 2 }` 根 `0d5c3b0725506697f7082d2ce3023da3d57afe8aa44035c524882b3923c1cf87`
- 兩邊 `(_.t) "z"` 都是 `0`

字面在建構時把兩個 `"k"` 本體收成一個 `%val`。`oo eval` 印出該分支已是 `_|_`，`Incompatible types: 1 vs 2`，`_` 分支是 `0`。`&` 把兩個繭放在 `%bodies`，印出是 `1` 與 `2`，施用於 `"k"` 時才相交。

資料鍵：

- `t: { @{ "k" }: 1, k: 5 }` 根 `7f0be6aa3b3e01ed926038be7a6eeb221bfdba93ee90744bfb72c688315ca9fc`
- `t: { @{ "k" }: 1 } & { k: 5 }` 根 `70ea829c048486318da1baaf104b10bc87b882412b7a5a36925745919ad59c42`
- 兩邊 `(_.t) "z"` 都是 `_|_` `#no_matching_branch`

字面把 `k: 5` 折進該分支，印出已是 `1` 對 `5` 的 `#conflict`。`&` 的提交值仍是 `%rules` 裡本體 `1`，資料軸另有 `k: 5`。兩件在施用 `"k"` 時才相交。

**R2-Q4** release 二進位，空目錄。`{ @{ "k" }: 1, j: 5 }.j` 是 `_`。`({ @{ "k" }: 1 } & { j: 5 }).j` 是 `5`。兩者 `=` 是 `#false`。

### 12.5 探針

`crates/oo/tests/a_table_that_forgot_its_patterns_r2_probe_test.rs` 未改，未 rustfmt。原探針、R-1 探針、兩支預先修訂未改。

全樹第三輪：原探針 17／17，1.65s，g1–g7 與 r1–r10 皆 `ok`。R-1 探針 6／6，0.42s，`ga1_a_new_key_still_joins`、`ga2_equal_bodies_still_meet_to_themselves`、`ra0_the_literal_meets`、`ra1_a_key_that_arrives_by_meet_meets_its_branch`、`ra2_a_default_that_arrives_by_meet_meets_the_default`、`ra3_a_root_built_in_two_evolves_answers_like_its_literal` 皆 `ok`。R-2 探針 10／10，1.16s，`gb1_a_chosen_branch_runs`、`gb2_bodies_of_one_pattern_still_meet`、`rb0_in_a_literal_an_unchosen_branch_does_not_run`、`rb1_an_arriving_body_of_an_unmatched_branch_does_not_run`、`rb2_an_existing_body_of_an_unmatched_branch_does_not_run`、`rb3_a_matching_but_not_minimal_branch_does_not_run`、`rb4_tables_that_meet_do_not_run_an_unchosen_branch`、`rb5_a_conflict_between_two_bodies_is_the_branchs`、`rb6_arrows_with_one_pattern_conflict_at_application`、`rb7_the_bodies_of_one_branch_are_a_set` 皆 `ok`。測試套件不印單支耗時。

交叉編譯 `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu`：`Finished` 4.73s，`^error` 0。

### 12.6 數字

1. 三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`：每輪 rc=0，`^error` 0，`test result:` 263 行，2541 passed，0 failed。`Running`／`test `／`test result:` 三輪各 3064 行，去掉耗時後相同。原始日誌的警告順序不必相同。
2. conformance 162／162，rc=0。跑者只印這一行總結。cwd 是 `/home/gali/nlang`。引擎是 `nlang-tools/target/release/oo`。
3. 空目錄：`~%Math./add (1, 2)` → 3，`(1, 3)` → 4，rc=0。這兩次 `eval` 沒有建立 `.oo/`。euid 1000。`/tmp/oo-ephemeral-*` 為 0。
4. 提交 `x: 0` 之後 `status` 的標準根是 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911 (available)`，該倉 `Universe is static (no staged changes).`。`x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。`v: 1 + 1` 與 `v: 1+1` 根 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。g4 四個根仍是 `039d07351a998261d3150af05a84bd0fcf0ad4133b24d644d35ab1e59517d2b4`、`e7793d964930ee884019432971e5082f10f1c5576c8abc89b4ab2f48dc013a27`、`24fe01b6001567e4b03edc5e0715cb4cf53eadea4945994d7a963b495382662a`、`c23f8a56608b9aa32b3fa089aa5d54ec4b0385b990d0cc2fe695970db114fb85`。新倉 `layout=9`，`encoding=5`。套件仍是 `oo` 0.77.0。未推送。

---

## 13. R-2 驗收（驗收方填）

### 13.1 R-2 驗收：**受理**（兩個修補回合；兩者起因皆在驗收方）

交付 `a241593`：同一分支的多個本體是 `{ %pattern, %bodies: { "{<位址>}": 繭 } }`，只剩一個即該繭；施用時的資料鍵與 `unify` 裡的兩個規則繭只併進集合、不強制；分派選中的分支才逐一施用再相交。驗收方重量：全樹 ×3 **263／2541／0**（rc 皆 0，去耗時排序測試行 md5 三輪相同）、交叉編譯 0 error、conformance 162／162、`x: 0`／`1+1`／`1 + 1`／g4 根逐位元組不動、標準根 `7038e250…` available、`~%Math./add` 3／4、新倉 `layout=9`／`encoding=5`。差異純度乾淨。
旁量〔交付二進位〕：字面裡作用域不同的一對本體建構時不求值；被選中的分支效應照常；規格引用的例子（`/fib 5`、情境 A、`SPEC_06` 範例 A、`_:` 預設、`it`、三元展開）皆照新文字作答。
R2-Q3／R2-Q4 的陳述（字面與 `&` 位址不同、表上導航不一致）入佇列 Inbox。規格收尾於 spec `local 7fd48a3`。
