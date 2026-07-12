# 工單:bare 前向引用解析(演化序不敏感)

> 2026-07-12 開單。出處:L2-17 驗收遺留(docs/divergence_cause_handover.md
> 驗收記錄「遺留」節)。語義依據:SPEC_03 合併交換律——one-shot 程式的
> 欄位是**同時性**的,觀測結果不得依 evolve 順序而變。

## 症狀(開單時已量測,兩處、兩層)

1. **引擎層——bare-path 引用鏈誤殺 `#divergent`**:
   `out: mid` / `mid: base` / `base: 1` 觀測 `out` → `_|_ #divergent`(應 `1`)。
   4-hop 同病。**數學形鏈無此病**(`out: mid + 1` / `mid: base + 1` → `3` ✓)。
   根因(開單方 diff-read 判讀,交付方須以量測覆核):L2-17 的 path 環守衛
   把 thunk **指向的路徑**(`path_coord_of(expr)`)插入 `computing`,
   而 `force_coord` 用**被 force 的座標名**查同一集合——`out: mid` 在 force
   out 的 thunk 時標記了 "mid",隨後查名 `mid`(本身是具體化 Thunk)進
   `force_coord("mid")` 即誤中。引用(thunk 住在 out、指向 mid)≠ 自環
   (`s: { v: s.v }`:thunk 住在 s.v、指向 s.v)。
2. **CLI 層——`oo run` 前向引用全滅為 `_`**:引擎內部(evolve+observe API)
   前向引用**今天就通**(`out: a` / `a: 5` → `5`;雙前向、前向態射、容器內
   前向、跨 refine 全綠)。`run_one_shot`(crates/oo/src/main.rs)在**每欄
   evolve 後立刻 observe 該欄**(store-put 迴圈,「方便 CAID 引用」),把
   具體化 Thunk 在後續欄位落地前固化成 `_`。跨檔前向(`oo run a.n b.n`)同病。

## 裁決

1. **同時性法則**:one-shot run 的全部檔案、全部欄位 = 同一宇宙快照。
   前向/後向引用同義;欄位順序重排不得改變任何觀測結果。
   (conformance 新向量 L1-26/27 已入 spec 語料庫,收案 = **47/47**。)
2. **引用 ≠ 環**:path 環偵測應 key 在 thunk 的**居住座標**(holder),
   不是它指向的路徑——或等價機制,交付方選,但:
   - 真環**全部**維持 `#divergent`(⊥ 側活釘已釘死:純引用環 `a: b`/`b: a`、
     經數學引用環、互指環、path 自環 `s: { v: s.v }`——最後兩者在
     divergence_probe_test,同須全綠);
   - 生產性遞迴(factorial)、深有限鏈照舊(既有活釘)。
3. **CLI 修法**:store-put 迴圈移到**全部 evolve 完成之後**(目的——值入
   Store 供 CAID 引用——必須保留,不得刪除);或等價做法(如唯讀觀測)。
   多檔 = 先全檔全欄 evolve、再統一 store-put、再 `--observe`。
4. **範圍限定 one-shot**:REPL 是互動序列,逐步語義合理,**不動**;
   `oo evolve`(持久 staged)亦不在本單。若交付方認為它們也該改,停手回報。

## 探針(已預置,兩檔)

- `crates/interpreter/tests/forward_ref_probe_test.rs`:**2 紅線**(3-hop/
  4-hop bare-path 鏈)+ **9 活釘**(含 ⊥ 側承重釘:`pin_ref_cycle_still_divergent`
  ——修鏈誤殺不得放走真引用環)。
- `crates/oo/tests/forward_ref_cli_probe_test.rs`:**3 紅線**(CLI bare 前向/
  鏈/跨檔)+ **4 活釘**(CLI 後向、跨檔後向、互指環 #divergent、欄內衝突
  #conflict 通道)。`env!("CARGO_BIN_EXE_oo")` 生二進位。
- 斷言不得動。un-ignore = 驗收門。
- 基線:workspace **731 過 0 敗 8 ignored**。期望終態:**736 過 0 敗 3 ignored**。

## 注意事項

- L2-17 全套(divergence_probe_test 10/10)交付前專跑——本單直接動它的機制。
- memo 交互:`#divergent` 已可入 force memo;修正誤殺後,曾被誤判的鏈形
  不得因 memo 殘留繼續 ⊥(留意 memo key 是否含被污染的 computing 狀態)。
- conformance:交付後自跑 `nlang-spec/scripts/run-conformance.py`,
  **47/47** 入交付記錄(L1-26 forward-ref、L1-27 forward-chain 為新向量)。
- bn_serial/fmt 不得動;顯示軸不在本單。

## 非目標

- 未定義名語義(`out: zzz_undefined` 維持 `_`,開放世界)——另議題;
- REPL / `oo evolve` 的逐步語義(見裁決 4);
- Union 去重;fuel 模型;use-before-def lint(想法 D)。

## 交付與驗收

同前單條款:非空提交、根因/宣稱附量測(反事實)、逐紅線對應、假前提
死碼掃描、既有斷言修改逐一列帳。驗收方將全套重跑(736/0/3)、
conformance 47/47 覆核、diff-read、對抗加測(欄位重排等價性、環×鏈混合、
多檔次序矩陣、store-put 目的保全)。
