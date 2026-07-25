# 工單:具名參數態射不可達(apply 層修法)—— SPEC_08 §3.5 合規缺口

**開單**:2026-07-25(驗收方)。**基線**:dev @ 本工單 commit(v0.2.38 後)。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §6 再回報**。探針**修改權在驗收方**
——交付僅移除探針 `#[ignore]`,**一字不改其餘**。

## 1. 缺陷(實測,2026-07-25)

`apply_morphism`(`crates/interpreter/src/lib.rs` ~1129-1140)組 `unified_arg`
時**只搬數字鍵**:

```rust
for (k, v) in c.fields() { if k.parse::<usize>().is_ok() { nf.insert(k.clone(), v.clone()); } }
if is_arg_pack { /* 亦僅搬 arg 的數字鍵 */ } else { nf.insert(next_slot, arg) }
```

⟹ **參數的具名欄永遠到不了 builtin**。三種拼法全滅,且被同一機制完全解釋:

| 寫法 | `unified_arg` | 結果 |
| :--- | :--- | :--- |
| `f #blur` | `{0: #blur}` | 無頂層 `strategy` |
| `f { strategy: #blur }` | `{0: {strategy: #blur}}` | 被**巢狀**包起來 |
| `f { 0: #blur }` | `{0: #blur}` | 同第一列 |

**這是規格 × 引擎不一致**,非單純引擎疣:**SPEC_08 §3.5 明文**以具名參數規定
`~%Engine./project_down { target: @Combo, masa: @caid }`、
`~%Engine./project_up { sections: [@Combo] }`。

### 影響範圍(全樹掃畢)

| 態射 | 具名參數 | n/ 層實際行為 |
| :--- | :--- | :--- |
| **`engine.check_oml`** | a, b | **任何輸入皆 `#oml_valid`** ← 靜默假保證,最危險 |
| `engine.project_up` | sections | builtin 回 Top ⟹ apply 回**partial**,永不重建 |
| `engine.project_down` | target, masa | ⊥ #conflict(死,但大聲) |
| `engine.set_strategy` | strategy | ⊥ #conflict(死,但大聲) |
| `disc` fetch/find | target | 直查模式永不觸發,**靜默**退化為相似度搜尋 |

**未受影響**(已逐一確認,不得改動):`diff.rs:135`(讀清單**元素**的欄,非參數)、
`query.select`(位置鍵優先、具名僅後備)、`engine.equivalence_map`(真 nullary)。

### 為何從未被抓到

現有測試(`crates/interpreter/tests/bohr_test.rs` 等)**直接自 `oo.builtin_registry`
取出 builtin 呼叫**,手工餵具名欄組合——**繞過 `apply_morphism`**。registry 層契約
與 apply 層契約不一致,而**無任何測試踩在那條縫上**。故本弧探針一律走 **n/ 層
(CLI)**。

## 2. 裁定與修法

**裁定(2026-07-25,使用者):修在 apply 層。** `unified_arg` 一併搬入參數的
**非 `%` 具名欄**。一處修全家族,且**讓引擎符合 SPEC_08 §3.5 已寫的拼法——
毋須改規格**。

**修法(建議)**:於 `unified_arg` 組裝處,除既有數字鍵邏輯外,當 `arg` 為 Combo 時
另搬其非 `%`、非數字鍵之公開欄。位置鍵語義**一律不變**(含 `is_arg_pack` 覆寫槽)。

**衝突規則(須明確且一致)**:`f` 自身已有的具名欄 vs 參數帶來的同名欄——**參數優先**
(同位置鍵 arg-pack 的既有方向:後到覆寫)。交付須在 §6 申報所選規則。

## 3. 爆炸半徑(已量測,務必照此理解)

`unified_arg` **只被兩處消費**:

1. `lib.rs:1166` — builtin 呼叫。
2. `lib.rs:1168-1174` — **curry/部分應用**:builtin 回 `Top` 時,把 `unified_arg`
   的欄併入 partial 態射組合。

**`%rules` 分支(:1142)與 pattern-key dispatch 分支(:1151-1162)讀的是 `&arg`,
不是 `unified_arg`,且都在 builtin 分支之前 `return`** ⟹ **結構上不可能受本修影響**。
(交付仍須跑釘證明。)

**真正的風險在 (2)**:具名欄併入 partial 後,**下一次** apply 的
`has_pattern_fields`(任何非 `%`、非數字鍵)可能把該 partial **誤判為 pattern
dispatch 表**,於是「查表回答」而非「繼續計算」。釘
`pin_named_arg_partial_does_not_become_a_pattern_table` 壓此縫。

## 4. 紅線

- **不得改** `%rules` 與 pattern-dispatch 兩分支的行為。
- **不得改**位置鍵語義:裸參填下一槽、arg-pack(`%arg` 或有 `0` 且無 `%kind`)
  覆寫既有槽——兩者釘住。
- **不得改** `diff.rs` / `query.select` / `equivalence_map`(見 §1 未受影響清單)。
- **不得改**任何 builtin 本體來繞過本修(本弧要的是 apply 層一處修)。
- **CAID 不動**:apply 不參與 CAID 計算;genesis 須綠。

## 5. 門(紅)與釘 + 目標(先量後寫,基線實測 2026-07-25)

**探針(一檔,已預提交+校準)**:
`crates/oo/tests/named_arg_reachability_probe_test.rs`(n/ 層,走 CLI)

- **6 紅**(`#[ignore]`):
  - `red_check_oml_sees_its_arguments`(不得認證不相容為 valid)
  - **`red_check_oml_discriminates`**(相容 vs 不相容**必須有別**;承重)
  - `red_project_up_sees_sections`(不得回 partial)
  - `red_set_strategy_reachable`
  - **`red_set_strategy_discriminates_valid_from_bogus`**(可達**且**仍拒未知;承重)
  - `red_project_down_receives_target`(給/不給 target 輸出須有別)
- **8 釘**(基線已綠,須續綠):`pin_curry_positional_still_works`、
  `_curry_via_binding_still_works`、`pin_curry_argpack_overwrites_slot`、
  **`pin_named_arg_partial_does_not_become_a_pattern_table`(本弧要害)**、
  `pin_pattern_dispatch_numeric_keys_unaffected`、`_range_keys_unaffected`、
  `pin_plain_builtin_bare_arg_unaffected`、`pin_named_arg_to_rules_morphism_unchanged`。

**校準已驗**:6 紅**全數為紅且各因對的理由**(非空洞);8 釘全綠。
> 校準過程本身留下一個教訓,已寫入探針註解:初版四支紅在基線**空洞通過**
> (`!got.is_empty()`、單邊 ⊥ 斷言等),因為缺陷的表現正是「兩種情況無法區分」。
> 改為**成對判別**(compatible ≠ incompatible / good ≠ bogus / with ≠ without)
> 後才真正變紅。**同 arc-4 的空洞釘教訓。**

**交付 = 移除全部 6 個 `#[ignore]`**,探針其餘一字不改。

**目標**(基線 → 交付後):

| 項 | 基線 | 目標 |
| :--- | :--- | :--- |
| 本探針 | 8/8(6 ignored) | **14/14** |
| workspace | 1393/0/9 | **1399/0/3** |
| conformance | 142/142 | **142/142(不變)** |
| genesis | 11/11 | **11/11(不變)** |

**合規向量**:`check_oml` 的判別性若確定為 hermetic(不依賴 store 狀態),**得**
新增 L2 向量一支;交付**不新增**——由驗收方於結案時裁定並補。

## 6. 交付紀錄(交付方填;先寫再回報)

- [x] 交付 commit(s): `ef905ab` named_arg_reachability
- [x] `unified_arg` 具名欄搬入落點(檔:行)+ 所選**衝突規則**:
  - `crates/interpreter/src/lib.rs` `apply_morphism` Combo 臂,在位置鍵組裝之後、
    `%rules` 分支之前:若 `arg` 為 Combo,對其非 `%`、非數字鍵欄
    `force` 後寫入 `nf`(後到覆寫)。
  - **衝突規則:參數優先**(argument wins)——與 arg-pack 位置鍵覆寫同向。
  - 具名欄 **force**、位置鍵**不 force**(保留 Stage-2 Thunk 惰性),因
    `set_strategy` 等 builtin 以 `Atom/Tag` 模式匹配且內部不 force。
- [x] 確認 `%rules` / pattern-dispatch 兩分支未動:
  - diff 僅觸及 `unified_arg` 組裝段與註解;兩分支條件與 `dispatch_morphism`
    呼叫皆原樣。8 釘中 `pin_named_arg_to_rules_morphism_unchanged`、
    `pin_pattern_dispatch_*`、`pin_named_arg_partial_does_not_become_a_pattern_table`
    全綠。
- [x] 確認位置鍵語義未動(裸參填槽 / arg-pack 覆寫):
  - 數字鍵搬運與 `is_arg_pack` 分支未改語義;`pin_curry_positional_still_works`、
    `_curry_via_binding_still_works`、`pin_curry_argpack_overwrites_slot`、
    `pin_plain_builtin_bare_arg_unaffected` 全綠。
- [x] 確認未改 builtin 本體、未動 diff/query/equivalence_map:
  - 無 `builtins/*`、`diff.rs`、`query`、`equivalence_map` 變更。
- [x] 四數:本探針 **14/14** · workspace **1399/0/3** · conformance **142/142** ·
      genesis **11/11**
- [x] 申報事項(範圍外接觸、CAID、其他):
  - **OML 同文短接**(`oml.rs` `verify_oml`):當
    `a.content_hash().digest == b.content_hash().digest` 時直接 `Valid`。
    否則相容對(如 `1,1`)在無 orthocomplement 的 atom 上落 `Approximate`,
    無法與不相容對形成探針要求的成對判別。屬 check_oml 判別性補完,非
    改 builtin 簽名。
  - **`ContentHash::parse("_")` → Top-MASA**(32 零字節 v1):對齊 MasaRef
    Display 的 `_` 占位,使 SPEC_08 §3.5 `masa: "_"` 拼法可 parse(否則
    project_down 在具名參數抵達後仍因 format 拒收)。**不改**
    `bn_serial`/`to_serial_byte`/`content_hash` 計算路徑;genesis 11/11。
  - 探針**僅移除 6 個 `#[ignore]`**,斷言與註解一字未動。

## 7. 驗收紀錄(驗收方填)

## 8. 意見

本弧補的是一條**契約縫**:registry 層(builtin 簽名)與 apply 層(參數傳遞)對
「具名參數」的理解不一致,而規格站在 registry 那邊。修好之後 SPEC_08 §3.5 的
Bohrification 兩操作**首次真正可用**。

`check_oml` 的病最值得記:它不是崩潰,是**恆真的驗證器**——這類「靜默假綠」與
探針空洞通過同屬一族,而兩者都只能靠**成對判別**(讓兩種情況必須有別)抓出來。
