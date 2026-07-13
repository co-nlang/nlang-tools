# G3 裁定草案:視界抹除(**已批准 2026-07-13**;歷史文件)

2026-07-13 起草、同日批准:**Q1 = 吸收傳播、Q2 = #fuel_exhausted**
(皆按推薦)。執行者(模型 #3)工程補充採納:引數載體/消費者拆清、
helper 不擴權改呼叫前短路。已入法 SPEC_08 §3.2.2;L2-21/22 入庫;
工單 `docs/g3_blur_erasure_handover.md`。本檔保留為裁定沿革。

2026-07-13,驗收方起草。G3 帳載範圍**第五次修正**:原帳「runaway 態射
cause 精化(#conflict → #divergent)」——範圍錯誤。

## 0. 重診斷(量測鏈)

| 向量 | 今日 | 註 |
|---|---|---|
| `/recursive: x -> /recursive (x+1)` 應用 | ⊥ `#conflict` | 原帳症狀 |
| 裸名 `rec`、同引數 `same x` | ⊥ `#conflict` | `/` 非變因 |
| `a: a + 1`(座標自指) | ⊥ `#divergent` | L2-17 偵測路正確 |
| ⊥ #divergent 引數過 apply/pipe | `#divergent` 保全 | ⊥ 短路無恙 |
| **平場 4000 項加法**(無遞迴) | ⊥ `#conflict` | **轉換是通用的,與 runaway 無關** |
| 4000 元素 list | 正常 | list 便宜,燃料死點在 math 鏈 |

**根因**:預設觀測策略 = **Blur**(EvalContext 預設;`%config %strategy
#blur`)。燃料耗盡時 `handle_resource_exhausted` 正確產出**一等視界值
`Value::Blur`**(SPEC_08 §4:#blur 快照、決定論 CAID、可提交;顯示形
`#blur { %cause: #fuel_exhausted, %caid: "…" }` 皆已實作)——然後**值
語境消費點沒有 Blur 臂**:

- `eval_math` 運算元 match 落 `_ => ⊥ #conflict`(eval.rs:843 一帶)——
  視界被鑄成衝突,原因、CAID、partial 全數抹除;
- 原子 cmp `value_context_operand` 對 Blur 走 `other => Ok(clone)` →
  漏到尾端結構比 → **默默 #false**(與 G1 同謊言類:把「觀測被截斷」
  謊報為「比過了、不等」);
- runaway 只是最容易踩到的入口(遞迴燒燃料 → 內層 Blur → 外層 math
  `(x+1)` 抹除)。

**「視界是語義截斷、不是錯誤」**(L2-17 判例、SYNTAX_07 §2 #4、
SPEC_08 §3)在值語境全線失守。Strict 策略下 handler 產 ⊥
#fuel_exhausted,經 ⊥ 短路應可保全(未全測;工單釘)。

## 1. 提案法條(SPEC_08 §4 增補「視界傳播律」)

**R1(值語境 Blur 吸收律)**:值語境(math 運算元、原子比較 `==`/`!=`
運算元、態射應用/管道之引數與體內)遇 `#blur` → 結果為**該 Blur 原樣
傳出**(cause/CAID/horizon 參數保全;效果照 max 合併;partial 快照不
參與後續運算)。與 ⊥ 吸收律同構,但本體地位不同:⊥ = 衝突、#blur =
視界——**值語境不得改寫本體地位**(不得鑄 #conflict、不得默默布林)。

**R2(cause 誠實)**:燃料耗盡的 cause = `#fuel_exhausted`(Strict 之
⊥ 與 Blur 之 %cause 同拼);`#divergent` **保留給偵測到的循環**
(L2-17 座標自指判例)。runaway 態射(引數遞增,不可判定)= 誠實
`#fuel_exhausted`,**非** `#divergent`。同引數自呼(`same x`,理論可
偵測)之升級 = 另案,不併本單。

**R3(meta 觀測)**:`#blur` 之 `%cause`/`%type` 觀測回其 BlurCause
標籤(`(/recursive 1).%type` → `#fuel_exhausted`)。

## 2. 待批決策點

- **Q1** 值語境遇 Blur:**吸收傳播(推薦)** vs 降轉 ⊥ #fuel_exhausted。
  推薦吸收:策略選擇權已在 handler(Strict → ⊥、Blur → 快照),值語境
  只該傳遞不該改判;降轉會使 #blur 的「可提交快照」地位形同虛設。
- **Q2** test_canonical 期望:`(/recursive 1).%type` 修後 =
  **#fuel_exhausted(推薦)** vs 原檔所寫 #divergent。推薦前者:引數
  遞增之 runaway 不可判定發散,#divergent 要留給偵測案(L2-17 先例);
  向量跟法,不跟舊檔註解。

## 3. 批准後動作(照公式)

- SPEC_08 §4 增「視界傳播律」節;REAL_04 cause 表補 #blur 對照註。
- conformance L2 向量:L2-21 runaway `%type` → `#fuel_exhausted`;
  L2-22 平場燃料耗盡顯示 = `#blur {…}` 形(非 ⊥ #conflict)。
- 工單:值語境消費點盤點(math/原子 cmp/引數短路/`=` 家族現況量測)
  + Blur 臂;紅門 = 上述向量 + 平場/runaway/同引數三形態;釘 = L2-17
  #divergent、⊥ 短路保全、Blur 顯示形、Strict 路徑。
- test_canonical 出 pending(G1 已關,G3 關後兩阻塞皆除)。

## 4. 鄰區(明示不在本裁定)

- 同引數自呼之 #divergent 偵測升級(force memo 級循環偵測)。
- 預設燃料 10000 之量級檢討(平場 4000 項加法即死;工程旋鈕,非語義)。
- `=` 家族 × Blur(集合家族不吸收 ⊥;Blur 之集合語義另議,先量測釘現況)。
- timeout → #incomplete 禁 blur(SPEC_08 §100 已法,不動)。
