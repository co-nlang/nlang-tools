# 工單:oo test 判定收緊(空洞真禁止,SPEC_16 §2.2 裁定 B)

**開單**:2026-07-20(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。

## 1. 法源(裁定 B,2026-07-20)

**SPEC_16 §2.2 改寫**:測試即觀測,通過=「這次觀測決定了一個
定形事實」。**Pass**=收斂為定形值(含 #true;定形非布林=合法
冒煙斷言)。**Fail**=`_|_`(報 %cause)/`#false`/`#fail`(駁斥)
/`_` Top/TopCaused(**未定**——空洞真禁止)/`#blur`(視界內
未定,報 blur %cause)。

## 2. 病灶(v0.2.28 量測)

runner 照舊法字面「非 ⊥ 即過」:`(_) == 5` → `_` → PASS 空洞;
runaway blur → PASS。實案兩枚均已治(effect_taint=元讀弧真綠;
test_canonical `.%type` 退役拼法=本弧開單遷移 `.%cause` 真綠)。
語料普查:除上述外 75 檔無其他非定形 test。

## 3. 修法方向與位點

- 位點=`crates/oo/src/main.rs` test 子命令觀測結果 match
  (~L497):現有臂 Bottom→FAIL、Tag false/fail→FAIL、`_`→PASS。
  增兩臂於 catch-all 前:
  - `Value::Top | Value::TopCaused{..}` → FAIL,訊息含
    undetermined 類字樣+欄名;
  - `Value::Blur(d)` → FAIL,訊息含 blur %cause(如
    fuel_exhausted)+欄名。
- exit code 機構照舊(failed>0 → exit 1)。
- **不動**:`--static-only` 臂(設計即不觀測)、測試發現規則
  (`test_`/`~%test` 前綴)、Summary 格式、語料內容(開單遷移
  已由驗收方完成)、interpreter crate(本弧純 oo runner)。

## 4. 門(紅)與釘

**已預提交+校準**(3 紅全紅正因〔空洞地過著〕、5 釘全綠)。

- `crates/oo/tests/test_verdict_probe_test.rs`(新檔,CLI 整合
  =CARGO_BIN_EXE_oo):紅=Top 空洞面 `(_)==5`(exit≠0+FAIL 帶
  欄名)/未定比較 `q.%nonsense == #io`/blur runaway(FAIL 含
  fuel_exhausted)。釘=#true 過/定形 combo 冒煙過/#false 失/
  ⊥ 失帶因/遷移後 `.%cause` 拼法真綠。
- 無 conformance 向量(CLI harness 面),矩陣 123 不動。

交付=移除全部 3 個 `#[ignore]`,探針檔**其餘一字不改**(修改權
在驗收方)。全 workspace 一顆不得翻紅;語料非 pending 不退。

## 5. 目標與交付紀錄

**目標**(基線實測 2026-07-20,先量後寫):探針 8/8;workspace
**1267/0/3**(基線 1264/0/6);conformance **123/123** 不退;
語料非 pending **75/0** 不退(含開單遷移後 test_canonical)。

**交付紀錄**(交付方填;先寫再回報):

- [ ] 交付 commit(s):
- [ ] 根因與修法(match 臂位置與訊息形制寫明):
- [ ] 探針/workspace/conformance/語料 四數:
- [ ] 申報事項(範圍外接觸、歧異記錄):

## 6. 驗收紀錄(驗收方)
