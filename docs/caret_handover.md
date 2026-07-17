# 工單:`^` 上溯解析(off-by-one 蟲族 + LHS 廢止)

**開單**:2026-07-17(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。

## 1. 裁定(已批 2026-07-17;SYNTAX_03 §4.4 #4 重寫、SPEC_07 §4.2.3 同步)

**Q1 — 容器鏈含根宇宙**:`^` = 當前容器之父、`^^` = 再上一層,鏈以
root 宇宙為最外層(二層深處 `^^` 即達 root;與 `_.` 絕對錨同鏈之相對
拼法)。root 級 `^` = 根無父 → overshoot ⊥(今日已對)。

**Q2 — 嚴格座標**:`^.x` = 指定層的座標存取——該層無 `x` → 開放 `_`
(疊加態預設),**不得**外溯祖層(外溯是裸名詞法鏈的事;`^` 是路徑錨)。

**Q3 — LHS `^` 定義鍵廢止**(文法層,`~.` 錨先例):定義鍵位(Named/
Path/任意巢深、root 級皆同)含 `^` = **parse error**。文法不含 = 引擎
天然合規。RHS `^` 拼法不動。語料零使用、parser goldens 全 RHS 面已查核。

## 2. 病灶(v0.2.18+ 量測)

引擎 `^ⁿ` = 從**欄位所在容器**起算 n−1 層(少爬一層),root 不可達:
- `s:{a:1, d:{a:9, v:^.a}}` → **9**(當前容器)——法 `1`(父);
- `c:{a:1, d:{v:^.a}}` → `_`;`w:{a:7,m:{n:{v:^^.a}}}` → `_`;
- `r_a:42; c:{d:{v:^^.r_a}}` → `_`(root 不可達)——法 `42`;
- LHS `w:{d:{^.z:5}}` → z 寫進 d 自己(同族寫面;廢止後 parse error)。

overshoot 雙面健康(`^^^` 從二層深、root 級 `^` 皆 ⊥ `#out_of_horizon`)
——修正後必須**繼續**成立(從 d:鏈 = d→c→root,`^^^` 恰好溢出;內部
計數改了但判決不變)。嫌疑位點:parser `Parent(n)` 編碼(單 `^` =
`Parent(0)`)與 eval 解析臂的計數約定——擇一為準,勿雙改互抵。
`bottom_meta_probe_test.rs` 之「scopes unwired 另案」註記 = 本單收帳,
交付時可順手更新該註(僅註解,非釘)。

## 3. 門(紅)與釘 —— `crates/interpreter/tests/caret_probe_test.rs`

**已預提交+校準**(6 紅全紅、7 釘全綠)。交付=移除 6 個 `#[ignore]`,
探針檔**其餘一字不改**(修改權在驗收方)。

紅門:
1. `red_caret_parent_basic` — `^.a` → 1
2. `red_caret_parent_shadowing` — 遮蔽決定形 → 1(L2-66;錯值謊言殺)
3. `red_caret_two_level` — `^^.a` 隔兩層 → 7
4. `red_caret_reaches_root` — `^^.r_a` → 42(L2-67)
5. `red_caret_arith_operand` — `^.a + 1` → 2
6. `red_caret_lhs_key_rejected` — 巢內/root 級/路徑鍵三形 parse error(Q3)

釘(全數保綠):`pin_caret_overshoot_deep`(L2-68)、
`pin_caret_overshoot_root_level`、`pin_caret_no_lexical_fallback`(Q2)、
`pin_root_anchor_still`(`_.`)、`pin_bare_lexical_still`(詞法鏈不動)、
`pin_rhs_caret_parses`(RHS 拼法保全)、`pin_caret_twin_eq`(防 frame
污染,詞法 %id 教訓同族)。

另:parser goldens(basic/roundtrip/golden_ast 之 RHS `^` 形)保綠;
詞法作用域/補完全釘、全 workspace 一顆不得翻紅;語料非 pending 不退。

## 4. 範圍外(碰到=停,不改)

- `^` × 管道(P2 禁令已法,SYNTAX_12 §2.4)。
- 態射體內 `^`(定義閉包 vs 呼叫點容器之綁定面)——遇歧異記錄勿裁。
- `#out_of_horizon` details 欄(requested/actual depth cocoon)——另案。
- 裸名詞法鏈機制本體(seal/lexical_forcing)——`^` 解析應是獨立路徑臂,
  勿為過門改詞法機制;若非動不可,停下報驗收方。

## 5. 目標與交付紀錄

**目標**:探針 13/13;workspace(開單基線見本 commit 訊息之**實測值**,
6 紅 `#[ignore]` 移除後全綠);conformance **107/107**(基線 105/107,
L2-66/67 翻綠、68 保綠);語料非 pending **78/0** 不退。

**交付紀錄**(交付方填;先寫再回報):

- [x] 交付 commit(s): (本交付 commit,見 `git log` caret)
- [x] 根因與修法(附量測;Parent(n) 編碼約定寫明):
  - **Parent(n) 編碼**(parser **不動**):`^.` → `Parent(0)`,`^^.` → `Parent(1)`,
    `n` = 第一支 `^` 之後的額外 caret 數(`matches('^').count()-1`)。
    Display 與 golden_ast 同律。
  - **T 引擎 off-by-one 根因**:`resolve_path_internal` 之 `Parent(count)` 臂
    用 `scopes[len-1-count]`,對 `Parent(0)` 落在 **current** frame
    (`scopes.last()`)——量測:`s.d.v` 之 `^.a` 回 `9`(子層謊言);
    且 scopes 不含 root → `^^.r_a` 不可達。
  - **修法(只改 eval 計數,不改 parser 編碼)**:`hops = count + 1`
    (Parent(0)=上溯 1 層=父)。`hops < len` → `scopes[len-1-hops]`;
    `hops == len` → **root 宇宙**(Q1 鏈頂);`hops > len` →
    `#out_of_horizon`。落地後 segment 走 `navigate_segments`(Q2 嚴格座標,
    缺鍵 open `_`,不外溯)。
  - **Q3 LHS 廢止**:`n.pest` 定義鍵改 `field_root_path = anchor_root ~
    path_segments?`(僅 `_.`);`field_key` 不再吃 `anchor_parent`。RHS 仍經
    primary 之完整 `anchored_path`。`parse_field_key` 認 `field_root_path`。
  - **註解**:`bottom_meta_probe_test` 「scopes unwired 另案」改為本弧已收帳
    (僅註,非釘)。
  - 量測:修前 6 紅全紅;修後探針 13/13;overshoot 雙面保綠。
- [x] 探針 13/13 / workspace / conformance / 語料 四數:
  - 探針 **13/13**
  - workspace **1122/0/3**(開單基線 1116/0/9)
  - conformance **107/107**(L2-66/67 翻綠、68 保綠)
  - 語料 unit+integration **74/0**(歷次同路徑)
  - parser goldens / lexical_scope / lexical_completion / bottom_meta 保綠
- [x] 申報事項(範圍外接觸、態射體內 ^ 歧異記錄、合法改善):
  - **未碰** `^`×管道、態射體內 `^` 綁定面(未遇歧異可裁)、
    `#out_of_horizon` details cocoon、裸名詞法鏈 seal/lexical_forcing 本體。
  - 詞法鏈與 `_.` 絕對錨獨立保綠(探針釘)。

## 6. 驗收紀錄(2026-07-17,驗收方)

**PASS——零代修(第十八例)**。交付 commit `0ff683b`。

- **Diff 純度** ✓:eval 計數改 `hops = count+1`(parser Parent(n) 編碼
  不動=工單「擇一為準」遵守)、`hops==len` → root 宇宙、`>` → ⊥;
  文法 `field_key` 改吃 `field_root_path`(僅 `_.` 錨),RHS 經 primary
  不動;bottom_meta 註解更新已申報(僅註非釘)。探針檔僅 6 個
  `#[ignore]` 移除。
- **獨立重跑** ✓:探針 13/13、workspace **1122/0/3**、conformance
  **107/107**(L2-66/67 翻綠、68 保綠)、語料非 pending 78/0。
- **對抗全正**:三層上溯(`^^^` 從深 3 → 值)、雙層遮蔽取外層、
  升降混合 `^.a.q`、cocoon 容器上溯、root 級深溢出 ⊥、展開旁 RHS
  `^` 正確綁父。
- **新曝光(另案記帳)**:態射體內 `^` 解析到**呼叫點容器**——
  `f: (n -> ^.k)` 於 `h: {k:8, r: 1|>f}` 回 8(=h.k),動態綁定
  風味;定義閉包 vs 呼叫點 = 裁定候選(工單預留「記錄勿裁」面,
  交付方未遇、驗收方補量入帳)。與 P2(`$` 不跨管道)之對照值得
  一併裁。
