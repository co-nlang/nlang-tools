# 標準根盤點：我們自己的洞

> 2026-08-16。基線實測：規格 `v0.24.0-draft.1`／引擎 `v0.24.0`。
>
> **這份文件不是規格草案。** 它是逐項列舉，目的是先看清楚洞在哪；
> 判準（如果有）應該從這裡長出來，不是先立好再套上去。
>
> 偵察背景與量測方法見同目錄 `an_inventory_nobody_took_recon.md`。

---

## 1. 骨架

`root_with_system()` 全五軸遞迴展開＝ **308 列**。

| | |
| :--- | :--- |
| 頂層 | **30** 項：`/add`（rules 1）、`@list`／`@option`／`@result`（types 3）、`~%` 模組 26 |
| 態射引用 | **256** 條，指向 **251** 個相異 builtin id |
| 資料欄 | **19** 格（見 §6） |
| digest | `hash:sha256:v1:65f52e2d…`，自 **v0.21.0 起四個 minor 版未動** |
| 逐模組 CAID 鎖 | `genesis.rs` **26 個 `SEED_*`** ＋ `seed_caids_are_stable`（今日綠）。未鎖：`/add`、`~%Effect`、`~%Engine`、`~%Official` |

---

## 2. 洞 A：digest 承諾的東西，比所有人假設的少

〔量，實測非讀碼〕同一個態射的兩種效果標記：

```
{ %morphism:#true, %builtin:"io.read_file" } as #pure → f7a44ee9ba966648…
{ %morphism:#true, %builtin:"io.read_file" } as #io   → f7a44ee9ba966648…
SAME: true
```

`bn_serial.rs:68` 逐字 `Value::Atom(kind, _effect, _rank)`——**效果被丟掉**；
`serialize_combo` 只寫 `closed` 旗標與排序後的欄位，**沒有效果位元組**（只有 `Thunk`
寫 `effect.to_serial_byte()`）。

⟹ **標準根的 digest 承諾的是：軸、鍵名、`%morphism: #true`、`%builtin: "<id>"`、
封閉旗標。就這些。**

⟹ **兩個引擎可以在 `65f52e2d…` 上完全一致，而對「讀檔案是不是純的」意見相反。**
REAL_05 Level 3 的義務一「CAID 全域一致性」在這種情況下**仍然成立**；
而 SPEC_08 §6 的能力閘正是以效果為鑰匙。

**這是本次盤點裡唯一一個「已經在線上、且與安全性有關」的洞。**

---

## 3. 洞 B：會碰到世界的 26 條

標準根裡非 `#pure` 的全部態射（`Pure` 230／`IO` 20／`#nondet` 1／`#state` 5）：

| 效果 | 路徑 | 規格提過 | 有向量 |
| :--- | :--- | :---: | :---: |
| IO | `~%Io./read_file` | **無** | 無 |
| IO | `~%Io./write_file` | **無** | 無 |
| IO | `~%Io./append_file` | **無** | 無 |
| IO | `~%Io./exists` | **無** | 無 |
| IO | `~%Env./args` | **無** | 無 |
| IO | `~%Env./cwd` | **無** | 無 |
| IO | `~%Env./get` | 有 | 無 |
| IO | `~%Process./pid` | **無** | 無 |
| IO | `~%Process./exit` | 有 | 無 |
| IO | `~%Csv./read_csv` | **無** | 無 |
| IO | `~%Query./where` | **無** | 無 |
| IO | `~%Time./now` | 有 | 無 |
| IO | `~%Engine./save` | **無** | 無 |
| IO | `~%Engine./observe` | 有 | 無 |
| IO | `~%Discovery./{advertise,connect,fetch,find,identify,identify_and_store}` | 有 ×6 | 無 ×6 |
| #nondet | `~%Math./random` | 有 | **有** |
| #state | `~%Engine./equivalence_map` | **無** | **有** |
| #state | `~%Engine./project_down` | 有 | **有** |
| #state | `~%Engine./project_up` | 有 | 無 |
| #state | `~%Engine./resolve` | **無** | 無 |
| #state | `~%Engine./set_strategy` | **無** | 無 |

**26 條裡：12 條規格連名字都沒有；23 條沒有任何符合性向量。整個 `~%Io`（4/4）
規格零提及、向量零覆蓋。**

### 3.1 `~%Query./where` 是 28 個裡唯一安全的那一個

> **⚠ 本節第一版寫「看起來是抄貼」——用戶指出並實測推翻。**
> `docs/worknotes/phase-43-handover.md:461` 逐字寫了理由：「宣告 IO 因為 pred 效果
> 在編譯時未知；實際傳播由 `max_effect` 計算決定」。**它是唯一有人為此寫下理由的一處。**

〔量〕標準根裡**會套用呼叫者所提供之態射**的內建共 **28 個**
（`cond.{if,cond,match}`、`list.{map,filter,fold,find,any,all,count,flat_map,
group_by,max_by,min_by,partition,reduce,scan,sort_by,take_while,drop_while,zip_with}`、
`option.{map,and_then,filter}`、`result.{map,and_then,map_err}`、`query.where`）。

**其中 27 個宣告 `#pure`，只有 `query.where` 宣告 `#io`**，且只有它在回傳時把結果
的效果拉到 `EffectTag::IO` 底線（`query.rs:165`；對照 `list.filter` 為
`lc.effect.union(f.effect())`，無底線）。

而那條底線是有牙齒的〔量 v0.24.0〕——同一個運算、同一個純述詞：

```
out: { %effect: #pure, v: ~%Query./where([1,2,3], (x -> #true)) }
  → _|_ (%cause: #effect_violation)  ;; declared #pure but observes #io
out: { %effect: #pure, v: ~%List./filter([1,2,3], (x -> #true)) }
  → { %effect: #pure, v: [1, 2, 3] }
```

### 3.2 而另外 27 個是漏的——但鍋在規格

〔量〕把不純的態射交給高階內建，`#pure` 繭**不擋**：

```
{ %effect: #pure, v: ~%List./map([1,2,3], (x -> ~%Time.now _)) }
  → { %effect: #pure, v: [1786852226951 ;; %effect: #io, …] }      ← 收下了
```

引擎**算對了**：該 list 自己的 `v.%effect` 實測為 `#io`，與直呼 `~%Time.now _` 相同。
是**閘沒有讀它**。

失守的邊界不是「高階內建」，是**態射應用本身**：

| 探針 | 結果 |
| :--- | :--- |
| `{ %effect:#pure, v: ~%Time.now _ }` | ⊥ `#effect_violation` ✅（＝ L2-100 向量） |
| `{ %effect:#pure, v: { w: ~%Time.now _ } }` | ⊥ ✅ |
| `{ %effect:#pure, v: [~%Time.now _] }` | ⊥ ✅ |
| `{ %effect:#pure, v: ((x -> ~%Time.now _) 1) }` | **收下** ❌ |
| `{ %effect:#pure, v: {{ w: ~%Time.now _ }} }` | 收下 ✅（繭＝規格明文的逃生門） |

⟹ **寫一個 lambda 並套用它，就足以讓一個 `#pure` 宣告成為假的。** 不需要任何標準庫。

**這是規格的鍋，而且是一句話跨兩個機制**：

*   **SPEC_08 §4.3** 逐字：「**靜態守護**：在預設的純粹上下文 (`#pure`) 中，若意外
    觸發了 `#io` 觀測，引擎將其阻擋並坍縮為 `_|_`」
*   **ERROR_CODES `#effect_violation`** 逐字：「顯式 `%effect: #pure` 宣告被
    **值的實際活動傳染效應**矛盾……（**SPEC_08 §4.3 靜態守護**，裁定 A；2026-07-24）」

後者在**同一句話裡**既說「值的實際傳染效應」（動態判準）又引「靜態守護」（靜態機制）。
**靜態守護看不穿態射應用——這正是 Phase 43 工單所寫的那個理由。** 引擎照著 SPEC_08
做了靜態的，於是動態那半從來沒有存在過。

⟹ **`/where` 不是錯的：它是唯一一個在守衛看不穿的前提下仍然誠實的。**
要嘛把守衛改成動態（值已經帶著正確的效果，只差有人讀），要嘛 28 個全部比照 `/where`
保守宣告。**在其中一條做完之前，拿掉 `/where` 的 IO 底線會是把唯一沒破的那一格也打破。**

依 `WORK_QUEUE` §2.2，「偽造安全裁決」屬 **interrupt-candidate**。

### 3.2 一個名字借了別人的 id

`~%Discovery./identify_and_store` 的 `%builtin` 是 **`engine.save`**，不是
`disc.*`。⟹ 251 個相異 id 對 256 個引用——**有 5 條是別名**。一張規格級的名字表
必須說得出哪些名字共用同一個實作點。

---

## 4. 洞 C：規格未描述的 181 條

251 條態射中 **181 條（72%）**在 `spec/zh_TW/**.md` 全文出現 0 次（寬鬆子字串比對，
故為下界）。分兩類，性質完全不同：

### 4.1 純電池（169 條，無語義風險）

`~%String` 29／`~%List` 31／`~%Math` 24／`~%Reflection` 16／`~%Bytes` 11／`~%Set` 8／
`~%Stat` 6／`~%Csv` 3／`~%Url` 4／`~%Path` 4／`~%Json` 3／`~%Complex` 3／`~%Diff` 3／
`~%Query` 3／`~%Toml` 2／`~%Regex` 1／`~%Time` 7／`@option` 5／`@result` 5 …

讀起來就是任何人寫直譯器時會加的東西：`/trim`、`/to_lower`、`/starts_with`、
`/pad_left`、`/levenshtein`、`/title_case`、`/word_count`、`/percentile`、
`/histogram`、`/is_prime`、`/factorial`。**它們對世界不作任何主張。**

### 4.2 有語義風險的（12 條，即 §3 的粗體列）

外加兩組**雖然是 `#pure` 但仍承重**的：

*   **密碼學原語**：`~%Bytes./sha256`、`/hmac_sha256`、`/base64_encode`、`/base64_decode`、
    `/to_hex`、`/from_hex`——全部規格零提及。而 REAL_03 把 `hash:sha256:v1` 定為
    CAID 演算法、REAL_02 的簽章走 HMAC 家族。**程式可以用一個規格從未祝福過的方式
    算出雜湊**，且那個名字在每一個宇宙的根裡。
*   **`~%Reflection./set`／`/delete`**：名字讀起來是變異，效果標記是 `#pure`。
    若它們回新值（函數式）則標記正確——**但沒有任何文件說是哪一種**。
    同模組的 `/bottom_cause` 直接觸到 ⊥ 的 cause 可讀性，而那正是 D40／D33 那條線
    上的爭點。

---

## 5. 洞 D：反映面錯位

REAL_05 Level 3 的「**反映與合成**：系統物件 (`~%`)、虛擬元欄位 (`%`)」指向 SPEC_11。
〔量〕SPEC_11 全文只出現兩個模組名：`~%Engine`（6 次）、`~%Repl`（2 次）。

| | 規格說 | 引擎有 |
| :--- | :--- | :--- |
| `~%Repl` | §1.2「會話層級……**不進入 Commit 歷史**」，全規格 14 次 | **沒有**（正確——會話層級的東西進不了版本綁定的常數） |
| `~%Engine.mass_map` | §1.1 指名：質量場 | **不存在** |
| `~%Engine.heat_map` | §1.1 指名：熱度場 | **不存在** |
| `~%Engine.horizons` | §1.1 指名：燃料與光錐邊界 | **不存在** |
| `~%Engine.state.differential` | 未提 | 存在，`#d1_converging` |
| `~%Reflection`（17 條態射） | **零次** | 存在 |

⟹ **Level 3 的 `~%` 義務追到源頭只落在 `~%Engine` 一個模組，而規格為它指名的三個
欄位一個都不存在；引擎真正拿來做反映的 `~%Reflection` 則規格從未提過。**

`state.differential` 另有一項〔量〕：`/set_strategy` 前後都是 `#d1_converging`，
**沒動**。它是一個凍結成常數的假狀態。

> **這一格自己說出了一條理由**：一個版本綁定的不可變值**沒有能力反映任何東西**。
> `~%Repl` 進不了標準根不是因為它不重要，是因為它是活的。同一個理由適用於
> `mass_map`／`heat_map`／`horizons`——它們如果真的進了標準根，就會變成
> `state.differential` 那樣的謊。

---

## 6. 洞 E：資料欄與形狀化石

標準根裡的**全部** 19 個非態射格：

| 格 | 值 | 洞 |
| :--- | :--- | :--- |
| `@option.%kind/%name/%some.%val/%none` | `#type`／`"option"`／`_`／`#none` | — |
| `@result.%kind/%name/%ok.%val/%err.%cause` | 同上 | — |
| `@list.%kind/%name` | `#type`／`"list"` | — |
| `~%Math.one` | `1` | SPEC_09 §3.1 有（EML 的自舉常數） |
| `~%Engine.state.differential` | `#d1_converging` | **§5：凍結的假狀態** |
| `~%Config.{fuel,max_branches,max_unification_depth,max_lifting_depth,max_pattern_nodes,timeout,strategy}` | 10000／64／256／32／1024／`#_`／`#blur` | SPEC_08 §3.1 的創世旋鈕，封閉家族 |

化石三則：

1.  **`/add` 孤兒**。rules 軸唯一的一項，來自 `71177b7` Genesis commit，無註解。
    〔量 v0.24.0〕`/add 1 2` → `3` 而 `/sub 5 2` → `_`；`/add: x -> x` →
    `#missing_key at /add.%builtin` 而 `/sub: x -> x` → 成功。
    **全語言唯一一個裸呼叫得動、卻不能在自己宇宙頂層定義的名字。**
2.  **`~%Official` 是空 combo**，而 ORDER_01／SPEC_10／REAL_01／SPEC_13 共引用 26 次。
3.  **`~%Discovery./identify_and_store` → `engine.save`**（§3.2）。

---

## 7. 洞 F：型別的三個族群沒有入憲

〔量 v0.24.0〕

| 族群 | 成員 | 住哪 | `"s" & @T` |
| :--- | :--- | :--- | :--- |
| 標準根型別 | `@list` `@option` `@result` | 標準根 types 軸 | ⊥（且帶代數介面 `%fmap`／`%some`／`%ok`） |
| 保留內建名（12） | any, num, complex, float, int, str, bool, list, combo, morphism, option, result | `type_constraint.rs:22`，**不在標準根** | ⊥ |
| 名義標籤 | `@zzz`、`@MyType` | 哪裡都不住 | `"s"`（**惰性**） |

⟹ SPEC_09 §2.1 的樹 ≈ 第二族，§2.5 的表 ≈ 第一族。**兩表不一致是因為它們在回答
兩個不同的問題，而沒有人說過**——不是因為有一表沒做完。
（§2.5 現行註記寫「`@zzz` 與 `@int` 印出的形逐字同構」：**形同構，行為相反**。）

**順帶的缺陷**：§2.1 把 `@u8..@u256`／`@i8..@i256` 寫成「固定寬度投影，**FFI 邊界**」。
〔量〕`300 & @u8` → `300`；`-1 & @u8` → `-1`；`"s" & @u8` → `"s"`。
`from_name("u8")` 落 `Unknown` ⟹ 惰性。同族 `@unit`／`@record`／`@type`／`@caid`
亦惰性（`super_parent()` 認得它們，但那只餵 `%super` 反映欄，不執法）。

---

## 8. 洞 G：義務面是空的

| | 向量數 | 碰到的 `~%` 模組 |
| :--- | ---: | :--- |
| L1 | 39 | **零個** |
| L2 | 23 | `~%Time`(8)、`~%Math`(8)、`~%Engine`(2)、`~%Effect`(1)、`~%Config`(1)〔另 `~%Mine`(2) 為負向測試〕 |
| L3 | **0** | REAL_05 §3.3 逐字：「**待 v1.0.0 前另波補齊**」 |

⟹ **26 個模組裡 5 個有義務、21 個沒有；而標準根本身屬於 Level 3，那一格的矩陣是空的。**
這就是 181 個態射能長進 digest 而沒人吭聲的機制。

---

## 9. 從這些洞看得出來的候選判準

三條，**互相正交**，各自能裁掉的東西不同。

### 判準 ①：標準根只裝版本綁定的常數

*   **裁得掉**：`~%Repl`（已不在）、`mass_map`／`heat_map`／`horizons`（永不得進）、
    `state.differential`（應移除）。理由由 §5 自己給出：不可變的東西反映不了任何東西。
*   **裁不掉**：181 條規格未描述的態射名——**它們全都是常數**，這條判準對它們沉默。
*   **成本**：零。它只是把已經為真的事寫下來，加上一條禁令。

### 判準 ②：非純態射的名字進標準根，規格必須為它寫下世界介面

*   **裁得掉**：§3 那 12 條粗體（要嘛補規格，要嘛移出標準根），以及 `/where` 的錯標。
*   **裁不掉**：§4.1 的 169 條純電池。
*   **⚠ 前提缺口**：**效果不進 digest**（§2）。所以「規格寫下效果」今天**無法由
    digest 執行**——兩個引擎仍可在同一個 digest 上對效果意見相反。
    要讓這條判準有牙齒，`%effect` 必須進入耐久形，而那**會移動 `65f52e2d…`**。
    （v0.23.0 的多重具備正是為此而做，這會是它第一次派上用場。）

### 判準 ③：標準根 ⊆ 合規義務所需（MVP 角度）

*   **裁得掉**：全部——它是唯一一條能處理 §4.1 那 169 條電池的判準。
*   **今天施不上**：§8——L3 零向量。**判準的內容要從矩陣讀出來，而那一格是空的。**
    ⟹ 採這條，就等於先接下「補 L3 向量」這件事，而它比寫清單大。

---

## 10. 尚未查證（列出來，不猜）

*   `~%Reflection./set`／`/delete` 是函數式還是變異式（決定 `#pure` 是否為謊）。
*   `~%Bytes./sha256` 與 REAL_03 的 `hash:sha256:v1` 是否同一個位元組管線。
*   移除 `/add` 之後工作區有多少測試／範例會紅。
*   26 個模組中，`~%Official` 空殼是否有別的引擎版本填過。
