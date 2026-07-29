use std::sync::Arc;
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, BottomCause, BottomDetail, MasaRef, ContentHash, ComboVal};
use crate::value::ObservationStrategy;
use nlang_parser::ast::{AtomKind, Path, PathAnchor, Span};
use num_traits::ToPrimitive;
use num_bigint::BigInt;

// ── Helpers ──────────────────────────────────────────────────

fn is_field_visible_in_masa(value: &Value, masa_ref: &MasaRef) -> bool {
    match masa_ref {
        MasaRef::Top => true,
        MasaRef::Digest(target_d) => match value.content_hash().masa_ref {
            MasaRef::Top => true,
            MasaRef::Digest(ref field_d) => field_d == target_d,
        },
    }
}

fn extract_list_items(list: &Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Vec<Value> {
    match list {
        Value::Combo(c) => {
            let mut items = Vec::new();
            for i in 0u32.. {
                if let Some(v) = c.get_field(&i.to_string()) {
                    items.push(oo.force(v.clone(), ctx));
                } else { break; }
            }
            items
        }
        _ => vec![],
    }
}

fn strip_projection_meta(value: &Value) -> Value {
    if let Value::Combo(cv) = value {
        let mut stripped = cv.clone();
        stripped.meta.shift_remove("%kind");
        stripped.meta.shift_remove("%masa");
        stripped.meta.shift_remove("%projection");
        stripped.data.shift_remove("%kind");
        stripped.data.shift_remove("%masa");
        stripped.data.shift_remove("%projection");
        Value::Combo(stripped)
    } else {
        value.clone()
    }
}

// ── Registration ─────────────────────────────────────────────

pub fn register_engine_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert("engine.observe".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Atom(AtomKind::Str(path_str), _, _) = arg.collapse() {
            let path = Path { anchor: PathAnchor::Bare, segments: path_str.split('.').map(|s| s.trim().to_string()).collect(), span: Span::default() };
            return oo.resolve_path(&path, ctx);
        }
        BottomCause::Conflict.into()
    }) as Arc<BuiltinFn>);

    // SPEC_08 §4.3 / §6.2 selective discharge (two-axis gate).
    m.insert(
        "effect.run_pure".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            // Axis 1 — before force: no effect_override grant → refuse even
            // pure args (capability is the gate, not the argument).
            if ctx.privilege.effect_override.is_none() {
                return Value::Bottom(Box::new(BottomDetail {
                    cause: BottomCause::PrivilegedRequired,
                    path: None,
                    message: Some(
                        "runPure requires effect_override grant (CLI --privileged / --grant)"
                            .to_string(),
                    ),
                    expected: None,
                    found: None,
                    involved: vec![],
                    ..Default::default()
                }));
            }
            let v = if let Value::Combo(ref c) = arg {
                c.get_field("0").cloned().unwrap_or(arg.clone())
            } else {
                arg
            };
            let forced = oo.force_recursive(v, ctx);
            let actual = forced.effect();
            // Axis 2 — coverage on actual active effects (Q2 all-or-nothing).
            if !ctx.privilege.may_discharge(actual) {
                let may = ctx
                    .privilege
                    .effect_override
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "#none".to_string());
                return Value::Bottom(Box::new(BottomDetail {
                    cause: BottomCause::PrivilegedRequired,
                    path: None,
                    message: Some(format!(
                        "runPure: horizon may discharge {} but the value observes {}",
                        may, actual
                    )),
                    expected: None,
                    found: None,
                    involved: vec![],
                    ..Default::default()
                }));
            }
            // SPEC_08 §6.2: record the discharge *fact* — which active tags
            // were actually overridden, not the mere presence of a grant.
            // Intent for commit re-presentation.
            //
            // ACCEPTOR REPAIR: passes `actual` rather than calling a bare
            // flag-setter. `runPure` over an already-pure value overrides
            // nothing, and the delivered build still demanded a capability at
            // commit and stamped `#privileged_effect` on it — an audit line
            // asserting an intervention that never happened. `#effect_override`
            // is defined as 「強制將**含副作用**節點標記為 `#pure`」; with no
            // effect there is nothing to force. `note_` ignores a Pure set.
            oo.note_privileged_discharge(actual);
            forced.purify_effects()
        }) as Arc<BuiltinFn>,
    );

    m.insert("engine.save".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        // caid_of_the_argument: unwrap only when apply wrapped (`%arg`).
        // Unconditional slot-0 took the first element of tuples / {{0:…}}.
        let v = crate::value::whole_argument(arg);
        let fv = oo.force_recursive(v, ctx);
        if let Ok(hash) = oo.store.put_value(&fv) {
            return Value::Atom(AtomKind::Str(hash.to_string()), EffectTag::IO, None);
        }
        BottomCause::Conflict.into()
    }) as Arc<BuiltinFn>);

    // Phase NEW: /%differential.{1,2,3}
    m.insert("engine.differential".to_string(), Arc::new(|arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| {
        match &arg {
            Value::Atom(AtomKind::Int(n), _, _) => {
                let tag = match n.to_u8().unwrap_or(0) { 1 => "d1_converging", 2 => "d2_branching", 3 => "d3_horizon", _ => "unknown" };
                Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::Pure, None)
            }
            Value::Combo(ref c) => {
                if let Some(Value::Atom(AtomKind::Int(d), _, _)) = c.get_field("%degree") {
                    let tag = match d.to_u8().unwrap_or(1) { 1 => "d1_converging", 2 => "d2_branching", _ => "d3_horizon" };
                    Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::Pure, None)
                } else {
                    Value::Atom(AtomKind::Tag("d1_converging".to_string()), EffectTag::Pure, None)
                }
            }
            _ => Value::Atom(AtomKind::Tag("d1_converging".to_string()), EffectTag::Pure, None),
        }
    }) as Arc<BuiltinFn>);

    // ── Phase 6: project_down, project_up, set_strategy ────────

    m.insert("engine.project_down".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let (target, masa_str) = if let Value::Combo(ref c) = arg {
            let t = c.get_field("target").cloned().unwrap_or(Value::Top);
            let m = c.get_field("masa").map(|v| oo.force(v.clone(), ctx).to_string_plain()).unwrap_or_default();
            (t, m)
        } else { return BottomCause::Conflict.into(); };

        let masa_hash = match ContentHash::parse(&masa_str) {
            Ok(h) => h, Err(_) => return BottomCause::Conflict.into(),
        };
        let target_forced = oo.force(target, ctx);

        let mut result_fields = IndexMap::new();
        if let Value::Combo(ref cv) = target_forced {
            for (k, v) in cv.fields() {
                if is_field_visible_in_masa(&v, &masa_hash.masa_ref) {
                    result_fields.insert(k.clone(), v.clone());
                }
            }
        } else {
            result_fields.insert("%val".to_string(), target_forced.clone());
        }

        result_fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("blur".to_string()), EffectTag::Pure, None));
        result_fields.insert("%masa".to_string(), Value::Atom(AtomKind::Str(masa_str.clone()), EffectTag::Pure, None));
        result_fields.insert("%projection".to_string(), Value::Atom(AtomKind::Tag("down".to_string()), EffectTag::Pure, None));

        let mut cv = crate::value::ComboVal::new(result_fields, false, IndexMap::new(), EffectTag::State, vec![]);
        cv.masa_ref = masa_hash.masa_ref.clone();
        Value::Combo(cv)
    }) as Arc<BuiltinFn>);

    m.insert("engine.project_up".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let sections_val = if let Value::Combo(ref c) = arg {
            c.get_field("sections").cloned().unwrap_or(Value::Top)
        } else { return BottomCause::Conflict.into(); };
        let sections_forced = oo.force(sections_val, ctx);
        let raw_sections = extract_list_items(&sections_forced, oo, ctx);
        if raw_sections.is_empty() { return Value::Top; }

        let sections: Vec<Value> = raw_sections.iter().map(|s| strip_projection_meta(s)).collect();

        // H² compatibility pre-check
        for i in 0..sections.len() {
            for j in (i+1)..sections.len() {
                let hi = sections[i].content_hash();
                let hj = sections[j].content_hash();
                let incompatible = match (&hi.masa_ref, &hj.masa_ref) {
                    (MasaRef::Digest(a), MasaRef::Digest(b)) => a != b,
                    _ => false,
                };
                if incompatible {
                    let mut meta = IndexMap::new();
                    meta.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("blur".to_string()), EffectTag::Pure, None));
                    meta.insert("%h2_obstruction".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
                    return Value::Union(raw_sections);
                }
            }
        }

        let mut result = sections[0].clone();
        for s in &sections[1..] {
            result = oo.unify_internal(result, s.clone(), ctx);
            if let Value::Bottom(_) = result { return result; }
        }
        result
    }) as Arc<BuiltinFn>);

    // Runtime override of the live observation strategy (State effect).
    // Initial value lives at ~%Config.strategy (SPEC_08 §3.1); this morphism
    // only mutates the current EvalContext for the rest of this observation.
    m.insert("engine.set_strategy".to_string(), Arc::new(|arg: Value, _oo: &Ouroboros, ctx: &mut EvalContext| {
        let tag = match &arg {
            Value::Atom(AtomKind::Tag(t), _, _) => t.trim_start_matches('#').to_string(),
            Value::Combo(c) => {
                if let Some(Value::Atom(AtomKind::Tag(t), _, _)) = c.get_field("strategy") {
                    t.trim_start_matches('#').to_string()
                } else { return BottomCause::Conflict.into(); }
            }
            _ => return BottomCause::Conflict.into(),
        };
        ctx.strategy = match tag.as_str() {
            "blur" => ObservationStrategy::Blur,
            "strict" => ObservationStrategy::Strict,
            "approximate" => ObservationStrategy::Approximate,
            _ => return BottomCause::Conflict.into(),
        };
        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::State, None)
    }) as Arc<BuiltinFn>);

    // ── Phase 7: check_oml ────────────────────────────────────

    m.insert("engine.check_oml".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let (a, b) = if let Value::Combo(ref c) = arg {
            let a = c.get_field("a").cloned().unwrap_or(Value::Top);
            let b = c.get_field("b").cloned().unwrap_or(Value::Top);
            (oo.force(a, ctx), oo.force(b, ctx))
        } else { return BottomCause::Conflict.into(); };

        let result = crate::oml::verify_oml(a, b, oo, ctx);
        match result {
            crate::oml::OMLResult::Vacuous =>
                Value::Atom(AtomKind::Tag("oml_vacuous".to_string()), EffectTag::Pure, None),
            crate::oml::OMLResult::Valid =>
                Value::Atom(AtomKind::Tag("oml_valid".to_string()), EffectTag::Pure, None),
            crate::oml::OMLResult::Approximate =>
                Value::Atom(AtomKind::Tag("oml_approximate".to_string()), EffectTag::Pure, None),
            crate::oml::OMLResult::Violation { rhs, expected } => {
                let mut fields = indexmap::IndexMap::new();
                fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("oml_violation".to_string()), EffectTag::Pure, None));
                fields.insert("rhs".to_string(), rhs);
                fields.insert("expected".to_string(), expected);
                if ctx.had_nondistrib_event {
                    fields.insert("%nondistributive".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
                }
                Value::Combo(crate::value::ComboVal::new(fields, true, indexmap::IndexMap::new(), EffectTag::Pure, vec![]))
            }
        }
    }) as Arc<BuiltinFn>);

    // `engine.sign_refine` retired (identity_persistence): language surface
    // must not obtain the operator private key. The ONLY engine consumer of
    // the private key is `oo refine --sign` → `authority::sign_refine`.
    // `engine.add_architect` already retired (store_boundary / REAL_01 §7.2).

    // ── Functor operations (Phase 15) ──────────────────────────────

    m.insert("option.map".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(f), Some(opt_v)) = (c.get_field("0"), c.get_field("1")) {
                let f = f.clone();
                let opt = oo.force(opt_v.clone(), ctx);
                let was_none = match &opt {
                    Value::Atom(AtomKind::Tag(t), _, _) => t.trim_start_matches('#') == "none",
                    _ => false,
                };
                let inner = match &opt {
                    Value::Combo(ref cv) => cv.get_field("%val").cloned(),
                    _ => None,
                };
                if was_none {
                    return Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
                }
                if let Some(val) = inner {
                    let mapped = if matches!(f, Value::Top) { val } else { oo.apply_morphism(f, val, ctx) };
                    let mut res_fields = IndexMap::new();
                    res_fields.insert("%val".to_string(), mapped);
                    return Value::Combo(ComboVal::new(res_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    m.insert("result.map".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(f), Some(res_v)) = (c.get_field("0"), c.get_field("1")) {
                let f = f.clone();
                let res = oo.force(res_v.clone(), ctx);
                match &res {
                    Value::Combo(ref cv) => {
                        if let Some(inner) = cv.get_field("%val").cloned() {
                            let mapped = if matches!(f, Value::Top) { inner } else { oo.apply_morphism(f, inner, ctx) };
                            let mut res_fields = IndexMap::new();
                            res_fields.insert("%val".to_string(), mapped);
                            return Value::Combo(ComboVal::new(res_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
                        }
                        if cv.get_field("%cause").is_some() {
                            return res.clone();
                        }
                    }
                    _ => {}
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    m.insert("result.map_err".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(f), Some(res_v)) = (c.get_field("0"), c.get_field("1")) {
                let f = f.clone();
                let res = oo.force(res_v.clone(), ctx);
                match &res {
                    Value::Combo(ref cv) => {
                        if cv.get_field("%val").is_some() {
                            return res.clone();
                        }
                        if let Some(cause) = cv.get_field("%cause").cloned() {
                            let mapped = if matches!(f, Value::Top) { cause } else { oo.apply_morphism(f, cause, ctx) };
                            let mut res_fields = IndexMap::new();
                            res_fields.insert("%cause".to_string(), mapped);
                            return Value::Combo(ComboVal::new(res_fields, false, IndexMap::new(), EffectTag::Pure, vec![]));
                        }
                    }
                    _ => {}
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // ── Phase 39: equivalence_map + resolve ───────────────────────

    // engine.equivalence_map: _ → {%kind:#equivalence_map, %count:Int, entries:list}  (State)
    // 回傳所有已知 refine 鏈的合成視圖：每個 from_caid 對應其鏈尾 to_caid。
    m.insert("engine.equivalence_map".to_string(), Arc::new(|_arg: Value, oo: &Ouroboros, _ctx: &mut EvalContext| {
        // 1. 取出所有 key（持鎖極短，立即釋放）
        let all_from: Vec<String> = match oo.refine_map.read() {
            Ok(map) => map.keys().cloned().collect(),
            Err(_)  => return BottomCause::Conflict.into(),
        };

        // 2. 對每個 key 跟蹤鏈尾（follow_refine 內部自己取讀鎖，安全）
        let mut entries: Vec<Value> = Vec::new();
        for from_str in &all_from {
            if let Ok(from_hash) = ContentHash::parse(from_str) {
                if let Ok(to_hash) = oo.follow_refine(&from_hash) {
                    let to_str = to_hash.to_string();
                    if to_str != *from_str {
                        let mut entry = IndexMap::new();
                        entry.insert("from".to_string(), Value::Atom(AtomKind::Str(from_str.clone()), EffectTag::State, None));
                        entry.insert("to".to_string(),   Value::Atom(AtomKind::Str(to_str),          EffectTag::State, None));
                        entries.push(Value::Combo(ComboVal::new(entry, false, IndexMap::new(), EffectTag::State, vec![])));
                    }
                }
            }
        }

        // 3. 包裝成 list
        let mut list_fields = IndexMap::new();
        list_fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::State, None));
        for (i, e) in entries.iter().enumerate() {
            list_fields.insert(i.to_string(), e.clone());
        }
        let entries_list = Value::Combo(ComboVal::new(list_fields, false, IndexMap::new(), EffectTag::State, vec![]));

        // 4. 建立結果 Combo
        let mut result = IndexMap::new();
        result.insert("%kind".to_string(),  Value::Atom(AtomKind::Tag("equivalence_map".to_string()), EffectTag::Pure, None));
        result.insert("%count".to_string(), Value::Atom(AtomKind::Int(BigInt::from(entries.len() as i64)), EffectTag::State, None));
        result.insert("entries".to_string(), entries_list);

        Value::Combo(ComboVal::new(result, true, IndexMap::new(), EffectTag::State, vec![]))
    }) as Arc<BuiltinFn>);

    // engine.resolve: {0: caid_str} → Str(State)
    // 跟蹤 refine 鏈到鏈尾，若 CAID 不在 map 中則回傳原字串。
    m.insert("engine.resolve".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let forced = oo.force(v, ctx);
        if let Value::Atom(AtomKind::Str(caid_str), _, _) = forced.collapse() {
            if let Ok(h) = ContentHash::parse(caid_str.as_str()) {
                return match oo.follow_refine(&h) {
                    Ok(resolved) => Value::Atom(AtomKind::Str(resolved.to_string()), EffectTag::State, None),
                    Err(_)       => Value::Top,
                };
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // ── Monad bind (and_then / chain, Phase 16) ────────────────────
 
    m.insert("option.and_then".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(f), Some(opt_v)) = (c.get_field("0"), c.get_field("1")) {
                let f = f.clone();
                let opt = oo.force(opt_v.clone(), ctx);
                let was_none = match &opt {
                    Value::Atom(AtomKind::Tag(t), _, _) => t.trim_start_matches('#') == "none",
                    _ => false,
                };
                let inner = match &opt {
                    Value::Combo(ref cv) => cv.get_field("%val").cloned(),
                    _ => None,
                };
                if was_none {
                    return Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
                }
                if let Some(val) = inner {
                    let applied = if matches!(f, Value::Top) { val } else { oo.apply_morphism(f, val, ctx) };
                    return applied;
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    m.insert("result.and_then".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(f), Some(res_v)) = (c.get_field("0"), c.get_field("1")) {
                let f = f.clone();
                let res = oo.force(res_v.clone(), ctx);
                match &res {
                    Value::Combo(ref cv) => {
                        if let Some(inner) = cv.get_field("%val").cloned() {
                            let applied = if matches!(f, Value::Top) { inner } else { oo.apply_morphism(f, inner, ctx) };
                            return applied;
                        }
                        if cv.get_field("%cause").is_some() {
                            return res.clone();
                        }
                    }
                    _ => {}
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // ── Option combinators (Phase 17) ─────────────────────────────

    m.insert("option.or".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(default_v), Some(opt_v)) = (c.get_field("0"), c.get_field("1")) {
                let default_v = default_v.clone();
                let opt = oo.force(opt_v.clone(), ctx);
                return match opt.collapse() {
                    Value::Atom(AtomKind::Tag(ref t), _, _) if t.trim_start_matches('#') == "none" => {
                        default_v
                    }
                    other => other.clone(),
                };
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    m.insert("option.unwrap_or".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(default_v), Some(opt_v)) = (c.get_field("0"), c.get_field("1")) {
                let default_v = default_v.clone();
                let opt = oo.force(opt_v.clone(), ctx);
                let was_none = match &opt {
                    Value::Atom(AtomKind::Tag(ref t), _, _) => t.trim_start_matches('#') == "none",
                    _ => false,
                };
                let inner = match &opt {
                    Value::Combo(ref cv) => cv.get_field("%val").cloned(),
                    _ => None,
                };
                if was_none { return default_v; }
                if let Some(v) = inner { return v; }
                return default_v;
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    m.insert("option.filter".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(pred_f), Some(opt_v)) = (c.get_field("0"), c.get_field("1")) {
                let pred_f = pred_f.clone();
                let opt = oo.force(opt_v.clone(), ctx);
                let is_none = match &opt {
                    Value::Atom(AtomKind::Tag(ref t), _, _) => t.trim_start_matches('#') == "none",
                    _ => false,
                };
                if is_none {
                    return Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
                }
                let inner = match &opt {
                    Value::Combo(ref cv) => cv.get_field("%val").cloned(),
                    _ => None,
                };
                if let Some(val) = inner {
                    let result = oo.apply_morphism(pred_f, val, ctx);
                    if matches!(result.collapse(), Value::Atom(AtomKind::Tag(ref t), _, _) if t.trim_start_matches('#') == "true") {
                        return opt;
                    }
                }
            }
        }
        Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);

    // ── Phase 18: result.unwrap ───────────────────────────────────

    m.insert("result.unwrap".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let res = oo.force(arg, ctx);
        match &res {
            Value::Combo(ref cv) => {
                if let Some(inner) = cv.get_field("%val").cloned() {
                    return inner;
                }
                if let Some(cause) = cv.get_field("%cause") {
                    return Value::Bottom(Box::new(BottomDetail {
                        cause: BottomCause::Conflict,
                        message: Some(format!("called unwrap on Err: {}", cause.to_string_plain())),
                        ..Default::default()
                    }));
                }
            }
            _ => {}
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // ── Phase 18: result.expect ───────────────────────────────────

    m.insert("result.expect".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(msg_v), Some(res_v)) = (c.get_field("0"), c.get_field("1")) {
                let msg = oo.force(msg_v.clone(), ctx).to_string_plain();
                let res = oo.force(res_v.clone(), ctx);
                match &res {
                    Value::Combo(ref cv) => {
                        if let Some(inner) = cv.get_field("%val").cloned() {
                            return inner;
                        }
                        if let Some(cause) = cv.get_field("%cause") {
                            return Value::Bottom(Box::new(BottomDetail {
                                cause: BottomCause::Conflict,
                                message: Some(format!("{}: {}", msg, cause.to_string_plain())),
                                ..Default::default()
                            }));
                        }
                    }
                    _ => {}
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // ── Phase 18: option.expect ───────────────────────────────────

    m.insert("option.expect".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(msg_v), Some(opt_v)) = (c.get_field("0"), c.get_field("1")) {
                let msg = oo.force(msg_v.clone(), ctx).to_string_plain();
                let opt = oo.force(opt_v.clone(), ctx);
                match &opt {
                    Value::Atom(AtomKind::Tag(ref t), _, _) if t.trim_start_matches('#') == "none" => {
                        return Value::Bottom(Box::new(BottomDetail {
                            cause: BottomCause::Conflict,
                            message: Some(msg),
                            ..Default::default()
                        }));
                    }
                    Value::Combo(ref cv) => {
                        if let Some(inner) = cv.get_field("%val").cloned() {
                            return inner;
                        }
                    }
                    _ => {}
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // ── Phase 26: option/result advanced combinators ───────────────

    // option.zip: {0: opt_a, 1: opt_b} → Option<{0:a, 1:b}>
    m.insert("option.zip".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(va), Some(vb)) = (c.get_field("0"), c.get_field("1")) {
                let oa = oo.force(va.clone(), ctx);
                let ob = oo.force(vb.clone(), ctx);
                let is_none = |v: &Value| matches!(v, Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none");
                let inner = |v: &Value| -> Option<Value> {
                    match v { Value::Combo(ref cv) => cv.get_field("%val").cloned(), _ => None }
                };
                if is_none(&oa) || is_none(&ob) {
                    return Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
                }
                if let (Some(a), Some(b)) = (inner(&oa), inner(&ob)) {
                    let mut pair = IndexMap::new();
                    pair.insert("0".to_string(), a);
                    pair.insert("1".to_string(), b);
                    let pair_val = Value::Combo(ComboVal::new(pair, true, IndexMap::new(), EffectTag::Pure, vec![]));
                    let mut res = IndexMap::new();
                    res.insert("%val".to_string(), pair_val);
                    return Value::Combo(ComboVal::new(res, false, IndexMap::new(), EffectTag::Pure, vec![]));
                }
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // option.flatten: Option<Option<T>> → Option<T>
    m.insert("option.flatten".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let outer = oo.force(v, ctx);
        let none = Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None);
        match &outer {
            Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none" => none,
            Value::Combo(ref cv) => {
                match cv.get_field("%val") {
                    None => Value::Top,
                    Some(inner) => {
                        let inner_forced = oo.force(inner.clone(), ctx);
                        match &inner_forced {
                            Value::Atom(AtomKind::Tag(t), _, _) if t.trim_start_matches('#') == "none" => none,
                            Value::Combo(ref icv) if icv.get_field("%val").is_some() => inner_forced.clone(),
                            _ => Value::Top,
                        }
                    }
                }
            }
            _ => Value::Top,
        }
    }) as Arc<BuiltinFn>);

    // result.and: {0: result_b, 1: result_a}
    m.insert("result.and".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vb), Some(va)) = (c.get_field("0"), c.get_field("1")) {
                let ra = oo.force(va.clone(), ctx);
                return match &ra {
                    Value::Combo(ref cv) if cv.get_field("%val").is_some() => vb.clone(),
                    Value::Combo(ref cv) if cv.get_field("%cause").is_some() => ra.clone(),
                    _ => Value::Top,
                };
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // result.or: {0: result_b, 1: result_a}
    m.insert("result.or".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg {
            if let (Some(vb), Some(va)) = (c.get_field("0"), c.get_field("1")) {
                let ra = oo.force(va.clone(), ctx);
                return match &ra {
                    Value::Combo(ref cv) if cv.get_field("%val").is_some() => ra.clone(),
                    Value::Combo(ref cv) if cv.get_field("%cause").is_some() => vb.clone(),
                    _ => Value::Top,
                };
            }
        }
        Value::Top
    }) as Arc<BuiltinFn>);

    // result.flatten: Result<Result<T,E>,E> → Result<T,E>
    m.insert("result.flatten".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
        let outer = oo.force(v, ctx);
        match &outer {
            Value::Combo(ref cv) => {
                if cv.get_field("%cause").is_some() {
                    return outer.clone();
                }
                if let Some(inner) = cv.get_field("%val") {
                    let inner_forced = oo.force(inner.clone(), ctx);
                    match &inner_forced {
                        Value::Combo(ref icv) if icv.get_field("%val").is_some() || icv.get_field("%cause").is_some() => {
                            return inner_forced.clone();
                        }
                        _ => return Value::Top,
                    }
                }
                Value::Top
            }
            _ => Value::Top,
        }
    }) as Arc<BuiltinFn>);
}