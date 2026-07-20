# 工單:序關係波 W3(非原子序落地 + 序×blur 二段律)

**開單**:2026-07-20(驗收方)。**基線**:dev @ 本工單 commit。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。

## 1. 法源(零新裁定;波計畫 2026-07-20 已批)

- **SYNTAX_06 §2.1 既有法**:`A <= B ⟺ (A & B) = A`(meet 歸約
  +G1 固化等值);`<`/`>` 真子集=`A <= B ∧ ¬(B <= A)`;§3
  combo 範例逐字;聯集=支集合包含(同一歸約自然涵蓋)。
- **SYNTAX_06 §4 #13(新,隨波裁定)**:序×`#blur` 隨 `=` 二段
  律——同 CAID → 自反答案(`<=`/`>=` `#true`、`<`/`>`
  `#false`);其餘吸收原樣(不得 `#false`/`#conflict`)。
- **極值律不動**(§4 #2):⊥/Top 臂已在非原子面健康(已釘)。

## 2. 病灶(v0.2.30 量測)

非原子序全面 ⊥ `#conflict`:combo(`{a:1} <= {a:@int}`)/聯集
(`(1|2) <= (1|2|3)`)/混合(`1 <= (1|2)`、`(1|2) <= @int`)/
blur(凍結釘)。**原料全健康**:`((1|2)&(1|2|3)) = (1|2)` →
`#true`、`({a:1}&{a:@int}) = {a:1}` → `#true`、開放世界 meet 增
欄 → `#false`——W3=純接線。

## 3. 修法方向與位點

- 位點=`eval.rs` cmp 家族之非原子臂(W2 交付後結構:is_tc 臂
  之後、combo/union → `#conflict` 臂)。
- **blur 臂最先**(在歸約前;歸約會把 blur 餵進 unify):復用
  `=` 二段律機構(eval.rs ~1156 blur boundary #6 臂):任一側
  `#blur` → 同 CAID(content_hash 含鹽同法)→ 自反答案;否則
  吸收(左優先原樣)。
- **通用歸約**:`lte(A,B) = (unify(A,B) 固化後 = A)`(複用既有
  unify + G1 固化等值,**勿**另寫逐型特例);`gte` 鏡像;
  `lt = lte ∧ ¬gte`、`gt` 鏡像。雙向歸約各自獨立求 meet。
- **不動**:極值臂(已在前)、原子臂(W2)、poset rank 臂、
  `=`/`==` 機構、W4 範圍、二元 builtin×聯集(另案)、parser。
- 效能註:歸約對大結構成本=兩次 meet+等值;視界機構自然節流,
  不另設防。

## 4. 門(紅)與釘

**已預提交+校準**(6 紅全紅正因〔非原子全 #conflict〕、4 釘
全綠;開單遷移紅×4=order_wave 圍欄釘(W3 圍欄拆除)、
combo_equality `pin_combo_lte_stays_conflict`(自反 `#true`)、
blur_boundary lt/lte 凍結釘(改吸收);conformance 紅×2=
L2-85/86)。

- `crates/interpreter/tests/order_w3_probe_test.rs`(新檔):
  紅=combo 子型別四面(L2-85 孿生+§3 範例逐字+多欄小集)/
  combo 真子集+自反/聯集包含五面(L2-86 孿生+等集雙拼)/混合
  原子×聯集×型別/blur 自反同 CAID/blur 吸收三面(雙向+combo)。
  釘=非原子極值三面/`=`+`==` 家族/W2 原子面/歸約原料。

交付=移除全部 10 個 `#[ignore]`(6 新紅+4 遷移紅),探針檔
**其餘一字不改**(修改權在驗收方)。全 workspace 一顆不得翻紅;
語料非 pending 不退。

## 5. 目標與交付紀錄

**目標**(基線實測 2026-07-20,先量後寫):w3 探針 10/10;
workspace **1288/0/3**(基線 1278/0/13);conformance
**125/125**(基線 123/125,L2-85/86 翻綠);語料非 pending
**75/0** 不退。

**交付紀錄**(交付方填;先寫再回報):

- [x] 交付 commit(s): (本交付 commit,見 `git log` — message 含 order_w3)
- [x] 根因與修法(blur 臂位置、歸約式、真子集實作寫明):
  - **根因**:非原子 set 族 cmp 末端凍結 `#conflict`;meet+等值原料已健康。
  - **blur 臂**(極值之後、poset/原子/歸約之前):`SYNTAX_06 §4 #13`
    二段律,復用 `=` 之 `blur_caid()`:
    - 雙側 Blur 且 CAID 同 → 自反(`<=`/`>=` `#true`,`<`/`>` `#false`)
    - 否則吸收(左優先原樣,effect max)——不得 `#false`/`#conflict`
  - **通用歸約**(`subset_lte`,W2 原子捷徑保留):`lte(A,B) =
    force(unify(A,B)) == force(A)`(G1 PartialEq);Bottom/Blur meet →
    false。`gte` 鏡像;`lt = lte ∧ ¬gte`、`gt` 鏡像;雙向各自獨立 meet。
  - 末端原 `#conflict` 凍結拆除 → combo/union/混合走歸約。
  - **遷移**:order_wave 圍欄、combo_equality 自反 lte、blur_boundary
    lt/lte 吸收;math_union `pin_cmp_union_frozen` 副產翻為
    `(2|9)<5` → `#false`(W3 法)。
- [x] 探針/workspace/conformance/語料 四數:
  - order_w3 探針 **10/10**(+ 遷移紅四/五處綠)
  - workspace **1288/0/3**
  - conformance **125/125**(L2-85/86 翻綠)
  - 語料 unit+integration **75/0**
- [x] 申報事項(範圍外接觸、歧異記錄):
  - 未碰 W4、`=`/`==`、parser、二元 builtin×聯集分派。
  - math_union 凍結釘非開單列名遷移,因 W3 接線自然翻面,已改釘並申報。

## 6. 驗收紀錄(驗收方)
