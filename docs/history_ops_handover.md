# 工單:`#rollback` + `#squash` —— 歷史鏈特權操作(SPEC_08 §6.2 收尾)

**開單**:2026-07-26(驗收方)。**基線**:dev @ 本工單 commit(v0.2.40 後)。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §6 再回報**。探針**修改權在驗收方**
——交付僅移除探針 `#[ignore]`,**一字不改其餘**。

## 1. 法源與定位

SPEC_08 §6.2 剩餘兩個操作本體:

| 操作 | 說明 | 審計標籤 |
| :--- | :--- | :--- |
| `#rollback` | 移動 HEAD 指標至任意歷史 Commit | 無(見 §2 裁定) |
| `#squash` | 壓縮並清除 Commit 歷史片段 | `#privileged_squash` |

**與 `#pin` 的分野**:兩者**都不碰格**,只移動 HEAD 與改寫 commit 鏈。且兩者
皆為**單一命令**(不跨 evolve→commit 兩行程),故 `#pin` 弧踩到的「意圖 ≠ 授權」
陷阱**在操作本身不會重演**——能力與效果在同一行程。

**能力槽已備**:`Privilege.rollback` / `Privilege.squash`(選擇性 discharge 弧
宣告,`--grant` 已可解析)。本弧啟用它們,§6.2 五操作至此全數有本體
(`#commit` 已於 `#pin` 弧退役)。

## 2. 裁定(2026-07-26,使用者)

### 裁定 R1:回溯必須留痕 —— 記在「下一個 commit」

§6.2 給 `#rollback` 的審計是「無(由歷史鏈追蹤)」。**量測顯示該保證在 n/ 不
成立**:回溯後 `oo log` 自新 HEAD 沿 parent 走,**被放棄的整段離開歷史**——
物件還在 store,但**無任何介面能列舉它**。git 敢這樣講是因為它有 reflog;n/
沒有,而且**在 n/ 歷史鏈的份量遠高於 git**(git 把軌跡留在本地、推乾淨的成果;
n/ 的鏈本身就是紀錄)。

**裁定**:`#rollback` **本身不建 commit**(它只移指標),但**回溯後的下一個
commit 必須記下被放棄的 HEAD**——記於 **commit 元資訊**,**絕不入值**
(§6.2 幾何指紋:不得移動任何 CAID)。那正是分歧真正進入鏈的時刻。

**副產物(設計上接受)**:歷史圖從此有**兩種邊**——`parent`(實線,收斂血緣)
與 `abandoned`(虛線,分歧標記)。**仍是 DAG**(無環、無 merge 歧義),多出的
是 git 藏起來的**真實結構**。n/ 自己的讀法:回溯是**反單調**的(disc/021),
那條虛線即 **n/^op 上箭頭在歷史圖上的現身**。

### 裁定 R2:可壓過放棄紀錄,squash 標記承接

`#squash` **得**壓掉一段放棄紀錄;其自身的 `#privileged_squash` 標記把事實
帶下去。即:

> **可以移除內容,不能移除「發生過移除」這件事。**

granularity 可失,**事實不可失**(squash 一個 squash,結果仍帶標記)。

### 由 R2 推出的設計要求(**務必照做**)

`#squash` **必須同時斷開區間內的 `parent` 邊與 `abandoned` 邊**。理由:被放棄
的 commit 因新邊而**永久可達**,單靠可達性掃描**永遠回收不到**。⟹

> **`#squash` 是 n/ 的 GC** —— 它是**唯一**能讓東西變成不可達的操作,而它受
> 特權且自我標記。之後真正回收位元組則退化為**機械、無特權**的可達性掃描。

git 把「從視野消失」(reflog 過期)與「刪除」(gc)拆成兩件;**n/ 併成同一件
受審計的事**。(**今日完全無 GC**、store 純 append-only——GC 本身**不在本弧**,
本弧只需讓「不可達」成為**可能**。)

## 3. 修法(建議)

**(A) CLI**(`oo/src/main.rs`),兩個新子命令,`--grant`/`--privileged` 復用
既有 `apply_cli_privilege`(**不得另寫解析**):

```
oo rollback <CAID> --grant rollback
oo squash   <BASE_CAID> --grant squash
```

- **`squash <BASE>` 語義(定死,無歧義)**:把 `<BASE>` **之後**至 HEAD 的所有
  commit 壓成**一個** commit,其 `parent` = `<BASE>`,其 `root` = 原 HEAD 的
  root(**內容不變**)。
- 兩者**皆須乾淨暫存區**:`is_dirty` 時**大聲拒**(避免暫存變更悄悄套到不同
  的 root 上而遺失)。
- 無能力 → **大聲拒** `#privileged_required`;能力**不得**互相授權。

**(B) 放棄紀錄**:
- `CommitMeta` 加欄(如 `abandoned: Option<Vec<String>>`),**必須**
  `#[serde(default, skip_serializing_if = "Option::is_none")]` —— 舊 commit
  反序列化與**雜湊**不得改變。
- rollback 時把被放棄的 HEAD 記於 `.oo/abandoned`(跨行程);**下一次 commit
  消費之並清除**。
- **與 `#pin` 的關鍵差異(勿混淆)**:此檔是**審計紀錄**,**不是授權**——故
  commit 端**不需**出示能力(特權行為是 rollback,當時已授權)。偽造它只會
  往歷史**加**一條假紀錄,不會取得任何能力。仍掛 REAL_02 帳(同 `#pin` 弧)。

**(C) `oo log` 顯示**:squash 標記(`CommitKind::Squash`,`Standard` 仍
`#[serde(other)]`)與放棄紀錄皆須顯示。

## 4. 紅線

- **CAID / 宇宙內容不動**:壓縮**歷史**不得改變宇宙**是什麼**(釘
  `red_squash_preserves_the_universe`)。未觸 `bn_serial`/`content_hash`;
  genesis 須綠。
- **舊 commit 相容**:既有 commit 物件的雜湊與反序列化不得改變。
- **rollback 不刪位元組**:只斷可達性(釘 `red_abandoned_commits_are_not_deleted`)。
- **不得改**:`#pin` 全套(能力格、pin_coords、commit 端能力重驗)、效應四弧、
  普通 evolve/commit/log 路徑。
- **能力互不授權**:`pin`/`effect_override`/`rollback`/`squash` 四者正交(釘)。

## 5. 門(紅)與釘 + 目標(先量後寫,基線實測 2026-07-26)

**探針(一檔,已預提交+校準)**:`crates/oo/tests/history_ops_probe_test.rs`

- **10 紅**(`#[ignore]`):rollback ×5(移動 HEAD／需能力／能力專屬／
  **下一 commit 記錄放棄**〔R1 承重〕／**放棄物件不刪**)、squash ×5(壓縮區間／
  需能力／結果有標記／**宇宙內容不變**／**壓過放棄紀錄後事實仍在**〔R2 承重〕)。
- **5 釘**:普通歷史無標記、log 走完整鏈、普通 evolve/commit 不受影響、
  歷史能力不授權 discharge、`effect_override` 不授權歷史操作。

**校準已驗**:10 紅全紅且各因對的理由;5 釘全綠。
> **連續第三弧出現空洞紅**(本次 3 支)。根因每次相同:**操作沒發生時,不變量
> 自動成立**——`red_abandoned_commits_are_not_deleted`(rollback 失敗 ⟹ 物件
> 當然還在)、`red_squash_preserves_the_universe`(squash 失敗 ⟹ 宇宙當然沒變)、
> `red_rollback_is_recorded_in_the_next_commit`(rollback 失敗 ⟹ 被放棄的 CAID
> 當然還在 log 裡,因為它還是 HEAD)。
>
> **升級後的規則(請一併遵守,勿在交付時弱化這些前置斷言)**:
> **每一支紅門都必須先斷言「被測操作確實發生了」,再斷言不變量。** 只斷言
> 不變量者,「什麼都沒發生」永遠通過。

**交付 = 移除全部 10 個 `#[ignore]`**,探針其餘一字不改。

**目標**(基線 → 交付後):

| 項 | 基線 | 目標 |
| :--- | :--- | :--- |
| 本探針 | 5/5(10 ignored) | **15/15** |
| workspace | 1419/0/13 | **1429/0/3** |
| conformance | 143/143 | **143/143(不變)** |
| genesis | 11/11 | **11/11(不變)** |

**合規向量**:本弧**不新增**——能力與兩子命令皆僅 CLI 可達(runner 不傳旗標),
同 arc-4／選擇性 discharge／`#pin` 先例。

## 6. 交付紀錄(交付方填;先寫再回報)

- [x] 交付 commit(s): 見 tip(本節寫畢後 commit;follow-up 補記 tip)
- [x] CLI(`rollback`/`squash` 子命令 + `--grant` 復用)落點:
  - `crates/oo/src/main.rs` `Commands::Rollback` / `Squash`;
    `run_rollback` / `run_squash` 皆走既有 `apply_cli_privilege`(不新寫解析)。
- [x] `#rollback`(移 HEAD + 重載 root + 髒暫存拒 + 能力閘)落點:
  - `universe.rs` `Universe::rollback`: `is_dirty` 拒;能力閘在 CLI
    (`!privilege.rollback` → `#privileged_required`);`set_head` + 自
    target commit 重載 root;不建 commit、不刪 store 物件。
- [x] 放棄紀錄(`.oo/abandoned` → 下一 commit 的 `CommitMeta`;serde 相容)落點:
  - rollback 將舊 HEAD 追加寫入 `.oo/abandoned`(審計意圖檔,非授權)。
  - `Universe::commit` 消費該檔填入 `meta.abandoned` 後清除;commit 端
    **不**再驗 rollback 能力(特權行為已在 rollback 完成)。
  - `CommitMeta.abandoned: Option<Vec<String>>` 帶
    `#[serde(default, skip_serializing_if = "Option::is_none")]`。
  - **自訂 `Debug`**:`abandoned == None` 時輸出與舊三欄 derive 相同,
    以保 `Commit::content_hash`(走 `format!("{:?}", meta)`)舊 commit
    雜湊不變;有放棄紀錄時 Debug 含 abandoned。
- [x] `#squash`(區間壓縮 + parent=BASE + root 不變 + **斷 parent 與 abandoned
      兩種邊** + `CommitKind::Squash`)落點:
  - `Universe::squash`:髒暫存拒;base 須為 HEAD 祖先;新 commit
    `parent=base`、`root=HEAD.root`、`kind=Squash`;不複製區間內
    abandoned 元資訊(邊隨中間 commit 離開 parent 鏈);清除 pending
    `.oo/abandoned`。
  - `CommitKind::Squash` 標籤字節 **3**(Standard=0/Refine=1/Pin=2 不變)。
- [x] `oo log` 顯示(squash 標記 + 放棄紀錄)落點:
  - `CommitKind::Squash` → `    squash`;`meta.abandoned` →
    `    abandoned <CAID>`(每條一行)。
- [x] **確認**:舊 commit 雜湊/反序列化不變;rollback 不刪物件;宇宙內容不變:
  - 未觸 `bn_serial`/值 `content_hash`;genesis **11/11**。
  - 探針:abandoned 物件仍存在;squash 後 `f1: 2` 仍 Evolution Conflict。
- [x] 四數:本探針 **15/15** · workspace **1429/0/3** · conformance **143/143** ·
      genesis **11/11**
- [x] 申報事項(範圍外接觸、CAID、其他):
  - 探針**僅移除 10 個 `#[ignore]`**。
  - `#pin` / 效應四弧 / 普通 evolve-commit 路徑未改能力語義。
  - 真正 GC 掃位元組**不在本弧**(store 仍 append-only);本弧只使不可達可能。
  - 測試內 `CommitMeta {…}` 字面量補 `abandoned: None`(編譯所需)。

## 7. 驗收紀錄(驗收方填)

## 8. 意見

本弧補完 §6.2。三個特權操作各自體現一種「特權是什麼」:
`#effect_override` 製造截面、`#pin` 逆轉格的單調性、`#rollback`/`#squash` 改寫
**歷史**本身。

最值得記的是 R2 推出的那條:**`#squash` 是 n/ 的 GC**。git 讓遺忘自動發生
(reflog 過期),n/ 讓遺忘**只能被明示地、受審計地執行**。這不是工程細節,是
「歷史鏈就是紀錄」這個立場的必然結論。
