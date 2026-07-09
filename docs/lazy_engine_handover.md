# Handover：惰性引擎第一刀（Call-by-Observation, Stage 1–3）

> 2026-07-07。交接對象：引擎側 agent（冷啟動）。
> 藍圖：GUIDE_03 §11（規範性依據在 SPEC_07 §4.2 P1–P5 與 SYNTAX_12 §4 定案註記）。
> 這是本專案目前**最深**的工程項——所以本 handover 分三個階段，**每階段獨立提交、
> 獨立驗收、全語料綠才進下一階**。Stage 3 含一個未決設計題（§5），**不得自行裁決**。

---

## 0. 使命與分階段

急切引擎在 evolve 期就坍縮開放項，做不出規格釘死的行為（GUIDE_03 §11.1 的銜尾蛇
向量）。目標：欄位求值改為**觀測時**（call-by-observation），分三刀：

| 階段 | 內容 | 語義可見性 | 驗收 |
| :--- | :--- | :--- | :--- |
| **1** | Thunk 補 context 槽＋欄位路徑 thunk 化＋固化邊界 | **零**（evolve 仍全量固化） | 全 85 套綠、不改任何期望值 |
| **2** | 惰性 unify＋管道綁定修正＋evolve 存 thunk＋observe 邊界固化 | `#no_context` 時點後移（觀測值不變）；開放項可儲存 | 定律套件原樣重跑＋新開放項測試 |
| **3** | 綁定傳播（銜尾蛇角落） | `v.w.x.a → "Logic"` | §11.1 終極向量 |

## 1. 必讀（順序）

1. GUIDE_03 **§11 全節**（本案藍圖；§11.2 有引擎現況三缺口的審計）
2. SPEC_07 §4.2（P1–P5——`$` 語義的規範源）＋ SYNTAX_12 §2.4／§4 定案註記
3. `crates/interpreter/tests/context_dollar_test.rs`（13）＋ `pipe_laws_test.rs`（13）
   ——**這 26 項是不可退讓的行為合同**（定律與求值策略正交，018 §7）
4. `docs/discussion/019`（tier 分類——Stage 4 memo 的依據，本案只需讀 §2）
5. 引擎關鍵位置：`eval.rs` combo 臂（`FieldKey::Path` 急切臂 vs `Named`/`Quoted`
   thunk 臂）、`lib.rs` `force`／`sub_context`／`navigate_segments`、
   `universe.rs` `evolve`（`force_recursive`）／`observe`、`unify.rs` 入口 force 對、
   `eval.rs` `pipe_apply`

## 2. Stage 1：機械刀（語義不可見）

1. **Thunk 加槽**：`Value::Thunk { expr, closure, effect }` →
   `{ expr, closure, context: Option<Box<Value>>, effect }`。
   建立時捕捉 `ctx.context_value`（P1：演化邊界內建的 thunk 生而有綁定；
   頂層／裸容器內建的 thunk `context: None` ＝ 開放項）。serde 相容注意：加
   `#[serde(default)]`。
2. **force 綁定規則**：`call_ctx.context_value = thunk.context.clone().or(原值)`
   ——thunk 自帶綁定優先，否則觀測方動態綁定（P3），仍無則 `$` 求值處自然落
   `_|_ #no_context`（已實作，`BottomCause::NoContext`）。
3. **欄位 thunk 化**：combo 臂的 `FieldKey::Path` 急切臂改建 Thunk（單段鍵直插；
   多段鍵 `a.b.c:` 經 `inject_path` 把 thunk 放葉位）。`Named`／`Quoted` 臂已是
   thunk，補 context 捕捉即可。
4. **固化邊界（照 §11.5，這些地方保持急切——是語義不是最佳化）**：
   Cocoon `{{}}` 構造時 force 欄位（本徵態封閉）；tuple 元素照舊急切（定長密封）；
   poset 照舊（rank 計算需值）；`%eval_mode: #eager` 逃生門保持可用。
5. **evolve 不動**：`force_recursive` 留在原位——Stage 1 的固化時點不變，
   語義面應零差異。
6. **防雷清單**：雙底表示（`Value::Bottom` 與字面量 `Atom(Bottom)`）在所有
   force／剪枝判定處都要雙查（現有先例：spread splice、管道分配）；
   `ctx.fuel = call_ctx.fuel` 回寫模式各 force 點保持；`predict_effect` 繼續當
   thunk 的效應預估。

**Stage 1 驗收**：全 workspace 85 套綠、**不允許改任何測試期望值**；
新增 1 測試：未觀測的重欄位不消耗燃料（fuel 帳前後對比）。

## 3. Stage 2：語義刀（固化時點後移）

1. **Thunk 的 CAID**（惰性 unify 的前提）：`content_hash(Thunk)` ＝
   H(expr 正準序列化 ‖ frame hash ‖ context hash | `#open`)——即 GUIDE_03 §11.3
   的 memo key 三元組。**先查現況**：`content_hash` 對 Thunk 現在算什麼（可能未
   定義或不穩定）；必須決定論且與求值無關。
2. **惰性 unify（force 迴避）**：`unify_internal` 現在入口就 force 兩側——改為：
   先算 CAID，`id_a == id_b` 早退（不 force）；`(Thunk, Top)`／`(Top, Thunk)`
   保 thunk 原樣；只有真正需要值做合併判斷時才 force。
   **警告**：route-A memo 的三守衛（exact-only／nondet 旁路／代際重置，
   `5e32a17`）不得弱化；thunk 算 exact（未求值≠部分）。
3. **管道綁定修正**：`pipe_apply` 的轉換器臂 `unify_internal(lv, rv, ctx)` 用的是
   **外層 ctx**——急切時代無差異，惰性化後這是錯的：合併途中 force 到的開放
   thunk 必須綁到**管道輸入**（P3 最近包圍演化）。改傳 `call_ctx`（`$ = lv`）。
   `pipe_laws_test` 的可加性測試會守住逐支綁定，不怕改壞。
4. **evolve 存 thunk**：`universe.evolve` 移除 `force_recursive`（存開放項——P3
   「開放項可儲存」的落地）；`universe.observe` 改為對**回傳值** `force_recursive`
   （掛 fuel；REPL 慣例觀測回傳值，互動體驗不變）。commit 序列化 thunk
   （`Value` 已 derive Serialize，驗證 roundtrip）。
5. **可見差異（合同內）**：`#no_context` 的坍縮時點從 evolve 移到 observe——
   `context_dollar_test` 的兩個 P3 測試**觀測結果不變**（仍 `_|_ #no_context`），
   但如果它們斷言了中間狀態需微調（允許，需在回函註明）。其他 24 項定律
   測試**原樣通過**。

**Stage 2 驗收**：26 項合同測試綠（P3 兩項允許註明微調）；新增測試：
(a) `w: { x: $ }` evolve 後 root 內 `x` 是 Thunk（開放項真的存了）；
(b) 觀測 `w.x` → `_|_ #no_context`；(c) commit→reload 後 thunk 行為不變。

## 4. Stage 3 前的必讀警告：綁定傳播是未決設計題

銜尾蛇向量 `v: <<_.>> |> <<_.>>`／`v.w.x.a → "Logic"` 需要一個 Stage 2 **沒有**
的機制：v 的管道因 CAID 早退**從未 force** `w.x`，事後觀測 `v.w.x.a` 時，`$` 的
綁定必須「穿過 v 的演化史」到達 x。三個候選方案（**不得自行擇一實作**——
先寫回函比較，使用者裁決）：

- **A 蓋章（stamping）**：管道結果內的開放 thunk 深走標記 `context = Some(lv)`。
  簡單；代價＝深走成本＋失去「觀測時」綁定的純度（變演化時綁定）；快照語義
  （v 綁的是 evolve 時的 root，不含 v 自身 → 全量觀測**有限**，與規格的
  fuel-視界宣稱不符）。
- **B 綁定包裝值**：新 `Value::Bound { inner, context }`，導航透明下降並在 force
  內層 thunk 時注入綁定。免深走；代價＝新 Value 變體波及所有 match（編譯器會
  逼你走遍）；快照語義同 A。
- **C 活引用晚綁定**：`<<_.>>` 求值為對 root 的**符號引用**（結構態不坍縮的字面
  意思），v 自身也存成 thunk；觀測 `v.w.x.a` 時才展開管道，屆時 root 已含 v
  （自指成立）→ 全量觀測自然撞 fuel 視界 ✓ 與規格宣稱完全吻合；代價＝最深
  （需要「引用」值與環判斷），且 v 的每次觀測重跑管道（除非配 memo）。
- 回函需比較：三案對規格三句話（`v.w.x.a` 有限／`v` 全量 → fuel／CAID ＝ 含 `$`
  的語法幾何）的滿足度、實作成本、與 Stage 4 memo 的相容性。

**Stage 3 驗收（裁決後）**：GUIDE_03 §11.1 全向量——`s`／`w`／`w.x → #no_context`
／`v.w.x.a → "Logic"`／`v` 全量觀測按裁決方案的預期行為。

## 5. 非目標（越界即打回）

- **不做 memo／DAG**（Stage 4+，另案；019 tier 調度屆時才進場）。
- 不改 parser、不改語法、不動 SPEC／SYNTAX（發現規格歧義 → 記回函）。
- 不改 REPL 介面與 CLI。
- 不「順手」優化 unify 的其他路徑——force 迴避以合同測試通過為限。

## 6. 風險與回退

- 每階段一個 commit（`local` 分支），訊息標 Stage N；壞了整階段 revert。
- 全語料是行為合同：integration harness（`tests/unit/`）對固化時點敏感的檔案
  若出現差異，先判斷是「時點後移的合法差異」還是真回歸——前者記回函，
  後者修引擎，**不改語料遷就引擎**。
- 效能不是本案驗收項，但若全套測試時間倍增，記數字進回函。

## 7. 回報格式

同 linter 案：提交於 nlang-tools `local`、回函寫（a) 各階段狀態 (b) Stage 3 三案
比較與建議 (c) 語料差異清單 (d) 規格歧義清單。回填 `nlang-spec/meta/ROADMAP.md`
增量收斂行與 `meta/ENGINE_SYNC.md`（#16 惰性半邊條目）。

---

## Stage 3 工單補遺（2026-07-07 裁決：**C 案，活引用晚綁定**）

規範性依據已上架：**SYNTAX_07 §2.4／SPEC_04 §1.2**（路徑運算元的結構態＝符號引用；
nlang-spec `6ca72d5`）。以下為施工細則。

### 3-pre｜混血 collapse 修復（阻斷點，先做、單獨 commit）

驗收探針發現（潛伏 bug，已驗非 Stage 2 回歸）：`collapse()` 與 `navigate_segments`
的 `%val` 拆包是貪婪的——見 `%val` 就拆、丟掉兄弟欄位，把原子演化的**混血 combo**
（`{a:"Logic", %val:"Logic"}`，SYNTAX_06 混血節點，合法值）壓成原子。後果：
`s: "Logic" |> { a: $ }` 後 `observe s.a → _|_ #invalid_path`——正典向量第一行斷。

**修法**：`%val` 拆包只對**純包裝**生效（除 `%val` 外無 data／rules／types 欄位；
force 的效應升級包裝、`%morphism` 定義包裝等純包裝不受影響）。兩處同修
（`Value::collapse`、`navigate_segments` 的 while-unwrap 迴圈）。
**驗收**：`observe s.a = "Logic"`；全語料綠——若有測試依賴混血被壓平，逐一判定
「合法差異 vs 回歸」記回函，不改期望值遷就。

### 3a｜引用值

- 新值種 `Value::Ref(Path)`（或等價；serde roundtrip；加變體會被編譯器逼著走遍
  所有 match——這是保護不是負擔）。**CAID ＝ 路徑的語法幾何**（不含任何綁定）。
- `Structural(expr)` 求值：expr 為 `Path` → `Ref(path)`；**非路徑運算元沿用現行
  幾何本體語義**（SYNTAX_07 §2.2），不動。
- force／navigate 遇 `Ref`：解引用 ＝ 對 `ctx.root`（**觀測當下**）resolve 該路徑；
  fuel 在解引用點記帳（force ＝ 觀測原語，GUIDE_03 §11.4）。

### 3b｜管道晚綁定

pipe 運算元含 `Ref` 時不在 evolve 期展開——整條 pipe 表達式存 thunk（Stage 2
機制現成）；觀測時 force 才展開，屆時 root 已含 v 自身 → 自指成立。

### 3c｜環與視界

解引用鏈的自指**不設特殊環偵測**——`%fuel` 視界即語義截斷（SYNTAX_07 §2.4 明文
「視界是語義截斷非錯誤」）；全量觀測撞視界回 `#fuel_exhausted`。`MAX_REFINE_HOPS`
先例僅供參考，**不要**複製 visited-set 邏輯到解引用（那會把語義截斷變成錯誤路徑）。

### Stage 3 驗收（終極向量全文）

```nlang
s: "Logic" |> { a: $ }     ;; observe s.a = "Logic"（3-pre）
w: { x: $.s }               ;; observe w.x → _|_ #no_context（Stage 2 已綠）
v: <<_.>> |> <<_.>>         ;; evolve 不炸、存 thunk
_: v.w.x.a                  ;; → "Logic"（路徑導向，有限步）
_: v                        ;; 全量 → #fuel_exhausted（自指迴歸至視界）
```

＋合同 26 項綠＋全語料綠。**非目標**：memo（Stage 4）；快照語法（若需快照另議，
勿發明語法）。

---

## Stage 3 驗收記錄（2026-07-07，回函不通過——有條件收下）

**判定：3-pre ✓、3a 部分、3b ✗（已代修）、3c ✗（已代修）；終極向量 4/5 兩行
交付時皆失敗。** 回函宣稱的 88 suites 綠屬實，但交付的 6 支測試全走 `oo.eval`
直呼，**沒有一支通過 `universe.evolve`**——合同繞過了真正的儲存路徑，恰好漏掉
向量中最難的兩行（4：`v.w.x.a`；5：`v` 全量）。

### 驗收探針發現（stage3_probe_test.rs，`db83eb8`）

1. **A 案快照從 unify 缺口漏回**（已代修）：`unify_internal` 只有 `(Top,Thunk)`
   保留 arm，沒有 Ref 對應——evolve 欄位合併 `unify(staged, {v: Ref})` 落到
   force 路徑，而 pub `unify` 用 engine 層 context（root＝純淨系統根），Ref 在
   evolve 期被解引用成**純淨根快照**（staged v = Combo、無 s/w/v、observe ctx
   fuel 零消耗，三證齊全）。修法：`(Top,Ref)/(Ref,Top)` 保留 arm。
2. **CAID 段界碰撞**（已代修）：Ref 的 `content_hash` 直接串接 segments，
   `<<a.bc>>` 與 `<<ab.c>>` 同 hash——CAID 相等正是 lazy-unify 早退的判等依據，
   健全性問題。修法：長度前綴分隔。
3. **自指全量觀測爆棧崩潰**（已代修）：`force_recursive` 遞迴不參與 depth 記帳、
   根解引用只收 1 fuel——「視界即截斷」在工程上沒接住，Rust 棧先死。修法：
   `force_recursive` 進出 depth 記帳（深度耗盡＝同一種語義截斷）＋解引用計價
   （根解引用 32、帶段路徑 1+len）。

### 整治工單（Stage 3-fix，待派）

- **F1（語義，主件）**：deref 再入須供 `$` 框架——解引用值成為後續導航／強制中
  自由 `$` 的動態綁定框架（活引用＝觀測再入；SYNTAX_07 §2.4「綁定發生於觀測時」
  的第二半）。實作備註:綁定屬於**解引用的下游子樹**，不得污染兄弟欄位（勿直接
  改 observer ctx 的 `context_value`；在 `navigate_segments`／`force_recursive`
  的 deref 分支下帶局部框架）。驗收＝拿掉 `stage3_probe_test.rs` 中
  `probe_v_w_x_a_yields_logic` 的 `#[ignore]` 後綠。**注意方向性**：直接
  `observe w.x` 必須維持 `#no_context`（向量第 2 行不得回歸）——綁定只在
  「穿過 deref」時成立。
- **F2（資源）**：stdlib 重根下自指全量觀測 OOM（SIGKILL）——每循環深拷貝整個
  宇宙,記憶體在視界觸發前耗盡。查明增長階數（疑似 memo 或 sub_context 克隆放大）,
  目標:視界觸發前記憶體有界。探針以極簡根通過,回歸時把
  `build_universe_with(dir, false)` 變體納入。
- **F3（衛生）**：Ref 與非 Top 具體值的 unify 仍走 force 路徑（evolve 期語境下
  ＝快照）——現階段合法路徑不觸發,但需明文:或延遲（存 Thunk）或明確規格裁決。
  回函標記即可,不必實作。

### 對照（模型評估參考,應 user 要求記錄）

Stage 1+2（前一 agent）:兩次回函、一次設計升級（A/B/C 敢問）、自證合同 26 項
含 evolve 全路徑、hash 缺 context 分量是我 armchair 指認後它修的。
Stage 3（本次模型）:一次回函、量詞誠實（「88 suites 綠」屬實）、程式碼局部
品質佳（is_pure_wrapper 乾淨、bn_serial 的 LEB128 正確）——但**測試設計迴避了
整合路徑**,六支測試全繞過 evolve,終極向量最難兩行無測試也無說明;變體加進
enum 後靠編譯器掃 match 的機械部分做全了,**跨模組的語義閉環（unify 缺口）沒想到**。
特徵:單元級精確、整合級盲區;回函不撒謊但也不主動暴露未覆蓋面。

### Stage 3-fix 驗收（2026-07-07 第二輪，通過）

**F1 ✓ 實測**：`v.w.x.a → "Logic"`（probe unignore 綠）；直接 `w.x → #no_context`
維持（stage2 12+3 綠，方向性守衛成立）；實作品質佳——兩處（`resolve_path_internal`
／`force_recursive`）皆 `is_ref` 判定 → force → `context_value` save/restore，
作用域限定 deref 子樹。全 workspace 598 過 0 敗。**F2**：根因定錨正確
（`sub_context` 每次 thunk force 深拷貝 root，O(depth×N×|root|)）；Arc<ComboVal>
改造另案排隊。**F3 ✓** 衛生筆記落在 unify.rs 現場。

**殘留一則細節（併入 F2 輪或 Stage 4，非阻斷）**：全量 `observe v` 的 fast path
（`resolve_path` 單段臂）在 force_recursive 之前就消耗掉 Ref，第 0 層欄位因此
無 `$` 框架（第 1 層起、即穿過內嵌 v 的子樹有）。現語義「observe v ＝ 對 root
的外部觀測」尚可自圓,且全量觀測撞視界為主行為;若日後裁定「透過 v ＝ 全樹再入」,
在單段臂補同款 scope 即可。

**銜尾蛇向量五行全綠——惰性化完成定義達成。** Stage 4（memo/DAG，019 tier 策略）
與 F2 Arc 改造為後續兩條獨立工線。

### F2 結案（2026-07-07，`97a0bcd`）——根因更正

量測推翻了兩層敘事（agent 的、和我上一輪驗收背書的）：

1. **OOM 真兇＝驗收探針自身的 `contains_horizon` 助手**——`all_fields_iter()`
   產擁有權克隆，對視界深度（~253 層）嵌套鏈遞迴＝O(depth×size)，峰值 15.6GB
   → SIGKILL。改按引用遍歷後即解。
2. **引擎側（含 pre-Arc）本就正確**：stash 對照實驗——舊引擎＋修好的助手，
   1.24s／峰值 435MB 通過。視界在 ~253 deref／depth 251 正確觸發，observe
   返回有界 ~256 層鏈。`sub_context` 深拷貝之說未經量測即寫入回函，證偽。
   （我上輪驗收寫「根因定錨正確」同樣未經量測——同罪，記此為誡：**回函裡
   「根因」二字必須附量測**。）
3. **`Arc<ComboVal>` root 保留**＝真實效能收益（sub_context 在每次 thunk force
   克隆 ctx，root 從全宇宙深拷貝變 refcount）；「測試 mutate root 受阻」實為
   path_test 4 行 `Arc::make_mut` 收尾。
4. 新驗收探針 `probe_v_full_stdlib_root_hits_horizon_no_oom`（stdlib 重根＋
   寬棧執行緒）。599 綠。

**Stage 3 全線結案。** 剩餘工線＝Stage 4（memo/DAG，019 tier 策略）、快照語法
（另案）。全量自指觀測的 256×|root| 具現化成本屬 Stage 4 視界計價議題。

---

## Stage 4 工單（2026-07-07 發出）：觀測 memo——019 tier 策略落地

**目標**：重複觀測不重算。`universe.observe` 是 `&self`（結果不回寫），同一 thunk
在每次觀測都重新 force——這是現在最大的重複成本。依 GUIDE_03 §11.3 的 memo 藍圖
＋ 019 tier 分級實作 **force 層 memo**。Route A（unify memo）已存在且已加固，
本工單不動它。

### 預裁決（工單內定案，勿重開）

1. **Tier C 的「永久 memo」收窄**。linter 的 C ＝ $-free（`classify_tier` R1），
   但 C 可含 Path——`x: a.b` 讀宇宙，永久 memo 不健全。§11.3 的「C 永久」只對
   **C₀（$-free 且 path-free 且 structural-free）**成立。Stage 4b 不單獨優化 C₀：
   **統一 key 形狀**（見 3），C₀ 的 root-free 永久化留 Stage 5。
2. **無新的 epoch 狀態**。M/C-with-paths 的失效不引入 `root_epoch` 計數器——
   **key 直接含 `ctx.root` 的 CAID**（內容尋址即世代；root 換代 → key 全 miss →
   自然重算。`cache_id` 快取使 root hash 攤平為 O(1)）。
3. **Memo key ＝ (expr CAID, frame CAID, context CAID | #open, root CAID)**。
   expr CAID 用 bn_serial 的正準 Thunk 序列化（Stage 2 已修 context 分量，直接
   複用）；frame ＝ closure Vec<ComboVal> 逐一 hash；context 無綁定時用 `#open`
   哨兵（P3：開放項身份含洞）。
4. **策略按 tier**：C、M → memo（key 如上）；**Q、U → 不跨觀測 memo**（單次觀測
   內的局部共享不做，留 Stage 5）。tier 判定：force(Thunk) 時對 expr 跑
   `classify_tier`（O(expr) 走訪，相對 eval 便宜；快取到 Thunk 結構留後續）。
5. **§11.3「fuel 代」簡化並記錄理由**：因為**不插入 Blur/Bottom**（見 6），任何
   已插入的結果都是在某視界下完整算完的——單調性下與視界無關，root CAID 分量
   已足。§11.3 的 fuel-generation 條款對應「快取截斷結果」的世界，本實作不進入。
   在 GUIDE_03 §11.3 加一行註記此簡化（規格側 commit）。
6. **Route A 三守衛在 force 層同款複製**：不插入 `Bottom`／`contains_blur()`；
   effect ≥ NonDet 完全旁路（查與插都旁路）；容量上限＋代際清空（`100_000` 同
   unify memo）。
7. **`force(Ref)` 絕不 memo**——deref＝觀測原語，活引用語義（C 案）依賴每次
   對當下 root 解引用。（Thunk 體內含 Ref 者無妨：其結果內嵌 deref 產物，key 的
   root CAID 分量涵蓋。）

### 施工順序

- **4a（機械，先行、單獨 commit）**：`classify_rhs`／`has_free_dollar`／
  `classify_tier`／`Tier`／`RhsForm` 從 `crates/oo/src/nlint.rs` 搬至
  `nlang-parser`（建議 `src/tier.rs` 模組）；`oo::nlint` 改 `pub use` 再匯出，
  外部 API 不變（相依方向：oo→interpreter→parser，engine 要用分類器只能放
  parser）。驗收：全 workspace 綠＋`oo lint` JSON 輸出對一份語料 golden 比對
  不變。
- **4b（memo 核心）**：`Ouroboros` 加 `force_memo: RwLock<HashMap<MemoKey, Value>>`；
  hook 僅在 `force()` 的 Thunk 臂（查→miss→eval→按 tier/守衛插入）。
- **4c（驗收探針，fuel 即觀測量）**：
  1. 同宇宙同路徑觀測兩次：第二次 fuel 消耗 **嚴格更少**（memo 命中），值 CAID
     相等；
  2. 兩次觀測之間 evolve 任一欄位：memo 全 miss，重算結果反映新 root（**活引用
     語義不得被 memo 破壞**——這條是紅線探針）;
  3. Q-tier expr（如 `$ == 5` 分支體）兩次觀測 fuel 不減（不跨觀測 memo）;
  4. memo_soundness_test 擴展：force 層 Blur/Bottom/NonDet 不插入。
- **合同**：pipe_laws（13）＋ context_dollar（12）＋ stage2（3）＋ stage3 probes
  （4，含 stdlib 重根）原樣綠（§11.6：定律與求值策略正交）。全語料綠。

### 非目標

Route B 每座標 dirty-DAG（Stage 5）；C₀ 永久化＋KV 持久化（§6.2，Stage 5）；
單次觀測內 Q 局部快取；快照語法；linter 行為變更（4a 是搬家不是改動）。

### 回函要求

按慣例：宣稱附量測（尤其探針 1 的 fuel 前後值）；設計偏離預裁決 1–7 任一條
須升級不得自行改；「根因」二字必須附量測。

### Stage 4 驗收記錄（2026-07-08，有條件收下後代修通過）

**交付**（`8a00b4d`＋`2b2e524`）：4a 搬家乾淨（oo re-export、JSON golden 不變）；
4b 架構正確（key 四分量、tier 策略、三守衛、Ref 不 memo）；4c **工單開四支探針、
交付三支——缺的兩支正是紅線**（同一模式第三次）。

**紅線探針結果**：
- B（evolve 失效）：交付即過——root CAID 分量正確。
- A（F1 框架×memo）：**FAIL，soundness bug**——key 的 context 分量僅取
  `thunk.context`，漏了觀測方 `ctx.context_value`（F1 deref 框架、pipe 綁定
  皆走此分量）。`v.w.x.a` 之後直接 `w.x`：同 key 命中，框架綁定的 "Logic" 被
  端給開放觀測（應 `#no_context`）。**修法（`8229676`）**：有效綁定
  （`thunk.context ∨ ctx.context_value`，與 §11.2 綁定規則同式）一次計算、
  key 與 call_ctx 共用——兩者從此不可能分歧。

**代修另兩項**：root CAID 原每 force `(*ctx.root).clone()` 深拷貝＋全量重算 →
`EvalContext.root_caid()` 惰性快取（每 root 版本一次，隨 sub_context 克隆傳播）；
還原被交付刪除的 P1/P3 綁定規則註解。

**記錄紀律**：交付側 spec 提交 `7c2b1db` 為**空提交**（記錄僅在 commit message；
16c 列、§11.3 實作註記〔預裁決 5〕皆未落檔）——驗收方補建（`b95d292`／`98a9e4c`）。

`stage4_redline_test.rs` 入永久套件。604 綠。**Stage 4 結案。**
殘留工線＝Stage 5（C₀ 永久化＋KV 持久化、Route B 每座標 dirty-DAG、Q 單觀測
局部快取）、快照語法（另案）。

---

## Stage 5 工單（2026-07-08 發出）：Route B——每座標失效與 C₀ 永久層

**目標**：Stage 4 的 memo 以 root CAID 為世代——**任何** evolve 全滅所有條目。
Route B 把失效精確到座標：無關座標的演化不再打掉快取。這是「增量收斂」的
本體(GUIDE_03 §4 Route B;DAG 節點=thunk 圖,早已確立與惰性引擎同源)。

### 紅線探針已預先寫好並提交（`019c40c`,`stage5_redline_test.rs`）

**驗收＝拿掉 R1/R3 的 `#[ignore]` 後全綠;R2 全程保持綠。**刪除或弱化探針＝
違反工單,有異議升級回函,不得自行改探針。基線已校準:R1/R3 現以正確理由
失敗(root CAID 全滅),R2 現行通過(失效正確性,過渡期間不得破)。

- **R1**:無關座標 evolve 後,memo 命中仍在(fuel 遞減保持)。
- **R2**:被讀座標 refine 後,觀測值反映新 root(失效不得漏)。
- **R3**:C₀ 條目(無 root 讀)在任意 evolve 下存活。

### 預裁決（勿重開）

1. **依賴收集**:`EvalContext` 加 `dep_collector: Option<HashSet<String>>`。
   記錄點=root/staged 座標讀取處:`resolve_path` 的 root/staged 查找、
   `resolve_path_internal` Bare 臂命中、`force(Ref)` 解引用。粒度=**頂層座標**
   (路徑首段);`<<_.>>` 根解引用與 Current/Parent 錨=**萬用依賴 `"*"`**
   (保守過度失效合法;自指鏈因此每 evolve 必失效——正確)。
2. **巢狀傳播**:force(Thunk) miss 時在 `call_ctx` 裝新收集器;eval 完把
   內層 deps **併回外層收集器**(外層結果內嵌內層結果,依賴要跟著上浮),
   與 `ctx.fuel = call_ctx.fuel` 同點處理。
3. **條目形狀**:`{ value, deps: HashSet<String> }`;key **移除 root_caid 分量**
   (失效改由 deps 承擔)。`deps = ∅` ⟹ 天然 C₀(R3 免費得證——不需要
   單獨的 C₀ 分類器;`is_c0` 靜態判定不做,運行時空依賴即永久)。
4. **失效=推送式**:`Ouroboros::invalidate_coords(&[String])`+反向索引
   (coord → keys;`"*"` 條目單列一份,任何失效都清)。呼叫點:
   `Universe::evolve`(座標=本次寫入的 field key)。**commit/load/refine =
   全清**(粗事件,直接 `force_memo.clear()`)。
5. **staged 閘門**:memo 查與插一律加 `ctx.staged.is_none()` 條件。理由:
   evolve 期 force 讀 staged,而 key 無 staged 分量——跨 staged 狀態可髒命中
   (Stage 4 遺留的窄縫,防禦性關閉)。附探針嘗試義務:能構造出髒命中就提測試,
   構造不出則閘門作為縱深防禦留下並在回函註明嘗試過程。
6. **有效綁定分量不動**(Stage 4 驗收修復`8229676`):key 的 context 分量
   繼續=有效綁定;R2 之外,`stage4_redline_test.rs` 兩支全程必綠。
7. **KV 持久化不做**(§6.2 引用照舊):C₀ 值重算便宜,跨進程快取的版本失效
   問題(引擎語義變更)成本>收益;規格側不動。**Q 單觀測局部快取不做**。

### 施工順序

5-pre(staged 閘門+嘗試探針,單獨 commit)→ 5a(dep_collector+記錄點+巢狀
傳播)→ 5b(條目改形+反向索引+invalidate_coords+evolve 掛鉤+全清事件)→
un-ignore R1/R3。

### 回函要求

宣稱附量測(R1 的 cold/warm/after 三值必列);偏離預裁決升級;「根因」附量測;
**記錄提交必須是非空檔案變更**(上輪 `7c2b1db` 為空提交、記錄只存在於 commit
message——`git show --numstat` 自查後再回函);ENGINE_SYNC 16d/ROADMAP/
GUIDE_03 若有規格面偏移一併落檔。

### Stage 5 驗收記錄（2026-07-08，通過——代修一實測洞＋兩縱深防禦）

**探針預置制度首戰生效**：R1/R2/R3 un-ignore 全綠，交付未跳過任何紅線——
成本結構反轉起作用了。

**驗收實測（dep-trace 儀器化）**：
- **Route B 核心真實工作**：transformer thunk 於觀測時收集 `{t}` → 反向索引 →
  `invalidate_coords` 精確移除 → 重算新鮮。R2 是真陰性。
- **實測洞（代修 `fe4522b`）**：engine 內部語境（`eval_context`——pub unify 的
  合併強制）以**純淨系統根＋無收集器**參與 memo，插入 deps=∅ 偽永久條目
  （refine 期條目擾動 4→6；加閘後純移除 4→3）。修=`EvalContext.memo_enabled`，
  `eval_context()` 關閉，查/插雙閘。
- **兩項縱深防禦（誠實標注）**：HIT 上浮 entry.deps（架構必需；**三次對抗構造
  皆無法武裝失效**——快取值內嵌未 force 的 thunk、內層條目獨立失效，固化窗口
  比模型窄。probe 留作迴歸守衛，反事實不武裝已註記檔內）；Named-prefix evolve
  雙形式推座標（實測 `/name:` 走 Quoted/Path 臂）。
- **順帶確立的語義事實**：封閉路徑欄位（`inner: t.flag`）＝evolve 期幾何拷貝，
  觀測不重讀——C 案邊界的引擎面驗證（活讀取必須 `<<t.flag>>`）。

**記錄紀律**：spec 交付提交 `e6c8e7d` **再次為空**（工單明文自查要求後仍違反）。
16d 列驗收方補建（`773a7e3`）。

**GUIDE_03 的增量收斂三路線至此**：Route A（unify memo，加固）＋惰性基底
（Stage 1–3）＋ Route B（每座標失效，本階）＝全線落地。殘留＝快照語法（另案）、
`Atom(Top)` unify bug（queue）。609 綠。
