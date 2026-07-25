# 工單:`#pin` —— 特權覆寫(SPEC_08 §6.2 第一個操作本體)

**開單**:2026-07-26(驗收方)。**基線**:dev @ 本工單 commit(v0.2.39 後)。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §6 再回報**。探針**修改權在驗收方**
——交付僅移除探針 `#[ignore]`,**一字不改其餘**。

## 1. 法源與定位

SPEC_08 §6.2:`#pin` = 「直接覆寫節點值,跳過收斂邏輯」,審計標籤
`#privileged_pin`。**裁定(2026-07-26,使用者):本弧只做 `#pin`**,
`#commit`/`#rollback`/`#squash` 另弧。

**分野(開弧理由)**:`#pin` 是 §6.2 五個操作中**唯一動到格本體**的;其餘三個
只移動歷史鏈。演化是**單調**的——根座標一旦綁定,不相容的重綁在 evolve 邊界
被拒(`universe.rs` G2-S 檢查)。`#pin` 是那條單調性的**特權且受審計的例外**,
即 discussion/021 所謂**「被檢疫的一劑 n/^op」**:移除約束是上箭頭移動,故需
外部作功(能力),恰如逆轉熵需作功。

**能力槽早已備妥**:`Privilege.pin`(選擇性 discharge 弧宣告為惰性槽)。本弧
**啟用**它——那一弧「一次定形、日後填槽」的設計於此兌現。

## 2. 表面(已量測,非臆測)

- **`oo run` 不適用**:`run_one_shot` 明文為「one-shot: pure universe, no local
  staged load」(main.rs),**永遠看不到已提交狀態**,故根衝突在該處根本不會
  發生。
- **`oo evolve` 才是持久宇宙命令**(它呼叫 `load_universe`),且**軸別正確**:
  SPEC_00 §1.2 把「改變宇宙」放在**演化軸**,不在觀測軸。
  ⟹ **`#pin` 掛在 `oo evolve`**。
- **兩步式,同 runPure**:`--grant pin` **授權**(P1 可信通道),`--pin`
  **請求**。**能力本身不得**讓演化悄悄變成非單調——必須顯式請求。

## 3. 修法(建議)

**(A) CLI**(`oo/src/main.rs`):`Evolve` 子命令加
`#[arg(long)] pin: bool` 與 `--grant <SPEC>`(可重複,同 run/eval 之
`apply_cli_privilege`,**復用既有解析,不得另寫一份**)。

**(B) 閘**:`--pin` 且 `ctx/engine` 具 `Privilege.pin` → 進入 pin 模式;
`--pin` 但無能力 → **大聲拒**(`#privileged_required` 字樣,非靜默降級為
普通 evolve);無 `--pin` → 現行單調路徑,**一字不動**。

**(C) 語義**(`universe.rs` evolve):pin 模式下——
1. **跳過 G2-S 根檢查**(~line 290-302 的 `for c in &evolved_coords { … unify(root_val, val) → Err }`)。
2. **staged 併入須為「取代」而非「meet」**:否則 staged 既有的不相容值會再度
   ⊥(或更糟:靜默 meet 成更窄的值)。pin 的語義是覆寫,不是收斂。
3. 其餘欄位/座標**不受影響**(pin 只作用於本次 evolve 的欄位)。

**(D) 審計 —— 落點由規格推導,不是選擇**:
SPEC_08 §6.2 語義保證明文「特權操作改變的是**收斂過程**,而非**幾何指紋**」,
節點 `%id` 仍依其**最終物理結構**計算 ⟹ **審計記錄不得存在於值之內**
(否則 CAID 位移)⟹ **只能落在 Commit 層**。
- 建議:`CommitKind` 既有 enum(`Refine`/`Standard`,且 `Standard` 帶
  `#[serde(other)]`)加一支 `Pin`——**舊 commit 反序列化不受影響**。
- `oo log` 須顯示該標記(目前 `engine.log()` 只回 `(hash, meta)`,需一併帶出)。
- **禁止**:把 `%cause`/`privileged` 之類欄位寫進被 pin 的值。

## 4. 紅線

- **CAID**:被 pin 的值其內容雜湊**必須**與同值正常寫入者相同(§6.2)。
  未觸 `bn_serial`/`to_serial_byte`/`content_hash`;genesis 須綠。
- **舊 commit 相容**:既有 commit 物件的雜湊與反序列化**不得**改變。
- **不得改**:無 `--pin` 時的 evolve 路徑(單調性是預設,pin 是例外);
  `Privilege` 結構與 `--grant` 解析語義(復用,不新寫);效應系統四弧;
  `%rules`/pattern-dispatch/curry(上一弧 A/B 已釘)。
- **能力不得擴權**:`--grant pin` **不得**授權 `#effect_override` 或其他操作。

## 5. 門(紅)與釘 + 目標(先量後寫,基線實測 2026-07-26)

**探針(一檔,已預提交+校準)**:`crates/oo/tests/pin_probe_test.rs`
(多步 CLI:`evolve` → `commit` → `evolve --pin` → `commit` → `log`/`status`)

- **7 紅**(`#[ignore]`):
  - `red_pin_overwrites_a_committed_binding`(本體)
  - `red_pin_requires_the_capability`(P1:請求無授權 → 大聲拒)
  - `red_pin_capability_is_operation_specific`(軸一:`effect_override` 不得授權 pin)
  - `red_capability_alone_does_not_pin`(**兩步式另一半**:有能力但未請求 → 仍單調)
  - **`red_pin_is_audited_in_the_commit`**(§6.1.3:`oo log` 見標記)
  - **`red_pin_does_not_mark_the_value`**(§6.2:值內無殘留)〔與上一支**成對**〕
  - `red_pinned_value_equals_a_normally_written_one`(同一 evolve 內 pin 的 x 與
    正常寫的 w 渲染須全等)
- **5 釘**(基線已綠,須續綠):`pin_ordinary_evolve_still_conflicts`(**預設仍
  單調**——若此支變綠即本弧失敗)、`pin_fresh_coordinate_still_evolves`、
  `pin_compatible_rebinding_still_evolves`、`pin_ordinary_commit_is_unmarked`
  (審計標記須專屬特權提交)、`pin_grant_still_refuses_effect_discharge`
  (承接上一弧)。

**校準已驗**:7 紅**全紅且各因對的理由**;5 釘全綠。
> 校準留下的教訓(已寫入探針註解):`red_pin_capability_is_operation_specific`
> 初版斷言「非空且未 staged」,在基線**空洞通過**(clap 未知旗標錯誤同樣滿足)。
> 改為斷言**特權拒絕字樣**後才真紅。**連續第二弧出現同型空洞紅**——凡「缺陷
> 表現為某事不發生」的紅門,必須斷言**發生了什麼**,不能只斷言**沒發生什麼**。

**觀察量的兩個實測限制(交付方請勿繞過)**:
1. **跨宇宙 CAID 比對不可行**:兩個全新 store 給相同輸入產生**不同**根摘要
   (每倉 salt/genesis 身分)。故所有不變量一律**在單一宇宙內**觀測。
2. **已提交狀態無乾淨觀測命令**:`oo run` 是純淨一次性宇宙,看不到已提交根。
   故值面觀測走 `oo status`(staged 渲染),歷史面走 `oo log`。

**交付 = 移除全部 7 個 `#[ignore]`**,探針其餘一字不改。

**目標**(基線 → 交付後):

| 項 | 基線 | 目標 |
| :--- | :--- | :--- |
| 本探針 | 5/5(7 ignored) | **12/12** |
| workspace | 1404/0/10 | **1411/0/3** |
| conformance | 143/143 | **143/143(不變)** |
| genesis | 11/11 | **11/11(不變)** |

**合規向量**:本弧**不新增**——能力與 `--pin` 皆僅 CLI 可達(runner 不傳旗標),
同 arc-4／選擇性 discharge 先例,CLI 探針為法定測具。

## 6. 交付紀錄(交付方填;先寫再回報)

- [x] 交付 commit(s): `5a1ea6b` #pin
- [x] CLI(`Evolve` 加 `--pin` + `--grant`,復用 `apply_cli_privilege`)落點:
  - `crates/oo/src/main.rs` `Commands::Evolve { pin, grants, files }`;
    `run_evolve` 呼叫既有 `apply_cli_privilege(&mut engine, false, &grants)`
    (與 run/eval 同解析器,不新寫一份)。
- [x] 閘(有請求無能力 → 大聲拒;無請求 → 原路徑)落點:
  - `run_evolve`:若 `pin && !engine.privilege.pin` →
    `bail!("#privileged_required: …")`,不 load/evolve/stage。
  - 無 `--pin`:`pin_mode=false`,G2-S 與 meet 路徑一字未動。
- [x] 語義(跳過 G2-S 根檢查 + staged 取代而非 meet)落點:
  - `universe.rs` `Universe::{pin_mode, pin_pending}` + `replace_merge`。
  - evolve:僅當 `pin_mode && privilege.pin` 時跳過 G2-S;staged 以
    `replace_merge` 覆寫寫入(非 `unify` meet);`pin_pending=true`。
  - commit:若 `pin_pending`,對 root 做 `replace_merge(root, staged)` 並
    標 `CommitKind::Pin`;否則原 meet。`.oo/pin_pending` 與 staged 並存
    (跨 CLI 行程),清於 commit 成功後。
- [x] 審計(Commit 層落點 + `oo log` 顯示;**值內無殘留**)落點:
  - `CommitKind::Pin`(serde 具名變體;`Standard` 仍 `#[serde(other)]`)。
  - `Ouroboros::log` 回 `(hash, meta, kind)`;`run_log` 對 Pin 印 `    pin`。
  - 值路徑無 `%cause`/`privileged` 寫入(探針成對釘)。
- [x] **確認**:被 pin 之值的 CAID 與正常寫入者相同;舊 commit 反序列化不變:
  - 未觸 `bn_serial`/`to_serial_byte`/值 `content_hash`;審計僅 commit
    kind 位元(Pin 標籤字節 **2**,Standard=0/Refine=1 不變)。
  - 同 evolve 內 pin 的 x 與正常寫的 w 渲染全等(探針)。
  - genesis **11/11**。
- [x] 四數:本探針 **12/12** · workspace **1411/0/3** · conformance **143/143** ·
      genesis **11/11**
- [x] 申報事項(範圍外接觸、CAID、其他):
  - 探針**僅移除 7 個 `#[ignore]`**。
  - 能力槽 `Privilege.pin` 僅啟用,結構未改;effect_override 等軸未擴權。
  - `#commit`/`#rollback`/`#squash` 本體未做(另弧)。

## 7. 驗收紀錄(2026-07-26,驗收方)

**PASS —— 兩件驗收代修,其一為特權升級(security)。** 交付 `5a1ea6b`。
核心機制與審計落點都對,兩件缺陷**都出在 pin 的作用範圍沒有被界定**。

- **Diff 純度** ✓:探針僅移除 7 個 `#[ignore]`。
- **機制審核** ✓:兩步閘(`--pin` 請求 × `Privilege.pin` 能力)、G2-S 跳過只在
  兩者齊備時、`CommitKind::Pin`(具名變體,`Standard` 仍 `#[serde(other)]`,
  舊 commit 反序列化不變)、`oo log` 顯示、值內無殘留。
- **四數**(兩次代修後)✓:本探針 **15/15**、workspace **1414/0/3**、
  conformance **143/143**、genesis **11/11**。

### 驗收代修 1:pin 的取代語義洩漏到未受特權的欄位

`commit` 在 `pin_pending` 時對**整個 staged** 做 `replace_merge`。實測:根有
`y: 5`,一個**普通**(未 pin)的放寬寫入 `y: @int`(G2-S 允許,因
`meet(5,@int)=5≠⊥`)在正常情況下提交後 `y` 仍為 `5`(**對照組實測確認**);
但只要同批次中有任一 pin,`y` 就被**取代**成 `@int`。
⟹ **一個只碰 `x` 的特權操作,改變了同批次中未受特權的 `y` 的語義**——違反工單
§3 C.3「pin 只作用於本次 evolve 的欄位」。
**修**:新增 `Universe.pin_coords`(實際在 pin 模式下寫入的座標集,隨
`.oo/pin_pending` 以 JSON 持久化);commit 改走 `pin_commit_merge`——**只有
pin_coords 取代**,其餘 staged **照常 meet**(且 meet 失敗仍照常大聲失敗:特權
是給那些座標的,不是給其餘的)。不可解析/舊格式的意圖檔案 → **空集合**
(安全側:不得讀成「全部」)。+ 迴歸釘兩支。

### 驗收代修 2:特權升級 —— 意圖檔案被當成授權(§6.1.2 後門)

`.oo/pin_pending` 的**存在**即驅動 commit 端的取代語義與 `Pin` 標記,而 commit
**完全不重驗能力**。而 `.oo/` 是**任何 n/ 程式都能寫**的目錄(`~%Io./write_file`
對 n/ 開放,`~%Json./stringify` 可產出合法 JSON)。**端到端證實**:一支完全
未授權的 n/ 程式(全程無 `--grant`、無 `--privileged`)

```
lst: ["y"]
out: ~%Io./write_file ".oo/pin_pending" (~%Json./stringify lst)
```

即取得 **#pin 覆寫語義**,且其 commit **被假標記為 pin**(歷史說謊)。
這正是 SPEC_08 §6.1.2 明禁的**隱式/無令牌後門**:程式自授權。
**修**:`oo commit` 加 `--grant`/`--privileged`(復用 `apply_cli_privilege`);
`pin_pending` 且無 `Privilege.pin` → **大聲拒** `#privileged_required`。
**意圖檔案記錄的是意圖,不是授權**——授權必須在**施加特權效果的當下**經可信
通道重新出示。修後重跑同一 exploit:commit 被拒、格未動。合法流程
(`evolve --grant pin --pin` → `commit --grant pin`)完好。
**探針修改權行使**:三處 `commit` 呼叫改為攜 `--grant pin`(工作流因代修**正當
地**改變),並補**升權迴歸釘** `pin_intent_file_is_not_authority`(把 exploit
原封不動寫進探針)。

### 掛帳(新)

- **殘留(REAL_02)**:偽造的意圖檔案若恰好搭上一次**合法**的 pin commit,其
  座標仍會被套用。徹底解法需要意圖檔案**可認證**(如以每倉密鑰 HMAC),而
  令牌的鑄造/生命週期依 §6.3 屬 **REAL_02**,非語言規格。已記,不在本弧。
- `#commit`/`#rollback`/`#squash` 本體(另弧;`#commit` 依裁定改規格文字)。
- `.oo/` 目錄對 n/ 程式可寫這件事本身值得單獨檢視(本弧只堵了 pin 這條路;
  其他以 `.oo/` 狀態為據的機制應同樣審視「檔案≠授權」)。

### 分類判定

新增 `oo evolve --pin` 與 `oo commit --grant`,皆**純增能力**;未 pin 的演化與
提交路徑逐字未動(釘覆蓋)。歸 **增量**。

## 8. 意見

本弧兌現前弧「能力槽一次定形」的設計:`#pin` 只需填 `Privilege.pin` 這個槽,
不必再動能力位形狀。

真正的重點在**兩個保證必須同時成立且互相牽制**——歷史上看得見(審計),值裡
看不見(指紋)。任一單獨成立都不夠:只有審計而值被污染 ⟹ CAID 位移,違反
§6.2;只有乾淨值而無審計 ⟹ 特權介入不可溯,違反 §6.1.3。探針把這兩支做成
**成對**紅門,就是為了防止交付偏向任何一邊。
