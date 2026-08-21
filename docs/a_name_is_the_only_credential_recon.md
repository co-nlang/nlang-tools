# 偵察：名字是唯一的憑證 —— a name is the only credential

> 開單 2026-08-21。工單 `docs/a_name_is_the_only_credential_recon_order.md`。
> **本檔只偵察。** 不改引擎、不改規格、不寫探針、不裁定。
>
> **基線（跑出來的）**：
> *   真二進位 `nlang-tools/target/debug/oo` → `oo v0.28.0-642-ge7b84dd`
>     （mtime 2026-08-21 00:56；`git describe` 停在 tie-back `e7b84dd`）。
> *   工作樹 `nlang-tools` HEAD `ed0ef5f`（相對 `e7b84dd` 只多本工單，引擎位元組未動）。
> *   規格工作樹 `nlang-spec` HEAD `153cbab`，`VERSION` = `0.28.0-draft.1`。
>     開單寫的是 `12071b1`；其後 Inbox 編輯未改 SPEC_05 §3.3 本條。
>
> 量測倉在 `/tmp/q035-*`，不進工作樹。

---

## 1. 一句話

施用時**只**拿 combo 的 `%builtin` 字串去查 `builtin_registry`。
值裡**沒有任何位元組**記得這欄是引擎投影的還是使用者寫的。
`~%Math./add` 與 `{{ %builtin: "math.add", %morphism: #true }}` **同一 CAID**。
因此 SPEC_05 §3.3 的 MUST NOT 不是「加一個檢查」——它要求一個目前不存在的來源區分。

---

## 2. 開單已量、本單複驗仍成立

〔跑，`ge7b84dd`〕

| 形 | 結果 |
| :--- | :--- |
| `{{ %builtin: "math.add", %morphism: #true }} (1,2)` | `3` |
| `{{ %builtin: "math.add" }} (1,2)`（無 `%morphism`） | `3` |
| `~%Math./add (1,2)` | `3` |
| `{ %builtin: "math.add", %kind: #morphism } (1,2)` | `⊥ #conflict` |
| `{{ %builtin: "process.exit", %morphism: #true }} 7` | **exit=7** |

單括號對照組不重做：開單已證明那個 ⊥ 與 `%builtin` 無關。

---

## 3. 六項量測

### 3.1 派送點在哪一行 〔讀碼 + 跑〕

**唯一**拿這個字串去查表的地方：

`crates/interpreter/src/lib.rs` `Ouroboros::apply_morphism`，約 3064–3066 行：

```
if let Some(Value::Atom(AtomKind::Str(builtin_id), _, _)) = c.get_field("%builtin") {
    if let Some(func) = self.builtin_registry.get(builtin_id) {
        let res = func(unified_arg.clone(), self, ctx);
```

`get_field("%builtin")` 〔讀 `value.rs:1038–1040`〕走 **meta 軸**（`%` 前綴剝掉之後的鍵 `builtin`）。

表本身 〔讀 `builtins/mod.rs:30` `create_default_builtins`〕是進程內 `HashMap<String, Arc<BuiltinFn>>`，
由 `Ouroboros` 建構時填一次。Genesis 〔讀 `lib.rs` `v0_22_standard_root`〕只是把**同一個字串**寫進標準根 combo 的 meta。

`%morphism` **不是**派送條件（開單已量；本單複驗無 `%morphism` 仍回 `3`）。
它只影響「這是不是態射」的判讀 〔讀 `value.rs:2791–2806` `is_morphism`〕。

miss 時不報「未知內建」：registry 沒有這個名字就落到後面的欄位查找，最後 `⊥ #conflict`。
〔跑〕`{{ %builtin: "no.such.thing" }} (1,2)` → `_|_ (%cause: #conflict)`。

**它有沒有辦法分辨引擎放的與使用者寫的？沒有。** 下一項是同一件事的位元組證明。

---

### 3.2 今天有沒有任何一個地方分得出來 〔讀碼 + 跑〕

**沒有。**

`ComboVal` 〔讀 `value.rs:880–908`〕可序列化的欄是
`data / types / rules / meta / system / local / closed / effect / relations / masa_ref`。
沒有 provenance、origin、span、source。
`cache_id` / `cycle_frame_id` / `pending_spreads` 標了 `serde(skip)`，不進 CAS。
AST 的 `Span` 在進值時丟掉。

〔跑〕同一內容、同一位址：

| 值 | `~%Discovery./identify` |
| :--- | :--- |
| `~%Math./add` | `…c58035a589f8104840aa743957c8b2a4c40b5a6161e0618e8b5ea1e0a2109174` |
| `{{ %builtin: "math.add", %morphism: #true }}` | **同一個** |
| `=` | `#true` |
| 印出 | 兩邊都是 `{{ %builtin: "math.add" / %morphism: #true }}` |

帶效果的引擎投影，差在 `ComboVal.effect`（印成 `%effect`），不是出處位元。
使用者把那一欄也寫上，CAID 就對齊：

| 值 | identify 尾碼 |
| :--- | :--- |
| `~%Math./random` | `74adc1dc…` |
| `{{ %builtin: "math.random", %morphism: #true }}`（缺 `%effect`） | `8630ab95…`（不同） |
| `{{ %builtin: "math.random", %morphism: #true, %effect: #nondet }}` | **`74adc1dc…`（同）** |
| `~%Io./write_file` vs 寫上 `%effect: #io` 的偽造 | **同** `b15a1d9e…` |
| `~%Process./exit` vs 寫上 `%effect: #io` 的偽造 | **同** `882f22d9…` |
| `~%Query./select` vs `{{ %builtin, %morphism, %kind: #logic }}` | `#true`（開單那一列的三欄形） |

⟹ 標準根投影出來的態射，與使用者寫的同形值，**可以逐位元組相同**。
引擎沒有任何多出來的秘密欄位。

**別名與偽造是同一個值。** 〔跑〕

```
add: ~%Math./add
r: add (1,2)
```

`fmt` 仍印 `add: ~%Math./add`（路徑還在源碼裡）。
evolve／commit 之後，使用者根物件的 JSON 是：

```
data.add.meta.builtin = "math.add"
data.r = 3
```

與手寫 `evil: {{ %builtin: "math.add", %morphism: #true }}` 的 combo **同一形狀**。
「從 `~%` 抄過來」在耐久層**不留下**「這是抄來的」的痕跡。

另：引號資料鍵不派送。〔跑〕`{ "%builtin": "math.add", %morphism: #true } (1,2)` → `⊥ #conflict`。
`get_field("%builtin")` 只看 meta 軸，資料軸上名叫 `%builtin` 的鍵不是憑證。
這不是守衛，只是軸路由。

---

### 3.3 四個拒絕層各自的可觀測後果 〔跑〕

今天四層都**不**拒絕。下列是「現況」與「若在那一層拒絕、其餘不動」的可觀測差。
拒絕的**機制**尚未存在，故「若拒絕」是用現有錯誤形狀當容器，不是臆造新 UX。

試驗檔 `forge.n`：

```
evil: {{ %builtin: "math.add", %morphism: #true }}
out: evil (1,2)
```

| 層 | 今天（跑） | 若在該層拒絕 |
| :--- | :--- | :--- |
| **parse**（`oo fmt`） | exit=0，把 `%builtin` 印回去。`fmt` **不施用**：`x: {{ %builtin: "process.exit" }} 7` 格式化後行程仍在。語法錯的現成形狀是 `Error: Parse Error: …` exit=1。 | 使用者看到 `Parse Error`。staged 不出現（evolve 到不了）。既有倉不動。源碼裡的 `%builtin:` 寫不出來。`add: ~%Math./add` **仍可寫**（源碼沒有這個欄名）。 |
| **evolve**（`oo evolve`） | exit=0，無訊息。staged JSON 已含 `meta.builtin`。**施用發生在 evolve 裡**：`x: {{ %builtin: "math.add" }} (1,2)` 的 staged 是 `x: 3`，不是態射；`x: {{ %builtin: "process.exit" }} 7` → **exit=7**，staged **沒寫出**（`save_staged` 在返回之後）。衝突的現成形狀是 `Evolution Conflict in "…": #conflict at …` exit=1。 | 若拒絕發生在 `apply_morphism` **之前**：應用與綁定都被擋，行程不會被 `process.exit` 殺掉，staged 維持上一狀態（`run_evolve` 失敗不 `save_staged`）。若拒絕發生在「看 staged 樹有沒有這個欄」而施用已做完：`process.exit` 已經終止，拒絕句印不出來。既有 HEAD 不動。 |
| **commit**（`oo commit`） | exit=0，`Commit successful: hash:sha256:v1:e69a3aee…`。根物件明文含 `"builtin":{"Atom":[{"Str":"math.add"},0,null]}`。空提交的現成形狀是 `Nothing to commit` exit=1。 | 使用者看到 commit 失敗（現成是 stderr + exit=1）。staged **仍留著**偽造值。HEAD／既有物件不動。下一次 `oo run` 同一份源碼仍會再走 evolve→apply。 |
| **apply**（`apply_morphism`；CLI 上是 `eval`／`run --observe`） | `eval` 與 `run --observe out` 都回 `3`；`process.exit` 終止。`eval` 看不見倉 〔跑〕提交後 `eval 'evil (1,2)'` → `_`——這是 Q-018，不是拒絕。 | 若只擋 CLI、不擋 `apply_morphism`：evolve 已經施用過了。若擋 `apply_morphism` 本身：evolve／eval／run 一齊停。因為沒有出處位元，這一擋**分不出** `~%Math./add` 與使用者寫的同形值（見 §3.2）。 |

**承重的結構事實**：evolve 不是「只搬結構」。綁定不施用；**施用式在 evolve 就派送**。
要把 `process.exit` 擋在終止之前，拒絕點必須在 `builtin_registry.get` **之前**，不能只放在 commit 或「觀察 CLI」。

---

### 3.4 既存耐久值：標準根會不會被讀取路徑拒絕 〔跑，不要推理〕

標準根 digest 〔跑 `oo status`〕：

`7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`（available）

落地形 〔讀 `storage.rs:328–337` + 跑解碼〕：

```
"standard-root:" + hex(canonical_cas_json(ComboVal))
```

明文 grep `math.add` **不會**命中它（開單已說；複驗：objects 裡唯一明文 `math.add` 是使用者根）。
hex 解碼之後：

| | 數 |
| :--- | ---: |
| `meta.builtin` 出現次數 | **255** |
| 相異內建名 | **251** |
| 解碼後含 `math.add` | 是 |

255−251 = 4 次重複指到同一個名字：`engine.differential` ×3、`engine.save` ×2、`list.map` ×2。

`get_value` 〔讀 `storage.rs:392–407`〕對 `standard-root:` 前綴是 hex→`ComboVal` JSON，
**同一套** `ComboVal` 反序列化，沒有第二種「引擎專用、不含 `%builtin`」的值。

〔跑〕這個倉 `oo status`／`oo log`／`oo run` 都成功，標準根是 available。
今天讀取路徑**沒有**拒絕 `%builtin`。

若在「任何解出來的 `ComboVal.meta` 含 `builtin` 就拒絕」：

*   使用者根 `2e31a3fb…` **會**被拒（明文就有）。
*   標準根 `7038e250…` **也會**被拒（255 處 `meta.builtin`）。
*   後果 〔對現成讀取失敗形〕：`oo log`／`inspect`／hydrate 走 `StoreReadError` 一族；
    沒有標準根就沒有 `~%Math`。這不是「使用者偽造被擋、標準庫還在」。

parse 層拒絕**打不中**標準根：它不是 n/ 源碼，是 hex 物件。

---

### 3.5 偽造射程：registry 全表 〔讀碼 + 跑標註〕

`create_default_builtins` 的 `m.insert("…")` 與 `math_cmp_pred!("…")`，扣掉建 list 時的 `"%kind"`：
**245** 個相異名、245 個插入點，未截斷。

標準根 251 個相異名 = 這 245 個 **加上 6 個只被投影、沒有註冊的名字**：

`math.bitAnd` `math.bitNot` `math.bitOr` `math.bitXor` `math.shl` `math.shr`

〔跑〕`~%Math./bitAnd (1,2)` 與 `{{ %builtin: "math.bitAnd" }} (1,2)` 都是 `⊥ #conflict`。
這六個名字在標準根裡是死欄，**不是**能力。

`(a)(b)(c)(d)` 的操作型定義（工單原句）：拿到名字之後能否造成
行程終止／檔案系統寫入／網路／非決定性。
**效果標籤不是這四格的定義**（`#io` 含讀檔與時鐘；`#nondet` 全表只有一處）。

#### 四格有命中的（跑或讀到系統呼叫）

| 名 | (a) 終止 | (b) FS 寫 | (c) 網路 | (d) 非決定 | 量 |
| :--- | :---: | :---: | :---: | :---: | :--- |
| `process.exit` | ● | | | | 偽造施用 exit=7；真投影印 `%effect: #io` 但 CAID 可被 `%effect: #io` 對齊 |
| `io.write_file` | | ● | | | 無 `--grant` 即寫出 `/tmp/q035_w.txt`，回 `#true  ;; %effect: #io`。真 `~%Io./write_file` 同樣無 grant 就寫 |
| `io.append_file` | | ● | | | 無 grant，檔案變 `from-forgemore` |
| `engine.save` | | ● | | | 無 grant，`.oo/objects` 5→6；寫入的是 CAS 物件（store 邊界擋的是語言層路徑裡的 `.oo`，擋不住這個內建） |
| `disc.connect` | | | ● | | 無 grant → `⊥ #privileged_required`。`--grant connect` → `#true`，把 `tcp://` 放進 peer 表（讀碼：此時未 dial） |
| `disc.fetch` | | | ● | | 對已 connect 的 `tcp://127.0.0.1:1` 施用 → `⊥ #conflict`（讀碼：`TcpStream::connect_timeout` 5s；連線被拒走 Conflict 而非 Timeout） |
| `math.random` | | | | ● | 兩次偽造得 `617`／`221`，`%effect: #nondet`。全 registry 唯一 `EffectTag::NonDet` |

#### 四格皆空、但宿主可見（標籤是 `#io`，非正式的「每次不一樣」或讀）

這些**不是** (a)(b)(c)(d)，列出來以免把「IO」誤讀成「寫入／網路」：

| 名 | 量／讀 |
| :--- | :--- |
| `io.read_file` | 偽造讀回 `"from-forgemore"  ;; %effect: #io`（讀，不寫） |
| `io.exists` | 偽造回 `#true  ;; %effect: #io` |
| `csv.read_csv` | 偽造讀 `/tmp/q035_w.txt` → `[["from-forgemore"]]` |
| `process.pid` | 偽造回 pid，`#io` |
| `env.get` / `env.args` / `env.cwd` | 偽造 `env.get "HOME"` → `"/home/gali"  ;; %effect: #io` |
| `time.now` | 偽造回毫秒時間戳，`#io`（非正式不重複；標籤不是 `#nondet`） |
| `disc.advertise` | 讀碼：寫進程內 GBB 表，不是線上 `#advertise` |
| `query.where` | 讀碼：結果帶 `EffectTag::IO` |

`path.*` 是純字串，不碰 FS。

#### Registry 全名（245，字典序，未截斷）

```
bytes.at
bytes.base64_decode
bytes.base64_encode
bytes.concat
bytes.from_hex
bytes.from_str
bytes.hmac_sha256
bytes.len
bytes.sha256
bytes.slice
bytes.to_hex
bytes.to_str
complex.conj
complex.imag
complex.phase
complex.real
cond.cond
cond.if
cond.match
csv.parse
csv.parse_with_headers
csv.read_csv
csv.stringify
diff.diff
diff.is_compatible
diff.patch
disc.advertise
disc.connect
disc.fetch
disc.find
disc.identify
effect.run_pure
engine.check_oml
engine.differential
engine.equivalence_map
engine.observe
engine.project_down
engine.project_up
engine.resolve
engine.save
engine.set_strategy
env.args
env.cwd
env.get
io.append_file
io.exists
io.read_file
io.write_file
json.get
json.keys
json.parse
json.stringify
list.all
list.any
list.at
list.chunk
list.concat
list.count
list.dedup
list.drop
list.drop_while
list.enumerate
list.filter
list.find
list.flat_map
list.flatten
list.fold
list.group_by
list.head
list.intersperse
list.len
list.map
list.max_by
list.min_by
list.partition
list.product
list.range
list.reduce
list.reverse
list.scan
list.slice
list.sort
list.sort_by
list.sum
list.tail
list.take
list.take_while
list.transpose
list.unique
list.window
list.zip
list.zip_with
math.abs
math.add
math.atan2
math.bits
math.ceil
math.choose
math.clamp
math.cos
math.cosh
math.div
math.eml
math.exp
math.factorial
math.floor
math.fract
math.gcd
math.gt
math.gte
math.hypot
math.is_prime
math.lcm
math.ln
math.log10
math.log2
math.lt
math.lte
math.max
math.min
math.mul
math.pow
math.pow_mod
math.random
math.rem
math.round
math.sign
math.sin
math.sinh
math.sqrt
math.sub
math.tanh
math.to_float
math.trunc
option.and_then
option.expect
option.filter
option.flatten
option.map
option.or
option.unwrap_or
option.zip
path.basename
path.dirname
path.extension
path.is_absolute
path.join
process.exit
process.pid
query.deep_merge
query.pluck
query.select
query.where
refl.bottom_cause
refl.delete
refl.entries
refl.get
refl.has
refl.is_blur
refl.is_bottom
refl.is_cocoon
refl.is_err
refl.is_none
refl.is_ok
refl.is_some
refl.keys
refl.set
refl.to_str
refl.type_of
refl.values
regex.find
regex.match
regex.replace
regex.split
result.and
result.and_then
result.expect
result.flatten
result.map
result.map_err
result.or
result.unwrap
set.contains
set.difference
set.from_list
set.intersection
set.is_disjoint
set.is_subset
set.is_superset
set.union
stat.histogram
stat.mean
stat.median
stat.percentile
stat.std_dev
stat.variance
str.char_at
str.chars
str.concat
str.contains
str.count
str.decode_uri
str.encode_uri
str.ends_with
str.format
str.from_int
str.index_of
str.is_empty
str.join
str.len
str.levenshtein
str.lines
str.pad_left
str.pad_right
str.parse_float
str.parse_int
str.repeat
str.replace
str.reverse
str.slice
str.split
str.starts_with
str.title_case
str.to_lower
str.to_upper
str.trim
str.trim_end
str.trim_start
str.word_count
time.add_days
time.add_hours
time.add_ms
time.diff
time.format
time.now
time.parse
time.to_iso8601
time.weekday
toml.parse
toml.stringify
url.decode
url.encode
url.join
url.parse
url.query_params
```

效果／grant 不是本單的守衛：`io.write_file` 無 grant 仍寫盤；`process.exit` 無 grant 仍終止；
`disc.connect` 的 grant 是第二道門，過了之後名字照樣是憑證。

---

### 3.6 線上 `#advertise` 〔跑，v0.28.0 重測〕

節點 `oo node serve --port 19572`（`ge7b84dd`）。
五種載荷皆 `%status: #rejected` `%reason: #malformed`，節點存活，`/tmp/q035_pwned.txt` 不存在，物件數仍為 3。

| 載荷 | 日誌理由 |
| :--- | :--- |
| `%ad: ~%Io./write_file("/tmp/q035_pwned.txt", "owned")`（施用式） | `advertisement body must be literal data, not an expression` |
| `%ad: {{ %builtin: "process.exit" }} 7`（施用式） | 同上 |
| `%ad: {{ %builtin: "process.exit", %morphism: #true }}`（字面 combo） | **過了字面閘**，然後 `missing required field node_id`。沒有施用。 |
| `%ad: { x: 1 }`（裸 combo） | `missing required field node_id` |
| 無 `%ad` | `missing %ad` |

〔讀 `oodp.rs:523–561, 778–781`〕字面閘是 AST 白名單（原子／list／tuple／combo），
**不**把 `%builtin` 當特殊欄。字面 combo 裡帶 `%builtin` 仍是資料，接著被八欄簽章閘擋下，
`eval_expr_value` 只建構值、不 `apply_morphism`。

⟹ 線上路徑仍關著。本單射程只含本地。一句話帶過，不展開。

---

## 4. 可裁的問題

每題只寫實測後果，不寫優劣。

### Q1. 「使用者資料不得含 `%builtin`」包不包括 `add: ~%Math./add`？

這是本單最重的一題。§3.2 已量：別名與手寫偽造在 CAS 裡是同一個 combo。

| 選 | 實測會變成什麼 |
| :--- | :--- |
| **A. 包**（使用者宇宙裡任何 `meta.builtin` 都不許） | evolve／commit `add: ~%Math./add` 會與手寫 `{{ %builtin: "math.add" }}` 同一理由失敗。O65 之後「交集進路徑即 import」（`_: ~%Math`）會把整張表抄進使用者根，每一欄都是 `meta.builtin`。 |
| **B. 不包**（禁的是源碼裡的 `%builtin:` 欄，不是禁這個值） | parse 擋 `%builtin:` 之後，使用者仍可用 `add: ~%Math./add` 得到**同一份**可提交、可派送的值。耐久層仍是名字當憑證。SPEC 第二句「不得依該名字派送」在 B 之下仍然沒兌現。 |

### Q2. 拒絕發生在哪一層？

四者不等價，§3.3 已量。

| 選 | 實測會變成什麼 |
| :--- | :--- |
| **parse** | 新源碼寫不出 `%builtin:`。`fmt` 失敗形狀已有。既有倉、已提交的 `meta.builtin`、以及 `add: ~%Math./add` 都不動。`process.exit` 的**源碼施用式**進不了 parser；已提交的綁定在 `run` 時仍會派送。 |
| **evolve（看樹、不擋 apply）** | 綁定 `evil: {{ %builtin }}` 可擋。施用式 `{{ %builtin: "process.exit" }} 7` **先派送再失敗**——今天這一形在 evolve 就 exit=7。 |
| **evolve／apply 在 `builtin_registry.get` 之前** | 施用式也停在終止之前；staged 不寫。因為沒有出處，`~%Math./add (1,2)` 走同一行，會一起停（除非另有 Q3 的區分）。 |
| **commit** | staged 已含偽造；`process.exit` 若在 evolve 已施用，commit 根本看不到。HEAD 不動。`oo run` 同一檔仍會再施用。 |
| **只擋 eval／run CLI** | evolve 已經施用。 |

---

## 4.1 驗收方補記（2026-08-21）：Q1 已裁，Q2 的格子要拆

> 以下由**驗收方**寫入，不是偵察方的量測；新量測逐條標明。
> 偵察本文（§1–§3、§4 五題、§5、§6）一字未動。

### Q1 ✅ 已裁（用戶，2026-08-21）：「包」是目標，但兌現方式是**引用**不是**拒絕**

**裁定**：使用者宇宙裡不該有 `meta.builtin` —— 但這句話要靠
「合法路徑根本不產生它」來成立，不是靠一條會誤傷 `_: ~%Math` 的拒絕。
**本題掛在弧 D（位址字面，O67 已裁 `_{…}`）之下，本弧不裁死 import 語義。**

**支持的量測**〔量 2026-08-21，驗收方，v0.28.0 真二進位，使用者根物件大小〕：

| 源碼 | 使用者根 | `builtin` 次數 |
| :--- | ---: | ---: |
| `v: 1`（對照） | **287 B** | 0 |
| `add: ~%Math./add` | 479 B | 1 |
| `/add: ~%Math./add` | 479 B | 1 |
| `add: ~%Math./add` ＋ `/add: add` | 479 B | **1** |
| `/add: ~%Math./add` ＋ `add: /add` | 479 B | **1** |
| **`_: ~%Math`** | **11,625 B** | 多 |
| 手寫偽造 `{{ %builtin: "math.add", … }}` | 479 B | 1 |

兩件事由此定下來：

1.  **宇宙內部的別名已經是引用**——兩個綁定共用一份，`builtin` 仍只出現一次。
    複製只發生在**跨越 `~%` 邊界**那一步。
2.  **跨界那一步是快照不是指標**——`_: ~%Math` 把整個模組抄進根，
    **對照空根是 40 倍**；CAS 去重救不了，因為那份拷貝在根**物件內部**。

**為何裁成「掛在弧 D」而不是現在選 A 或 B**：兩邊都不通。
選 A（包）會讓 `_: ~%Math` 的每一欄失敗；選 B（不包）等於在規格裡寫下
「別名可以拿到同一個值」，替一個要被拆掉的實作背書。
而**這個改動已經做過一次**——**O50（v0.21.0）**：根不存 `system` 的內容改存 digest，
根物件 **72,555 → 1,511 B**，代價是**破壞性 #14，身分軸，90 天時鐘重啟**。
⟹ 價碼是量過的，不是估的；缺的只有「怎麼寫出一個引用」，而那正是 O67。

### Q2 ⚠ 五個選項不在同一條軸上 —— 這是**開單方**的框格錯誤

工單 §3 第 3 項寫「四個拒絕層：parse／evolve／commit／apply」，
把它當成一把梯子。**它不是**：前三個是「值**何時被寫下**」，
apply 是「名字**何時被使用**」。偵察忠實地量了每一格，
橫著讀對不起來是格子的問題，不是量測的問題。

SPEC_05 §3.3 那一句本來就有兩半，各自對應不同的層：

| | 條文 | 治什麼 | 可能的層 |
| :--- | :--- | :--- | :--- |
| **(a)** | 「必須拒絕使用者資料中**出現**的 `%builtin`」 | 值寫進去的時候 | parse／evolve／commit |
| **(b)** | 「**且不得依該名字派送**」 | 名字被使用的時候 | `builtin_registry.get` 之前 |

**⟹ 建議把 Q2 拆成 Q2a（寫入層）與 Q2b（派送層）再裁。**

**決定性量測，證明兩者不是替代關係**〔量 2026-08-21，驗收方〕：

```
# 第一步：提交一個偽造，之後它只活在歷史裡
boom: {{ %builtin: "process.exit", %morphism: #true }}

# 第二步：源碼裡一個 %builtin 都沒有
out: boom 7
```

`oo evolve b.n` → **exit=7，行程終止**。

⟹ **任何放在 parse 的拒絕，都會被一分鐘前提交的宇宙繞過。**
只做 (a)，每一個已提交的宇宙照樣活著；只做 (b)，偽造照樣躺在歷史裡。

⚠ **量這一格時容易被 Q-018 騙**：同一個實驗改用 `oo run b.n --observe out` 回 `_`
（`run` 看不見倉），看起來像被擋住了。**那不是拒絕。** 驗收方第一次即誤讀。

### Q1 對 Q2 的影響：只動 (a)，不動 (b)

*   **(a) 被改變的是性質不是位置**：引用式 import 之下，合法路徑不產生
    `meta.builtin` ⟹ 使用者資料裡出現它就**無歧義地是偽造** ⟹ (a) 從
    「一條會誤傷合法用法的拒絕」變成「一條沒有偽陽性的規則」。
*   **(b) 完全不動**：它受制於偵察 §3.3 的承重句——**施用發生在 evolve**，
    故拒絕點必須在 `builtin_registry.get` 之前。且 (b) 另有一個 Q1 幫不上的難處：
    沒有出處位元，這一擋分不出 `~%Math./add` 與同形偽造（偵察 §3.2）。
    **(b) 的乾淨解也在弧 D 下游**，但路徑不同：(a) 靠「不會產生」，(b) 靠「派送有出處」。

---

### Q3 ✅ 已裁（用戶）：**B —— 不再認名字**，而憑證來自**脈絡**不是來自**值**

**前置已量，且比偵察預估便宜**〔量／讀 2026-08-21，驗收方〕：

*   `ComboVal` 十個可序列化欄位（`data/types/rules/meta/system/local/closed/effect/relations/masa_ref`）
    **無一指向標準根** ⟹ 「該**值**依賴哪個標準根」今天問不出來，且不該用新增欄位去問。
*   但 `EvalContext` 有 `pub standard_root: Arc<ComboVal>`〔讀 `lib.rs:101`〕，
    而派送那一行〔讀 `lib.rs:3064–3066`〕**手上就有 `ctx`**。
    ⟹ **不必新造機制、不必新增欄位、不必動身分。**

**規則形狀**：`%builtin` 之名，只有在**這個宇宙的**標準根確實 project 了它時才派送。
與 **O55** 對齊——`ctx.standard_root` 正是被指名的那一份〔讀 `universe.rs:241`〕，
**舊倉自動得到舊的名字集合**，版本綁定不必另外設計。

**威脅模型同時被更正**〔量，驗收方〕：

| | |
| :--- | ---: |
| registry 相異名 | **245** |
| 標準根相異名 | **251** |
| **registry ＼ 標準根** | **∅** |

且偵察標為危險的七個（`process.exit`／`io.write_file`／`io.append_file`／`engine.save`／
`disc.connect`／`disc.fetch`／`math.random`）**全部路徑可達**。
⟹ **今天偽造拿不到 `~%` 沒給的能力**；這是**課責**缺陷，不是提權缺陷。

**真正的洞**：〔讀 `lib.rs:758`／`:3066`〕`builtin_registry` 是行程內的表，**派送從不查根**。
證據是那 6 個死名——`math.bitAnd` 在標準根裡、不在 registry，故 `~%Math./bitAnd` 取得到態射
而 `~%Math./bitAnd (1,2)` = `⊥ #conflict`。
⟹ **根與 registry 是兩個獨立的集合，今天恰好嵌套是巧合不是機制**，
故 O55 的版本綁定對 `%builtin` 今天完全失效。

**落地時的兩個前置**：(i) `ctx.standard_root` 預設為空〔讀 `lib.rs:182`〕，
須把「沒有標準根」與「標準根裡沒這個名字」分成兩個具名答覆（**Q-031 同族**）；
(ii) 那 6 個死名會**通過**名字檢查再倒在 registry 查表，兩個成因走同一個 `⊥` 出口。

### Q4 ✅ 已裁（用戶）：**C —— 讀取路徑不拒**

**A 被排除的理由是原則不是成本**：SPEC_05 §110 逐字「本欄是**引擎向標準庫投影自身實作的方式**」
⟹ 標準根裡的 `%builtin` **依定義永遠合法**，A 會拒掉一個定義上合法之物。
（成本亦已量：`7038e250…` 自己 255 處命中。）

**B 罰錯人**：〔量 §3.2〕`add: ~%Math./add` 與手寫偽造**逐位元組相同**
⟹ B 把合法拼法罰得跟偽造一樣重，且罰的是**已經存在的舊倉**。

**A 與 B 破的都是可讀性**——REAL_03 §6.8.1 的區分：可重現性破了歷史還在，
**可讀性破了歷史打不開**。

**C 之所以安全，是因為 Q3.B**：〔量，驗收方〕已提交的偽造確實會走到 `lib.rs:3066`
（源碼不含 `%builtin` 的 `out: boom 7` → exit=7）⟹ 讀取路徑再拒一次
**買不到 Q3.B 沒買到的東西**，只多付一份可讀性。
**同族先例 ＝ O53**：「引擎**永不**在讀取路徑上替它寫」——閘裝在**行使能力**處。

**C 附帶一條義務**（否則它就只是「什麼都不做」）：

> 讀取路徑**得**（SHOULD）在解出含 `meta.builtin` 的**使用者**物件時留下可查詢的記號，
> 但**不得**因此拒絕開啟或改寫該物件。

⟹ 「歷史裡有多少偽造」問得出來，日後要升級成拒絕時**手上已經有數字**。

### Q5 ✅ 已裁（用戶）：**A —— 本單不收效果閘**

〔量〕`io.write_file`／`process.exit` **無 `--grant` 仍生效** ⟹ grant 今天連半個閘都不是；
`disc.connect` 的 grant 屬 SPEC_08 §6 另一條線。射程維持「名字不得當憑證」。

### ⚠ 仍未裁：**Q2a（寫入層）**

Q3.B 已回答 Q2b（派送）。**Q2a 沒有被任何一題回答**：使用者在源碼裡手寫 `%builtin:`，
要不要在 parse／evolve／commit 擋，擋在哪一層。

〔量〕今天**沒有無歧義的判準**——合法別名與偽造同位元組。
⟹ **Q2a 的乾淨解也在弧 D 下游**：弧 D 之後，使用者資料裡任何 `meta.builtin`
都無歧義是手寫的，parse 層拒絕才沒有偽陽性。

---

### Q3. 派送還要不要認這個名字？

SPEC 同一句有兩半：拒絕出現、**且不得依該名字派送**。

| 選 | 實測會變成什麼 |
| :--- | :--- |
| **A. 繼續認名字**（只擋書寫） | 今天的 `apply_morphism` 可不動。標準根、`~%Math./add`、已提交別名都還能算。Q1 選 B 時這是唯一還能跑的組合。Q1 選 A 時，「宇宙裡不許有這個欄」與「施用時還靠這個欄」衝突。 |
| **B. 不再認名字** | 必須另有派送憑證。今天沒有。若不做新憑證就把 `builtin_registry.get` 拿掉：`~%Math./add (1,2)` 與偽造一齊變 `⊥ #conflict`（與今天未知名同一條路）。 |

### Q4. 讀取路徑碰到已提交的 `%builtin` 怎麼辦？

今天使用者根與標準根解出來都是帶 `meta.builtin` 的 `ComboVal`。

| 選 | 實測會變成什麼 |
| :--- | :--- |
| **A. 凡 `meta.builtin` 即拒** | `7038e250…` 自己 255 處命中。現有倉 `status` 仍說 available 的那個根會打不開。 |
| **B. 只拒非 `standard-root:` 物件** | 使用者根 `2e31a3fb…` 打不開；標準根仍 hydrate。已提交的 `add: ~%Math./add` 與手寫偽造同一命運。 |
| **C. 讀取不拒，留給 Q2 的寫入／施用層** | 既有倉繼續打開。已提交的偽造在選「擋 apply」之前仍是能力。 |

### Q5. 效果閘要不要算本單的一部分？

〔跑〕`io.write_file`／`process.exit` **無** `--grant` 仍生效；`disc.connect` 有 grant 才進 peer 表。

| 選 | 實測會變成什麼 |
| :--- | :--- |
| **A. 本單不收效果閘** | 射程維持「名字不得當憑證」。即使日後 IO 要 grant，`process.exit` 今天沒有 grant 可出示。 |
| **B. 把 grant 當半個閘** | 擋不住 `process.exit` 與無 grant 的寫檔。connect 的 grant 是另一條 SPEC_08 §6 線，工單已說不收 #23。 |

---

## 5. implementation 工單邊界（待裁之後）

*   **射程句**：本地信任邊界。線上 `#advertise` 仍關著，不在 implementation 裡重做字面閘。
    不收 Q-034（#23 效果傳染）、不收 Q-018（`eval` 看不見倉；量第 3 項時繞過即可）。
*   **派送點**：只有 `apply_morphism` 那一處 `builtin_registry.get`。沒有第二個表。
*   **身分軸**：parse／evolve／commit 拒絕**使用者源碼**，不改 genesis、不改 `for_cas_storage`，
    標準根仍是 `7038e250…`。若選 Q4.A（讀取路徑拒所有 `meta.builtin`）或改投影欄本身，
    標準根 digest 會動——那是 WORK_QUEUE §9.0 的身分搬遷，不是本弧預設能搭便車的。
*   **跨版本**：已提交的使用者根裡已經有明文 `meta.builtin`（本單倉 `2e31a3fb…`、`bf7bb14f…`）。
    Q4 沒裁之前，implementation 不能假設舊倉「沒有這個欄」。
*   **紅線（承工單）**：拒絕層未裁之前不要改 `lib.rs`。不要把「標準根也用 `%builtin`」寫成缺陷不成立。
*   **探針（裁完才寫）**：至少要蓋
    (1) 手寫 `%builtin` 在裁定層的可觀測失敗形、
    (2) `~%Math./add (1,2)` 在同一二進位是否仍是 `3`、
    (3) `add: ~%Math./add`（Q1 的那一形）、
    (4) 舊倉帶明文 `math.add` 的根打不打得開、
    (5) `process.exit` 不得再把試驗行程殺掉。

---

## 6. 自檢

*   [x] §3 六項都有結果；沒有「應該／大概／若有」充當量測。
*   [x] 二進位 `oo v0.28.0-642-ge7b84dd`，commit `e7b84dd`／工作樹 `ed0ef5f`。
*   [x] 第 5 項 registry **245** 名未截斷；標準根相異名 **251**（含 6 個未註冊）。
*   [x] 本偵察過程未改 `crates/`、未改規格；出口只有本檔。
*   [x] 沒有「建議採用 X」；只有「選 X 的後果是實測的 Y」。
