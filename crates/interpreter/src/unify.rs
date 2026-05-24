use std::collections::HashSet;
use indexmap::IndexMap;
use crate::{Ouroboros, EvalContext};
use crate::value::{Value, ComboVal, BottomCause, BottomDetail, EffectTag, MasaRef, BlurDetail};
use crate::type_constraint::{TypeConstraint, type_constraint_meet, is_type_constraint_combo, get_type_constraint_name};
use crate::observation::handle_resource_exhausted;
use nlang_parser::ast::AtomKind;

const EPSILON_COHERENT: f64 = 0.1;

enum MergeDecision {
    Merge,
    H1Split { theta: f64 },
    H2Split,
}

fn phase_merge_decision(a: &ComboVal, b: &ComboVal) -> MergeDecision {
    // Step 1: MASA compatibility check (H²)
    let h2_incompatible = match (&a.masa_ref, &b.masa_ref) {
        (MasaRef::Top, _) | (_, MasaRef::Top) => false,
        (MasaRef::Digest(da), MasaRef::Digest(db)) => da != db,
    };
    if h2_incompatible {
        return MergeDecision::H2Split;
    }

    // Step 2: geometric phase difference (Phase 1b: architecture only)
    // TODO Phase 4: replace with arccos(Tr(P_A · P_B)) eigenvalue computation
    // Returning 0.0 so all Combos merge (architecture-only deployment)
    let theta = 0.0;

    // Step 3: three-way decision
    if theta < EPSILON_COHERENT {
        MergeDecision::Merge
    } else {
        MergeDecision::H1Split { theta }
    }
}

#[allow(dead_code)]
fn approximate_phase_diff(_sketch_a: &str, _sketch_b: &str) -> f64 {
    // TODO Phase 4: replace with real eigenvalue-based computation
    0.0
}

fn make_h1_split_bottom(a: &ComboVal, b: &ComboVal, theta: f64) -> Value {
    Value::Bottom(Box::new(BottomDetail {
        cause: BottomCause::H1Split,
        path: None,
        message: Some(format!("H¹ phase obstruction: θ={:.4} rad ≥ ε_coherent={}", theta, EPSILON_COHERENT)),
        expected: None,
        found: None,
        involved: vec![
            Value::Combo(a.clone()).content_hash(),
            Value::Combo(b.clone()).content_hash(),
        ],
        obstruction_degree: Some(1),
        holonomy: Some(crate::value::Holonomy::Phase(theta)),
    }))
}

fn make_h2_split_bottom(a: &ComboVal, b: &ComboVal) -> Value {
    Value::Bottom(Box::new(BottomDetail {
        cause: BottomCause::H2Split,
        path: None,
        message: Some(format!("H² MASA obstruction: incompatible contexts {} vs {}", a.masa_ref, b.masa_ref)),
        expected: None,
        found: None,
        involved: vec![
            Value::Combo(a.clone()).content_hash(),
            Value::Combo(b.clone()).content_hash(),
        ],
        obstruction_degree: Some(2),
        holonomy: Some(crate::value::Holonomy::NegI),
    }))
}

impl Ouroboros {
    pub fn unify(&self, a: Value, b: Value) -> Value {
        let mut ctx = self.eval_context();
        if let Err(e) = ctx.check_resources(10) { 
            return handle_resource_exhausted(e, ctx.strategy, &ctx.horizon_salt, ctx.fuel, None, EffectTag::Pure);
        }
        self.unify_internal(a, b, &mut ctx)
    }

    pub fn unify_internal(&self, a: Value, b: Value, ctx: &mut EvalContext) -> Value {
        let a = self.force(a, ctx).collapse().clone();
        let b = self.force(b, ctx).collapse().clone();
        let id_a = a.content_hash(); let id_b = b.content_hash();
        if id_a == id_b { return a; }
        if let (Value::Atom(AtomKind::Tag(ta), _, _), Value::Atom(AtomKind::Tag(tb), _, _)) = (&a, &b) { 
            if ta.trim_start_matches('#') == tb.trim_start_matches('#') { return a.clone(); } 
        }
        
        if let (Value::Combo(ac), Value::Combo(bc)) = (&a, &b) {
            if is_type_constraint_combo(ac) && !is_type_constraint_combo(bc) {
                if let Some(type_name) = get_type_constraint_name(ac) {
                    return type_constraint_meet(b.clone(), &type_name);
                }
            }
            if is_type_constraint_combo(bc) && !is_type_constraint_combo(ac) {
                if let Some(type_name) = get_type_constraint_name(bc) {
                    return type_constraint_meet(a.clone(), &type_name);
                }
            }
        }
        
        match (&a, &b) { 
            (Value::Top, Value::Union(_)) => {} 
            (Value::Union(_), Value::Top) => {}
            (Value::Top, _) => return b, 
            (_, Value::Top) => return a, 
            (Value::Bottom(c), _) => return Value::Bottom(c.clone()), 
            (_, Value::Bottom(c)) => return Value::Bottom(c.clone()), 
            _ => {} 
        }
        let cache_key = if id_a.digest <= id_b.digest { (id_a, id_b) } else { (id_b, id_a) };
        if let Ok(memo) = self.unify_memo.read() { if let Some(cached_res) = memo.get(&cache_key) { return cached_res.clone(); } }
        let mut result = self.do_unify(a.clone(), b.clone(), ctx); 
        let combined_effect = a.effect().max(b.effect());
        if let Value::Combo(ref mut cv) = result { cv.effect = cv.effect.max(combined_effect); }
        if !matches!(result, Value::Bottom(_)) { if let Ok(mut memo) = self.unify_memo.write() { memo.insert(cache_key, result.clone()); } }
        result
    }

    fn do_unify(&self, a: Value, b: Value, ctx: &mut EvalContext) -> Value {
        match (a, b) {
            (Value::Atom(AtomKind::Tag(ta), ae, ra), Value::Atom(AtomKind::Tag(tb), be, rb)) if ta.trim_start_matches('#') == tb.trim_start_matches('#') => {
                Value::Atom(AtomKind::Tag(ta), ae.max(be), ra.or(rb))
            }
            (Value::Atom(ak, ae, ra), Value::Atom(bk, be, rb)) if ak == bk => Value::Atom(ak, ae.max(be), ra.or(rb)),
            (Value::Atom(ak, ae, ra), Value::Combo(mut cv)) | (Value::Combo(mut cv), Value::Atom(ak, ae, ra)) => { 
                if is_type_constraint_combo(&cv) {
                    if let Some(type_name) = get_type_constraint_name(&cv) {
                        return type_constraint_meet(Value::Atom(ak, ae, ra), &type_name);
                    }
                }
                let val_key = "%val".to_string(); 
                let existing_val = cv.get_field(&val_key).cloned().unwrap_or(Value::Top); 
                let merged_val = self.unify_internal(Value::Atom(ak, ae, ra), existing_val, ctx); 
                if let Value::Bottom(c) = merged_val { return Value::Bottom(c); } 
                cv.insert_field(&val_key, merged_val); 
                Value::Combo(cv) 
            }
            (Value::Combo(ac), Value::Combo(bc)) => self.unify_combo(ac, bc, ctx),
            (Value::Union(mut branches), other) | (other, Value::Union(mut branches)) => { 
                branches.sort_by_key(|b| self.tropical_weight(b));
                let max_branches = ctx.max_branches;
                let mut results: Vec<Value> = Vec::new();
                for branch in branches.into_iter().take(max_branches * 2) {
                    let r = self.unify_internal(branch, other.clone(), ctx);
                    match &r {
                        Value::Bottom(detail) => {
                            if matches!(detail.cause, BottomCause::H1Split | BottomCause::H2Split) {
                                ctx.had_nondistrib_event = true;
                            }
                        }
                        _ => {
                            results.push(r);
                            if results.len() >= max_branches { break; }
                        }
                    }
                }
                match results.len() { 
                    0 => BottomCause::Conflict.into(), 
                    1 => results.into_iter().next().unwrap(), 
                    _ => Value::Union(results),
                }
            }
            // Blur unification rules
            (Value::Blur(ba), Value::Blur(bb)) => {
                let merged_partial = match (ba.partial.as_deref(), bb.partial.as_deref()) {
                    (Some(pa), Some(pb)) => {
                        let unified = self.unify_internal(pa.clone(), pb.clone(), ctx);
                        if matches!(unified, Value::Bottom(_)) {
                            Some(Box::new(Value::Union(vec![pa.clone(), pb.clone()])))
                        } else {
                            Some(Box::new(unified))
                        }
                    }
                    (Some(p), None) | (None, Some(p)) => Some(Box::new(p.clone())),
                    (None, None) => None,
                };
                let eff = ba.effect.max(bb.effect);
                let base = if ba.horizon.fuel_remaining <= bb.horizon.fuel_remaining { ba } else { bb };
                Value::Blur(BlurDetail {
                    cause: base.cause.clone(),
                    horizon: base.horizon.clone(),
                    partial: merged_partial,
                    effect: eff,
                })
            }
            (Value::Blur(_), b @ Value::Bottom(_)) => b,
            (a @ Value::Bottom(_), Value::Blur(_)) => a,
            (ba @ Value::Blur(_), Value::Top) => ba,
            (Value::Top, bb @ Value::Blur(_)) => bb,
            (Value::Blur(bd), other) => {
                let new_partial = match bd.partial.as_deref() {
                    Some(existing) => {
                        let unified = self.unify_internal(existing.clone(), other.clone(), ctx);
                        if matches!(unified, Value::Bottom(_)) {
                            Some(Box::new(other.clone()))
                        } else {
                            Some(Box::new(unified))
                        }
                    }
                    None => Some(Box::new(other.clone())),
                };
                Value::Blur(BlurDetail { partial: new_partial, ..bd.clone() })
            }
            (other, Value::Blur(bd)) => {
                let new_partial = match bd.partial.as_deref() {
                    Some(existing) => {
                        let unified = self.unify_internal(existing.clone(), other.clone(), ctx);
                        if matches!(unified, Value::Bottom(_)) {
                            Some(Box::new(other.clone()))
                        } else {
                            Some(Box::new(unified))
                        }
                    }
                    None => Some(Box::new(other.clone())),
                };
                Value::Blur(BlurDetail { partial: new_partial, ..bd.clone() })
            }
            (a, b) => Value::Bottom(Box::new(BottomDetail { 
                cause: BottomCause::Conflict, 
                path: None, 
                message: Some(format!("Incompatible types: {:?} vs {:?}", a, b)),
                expected: Some(a.clone()),
                found: Some(b.clone()),
                involved: vec![a.content_hash(), b.content_hash()],
             ..Default::default() })),
        }
    }

    fn unify_combo(&self, a: ComboVal, b: ComboVal, ctx: &mut EvalContext) -> Value {
        // Phase 1b: phase-aware merge entry
        match phase_merge_decision(&a, &b) {
            MergeDecision::H2Split => return make_h2_split_bottom(&a, &b),
            MergeDecision::H1Split { theta } => return make_h1_split_bottom(&a, &b, theta),
            MergeDecision::Merge => {}
        }

        if is_type_constraint_combo(&a) && is_type_constraint_combo(&b) {
            let ta = get_type_constraint_name(&a);
            let tb = get_type_constraint_name(&b);
            if let (Some(na), Some(nb)) = (ta, tb) {
                if na == nb { return Value::Combo(a); }
                let ca = TypeConstraint::from_name(&na);
                let cb = TypeConstraint::from_name(&nb);
                let subtype_check = self.check_subtype_relation(&ca, &cb);
                if subtype_check { return Value::Combo(a); }
                let reverse_check = self.check_subtype_relation(&cb, &ca);
                if reverse_check { return Value::Combo(b); }
            }
        }
        
        let mut rf = IndexMap::new(); let mut rl = IndexMap::new(); 
        let all_keys: HashSet<_> = a.field_keys().into_iter().chain(b.field_keys().into_iter()).collect();
        for key in all_keys {
            let va = a.get_field(&key).cloned().unwrap_or(Value::Top); 
            let vb = b.get_field(&key).cloned().unwrap_or(Value::Top);
            if a.closed && !a.contains_key(&key) && !vb.is_top() { 
                return Value::Bottom(Box::new(BottomDetail { 
                    cause: BottomCause::MissingKey, 
                    path: Some(key.clone()), 
                    message: Some(format!("Key '{}' missing in closed Cocoon", key)), 
                    expected: None, 
                    found: Some(vb.clone()), 
                    involved: vec![],
                 ..Default::default() }));
            }
            if b.closed && !b.contains_key(&key) && !va.is_top() { 
                return Value::Bottom(Box::new(BottomDetail { 
                    cause: BottomCause::MissingKey, 
                    path: Some(key.clone()), 
                    message: Some(format!("Key '{}' missing in incoming closed Cocoon", key)), 
                    expected: Some(va.clone()), 
                    found: None, 
                    involved: vec![],
                 ..Default::default() }));
            }
            let merged = self.unify_internal(va, vb, ctx); 
            if let Value::Bottom(mut detail) = merged { 
                let cp = detail.path.map(|p| format!("{}.{}", key, p)).unwrap_or(key.clone()); 
                detail.path = Some(cp); 
                return Value::Bottom(detail); 
            }
            if !merged.is_top() { rf.insert(key.clone(), merged); }
        }
        let all_lkeys: HashSet<_> = a.local_keys().into_iter().chain(b.local_keys().into_iter()).collect();
        for key in all_lkeys {
            let key_stripped = key.trim_start_matches('~');
            let va = a.local.get(key_stripped).cloned().unwrap_or(Value::Top); 
            let vb = b.local.get(key_stripped).cloned().unwrap_or(Value::Top);
            let merged = self.unify_internal(va, vb, ctx); 
            if let Value::Bottom(mut detail) = merged { 
                let cp = detail.path.map(|p| format!("~{}.{}", key, p)).unwrap_or(format!("~{}", key)); 
                detail.path = Some(cp); 
                return Value::Bottom(detail); 
            }
            if !merged.is_top() { rl.insert(key.clone(), merged); }
        }
        Value::Combo(ComboVal::new(rf, a.closed || b.closed, rl, a.effect.max(b.effect), a.relations.iter().chain(b.relations.iter()).cloned().collect()))
    }
    
    pub fn check_subtype_relation(&self, child: &TypeConstraint, parent: &TypeConstraint) -> bool {
        match (child, parent) {
            (_, TypeConstraint::Any) => true,
            (TypeConstraint::Int, TypeConstraint::Num) => true,
            (TypeConstraint::Float, TypeConstraint::Num) => true,
            (TypeConstraint::Complex, TypeConstraint::Num) => true,
            (TypeConstraint::Float, TypeConstraint::Complex) => true,
            (TypeConstraint::Unknown(a), TypeConstraint::Unknown(b)) if a == b => true,
            _ => false,
        }
    }
}