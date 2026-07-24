# 工單:~%Effect./runPure + 特權(SPEC_08 §4.3 / §6)—— 效應系統波 arc 4

**開單**:2026-07-24(驗收方)。**基線**:dev @ 本工單 commit(v0.2.36 後)。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。探針/向量**修改權在
驗收方**——交付僅移除探針 `#[ignore]`,一字不改其餘。

## 1. 法源(裁定 P1,2026-07-24,使用者)

SPEC_08 §4.3 `~%Effect./runPure` + §6 特權模式。**裁定 P1(可信通道能力位)**:
特權 = **horizon 上的能力**(`EvalContext.privileged`),**只能經可信程式外通道
設定**(CLI `oo run --privileged` / init)。**程式內無法自授權**(§6.1.2 嚴禁
隱式/無令牌後門)。token 字串的鑄造/生命週期屬 **REAL_02**(協議層),非語言
——語言只見布林能力。

`runPure <node>`(= §6.2 `#effect_override` 特權操作):
- **具特權**:強制執行該節點(force),將結果 `%effect` 固化為 `#pure`
  (io 已由外部代理 → 合法純資料)。
- **無特權**:坍縮 `_|_ (%cause: #privileged_required)`(§6.1.2 無後門)。
- **觀測投影**:原節點 CAID **不變**(§4.3/§6.2:特權改收斂過程,非幾何指紋)。

## 2. 修法(建議)

**(A) 特權能力位**:
- `Ouroboros.privileged: bool`(新欄,預設 false);建構後由可信通道設定
  (`set_privileged`/pub 欄)。
- `EvalContext.privileged: bool`(新欄);於 `eval_context()`(lib.rs:965)
  設 `ctx.privileged = self.privileged`;**經 `clone`/`sub_context` 自然繼承**
  (plain 欄,無需特判——同其他 ctx 純量欄)。

**(B) `BottomCause::PrivilegedRequired`**(value.rs enum,**append-only 尾**):
`as_tag` → `"privileged_required"`。

**(C) `~%Effect` 系統物件 + `/runPure`**(lib.rs 註冊,仿 `~%Discovery`
~line 764-766;closed cocoon,EffectTag::Pure):
- builtin `effect.run_pure` `|arg, oo, ctx|`:
  - `if ctx.privileged { let forced = oo.force_recursive(arg, ctx); forced.purify_effects() }`
  - `else { Value::Bottom(EffectViolation… 不,PrivilegedRequired) }`
    →`_|_ %cause #privileged_required`。

**(D) `Value::purify_effects()`**(value.rs,**鏡像 arc-2 `solidify_effects`**
但活動→**Pure**):遞迴 Atom/Combo(全軸)/Union/Blur/Range/Thunk,
`purify_active(e)= if e.has_active() { Pure } else { e }`(`#cached` 亦→? 見下)。
- **邊界**:runPure 討的是**活動副作用**(io/nondet/state)→ pure。`#cached`
  是已固化歷史,runPure 之 force 後一般不出現;若出現,保持不動(非活動)。
  即 `purify_active` 只清活動位,同 `has_active()` 判斷。

**(E) CLI `--privileged` 旗標**(oo/main.rs):
- `Run { …, #[arg(long)] privileged: bool }`(及 `Eval` 若要);
  `run_one_shot`/`run_eval` 收旗標 → 建 engine 後 `engine.set_privileged(true)`。
- **只此可信通道設定**;程式內 `~%…` 無設定口(P1 安全核心)。

## 3. CAID / 範圍柵欄

- **CAID 不動**:runPure = 取回/force 值上的效應投影,**不寫回、不改原節點
  CAID**(§4.3/§6.2)。未觸 bn_serial/to_serial_byte/content_hash。genesis 綠。
- **做**:privileged runPure discharge(force+purify)、unprivileged → ⊥
  #privileged_required、CLI `--privileged` 可信通道、能力位繼承。
- **不做(掛帳後續弧)**:`#pin` + 其餘 §6 特權操作(#commit/#rollback/#squash
  ——**同能力位基建**,各自另弧)、commit 層審計透明度(§6.1.3 非-⊥ 結果的
  `#privileged_*` 標記——本弧 discharge 結果依設計「可視為純」,審計歸 commit
  層 REAL_02)、token 字串驗證(REAL_02)、實體線程隔離(§6.1.1,REAL_02)、
  `#ext:`(§4.1)、CAID 全集參與(§4.1)。

## 4. 風險與紅線

- **安全核心(P1)**:privileged **只**由 `--privileged`/init 設定;**嚴禁**任何
  程式內(`~%Config` 寫入、欄位、態射)設定特權的路徑(§6.1.2 後門)。
  交付須確認無此路徑。
- **不動**:`.%effect` 讀取臂、arc-1 union、arc-2 solidify、arc-3 守護、
  ⊥/blur 白名單、EffectTag 型別。runPure 為**新增**系統態射 + 能力閘。
- 一般 io(未經 runPure)在特權 run 中**仍 #io**(特權非全域淨化;釘覆蓋)。

## 5. 門(紅)與釘 + 目標(先量後寫,基線實測 2026-07-24)

**探針(二檔,已預提交+校準)**:
- **A. `crates/interpreter/tests/effect_runpure_probe_test.rs`**(預設=非特權 harness):
  - **3 紅**(`#[ignore]`):`red_runpure_blocked_unprivileged`(got `_`→⊥
    #privileged_required)、`_read_propagates_bottom`(got `#pure`→⊥)、
    `_pure_arg_blocked`(純參亦擋,got `_`→⊥)。
  - **3 釘**:一般 io 照流 #io、多活動 #io|#nondet、⊥ 白名單。
- **B. `crates/oo/tests/runpure_cli_probe_test.rs`**(CLI 可信通道):
  - **4 紅**(`#[ignore]`):`cli_runpure_privileged_discharges`(`--privileged`
    → `#pure`)、`_privileged_clean_value`(discharge 值=裸整數無尾註)、
    `_no_flag_blocked`(無旗標 → privileged_required)、`_privileged_plain_io_is_opt_in`
    (`--privileged` 但純 io 未 runPure → 仍 #io)。
  - **1 釘**:一般 run(無旗標)io 照流 #io。

**交付 = 移除全部 7 個 `#[ignore]`**(A×3 + B×4),探針其餘一字不改。

**目標**(基線 → 交付後):
- 探針 A **6/6**、B **5/5**。
- workspace **1369/0/3**(基線 with-probes 1362/0/10)。
- conformance **142/142**(基線 141/142;L2-103=非特權 runPure→⊥
  #privileged_required 現紅 got `_`)。**特權路徑無合規向量**:runner 不傳
  `--privileged`(無狀態契約),CLI 探針為特權路徑法定測具。
- 語料非 pending 不退。

## 6. 交付紀錄(交付方填;先寫再回報)

- [x] 交付 commit(s): `5b35303` effect_runpure arc 4
- [x] 能力位(Ouroboros/EvalContext.privileged + eval_context 設定 + 繼承)落點:
  - `Ouroboros.privileged: bool` 預設 false;`set_privileged` 僅 pub 可信口。
  - `EvalContext.privileged`;`eval_context` / `universe.evolve` / `universe.observe`
    從 engine 拷入;sub_context clone 自然繼承。
- [x] `~%Effect./runPure` 註冊 + builtin(force+purify / ⊥)落點:
  - `lib.rs` root_with_system 註冊 `~%Effect` closed + `/runPure` → `effect.run_pure`。
  - `engine.rs` builtin: 特權 → force_recursive + purify_effects;否則
    ⊥ #privileged_required。
- [x] `purify_effects()` + `BottomCause::PrivilegedRequired` 落點:
  - `value.rs` 鏡像 solidify(活動→Pure;cached 不動);enum 尾
    PrivilegedRequired + as_tag/primary_rank。
- [x] CLI `--privileged` 旗標接線(run_one_shot/run_eval → set_privileged):
  - `oo run --privileged` / `oo eval --privileged`。
- [x] **安全確認**:無程式內自授權路徑(§6.1.2):
  - 全樹 `set_privileged` 僅 main.rs CLI 與引擎 API;無 n/ 欄位/`~%Config`
    寫入特權。
- [x] 探針 A 6/6 · B 5/5 / workspace / conformance / 語料 四數:
  - A effect_runpure **6/6**;B runpure_cli **5/5**
  - workspace **1369/0/3**
  - conformance **142/142**(L2-103 翻綠)
  - 語料 **75/0**(68+7)
- [x] 申報事項(範圍外接觸、CAID、其他):
  - CAID/bn_serial 未動;runPure 為觀測投影。
  - #pin/commit 層審計/token 字串/線程隔離/#ext 未做(掛帳)。

## 7. 驗收紀錄(驗收方填)
