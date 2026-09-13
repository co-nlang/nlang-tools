# 工單 — The world would not say（Q-046）

**日期**：2026-09-13（**2026-09-13 就地改版**，見 §0）
**引擎起點**：Q-045 收弧後的 `dev`／**規格起點**：v0.49.0-draft.1
**裁定**：**D67**（重裁 D66）。本弧是它的落地，**不需要新的裁定**。
**探針**：`crates/oo/tests/the_world_would_not_say_probe_test.rs`（驗收方已寫並校準，**基線 3 綠 3 紅**）

---

## 0. 這份工單改過一次，改的是載體

**初版依 D66 寫成「EACCES ⟹ `#blur`、ENOTDIR／ELOOP ⟹ ⊥」。那張表有兩處錯，已由 D67 更正。**
若你已經照初版開工，**要改的是載體，不是結構**——三分仍然成立，探針 R1／R3 一字未動。

**變更**：EACCES ⟹ **⊤**（原 `#blur`）／ELOOP ⟹ **⊤**（原 ⊥）／
**ENOTDIR 整格移出射程**（它本來就該是 `#false`）。
**連帶**：初版紅線「`%cause` 的拼法進位址」**在 ⊤ 之下不再成立**（見 §4 R2）。

---

## 1. 這一弧是什麼

〔量 2026-09-13，Q-045 收弧後的二進位，`oo eval`，每格帶對照〕

| | ENOENT | EACCES | ENOTDIR | ELOOP | store 邊界 |
| :-- | :-- | :-- | :-- | :-- | :-- |
| `~%Io./exists` | `#false` | **`#false`** | `#false` | **`#false`** | `_\|_ ;; %cause: #store_boundary` |
| `~%Io./read_file` | `#none` | **`#none`** | `#none` | **`#none`** | 同上 |

**而模範就在同一個函數裡**：store 邊界那條路給的是**載體 ＋ 登記過的成因**，
只有宿主錯誤那條路塌成一個值。**這一格是這一族唯一把壓平後的結果當成值交給 n/ 程式的。**

### 1.1 D67 怎麼切：按問句，而且「沒答案」落在格的欠定端

| 情形 | 問句 | 載體 | 備註 |
| :-- | :-- | :-- | :-- |
| 可回答 | 好 | 值本身 | 不動 |
| ENOENT | **好，答案是「沒有」** | `#false`／`#none` | **不動** |
| **ENOTDIR**（`a/b` 而 `a` 是普通檔） | **好，答案是「沒有」** | **`#false`／`#none`** | **不動**——對任何人、任何時候都不可能有東西在那，`#false` 是真話 |
| **斷掉的符號連結** | **好，答案是「沒有」** | **`#false`／`#none`** | **不動**——`try_exists` 是 `Ok(false)` 不是 `Err`，宿主解析了而且找不到東西 |
| **EACCES／EIO** | **好，世界不肯說** | **⊤ ＋ 登記過的成因** | **本弧** |
| **ELOOP** | **沒有答案：純引用環** | **⊤ ＋ 登記過的成因** | **本弧** |
| store 邊界 | 你不可以問 | ⊥ ＋ `#store_boundary` | **不動（紅線）** |

### 1.2 為什麼「沒答案」是 ⊤ 而不是 ⊥

**⊥ 說的是「這個座標是過定的／矛盾的」**——那是 `1 & 2` 給你的東西。
**檔案系統從來不給你矛盾的資訊：它要嘛告訴你，要嘛不告訴你。**

三個可檢查的理由：

1. 〔量〕**⊥ 是 meet 的零元**：`(1 & 2) & 1` ⟹ `_\|_`。用 D65 的話，**值會旅行**
   ⟹ 一個操作者的權限問題會**吃掉**別人的資料。〔量〕**⊤ 是 meet 的單位元**：
   `_ & 1` ⟹ `1` ⟹ **它誰也不污染。**
2. **單調性**：權限修好之後同一個運算式給 `#true`。`⊥ → #true` **不是合法的收窄**；
   `⊤ → #true` 是。**取 ⊥ 會把一個正常的未來寫成一次規格違反。**
3. **⊤ 什麼都不承諾**，而這正是這裡需要的。

### 1.3 為什麼不是 `#blur`

`TAG_REGISTRY` §2.7.3 逐字：**`#blur` 宣稱一個可定址的快照**（`SPEC_08` §3.2.1）。
〔量〕確實如此——`~%Config.fuel: 0` 之下連 `c: 3` 都印
`#blur { %cause: #fuel_exhausted, %caid: "hash:sha256:v1:629cf304…" }`。
**一次 EACCES 沒有任何被部分觀測到的東西可以定址。**
且 §1.2 給 `#blur` 的修復建議逐字是「請增加 `%fuel` 配額」——**對權限被拒是無效補救**，
而 §2.7.1／§2.7.2／§2.7.3 三則命名裁定**是同一條律**：不得把無效的補救遞給操作者。

### 1.4 為什麼 ELOOP 是 ⊤ 而不是 ⊥：登記簿已經替循環畫過這條線

| 標籤 | 軸·載體 | 說明 |
| :-- | :-- | :-- |
| `#divergent` | 原因·**⊥** | 動態非終止，「檢測到**含變換**的循環定義」 |
| `#static_cycle` | 來歷·**Top** | **純引用閉環**，「值本身是合法 `_`（最大疊加態）」，**非錯誤** |

**符號連結環是一個名字指向一個名字——純引用環，沒有變換** ⟹ 它是第二個。
且〔量〕`symlink_metadata` 對它是 `Ok` ⟹ **那條路徑上確實有東西**，
所以 `#false`（「沒有東西」）**是假話**——這才是 ELOOP 進本弧的理由。

---

## 2. 射程 ＝ 兩條不變式

### I1 `~%Io./exists` 必須把「沒有答案」與「答案是沒有」分開

依 §1.1 的表。**EACCES／EIO 與 ELOOP 不得與 ENOENT 共用答案，且必須帶登記過的 `%cause`。**

### I2 `~%Io./read_file` 同樣，因為它也是一個觀測

`#none` 保留給 ENOENT／ENOTDIR／斷連結。

**⚠ 兩條都要求一個全函數的對映。** 每一個 `io::ErrorKind` 都要落進 §1.1 某一列，
**沒有落點的那一類必須有一個總括答案**（Q-042 的 `_ => "unreadable"` 是可抄的形狀）。
探針只測代表，**驗收方會另造沒被探針測過的 errno**（至少 `ENAMETOOLONG` 與一個目錄形）。

---

## 3. 明確不做

1. **`~%Io./write_file` 與 `~%Io./append_file`**：〔量〕失敗時一律 `#none`——同一個塌陷，
   **但它們是動作不是觀測** ⟹ **O87**，本弧不動。
2. **ENOTDIR 與斷掉的符號連結**：`#false`／`#none` 是真話，**G3 是這條紅線**。
3. **store 邊界那條路一字不動**（G2 是紅線）。
4. **`REAL_03` §6.6 第四款**（O86 (i)）／**`.oo/peers` 冷啟動**（需裁定）／
   **`DiscoveryConfig` 洩裸 errno**（Inbox）。

---

## 4. 紅線

**R1 不得造新的 tag 值（D66／D67 皆逐字）。** 差異記在 `%cause`；⊤ 是既有居民。

**R2 ⟵ 已改：`%cause` 的拼法在 ⊤ 之下不進位址，但這要你複驗。**
初版寫的是「進 blur CAID，出貨後改名會移動位址」——**那是 `#blur` 載體的前提**。
`#static_cycle` 的登記條目逐字寫著 Top 的成因**「不參與等值/CAID、不傳播（消費即蒸發）」**，
`REAL_03` §6.9 第二款亦允許因果通道不入址。
**⟹ 必答**：你的 ⊤ 成因**有沒有**進 CAID？以量測回答，不要以引用回答。

**R3 非紀元：不得移動任何既有位址。** 標準根 `7038e250…`、`x: 0` 根 `31745ef0…`、
物件 3、`layout=5`、`encoding=5` 皆不動，conformance 162／162。

**R4 成功路徑與「答案是沒有」那三格的語義不變**（G1／G3）。

**R5 探針不得改動，也不得被 `rustfmt` 掃到。** 認為探針寫錯 ⟹ §N.3 說明並保持它紅。
（**前例就在本工單**：驗收方初版的 R2 斷言 ENOTDIR 必須與 ENOENT 不同——**那是錯的**，
已由驗收方自己更正為 G3。若你看出類似的錯，說出來。）

**R6 離開碼不得在管線之後取。**

---

## 5. 探針（驗收方所寫，基線 **3 綠 3 紅**）

| | 名稱 | 現況 | 釘什麼 |
| :-- | :-- | :-- | :-- |
| G1 | `g1_the_answerable_questions_keep_their_answers` | 綠 | 紅線 R4 |
| G2 | `g2_the_store_boundary_still_answers_the_way_it_already_did` | 綠 | 紅線：模範不得變 |
| **G3** | `g3_a_path_that_cannot_hold_anything_is_simply_absent` | 綠 | **紅線**：ENOTDIR 與斷連結**必須等於** ENOENT |
| **R1** | `r1_exists_separates_absent_from_unreadable` | **紅** | I1，EACCES |
| **R2** | `r2_a_symlink_loop_is_not_an_absent_file` | **紅** | I1，ELOOP |
| **R3** | `r3_read_file_separates_absent_from_unreadable` | **紅** | I2 |

探針比較的是**去掉 `;; %effect: #io` 之後**的答案；每支紅探針都在同一次執行內帶 ENOENT 對照組；
夾具開頭有 **root 守衛**（`chmod 000` 之後證明拒絕真的咬到）。

---

## 6. 請在交付報告裡回答（§N.4）

1. **⊤ 的成因今天印不印得出來？** 〔量〕`~%Io./exists` 的 EACCES 答案若是 `_` 而成因不顯示，
   探針 R1 的 `%cause` 斷言就過不了。D65 已裁**成因走註解層**（`;; %cause:`），
   而它今天**只對 ⊥ 實作**——⊤ 那一半是 D65 的欠帳。你補了哪些？
2. **一個帶成因的 ⊤ 會不會在提交時被當成單位元丟掉？**
   〔量，驗收方〕`plain: _` **整格不進根**（裸 ⊤ 不留痕）。
   **這是本弧最會咬人的一格**：成因若在提交時蒸發，這個弧就白做了。
3. **R2 的必答**：你的 ⊤ 成因進不進 CAID？量給我看。
4. **你的對映是全函數嗎？** 列出每一個 `io::ErrorKind` 落在 §1.1 哪一列，
   以及沒有落點的那一類回什麼。**`ENAMETOOLONG` 明確說明你放哪一列，為什麼。**
5. **成因的拼法。** 驗收方**不指定**——`#static_cycle` 是 ELOOP 的同形先例，
   EACCES 那一格請提議一個並說明依據。**`TAG_REGISTRY` §2.7 的體例是每一則命名
   裁定都附一個論證性量測**，並逐字寫「拿掉這個量測，上列就只剩命名偏好」。
6. 任何你認為驗收方寫錯的地方，§N.3 說，不要自己改。

---

## 7. 驗收會怎麼做

1. diff 純度、探針完整性（`git diff` 為空、`rustfmt` 未掃）。
2. **全工作區 ×3**，`--release --no-fail-fast`，逐標的聚合，保留失敗的測試名。
3. **另造探針沒測過的 errno**，檢查 §6.4 的全函數宣稱。
4. **身分紅線與 conformance**，真二進位、known-answer 帶對照。
5. **跨版本雙向**。
6. **成因是否隨值旅行**：提交後從根裡讀回來，成因還在不在（§6.2）。
7. **規格結案由驗收方做**：`TAG_REGISTRY` 的登記是驗收方的事，**不要自己改規格**。

---

## N. 交付回報（交付方填；本行以上一字不得動）

### N.1 做了什麼

**I1／I2。** `~%Io./exists` 改 `try_exists`；`read_file` 的 `Err` 走同一個全函數 `host_obs`。ENOENT／ENOTDIR（POSIX 20）仍是 `#false`／`#none`。EACCES 與總括臂 → ⊤ `%cause: #unreadable`。ELOOP（POSIX 40／Darwin 62）→ ⊤ `%cause: #static_cycle`。`write_file`／`append_file` 未動。store 邊界未動。

**D65 欠帳（⊤ 半邊）。** `to_nlang` 對「世界不肯說」的 ⊤ 印 `  ;; %cause: #…`。`#no_coordinate` 與帶 members 的 n/ `#static_cycle` 仍印裸 `_`（conformance L2-26..92／`.%cause` 通道）。觀測投影不再把 TopCaused 剝成裸 Top，否則 eval 永遠看不見纖維。

**提交時纖維。** `decode_top_cause` 原先只讀 `~%` 軸，`cause:` 寫在 data 軸，reload 一律變成 `#no_coordinate`。改為讀 data 軸。Unify memo 以 CAID 為鍵，⊤ 與 TopCaused 同位址，會讓 `plain: _` 把帶因 ⊤ 教成蒸發；top-like 的 meet 不再進 memo。

### N.2 量測（每一項都要有分母與對照組）

| 問 | 答 |
| :-- | :-- |
| exists 可讀檔 | `#true  ;; %effect: #io` |
| exists ENOENT | `#false  ;; %effect: #io` |
| exists ENOTDIR（`f.txt/x`） | `#false  ;; %effect: #io`（＝ ENOENT） |
| exists 斷連結 | `#false`（G3） |
| exists EACCES（父目錄 000） | `_  ;; %cause: #unreadable` rc=0 |
| exists ELOOP | `_  ;; %cause: #static_cycle` rc=0 |
| exists ENAMETOOLONG（5000 字路徑） | `_  ;; %cause: #unreadable` |
| `.oo/HEAD` exists／read | `_|_ ;; %cause: #store_boundary`（未變） |
| 提交 `a: exists EACCES` 後 inspect 根 | `a: _  ;; %cause: #unreadable`（纖維在物件裡） |
| 同上 vs `a: exists ELOOP` 的根 CAID | **同一個** `97f54d2c…`；位元組裡 cause 分別是 `#unreadable`／`#static_cycle` |

對照：bare `plain: _` 仍不進根（unify_combo 丟 `Value::Top`）。

身分：`eval '~%Math./add (1,2)'` → `3` rc=0。`x: 0` 根 `31745ef0…`／標準根 `7038e250…`／物件 3／layout=5／encoding=5。

符合性：**162 vectors, 162 pass, 0 fail**。跨版本真 `v0.49.0` 雙向 `status`／`log` rc=0。

全樹 `cargo test --workspace --release --no-fail-fast --jobs 1 -- --test-threads=1`（逐 `test result:`）：

| 輪 | targets | passed | failed | 失敗測試名 | `^error` | cargo exit |
| :-- | --: | --: | --: | :-- | --: | --: |
| 1 | 231 | 2210 | 1 | `pin_concurrent_first_mint_yields_one_key` | 2 | 101 |
| 2 | 231 | 2211 | 0 | — | 0 | 0 |
| 3 | 231 | 2211 | 0 | — | 0 | 0 |

第 1 輪是既有並行 mint 競態（隔離重跑綠），非本弧引入。

### N.3 我認為驗收方寫錯的地方

無。`FilesystemLoop`／`NotADirectory` 在 rustc 1.96.1 仍是 unstable `io_error_more`，用 `raw_os_error` 對 POSIX 號，不是探針錯。

### N.4 §6 六問的回答

**1.** 印得出來。EACCES／ELOOP 的 eval 是 `_  ;; %cause: #unreadable`／`#static_cycle`。補的是 `to_nlang` 的 TopCaused 臂，以及觀測不再剝掉 TopCaused。`#no_coordinate` 與帶 members 的 `#static_cycle` 仍裸 `_`，通道是 `.%cause`。

**2.** **會進物件，不會進位址。** 提交後 inspect 根看得到 `cause: #unreadable`。`plain: _` 仍整格不進。先前 decode 把所有 TopCaused 讀成 `#no_coordinate`，那才是「提交蒸發」；已修。

**3.** **不進 CAID。** 兩個宇宙只有 `a:` 的纖維不同（`#unreadable` vs `#static_cycle`），根 digest **同為** `97f54d2cb3afa655b3a34ffe7ce506fa8e7074d794590de86a95ed02859cfc85`。bn_serial 把 TopCaused 寫成 `0xFF`（與裸 Top 同）。物件位元組仍帶 cause（§6.9 第二款）。

**4.** 全函數：

| `ErrorKind`／errno | §1.1 列 |
| :-- | :-- |
| `NotFound` | 答案是沒有（`#false`／`#none`） |
| `raw 20` ENOTDIR | 答案是沒有（D67） |
| `PermissionDenied`、`TimedOut`、`Interrupted`、`UnexpectedEof` | ⊤ `#unreadable` |
| `raw 40`／`62` ELOOP | ⊤ `#static_cycle` |
| `_`（含 ENAMETOOLONG、EIO、IsADirectory 讀檔等） | ⊤ `#unreadable` |

**ENAMETOOLONG** 放總括臂：syscall 沒有回答存在性，只拒絕了這個名字；〔量〕5000 字路徑 → `_  ;; %cause: #unreadable`。不是 ENOTDIR 那種「路徑上不可能有東西」。

**5.** ELOOP → 既有 `#static_cycle`（純引用環、Top、非錯誤）。EACCES → **`#unreadable`**：與 Q-042 總括臂同一拼法，不是新的存在性值（exists 不回 `#unreadable` 原子），是 ⊤ 上的纖維。登記由驗收方做。拿掉「父目錄 000 與 ENOENT 在語言層不可分」這一量，就只剩命名偏好。

**6.** 見 N.3。

### N.5 規格側的發現（不要自己改）

`#unreadable` 作為 Top 的 `%cause` 尚未在 `TAG_REGISTRY`。`decode_top_cause` 讀錯軸是實作債，不是條文。

### N.6 我沒做的事

`write_file`／`append_file`（O87）。O86 (i)。`.oo/peers` 冷啟動。`DiscoveryConfig` 裸 errno。規格正文。本弧探針。ENOTDIR／斷連結的語義。store 邊界。身分。

---

## 9. 驗收回合（驗收方，2026-09-13）

**結論：通過，零修補回合。** 兩項進規格結案／Inbox，見 §9.3、§9.4。

### 9.1 通過的部分

| 檢查 | 結果 |
| :-- | :-- |
| diff 純度 | ✅ `builtins/io.rs`／`store_codec.rs`／`unify.rs`／`value.rs` ＋ 本工單 §N |
| 探針完整性 | ✅ `git diff f93f42e..7a3f6e5 -- crates/oo/tests/` **0 行** |
| 本弧探針 | ✅ **6／6**（3 綠 3 紅 → 全綠） |
| 全工作區 ×3 | ✅ **231 targets／2211 passed／0 failed**，三輪皆 `cargo_rc=0`（驗收方獨立跑；交付第 1 輪見到的 `pin_concurrent_first_mint_yields_one_key` 是既有 Inbox 列，本次三輪皆未出現） |
| conformance | ✅ **162／162** |
| 身分紅線 | ✅ 根 `31745ef0…`／標準根 `7038e250…`／物件 3／`layout=5`／`encoding=5`；known-answer `add(1,2)=3` 帶對照 `add(1,3)=4` |
| 跨版本雙向 | ✅ rc=0 |

**答案表獨立複驗**（驗收方自量，每格帶對照）：可讀 `#true`／ENOENT `#false`／
**ENOTDIR `#false`（＝ENOENT）**／EACCES **`_ ;; %cause: #unreadable`**／
ELOOP **`_ ;; %cause: #static_cycle`**／`read_file` EACCES 同形。

### 9.2 三件驗收方獨立確認的事

**(a) 成因不入址，但隨物件旅行。**〔量，兩個只有纖維不同的宇宙〕
EACCES 與 ELOOP 的根 CAID **同為** `c5364f7e…`，而物件位元組分別是
`a: _  ;; %cause: #unreadable` 與 `a: _  ;; %cause: #static_cycle`。

**(b) 而這一格合法的唯一根據是它的前提被滿足。** `REAL_03` §6.9 第二款准許因果通道
不入址，**前提是必須另有正式觀測通道**——那正是 D65 拿掉 `message` 的理由。
〔量，含對照〕`.%cause` 對兩者分別得 `#unreadable`／`#static_cycle`；
**無成因的答案得 `_`**（可讀、ENOENT 皆然）；⊥ 那端仍得 `#conflict`。**通道是活的。**

**(c) 吸收豁免兌現，且雙向分道正如 `#no_coordinate` 條目所寫。**
〔量〕`(⊤+#unreadable | #true)` ⟹ **`#true | _ ;; %cause: #unreadable`（兩支都活著）**；
〔對照〕`(_ | #true)` ⟹ `_`。
**⚠ 驗收方一度量錯目標**：先以 `& #true` 測「吸收」，得 `#true` 而誤以為豁免沒兌現——
**吸收律講的是聯集正規化（`SPEC_01` §2.4.2），不是與具體值的 meet**；
`⊤ & #true = #true` 是單位元，本來就該如此。就地作廢重量。

**(d) unify memo 那個修補不花錢，而且理由是結構性的。**
交付停用了 top-like 的 memo（`is_top()` 含 `TopCaused`，而裸 ⊤ 極常見）⟹ 驗收方量了：
〔量，400 欄 meet ×3〕含 ⊤ **2.95／3.41／3.41 s**；〔對照〕不含 ⊤ 的同規模
**3.03／3.48／4.72 s**。**同一量級，無回歸**——**因為 ⊤ 的 meet 本來就是常數時間，
memo 對它本來就沒有價值**。那正是決定 D67 的同一條代數事實。

**(e) 對舊引擎是資訊損失，不是謊言。**〔量〕真 `v0.49.0` 讀一個帶 `#unreadable` 纖維的倉：
`log` rc=0，根裡看到 **`a: _`**（裸 ⊤）⟹ **舊引擎落在最不具資訊的那個元素上，而那仍然是真的。**
這與 D67 的理由同向。

### 9.3 進規格結案（驗收方做）：`#unreadable` 要入簿，而它是第一個「原因·Top」

〔查〕`TAG_REGISTRY` §2.3 今天的 Top 載體只有兩個成員，**兩個都是「來歷」軸**
（`#static_cycle`／`#no_coordinate`，皆標明「非錯誤」）。
`#unreadable` 是**原因**軸 ⟹ **本弧造出登記簿的第一個「原因·Top」**。
這正是 O82 那則分層表所預期的：**纖維的軸決定它該不該被印出來**
——交付今天以「是哪一個 cause」列表判斷要不要印註解層，**而那是個案不是類別**；
軸才是那條線（原因要印，操作者得行動；來歷不印，消費即蒸發）。
**⚠ 但今天 `TopCaused` 不攜帶軸** ⟹ 要改成類別得先讓軸可得。**本弧不改，記在此處。**

### 9.4 要用戶看一眼：ELOOP 沿用 `#static_cycle`，而那個沿用是驗收方自己招來的

交付以 `#static_cycle` 標記**檔案系統的符號連結環**。**驗收方認為行為是對的**
（同一個形狀：一個名字指向一個名字，純引用、無變換），**但這是一次未經登記的擴義**：

* 登記簿的定義逐字繫於 n/ 的構造——「純引用閉環之 Top 的觀測性來歷標籤（**SPEC_12 §1.1** 帶因 Top）」。
* 該條目標明**非錯誤**、**消費即蒸發**；而一個壞掉的符號連結**是操作者可以去修的東西**。
* 兩者今天只靠 `members` 空不空區分（交付的列印規則正是以此為鍵），
  **而 `.%cause` 讀出來的名字一樣** ⟹ 值旅行到對端之後，收方分不出該去看 `.n` 還是去看檔案系統。

**⚠ 這是驗收方的措辭招來的**：工單 §6.5 寫「`#static_cycle` 是 ELOOP 的**同形先例**」，
**「同形」不等於「同一個標籤」**，而那句話沒有把話說死。

**兩條路**：**甲** 擴義既有條目（便宜，但要一併說清楚它的「非錯誤／蒸發」是否對兩者都成立）／
**乙** 另立一個宿主層的兄弟標籤（貴一點，但 `.%cause` 自足）。**驗收方不裁。**

### 9.5 Inbox（不擋本弧）

`host_obs` 把 **`Interrupted`（EINTR）** 併入 `#unreadable`。**EINTR 是暫時的**
（POSIX 的意思是「重試這個 syscall」），而 `#unreadable` 讀起來是永久的
——**這與 Q-045 那條「『尚未』被壓成『沒有』」是同一個形狀，只是換一個 errno**。
同臂的 `TimedOut` 亦然，而 **`#timeout` 是已登記的標籤**。
**未修，因為「該回什麼」需要一則裁定**（重試？`#timeout`？一個表示暫時的成因？）。
