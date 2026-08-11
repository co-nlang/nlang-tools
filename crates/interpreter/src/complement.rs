use crate::observation::handle_resource_exhausted;
use crate::value::{BottomCause, BottomDetail, ComboVal, Value};
use crate::{mbu, EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;

impl Ouroboros {
    pub fn orthocomplement(&self, v: Value, ctx: &mut EvalContext) -> Value {
        // Complement distributes through runtime unions/combos.  Every
        // recursive member visit is a semantic subspace expansion, so a
        // dynamically large value cannot make this structural walk free.
        if let Err(e) = ctx.check_resources(mbu::SUBSPACE_EXPANSION) {
            let partial = if crate::observation::needs_partial_body(&e, ctx.strategy) {
                Some(v.clone())
            } else {
                None
            };
            return handle_resource_exhausted(e, ctx.strategy, &*ctx, partial, v.effect());
        }
        let forced = self.force(v.clone(), ctx);
        let effect = forced.effect();

        match forced {
            Value::Top | Value::TopCaused { .. } => Value::Atom(AtomKind::Bottom, effect, None),

            Value::Bottom(_) => Value::Atom(AtomKind::Top, effect, None),

            Value::Atom(AtomKind::Top, e, _) => Value::Atom(AtomKind::Bottom, e, None),
            Value::Atom(AtomKind::Bottom, e, _) => Value::Atom(AtomKind::Top, e, None),

            Value::Atom(AtomKind::Tag(ref t), e, _) if t.trim_start_matches('#') == "true" => {
                Value::Atom(AtomKind::Tag("false".to_string()), e, None)
            }
            Value::Atom(AtomKind::Tag(ref t), e, _) if t.trim_start_matches('#') == "false" => {
                Value::Atom(AtomKind::Tag("true".to_string()), e, None)
            }

            Value::Atom(AtomKind::TagStart, e, _) => Value::Atom(AtomKind::TagEnd, e, None),
            Value::Atom(AtomKind::TagEnd, e, _) => Value::Atom(AtomKind::TagStart, e, None),

            Value::Atom(_, _e, _) => Value::Bottom(Box::new(BottomDetail {
                cause: BottomCause::Conflict,
                path: None,
                message: Some("Orthocomplement not defined for this atom type".to_string()),
                expected: None,
                found: Some(forced),
                involved: vec![],
                ..Default::default()
            })),

            Value::Combo(cv) => self.complement_combo(cv, ctx),

            Value::Union(branches) => {
                let complements: Vec<Value> = branches
                    .into_iter()
                    .map(|b| self.orthocomplement(b, ctx))
                    .collect();

                if complements.len() == 1 {
                    complements.into_iter().next().unwrap()
                } else {
                    let mut acc = complements[0].clone();
                    for c in complements.into_iter().skip(1) {
                        acc = self.unify_internal(acc, c, ctx);
                        if matches!(acc, Value::Bottom(_)) {
                            return acc;
                        }
                    }
                    acc
                }
            }

            Value::Blur(bd) => Value::Blur(bd),

            Value::Thunk { .. } => {
                let inner = self.force(forced, ctx);
                self.orthocomplement(inner, ctx)
            }

            _ => Value::Bottom(Box::new(BottomDetail {
                cause: BottomCause::Conflict,
                path: None,
                message: Some("Orthocomplement not defined for this value type".to_string()),
                expected: None,
                found: Some(forced),
                involved: vec![],
                ..Default::default()
            })),
        }
    }

    fn complement_combo(&self, cv: ComboVal, ctx: &mut EvalContext) -> Value {
        if cv.closed {
            let mut complemented = ComboVal::default();
            complemented.closed = true;
            complemented.effect = cv.effect;
            for (k, v) in cv.all_fields_iter() {
                let c = self.orthocomplement(v, ctx);
                if !matches!(c, Value::Bottom(_) | Value::Atom(AtomKind::Top, _, _)) {
                    complemented.insert_field(&k, c);
                }
            }
            Value::Combo(complemented)
        } else {
            let mut all_complements: Vec<Value> = Vec::new();
            for (_, v) in cv.all_fields_iter() {
                let c = self.orthocomplement(v, ctx);
                if !matches!(c, Value::Atom(AtomKind::Top, _, _)) {
                    all_complements.push(c);
                }
            }

            if all_complements.is_empty() {
                return Value::Atom(AtomKind::Top, cv.effect, None);
            }

            // De Morgan: !(A & B) = !A | !B for open Combos
            crate::value::normalize_union(all_complements)
        }
    }
}

impl Value {
    pub fn is_orthogonal_to(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Atom(AtomKind::Top, _, _), Value::Atom(AtomKind::Bottom, _, _)) => true,
            (Value::Atom(AtomKind::Bottom, _, _), Value::Atom(AtomKind::Top, _, _)) => true,
            (
                Value::Atom(AtomKind::Tag(ref t1), _, _),
                Value::Atom(AtomKind::Tag(ref t2), _, _),
            ) => t1.trim_start_matches('#') != t2.trim_start_matches('#'),
            _ => false,
        }
    }
}
