# 偵察：一把還沒寫完的金鑰

> 起因：Q-050 驗收的全樹 ×3，run1 唯一的紅是
> `identity_persistence_probe_test::pin_concurrent_first_mint_yields_one_key`
> ——而那一列 Inbox 逐字寫著「驗收方四次全跑一次都沒有重現」。**本次是第一次。**
> 量測用 `v0.54.0` 標籤建置。known-answer：好金鑰時 `oo identity` rc=0 並印出 64-hex。

---

## 1. 判別 (b) 先答，因為它決定這張卡的嚴重度

Inbox 那一列掛著 `interrupt-candidate`，理由是「若引擎拿壞金鑰去簽，就要升級」。
**答案是不會。**〔量，控制組活的——好金鑰時 `refine --sign` 的拒絕理由是**單調性**
（`new ⋢ old`，表示它過了簽章那一步），半截金鑰時是**簽不出來**〕

| 路徑 | 半截金鑰（截成 20 B） |
| :-- | :-- |
| `oo identity` | rc=1，`not a valid PKCS#8 Ed25519 key (KeyRejected("InvalidEncoding")); file left unchanged`，**事後檔案大小未變** |
| `oo refine --sign` | rc=1，**`Signing failed: identity file …: not a valid PKCS#8`** |
| `oo commit` | **rc=0 成功**——**而那是對的**：commit 根本不簽（`meta.author` 是固定字串 `"oo-cli"`） |

〔讀 `authority.rs:23`〕`sign_refine` **自己重驗一次** `from_pkcs8`
⟹ **壞位元組到簽章之間沒有路**。
**⟹ 降級為健壯性列**：失敗形是**一次響亮的暫時性拒絕**，不是資料遺失、不是假裁決。

## 2. 成因：過去的修補保護錯了對象

〔讀 `value.rs`〕

```rust
fn create_new_at(&self, path) {
    let mut f = opts.open(path)?;        // create_new + 0600：名字被原子地佔下
    f.write_all(&self.private_key)?;      // 內容在那之後才到   ← 視窗
    f.sync_all()?;
}
pub fn load_or_mint(path) {
    if path.exists() { return Self::load(path); }   // ← 一次，沒有重試
    …
    Err(AlreadyExists) => Self::load_after_race(path),   // ← 100 × 1ms 有界重試
}
```

`load_after_race` 的 docstring 自己就寫著「`create_new` claims the path before the bytes
are written, so a loser arriving inside that window would read an empty file」
——**它保護了「輸掉 `create_new` 的賽跑者」，沒保護「根本沒參賽的旁觀者」**。
而旁觀者走的是第一行那條 `path.exists()`，**一次，沒有重試**。

## 3. ⚠ 驗收方第一次的「機制確認」重現的是錯誤訊息，不是缺陷

第一次驗證時把金鑰檔弄成 0 B，得到逐字相同的錯誤，就記下「機制確認」。
**那是錯的**：一個**永久** 0 B 的金鑰檔報錯是**正確行為**，重試一萬次也該報錯。
**缺陷只在「暫時不完整」那一格**——檔案現在不完整，而幾十微秒後會完整。

〔量，把兩者分開構造〕

| 構造 | rc | 判讀 |
| :-- | --: | :-- |
| **A** 永久 0 B（沒人在寫） | 1 | **正確，必須保持** |
| **B** 先 0 B，背景 20ms 後寫入完整金鑰 | **1** | **缺陷**——它沒有等，而重試預算是 100ms |
| 對照 金鑰已完整 | 0 | 印出的 hex ＝ 檔案裡那把 |

**⟹ 這一格是本弧唯一的紅，而它必須這樣構造才紅。**
用 (A) 當紅探針會釘住一個錯的期待；用既有那支併發探針則是 1／3 的隨機紅。

## 4. 紀錄：本弧的紅探針是「確定性化」一個既有的隨機紅

既有 `pin_concurrent_first_mint_yields_one_key`（**別弧的探針，本弧不得動**）
測的是同一個性質，而它 1／3 紅。**本弧的 R 用 (B) 的構造把它變成確定的**
⟹ 修完之後那支間歇紅也應該停止，**但那是推論不是本弧的斷言**，
因為它的隨機性還有第二個來源（`load_after_race` 那條分支本身是否夠長）。

## 5. 開卡前沒量的

*   **`~/.oo` 之外還有誰走 `load_or_mint`**：節點金鑰（`resolve_node_home`）是不是同一條路，未量。
*   **`load_after_race` 的 100 × 1ms 夠不夠**：在高負載下（全樹 ×3 的情境）未量。
*   **`f.sync_all()` 之後、但 `create_new` 的 metadata 尚未 sync 時斷電**會怎樣，未量（不同題）。
*   **驗收方一次量測失誤**：用 `i:` 當欄位名得到 `Error: Not a path`
    ——**`i` 是虛數單位**（`SYNTAX_02` §4.10），那是量測踩到語言，不是引擎。
