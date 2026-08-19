# 偵察 Q-033：只有一台引擎造得出的根 —— a root only one engine can build

> 開單 2026-08-19。基線：`nlang-tools dev e1c018b`（tie-back `v0.26.0`，
> `crates/oo/Cargo.toml` = `0.26.0`）；規格 `nlang-spec local 559277b`
>（`v0.26.0-draft.1` 之帳務收尾）。
>
> **本單只偵察，不改規格正文、不改引擎、不先替用戶裁定。**
> 出口是四則可裁的問題，以及裁後可拆成 implementation 工單的邊界。

---

## 1. 一句話

REAL_03 §6.8.2 已連續五版要求「標準根是核心規格定義的具體值，CAID 由規格公佈並與版本
綁定」，但 v0.26.0 的真相仍只活在 `root_with_system()` 的 Rust 建構程序裡。

Q-032 已經讓標準根成為可定址的 CAS 物件，卻還沒有讓另一個實作**造得出那個物件**。
現在磁碟上可以找到 `2da5b713…`，規格讀者仍無法由規格得到 `2da5b713…`。

這不是「把 digest 抄進表格」就完成。至少還有三個未裁邊界：

1. `@type` 的三種角色重疊，連「哪些東西屬於標準根」都未定；
2. v0.26.0 標準根含規格未列、拼法不同與已被別節否定的內容；
3. 現有印出格式與 L3 runner 都不能承載可重造的 manifest。

---

## 2. 先拆掉一個同名誤會：這裡有兩個 L3

| 名字 | 實際意思 | 今天的缺口 |
| :--- | :--- | :--- |
| REAL_03 §6.8.2 的「第三層」 | 標準根的**內容定義**；其他實作據此重造同一 digest | 沒有 manifest，正是 Q-033 |
| REAL_05 的 **Level 3** | Ouroboros／耐久互通符合性層級 | 向量為零；runner 只跑 L1／L2 |

所以「標準根清單卡在 L3 零向量」把因果倒過來了：**第一個標準根 manifest／CAID
本身應該成為第一個 Level 3 向量的輸入**，而不是等待一套尚不存在的 L3 向量先替它定義。

〔量〕規格倉的 `scripts/run-conformance.py` 目前把 level 寫死為 L1／L2；向量介面只有
`oo run FILE --observe out`。它不能建立／讀取 store，也不能核對標準根物件或 commit。

---

## 3. v0.26.0 的真標準根是什麼

以空倉 `evolve`＋`commit`，從 root 的 `standard_root` 指名回查 CAS：

| 量 | v0.26.0 現值 |
| :--- | :--- |
| 標準根 CAID | `2da5b71371649291cfa5dc5d0cd019464d248e98645b3901938e1c08d2172c2c` |
| packed 物件 | 137,028 bytes；外層是 JSON string `standard-root:<hex JSON>` |
| 解碼後 JSON payload | 68,506 bytes |
| `oo inspect` 顯示 | 21,913 bytes（另含 CAID／MASA 表頭） |
| 頂層 data／meta／local | 空 |
| 頂層 rules | `/add` |
| 頂層 types | `list`、`option`、`result` |
| 頂層 system | 26 個模組 |
| builtin 引用 | 256 筆、251 個不同 builtin id |
| 非 `#pure` 的耐久效果 | `#io` 20、`#nondet` 1、`#state` 5 |

26 個 system 模組逐字為：

```text
Bytes Complex Cond Config Csv Diff Discovery Effect Engine Env Io Json List Math
Official Path Process Query Reflection Regex Set Stat String Time Toml Url
```

builtin 引用依 registry 前綴計數：

```text
bytes 12   complex 4   cond 3    csv 4       diff 3     disc 5
effect 1   engine 12   env 3     io 4        json 4     list 41
math 49    option 8    path 5    process 2   query 4    refl 17
regex 4    result 8    set 8     stat 6      str 33     time 9
toml 2     url 5
```

這張表是**現實盤點**，不是規範清單。尤其 packed JSON 是 Q-012 預定替換的儲存編碼，
不能因為今天碰巧可解碼，就倒升成語言規格。

---

## 4. 「型別三族群」不是三張互斥清單

現樹至少有三層角色，而且互相重疊：

| 層 | 現樹內容 | 性質 |
| :--- | :--- | :--- |
| 約束／validator 名字 | `any num complex float int str bool list combo morphism option result` | `TypeConstraint` 的 12 個保留名；裸單段 `@name` 可短路成 marker |
| 標準根豐富型別值 | `list option result` | 有 `%fmap` 等欄位，住在標準根 types 軸 |
| 階層／名義名字 | §2.1 的 `unit record type caid`、定寬整數，以及任意未知名 | `super_parent` 認得階層；多數沒有 validator，未知名落到 `combo` |

`list`／`option`／`result` 同時屬前兩層。因此「保留名／標準根型別／名義開放」不能只靠
把名字分進三個桶解決；必須先裁**同一拼法是否本來就允許代表兩個值**。

### 4.1 同一拼法，今天真的有兩個身分

〔量，v0.26.0〕：

| 式 | 結果 |
| :--- | :--- |
| `@list` | 約束 marker `{{ %kind: #type, %name: "list" }}` |
| `@list.%fmap` | 標準根的豐富型別節點，可取得 `list.map` |
| `(@list).%fmap` | `_` |
| `@list.%id` | `c596dc99…`（豐富節點） |
| `(@list).%id` | `2a43f297…`（marker） |
| `@list = (@list)` | `#true` |

`@option`／`@result` 同樣各有兩個 CAID。也就是解析／投影深度會決定同一字面名稱落在哪一
個值；相等判斷又把兩者說成相等。這若是設計，規格必須明說；若不是，manifest 不能把它
凍結成既成事實。

另外，Q-032 解封的是使用者座標：使用者可定義 `@int.mine`、`@unit.mine`、`@u8.mine`、
`@list.mine` 甚至 `@zzz.mine`。但裸單段 builtin 名仍先短路成 marker。故「不可遮蔽」目前
不是整個名字空間的性質，而是**某一種觀測形的優先序**。

### 4.2 兩張規格表與現樹仍未對齊

- SPEC_09 §2.1 型別樹有 `unit`、定寬整數、`record`、`type`、`caid` 等，沒有
  `option`／`result`。
- SPEC_09 §2.5 的錨點表有 `list`／`option`／`result`／`morphism`／`num`，CAID 仍是
  placeholder。
- 標準根 types 軸只有 `list`／`option`／`result`。
- 引擎 validator 保留集合則是上表 12 名。

所以 Q-033 不能先挑其中一張表抄成 manifest；那只會把另一張表與現行 validator 留成
下一筆舊帳。

---

## 5. 標準根盤點也不是把 26 個模組全數蓋章

現值與規格可見名稱已有直接反例：

| 觀測 | 結果／含義 |
| :--- | :--- |
| 規格使用 `~%Str` | 現引擎 `~%Str` → `_`；標準根實際是 `~%String` |
| 規格提到 `~%System`、`~%Logic`、`~%Repl` | 現標準根皆無；觀測為 `_` |
| 標準根有 `~%Official` | 是空的閉合 combo `{{ }}` |
| SPEC_13 要求 reference implementation 不得本地合成 `~%Official` | 現值仍保留一個本地空殼；不能未裁就把它定為規範內容 |

此外 `Bytes`、`Complex`、`Diff`、`Io`、`Json`、`Path`、`Query`、`Reflection`、`Regex`、
`Set`、`Stat`、`String`、`Toml`、`Url` 在核心規格正文沒有同名模組的規範性清單。
它們可能值得成為標準庫，卻不能只因 reference engine 已經 ship 就自動取得憲法地位。

---

## 6. manifest 今天沒有可直接沿用的輸出形

對 v0.26.0 `root_with_system()` 做兩個只讀 round-trip 探針：

1. `Value::to_string_plain()` 把 combo 縮成 `{...}`，parser 在第 1 行即拒絕；
2. `Value::to_nlang(0)` 展開全文，但列到 `/%differential.1:` 時 parser 回
   `expected field`。

後者揭示的不是單一 printer bug：標準根含 parser 不能直接當欄名接回的內部座標。
因此「由引擎印一份 `.n` 放進規格」目前不可用。

可裁的發布策略至少有三種：

| 選項 | 做法 | 代價／風險 |
| :--- | :--- | :--- |
| A. 凍結 v0.26.0 全量 | 26 模組、251 builtin id 全部成為規範 | 最便宜；同時把 `Str/String`、空 `Official` 與未規範累積物一併蓋章 |
| B. 裁出最小核心根 | 只保留核心規格真正承諾的值，其餘另遷 | 最乾淨；標準根 CAID 再移動，須靠 Q-025 的歷史列保留舊根 |
| C. 核心根＋外部分發擴充 | 標準根只定核心，其他模組成為版本化擴充 | 可分清規範層；必須證明不偷偷造出第二個「標準根」而違反 O52 |

無論選哪一個，發布 artifact 仍須同時滿足：

- 人能審閱每個座標與其規範性；
- 另一實作不依 Rust registry 或 JSON 容器即可重造值；
- 一份機器向量固定 canonical bytes／CAID；
- manifest 的 schema／排序／字串與特殊欄名 escaping 本身版本化；
- 舊版標準根 digest 留在 `shipped_standard_roots()` 歷史列。

目前最少矛盾的候選形不是「一個 dump」，而是**語義 manifest ＋ canonical vector**兩件：
前者定座標和值，後者固定重造後的 CAID。是否採用，留給本單裁定。

---

## 7. 最小 Level 3 向量要驗什麼

第一個向量不能只斷言 reference engine 自己寫出的十六進位字串。最小跨實作契約應含：

1. 讀取指定規格版本的標準根 manifest；
2. 重造 canonical value；
3. 得到指定標準根 CAID；
4. 驗證少量 landmark（各軸、型別節點、效果欄位與特殊名字），讓錯誤不是只剩 digest mismatch；
5. 用該 digest 建一個最小 root／commit，再由另一份實作或獨立 vector reader 回讀。

現有 runner 無法表達第 1、3、5 項。裁後 implementation 必須先選：

- 擴充 vector metadata 與 runner，讓它能要求 `standard-root`／store 操作；或
- 增加一個規範性 CLI／觀測面，把標準根 CAID 變成 L3 runner 可比較的輸出。

只加一支 reference-engine Rust test 不算 Level 3：它仍只證明同一份 Rust 能重演自己。

---

## 8. 交付用戶裁定的四題

### D1 — 型別名字的模型

裁 `@list` 這類名字是：

- **雙角色是設計**：marker 與豐富節點各自有明文解析／投影規則；或
- **單一值**：約束能力與標準根欄位合成同一個可觀測值。

同時裁未知 `@zzz` 是開放名義型別，還是必須拒絕。兩件不能分開只修表格。

### D2 — 哪些內容取得規範地位

在 §6 的 A／B／C 中選一路；並逐案處理 `/add`、`Str/String`、`Official` 與未成文模組，
不得以「都先保留」代替裁定。

### D3 — 規範 artifact

裁 manifest 的規範來源與 canonical vector 的格式；不得使用現行 packed JSON，也不能假定
`to_nlang()` 已可 round-trip。

### D4 — 第一個 Level 3 runner 契約

裁 vector 是擴充 runner 直接操作 store，或新增規範性觀測面；最低驗收採 §7 五項。

---

## 9. 本偵察的完成條件與明確不做

### 9.1 完成條件

- [x] 盤點 v0.26.0 標準根的真 CAID、軸、模組、builtin 與效果數量。
- [x] 證明型別不是三個互斥清單，並量到同名雙身分。
- [x] 列出規格名稱與現值的具體反例。
- [x] 實測兩個既有 printer 都不能作 round-trip manifest。
- [x] 定義第一個 Level 3 向量至少要驗的性質，並指出 runner 缺口。
- [ ] 用戶裁 D1–D4；裁後才可拆 implementation 工單與校準紅探針。

### 9.2 明確不做

- 不在偵察期改 SPEC_09／REAL_03／REAL_05 正文。
- 不改 `root_with_system()`、型別解析、守衛或 builtin registry。
- 不先重算／發布新的標準根 CAID。
- 不把 Q-034（`#23`／效果守衛）或 Q-035（使用者 `%builtin` 偽造）混進本單。
- 不為 `/add`、三個標準根型別或四個拼法差異寫名字特例。

**狀態：偵察已開，量測完成；停在 D1–D4 的用戶裁定前。**
