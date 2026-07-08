use std::collections::{HashMap, HashSet, VecDeque};
use indexmap::IndexMap;
use nlang_parser::ast::{Expr, ExprKind, FieldKey, Prefix, AtomKind};
use crate::{Ouroboros, EvalContext, CmpOp};
use crate::value::{Value, ComboVal, EffectTag, BottomCause, BottomDetail, ValRelation, RelOp as ValRelOp};
use crate::type_constraint::{TypeConstraint, is_type_constraint_combo, get_type_constraint_name};
use crate::observation::handle_resource_exhausted;
use num_bigint::BigInt;
use num_traits::{ToPrimitive, Zero};

#[derive(Debug, Clone, Copy)]
enum MathOp { Add, Sub, Mul, Div, Rem }

impl Ouroboros {
    pub fn predict_effect(&self, expr: &Expr, ctx: &EvalContext) -> EffectTag {
        match &expr.kind {
            ExprKind::Atom(_) => EffectTag::Pure,
            ExprKind::Path(path) => {
                let first = if !path.segments.is_empty() { path.segments[0].trim() } else { "" };
                if first.starts_with("~%") { return EffectTag::IO; }
                let mut e = EffectTag::Pure;
                for scope in ctx.scopes.iter().rev() {
                    if let Some(v) = scope.get_field(first) { e = e.max(v.effect()); break; }
                    let ln = format!("/{}", first);
                    if let Some(v) = scope.get_field(&ln) { e = e.max(v.effect()); break; }
                }
                if let Some(v) = ctx.root.get_field(first) { e = e.max(v.effect()); }
                if let Some(ref s) = ctx.staged { if let Some(v) = s.get_field(first) { e = e.max(v.effect()); } }
                e
            }
            ExprKind::Apply(f, arg) => self.predict_effect(f, ctx).max(self.predict_effect(arg, ctx)),
            ExprKind::Pipe(l, r) => self.predict_effect(l, ctx).max(self.predict_effect(r, ctx)),
            ExprKind::Combo { fields, .. } => {
                let mut e = EffectTag::Pure;
                for f in fields { e = e.max(self.predict_effect(&f.value, ctx)); }
                e
            }
            ExprKind::Meet(a, b) | ExprKind::Join(a, b) | ExprKind::Add(a, b) | ExprKind::Sub(a, b) |
            ExprKind::Mul(a, b) | ExprKind::Div(a, b) | ExprKind::Eq(a, b) | ExprKind::Ne(a, b) => {
                self.predict_effect(a, ctx).max(self.predict_effect(b, ctx))
            }
            ExprKind::Lens(obj, _) => self.predict_effect(obj, ctx),
            ExprKind::List(items) => {
                let mut e = EffectTag::Pure;
                for i in items { e = e.max(self.predict_effect(i, ctx)); }
                e
            }
            _ => EffectTag::Pure,
        }
    }

    pub fn eval(&self, expr: &Expr, ctx: &mut EvalContext) -> Value {
        ctx.depth += 1;
        let res = self.eval_internal(expr, ctx);
        ctx.depth -= 1;
        res
    }

    fn compute_ranks(&self, relations: &[ValRelation]) -> HashMap<String, i64> {
        let mut adj = HashMap::new();
        let mut nodes = HashSet::new();
        let mut eqs: Vec<(String, String)> = Vec::new();
        let mut has_incoming = HashSet::new();
        for r in relations {
            nodes.insert(r.left.clone());
            nodes.insert(r.right.clone());
            match r.op {
                ValRelOp::Lt | ValRelOp::Lte => {
                    adj.entry(r.left.clone()).or_insert(Vec::new()).push(r.right.clone());
                    has_incoming.insert(r.right.clone());
                }
                ValRelOp::Gt | ValRelOp::Gte => {
                    adj.entry(r.right.clone()).or_insert(Vec::new()).push(r.left.clone());
                    has_incoming.insert(r.left.clone());
                }
                ValRelOp::Eq => eqs.push((r.left.clone(), r.right.clone())),
            }
        }
        let mut ranks = HashMap::new();
        let start_node = "#_|_".to_string();
        let mut queue = VecDeque::new();
        if nodes.contains(&start_node) {
            queue.push_back((start_node.clone(), 0i64));
            ranks.insert(start_node, 0);
        } else {
            // implicit boxing (SYNTAX_10 §4.7): #_|_ <= every member — seed the sources at rank 1
            for n in &nodes {
                if !has_incoming.contains(n) {
                    queue.push_back((n.clone(), 1i64));
                    ranks.insert(n.clone(), 1);
                }
            }
        }
        while let Some((u, r)) = queue.pop_front() {
            if let Some(neighbors) = adj.get(&u) {
                for v in neighbors {
                    let new_r = r + 1;
                    let cur_r = ranks.entry(v.clone()).or_insert(new_r);
                    if new_r > *cur_r { *cur_r = new_r; }
                    queue.push_back((v.clone(), *cur_r));
                }
            }
        }
        // same-rank declarations (`=`): propagate known ranks across eq pairs
        let mut changed = true;
        while changed {
            changed = false;
            for (a, b) in &eqs {
                match (ranks.get(a).copied(), ranks.get(b).copied()) {
                    (Some(ra), None) => { ranks.insert(b.clone(), ra); changed = true; }
                    (None, Some(rb)) => { ranks.insert(a.clone(), rb); changed = true; }
                    _ => {}
                }
            }
        }
        ranks
    }

    // Element-position evaluation with spread splicing (ENGINE_SYNC #17;
    // SPEC_03 §3.1 / SYNTAX_04 §4.8): `[...xs, y]` splices the numeric-keyed
    // public fields of xs in index order, reindexed into the target. Unboxing
    // discards the shell and releases inner effect tags; values with no
    // numeric-keyed fields (atoms, Top — no shell to discard) contribute
    // nothing; a Bottom spread source collapses the whole container.
    fn eval_elements(&self, items: &[Expr], ctx: &mut EvalContext) -> std::result::Result<(IndexMap<String, Value>, EffectTag), Value> {
        let mut res = IndexMap::new();
        let mut me = EffectTag::Pure;
        let mut idx = 0usize;
        for item in items {
            if let ExprKind::Spread(inner) = &item.kind {
                let sv = self.force(self.eval(inner, ctx), ctx);
                // both bottom representations collapse the container:
                // runtime Value::Bottom (with cause) and the literal _|_ atom
                if matches!(sv, Value::Bottom(_)) || matches!(sv, Value::Atom(AtomKind::Bottom, _, _)) {
                    return Err(sv);
                }
                me = me.max(sv.effect());
                if let Value::Combo(cv) = sv {
                    let mut keys: Vec<usize> = cv.data.keys().filter_map(|k| k.parse::<usize>().ok()).collect();
                    keys.sort_unstable();
                    for k in keys {
                        if let Some(v) = cv.data.get(&k.to_string()) {
                            me = me.max(v.effect());
                            res.insert(idx.to_string(), v.clone());
                            idx += 1;
                        }
                    }
                }
                continue;
            }
            let val = self.eval(item, ctx);
            me = me.max(val.effect());
            res.insert(idx.to_string(), val);
            idx += 1;
        }
        Ok((res, me))
    }

    // One pipe step for a single (non-superposed) input value: binds $ := lv,
    // evaluates the RHS, then dispatches on its form — morphism application
    // (with list functor-lifting fallback), transformer merge, or passthrough.
    fn pipe_apply(&self, lv: Value, r: &Expr, ctx: &mut EvalContext) -> Value {
        let mut call_ctx = self.sub_context(ctx);
        call_ctx.context_value = Some(lv.clone());
        let rv = self.eval(r, &mut call_ctx);
        ctx.fuel = call_ctx.fuel;
        if rv.is_morphism() {
            let res = self.apply_morphism(rv.clone(), lv.clone(), ctx);
            if matches!(res, Value::Bottom(_) | Value::Top) {
                if let Value::Combo(ref cv) = lv {
                    if self.is_list(&lv, ctx) {
                        let mut res_fields = IndexMap::new();
                        let mut max_e = lv.effect();
                        let mut lifted = false;
                        for (k, v) in &cv.fields() {
                            if k.parse::<usize>().is_ok() {
                                let item = self.force(v.clone(), ctx);
                                let item_res = self.apply_morphism(rv.clone(), item, ctx);
                                if !matches!(item_res, Value::Bottom(_)) {
                                    let solidified = self.force_recursive(item_res, ctx);
                                    max_e = max_e.max(solidified.effect());
                                    res_fields.insert(k.clone(), solidified);
                                    lifted = true;
                                }
                            } else {
                                res_fields.insert(k.clone(), v.clone());
                            }
                        }
                        if lifted {
                            res_fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
                            return Value::Combo(ComboVal::new(res_fields, false, IndexMap::new(), max_e, vec![]));
                        }
                    }
                }
            }
            res
        } else if let Value::Combo(rc) = rv {
            // Stage 2 (§3.3): the transformer arm must unify with call_ctx, not
            // the outer ctx. In the eager era this was immaterial; with lazy unify,
            // thunks forced mid-merge must bind `$` to the pipe input (P3: nearest
            // enclosing evolution), not the observer's context. call_ctx.context_value
            // was set to lv above (line 164).
            let res = self.unify_internal(lv, Value::Combo(rc), &mut call_ctx);
            ctx.fuel = call_ctx.fuel;
            res
        } else {
            // atomic collapse (SPEC_07 §4.1 form 3): forced intersection with
            // the RHS value — was a passthrough that discarded the input
            // (`5 |> #ok` returned #ok instead of _|_; ENGINE_SYNC #18)
            self.unify_internal(lv, rv, ctx)
        }
    }

    fn eval_internal(&self, expr: &Expr, ctx: &mut EvalContext) -> Value {
        if let Err(e) = ctx.check_resources(1) {
            return handle_resource_exhausted(e, ctx.strategy, &ctx.horizon_salt, ctx.fuel, None, EffectTag::Pure);
        }
        match &expr.kind {
            ExprKind::Atom(kind) => match kind.clone() {
                AtomKind::Top => Value::Top,
                k => Value::Atom(k, EffectTag::Pure, None),
            },
            ExprKind::Combo { fields, relations, closed } => {
                if let Err(e) = ctx.check_resources(10 + (fields.len() as u64) * 2) {
                    return handle_resource_exhausted(e, ctx.strategy, &ctx.horizon_salt, ctx.fuel, None, EffectTag::Pure);
                }
                let mut rf = IndexMap::new();
                let mut rl = IndexMap::new();
                let mut me = EffectTag::Pure;
                let mut rv = Vec::new();
                for r in relations {
                    let lt = Value::Atom(r.left.clone(), EffectTag::Pure, None).to_string_plain();
                    let rt = Value::Atom(r.right.clone(), EffectTag::Pure, None).to_string_plain();
                    rv.push(ValRelation {
                        left: lt.clone(),
                        op: match r.op {
                            nlang_parser::ast::RelOp::Lt => ValRelOp::Lt,
                            nlang_parser::ast::RelOp::Gt => ValRelOp::Gt,
                            nlang_parser::ast::RelOp::Lte => ValRelOp::Lte,
                            nlang_parser::ast::RelOp::Gte => ValRelOp::Gte,
                            nlang_parser::ast::RelOp::Eq => ValRelOp::Eq,
                        },
                        right: rt.clone(),
                    });
                    if !rf.contains_key(&lt) { rf.insert(lt, Value::Atom(r.left.clone(), EffectTag::Pure, None)); }
                    if !rf.contains_key(&rt) { rf.insert(rt, Value::Atom(r.right.clone(), EffectTag::Pure, None)); }
                }
                for f in fields {
                    match &f.key {
                        FieldKey::Quoted(name) if name == "..." => {
                            let val = self.eval(&f.value, ctx);
                            if let Value::Combo(ref cv) = val {
                                rf.extend(cv.fields().clone());
                                rl.extend(cv.local_fields().clone());
                                if !*closed { me = me.max(cv.effect); }
                            }
                        }
                        FieldKey::Named { name, prefix } => {
                            let is_p = matches!(prefix, Some(Prefix::Private));
                            let te = self.predict_effect(&f.value, ctx);
                            let mut val = Value::Thunk { expr: Box::new(f.value.clone()), closure: ctx.scopes.clone(), context: ctx.context_value.clone().map(Box::new), effect: te };
                            if matches!(prefix, Some(Prefix::Logic)) {
                                val = Value::Combo(ComboVal::new(
                                    IndexMap::from_iter(vec![
                                        ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                                        ("%val".to_string(), val)
                                    ]),
                                    false,
                                    IndexMap::new(),
                                    EffectTag::Pure,
                                    vec![]
                                ));
                            }
                            if !*closed { me = me.max(te); }
                            let key = match prefix {
                                Some(Prefix::Logic) => format!("/{}", name),
                                Some(Prefix::Type) => format!("@{}", name),
                                Some(Prefix::Meta) => format!("%{}", name),
                                Some(Prefix::System) => format!("~%{}", name),
                                _ => name.trim().to_string(),
                            };
                            if is_p { rl.insert(name.trim().to_string(), val); } else { rf.insert(key, val); }
                        }
                        FieldKey::Quoted(name) => {
                            let te = self.predict_effect(&f.value, ctx);
                            let thunk = Value::Thunk { expr: Box::new(f.value.clone()), closure: ctx.scopes.clone(), context: ctx.context_value.clone().map(Box::new), effect: te };
                            if !*closed { me = me.max(te); }
                            rf.insert(name.trim().to_string(), thunk);
                        }
                        FieldKey::Pattern(pe) => {
                            let pk = self.eval(pe, ctx).to_string_plain().trim().to_string();
                            let te = self.predict_effect(&f.value, ctx);
                            let rb = Value::Combo(ComboVal::new(
                                IndexMap::from_iter(vec![("%val".to_string(), Value::Thunk { expr: Box::new(f.value.clone()), closure: ctx.scopes.clone(), context: ctx.context_value.clone().map(Box::new), effect: te })]),
                                true,
                                IndexMap::new(),
                                te,
                                vec![]
                            ));
                            rf.insert(pk, rb);
                            rf.insert("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
                        }
                        FieldKey::Path(p) => {
                            // Stage 2 (call-by-observation): build a Thunk carrying the
                            // current ctx.context_value (P1 binding) and scopes. In a
                            // closed (cocoon) context, force immediately — cocoon is a
                            // solidification boundary (GUIDE_03 §11.5). In an open combo,
                            // leave the thunk lazy — evolve stores it, observe forces it.
                            let te = self.predict_effect(&f.value, ctx);
                            let thunk = Value::Thunk {
                                expr: Box::new(f.value.clone()),
                                closure: ctx.scopes.clone(),
                                context: ctx.context_value.clone().map(Box::new),
                                effect: te,
                            };
                            let val = if *closed { self.force(thunk, ctx) } else { thunk };
                            if !*closed { me = me.max(te); } else { me = me.max(val.effect()); }
                            let mut tmp = ComboVal::new(IndexMap::new(), *closed, IndexMap::new(), EffectTag::Pure, vec![]);
                            let _ = self.inject_path(&mut tmp, &p.segments, val);
                            rf.extend(tmp.fields());
                            rl.extend(tmp.local_fields());
                        }
                    }
                }
                let ranks = self.compute_ranks(&rv);
                for (tag_name, rank) in ranks {
                    if let Some(v) = rf.get_mut(&tag_name) {
                        if let Value::Atom(ak, ae, _) = v.clone() {
                            rf.insert(tag_name, Value::Atom(ak, ae, Some(rank)));
                        }
                    }
                }
                let mut res = Value::Combo(ComboVal::new(rf, *closed, rl, me, rv));
                if let Value::Combo(ref cv) = res {
                    if let Some(mode_v) = cv.fields().get("%eval_mode") {
                        let m = self.force(mode_v.clone(), ctx);
                        if m.to_string_plain().trim_start_matches('#') == "eager" {
                            res = self.force_recursive(res, ctx);
                        }
                    }
                }
                res
            }
            ExprKind::Path(p) => self.resolve_path(p, ctx),
            ExprKind::Apply(f, a) => {
                if let Err(e) = ctx.check_resources(5) {
                    return handle_resource_exhausted(e, ctx.strategy, &ctx.horizon_salt, ctx.fuel, None, EffectTag::Pure);
                }
                let fv = self.eval(f, ctx);
                let av = self.eval(a, ctx);
                self.apply_morphism(fv.clone(), av.clone(), ctx)
            }
            ExprKind::Pipe(l, r) => {
                let lv = self.eval(l, ctx);
                if let Value::Bottom(_) = lv { return lv; }
                // bind additivity (SPEC_07 §4 疊加態平等演化; ENGINE_SYNC #18):
                // a superposed input evolves branchwise with its OWN $ binding —
                // (A|B) |> f ≡ (A|>f) | (B|>f); ⊥ branches prune (| identity)
                if let Value::Union(branches) = lv {
                    let mut out = Vec::new();
                    for b in branches {
                        let res = self.pipe_apply(b, r, ctx);
                        if !matches!(res, Value::Bottom(_)) && !matches!(res, Value::Atom(AtomKind::Bottom, _, _)) {
                            out.push(res);
                        }
                    }
                    return match out.len() {
                        0 => BottomCause::Conflict.into(),
                        1 => out.into_iter().next().unwrap(),
                        _ => Value::Union(out),
                    };
                }
                self.pipe_apply(lv, r, ctx)
            }
            ExprKind::Morphism { param, body } => {
                let pk = match &param.kind {
                    ExprKind::Path(p) => {
                        let last = p.segments.last().cloned().unwrap_or_else(|| "_".to_string());
                        last.trim().trim_start_matches(|c| c == '/' || c == '@' || c == '~' || c == '%').to_string()
                    }
                    ExprKind::Atom(AtomKind::Tag(t)) => t.trim().to_string(),
                    _ => "_".to_string(),
                };
                let te = self.predict_effect(body, ctx);
                let mut rule_fields = IndexMap::new();
                rule_fields.insert("%code".to_string(), Value::Code(Box::new(*body.clone())));
                let mut closure_fields = IndexMap::new();
                let current_scopes = ctx.scopes.clone();
                for (i, s) in current_scopes.iter().enumerate() {
                    closure_fields.insert(i.to_string(), Value::Combo(s.clone()));
                }
                rule_fields.insert("%closure".to_string(), Value::Combo(ComboVal::new(closure_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
                let mut rules = IndexMap::new();
                rules.insert(pk, Value::Combo(ComboVal::new(rule_fields, true, IndexMap::new(), te, vec![])));
                let mut fields = IndexMap::new();
                fields.insert("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
                fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("logic".to_string()), EffectTag::Pure, None));
                fields.insert("%rules".to_string(), Value::Combo(ComboVal::new(rules, true, IndexMap::new(), te, vec![])));
                Value::Combo(ComboVal::new(fields, true, IndexMap::new(), te, vec![]))
            }
            // $ rules P1-P5 (SPEC_07 §4.2, 2026-07-05): bound only at evolution
            // boundaries (pipe / morphism application); a free $ observed without
            // an enclosing evolution collapses to _|_ #no_context (P3)
            ExprKind::Context => match &ctx.context_value {
                Some(v) => v.clone(),
                None => Value::Bottom(Box::new(BottomDetail {
                    cause: BottomCause::NoContext,
                    message: Some("free `$` observed without an enclosing evolution (P3)".to_string()),
                    ..Default::default()
                })),
            },
            ExprKind::Ternary { cond, then_branch, else_branch } => {
                let cv = self.eval(cond, ctx).collapse().clone();
                match cv {
                    Value::Atom(AtomKind::Tag(ref t), _, _) if t.trim_start_matches('#') == "true" => self.eval(then_branch, ctx).with_effect(cv.effect()),
                    Value::Atom(AtomKind::Tag(ref t), _, _) if t.trim_start_matches('#') == "false" => self.eval(else_branch, ctx).with_effect(cv.effect()),
                    Value::Bottom(d) => Value::Bottom(d),
                    _ => Value::Top,
                }
            }
            ExprKind::TypeAnnotation(v, t) => {
                let vv = self.eval(v, ctx);
                let tv = self.eval(t, ctx);
                self.unify_internal(vv, tv, ctx)
            }
            ExprKind::Meet(a, b) => {
                let va = self.eval(a, ctx);
                let vb = self.eval(b, ctx);
                self.unify_internal(va, vb, ctx)
            }
            ExprKind::Join(a, b) => {
                let va = self.eval(a, ctx);
                let vb = self.eval(b, ctx);
                Value::Union(vec![va, vb])
            }
            ExprKind::Diff(a, b) => {
                let va = self.eval(a, ctx);
                let vb = self.eval(b, ctx);
                let vb_complement = self.orthocomplement(vb, ctx);
                self.unify_internal(va, vb_complement, ctx)
            }
ExprKind::Add(a, b) => self.eval_math(a, b, ctx, MathOp::Add, |x: &BigInt, y: &BigInt| x + y, |x: f64, y: f64| x + y, Some(|sx: &str, sy: &str| format!("{}{}", sx, sy))),
    ExprKind::Sub(a, b) => self.eval_math(a, b, ctx, MathOp::Sub, |x: &BigInt, y: &BigInt| x - y, |x: f64, y: f64| x - y, None::<fn(&str, &str) -> String>),
    ExprKind::Mul(a, b) => self.eval_math(a, b, ctx, MathOp::Mul, |x: &BigInt, y: &BigInt| x * y, |x: f64, y: f64| x * y, None::<fn(&str, &str) -> String>),
    ExprKind::Div(a, b) => self.eval_math(a, b, ctx, MathOp::Div, |x: &BigInt, y: &BigInt| if y.is_zero() { BigInt::zero() } else { x / y }, |x: f64, y: f64| x / y, None::<fn(&str, &str) -> String>),
    ExprKind::Rem(a, b) => self.eval_math(a, b, ctx, MathOp::Rem, |x: &BigInt, y: &BigInt| if y.is_zero() { BigInt::zero() } else { x % y }, |x: f64, y: f64| x % y, None::<fn(&str, &str) -> String>),
            ExprKind::Eq(a, b) => self.eval_binary_cmp(a, b, ctx, CmpOp::Eq),
            ExprKind::Ne(a, b) => self.eval_binary_cmp(a, b, ctx, CmpOp::Ne),
            ExprKind::Lt(a, b) => self.eval_binary_cmp(a, b, ctx, CmpOp::Lt),
            ExprKind::Gt(a, b) => self.eval_binary_cmp(a, b, ctx, CmpOp::Gt),
            ExprKind::Lte(a, b) => self.eval_binary_cmp(a, b, ctx, CmpOp::Lte),
            ExprKind::Gte(a, b) => self.eval_binary_cmp(a, b, ctx, CmpOp::Gte),
            ExprKind::List(items) => {
                match self.eval_elements(items, ctx) {
                    Err(bottom) => bottom,
                    Ok((mut res, me)) => {
                        res.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
                        Value::Combo(ComboVal::new(res, false, IndexMap::new(), me, vec![]))
                    }
                }
            }
            ExprKind::Lens(obj, key) => {
                let ov = self.eval(obj, ctx);
                let kv = self.eval(key, ctx);
                let ks = kv.collapse().to_string_plain();
                self.navigate_segments(ov, &[ks], ctx)
            }
            ExprKind::Interpolated(parts) => {
                let mut res = String::new();
                let mut max_e = EffectTag::Pure;
                for part in parts {
                    match part {
                        nlang_parser::ast::StringPart::Literal(s) => res.push_str(s),
                        nlang_parser::ast::StringPart::Interpolated(expr) => {
                            let val = self.eval(expr, ctx);
                            let solidified = self.force_recursive(val, ctx);
                            max_e = max_e.max(solidified.effect());
                            res.push_str(&solidified.to_string_plain());
                        }
                    }
                }
                Value::Atom(AtomKind::Str(res), max_e, None)
            }
            ExprKind::Structural(e) => {
                // Stage 3 (§3a): structural form <<path>> — symbolic reference.
                // The structural brackets are "this is a reference, don't evaluate
                // it yet." Non-path operands keep current geometric semantics
                // (SYNTAX_07 §2.2).
                if let ExprKind::Path(p) = &e.kind {
                    Value::Ref(p.clone())
                } else {
                    self.eval(e, ctx)
                }
            },
            ExprKind::Unary { op, expr } => {
                let v = self.eval(expr, ctx).collapse().clone();
                match op {
                    nlang_parser::ast::UnaryOp::Not => self.orthocomplement(v, ctx),
                    nlang_parser::ast::UnaryOp::Neg => match v {
                        Value::Atom(AtomKind::Int(i), e, _) => Value::Atom(AtomKind::Int(-i), e, None),
                        Value::Atom(AtomKind::Float(f), e, _) => Value::Atom(AtomKind::Float(-f), e, None),
                        Value::Atom(AtomKind::Complex(r, i), e, _) => Value::Atom(AtomKind::Complex(-r, -i), e, None),
                        _ => BottomCause::Conflict.into(),
                    },
                }
            }
            ExprKind::Spread(e) => self.eval(e, ctx),
            // Tuple (a, b): fixed-arity positional packet — closed combo with numeric keys.
            // The seal is the ARITY seal only (SYNTAX_04 §2.5): no new fields. Effect
            // shielding is Cocoon-exclusive — tuple effect = max over elements
            // (2026-07-06 ruling: decoupled from the cocoon analogy).
            ExprKind::Tuple(items) => {
                if let Err(e) = ctx.check_resources(10 + (items.len() as u64) * 2) {
                    return handle_resource_exhausted(e, ctx.strategy, &ctx.horizon_salt, ctx.fuel, None, EffectTag::Pure);
                }
                match self.eval_elements(items, ctx) {
                    Err(bottom) => bottom,
                    Ok((rf, me)) => Value::Combo(ComboVal::new(rf, true, IndexMap::new(), me, vec![])),
                }
            }
            // Poset literal #{ ... }: relation-only combo; members get ranks (SYNTAX_10)
            ExprKind::Poset(relations) => {
                if let Err(e) = ctx.check_resources(10 + (relations.len() as u64) * 2) {
                    return handle_resource_exhausted(e, ctx.strategy, &ctx.horizon_salt, ctx.fuel, None, EffectTag::Pure);
                }
                let mut rf = IndexMap::new();
                let mut rv = Vec::new();
                for r in relations {
                    let lt = Value::Atom(r.left.clone(), EffectTag::Pure, None).to_string_plain();
                    let rt = Value::Atom(r.right.clone(), EffectTag::Pure, None).to_string_plain();
                    rv.push(ValRelation {
                        left: lt.clone(),
                        op: match r.op {
                            nlang_parser::ast::RelOp::Lt => ValRelOp::Lt,
                            nlang_parser::ast::RelOp::Gt => ValRelOp::Gt,
                            nlang_parser::ast::RelOp::Lte => ValRelOp::Lte,
                            nlang_parser::ast::RelOp::Gte => ValRelOp::Gte,
                            nlang_parser::ast::RelOp::Eq => ValRelOp::Eq,
                        },
                        right: rt.clone(),
                    });
                    if !rf.contains_key(&lt) { rf.insert(lt, Value::Atom(r.left.clone(), EffectTag::Pure, None)); }
                    if !rf.contains_key(&rt) { rf.insert(rt, Value::Atom(r.right.clone(), EffectTag::Pure, None)); }
                }
                let ranks = self.compute_ranks(&rv);
                for (tag_name, rank) in ranks {
                    if let Some(v) = rf.get_mut(&tag_name) {
                        if let Value::Atom(ak, ae, _) = v.clone() {
                            rf.insert(tag_name, Value::Atom(ak, ae, Some(rank)));
                        }
                    }
                }
                Value::Combo(ComboVal::new(rf, false, IndexMap::new(), EffectTag::Pure, rv))
            }
            // Lattice-family equality `=`: non-collapsing structural comparison (partial impl; SYNTAX_06/10)
            ExprKind::LatticeEq(a, b) => {
                let va = self.eval(a, ctx);
                if let Value::Bottom(_) = va { return va; }
                let vb = self.eval(b, ctx);
                if let Value::Bottom(_) = vb { return vb; }
                let res_e = va.effect().max(vb.effect());
                let eq = match (&va, &vb) {
                    (Value::Atom(x, _, _), Value::Atom(y, _, _)) => x == y,
                    _ => va == vb,
                };
                Value::Atom(AtomKind::Tag(if eq { "true".to_string() } else { "false".to_string() }), res_e, None)
            }
            // Direction probe `<=>`: returns an order tag, never a boolean (SYNTAX_10 §2.3)
            ExprKind::Probe(a, b) => {
                let va = self.eval(a, ctx);
                if let Value::Bottom(_) = va { return va; }
                let vb = self.eval(b, ctx);
                if let Value::Bottom(_) = vb { return vb; }
                let res_e = va.effect().max(vb.effect());
                let ca = va.collapse().clone();
                let cb = vb.collapse().clone();
                if ca.is_top() || cb.is_top() { return Value::Top; }
                if let Value::Bottom(d) = ca { return Value::Bottom(d); }
                if let Value::Bottom(d) = cb { return Value::Bottom(d); }
                let dir_tag = |o: Option<std::cmp::Ordering>| -> Value {
                    match o {
                        Some(std::cmp::Ordering::Less) => Value::Atom(AtomKind::Tag("lt".to_string()), res_e, None),
                        Some(std::cmp::Ordering::Greater) => Value::Atom(AtomKind::Tag("gt".to_string()), res_e, None),
                        Some(std::cmp::Ordering::Equal) => Value::Atom(AtomKind::Tag("eq".to_string()), res_e, None),
                        None => Value::Atom(AtomKind::Tag("un".to_string()), res_e, None),
                    }
                };
                let as_f64 = |k: &AtomKind| -> Option<f64> {
                    match k {
                        AtomKind::Int(i) => i.to_f64(),
                        AtomKind::Float(f) => Some(*f),
                        _ => None,
                    }
                };
                match (&ca, &cb) {
                    (Value::Atom(xk, _, rx), Value::Atom(yk, _, ry)) => {
                        if let (Some(x), Some(y)) = (as_f64(xk), as_f64(yk)) {
                            return dir_tag(x.partial_cmp(&y));
                        }
                        if let (AtomKind::Str(x), AtomKind::Str(y)) = (xk, yk) {
                            return dir_tag(Some(x.cmp(y)));
                        }
                        let x_tagish = matches!(xk, AtomKind::Tag(_) | AtomKind::TagStart | AtomKind::TagEnd);
                        let y_tagish = matches!(yk, AtomKind::Tag(_) | AtomKind::TagStart | AtomKind::TagEnd);
                        if x_tagish && y_tagish {
                            if xk == yk { return dir_tag(Some(std::cmp::Ordering::Equal)); }
                            if let (Some(x), Some(y)) = (rx, ry) { return dir_tag(Some(x.cmp(y))); }
                            return dir_tag(None); // no shared order info: incomparable
                        }
                        BottomCause::Conflict.into()
                    }
                    _ => BottomCause::Conflict.into(),
                }
            }
            _ => BottomCause::Conflict.into(),
        }
    }

    fn eval_math<FI, FF, FS>(&self, a: &Expr, b: &Expr, ctx: &mut EvalContext, op: MathOp, op_i: FI, op_f: FF, op_s: Option<FS>) -> Value
        where FI: Fn(&BigInt, &BigInt) -> BigInt, FF: Fn(f64, f64) -> f64, FS: Fn(&str, &str) -> String
    {
        // Stage 2 (紀律 2): value-judgment point — force thunks before arithmetic.
        // `$.a + 1` inside a pipe transformer evals `$.a` to a Thunk (the field
        // is lazily stored); arithmetic needs the actual value.
        let va = self.force(self.eval(a, ctx), ctx);
        if let Value::Bottom(_) = va { return va; }
        let vb = self.force(self.eval(b, ctx), ctx);
        if let Value::Bottom(_) = vb { return vb; }
        let res_e = va.effect().max(vb.effect());
        let ca = va.collapse();
        let cb = vb.collapse();
        
        if self.is_order_anchor(&ca) || self.is_order_anchor(&cb) {
            return Value::Bottom(Box::new(BottomDetail {
                cause: BottomCause::ArithmeticOnAnchor,
                path: None,
                message: Some("Arithmetic operations on order anchors (#_, #_|_) are prohibited".to_string()),
                expected: None,
                found: Some(if self.is_order_anchor(&ca) { ca.clone() } else { cb.clone() }),
                involved: vec![],
             ..Default::default() }));
        }
        
        match (ca, cb) {
            (Value::Atom(AtomKind::Complex(r1, i1), _, _), Value::Atom(AtomKind::Complex(r2, i2), _, _)) => {
                self.eval_complex_math(*r1, *i1, *r2, *i2, op, res_e)
            }
            (Value::Atom(AtomKind::Complex(r, i), _, _), Value::Atom(AtomKind::Int(y), _, _)) => {
                self.eval_complex_math(*r, *i, y.to_f64().unwrap_or(0.0), 0.0, op, res_e)
            }
            (Value::Atom(AtomKind::Complex(r, i), _, _), Value::Atom(AtomKind::Float(y), _, _)) => {
                self.eval_complex_math(*r, *i, *y, 0.0, op, res_e)
            }
            (Value::Atom(AtomKind::Int(x), _, _), Value::Atom(AtomKind::Complex(r, i), _, _)) => {
                self.eval_complex_math(x.to_f64().unwrap_or(0.0), 0.0, *r, *i, op, res_e)
            }
            (Value::Atom(AtomKind::Float(x), _, _), Value::Atom(AtomKind::Complex(r, i), _, _)) => {
                self.eval_complex_math(*x, 0.0, *r, *i, op, res_e)
            }
            (Value::Atom(AtomKind::Int(x), _, _), Value::Atom(AtomKind::Int(y), _, _)) => {
                let result = op_i(x, y);
                Value::Atom(AtomKind::Int(result), res_e, None)
            }
            (Value::Atom(AtomKind::Float(x), _, _), Value::Atom(AtomKind::Float(y), _, _)) => {
                self.sanitize_float_result(op_f(*x, *y), res_e)
            }
            (Value::Atom(AtomKind::Int(x), _, _), Value::Atom(AtomKind::Float(y), _, _)) => {
                self.sanitize_float_result(op_f(x.to_f64().unwrap_or(0.0), *y), res_e)
            }
            (Value::Atom(AtomKind::Float(x), _, _), Value::Atom(AtomKind::Int(y), _, _)) => {
                self.sanitize_float_result(op_f(*x, y.to_f64().unwrap_or(0.0)), res_e)
            }
            (Value::Atom(AtomKind::Str(x), _, _), Value::Atom(AtomKind::Str(y), _, _)) => {
                if let Some(f) = op_s { Value::Atom(AtomKind::Str(f(x, y)), res_e, None) } else { BottomCause::Conflict.into() }
            }
            (Value::Top, _) | (_, Value::Top) => Value::Top,
            _ => BottomCause::Conflict.into(),
        }
    }
    
    fn eval_complex_math(&self, r1: f64, i1: f64, r2: f64, i2: f64, op: MathOp, effect: EffectTag) -> Value {
        match op {
            MathOp::Add => Value::Atom(AtomKind::Complex(r1 + r2, i1 + i2), effect, None),
            MathOp::Sub => Value::Atom(AtomKind::Complex(r1 - r2, i1 - i2), effect, None),
            MathOp::Mul => Value::Atom(AtomKind::Complex(r1 * r2 - i1 * i2, r1 * i2 + i1 * r2), effect, None),
            MathOp::Div => {
                let denom = r2 * r2 + i2 * i2;
                if denom == 0.0 { return BottomCause::NumericalError.into(); }
                let new_r = (r1 * r2 + i1 * i2) / denom;
                let new_i = (i1 * r2 - r1 * i2) / denom;
                Value::Atom(AtomKind::Complex(new_r, new_i), effect, None)
            }
            MathOp::Rem => BottomCause::Conflict.into(),
        }
    }
    
    fn is_order_anchor(&self, v: &Value) -> bool {
        matches!(v, Value::Atom(AtomKind::TagEnd, _, _) | Value::Atom(AtomKind::TagStart, _, _))
    }
    
    fn sanitize_float_result(&self, result: f64, effect: EffectTag) -> Value {
        if result.is_nan() {
            return Value::Bottom(Box::new(BottomDetail {
                cause: BottomCause::NumericalError,
                path: None,
                message: Some("NaN result collapsed to _|_".to_string()),
                expected: None,
                found: None,
                involved: vec![],
             ..Default::default() }));
        }
        if result.is_infinite() {
            if result.is_sign_positive() {
                return Value::Atom(AtomKind::TagEnd, effect, None);
            } else {
                return Value::Atom(AtomKind::TagStart, effect, None);
            }
        }
        Value::Atom(AtomKind::Float(result), effect, None)
    }

    fn eval_binary_cmp(&self, a: &Expr, b: &Expr, ctx: &mut EvalContext, op: CmpOp) -> Value {
        // Stage 2 (紀律 2): value-judgment point — force thunks before comparison.
        let va = self.force(self.eval(a, ctx), ctx);
        if let Value::Bottom(_) = va { return va; }
        let vb = self.force(self.eval(b, ctx), ctx);
        if let Value::Bottom(_) = vb { return vb; }
        let res_e = va.effect().max(vb.effect());
        let ca = va.collapse();
        let cb = vb.collapse();

        if ca.is_top() || cb.is_top() { return Value::Top; }
        if let Value::Bottom(d) = ca { return Value::Bottom(d.clone()); }
        if let Value::Bottom(d) = cb { return Value::Bottom(d.clone()); }

        let op_fn = |x: f64, y: f64| match op {
            CmpOp::Eq => x == y,
            CmpOp::Ne => x != y,
            CmpOp::Lt => x < y,
            CmpOp::Gt => x > y,
            CmpOp::Lte => x <= y,
            CmpOp::Gte => x >= y,
        };

        match (ca.clone(), cb.clone()) {
            (Value::Atom(AtomKind::Int(x), _, _), Value::Atom(AtomKind::Int(y), _, _)) => {
                return Value::Atom(AtomKind::Tag(if op_fn(x.to_f64().unwrap_or(0.0), y.to_f64().unwrap_or(0.0)) { "true".to_string() } else { "false".to_string() }), res_e, None);
            }
            (Value::Atom(AtomKind::Float(x), _, _), Value::Atom(AtomKind::Float(y), _, _)) => {
                return Value::Atom(AtomKind::Tag(if op_fn(x, y) { "true".to_string() } else { "false".to_string() }), res_e, None);
            }
            (Value::Atom(AtomKind::Int(x), _, _), Value::Atom(AtomKind::Float(y), _, _)) => {
                return Value::Atom(AtomKind::Tag(if op_fn(x.to_f64().unwrap_or(0.0), y) { "true".to_string() } else { "false".to_string() }), res_e, None);
            }
            (Value::Atom(AtomKind::Float(x), _, _), Value::Atom(AtomKind::Int(y), _, _)) => {
                return Value::Atom(AtomKind::Tag(if op_fn(x, y.to_f64().unwrap_or(0.0)) { "true".to_string() } else { "false".to_string() }), res_e, None);
            }
            _ => {}
        }

        match op {
            CmpOp::Eq => return Value::Atom(AtomKind::Tag(if ca == cb { "true".to_string() } else { "false".to_string() }), res_e, None),
            CmpOp::Ne => return Value::Atom(AtomKind::Tag(if ca != cb { "true".to_string() } else { "false".to_string() }), res_e, None),
            _ => {}
        }

        if let (Value::Atom(ak, _, rx), Value::Atom(bk, _, ry)) = (ca, cb) {
            if matches!(ak, AtomKind::Tag(_) | AtomKind::TagStart | AtomKind::TagEnd) &&
               matches!(bk, AtomKind::Tag(_) | AtomKind::TagStart | AtomKind::TagEnd) {
                if let (Some(rx_val), Some(ry_val)) = (rx, ry) {
                    return Value::Atom(AtomKind::Tag(if op_fn(*rx_val as f64, *ry_val as f64) { "true".to_string() } else { "false".to_string() }), res_e, None);
                }
            }
        }

        if matches!(op, CmpOp::Lte | CmpOp::Gte) {
            if let (Value::Combo(ac), Value::Combo(bc)) = (ca.clone(), cb.clone()) {
                if is_type_constraint_combo(&ac) && is_type_constraint_combo(&bc) {
                    if let (Some(na), Some(nb)) = (get_type_constraint_name(&ac), get_type_constraint_name(&bc)) {
                        let ta = TypeConstraint::from_name(&na);
                        let tb = TypeConstraint::from_name(&nb);
                        let result = match op {
                            CmpOp::Lte => self.check_subtype_relation(&ta, &tb),
                            CmpOp::Gte => self.check_subtype_relation(&tb, &ta),
                            _ => false,
                        };
                        return Value::Atom(AtomKind::Tag(if result { "true".to_string() } else { "false".to_string() }), res_e, None);
                    }
                }
            }
        }

        BottomCause::Conflict.into()
    }
}