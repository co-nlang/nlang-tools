# 工單:G2 解體 — 多參自動柯里 + root 內建影蓋中毒 + 原子×態射 (2026-07-12)

**執行者**:模型 #3
**驗收者**:專案腦(探針已預先提交,紅線不可動)
**探針**:
- `crates/interpreter/tests/slash_shadow_multiparam_probe_test.rs`(10 紅門 + 10 釘)
- `crates/oo/tests/slash_shadow_cli_probe_test.rs`(1 紅門 + 2 釘)

紅門今日全紅、釘今日全綠(已校準實測)。**驗收 = 紅門全綠 + 釘全綠 + 全套件無退化 + conformance 全綠(含新增 L1-29/L1-30)。**

---

## 0. 重要:G2 已重新診斷 — 別照舊帳修

語料清理時的帳載「`/` 前綴柯里定義破壞所有應用形態」**範圍錯誤**。
2026-07-12 重新量測,G2 解體為三個獨立缺陷(外加一個範圍外新缺口):

| 件 | 真相 | 今日行為 |
|---|---|---|
| **G2-M** | 多參糖 `x y -> body`(SYNTAX_11 §表格:合法「自動柯里化」)解析成 `Morphism{param: Apply(x,y), …}`,eval 打包時 param 非單一路徑 → 規則鍵退化,分派永不命中。**裸名與 `/` 定義同壞** —— `/` 從來不是變因 | `aeq 5 5` → `_`;`5 \|> aeq 5` → `5`(默默回傳管入值) |
| **G2-S** | 使用者 `/name:` 定義撞上 **root 頂層內建規則座標**(目前恰只有 `/add` = math.add 閉繭)。evolve 靜默成功,毒發於 observe 入口的 `unify(root, staged)`:內建閉繭缺 `%rules` 鍵 → MissingKey ⊥ → **整個宇宙所有觀測全 ⊥ #conflict、無路徑資訊、exit 0** | `/add: (x->…)` + `z: 42` → 觀測 z 也 ⊥ |
| **G2-C** | `do_unify` 的 Atom×Combo 臂(unify.rs:219–231)把原子無條件塞成**任意** combo 的 `%val` —— 包括閉合態射繭 | `/add: 7` → 觀測 `/add` 得 `{%builtin:"math.add", %morphism:#true, %val:7}`(閉繭長新鍵、無衝突) |
| G5(範圍外) | tuple 參數 `((x, y) -> …)` 解析正確(`Morphism(Tuple[…])`)但引擎分派側解構**未實作**,全應用形態 `_`。另擇期派單,**本單不碰** | `((x,y) -> x+y) (3,5)` → `_` |

語料當時全滅的兩個真因:斷言庫用多參糖(G2-M)+ 範例愛用 `add` 這個唯一撞名字(G2-S)。兩蟲疊影 → 誤判成「`/` 全滅」。不撞名的 `/myadd`、`/assert_eq`(顯式柯里)今日**全形態正常**,已釘。

---

## 1. 裁定(規格依據)

### R-M:多參糖 ≡ 顯式柯里(SYNTAX_11 §表格「柯里化多參數(空白分隔,自動柯里化)」)
`x y -> body` **必須**等價 `x -> (y -> body)`,n 元遞推(`x y z -> b` ≡ `x -> (y -> (z -> b))`)。
去糖位置 = **AST 建構期**(SPEC_14 §2.3 已有 builder 摺疊先例;parser `lib.rs:105/121` 兩處 Morphism 建構點)。
- **只摺疊**葉子全為裸單段路徑的 Apply 鏈;其他 param 形(Tuple、pattern、雜項)**保持現狀不動**(tuple = G5,pattern 分派已有機制)。
- 合成的巢狀 Morphism 需帶合理 span(nlint R1–R3 靠 span 定位;沿用外層 span 即可,不必精切)。

### R-S:root 座標單調演化,不相容 → evolve 邊界即死、帶名報錯
在 `universe.evolve`:對本欄位將寫入的每個座標(`field_coords` 已算好),若 **root** 已有該座標且 `unify(root值, 新值)` = ⊥,**立即回傳 Err**,CLI 走與資料軸衝突相同的 `Evolution Conflict` 路徑(exit ≠ 0、stderr 帶座標名)。
- 相容演化(unify 非 ⊥)照常放行 —— 檢查**只在 ⊥ 時**攔截,不是凍結 root。
- 不撞名 `/` 定義、combo 局部 `/add`、資料軸 `add:`(與規則軸不同座標)全部不受影響(已釘)。

### R-C:原子 × 態射 combo = ⊥ #conflict
`do_unify` Atom×Combo 臂:若 combo `is_morphism()`(value.rs:794,查 `%morphism`/`%rules`/`%builtin`),回 ⊥ Conflict。態射不是值容器。
- **非態射** combo 的 `%val` 吸收行為**保留**(釘 `pin_nonmorphism_val_absorb_survives`)。
- R-C 落地後,`/add: 7` 自動經 R-S 在 evolve 即死(紅門 `red_shadow_builtin_add_atom_errs_at_evolve`)。

---

## 2. 地圖(量測過的落點,非指令)

- parser `crates/parser/src/lib.rs:105`(後綴鏈)與 `:121`(infix)—— Morphism 建構,R-M 摺疊點。建議抽 helper `fold_multiparam(param, body, span)`。
- `crates/interpreter/src/universe.rs:60 evolve()` —— R-S 檢查點;`field_coords()`(同檔 :9)已回傳本欄位座標;root 取值 `self.root.get_field(coord)`。
- `crates/interpreter/src/unify.rs:219–231` —— Atom×Combo 臂,R-C 加 morphism guard。
- eval.rs:428 Morphism 打包(pk 抽取)**不用改** —— 去糖後 param 恆為單一路徑;`_` fallback 留給 pattern 參數。
- nlint R4 的 `collect_param_names` 已有 Apply 臂(R4 驗收代修)—— 去糖後該臂對 parser 產出成死碼但**留著**(belt-and-suspenders,防手構 AST)。

## 3. 邊界與陷阱

1. **摺疊守門要嚴**:`Apply` 鏈中任一葉非裸單段路徑(帶錨點、多段、非 Path)→ 不摺疊、保持現行打包。寧漏勿誤。
2. R-S 只查 **root**,不查 staged(staged×staged 衝突已有既有機制,釘 `pin_data_axis_conflict_errs_at_evolve` 看守)。
3. R-S 錯誤的 cause 不釘死(閉繭案是 MissingKey、原子案是 Conflict 皆可)——紅門只驗 `is_err` + CLI 標籤;**別**為了統一 cause 去改 unify 判定。
4. unify_memo 快取 Atom×Combo 結果 —— R-C 改判後注意既有快取鍵不跨判定(每次 run 新引擎,實際無殘留;別自行加清快取邏輯)。
5. 全語料回歸:`oo test tests/unit tests/integration`(65/0 + 7/0 基線)+ workspace 762/0/3 + conformance(spec 側 `scripts/run-conformance.py --engine`,含新增 L1-29 多參柯里、L1-30 `/` 定義應用,交付時應 50/50)。
6. 交付紀錄照舊格式:根因、diff 摘要、量測(紅門前後、全套件、conformance)、未動聲明(探針檔、既有測試)。

## 4. 非目標

- G5 tuple 參數解構(另單)。
- Range 合併、fmt、`%val` 吸收的通盤重審(只加 morphism guard)。
- root 內建面的增減(`/add` 是否該存在於 root 頂層 = 規格議題,另議;本單只治毒發模式)。

---

## 驗收紀錄(驗收者填)

（待交付）
