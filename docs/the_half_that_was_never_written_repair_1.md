# Q-032 Repair Round 1

> 2026-08-16。交付 `nlang-tools a71a69b` 已提交並驗收。**未通過。**
> 本輪基線 ＝ 該交付；工單本體不變，見 `the_half_that_was_never_written_handover.md`。

---

## 0. 做對了的（不要動它們）

*   探針檔**逐位元組只少了四個 `#[ignore]`** ✓
*   本弧探針 **8/8**，含四個控制組全綠 ✓
*   **歷史列已加**：`from_roots([self.root_with_system(), Self::v0_22_standard_root()])`
    ——一個正規化後的新值，一個原始的舊值。`supports_standard_root("65f52e2d…")` 為真，
    `status` 回報 `(available)` ✓
*   新引擎**讀寫自己的倉完全正常**（新標準根 `2da5b713…`）⟹ 新規則自洽 ✓
*   conformance **143/143** ✓

---

## 1. 未通過之一：舊倉打不開（設計，已裁 O63）

〔量，兩個真二進位、未竄改任何位元組〕

```
舊引擎 oo v0.25.0（標準根 65f52e2d…）建倉並提交
新引擎 oo v0.25.0-611（標準根 2da5b713…）操作它：

  status  → Standard root dependency: 65f52e2d… (available)   ← 歷史列在
  log     → #caid_mismatch
              requested   …:0ebe51f5999f0c3e2b8d9098c7dc9a37…
              recomputed  …:b7025e4f0192dd886383ed0ed5dc2621…
  evolve  → 同上
  commit  → 同上（HEAD 未動——沒有寫壞，這一點是對的）
```

**診斷**：拆開改變了根的**位址規則**——

| | 根的位址算什麼 |
| :--- | :--- |
| 舊 | `hash(標準根 ⊕ 使用者內容)` |
| 新 | `hash(殘差 ＋ 指名依賴)` |

⟹ **持有舊的「值」不夠，還要持有舊的「規則」。**
O55 的表是 `digest → 標準根值`；本弧第一次讓它需要 `digest → 值 ＋ 讀法`。

### 1.1 裁定 O63：走格式閘，不走「規則隨標準根走」

**要做的**：`.oo/objects/` 的**容器宣告**升版；舊倉在容器層被辨識，以**舊讀法**讀取。

**不得**把讀法塞進標準根表。理由逐字：REAL_03 §6.8.2 寫著
「**新增一版＝ `from_roots([…])` 多一個元素，解析邏輯零改動**」——把解碼分支路由到那張表
會直接推翻它，且每加一代就多一條永久維護的分支。

**依據**：O23 已裁「物件編碼由 `.oo/objects/` 這個容器宣告，不由 `.oo/format`，
也不由物件自己」。〔量〕**編碼軸自 2026-07-28 引入以來從未真的被撥動過**，本弧是第一次。

**而 §6.8 第四條 MUST 從另一面預備了這件事**：

> 格式須先宣告（MUST）……使不認得本形式的引擎在**閘上**拒絕，
> 而非按舊讀法解讀出一個不同的根。

**本次失敗正是它的鏡像**：新引擎按**新**讀法，把一個舊根解讀成了一個不同的根。

### 1.2 兩層不得互相取代

*   **歷史標準根值**仍須持有（C3 看守）——那是 O55／O56。
*   **格式閘**另治位址規則——那是 O63。

做完之後，跨版本矩陣必須全綠：舊倉在新引擎上**讀得回、寫得進、位址不動**。

---

## 2. 未通過之二：21 支紅未被回報（紀律）

〔量〕`cargo test --workspace --no-fail-fast`：

```
1943 passed / 21 failed / 0 ignored（201 套件）
基線                    1956 passed /  0 failed / 0 ignored（200 套件）
```

**11 個套件紅**：

```
a_value_not_a_recipe_probe_test      effect_cached_probe_test
every_byte_or_none_probe_test        held_but_unopenable_probe_test
knob_that_does_nothing_probe_test    limit_you_cannot_choose_probe_test
local_gc_probe_test                  name_points_at_remedy_probe_test
print_what_can_be_read_probe_test    slash_shadow_cli_probe_test
snapshot_not_a_reading_probe_test    verdict_must_gate_probe_test
```

回報只說「完成」。**工單 §6.1 要求附原始輸出、§6.4 要求列出受影響測試——兩項都沒有做。**

### 2.1 處置：逐一分類，不是修到綠

**每一支紅必須落入下列三類之一，並在回報中寫明是哪一類與為什麼：**

| 類 | 意義 | 處置 |
| :-- | :--- | :--- |
| **A 預期會變** | 該測試斷言的是**本弧刻意改掉**的行為（例如「根裡有 `~%Math`」、「根位址是某個定值」） | **改測試**，並說明它原本鎖的是哪一個舊行為 |
| **B 真回歸** | 本弧不該碰它 | **改實作** |
| **C 說不清** | 你不確定是 A 還是 B | **回報，不要猜**——「未定」是合法答案 |

⚠ **A 類本身就是本弧的證據**（同 Q-030 的 `bohr_test` 四個短 digest fixture）。
**不得整批改綠而不分類**——那會把「我們刻意改了什麼」這件事一起抹掉。

以下幾支從名字看**很可能是 A**，但**仍須你逐一確認並說明**，不接受我這裡的猜測當結論：
`p1_the_root_caid_does_not_move`、`p4_root_caid_does_not_move`、
`p1_plain_commit_root_is_unchanged`、
`r3_the_root_carries_the_digest_of_system_not_its_body`。

而 `p2_refine_aborts_when_the_shadow_scan_meets_a_root_it_cannot_open`
（Q-031 的探針）**紅得可疑**——那是上一弧剛驗收過的行為，**優先當 B 處理直到你證明它是 A**。

---

## 3. 仍未交的自檢項

工單 §6 七項，回報裡缺：

*   §6.1 全跑的**原始輸出**
*   §6.3 `genesis_test` 的**新舊兩組 seed 數值**〔已知新標準根為 `2da5b713…`，
    但 26 個 seed 的新舊對照未交〕
*   §6.4 受影響測試清單（＝ §2.1）
*   §6.5 四項量測：拆開後根物件的實際形／`#cached` 固化在定義側還是觀測側／
    **`#blur` 的 CAID 是否真的含視界參數**／新舊標準根 digest

### 3.1 交付方回填（2026-08-17；Repair 1 完成）

以下是最終樹的自檢證據。第一次受限環境全跑出現的 TCP `Operation not permitted` 不列入
產品結果；最終輪以可 bind 本機 TCP 的同一棵樹重跑。

#### §6.1 全跑原始結果

本輪接單時的原始基線就是 §2 所載；Repair 1 最終輪：

```
$ cargo test --workspace --no-fail-fast --quiet
[exit 0]

$ awk '/^test result:/ { suites += 1; passed += $4; failed += $6; ignored += $8 }
       END { print suites, passed, failed, ignored }' <raw-output>
201 1964 0 0
```

完整原始輸出共 3,421 行／102,128 bytes，施工環境留於
`/tmp/nlang-q032-repair1-workspace.out`。

#### §6.3 genesis seed 對照

```
$ cargo test -p nlang-interpreter --test genesis_test --quiet
running 11 tests
...........
test result: ok. 11 passed; 0 failed
```

以 `a71a69b^..a71a69b` 的 `crates/interpreter/src/genesis.rs` 為新舊對照；26 個 seed 中 8 個因
新的標準根投影而移動，另外 18 個未變：

| seed | 舊 CAID digest | 新 CAID digest |
| :-- | :-- | :-- |
| `~%Math` | `480f9e87d91fe53267b719ddfa33522486d9fdf1bb51456892ad9804dc6b2d6f` | `cc23f6843cb0b7c0f3c0dcfe2e9917e06bf1c52a2891d9ad34670d95bd76362a` |
| `~%Discovery` | `1cfb41b083aeedd1c0acbe2c6a153809006ee30f8af6aa7b12f7aef7cb34d295` | `b65ddfc504fa3c5cc2f5b8bb8d91070b768eb96a0b6b22cbf9cfa6c6ead34e6e` |
| `~%Time` | `783cf3bba9a6c40b8c5c123fd9c19167da88b4e6ba2d6cbca5d6563644761e50` | `ca9e5210723da6734b4639b97a198e73a33e193410b20db6a906f457a8475ec8` |
| `~%Io` | `e620bfad72ec3142d4ffbb7d37955496d831e47566551a3609df61d1a47f7590` | `34b279849f01015b68bb7b41455c073b52e7aded96deec8b835b6d6c9b70da89` |
| `~%Env` | `361d79419a1f56a72f923115812360c8aee25a35f47163068223f13acfcef334` | `94e3cad273f361c3565773a3aa47580553e6e0da5c44d7cbc68b1f090e0b80fa` |
| `~%Process` | `e2720f7dd95ce94e03f1d33b724d13a16c1075dcb95b794ebe79f29c5cb25ada` | `2a984173c4d44439b7af4c09c548a53e9ac85bfc9bc63454ac3b9a4942346a35` |
| `~%Query` | `ed1e83ba547dd53732d265531fd219627d5e24bd9583f3255b8a255cac173c3c` | `8461756f5bd6cc7a35eb63021a6a82cc8734fc2a598f2fd101d9d31cabe84028` |
| `~%Csv` | `e27bde26d0e45265e5fe7e6d95828e9b1844fc7b1063b0e54334a6cd74332f8a` | `a0c4b42e10550f6c77ff36d00331e2390ac6912d9afb65913d52a73ba824c681` |

#### §6.4 受影響測試分類（最終）

| 基線失敗／受影響處 | 類 | 原先鎖住的行為 | 本輪處置／結論 |
| :-- | :-- | :-- | :-- |
| `every_byte_or_none` 的 `p1_the_root_caid_does_not_move` | A | 根 CAID 為拆開前的定值 | 根已改為殘差位址；fixture 改為新值 `7d15268e…`。 |
| `print_what_can_be_read` 的 `p4_root_caid_does_not_move` | A | 同一拆開前根 CAID | 同上，fixture 改為 `7d15268e…`。 |
| `every_byte_or_none` 的 `r5_the_store_format_says_it_changed`、`atomic_write` 的 format fixture | A | 新倉仍宣告 `encoding=3` | O63 首次撥動容器編碼軸，預期為 `encoding=4`。 |
| `a_value_not_a_recipe` 的根物件挑選 helper | A | 「最大物件」必是使用者根 | 拆開後標準根的 packed 物件可更大；改由 HEAD commit 的 `root.digest` 指到真正根。原有 11 項斷言仍全綠。 |
| `held_but_unopenable` 的 `p2_refine_aborts_when_the_shadow_scan_meets_a_root_it_cannot_open` | A（已證明） | `!contains("Combo")` 必是 commit 物件 | packed 標準根使此推論失效，導致測試把字串物件當 commit；改以 JSON `root` 欄位辨識 commit，Q-031 的 6 項斷言全綠。 |
| `knob_that_does_nothing`、`limit_you_cannot_choose` 的 plain-root literal | A | O41 後的舊根定值 `8698d297…` | O58 刻意拆根；更新為最終值 `fcfcf264…`，其餘關係斷言不動。 |
| `snapshot_not_a_reading` 的 value-only／morphism root literal | A | O42/M4 後的舊根定值 | O58 刻意拆根；更新為 `483a1b42…`／`76ae74dd…`。 |
| `name_points_at_remedy::p2_fuel_blur_caid_holds` | A | 拆根前的固定 blur CAID | §6.5 證實視界參數與根上下文在身分裡；更新 literal 為 `de65bce3…`，跨程序相等關係保留。 |
| `slash_shadow_cli::red_cli_slash_add_shadow_is_loud` | A | `/add` 與內嵌標準根衝突、必須 loud fail | O58 §2.4 明裁四個孤兒可遮蔽；改釘使用者 overlay 成功，未加名字特例。 |
| `verdict_must_gate` 的 6-object literals | A | 倉只有 3 commits ＋ 3 roots | O58 使被指名標準根成為第 7 個真物件；改釘 7/7/0，權限與 verdict 斷言不動。 |
| `a_store_you_did_not_write::r3` 的「先寫 encoding-4、再改標籤為 3」fixture | A | 改宣告即可構造舊倉 | 那會構造說謊容器；fixture 改為在第一筆值寫入前選 encoding 3，並釘讀不改宣告。真舊倉另由 §3.2 證明。 |
| `effect_cached::red_fetch_multi_active_collapses` | B | 觀測投影只需改 `ComboVal.effect` | O61 新增耐久 `%effect` 後也須在觀測側移除其耐久拼法，否則它冒充顯式欄位而被再次傳染。修後 11/11。 |
| `local_gc` 的 reachability control／clean-store test | B（含 harness 同步） | walker 只認一般 `digest` 邊 | O58 sentinel 是到標準根物件的真 CAS 邊；產品 walker 與獨立量測器都補上該邊，標準根不再被當垃圾。 |
| 真 v0.25.0 舊倉 CAID mismatch | B | 只補回頂層標準座標就是完整舊讀法 | encoding 3 還須遞迴 hydrate system table，寫入亦須走 O61 前的 legacy CAS 投影；修後見 §3.2。 |

最終沒有 C 類；上述修改後全 workspace `0 failed`。

#### §6.5 四項量測

* 根物件實際形：新根物件是使用者殘差的 `Combo`，帶有
  `__nlang_system_digest = 2da5b71371649291cfa5dc5d0cd019464d248e98645b3901938e1c08d2172c2c`
  的指名依賴；標準根本體不再內嵌於它。
* `#cached`：`solidify_effects` 是**觀測投影側**使用的正規化；`#cached` 不可被當成
  `#pure` 一併抹除，仍保有 §4.2.4 的固化語義。
* `#blur`：其 `blur_caid` 納入 `HorizonParams::encode_chs` 的 digest，故 CAID **含** fuel、
  strategy、max branches、max unification depth、max lifting、max pattern 等視界參數；
  `fuel_remaining` 不在該編碼中。
* 標準根 digest：舊 `65f52e2da48baa550d7340c0fdc214fd1f9925577a96ffec59bc34f8b2bcbe72`；
  新 `2da5b71371649291cfa5dc5d0cd019464d248e98645b3901938e1c08d2172c2c`。

補記：當前二進位版本為 `oo v0.25.0-611-g5b9a04e`。

其餘原始自檢輸出：

```
$ python3 scripts/run-conformance.py --engine /home/gali/nlang/nlang-tools/target/debug/oo \
    --corpus /home/gali/nlang/nlang-spec/conformance
143 vectors, 143 pass, 0 fail

$ cargo test -p nlang-interpreter --test genesis_test --quiet
test result: ok. 11 passed; 0 failed; 0 ignored

$ cargo test -p oo --test the_half_that_was_never_written_probe_test --quiet
test result: ok. 8 passed; 0 failed; 0 ignored
```

### 3.2 O63 真二進位跨版本矩陣

儀器：舊 `/home/gali/nlang-baselines/v0.25.0-verify-target/debug/oo`（`oo v0.25.0`）；
新 `/home/gali/nlang/nlang-tools/target/debug/oo`（`oo v0.25.0-611-g5b9a04e`）。

| 步驟 | 結果 |
| :-- | :-- |
| 舊引擎建倉、提交 | commit `4bcfdb0f…`，root `7a6a82df…`，標準根 `65f52e2d…`。 |
| 新引擎讀舊倉 | `status` 回 `(available)` 且 static；`log` 正常列出舊提交，零 CAID mismatch。 |
| 新引擎寫舊倉 | 追加 commit `f11d64fd…` 成功；容器明確遷為 `layout=2`／`encoding=3`，沒有冒充 encoding 4。 |
| 位址不動 | 舊 root `7a6a82df…` 的物件路徑仍在；新引擎 `log` 同時讀回新舊兩筆。 |
| 反向控制 | 舊 v0.25.0 再讀新引擎追加後的倉，`status` 與兩筆 `log` 仍全綠。 |

---

## 4. 完成條件（本輪）

1.  跨版本矩陣全綠：**舊引擎建的倉，新引擎讀得回、寫得進、位址不動**。
    （驗收方會用兩個真二進位重驗，不看探針。）
2.  `cargo test --workspace --no-fail-fast` **回到 0 failed**，且 §2.1 的分類表已交。
3.  本弧探針仍 **8/8、0 ignored**，該檔仍**只少四個 `#[ignore]`**。
4.  §3 四項自檢補齊，附原始輸出。
5.  `git diff` 不含任何 `spec/` 或 `meta/` 下的檔案。

**交付方最終自檢：1–5 全部達成。** 探針相對 `a71a69b^` 的唯一變更仍是四行
`#[ignore]` 被移除；Repair 1 沒有再碰該檔。`git diff --check` 通過，diff 無 `spec/`／`meta/`。

---

## 5. 紅線（不變）

*   **絕不 `git add -A`**；`git stash` 停用。
*   探針檔的修改權在驗收方。
*   工作區全跑進行中**不得改動樹**。
*   **不確定就回報**。本輪最不希望看到的是 21 支紅被無聲改綠。
