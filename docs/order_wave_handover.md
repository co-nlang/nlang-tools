# 工單:序關係波 W1+W2(數值謂詞 + 原子序翻面)

**開單**:2026-07-20(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。
**注意**:本弧含 **v0.2.0 後首筆破壞性變更**(CHANGELOG Layer 1)。

## 1. 法源(波計畫已批 2026-07-20)

- **W1(SPEC_09 §3 新表列)**:`~%Math./lt /lte /gt /gte` 布林
  謂詞;int/float 依值跨比(SYNTAX_02);非數值輸入 → ⊥。
- **W2(SYNTAX_06 §2.5/§4 #10 既有法,偏差退役)**:原子
  `<`/`<=`/`>`/`>=` = 子集語義 `A <= B ⟺ (A & B) = A`——相異
  原子單集不互含 → 乾淨 `#false`(**非 ⊥**;`=` 家族不吸收);
  自反 `#true`;`<`/`>` 真子集(相等 → `#false`);原子 vs 型別
  原子=子型別(`1 <= @int` → `#true`、`1 < @int` → `#true`、
  `@int <= 1` → `#false`)。
- **不動軌**:poset 鏈序=格序本尊(宣告序照答);`=`/`==` 兩家
  族;極值律(⊥ ⊆ x/x ⊆ Top/Top ⊄ ⊥);combo/union 序=W3
  凍結(`#conflict` 照舊);blur×序=W3。

## 2. 病灶(v0.2.29 量測)

原子 `<` 族判數值(`3 <= 5`/`3 < 5`/`5 > 3` → `#true`=已記載
偏差);`1 <= @int` → ⊥ `#conflict`(子型別面未實作);
`~%Math./lt` → ⊥ `#missing_key`(謂詞不存在)。翻面穩定面:
`3 >= 5` `#false`、`2 <= 2` `#true`、`3 <= 3.0` `#true`(依值
同單集)、poset 全序面、極值面。

## 3. 修法方向與位點

- **W1 位點**:`lib.rs` math_builtins 表+`eval.rs`(或所在)之
  math builtin 分派——四謂詞,柯里化/管道與既有 math 態射同軌
  (`3 |> ~%Math./lt 5` 語義=lt(5, 3)=`#false`,照既有二元
  builtin 參數序慣例);int/float 依值跨比;非數值 → ⊥(照
  math 族既有錯誤形制)。
- **W2 位點**:`<` 族原子比較臂(eval 層 cmp)——數值分支改
  子集歸約:同值(依值,跨 int/float)→ `<=`/`>=` `#true`、
  `<`/`>` `#false`;異值 → 全 `#false`;原子×型別原子走既有
  unify 子型別判定(`(A & B) = A` 歸約或等價機構)。**poset
  分支勿動**;combo/union 臂勿動(維持 `#conflict` 凍結)。
- **語料遷移(交付步,協議內)**:`tests/unit/test_comparison.n`
  第 4 行 `~t1: (10 > 5)` → `~t1: ~%Math./gt 10 5`(其餘不動;
  `2 <= 2`/`"a" == "a"` 翻面穩定)。
- **不動**:`==`/`!=`/`=` 機構、poset(SYNTAX_10)、G1 combo
  等值、blur 二段律、W3/W4 範圍、parser。

## 4. 門(紅)與釘

**已預提交+校準**(6 紅全紅正因、5 釘全綠;另兩處開單遷移紅
=cmp_extremes `finite_numeric_compare_unchanged`(五面全改
`#false`)、eval_test `test_cmp_eval`(`10 > 5` → `#false`);
conformance 遷移紅×2=L1-20 期望 `#false`、L2-10 條件改拼
`~%Math./gt`)。

- `crates/interpreter/tests/order_wave_probe_test.rs`(新檔):
  紅=W1 四謂詞雙面/int-float 跨比/柯里化+管道;W2 原子翻面
  四拼(L1-20 孿生,含字串 `"a" < "b"`)/原子×型別子型別四面。
  釘=自反+翻面穩定鏡像+浮點依值/poset 三面/極值三面(含
  L1-23 孿生)/`=`/`==` 家族/combo+union 凍結 `#conflict`。

交付=移除全部 8 個 `#[ignore]`(6 新紅+2 遷移紅),探針檔
**其餘一字不改**(修改權在驗收方)。全 workspace 一顆不得翻紅;
語料非 pending 不退(含遷移後 test_comparison)。

## 5. 目標與交付紀錄

**目標**(基線實測 2026-07-20,先量後寫):探針 11/11;workspace
**1278/0/3**(基線 1270/0/11);conformance **123/123**(基線
121/123,L1-20/L2-10 翻綠);語料非 pending **75/0**(含遷移後
test_comparison)。

**交付紀錄**(交付方填;先寫再回報):

- [ ] 交付 commit(s):
- [ ] 根因與修法(謂詞分派位點、cmp 臂改法、poset 分支保全寫明):
- [ ] 探針/workspace/conformance/語料 四數:
- [ ] 申報事項(範圍外接觸、歧異記錄):

## 6. 驗收紀錄(驗收方)
