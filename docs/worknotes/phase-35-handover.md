# Phase 35 交接文件

> 狀態：待實作  
> 前置：Phase 34 完成（~392 tests passing）  
> 目標：A+B — List Round 2（4 builtins）+ Math Round 2（4 builtins）

---

## 概覽

| 任務 | 位置 | 內容 |
|:-----|:-----|:-----|
| Task 1 | `crates/interpreter/src/builtins/list.rs` | 新增 4 個 list builtins |
| Task 2 | `crates/interpreter/src/builtins/math.rs` | 新增 4 個 helper 函式 + 4 個 math builtins |
| Task 3 | `crates/interpreter/src/lib.rs` | 更新 `~%List`（+4）和 `~%Math`（+4）morphism 列表 |
| Task 4 | `crates/interpreter/src/genesis.rs` | 重跑 seed test → 更新 SEED_LIST、SEED_MATH |
| Tests  | `crates/interpreter/tests/list_p35_test.rs`（新建） | ~8 個測試 |
| Tests  | `crates/interpreter/tests/math_p35_test.rs`（新建） | ~7 個測試 |

預期完成後：**~392 + 15 ≈ 407 tests**

---

## Builtin 語義速查

### List Round 2

| builtin | 輸入 | 輸出 | 說明 |
|:--------|:-----|:-----|:-----|
| `list.enumerate` | `{0: list}` | list of `{0:Int, 1:Value}` | 加上 0-based 索引 |
| `list.sort_by` | `{0: cmp_fn, 1: list}` | sorted list | `cmp_fn({0:a,1:b})` → Int（負/0/正） |
| `list.dedup` | `{0: list}` | list | 移除**相鄰**重複（全局去重用 `list.unique`） |
| `list.intersperse` | `{0: sep, 1: list}` | list | 每兩元素間插入 sep |

### Math Round 2

| builtin | 輸入 | 輸出 | 說明 |
|:--------|:-----|:-----|:-----|
| `math.factorial` | `{0: n}` | Int | `n!`；n < 0 → Bottom |
| `math.choose` | `{0: n, 1: k}` | Int | `C(n,k)`；k<0 或 k>n → 0；n<0 → Bottom |
| `math.is_prime` | `{0: n}` | `#true`\|`#false` | 確定性 Miller-Rabin（12 個見證） |
| `math.pow_mod` | `{0: base, 1: exp, 2: mod}` | Int | `(base^exp) % mod`；exp<0 或 mod≤0 或 base<0 → Bottom |

---

## Task 1：list.rs 新增 4 個 builtins

在 `list.window` 的 `}) as Arc<BuiltinFn>);` 之後（第 704 行後），`}` 閉合括號之前，貼入：

```rust
    // list.enumerate: {0: list} → list of {0: Int, 1: Value}
    m.insert("list.enumerate".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let list = oo.force(v, ctx);
        let items = extract_list_items(&list);
        let pairs: Vec<Value> = items.into_iter().enumerate().map(|(i, item)| {
            let mut pair = IndexMap::new();
            pair.insert("0".to_string(), Value::Atom(AtomKind::Int(BigInt::from(i)), EffectTag::Pure, None));
            pair.insert("1".to_string(), item);
            Value::Combo(ComboVal::new(pair, false, IndexMap::new(), EffectTag::Pure, vec![]))
        }).collect();
        build_list_value(pairs)
    }) as Arc<BuiltinFn>);

    // list.sort_by: {0: cmp_fn, 1: list} → sorted list (stable sort)
    // cmp_fn({0: a, 1: b}) → Int: negative → a first, 0 → equal, positive → b first
    m.insert("list.sort_by".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vf), Some(vl)) = (c.get_field("0"), c.get_field("1")) {
                let cmp_fn = vf.clone();
                let list = oo.force(vl.clone(), ctx);
                let mut items = extract_list_items(&list);
                items.sort_by(|a, b| {
                    let mut pair = IndexMap::new();
                    pair.insert("0".to_string(), a.clone());
                    pair.insert("1".to_string(), b.clone());
                    let pair_val = Value::Combo(ComboVal::new(pair, true, IndexMap::new(), EffectTag::Pure, vec![]));
                    let result = oo.apply_morphism(cmp_fn.clone(), pair_val, ctx);
                    match result.collapse() {
                        Value::Atom(AtomKind::Int(n), _, _) if n.is_negative() => std::cmp::Ordering::Less,
                        Value::Atom(AtomKind::Int(n), _, _) if n.is_positive() => std::cmp::Ordering::Greater,
                        _ => std::cmp::Ordering::Equal,
                    }
                });
                return build_list_value(items);
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // list.dedup: {0: list} → list (remove consecutive duplicates, stable)
    // Uses to_nlang(0) for equality, same as list.unique.
    m.insert("list.dedup".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let list = oo.force(v, ctx);
        let items = extract_list_items(&list);
        let mut out: Vec<Value> = Vec::new();
        let mut last_key: Option<String> = None;
        for item in items {
            let forced = oo.force(item, ctx);
            let key = forced.to_nlang(0);
            if Some(&key) != last_key.as_ref() {
                last_key = Some(key);
                out.push(forced);
            }
        }
        build_list_value(out)
    }) as Arc<BuiltinFn>);

    // list.intersperse: {0: sep, 1: list} → list with sep inserted between elements
    m.insert("list.intersperse".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vs), Some(vl)) = (c.get_field("0"), c.get_field("1")) {
                let sep = vs.clone();
                let list = oo.force(vl.clone(), ctx);
                let items = extract_list_items(&list);
                if items.len() <= 1 {
                    return build_list_value(items);
                }
                let mut out = Vec::new();
                for (i, item) in items.into_iter().enumerate() {
                    if i > 0 { out.push(sep.clone()); }
                    out.push(item);
                }
                return build_list_value(out);
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
```

---

## Task 2：math.rs 新增 helper 函式 + 4 個 builtins

### Step A：更新 use 行（第 7 行）

找到：
```rust
use num_traits::{Signed, Zero, ToPrimitive};
```

替換為：
```rust
use num_traits::{Signed, Zero, One, ToPrimitive};
```

### Step B：新增 4 個 helper 函式（在 `bigint_gcd` 函式之後，`pub fn register_math_builtins` 之前）

```rust
fn bigint_factorial(n: BigInt) -> BigInt {
    let mut result = BigInt::one();
    let mut i = BigInt::from(2i64);
    while i <= n {
        result *= &i;
        i += 1i64;
    }
    result
}

fn bigint_choose(n: &BigInt, k: &BigInt) -> BigInt {
    if k.is_negative() || k > n { return BigInt::zero(); }
    let n_minus_k = n - k;
    // Use smaller of k and n-k for fewer iterations
    let k_eff = if n_minus_k < *k { n_minus_k } else { k.clone() };
    let mut result = BigInt::one();
    let mut i = BigInt::zero();
    while i < k_eff {
        // Always exact: C(n, i+1) = C(n, i) * (n-i) / (i+1)
        result = &result * (n - &i) / (&i + BigInt::one());
        i += 1i64;
    }
    result
}

fn bigint_modpow(mut base: BigInt, mut exp: BigInt, modulus: &BigInt) -> BigInt {
    // Special case: any number mod 1 is 0
    if modulus == &BigInt::one() { return BigInt::zero(); }
    let mut result = BigInt::one();
    base = base % modulus;
    while exp > BigInt::zero() {
        if &exp % 2i64 == BigInt::one() {
            result = &result * &base % modulus;
        }
        exp /= 2i64;
        base = &base * &base % modulus;
    }
    result
}

fn is_prime_miller_rabin(n: &BigInt) -> bool {
    if n < &BigInt::from(2i32) { return false; }
    if n == &BigInt::from(2i32) || n == &BigInt::from(3i32) { return true; }
    if (n % 2i32) == BigInt::zero() { return false; }

    // Write n-1 as 2^r * d (d odd)
    let mut d = n - BigInt::one();
    let mut r = 0u32;
    while (&d % 2i32) == BigInt::zero() {
        d /= 2i32;
        r += 1;
    }

    // Deterministic for all n < 3,317,044,064,679,887,385,961,981
    let witnesses: &[i64] = &[2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];
    let n_minus_one = n - BigInt::one();

    'witness: for &w in witnesses {
        let a = BigInt::from(w);
        if &a >= n { continue; }
        let mut x = bigint_modpow(a, d.clone(), n);
        if x == BigInt::one() || x == n_minus_one { continue; }
        for _ in 0..(r - 1) {
            x = &x * &x % n;
            if x == n_minus_one { continue 'witness; }
        }
        return false;
    }
    true
}
```

### Step C：新增 4 個 builtins（在 `math.log10` 的 `}) as Arc<BuiltinFn>);` 之後，`}` 之前）

```rust
    // math.factorial: {0: n} → Int; n < 0 → Bottom
    m.insert("math.factorial".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Int(n), _, _) = oo.force(v, ctx).collapse() {
            if n.is_negative() { return BottomCause::Conflict.into(); }
            return Value::Atom(AtomKind::Int(bigint_factorial(n.clone())), EffectTag::Pure, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // math.choose: {0: n, 1: k} → Int (C(n,k)); n < 0 → Bottom; k < 0 or k > n → 0
    m.insert("math.choose".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vn), Some(vk)) = (c.get_field("0"), c.get_field("1")) {
                let fn_v = oo.force(vn.clone(), ctx);
                let fk_v = oo.force(vk.clone(), ctx);
                if let (Value::Atom(AtomKind::Int(n), _, _), Value::Atom(AtomKind::Int(k), _, _)) =
                    (fn_v.collapse(), fk_v.collapse())
                {
                    if n.is_negative() { return BottomCause::Conflict.into(); }
                    return Value::Atom(AtomKind::Int(bigint_choose(n, k)), EffectTag::Pure, None);
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // math.is_prime: {0: n} → #true | #false (deterministic Miller-Rabin)
    m.insert("math.is_prime".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        if let Value::Atom(AtomKind::Int(n), _, _) = oo.force(v, ctx).collapse() {
            let tag = if is_prime_miller_rabin(n) { "true" } else { "false" };
            return Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::Pure, None);
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // math.pow_mod: {0: base, 1: exp, 2: mod} → Int ((base^exp) % mod)
    // Requires: base ≥ 0, exp ≥ 0, mod > 0; otherwise → Bottom
    m.insert("math.pow_mod".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vb), Some(ve), Some(vm)) = (c.get_field("0"), c.get_field("1"), c.get_field("2")) {
                let fb = oo.force(vb.clone(), ctx);
                let fe = oo.force(ve.clone(), ctx);
                let fm = oo.force(vm.clone(), ctx);
                if let (
                    Value::Atom(AtomKind::Int(base), _, _),
                    Value::Atom(AtomKind::Int(exp), _, _),
                    Value::Atom(AtomKind::Int(modulus), _, _),
                ) = (fb.collapse(), fe.collapse(), fm.collapse()) {
                    if base.is_negative() || exp.is_negative() || modulus <= &BigInt::zero() {
                        return BottomCause::Conflict.into();
                    }
                    return Value::Atom(
                        AtomKind::Int(bigint_modpow(base.clone(), exp.clone(), modulus)),
                        EffectTag::Pure, None,
                    );
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);
```

---

## Task 3：更新 `lib.rs` 的 morphism 列表

### `~%List`（找到 `/window` 條目後追加）

找到：
```rust
            ("/window",    "list.window"),
        ];
```

替換為：
```rust
            ("/window",    "list.window"),
            // Phase 35
            ("/enumerate",   "list.enumerate"),
            ("/sort_by",     "list.sort_by"),
            ("/dedup",       "list.dedup"),
            ("/intersperse", "list.intersperse"),
        ];
```

### `~%Math`（找到 `/log10` 條目後追加）

找到：
```rust
            ("/log10",  "math.log10"),
        ];
```

替換為：
```rust
            ("/log10",     "math.log10"),
            // Phase 35
            ("/factorial", "math.factorial"),
            ("/choose",    "math.choose"),
            ("/is_prime",  "math.is_prime"),
            ("/pow_mod",   "math.pow_mod"),
        ];
```

---

## Task 4：重跑 seed test → 更新 genesis.rs

```bash
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture 2>&1
```

從輸出的 `UPDATE:` 行找到 `~%List` 和 `~%Math` 的新 CAID，更新 `genesis.rs` 中的 `SEED_LIST` 和 `SEED_MATH`。其他 seed 不受影響。

---

## 測試（`tests/list_p35_test.rs`）

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag, ComboVal};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn int(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
fn str_val(s: &str) -> Value { Value::Atom(AtomKind::Str(s.to_string()), EffectTag::Pure, None) }
fn combo1(a: Value) -> Value {
    let mut m = IndexMap::new(); m.insert("0".to_string(), a);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn combo2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn list(items: Vec<Value>) -> Value {
    let mut m = IndexMap::new();
    for (i, v) in items.into_iter().enumerate() { m.insert(i.to_string(), v); }
    m.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn list_len(v: &Value) -> usize {
    match v { Value::Combo(c) => c.fields_iter().filter(|(k,_)| k.parse::<usize>().is_ok()).count(), _ => panic!() }
}
fn list_at(v: &Value, i: usize) -> &Value {
    match v { Value::Combo(c) => c.get_field(&i.to_string()).unwrap(), _ => panic!() }
}
fn as_int(v: &Value) -> i64 {
    match v { Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(), o => panic!("{:?}", o) }
}

// ── list.enumerate ─────────────────────────────────────────────────

#[test]
fn test_list_enumerate_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.enumerate", combo1(list(vec![int(10), int(20), int(30)])));
    assert_eq!(list_len(&r), 3);
    // First element: {0: 0, 1: 10}
    let pair0 = list_at(&r, 0);
    if let Value::Combo(c) = pair0 {
        assert_eq!(as_int(c.get_field("0").unwrap()), 0);
        assert_eq!(as_int(c.get_field("1").unwrap()), 10);
    } else { panic!(); }
}

#[test]
fn test_list_enumerate_empty() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.enumerate", combo1(list(vec![])));
    assert_eq!(list_len(&r), 0);
}

// ── list.sort_by ───────────────────────────────────────────────────

#[test]
fn test_list_sort_by_ascending() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    // cmp_fn: {0: a, 1: b} → a - b (ascending)
    let cmp_fn = oo.builtin_registry.get("math.sub").unwrap().clone();
    let cmp_val = Value::Combo(ComboVal::new(
        IndexMap::from_iter(vec![
            ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
            ("%builtin".to_string(),  Value::Atom(AtomKind::Str("math.sub".to_string()), EffectTag::Pure, None)),
        ]),
        true, IndexMap::new(), EffectTag::Pure, vec![],
    ));
    let r = call(&oo, &mut ctx, "list.sort_by", combo2(cmp_val, list(vec![int(3), int(1), int(2)])));
    assert_eq!(as_int(list_at(&r, 0)), 1);
    assert_eq!(as_int(list_at(&r, 1)), 2);
    assert_eq!(as_int(list_at(&r, 2)), 3);
}

// ── list.dedup ─────────────────────────────────────────────────────

#[test]
fn test_list_dedup_consecutive() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.dedup",
        combo1(list(vec![int(1), int(1), int(2), int(3), int(3), int(1)])));
    assert_eq!(list_len(&r), 4);  // [1, 2, 3, 1]
    assert_eq!(as_int(list_at(&r, 0)), 1);
    assert_eq!(as_int(list_at(&r, 1)), 2);
    assert_eq!(as_int(list_at(&r, 2)), 3);
    assert_eq!(as_int(list_at(&r, 3)), 1);
}

#[test]
fn test_list_dedup_no_consecutive() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.dedup",
        combo1(list(vec![int(1), int(2), int(3)])));
    assert_eq!(list_len(&r), 3);
}

// ── list.intersperse ───────────────────────────────────────────────

#[test]
fn test_list_intersperse_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.intersperse",
        combo2(int(0), list(vec![int(1), int(2), int(3)])));
    assert_eq!(list_len(&r), 5);  // [1, 0, 2, 0, 3]
    assert_eq!(as_int(list_at(&r, 0)), 1);
    assert_eq!(as_int(list_at(&r, 1)), 0);
    assert_eq!(as_int(list_at(&r, 2)), 2);
    assert_eq!(as_int(list_at(&r, 3)), 0);
    assert_eq!(as_int(list_at(&r, 4)), 3);
}

#[test]
fn test_list_intersperse_single_element() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.intersperse",
        combo2(int(0), list(vec![int(42)])));
    assert_eq!(list_len(&r), 1);
    assert_eq!(as_int(list_at(&r, 0)), 42);
}

#[test]
fn test_list_intersperse_empty() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    let r = call(&oo, &mut ctx, "list.intersperse",
        combo2(int(0), list(vec![])));
    assert_eq!(list_len(&r), 0);
}
```

---

## 測試（`tests/math_p35_test.rs`）

```rust
use nlang_interpreter::{Ouroboros, EvalContext};
use nlang_interpreter::value::{Value, EffectTag};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use indexmap::IndexMap;
use nlang_interpreter::value::ComboVal;

fn make_oo() -> Ouroboros { Ouroboros::new_in_memory() }
fn int(n: i64) -> Value { Value::Atom(AtomKind::Int(BigInt::from(n)), EffectTag::Pure, None) }
fn combo1(a: Value) -> Value {
    let mut m = IndexMap::new(); m.insert("0".to_string(), a);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn combo2(a: Value, b: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn combo3(a: Value, b: Value, c: Value) -> Value {
    let mut m = IndexMap::new();
    m.insert("0".to_string(), a); m.insert("1".to_string(), b); m.insert("2".to_string(), c);
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}
fn call(oo: &Ouroboros, ctx: &mut EvalContext, name: &str, arg: Value) -> Value {
    oo.builtin_registry.get(name).unwrap().clone()(arg, oo, ctx)
}
fn as_int(v: &Value) -> i64 {
    match v { Value::Atom(AtomKind::Int(n), _, _) => n.to_i64().unwrap(), o => panic!("{:?}", o) }
}
fn is_bottom(v: &Value) -> bool { matches!(v, Value::Bottom(_)) }
fn is_true(v: &Value)  -> bool { matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "true") }
fn is_false(v: &Value) -> bool { matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "false") }

// ── math.factorial ─────────────────────────────────────────────────

#[test]
fn test_math_factorial_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    assert_eq!(as_int(&call(&oo, &mut ctx, "math.factorial", combo1(int(5)))), 120);
    assert_eq!(as_int(&call(&oo, &mut ctx, "math.factorial", combo1(int(0)))), 1);
    assert_eq!(as_int(&call(&oo, &mut ctx, "math.factorial", combo1(int(1)))), 1);
}

#[test]
fn test_math_factorial_negative_is_bottom() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    assert!(is_bottom(&call(&oo, &mut ctx, "math.factorial", combo1(int(-1)))));
}

// ── math.choose ────────────────────────────────────────────────────

#[test]
fn test_math_choose_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    assert_eq!(as_int(&call(&oo, &mut ctx, "math.choose", combo2(int(5), int(2)))), 10);
    assert_eq!(as_int(&call(&oo, &mut ctx, "math.choose", combo2(int(5), int(0)))), 1);
    assert_eq!(as_int(&call(&oo, &mut ctx, "math.choose", combo2(int(5), int(5)))), 1);
    // k > n → 0
    assert_eq!(as_int(&call(&oo, &mut ctx, "math.choose", combo2(int(3), int(5)))), 0);
}

// ── math.is_prime ──────────────────────────────────────────────────

#[test]
fn test_math_is_prime() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    assert!(is_true(&call(&oo,  &mut ctx, "math.is_prime", combo1(int(2)))));
    assert!(is_true(&call(&oo,  &mut ctx, "math.is_prime", combo1(int(7)))));
    assert!(is_true(&call(&oo,  &mut ctx, "math.is_prime", combo1(int(97)))));
    assert!(is_false(&call(&oo, &mut ctx, "math.is_prime", combo1(int(1)))));
    assert!(is_false(&call(&oo, &mut ctx, "math.is_prime", combo1(int(9)))));
    assert!(is_false(&call(&oo, &mut ctx, "math.is_prime", combo1(int(100)))));
}

// ── math.pow_mod ───────────────────────────────────────────────────

#[test]
fn test_math_pow_mod_basic() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    // 2^10 mod 1000 = 1024 mod 1000 = 24
    assert_eq!(as_int(&call(&oo, &mut ctx, "math.pow_mod", combo3(int(2), int(10), int(1000)))), 24);
    // 3^0 mod 7 = 1
    assert_eq!(as_int(&call(&oo, &mut ctx, "math.pow_mod", combo3(int(3), int(0), int(7)))), 1);
}

#[test]
fn test_math_pow_mod_invalid_is_bottom() {
    let oo = make_oo(); let mut ctx = oo.eval_context();
    // negative exp
    assert!(is_bottom(&call(&oo, &mut ctx, "math.pow_mod", combo3(int(2), int(-1), int(7)))));
    // mod = 0
    assert!(is_bottom(&call(&oo, &mut ctx, "math.pow_mod", combo3(int(2), int(3), int(0)))));
}
```

---

## Cargo.toml：新增兩個 test 條目

```toml
[[test]]
name = "list_p35_test"
path = "tests/list_p35_test.rs"

[[test]]
name = "math_p35_test"
path = "tests/math_p35_test.rs"
```

---

## 設計備忘

### `list.dedup` vs `list.unique`
- `list.dedup`：移除**相鄰**重複，`[1,1,2,1]` → `[1,2,1]`（順序保留，穩定）
- `list.unique`：移除**所有**重複，`[1,1,2,1]` → `[1,2]`（HashSet，初次出現者保留）

### `list.sort_by` 比較函式慣例
`cmp_fn({0:a, 1:b})` 返回負整數 → a 排在 b 之前（升序）。與 Rust/C 的 `cmp` 慣例一致。

### `bigint_choose` 的精確除法
用 Pascal 遞迴：`C(n,i+1) = C(n,i) * (n-i) / (i+1)`。中間每步除法皆精確（C(n,k) 始終是整數）。迭代時選較小的 `min(k, n-k)` 以減少迭代次數。

### `is_prime_miller_rabin` 確定性範圍
使用見證 {2,3,5,7,11,13,17,19,23,29,31,37}，對所有 n < 3.3×10²⁴ 完全確定性。
- r=0 的邊界：if `r=1`，內層迴圈 `0..(r-1)` = `0..0` 不執行，直接返回 false（正確）。

### `math.pow_mod` 的限制
要求 base ≥ 0, exp ≥ 0, mod > 0，否則返回 Bottom(Conflict)。

---

## 驗證步驟

```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools

# 1. 編譯
cargo build --manifest-path crates/interpreter/Cargo.toml 2>&1

# 2. 新測試
cargo test --manifest-path crates/interpreter/Cargo.toml list_p35_test -- --nocapture
cargo test --manifest-path crates/interpreter/Cargo.toml math_p35_test -- --nocapture

# 3. seed 更新後穩定性
cargo test --manifest-path crates/interpreter/Cargo.toml seed_caids_are_stable -- --nocapture

# 4. 全套不退步
cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：~407 tests, 0 failed
```
