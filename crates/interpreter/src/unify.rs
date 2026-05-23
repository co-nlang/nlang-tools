use std::collections::HashSet;
use indexmap::IndexMap;
use crate::{Ouroboros, EvalContext};
use crate::value::{Value, ComboVal, BottomCause, BottomDetail, EffectTag};
use crate::type_constraint::{TypeConstraint, type_constraint_meet, is_type_constraint_combo, get_type_constraint_name};
use crate::observation::handle_resource_exhausted;
use nlang_parser::ast::AtomKind;

impl Ouroboros {
    pub fn unify(&self, a: Value, b: Value) -> Value {
        let mut ctx = EvalContext::new(self.root_with_system());
        if let Err(e) = ctx.check_resources(10) { 
            return handle_resource_exhausted(e, ctx.strategy, &ctx.horizon_salt, None, EffectTag::Pure);
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
                let results: Vec<Value> = branches.into_iter().map(|branch| self.unify_internal(branch, other.clone(), ctx)).filter(|v| !matches!(v, Value::Bottom(_))).collect(); 
                match results.len() { 
                    0 => BottomCause::Conflict.into(), 
                    1 => results.into_iter().next().unwrap(), 
                    _ => Value::Union(results)
                } 
            }
            (a, b) => Value::Bottom(Box::new(BottomDetail { 
                cause: BottomCause::Conflict, 
                path: None, 
                message: Some(format!("Incompatible types: {:?} vs {:?}", a, b)),
                expected: Some(a.clone()),
                found: Some(b.clone()),
                involved: vec![a.content_hash(), b.content_hash()]
            })),
        }
    }

    fn unify_combo(&self, a: ComboVal, b: ComboVal, ctx: &mut EvalContext) -> Value {
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
                    involved: vec![] 
                })); 
            }
            if b.closed && !b.contains_key(&key) && !va.is_top() { 
                return Value::Bottom(Box::new(BottomDetail { 
                    cause: BottomCause::MissingKey, 
                    path: Some(key.clone()), 
                    message: Some(format!("Key '{}' missing in incoming closed Cocoon", key)), 
                    expected: Some(va.clone()), 
                    found: None, 
                    involved: vec![] 
                })); 
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