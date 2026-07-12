# 工單:G1 combo 等值 (2026-07-13)

**執行者**:模型 #3
**驗收者**:專案腦(探針已預先提交,紅線不可動)
**探針**:`crates/interpreter/tests/combo_equality_probe_test.rs`(11 紅門 + 14 釘;已校準:紅門今日全紅、釘今日全綠)

**驗收 = 紅門全綠 + 釘全綠 + 全套件無退化(基線 813/0/3)+ 語料 72/0 + conformance 全綠(含新增 L1-33~36,交付時應 56/56)。**

---

## 0. 裁定(已批,SYNTAX_06 §4 #11–13 已入法;引擎追法)

病灶:cmp 比**未固化**combo——Thunk PartialEq = AST 等值**含 span 與符號
拼寫**。行內 `{a:1} = {a:1}` 都 #false;`x == x` #true 只因 span 巧合。

- **#11(`=` 於 Combo)**:固化後**外延結構等值**——六軸(data/rules/
  types/meta/system/local)+ closed + relations 逐欄;巢狀遞迴**同一關係**;
  欄位書寫順序不參與;**效果標籤參與**。與聯集去重(L1-28)**同一個**
  等值關係:全引擎唯一等值,cmp 與去重不得分家。
- **#12(`==`/`!=` 於 Combo)**:塌縮後仍非原子(無 `%val`)→
  **⊥ #conflict**,不得默默 #false。混血(帶 `%val`)塌縮至原子後照
  原子家族比:`(3 & {note:"n"}) == 3` → `#true`。`!=` 鏡像(誤用同樣
  ⊥,**不是** #true)。
- **#13(固化防火牆)**:比較家族運算元判定前必須固化;span/拼寫不得
  影響任何語義判定。

**家族邊界雙向移動**:`x == x`、`(x & z) == x`(combo)今日 #true →
裁定後 ⊥ #conflict。紅門已釘兩側(`=` 家族回歸的教訓)。

## 1. 地圖

- **`=`**:`eval.rs:694 ExprKind::LatticeEq`——今日只 `self.eval`,
  **不 force**;`_ => va == vb` 落到含 Thunk 的 PartialEq。修法:兩側
  `force_recursive`(lib.rs:1141)後再比。
- **`==`/`!=`**:`eval.rs:863 eval_binary_cmp` 原子家族段——今日
  `collapse()`(value.rs:823)只解**純包裝**(is_pure_wrapper),混血
  (`%val` + 資料欄)不解,落到 :943 `ca == cb` 結構比 → 默默 #false。
  修法:原子家族段內,塌縮後若仍為 Combo——有 `%val` 者取 `%val`
  (遞迴塌縮)入原子比較;無 `%val` 者 → ⊥ #conflict。
- **統一關係已存在**:`ComboVal PartialEq`(value.rs:320)= 六軸+closed
  +effect+relations,IndexMap 等值天然無序——即 `normalize_union`
  (value.rs:72)去重用的關係。**固化後**的值走它就是裁定的 #11。
- **rules 軸 span 盲**(紅門 `red_lattice_eq_rules_axis_span_blind`):
  態射繭不可 force,`Code`/殘餘 `Thunk` 的 Expr PartialEq 含 span →
  同拼寫不同行仍不等。修法自選,例:比較前對 Code/Thunk 走 span 無關
  的比對(canonical 列印比對,或 span 歸零後 AST 比)。**注意**:此
  關係 dedupe 也要同步用(唯一等值)——若你改在 Value/ComboVal 關係層,
  dedupe 自動受惠;若只改 cmp 局部,必須說明為何 dedupe 不分家。

## 2. 邊界與陷阱

1. **勿全域改 `collapse()`**:它被 Probe(`<=>`)、集合家族極值端等
   多處共用;混血取 `%val` 建議在原子家族段局部處理,或新增專用
   helper。若你選擇改 `collapse()` 本體,交付紀錄必須列出全部呼叫點
   的行為論證 + 全套件證明。
2. **釘 `pin_combo_lte_stays_conflict`**:`<=` 於 combo 今日 ⊥
   #conflict——**§4.10 另案**,本單不得順手改。
3. **釘 `pin_hybrid_observe_current_full_print`**:混血**觀測**今日印
   全 combo(規格 §4 #6 說讀 `%val`)——已另帳 **G6**,本單不碰
   觀測/顯示路徑。
4. **釘 `pin_lattice_eq_morphism_spelling_sensitive`**:span 盲 ≠ alpha
   等價;`(q -> q)` vs `(w -> w)` 維持 #false。
5. **效果參與**(Q2 已裁):Atom PartialEq 已含 effect;`LatticeEq`
   今日的原子臂 `x == y` 只比 AtomKind、**忽略 effect**——統一關係後
   effect 應參與。若此舉造成既有套件退化,停下記錄量測,勿私調。
6. **`=` 不吸收**:⊥/⊤ 於 `=` 是運算元(釘
   `pin_lattice_eq_bottom_clean_booleans`);force 兩側時勿把 ⊥ 短路成
   吸收(那是 `==` 家族的律)。
7. **force 深度與燃料**:`force_recursive` 於深巢/大 combo 逐欄扣燃料
   (既有);cmp 沿用呼叫方 ctx,勿新開 context。發散欄位比較 → 燃料
   耗盡走既有 ⊥ 路徑,勿加新上限。
8. 全語料回歸 + conformance L1-33~36(spec 側已入庫,今日四紅)。
9. 交付紀錄照舊格式(根因、diff、量測、未動聲明)。

## 3. 非目標

- `<`/`<=` 於 combo(§4.10 另案)、cmp × Union 分配(`(1|2) == 2`
  現 #false,另案)、G6 混血觀測塌縮(另案)、G3、去重規則語義本身
  (只允許共享關係的實作重構)、fmt/CAID(若 canonical 列印被借用於
  比對,不得動列印本身)。
- 語料掃描:combo `==` 斷言(清理時已改寫為逐欄,預期零波及;交付時
  驗證並列出掃描結果)。

---

## 交付記錄(2026-07-13, implementer)

### 根因

cmp 比未固化 combo;Thunk/Code `PartialEq` 含 span → 行內字面量也不等。
`==` 對無 `%val` 的 combo 落結構比 → 默默 `#false`(說謊)。

### 修復

1. **`=` (`LatticeEq`)**:兩側 `force_recursive` 後走 `Value::PartialEq`
   (六軸+closed+effect+relations;IndexMap 無序)。
2. **`==`/`!=`**:`force_recursive` + 局部 `atomic_family_operand`——
   pure wrapper / hybrid 取 `%val` 遞迴;無 `%val` 的 combo → ⊥ `#conflict`
   (**未**改全域 `collapse()`)。
3. **唯一等值 / span 盲**:`Value::PartialEq` 的 Code/Thunk 改
   `without_spans()` 比對 → dedupe 自動同步;拼寫仍敏感
   (`q`≠`w` 釘仍綠)。

### 既有期望修正(語料)

list 是非塌縮 combo,`==` 變家族誤用。改 `=`(結構等值):

| 檔 | 斷言 |
|----|------|
| `tests/unit/test_reflection.n` | `test_keys`: `==` → `=` |
| `tests/unit/test_stdlib_v2.n` | `test_list_sort/reverse/slice`: `==` → `=` |

其餘 `==` 掃描為原子/標籤/字串比較,零波及。

### 未動

`<=` 於 combo、G6 混血觀測、cmp×Union、`collapse()` 本體、fmt。

### 量測

| 項目 | 結果 |
|------|------|
| combo_equality probes | **25/25** |
| workspace | **838 過 0 敗 3 ignored** |
| conformance | **56/56**(L1-33~36) |
| unit+integration | **72/0**(語料修 4 斷言後) |
---

## 驗收紀錄(2026-07-13,驗收方)

**判定:通過——零代修(第七例)。**

獨立重測:探針 diff 僅 11 個 `#[ignore]` 移除、斷言原封;探針 **25/25**;
workspace **838/0/3**(813+25,吻合);語料 **72/0**(4 斷言改拼後);
conformance **56/56**(L1-33~36 關門)。

diff 逐條對裁定:`=` 兩側 force_recursive 後走全引擎 PartialEq(原子
特判臂移除 → effect 參與,Q2 落實;⊥ 仍為運算元不吸收);`==`/`!=`
局部 `atomic_family_operand` 遞迴剝 `%val`、無 `%val` → ⊥ #conflict
(⊥ 吸收檢查在剝殼**前**,吸收律保住;未動全域 `collapse()`——陷阱 1
照辦);span 盲落在關係層(Code/Thunk `without_spans()`,既有 API)
→ cmp 與 dedupe 同一關係(地圖建議路線)。語料 4 斷言 `==`→`=` 合法
(list = 非塌縮 combo,`==` 依 #12 為家族誤用;逐檔核對)。

對抗性邊界戳刺(工單外,全數健康):
- `{a:1} != {a:2}` → ⊥ #conflict(誤用判定不依值——不等也 ⊥,對)。
- 混血×混血:`(3 & {n:"a"}) == (3 & {n:"b"})` → `#true`(雙側剝殼)。
- 家族對偶:`(3 & {note:"n"}) = 3` → `#false`(外延結構不同)而
  `== 3` → `#true`(塌縮後同原子)——兩家族各答各的問題,正確。
- 巢狀 list `=`、Union `=` 自身 → `#true`。
- **反事實(v0.2.5 worktree)**:重複座標同拼寫合併、態射 combo 聯集
  去重——兩版行為一致,**無未申報行為移動**。觀察:態射繭去重在舊版
  已 span 容忍(機制在繭路徑,非 PartialEq;不影響本單),關係層
  span 盲的可觀測面 = cmp 紅門本身。

模型 #3 檔案:零代修第七例(連七)。
