# 工單:詞法作用域鏈(SPEC_04 §2.1 既有法追法)(2026-07-16)

**執行者**:模型 #3
**驗收者**:專案腦(探針已預先提交,紅線不可動——**全部**探針檔皆紅線。
若交付中發現任何既有釘因新法必紅:**停下報驗收方**,由驗收方修釘;
單方遷移直接計代修。)
**探針**:`crates/interpreter/tests/lexical_scope_probe_test.rs`
(9 紅門 + 11 釘;已校準:紅門今日全紅、釘今日全綠)

**驗收 = 紅門全綠 + 釘全綠 + 全套件無退化(基線 984/0/3 + 本探針
20 測 = 應 1004)+ 語料非 pending 全零敗 + conformance 全綠(含
新增 L2-43~46,交付時應 85/85)。**

**紀錄義務提醒**:交付紀錄先寫進本檔再回報(上單紀錄被上下文
壓縮切斷——先寫單、後回報,壓縮就切不掉)。

---

## 0. 法(既有,零新裁定)

SPEC_04 §2.1 `resolve_bare_name(s, H)`:由內而外沿作用域鏈逐層
搜尋,fields(H) 含 **Data/Type/Logic/Meta** 四軸,首中即回
(內層遮蔽外層);遍歷盡 → `_`(開放世界)。§3.3:態射體裸名經
**定義時閉包 scope** 解析。私有軸 `~key` 同構規則已於私有軸弧
實施(§3.1 #1/#2)——本弧是它的**公有對應面**。

## 1. 量測(v0.2.13,六病六健)

| 面 | 今日 | 法定 |
|---|---|---|
| 兄弟欄 `c:{k:5, d:k+1}`,c.d | `_` | 6 |
| 鏈式兄弟 `e: d + k` | `_` | 11 |
| holder 態射 `c:{k:5, f:(x->x+k)}`,`1\|>c.f` | `_` | 6 |
| 態射體 spread 兄弟 `f:(x->{...p2})` | `_` | 2 |
| 巢狀 holder `w.c.f` | `_` | 6 |
| 非根祖先提升 `w:{k:5, c:{d:k+1}}` | `_` | 6 |
| **遮蔽** `k:5; c:{k:7, d:k+1}` | **6(錯值!)** | 8 |
| 遮蔽態射形 | **6(錯值)** | 8 |
| 顯示 `c` | `d: _` | `d: 6` |

健康:根層提升、引數遮蔽、柯里捕獲、帶 `~` 欄 combo 整鏈活、
factory 範例、態射體 spread 根 combo。

**根因(變因煙槍)**:`value.rs seal_defining_scope` 的
「`c.local.is_empty()` 跳過」——私有軸弧為防公有 combo Thunk
等值/unify 毒化而設;副作用=公有 combo 的 frame 永不注入欄
thunk 閉包,詞法鏈斷。加一個 `~z` 即整鏈復活(M11 實證)。

## 2. 實作路線(擇一,實作裁量權在你;絆線釘管安全)

**(a) 拆門統一 seal(建議)**:移除 local-empty 跳過,所有 combo
字面量注入 frame。原防污染顧慮由**絆線釘**看管:
`pin_twin_literal_eq_tripwire`(雙生字面量 `=` #true)+
`pin_caid_stability_tripwire`(`%id` 穩定)。若拆門後兩釘紅:
等值/CAID 層須對 frame 中性化(如 Thunk PartialEq 忽略注入
frame、CAID 走 forced 值),**勿弱化釘**;無法中性化 → 停下
報驗收方。

**(b) force 時串鏈**:欄 thunk force 時把 holder frame 推上
scope 鏈(導航/observe 知道 holder)。注意態射值被外呼時 holder
不在場——捕獲仍需定義時注入,恐兩套機構;除非有乾淨統一,
(a) 優先。

**遮蔽序**:frame 注入位置必須讓**內層先中**(紅門
`red_shadowing_inner_first`/`_morphism` 以 8 為準;今日 6=外層
頂替,修時注意 scope 鏈查找方向)。

## 3. 邊界與陷阱

1. **絆線雙釘**(上述)——原 seal 門的存在理由;紅了停下。
2. **私有軸弧全釘不可回歸**:factory/insider spread/外部排除
   三釘在檔;統一 seal 路線天然合併私有+公有兩鏈,私值捕獲
   (`pin_private_combo_chain_lives`)須仍活。
3. **E2 量測記錄義務**:`x: {k:5, d:k+1}\nout: x = {k:5, d:6}`
   今日 **#false**——非釘;修後量測記錄。翻 #true=法向改善
   (`=` 應比內容),照記;**翻到其他值=停**。
4. **效果標籤**:frame 注入勿改欄位 effect 推導(predict_effect
   在注入前後應同值)。
5. **spread insider 判準**:`spread_target_is_insider` 對
   ctx.scopes frame 逐一比對——公有 combo 現在也入鏈,frame 數
   增;判準語義不變但覆蓋面變寬,外部排除釘+偽造安全論證
   (spread privacy 弧)不可回歸。
6. **循環**:frame 注入 = combo 克隆快照(私有弧手法:注入前
   克隆,防自指無限)。深巢 combo 效能注意(可考慮 Arc frame);
   全語料時間不得顯著劣化(交付紀錄附語料耗時前後值)。
7. **前向引用**:根層 `k` 後定義 `c` 先引用——今日健康
   (`pin_root_lifting` 涵蓋根層;前向機構 computing 勿動)。
8. **`%code`/顯示**:frame 不得出現在顯示投影(私有弧
   strip/display 兩路已接;新增公有 frame 同須隱形——顯示紅門
   `red_display_siblings_resolved` 只認欄位值)。
9. 全語料回歸 + conformance L2-43~46(今日四紅)。
10. **交付紀錄先寫後報**(根因、diff、量測、未動聲明、E2 記錄、
    語料耗時前後)。

## 4. 非目標

- eq×thunk 強迫語義(E2 面,另案候選)。
- 前向引用×spread(凍結釘在 spread_collision 檔)。
- `~%` 系統軸解析(L2-23 守,勿動)。
- lint 層遮蔽提示(想法 D 候選,另議)。
