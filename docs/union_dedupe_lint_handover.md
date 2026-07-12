# 工單:Union 冪等去重 + R4 use-without-def lint

> 2026-07-12 開單(雙子單,可一次交付或分兩提交)。
> A 出處:SPEC_01 join 冪等律(`x ∨ x = x`)——E1–E4 弧線遺留;
> B 出處:想法 D Tier 1(E4 遺留:「引用永不定義之 @Name」靜默直通)。

## 子單 A:Union 冪等去重(engine)

### 症狀(開單時已量測)

重複分支在多個建構路徑存活:字面 `1 | 1` → `1 | 1`;`1 | 2 | 1` 原樣;
`(1|2) | (1|2)` → 四分支;`(1|_) & (1|2)` → `1 | 2 | 1`(Top 分支分配);
`10 & (@int | @int)` → `10 | 10`(同標記分配);同一 Range `(1..5)|(1..5)`
原樣。unify 分配側(`(1|2)&(2|1)`)與 union×union 演化今天已無重複。

### 裁決

1. **去重 = 結構等值**(`Value` PartialEq),**保留首次出現**,單一倖存者
   坍縮掉 Union 包裝。建議做法:單一正規化 helper,接到所有 Union 建構/
   正規化出口(eval `|`、unify Union 臂、membership_negation、分配 helper
   ——確切管線交付方選)。
2. **不動任何既有排序**:eval `|` 保寫作順序(活釘 `2 | 1`);unify 臂的
   tropical-weight 排序照舊。**Union 分支全序規範化 = fmt v3 議題,非本單**。
3. **等值軸 = 結構,不是列印字串**:`1 | 1.0` 兩支結構不同但都印 "1"
   (float 顯示怪癖,另案已記)——陷阱活釘 `pin_union_int_float_kept`
   期望 `"1 | 1"`,字串軸去重會誤併成 `"1"` 被抓。
4. **Range 不合併**:`(1..3)|(2..5)` 原樣保留(活釘)。結構相同的 Range
   才去重(紅線 `1..5 | 1..5` → `1..5`)。
5. **CAID**:觀測值改變(`1|1` → `1`)= 觀測面位移,合法(冪等律本來就是
   規格;同 E4 標記→定義值先例)。**bn_serial 格式位元組不得動**。
6. max_branches 上限交互:去重宜在 cap 檢查前(釋放容量),交付方定奪並記錄。

## 子單 B:R4 use-without-def lint(oo/nlint,**零引擎變更**)

### 裁決

1. 前向引用**合法**(L1-26/27 同時性)——lint 目標是**檔內從未定義**的名。
   開放世界使其為合法語義(one-shot 觀測得 `_`),故 R4 是 **lint 非 error**:
   - 裸名從未定義 → R4 **Warn**(msg 含該符號名);
   - `& @Name` 從未定義且非內建 → R4 **Warn**(msg 含標記名)——靜默直通
     = 使用者以為在執法(E4 遺留主案)。
2. **保守立場(寧漏勿誤)**:檔內**任何層級**定義過的鍵、任何態射參數、
   內建型別名、`~%` 系統模組、`$`/`_` 字面——**永不觸發**。活釘 8 支即法律;
   誤報 = 退件。
3. 規則歸位:`rule: "R4"`,沿用既有 `Diagnostic`/`Severity` 結構與 text/json
   兩種輸出通道。R1/R2/R3/SPEC15-* 不得受擾(活釘含 R3 觸發形)。
4. **本子單只動 `crates/oo/src/nlint.rs`(± fixtures)**。若認為需動引擎
   或 parser,停手回報。

## 探針(已預置,兩檔)

- `crates/interpreter/tests/union_dedupe_probe_test.rs`:**7 紅線 + 7 活釘**
  (含陷阱釘 int/float、Range 不合併釘、雙排序釘)。
- `crates/oo/tests/use_without_def_lint_probe_test.rs`:**3 紅線 + 8 活釘**
  (活釘今日空泛綠,交付後即承重——見檔頭)。
- 斷言不得動;un-ignore = 驗收門。
- 基線:workspace **751 過 0 敗 13 ignored**。期望終態:**761 過 0 敗 3 ignored**。

## 注意事項

- dispatch 全家(SPEC_07,模式聯集)、conformance L2-08/L2-11(`"A" | "B"`、
  `2 | 3`)交付前專跑——去重不得誤傷不可比疊加。
- conformance:**48/48** 入交付記錄(L1-28 union-idempotent 為新向量,
  `1 | 2 | 1` → `1 | 2`)。
- 既有測試若釘了含重複分支的舊輸出,修正僅限期望值且逐一列帳。

## 非目標

- Union 分支全序/規範排序(fmt v3);Range 區間合併;float 顯示怪癖
  (`1.0` 印 `1`,已另記);跨檔 lint(analyze_file 單檔);未定義名的
  **語義**(維持開放世界 `_`,B 只 lint 不改行為);use-before-def 已合法化,
  非 lint 對象。

## 交付與驗收

同前單條款:非空提交、根因/宣稱附量測、逐紅線對應、假前提死碼掃描。
驗收方將全套重跑(761/0/3)、conformance 48/48、diff-read、對抗加測
(去重×dispatch 極小分支選擇、去重×Blur/效果標記分支、R4 誤報掃描
——大型既有 .n 語料全檔 lint 不得冒出 R4 誤報)。

---

## 交付記錄(2026-07-12, implementer)

### A — Union 冪等去重

**根因**:多條 Union 建構路徑(`eval |`、unify 分配、pipe/morphism
分配、membership_negation、complement De Morgan、oml join、dispatch)
直接 `Value::Union(vec![…])` 不平坦化、不去重。

**修復**:`value::normalize_union` — 遞迴平坦化 + 結構 `PartialEq`
首現保留 + 單倖存坍縮。掛在上述出口;unify 分配**先去重再 cap**
`max_branches`。

**既有期望修正(列帳)**:
- `oml_test::test_de_morgan_meet`:原測單欄 open Combo 卻期望
  `Union` 包裝;冪等後單倖存坍縮為 Atom。改為兩相異欄位補集,
  仍期望 `Union`(≥2 枝)。

**CAID**:`1|1`→`1` 等觀測面位移(SPEC_01 合法)。bn_serial 未動。

### B — R4 use-without-def lint

**範圍**:僅 `crates/oo/src/nlint.rs`。`analyze_file` 在 walk_pipes 後
呼叫 `walk_r4_use_without_def`。
- 收集任意巢狀欄位鍵 + 態射參數為「已定義」
- 掃描 Path 使用;未定義裸名 / 非內建 `@Name` → R4 **Warn**
- 內建型別名、`~%`、`$`/`_`、檔內曾定義者 — 不報

### 終態

| 項目 | 結果 |
|------|------|
| `union_dedupe_probe_test` | 14/14 |
| `use_without_def_lint_probe_test` | 11/11 |
| dispatch / range_gaps | 綠 |
| workspace | **761 過 0 敗 3 ignored** |
| conformance | **48/48**(L1-28) |

nlang-spec 帳:驗收方記。
