use crate::value::{EffectTag, Value};
use crate::{BuiltinFn, EvalContext, Ouroboros};
use nlang_parser::ast::AtomKind;
use std::collections::HashMap;
use std::sync::Arc;

pub fn register_cond_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert(
        "cond.if".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(cond), Some(then_b), Some(else_b)) =
                    (c.get_field("0"), c.get_field("1"), c.get_field("2"))
                {
                    let fc = oo.force(cond.clone(), ctx);
                    if let Value::Atom(AtomKind::Tag(ref t), ce, _) = fc.collapse() {
                        let branch = if t.trim_start_matches('#') == "true" {
                            then_b
                        } else {
                            else_b
                        };
                        let res = oo.apply_morphism(
                            branch.clone(),
                            Value::Atom(AtomKind::Unit, EffectTag::Pure, None),
                            ctx,
                        );
                        return res.with_effect(*ce);
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "cond.cond".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let target = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg.clone()
            };
            let farg = oo.force(target, ctx);
            if let Value::Combo(ref cv) = farg.collapse() {
                if oo.is_list(&farg, ctx) {
                    for (_k, pair_v) in &cv.fields() {
                        if _k.parse::<usize>().is_ok() {
                            let fpair = oo.force(pair_v.clone(), ctx);
                            if let Value::Combo(ref pc) = fpair.collapse() {
                                if let (Some(cond_m), Some(action_m)) =
                                    (pc.get_field("0"), pc.get_field("1"))
                                {
                                    let fc = oo.force(cond_m.clone(), ctx);
                                    if let Value::Atom(AtomKind::Tag(ref t), ce, _) = fc.collapse()
                                    {
                                        if t.trim_start_matches('#') == "true" {
                                            let res = oo.apply_morphism(
                                                action_m.clone(),
                                                Value::Atom(AtomKind::Unit, EffectTag::Pure, None),
                                                ctx,
                                            );
                                            return res.with_effect(*ce);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "cond.match".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(val_v), Some(pats_v)) = (c.get_field("0"), c.get_field("1")) {
                    let val = oo.force(val_v.clone(), ctx);
                    let pats = oo.force(pats_v.clone(), ctx);
                    if let Value::Combo(ref pc) = pats.collapse() {
                        let mut i = 0usize;
                        while let Some(pair_v) = pc.get_field(&i.to_string()) {
                            let pair = oo.force(pair_v.clone(), ctx);
                            if let Value::Combo(ref pair_c) = pair.collapse() {
                                if let (Some(pat), Some(action)) =
                                    (pair_c.get_field("0"), pair_c.get_field("1"))
                                {
                                    let unified = oo.unify_internal(val.clone(), pat.clone(), ctx);
                                    if !matches!(unified, Value::Bottom(_)) {
                                        // If action is Top, return unified directly (no transformation)
                                        if matches!(action, Value::Top) {
                                            return unified;
                                        }
                                        return oo.apply_morphism(action.clone(), unified, ctx);
                                    }
                                }
                            }
                            i += 1;
                        }
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );
}
