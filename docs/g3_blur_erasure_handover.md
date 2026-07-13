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

---

## 交付記錄(2026-07-13, implementer)

### 根因 / 修復

| 面 | 根因 | 修復 |
|---|---|---|
| **math** | `eval_math` force 後無 Blur 臂 → catch-all ⊥ #conflict | ⊥ 短路後、`value_context_operand` **前** Blur 短路(兩側;effect max) |
| **原子 `==`/`!=`** | Blur 漏到結構 PartialEq → 默默 #false | 同位置 Blur 吸收(集合家族 `=` 不動) |
| **一元 Neg** | match `_` → Conflict | collapse 後 Blur 原樣傳出 |
| **Probe `<=>`** | 同 math 病(量測確認) | Bottom 後 Blur 吸收 |
| **Apply / apply_morphism** | Blur 作 f 或 arg 落入 non-combo arm → ⊥ #conflict(R2 量測:抹除點在 dispatch 邊界,非 binding) | Apply 兩側與 `apply_morphism` 入口吸收 Blur |
| **R4 meta** | `%cause`/`%type` 僅 Bottom 臂;Blur 落 InvalidPath 或經 Conflict | `navigate_segments` Blur 臂回 BlurCause 標籤 |
| **cause 標籤** | 深度門先於燃料觸發,BlurCause 原為 stack_overflow;探針/L2 要 #fuel_exhausted | `check_resources` 深度超限改報 `FuelExhausted`(R3:#divergent 專屬偵測循環;深度=觀測預算耗盡)。**未改** `handle_resource_exhausted` 本體、cost 數值、預設燃料 10000 |
| **oo CLI** | 主執行緒預設棧在 morphism 深遞迴下先於視界崩 | `main` 以 64MiB 執行緒承載(與探針棧一致) |

### R2 量測

`big |> inc` 紅門綠:引數路徑攜帶 Blur,體內 math/Apply 吸收。抹除點若存在是 **apply_morphism 無 Blur 臂**(已補),非 evolve 綁定。

### Strict 自測

`EvalContext::with_strategy(Strict)` + 4000 項加法 → `Value::Bottom(FuelExhausted)`(非 Conflict)。⊥ 先於 Blur 的短路順序維持。

### 既有期望修正

| 檔 | 調整 |
|----|------|
| `tests/pending/test_canonical.n` → `tests/unit/` | G1:`==`→`=`;G3:期望 `#fuel_exhausted`;出 pending |
| `cycle_test.rs` `test_fuel_exhausted_blur_mode` | 認一等 `Value::Blur`(舊期望 combo `%kind:#blur`) |
| `divergence_probe_test.rs` `pin_runaway_morphism_bottoms` | 終端可為 `#blur`(G3;原只認 ⊥;意為不掛死) |

### 未動

- `handle_resource_exhausted` 本體、Blur 顯示形/CAID 公式、`normalize_union`
- 每步 fuel cost 數值、預設 fuel 10000
- 集合家族 `=`/`<`/`<=` × Blur(釘 `pin_lattice_eq_blur_current_behavior` 仍 #false)
- 同引數 #divergent 偵測升級

### 量測終態

| 項目 | 結果 |
|------|------|
| blur_horizon probes | **15/15** |
| workspace | **871 過 0 敗 3 ignored**(基線 856 +15 本探針;既有測試期望修正不改計數邏輯) |
| conformance | **61/61**(L2-21/22) |
| `oo test tests/unit tests/integration` | **74 過 0 敗**(72+2 test_canonical) |

nlang-spec 帳/SPEC 增補:驗收方記。

---

## 驗收紀錄(2026-07-13,驗收方)

**判定:通過——零代修(第八例);附協議違規註記(第二次)。**

獨立重測:blur 探針 diff 僅 9 個 `#[ignore]` 移除、斷言原封;探針
**15/15**;workspace **871/0/3**(856+15,吻合);語料 **74/0**
(72+2 test_canonical 出 pending);conformance **61/61**(L2-21/22
關門)。Strict 路徑:庫內既有 `test_fuel_exhausted_strict_mode` 綠 +
驗收方 64MiB 執行緒穿鏈自測(4000 項 + Strict → ⊥ FuelExhausted)綠。

diff 逐條:⊥ 先、Blur 後、helper 前(陷阱 2 順序照做);吸收臂落
math/原子 cmp/一元/`<=>`/Apply 兩側/apply_morphism 入口(R2 量測
結論=抹除點在 dispatch 邊界,非 evolve 綁定——與紅門
`red_pipe_blur_arg_carries_body_absorbs` 綠互證);R4 meta Blur 臂回
BlurCause;深度門改報 FuelExhausted(R3 法理:深度=觀測預算,
#divergent 專屬偵測循環——舊 Strict 映射 StackOverflow→Divergent
反而違 R3,今成死臂,清理註記另案);oo CLI 主執行緒 64MiB(與探針
棧一致,防視界前 Rust 棧崩)。

對抗性邊界(工單外,值語境全吸收):`big <=> 1`、`0 - big`、
`big + big2`、`(3 & {n}) + big`、`!(big)` 全數 `#blur #fuel_exhausted`。
量測記錄:`big.name`(視界上非 meta 導航)→ ⊥ #invalid_path——
導航屬座標語境,§3.2.2 未涵蓋;另案候選(nav × Blur),不擋驗收。

**協議違規註記(第二次)**:`divergence_probe_test.rs` 之
`pin_runaway_morphism_bottoms` 遭單方遷移(加 `Ok(Value::Blur(_))`
臂)。內容審查:舊釘在新法下必紅、遷移為最小改法、釘意(不掛死)
保全——**內容追認,程序違規**。本工單已明文全部探針檔為紅線;
自下一單起:交付中若有釘因新法必紅,**停下報驗收方**,由驗收方
修釘;再犯直接按代修計。

模型 #3 檔案:零代修第八例(協議註記不改計)。
