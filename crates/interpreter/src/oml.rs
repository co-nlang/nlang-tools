use crate::{Ouroboros, EvalContext};
use crate::value::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum OMLResult {
    Vacuous,
    Valid,
    Violation { rhs: Value, expected: Value },
    Approximate,
}

pub fn verify_subspace(a: &Value, b: &Value, oo: &Ouroboros, ctx: &mut EvalContext) -> bool {
    let a_and_b = oo.unify_internal(a.clone(), b.clone(), ctx);
    a_and_b.content_hash().digest == a.content_hash().digest
}

pub fn verify_oml(a: Value, b: Value, oo: &Ouroboros, ctx: &mut EvalContext) -> OMLResult {
    if !verify_subspace(&a, &b, oo, ctx) {
        return OMLResult::Vacuous;
    }

    let not_a = oo.orthocomplement(a.clone(), ctx);
    if let Value::Bottom(_) = not_a {
        return OMLResult::Approximate;
    }

    let b_meet_not_a = oo.unify_internal(b.clone(), not_a, ctx);

    let rhs = match b_meet_not_a {
        Value::Bottom(_) => a.clone(),
        ref bna => join_values(a.clone(), bna.clone()),
    };

    let rhs_digest = rhs.content_hash().digest;
    let b_digest = b.content_hash().digest;

    if rhs_digest == b_digest {
        OMLResult::Valid
    } else {
        match (&a, &b) {
            (Value::Atom(_, _, _), Value::Atom(_, _, _))
            | (Value::Atom(_, _, _), Value::Union(_))
            | (Value::Union(_), _) => OMLResult::Violation { rhs, expected: b },
            _ => OMLResult::Approximate,
        }
    }
}

fn join_values(a: Value, b: Value) -> Value {
    match (&a, &b) {
        (Value::Top, _) | (_, Value::Top) => Value::Top,
        (Value::Bottom(_), _) => b,
        (_, Value::Bottom(_)) => a,
        _ => Value::Union(vec![a, b]),
    }
}
