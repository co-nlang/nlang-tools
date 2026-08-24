use crate::lattice_sketch;
use crate::observation::handle_resource_exhausted;
use crate::type_constraint::{
    get_type_constraint_name, is_type_constraint_combo, type_constraint_meet, TypeConstraint,
};
use crate::value::{
    normalize_union, primary_bottom_from_culled, BlurDetail, BottomCause, BottomDetail, ComboVal,
    EffectTag, MasaRef, Value,
};
use crate::{mbu, EvalContext, Ouroboros};
use indexmap::{IndexMap, IndexSet};
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use num_traits::{ToPrimitive, Zero};
use std::cmp::Ordering;

const EPSILON_COHERENT: f64 = 0.1;

/// REAL_03 §6.7 / Q-037: `ComboVal::pending_spreads` is `#[serde(skip)]` and
/// does not enter the CAID. Two unexpanded spread results (`{ ...{ a: 1 } }`
/// and `{ ...{ b: 2 } }`) therefore share a digest, and a CAID early-out
/// would keep one operand and drop the other — including a disagreement
/// that should have been `⊥ #conflict`.
///
/// The field stays out of identity (G4). A digest that cannot see it is
/// not a license to skip work it would change.
fn caid_is_incomplete(v: &Value) -> bool {
    match v {
        Value::Combo(c) => {
            !c.pending_spreads.is_empty()
                || c.data.values().any(caid_is_incomplete)
                || c.types.values().any(caid_is_incomplete)
                || c.rules.values().any(caid_is_incomplete)
                || c.meta.values().any(caid_is_incomplete)
                || c.system.values().any(caid_is_incomplete)
                || c.local.values().any(caid_is_incomplete)
        }
        Value::Union(xs) => xs.iter().any(caid_is_incomplete),
        _ => false,
    }
}

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

    // Step 2: H¹ phase obstruction — only for explicit MASA context combos.
    // Top-MASA combos are context-free; geometric check is undefined for them.
    let theta = match (&a.masa_ref, &b.masa_ref) {
        (MasaRef::Digest(_), MasaRef::Digest(_)) => lattice_sketch::phase_diff_between(a, b),
        _ => 0.0,
    };

    // Step 3: three-way decision
    if theta < EPSILON_COHERENT {
        MergeDecision::Merge
    } else {
        MergeDecision::H1Split { theta }
    }
}

fn make_h1_split_bottom(a: &ComboVal, b: &ComboVal, theta: f64) -> Value {
    Value::Bottom(Box::new(BottomDetail {
        cause: BottomCause::H1Split,
        path: None,
        message: Some(format!(
            "H¹ phase obstruction: θ={:.4} rad ≥ ε_coherent={}",
            theta, EPSILON_COHERENT
        )),
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
        message: Some(format!(
            "H² MASA obstruction: incompatible contexts {} vs {}",
            a.masa_ref, b.masa_ref
        )),
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
        self.unify_internal(a, b, &mut ctx)
    }

    /// When CAID equal: keep caused-Top provenance if either side has it.
    /// Prefer static_cycle over no_coordinate when both are caused; merge
    /// members when both are static_cycle.
    fn prefer_caused_top(a: Value, b: Value) -> Value {
        match (a, b) {
            (
                Value::TopCaused {
                    cause: c1,
                    members: m1,
                },
                Value::TopCaused {
                    cause: c2,
                    members: m2,
                },
            ) => {
                // Prefer static_cycle; merge members when both static.
                if c1 == "static_cycle" || c2 == "static_cycle" {
                    let mut m = m1;
                    m.extend(m2);
                    crate::value::static_cycle_top(m)
                } else {
                    Value::TopCaused {
                        cause: c1,
                        members: m1,
                    }
                }
            }
            (Value::TopCaused { cause, members }, _) | (_, Value::TopCaused { cause, members }) => {
                Value::TopCaused { cause, members }
            }
            (a, _) => a,
        }
    }

    pub fn unify_internal(&self, a: Value, b: Value, ctx: &mut EvalContext) -> Value {
        // A recursive distribution is still a sequence of semantic lattice
        // intersections/unions.  Put the bill at the shared operation rather
        // than only at surface `&` / `|`, so a runtime-sized union or spread
        // cannot make additional merges free.
        if let Err(e) = ctx.check_resources(mbu::ORTHOGONAL_MERGE) {
            let partial = if crate::observation::needs_partial_body(&e, ctx.strategy) {
                Some(Value::Union(vec![a.clone(), b.clone()]))
            } else {
                None
            };
            return handle_resource_exhausted(e, ctx.strategy, &*ctx, partial, EffectTag::Pure);
        }
        // Stage 2 (call-by-observation): lazy unify — CAID early-out, Top/Thunk
        // preserve thunk, force only when value-judgment needed.
        let id_a = a.content_hash();
        let id_b = b.content_hash();
        if id_a == id_b && !caid_is_incomplete(&a) && !caid_is_incomplete(&b) {
            // Top and TopCaused share a CAID (lattice identity) — prefer
            // caused provenance so static-cycle fields survive combo merge.
            return Self::prefer_caused_top(a, b);
        }

        match (&a, &b) {
            (Value::Top | Value::TopCaused { .. }, Value::Thunk { .. }) => return b,
            (Value::Thunk { .. }, Value::Top | Value::TopCaused { .. }) => return a,
            // Atom(Top) alias — the literal `_` evaluates to Value::Top (eval
            // normalization), but manually constructed Atom(Top) still exists.
            // Re-enter as Value::Top rather than short-circuiting `other.clone()`:
            // a bare return would skip the Value::Top-specific handling below —
            // notably the (Top, Union) fall-through to do_unify, which sorts and
            // caps branches — so Atom(Top) & Union would yield an un-normalized
            // union with a different content_hash than Value::Top & Union. The
            // recursion makes Atom(Top) a *faithful* alias (it cannot re-hit this
            // arm: `other` is the non-Top operand).
            (Value::Atom(AtomKind::Top, _, _), other)
            | (other, Value::Atom(AtomKind::Top, _, _)) => {
                return self.unify_internal(Value::Top, other.clone(), ctx)
            }
            // Atom(Bottom) alias — dual of Atom(Top). Literal `_|_` evaluates to
            // Value::Bottom (eval normalization), but complement / manual
            // construction still emit Atom(Bottom). Re-enter as declared-empty
            // Bottom(Conflict) so absorption arms below run (SYNTAX_06 §4.1).
            (Value::Atom(AtomKind::Bottom, _, _), other)
            | (other, Value::Atom(AtomKind::Bottom, _, _)) => {
                return self.unify_internal(BottomCause::Conflict.into(), other.clone(), ctx);
            }
            // Stage 3 (§3a/§3b): a Ref must survive unify un-dereferenced — the
            // force below runs in the *caller's* context (engine-level at evolve
            // field-merge), and dereferencing there is evolve-time snapshotting,
            // i.e. exactly the A-case semantics the C ruling rejected.
            (Value::Top | Value::TopCaused { .. }, Value::Ref(_)) => return b,
            (Value::Ref(_), Value::Top | Value::TopCaused { .. }) => return a,
            // F3 (§3-fix, hygiene note): Ref vs any non-Top concrete value
            // (Combo, Atom, Union, Thunk, etc.) is NOT preserved here — the
            // match falls through to the force path. In an evolve-context
            // (where the caller's ctx is engine-level with a clean system
            // root), force dereferences the Ref against the system root,
            // producing an A-case snapshot. Currently, this path cannot be
            // triggered for legal Ref-bearing unify pairs: evolven fields
            // arrive as combos with Thunk values, not bare Refs; the C-case
            // pipe result (bare Ref) is only unified against Top via the
            // arms above. If future grammar produces (Ref, Atom) or (Ref,
            // Combo) pairs at evolve time, this path should either defer
            // (wrap in Thunk) or receive an explicit spec ruling.
            _ => {}
        }

        if let (Value::Atom(AtomKind::Tag(ta), _, _), Value::Atom(AtomKind::Tag(tb), _, _)) =
            (&a, &b)
        {
            if ta.trim_start_matches('#') == tb.trim_start_matches('#') {
                return a.clone();
            }
        }

        let a = self.force(a, ctx).collapse().clone();
        let b = self.force(b, ctx).collapse().clone();
        let id_a = a.content_hash();
        let id_b = b.content_hash();
        if id_a == id_b && !caid_is_incomplete(&a) && !caid_is_incomplete(&b) {
            return Self::prefer_caused_top(a, b);
        }

        // Type-marker × Range (E1) — acceptance repair: this early arm owns
        // ONLY the new value kind (Range). Every other kind DECLINES to the
        // established downstream arms (Union distribution, Combo×Combo subtype,
        // Combo×Atom at the atom arm) — hoisting marker×ANY here preempted
        // Union distribution: `(10|20) & @int` regressed to ⊥ (5b501e5 arm-order
        // bug class, 4th occurrence).
        if let Value::Combo(ac) = &a {
            if is_type_constraint_combo(ac) && matches!(&b, Value::Range { .. }) {
                if let Some(type_name) = get_type_constraint_name(ac) {
                    return type_constraint_meet(b.clone(), &type_name);
                }
            }
        }
        if let Value::Combo(bc) = &b {
            if is_type_constraint_combo(bc) && matches!(&a, Value::Range { .. }) {
                if let Some(type_name) = get_type_constraint_name(bc) {
                    return type_constraint_meet(a.clone(), &type_name);
                }
            }
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
            // Caused Top ≡ Top for lattice unit against non-Top (provenance
            // evaporates on consumption). Two top-like values: prefer
            // TopCaused so evolve can still store static-cycle provenance
            // (bare Top fields are dropped by unify_combo).
            (Value::Top | Value::TopCaused { .. }, Value::Union(_)) => {}
            (Value::Union(_), Value::Top | Value::TopCaused { .. }) => {}
            (Value::TopCaused { cause, members }, Value::Top)
            | (Value::Top, Value::TopCaused { cause, members }) => {
                // Caused preferred over bare (ruling C / Top family priority).
                return Value::TopCaused {
                    cause: cause.clone(),
                    members: members.clone(),
                };
            }
            (
                Value::TopCaused {
                    cause: c1,
                    members: m1,
                },
                Value::TopCaused {
                    cause: c2,
                    members: m2,
                },
            ) => {
                if c1 == "static_cycle" || c2 == "static_cycle" {
                    let mut m = m1.clone();
                    m.extend(m2.iter().cloned());
                    return crate::value::static_cycle_top(m);
                }
                return Value::TopCaused {
                    cause: c1.clone(),
                    members: m1.clone(),
                };
            }
            (Value::Top, Value::Top) => return Value::Top,
            (Value::Top | Value::TopCaused { .. }, _) => return b.bare_top_if_caused(),
            (_, Value::Top | Value::TopCaused { .. }) => return a.bare_top_if_caused(),
            (Value::Bottom(c), _) => return Value::Bottom(c.clone()),
            (_, Value::Bottom(c)) => return Value::Bottom(c.clone()),
            _ => {}
        }

        // Range membership / stepless intersection (SPEC_02 §3; range_eval work order).
        // Placed after Top/Bottom identity so those arms stay automatic.
        if let Some(r) = range_unify(&a, &b) {
            return r;
        }

        let nondet =
            a.effect().contains(EffectTag::NonDet) || b.effect().contains(EffectTag::NonDet);
        let cache_key = if id_a.digest <= id_b.digest {
            (id_a, id_b)
        } else {
            (id_b, id_a)
        };
        if !nondet {
            if let Ok(memo) = self.unify_memo.read() {
                if let Some(cached_res) = memo.get(&cache_key) {
                    return cached_res.clone();
                }
            }
        }
        let mut result = self.do_unify(a.clone(), b.clone(), ctx);
        let combined_effect = a.effect().union(b.effect());
        if let Value::Combo(ref mut cv) = result {
            cv.effect = cv.effect.union(combined_effect);
        }
        if !nondet && !matches!(result, Value::Bottom(_)) && !result.contains_blur() {
            if let Ok(mut memo) = self.unify_memo.write() {
                const UNIFY_MEMO_CAP: usize = 100_000;
                if memo.len() >= UNIFY_MEMO_CAP {
                    memo.clear();
                }
                memo.insert(cache_key, result.clone());
            }
        }
        result
    }

    fn do_unify(&self, a: Value, b: Value, ctx: &mut EvalContext) -> Value {
        match (a, b) {
            (Value::Atom(AtomKind::Tag(ta), ae, ra), Value::Atom(AtomKind::Tag(tb), be, rb))
                if ta.trim_start_matches('#') == tb.trim_start_matches('#') =>
            {
                Value::Atom(AtomKind::Tag(ta), ae.union(be), ra.or(rb))
            }
            (Value::Atom(ak, ae, ra), Value::Atom(bk, be, rb)) if ak == bk => {
                Value::Atom(ak, ae.union(be), ra.or(rb))
            }
            (Value::Atom(ak, ae, ra), Value::Combo(mut cv))
            | (Value::Combo(mut cv), Value::Atom(ak, ae, ra)) => {
                if is_type_constraint_combo(&cv) {
                    if let Some(type_name) = get_type_constraint_name(&cv) {
                        return type_constraint_meet(Value::Atom(ak, ae, ra), &type_name);
                    }
                }
                // G2-C: morphisms are not value-carriers — Atom × morphism = ⊥.
                // Non-morphism combos keep %val absorb (pin_nonmorphism_val_absorb).
                if Value::Combo(cv.clone()).is_morphism() {
                    return BottomCause::Conflict.into();
                }
                let val_key = "%val".to_string();
                let existing_val = cv.get_field(&val_key).cloned().unwrap_or(Value::Top);
                let merged_val = self.unify_internal(Value::Atom(ak, ae, ra), existing_val, ctx);
                if let Value::Bottom(c) = merged_val {
                    return Value::Bottom(c);
                }
                cv.insert_field(&val_key, merged_val);
                Value::Combo(cv)
            }
            (Value::Combo(ac), Value::Combo(bc)) => self.unify_combo(ac, bc, ctx),
            (Value::Union(mut branches), other) | (other, Value::Union(mut branches)) => {
                let max_branches = ctx.max_branches;
                // Preserve source / navigate encounter order among survivors
                // (F4: `({a:1}|7).a` → `1 | _`, partial-field → `_ | 2`).
                // Tropical sort only when over the collection budget so
                // max_branches truncation still prefers light branches.
                if branches.len() > max_branches * 2 {
                    branches.sort_by_key(|b| self.tropical_weight(b));
                }
                let mut results: Vec<Value> = Vec::new();
                // T3 (union_cull): keep culled BottomDetail so all-⊥ can
                // pass the primary member out verbatim (not normalize_union
                // "empty union after normalize" jargon). Sort/cap/nondistrib
                // logic unchanged — arm-order minefield, minimal diff.
                let mut culled: Vec<BottomDetail> = Vec::new();
                // Collect without early cap so structural dedupe can free
                // capacity first (SPEC_01 idempotence before max_branches).
                for branch in branches.into_iter().take(max_branches * 2) {
                    let r = self.unify_internal(branch, other.clone(), ctx);
                    match r {
                        Value::Bottom(detail) => {
                            if matches!(detail.cause, BottomCause::H1Split | BottomCause::H2Split) {
                                ctx.had_nondistrib_event = true;
                            }
                            culled.push(*detail);
                        }
                        other => {
                            results.push(other);
                        }
                    }
                }
                if results.is_empty() {
                    return primary_bottom_from_culled(culled);
                }
                let deduped = self.normalize_union_absorbing(results, ctx);
                match deduped {
                    Value::Union(mut bs) if bs.len() > max_branches => {
                        bs.truncate(max_branches);
                        Value::Union(bs)
                    }
                    other => other,
                }
            }
            // Blur unification rules
            // O46: merge blurs as a set of horizon records (not meet / not order).
            (Value::Blur(ba), Value::Blur(bb)) => Value::Blur(BlurDetail::merge_set(&ba, &bb)),
            (Value::Blur(_), b @ Value::Bottom(_)) => b,
            (a @ Value::Bottom(_), Value::Blur(_)) => a,
            (ba @ Value::Blur(_), Value::Top) => ba,
            (Value::Top, bb @ Value::Blur(_)) => bb,
            // O47 / SPEC_03 §90: absorb other into blur without rewriting the
            // snapshot (cause / CAID / horizon params / partial preserved).
            (ba @ Value::Blur(_), _other) => ba,
            (_other, bb @ Value::Blur(_)) => bb,
            (a, b) => Value::Bottom(Box::new(BottomDetail {
                cause: BottomCause::Conflict,
                path: None,
                message: Some(format!("Incompatible types: {:?} vs {:?}", a, b)),
                expected: Some(a.clone()),
                found: Some(b.clone()),
                involved: vec![a.content_hash(), b.content_hash()],
                ..Default::default()
            })),
        }
    }

    fn unify_combo(&self, a: ComboVal, b: ComboVal, ctx: &mut EvalContext) -> Value {
        // forward_spread: expand deferred sources before field lattice merge
        // so `{...(1,2)} |> {s: …}` sees numeric keys from the unbox.
        // Engine-internal expand may re-queue unresolved Top (wrong root);
        // residual pending must survive ComboVal::new below.
        let a = match self.expand_combo_pending(a, ctx) {
            Value::Combo(c) => c,
            other => return self.unify_internal(other, Value::Combo(b), ctx),
        };
        let b = match self.expand_combo_pending(b, ctx) {
            Value::Combo(c) => c,
            other => return self.unify_internal(Value::Combo(a), other, ctx),
        };
        let mut pending_spreads = a.pending_spreads.clone();
        pending_spreads.extend(b.pending_spreads.iter().cloned());
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
                if na == nb {
                    return Value::Combo(a);
                }
                let ca = TypeConstraint::from_name(&na);
                let cb = TypeConstraint::from_name(&nb);
                let subtype_check = self.check_subtype_relation(&ca, &cb);
                if subtype_check {
                    return Value::Combo(a);
                }
                let reverse_check = self.check_subtype_relation(&cb, &ca);
                if reverse_check {
                    return Value::Combo(b);
                }
            }
        }

        // Merge each axis by stored name. Flattening through field_keys()
        // + insert_field re-routes a data key `@t` onto the type axis (Q1).
        let merge_axis = |left: &IndexMap<String, Value>,
                          right: &IndexMap<String, Value>,
                          display: &str,
                          engine: &Self,
                          ctx: &mut EvalContext|
         -> Result<IndexMap<String, Value>, BottomDetail> {
            let mut out = IndexMap::new();
            // Content order, not the process-seeded hasher: left's insertion
            // order, then right-only keys in right's order. REAL_03 §6.7's
            // ban on hash-table iteration for serialization applies to
            // evaluation order the same way — fuel makes that order an address.
            let mut keys = IndexSet::new();
            keys.extend(left.keys());
            keys.extend(right.keys());
            for key in keys {
                let va = left.get(key).cloned().unwrap_or(Value::Top);
                let vb = right.get(key).cloned().unwrap_or(Value::Top);
                let va_is_top = va.is_top();
                let vb_is_top = vb.is_top();
                let shown = format!("{display}{key}");
                let is_no_constraint =
                    |v: &Value, is_top: bool, engine: &Self, ctx: &mut EvalContext| -> bool {
                        if is_top {
                            return true;
                        }
                        let f = engine.force(v.clone(), ctx).collapse().clone();
                        f.is_top()
                    };
                if a.closed
                    && !left.contains_key(key)
                    && !is_no_constraint(&vb, vb_is_top, engine, ctx)
                {
                    return Err(BottomDetail {
                        cause: BottomCause::MissingKey,
                        path: Some(shown.clone()),
                        message: Some(format!("Key '{}' missing in closed Cocoon", shown)),
                        expected: None,
                        found: Some(vb.clone()),
                        involved: vec![],
                        ..Default::default()
                    });
                }
                if b.closed
                    && !right.contains_key(key)
                    && !is_no_constraint(&va, va_is_top, engine, ctx)
                {
                    return Err(BottomDetail {
                        cause: BottomCause::MissingKey,
                        path: Some(shown.clone()),
                        message: Some(format!("Key '{}' missing in incoming closed Cocoon", shown)),
                        expected: Some(va.clone()),
                        found: None,
                        involved: vec![],
                        ..Default::default()
                    });
                }
                let merged = engine.unify_internal(va, vb, ctx);
                if let Value::Bottom(mut detail) = merged {
                    if va_is_top || vb_is_top {
                        out.insert(key.clone(), Value::Bottom(detail));
                        continue;
                    }
                    let cp = detail
                        .path
                        .as_ref()
                        .map(|p| format!("{shown}.{p}"))
                        .unwrap_or_else(|| shown.clone());
                    detail.path = Some(cp);
                    return Err(*detail);
                }
                if !matches!(&merged, Value::Top) {
                    out.insert(key.clone(), merged);
                }
            }
            Ok(out)
        };
        let data = match merge_axis(&a.data, &b.data, "", self, ctx) {
            Ok(m) => m,
            Err(d) => return Value::Bottom(Box::new(d)),
        };
        let types = match merge_axis(&a.types, &b.types, "@", self, ctx) {
            Ok(m) => m,
            Err(d) => return Value::Bottom(Box::new(d)),
        };
        let rules = match merge_axis(&a.rules, &b.rules, "/", self, ctx) {
            Ok(m) => m,
            Err(d) => return Value::Bottom(Box::new(d)),
        };
        let meta = match merge_axis(&a.meta, &b.meta, "%", self, ctx) {
            Ok(m) => m,
            Err(d) => return Value::Bottom(Box::new(d)),
        };
        let system = match merge_axis(&a.system, &b.system, "~%", self, ctx) {
            Ok(m) => m,
            Err(d) => return Value::Bottom(Box::new(d)),
        };
        let local = match merge_axis(&a.local, &b.local, "~", self, ctx) {
            Ok(m) => m,
            Err(d) => return Value::Bottom(Box::new(d)),
        };
        let mut out = ComboVal::default();
        out.closed = a.closed || b.closed;
        out.effect = a.effect.union(b.effect);
        out.relations = a
            .relations
            .iter()
            .chain(b.relations.iter())
            .cloned()
            .collect();
        out.data = data;
        out.types = types;
        out.rules = rules;
        out.meta = meta;
        out.system = system;
        out.local = local;
        // Preserve deferred spreads that expand could not resolve yet
        // (forward_spread: evolve/observe-entry unify vs system root).
        out.pending_spreads = pending_spreads;
        Value::Combo(out)
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

// ── Range lattice ops (closed-closed; anchors = ±∞) ─────────────────────────

/// Returns Some(result) only for the pairs this knife owns (Range×Range,
/// Atom×Range). Everything else — Union, Thunk, Ref, Combo — must DECLINE
/// (None) so the existing machinery runs: Union distribution (do_unify),
/// thunk forcing, combo mismatch. The original Conflict catch-all here
/// preempted Union distribution — `(1|7) & 1..3` measured Conflict instead
/// of 1 (SPEC_07 §4 疊加態平等演化) — same bug class as Atom(Top)&Union
/// (5b501e5): an early arm stealing operands from downstream normalization.
fn range_unify(a: &Value, b: &Value) -> Option<Value> {
    match (a, b) {
        (Value::Range { .. }, Value::Range { .. }) => Some(range_intersect(a, b)),
        (atom @ Value::Atom(_, _, _), Value::Range { start, end, step })
        | (Value::Range { start, end, step }, atom @ Value::Atom(_, _, _)) => {
            Some(range_membership(atom, start, end, step.as_deref()))
        }
        _ => None,
    }
}

fn range_membership(atom: &Value, start: &Value, end: &Value, step: Option<&Value>) -> Value {
    // x must be a numeric atom
    let Some(x) = numeric_f64(atom) else {
        return BottomCause::Conflict.into();
    };
    let Some(lo) = bound_key(start) else {
        return BottomCause::Conflict.into();
    };
    let Some(hi) = bound_key(end) else {
        return BottomCause::Conflict.into();
    };
    // closed-closed: lo ≤ x ≤ hi
    if bound_cmp(&lo, &bound_key_num(x))
        .map(|o| o == Ordering::Greater)
        .unwrap_or(true)
    {
        return BottomCause::Conflict.into();
    }
    if bound_cmp(&bound_key_num(x), &hi)
        .map(|o| o == Ordering::Greater)
        .unwrap_or(true)
    {
        return BottomCause::Conflict.into();
    }
    // step: (x - start) % step == 0; anchors as start use 0 as offset base for density
    if let Some(st) = step {
        if !on_step(atom, start, st) {
            return BottomCause::Conflict.into();
        }
    }
    atom.clone()
}

fn range_intersect(a: &Value, b: &Value) -> Value {
    let (
        Value::Range {
            start: s1,
            end: e1,
            step: st1,
        },
        Value::Range {
            start: s2,
            end: e2,
            step: st2,
        },
    ) = (a, b)
    else {
        return BottomCause::Conflict.into();
    };
    // Explicit step on either side → deferred (CRT); honest Conflict
    if st1.is_some() || st2.is_some() {
        return BottomCause::Conflict.into();
    }
    let Some(lo1) = bound_key(s1) else {
        return BottomCause::Conflict.into();
    };
    let Some(hi1) = bound_key(e1) else {
        return BottomCause::Conflict.into();
    };
    let Some(lo2) = bound_key(s2) else {
        return BottomCause::Conflict.into();
    };
    let Some(hi2) = bound_key(e2) else {
        return BottomCause::Conflict.into();
    };
    let lo = bound_max(&lo1, &lo2);
    let hi = bound_min(&hi1, &hi2);
    match bound_cmp(&lo, &hi) {
        Some(Ordering::Greater) => BottomCause::Conflict.into(), // empty
        Some(Ordering::Equal) => {
            // singleton collapses to the atom (prefer concrete Int if both match)
            bound_to_value(&lo)
        }
        Some(Ordering::Less) => Value::Range {
            start: Box::new(bound_to_value(&lo)),
            end: Box::new(bound_to_value(&hi)),
            step: None,
        },
        None => BottomCause::Conflict.into(),
    }
}

/// Bound key: (-1, _) = −∞ (TagStart), (1, _) = +∞ (TagEnd), (0, f) = number.
#[derive(Clone, Copy)]
struct BoundKey(i8, f64);

fn bound_key(v: &Value) -> Option<BoundKey> {
    match v {
        Value::Atom(AtomKind::TagStart, _, _) => Some(BoundKey(-1, 0.0)),
        Value::Atom(AtomKind::TagEnd, _, _) => Some(BoundKey(1, 0.0)),
        Value::Atom(AtomKind::Int(i), _, _) => Some(BoundKey(0, i.to_f64()?)),
        Value::Atom(AtomKind::Float(f), _, _) => Some(BoundKey(0, *f)),
        _ => None,
    }
}

fn bound_key_num(x: f64) -> BoundKey {
    BoundKey(0, x)
}

fn bound_cmp(a: &BoundKey, b: &BoundKey) -> Option<Ordering> {
    match (a.0, b.0) {
        (-1, -1) | (1, 1) => Some(Ordering::Equal),
        (-1, _) => Some(Ordering::Less),
        (_, -1) => Some(Ordering::Greater),
        (1, _) => Some(Ordering::Greater),
        (_, 1) => Some(Ordering::Less),
        (0, 0) => a.1.partial_cmp(&b.1),
        _ => None,
    }
}

fn bound_max(a: &BoundKey, b: &BoundKey) -> BoundKey {
    match bound_cmp(a, b) {
        Some(Ordering::Less) => *b,
        _ => *a,
    }
}

fn bound_min(a: &BoundKey, b: &BoundKey) -> BoundKey {
    match bound_cmp(a, b) {
        Some(Ordering::Greater) => *b,
        _ => *a,
    }
}

fn bound_to_value(k: &BoundKey) -> Value {
    match k.0 {
        -1 => Value::Atom(AtomKind::TagStart, EffectTag::Pure, None),
        1 => Value::Atom(AtomKind::TagEnd, EffectTag::Pure, None),
        _ => {
            // Prefer Int when the float is integral
            if k.1.fract() == 0.0 && k.1.abs() < (i64::MAX as f64) {
                Value::Atom(
                    AtomKind::Int(BigInt::from(k.1 as i64)),
                    EffectTag::Pure,
                    None,
                )
            } else {
                Value::Atom(AtomKind::Float(k.1), EffectTag::Pure, None)
            }
        }
    }
}

fn numeric_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Atom(AtomKind::Int(i), _, _) => i.to_f64(),
        Value::Atom(AtomKind::Float(f), _, _) => Some(*f),
        _ => None,
    }
}

fn on_step(x: &Value, start: &Value, step: &Value) -> bool {
    // Integer-preferred path for exactness
    if let (
        Value::Atom(AtomKind::Int(xv), _, _),
        Value::Atom(AtomKind::Int(sv), _, _),
        Value::Atom(AtomKind::Int(st), _, _),
    ) = (x, start, step)
    {
        if st.is_zero() {
            return false;
        }
        let diff = xv - sv;
        return (&diff % st).is_zero();
    }
    // Anchor start: treat as dense base 0 for float step? Prefer numeric start only.
    let (Some(xf), Some(sf), Some(stf)) = (numeric_f64(x), numeric_f64(start), numeric_f64(step))
    else {
        // start is anchor → only accept if step is zero-offset from a numeric reading;
        // for TagStart/-∞ with step, membership reduces to "any number on the ray"
        // with no discrete grid — treat as dense (always on-step if in bounds).
        if matches!(
            start,
            Value::Atom(AtomKind::TagStart | AtomKind::TagEnd, _, _)
        ) {
            return true;
        }
        return false;
    };
    if stf == 0.0 {
        return false;
    }
    let q = (xf - sf) / stf;
    (q - q.round()).abs() < 1e-9
}
