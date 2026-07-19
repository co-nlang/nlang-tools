# 工單:~%Config 欄名/型別驗證(封閉旋鈕家,SPEC_09 §6)

**開單**:2026-07-20(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。

## 1. 法源(裁定 A,2026-07-20)

- **SPEC_09 所有權條款豁免收窄**:root `~%Config.<裸名欄>` 合法
  ——但**名必屬創世旋鈕表、值必合旋鈕型**。違者(未知名含
  typo/錯型/⊥/Top)於 **evolve 邊界帶名報錯**(同 root 大聲死
  機構,CLI exit 1;ERROR_CODES `#invalid_config`=錯誤類名,
  **不鑄節點級 ⊥**)。旋鈕家封閉:未來旋鈕走規格書演化流程,
  第三方引擎走 `~%Engine`。
- **SPEC_09 §6 旋鈕表**:`fuel`/`timeout`/`max_branches`/
  `max_unification_depth`/`max_lifting_depth`/`max_pattern_nodes`
  =非負整數;`strategy` ∈ {`#blur`, `#strict`, `#approximate`}。
- **顯示面**:觀測 `~%Config` 本體=**有效配置**(創世 ∧ 覆寫
  合成後全七鈕),非 staged 殘片。

## 2. 病灶(v0.2.27 量測)

三違規面全靜默收下:`fool: 50` rc=0;謊面 `feul: 99999` 靜默
忽略(長鏈照舊 10000 處 blur);`fuel: "lots"`/`strategy: 5`
收下後消費端 pattern-match 失敗回預設。顯示面:`out: ~%Config`
只顯示 staged 殘片 `{ fuel: 50 }`。健康面(勿動):逐鈕讀
`~%Config.fuel`→50、透鏡 `(~%Config).max_branches`→64、無關鈕
透見創世 `timeout`→1000。

## 3. 修法方向與位點

- **驗證點**=`universe.rs` evolve 之 root Config 寫入路徑
  (`is_root_config_field_write` 分支,staged partial overlay
  一帶):名籍檢查可於 RHS 求值前;型別檢查於**求值後**
  (`fuel: 40 + 10` 合法=50,已釘)。違者
  `Err(BottomCause::InvalidConfig)`(**新變體,枚舉尾端追加**
  =fmt 紀律;ERROR_CODES 已登記)。
- **顯示面**=observe 之 `~%Config` 綁定(universe.rs 觀測
  overlay 一帶):回傳創世 ∧ 覆寫合成之完整 combo,非殘片。
  逐鈕讀已健康,勿動其機構。
- **不動**:combo 內 `~%Config` 不豁免與整組 `~%Config: {...}`
  大聲拒(system_axis 釘)、`is_system_axis_lhs_forbidden` 本體、
  消費端(eval_context/observe ctx 讀鈕)、`%fuel` 節點級提示、
  parser。整組替換形立法=另案,勿實作。

## 4. 門(紅)與釘

**已預提交+校準**(6 紅全紅正因〔5 靜默收下+顯示殘片缺
timeout〕、4 釘全綠)。

- `crates/interpreter/tests/config_validation_probe_test.rs`(新檔):
  紅=未知名/typo 謊面/錯型 int 雙拼(字串+負值)/錯型 strategy
  雙拼(非標籤+集外標籤)/⊥+Top 雙拼/有效配置顯示七鈕全列。
  釘=七鈕合法寫全過/覆寫透讀三拼/expr RHS 求值後驗型/fuel 50
  真效 blur。
- 本弧**無 conformance 向量**(大聲死+多行顯示皆不可向量),
  門全在探針;矩陣 123 不動。

交付=移除全部 6 個 `#[ignore]`,探針檔**其餘一字不改**(修改權
在驗收方)。全 workspace 一顆不得翻紅;語料非 pending 不退。

## 5. 目標與交付紀錄

**目標**(基線實測 2026-07-20,先量後寫):探針 10/10;workspace
**1259/0/3**(基線 1253/0/9);conformance **123/123** 不退;
語料非 pending **75/0**(unit 68 + integration 7)不退。

**交付紀錄**(交付方填;先寫再回報):

- [ ] 交付 commit(s):
- [ ] 根因與修法(驗證點位置、InvalidConfig 變體、顯示面機構寫明):
- [ ] 探針/workspace/conformance/語料 四數:
- [ ] 申報事項(範圍外接觸、歧異記錄):

## 6. 驗收紀錄(驗收方)
