# 工單:效應組合 = 集合聯集(SPEC_08 §4.1)—— 效應系統波 arc 1

**開單**:2026-07-23(驗收方)。**基線**:dev @ 本工單 commit(v0.2.33 後)。
**協議提醒**:完成後**先把交付紀錄寫進本檔 §5 再回報**。探針/語料/向量的
**修改權在驗收方**——交付僅移除探針 `#[ignore]`,一字不改其餘。

## 1. 法源(裁定,2026-07-23,使用者)

效應模型由**純量全序**遷至**集合半格**,匹配 SPEC_08 §4.1 組合矩陣。

- 現況:`EffectTag { Pure=0, State=1, IO=2, NonDet=3 }`,以 `.max()` 組合。
- 病灶:`io ⊔ nondet` 被 max 坍縮成 `#nondet`——**IO 事實憑空消失**。
  `io`/`nondet`/`state` 本是**不可比兄弟**,強加的全序是範疇錯誤。
- 裁定:效應是**標籤集合**,`|` 是真 join。組合 = 集合聯集(§4.1 矩陣),
  冪等 `E|E=E` 自然成立。這是 n/ 格論的誠實面。

## 2. 新表示型(建議實作)

`EffectTag` 改為 **`Copy` bitset newtype**,保留同名 + 既有常數拼法,使
480 個 `EffectTag::Pure` 預設點與 ~60 個生產點**原地不改**:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EffectTag(u8);   // bit0=IO bit1=NonDet bit2=State bit3=Cached; 0=Pure
impl EffectTag {
    pub const Pure:   EffectTag = EffectTag(0);
    pub const IO:     EffectTag = EffectTag(1);
    pub const NonDet: EffectTag = EffectTag(2);
    pub const State:  EffectTag = EffectTag(4);
    pub const Cached: EffectTag = EffectTag(8);   // 保留;arc 1 無生產者(§4.2.4 另案)
    pub fn union(self, o: EffectTag) -> EffectTag { EffectTag(self.0 | o.0) }
    pub fn contains(self, o: EffectTag) -> bool { self.0 & o.0 == o.0 && o.0 != 0 }
    pub fn is_pure(self) -> bool { self.0 == 0 }
}
// BitOr = union（可選，便於 a | b 拼法）
```

**關鍵:移除 `PartialOrd/Ord`**——讓編譯器點名每一處 `.max(` 與 `> Pure`,
杜絕「靜默取單標籤」的錯誤語義。逐點機械轉換(見 §3)。

**Display for EffectTag**:渲染集合,標籤序 = **字母序**(與 SPEC_01 §2.4.1
聯集顯示、與 `.%effect` 讀取產生的 normalize_union **一致**;實測 `#io | #nondet
| #state`、`#io | #pure`)。空集(Pure)→ `#pure`(顯示層已守 `!= Pure` 才印)。
單標籤 → `#io`。多標籤 → `#io | #nondet`。

## 3. 修法位點(編譯器導引)

**(A) 組合:全部 `.max(` on effects → `.union(`(或 `|`)**——共 71 處,散在
`lib.rs`/`eval.rs`(含 `predict_effect` 的 Apply/Pipe/Combo/Meet…臂、
`eval_elements` 的 `me = me.max(...)`)、`unify.rs`(§4.2.2 合一傳染)、
`value.rs`(`effect()` 的 Union/Range 臂、`collapse_with_effect`、`with_effect`)、
`builtins/{list,query,math}.rs`。移除 Ord 後編譯器逐一點名。
- **語義**:`a.union(b)`。Union 值的 `effect()`(value.rs:1394)= 各支 effect
  的**聯集**(原 `.map(effect).max()` → fold union)。

**(B) 純度判定:`> EffectTag::Pure` → `!= EffectTag::Pure`(或 `!is_pure()`)**
——兩處:`value.rs:1488`(顯示尾註閘)、`value.rs:1561`(CAID horizon salt)。
二元結果不變(Pure vs 非 Pure),CAID 行為**完全不動**(見 §4)。

**(C) `.%effect` 讀取:`effect_tag_atom`(lib.rs:213)改產多標籤**:
- 空集 → `#pure` 原子(不變)。
- 單標籤 → 該 tag 裸原子(不變)。
- **多標籤 → `normalize_union` of the tag 原子**(字母序由 §2.4.1 自然給定;
  勿手排)。呼叫點 lib.rs:1709 / 1798 不動,只換 helper 內部。

**(D) 顯示尾註**(value.rs:1488)已透過 `Display for EffectTag` 走(B)之後
自然渲染多標籤 `;; %effect: #io | #nondet`。確認與(C)同序。

**不動**:
- `⊥`/`#blur` 元讀白名單(lib.rs ~1607-1641 臂)——`%effect` 非白名單,
  ⊥ 照 F1 傳過、blur 吸收。
- 顯式 `%effect` 欄位優先(spoof)——欄位查找在讀取臂之前,不動。
- Cocoon 屏蔽:`predict_effect` 的 `if *closed { return Pure }`(eval.rs:639)
  與構造時 closed-skip **保留**;union 化後屏蔽仍止於繭壁(pin 覆蓋)。
- EffectTag 的 `#pure`/單標籤顯示、生產者標籤(io/nondet/state)不動。

## 4. CAID 穩定性守則(紅線)

`.%caid` 現對常規值讀 `_`(不可觀測,同 B5 marker);CAID 穩定由**全套回歸**
(workspace + conformance)把關。表示型換代**不得位移任何既有 CAID**:

- **Atom / Combo 的 bn_serial 不序列化 effect**(bn_serial.rs:60 `_effect` 忽略;
  Combo 臂亦無 effect push)→ 原子/combo CAID 與 effect **無關**,天然穩定。
- **唯一風險 = Thunk**(bn_serial.rs:93 `buf.push(*effect as u8)`)。newtype 後
  `as u8` 不編譯 → 改用 **`to_serial_byte()`**,將 4 個既有單標籤映回**舊序數**:
  ```rust
  fn to_serial_byte(self) -> u8 {
      match self {
          EffectTag::Pure => 0, EffectTag::State => 1,
          EffectTag::IO => 2, EffectTag::NonDet => 3,   // 舊序數,byte 完全不變
          _ => 0x80 | self.0,   // 多標籤/Cached(arc 1 前不可能出現)= 新高位空間
      }
  }
  ```
  → 任何 arc 1 前可表達的 thunk 節點 byte **逐位元相同**,CAID 零位移。
- `content_hash_with_salt` 的 salt 條件由(B)保持二元不變。§4.1「效應集合完整
  參與 CAID 規範化」= **參與義務另案**,arc 1 不做。

## 5. 範圍柵欄

**做**:組合=集合聯集(§4.1 矩陣)、多標籤 `.%effect` 讀取與顯示尾註、
冪等去重、合一/態射傳染的聯集化。

**不做(掛帳後續弧,勿實作)**:
- `#cached` 固化(§4.2.4,坍縮獲穩定 CAID 後 io/nondet/state → cached)——
  Cached 位保留,無生產者。
- `#ext:<id>` 自訂標籤(§4.1)——開放式,不入固定 bitset。
- 靜態守護:純語境觸 `#io` → `⊥ #effect_violation`(§4.3)。
- `~%Effect./runPure` handler + `%privilege_token`(§4.3)。
- 效應集合完整參與 CAID(§4.1 參與義務)——arc 1 保持二元 horizon salt。

## 6. 門(紅)與釘 + 目標(先量後寫,基線實測 2026-07-23)

**探針** `crates/interpreter/tests/effect_union_probe_test.rs`(已預提交+校準):
- **7 紅**(`#[ignore]`,全紅正因 got 純量 max):`red_compose_io_nondet`
  (got `#nondet` → `#io | #nondet`)、`_io_state`(got `#io` → `#io | #state`)、
  `_nondet_state`(`#nondet` → `#nondet | #state`)、`_three_tags`
  (`#nondet` → `#io | #nondet | #state`)、`_unify_join`(`#nondet` →
  `#io | #nondet`)、`_idempotent_multi`(`#nondet` → `#io | #nondet`)、
  `_display_tail`(尾註 `#nondet` → `#io | #nondet`)。
- **8 釘**(全綠須守):pure 預設、單標籤裸原子、冪等單標籤(`#io` 非 `#io|#io`)、
  合一單標籤 join、cocoon 屏蔽(含多標籤內部)、**union 值投影**
  (`(5|io).%effect → #io | #pure`,與效應集合不同機制不得混)、⊥ 白名單、
  單尾註+pure 靜默。

**交付 = 移除 7 個 `#[ignore]`**,探針其餘一字不改。

**目標**(基線 → 交付後):
- effect_union 探針 15/15(基線 8 綠 + 7 ignore)。
- workspace **1336/0/3**(基線 with-probes 1329/0/10)。
- conformance **138/138**(基線 135/138;L2-97/98/99 現紅 got `#nondet`)。
- 語料非 pending 不退(effect_taint 之 `.%effect==#io` 單標籤未動;integration 綠)。
  runner:`nlang-spec/scripts/run-conformance.py --engine <oo>`。

全 workspace 一顆不得翻紅;移除 Ord 引發的所有編譯點須**逐一轉 union/`!=`**,
不得為過編譯而 `#[derive(Ord)]` 復原(那會復活純量語義=謊)。

## 7. 交付紀錄(交付方填;先寫再回報)

- [ ] 交付 commit(s):
- [ ] 新型 EffectTag(bitset/consts/union/Display 序/to_serial_byte)落點:
- [ ] `.max→union` 轉換點數與 `>Pure→!=Pure` 兩點確認:
- [ ] `effect_tag_atom` 多標籤 normalize_union 落點:
- [ ] CAID 穩定驗證(Thunk to_serial_byte 舊序數;全套回歸綠):
- [ ] 探針 15/15 / workspace / conformance / 語料 四數:
- [ ] 申報事項(移除 Ord 的非效應用點、序列化格式、範圍外接觸):

## 8. 驗收紀錄(驗收方填)
