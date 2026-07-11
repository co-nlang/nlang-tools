# 工單:E4 nominal `@Name` 引用接線(定義參與合併)

> 2026-07-11 開單。缺口記錄:nlang-spec `meta/ENGINE_SYNC.md`「Range 語義補完」
> E4 條。症狀:README 門面範例 `user: ~payload & @Adult` 完全不 enforce——
> 違規值被**靜默放行**(比 Conflict 更毒的一類)。

## 根因(開單時已定位)

`lib.rs` 路徑解析:`TypeConstraint::is_type_constraint_path` = **任何 `@` 開頭
的名字**都直接變 `{{%kind: #type_constraint, %type: "…"}}` 標記,且該檢查位於
scope/staged/root 查找**之前**(lib.rs ~1133)——使用者定義(`@Adult: {…}`)
永遠不被查到;Unknown 標記的 validate = 無條件放行。

## 裁決

1. **解析順序**:
   - `@name` 屬**內建保留集**(`TypeConstraint::from_name` 非 Unknown 的名字:
     int/float/str/bool/num/complex/option/result/any…)→ 標記,**語義不變**。
   - 其餘 `@Name` → 走**正常查找鏈**(scopes → staged → root;含 record_dep、
     lazy force——與裸名同路;注意既有 prefix-alternates 迴圈已會嘗試 `@`+name,
     儲存形式不需改)。找到 → 即定義值。
   - 查無 → **維持 Unknown 標記直通 fallback**(活釘 `e4_undefined_typeref_passthrough`)。
2. **內建名保留**:使用者 `@int: {…}` 定義**不得遮蔽**內建
   (活釘 `e4_builtin_reserved_not_shadowable`)。linter 警告屬後續,不在本單。
3. **Deref 即值(Trinity)**:解引用後的定義就是普通值,**合併語義零特例**——
   密封 `{{}}` 模板依 SPEC_03 = 窮盡 schema(多餘欄位 ⊥);開放 `{}` 模板只
   約束列出的欄位。**合併機制本身不得動**(`guard_direct_template_merges` 釘死)。
4. **惰性**:查到的定義 force 頂層即可,欄位保持 thunk——遞迴型別
   (`@Tree: { next: @Tree | () }`)必須可終止(紅線+活釘成對)。禁止 eager 深 force。
5. **臂序常規**(第五例警告):新增早臂只擁有自己的值種,其餘 DECLINE。
   Union 紅線成對釘(`0.5 & (@Neg|@Pos)` → ⊥;`10 & (@Neg|@Pos)` → `10` 單枝,
   今天是 `10 | 10` 直通不去重——**只要求此向量經分配收斂,不要求實作通用
   Union 去重正規化**)。

## 探針(已預置)

`crates/interpreter/tests/nominal_ref_probe_test.rs`:**8 紅線**(un-ignore=
驗收門)+ **7 活測**(兩側釘+護欄,全程綠)。斷言不得動。
基線:workspace **700 過 0 敗 11 ignored**(693+7 活;11=3 既存+8 紅線)。
期望終態:**708 過 0 敗 3 ignored**。

## 附帶義務(交付記錄須含)

- **CAID 注意**:使用 user `@` 引用的程式,其觀測值由標記變為定義值——
  觀測面 CAID 位移(語義修正,合法差異),**須在交付記錄中明列**;
  bn_serial 佈局與存儲層 thunk 表達式不得變。
- **量測回報項(非驗收門)**:use-before-def(`user: … & @Adult` 寫在
  `@Adult: {…}` 之前)的行為——回報即可,不裁。
- 鄰域套件必須原樣綠:`range_gaps_probe_test`(26)、`dispatch_test`、
  memo/Stage 紅線全套。

## 非目標(碰了算越權)

- Union 去重的通用正規化(`10|10` wart 另案)。
- 合併/密封機制任何改動;`%kind` 型別邊界標記(SYNTAX_04 §4.7 有意留白)。
- linter 遮蔽警告;SPEC_15 反模式條目(驗收方做規格側回寫)。
- bn_serial/fmt 任何變更。

## 交付與驗收

同 range_gaps 條款:非空提交、根因/宣稱附量測(stash/worktree 反事實)、
交付記錄逐紅線對應。驗收方將全套重跑(708/0/3)、diff-read、對抗加測
(Thunk/Ref/Combo × 新解析路徑;鏡像;deep 遞迴 fuel 行為)、
**假前提死碼掃描**(上單新增之標準檢查:註解宣稱的前提必須與 diff 事實一致)。

---

## 交付記錄(2026-07-11, implementer)

### 根因 / 反事實

- **根因**:`resolve_path` 對 `is_type_constraint_path`(= 任何 `@` 前綴)在
  scopes/staged/root 查找**之前**一律回 type_constraint 標記 → 使用者
  `@Adult: {…}` 永不被查到;Unknown validate = 無條件放行。
- **修復**(最小刀口):
  1. 僅 `TypeConstraint::is_builtin_type_name`(`from_name` ≠ Unknown)短路成標記。
  2. 其餘 `@Name` 走既有 scopes → staged → root 鏈(force / record_dep 同裸名)。
  3. 查無 → `TypeConstraint::marker_value` Unknown 直通(活釘
     `e4_undefined_typeref_passthrough`)。
- **反事實**:修復前 8 紅線 ignore / 7 活測綠(wrong reason pass-through);
  修復後 15/15;合併機制(`guard_direct_template_merges`)與 builtin 標記
  (`guard_builtin_marker_semantics`)零 diff。

### 紅線對應

| 探針 | 機制 |
|------|------|
| `e4_violating_merge_is_bottom` / `_mirror` | `@Adult` → 定義模板;age 15 & 18.. → ⊥ |
| `e4_deref_shape_is_definition` | 解引用得 Combo 定義,非 `%kind` 標記 |
| `e4_sealed_exhaustive_extra_field_bottom` | 密封 `{{}}` 窮盡 schema 經 deref 生效 |
| `e4_projection_through_template` | 欄位內 `@float` 投影 1→1.0 |
| `e4_recursive_type_enforces_and_terminates` | 頂層 force,欄位 thunk;`v: "s"` 違 `@int` → ⊥ |
| `e4_union_of_typerefs_enforces` / `_passing` | 分配後 (⊥\|10)→10 / 0.5 兩枝皆 ⊥ |

### 量測(非驗收門)

**use-before-def**(evolve 順序:`user: … & @Adult` 寫在 `@Adult: {…}` 之前):

| 向量 | 結果 |
|------|------|
| use-before-def, 滿足值 | age=25 通過(查無 → Unknown 直通) |
| use-before-def, 違規 age=15 | **靜默放行**(非 Bottom)——定義尚未入 staged |
| use-before-def, 觀察 `@Adult` 本身 | Unknown 標記(`is_marker=true`) |

原因:逐 field evolve,引用求值時定義尚未 staged;已寫入之 meet 結果不會
因後續定義重算。def-then-use 為預期主路徑。不在本單範圍(無 hoisting /
re-eval)。

### CAID 注意

使用 user `@Name` 引用的程式,觀測值由 **type_constraint 標記** 變為
**定義值**(語義修正,合法 CAID 位移)。bn_serial 佈局與存儲層 thunk 表達式
未變;builtin `@int` 等標記路徑未變。

### 量測終態

- `nominal_ref_probe_test`: **15 過 0 敗 0 ignored**
- 鄰域:`range_gaps_probe_test` 26、`dispatch_test` 5、memo 紅線 2 — 全綠
- workspace:**708 過 0 敗 3 ignored**

### 未動(非目標)

合併引擎 / Union 通用去重 / bn_serial / fmt / linter 遮蔽 / nlang-spec 回寫
(驗收方記帳)。
