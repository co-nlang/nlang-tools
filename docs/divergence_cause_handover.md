# 工單:L2-17 發散偵測 + ⊥ `%cause` 列印

> 2026-07-11 開單。出處:nlang-spec 合規矩陣(REAL_05 §3.5、`conformance/L2/17-divergence.n`)
> 與 ENGINE_SYNC「合規矩陣曝光之引擎缺口」。收案後參考引擎應達 **conformance 45/45**
> ——裸核(去 pre-release 標)的最後一塊。

## 症狀(開單時已量測)

- `a: a + 1` 觀測 `a` → **`_`**(自指在自身 force 期間查不到 → 開放項 → Top)
  ——「宣稱萬有」級靜默錯。`x: x`、互指 `a↔b` 同病。
- `s: { v: s.v }` 觀測 `s.v` → **Rust 疊爆**(2 MiB 測試線程)或 fuel 耗盡
  (64 MiB)——語義環在物理極限前未被辨識。
- ⊥ 的 `to_nlang` 只印 `_|_  ;; message`,無 `%cause` 標籤(conformance runner
  的 cause 比對因此只能 SHOULD)。

## 裁決

1. **環 = 同 thunk 在自身 force 期間被重入**(in-flight 集合,以本次觀測為界)
   → `⊥ #divergent`。**必須在物理極限(Rust 疊/fuel)之前**由語義層辨識。
2. **只有真重入算環**。三個不得誤殺的邊界(活釘全數釘死):
   - **生產性遞迴**:`/f (n - 1)` 縮參態射遞迴 = 每次新 apply,非同 thunk 重入
     (`pin_productive_recursion_factorial`,**承重釘**:factorial 5 → 120);
   - **未定義名維持 `_`**(開放世界語義;偵測 key 在 in-flight thunk,
     不在 lookup miss)(`pin_undefined_name_stays_top`);
   - **深而有限**的鏈維持 fuel 語義(`#fuel_exhausted` 不得改判 `#divergent`)。
3. **⊥ 列印**:`Value::to_nlang` Bottom 臂 → `_|_ (%cause: #<tag>)`,
   message 保留於其後(`  ;; message`)。格式循 Blur 臂先例。
   **bn_serial 位元組不得動**(顯示軸 ≠ 身分軸);若發現 `Value::to_nlang`
   餵進任何雜湊路徑,**停手回報**,不得自行取捨。
4. free-$ `#no_context`(P3)與 runaway 態射的 bottoming 行為不動
   (`pin_free_context_no_context`、`pin_runaway_morphism_bottoms`)。

## 探針(已預置)

`crates/interpreter/tests/divergence_probe_test.rs`:**5 紅線**(un-ignore =
驗收門)+ **5 活釘**。斷言不得動。helper 在 64 MiB 專用線程跑觀測
(debug 測試線程 2 MiB 不夠 eval 遞迴;沿 parser 先例)。
基線:workspace **713 過 0 敗 8 ignored**(109 套)。期望終態:**718 過 0 敗 3 ignored**。

## 注意事項

- **memo 交互(Stage 4/5)**:in-flight 追蹤不得污染 force-層 memo;
  `#divergent` 結果可入 memo(座標精化後由 Route B 失效)。
  memo/Stage 紅線套件交付前專跑。
- 既有測試若釘了舊 `_|_  ;; msg` 顯示格式,修正**僅限顯示斷言**,
  逐一列入交付記錄。
- 交付後自行跑 `nlang-spec/scripts/run-conformance.py`,**45/45** 入交付記錄
  (L2-16 的 %cause note 應消失)。

## 非目標

- 未定義名語義(開放世界 `_`)——另議題;
- fuel 模型/預設值調整;SPEC_12 遞迴驗證機制;
- runaway 態射遞迴的 cause 精化(現走 fuel 通道,合法);
- bn_serial/fmt;Union 去重。

## 交付與驗收

同前單條款:非空提交、根因/宣稱附量測(反事實)、逐紅線對應、
假前提死碼掃描。驗收方將全套重跑(718/0/3)、conformance 45/45 覆核、
diff-read、對抗加測(環×memo、環×Union、深遞迴 fuel 邊界、
display 格式×既有 golden)。
