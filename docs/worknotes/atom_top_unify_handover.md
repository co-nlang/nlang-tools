# 工單：`Atom(Top)` 不走 unify 的 Top arm——格律「Top＝么元」被破

發出：2026-07-08。獨立小刀任務,可派任何 agent;驗收方可能為另一模型(Opus),
驗收章節寫在最後,自足。

## 症狀(全部已實測,最小重現＝預置探針)

字面量 `_` 求值為 `Value::Atom(AtomKind::Top, …)`,而 unify 的么元臂只匹配
`Value::Top` **變體**。於是:

1. `r: _ & 5` → `_|_ Conflict("Incompatible types: Atom(Top) vs Atom(Int(5))")`
   ——應為 `5`(Top 是 meet 么元)。
2. `engine.unify(Atom(Top), 5)` 兩個方向都 Conflict。
3. **演化面(最傷)**:`t: { flag: _ }` 之後 refine `t: { flag: 2 }` → evolve 整個
   `Err(Conflict)`——「先宣告座標為 Top、後續精煉」是最正典的單調演化,現在做不到。

## 根因錨點(已讀碼定位,勿重查)

- `eval.rs` `ExprKind::Atom(kind) => Value::Atom(kind.clone(), …)`——`_` 就地變
  `Atom(AtomKind::Top)`;`resolve_path` 的 `name == "_"` 臂(lib.rs ~916)同樣
  回 `Atom(AtomKind::Top)`。
- `unify.rs` `unify_internal` 的么元臂(~137–144)與 `do_unify` 全部只 match
  `Value::Top`;`Atom(AtomKind::Top)` 落到 atom-mismatch 臂 → Conflict。
- 對照:`Bottom` 也有同樣的雙拼寫(`Value::Bottom` vs `Atom(AtomKind::Bottom)`),
  多處測試同時 match 兩種——**雙拼寫是系統性的**,本工單只修 Top 的 unify 語義,
  不做全面正規化(見非目標)。

## 修法(二選一,預裁決傾向 A)

- **A(建議)**:求值端正規化——`ExprKind::Atom(AtomKind::Top)` → `Value::Top`
  (eval 與 resolve_path 的 `_` 臂兩處)。單一來源,下游全部自動正確。
  **風險**:有代碼依賴 `Atom(Top)` 形態(序列化、模式匹配、`to_nlang` 回印)。
  全語料+全套件是安全網;若有測試依賴 `Atom(Top)` 形態,逐一判定「合法差異 vs
  回歸」記回函,**不改期望值遷就**。
- **B(保守)**:unify 端加別名臂——在 `unify_internal` 早退區與 `do_unify` 各加
  `Atom(AtomKind::Top)` 等同 `Value::Top` 的判定。侵入小,但雙拼寫繼續存在,
  每個新的值判斷點都要記得兩種形態(已經漏過一次的模式)。
- 若 A 過程中發現 `Value::Bottom` vs `Atom(Bottom)` 的同款問題:**記回函,不擴
  修**(那是另一張工單的事)。

## 驗收(探針已預置,`1e12872`)

`crates/interpreter/tests/atom_top_unify_probe_test.rs` 三支,現 `#[ignore]`,
基線=以 atom-mismatch Conflict 失敗(2026-07-08 校準)。

**驗收 ＝ 拿掉三個 `#[ignore]` 後全綠 ＋ 全 workspace 綠(基線 609)＋全語料綠。**
刪除或弱化探針=違反工單。

## 非目標

- Top/Bottom 雙拼寫的全面正規化(系統性工程,另案)。
- `collapse()`/效應傳播/序列化格式的任何變更。
- 順手重構 unify(memo 語義剛在 Stage 4/5 定案,見 lazy_engine_handover 驗收記錄
  ——`memo_enabled`/有效綁定/deps 機制**一字不動**)。

## 給驗收方(如非本 session 的模型,先讀這段)

1. 驗收慣例:**不信回函,重跑量測**。全 workspace:
   `cargo test --workspace`(基線 609 過 0 敗,本工單後應 612)。
2. 檢查交付**沒有動**:`unify.rs` 的 Stage 4/5 memo 區(`memo_enabled`、
   `(Top,Thunk)/(Top,Ref)` 保留臂、`staged_ok`)、`force()` 的 memo hook、
   `stage4_redline_test.rs`/`stage5_redline_test.rs`/
   `stage5_acceptance_probe_test.rs` 全部原樣綠。
3. 若走修法 A,抽查 `to_nlang` 回印與 bn_serial roundtrip 對 `_` 的行為
   (`Value::Top` 的序列化臂已存在;確認 CAID 穩定——`_` 的新舊求值形態
   CAID **會**不同,屬合法差異,但需回函記錄)。
4. 記錄:ENGINE_SYNC 補列、ROADMAP queue 該行銷帳——**提交必須非空**
   (`git show --numstat` 自查;本專案已發生兩次空提交事故,見
   lazy_engine_handover Stage 4/5 驗收記錄)。
5. 本專案驗收紀律:「根因」二字必須附量測;宣稱附數字;偏離預裁決升級不自改。
