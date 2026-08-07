# W0′ 交接:一個不改變任何行為的偵測,不是偵測

**開弧日**:2026-08-07
**基線**:`dev b5f39bc`(= `top 86b1bc2`,v0.11.0)
**來源**:`nlang-spec meta/oo/STATUS.md` W0′;`docs/discussion/028` §8.4
**前一弧**:`docs/atomic_writes_handover.md`(v0.11.0)。本弧收它的殘帳,並加上一件更大的。

---

## 0. 一句話

v0.11.0 讓耐久寫入不再撕裂。本弧治的是**下一層**:

> **有三個地方偵測到了問題,然後什麼也沒改變。**
> 寫入失敗被 `.ok()?` 吞掉;完整性裁決被印出來但不擋回收;而回收路徑根本沒有重算位址。

三者是同一個類別,而其中最大的一件**已經違反一條 2026-07-26 就寫下的 MUST**。

---

## 1. 缺陷

### 1.1 `run_gc` 在走訪不完整時照樣清掉物件 ── **既有 MUST 之不合規**

`gc.rs` 的可達性走訪(`mark`):

```
gc.rs:98    物件不存在        → continue,不記錄
gc.rs:101   seen.insert(d)     ← 在讀取與解碼「之前」
gc.rs:102   read_raw_digest 失敗 → 記 integrity #object_undecodable → continue
gc.rs:108   JSON 解不開         → 記 integrity #object_undecodable → continue
gc.rs:115   refs_of(&json, …)   ← 只有解得開的才會被展開
gc.rs:161   run_gc:取得 report 後，從不讀 report.integrity，照刪所有非 live
```

`continue` 的語義是**把它當成葉子**。物件自己因為在 `seen` 裡而不會被掃——`GcReport.integrity`(`gc.rs:24`)的註解「reported, never swept」對**它自己**為真——但**只能經由它到達的後代不在 `live` 裡,於是被掃掉**。註解對後代為假。

**〔量 2026-08-07〕** 操作者看到的是:`format_plan_report` 印出 integrity 那一行,緊接著 `format_done_report` 印出「removed N objects, freed M bytes」。**兩行之間沒有任何東西說它們有關。**

> **這不是「沒人讀」,是「讀了、印了、但不擋」。**

**規範依據不需要新裁定。REAL_03 §6.6 已經逐字治了這件事:**

> **消費端不得丟棄裁決(MUST)**:偵測若被呼叫端沉默丟棄則等同未偵測。任何在讀取失敗時**跳過**檢查而繼續的路徑,**必須**分流:僅「不存在／不透明」得依 §9.1 續行;`#caid_mismatch` 與 `#object_undecodable` **必須**中止該項檢查並回報。

`mark` 的 `continue` 正是該款所禁的那個形狀。

### 1.2 同一條路徑從不重算位址

`mark` 用的是 `storage.rs:209 read_raw_digest`:

```rust
pub fn read_raw_digest(&self, digest_hex: &str) -> Result<Vec<u8>> {
    let p = self.digest_path(digest_hex);
    if !p.exists() { anyhow::bail!("not found"); }
    Ok(fs::read(p)?)          // ← 位元組原樣回傳,不重算
}
```

對照 `storage.rs:224 get_value` ── 它**有**重算並回 `CaidMismatch`。**兩條讀路徑,只有一條守 §6.6 的「重算義務(MUST)」。**

後果比 §1.1 更難看:一個位元組被替換成「合法 JSON、但是別的值」的物件,GC 走訪會**跟著竄改者的引用走**,於是真正的後代不可達 → 被掃掉。而 `#caid_mismatch` 在這條路徑上**根本不會被產生**。

**注意 §6.6 同時警告了修法的陷阱**:「裁決必須為真(MUST)」——`#object_undecodable` **不得因解碼器選錯而發出**。而這個庫**同時裝值與 Commit**(v0.2.52 判例:`oo inspect` 對引擎自己寫的 Commit 回 `#object_undecodable`)。目前的 `mark` 解成泛型 `JsonValue`,所以它**不會**誤判——修法不得把這個性質弄丟。

### 1.3 v0.11.0 的殘帳:兩處寫入沒有走那條唯一的實作

```
peers.rs:451   let tmp = path.with_extension("directory.tmp");   ← 可預測的暫存名
peers.rs:452   fs::write(&tmp, &body).ok()?;                     ← 無 fsync,吞錯
peers.rs:453   fs::rename(&tmp, &path).ok()?;                    ← 吞錯
peers.rs:421   if let Some(c) = compact(…)                       ← 呼叫端也不看 None
oodp.rs:630    write_file:std::fs::write(path, body)            ← 裸寫(錯誤有傳播)
```

`.ok()?` 使**壓實失敗與「這次不需要壓實」不可分**。呼叫端 `if let Some(c)` 只是少印一行日誌。

**可預測的暫存名是獨立的第二個缺陷**:`atomic_write`(`storage.rs:14`)用 `tempfile` 取唯一名;`with_extension("directory.tmp")` 對每個行程都一樣 ⟹ **兩個並行寫者在同一個暫存檔上交錯,然後其中一個把交錯的結果 rename 上去**。v0.11.0 消滅的正是這個結果,只是換了條路徑進來。

`discovery_config.rs:65` 有**同一個暫存名問題**,但其餘正確(`File::create` + `write_all` + `sync_all` + `rename`)。它不是缺陷路徑,它是**第二份實作**。

---

## 2. 裁定

| # | 裁定 |
| :-- | :--- |
| **R1** | **§1.1／§1.2 不需要新裁定**——REAL_03 §6.6 的三條 MUST(重算義務／消費端不得丟棄裁決／裁決必須為真)已經覆蓋。本弧是**符合性**工作,不是規格工作。⟹ **收尾記入 `meta/ENGINE_SYNC.md`,不進規格正文。** |
| **R2** | 走訪遇到 `#object_undecodable` 或 `#caid_mismatch` 時,**`run_gc` 不得清除任何物件**,並以非零狀態回報。理由:回收是**不可逆且特權**的(SPEC_08 §6.2),而一次不完整的走訪算出的 `live` 是一個**假的**答案。**不設覆寫旗標**——需要時另開。 |
| **R3** | **「不存在」維持 `continue` 且維持不記錄。** §6.6 明文允許「不存在／不透明」依 §9.1 續行。⟹ 這不是缺陷,**且交付不得把它改成致命**。(此項同時關掉 `STATUS.md` O34 的後半。) |
| **R4** | 耐久寫入**只能有一份實作**:`storage::atomic_write`。`peers::compact`、`AffiliationClaim::write_file`、`DiscoveryConfig::write` 三處全部改走它。 |
| **R5** | 壓實失敗**必須**與「不需要壓實」可分,並且**必須**被呼叫端看見。 |

---

## 3. 要求交付的內容

### 3.1 GC(§1.1、§1.2)

1. `mark` 讀取物件後**必須重算位址**並與請求的 digest 比對。**必須**分別支援值與 Commit 兩種解碼,兩者皆失敗才發 `#object_undecodable`(§6.6 裁決必須為真;勿重蹈 v0.2.52)。
2. 走訪必須把三種結果分開:`verified` / `#caid_mismatch` / `#object_undecodable`,並在後兩者發生時把**走訪標記為不完整**。
3. `run_gc`:走訪不完整 ⟹ **不清除任何物件**,回錯誤。`plan_gc`(dry-run)仍**必須**照常列出它看到的一切——診斷不得跟著一起消失。
4. **不存在**的物件維持現行行為(R3)。

### 3.2 寫入(§1.3)

5. `peers::compact`:改用 `storage::atomic_write`;錯誤**回傳**而非 `.ok()?`;呼叫端(`peers.rs:421`)必須把失敗記進它回傳的 logs。
6. `AffiliationClaim::write_file`:改用 `storage::atomic_write`。
7. `DiscoveryConfig::write`:改用 `storage::atomic_write`,刪掉那份重複實作。

### 3.3 明確**不在**本弧範圍

| 不做 | 為什麼 |
| :-- | :--- |
| `value.rs:1914` 節點私鑰寫入 | 它靠 `create_new(true)` 取得「並行首次鑄造恰好一個贏家」,而 `load_or_mint` 的註解明文依賴該性質。**temp+rename 會把它破壞掉。** 需要一個裁定(候選:`hard_link` 同時給原子性與互斥),**另開帳** |
| `peers.rs:400` 追加路徑只 `flush` 不 `sync_all` | 追加日誌對「撕裂的最後一行」有既定容忍(讀路徑逐行 `.ok()` 略過)。**每則廣告都 fsync 是效能裁定**,不在此弧。**但**同處 `let _ = writeln!(f, "{h}")` 丟棄表頭寫入結果,**這一件請一併修**(表頭失敗而記錄成功 ⟹ 無表頭檔案) |
| `builtins/io.rs:48,102`(`~%IO` 寫檔) | 那是 **n/ 程式**在寫檔。「語言的寫入是不是原子的」是 SPEC 的語義問題,不是引擎耐久狀態 |
| `main.rs:1136`(`oo fmt --write`) | 寫的是**使用者原始碼**不是 `.oo/`。崩機中途會毀掉來源檔——真的,但屬另一個範圍。**另開帳** |
| 並行 evolve 的遺失更新 | 屬 W12(CAS+重試) |

---

## 4. 可滿足性檢查

工單自身先跑一次,確認沒有要求一個造不出來的世界:

| 要求 | 可滿足嗎 |
| :-- | :--- |
| 造一個「可達但不可解碼」的物件 | ✅ 物件檔案就在 `.oo/objects/sha256/`,測試側可直接覆寫其位元組。**探針必須自己造,不能等它自然發生。** |
| 造一個「可達但位址說謊」的物件 | ✅ 把某個物件的位元組換成**另一個合法物件的位元組**。兩者都是合法 JSON,故只有重算抓得到 |
| 斷言「後代被掃掉」 | ⚠ **不能用引擎自己的走訪算預期存活集**。既有 `local_gc_probe_test.rs:133 refs_of` 是測試側走訪,但**它與引擎共用同一個盲點**(也是 JSON 解析)。⟹ 預期集**必須在竄改之前**先算好並記下來 |
| 斷言「壓實失敗被看見」 | ✅ 令目錄不可寫(或令暫存路徑指向不可寫處)使寫入必失敗 |
| 斷言「兩個並行寫者不交錯」 | ⚠ 這是機率型的。**改用決定性判準**:斷言暫存檔名對同一路徑的兩次呼叫**不相同**(v0.11.0 用過的 inode 簽名同類手法) |

---

## 5. 探針(驗收方所有)

新檔 `crates/oo/tests/verdict_must_gate_probe_test.rs`。命名依既有慣例:`C*` = control、`R*` = 交付前必紅、`P*` = 交付前必綠之釘。

| # | 內容 |
| :-- | :--- |
| **C1** | **control,必須第一個跑**:未經竄改的倉庫,`oo gc --grant gc` 成功,且回收集合等於獨立算出的不可達集合。若 C1 掛,底下每一支紅都可能只是因為「什麼都壞了」 |
| **C2** | **control**:同一次執行內斷言一個「存在」——竄改前先記下某個**存在且可解碼**的後代 digest,證明該後代在基線是被走訪到的。(避免「斷言不存在」的紅在基線就假綠) |
| **R1** | 可達物件被改成不可解碼 ⟹ `oo gc` **不得清除任何物件**,且以非零狀態退出 |
| **R2** | 同上情形 ⟹ **只經該物件可達的後代仍在磁碟上**(預期集在竄改前算好) |
| **R3** | 可達物件的位元組被換成**另一個合法物件**的位元組 ⟹ 必須產生 `#caid_mismatch`(今天完全不會出現此裁決) |
| **R4** | `peers::compact` 在寫入必然失敗的環境下 ⟹ 失敗**必須**出現在回傳的 logs 中,且**不得**與「不需要壓實」同形 |
| **R5** | `AffiliationClaim::write_file` / `DiscoveryConfig::write` / `peers::compact` 三者產生的暫存檔名,對同一目標路徑的兩次呼叫**必須不同** |
| **P1** | **不存在**的可達 digest 仍讓 GC 正常完成(R3 裁定的反面釘——防止交付過度修正) |
| **P2** | `oo gc --dry-run` 在**竄改後**仍**必須**印出它看到的一切(診斷不得隨著把關一起消失) |
| **P3** | 引擎自己寫的 **Commit** 物件**不得**被判為 `#object_undecodable`(v0.2.52 判例的迴歸釘;修法引入第二個解碼器時最可能撞到) |
| **P4** | 未竄改倉庫的 `oo gc` 輸出與基線**逐字相同**(除非本弧刻意改字) |

**校準**:R1–R5 在 `dev b5f39bc` 上必須全紅**且紅在對的理由上**;C1、C2、P1–P4 必須全綠。紅的以 `#[ignore]` 標記,交付方**只得移除 `#[ignore]`**,不得編輯探針。

---

## 6. 驗收量測

1. **diff 純度**:交付只碰 `gc.rs`、`storage.rs`、`peers.rs`、`oodp.rs`、`discovery_config.rs`,以及必要的呼叫端。
2. **獨立重跑**:整個 workspace 全跑,不只本套件(v0.10.0 的 `advert_persistence` 教訓——只有獨立重跑未動的套件才抓得到)。
3. **重複穩定**:候選樹連跑五次。
4. **對抗**:R3 的竄改載荷**必須是一個會被真的走訪的合法物件**,不能只是形狀錯的位元組(v0.2.50 教訓)。
5. **跨版本**:v0.11.0 建立的倉庫,用交付版 `oo gc --dry-run` 開啟,報告數字不變。

---

## 7. 掛帳(本弧開出,不在本弧解)

| 內容 | 為什麼 |
| :-- | :--- |
| `value.rs:1914` 私鑰寫入非原子,而正確修法會與 `create_new` 的互斥性衝突 | 需裁定;候選 `hard_link` |
| `oo fmt --write` 崩機中途會毀掉使用者原始碼 | 不是 `.oo/` 狀態,但是使用者資料 |
| `peers.rs:400` 追加不 fsync | 效能裁定,非缺陷 |
| `~%IO` 的寫入語義是否該是原子的 | SPEC 問題 |

---

## 8. 交付紀錄(交付方填)

*(留白)*
