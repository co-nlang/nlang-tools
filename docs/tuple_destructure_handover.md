# 工單:G5 tuple 參數位置解構 (2026-07-12)

**執行者**:模型 #3
**驗收者**:專案腦(探針已預先提交,紅線不可動)
**探針**:`crates/interpreter/tests/tuple_destructure_probe_test.rs`(8 紅門 + 7 釘;已校準:紅門今日全紅、釘今日全綠)

**驗收 = 紅門全綠 + 釘全綠 + 全套件無退化(基線 785/0/3)+ 語料 72/0 + conformance 全綠(含新增 L1-31,交付時應 51/51)。**

---

## 0. 現況(已量測)

- `((x, y) -> x + y)` 解析**正確**:`Morphism(Tuple([Path(x), Path(y)]), body)`。
- eval 打包(eval.rs:428)的參數鍵抽取對 Tuple 落到 `_` fallback → 規則鍵 `_`(resolve_pattern → Top,全配),但 `apply_single_rule` 只綁 `it`/`0`/鍵名 —— **`x`、`y` 從未入 scope** → 體內裸名走開放世界 → 全應用形態回 `_`(juxta / inline / pipe 皆實測)。
- tuple 值表示 = 閉繭 combo `{{0: v0, 1: v1, …}}`(純 data 軸、數字鍵);`.0` 導航正常;`$` 綁整包正常(L2-04 綠)。
- 柯里×tuple 嚴格對偶已健康:`(x y -> x) (3, 5)` 正確將**整個** tuple 綁給 `x`(已釘雙面)。

## 1. 裁定(SYNTAX_11 規則 4「一個 tuple 參數(位置解構)」;SYNTAX_09 §2 定義側選哪種、應用側就得用哪種)

### R-P 打包
eval Morphism 臂:param 為 **Tuple 且所有元素皆裸單段 Path** 時,單一規則附加 `%params` 元資料(序→參名;建議 combo `{0: "x", 1: "y"}` 或等價形,實作自選但 bn_serial 穩定)。規則鍵取可讀形(建議 `"(x, y)"`;resolve_pattern 對其自然回 Top,全配後由解構步驟把關)。
- **守門從嚴**:巢狀 tuple(`((x, (y, z)) -> …)`)、非 Path 元素、tuple 混柯里(`((x,y) z -> …)`,parser 摺疊守門已擋)→ **一律保持現行打包不動**。寧漏勿誤。

### R-B 綁定(apply_single_rule)
規則帶 `%params`(k 個名)時:
1. 引數(force 後)必須是 **tuple 形 combo**:data 軸恰有鍵 `"0"…"k-1"`、**arity 精確**(多欄、少欄皆敗)。
2. 敗 → `⊥ #conflict`(訊息帶 destructure 字樣佳,不釘死)。**不是 NoMatch 靜默**,也不准退化成 Top。
3. 成 → 逐名綁 `arg.i`,**保留**既有 `it` / context(`$`)= 整包引數之綁定(紅門 `red_tuple_body_sees_context_whole` 驗 `$.0` 與解構名共存)。

## 2. 地圖(量測過的落點)

- `crates/interpreter/src/eval.rs:428` ExprKind::Morphism —— pk 抽取處加 Tuple 臂(R-P)。
- `crates/interpreter/src/dispatch.rs:190 apply_single_rule` —— `%code` 分支內、綁 arg_map 處加 `%params` 解構(R-B)。
- `resolve_pattern`(dispatch.rs:62)**不用改**——非 @/#/數/字串鍵本就回 Top。
- 分派其餘機制(極小元過濾、Multiple 聯集)不動。

## 3. 邊界與陷阱

1. `it`/`0` 既有綁定語義:arg_map 現綁 `"0"` = **整個引數**(非 arg.0)。解構名若恰叫 `0` 不可能(裸名不會是數字),但**別**動 `"0"`= 整包的既有約定(有測試依賴)。
2. tuple 形判定看 **data 軸數字鍵**,勿用 `closed` 旗當判準(其他閉繭也 closed)。空 combo、含非數字鍵 combo → 非 tuple 形 → ⊥。
3. `%params` 是規則的**新元資料鍵**:確認 `%`-前綴鍵在 dispatch 迴圈(dispatch.rs:12 `starts_with('%')` skip)天然跳過——它不是 pattern,別讓它參與配對。
4. bn_serial/CAID:新增 `%params` 只影響**新寫法**(今日該寫法輸出 Top,無人依賴);既有值的 CAID 不得移動。交付紀錄註記此點。
5. 全語料回歸 + conformance:L1-31(spec 側已入庫,`tf: ((x, y) -> x + y)` / `tf (3, 5)` → `8`)交付時 51/51。
6. 交付紀錄照舊格式(根因、diff、量測、未動聲明)。

## 4. 非目標

- 巢狀 tuple 解構、tuple 混柯里參數(量測後另議)。
- Unit `()` 參數、具名解構(`{a, b}` 形)——規格無此物。
- 分派表(Combo-as-Union)鍵側的 tuple 模式。
- resolve_pattern 的字串模式體系重構。

---

## 交付記錄(2026-07-12, implementer)

### 根因

`ExprKind::Morphism` 打包時 Tuple param 落到 `_` 鍵; `apply_single_rule` 只綁
`it`/`0`/鍵名,從不綁 `x`/`y` → 體內裸名開放世界 → `_`。

### 修復

**R-P** (`eval.rs`): Tuple 且元素全為裸單段 Path → 規則鍵 `"(x, y)"` +
`%params` 閉繭 `{0: "x", 1: "y", …}`(Str atoms)。其餘 Tuple 形保持 `_` 打包。

**R-B** (`dispatch.rs` `apply_single_rule`): 見 `%params` 時,引數必須是 data
軸恰有 `"0"…"k-1"` 的 tuple combo(精確 arity);成功則逐名綁定,**保留**
`it`/`0`/pattern_key 整包與 `$`=整包。失敗 → ⊥ `#conflict`(無 message,
對齊探針顯示)。

### CAID / bn_serial

`%params` 僅影響新寫法(今日該寫法輸出 Top,無人依賴);bn_serial 佈局未改。

### 未動

巢狀 tuple、tuple 混柯里、resolve_pattern、分派極小元/Multiple。

### 量測

| 項目 | 結果 |
|------|------|
| tuple_destructure probes | **15/15** |
| workspace | **800 過 0 敗 3 ignored** |
| conformance | **51/51**(L1-31) |
| unit+integration | **72/0** |
