use crate::{Ouroboros, EvalContext, Value, ComboVal, EffectTag, BottomCause, BottomDetail};
use indexmap::IndexMap;
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;

impl Ouroboros {
    pub fn dispatch_morphism(&self, rules: &ComboVal, arg: &Value, ctx: &mut EvalContext) -> MorphismDispatchResult {
        // (pattern_key, pattern_value, unified, rule_val)
        let mut matching_branches: Vec<(String, Value, Value, Value)> = Vec::new();

        for (pattern_key, rule_val) in rules.all_fields_iter() {
            if pattern_key.starts_with('%') {
                continue;
            }
            let pattern_value = self.resolve_pattern(&pattern_key, ctx);
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

    fn resolve_pattern(&self, pattern_key: &str, ctx: &mut EvalContext) -> Value {
        let trimmed = pattern_key.trim();

        if trimmed == "_" || trimmed == "it" || trimmed == "0" {
            return Value::Top;
        }

        // E2: range canonical keys (`4..#_`, `1..9`, `4..6`, …) must resolve to
        // Value::Range — not fall through to Top (silent match-all).
        if trimmed.contains("..") {
            if let Ok(expr) = nlang_parser::parse_expr_only(trimmed) {
                let v = self.eval(&expr, ctx);
                if matches!(v, Value::Range { .. }) {
                    return v;
                }
                // Parsed but not a range (e.g. arithmetic with `..` in a string
                // path) — fall through to legacy string arms.
            }
        }

        if !trimmed.starts_with('@')
            && !trimmed.starts_with('#')
            && trimmed.parse::<i64>().is_err()
            && trimmed.parse::<f64>().is_err()
            && !(trimmed.starts_with('"') && trimmed.ends_with('"'))
        {
            return Value::Top;
        }

        if trimmed.starts_with('@') {
            let type_name = trimmed.trim_start_matches('@');
            if type_name.starts_with('{') {
                return Value::Top;
            }
            return Value::Combo(ComboVal::new(
                IndexMap::from_iter(vec![
                    (
                        "%kind".to_string(),
                        Value::Atom(
                            AtomKind::Tag("type_constraint".to_string()),
                            EffectTag::Pure,
                            None,
                        ),
                    ),
                    (
                        "%type".to_string(),
                        Value::Atom(
                            AtomKind::Str(type_name.to_string()),
                            EffectTag::Pure,
                            None,
                        ),
                    ),
                ]),
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            ));
        }

        if trimmed.parse::<BigInt>().is_ok() {
            return Value::Atom(
                AtomKind::Int(trimmed.parse::<BigInt>().unwrap()),
                EffectTag::Pure,
                None,
            );
        }

        if trimmed.parse::<f64>().is_ok() {
            return Value::Atom(
                AtomKind::Float(trimmed.parse::<f64>().unwrap()),
                EffectTag::Pure,
                None,
            );
        }

        if trimmed.starts_with('"') && trimmed.ends_with('"') {
            let s = trimmed[1..trimmed.len() - 1].to_string();
            return Value::Atom(AtomKind::Str(s), EffectTag::Pure, None);
        }

        if trimmed.starts_with('#') {
            return Value::Atom(AtomKind::Tag(trimmed.to_string()), EffectTag::Pure, None);
        }

        Value::Top
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

    fn apply_single_rule(
        &self,
        rule: Value,
        arg: Value,
        pattern_key: String,
        ctx: &mut EvalContext,
    ) -> Value {
        let rule = self.force(rule, ctx);

        if let Value::Combo(ref rc) = rule {
            // Morphic rule: %code body
            if let Some(Value::Code(expr)) = rc.get_field("%code") {
                let mut call_ctx = self.sub_context(ctx);

                if let Some(Value::Combo(cc)) = rc.get_field("%closure") {
                    for (_, sv) in &cc.fields() {
                        if let Value::Combo(s) = sv {
                            call_ctx.scopes.push(s.clone());
                        }
                    }
                }

                let param_name = pattern_key.trim().to_string();
                let mut arg_map = IndexMap::new();
                // Whole-argument bindings (keep even under tuple destructure).
                arg_map.insert("it".to_string(), arg.clone());
                arg_map.insert("0".to_string(), arg.clone());
                arg_map.insert(param_name.clone(), arg.clone());

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

                call_ctx
                    .scopes
                    .push(ComboVal::new(arg_map, false, IndexMap::new(), EffectTag::Pure, vec![]));
                call_ctx.context_value = Some(arg.clone());

                let out = self.eval(expr, &mut call_ctx);
                ctx.fuel = call_ctx.fuel;
                return out;
            }

            // E2: constant rule `{{%val: v}}` — return forced v (not unify with arg).
            // If v is itself a morphism, apply it to the argument (e.g.
            // `{ @{ 4.. }: (x -> x + 1) } 5` → 6).
            if let Some(val) = rc.get_field("%val") {
                let forced = self.force(val.clone(), ctx);
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
            MorphismDispatchResult::NoMatch => Value::Bottom(Box::new(BottomDetail {
                cause: BottomCause::Conflict,
                path: None,
                message: Some("No matching branch".to_string()),
                expected: None,
                found: None,
                involved: vec![],
                ..Default::default()
            })),
        }
    }
}
