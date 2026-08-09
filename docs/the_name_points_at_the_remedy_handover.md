# W4′ 交接:名字要指向補救

**開弧日**:2026-08-09
**基線**:`dev 72c5fa8`(= `top e36706d`,v0.13.0)
**來源**:`nlang-spec` **ERROR_CODES §2.7.1 / §2.7.2**(規範已落,`local e758c46`);
`meta/oo/STATUS.md` W4′;ENGINE_SYNC 2026-08-09 盤點
**性質**:**純符合性**——規格條文已在,引擎未跟上。**引擎側走 minor**(見 §2.3:動到 CAID)
**破壞性**:**是**(條目 #9)。只影響**由深度耗盡產生的 `#blur` 值**之 CAID,範圍已量,見 §2.3

---

## 0. 一句話

> **引擎用「你可能正在被攻擊」回報「跳數用完了」,用「加燃料」回報「結構太深」——兩個名字都指向做不到或做了沒用的補救。**

---

## 1. 缺陷

### 1.1 §2.7.1:資源邊界被說成攻擊

`crates/interpreter/src/builtins/disc.rs:472`——**`#semantic_eclipse` 的唯一鑄造點**:

```rust
if ctx.disc_routing_hops >= MAX_ROUTING_HOPS {        // MAX_ROUTING_HOPS = 16
    cause: BottomCause::SemanticEclipse,
    message: format!("Routing budget exceeded after {} hops", MAX_ROUTING_HOPS),
```

訊息說的是預算,標籤說的是攻擊。而登記簿裡真正的偵測碼 `#semantic_isolation`
**在引擎中一次也沒有出現**(〔量 2026-08-09〕全樹零命中)。

⟹ **對端只是比較遠的使用者會被告知他可能正在被攻擊,而真正做偵測的那個名字不存在。**

連測試名都把這件事固化了:`test_find_hop_budget_exceeded_returns_semantic_eclipse`。

### 1.2 §2.7.2:深度耗盡被說成燃料耗盡

`crates/interpreter/src/lib.rs:193`:

```rust
if self.depth > self.max_unification_depth as u32 {
    return Err(ResourceExhausted::FuelExhausted);   // ← 深度,回報燃料
}
```

**〔量 2026-08-09,端到端〕** `~%Config.max_unification_depth: 2` 下合併兩個四層 combo:

```
out: {  a: #blur { %cause: #fuel_exhausted, %caid: "hash:sha256:v1:6ebb46d7…" } }
```

同一份來源把 depth 設回 64 即完全收斂。**燃料一格都沒少,而操作者被告知去加燃料。**
正確的補救是攤平結構或提高 `max_unification_depth`——**兩個不同的旋鈕。**

### 1.3 這個類別在本倉已經被認出來過一次

`value.rs` 的 `PeerTimeout` 變體上有一段前任驗收方寫的註解:

> ERROR_CODES gives `#timeout` the remedy 「請優化性能、減少嵌套,或放寬時間限制」,
> which is **not merely unhelpful for a silent peer, it points the reader at their own code**.

**那正是本弧的論旨,只是上一次只修了一個實例。** 本弧修另外兩個。

---

## 2. 量測

### 2.1 反向盤點(`nlang-spec/scripts/error-code-inventory.py`,含雙控制)

W4 落地後,引擎有而登記簿沒有者 = **3**,三者皆刻意:
`#invalid_path`(已廢止)／**`#semantic_eclipse`(§2.7.1 廢止,引擎仍在鑄)**／
`#stack_overflow`(§2.7.2 決定不入登記簿)。

### 2.2 `#max_depth_exceeded` 從未被鑄

〔量 2026-08-09〕`MaxDepthExceeded`／`max_depth_exceeded` 於全引擎樹 **零命中**。
`#max_nodes_exceeded`／`#max_lifting_exceeded`／`#max_branches_exceeded` 亦同。
**本弧只處理 depth 那一個**——另外三個沒有已量到的錯誤回報,列為相鄰項(§9)。

### 2.3 破壞面已量,而且只有一半

| | 進 CAID？ | 證據 | 本弧影響 |
| :-- | :-- | :-- | :-- |
| **`BottomCause`** | **否** | `bn_serial.rs:59` `Value::Bottom(_) => buf.push(0xFE)`——**cause 被丟棄** | 新增／改名變體**不動任何 CAID** |
| **`BlurCause`** | **是** | `bn_serial.rs` `bd.cause.as_bytes()` 進雜湊 | **深度耗盡所產生的 `#blur`,其 CAID 會移動** |

⟹ **§2.7.1 完全非破壞性**(只涉 BottomCause)。
⟹ **§2.7.2 的 Strict 半邊非破壞性,Blur 半邊破壞。**
被移動的具體值:上面 §1.2 那個 `6ebb46d7…`。

**破壞範圍的界線要說準**:**只有「因深度耗盡而產生的 `#blur`」**會移動。
因燃料耗盡而產生的 `#blur` **不得**移動——那是**釘 P2**。

> **不因破壞性而拆弧。** §2.7.1 與 §2.7.2 是同一件事的兩個實例
> (「名字要指向補救」),依 `feedback_eager_in_deep_water`:**拆弧的正當理由
> 只有「這是兩件不同的事」,「這樣不破壞相容」不是理由。**

---

## 3. 射程

**做:**

1. **新增 `BottomCause::RoutingBudgetExceeded`**(→ `#routing_budget_exceeded`),
   **加在列舉尾端**(fmt v2 append-only 紀律)。`disc.rs:472` 改鑄它。
2. **新增 `ResourceExhausted::DepthExceeded`**;`lib.rs:193` 的深度閘改回傳它。
   `handle_resource_exhausted` 三支各自對應:
   * Strict → **新增 `BottomCause::MaxDepthExceeded`**(→ `#max_depth_exceeded`,列舉尾端)
   * Blur → **新增 `BlurCause::MaxDepthExceeded`**(→ `b"max_depth_exceeded"`,**列舉尾端**)
   * Approximate → 不變

   **三個新 cause 各需改三處對映**——`as_tag()`(裸名)、`BottomDetail::as_cause_combo`
   (帶 `#`)、`crates/oo/src/main.rs:49`(帶 `#`)。**漏掉任何一處都不會有測試變紅**
   (§4.1 校準時量到)。交付**必須三處都改**;**不要求合併它們**,合併是另一件事(§9)。
3. **`BlurCause::StackOverflow` 停止對外**:`handle_resource_exhausted` 不得再產生它。
   **變體本身不得刪除**——它在 `as_bytes()` 內、屬 CAID 位元組表,刪除是 fmt 紀律問題,
   不在本弧(§9)。
4. **更新既有測試 `crates/interpreter/tests/semantic_eclipse_test.rs`**:
   見 §7 的明文授權——**只准改期望值與測試名,不准刪測試**。

**不做:**

- 不實作 `#semantic_isolation` 的偵測(那需要 meet 兩條路徑,見 §9)。
- 不碰 `#max_nodes_exceeded`／`#max_lifting_exceeded`／`#max_branches_exceeded`。
- 不刪任何列舉變體。
- **不碰任何規格檔**。

---

## 4. 探針

檔:`crates/oo/tests/name_points_at_remedy_probe_test.rs`(已隨本工單提交,已校準)

**紅測全部標 `#[ignore]`。#3 只准移除 `#[ignore]`。**

| # | 類 | 斷言 | 基線 |
| :-- | :-- | :-- | :-- |
| **C1** | 控制 | **真的燃料耗盡仍回 `#fuel_exhausted`**,且該路徑確實產生了一個視界值 | 綠 |
| **C2** | 控制 | 深度**充足**時同一份來源完全收斂(證明 R1/R2 的紅不是「這段程式壞了」) | 綠 |
| **R1** | 紅 | 深度耗盡（strict）的 `%cause` 為 `#max_depth_exceeded`,**且不是** `#fuel_exhausted` | 紅 |
| **R2** | 紅 | 深度耗盡（blur）的 `%cause` 同上 | 紅 |
| **R3** | 紅 | 跳數預算用盡回 `#routing_budget_exceeded`,**且不是** `#semantic_eclipse` | 紅 |
| **R4** | 紅 | **沒有任何地方再鑄 `#semantic_eclipse`**:掃 interpreter 全樹的 `cause: BottomCause::SemanticEclipse` 建構點,須為 0。**不是**要求該名消失——§2.7.1 明文允許保留**讀取**能力(同 `#invalid_path`)。掃描自帶控制:樹裡必須先找得到 cause 建構 | 紅 |
| **P1** | 釘 | **`⊥` 的 CAID 不隨 cause 改變**——兩個不同 cause 的 ⊥,CAID 必須相同 | 綠 |
| **P2** | 釘 | **燃料耗盡所產生的 `#blur`,其 CAID 逐位元組不變**(`fuel=5` → `e4dc016e…`) | 綠 |
| **P3** | 釘 | `#timeout` 與 `#peer_timeout` 仍可分(§1.3 的前例不得被本弧回退) | 綠 |

### 4.1 校準(2026-08-09,基線 `dev 72c5fa8`)

**三支控制／釘綠、四支紅各自紅在自己的理由上:**

| # | 基線訊息(摘) |
| :-- | :-- |
| R1 | `depth exhaustion reported as fuel exhaustion — the remedy it hands the operator (add fuel) is the wrong knob:`<br>`{ a: _\|_ (%cause: #fuel_exhausted)  ;; Resource exhausted in strict mode }` |
| R2 | `blur from depth exhaustion is caused #fuel_exhausted` |
| R3 | `a spent routing budget is reported as a suspected attack` — `left: "semantic_eclipse"` / `right: "semantic_eclipse"` |
| R4 | `#semantic_eclipse is still being minted at 1 site(s): ["cause: BottomCause::SemanticEclipse,"]`(掃描控制先綠:樹裡確實有 cause 建構) |

**校準過程量到兩件,兩件都已反映在探針裡:**

1. **`BottomCause::as_tag()` 回的是不帶 `#` 的裸名**(`"timeout"` 而非 `"#timeout"`)。
   帶 `#` 的形出現在**另外兩處**——`BottomDetail::as_cause_combo`(value.rs:1174 起)
   與 `crates/oo/src/main.rs:49`。**同一個對映寫了三遍,兩種拼法**;
   探針改用裸名,並把這件事列為相鄰項(§9)。
2. **`oo init` 在 `ScratchDir` 上會失敗**,而 `oo run` 不需要它(`where_the_conflict_is`
   的既有探針也是直接 evolve)。原本的 `assert!(init.status.success())` 會讓
   **每一支探針都因為錯誤的理由而紅**——已移除。

### 4.2 R2 是**唯一**會移動 CAID 的斷言

R2 綠掉的那一刻,`6ebb46d7…` 就不再是那個值的位址。**這是本弧唯一的破壞面**,
而 **P2 是它的界線**:燃料側不得跟著動。交付若讓 P2 變紅,即為越界。

---

## 5. 成功標準

1. §4 四支紅全綠、三支控制／釘不動。
2. workspace **≥ 1802 / 0 / 3**,`conformance` 143/143,`genesis` 11/11。
   〔開弧基線(含本工單探針)實測 **1798 / 0 / 7**;四支紅解除 `#[ignore]` 後
   7 → 3、1798 → 1802。交付前基線(無探針)為 1793 / 0 / 3。〕
3. `nlang-spec/scripts/error-code-inventory.py` 的反向清單由 **3 降為 2**
   (`#semantic_eclipse` 消失;`#invalid_path` 與 `#stack_overflow` 依裁定保留)。
4. **交付必須自行回報 `6ebb46d7…` 的新值**——那是破壞性條目要登記的東西。

---

## 6. 不變量

- `bn_serial.rs` **只准新增 `BlurCause::MaxDepthExceeded` 的位元組對映**,其餘一行不得動。
- 任何列舉**只准在尾端新增**,不得刪除、不得重排(fmt v2 append-only)。
- `crates/*/tests/**` 除 §7 明文授權者外,**只准移除 `#[ignore]`**。
- 不得 `git add -A`。

---

## 7. 對既有測試的明文授權

`crates/interpreter/tests/semantic_eclipse_test.rs` 斷言的是**已被 §2.7.1 推翻的舊裁定**,
因此它**必須**被更新。授權範圍嚴格如下:

* **准**:把 `BottomCause::SemanticEclipse` 的期望改為 `BottomCause::RoutingBudgetExceeded`;
  把測試名 `test_find_hop_budget_exceeded_returns_semantic_eclipse` 改為指向新標籤的名字;
  把 `assert_eq!(BottomCause::SemanticEclipse.as_tag(), …)` 改為對應的新斷言。
* **不准**:刪除任何測試、弱化任何斷言、把測試標 `#[ignore]`。
* **理由要寫在該檔的檔頭註解裡**,並指向 ERROR_CODES §2.7.1。

〔規則:探針修改權在驗收方。本節是**逐項的例外授權**,不是通則的放寬——
因為那支測試編碼的是規格已經改掉的東西,留著它會使交付**必然**失敗。〕

---

## 8. 收尾分工

| 誰 | 做什麼 |
| :-- | :-- |
| #3 | §3「做」四項 ＋ 移除 `#[ignore]` ＋ §7 授權範圍內的既有測試更新 |
| 驗收方 | 診斷純度／探針完整性／獨立全 workspace 重跑／重複 ×5／對抗／跨版本;**CHANGELOG 破壞性條目 #9 與 ENGINE_SYNC 由驗收方寫** |

---

## 9. 相鄰項(本弧不做,已掛帳)

| 項 | 內容 |
| :-- | :-- |
| `#semantic_isolation` 的實作 | 需要對兩條信任路徑的結果做 meet 並判 `_\|_`(APP_05 §7.3 判準)。**規格已有,引擎全無**;與 disc 029 §4 同源 |
| `#max_nodes_exceeded` 等三個 | 引擎從未鑄。**未量到錯誤回報**,故不在本弧——不要為了對齊而發明鑄造點 |
| `BlurCause::StackOverflow` 變體刪除 | 在 CAID 位元組表內,屬 fmt 紀律 |
| `#incomplete` 未實作 | 規格有狀態、引擎無。見 ENGINE_SYNC 2026-08-09 |
| **BottomCause → 顯示字串的三份對映** | `as_tag()`(裸名)／`BottomDetail::as_cause_combo`(帶 `#`)／`oo/src/main.rs:49`(帶 `#`)。**一個概念三處拼寫、兩種形式**,新增一個 cause 必須記得改三個地方——而**忘記其中一個不會有任何測試變紅**。〔量 2026-08-09,校準時撞到〕**本弧會新增兩個 cause,因此會第一次同時付這筆代價**;工單要求交付**三處都改**,但**不要求合併它們**——合併是另一件事 |

---

## 10. 驗收(驗收方,2026-08-09)

**結論:接受,引擎側零代修。** 交付 `feee5ee`,開弧 `1d23f31`,基線 `72c5fa8`(v0.13.0)。

### 10.1 量測

| 項 | 結果 |
| :-- | :-- |
| 探針 | **9/9** |
| **重複穩定 ×5** | 9/9 五次全同 |
| **獨立全 workspace 重跑** | **1802 / 0 / 3**(＝成功標準) |
| genesis | **11/11** |
| conformance | **143/143**(語料六向量由驗收方更新,見 10.4) |
| 深度型 `#blur` 的 CAID | `6ebb46d7…` → **`6b537130…`**(如預期移動) |
| **釘 P2:燃料型 `#blur`** | **`e4dc016e…` 逐位元組不變** |

### 10.2 診斷純度

引擎側四檔改動皆在射程內。三個列舉**全在尾端新增**、無刪除、無重排。
`BlurCause::StackOverflow` 保留而不再鑄。探針**只刪四行 `#[ignore]`**。

**交付比工單多做對一處**:`BottomCause::obstruction_degree` 的 `=> 3` 分支
也必須收新變體——工單未列。連同 `as_tag`／`as_cause_combo`／`oo/src/main.rs`,
**一個新 cause 要改四處,而漏掉任何一處都不會有測試變紅**(§9 已掛帳)。

### 10.3 驗收方的工單有兩處錯,根因記在工單側

1. **§7 的例外授權漏了一個檔**。只列了 `semantic_eclipse_test.rs`,
   實際 `disc_multihop_test.rs` 同類。#3 用同樣的理由改了並寫了註解。
   **根因:我掃了鑄造點,沒掃測試裡的期望點**(與 v0.6.0「工單列舉漏兩個套件」同型)。
2. **成功標準 §5.3 與自己的裁定衝突**。我寫「反向盤點 3 降為 2」,
   但 §2.7.1 **明文允許保留讀取能力** ⟹ 列舉分支必須留著 ⟹ 盤點必然仍看得到。
   **正確的檢查是探針 R4(鑄造點 = 0),而它是綠的。**

### 10.4 語料六向量:它們原本編碼的就是這個缺陷

`L2-22／24／26／57／58` 全是 `1 + 1 + …`(深度形),期望由 `#fuel_exhausted`
改為 `#max_depth_exceeded`,無爭議。

**`L2-21 runaway-cause-honest` 不是**——它撞上 SPEC_08 §3.2.2 第 3 款的**明文處方**
「不可判定的 runaway ＝誠實 `#fuel_exhausted`」。驗收方**停在這裡等用戶裁定**,
未自行改語料。用戶裁定(2026-08-09):**改條文,依實際耗盡者**。
理由(用戶):原文成立於只設想 CPU／時間兩種資源的時期,而依 `%cause` 的設計理念
**標籤只增不減**,把處方綁死在一個名字上每加一種資源就要改一次。

### 10.5 對抗量測抓到一件比本弧更大的事 → **W4″**

**新名字指向的旋鈕是壞的。**

| 量測 | 結果 |
| :-- | :-- |
| 臨界值二分 | **n=256 通過 / n=257 失敗**——正是預設值 |
| 設 `~%Config.max_unification_depth: 4000` | 臨界值**仍是 256/257** |
| 同一次執行讀回該旋鈕 | **4000**(寫進去了、讀得到、**沒有被用**) |
| 提交後再讀 | **256**(連值都沒存活) |
| **v0.7.0 同一組量測** | **同樣忽略該旋鈕** ⟹ **先於本交付至少六個 minor** |

⟹ 本弧使名字指向了**正確的**旋鈕,而那個旋鈕不動。
**論旨只兌現一半**,但**不是交付的責任**——工單沒有要求旋鈕。已開 W4″。
