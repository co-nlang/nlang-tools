# 工單：頂層丟掉的東西（Q-072）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-072／裁定 `meta/oo/STATUS.md` **D90**（甲）／規格明文 `SYNTAX_03` §2 第 3 條（點路徑即巢狀座標）
> 既有規格：`SYNTAX_03` §2 第 4 條（四種錨）、§4 邊界 #1（三種 `_` 形）、#4（LHS `^` 廢止，2026-07-17）、#8（引號內的點屬於名字）；`SYNTAX_05` 所有權（`~%` 唯引擎鑄造）；`SPEC_09` §6（`~%Config` 旋鈕）
> 探針（已預先提交並校準）`crates/oo/tests/what_the_top_dropped_probe_test.rs`
> 基線：dev `32904dc`／`oo v0.73.0` ⟹ **6 綠 9 紅**，三輪一致；九支紅的都紅在一般斷言，零空洞讀數。

## 1. 缺陷

〔量，`v0.73.0` 標籤建置；v0.40.0、v0.60.0 同〕檔案**頂層**的多段鍵全部被**無聲丟掉**：
`a.b: 42`、`a.b.c: 42`、`@a.b: 42`、`#t.x: 42`、`~h.b: 42`、`/f.g: 42`、`%m.g: 42`、`1.a: 42`、`a.1: 42`、`_.a: 42`、`_.a.b: 42`。
`run --format` 得 `{}`、rc=0；`evolve` rc=0 而 `status` 不見它；提交後讀回是 `_`；`test` 說 `Evolving field: t.x` 然後讀不到它；`repl` 答 `no coordinate to observe: a.b`。
**衝突也被吞**：`a: { b: 1 }` ＋ `a.b: 2` 得 `a: { b: 1 }`，而 `a: { b: 1 }` ＋ `a: { b: 2 }` 是 `Evolution Conflict … #conflict at a.b`（rc=1）。
**同樣的鍵放進 Combo 裡全部正確**：`x: { a.b: 42 }` ＝ `x: { a: { b: 42 } }`，`x: { @a.b: 42 }`、`x: { #t.x: 42 }`、`x: { ~h.b: 42 }` 等皆然，巢內衝突照報 ⟹ **參照行為在引擎內已經存在，頂層是唯一離群者**。
〔讀〕`interpreter/src/universe.rs` 根折疊的寫入過濾只認單段裸鍵與 `~%Config.<knob>`，其餘當「不可寫」濾掉（`unreachable!("coords already filtered non-writable keys")`）；一次性求值路徑另有一處同樣丟。
〔量〕巢內的 `x: { _.a: 1 }` 被寫到 `x.a`（當相對路徑）——規格沒有這種讀法。
〔量〕**Combo 內也有一處不等價**：值裡的 `^` 從「寫下這個欄位的容器」數，而不是從路徑造出的容器數——`x: { y: 1, a.b: ^.y }` 得 `b: _`，`x: { y: 1, a: { b: ^.y } }` 得 `b: 1`；`a.b.c: ^^.y` 得 `#out_of_horizon`，巢狀寫法得 1。
夾具 229 個 `.n`、規格程式碼塊、既有 Rust 測試，**無一**在 LHS 用多段鍵或 `_.` 鍵。

## 2. 裁定與依據

*   **`SYNTAX_03` §2 第 3 條（明文 MUST，不需新裁定）**：`a.b.c: v` 與巢狀 Combo `{a: {b: {c: v}}}` 等價。
*   **D90（用戶，甲）**：**定義鍵一律相對於它所在的容器。** LHS 不得以 `_.` 起錨——與 `^`（2026-07-17 廢止）、`_{…}`（只在 RHS）同列。頂層 `_.a:` 只是 `a:` 的第二種拼法（一概念一拼法）；巢內 `_.a:` 會從字面量裡寫穿到 root，是 LHS `^` 廢止理由的原樣重演。**剖析器拒絕**，整個檔案不落地。
*   **不變的**：RHS 的 `_.`（從本宇宙 root 讀）；`_:`（root 這個鍵——其語義另有一列在 Inbox，**本弧不動**）；引號內的點屬於名字；`~%Config.<knob>` 的部分覆寫；`~%` 的所有權拒絕。

## 3. 射程＝不變式（不是機制）

*   **I1** **檔案頂層的欄位，與同樣的欄位放在 Combo 裡，行為相同。** 剖析器在頂層接受的每一種鍵形，落地的就是它定義上等於的那個巢狀 Combo：值相同（r2）、提交出來的根位址逐位元組相同（r1）、與兄弟欄位交集（r4，同一檔與跨提交）、衝突與巢狀寫法**同 rc 同輸出**（r3）。**修類別不修個案**：頂層的任何鍵形若沒有落地也沒有具名拒絕，都是本條的違反——請在 Q1 列出你檢查過的每一種鍵形。**值裡的相對引用（裸名、`^.`）照巢狀寫法解析：路徑造出的容器就是容器，`^` 要數它們——頂層與 Combo 內皆然**（r9；Combo 內今天就錯，見 §1）。
*   **I2** **每一個入口都一樣**：`run`（r2、r3）、`evolve`／`status`／`commit`（r1、r3、r4、r5）、`test`（r6）、`repl`（r7）。各入口的自述為真：說「演化了 `t.x`」就要真的有 `t.x`。
*   **I3** **D90：沒有任何定義鍵以 `_.` 起錨**——頂層、Combo 內、清單或任何運算式裡的字面量內皆然，包括 `_.:`。剖析錯誤、rc≠0；`evolve` 整個檔案不落地、`.oo/` 逐位元組不變；`run`／`fmt`／`eval` 同樣拒絕（r8）。拒絕要指出是哪一個鍵（位置或鍵的寫法），逐字見 Q3。
*   **I4** **豁免與既有拒絕不變**：`~%Config.<knob>` 照舊是部分覆寫（g1）；寫進引擎鑄造的 `~%` 軸照舊拒絕且不落地（g2）；RHS 的 `_.` 照舊從 root 讀（g3）；`"a.b"` 照舊是一個名字（g4）；Combo 內本來就對的照舊對（g5）；`^.`、`_{…}.`、`a[0]` 作鍵照舊被拒（g6）。
*   **I5** `repl` 對多段鍵答它的值，後面的行看得見它（r7）——Q-071 交付的 `no coordinate to observe: a.b` 那一格因此消失；`_.a:` 答剖析錯誤、會話繼續。
*   **I6** 不新增耐久檔、不改磁碟格式、不推進佈局；既有值的位址與提交位址不動。

## 4. 紅線（今天綠，必須保持綠）

*   g1–g6；`an_interactive_eval` 11、`the_universe_you_are_in` 14、`where_the_conflict_is` 9、`where_there_is_no_universe` 13、`one_writer_at_a_time` 10。
*   **交叉編譯** `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   conformance 162／162；`x: 0` 根 `31745ef0…`、`v: 1 + 1` 與 `1+1` 根 `f4f32e7b…`、標準根 `7038e250…`；`~%Math./add` (1, 2)=3、(1, 3)=4；新倉 `layout=8`／`encoding=5`。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** I1：頂層每一種鍵形（`Path` 的段數與各段的前綴／tag／數字、`Quoted`、`Pattern`、`Named` 若仍可達）各落地為什麼；有沒有哪一種在 Combo 內也沒有自然答案，你怎麼答（不得靜默）。
*   **Q2** 你把「頂層等於 Combo」做在哪裡：一處還是逐入口；**每個入口怎麼保證經過它**。`fmt` 對點路徑鍵的輸出是否改變（不要求改；陳述事實）。
*   **Q3** D90 的拒絕，逐字：頂層 `_.a: 1`、巢內 `x: { _.a: 1 }`、`_.: 1`，分別在 `run`、`evolve`、`repl` 裡。
*   **Q4** 值裡的相對引用：頂層與 Combo 內，`a.b: y`、`a.b: ^.y`、`a.b.c: ^^.y`、`a.b.c: ^.y` 各得什麼，與巢狀寫法逐一比較——必須相同（I1、r9）。另：閉合 Combo（繭）裡的點路徑鍵，路徑造出的中間容器是開還是閉——陳述你的選擇與理由，並說明與 `{| a: { b: v } |}` 是否相同。
*   **Q5** 事實陳述：`_: { a: 42 }` 在頂層今天落地為什麼（**不要改它**，Inbox 另有一列）。

## 6. 明文不在射程內

*   `_:`（root 這個鍵）的語義——Inbox 另列。
*   LHS 的 `[expr]` 段（動態鍵）——照舊拒絕，不賦予語義。
*   `lint` 的圖近似（點路徑鍵不入圖）。
*   規格文字（`SYNTAX_03` §4 邊界 #1 改寫、`SPEC_14` §2.2 `field_key` 收窄到相對靜態路徑）、`CHANGELOG`：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針 r1–r9 基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。**
*   措辭無關：每支紅探針都拿點路徑鍵與它定義上等於的巢狀寫法，經同一個指令、在兩個放著同名檔案的目錄裡比較；提交以根位址（`oo inspect <HEAD>` 的 `root:`）逐位元組比；`.oo/` 逐位元組比。`rustfmt` 不得掃探針檔。
*   〔驗收方校準〕**兩極皆做**：
    基線 6 綠 9 紅（三輪一致）。**初版兩處錯，已修**：g1、g3 用 `run --format` 讀運算式的值，而 `--format` 不強制 `+ 1`——基線上就紅；改用 `run --observe <路徑>`。
    一個最小參考實作（剖析器 `field_key` 拿掉 `field_root_path` 並對 `_` ＋ `.` 加否定前瞻；`main.rs` 在 `fmt` 以外的入口把頂層多段裸鍵 `a.b…: v` 改寫為 `a: { b…: v }`，`~%` 起首者不動）；Combo 內同樣把多段鍵改寫成巢狀 Combo 再求值 ⟹ 本探針 15／15。**只拿掉 `field_root_path` 不夠**：`_.a` 會改從一般 `path` 以「第一段叫 `_`」進來，落地成名叫 `_` 的座標——參考實作第一版就是這樣，r8 擋下。**Combo 內只降一層也不夠**：`--format` 把未強制的內層原樣印成 `b.c: 42`——參考實作第二版就是這樣，g5 擋下。
    **六個變異各自讓一支守衛轉紅**：`~%Config.fuel` 也被降 ⟹ g1；拿掉 `~%` 所有權檢查 ⟹ g2；RHS 也不准 `_.` ⟹ g3；引號鍵裡的點拆成路徑 ⟹ g4；Combo 內的改寫丟掉內層 ⟹ g5；鍵接受 `^.` ⟹ g6。
    **無須預先修訂任何既有測試**（grep 過 LHS `_.` 與頂層多段鍵：夾具、規格程式碼塊、Rust 測試皆無）。
    同一參考實作跑全樹：**257 target／2481 passed／0 failed，exit 0**（＝ v0.73.0 的 256／2466 加上本探針 15 支）；交叉編譯 0 error。
    **參考實作是在入口逐一包一層——那是驗收方為了校準的捷徑，不是設計，不要照抄**（I2、Q2）。

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 做了什麼

點路徑鍵與它定義上的巢狀 Combo 同一條路：`expand_dotted_field` 把 `a.b.c: v` 展開成 `a: { b: { c: v } }`，中間容器是開的。`Universe::evolve` 與 Combo 字面量建構各呼叫它一次。`run`、`evolve`、`status`、`commit`、`test`、`repl`、`eval` 的寫入都經過 `evolve`；值裡的 Combo 經過字面量建構，所以 `^` 數的是路徑造出的那幾層容器。

`~%` 起首的鍵不展開。`~%Config.<knob>` 仍是部分覆寫。其餘 `~%` 仍在演化邊界拒絕，檔案不落地。

D90：`parse_field` 拒絕錨在根上的定義鍵，錯誤帶鍵的寫法與位元組位置。`_.` 在右值仍從根讀。

`_:` 未改。版本仍是 `oo v0.73.0`。沒有新耐久檔，佈局與編碼仍是新倉的 `layout=8`／`encoding=5`。三個根位址與工單相同。

### 8.2 順手改動（逐項指名）

1. `crates/interpreter/src/universe.rs`：`expand_dotted_field`；`evolve` 在所有權檢查之後展開多段裸鍵。`@{expr}` 改走與 Combo 字面量相同的單欄位建構再落入根。
2. `crates/interpreter/src/eval.rs`：Combo 字面量每個欄位先經同一個展開。
3. `crates/parser/src/lib.rs`：根錨定義鍵的拒絕。`crates/parser/src/n.pest` 只改了旁邊那兩行註解。
4. `crates/oo/src/main.rs`：`repl` 對多段裸鍵讀該路徑的葉；沒有單一讀回路徑、但有新座標時，印那些新座標。

探針、規格、夾具、`Cargo.lock`、版本都未改。

### 8.3 工單哪裡是錯的

無。工單寫的參考全樹是 257 target、2481 passed、0 failed。這次三輪的 `test result:` 與此相同。

### 8.4 工單指名要你回答的問題

**Q1** `field_key` 的順序是 `anon_set | field_root_path | path | named_key | multiline_str | quoted_key | tag`。`path` 在 `named_key` 之前，所以使用者寫的一段鍵是 `Path` Bare。下列是對這支剖析器與 `run --format` 量的。

多段裸鍵展開後與巢狀寫法同一份 `run --format`、同一個提交根（r1、r2）：

| 頂層鍵 | 落地 |
| :-- | :-- |
| `a.b: 42`、`a.b.c: 42` | 資料軸上的巢狀 Combo |
| `@a.b: 42` | 型別軸 `@a` 裡的 `{ b: 42 }` |
| `#t.x: 42` | 資料鍵 `"#t"` 裡的 `{ x: 42 }`（`#t` 是 `path` 的一段；`tag` 規則排在 `path` 之後） |
| `~h.b: 42`、`a.~h: 42` | 私有軸 |
| `/f.g: 42` | 規則軸 |
| `%m.g: 42` | 元軸 |
| `1.a: 42`、`a.1: 42` | 段文字就是座標名 |
| `a._: 42` | `{ a: { _: 42 } }` |

一段裸鍵、引號鍵（`"a.b"`、`"""a.b"""`）照舊是一個名字。引號裡的點不拆（g4）。

`Named` 從這套文法到不了。臂仍在，寫入規則未改。

`@{1}: 42` 是 `Pattern`。頂層與 Combo 內用同一套建構（`%morphism: #true`，座標是模式求值後的字）。落在根上時，純包裝 `%val` 會被根的交疊剝掉，`run --format` 是 `1: 42`；這與頂層 `a: {{ %val: 42 }}` 印成 `a: 42` 相同。放在 `x: { @{1}: 42 }` 裡時格式仍看得到 `{{ %val: 42 }}`，`run --observe x.1` 是 `42`。`repl` 印新座標：`=> 42` 與 `=> #true`。

`~%Config.fuel` 仍是部分覆寫（g1）。`~%Foo.b` 仍是 `#system_reserved`，`run` 與 `evolve` 都 rc≠0，`.oo/` 不變（g2）。

`_.a`、`_.a.b`、`_.:`、Combo 內與清單內的 `_.a` 在剖析器拒絕，整份檔不進 `evolve`。

**Q2** 展開函式在 `universe.rs` 的 `expand_dotted_field`，一處。`evolve` 開頭呼叫它，所以 `run`、`evolve`／`status`／`commit`、`test`、`repl`、`eval` 都經過。Combo 字面量在求值每個欄位前呼叫同一個函式，所以巢內與頂層同一條規則。`fmt` 只印剖析樹，不展開：`a.b.c: 42` 的 `oo fmt` 仍印

```
a.b.c: 42
c: 1
```

rc 0。

**Q3** 拒絕句是 `definition key may not anchor at the root: <鍵> (at byte <位元組>)`。

| 輸入 | `run`／`evolve` | `repl` |
| :-- | :-- | :-- |
| `_.a: 1` | rc 1，`Parse Error in "…/t.n": definition key may not anchor at the root: _.a (at byte 0)` | rc 0，`Parse Error: definition key may not anchor at the root: _.a (at byte 0)`，會話繼續 |
| `x: { _.a: 1 }` | rc 1，同一句，鍵是 `_.a`，`at byte 5` | rc 0，`Parse Error: … _.a (at byte 5)` |
| `_.: 1` | rc 1，鍵是 `_.`，`at byte 0` | rc 0，`Parse Error: … _. (at byte 0)` |

`fmt` 是 rc 1，`Parse Error: definition key may not anchor at the root: …`。`oo eval '{ _.a: 42 }'` 是 rc 1，`Parse error: definition key may not anchor at the root: _.a (at byte 2)`。`evolve` 拒絕後該目錄沒有 `.oo/`。

**Q4** `run --observe` 與巢狀寫法相同：

| 寫法 | 觀測 | 結果 |
| :-- | :-- | :-- |
| `y: 1` 然後 `a.b: y`，以及 `a: { b: y }` | `a.b` | `1` |
| `y: 1` 然後 `a.b: ^.y`，以及 `a: { b: ^.y }` | `a` | `{ b: 1 }` |
| `y: 1` 然後 `a.b.c: ^^.y`，以及 `a: { b: { c: ^^.y } }` | `a` | `{ b: { c: 1 } }` |
| `y: 1` 然後 `a.b.c: ^.y`，以及 `a: { b: { c: ^.y } }` | `a` | `{ b: { c: _ } }` |
| `x: { y: 1, a.b: ^.y }` 與 `x: { y: 1, a: { b: ^.y } }` | `x.a` | `{ b: 1 }`（r9） |
| `x: { y: 1, a.b.c: ^^.y }` 與巢狀三層 | `x.a` | `{ b: { c: 1 } }`（r9） |

繭的中間容器是開的。`x: {{ a.b: 42 }}` 的 `run --format` 與 `x: {{ a: { b: 42 } }}` 相同，外層閉、內層 `{ b: 42 }` 開。`x.a.z` 兩邊都是 `_`。`x: {{ a: {{ b: 42 }} }}` 的 `x.a.z` 是 `_|_  ;; %cause: #missing_key`。所以與 `{| a: { b: v } |}` 相同，與內層也是繭的寫法不同。選開的理由：路徑只多造容器，閉合是寫下那個欄位的那一層繭自己的事。

**Q5** 頂層 `_: { a: 42 }` 今天落在資料座標 `_`，值是 `{ a: 42 }`。`run --format` 與 `evolve` 之後的 `status` 都印這份 Combo。本弧沒有改它。

### 8.5 探針

`crates/oo/tests/what_the_top_dropped_probe_test.rs` 未改，未 rustfmt。全樹第三輪 15／15，2.42s。

紅線，全樹第三輪皆 `test result: ok`、0 failed：`an_interactive_eval` 11（0.75s）、`the_universe_you_are_in` 14（1.34s）、`where_the_conflict_is` 9（0.69s）、`where_there_is_no_universe` 13（3.73s）、`one_writer_at_a_time` 10（17.64s）。

交叉編譯 `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu`：`Finished` 5.02s，`^error` 0。

### 8.6 數字

1. 三輪 `cargo test --workspace --release --offline --no-fail-fast --jobs 1 -- --test-threads=1`：每輪 rc=0，`^error` 0，`test result:` 257 行，2481 passed，0 failed。`running`／`test `／`test result:` 三輪各 2995 行，去掉耗時後相同。
2. conformance 162／162，rc=0。cwd 是 `/home/gali/nlang`。
3. `~%Math./add (1, 2)` → 3，`(1, 3)` → 4，rc=0。這兩次 `eval` 沒有建立 `.oo/`。euid 1000。
4. `x: 0` 根 `31745ef0e8bfde3d8a2673b7dce5bb5cd74f3a7f2cc6f5422aa043c8dce5589a`。`v: 1 + 1` 與 `1+1` 根 `f4f32e7bc4ebcdd3ae23b10128e99a4b7d71996d236a161cb00849e6451c04d1`。標準根 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`（`status` 該行帶 `(available)`）。新倉 `.oo/format` 為 `layout=8`，`.oo/objects.format` 為 `encoding=5`。

### 8.7 你認為需要改規格之處

規格檔這次沒有改。建議驗收方依工單第 6 節改 `SYNTAX_03` §4 邊界 #1，以及 `SPEC_14` §2.2 把 `field_key` 收到相對靜態路徑，並寫上 Q3 的拒絕句。

---

## 9. 驗收（驗收方填）

**受理，零修補回合。** 交付 `b92ebd1`：探針、分隔線以上、`Cargo.lock`、版本皆未動。
**交叉編譯**（`touch` 後強制重查）0 error。**全樹 ×3**（期間不跑其他量測）：**257 target／2481 passed／0 failed，`^error` 0，exit 0**，三輪逐行相同。
**驗收方矩陣**〔交付二進位〕：`a.b: 42`＋`c: 1` 經 `evolve`／`status`／`commit` 後 `_.a.b` 為 42；再提交 `a.c: 9` 得 `_.a` ＝ `{ b: 42, c: 9 }`；`a.b: 43` 對 `HEAD` 報 `#conflict at a.b`、rc=1；含 `_.d: 7` 的檔整個拒絕並指出 `_.d (at byte 13)`。頂層 `@{1}: 42`＋`k: 5`：v0.73.0 `run --format` 只得 `k: 5`，交付得 `%morphism: #true`／`1: 42`／`k: 5`，提交後三者皆讀得回。
**報告一處與事實不符**：Q3「`evolve` 拒絕後該目錄沒有 `.oo/`」——在沒有宇宙的目錄裡，被拒絕的 `evolve` 會留下 `.oo/`；v0.73.0 對普通剖析錯誤（`a: (`）相同 ⟹ 既有行為、非本弧退化，記入 Inbox。
身分：`31745ef0…`／`f4f32e7b…`（`1 + 1` 與 `1+1`）／標準根 `7038e250…`；known-answer 3／4；conformance 162／162；新倉 `layout=8`／`encoding=5`。
