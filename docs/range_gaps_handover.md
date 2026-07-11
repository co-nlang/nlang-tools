# 工單:Range 語義補完 E1–E3(型別標記×Range/分派鍵/正交補)

> 2026-07-11 開單。缺口由規格側「比較節遷移」驗證時首次曝光(非回歸——舊拼法
> 從不過 parser)。規格檔案:nlang-spec `meta/ENGINE_SYNC.md`「Range 語義補完
> 缺口」節。被卡的規格向量:REAL_05 合規向量 L1-05、SPEC_03 §3.2/SPEC_06 §/
> SPEC_07 §5 分派範例、SPEC_07 §5 check_pos(`!(..0)`)。
> **E4(nominal `@Name` 引用不攜帶定義)不在本單——另單處理,勿順手修。**

## 探針(已預置,commit 見 git log)

`crates/interpreter/tests/range_gaps_probe_test.rs`:

- **15 條紅線**(`#[ignore]`)——**去掉 ignore 並全綠 = 驗收門**。斷言本身
  一字不得改;紅線頭註可補充但不得刪改語義。
- **10 條活測**(6 護欄 + 4 條「⊥ 側兩側釘」)——全程必須保持綠。
  4 條 ⊥ 側釘的意義:它們今天就綠,但是**因錯誤原因**(如 `@int & Range`
  整體先變 ⊥ 再吸收);修完後必須**因正確原因**依然 ⊥。這是 Atom(Bottom)
  案的教訓制度化:修 bug 移動家族邊界時,兩側都要釘。

期望終態:workspace **682 過 0 敗 3 ignored**(667+15;3 ignored 為既存已知,
與本單無關,不得動)。

## E1 型別標記 × Range(小刀)

**現況**:`@int & 6..` → ⊥。根因:`type_constraint.rs` `validate_value` 對
`Value::Range` 落 `_ => Fail`(unify.rs:149/154/202 進 `type_constraint_meet`)。

**裁決**:`@T & (a..b[..s])` = **Range 原樣**(精化語義),iff 每個**非錨點界**
單獨通過 @T 驗證;錨點(TagStart/TagEnd)一律通過。任一界不過 → ⊥ Conflict。

**硬性約束**:
- `PassWithProjection` 視同 Pass,但**界一律不得改寫**(`@float & 1..9` 通過,
  界保持 Int `1..9`,不得變 `1.0..9.0`)——bn_serial 位元不得位移,fmt v2 已凍結。
- 修在 validate/meet 層,不是 unify 加早臂;若確需早臂,**不擁有的值種一律
  DECLINE(fall through),不得 Conflict**——5b501e5 臂序蟲族已三例,紅線
  `e1_union_distributes_over_marker_range` 就是打這個的。
- 鏡像(`6.. & @int`)走同一路徑,禁止鏡像複製臂。

## E2 分派鍵含 Range(三個子缺陷,`dispatch.rs`)

**現況**:`{ @{ 4.. }: "A" } 5` → ⊥「Rule has no %code」。定位量測(開單時已做):
combo 構造正常(range 鍵正規化為字串 `4..#_`);Conflict 全在 dispatch 內。

1. **`apply_single_rule` 只認 `%code`**:常值規則(`{{%val: v}}`)直接
   「Rule has no %code」⊥。修:`%val` 臂 → force 後**回傳 v**(不是與 arg unify)。
2. **`resolve_pattern` 是字串型解析**(dispatch.rs:49–97):range 正規鍵字串
   (`4..#_`/`1..9`)不被辨識,落到 **Top(全匹配)——靜默錯配**。修:鍵字串
   先嘗試以 parser 解析為 range 字面量(可用 `parse_expr_only` + ctx eval),
   成功 → `Value::Range` 作 pattern。**非 range 鍵維持現行為**(字串型架構
   的整體重造不在本單;field_key path-vs-named 另案)。
   注意:動態鍵(`@{ @int & 4.. }`)構造時先 eval——**依賴 E1 先修**,修好後
   鍵值 = Range、存鍵 = 其 canonical print,與 1. 同路。交付時請自行量測確認
   構造後的鍵字串形態並記錄。
3. **`filter_minimal_branches` 比錯對象**(dispatch.rs:99–126):現行比較
   unified 值——原子引數下所有 arm 的 unified 都等於該原子,極小元判定失效
   (SPEC_07 情境 C 永遠 Multiple)。修:比較 **pattern 值**:
   `p_i & p_j == p_i 且 ≠ p_j ⟹ p_i 嚴格更細 ⟹ j 非極小`(Range∩Range 已有,
   `range_intersect` 直接可用)。
   **兩側釘死**:`e2_subset_pattern_is_unique_minimal`(子集 → 單選 "C")與
   `e2_incomparable_patterns_stay_multiple`(不可比 → 保持 "A"|"B")。
   **本單最高回歸風險點**:`dispatch_test.rs` 全數必須原樣綠,交付前單獨跑。

## E3 Range 正交補(裁決先行,實作面窄)

**現況**:`!(..0)` 即 ⊥(`complement.rs` 無 Range 臂落 `_ => Conflict`)。

**裁決(不可協商)**:
- **禁止新增 Value 變體**。`!(稠密Range)` 無閉形式具體化(`(0,∞)` 不是閉閉
  區間);殘形值表示 = fmt v3 議題,不在本單。
- **禁止整數域具體化**(`!(..0)` ↛ `1..`):會錯殺稠密成員——紅線
  `e3_dense_member_passes`(`0.5 & !(..0)` → `0.5`)就是打這個的。
- 可用語義 = **meet 語境的成員否定**:`x & !(a..b)` ⟺ 若 `x & (a..b)` = ⊥
  則 x,否則 ⊥。閉端故 `0 & !(..0)` → ⊥(活測已釘)。
- 實作路線(建議,非強制):eval 的 meet 臂在求值運算元前辨識 AST 形
  `Unary(Not, e)`,若 eval(e) 為 Range → 改寫為成員否定;兩側運算元次序同路
  (鏡像紅線)。態射體 `@{ $ & !(..0) }` 是同一 AST 形(check_pos 紅線覆蓋)。
- **standalone `!(range)` 維持 ⊥**(活測 `guard_standalone_not_range_stays_bottom_not_silent`
  釘 Bottom;錯誤訊息可改善,靜默或給出錯誤的具體集合 = 違規)。
- `orthocomplement` 其餘臂一概不動。

## 非目標(碰了算越權)

- E4 nominal `@Name` 引用接線(另單)。
- 步進∩步進(CRT)、Range 作 `<`/`<=` 家族運算元(§4.10 另案)。
- `resolve_pattern` 字串型架構整體重造;field_key path-vs-named。
- fmt v3 殘形值;任何 bn_serial 佈局變更。
- 效能。

## 交付與驗收

- 交付 = nlang-tools `local` 上的 commits(不得空提交;`git show --numstat`
  須非空)+ 簡短交付記錄(改了哪些檔、每條紅線對應哪個修復、量測輸出)。
- **根因與宣稱須附量測**(counterfactual:stash 修復重跑探針證明紅→綠因果)。
- 驗收方將:全套重跑(682/0/3)、diff-read 全部改動、探針斷言逐條比對、
  對抗加測(Union/Thunk/Combo × 新臂;鏡像;`dispatch_test.rs` 專跑)、
  stash 反事實。歷史案例中「探針頭註被壓縮」屬良性,「斷言變動」= 直接退件。
