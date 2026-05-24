use std::sync::Arc;
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, BottomCause, BottomDetail, MasaRef, ContentHash};
use crate::value::ObservationStrategy;
use nlang_parser::ast::{AtomKind, Path, PathAnchor, Span};
use num_traits::ToPrimitive;

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
    
    m.insert("engine.save".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = if let Value::Combo(ref c) = arg { c.get_field("0").cloned().unwrap_or(arg.clone()) } else { arg.clone() };
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

    // ── Phase 8: authority signing ─────────────────────────────

    m.insert("engine.sign_refine".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let (src_strs, tgt_strs) = if let Value::Combo(ref c) = arg {
            let extract_list = |key: &str, c: &crate::value::ComboVal, oo: &Ouroboros, ctx: &mut EvalContext| -> Vec<String> {
                let mut result = Vec::new();
                for i in 0u32.. {
                    if let Some(Value::Combo(lc)) = c.get_field(key) {
                        if let Some(v) = lc.get_field(&i.to_string()) {
                            result.push(oo.force(v.clone(), ctx).to_string_plain());
                        } else { break; }
                    } else { break; }
                }
                result
            };
            (extract_list("source_caids", c, oo, ctx), extract_list("target_caids", c, oo, ctx))
        } else { return BottomCause::Conflict.into(); };

        let src_hashes: Vec<_> = src_strs.iter().filter_map(|s| ContentHash::parse(s).ok()).collect();
        let tgt_hashes: Vec<_> = tgt_strs.iter().filter_map(|s| ContentHash::parse(s).ok()).collect();
        let payload = crate::authority::compute_refine_payload(&src_hashes, &tgt_hashes);
        match crate::authority::sign_refine(&payload, &oo.identity) {
            Ok(auth) => {
                let mut fields = indexmap::IndexMap::new();
                fields.insert("signer_pubkey_hex".to_string(), Value::Atom(AtomKind::Str(auth.signer_pubkey_hex), EffectTag::Pure, None));
                fields.insert("signature_hex".to_string(), Value::Atom(AtomKind::Str(auth.signature_hex), EffectTag::Pure, None));
                if let Some(ts) = auth.timestamp {
                    fields.insert("timestamp".to_string(), Value::Atom(AtomKind::Str(ts), EffectTag::Pure, None));
                }
                Value::Combo(crate::value::ComboVal::new(fields, true, indexmap::IndexMap::new(), EffectTag::IO, vec![]))
            }
            Err(e) => Value::Bottom(Box::new(BottomDetail { cause: BottomCause::Conflict, message: Some(format!("sign_refine: {}", e)), ..Default::default() }))
        }
    }) as Arc<BuiltinFn>);

    m.insert("engine.add_architect".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let pubkey_hex = oo.force(arg, ctx).to_string_plain();
        if pubkey_hex.len() != 64 { return BottomCause::Conflict.into(); }
        if let Ok(mut reg) = oo.architect_registry.write() {
            reg.insert(pubkey_hex);
            if let Some(ref base_dir) = oo.base_dir {
                let _ = oo.store.save_architects(base_dir, &reg);
            }
            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::IO, None)
        } else { BottomCause::Conflict.into() }
    }) as Arc<BuiltinFn>);
}