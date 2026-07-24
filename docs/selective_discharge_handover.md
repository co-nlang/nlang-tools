# 工單:選擇性 discharge —— 特權能力格(SPEC_08 §4.3 / §6.2)

**開單**:2026-07-25(驗收方)。**基線**:dev @ 本工單 commit(v0.2.37 後)。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §6 再回報**。探針**修改權在驗收方**
——交付僅移除探針 `#[ignore]`,**一字不改其餘**。

## 1. 法源(裁定 Q1 + Q2,2026-07-25,使用者)

### 裁定 Q1(兩軸複合)

特權能力**不是布林,是結構化值**。欄位 = SPEC_08 §6.2 的**五個特權操作**;
其中 `effect_override` 攜帶一個**效應標籤集**(此 horizon 可 discharge 哪些活動
標籤)。其餘四項(`pin`/`commit`/`rollback`/`squash`)為**已宣告但惰性的空槽**
——接受並儲存,但目前無操作消費(各自的弧日後填槽)。

- **軸一** = 哪個特權操作。**軸二** = `#effect_override` 內可 discharge 哪些標籤。
- 理由:只建軸二,`#pin` 落地時能力位形狀須重設計一次。

### 裁定 Q2(全有全無)

能力集 `C`、值 force 後的**活動**效應 `E`:

- `C ⊇ E` → discharge,結果 `%effect` 固化 `#pure`。
- `E ⊄ C` → `_|_ (%cause: #privileged_required)`。**不做部分 discharge**
  ——名為 `runPure` 的態射**絕不得回傳非純值**(命名誠實;延續 arc-3
  「說謊即崩潰」的紀律)。

### 續承 arc-4 不變量

**閘是能力,不是參數**:未獲 `effect_override` 授權時,**連純參數也拒**
(discharge 是一個特權**操作**)。此為 arc-4 既有行為,不得回歸。

## 2. 修法(建議)

**(A) `Privilege` 結構**(interpreter;`Copy`,以維持 ctx 繼承零成本):

```rust
#[derive(Clone, Copy, Debug, Default)]
pub struct Privilege {
    /// §6.2 #effect_override:None = 未授權此操作(連純參亦拒);
    /// Some(tags) = 得 discharge 恰好這些活動標籤。
    pub effect_override: Option<EffectTag>,
    /// §6.2 已宣告但惰性的槽(目前無操作消費)。
    pub pin: bool,
    pub commit: bool,
    pub rollback: bool,
    pub squash: bool,
}
```

- `Privilege::NONE`(全拒,`Default`)/ `Privilege::all()`(全授,
  `effect_override: Some(IO|NonDet|State)` + 四槽 true)。
- `union(self, other)`:逐欄 join(`effect_override` 取 `Option` 的
  union——`None ∪ Some(x) = Some(x)`;布林取 `||`)。**重複 `--grant` 靠它累加**。
- `may_discharge(&self, e: EffectTag) -> bool`:
  `match self.effect_override { None => false, Some(c) => c.contains_all(e.active_part()) }`。
- **新增 `EffectTag::active_part()`**:遮罩至 `IO|NonDet|State`。
  **要害**:覆蓋判定只看**活動**位——`#cached` 不是 discharge 對象
  (arc-2:它已是固化歷史),不得因值帶 `#cached` 而誤拒。

**(B) 能力位接線**(取代 `privileged: bool`,共 9 點,見 §5 量測):

- `Ouroboros.privilege: Privilege`(預設 `NONE`)。
- `EvalContext.privilege: Privilege`;`eval_context()`(lib.rs:1049)、
  `universe.evolve`(:174)、`universe.observe`(:392)由 engine 拷入;
  `sub_context` clone 自然繼承(`Copy` 純量欄,同現行)。
- **`set_privileged(bool)` 保留為相容墊片**(`true → Privilege::all()`、
  `false → NONE`),另加 `set_privilege(Privilege)`。舊 API 不得破壞。

**(C) `effect.run_pure`**(builtins/engine.rs:66-)——**求值順序是要害**:

1. **軸一閘,force 之前**:`ctx.privilege.effect_override.is_none()` →
   立即 `⊥ #privileged_required`,**不 force**。(精確保住 arc-4:無授權時
   純參亦拒、且不觸發任何副作用。)
2. `let forced = oo.force_recursive(v, ctx);`
3. **軸二覆蓋判定,用 force 後的實際效應**:
   `if !ctx.privilege.may_discharge(forced_effect) → ⊥ #privileged_required`
   (訊息**須列出缺哪些標籤**,例:`runPure: horizon may discharge #io but
   the value observes #io | #nondet`)。
4. 否則 `forced.purify_effects()`。

> **為何軸二不用 `predict_effect` 預檢**:predict 是**過度近似**,會造成
> 偽拒(合法呼叫被擋)。而「force 後才拒」不外洩任何能力——未授權程式
> **本來就能**直接跑 `~%Time.now _`(照樣 #io),故此處 force 不授予新權力。
> 兩相權衡取**實際效應**判定。

**(D) CLI `--grant`**(oo/main.rs;`Run` 與 `Eval` 皆加):

```
--privileged          既有布林 = 全授(v0.2.37 契約,不得改)
--grant <SPEC>        可重複;各筆以 union 累加
  SPEC ::= effect_override[:<tag>[+<tag>]*]   裸寫 = 全部活動標籤
         | pin | commit | rollback | squash    (惰性槽)
  <tag> ::= io | nondet | state
```

- `--privileged` 與 `--grant` 併用 = union。
- **未知 SPEC → 大聲死**(CLI 錯誤,非靜默忽略;同 `~%Config` 封閉旋鈕家
  教訓)。錯誤訊息**須含該 SPEC 字串**與 `grant` 字樣(探針釘住,防止與
  clap 的 unknown-flag 錯誤混淆)。
- 未知 `<tag>` 同樣大聲死。

**(E) `predict_effect` 縫**:**不動**。arc-4 代修(callee 為
`~%Effect./runPure` → predict 回 `Pure`)在本弧仍雙面皆真:獲授權→純值、
未授權→⊥(亦純效應)。釘 `pin_guard_runpure_seam_no_false_violation` 守。

## 3. CAID / 範圍柵欄

- **CAID 不動**:能力屬 horizon,**不可觀測、不入 CAID、不寫回節點**
  (同 arc-4)。未觸 `bn_serial`/`to_serial_byte`/`content_hash`。genesis 須綠。
- **做**:`Privilege` 結構 + union + `may_discharge`、`active_part()`、
  兩軸閘、CLI `--grant`(run+eval)、能力繼承、相容墊片。
- **不做(掛帳)**:`#pin`/`#commit`/`#rollback`/`#squash` **操作本身**
  (本弧只填能力槽);能力位對程式可觀測(維持不可觀測——可讀會構成
  horizon 依賴通道);token 字串驗證與線程隔離(REAL_02 §6.1.1/6.1.2);
  commit 層審計標記(§6.1.3);`#ext:`(已降級結案);
  `~%Engine./set_strategy` 直呼參數包裝疣(既有,與本弧無關)。
- **spec 編輯由驗收方於結案時進行**——交付方**不改 spec**。

## 4. 風險與紅線

- **安全核心(P1,最高優先)**:能力**只**由 `--privileged`/`--grant`/init
  設定。**嚴禁**任何程式內(`~%Config` 寫入、欄位、態射、`~%Effect` 自身)
  設定或提升能力的路徑(§6.1.2 無後門)。交付**須逐點確認並在 §6 申報**。
- **不得回歸**:arc-1 union、arc-2 solidify、arc-3 守護、arc-4 runPure 語義、
  `.%effect` 讀取臂、⊥/blur 白名單、`EffectTag` 的 CAID 序列化。
- **`--privileged` 語義不得改**:仍是「全授」,既有腳本零影響。
- **`#cached` 不參與覆蓋判定**(只看 active_part);漏此將誤拒 arc-2 的
  取回值。

## 5. 門(紅)與釘 + 目標(先量後寫;基線實測 2026-07-25)

**探針(一檔,已預提交+校準)**:
`crates/oo/tests/selective_discharge_probe_test.rs`

CLI 為法定測具——P1 說能力在程式內無法建立,故可信通道即 CLI(同 arc-4)。

- **9 紅**(`#[ignore]`):
  - 軸二:`red_grant_io_discharges_io`、`_io_refuses_nondet`、
    **`_io_refuses_mixed_partial_coverage`(Q2 承重案)**、
    `_both_discharges_mixed`、`_accumulates_by_repetition`、
    `_bare_effect_override_covers_all_active`
  - 軸一:**`red_pin_grant_does_not_authorize_effect_override`(Q1 承重案)**、
    `red_inert_slot_is_accepted_and_harmless`
  - 護欄:`red_unknown_grant_is_a_loud_error`
- **6 釘**(基線已綠,須續綠):`pin_bare_privileged_still_grants_all`、
  `_pure_arg_returns_value`、`pin_no_capability_refuses_even_pure_arg`、
  `pin_no_flag_plain_io_flows`、`pin_privileged_plain_io_is_opt_in`、
  `pin_guard_runpure_seam_no_false_violation`。

**基線校準已驗**:9 紅全數因 `--grant` 為未知旗標而失敗(clap
`unexpected argument '--grant' found`),**無一空洞通過**;6 釘全綠。

**交付 = 移除全部 9 個 `#[ignore]`**,探針其餘一字不改。

**目標**(基線 → 交付後):

| 項 | 基線 | 目標 |
| :--- | :--- | :--- |
| 本探針 | 6/6(9 ignored) | **15/15** |
| workspace | 1376/0/12 | **1385/0/3** |
| conformance | 142/142 | **142/142(不變)** |
| 語料(workspace 內) | unit corpus 27 parsed / 0 skipped | 不變 |

**本弧無新增合規向量**:runner 不傳 `--privileged`/`--grant`(無狀態契約),
能力路徑全部僅 CLI 可達——CLI 探針即法定測具(同 arc-4 先例)。

## 6. 交付紀錄(交付方填;先寫再回報)

- [x] 交付 commit(s): `cea0eda` selective_discharge
- [x] `Privilege` 結構 + `union` + `may_discharge` + `EffectTag::active_part()` 落點:
  - `value.rs`: `Privilege { effect_override, pin, commit, rollback, squash }`;
    `NONE`/`all()`/`union`/`may_discharge`;`EffectTag::active_part`/`all_active`。
- [x] 能力位接線(Ouroboros/EvalContext + eval_context/evolve/observe + 相容墊片)落點:
  - `Ouroboros.privilege` / `EvalContext.privilege`(取代 bool);
    `set_privilege` / `grant_privilege` / `set_privileged` 墊片(true→all)。
  - 拷入:eval_context、universe.evolve、universe.observe。
- [x] `effect.run_pure` 兩軸閘(**軸一在 force 前**、軸二用實際效應)落點:
  - 軸一:`effect_override.is_none()` → ⊥,不 force。
  - 軸二:`!may_discharge(forced.effect())` → ⊥,訊息列 may vs observes。
  - 否則 `purify_effects()`。
- [x] CLI `--grant`(run+eval;union 累加;未知 SPEC/tag 大聲死)落點:
  - `parse_grant_spec` + `apply_cli_privilege`;未知 SPEC/tag 含字串與 `grant`。
- [x] **安全確認**:全樹 `set_privilege`/`set_privileged` 呼叫點列舉,確認
      **無程式內自授權路徑**(§6.1.2):
  - 呼叫僅 `oo/main.rs` CLI 與 `lib.rs` API 定義;`grant_privilege` 同。
  - 無 n/ 欄位/`~%Config`/態射可寫能力位。
- [x] 四數:本探針 **15/15** · workspace **1385/0/3** · conformance **142/142** ·
      語料 **75/0**(68+7)
- [x] 申報事項(範圍外接觸、CAID、其他):
  - CAID/bn_serial 未動;能力僅 horizon。
  - pin/commit/rollback/squash 操作本體未實作(僅槽位)。

## 7. 驗收紀錄(驗收方填)

## 8. 意見

本弧把 arc-4 的 P1 布林能力位升為**能力格**,並一次把 §6.2 五個操作的槽位
定形——`#pin`/`#commit`/`#rollback`/`#squash` 各弧日後只需填槽,不必再動
能力位形狀。能力本身是格值(精煉 = 收緊權限、meet = 權限交集),與 n/ 自身
的語法同構。
