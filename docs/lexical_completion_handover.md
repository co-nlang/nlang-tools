# 工單:詞法鏈補完(SPEC_04 §2.1 全遞迴;上弧殘欠)(2026-07-16)

**執行者**:模型 #3
**驗收者**:專案腦(探針已預先提交,紅線不可動——**全部**探針檔皆紅線,
含上弧 `lexical_scope_probe_test.rs` 全 21 測。若交付中發現任何既有釘
因新法必紅:**停下報驗收方**;單方遷移計代修。)
**探針**:`crates/interpreter/tests/lexical_completion_probe_test.rs`
(9 紅門 + 8 釘;已校準:紅門今日全紅、釘今日全綠)

**驗收 = 紅門全綠 + 釘全綠 + 全套件無退化(基線 1005/0/3 + 本探針
17 測 = 應 1022)+ 語料非 pending 全零敗 + conformance 全綠(含
新增 L2-47~49,交付時應 88/88)。**

**紀錄義務**:交付紀錄先寫進本檔再回報。

---

## 0. 法(既有,零新裁定)

SPEC_04 §2.1 `resolve_bare_name` 是**全遞迴**的:任意鏈深、Combo
與 Cocoon 內一體適用、首中即回。上弧交付的兩段 snap/frame 手法
=深度 2 上限(結構性:每加一跳需再一層注入),本單補完。

## 1. 量測(八病四健)

| 面 | 今日 | 法定 |
|---|---|---|
| 3 跳鏈 `g2: e+1` | `_` | 12 |
| 4 跳鏈 `h2: g2+e` | `_` | 23 |
| 態射引 2 跳兄弟 `f:(x->x+e)` | `_` | 12 |
| 私有 combo 3 跳 | `_` | 12 |
| 顯示 | `g2: _, h2: _` | `g2: 12, h2: 23` |
| cocoon 兄弟 `{{k:5, d:k+1}}.d` | `_` | 6 |
| cocoon 態射 | `_` | 6 |
| **cocoon 遮蔽** `k:5; {{k:7,d:k+1}}.d` | **6(錯值第二例)** | 8 |
| 巢內 cocoon | `_` | 6 |

健康(釘):2 跳鏈 11、鏈×提升 8、cocoon 純 force 2、cocoon 雙生
`=` #true。

**根因兩支**:(1) 深度牆——frame 內的 thunk 只帶 snap(未密封
克隆),從 frame 取出的欄再解析下一跳時無鏈可走;(2) cocoon——
closed 語境於字面量建構時**先 force 欄位、後跑 seal**,force 當下
無 frame,`_` 被烤死進本徵態(遮蔽錯值=外層頂替)。

## 2. 設計方向(裁量在你;守欄如下)

**深度牆**:建議放棄「注入即自足」路線,改**解析時 frame 上推**:
裸名中於 scope 鏈某 frame 欄、且值為 Thunk → 以「該 frame 在鏈上」
的 ctx 續 force(深度自然遞迴,不需 n 層注入)。或其他深度無關
機構,唯守:

1. **循環守衛必保凍結釘**:互指 `{a2:b2, b2:a2}` → `_`、自指
   `{d:d+1}` → `_`、workspace cycle_test **Top**——上弧棄案正是
   因為 ambient 路線把它翻成 #divergent。守衛語義=**再入視為
   未解、續鏈外溯**(最終 `_`),不得鑄 #divergent(該裁定是
   另案候選,勿越權)。in_flight content-hash 再入機構可用。
2. **絆線三釘**:上弧 `pin_twin_literal_eq`/`pin_caid_stability`
   /`pin_caid_cross_depth_repair`(%id 走 force_recursive 固化,
   驗收代修)全在檔;新機構不得回歸。
3. 上弧九紅門(現綠)全數不可回歸(2 跳、遮蔽 8、顯示等)。

**cocoon**:先 seal 後 force(建構序對調),或 force 時 frame 置
鏈頂(遮蔽序:內層先中,紅門以 8 為準)。唯守:

4. **本徵態封閉語義勿動**:cocoon force-at-build 本身是法
   (GUIDE_03 §11.5 固化邊界)——只改 force 時**看得見什麼**,
   不改**何時 force**的對外可觀測面;`pin_cocoon_plain_force`
   /`pin_cocoon_twin_eq` 守。
5. **cocoon 未定義鍵凍結**:`{{a:1}}.b` → `_` 照舊(已曝光違法
   =SPEC_03 §1.3 本徵態預設 ⊥ 未實施,**另案**,勿順手修——
   凍結釘在檔)。
6. **效果隔離勿動**(SPEC_03 §1.2 #3;corpus effect_cocoon 守)。

## 3. 邊界與陷阱

1. 長鏈燃料:50 跳鏈量測記錄(耗盡應走視界律 #blur,非 panic)。
2. 效能:語料耗時前後值入交付紀錄(上弧 0.68→1.18s;再顯著
   劣化須說明)。
3. 前向引用機構(computing/in_flight)勿破——`pin_root_lifting`
   (上弧)+ forward-ref 語料守。
4. spread insider 判準(frame 鏈變長/變形時)——spread privacy
   釘在檔。
5. 顯示投影:frame 不現身(上弧紅門現綠,守)。
6. 全語料回歸 + conformance L2-47~49(今日三紅)。
7. 交付紀錄:根因、diff、量測(含 50 跳、語料耗時)、未動聲明。

## 4. 非目標

- cocoon 本徵態預設 ⊥(另案,凍結釘)。
- 互指/自指語義裁定(另案候選)。
- eq×thunk、前向引用×spread(各自凍結)。
- 效果隔離、`~%` 系統軸。

---

## 5. 交付紀錄(2026-07-16,模型 #3)

### 根因
1. **深度牆**:上弧兩段 snap/frame 結構性限深 2——frame 內 thunk 只帶
   snap,再取下一跳無 ambient holder。
2. **cocoon**:Path 鍵在 seal 前 `force`(closed 臂),兄弟裸名無 frame
   → `_` 烤進本徵態;遮蔽錯值=外層根 `k` 頂替。

### Diff
1. **`force_lexical_name`**(lib.rs):scope 鏈命中欄 → 標記
   `ctx.lexical_forcing`;`force` 在 lexical 期間**保留 ambient scopes**
   +  thrunk closure 內層推入 → 任意跳深自然遞迴。
2. **軟再入**:`in_flight` 再入且 `lexical_forcing` 非空 → `Top`(未解
   外溯),否則仍 `#divergent`(L2-17 路徑/座標循環)。互指/自指兄
   弟釘與 `cycle_test` Top 守住。
3. **`seal_defining_scope`**:回到單段預注入 frame(深度改由解析
   時上推承擔;絆線同拼寫仍綠)。
4. **cocoon**:Path 臂不再建構期 force;seal 後 `force_recursive`
   (GUIDE_03 §11.5 固化邊界保留,僅改 force 時可見 frame)。

### 量測
| 項 | 結果 |
|---|---|
| 本探針 9 紅+8 釘 | **17/17** |
| 上弧 lexical_scope 21 | **21/21**(無回歸) |
| workspace | **1022/0/3**(1005+17) |
| 語料 | **74/0** |
| 語料耗時 | 上弧後 **1.18s** → 本單 **0.74s**(單段 seal 更輕) |
| conformance | **88/88**(L2-47~49) |
| 50 跳鏈 `c0..c50` | **50**(64MiB 棧;預設小棧可能 overflow——oo 主線
   程已 64MiB;未 panic 於生產配置) |
| 絆線 / 凍結互指自指 / cocoon miss | **綠** |

### 未動聲明
- cocoon 未定義鍵 ⊥(凍結 `_`)、互指/自指另裁定、eq×thunk、
  前向×spread、效果隔離、`~%`、computing/in_flight 真循環 `#divergent`。

---

## 6. 驗收紀錄(2026-07-16,驗收方)

**判定:通過——零代修(第十三例);紀錄義務履行。**

獨立重測:純度乾淨(僅 9 個 `#[ignore]` 移除);本探針 17/17、
上弧 21 釘無回歸、workspace **1022/0/3**、語料非 pending 全零敗、
conformance **88/88**(L2-47~49 關門)。

diff 審查:`lexical_forcing` 軟再入=上弧棄案的**手術版**——
ambient 只在詞法 force 期間保留、L2-17 真循環照舊 #divergent、
凍結釘(互指/自指/cycle_test Top)全守;cocoon 先 seal 後
force_recursive(固化邊界保留,effect 於固化後重取 max);單段
seal 回歸使語料耗時 1.18→0.74s(淨改善)。

對抗性邊界(工單外,全正):**動態作用域洩漏兩形皆淨**——
param 形 `d:k+1; f:(k->d); 9|>f` → `_`(呼叫方 k 不得滲入詞法
鏈)、caller-holder 形 `w2:{k:3, use:c.d}` → `_` ✓;cocoon 3 跳
12 ✓;6 跳鏈 6 ✓;遮蔽×鏈 15 ✓;param 遮蔽兄弟 10 ✓;cocoon
雙生 %id #true ✓;互指算術 `_`(凍結一致)✓。

記錄:50 跳鏈於 64MiB 棧綠(小棧或 overflow=既有模式,oo 主線
程已配);cocoon 本徵態預設 ⊥ 另案在佇列。

模型 #3 檔案:零代修第十三例。上弧殘欠清償,SPEC_04 §2.1 全遞迴
到位。
