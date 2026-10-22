use crate::observation::handle_resource_exhausted;
use crate::value::{BottomCause, BottomDetail, Value};
use crate::{mbu, EffectTag, EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;

/// `@bool` as its two members.
pub(crate) fn bool_members() -> Value {
    Value::Union(vec![
        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
        Value::Atom(AtomKind::Tag("false".to_string()), EffectTag::Pure, None),
    ])
}

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

            // D116: `!(w & !c)` is `!w | c`; `!!c` is `c`.
            Value::Complement { within, without } => {
                let w = self.orthocomplement(*within, ctx);
                self.normalize_union_absorbing(vec![w, *without], ctx)
            }

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

            // D116: `!` is taken in the universe. `!#true` is everything that
            // is not `#true` (`@bool & !#true` is `#false`); `!0`, `!(..0)`,
            // `!@str`, `!{ a: 1 }` have no closed form and stay complements.
            other => Value::Complement {
                within: Box::new(Value::Top),
                without: Box::new(other),
            },
        }
    }

    /// D116: `x & (within & !without)`. Inside `without` is ⊥, outside it is
    /// `x`; a part of each stays a complement. A union is decided member by
    /// member, and `@bool` is its two members.
    pub(crate) fn meet_complement(
        &self,
        x: Value,
        within: Value,
        without: Value,
        ctx: &mut EvalContext,
    ) -> Value {
        let x = self.unify_internal(x, within, ctx);
        let x = match x {
            Value::Combo(ref c) if crate::type_constraint::is_type_value_combo(c)
                && crate::type_constraint::get_type_constraint_name(c).as_deref() == Some("bool") =>
            {
                bool_members()
            }
            other => other,
        };
        match x {
            Value::Bottom(_) => x,
            Value::Complement { within: w, without: c } => {
                let c = self.removed_union(*c, without, ctx);
                self.meet_complement(*w, Value::Top, c, ctx)
            }
            Value::Union(members) => {
                let parts: Vec<Value> = members
                    .into_iter()
                    .map(|m| self.meet_complement(m, Value::Top, without.clone(), ctx))
                    .collect();
                self.normalize_union_absorbing(parts, ctx)
            }
            x => {
                let meet = self.unify_internal(x.clone(), without.clone(), ctx);
                if matches!(meet, Value::Bottom(_)) {
                    x
                } else if self.same_value(meet, x.clone(), ctx) {
                    Value::Bottom(Box::new(BottomDetail {
                        cause: BottomCause::Conflict,
                        message: Some(format!(
                            "{} is inside {}, which this complement leaves out",
                            x.to_nlang(0),
                            without.to_nlang(0)
                        )),
                        ..Default::default()
                    }))
                } else {
                    Value::Complement {
                        within: Box::new(x),
                        without: Box::new(without),
                    }
                }
            }
        }
    }

    /// `a` and `b` are one value once forced (a literal combo's fields are
    /// thunks until a meet forces them).
    fn same_value(&self, a: Value, b: Value, ctx: &mut EvalContext) -> bool {
        self.force_recursive(a, ctx) == self.force_recursive(b, ctx)
    }

    /// `x` lies inside `y`: their meet is `x`.
    fn inside(&self, x: &Value, y: &Value, ctx: &mut EvalContext) -> bool {
        let meet = self.unify_internal(x.clone(), y.clone(), ctx);
        !matches!(meet, Value::Bottom(_)) && self.same_value(meet, x.clone(), ctx)
    }

    /// What two complements leave out, together: a member inside another
    /// is dropped, so `!(..10) & !(..0)` is `!(..10)`. Its own relation, not
    /// union absorption, which a `<=` probe fences off.
    fn removed_union(&self, a: Value, b: Value, ctx: &mut EvalContext) -> Value {
        let mut flat: Vec<Value> = Vec::new();
        for v in [a, b] {
            match v {
                Value::Union(items) => flat.extend(items),
                other => flat.push(other),
            }
        }
        let mut keep: Vec<Value> = Vec::new();
        for (i, m) in flat.iter().enumerate() {
            let covered = flat.iter().enumerate().any(|(j, o)| {
                j != i && self.inside(m, o, ctx) && (!self.inside(o, m, ctx) || j < i)
            });
            if !covered {
                keep.push(m.clone());
            }
        }
        match keep.len() {
            1 => keep.pop().unwrap(),
            _ => Value::Union(keep),
        }
    }
}
