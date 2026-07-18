# 工單:blur 顯示序鍵去鹽(SPEC_01 §2.4.1 #5 修正)

**開單**:2026-07-18(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。

## 1. 法源(§2.4.1 起草洞修正,已入法 2026-07-18)

- **SPEC_01 §2.4.1 #5(修訂)**:#blur 族內鍵 =
  **(%cause 名稱字典序, 剩餘燃料升序, 策略)**,**明確排除
  %caid/鹽**;鍵相等者穩定保序(相遇序)。
- 原文「依正典顯示字串字典序」是起草洞:blur 顯示字串內嵌帶鹽
  %caid,字串鍵滲鹽 → 跨行程拼法非決定,正是「禁 digest 鍵」
  條款要防的事(顯示序弧驗收實測:雙 blur 聯集兩次 CLI 先後
  翻轉)。

## 2. 病灶(post-7a515bb 量測)

`display_order_cmp` 族階 4(blur)臂與族階 3 共用
`to_nlang(0)` 字串鍵——同 cause 雙 blur 之相對序由鹽決定。
單 blur / 異 cause 不受影響(字串前綴在 %caid 前已分勝負)。

## 3. 修法方向與位點

- `value.rs` `display_order_cmp`:族階 4 拆出獨立臂——比較鍵
  `(cause 名稱位元組, fuel_remaining, strategy 序)`;全等 →
  `Ordering::Equal`(穩定排序自然保相遇序)。
- 族階 3(結構值)字串鍵**不動**;blur 顯示文字**不動**
  (照印 %caid);`blur_caid`/bn_serial 鹽**不動**(身分照舊
  帶鹽——顯示鍵≠身分)。

## 4. 門(紅)與釘 —— `crates/interpreter/tests/blur_display_key_probe_test.rs`

**已預提交+校準**(2 紅全紅、5 釘全綠)。**探針形制=鹽證明
雙排列門**:同輸入兩種順序都必須出法定序,今日字串排序必在
其中一個排列上強加 caid 序 → 決定性翻紅,不靠鹽運氣。

紅門:同 cause 同 fuel 異鹽平手穩定(雙排列)/同 cause 異
fuel 對抗鹽升序(0xFE/0x01 校準確認對抗)。
釘:異 cause 依名序/族階不動(值<blur<Top)/顯示文字保
%caid/blur_caid 保鹽/lucky-salt fuel 序(校準巧合綠轉釘,
E1-E3 教訓 (i))。

交付=移除 2 個 `#[ignore]`,探針檔**其餘一字不改**(修改權
在驗收方)。全 workspace 一顆不得翻紅;語料非 pending 不退。
conformance 無法載此面(向量單次執行測不到跨行程非決定),
矩陣 116 不動。

## 5. 目標與交付紀錄

**目標**(基線實測 2026-07-18,先量後寫):探針 7/7;workspace
**1192/0/3**(基線 1190/0/5);conformance **116/116** 不動;
語料非 pending **74/0** 不退。

**交付紀錄**(交付方填;先寫再回報):

- [x] 交付 commit(s): (本交付 commit,見 `git log` blur_display_key)
- [x] 根因與修法(blur 臂鍵構成寫明):
  - **根因**:`display_order_cmp` 族階 4 與結構族共用 `to_nlang(0)` 字串鍵;
    blur 顯示內嵌帶鹽 `%caid` → 同 cause 雙 blur 相對序由鹽決定(跨行程翻轉)。
  - **修法**:族階 4 獨立臂——鍵 =
    `(cause.as_str() 字典序, fuel_remaining 升序, strategy 序號
    Blur=0/Strict=1/Approximate=2)`;全等 → `Ordering::Equal`(穩定排序保
    相遇序)。**不含** salt/caid。族階 3 字串鍵不動;blur 顯示文仍印
    %caid;`blur_caid`/bn_serial 鹽不動。
- [x] 探針/workspace/conformance/語料 四數:
  - 探針 **7/7**
  - workspace **1192/0/3**
  - conformance **116/116**(本面非矩陣可載,不動)
  - 語料 unit+integration **74/0**
  - display_order 17/17 保綠
- [x] 申報事項(範圍外接觸、歧異記錄):
  - **未碰** blur 顯示文字、吸收/unify 律、bn_serial 身分鹽、非 blur 族鍵。

## 6. 驗收紀錄(驗收方填)
