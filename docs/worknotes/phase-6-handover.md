# Phase 6 交接文件：Bohrification 視角切換 (SPEC_08 §3.5)

## 目標

實作 Bohrification 伴隨（$Q \dashv B$）的兩個方向操作：

| 操作 | 方向 | 語義 |
|:-----|:----:|:-----|
| `~%Engine./project_down` | $Q \to B$ | 將量子 Combo 投影到指定 MASA 語境，產生 `#blur` 局部截面 |
| `~%Engine./project_up`   | $B \to Q$ | 從多個 MASA 截面重建全域量子疊加態 |
| `~%Engine./set_strategy` | —          | 在執行期動態切換觀測策略（`#blur`/`#strict`/`#approximate`） |

**依賴前置**：Phase NEW ✅（H¹/H² obstruction）、Phase 1b ✅（`phase_merge_decision`）、Phase 4 ✅（`masa_compatible`）

---

## 規格對應

| 實作項目 | 規格章節 |
|:---------|:---------|
| `project_down` Q→B | SPEC_08 §3.5 |
| `project_up` B→Q   | SPEC_08 §3.5 |
| `%strategy` 策略切換 | SPEC_08 §3.4 |
| `ObservationStrategy::{Blur,Strict,Approximate}` | SPEC_08 §3.4（已實作，本 Phase 對外暴露） |

---

## 1. `engine.project_down`（新增到 `src/builtins/engine.rs`）

### 1.1 輸入格式

```nlang
~%Engine./project_down {
    target: <任意 Value>     ;; 要投影的量子態（通常為 Combo）
    masa:   "<caid_string>"  ;; 目標 MASA 的 CAID（v2 格式，含 masa_ref）
}
```

### 1.2 算法

```
1. 從 arg 取出 target（欄位 "target"）與 masa 字串（欄位 "masa"）
2. 解析 masa 字串 → ContentHash，取 masa_ref
3. 對 target Combo 的每個欄位執行可視性判定：
     field_visible(v, masa_ref) =
       v.content_hash().masa_ref == MasaRef::Top      → 可見（無上下文限制）
       v.content_hash().masa_ref == masa_ref           → 可見（在此 MASA 下）
       否則                                            → 不可見（量子相干性阻止）
4. 建立結果 Combo：
     - 保留可見欄位
     - 設定 result.masa_ref = Digest(masa_digest)
     - 插入 %kind: #blur, %masa: "<masa_caid_str>", %projection: #down
     - EffectTag = State（改變觀測上下文）
5. 若 target 為 non-Combo：直接包裝為 { %val: target, %kind: #blur, %masa: "<masa>", %projection: #down }
```

### 1.3 實作

```rust
m.insert("engine.project_down".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    let (target, masa_str) = if let Value::Combo(ref c) = arg {
        let t = c.get_field("target").cloned().unwrap_or(Value::Top);
        let m = c.get_field("masa").map(|v| oo.force(v.clone(), ctx).to_string_plain())
                 .unwrap_or_default();
        (t, m)
    } else {
        return BottomCause::Conflict.into();
    };

    let masa_hash = match crate::value::ContentHash::parse(&masa_str) {
        Ok(h) => h,
        Err(_) => return BottomCause::Conflict.into(),
    };
    let target_forced = oo.force(target, ctx);

    let mut result_fields = IndexMap::new();
    // 投影欄位
    if let Value::Combo(ref cv) = target_forced {
        for (k, v) in cv.fields() {
            if is_field_visible_in_masa(v, &masa_hash.masa_ref) {
                result_fields.insert(k.clone(), v.clone());
            }
        }
    } else {
        result_fields.insert("%val".to_string(), target_forced.clone());
    }

    // 投影元資訊標記
    result_fields.insert("%kind".to_string(),
        Value::Atom(AtomKind::Tag("blur".to_string()), EffectTag::Pure, None));
    result_fields.insert("%masa".to_string(),
        Value::Atom(AtomKind::Str(masa_str.clone()), EffectTag::Pure, None));
    result_fields.insert("%projection".to_string(),
        Value::Atom(AtomKind::Tag("down".to_string()), EffectTag::Pure, None));

    let mut cv = crate::value::ComboVal::new(
        result_fields, false, IndexMap::new(), EffectTag::State, vec![]
    );
    cv.masa_ref = masa_hash.masa_ref.clone();
    Value::Combo(cv)
}) as Arc<BuiltinFn>);
```

### 1.4 輔助函式（加在 `engine.rs` 模組層級）

```rust
fn is_field_visible_in_masa(value: &Value, masa_ref: &crate::value::MasaRef) -> bool {
    match masa_ref {
        crate::value::MasaRef::Top => true,  // Top MASA = 全視角，所有欄位可見
        crate::value::MasaRef::Digest(target_d) => {
            match value.content_hash().masa_ref {
                crate::value::MasaRef::Top => true,
                crate::value::MasaRef::Digest(ref field_d) => field_d == target_d,
            }
        }
    }
}
```

---

## 2. `engine.project_up`（新增到 `src/builtins/engine.rs`）

### 2.1 輸入格式

```nlang
~%Engine./project_up {
    sections: [<Combo1>, <Combo2>, ...]   ;; 來自不同 MASA 的 #blur 截面列表
}
```

### 2.2 算法

```
1. 從 arg 取出 sections 列表（欄位 "sections"，應為 List Combo）
2. 對每個 section 剝除投影元標記（%kind, %masa, %projection）
3. 對所有截面執行兩兩 MASA 相容性檢查：
     - 若任意兩截面 H² 不相容 → 回傳 Union(sections)，標記 %blur
     - 若全部相容 → 逐步 unify_internal() 合併
4. 回傳合併結果（Combo 或 Union）
```

### 2.3 實作

```rust
m.insert("engine.project_up".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
    // 取出 sections 列表
    let sections_val = if let Value::Combo(ref c) = arg {
        c.get_field("sections").cloned().unwrap_or(Value::Top)
    } else {
        return BottomCause::Conflict.into();
    };
    let sections_forced = oo.force(sections_val, ctx);

    // 從 List Combo 解包各截面
    let raw_sections: Vec<Value> = extract_list_items(&sections_forced, oo, ctx);
    if raw_sections.is_empty() {
        return Value::Top;
    }

    // 剝除投影元標記，保留語義欄位
    let sections: Vec<Value> = raw_sections.iter()
        .map(|s| strip_projection_meta(s))
        .collect();

    // H² 相容性預檢
    for i in 0..sections.len() {
        for j in (i+1)..sections.len() {
            let hi = sections[i].content_hash();
            let hj = sections[j].content_hash();
            let incompatible = match (&hi.masa_ref, &hj.masa_ref) {
                (crate::value::MasaRef::Digest(a), crate::value::MasaRef::Digest(b)) => a != b,
                _ => false,
            };
            if incompatible {
                // H² obstruction：無法重建，回傳帶標記的 Union
                let mut meta = IndexMap::new();
                meta.insert("%kind".to_string(),
                    Value::Atom(AtomKind::Tag("blur".to_string()), EffectTag::Pure, None));
                meta.insert("%h2_obstruction".to_string(),
                    Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
                // 回傳原始截面的 Union（保留 MASA 資訊）
                return Value::Union(raw_sections);
            }
        }
    }

    // 全部相容：逐步合併
    let mut result = sections[0].clone();
    for s in &sections[1..] {
        result = oo.unify_internal(result, s.clone(), ctx);
        if let Value::Bottom(_) = result {
            return result; // 合併失敗
        }
    }
    result
}) as Arc<BuiltinFn>);
```

### 2.4 輔助函式

```rust
fn extract_list_items(list: &Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Vec<Value> {
    match list {
        Value::Combo(c) => {
            let mut items = Vec::new();
            let mut i = 0;
            while let Some(v) = c.get_field(&i.to_string()) {
                items.push(oo.force(v.clone(), ctx));
                i += 1;
            }
            items
        }
        _ => vec![],
    }
}

/// 剝除 %kind, %masa, %projection 標記，回傳語義欄位
fn strip_projection_meta(value: &Value) -> Value {
    if let Value::Combo(cv) = value {
        let mut stripped = cv.clone();
        stripped.data.shift_remove("%kind");
        stripped.data.shift_remove("%masa");
        stripped.data.shift_remove("%projection");
        stripped.meta.shift_remove("%kind");
        stripped.meta.shift_remove("%masa");
        stripped.meta.shift_remove("%projection");
        Value::Combo(stripped)
    } else {
        value.clone()
    }
}
```

---

## 3. `engine.set_strategy`（新增到 `src/builtins/engine.rs`）

### 3.1 輸入格式

```nlang
~%Engine./set_strategy #blur         ;; 切換到模糊策略（預設）
~%Engine./set_strategy #strict       ;; 切換到嚴格策略
~%Engine./set_strategy #approximate  ;; 切換到近似策略
```

### 3.2 實作

```rust
m.insert("engine.set_strategy".to_string(), Arc::new(|arg: Value, _oo: &Ouroboros, ctx: &mut EvalContext| {
    let tag = match &arg {
        Value::Atom(AtomKind::Tag(t), _, _) => t.trim_start_matches('#').to_string(),
        Value::Combo(c) => {
            if let Some(Value::Atom(AtomKind::Tag(t), _, _)) = c.get_field("strategy") {
                t.trim_start_matches('#').to_string()
            } else { return BottomCause::Conflict.into(); }
        }
        _ => return BottomCause::Conflict.into(),
    };
    ctx.strategy = match tag.as_str() {
        "blur"        => crate::observation::ObservationStrategy::Blur,
        "strict"      => crate::observation::ObservationStrategy::Strict,
        "approximate" => crate::observation::ObservationStrategy::Approximate,
        _ => return BottomCause::Conflict.into(),
    };
    Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::State, None)
}) as Arc<BuiltinFn>);
```

---

## 4. 修改 `src/lib.rs`：在 `~%Engine` 掛載三個新態射

在 `root_with_system()` 中的 `engine_fields` 區塊（約 lib.rs:182）加入：

```rust
engine_fields.insert("/project_down".to_string(),
    engine_morph("/project_down", "engine.project_down", EffectTag::State));
engine_fields.insert("/project_up".to_string(),
    engine_morph("/project_up", "engine.project_up", EffectTag::State));
engine_fields.insert("/set_strategy".to_string(),
    engine_morph("/set_strategy", "engine.set_strategy", EffectTag::State));
```

並在 `state_inner` 新增當前策略的快照欄位：

```rust
state_inner.insert("strategy".to_string(),
    Value::Atom(AtomKind::Tag("blur".to_string()), EffectTag::Pure, None));
```

---

## 5. 測試：`crates/interpreter/tests/bohr_test.rs`（新建）

最少 9 個測試：

| # | 測試名稱 | 驗證內容 |
|---|----------|----------|
| 1 | `test_project_down_top_masa` | `project_down` 傳入 MasaRef::Top 的 masa → 保留所有欄位 |
| 2 | `test_project_down_filters_fields` | 欄位 masa 不符 → 欄位被過濾 |
| 3 | `test_project_down_adds_blur_tag` | 結果含 `%kind: #blur`、`%projection: #down` |
| 4 | `test_project_down_noncombo_target` | target 為 Atom → 包裝到 `%val` |
| 5 | `test_project_up_single_section` | 單一截面 → 剝除 meta 後直接返回 |
| 6 | `test_project_up_compatible_sections` | 兩個相容截面 → 成功合併為 Combo |
| 7 | `test_project_up_h2_incompatible` | 兩個 H² 不相容截面 → 返回 Union（不 crash） |
| 8 | `test_set_strategy_blur` | `set_strategy #blur` → `ctx.strategy == Blur` |
| 9 | `test_set_strategy_strict` | `set_strategy #strict` 後，資源耗盡 → 返回 Bottom，不返回 `#blur` Combo |

---

## 6. 設計決策與限制

### `project_down` 的欄位可視性近似

目前以 `content_hash().masa_ref` 判斷欄位是否在指定 MASA 下可見。這是近似：
- **精確語義**：欄位可視 ⟺ 欄位算子與 MASA 中所有算子交換
- **Phase 6 近似**：欄位的 masa_ref 與目標 MASA 相同（或為 Top）
- **Phase 7**：引入交換子代數計算（Commutator Algebra Detection）取代近似

### `project_up` 的 L-S 重建限制

SPEC_08 §3.5 明確標注：`project_up` 的理論基礎目前為開放問題（Conjecture）。
Phase 6 的實作對所有 MASA 相容的截面直接進行 `unify_internal()`，這對應 L-S 重建的
樂觀路徑。若截面在實作中產生 H¹ 相位差（但未達 H² 等級），結果會帶有正確的 holonomy 資訊。

### `set_strategy` 的作用域

`ctx.strategy` 是 `EvalContext` 的欄位——它只影響當前求值上下文的資源耗盡處理行為。
若在 REPL 或單次 `oo run` 中設定，效果限於該次執行，不持久化到 Universe 狀態。

### `ComboVal::masa_ref` 欄位

注意 `strip_projection_meta()` 使用 `shift_remove` 直接操作 `ComboVal` 的 `data` 和
`meta` Map。`%kind`、`%masa`、`%projection` 在 BN/ 序列化中屬 meta 優先級（prefix `%`），
因此應從 `cv.meta` 移除，而非 `cv.data`。請確認 `ComboVal::get_field()` 的查找順序，
對應移除正確的 Map。

---

## 7. 完成標準

- [ ] `engine.project_down` 實作並註冊
- [ ] `engine.project_up` 實作並註冊（含 `extract_list_items`, `strip_projection_meta`）
- [ ] `engine.set_strategy` 實作並註冊
- [ ] `~%Engine` Combo 新增 `/project_down`、`/project_up`、`/set_strategy` 態射
- [ ] `~%Engine.state.strategy` 快照欄位
- [ ] `tests/bohr_test.rs`：9 個測試，全數通過
- [ ] `cargo test` 全數通過（預期 116+ 個測試）
