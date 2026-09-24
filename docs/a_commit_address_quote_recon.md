# 偵察：commit 位址的報價（Q-056，驗收方自做，不交接）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-056／請裁 `meta/oo/STATUS.md` **O92**
> 基線：`oo v0.58.0` 標籤建置（`/home/gali/nlang-baselines/v0.58.0-verify`），known-answer `3`／`4` 已過；
> 離開碼一律取自 `oo` 自己，不經管線。rustc 1.96.1。

原題是一句話：「commit 的位址算法沒有規範，而參考實作的算法是 Rust 專屬的，**先報價**」。
報價的過程量到了**比可攜性更急的東西**：commit 物件裡有三類位元組在位址之外，其中一類是審計裁決。

## 1. 算法（讀 `value.rs` `Commit::content_hash`）

```
buf = [parent.digest]?          ← 32 B 或 0 B，無存在位元
    ‖ root.digest               ← 只有 digest；根 CAID 的版本／masa／sketch 不在內
    ‖ kind                      ← 1 B（Standard 0／Refine 1／Pin 2／Squash 3）
    ‖ refine.source_caids.digest*   ← 逐個接上，無個數
    ‖ refine.target_caids.digest*   ← 逐個接上，無個數
    ‖ LEB128(len) ‖ format!("{:?}", meta)   ← Rust 的 Debug 排版
CAID = hash:sha256:v1:sha256(buf)
```

`RefineInfo` 另有三個欄位**寫進磁碟、讀回來、印給人看，但不在 `buf` 裡**：
`authority`（簽署者公鑰、簽章、時間）、`shadow_affected`、`authority_status`
（原始碼註解逐字：「Not hashed into the commit CAID beyond source/target digests」）。

## 2. 量到的（依嚴重度）

### F1 審計裁決可以被偽造，而位址驗證放它過（`interrupt-candidate`，§2.2 第 2 款）

〔量〕一個 genesis 狀態下的 `oo refine`（無架構師登記 ⟹ 豁免）落地為 `refine authority: unverified`。
把 commit 物件裡的 `authority_status: "unverified"` 改成 `"verified"`：

| | `oo log` | `oo inspect <commit>` | `oo status` |
| :-- | :-- | :-- | :-- |
| **竄改 `authority_status`** | **rc=0，逐字 `refine authority: verified`** | rc=0 | rc=0 |
| 對照：竄改同一物件的 `message` | rc=1，`#caid_mismatch … recomputed de46dd03…` | — | — |

⟹ 讀路徑的位址驗證是活的（對照組開火），**而一次經位址驗證的讀取，回傳了那個位址並未承諾的審計裁決**。
這是 D65（`message` 不在 `_|_` 的 CAID 內）的同族，**而它更重**：那裡是一段自由文字，這裡是
`SPEC_08` §6.2 的審計面——「這次精煉有沒有經過密碼學驗證」。
`authority` 本身（誰簽的）也不在位址內 ⟹ 簽章可以被拿掉或換成另一個有效簽章而位址不動（**讀，未量**）。
〔未量〕對等節點取回 commit 物件時是否同樣只驗這個位址——若是，**這是服務面，不只本機面**。

### F2 兩個不同的精煉共用一個位址（`REAL_03` §6.7 雙射）

〔量〕`oo refine --source R1 R2 --target R2` 落地為 `fe31a46e…`。把物件裡的第二個 source 移到 target 最前面
（source=[R1]、target=[R2, R2]）：`inspect` rc=0、`log` rc=0，**位址不變**。
成因是兩張清單逐個接上而無個數。**哪些 CAID 是 target 有語義**：`REAL_03` §9.1 的不透明模式逐字以
「該 CAID 出現在 `#refine` Commit 的 `target_caids` 欄位中」為觸發條件。

### F3 commit 的 `root:` 印出一個它沒有承諾的位址（`REAL_03` §6.7）

〔量〕改 commit 物件裡根 CAID 的 sketch 一個字元：`inspect <commit>` rc=0 並印出**被改過的**根位址；
`log`／`status` 讀到根時才 rc=1。⟹ 下游抓得到，但 `inspect` 那一面說了假話。**比 F1／F2 輕。**

### F4 位址依賴宿主的排版，而那個排版依賴宿主的 Unicode 表（原題）

`format!("{:?}", meta)` 把 `message` 以 Rust 的字串 Debug 規則跳脫。〔讀＋量機制〕那個規則對一個碼點
**是否原樣輸出**，取決於標準庫內建的 Unicode 表：本工具鏈 `char::UNICODE_VERSION = 17.0.0`，
未指派的碼點輸出 `\u{…}`（例：U+0378 → `"\u{378}"`、U+2FFFD → `"\u{2fffd}"`），已指派且可印者原樣。
⟹ **推論（未量，手上只有一個工具鏈）**：一則含有「寫入時尚未指派、之後被指派」碼點的訊息，
換一個 Unicode 表較新的 rustc 重建引擎之後，**重算出另一個 CAID** ⟹ 一個沒有人動過的儲存，在引擎升級後讀成 `#caid_mismatch`。
情境不罕見：使用者的作業系統比引擎的工具鏈先支援一個新表情符號。
**第二個實作**要算出同一個 commit 位址，必須逐位元組重現 Rust 的 `debug_struct` 排版**與**某一版 Unicode 的可印表。

### F5 規格與引擎兩邊都沒寫

`REAL_03` 全文沒有一條定義 commit 位址怎麼算；而 §1（第 46 行）逐字寫著
「**v1** 僅用於 ORDER_00 創世 commit 的原點自舉」——**參考實作的每一個 commit 都是 v1**。

## 3. 報價：改成逐欄位的正準序列化，什麼會動

*   **若把新算法施於既有 commit，每一個的位址都會動**，而引用它們的地方有四個：`HEAD`、後代 commit 的 `parent`、
    ○ 框的 `commit:`／`ancestor:` 註記（D52／D55）、以及**後代 commit 的 `meta.abandoned`**
    ——最後一個在被雜湊的 `meta` 裡 ⟹ **改寫會沿鏈連鎖**。
*   **「改寫全部歷史」被一條既有的 MUST 擋住**：`REAL_02` §5.1.1「推進只動容器：**不得**移動 `HEAD`、
    **不得**改寫任何物件」。⟹ 可行的路只剩**雙讀**：既有 commit 永遠以舊算法驗證，新 commit 用新算法，
    由某個**寫進位址或寫進雜湊**的選擇子分辨（O75：決定解碼的自報必須在雜湊內）。**O63 的格式閘是現成判例。**
*   ⟹ **舊算法無論如何都要被寫進規格**（否則第二個實作驗不了任何既有 commit），
    而 F4 意味著它對非 ASCII 訊息**寫不成一個與工具鏈無關的定義**——只能釘死在某一版 Unicode 上，並自陳。
*   測試面：11 個檔含 7 個不同的寫死 v1 字面值（**未逐一分辨哪些是 commit**——`v1` 也用於值）；其中兩份夾具的 `HEAD`
    （`pre_sentinel_repo`，v0.20.0；`encoding4_repo`）**確定是舊 commit，正是雙讀的活見證**；conformance 語料 0 個。
*   身分紅線：值的位址（`x: 0`／`v: 1 + 1`／標準根）**不受影響**——commit 不是值的一部分。

## 4. 請裁（O92）

| | 形 | F1 | F2 | F3 | F4 | 代價 |
| :-- | :-- | :-- | :-- | :-- | :-- | :-- |
| **甲** | **commit 就是一個值**：位址＝它在磁碟上那個 n/ 值（`{ kind: … root: … meta: … refine: … }`）的正準編碼，走 `REAL_03` §6.2 同一張表；舊 commit 以舊算法雙讀，選擇子由宣告推進（一次明示的 `migrate`） | 關 | 關 | 關 | 新 commit 關；舊的釘在 Unicode 17 並自陳 | 一次紀元（新 commit 才動）；commit 物件改走值的編碼器 |
| **乙** | 保留現行算法、把它寫成規格（排版文法＋跳脫釘在 Unicode 17），只把漏掉的欄位與清單個數補進 `buf` | 關 | 關 | 關 | **不關**：Rust 的形狀成為規範 | 只動 refine commit；第二個實作要背 Unicode 17 的可印表 |
| **丙** | 先只修 F1：讀取時不再信任存下的 `authority_status`，由簽章對已雜湊的 payload 重驗 | 關（升級方向） | 不關 | 不關 | 不關 | 不動任何位址；「當時的架構師登記」是歷史事實，讀取時重驗不出來 ⟹ 仍要一則小裁定 |

**驗收方建議甲**，理由：它是唯一一個**由構造**讓「磁碟上的每一個欄位都在位址裡」成立的形——F1／F2／F3 是同一個
缺陷的三個實例（位址是手工挑欄位算的），逐欄位補（乙）就是「修個案」。而「commit 是一個值」在本專案裡
早有伏筆：它在磁碟上**已經是**一個 n/ 值，只是位址不是照那個值算的。
**若甲要排在別的身分搬遷之後**（Inbox 有數列寫著「只能併進下一次身分搬遷」：`PathAnchor::Current` 的
`0x03`、弧 D-3），**建議先做丙**，因為 F1 是一個今天就做得到的審計偽造（本機需要 `.oo` 的寫入權——`REAL_01` §6.3.3 把工作區放在斷言層；**對等面未量**，那才是它的重量所在）。

**已知不在本報價內**：`oo inspect <commit>` 印出 `parent: (none)` 的 refine commit（祖先走 ○ 註記，D55）
——那是既有設計，不是本題。
