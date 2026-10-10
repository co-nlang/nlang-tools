use crate::{BottomCause, BottomDetail, ComboVal, EffectTag, EvalContext, Ouroboros, Value};
use indexmap::IndexMap;
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

/// REAL_03 §5.2 item 3: a pattern branch is named by the pattern's
/// content digest. The name is a function of the value, so it is
/// injective up to the hash and does not depend on write order.
/// `"{" … "}"` cannot collide with a numeric curry slot.
pub(crate) fn pattern_branch_name(pattern: &Value) -> String {
    format!(
        "{{{}}}",
        hex::encode(crate::bn_serial::content_digest(pattern))
    )
}

/// A table's non-pattern key is a constraint: `_` is Top, a key that
/// is entirely an integer is that integer, anything else is that string.
pub(crate) fn constraint_from_data_key(key: &str) -> Value {
    if key == "_" {
        return Value::Top;
    }
    if let Ok(n) = key.parse::<BigInt>() {
        return Value::Atom(AtomKind::Int(n), EffectTag::Pure, None);
    }
    Value::Atom(AtomKind::Str(key.to_string()), EffectTag::Pure, None)
}

pub(crate) fn rules_have_pattern(rules: &ComboVal) -> bool {
    rules.all_fields_iter().any(|(k, v)| {
        if k.starts_with('%') {
            return false;
        }
        match v {
            Value::Combo(rc) => rc.get_field("%pattern").is_some(),
            _ => false,
        }
    })
}

pub(crate) fn branch_cocoon(pattern: Value, body: Value) -> Value {
    let te = body.effect();
    Value::Combo(ComboVal::new(
        IndexMap::from_iter([
            ("%pattern".to_string(), pattern),
            ("%val".to_string(), body),
            // Same anti-peel as a table rule built by eval.
            ("_".to_string(), Value::Top),
        ]),
        true,
        IndexMap::new(),
        te,
        vec![],
    ))
}

/// A dispatch branch: a pattern plus one body, or a set of bodies.
pub(crate) fn is_rule_cocoon(c: &ComboVal) -> bool {
    c.get_field("%pattern").is_some()
        && (c.get_field("%val").is_some()
            || c.get_field("%code").is_some()
            || c.get_field("%bodies").is_some())
}

/// Bodies of one branch are a set keyed by each body's content digest.
/// The key sorts, so which side arrived first is not in the bytes, and a
/// body already in the set is the same entry. One body is that body.
pub(crate) fn join_rule_cocoons(a: ComboVal, b: ComboVal) -> Value {
    let pattern = a
        .get_field("%pattern")
        .expect("rule cocoon has %pattern")
        .clone();
    let mut set: std::collections::BTreeMap<String, Value> = std::collections::BTreeMap::new();
    for c in [a, b] {
        if let Some(Value::Combo(bs)) = c.get_field("%bodies") {
            for (k, v) in bs.data.iter() {
                set.insert(k.clone(), v.clone());
            }
        } else {
            let v = Value::Combo(c);
            set.insert(
                format!("{{{}}}", hex::encode(crate::bn_serial::content_digest(&v))),
                v,
            );
        }
    }
    if set.len() == 1 {
        return set.into_values().next().unwrap();
    }
    let bodies = ComboVal::new(
        set.into_iter().collect(),
        true,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    );
    let mut holder = IndexMap::new();
    holder.insert("%pattern".to_string(), pattern);
    holder.insert("%bodies".to_string(), Value::Combo(bodies));
    Value::Combo(ComboVal::new(
        holder,
        true,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

pub(crate) fn is_plain_table_key(key: &str) -> bool {
    !(key.starts_with('%') || key.starts_with('/') || key.starts_with('@') || key.starts_with('~'))
}

/// The branch a table key spells: the rule, the constraint, and its name.
/// A key that spells no branch is absent — navigation does not use `_`.
pub(crate) fn table_branch_for_name(c: &ComboVal, key: &str) -> Option<(Value, Value, String)> {
    if !is_plain_table_key(key) || key.is_empty() {
        return None;
    }
    match c.get_field("%rules") {
        Some(Value::Combo(rules)) if rules_have_pattern(rules) => {
            let pattern = constraint_from_data_key(key);
            let name = pattern_branch_name(&pattern);
            rules.get_field(&name).cloned().map(|r| (r, pattern, name))
        }
        _ => None,
    }
}

/// After bodies are forced, re-key each branch's set by content and drop
/// duplicates. One body is that body. The digest does not see a thunk's
/// context, because the thunk has already been forced.
pub(crate) fn canonical_bodies(v: Value) -> Value {
    match v {
        Value::Combo(mut c) => {
            if let Some(Value::Combo(bs)) = c.get_field("%bodies").cloned() {
                let mut set: std::collections::BTreeMap<String, Value> =
                    std::collections::BTreeMap::new();
                for (_, b) in bs.data.iter() {
                    let b = canonical_bodies(b.clone());
                    let key = format!(
                        "{{{}}}",
                        hex::encode(crate::bn_serial::content_digest(&b))
                    );
                    set.insert(key, b);
                }
                if set.len() == 1 {
                    return set.into_values().next().unwrap();
                }
                let bodies = ComboVal::new(
                    set.into_iter().collect(),
                    true,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                );
                c.insert_field("%bodies", Value::Combo(bodies));
                return Value::Combo(c);
            }
            for axis in [
                &mut c.data,
                &mut c.types,
                &mut c.rules,
                &mut c.meta,
                &mut c.system,
                &mut c.local,
            ] {
                let keys: Vec<String> = axis.keys().cloned().collect();
                for k in keys {
                    if let Some(x) = axis.shift_remove(&k) {
                        axis.insert(k, canonical_bodies(x));
                    }
                }
            }
            Value::Combo(c)
        }
        other => other,
    }
}

/// A value with a pattern key has one shape: every plain data key is the
/// branch its constraint names. Does not walk into `%rules` or into values
/// that are not themselves tables — a rules combo is not normalized again.
pub(crate) fn normalize_table(mut c: ComboVal) -> ComboVal {
    let rules = match c.get_field("%rules") {
        Some(Value::Combo(r)) if rules_have_pattern(r) => r.clone(),
        _ => return c,
    };
    let moved: Vec<(String, Value)> = c
        .data
        .iter()
        .filter(|(k, _)| is_plain_table_key(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if moved.is_empty() {
        return c;
    }
    let mut out = rules.clone();
    for (k, v) in moved {
        c.data.shift_remove(&k);
        let pattern = constraint_from_data_key(&k);
        let name = pattern_branch_name(&pattern);
        let arriving = branch_cocoon(pattern, v);
        match (out.data.get(&name).cloned(), arriving) {
            (Some(Value::Combo(e)), Value::Combo(r)) => {
                out.data.insert(name, join_rule_cocoons(e, r));
            }
            (_, arriving) => {
                out.data.insert(name, arriving);
            }
        }
    }
    c.insert_field("%rules", Value::Combo(out));
    c
}

impl Ouroboros {
    /// Data keys that arrived beside an already-built `%rules` are
    /// constraints too. A key whose pattern already names a branch joins
    /// that branch's body set. Nothing is forced here: dispatch forces a
    /// body only when it selects the branch. A new key is added.
    pub(crate) fn rules_with_parent_data(
        &self,
        rules: &ComboVal,
        parent: &ComboVal,
        _arg: &Value,
        _ctx: &mut EvalContext,
    ) -> ComboVal {
        let mut out = rules.clone();
        for (k, v) in &parent.data {
            let pattern = constraint_from_data_key(k);
            let name = pattern_branch_name(&pattern);
            if let Some(existing) = out.data.get(&name).cloned() {
                let arriving = branch_cocoon(pattern.clone(), v.clone());
                if let (Value::Combo(e), Value::Combo(r)) = (existing, arriving) {
                    out.data.insert(name, join_rule_cocoons(e, r));
                }
            } else {
                out.data.insert(name, branch_cocoon(pattern, v.clone()));
            }
        }
        out
    }

    pub fn dispatch_morphism(
        &self,
        rules: &ComboVal,
        arg: &Value,
        ctx: &mut EvalContext,
    ) -> MorphismDispatchResult {
        // (pattern_key, pattern_value, unified, rule_val)
        let mut matching_branches: Vec<(String, Value, Value, Value)> = Vec::new();

        for (pattern_key, rule_val) in rules.all_fields_iter() {
            if pattern_key.starts_with('%') {
                continue;
            }
            let pattern_value = match &rule_val {
                Value::Combo(rc) => match rc.get_field("%pattern") {
                    Some(p) => self.force(p.clone(), ctx),
                    // Binder-only rule: no stored pattern, matches any argument.
                    None => Value::Top,
                },
                _ => Value::Top,
            };
            let unified = self.unify_internal(arg.clone(), pattern_value.clone(), ctx);

            if !matches!(unified, Value::Bottom(_)) {
                matching_branches.push((
                    pattern_key.clone(),
                    pattern_value,
                    unified,
                    rule_val.clone(),
                ));
            }
        }

        if matching_branches.is_empty() {
            return MorphismDispatchResult::NoMatch;
        }

        let minimal_elements = self.filter_minimal_branches(&matching_branches, ctx);

        match minimal_elements.len() {
            0 => MorphismDispatchResult::NoMatch,
            1 => {
                let (pattern_key, _, _, rule) = &minimal_elements[0];
                let result =
                    self.apply_single_rule(rule.clone(), arg.clone(), pattern_key.clone(), ctx);
                MorphismDispatchResult::Single(result)
            }
            _ => {
                let results: Vec<Value> = minimal_elements
                    .iter()
                    .map(|(pattern_key, _, _, rule)| {
                        self.apply_single_rule(rule.clone(), arg.clone(), pattern_key.clone(), ctx)
                    })
                    .filter(|v| !matches!(v, Value::Bottom(_)))
                    .collect();

                if results.is_empty() {
                    MorphismDispatchResult::NoMatch
                } else if results.len() == 1 {
                    MorphismDispatchResult::Single(results.into_iter().next().unwrap())
                } else {
                    MorphismDispatchResult::Multiple(results)
                }
            }
        }
    }

    /// Minimal elements by **pattern** refinement (SPEC_07 情境 C):
    /// `p_i & p_j == p_i ∧ p_i ≠ p_j` ⇒ p_i is strictly finer ⇒ j is non-minimal.
    fn filter_minimal_branches(
        &self,
        branches: &[(String, Value, Value, Value)],
        ctx: &mut EvalContext,
    ) -> Vec<(String, Value, Value, Value)> {
        let n = branches.len();
        if n <= 1 {
            return branches.to_vec();
        }

        let mut is_minimal = vec![true; n];

        for i in 0..n {
            for j in 0..n {
                if i == j {
                    continue;
                }

                let (_, pattern_i, _, _) = &branches[i];
                let (_, pattern_j, _, _) = &branches[j];

                let meet_ij = self.unify_internal(pattern_i.clone(), pattern_j.clone(), ctx);

                // p_i strictly finer than p_j → j not minimal
                if meet_ij == *pattern_i && meet_ij != *pattern_j {
                    is_minimal[j] = false;
                }
            }
        }

        branches
            .iter()
            .enumerate()
            .filter(|(i, _)| is_minimal[*i])
            .map(|(_, b)| b.clone())
            .collect()
    }

    pub(crate) fn apply_single_rule(
        &self,
        rule: Value,
        arg: Value,
        pattern_key: String,
        ctx: &mut EvalContext,
    ) -> Value {
        let rule = self.force(rule, ctx);

        if let Value::Combo(ref rc) = rule {
            // Several bodies of this branch. Each keeps its own closure;
            // `$` is this argument. The results meet. A branch dispatch
            // did not select never reaches here, so its bodies stay thunks.
            if let Some(Value::Combo(bs)) = rc.get_field("%bodies") {
                let mut acc = Value::Top;
                for (_, body) in bs.data.iter() {
                    let r = self.apply_single_rule(body.clone(), arg.clone(), pattern_key.clone(), ctx);
                    acc = self.unify_internal(acc, r, ctx);
                }
                return acc;
            }
        }

        if let Value::Combo(ref rc) = rule {
            // Morphic rule: %code body
            if let Some(Value::Code(expr)) = rc.get_field("%code") {
                let mut call_ctx = self.sub_context(ctx);

                // SPEC_07 §4.2.3 (caret_body 2026-07-19): body `^` is a
                // definition-time path abbreviation — hop chain =
                // [definition frames (holder → …)] + body (param frame as
                // current), then root via Parent hops==len. Must NOT inherit
                // call-site scopes after frames (chimera leak: same literal
                // `^^` read different worlds per call site). Bare-name lexical
                // resolution still uses these definition frames only; `$` is
                // the sole call-site data channel (context_value below).
                call_ctx.scopes.clear();
                if let Some(Value::Combo(cc)) = rc.get_field("%closure") {
                    for (_, sv) in &cc.fields() {
                        if let Value::Combo(s) = sv {
                            call_ctx.scopes.push(std::sync::Arc::new(s.clone()));
                        }
                    }
                }

                let mut arg_map = IndexMap::new();
                // `x @T` stores the binder next to the pattern. A binder-only
                // rule has no `%pattern`; its key is the binder (or the
                // tuple key). A digest key is not a name. `it` and the
                // implicit `0` are retired (D98).
                let param_from_rule = rc.get_field("%param").and_then(|v| {
                    match self.force(v.clone(), ctx) {
                        Value::Atom(AtomKind::Str(s), _, _) => Some(s),
                        _ => None,
                    }
                });
                if let Some(name) = param_from_rule {
                    arg_map.insert(name, arg.clone());
                } else if rc.get_field("%pattern").is_none() {
                    arg_map.insert(pattern_key.trim().to_string(), arg.clone());
                }

                // G5 R-B: `%params` → positional destructure of a tuple arg.
                if let Some(params_v) = rc.get_field("%params") {
                    let names = match self.force(params_v.clone(), ctx) {
                        Value::Combo(pc) => {
                            let mut names = Vec::new();
                            let mut i = 0usize;
                            loop {
                                match pc.get_field(&i.to_string()) {
                                    Some(Value::Atom(AtomKind::Str(s), _, _)) => {
                                        names.push(s.clone());
                                        i += 1;
                                    }
                                    _ => break,
                                }
                            }
                            names
                        }
                        _ => Vec::new(),
                    };
                    if names.is_empty() {
                        return BottomCause::Conflict.into();
                    }
                    let k = names.len();
                    let arg_f = self.force(arg.clone(), ctx);
                    match extract_tuple_fields(&arg_f, k) {
                        Some(fields) => {
                            for (name, val) in names.into_iter().zip(fields.into_iter()) {
                                arg_map.insert(name, val);
                            }
                        }
                        None => {
                            // Destructure failure (arity / non-tuple) = ⊥ #conflict
                            return BottomCause::Conflict.into();
                        }
                    }
                }

                // Param frame = body "current" level (isomorphic to nested
                // literal inside the holder); ^ = holder, ^^ = holder's parent.
                call_ctx.scopes.push(std::sync::Arc::new(ComboVal::new(
                    arg_map,
                    false,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                )));
                call_ctx.context_value = Some(arg.clone());

                let out = self.eval(expr, &mut call_ctx);
                ctx.fuel = call_ctx.fuel;
                return out;
            }

            // E2: constant rule `{{%val: v}}` — return forced v (not unify with arg).
            // If v is itself a morphism, apply it to the argument (e.g.
            // `{ @{ 4.. }: (x -> x + 1) } 5` → 6).
            if let Some(val) = rc.get_field("%val") {
                // A stored thunk context would hide the matched input.
                // `$` in the branch is that input (SYNTAX_12 §2 #5).
                let val = match val.clone() {
                    Value::Thunk {
                        expr,
                        closure,
                        effect,
                        ..
                    } => Value::Thunk {
                        expr,
                        closure,
                        context: Some(Box::new(arg.clone())),
                        effect,
                    },
                    other => other,
                };
                let forced = self.force(val, ctx);
                if forced.is_morphism() {
                    return self.apply_morphism(forced, arg, ctx);
                }
                return forced;
            }
        }

        Value::Bottom(Box::new(BottomDetail {
            cause: BottomCause::Conflict,
            path: None,
            message: Some("Rule has no %code".to_string()),
            expected: None,
            found: Some(rule),
            involved: vec![],
            ..Default::default()
        }))
    }
}

/// G5: argument is a tuple-shaped combo with exact data keys `"0"…"k-1"`.
fn extract_tuple_fields(arg: &Value, k: usize) -> Option<Vec<Value>> {
    let cv = match arg {
        Value::Combo(c) => c,
        _ => return None,
    };
    // Data axis only — exact arity, no extra fields.
    if cv.data.len() != k {
        return None;
    }
    let mut out = Vec::with_capacity(k);
    for i in 0..k {
        let key = i.to_string();
        if !cv.data.contains_key(&key) {
            return None;
        }
        out.push(cv.data.get(&key).cloned().unwrap());
    }
    Some(out)
}

pub enum MorphismDispatchResult {
    Single(Value),
    Multiple(Vec<Value>),
    NoMatch,
}

impl MorphismDispatchResult {
    pub fn to_value(self, effect: EffectTag) -> Value {
        match self {
            MorphismDispatchResult::Single(v) => v.with_effect(effect),
            MorphismDispatchResult::Multiple(vs) => {
                if vs.len() == 1 {
                    vs.into_iter().next().unwrap().with_effect(effect)
                } else {
                    crate::value::normalize_union(vs).with_effect(effect)
                }
            }
            MorphismDispatchResult::NoMatch => BottomCause::NoMatchingBranch.into(),
        }
    }
}
