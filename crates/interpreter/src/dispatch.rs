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

/// A table's non-pattern key is a constraint. D107: the key is the
/// coordinate string, and the table reads it as the atom the parser
/// would read from that spelling. `_` is Top, an integer spelling is
/// that integer, `#ident` / `#_` / `#_|_` is that tag, and anything
/// else is that string. `#a:` and `"#a":` are one coordinate, so both
/// are the tag. A string that only looks like another atom is written
/// `@{ "…" }:`, which never reaches here.
pub(crate) fn constraint_from_data_key(key: &str) -> Value {
    if key == "_" {
        return Value::Top;
    }
    if let Ok(n) = key.parse::<BigInt>() {
        return Value::Atom(AtomKind::Int(n), EffectTag::Pure, None);
    }
    // `#_` is TagEnd and `#_|_` is TagStart. A later `#` + ident test
    // would read `#_` as the tag whose name is `_`.
    match key {
        "#_" => return Value::Atom(AtomKind::TagEnd, EffectTag::Pure, None),
        "#_|_" => return Value::Atom(AtomKind::TagStart, EffectTag::Pure, None),
        _ => {}
    }
    if let Some(t) = key.strip_prefix('#') {
        if is_tag_name(t) {
            return Value::Atom(AtomKind::Tag(t.to_string()), EffectTag::Pure, None);
        }
    }
    Value::Atom(AtomKind::Str(key.to_string()), EffectTag::Pure, None)
}

/// `tag` in the grammar: `#` then an identifier.
fn is_tag_name(t: &str) -> bool {
    let mut cs = t.chars();
    matches!(cs.next(), Some(c) if c.is_alphabetic() || c == '_')
        && cs.all(|c| c.is_alphanumeric() || c == '_' || c == '-')
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

/// A body stays code until its branch is chosen when forcing it might
/// run something. The stored effect is not enough: an arrow's tag is
/// pure, so applying one is deferred even when prediction says `#pure`.
/// A closed pure body that does not apply anything may still be a value.
pub(crate) fn is_deferred_body(v: &Value) -> bool {
    match v {
        Value::Thunk { expr, effect, .. } => {
            *effect != EffectTag::Pure
                || nlang_parser::tier::has_free_dollar(expr)
                || expr_applies(expr)
        }
        _ => false,
    }
}

/// An `Apply` outside a nested arrow body. Forcing an arrow builds it
/// and does not run its body, so an application in that body is not
/// this body's application. Both sides of `|>` count: forcing the pipe
/// runs the right-hand side.
fn expr_applies(expr: &nlang_parser::ast::Expr) -> bool {
    use nlang_parser::ast::{Expr, ExprKind, Field, FieldKey, StringPart};
    fn go(expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Apply(_, _) => true,
            // Forcing a pipe runs its right-hand side. An arrow there is
            // predicted pure, the same hole as an application.
            ExprKind::Pipe(lhs, rhs) => go(lhs) || pipe_rhs_underreports(rhs) || go(rhs),
            ExprKind::Morphism { param, body: _ } => go(param),
            ExprKind::Interpolated(parts) => parts.iter().any(|part| match part {
                StringPart::Interpolated(e) => go(e),
                StringPart::Literal(_) => false,
            }),
            ExprKind::Combo { fields, .. } => fields.iter().any(field_applies),
            ExprKind::Meet(a, b)
            | ExprKind::Join(a, b)
            | ExprKind::Diff(a, b)
            | ExprKind::Add(a, b)
            | ExprKind::Sub(a, b)
            | ExprKind::Mul(a, b)
            | ExprKind::Div(a, b)
            | ExprKind::Rem(a, b)
            | ExprKind::Eq(a, b)
            | ExprKind::Ne(a, b)
            | ExprKind::Lt(a, b)
            | ExprKind::Gt(a, b)
            | ExprKind::Lte(a, b)
            | ExprKind::Gte(a, b)
            | ExprKind::LatticeEq(a, b)
            | ExprKind::Probe(a, b)
            | ExprKind::TypeAnnotation(a, b)
            | ExprKind::Lens(a, b) => go(a) || go(b),
            ExprKind::Ternary {
                cond,
                then_branch,
                else_branch,
            } => go(cond) || go(then_branch) || go(else_branch),
            ExprKind::Unary { expr: e, .. }
            | ExprKind::AnonSet(e)
            | ExprKind::Spread(e)
            | ExprKind::Structural(e)
            | ExprKind::Complement(e) => go(e),
            ExprKind::List(items) | ExprKind::Tuple(items) => items.iter().any(go),
            ExprKind::Range { start, end, step } => {
                go(start) || go(end) || step.as_ref().is_some_and(|s| go(s))
            }
            ExprKind::Path(_) | ExprKind::Atom(_) | ExprKind::Poset(_) | ExprKind::Context => false,
        }
    }
    fn field_applies(field: &Field) -> bool {
        let key_applies = match &field.key {
            FieldKey::Pattern(e) => go(e),
            FieldKey::Named { .. } | FieldKey::Quoted(_) | FieldKey::Path(_) => false,
        };
        key_applies || go(&field.value)
    }
    /// A pipe applies this expression. A builtin path's stored effect is
    /// what prediction already recorded; an arrow's tag is pure either way.
    fn pipe_rhs_underreports(expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Morphism { .. } => true,
            ExprKind::Path(p) => !p
                .segments
                .iter()
                .any(|s| s.trim().trim_start_matches('/').starts_with("~%")),
            ExprKind::Pipe(_, rhs) => pipe_rhs_underreports(rhs),
            _ => false,
        }
    }
    go(expr)
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

/// D105: a morphism's rows as table branches. A binder-only rule is the
/// Top row and keeps its binder in `%param`. A morphism with no rules (a
/// builtin, a partial application) is the Top row whose body applies it.
/// `None` when the value is not a morphism, or every row already has a pattern.
pub(crate) fn as_table_morphism(c: &ComboVal) -> Option<ComboVal> {
    let top_name = pattern_branch_name(&Value::Top);
    let mut acc: IndexMap<String, Value> = IndexMap::new();
    let mut converted = false;
    let join = |acc: &mut IndexMap<String, Value>, name: String, v: Value| {
        let v = match (acc.shift_remove(&name), v) {
            (Some(Value::Combo(e)), Value::Combo(r)) => join_rule_cocoons(e, r),
            (_, v) => v,
        };
        acc.insert(name, v);
    };
    match c.get_field("%rules") {
        Some(Value::Combo(rules)) => {
            for (k, v) in rules.all_fields_iter() {
                if k.starts_with('%') {
                    continue;
                }
                match v {
                    Value::Combo(rc) if rc.get_field("%pattern").is_some() => {
                        join(&mut acc, k.clone(), Value::Combo(rc.clone()));
                    }
                    Value::Combo(rc) => {
                        let mut fields: IndexMap<String, Value> = rc
                            .all_fields_iter()
                            .map(|(fk, fv)| (fk.clone(), fv.clone()))
                            .collect();
                        fields.insert("%pattern".to_string(), Value::Top);
                        if rc.get_field("%param").is_none() && rc.get_field("%params").is_none() {
                            fields.insert(
                                "%param".to_string(),
                                Value::Atom(
                                    AtomKind::Str(k.trim().to_string()),
                                    EffectTag::Pure,
                                    None,
                                ),
                            );
                        }
                        let row = ComboVal::new(fields, true, IndexMap::new(), rc.effect, vec![]);
                        join(&mut acc, top_name.clone(), Value::Combo(row));
                        converted = true;
                    }
                    _ => return None,
                }
            }
        }
        _ if c.get_field("%builtin").is_some() => {
            acc.insert(top_name, branch_cocoon(Value::Top, Value::Combo(c.clone())));
            converted = true;
        }
        _ => return None,
    }
    if !converted {
        return None;
    }
    let mut effect = EffectTag::Pure;
    for v in acc.values() {
        effect = effect.union(v.effect());
    }
    let mut fields = IndexMap::new();
    fields.insert(
        "%morphism".to_string(),
        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
    );
    fields.insert(
        "%kind".to_string(),
        Value::Atom(AtomKind::Tag("logic".to_string()), EffectTag::Pure, None),
    );
    fields.insert(
        "%rules".to_string(),
        Value::Combo(ComboVal::new(acc, false, IndexMap::new(), effect, vec![])),
    );
    Some(ComboVal::new(fields, true, IndexMap::new(), effect, vec![]))
}

pub(crate) fn is_morphism_combo(c: &ComboVal) -> bool {
    c.get_field("%morphism").is_some()
        || c.get_field("%builtin").is_some()
        || matches!(c.get_field("%rules"), Some(Value::Combo(_)))
}

impl Ouroboros {
    /// D105: `f & g` of two morphisms is one table. Fires only when a side
    /// has a row without a pattern; two tables already meet. Two equal
    /// morphisms never get here (`unify_internal` returns the one).
    pub(crate) fn meet_as_tables(
        &self,
        a: &ComboVal,
        b: &ComboVal,
        ctx: &mut EvalContext,
    ) -> Option<Value> {
        if !(is_morphism_combo(a) && is_morphism_combo(b)) {
            return None;
        }
        let ta = as_table_morphism(a);
        let tb = as_table_morphism(b);
        if ta.is_none() && tb.is_none() {
            return None;
        }
        let ta = Value::Combo(ta.unwrap_or_else(|| a.clone()));
        let tb = Value::Combo(tb.unwrap_or_else(|| b.clone()));
        Some(self.unify_internal(ta, tb, ctx))
    }
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
pub(crate) fn normalize_table(c: ComboVal) -> ComboVal {
    let mut c = normalize_table_rows(c);
    // D109: deferred bodies carry only what they capture, so one body has one
    // identity however its table was built.
    if let Some(Value::Combo(rules)) = c.get_field("%rules").cloned() {
        if rules_have_pattern(&rules) {
            let mut out = rules.clone();
            for (k, v) in rules.data.iter() {
                out.data.insert(k.clone(), narrow_rule(v.clone()));
            }
            c.insert_field("%rules", Value::Combo(out));
        }
    }
    c
}

fn narrow_rule(v: Value) -> Value {
    match v {
        Value::Combo(mut rc) if is_rule_cocoon(&rc) => {
            if let Some(body) = rc.meta.get("val").cloned() {
                if is_deferred_body(&body) {
                    rc.meta.insert("val".to_string(), crate::eval::narrow_deferred_body(body));
                }
            }
            if let Some(Value::Combo(mut bs)) = rc.meta.get("bodies").cloned() {
                let keys: Vec<String> = bs.data.keys().cloned().collect();
                for k in keys {
                    if let Some(b) = bs.data.shift_remove(&k) {
                        bs.data.insert(k, narrow_rule(b));
                    }
                }
                rc.meta.insert("bodies".to_string(), Value::Combo(bs));
            }
            Value::Combo(rc)
        }
        other => other,
    }
}

fn normalize_table_rows(mut c: ComboVal) -> ComboVal {
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

    /// Navigation reads the value a branch stores. `$` in that body is the
    /// constraint. A stored arrow or table is returned. Dispatch still
    /// applies a selected branch through `apply_single_rule`.
    pub(crate) fn read_branch(
        &self,
        rule: Value,
        arg: Value,
        pattern_key: String,
        ctx: &mut EvalContext,
    ) -> Value {
        let rule = self.force(rule, ctx);
        if let Value::Combo(ref rc) = rule {
            if let Some(Value::Combo(bs)) = rc.get_field("%bodies") {
                let mut acc = Value::Top;
                for (_, body) in bs.data.iter() {
                    let r = self.read_branch(body.clone(), arg.clone(), pattern_key.clone(), ctx);
                    acc = self.unify_internal(acc, r, ctx);
                }
                return acc;
            }
            if rc.get_field("%code").is_none() {
                if let Some(val) = rc.get_field("%val") {
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
                    return self.force(val, ctx);
                }
            }
        }
        self.apply_single_rule(rule, arg, pattern_key, ctx)
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
                let params =
                    ComboVal::new(arg_map, false, IndexMap::new(), EffectTag::Pure, vec![]);
                call_ctx.scopes.push(std::sync::Arc::new(params));
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

/// Branch names and body-set keys are digests written into the value. A
/// value read from (or written to) a store of another identity epoch carries
/// the other epoch's names; re-key them under the epoch now in force.
pub fn rekey_identity_names(v: Value) -> Value {
    match v {
        Value::Combo(c) => Value::Combo(rekey_combo(c)),
        Value::Union(items) => Value::Union(items.into_iter().map(rekey_identity_names).collect()),
        Value::Thunk { expr, closure, context, effect } => Value::Thunk {
            expr,
            closure: closure
                .iter()
                .map(|f| std::sync::Arc::new(rekey_combo(f.as_ref().clone())))
                .collect(),
            context: context.map(|c| Box::new(rekey_identity_names(*c))),
            effect,
        },
        Value::Range { start, end, step } => Value::Range {
            start: Box::new(rekey_identity_names(*start)),
            end: Box::new(rekey_identity_names(*end)),
            step: step.map(|s| Box::new(rekey_identity_names(*s))),
        },
        other => other,
    }
}

fn rekey_map(m: IndexMap<String, Value>) -> IndexMap<String, Value> {
    m.into_iter().map(|(k, v)| (k, rekey_identity_names(v))).collect()
}

fn rekey_combo(mut c: ComboVal) -> ComboVal {
    c.data = rekey_map(std::mem::take(&mut c.data));
    c.types = rekey_map(std::mem::take(&mut c.types));
    c.rules = rekey_map(std::mem::take(&mut c.rules));
    c.meta = rekey_map(std::mem::take(&mut c.meta));
    c.system = rekey_map(std::mem::take(&mut c.system));
    c.local = rekey_map(std::mem::take(&mut c.local));
    if let Some(Value::Combo(bs)) = c.get_field("%bodies").cloned() {
        let mut set: std::collections::BTreeMap<String, Value> = std::collections::BTreeMap::new();
        for (_, b) in bs.data.clone() {
            let key = format!("{{{}}}", hex::encode(crate::bn_serial::content_digest(&b)));
            set.insert(key, b);
        }
        let mut nb = bs.clone();
        nb.data = set.into_iter().collect();
        nb.cache_id = crate::value::default_cache_id();
        c.insert_field("%bodies", Value::Combo(nb));
    }
    if let Some(Value::Combo(rules)) = c.get_field("%rules").cloned() {
        if rules_have_pattern(&rules) {
            let mut out = rules.clone();
            out.data = IndexMap::new();
            for (k, r) in rules.data {
                let name = match &r {
                    Value::Combo(rc) => match rc.get_field("%pattern") {
                        Some(p) => pattern_branch_name(p),
                        None => k,
                    },
                    _ => k,
                };
                out.data.insert(name, r);
            }
            out.cache_id = crate::value::default_cache_id();
            c.insert_field("%rules", Value::Combo(out));
        }
    }
    c.cache_id = crate::value::default_cache_id();
    c
}
