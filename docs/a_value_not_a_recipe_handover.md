# a value, not a recipe — Q-010b 工單(紀元弧)

**Queue ID**:`WORK_QUEUE` Q-010b
**基線**:引擎 `v0.20.0` / 規格 `v0.20.0-draft.1`
**規格裁定依賴**:**O35＝A**(含 2026-08-14 態射面澄清)、**O49 重述**、**O50**、**O51**;
條文見 **SPEC_05 §3.3**、REAL_03 §6.7
**偵察**:`docs/what_the_store_keeps_recon.md`(Q-001)、`docs/what_forcing_leaves_behind_recon.md`
**探針**:`crates/oo/tests/a_value_not_a_recipe_probe_test.rs`(**已預先提交並校準**)

---

## 1. 一句主張

> **耐久形是一個值,不是一份食譜。**

提交是**固化邊界**:寫進歷史的是**觀測結果**,不是還沒跑的程式。

## 2. 三件事,一個紀元

三者**都移動身分**,故只能同批。分開做就是三個紀元,而紀元不是可以隨手開的東西。

| # | 做什麼 | 依據 | 基線量測 |
| :-- | :--- | :--- | :--- |
| **1** | **提交時強制** | O35＝A、O51 | `k1: 1 + 2` 存成 `Thunk{expr, closure}` |
| **2** | **`%closure` 僅捕捉自由變數** | O49-ii、SPEC_05 §3.3 **MUST** | 閉包鏡射整個作用域,~16 B/無關鄰居 |
| **3** | **根存 `system` 的 digest,不存內容** | O50 | `system` 61,912 B / 根 72,555 B ＝ **85%** |

### 2.1 態射面的澄清(不是重新裁定)

A 的「值」在態射上指**被引述的形**(`%rules` → `%code`＋`%closure`),**不是被算完的形**。

理由:意義面(算子所是的映射)是數學物件,無窮定義域無妨;但**耐久層只能裝有限製品**,
故存**呈現**而非**列舉**(SPEC_05 §3.3)。而 A 原本要防的是「讀取即執行」,
**被引述的本體讀回來不會被執行**——`%code` 條款「引述不是求值」正是此意。

### 2.2 強制不會消滅閉包

〔量〕強制把 `Thunk{expr, closure}` **改名**為態射 Combo 的 `%code`／`%closure`,
而該閉包**是承重的**:捕捉 `y` 的態射施用得 `15`;`fact 5` 得 `120`,**成立正是因為
`fact` 在自己的閉包裡**。

⟹ **「不得含自身」這個曾被提出的措辭已撤回。** 遞迴算子的自我參照**是**其本體的自由
變數,「只保留自由變數」這個判準自然保住它。**判準要寫性質,不要寫症狀。**

## 3. 明確不做

| 不做 | 為什麼 |
| :--- | :--- |
| **動 `.oo/staged`** | O51:工作階段**保留 Thunk**,強制發生在 **commit** 不在 evolve(P1 守) |
| **刪除 `%closure`** | SPEC_05 §3.3 **MUST** 保留(P2 守) |
| **換編碼形(JSON → n/)** | O31／W8′-c |
| **遷移既有物件** | 沿用 Q-010a 的 (a) 裁定 |
| **實作 `%builtin` 的 MUST NOT** | 另列 Inbox;本弧不含 |

## 4. 探針(已預先提交並校準)

**校準〔量 2026-08-14,基線〕:7 綠 / 4 紅,紅全部紅在對的理由上。**

| # | 探針 | 基線 | 守什麼 |
| :-- | :--- | :--- | :--- |
| **C0** | 倉裡真有物件且含 `app` | 綠 | 首位。R1–R3 皆為「不存在」斷言,走訪器失效時會靠找不到而通過 |
| **C1** | `k1`／`f`／`v1` 仍讀得回,且無 `caid_mismatch` | 綠 | 值沒在過程中死掉 |
| **C2** | **`fact 5` ＝ `120`** | 綠 | **自由變數分析的核心控制**。「丟掉自我參照」的實作會通過 R2 而弄壞這支 |
| **C3** | `({ y: 10, f: x -> x + y }.f) 5` ＝ `15` | 綠 | 閉包不是可選的 |
| **P1** | `.oo/staged` 仍含 `Thunk` | 綠 | O51 射程 |
| **P2** | 存下的態射仍有閉包 | 綠 | 防交付照 **O49 舊措辭**施工 |
| **P3** | **Q-010a 的保證重述**:無 span／零換行／兩次相同源碼位元組相同 | 綠 | 改寫寫入路徑的弧正是它們會靜默倒退的地方 |
| **R1** | `k1: 1 + 2` 存成值不是 Thunk | **紅** | 含存在半(態射仍在),否則「沒有 Thunk」可能是因為store 空了 |
| **R2** | 閉包只含 `v1`,不含 `v2`／`v3` | **紅** | 含存在半(`v1` **必須**留下) |
| **R3** | 根不內嵌 `math.add` 等內建 | **紅** | 含存在半(根仍有 `system` 槽) |
| **R4** | 解不開的表 digest 被指名拒絕 | **紅** | **沒人解析的 digest 不是依賴,是裝飾** |

### 4.1 校準抓到的一件事(記給後人)

R2 首版在基線**錯誤地綠**。原因:我用「切到下一個兄弟鍵為止」來界定 `f` 的子樹,
而**閉包鏡射整個作用域,所以每個兄弟鍵也出現在 `f` 內部**,切點落進鏡像中間。

**R2 要量的正是那面鏡子,而一個會被鏡子絆倒的切法量不到它。** 已改為純括號配對
(`field_slice`),R4 找根的 `system` 槽時同一個毛病也一併修掉。

## 5. 驗收形狀

1. **diff 純度**:探針檔除 `#[ignore]` 外逐位元不變。
2. **探針完整性**:交付前 4 紅全紅、7 綠全綠;交付後 11/11。
3. **獨立全 workspace 重跑** ＋ **conformance** ＋ **genesis**。
4. **重複穩定 ×5**。
5. **跨版本(真二進位,雙向)**:v0.20.0 寫的倉→新引擎必須讀得開;新倉→v0.20.0
   **必須在格式閘拒絕並說出版本號**。⟹ **本弧須再升 `.oo/format`(2→3)**:根的
   `system` 槽由內容變成 digest,舊引擎若照舊解讀會**靜默得到一個不同的宇宙**,
   那比讀不到更糟。
6. **加性關鍵量測**:根物件大小(基線 72,555 B,其中 `system` 61,912 B)。
   **不得預設變小**——強制會把 `1 + 2` 變成 `3`,但也會把惰性展開成實際結構。
7. **破壞性條目**:**身分軸**(與 Q-010a 的格式軸不同)⟹ **90 天時鐘重啟**。

## 6. 交付方要注意的兩處

1. **自由變數分析必須處理遮蔽與相互遞迴。** C2 只釘了單一遞迴;若實作以「名字出現在
   本體文字中」近似自由變數,遮蔽(`x -> { x: 1, y: x }`)會給出錯誤答案。**這是本弧
   唯一需要正確性論證的部分**,其餘兩件是機械變換。
2. **不收斂的值在 commit 時的行為未裁。** O51 給了方向(「不能當 head,除非你觀測出
   一個有限的陳述」),但**沒有裁定訊息形狀**。若交付撞到,**寫進交付紀錄並回報,
   不要自己決定**——那是一則新的 O 帳。

---

# 7. 驗收第一輪:不通過(2026-08-14)

**探針 11/11,全 workspace 1886 / 22 / 0。** 三件必須修,外加一件我自己的工單缺漏。

## 7.1 綠的部分先說清楚,因為它是真的

| 項 | 量測 |
| :--- | :--- |
| diff 純度 | 探針檔只少四行 `#[ignore]`,零其他改動 |
| 探針完整性 | 交付後 11/11 |
| 強制確實發生 | 磁碟上 `1 + 2` 是 `3`;`.oo/staged` 仍留 Thunk |
| 根縮減 | 72,555 → **1,387 B**(−98.1%) |
| 跨版本(真二進位,雙向) | 新倉→v0.20.0:**在格式閘拒絕並說出「format 3 … understands 1 through 2」**;舊倉→新引擎:讀得開,且**讀不會升級 format** |
| 根的決定性 | 同源碼三倉,根逐位元同一(364 B / 同雜湊) |

## 7.2 D1 — 一個叫 `Combo` 的使用者鍵讓提交後的根讀不回來

```
app: { Combo: 7, other: 1 + 2 }
→ commit：Commit successful
→ log：   #object_undecodable … invalid value: map, expected map with a single key
```

**寫路徑是對的。** 磁碟上 `"Combo":{"Atom":[{"Int":[1,[7]]},0,null]}` 完好,`other` 也已強制成 `3`。
壞的是讀路徑:`expand_root_system_json` 的 `None` 分支對**任何**位於 `"Combo"` 鍵下、
缺 `"system"` 的物件插入 `"system": {}`。使用者的 Atom 因此長出第二個鍵,
externally-tagged enum 就解不開了。

- 半徑:實測六個標籤名,**只有 `Combo` 中彈**,值是 Atom 或 Combo 都中。
- 基線 v0.20.0 同一份源碼 `log` 正常 ⟹ **本弧新造的迴歸**。
- 類別:**JSON 形狀啟發式套在使用者可控的鍵上**——與 Q-010a 的 `strip_ast_spans`
  同一類。那次的結論是改走**型別導向**(`for_cas_storage()`),這裡應同樣處理:
  投影與還原都該在 `Value` 上做,不在序列化後的 JSON 上做。

## 7.3 D2 — 閉包收窄漏捕捉,而且是靜默回 `_`

四個證人,兩個機制。全部是**答案變成 `_`,不是報錯**。

**機制 (a) 遞移依賴沒被跟上。** 只取本體的自由名,但被取的值自己還有依賴:

| 表達式 | v0.20.0 | 交付 |
| :--- | :--- | :--- |
| `({ k: 5, d: k + 1, e: d + k, f: (x -> x + e) }).f 1` | `12` | **`_`** |
| `isEven/isOdd` 相互遞迴,`isEven 2` 起 | `#true` | **`_`** |

`red_morphism_on_deep_sibling`(倉裡既有探針)即此。相互遞迴**第一跳還活著**
(`isEven 1` → `#false`),第二跳才斷——所以不是「相互遞迴不支援」,是**收窄沒有取到不動點**。
自我遞迴任意深度都對(`fact 30` 正確),因為 `fact` 的自由名只有它自己。

⟹ 修法:自由名的**遞移閉包**,對被捕捉的值再取其自由名,直到不動點。

**機制 (b) 收窄時只帶了六個軸裡的兩個。**〔驗收後更正:我原寫「名字空間對不上」,
量測推翻——`insert_field` 按前綴分軸(`~%X`→system、`/x`→rules、`@x`→types、
`%x`→meta、`~x`→local、其餘→data),而 `capture_free_fields` 從
`ComboVal::default()` 起手,只指派 `data`(過濾後)與 `local`。**其餘四軸留在預設值。**〕

| 表達式 | 落在哪軸 | v0.20.0 | 交付 |
| :--- | :--- | :--- | :--- |
| `{ u: 1, f: x -> x + u }.f 5` | data | `6` | `6` |
| `{ ~u: 1, f: x -> x + ~u }.f 5` | local | `6` | `6` |
| `{ %u: 1, f: x -> x + %u }.f 5` | meta | `6` | **`_`** |
| `{ /u: 1, f: x -> x + /u }.f 5` | rules | `6` | **`_`** |
| `{ @u: 1, f: x -> x + @u }.f 5` | types | `6` | **`⊥ #conflict`** |

`capture_free_fields` 的註解只談 `local`,讀起來像是「公開軸收窄、私有軸保留」的裁定,
但**沒有任何一軸叫「私有」**——掉的是 meta／rules／types 三個各自有意義的軸。
⟹ 修法不是補一條 `local` 的例外,是**自由名判準必須對每一軸各自成立**;
`relations`／`masa_ref` 同樣被留在預設值,一併查。

## 7.4 D3 — 加了第二個解碼器,但只搬了四個讀者

`get_root` 是對的,`Universe::load` / rollback / refine / `oo inspect` 也都搬了。
**沒搬的仍在用 `get_value`,而 format-3 的根用 `get_value` 讀不出來。** 後果:

| 讀者 | 實測 |
| :--- | :--- |
| `gc.rs:96 verify_reachable_object` | **`oo gc` 在健康倉上指控它自己損壞**並拒絕執行 |
| `universe.rs:1082`(shadow scan) | `peer_fetch_verification` 兩支掛 |
| `oodp.rs:382`(`#fetch` 服務) | `universe_determinism` 的跨引擎解析掛 |

`oo gc: 6 objects, 6 reachable, 0 collectable` 之後接
`integrity #object_undecodable: reachable digest … cannot be decoded`
——這直接違反 **REAL_03 §6.6「裁決必須為真」**,也正是 v0.2.55 弧修掉的那一類
(把協定/格式層的話當成完整性裁決)。方向是安全的(什麼都沒刪),但裁決是假的。

**建議:不要逐處補 `get_root`。** 值得問的是為什麼會有兩個解碼器——
`get_value` 對 format-3 的根**沒有正確答案**,那它就不該是可呼叫的路徑。

## 7.5 我的工單缺漏:12 支必然被打掉的釘沒有被列為「預定改變」

22 支失敗裡 **12 支不是交付的錯**,工單本來就該事先列出來:

- **`.oo/format` 2→3**:`p2_format_moves_only_when_declared`、`r5_the_store_format_says_it_changed`。
  §5 寫了要升,卻沒去 grep 誰在斷言它是 2。
- **根 CAID 移動**:`p1_the_root_caid_does_not_move`、`p4_root_caid_does_not_move`。紀元弧的定義就是這個。
- **`r1_no_span_survives_into_a_cas_object`**:它的守衛自己寫著「no Thunk 代表交付
  overshot（that is Q-010b）」——**倒數計時器,而 Q-010b 就是它等的那一弧**。
  既有規則已經說了這種釘到期時要列為預定改變,我還是漏了。
- **儀器失效七支**(`c0`／`c1_a_semantic_tamper_is_caught`／`c2_the_value_survives_a_round_trip`／
  `r2_identical_source_gives_identical_bytes`／`r3_whitespace_does_not_change_the_stored_bytes`／
  `r6_a_user_field_named_span_survives`／`red_no_leaf_of_the_root_varies_between_processes`):
  **`root_object` = `max_by_key(len)`**。根從 72,555 B 掉到小於 commit 物件,
  於是這些 fixture 開始量 commit。逐一實測真正的根之後全是乾淨的——
  含資料毀損哨兵 `r6`:四個案例的欄位全在磁碟上、全讀得回。

  ⟹ **新常設規則:一弧若改變「根與 commit 誰比較大」,就會打掉每一個
  以大小找根的 fixture。「最大的物件就是根」是一個沒有人寫下來的假設。**

## 7.6 已裁:標準根一個 digest(O52,2026-08-14 用戶)

**交付剝掉的不只 `system`。** `project_standard_root` 對六個軸都做「與標準根相同就拿掉」,
所以 72,555 → 1,387 B 遠超 O50 所裁的 `system`。〔量 v0.20.0〕標準根實際佔 **67,329 B,分佈三軸**:

| 軸 | 大小 | 內容 |
| :--- | ---: | :--- |
| `system` | 61,912 B | 26 個 `~%X`(`Io`／`Config`／`Discovery`…) |
| `types` | 5,186 B | `list`／`option`／`result` |
| `rules` | 231 B | 只有 `add`——**建構順序的孤兒**,見下 |

**裁定:digest 覆蓋整個 `root_with_system()`,一個,不是三個。**
理由是三軸**一起變動**(換 build 就一起換),而 SPEC_09 §2.5 的
「所有符合規格的引擎必須內建其定義並對齊其 CAID」**要的就是這一個對象**
——該表的 CAID 欄整欄是 `hash:sha256:v1:0001...` 佔位,今天沒有可對齊的東西。

⟹ 交付要改的:`standard_table_digest` 已經算的是**整個 `standard: &ComboVal`**,
但 `project_standard_root` 存進 `system` 軸的是 `system_table_digest(&standard.system)`
(只有 `system` 子樹)。改成整根的 digest,且 `hydrate_system_table` 的指名拒絕訊息
要說「這個標準根」而非「這張表」。**六軸剝除本身保留**——它現在有守衛了。

### 7.6.1 兩筆順帶查到的既有帳(不屬本弧,已入 WORK_QUEUE Inbox)

1. **`/add` 是使用者唯一不能在自己宇宙頂層定義的規則名。** `root_with_system` 只有
   一次頂層 `fields.insert("/…")`,就是 `/add`;其餘 `/sub`／`/mul`… 只進 `~%Math`。
   〔量〕`evolve` 一條 `/add: (p) -> 42` 失敗於 `#missing_key at /add.%kind`,
   而 `/sub`／`/mul`／`/frobnicate` 皆成功。
2. **標準型別:規格兩張表互不相容,引擎只兌現三個。** SPEC_09 §2.1 樹(~20 名)與
   §2.5 下表(5 名)名單不同;引擎只有 `list`／`option`／`result` 是真的,
   **其餘憑名字現造**——`@zzz` 與 `@int` 印出逐字同形的值,而兩者 `.%kind` 皆為 `_`。

## 7.7 下一輪

修 D1／D2／D3;12 支預定改變由驗收方處理(探針修改權在驗收方),
7.6 待裁。修完後仍走完整驗收:全 workspace ×5、conformance、genesis、跨版本雙向。

---

# 8. 驗收第二輪:D2／D3／O52 通過,D1 未修好(2026-08-14)

**全 workspace 1896 / 12 / 0**,而這 12 支**正好是 §7.5 列的那 12 支預定改變/儀器失效,
一支不多一支不少**。第一輪的十支真迴歸全部消失。

## 8.1 通過的

| | 量測 |
| :--- | :--- |
| **D2** | 五軸逐一實測全回 `6`(`u`／`%u`／`~u`／`/u`／`@u`);深層兄弟 `12`;相互遞迴 `isEven 4`→`#true`;`fact 30` 正確 |
| **D2 收窄仍是真的** | `{ y, v2, v3, f }` 的閉包只有 `y`;`{ %secret, %used, f }` 只有 `%used` |
| **D2 不動點沒有變成放棄** | `{ k, d, e, junk, f: x -> x + e }` 的閉包＝`{d, e, k}`,**`junk` 不在**——R2 的 fixture 沒有遞移鏈,抓不到「有 Thunk 就整片鏡射」這種解法,這支是額外量的 |
| **D3** | `oo gc` 在健康倉上乾淨收場;十支真迴歸歸零。作法是**所有讀者共用一個解碼器**(`get_value` 自己認得 format-3 根),不是逐處補 `get_root` |
| **O52** | `project_standard_root` 存的是 `standard_table_digest(standard)`＝**整個標準根**;拒絕訊息已改為 "standard root digest … is unavailable" |
| **跨版本雙向** | 新倉→v0.20.0 在格式閘拒絕並說出版本;舊倉→新引擎讀得開(71,218 B 根),且**讀不升級 format** |

## 8.2 D1 未修好:判準換窄了,沒有換掉

交付把守衛改成 `object.len() == 1`,註解也寫對了診斷
(「A user is free to name a data coordinate `Combo`; treating that map as a Value was
the D1 shape-inference bug」)。**但 `len()==1` 不是那個區分。** 使用者的軸映射**剛好只有
一個條目**時,它就是一個長度 1、鍵為 `Combo` 的物件——與 `Value::Combo` 的序列化形無法區分。

〔量 v0.20.0＋本次交付〕

| 源碼 | 結果 |
| :--- | :--- |
| `app: { Combo: 7 }` | **BROKEN** |
| `app: { @Combo: 7 }`／`{ %Combo: 7 }`／`{ /Combo: 7 }`／`{ ~Combo: 7 }` | **BROKEN**(五軸全中) |
| `Combo: 7`(根層) | **BROKEN** |
| `app: { Combo: 7, z: 1 }` | ok ← **同軸加一個鄰居就遮住** |

**這比第一輪更難發現**:同一個鍵名,加減一個無關欄位就在壞與不壞之間切換。
`commit` 依然報成功,物件依然讀不回。

### 8.2.1 建議:不要再找更窄的判準,把手術拿掉

`compact_root_system_json`／`expand_root_system_json` 這一對**只做兩件事,兩件都是外觀**:

1. 把 `{"__nlang_system_digest":{"Atom":[…]}}` 印成裸字串 `"<64hex>"`
2. 拿掉巢狀 Combo 的空 `"system":{}`

⟹ **把 sentinel 留在型別形(`Value` 上),兩個函式整個刪掉**,就沒有任何地方在猜形狀。
這正是 Q-010a 的同一課:`strip_ast_spans` 的解法不是更聰明的 JSON 述詞,是
`for_cas_storage()` 的型別導向投影。

〔量〕代價:digest 欄 75 → 127 B(**+52 B**);空 `system` 每個 13 B,
使用者子樹(3,755 B)裡有 **5 個**,合計 **+65 B**。**在 1,347 B 的根上約 +117 B,
而本弧的收益是 72,555 → 1,347。**

若堅持要保留外觀,唯一能真正區分的是**位置**而非形狀:只在**已知是 Value 的節點**上動手
(即由型別走訪帶路),而那等價於做在 `Value` 上。

## 8.3 下一輪

只剩 D1。12 支預定改變仍由驗收方處理。修完後仍走完整驗收。
