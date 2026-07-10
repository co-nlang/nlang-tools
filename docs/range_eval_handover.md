# 工單:Range／`@{ expr }` 求值語義落地——`Value::Range` + 最小 unify 刀

發出:2026-07-10。裁決已完成(nlang-spec `c3c7cdd`:SPEC_02 §3、SYNTAX_04 §4.5/§4.7),
本單為引擎落地。比 cmp extremes 大一號(新 Value 變體,碰 bn_serial 與 collapse)。
驗收方可能為另一模型,驗收章節在最後,自足。

## 裁決摘要(規格原文為準;本節只是導讀)

1. **`a..b` = 閉閉區間集合 [a,b],不是迴圈**。符號化格論值:觀測不物化、不塌縮。
   「range 不是 `for`」——半開迭代歸 `~%List./range`(態射 API 維持 `[start,end)`,
   **不動**;list_p25_test.rs 是它的活護欄)。
2. **缺界預設 = 序位錨點**:`1..` → `1..#_`、`..10` → `#_|_..10`、`..` → `#_|_..#_`。
   現行 parser 用 `Top` 表缺界是**撞規格**(序極值 ≠ 資訊極值),要修。
3. **`@{ e } ≡ e`(求值透明)**;`@{}` ≡ `_|_` 已落地勿動。
4. **unify 最小刀**:成員判定 + 無步進交集;步進∩步進另案。

## 症狀(基線 2026-07-10,`75a16c1`)

`1..10`/`@{ 5 }` 等一切 range/anon_set 表達式落 eval 萬用臂 → ⊥Conflict;
`{x: 1..10}` 觀測 `x: _|_` 全靜默;`t:{n: 1..10}` 後 refine `t:{n: 5}` → evolve
`Err(Conflict)`(正典單調演化做不到——與 Atom(Top) 案同款症狀面)。

## 施工面(預裁決;錨點已讀碼定位)

### A. `Value::Range` 變體(interpreter value.rs)

- `Range { start: Box<Value>, end: Box<Value>, step: Option<Box<Value>> }`——**界存
  求值後的 Value**(Atom Int/Float/TagStart/TagEnd),非 Expr。
- `collapse(Range)` = 自身(no-op);`contains_blur` = false;effect = 界之 max。
- **canonical print** `to_nlang`:`a..b`/`a..b..s`,無空格(探針釘死)。
- **bn_serial**:新 tag byte(現有程式中 range 全 ⊥、無既存 CAID 依賴——低風險,
  仍須回函記 CAID 位元格式新增)。content_hash 走一般路徑。

### B. eval 臂(eval.rs `ExprKind::Range` + `ExprKind::AnonSet`)

- `Range`:對 start/end/step 各 `eval`+`force`(變數界因此免費支援:`1..y` 於觀測期
  對當下宇宙解析——紅線 `range_variable_bound_resolves_at_observation` 釘此)。
  界為 Bottom → 整體 ⊥ 傳播。界非數值/非錨點(如字串)→ 仍構造 Range 值(符號
  保留),語義運算不支援(見 D)。
- `AnonSet(e)`:**直接 `self.eval(e, ctx)`**(透明)。`AnonSet(Atom(Bottom))`(即
  `@{}`)經 e 的 Bottom 正規化自動 ≡ `_|_`——bottom_spelling 活護欄
  `anon_empty_set_is_bottom` 會驗,勿特判。

### C. unify 臂(unify.rs;放在 Ref/Thunk 保留臂之後、do_unify 之前)

真值表(閉閉;錨點 = ±∞):

| 左 & 右 | 結果 |
| :--- | :--- |
| 數值原子 x & Range | x ∈ [start,end] 且合步進 → x;否則 ⊥Conflict |
| Range & Range(雙方無顯式步進) | 交集 [max(lo), min(hi)]:空 → ⊥;單點 → **塌縮為原子**;否則 Range |
| Range & Range(任一方顯式步進) | **另案**(CRT)——⊥Conflict 並於回函記錄(不擴修) |
| 非數值 & Range、字串界 Range & 任何 | ⊥Conflict(誠實不支援;回函記錄) |
| Top/Bottom × Range | 走既有么元/吸收臂(在 Range 臂之前,自動正確) |

- 步進成員判定:`(x - start) % step == 0` 且在界內(`10 & 0..10..2` → 10,閉端
  在步進上)。@int 無顯式步進 = 步進 1;@float 無步進 = 稠密(僅界判定)。
- 對稱:兩個方向都要(`5 & 1..10` 與 `1..10 & 5`)。

### D. parser 缺界預設修正(nlang-parser lib.rs `Rule::range` 臂)

`start.unwrap_or(Atom(Top))` → `Atom(TagStart)`;end 同 → `Atom(TagEnd)`。
**授權的 golden 變更(僅此三條)**:golden_ast.rs 的 `..`/`..10`/`1..` 三向量由
`Atom(Top)` 形狀改為錨點形狀(與 range_bounds_probe.rs 紅線一致)——這是修正
撞規格的向量,**不是弱化**;golden_ast.rs 其餘一概不動。fuzz 生成器不受影響
(它只生成有界 range)。

### E. 明確不做(非目標)

- 步進∩步進交集(CRT)、Range 與 `@int`/`@num` 型別格整合、Range 的 `<=` 子集比較
  (`1..5 <= 1..10`——現落 cmp 有限路徑尾端 Conflict,誠實,另案)、`%kind` 型別
  邊界標記(`@{e}` 透明,SYNTAX_04 §4.7 留白)、`~%List./range` 任何變更。
- memo 區(Stage 4/5)、cmp extremes 臂、Atom(Top)/Atom(Bottom) 正規化:照例
  **一字不動**。Range 值進 memo 屬 C tier 自然行為,無需特判。

## 驗收(探針已預置,`16db350`)

- `crates/interpreter/tests/range_eval_probe_test.rs`:**7 支紅線 `#[ignore]`**
  (觀測即自身×3 print、成員判定含錨點與閉端、步進成員、交集含單點塌縮與空⊥、
  宣告後精煉 evolve、變數界、`@{e}` 透明×3)。
- `crates/parser/tests/range_bounds_probe.rs`:**3 支紅線**(缺界錨點預設)。
- 活護欄(既存套件,不重複建):list_p25_test.rs(`~%List./range` 半開不動)、
  bottom_spelling_probe_test.rs(`@{}`≡⊥、吸收/`=` 家族)、cmp_extremes_probe_test.rs、
  golden_ast.rs(除授權三條外不動)、全 workspace。

**驗收 = 拿掉 10 個 `#[ignore]` 後全綠 + 全 workspace 綠。基線 2026-07-10 實測:
106 套 656 過 0 敗 13 ignored(10 = 本工單紅線;3 = 既存已知議題,不許動)。
交付後應為 666 過 0 敗 3 ignored。** 刪除或弱化探針=違反工單;golden 三條授權
變更之外動 golden = 違反工單。

## 給驗收方(如非本 session 的模型,先讀這段)

1. **不信回函,重跑量測**:`cargo test --workspace`(656/0/13 → 666/0/3)。
2. diff-read 探針檔(兩支)斷言未弱化;golden_ast.rs 的 diff **只含授權三條**;
   `git show --numstat` 自查非空提交。
3. CLI 逐條量測:`oo eval '{x: 1..10}.x'`(應印 `1..10` 非 `_|_`)、症狀表與
   真值表抽測、**非目標防火牆**:`~%List./range (0, 3)` 仍 `[0, 1, 2]`(半開)、
   `_|_ == _|_` 仍 ⊥、`3 <= 5` 仍 `#true`。
4. **CAID 檢查**:bn_serial 新增 Range tag byte(合法新增,無既存依賴)——回函
   須記格式版本影響;含 range 的 Thunk expr 之 to_nlang 已存在(parser 端),
   Value 端新 print 不影響既存 CAID。
5. 記錄:ENGINE_SYNC 補列、ROADMAP 銷帳。紀律:「根因」附量測;宣稱附數字;
   偏離預裁決升級不自改(特別是真值表與閉閉端點——那是使用者裁決,不是實作
   自由度)。
6. 兩家族邊界教訓(ENGINE_SYNC #19):本單邊界 = 「Range 語義」vs「既存 list.range
   /unify/cmp」——紅線釘新語義側,既存套件釘不動側,驗收兩側都要量。
