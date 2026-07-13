# 工單:G3 視界傳播律 (2026-07-13)

**執行者**:模型 #3
**驗收者**:專案腦(探針已預先提交,紅線不可動——**全部** `*_probe_test.rs`/`*_redline_test.rs` 檔皆紅線,不限本單)
**探針**:`crates/interpreter/tests/blur_horizon_probe_test.rs`(9 紅門 + 6 釘;已校準:紅門今日全紅、釘今日全綠)

**驗收 = 紅門全綠 + 釘全綠 + 全套件無退化(基線 856/0/3)+ 語料回歸 + conformance 全綠(含新增 L2-21/22,交付時應 61/61)+ test_canonical 出 pending(見 §3)。**

---

## 0. 裁定(已批;SPEC_08 §3.2.2 視界傳播律已入法)

根因:預設策略 Blur → 燃料耗盡正確產一等 `Value::Blur`(顯示
`#blur { %cause: …, %caid: … }`、決定論 CAID 皆既有)→ **值語境
消費點無 Blur 臂**:eval_math catch-all 鑄 ⊥ #conflict、原子 cmp
漏到結構比默默 #false。通用於一切燃料耗盡(平場 4000 項加法同死),
runaway 只是入口。

- **R1 值語境吸收**:math 運算元/原子比較運算元/一元運算/態射體內
  值語境遇 Blur → **原樣傳出**(cause/CAID/視界參數保全、效果 max、
  partial 不參與運算)。不得鑄 #conflict、不得默默布林。
- **R2 引數是載體**:綁定/force 邊界只攜帶 Blur,不消費;吸收發生於
  體內第一個值語境。**不必預設大改 dispatch**——工單量測一筆:若
  某綁定/force 邊界把 Blur 改鑄,那才是引數路徑的 bug(交付紀錄列
  量測結果)。
- **R3 cause 誠實**:燃料耗盡恆 #fuel_exhausted(Strict ⊥ 與 Blur
  %cause 同拼);#divergent 保留給偵測循環(L2-17 座標自指,釘)。
  同引數自呼偵測升級另案——今日誠實 fuel(紅門釘死)。
- **R4 meta**:Blur 之 `%cause`/`%type` 回 BlurCause 標籤。

## 1. 地圖與實作建議(含執行者自己的補充,採納入單)

- **短路位置 = 呼叫 helper 之前**,與既有 Bottom 短路並列:
  - `eval_math`(eval.rs:822 一帶):`if let Value::Blur(_) = va { return va; }`
    式短路(兩側;在 `value_context_operand` 之前)。**helper 不擴權**
    ——`value_context_operand` 繼續只服務「可剝殼的值」,對 Blur 的
    `other => Ok(clone)` 不再會被走到(短路在前)。
  - `eval_binary_cmp`(eval.rs:886 一帶):force_recursive 之後、兩家族
    分流之前或原子家族段內,Blur 短路(原子家族吸收;**集合家族
    `=`/`<`/`<=` 不動**——釘 `pin_lattice_eq_blur_current_behavior`
    凍結今日 #false,另案)。
  - 一元運算(eval.rs:622 `Unary`):collapse 後 match `_ => Conflict`
    同病;補 Blur 短路。
  - `<=>` Probe:量測現況並於交付紀錄記載;若同病,同法補(低風險)。
- **R4 meta 路徑**:lib.rs 導航 meta 段(`%cause`/`%type` 特殊段,
  :1385 一帶)現對 `Value::Blur(_) => val` 原樣回(lib.rs:1122 是
  force;meta 段另查)——量測 `big.%cause` 今日回
  `{{ %type: #conflict }}` 怪形(見 L2-22 校準),修至回 BlurCause
  標籤(`#fuel_exhausted`)。
- **R2 量測義務**:`big |> inc` 紅門過了即證引數路徑無恙;若不過,
  先量測抹除點(binding? force? unify?)再最小修,交付紀錄附量測。
- **Strict 路徑自測義務**:規格 R3 涵蓋 Strict(⊥ #fuel_exhausted)。
  探針走預設 Blur 策略;交付紀錄須附 Strict 路徑的 lib 級自測
  (`EvalContext::with_strategy(Strict)` + 耗盡 → ⊥ cause 為
  FuelExhausted 非 Conflict)一筆。

## 2. 邊界與陷阱

1. **本體地位不可互鑄**:Blur ≠ ⊥。不得順手把 Blur「升級」成
   ⊥ #fuel_exhausted(那是 Strict 的工作)。
2. **⊥ 短路優先序**:兩側一 ⊥ 一 Blur → 依既有 ⊥ 先查邏輯保持
   (⊥ 檢查在前,行為不變;交付紀錄註明順序)。
3. **效果標籤**:Blur 傳出時 effect 併運算元 max(BlurDetail.effect
   欄既有)。
4. **勿動** `handle_resource_exhausted`、Blur 顯示形、blur CAID 計算、
   `normalize_union`(Union × Blur 支未入本單;若語料曝異常,記帳
   勿修)。
5. **燃料成本勿調**:預設 10000 的量級檢討另案;本單不改任何 cost。
6. 全語料回歸 + conformance L2-21/22(今日紅)。
7. 交付紀錄照舊格式(根因、diff、量測、未動聲明)。

## 3. test_canonical 出 pending(G1+G3 雙阻塞已除)

`tests/pending/test_canonical.n` 遷回 `tests/unit/`,同時依現行法改拼:
- `test_canonical_order`:`~messy == {…}` 為 combo 家族誤用(G1 #12)
  → 改 `=`(SYNTAX_06 §4 #11)。
- `test_fuel_exhaustion`:期望改 `#fuel_exhausted`(R3;原註 #divergent
  已被裁定否決——向量跟法)。
- 檔頭 PENDING 註解改為結案註,unit 計數變動於交付紀錄記載。

## 4. 非目標

- 同引數自呼之 #divergent 偵測(force memo 級循環偵測,另案)。
- `=`/`<`/`<=` 集合家族 × Blur(另案;釘現況)。
- Union × Blur 分支語義、timeout `#incomplete`(SPEC_08 §3.2 已法)、
- 預設燃料量級、fmt/CAID。
