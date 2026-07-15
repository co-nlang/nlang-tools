# 工單:私有軸實施(SPEC_04 §3 幾何封裝)(2026-07-15)

**執行者**:模型 #3
**驗收者**:專案腦(探針已預先提交,紅線不可動——**全部**探針檔皆紅線。
若交付中發現任何既有釘因新法必紅:**停下報驗收方**,由驗收方修釘;
單方遷移直接計代修。)
**探針**:`crates/interpreter/tests/private_axis_probe_test.rs`
(11 紅門 + 8 釘;已校準:紅門今日全紅、釘今日全綠)

**驗收 = 紅門全綠 + 釘全綠 + 全套件無退化(基線 930/0/3 − 1 遷移
+ 本探針 19 測 = 應 948)+ 語料 74/0 + conformance 全綠(含新增
L2-32~35,交付時應 74/74)。**

註:bottom_meta 檔 `pin_private_axis_current_behavior`(凍結 `p.~s` → 1,
明文另案)已由**驗收方**於本開單 commit 遷移,後繼紅門
`red_outward_dotted_blocked`。

---

## 0. 裁定(已批;SPEC_04 §3.1 #4/#5 新增、§2.2 `~.` 廢止已入法)

量測:私有軸**全面反轉**——外阻全通、內通全斷(規格自身 factory
範例回 `_`)、態射捕獲死、顯示全裸。

- **內外判準是幾何的**(#5):裸名 `~key` 經作用域鏈解析=內部人
  (定義 combo 及子孫的求值語境天然持有 scope);點段 `.~key` 下降
  進 combo =外部定位 → ⊥ `#private_access_violation`,無例外
  (`_.~key` 同;內部人恆有裸名可用)。
- **向內可見+提升**(#1/#2):combo 內裸 `~key` 沿 scope 鏈向祖先
  查找(shared privacy;規格 factory 範例 = L2-33)。
- **值捕獲**(§3.3):態射 body 裸 `~key` 經**定義時閉包 scope**
  解析;外呼得值、不得反向點入。
- **觀測投影剝除**(#4,唯一新法):顯示(坍縮+結構態)剝 local
  軸,每層深度皆然;CAID/`=`/`%id` 六軸**不變**(釘已證 local 參與
  內容身分)。
- **`~.` 錨廢止**(§2.2):文法本無=天然合規;釘
  `pin_tilde_anchor_stays_unparsed` 防未經裁定復活。

## 1. 地圖與實作建議

1. **外阻**(navigate_segments):Combo 臂取欄前攔截——seg 以 `~`
   開頭**且非 `~%` 前綴** → ⊥ PrivateAccessViolation(BottomCause
   變體既有)。位置在 get_field 之前;⊥/Blur 臂之後(它們不受影響)。
2. **內通**(裸名解析):`~key` 之 Path(Bare) 解析須沿 ctx.scopes
   鏈逐層查 `~key` 欄(含當前 combo 求值語境)。量測起點:root 層
   已活(root 欄位直接在 scope),combo 內斷——查 evolve/eval 進
   combo 字面量時是否推 scope、bare 解析是否試 `~` 拼法。
3. **捕獲**(dispatch):閉包 scope 已有推入機制(dispatch.rs:207);
   確認態射定義時捕獲的 scope 鏈含定義 combo,body 裸 `~key` 沿之
   解析。紅門 `red_morphism_capture` 守。
4. **顯示剝除**(觀測出口投影,G6 project_value_context 一帶 +
   結構態顯示):遞迴剝 `~` 開頭欄位(**`~%` 豁免**);只動顯示
   投影,勿動 to_nlang 之外的任何序列化/CAID 路徑。

## 2. 邊界與陷阱

1. **`~%` 系統軸豁免**(最大陷阱):`~%Config` 等鍵同以 `~` 開頭;
   外阻與剝除都必須放行 `~%` 前綴。釘 `pin_system_axis_exempt_nav`
   + conformance L2-23 在門口守著。
2. **CAID/fmt 勿動**:剝除是顯示層;`%id`/`=`/存檔六軸照舊(兩釘守)。
3. **語料形**:test_entropy 根層 `~c`/`...~c` spread = 裸名路線,
   須不受影響(釘 `pin_root_private_spread_lives`)。
4. **⊥/Blur 導航臂勿動**(前兩弧已法;釘 `pin_blur_and_bottom_meta_
   unaffected` 抽測)。
5. **`--observe` 直接觀測根層 `~x`**:CLI 路徑首段裸名走 scope 路線
   (與 `out: ~x` 同形)——本單不特別封鎖,量測記錄現況即可
   (根 combo 即觀測語境=內部);若實作後行為變動,交付紀錄記載。
6. **效果標籤**:違規 ⊥ 照 BottomDetail 慣例;effect 傳遞照舊。
7. 全語料回歸 + conformance L2-32~35(今日四紅)。
8. 交付紀錄照舊格式(根因、diff、量測、未動聲明)。

## 3. 非目標

- `~.` 錨文法新增(已廢止,勿加)。
- `~%` 影蓋靜默(另案)。
- 跨宇宙觀測之私有語義(REAL 層,另案)。
- lint 層「疑似外部私有存取」提示(想法 D 儀器候選,另議)。
