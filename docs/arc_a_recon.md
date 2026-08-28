# 弧 A 偵察（Q-013）— 引擎內部普查

> **Queue ID**：`WORK_QUEUE` Q-013（Active，偵察）
> **基線**：引擎 `v0.37.0` 標籤二進位（本機 `nlang-tools/target/release/oo`，建於
> `7b17c9f`；`--version` 字串仍印 `v0.36.0-694-ge86769a+`，與標籤不同步）／
> 規格 `v0.37.0-draft.1`／工作樹 `nlang-tools dev cf10dad`（乾淨，本檔是唯一產物）。
> **這是偵察，不是實作。** 下面若有「一行就能修」的東西，只寫進報告。
> **未重量**驗收方 2026-08-28 已量的八項（brief §1）；本檔只答七題。
> **身分**：本輪零改動。抽樣使用者根仍帶
> `~%__nlang_system_digest: "7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911"`。

先讀了 `meta/oo/commit.md` §1.10 與 `observation_result.md` §1。裁定 D42–D47 不重開。

---

## 0. 七題各一句

| | 答案 |
| :-- | :-- |
| **Q1** | commit 強制求值是**一個**呼叫點：`universe.rs:828` `force_recursive`。unify／pin-merge 不強制。 |
| **Q2** | 拿得到。每個 `Value` 有 `effect()`；Thunk 另有欄位，非純時寫進 staged 的 `~%__nlang_effect`。有一個不誠實的家（forward-miss 一律 `#pure`）。 |
| **Q3** | 觀測入口是 `Universe::observe` → `force_recursive`。判別器住在 `force_recursive` 的 Combo 快路，**叫得到**；Thunk／原子那兩臂不走它，等價訊號是 `matches!(Thunk)`。 |
| **Q4** | **沒有 ○ 實體。** 一切是單一檔 `.oo/staged`（無序、無身分、commit 後刪）。 |
| **Q5** | 比 `observation_result.md` §1 更錯位：`partial` 已是 CAID，本體在 `partial_body`；印表機只印 cause＋blur CAID。改動面約 **2 型／~5 建構／~25 讀取**。 |
| **Q6** | 引擎沒有「固化」這個識別字。`staged`／`solidif*`／`force_recursive` 是值／歷史軸（D45 射程）；`solidify_effects` 是 `%effect` 軸（丁，不得改語義）；`freeze`／`collapse` 不是這條軸。 |
| **Q7** | 路徑形走 evolve 的豁免枝（`is_root_config_field_write`）；combo 形走系統軸禁寫。**意圖是路徑形。** `eval.rs:1384` 治的是 combo **字面量內部**，不是 root 路徑形。 |

---

## Q1 — commit 的強制求值發生在哪一行？

**一個點。** `Universe::commit` 在 meet 之後、鑄根之前，對整棵新根呼叫一次
`force_recursive`：

```822:831:crates/interpreter/src/universe.rs
        // O35/O51: commit, not evolve, is the solidification boundary. The
        // staged workset remains lazy; history receives its observation.
        let mut commit_ctx = crate::EvalContext::new(new_root.clone())
            .with_standard_root(self.standard_root.clone());
        commit_ctx.memo_enabled = false;
        commit_ctx.preserve_refs = true;
        let new_root = match engine.force_recursive(Value::Combo(new_root), &mut commit_ctx) {
            Value::Combo(root) => root,
            _ => return Err(anyhow::anyhow!("Commit observation did not produce a root")),
        };
```

其前的 `engine.unify`（`:814`）／`pin_commit_merge`（`:809`）是懶 meet：
`unify.rs:176-177` 明寫 `Top & Thunk` 保留 Thunk，「force only when
value-judgment needed」。`squash`（`:1037-1052`）複製既有根、不強制。
`observe`（`:1106`）與 `eval_observed`（`lib.rs:3252`）是**觀測**入口，不是
commit。

`force_recursive` 本身是樹走訪（Combo 六軸各 `force_recursive`，
`lib.rs:3674-3715`）。那是這一個呼叫的實作，不是第二個 commit 邊界。

D46 要改成「只強制不可重現的一格」：改的就是 **`:828` 這一刀**（走訪時看
`Value::effect()`／Thunk 的 `effect`，`#pure` 跳過）。不必在 CLI 或 unify 再插一刀。

### 可重跑（拆檔；同檔會在 evolve 就算完，見 brief §1 第 4 項，不重量）

```bash
OO=nlang-tools/target/release/oo
W=$(mktemp -d); export OO_IDENTITY="$W/id" OO_NODE_HOME="$W/nh"; cd "$W"
printf 'c: a + b\n' > c.n
printf 'a: 1\nb: 2\n' > ab.n
$OO evolve c.n && $OO evolve ab.n
$OO status
# staged 逐字：c: a + b（編碼見 .oo/staged 的 ~%__nlang_thunk）
$OO commit -m q1
COMMIT=$($OO log | awk '/^commit /{print $2; exit}')
ROOT=$($OO inspect "$COMMIT" | awk '/^root:/{print $2; exit}')
$OO inspect "$ROOT"
# 根裡逐字：c: 3
# 摘要 067627389c…（與 STATUS D46 註記同一顆使用者根形）
```

算不出來的一格（只 evolve `c: a + b` 就 commit）→ 根 `c: _`。成因仍是 `:828`：
強制一個不可解的 thunk 得到 Top／⊥，寫進歷史。

既有探針：`crates/oo/tests/a_value_not_a_recipe_probe_test.rs`（O35／O51，
「commit 是固化邊界、staged 保持 Thunk」）。

**順帶（寫進報告，不改）：** 頂層 evolve **已經** `engine.eval`（`universe.rs:389`）。
`1 + 2`、`~%Time./now #trigger` 在 evolve 就變成 `3`／時間戳，commit 的
`force_recursive` 對它們無工可做。`:828` 真正吃到的是 **combo 欄位 thunk** 與
**forward-miss／拆檔 thunk**。D46 的「● 留純 thunk」若只改 commit、不動 evolve
頂層 eval，頂層純計算仍然進 ●——那是第二個邊界，開單時要寫明。

---

## Q2 — `%effect` 在那一點拿不拿得到？

**拿得到。** `:828` 的引數是 `Value::Combo(new_root)`。每一格都是 `Value`：

```2940:2958:crates/interpreter/src/value.rs
    pub fn effect(&self) -> EffectTag {
        match self {
            Value::Combo(c) => c.effect,
            Value::Atom(_, e, None) => *e,
            Value::Atom(_, e, Some(_)) => *e,
            Value::Thunk { effect, .. } => *effect,
            …
            Value::Blur(bd) => bd.effect,
            …
            _ => EffectTag::Pure,  // Top / Ref / Bottom / Code
        }
    }
```

Thunk 的 `effect` 在建構時寫入：

* combo 字面量欄位：`combo_field_from_expr`（`eval.rs:256-278`）← `predict_effect`
* 非純才進磁碟：`store_codec.rs:416-418` `~%__nlang_effect`；`#pure` 省略（與
  Combo 的「缺席＝純」同一拼法）
* 印表機：`to_nlang` Thunk 臂（`value.rs:3189-3194`）非純時加 `;; %effect: #io`

### 證法（先證明不是「沒呼叫／沒讀到」）

同一 combo 裡三個 thunk，staged **尚未**觀測：

```nlang
box: {
  pure_t: 1 + 2
  io_t: ~%Time./now #trigger
  math_t: ~%Math./add (1, 2)
}
```

`oo evolve` 後 `oo status` 逐字：

```
io_t: ~%Time./now #trigger  ;; %effect: #io
math_t: ~%Math./add (1, 2)
pure_t: 1 + 2
```

* `io_t` 的表達式仍是 `~%Time./now #trigger`，不是時間戳 ⟹ **呼叫沒發生**。
* 換成 `math_t`／`pure_t`，`;; %effect` **消失**（規範缺席＝`#pure`）⟹
  讀到的是預測標籤，不是預設值。
* `.oo/staged` 對 `io_t` 有 `~%__nlang_effect: #io`，對另外兩個沒有。

對照：同一兩個呼叫寫在**頂層**，evolve 就跑了
（`io_top: 1787888481753  ;; %effect: #io`，`pure_top: 3`）。那是 Q1 的
「頂層 eval」，不是 Q2 的洞。

`EffectTag::is_pure`（`value.rs:47`）就是 D46 的判準 API。

### 不誠實的一個家

`universe.rs:409-414`：evolve 把 forward-miss 重寫成 Thunk 時 **寫死
`EffectTag::Pure`**，不走 `predict_effect`。一個稍後才綁上 `#io` 來源的名字，
在 commit 當下會被看成純的。D46 若只信 `effect()`，這一格會被留下 thunk，
而它正是「不可重現、必須進 ●」的那類。開單時要嘛在重寫處改叫 `predict_effect`，
要嘛 D46 走訪不信任 forward-miss 標籤。

---

## Q3 — `solid_combo_expansion_cost` 在觀測邊界上叫不叫得到？

函式在 `lib.rs:419-452`（brief 寫 `:418`，註解多了一行）。回 `None`＝遇到
`Thunk`／`Ref`／未決 spread，「需要真正的 forcing 路徑」。

**真正的觀測入口**是 `Universe::observe`（`universe.rs:1059`）：

1. overlay staged `~%Config`、unify 根
2. `engine.resolve_path`（`:1095`）
3. `engine.force_recursive(res, &mut ctx)`（`:1106`）

CLI 投影：`oo run --observe`（`main.rs:1411`）、`oo eval`（`:1486`，**空白宇宙**，
不能拿來讀已提交的欄）、REPL（`:1217`）、`oo test`（`:1707`）。

`solid_combo_expansion_cost` 的兩個呼叫都在 `force_recursive` 裡
（`:3546` 入口快路、`:3631` Combo 強制後再查一次）。所以：

* **觀測 Combo 時叫得到**——而且今天的用途就是 D47 要的那句話的一半：
  `Some(cost)` ⟹ 已經實心，**不再化約 thunk**，只帳單再回傳原值；
  `None` ⟹ 走真正的 forcing 路徑。
* **觀測 Thunk／原子時不叫它。** Thunk 走 `:3567` 的 peel（那就是化約）；
  原子走 `:3596` 的固定進場費 `SUBSPACE_EXPANSION`。
  等價訊號：`matches!(val, Value::Thunk { .. })`。
* observe **自己**沒有先問這個函式再決定要不要 `force_recursive`。
  D47「有沒有真的化約」若要在寫 ○ 之前判定，可以在 `:1106` 之前對
  `res` 當 Combo 調一次（純函式、無副作用）；Thunk 臂不必調。

驗收方已量「燃料是固定進場費」。本輪只確認入口仍是 `observe`→`force_recursive`，
且 fuel=0 時原子與短加法鏈都 `#blur`／fuel=1 原子成功：

```bash
$OO run atom.n --observe c   # ~%Config.fuel: 0 / c: 3     → #blur { %cause: #fuel_exhausted, %caid: "…629cf304…" }
$OO run chain.n --observe c  # fuel: 0 / c: 1+1+1+1+1      → #blur { %cause: #fuel_exhausted, %caid: "…041bdcee…" }
$OO run atom1.n --observe c  # fuel: 1 / c: 3               → 3
```

兩個 blur 的 CAID 不同 ⟹ `BlurDetail.partial` 的內容進了身分（CHS），
但印表機**不印**走到哪裡（只印 cause＋blur 自己的 CAID）。這就是 Q5。

既有探針：`crates/oo/tests/the_meter_reads_two_probe_test.rs`（註解逐字點名
`force_recursive`）。

---

## Q4 — 今天有沒有任何「○」的實體？

**沒有。** `Universe`（`universe.rs:214-220`）的耐久狀態是 `head`＋`root`＋
`standard_root`＋**一個** `staged: ComboVal`。沒有 savepoint 型別、沒有 ○ id、
沒有 `refs/`、沒有鏈。

`ComboVal`（`value.rs:886-914`）可序列化欄位是六軸 map＋`closed`＋`effect`＋
`relations`＋`masa_ref`。`pending_spreads`／快取 id 皆 `#[serde(skip)]`。
**沒有序號、沒有本地身分。**

`.oo/staged` 是單一文件：框 `#nlang/store staged` 加一個 combo
（`store_codec.rs:159-161` `encode_staged`）。抽樣 28 B 量的是空／小 combo；
有 thunk 時是整份 n/ 值，仍是**一個檔**。

| | 位置 |
| :-- | :-- |
| **寫** | `Universe::save_staged` `universe.rs:707`（blur partial 先入 CAS，再 `atomic_write` staged）。呼叫者：`oo evolve` `main.rs:463`；commit 留下 `~%Config` 時 `:868` |
| **讀** | `Universe::load_staged` `:748`（框 → `decode_staged`，否則 JSON）。呼叫者：`load_universe` `main.rs:1605`（幾乎所有讀宇宙的 CLI） |
| **刪** | commit 成功且沒有保留 Config：`:872-875` `remove_file(.oo/staged)`。Config 覆寫則**改寫**為只含 `~%Config` 的 staged（`:863-868`），不刪 |

旁邊還有 `pin_pending`、`effect_pending`、`abandoned`——意圖／審計，不是 ○ 鏈。
`squash` 在 dirty 時拒絕（`:1005`），所以不負責刪 staged。

`REAL_01` §10.1「進程崩潰，Staged 內容丟失」今天字面成立，與 D43「每個 ○
都已經是持久的」相反——驗收方 R1 已寫。本輪只補：引擎裡**沒有第二個槽**可以讓
○ 住。A3 的日誌正準形仍應等本問與 Q6（本 recon 的產出）。

---

## Q5 — `BlurDetail.partial` 的錯位有多大？

`observation_result.md` §1 引的是舊形：

```rust
pub partial: Option<Box<Value>>, // 走到哪裡 —— Q1,被降級成一個欄位
```

v0.37.0 實際是（`value.rs:1913-1927`）：

```rust
pub struct BlurDetail {
    pub cause: BlurCause,                 // Q2
    pub horizon: HorizonParams,           // Q2 的視界
    pub partial: Option<ContentHash>,     // Q1 的 CAID，不是值
    pub partial_body: Option<Box<Value>>, // Q1 的值，#[serde(skip)]，evolve 才寫 CAS
    pub effect: EffectTag,                // Q3
    pub co_horizons: Vec<HorizonRecord>,  // 同一錯位再複製一份
}
```

`HorizonRecord`（`:1885`）同樣有 `partial`＋`partial_body`。

錯位比 §1 判定的**更深一層**：格上位置不但被包進「為什麼停」，而且被編成
一個**只能經 CAS 取回**的位址；印表機（`to_nlang` `:3171-3177`）只印

```
#blur { %cause: #fuel_exhausted, %caid: "<blur 自己的 CHS>" }
```

Q1 對操作者不可見。Q3 那兩個不同的 blur CAID 證明 partial 進了身分，
但「走到哪裡」仍然不是一等答案。

### 改動面估計（讓 Q1 可獨立取得；不實作）

| 面 | 處數 | 名單 |
| :-- | ---: | :-- |
| **型別** | **2** | `BlurDetail`、`HorizonRecord`。若 Q1 升成觀測結果的獨立欄（或獨立型），這兩個要拆；`Value` 多一個變體則另加 1 |
| **建構** | **5 產線** | `from_single`（`value.rs:1930`，Value→CAID＋body）← `observation.rs:124` `handle_resource_exhausted`；`math.rs:647/1252/1282` 三處直接 `from_single`；`merge_set` `unify.rs:426`；`store_codec.rs` `decode_blur`（讀回時 `partial_body: None`） |
| **把「走到哪裡」塞進去的呼叫** | **~20** | `handle_resource_exhausted(` 產線 20 處（`lib.rs` 8、`eval.rs` 9、`unify.rs` 1、`complement.rs` 1）。幾乎都先 `needs_partial_body` 再把剩餘 `Value` 當 `partial` 傳入 |
| **讀取** | **~12 產線** | CHS／`blur_caid`（`:1903`、`:1974`）；`persist_partials`／`persist_blur_partials`（`:2011`、`:2154`）；`store_codec` 寫／讀 `partial` 雜湊（`:454`、`:1431`）；`bn_serial.rs:158`；`solidify_effects`／`purify_effects` 走 `partial_body`；`holds_meta_builtin`；`to_nlang`（**不讀 partial 值**） |

測試側另有 `blur_test.rs` 大量 `from_single`——改型別會跟著紅，不算產線。

A3 若要讓 ○ **存**局部收斂結果，最小形不是改印表機，是讓觀測結果在 Blur
**之外**拿得到 `partial_body` 那個 `Value`（或它的格位置）。今天唯一的一等
通道是 CAS 裡那顆 CAID，而且只在 `save_staged` 之後才保證落盤。

---

## Q6 — 引擎側拼法普查

規格側：`固化` 79 行／15 檔／四義。引擎 **Rust 識別字沒有「固化」**
（`crates/` 僅測試註解 2 處）。對應的是下面這張表。計數＝
`crates/interpreter/src`＋`crates/oo/src`（不含 `**/tests/**`），2026-08-28。

| 識別字 | 檔／次（src） | 模組 | D45 |
| :-- | --: | :-- | :-- |
| **`staged`** | 7 檔／160 次 | `universe.rs`（主家）、`main.rs`、`store_codec.rs`、`lib.rs`、`value.rs`、`eval.rs`、`storage.rs` | **射程內（乙的易失前身）**。就是今天的「○ 該住的那個槽」，但沒有 ○ 的三個性質（順序／身分／耐久） |
| **`solidif*`** | 7 檔／62 次 | 見下分義 | 一字兩軸，必須拆 |
| **`force_recursive`** | 5 檔／46 次 | `lib.rs`（定義）、`universe.rs`（commit＋observe）、`eval.rs`（繭／比較）、`builtins/engine.rs`、`builtins/list.rs` | **射程內（甲＝求值）**。commit `:828` 與 observe `:1106` 是兩個語義動作共用同一個實作 |
| **`solidify_effects` / `solidify_active_effect`** | 3 檔／20 次 | `value.rs`（定義，SPEC_08 §4.2.4 `#io`→`#cached`）、`main.rs:1560`（inspect 觀測投影）、`builtins/disc.rs`（fetch／find 出站） | **射程外（丁，`%effect` 軸）**。D45／D46 都說這條要被**引用**不得改語義 |
| **`freeze`** | 3 檔／3 次 | `value.rs:1523`、`type_constraint.rs:241`、`genesis.rs:1` 註解 | **無關**。fmt v2「列舉凍結」，不是求值也不是 commit |
| **`collapse`** | 24 檔／277 次 | `Value::collapse`（`%val` 純包裝剝離）＋ math／list／string 等內建 | **無關／格軸**。不是固化。動它會動 L2 與內建 |
| **`固化`（漢字）** | 0（src） | — | 引擎不拼這個字；註解用英文 solidification |

`solidif*` 62 次的分義（同一字，開單不得混改）：

| 義 | 代表 | D45 |
| :-- | :-- | :-- |
| **甲 求值** | `universe.rs:822`「commit is the solidification boundary」；`observe:1091`；`eval.rs` 繭 GUIDE_03 §11.5；`lib.rs:3244` `eval_observed`；cmp 的 `solidify: bool`（`:384`） | 射程內；正準應改叫**觀測／強制** |
| **乙 進歷史** | 同上 commit 註解把甲乙寫在一句（與規格 `SPEC_10` §3 同病） | 射程內；正準就是 **commit** |
| **丁 `%effect`→`#cached`** | `value.rs:2786` `solidify_active_effect`；disc 出站；inspect | **不得動語義** |

`staged` 與 ○ 在耐久性上相反——`commit.md` §1.10.6 已寫，引擎側數字確認：
它是唯一工作集，commit 成功就刪。

---

## Q7 — `~%Config` 兩種拼法為何一活一死？

**路徑形是意圖。combo 形是系統軸所有權正在生效。**

### 活的那條：root `~%Config.<裸名>`

`Universe::evolve` 開頭（`universe.rs:337-351`）對 LHS 做
`is_system_axis_lhs_forbidden`。豁免在前：

```58:71:crates/interpreter/src/universe.rs
fn is_root_config_field_write(key: &FieldKey) -> bool {
    match key {
        FieldKey::Path(p)
            if p.anchor == PathAnchor::Bare
                && p.segments.len() == 2
                && p.segments[0].trim() == "~%Config" =>
        { … 第二段是裸名 … }
        _ => false,
    }
}
```

```183:186:crates/interpreter/src/universe.rs
fn is_system_axis_lhs_forbidden(key: &FieldKey) -> bool {
    if is_root_config_field_write(key) {
        return false;
    }
```

通過後走 `FieldKey::Path(p) if is_root_config_field_write`（`:458-488`）：
名籍／型別檢查，把裸名寫進 staged 裡一個**開**的 `~%Config` 殘片。
註解逐字：「Root `~%Config.<bare>` horizon-parameter family (SPEC_08 §3.1)
— write exempt。」

### 死的那條：root `~%Config: { … }`

這是 `FieldKey::Named { prefix: Some(Prefix::System), name: "Config" }`。
**不是** `is_root_config_field_write`（那只認兩段 Path）。
`is_system_axis_lhs_forbidden` 的 Named＋System 臂（`:188-191`）為真 →
evolve 立刻 `Err(BottomDetail { cause: SystemReserved })`，CLI
`Evolution Conflict … #system_reserved at ~%Config`。
`--privileged`／`--grant pin` 都在 evolve 之前，繞不開
（`oo run combo.n --privileged --observe out` 同樣 exit 1）。

### `eval.rs:1384` 治的不是 root 路徑形

那一刀在 **combo 字面量**的 `FieldKey::Path` 臂：首段以 `~%` 開頭 → 該欄鑄
⊥ `#system_reserved`。Named 臂 `:1275-1276` 對 `Prefix::System` 做同一件事。

所以：

| 拼法 | 走哪裡 | 結果 |
| :-- | :-- | :-- |
| root `~%Config.fuel: 0` | evolve 豁免枝 `:458` | 活；staged `{ ~%Config: { fuel: 0 } }` |
| root `~%Config: { fuel: 0 }` | evolve 禁寫 `:346` | 死；`#system_reserved` |
| `{ ~%Config: { fuel: 0 } }` | `eval.rs:1275` | 死；欄是 ⊥ |
| `{ ~%Config.fuel: 0 }` | `eval.rs:1384` | 死；欄是 ⊥ |

### 意圖（引規格，不猜）

`SPEC_09` 開頭所有權條款豁免逐字：

> root 之 `~%Config.<裸名欄>` 寫入＝視界參數規範家（SPEC_08 §3.1），合法……
> **combo 內寫 `~%Config` 不豁免**（節點級提示自有 `%fuel` 降級管道）。

`SPEC_08` §3.1：規範家是裸名路徑（`~%Config.fuel`、`~%Config.timeout`、…）。
`CHANGELOG` 2026-07-20：整組替換形「另案不預斷」——combo 形從未被立法為合法。
探針 `system_axis_probe_test.rs:16-18`、`155-173` 把路徑形當「THE trap pin」、
combo 形列為不在本弧。

⟹ **路徑形是意圖；combo 形該死。** Q7 不修。若有人要把整組替換立法，那是
CHANGELOG 已記的另案，不是 bug。

### 可重跑

```bash
printf '~%%Config: { fuel: 0 }\n' > combo.n
$OO evolve combo.n
# Evolution Conflict … #system_reserved at ~%Config  rc=1

printf '~%%Config.fuel: 0\n' > path.n
$OO evolve path.n && $OO status
# staged: { ~%Config: { fuel: 0 } }

$OO run combo.n --privileged --observe out
# 同樣 #system_reserved（evolve 沒有 --privileged 旗標；run 有，仍然死）
```

既有探針：`system_axis_probe_test.rs::pin_config_fuel_write`、
`config_validation_probe_test.rs`（名／型；註解明寫整組替換不在射程）。

---

## 開單時用得上的三件（本輪不改）

1. **D46 的落點是 `universe.rs:828` 一刀**，但頂層 evolve 已經 eval。只改
   commit 救不了頂層純計算進 ●。forward-miss thunk 的 `effect` 不可信。
2. **D47 的判別器已在 `force_recursive` 裡**；要在寫 ○ 之前問「有沒有化約」，
   對 Combo 調 `solid_combo_expansion_cost`、對 Thunk 看型別即可。不要用燃料。
3. **沒有 ○ 可寫。** A3 的日誌正準形若現在開工，會把 `staged` 的歧義烘進格式。
   Q1 位置要先能從 Blur 裡獨立取出（Q5），否則觀測條款「存局部收斂」沒有載體。

紅線未動。規格／引擎皆未改。
