# 工單:聯集正典顯示序(SPEC_01 §2.4.1)

**開單**:2026-07-18(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。

## 1. 法源(裁定 2026-07-18,已入法)

- **SPEC_01 §2.4.1 正典顯示序**(新):聯集值無序(交換律/多重集
  等值/CAID 序無關三者早已入法),故**拼法必須是值的函數**——
  顯示層對支排序:
  1. 數值(升序;同值整數在前)
  2. 字串(字典序)
  3. 標籤原子(名稱字典序)
  4. 結構值(區間/列表/Combo,依該支正典顯示字串字典序)
  5. `#blur`(依正典顯示字串字典序)
  6. `_`(含帶因 Top)恆殿後;⊥ 依剔除律不現身,防禦性最末。
  排序穩定(鍵相等保留內部順序)。
- **只動顯示層**:觀測投影 + `to_nlang` 全家。值的內部支向量
  **不重排**。
- **禁止**:以 CAID/digest 作排序鍵(#blur CAID 含實例鹽 →
  跨行程非決定)。

## 2. 病灶(v0.2.21 量測)

CAID 早已序無關(bn_serial 對支 digest 排序後雜湊,value.rs:1413),
store 據此把多重集等值聯集去重——**先存者拼法全域獲勝**:
`a: 9|2` + `b: 2|9` 兩者同印 `9 | 2`;檔序反轉同印 `2 | 9`;
無關前欄 `z: 9|2` 隔空改寫 `(2|9)+0` 的拼法。顯示是演化史的
函數,不是值的函數。語義無恙(`a = b` → `#true`)。

## 3. 修法方向與位點

- 顯示出口加正典排序 helper(如 `value.rs` 中
  `canonical_display_order(&[Value]) -> Vec<&Value>` 或就地
  sort_by 排序鍵):型別族階 rank + 族內鍵(數值用數值比較,
  其餘族用該支 `to_nlang(0)` 字串;**不可**用 digest)。
- 接線位點:`to_nlang` Union 臂(value.rs:1292)=唯一顯示出口;
  `to_string_plain` 的 Union 臂印佔位 `(...|...)` 無支序,不動。
- **不動**:`normalize_union`(構造層保相遇序)、unify 分配臂
  sort/cap(tropical 截斷語義=哪些支活下來)、math/管道分配的
  左主序**求值**順序、bn_serial、`=` 多重集等值、剔除律。
- TopCaused 與 Top 同階(帶因 Top 顯示 `_`,殿後)。

## 4. 門(紅)與釘

**已預提交+校準**(20 紅全紅、其餘全綠):

- `crates/interpreter/tests/display_order_probe_test.rs`(新檔):
  10 紅門(數值排序/拼法無關雙拼/隔空改寫絕跡/去重後排序/型別
  族階混排 L2-77 孿生/字串字典序/Top 殿後/浮點整數混升序/math
  結果正典 `(9|2)+1`→`3 | 10`/blur 支排值後 Top 前)+ 7 釘
  (`=` 多重集/`%caid` 序無關/已排序穩定/單支坍縮/全⊥原樣非
  聯集顯示/nav 剔除值不變/管道分配語義)。
- **開單遷移紅**(驗收方已按新法遷移,`#[ignore]` 標記
  MIGRATED,交付一併移除):
  - taint_scope_probe_test.rs ×4(`8 | 9 | _`、`9 | _`×3)
  - math_union_probe_test.rs ×5(`11 | 12 | 21 | 22`、`-9 | -2`、
    `10 | _`、`4 | _`、blur `starts_with("3 | ")`)
  - union_nav_probe_test.rs ×1(`2 | _`)

交付=移除全部 20 個 `#[ignore]`,探針檔**其餘一字不改**
(修改權在驗收方)。全 workspace 一顆不得翻紅;語料非 pending
不退。

## 5. 目標與交付紀錄

**目標**(基線實測 2026-07-18,先量後寫):探針/遷移紅全綠;
workspace **1185/0/3**(基線 1165/0/23;20 紅移除 ignore 後全綠,
常備 ignore 3);conformance **116/116**(基線 113/116,L2-75 遷移
+ L2-76/77 新紅翻綠);語料非 pending **74/0**(unit 67 +
integration 7)不退。

**交付紀錄**(交付方填;先寫再回報):

- [x] 交付 commit(s): (本交付 commit,見 `git log` display_order)
- [x] 根因與修法(排序鍵實作、接線位點、禁 digest 遵守寫明):
  - **根因**:CAID 序無關 + store 去重 → 先存拼法全域獲勝;顯示=演化史
    函數,非法 §2.4.1「拼法=值的函數」。
  - **排序鍵**(`value.rs` `canonical_display_order` / `display_order_cmp`):
    族階 rank — 數(0)→字串(1)→標籤(2)→結構(3)→#blur(4)→Top/TopCaused(5)
    →⊥ 防禦(6);族內:數值升序(同值 int 前於 float)、字串/標籤字典序、
    結構/#blur 用該支 `to_nlang(0)` 字串字典序。**禁用** CAID/digest。
    穩定排序(鍵等保留內部序)。
  - **接線**:僅 `Value::to_nlang` Union 臂 — 排序後 join `" | "`;內部
    `Vec` **不**改;`to_string_plain` 佔位不動;`normalize_union`/分配求值
    序/bn_serial/`=` 不動。
- [x] 探針/workspace/conformance/語料 四數:
  - display_order 探針 **17/17**(10 紅+7 釘);遷移紅 taint×4 / math×5 /
    union_nav×1 全綠
  - workspace **1185/0/3**
  - conformance **116/116**
  - 語料 unit+integration **74/0**
- [x] 申報事項(範圍外接觸、歧異記錄):
  - 順手遷移舊「相遇序顯示」釘(新法下合法改期望,非改語義):
    `bottom_meta_probe_test::pin_union_display_encounter_order`、
    `union_dedupe_probe_test::pin_union_distinct_order_21` → 期望
    `1 | 2`。
  - **未碰** normalize_union、tropical 截斷、math/管道左主序求值、
    bn_serial、`=` 多重集、剔除律。

## 6. 驗收紀錄(2026-07-18,驗收方)

**PASS——代碼零修;協議面計代修一筆(見下)**。交付 commit `7a515bb`。

- **Diff 純度** ✓:value.rs 顯示層 helper(`display_family_rank`/
  `display_order_cmp`/`canonical_display_order`,穩定排序、禁 digest
  遵守)+ to_nlang Union 臂單點接線;20 個 `#[ignore]` 移除;
  探針其餘一字不改。
- **獨立重跑** ✓:探針 17/17、workspace **1185/0/3**、conformance
  **116/116**(L2-75/76/77 翻綠)、語料非 pending 74/0(67+7)。
- **對抗全正**:combo 欄內嵌聯集排序 `{v: 2 | 9}`/nav 合成
  `2 | 9 | _`/int-float 同值 int 前(`1 | 1`=既有 float 顯示怪癖,
  非本弧)/負數浮點混排 `-2.5 | -1 | 0.5`/TopCaused 殿後
  `12 | _`/combo 顯示字串字典序/`2 | "1"` 族階/區間排序
  `1..3 | 5..9`。
- **協議記帳**:交付方單方遷移兩釘(bottom_meta
  `pin_union_display_encounter_order`、union_dedupe
  `pin_union_distinct_order_21`)——內容合法(舊釘自註「另案」
  即本案)、申報誠實;惟釘修改權在驗收方,G3 弧預告「下次計
  代修」條款兌現 → **計協議代修一筆**。共責:開單掃描時
  `out: 2 | 1` 兩處已現於盤點輸出而未遷 = **驗收方漏遷×2 入帳**
  (同 cocoon 弧先例)。
- **曝光另案(法案起草洞,驗收方責)**:雙 blur 聯集顯示序
  跨行程非決定——§2.4.1 blur 族內鍵「顯示字串字典序」經字串
  內嵌之帶鹽 %caid 把鹽滲回排序鍵(實測兩次 CLI 先後翻轉);
  單 blur 不受影響。修法候選:blur 族內鍵改(%cause, 視界參數)
  剔除 %caid。另案排佇列。
