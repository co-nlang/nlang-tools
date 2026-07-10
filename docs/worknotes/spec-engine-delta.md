# Spec-Engine Delta：2026-05-18 規格修改對引擎的影響

本文件記錄 `nlang-spec` 在 P0–P2 修改後，與 `nlang-tools` 當前引擎實作的差距。
基於 `implementation-status.md` 的 Phase 1–4 路線圖，標註新增/變更的項目。

---

## 1. 新增規格項目（文件中有但引擎完全沒有的）

| 項目 | 規格位置 | 引擎影響 | 優先級 | Phase |
|:---|:---|:---:|:---:|:---:|
| CAID v2 格式 `<masa_ref>` | REAL_03 §2.2 | CAID 字串格式變更 | **P0** | 1a |
| 複數譜量化與編碼 | REAL_03 §3.2, APP_05 §3.5 | Spectral sketch 從實數改為振幅+相位交錯編碼 | **P0** | 1a |
| 相位感知合併（$\varepsilon_{coherent}$） | REAL_03 §4.1, SPEC_06 §1.3.1 | `&` meet 新增三路決策邏輯 | **P0** | 1b |
| `%obstruction_degree` 標籤 | SPEC_06 §1.3.1 | meet 衝突時新增 #h1_phase / #h2_sign 等輸出 | P1 | 4 |
| `%cause` 上鏈格式 | SPEC_06 §1.3.2 | 衝突記錄新增 `%degree`, `%cocycle`, `%holonomy` | P1 | 4 |
| `/%differential.{1,2,3}` 態射 | SPEC_07 §2 | 新增 3 個內建態射 | P1 | 4 |
| `~%Engine./project_down`, `/project_up` | SPEC_08 §3.5 | 新增 2 個內建操作 | P1 | 6 |
| LADD GBB `nerve_structure` 欄位 | APP_05 §2.2 | AdvertiseGeometry 新增可選欄位 | P2 | 7 |
| MASA 前置過濾 + 神經感知路由 | APP_05 §4.1, §4.3 | 路由新增 MASA 相容性檢查階段 | P2 | 5, 7 |

---

## 2. 既有路線圖的變更點

### Phase 1（CAID Infrastructure）— 大改

原計劃：
```
BN/ serialization → Lattice Sketch → CAID v1 → genesis seeds
```

新計劃：
```
BN/ serialization → Complex Lattice Sketch (振幅+相位)
    → CAID v2 (masa_ref + sketch_ℂ + digest)
    → MASA seed CAIDs (masa_ref = "_")
    → 相位感知合併邏輯
```

具體變更：

| 原項目 | 新狀態 | 說明 |
|:---|:---:|:---|
| CAID 格式 `hash:<algo>:v1:<sketch>:<digest>` | **變更為 v2** | `hash:sha256:v2:<masa_ref>:<sketch_ℂ>:<digest>` + 保留 v1 作為創世 |
| Lattice Sketch（純實數） | **改為複數譜** | 振幅 + 相位兩路 Delta 編碼 |
| Genesis seeds | **新增 MASA seeds** | 基底 MASA 的 CAID 使用 `<masa_ref> = _` |
| — | **新增** 相位合併 | `&` meet 新增 $\varepsilon_{coherent}$ 判斷 |

### Phase 2（Standard Library）— 小改

| 變更 | 說明 |
|:---|:---|
| EML 運算子優先級**不變** | 仍為重要項目，跨版本相容性依賴 EML |
| Genesis defaults **不變** | `%fuel`, `%max_branches` 等參數仍需 |

### Phase 3（Refinement）— 無變更

`#refine` 機制不受 P0–P2 影響，繼續按原規格實作。

### Phase 4（LADD Basics）— 中度改動

| 原項目 | 新狀態 | 說明 |
|:---|:---:|:---|
| 引力路由權重計算 | 不變 | $W_i$ 公式仍然有效 |
| 譜距離計算 | **改為複數版 $d_L^{\mathbb{C}}$** | 新增 MASA 參考系投影 $\hat{P}_B = \Pi_M P_B \Pi_M^\dagger$ |
| — | **新增** MASA 前置過濾 | 路由第一級：檢查 MASA overlap |
| — | **新增** 神經感知路由 | 選用 `nerve_structure` 加速過濾 |

---

## 3. 已達成的規格項目（引擎無需實作）

下列項目為純文檔/理論性質，不影響引擎程式碼：

| 項目 | 位置 |
|:---|:---|
| Čech 神經形式化 | APP_04 §2.3 |
| $L_r \times E_r$ 矩陣 | SPEC_00 §4.2 |
| $H^1$/$H^2$ 非分配性區分 | SPEC_01 §2.5.1 |
| 微分態射語義定義 | SPEC_07 §2（引擎僅需實作 `/%differential.2` 分支） |

---

## 4. 引擎實作建議進度（更新版）

```
Phase 1a: BN/ + 複數譜 + CAID v2 字串             (2-3 週)
Phase 1b: ε_coherent 相位感知合併                    (1 週)
Phase 1c: MASA seed CAIDs + genesis                  (0.5 週)
──────────────────────────────────────────────────
Phase 2:  標準庫補完（EML、genesis defaults）          (1-2 週)  ← 可並行
Phase 3:  #refine 機制                                (2 週)    ← 可並行
──────────────────────────────────────────────────
Phase 4:  %obstruction_degree + %cause 上鏈           (1 週)
         + /%differential.{1,2,3} 態射               (內含)
──────────────────────────────────────────────────
Phase 5:  LADD 基礎（含 MASA 過濾 + 複數譜距離）        (3-4 週)
Phase 6:  %project_down / %project_up                 (1 週)
Phase 7:  LADD 神經感知路由                           (1 週)
```
