use crate::observation::{handle_resource_exhausted, needs_partial_body};
use crate::type_constraint::{get_type_constraint_name, is_type_constraint_combo, TypeConstraint};
use crate::value::{
    normalize_union, primary_bottom_from_culled, BottomCause, BottomDetail, ComboVal, EffectTag,
    RelOp as ValRelOp, ValRelation, Value,
};
use crate::{mbu, CmpOp, EvalContext, Ouroboros};
use indexmap::IndexMap;
use nlang_parser::ast::{AtomKind, Expr, ExprKind, FieldKey, PathAnchor, Prefix, UnaryOp};
use num_bigint::BigInt;
use num_traits::{ToPrimitive, Zero};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Copy)]
enum MathOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

/// Names bound by an arrow parameter. Only bare one-segment paths bind a
/// lexical name; all other parameter shapes are dispatch patterns.
fn morphism_parameter_names(param: &Expr) -> HashSet<String> {
    let mut names = HashSet::new();
    match &param.kind {
        ExprKind::Path(p) if p.anchor == PathAnchor::Bare && p.segments.len() == 1 => {
            names.insert(p.segments[0].trim().to_string());
        }
        ExprKind::Tuple(items) => {
            for item in items {
                names.extend(morphism_parameter_names(item));
            }
        }
        _ => {}
    }
    names
}

/// Lexically free bare names in an expression. Combo fields form a
/// simultaneous local scope, which preserves both self- and mutual recursion.
fn free_bare_names(expr: &Expr, bound: &HashSet<String>) -> HashSet<String> {
    fn walk(expr: &Expr, bound: &HashSet<String>, out: &mut HashSet<String>) {
        match &expr.kind {
            ExprKind::Path(p) if matches!(p.anchor, PathAnchor::Bare | PathAnchor::Parent(_)) => {
                if let Some(name) = p.segments.first() {
                    let name = name.trim();
                    if !bound.contains(name) {
                        out.insert(name.to_string());
                    }
                }
            }
            ExprKind::Morphism { param, body } => {
                let mut nested = bound.clone();
                nested.extend(morphism_parameter_names(param));
                walk(body, &nested, out);
            }
            ExprKind::Combo { fields, .. } => {
                let mut nested = bound.clone();
                for field in fields {
                    if let FieldKey::Named { name, .. } = &field.key {
                        nested.insert(name.trim().to_string());
                    }
                }
                for field in fields {
                    walk(&field.value, &nested, out);
                }
            }
            ExprKind::Apply(a, b) | ExprKind::Pipe(a, b) | ExprKind::Meet(a, b)
            | ExprKind::Join(a, b) | ExprKind::Diff(a, b) | ExprKind::Add(a, b)
            | ExprKind::Sub(a, b) | ExprKind::Mul(a, b) | ExprKind::Div(a, b)
            | ExprKind::Rem(a, b) | ExprKind::Eq(a, b) | ExprKind::Ne(a, b)
            | ExprKind::Lt(a, b) | ExprKind::Gt(a, b) | ExprKind::Lte(a, b)
            | ExprKind::Gte(a, b) | ExprKind::TypeAnnotation(a, b)
            | ExprKind::Lens(a, b) | ExprKind::LatticeEq(a, b) | ExprKind::Probe(a, b) => {
                walk(a, bound, out); walk(b, bound, out);
            }
            ExprKind::Ternary { cond, then_branch, else_branch } => {
                walk(cond, bound, out); walk(then_branch, bound, out); walk(else_branch, bound, out);
            }
            ExprKind::Unary { expr, .. } | ExprKind::AnonSet(expr) | ExprKind::Spread(expr)
            | ExprKind::Structural(expr) | ExprKind::Complement(expr) => walk(expr, bound, out),
            ExprKind::List(items) | ExprKind::Tuple(items) => for item in items { walk(item, bound, out); },
            ExprKind::Interpolated(parts) => for part in parts {
                if let nlang_parser::ast::StringPart::Interpolated(expr) = part { walk(expr, bound, out); }
            },
            ExprKind::Range { start, end, step } => {
                walk(start, bound, out); walk(end, bound, out);
                if let Some(step) = step { walk(step, bound, out); }
            }
            _ => {}
        }
    }
    let mut out = HashSet::new();
    walk(expr, bound, &mut out);
    out
}

fn captures(free: &HashSet<String>, prefix: &str, name: &str) -> bool {
    free.contains(name) || free.contains(&format!("{prefix}{name}"))
}

fn value_dependencies(value: &Value) -> HashSet<String> {
    match value {
        Value::Thunk { expr, .. } | Value::Code(expr) => free_bare_names(expr, &HashSet::new()),
        _ => HashSet::new(),
    }
}

/// Follow value dependencies to a fixed point before taking the frame slice:
/// a captured `e` whose recipe reads `d` is not usable unless `d` travels too.
fn close_free_dependencies(scopes: &[std::sync::Arc<ComboVal>], free: &mut HashSet<String>) {
    loop {
        let before = free.len();
        for scope in scopes {
            for (name, value) in &scope.data {
                if captures(free, "", name) { free.extend(value_dependencies(value)); }
            }
            for (name, value) in &scope.types {
                if captures(free, "@", name) { free.extend(value_dependencies(value)); }
            }
            for (name, value) in &scope.rules {
                if captures(free, "/", name) { free.extend(value_dependencies(value)); }
            }
            for (name, value) in &scope.meta {
                if captures(free, "%", name) { free.extend(value_dependencies(value)); }
            }
            for (name, value) in &scope.system {
                if captures(free, "~%", name) { free.extend(value_dependencies(value)); }
            }
            for (name, value) in &scope.local {
                if captures(free, "~", name) { free.extend(value_dependencies(value)); }
            }
        }
        if free.len() == before { break; }
    }
}

fn capture_free_fields(scope: &ComboVal, free: &HashSet<String>) -> ComboVal {
    let mut captured = ComboVal::default();
    for (name, value) in &scope.data {
        if captures(free, "", name) {
            captured.data.insert(name.clone(), value.clone());
        }
    }
    for (target, source, prefix) in [
        (&mut captured.types, &scope.types, "@"), (&mut captured.rules, &scope.rules, "/"),
        (&mut captured.meta, &scope.meta, "%"), (&mut captured.system, &scope.system, "~%"),
        (&mut captured.local, &scope.local, "~"),
    ] {
        for (name, value) in source {
            if captures(free, prefix, name) { target.insert(name.clone(), value.clone()); }
        }
    }
    captured.closed = scope.closed;
    captured.effect = scope.effect;
    captured.relations = scope.relations.clone();
    captured.masa_ref = scope.masa_ref.clone();
    captured
}

/// G1 #12 / G6: value-context operand (math, atomic `==`/`!=`).
/// Pure wrappers collapse; hybrid combos peel `%val` (recursively);
/// non-collapsible combos (no `%val`) → Err (caller → ⊥ #conflict).
/// Does not mutate `collapse()` itself (shared with Probe / set family).
fn value_context_operand(v: &Value) -> Result<Value, ()> {
    let c = v.collapse();
    match c {
        Value::Combo(cv) => {
            // Structural-view mark holds the full node in %node (not %val —
            // pure-wrapper collapse would erase the mark during lattice
            // unify). Peel the mark then continue so math still reads the
            // hybrid's %val.
            if crate::value::is_structural_view(cv) {
                if let Some(inner) = crate::value::structural_node(cv) {
                    return value_context_operand(inner);
                }
            }
            if let Some(inner) = cv.get_field("%val") {
                value_context_operand(inner)
            } else {
                Err(())
            }
        }
        other => Ok(other.clone()),
    }
}

/// SPEC_03 §3.1 / SPEC_04 §3.1 #5: spread target is an *insider* when it
/// appears in the current evaluation scope chain (defining-combo frames
/// from `seal_defining_scope`). Full `PartialEq` first; if seal left a
/// pre-inject clone in the thunk closure while the live target has sealed
/// field thunks, fall back to same-axes-keys + equal `local` (privacy surface).
fn spread_target_is_insider(target: &ComboVal, ctx: &EvalContext) -> bool {
    ctx.scopes.iter().any(|frame| {
        let frame = frame.as_ref();
        if frame == target {
            return true;
        }
        frame.local == target.local
            && frame.closed == target.closed
            && frame.data.keys().eq(target.data.keys())
            && frame.types.keys().eq(target.types.keys())
            && frame.rules.keys().eq(target.rules.keys())
            && frame.meta.keys().eq(target.meta.keys())
            && frame.system.keys().eq(target.system.keys())
    })
}

/// C4 (SPEC_03 §3.1): direct-name circular spread — path first segment is a
/// coordinate under construction (`ctx.computing`, filled by evolve / force_coord).
/// Field spreads parse as `ExprKind::Spread(inner)` (value of `...` key); also
/// accept bare Path for robustness.
fn spread_path_is_under_construction(expr: &Expr, ctx: &EvalContext) -> bool {
    let core = match &expr.kind {
        ExprKind::Spread(inner) => inner.as_ref(),
        _ => expr,
    };
    match &core.kind {
        ExprKind::Path(p) if p.anchor == PathAnchor::Bare && !p.segments.is_empty() => {
            let first = p.segments[0].trim();
            ctx.computing.contains(first)
        }
        _ => false,
    }
}

/// C4 ancestor form at construction: `a: { b: { ...a } }` — nested combo whose
/// *only* fields are spreads of a name under construction. Distinct from the
/// insider pattern `c2: { ...p, rd: ~s }` which has additional fields and must
/// remain legal (spread-privacy pin).
fn expr_is_pure_circular_spread(expr: &Expr, ctx: &EvalContext) -> bool {
    if ctx.computing.is_empty() {
        return false;
    }
    match &expr.kind {
        ExprKind::Combo { fields, .. } if !fields.is_empty() => fields.iter().all(|f| {
            matches!(&f.key, FieldKey::Quoted(name) if name == "...")
                && spread_path_is_under_construction(&f.value, ctx)
        }),
        _ => false,
    }
}

/// Atom-spread shell: `{%val: v}` plus a data-axis `_ : _` so evolve unify
/// does not peel the pure-wrapper (same anti-peel as ⊥ %cause cocoon).
/// Collapsed observation still projects via `%val` when appropriate.
fn atom_spread_shell(atom: Value) -> Value {
    let mut fields = IndexMap::new();
    fields.insert("%val".to_string(), atom);
    fields.insert("_".to_string(), Value::Top);
    Value::Combo(ComboVal::new(
        fields,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

/// G6: mark a value as structural-view (`<<non-path>>`) so observation
/// display preserves the full node. Payload is `%node`, **not** `%val`:
/// pure wrappers (`%val` + %-meta only) are peeled by `collapse()` during
/// lattice unify, which would erase the structural signal before observe.
/// SPEC_08 §4.3 Model A: true iff combo carries an explicit `%effect: #pure`
/// meta declaration (SYNTAX_08 writable meta purity assertion).
fn declared_pure_meta(cv: &ComboVal, oo: &Ouroboros, ctx: &mut EvalContext) -> bool {
    let Some(raw) = cv.get_field("%effect").cloned() else {
        return false;
    };
    let v = oo.force(raw, ctx);
    match v.collapse() {
        Value::Atom(AtomKind::Tag(t), _, _) => t.trim_start_matches('#') == "pure",
        _ => false,
    }
}

fn mark_structural_view(inner: Value) -> Value {
    let mut fields = IndexMap::new();
    fields.insert(
        "%structural".to_string(),
        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
    );
    fields.insert("%node".to_string(), inner);
    Value::Combo(ComboVal::new(
        fields,
        true,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

impl Ouroboros {
    /// SPEC_03 §3.1 / §1.1: collision-aware field write — key already present
    /// → force both sides and `unify_internal` (intersect `&`); absent → insert
    /// as-is (preserves Thunk laziness on non-colliding keys).
    fn merge_field_into(
        &self,
        map: &mut IndexMap<String, Value>,
        key: String,
        incoming: Value,
        ctx: &mut EvalContext,
    ) {
        if let Some(existing) = map.get(&key) {
            let merged = self.unify_internal(existing.clone(), incoming, ctx);
            map.insert(key, merged);
        } else {
            map.insert(key, incoming);
        }
    }

    /// SYNTAX_06 §2.1: A <= B ⟺ (A & B) = A after solidify (G1 PartialEq).
    /// Atom shortcuts (numeric-by-value, singleton PartialEq) + general
    /// non-atom wiring (W3). Sets `union_absorb_fence` around the meet so
    /// absorption normalization cannot re-enter (union_absorption fixpoint).
    pub(crate) fn subset_lte(&self, a: &Value, b: &Value, ctx: &mut EvalContext) -> bool {
        self.subset_lte_inner(a, b, ctx, true)
    }

    /// Absorption uses `solidify=false`: raw-branch compare only — never
    /// force_recursive (recursive types like `@Tree | ()` would diverge).
    fn subset_lte_inner(
        &self,
        a: &Value,
        b: &Value,
        ctx: &mut EvalContext,
        solidify: bool,
    ) -> bool {
        // Numbers by value: 3 and 3.0 are one singleton.
        match (a, b) {
            (Value::Atom(AtomKind::Int(x), _, _), Value::Atom(AtomKind::Int(y), _, _)) => {
                return x == y;
            }
            (Value::Atom(AtomKind::Float(x), _, _), Value::Atom(AtomKind::Float(y), _, _)) => {
                return x == y;
            }
            (Value::Atom(AtomKind::Int(x), _, _), Value::Atom(AtomKind::Float(y), _, _)) => {
                return x.to_f64() == Some(*y);
            }
            (Value::Atom(AtomKind::Float(x), _, _), Value::Atom(AtomKind::Int(y), _, _)) => {
                return Some(*x) == y.to_f64();
            }
            _ => {}
        }
        if matches!(a, Value::Atom(..)) && matches!(b, Value::Atom(..)) {
            return a == b;
        }
        // ACCEPTANCE REPAIR (2026-07-20): the absorption probe must not
        // leave evaluation state behind. Its meet forces thunks, and the
        // residue (taint / cycle chain / in-flight marks) made a later
        // static-cycle re-entry classify as transform → ⊥ #divergent →
        // culled: `w: {q: p.v | 9}` read `9` in-harness but `_` via CLI
        // — a force-order-dependent verdict (the disease the taint-scope
        // arc cured; SPEC_00 Invariant 1 convergence determinism).
        // Absorption is a QUERY: probe on an isolated context, charge the
        // fuel back so the horizon stays honest, discard the rest.
        // The solidify path (cmp家族, W3-shipped) keeps its own ctx.
        let meet = if solidify {
            let saved_fence = ctx.union_absorb_fence;
            ctx.union_absorb_fence = true;
            let m = self.unify_internal(a.clone(), b.clone(), ctx);
            ctx.union_absorb_fence = saved_fence;
            m
        } else {
            let mut probe = ctx.clone();
            probe.union_absorb_fence = true;
            // The probe must not publish through the engine-global force
            // memo either: a value forced under probe conditions would be
            // read back by the real evaluation (that is how the ⊥ verdict
            // reached the union and culled the static-cycle branch).
            probe.memo_enabled = false;
            let m = self.unify_internal(a.clone(), b.clone(), &mut probe);
            ctx.fuel = probe.fuel;
            m
        };
        match meet {
            Value::Bottom(_) => false,
            Value::Blur(_) => false,
            m => {
                if solidify {
                    let m = self.force_recursive(m, ctx);
                    let a_s = self.force_recursive(a.clone(), ctx);
                    if matches!(m, Value::Blur(_)) || matches!(a_s, Value::Blur(_)) {
                        return false;
                    }
                    m == a_s
                } else {
                    // Raw-branch absorption: collapse only, no force.
                    m.collapse() == a.collapse()
                }
            }
        }
    }

    /// SPEC_01 §2.4.2 (+ ruling C): union normalize with absorption.
    /// Cull ⊥ → Top-family / diagnostic peel (BEFORE PartialEq dedupe) →
    /// absorb among non-diagnostics → reattach diagnostics.
    /// Diagnostic members (blur + caused Top): never absorb, never absorbed;
    /// a value containing a diagnostic at any depth may not act as absorber.
    /// Bare Top alone collapses non-diagnostic siblings. Fence: nested
    /// normalize stays pure dedupe.
    pub(crate) fn normalize_union_absorbing(
        &self,
        branches: impl IntoIterator<Item = Value>,
        ctx: &mut EvalContext,
    ) -> Value {
        if ctx.union_absorb_fence {
            return normalize_union(branches);
        }

        fn push_flat(v: Value, out: &mut Vec<Value>) {
            match v {
                Value::Union(inner) => {
                    for b in inner {
                        push_flat(b, out);
                    }
                }
                other => out.push(other),
            }
        }
        let mut flat = Vec::new();
        for b in branches {
            push_flat(b, &mut flat);
        }
        // Flattening may expose a union received through a path or a thunk,
        // whose cardinality is not bounded by this union expression's AST.
        // Bill each projected branch rather than the implementation's nested
        // representation depth.
        if let Err(e) = ctx.check_resources(flat.len() as u64 * mbu::SUBSPACE_EXPANSION) {
            let partial = if needs_partial_body(&e, ctx.strategy) {
                Some(Value::Union(flat))
            } else {
                None
            };
            return handle_resource_exhausted(e, ctx.strategy, &*ctx, partial, EffectTag::Pure);
        }
        // Cull ⊥ (G4).
        let mut culled: Vec<BottomDetail> = Vec::new();
        let mut non_bot: Vec<Value> = Vec::new();
        for b in flat {
            match b {
                Value::Bottom(d) => culled.push(*d),
                Value::Atom(AtomKind::Bottom, _, _) => culled.push(BottomDetail {
                    cause: BottomCause::Conflict,
                    path: None,
                    message: None,
                    expected: None,
                    found: None,
                    involved: vec![],
                    obstruction_degree: None,
                    holonomy: None,
                }),
                other => non_bot.push(other),
            }
        }
        if non_bot.is_empty() {
            return primary_bottom_from_culled(culled);
        }

        // Shallow-peel Thunks once so diagnostic Tops surface before
        // classification. Without this, `Thunk(p.v) | 9` lets the thunk
        // absorb 9 via unify (meet forces the cycle to TopCaused ≡ unit).
        // Isolated probe ctx: no memo pollution / taint write-back.
        let peeled: Vec<Value> = non_bot
            .into_iter()
            .map(|v| {
                let mut probe = ctx.clone();
                probe.union_absorb_fence = true;
                probe.memo_enabled = false;
                let mut v = v;
                let mut n = 0u32;
                while matches!(&v, Value::Thunk { .. }) && n < 8 {
                    v = self.force(v, &mut probe);
                    n += 1;
                }
                ctx.fuel = probe.fuel;
                v
            })
            .collect();

        // BEFORE PartialEq dedupe: peel diagnostics + bare Top so caused
        // and bare are never merged by first-wins equality (ruling C).
        let mut diagnostics: Vec<Value> = Vec::new();
        let mut bare_top = false;
        let mut others: Vec<Value> = Vec::new();
        for b in peeled {
            match b {
                Value::Blur(_) | Value::TopCaused { .. } => diagnostics.push(b),
                Value::Top => bare_top = true,
                other => others.push(other),
            }
        }

        // Dedupe non-diagnostic others (G1 PartialEq).
        let mut unique: Vec<Value> = Vec::new();
        for b in others {
            if !unique.iter().any(|u| u == &b) {
                unique.push(b);
            }
        }
        // Dedupe diagnostics left-first (PartialEq collapses equal Tops;
        // first-wins preserves leftmost cause when causes PartialEq-match).
        let mut diag_unique: Vec<Value> = Vec::new();
        for d in diagnostics {
            if !diag_unique.iter().any(|u| u == &d) {
                diag_unique.push(d);
            }
        }

        // Bare Top absorbs all non-diagnostic siblings (user said "anything").
        let rest = if bare_top {
            vec![Value::Top]
        } else {
            // Absorb among others: drop i if some j with i <= j AND j is a
            // lawful absorber (no diagnostic at any depth; residual Thunk
            // also disqualified — unknown content).
            const DIAG_DEPTH: u32 = 8;
            let n = unique.len();
            let mut keep = vec![true; n];
            let can_absorb: Vec<bool> = unique
                .iter()
                .map(|a| {
                    !matches!(a, Value::Thunk { .. })
                        && !crate::value::contains_diagnostic(a, DIAG_DEPTH)
                })
                .collect();
            for i in 0..n {
                if !keep[i] {
                    continue;
                }
                for j in 0..n {
                    if i == j || !keep[j] || !can_absorb[j] {
                        continue;
                    }
                    if self.subset_lte_inner(&unique[i], &unique[j], ctx, false) {
                        keep[i] = false;
                        break;
                    }
                }
            }
            unique
                .into_iter()
                .zip(keep)
                .filter_map(|(b, k)| if k { Some(b) } else { None })
                .collect()
        };

        let mut out = rest;
        out.extend(diag_unique);
        match out.len() {
            0 => primary_bottom_from_culled(culled),
            1 => out.into_iter().next().unwrap(),
            _ => Value::Union(out),
        }
    }

    /// SPEC_03 §3.1 timing (forward_spread): expand deferred spread sources
    /// at observation/force convergence. Applies the same laws as the former
    /// eager construction arm (intersect merge, ⊥ collapse, blur absorb,
    /// Top no-op, private exclusion, C4 under-construction guard).
    ///
    /// Engine-internal unify (`memo_enabled == false`) runs against the
    /// pristine system root — open-miss Top there is *not* "never-defined",
    /// it is "binding not on this root". Re-queue those sources so evolve/
    /// observe-entry unify cannot silently consume forward spreads.
    /// Observation contexts (memo_enabled) treat Top as true no-op.
    pub(crate) fn expand_combo_pending(&self, mut c: ComboVal, ctx: &mut EvalContext) -> Value {
        if c.pending_spreads.is_empty() {
            return Value::Combo(c);
        }
        let spreads = std::mem::take(&mut c.pending_spreads);
        let mut blur_absorb: Option<crate::value::BlurDetail> = None;
        for src in spreads {
            // C4 re-check at expansion (cycle via alias detour).
            if let Value::Thunk { ref expr, .. } = src {
                if spread_path_is_under_construction(expr, ctx) {
                    return BottomCause::Divergent.into();
                }
            }
            // Keep original for re-queue (force consumes the Thunk).
            let src_for_requeue = src.clone();
            // Spread expansion is not a pure-ref hop: if forcing the source
            // re-enters the combo under construction (alias detour
            // `al: a; a: {...al}`), classify as #divergent not static Top.
            let saved_taint = ctx.chain_transform_taint;
            ctx.chain_transform_taint = true;
            let val = self.force(src, ctx);
            ctx.chain_transform_taint = saved_taint;
            if let Value::Bottom(d) = val {
                return Value::Bottom(d);
            }
            if matches!(val, Value::Atom(AtomKind::Bottom, _, _)) {
                return BottomCause::Conflict.into();
            }
            if let Value::Blur(bd) = val {
                c.effect = c.effect.union(bd.effect);
                blur_absorb = Some(match blur_absorb.take() {
                    None => bd,
                    Some(prev) => {
                        match self.unify_internal(Value::Blur(prev), Value::Blur(bd), ctx) {
                            Value::Blur(merged) => merged,
                            Value::Bottom(d) => return Value::Bottom(d),
                            other => {
                                if let Value::Blur(m) = other {
                                    m
                                } else {
                                    return other;
                                }
                            }
                        }
                    }
                });
                continue;
            }
            c.effect = c.effect.union(val.effect());
            match val {
                Value::Combo(ref cv) => {
                    for (k, v) in cv.fields() {
                        if let Some(existing) = c.get_field(&k).cloned() {
                            let merged = self.unify_internal(existing, v, ctx);
                            c.insert_field(&k, merged);
                        } else {
                            c.insert_field(&k, v);
                        }
                    }
                    if spread_target_is_insider(cv, ctx) {
                        for (k, v) in cv.local_fields() {
                            let bare = k.trim().trim_start_matches('~').to_string();
                            if let Some(existing) = c.local.get(&bare).cloned() {
                                let merged = self.unify_internal(existing, v, ctx);
                                c.local.insert(bare, merged);
                            } else {
                                c.local.insert(bare, v);
                            }
                        }
                    }
                    if !c.closed {
                        c.effect = c.effect.union(cv.effect);
                    }
                }
                Value::Top | Value::TopCaused { .. } => {
                    if !ctx.memo_enabled || (ctx.in_evolve && c.closed) {
                        // Engine-internal (system root): binding may not be on
                        // this root — requeue. Evolve-phase CLOSED combo: the
                        // cocoon construction force (GUIDE_03 §11.5) runs
                        // before later fields evolve — requeue so the sealed
                        // key set can still gain the forward source at
                        // observation. Open combos during evolve consume as
                        // no-op (baseline-preserving: evolve-time computed
                        // expressions over pending combos = pre-existing
                        // eager-computation debt, ledgered).
                        c.pending_spreads.push(src_for_requeue);
                    }
                    // Observation: Top no-op (never-defined / open hole).
                }
                Value::Atom(ak, ae, rank) => {
                    let shell = atom_spread_shell(Value::Atom(ak, ae, rank));
                    if let Value::Combo(cv) = shell {
                        for (k, v) in cv.fields() {
                            if let Some(existing) = c.get_field(&k).cloned() {
                                let merged = self.unify_internal(existing, v, ctx);
                                c.insert_field(&k, merged);
                            } else {
                                c.insert_field(&k, v);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(bd) = blur_absorb {
            return Value::Blur(bd);
        }
        Value::Combo(c)
    }

    /// E3: if one operand is AST `!(e)` and eval(e) is Range, rewrite meet to
    /// membership negation: x if x∉range else ⊥. Mirror both orders.
    /// Does NOT handle standalone `!(range)` (that stays orthocomplement ⊥).
    fn try_meet_not_range(&self, a: &Expr, b: &Expr, ctx: &mut EvalContext) -> Option<Value> {
        let not_range = |e: &Expr, ctx: &mut EvalContext| -> Option<Value> {
            match &e.kind {
                ExprKind::Unary {
                    op: UnaryOp::Not,
                    expr: inner,
                }
                | ExprKind::Complement(inner) => {
                    let rv = self.eval(inner, ctx);
                    if matches!(rv, Value::Range { .. }) {
                        Some(rv)
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };

        if let Some(range) = not_range(a, ctx) {
            let x = self.eval(b, ctx);
            return Some(self.membership_negation(x, range, ctx));
        }
        if let Some(range) = not_range(b, ctx) {
            let x = self.eval(a, ctx);
            return Some(self.membership_negation(x, range, ctx));
        }
        None
    }

    /// x ∉ range → x; x ∈ range → ⊥. Distributes over Union.
    fn membership_negation(&self, x: Value, range: Value, ctx: &mut EvalContext) -> Value {
        if let Value::Union(branches) = x {
            let mut out = Vec::new();
            for b in branches {
                let r = self.membership_negation(b, range.clone(), ctx);
                if !matches!(r, Value::Bottom(_)) {
                    out.push(r);
                }
            }
            return self.normalize_union_absorbing(out, ctx);
        }
        let meet = self.unify_internal(x.clone(), range, ctx);
        if matches!(meet, Value::Bottom(_)) {
            x
        } else {
            BottomCause::Conflict.into()
        }
    }

    pub fn predict_effect(&self, expr: &Expr, ctx: &EvalContext) -> EffectTag {
        match &expr.kind {
            ExprKind::Atom(_) => EffectTag::Pure,
            ExprKind::Path(path) => {
                // SPEC_09 §4 effect table: read the *stored* effect on the
                // resolved value (modules/morphisms carry their own tags).
                // Do NOT blanket-tag `~%…` as IO — ~%Math is pure; ~%Env
                // morphisms are genuinely IO. Multi-segment walk required
                // so `~%Math.abs` / `~%Env.get` hit the leaf morphism tag,
                // not only the module shell (often Pure).
                if path.segments.is_empty() {
                    return EffectTag::Pure;
                }
                let first = path.segments[0].trim();
                let mut found: Option<Value> = None;
                for scope in ctx.scopes.iter().rev() {
                    if let Some(v) = scope.get_field(first) {
                        found = Some(v.clone());
                        break;
                    }
                    let ln = format!("/{}", first);
                    if let Some(v) = scope.get_field(&ln) {
                        found = Some(v.clone());
                        break;
                    }
                }
                if found.is_none() {
                    if let Some(v) = ctx.root.get_field(first) {
                        found = Some(v.clone());
                    } else {
                        let ln = format!("/{}", first);
                        if let Some(v) = ctx.root.get_field(&ln) {
                            found = Some(v.clone());
                        }
                    }
                }
                if found.is_none() {
                    if let Some(v) = ctx.standard_root.get_field(first) {
                        found = Some(v.clone());
                    } else {
                        let ln = format!("/{}", first);
                        if let Some(v) = ctx.standard_root.get_field(&ln) {
                            found = Some(v.clone());
                        }
                    }
                }
                if let Some(ref s) = ctx.staged {
                    if let Some(v) = s.get_field(first) {
                        found = Some(v.clone());
                    } else {
                        let ln = format!("/{}", first);
                        if let Some(v) = s.get_field(&ln) {
                            found = Some(v.clone());
                        }
                    }
                }
                let Some(mut cur) = found else {
                    return EffectTag::Pure;
                };
                for seg in path.segments.iter().skip(1) {
                    let seg = seg.trim();
                    match &cur {
                        Value::Combo(c) => {
                            if let Some(v) = c
                                .get_field(seg)
                                .or_else(|| c.get_field(&format!("/{}", seg)))
                                .or_else(|| c.get_field(&format!("@{}", seg)))
                            {
                                cur = v.clone();
                            } else {
                                break;
                            }
                        }
                        _ => break,
                    }
                }
                cur.effect()
            }
            ExprKind::Apply(f, arg) => {
                // Acceptance repair (arc 4): `~%Effect./runPure X` DISCHARGES
                // X's active effect — privileged → pure value, unprivileged →
                // ⊥ (itself pure-effect). Either way the arg's io must NOT
                // propagate into a container's predicted contagion, else
                // arc-3's static guard spuriously ⊥s a legitimate
                // `{ %effect: #pure, v: runPure(io) }`. Canonical `~%Effect`
                // callee detected syntactically; an aliased-morphism callee
                // stays conservative (ledgered — predict may over-report).
                if let ExprKind::Path(p) = &f.kind {
                    let last = p.segments.last().map(|s| s.trim_start_matches('/'));
                    let via_effect = p
                        .segments
                        .iter()
                        .any(|s| s.trim_start_matches('/') == "~%Effect");
                    if last == Some("runPure") && via_effect {
                        return EffectTag::Pure;
                    }
                }
                self.predict_effect(f, ctx)
                    .union(self.predict_effect(arg, ctx))
            }
            ExprKind::Pipe(l, r) => self
                .predict_effect(l, ctx)
                .union(self.predict_effect(r, ctx)),
            ExprKind::Combo { fields, closed, .. } => {
                // §4.2.1 Shield: a cocoon literal predicts its OWN tag
                // (#pure) — contagion stops at the wall. Joining through
                // it leaked the interior into the parent's %effect
                // (literal/alias spelling divergence; acceptance repair).
                if *closed {
                    return EffectTag::Pure;
                }
                let mut e = EffectTag::Pure;
                for f in fields {
                    e = e.union(self.predict_effect(&f.value, ctx));
                }
                e
            }
            ExprKind::Meet(a, b)
            | ExprKind::Join(a, b)
            | ExprKind::Add(a, b)
            | ExprKind::Sub(a, b)
            | ExprKind::Mul(a, b)
            | ExprKind::Div(a, b)
            | ExprKind::Eq(a, b)
            | ExprKind::Ne(a, b) => self
                .predict_effect(a, ctx)
                .union(self.predict_effect(b, ctx)),
            ExprKind::Lens(obj, _) => self.predict_effect(obj, ctx),
            ExprKind::List(items) => {
                let mut e = EffectTag::Pure;
                for i in items {
                    e = e.union(self.predict_effect(i, ctx));
                }
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
                    adj.entry(r.left.clone())
                        .or_insert(Vec::new())
                        .push(r.right.clone());
                    has_incoming.insert(r.right.clone());
                }
                ValRelOp::Gt | ValRelOp::Gte => {
                    adj.entry(r.right.clone())
                        .or_insert(Vec::new())
                        .push(r.left.clone());
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
                    if new_r > *cur_r {
                        *cur_r = new_r;
                    }
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
                    (Some(ra), None) => {
                        ranks.insert(b.clone(), ra);
                        changed = true;
                    }
                    (None, Some(rb)) => {
                        ranks.insert(a.clone(), rb);
                        changed = true;
                    }
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
    fn eval_elements(
        &self,
        items: &[Expr],
        ctx: &mut EvalContext,
    ) -> std::result::Result<(IndexMap<String, Value>, EffectTag), Value> {
        let mut res = IndexMap::new();
        let mut me = EffectTag::Pure;
        let mut idx = 0usize;
        for item in items {
            if let ExprKind::Spread(inner) = &item.kind {
                let sv = self.force(self.eval(inner, ctx), ctx);
                // both bottom representations collapse the container:
                // runtime Value::Bottom (with cause) and the literal _|_ atom
                if matches!(sv, Value::Bottom(_))
                    || matches!(sv, Value::Atom(AtomKind::Bottom, _, _))
                {
                    return Err(sv);
                }
                me = me.union(sv.effect());
                if let Value::Combo(cv) = sv {
                    let mut keys: Vec<usize> = cv
                        .data
                        .keys()
                        .filter_map(|k| k.parse::<usize>().ok())
                        .collect();
                    keys.sort_unstable();
                    for k in keys {
                        if let Some(v) = cv.data.get(&k.to_string()) {
                            me = me.union(v.effect());
                            res.insert(idx.to_string(), v.clone());
                            idx += 1;
                        }
                    }
                }
                continue;
            }
            let val = self.eval(item, ctx);
            me = me.union(val.effect());
            res.insert(idx.to_string(), val);
            idx += 1;
        }
        Ok((res, me))
    }

    // One pipe step for a single (non-superposed) input value: binds $ := lv,
    // evaluates the RHS, then dispatches on its form — morphism application
    // (with list functor-lifting fallback), transformer merge, or passthrough.
    fn pipe_apply(&self, lv: Value, r: &Expr, ctx: &mut EvalContext) -> Value {
        if let Err(e) = ctx.check_resources(mbu::OPERATOR_APPLICATION) {
            let partial = if needs_partial_body(&e, ctx.strategy) {
                Some(lv.clone())
            } else {
                None
            };
            return handle_resource_exhausted(e, ctx.strategy, &*ctx, partial, lv.effect());
        }
        let mut call_ctx = self.sub_context(ctx);
        call_ctx.context_value = Some(lv.clone());
        // Solidify field *thunks* on multi-segment paths like `p.add`
        // (GUIDE_03 §11.4 leaves them unforced) so morphism judgment sees
        // the sealed combo. Do **not** force Refs — Stage 3 live-Ref late
        // binding requires the pipe RHS to stay symbolic until apply.
        let mut rv = self.eval(r, &mut call_ctx);
        while matches!(&rv, Value::Thunk { .. }) {
            rv = self.force(rv, &mut call_ctx);
        }
        ctx.fuel = call_ctx.fuel;
        if rv.is_morphism() {
            let res = self.apply_morphism(rv.clone(), lv.clone(), ctx);
            if matches!(res, Value::Bottom(_) | Value::Top) {
                if let Value::Combo(ref cv) = lv {
                    if self.is_list(&lv, ctx) {
                        if let Err(e) = ctx.check_resources(mbu::LIFTING_BASE) {
                            let partial = if needs_partial_body(&e, ctx.strategy) {
                                Some(lv.clone())
                            } else {
                                None
                            };
                            return handle_resource_exhausted(
                                e,
                                ctx.strategy,
                                &*ctx,
                                partial,
                                lv.effect(),
                            );
                        }
                        let mut res_fields = IndexMap::new();
                        let mut max_e = lv.effect();
                        let mut lifted = false;
                        for (k, v) in &cv.fields() {
                            if k.parse::<usize>().is_ok() {
                                if let Err(e) = ctx.check_resources(mbu::OPERATOR_APPLICATION) {
                                    return handle_resource_exhausted(
                                        e,
                                        ctx.strategy,
                                        &*ctx,
                                        None,
                                        lv.effect(),
                                    );
                                }
                                let item = self.force(v.clone(), ctx);
                                let item_res = self.apply_morphism(rv.clone(), item, ctx);
                                if !matches!(item_res, Value::Bottom(_)) {
                                    let solidified = self.force_recursive(item_res, ctx);
                                    max_e = max_e.union(solidified.effect());
                                    res_fields.insert(k.clone(), solidified);
                                    lifted = true;
                                }
                            } else {
                                res_fields.insert(k.clone(), v.clone());
                            }
                        }
                        if lifted {
                            res_fields.insert(
                                "%kind".to_string(),
                                Value::Atom(
                                    AtomKind::Tag("list".to_string()),
                                    EffectTag::Pure,
                                    None,
                                ),
                            );
                            return Value::Combo(ComboVal::new(
                                res_fields,
                                false,
                                IndexMap::new(),
                                max_e,
                                vec![],
                            ));
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
        // Traversing the already-supplied AST is bounded by that AST's size;
        // it is therefore not an MBU operation.  Keep this resource check for
        // depth/stack/timeout enforcement, while charging only the semantic
        // operations selected below (REAL_01 §9.1; meter_reads_two §6.3).
        if let Err(e) = ctx.check_resources(0) {
            // O42 R-3: node_content for Blur only — never clone a deep AST
            // for stack_overflow / Strict (see needs_partial_body).
            let partial = if needs_partial_body(&e, ctx.strategy) {
                Some(Value::Code(Box::new(expr.clone().without_spans())))
            } else {
                None
            };
            return handle_resource_exhausted(e, ctx.strategy, &*ctx, partial, EffectTag::Pure);
        }
        match &expr.kind {
            ExprKind::Atom(kind) => match kind.clone() {
                // Dual of Top (04df5c4): lattice bottom is Value::Bottom, not
                // Atom(AtomKind::Bottom). Declared empty uses Conflict cause —
                // same object as empty AnonSet `@{}` (eval wildcard → Conflict).
                AtomKind::Top => Value::Top,
                AtomKind::Bottom => BottomCause::Conflict.into(),
                k => Value::Atom(k, EffectTag::Pure, None),
            },
            ExprKind::Combo {
                fields,
                relations,
                closed,
            } => {
                // Literal construction is bounded by the supplied AST. Dynamic
                // spread expansion is charged later, at its semantic reads and
                // merges; this check remains for non-fuel horizon gates.
                if let Err(e) = ctx.check_resources(0) {
                    let partial = if needs_partial_body(&e, ctx.strategy) {
                        Some(Value::Code(Box::new(expr.clone().without_spans())))
                    } else {
                        None
                    };
                    return handle_resource_exhausted(
                        e,
                        ctx.strategy,
                        &*ctx,
                        partial,
                        EffectTag::Pure,
                    );
                }
                let mut rf = IndexMap::new();
                let mut rl = IndexMap::new();
                let mut me = EffectTag::Pure;
                let mut rv = Vec::new();
                // T1 (cause_canon): blur spread absorbs the target but does
                // NOT early-return — remaining sources/fields keep folding
                // through unify. ⊥ early-return stays lawful (⊥ absorbs
                // everything including blur). If we finish with a blur
                // absorb and no later ⊥, emit the blur snapshot (single-
                // source blur absorb law from blur_spread arc).
                let mut blur_absorb: Option<crate::value::BlurDetail> = None;
                // SPEC_03 §3.1 timing (forward_spread): defer source expansion
                // until observation convergence so forward-defined sources
                // participate. Eager construction made position change results.
                let mut pending_spreads: Vec<Value> = Vec::new();
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
                    if !rf.contains_key(&lt) {
                        rf.insert(lt, Value::Atom(r.left.clone(), EffectTag::Pure, None));
                    }
                    if !rf.contains_key(&rt) {
                        rf.insert(rt, Value::Atom(r.right.clone(), EffectTag::Pure, None));
                    }
                }
                for f in fields {
                    match &f.key {
                        FieldKey::Quoted(name) if name == "..." => {
                            // SPEC_03 §3.1: spread is lattice merge (intersect on
                            // key collision), not last-wins overwrite.
                            // C4: direct-name circular → ⊥ #divergent before eval
                            // (under-construction stack / force in_flight).
                            // Guard stays armed at construction AND re-checked at
                            // expansion (forward_spread cycle red gate).
                            if spread_path_is_under_construction(&f.value, ctx) {
                                return BottomCause::Divergent.into();
                            }
                            // Defer expansion: source as Thunk; expand at force /
                            // navigate convergence so forward-defined sources
                            // resolve (SPEC_03 §3.1 timing clause).
                            let te = self.predict_effect(&f.value, ctx);
                            pending_spreads.push(Value::Thunk {
                                expr: Box::new(f.value.clone()),
                                closure: ctx.scopes.clone(),
                                context: ctx.context_value.clone().map(Box::new),
                                effect: te,
                            });
                            if !*closed {
                                me = me.union(te);
                            }
                        }
                        FieldKey::Named { name, prefix } => {
                            let is_p = matches!(prefix, Some(Prefix::Private));
                            let te = self.predict_effect(&f.value, ctx);
                            // SPEC_09: combo-level `~%` definition keys mint ⊥.
                            let mut val = if matches!(prefix, Some(Prefix::System)) {
                                BottomCause::SystemReserved.into()
                            } else if expr_is_pure_circular_spread(&f.value, ctx) {
                                // C4 ancestor: pure re-spread of a name under
                                // construction → bind ⊥ #divergent at this field
                                // (avoids force_recursive runaway nesting).
                                BottomCause::Divergent.into()
                            } else {
                                Value::Thunk {
                                    expr: Box::new(f.value.clone()),
                                    closure: ctx.scopes.clone(),
                                    context: ctx.context_value.clone().map(Box::new),
                                    effect: te,
                                }
                            };
                            if matches!(prefix, Some(Prefix::Logic)) {
                                val = Value::Combo(ComboVal::new(
                                    IndexMap::from_iter(vec![
                                        (
                                            "%morphism".to_string(),
                                            Value::Atom(
                                                AtomKind::Tag("true".to_string()),
                                                EffectTag::Pure,
                                                None,
                                            ),
                                        ),
                                        ("%val".to_string(), val),
                                    ]),
                                    false,
                                    IndexMap::new(),
                                    EffectTag::Pure,
                                    vec![],
                                ));
                            }
                            if !*closed {
                                me = me.union(te);
                            }
                            let key = match prefix {
                                Some(Prefix::Logic) => format!("/{}", name),
                                Some(Prefix::Type) => format!("@{}", name),
                                Some(Prefix::Meta) => format!("%{}", name),
                                Some(Prefix::System) => format!("~%{}", name),
                                _ => name.trim().to_string(),
                            };
                            // §1.1 repeated-key merge = intersect (same as spread).
                            if is_p {
                                self.merge_field_into(&mut rl, name.trim().to_string(), val, ctx);
                            } else {
                                self.merge_field_into(&mut rf, key, val, ctx);
                            }
                        }
                        FieldKey::Quoted(name) => {
                            let te = self.predict_effect(&f.value, ctx);
                            let thunk = Value::Thunk {
                                expr: Box::new(f.value.clone()),
                                closure: ctx.scopes.clone(),
                                context: ctx.context_value.clone().map(Box::new),
                                effect: te,
                            };
                            if !*closed {
                                me = me.union(te);
                            }
                            self.merge_field_into(&mut rf, name.trim().to_string(), thunk, ctx);
                        }
                        FieldKey::Pattern(pe) => {
                            let pk = self.eval(pe, ctx).to_string_plain().trim().to_string();
                            let te = self.predict_effect(&f.value, ctx);
                            let rb = Value::Combo(ComboVal::new(
                                IndexMap::from_iter(vec![(
                                    "%val".to_string(),
                                    Value::Thunk {
                                        expr: Box::new(f.value.clone()),
                                        closure: ctx.scopes.clone(),
                                        context: ctx.context_value.clone().map(Box::new),
                                        effect: te,
                                    },
                                )]),
                                true,
                                IndexMap::new(),
                                te,
                                vec![],
                            ));
                            self.merge_field_into(&mut rf, pk, rb, ctx);
                            // Repeated %morphism inserts: #true & #true = #true.
                            self.merge_field_into(
                                &mut rf,
                                "%morphism".to_string(),
                                Value::Atom(
                                    AtomKind::Tag("true".to_string()),
                                    EffectTag::Pure,
                                    None,
                                ),
                                ctx,
                            );
                        }
                        FieldKey::Path(p) => {
                            // Stage 2 (call-by-observation): build a Thunk carrying the
                            // current ctx.context_value (P1 binding) and scopes.
                            // Closed (cocoon) solidification is deferred until AFTER
                            // seal_defining_scope — force-at-build without a frame
                            // baked `_` into eigenstate (sibling / shadowing wrong).
                            // Bare single-segment path keys (`b: …`) parse as Path, not
                            // Named — C4 pure-ancestor check must run here too.
                            // SPEC_09: combo-level `~%…` definition keys mint
                            // ⊥ #system_reserved (no self-heal via lexical skip).
                            let te = self.predict_effect(&f.value, ctx);
                            let sys_reserved = p
                                .segments
                                .first()
                                .map(|s| s.trim().starts_with("~%"))
                                .unwrap_or(false);
                            let val = if sys_reserved {
                                BottomCause::SystemReserved.into()
                            } else if expr_is_pure_circular_spread(&f.value, ctx) {
                                BottomCause::Divergent.into()
                            } else {
                                Value::Thunk {
                                    expr: Box::new(f.value.clone()),
                                    closure: ctx.scopes.clone(),
                                    context: ctx.context_value.clone().map(Box::new),
                                    effect: te,
                                }
                            };
                            // Effect: open tracks predicted; closed takes max after force.
                            if !*closed {
                                me = me.union(te);
                            } else if sys_reserved {
                                me = me.union(val.effect());
                            }
                            let mut tmp = ComboVal::new(
                                IndexMap::new(),
                                *closed,
                                IndexMap::new(),
                                EffectTag::Pure,
                                vec![],
                            );
                            // SPEC_09 ownership (acceptance repair): a forbidden
                            // path key must NOT materialize its intermediate
                            // nodes ({~%Math.add: 7} minting ~%Math: {add: ⊥}
                            // resurrects the silent shadow via the second
                            // spelling) — the WHOLE field collapses at the
                            // first segment.
                            if sys_reserved {
                                let _ = self.inject_path(&mut tmp, &p.segments[..1], val);
                            } else {
                                let _ = self.inject_path(&mut tmp, &p.segments, val);
                            }
                            // Path-key sibling merge: {a:{x:1}, a.y:2} → a merges.
                            for (k, v) in tmp.fields() {
                                self.merge_field_into(&mut rf, k, v, ctx);
                            }
                            for (k, v) in tmp.local_fields() {
                                self.merge_field_into(&mut rl, k, v, ctx);
                            }
                        }
                    }
                }
                // T1: if a blur source absorbed the target and no later ⊥
                // collapsed the fold, the combo is that #blur snapshot.
                // Plain fields interleaved after blur do not resurrect the
                // target (blur absorbs; same end-state as single-source law).
                // Note: blur absorb from *deferred* spreads is applied at
                // expand time, not here (construction no longer forces sources).
                if let Some(bd) = blur_absorb {
                    return Value::Blur(bd);
                }
                let ranks = self.compute_ranks(&rv);
                for (tag_name, rank) in ranks {
                    if let Some(v) = rf.get_mut(&tag_name) {
                        if let Value::Atom(ak, ae, _) = v.clone() {
                            rf.insert(tag_name, Value::Atom(ak, ae, Some(rank)));
                        }
                    }
                }
                let mut combo = ComboVal::new(rf, *closed, rl, me, rv);
                combo.pending_spreads = pending_spreads;
                // SPEC_04 §2.1 / §3.1: bare names resolve through the defining
                // combo as a scope frame (public + private).
                crate::value::seal_defining_scope(&mut combo);
                let mut res = Value::Combo(combo);
                // GUIDE_03 §11.5: cocoon is a solidification boundary — force
                // after seal so siblings see the holder frame (inner-first).
                // O47: if solidification would immediately hit the depth
                // ceiling on a *flat* cocoon (no nested Combo), skip force so
                // `{{b:1}} & depth_blur` does not mint a co-horizon.
                // Nested cocoons still solidify (and mint blur when deep).
                if *closed {
                    let would_hit_ceiling =
                        (ctx.depth as u32).saturating_add(1) > ctx.max_unification_depth as u32;
                    // Flat = no nested structure in data (Combo, or Thunk of
                    // Combo/Meet). Bare path keys store atoms as Thunks — those
                    // are still flat. Deep cocoons store nested expr Thunks.
                    let flat = match &res {
                        Value::Combo(c) => c.data.values().all(|v| match v {
                            Value::Combo(_) => false,
                            Value::Thunk { expr, .. } => !matches!(
                                &expr.kind,
                                ExprKind::Combo { .. }
                                    | ExprKind::Meet(_, _)
                                    | ExprKind::Join(_, _)
                            ),
                            _ => true,
                        }),
                        _ => true,
                    };
                    if !(would_hit_ceiling && flat) {
                        res = self.force_recursive(res, ctx);
                        me = me.union(res.effect());
                        if let Value::Combo(ref mut cv) = res {
                            cv.effect = me;
                        }
                    }
                }
                if let Value::Combo(ref cv) = res {
                    if let Some(mode_v) = cv.fields().get("%eval_mode") {
                        let m = self.force(mode_v.clone(), ctx);
                        if m.to_string_plain().trim_start_matches('#') == "eager" {
                            res = self.force_recursive(res, ctx);
                        }
                    }
                }
                // SPEC_08 §4.3 (Model A): declared `%effect: #pure` contradicted
                // by actual active contagion → ⊥ #effect_violation. Cocoon
                // auto-exempts: closed shield leaves cv.effect pure, so
                // has_active is false (no special-case).
                if let Value::Combo(ref cv) = res {
                    if declared_pure_meta(cv, self, ctx) && cv.effect.has_active() {
                        return Value::Bottom(Box::new(BottomDetail {
                            cause: BottomCause::EffectViolation,
                            path: None,
                            message: Some(format!("declared #pure but observes {}", cv.effect)),
                            expected: None,
                            found: None,
                            involved: vec![],
                            ..Default::default()
                        }));
                    }
                }
                res
            }
            ExprKind::Path(p) => self.resolve_path(p, ctx),
            ExprKind::Apply(f, a) => {
                if let Err(e) = ctx.check_resources(mbu::OPERATOR_APPLICATION) {
                    let partial = if needs_partial_body(&e, ctx.strategy) {
                        Some(Value::Code(Box::new(expr.clone().without_spans())))
                    } else {
                        None
                    };
                    return handle_resource_exhausted(
                        e,
                        ctx.strategy,
                        &*ctx,
                        partial,
                        EffectTag::Pure,
                    );
                }
                let fv = self.eval(f, ctx);
                let av = self.eval(a, ctx);
                // G3 R1: Apply absorbs #blur before morphism dispatch
                // (apply_morphism has no Blur arm → would mint ⊥ #conflict).
                if let Value::Blur(bd) = &fv {
                    let mut bd = bd.clone();
                    bd.effect = bd.effect.union(av.effect());
                    return Value::Blur(bd);
                }
                if let Value::Blur(bd) = &av {
                    let mut bd = bd.clone();
                    bd.effect = bd.effect.union(fv.effect());
                    return Value::Blur(bd);
                }
                self.apply_morphism(fv.clone(), av.clone(), ctx)
            }
            ExprKind::Pipe(l, r) => {
                let lv = self.eval(l, ctx);
                if let Value::Bottom(_) = lv {
                    return lv;
                }
                // G3 R2: pipe argument carries #blur into the body; do not
                // re-mint at the pipe boundary (absorption happens in body
                // value contexts / apply). Pass Blur through when the RHS
                // never consumes it — apply_morphism already absorbs.
                // bind additivity (SPEC_07 §4 疊加態平等演化; ENGINE_SYNC #18):
                // a superposed input evolves branchwise with its OWN $ binding —
                // (A|B) |> f ≡ (A|>f) | (B|>f); ⊥ branches prune (| identity)
                if let Value::Union(branches) = lv {
                    let mut out = Vec::new();
                    for b in branches {
                        let res = self.pipe_apply(b, r, ctx);
                        if !matches!(res, Value::Bottom(_))
                            && !matches!(res, Value::Atom(AtomKind::Bottom, _, _))
                        {
                            out.push(res);
                        }
                    }
                    return self.normalize_union_absorbing(out, ctx);
                }
                self.pipe_apply(lv, r, ctx)
            }
            ExprKind::Morphism { param, body } => {
                // G5: Tuple of bare single-segment paths → one rule + %params
                // (positional destructure). Nested/non-path tuples keep `_` key.
                let (pk, tuple_params) = match &param.kind {
                    ExprKind::Path(p) => {
                        let last = p
                            .segments
                            .last()
                            .cloned()
                            .unwrap_or_else(|| "_".to_string());
                        (
                            last.trim()
                                .trim_start_matches(|c| {
                                    c == '/' || c == '@' || c == '~' || c == '%'
                                })
                                .to_string(),
                            None,
                        )
                    }
                    ExprKind::Atom(AtomKind::Tag(t)) => (t.trim().to_string(), None),
                    ExprKind::Tuple(items) => {
                        let mut names = Vec::new();
                        let mut ok = !items.is_empty();
                        for it in items {
                            match &it.kind {
                                ExprKind::Path(p)
                                    if p.anchor == PathAnchor::Bare && p.segments.len() == 1 =>
                                {
                                    names.push(p.segments[0].trim().to_string());
                                }
                                _ => {
                                    ok = false;
                                    break;
                                }
                            }
                        }
                        if ok {
                            let key = format!("({})", names.join(", "));
                            (key, Some(names))
                        } else {
                            ("_".to_string(), None)
                        }
                    }
                    _ => ("_".to_string(), None),
                };
                let te = self.predict_effect(body, ctx);
                let mut rule_fields = IndexMap::new();
                rule_fields.insert("%code".to_string(), Value::Code(Box::new(*body.clone())));
                let mut free = free_bare_names(body, &morphism_parameter_names(param));
                close_free_dependencies(&ctx.scopes, &mut free);
                let mut closure_fields = IndexMap::new();
                let current_scopes = ctx.scopes.clone();
                for (i, s) in current_scopes.iter().enumerate() {
                    let captured = capture_free_fields(s, &free);
                    // Frame position is semantic for `^` navigation. Keep an
                    // empty frame as an empty frame rather than compressing
                    // the chain and turning a parent reference into another
                    // coordinate.
                    closure_fields.insert(i.to_string(), Value::Combo(captured));
                }
                rule_fields.insert(
                    "%closure".to_string(),
                    Value::Combo(ComboVal::new(
                        closure_fields,
                        true,
                        IndexMap::new(),
                        EffectTag::Pure,
                        vec![],
                    )),
                );
                // G5 R-P: index → param name metadata (dispatch skips % keys).
                if let Some(names) = tuple_params {
                    let mut pf = IndexMap::new();
                    for (i, n) in names.into_iter().enumerate() {
                        pf.insert(
                            i.to_string(),
                            Value::Atom(AtomKind::Str(n), EffectTag::Pure, None),
                        );
                    }
                    rule_fields.insert(
                        "%params".to_string(),
                        Value::Combo(ComboVal::new(
                            pf,
                            true,
                            IndexMap::new(),
                            EffectTag::Pure,
                            vec![],
                        )),
                    );
                }
                let mut rules = IndexMap::new();
                rules.insert(
                    pk,
                    Value::Combo(ComboVal::new(
                        rule_fields,
                        true,
                        IndexMap::new(),
                        te,
                        vec![],
                    )),
                );
                let mut fields = IndexMap::new();
                fields.insert(
                    "%morphism".to_string(),
                    Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                );
                fields.insert(
                    "%kind".to_string(),
                    Value::Atom(AtomKind::Tag("logic".to_string()), EffectTag::Pure, None),
                );
                fields.insert(
                    "%rules".to_string(),
                    Value::Combo(ComboVal::new(rules, true, IndexMap::new(), te, vec![])),
                );
                Value::Combo(ComboVal::new(fields, true, IndexMap::new(), te, vec![]))
            }
            // $ rules P1-P5 (SPEC_07 §4.2, 2026-07-05): bound only at evolution
            // boundaries (pipe / morphism application); a free $ observed without
            // an enclosing evolution collapses to _|_ #no_context (P3)
            ExprKind::Context => match &ctx.context_value {
                Some(v) => v.clone(),
                None => Value::Bottom(Box::new(BottomDetail {
                    cause: BottomCause::NoContext,
                    message: Some(
                        "free `$` observed without an enclosing evolution (P3)".to_string(),
                    ),
                    ..Default::default()
                })),
            },
            ExprKind::Ternary {
                cond,
                then_branch,
                else_branch,
            } => {
                let cv = self.eval(cond, ctx).collapse().clone();
                match cv {
                    Value::Atom(AtomKind::Tag(ref t), _, _)
                        if t.trim_start_matches('#') == "true" =>
                    {
                        self.eval(then_branch, ctx).with_effect(cv.effect())
                    }
                    Value::Atom(AtomKind::Tag(ref t), _, _)
                        if t.trim_start_matches('#') == "false" =>
                    {
                        self.eval(else_branch, ctx).with_effect(cv.effect())
                    }
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
                // E3: meet-context membership negation —
                // `x & !(range)` / `!(range) & x` ⟺ x if x ∉ range else ⊥.
                // Standalone `!(range)` still goes through Unary → orthocomplement → ⊥.
                if let Some(v) = self.try_meet_not_range(a, b, ctx) {
                    return v;
                }
                let va = self.eval(a, ctx);
                let vb = self.eval(b, ctx);
                self.unify_internal(va, vb, ctx)
            }
            ExprKind::Join(a, b) => {
                // Join normally normalizes without passing through
                // `unify_internal`; bill its one semantic union here.  Any
                // runtime-sized absorption comparisons subsequently recurse
                // through `unify_internal` and bill there as well.
                if let Err(e) = ctx.check_resources(mbu::ORTHOGONAL_MERGE) {
                    return handle_resource_exhausted(
                        e,
                        ctx.strategy,
                        &*ctx,
                        None,
                        EffectTag::Pure,
                    );
                }
                let va = self.eval(a, ctx);
                let vb = self.eval(b, ctx);
                // SPEC_01 §2.4.2: absorb after dedupe (union_absorption).
                self.normalize_union_absorbing(vec![va, vb], ctx)
            }
            ExprKind::Diff(a, b) => {
                let va = self.eval(a, ctx);
                let vb = self.eval(b, ctx);
                let vb_complement = self.orthocomplement(vb, ctx);
                self.unify_internal(va, vb_complement, ctx)
            }
            ExprKind::Add(a, b) => self.eval_math(
                a,
                b,
                ctx,
                MathOp::Add,
                |x: &BigInt, y: &BigInt| x + y,
                |x: f64, y: f64| x + y,
                Some(|sx: &str, sy: &str| format!("{}{}", sx, sy)),
            ),
            ExprKind::Sub(a, b) => self.eval_math(
                a,
                b,
                ctx,
                MathOp::Sub,
                |x: &BigInt, y: &BigInt| x - y,
                |x: f64, y: f64| x - y,
                None::<fn(&str, &str) -> String>,
            ),
            ExprKind::Mul(a, b) => self.eval_math(
                a,
                b,
                ctx,
                MathOp::Mul,
                |x: &BigInt, y: &BigInt| x * y,
                |x: f64, y: f64| x * y,
                None::<fn(&str, &str) -> String>,
            ),
            ExprKind::Div(a, b) => self.eval_math(
                a,
                b,
                ctx,
                MathOp::Div,
                |x: &BigInt, y: &BigInt| if y.is_zero() { BigInt::zero() } else { x / y },
                |x: f64, y: f64| x / y,
                None::<fn(&str, &str) -> String>,
            ),
            ExprKind::Rem(a, b) => self.eval_math(
                a,
                b,
                ctx,
                MathOp::Rem,
                |x: &BigInt, y: &BigInt| if y.is_zero() { BigInt::zero() } else { x % y },
                |x: f64, y: f64| x % y,
                None::<fn(&str, &str) -> String>,
            ),
            ExprKind::Eq(a, b) => self.eval_binary_cmp(a, b, ctx, CmpOp::Eq),
            ExprKind::Ne(a, b) => self.eval_binary_cmp(a, b, ctx, CmpOp::Ne),
            ExprKind::Lt(a, b) => self.eval_binary_cmp(a, b, ctx, CmpOp::Lt),
            ExprKind::Gt(a, b) => self.eval_binary_cmp(a, b, ctx, CmpOp::Gt),
            ExprKind::Lte(a, b) => self.eval_binary_cmp(a, b, ctx, CmpOp::Lte),
            ExprKind::Gte(a, b) => self.eval_binary_cmp(a, b, ctx, CmpOp::Gte),
            ExprKind::List(items) => match self.eval_elements(items, ctx) {
                Err(bottom) => bottom,
                Ok((mut res, me)) => {
                    res.insert(
                        "%kind".to_string(),
                        Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
                    );
                    Value::Combo(ComboVal::new(res, false, IndexMap::new(), me, vec![]))
                }
            },
            ExprKind::Lens(obj, key) => {
                let ov = self.eval(obj, ctx);
                let kv = self.eval(key, ctx);
                let ks = kv.collapse().to_string_plain();
                self.navigate_segments(ov, &[ks], ctx, "")
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
                            max_e = max_e.union(solidified.effect());
                            res.push_str(&solidified.to_string_plain());
                        }
                    }
                }
                Value::Atom(AtomKind::Str(res), max_e, None)
            }
            ExprKind::Structural(e) => {
                // Stage 3 (§3a): structural form <<path>> — symbolic reference.
                // The structural brackets are "this is a reference, don't evaluate
                // it yet." Non-path operands keep geometric body but mark
                // structural-view so G6 observation does not peel %val
                // (SYNTAX_07 §2.2 / §4 #6 duality).
                if let ExprKind::Path(p) = &e.kind {
                    Value::Ref(p.clone())
                } else {
                    mark_structural_view(self.eval(e, ctx))
                }
            }
            ExprKind::Unary { op, expr } => {
                let v = self.eval(expr, ctx).collapse().clone();
                // G3 R1: unary value context absorbs #blur (do not mint #conflict).
                if let Value::Blur(_) = &v {
                    return v;
                }
                match op {
                    nlang_parser::ast::UnaryOp::Not => self.orthocomplement(v, ctx),
                    nlang_parser::ast::UnaryOp::Neg => match v {
                        Value::Atom(AtomKind::Int(i), e, _) => {
                            Value::Atom(AtomKind::Int(-i), e, None)
                        }
                        Value::Atom(AtomKind::Float(f), e, _) => {
                            Value::Atom(AtomKind::Float(-f), e, None)
                        }
                        Value::Atom(AtomKind::Complex(r, i), e, _) => {
                            Value::Atom(AtomKind::Complex(-r, -i), e, None)
                        }
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
                // Fixed tuple construction is AST-bounded (not an MBU row).
                if let Err(e) = ctx.check_resources(0) {
                    let partial = if needs_partial_body(&e, ctx.strategy) {
                        Some(Value::Code(Box::new(expr.clone().without_spans())))
                    } else {
                        None
                    };
                    return handle_resource_exhausted(
                        e,
                        ctx.strategy,
                        &*ctx,
                        partial,
                        EffectTag::Pure,
                    );
                }
                match self.eval_elements(items, ctx) {
                    Err(bottom) => bottom,
                    Ok((rf, me)) => {
                        Value::Combo(ComboVal::new(rf, true, IndexMap::new(), me, vec![]))
                    }
                }
            }
            // Poset literal #{ ... }: relation-only combo; members get ranks (SYNTAX_10)
            ExprKind::Poset(relations) => {
                // Rank derivation is over the literal's relation AST.  It is
                // not the REAL_01 spectral-calibration operation (which is no
                // language-level evaluator path in this engine yet).
                if let Err(e) = ctx.check_resources(0) {
                    let partial = if needs_partial_body(&e, ctx.strategy) {
                        Some(Value::Code(Box::new(expr.clone().without_spans())))
                    } else {
                        None
                    };
                    return handle_resource_exhausted(
                        e,
                        ctx.strategy,
                        &*ctx,
                        partial,
                        EffectTag::Pure,
                    );
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
                    if !rf.contains_key(&lt) {
                        rf.insert(lt, Value::Atom(r.left.clone(), EffectTag::Pure, None));
                    }
                    if !rf.contains_key(&rt) {
                        rf.insert(rt, Value::Atom(r.right.clone(), EffectTag::Pure, None));
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
                Value::Combo(ComboVal::new(
                    rf,
                    false,
                    IndexMap::new(),
                    EffectTag::Pure,
                    rv,
                ))
            }
            // Closed interval set [start, end] (SPEC_02 §3 / SYNTAX_04 §4.5).
            // Bounds evaluated at observation time (variable bounds free).
            ExprKind::Range { start, end, step } => {
                let vs = self.force(self.eval(start, ctx), ctx);
                if let Value::Bottom(_) = &vs {
                    return vs;
                }
                let ve = self.force(self.eval(end, ctx), ctx);
                if let Value::Bottom(_) = &ve {
                    return ve;
                }
                let vst = match step {
                    Some(s) => {
                        let v = self.force(self.eval(s, ctx), ctx);
                        if let Value::Bottom(_) = &v {
                            return v;
                        }
                        Some(Box::new(v))
                    }
                    None => None,
                };
                Value::Range {
                    start: Box::new(vs),
                    end: Box::new(ve),
                    step: vst,
                }
            }
            // `@{ e } ≡ e` (SYNTAX_04 §4.7); empty `@{}` is AnonSet(Bottom) → ⊥.
            ExprKind::AnonSet(inner) => self.eval(inner, ctx),
            // Lattice-family equality `=` (SYNTAX_06 §4 #11/#13): solidify
            // both sides, then one engine-wide PartialEq (span-blind Code/
            // Thunk; effect participates; field order free via IndexMap).
            // Set family does NOT absorb ⊥ as a return value: `_|_` is an
            // operand (empty set) → boolean verdict.
            // Blur boundary #6 (SPEC_08 §3.2.2): two-stage after solidify —
            // ⊥ first (unchanged), then Blur (same-CAID → #true; else absorb
            // left-priority — never silent #false).
            ExprKind::LatticeEq(a, b) => {
                let va = self.force_recursive(self.eval(a, ctx), ctx);
                let vb = self.force_recursive(self.eval(b, ctx), ctx);
                // Stage ⊥ (set-family operand semantics — not G3 value absorb).
                let eq_bottom = match (&va, &vb) {
                    (Value::Bottom(_), Value::Bottom(_)) => Some(true),
                    (Value::Bottom(_), _) | (_, Value::Bottom(_)) => Some(false),
                    _ => None,
                };
                if let Some(eq) = eq_bottom {
                    let res_e = va.effect().union(vb.effect());
                    return Value::Atom(
                        AtomKind::Tag(if eq {
                            "true".to_string()
                        } else {
                            "false".to_string()
                        }),
                        res_e,
                        None,
                    );
                }
                // Stage Blur (#6).
                match (&va, &vb) {
                    (Value::Blur(ba), Value::Blur(bb)) if ba.blur_caid() == bb.blur_caid() => {
                        let res_e = va.effect().union(vb.effect());
                        return Value::Atom(AtomKind::Tag("true".to_string()), res_e, None);
                    }
                    (Value::Blur(bd), _) => {
                        let mut bd = bd.clone();
                        bd.effect = bd.effect.union(vb.effect());
                        return Value::Blur(bd);
                    }
                    (_, Value::Blur(bd)) => {
                        let mut bd = bd.clone();
                        bd.effect = bd.effect.union(va.effect());
                        return Value::Blur(bd);
                    }
                    _ => {}
                }
                let res_e = va.effect().union(vb.effect());
                let eq = va == vb;
                Value::Atom(
                    AtomKind::Tag(if eq {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    }),
                    res_e,
                    None,
                )
            }
            // Direction probe `<=>`: returns an order tag, never a boolean (SYNTAX_10 §2.3)
            ExprKind::Probe(a, b) => {
                let va = self.eval(a, ctx);
                if let Value::Bottom(_) = va {
                    return va;
                }
                let vb = self.eval(b, ctx);
                if let Value::Bottom(_) = vb {
                    return vb;
                }
                // G3 R1: Probe is a value-context consumer — absorb #blur
                // (measured same disease as math catch-all before this arm).
                if let Value::Blur(bd) = &va {
                    let mut bd = bd.clone();
                    bd.effect = bd.effect.union(vb.effect());
                    return Value::Blur(bd);
                }
                if let Value::Blur(bd) = &vb {
                    let mut bd = bd.clone();
                    bd.effect = bd.effect.union(va.effect());
                    return Value::Blur(bd);
                }
                let res_e = va.effect().union(vb.effect());
                let ca = va.collapse().clone();
                let cb = vb.collapse().clone();
                if ca.is_top() || cb.is_top() {
                    return Value::Top;
                }
                if let Value::Bottom(d) = ca {
                    return Value::Bottom(d);
                }
                if let Value::Bottom(d) = cb {
                    return Value::Bottom(d);
                }
                let dir_tag = |o: Option<std::cmp::Ordering>| -> Value {
                    match o {
                        Some(std::cmp::Ordering::Less) => {
                            Value::Atom(AtomKind::Tag("lt".to_string()), res_e, None)
                        }
                        Some(std::cmp::Ordering::Greater) => {
                            Value::Atom(AtomKind::Tag("gt".to_string()), res_e, None)
                        }
                        Some(std::cmp::Ordering::Equal) => {
                            Value::Atom(AtomKind::Tag("eq".to_string()), res_e, None)
                        }
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
                        let x_tagish =
                            matches!(xk, AtomKind::Tag(_) | AtomKind::TagStart | AtomKind::TagEnd);
                        let y_tagish =
                            matches!(yk, AtomKind::Tag(_) | AtomKind::TagStart | AtomKind::TagEnd);
                        if x_tagish && y_tagish {
                            if xk == yk {
                                return dir_tag(Some(std::cmp::Ordering::Equal));
                            }
                            if let (Some(x), Some(y)) = (rx, ry) {
                                return dir_tag(Some(x.cmp(y)));
                            }
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

    fn eval_math<FI, FF, FS>(
        &self,
        a: &Expr,
        b: &Expr,
        ctx: &mut EvalContext,
        op: MathOp,
        op_i: FI,
        op_f: FF,
        op_s: Option<FS>,
    ) -> Value
    where
        FI: Fn(&BigInt, &BigInt) -> BigInt,
        FF: Fn(f64, f64) -> f64,
        FS: Fn(&str, &str) -> String,
    {
        // Stage 2 (紀律 2): value-judgment point — force thunks before arithmetic.
        // `$.a + 1` inside a pipe transformer evals `$.a` to a Thunk (the field
        // is lazily stored); arithmetic needs the actual value.
        let va = self.force(self.eval(a, ctx), ctx);
        // ⊥ short-circuit first (G3 trap 2: order preserved vs Blur).
        if let Value::Bottom(_) = va {
            return va;
        }
        let vb = self.force(self.eval(b, ctx), ctx);
        if let Value::Bottom(_) = vb {
            return vb;
        }
        // G3 R1: whole-operand Blur short-circuit BEFORE Union distribute and
        // before value_context_operand — single-value `big + 1` absorbs.
        if let Value::Blur(bd) = &va {
            let mut bd = bd.clone();
            bd.effect = bd.effect.union(vb.effect());
            return Value::Blur(bd);
        }
        if let Value::Blur(bd) = &vb {
            let mut bd = bd.clone();
            bd.effect = bd.effect.union(va.effect());
            return Value::Blur(bd);
        }
        // SPEC_07 §4: Union distribution after operand-level ⊥/Blur, before
        // value-context peel (math_union arc).
        self.eval_math_values(va, vb, ctx, op, &op_i, &op_f, op_s.as_ref())
    }

    /// Value-level math kernel: Union distribute (left-major) then atom matrix.
    /// Recursive so branch results re-enter Blur/Top/⊥ single-value arms.
    fn eval_math_values<FI, FF, FS>(
        &self,
        va: Value,
        vb: Value,
        ctx: &mut EvalContext,
        op: MathOp,
        op_i: &FI,
        op_f: &FF,
        op_s: Option<&FS>,
    ) -> Value
    where
        FI: Fn(&BigInt, &BigInt) -> BigInt,
        FF: Fn(f64, f64) -> f64,
        FS: Fn(&str, &str) -> String,
    {
        // Force residual thunks (field projections may still be lazy).
        let mut va = va;
        let mut peel = 0u32;
        while matches!(&va, Value::Thunk { .. }) && peel < 32 {
            va = self.force(va, ctx);
            peel += 1;
        }
        let mut vb = vb;
        peel = 0;
        while matches!(&vb, Value::Thunk { .. }) && peel < 32 {
            vb = self.force(vb, ctx);
            peel += 1;
        }

        // Branch-level ⊥ / Blur (same order as operand-level: ⊥ then Blur).
        if let Value::Bottom(_) = &va {
            return va;
        }
        if let Value::Bottom(_) = &vb {
            return vb;
        }
        if let Value::Blur(bd) = &va {
            let mut bd = bd.clone();
            bd.effect = bd.effect.union(vb.effect());
            return Value::Blur(bd);
        }
        if let Value::Blur(bd) = &vb {
            let mut bd = bd.clone();
            bd.effect = bd.effect.union(va.effect());
            return Value::Blur(bd);
        }

        // Union distribution — left-operand-major (SPEC_07 §4). Cull per-
        // branch ⊥ (union_cull law); all-⊥ → primary member verbatim.
        // Budget: same discipline as unify Union-distribution arm.
        if let Value::Union(branches) = va {
            return self
                .eval_math_distribute_branches(branches, true, vb, ctx, op, op_i, op_f, op_s);
        }
        if let Value::Union(branches) = vb {
            return self
                .eval_math_distribute_branches(branches, false, va, ctx, op, op_i, op_f, op_s);
        }

        // G6: value-context peels hybrid %val (and pure wrappers); plain
        // combos without %val stay on the Conflict path below.
        let va = match value_context_operand(&va) {
            Ok(v) => v,
            Err(()) => return BottomCause::Conflict.into(),
        };
        let vb = match value_context_operand(&vb) {
            Ok(v) => v,
            Err(()) => return BottomCause::Conflict.into(),
        };
        if let Value::Bottom(_) = va {
            return va;
        }
        if let Value::Bottom(_) = vb {
            return vb;
        }
        // Nested unions after peel (rare hybrid shells).
        if matches!(&va, Value::Union(_)) || matches!(&vb, Value::Union(_)) {
            return self.eval_math_values(va, vb, ctx, op, op_i, op_f, op_s);
        }

        let res_e = va.effect().union(vb.effect());
        let ca = va.collapse();
        let cb = vb.collapse();

        if self.is_order_anchor(&ca) || self.is_order_anchor(&cb) {
            return Value::Bottom(Box::new(BottomDetail {
                cause: BottomCause::ArithmeticOnAnchor,
                path: None,
                message: Some(
                    "Arithmetic operations on order anchors (#_, #_|_) are prohibited".to_string(),
                ),
                expected: None,
                found: Some(if self.is_order_anchor(&ca) {
                    ca.clone()
                } else {
                    cb.clone()
                }),
                involved: vec![],
                ..Default::default()
            }));
        }

        match (ca, cb) {
            (
                Value::Atom(AtomKind::Complex(r1, i1), _, _),
                Value::Atom(AtomKind::Complex(r2, i2), _, _),
            ) => self.eval_complex_math(*r1, *i1, *r2, *i2, op, res_e),
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
                // Integer div/rem by zero → ⊥ #numerical_error so union
                // branches can cull (math_union red gate). Float Inf/#_
                // path unchanged via sanitize_float_result.
                if matches!(op, MathOp::Div | MathOp::Rem) && y.is_zero() {
                    return Value::Bottom(Box::new(BottomDetail {
                        cause: BottomCause::NumericalError,
                        path: None,
                        message: Some("integer division by zero".to_string()),
                        expected: None,
                        found: None,
                        involved: vec![],
                        ..Default::default()
                    }));
                }
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
                if let Some(f) = op_s {
                    Value::Atom(AtomKind::Str(f(x, y)), res_e, None)
                } else {
                    BottomCause::Conflict.into()
                }
            }
            (Value::Top | Value::TopCaused { .. }, _)
            | (_, Value::Top | Value::TopCaused { .. }) => Value::Top,
            _ => BottomCause::Conflict.into(),
        }
    }

    /// Distribute math over one Union side. `left_is_union` selects which
    /// operand is branched; the other is held fixed (left-major when both
    /// are unions via nested recursion on the right).
    fn eval_math_distribute_branches<FI, FF, FS>(
        &self,
        branches: Vec<Value>,
        left_is_union: bool,
        other: Value,
        ctx: &mut EvalContext,
        op: MathOp,
        op_i: &FI,
        op_f: &FF,
        op_s: Option<&FS>,
    ) -> Value
    where
        FI: Fn(&BigInt, &BigInt) -> BigInt,
        FF: Fn(f64, f64) -> f64,
        FS: Fn(&str, &str) -> String,
    {
        let max_branches = ctx.max_branches;
        let mut survivors: Vec<Value> = Vec::new();
        let mut culled: Vec<BottomDetail> = Vec::new();
        // Collect without early cap so structural dedupe can free capacity
        // (same budget discipline as unify Union-distribution).
        for b in branches
            .into_iter()
            .take(max_branches.saturating_mul(2).max(2))
        {
            let b = self.force(b, ctx);
            let r = if left_is_union {
                self.eval_math_values(b, other.clone(), ctx, op, op_i, op_f, op_s)
            } else {
                self.eval_math_values(other.clone(), b, ctx, op, op_i, op_f, op_s)
            };
            match r {
                Value::Bottom(d) => culled.push(*d),
                other_r => survivors.push(other_r),
            }
        }
        if survivors.is_empty() {
            return primary_bottom_from_culled(culled);
        }
        let deduped = self.normalize_union_absorbing(survivors, ctx);
        match deduped {
            Value::Union(mut bs) if bs.len() > max_branches => {
                bs.truncate(max_branches);
                Value::Union(bs)
            }
            other => other,
        }
    }

    fn eval_complex_math(
        &self,
        r1: f64,
        i1: f64,
        r2: f64,
        i2: f64,
        op: MathOp,
        effect: EffectTag,
    ) -> Value {
        match op {
            MathOp::Add => Value::Atom(AtomKind::Complex(r1 + r2, i1 + i2), effect, None),
            MathOp::Sub => Value::Atom(AtomKind::Complex(r1 - r2, i1 - i2), effect, None),
            MathOp::Mul => Value::Atom(
                AtomKind::Complex(r1 * r2 - i1 * i2, r1 * i2 + i1 * r2),
                effect,
                None,
            ),
            MathOp::Div => {
                let denom = r2 * r2 + i2 * i2;
                if denom == 0.0 {
                    return BottomCause::NumericalError.into();
                }
                let new_r = (r1 * r2 + i1 * i2) / denom;
                let new_i = (i1 * r2 - r1 * i2) / denom;
                Value::Atom(AtomKind::Complex(new_r, new_i), effect, None)
            }
            MathOp::Rem => BottomCause::Conflict.into(),
        }
    }

    fn is_order_anchor(&self, v: &Value) -> bool {
        matches!(
            v,
            Value::Atom(AtomKind::TagEnd, _, _) | Value::Atom(AtomKind::TagStart, _, _)
        )
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
                ..Default::default()
            }));
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
        // Stage 2 (紀律 2) + G1 #13: solidify before any comparison judgment.
        // Always evaluate both sides so set-family effect max is honest and
        // set-family never short-circuits on ⊥ (SYNTAX_06 §4.1/§4.2 two-family split).
        let va = self.force_recursive(self.eval(a, ctx), ctx);
        let vb = self.force_recursive(self.eval(b, ctx), ctx);

        // ── Atomic family (`==` / `!=`): absorbing ⊥/⊤ (SYNTAX_06 §4.1) ──
        // Policy unchanged from pre-split path: ⊥ before ⊤; return the lattice
        // extreme, never a clean boolean when either side is extreme.
        if matches!(op, CmpOp::Eq | CmpOp::Ne) {
            // ⊥ short-circuit first (order vs Blur preserved).
            if let Value::Bottom(_) = &va {
                return va;
            }
            if let Value::Bottom(_) = &vb {
                return vb;
            }
            // SPEC_08 §3.2.2 #6(a): both #blur and same CAID → definite boolean
            // (O42 makes this reachable). #6(b): different CAID → absorb left.
            // G3 R1: one side blur → absorb — never silent #false via PartialEq.
            let res_e = va.effect().union(vb.effect());
            match (&va, &vb) {
                (Value::Blur(ba), Value::Blur(bb)) if ba.blur_caid() == bb.blur_caid() => {
                    return Value::Atom(
                        AtomKind::Tag(
                            if matches!(op, CmpOp::Eq) {
                                "true"
                            } else {
                                "false"
                            }
                            .to_string(),
                        ),
                        res_e,
                        None,
                    );
                }
                (Value::Blur(bd), _) => {
                    let mut bd = bd.clone();
                    bd.effect = res_e;
                    return Value::Blur(bd);
                }
                (_, Value::Blur(bd)) => {
                    let mut bd = bd.clone();
                    bd.effect = res_e;
                    return Value::Blur(bd);
                }
                _ => {}
            }
            let res_e = va.effect().union(vb.effect());
            // G1 #12: peel hybrid %val into the atomic family; non-collapsible
            // combo (no %val) → ⊥ #conflict (never silent #false). Local to
            // this family — does not change collapse() used by Probe / set.
            let ca = match value_context_operand(&va) {
                Ok(v) => v,
                Err(()) => return BottomCause::Conflict.into(),
            };
            let cb = match value_context_operand(&vb) {
                Ok(v) => v,
                Err(()) => return BottomCause::Conflict.into(),
            };
            if ca.is_top() || cb.is_top() {
                return Value::Top;
            }
            if let Value::Bottom(d) = &ca {
                return Value::Bottom(d.clone());
            }
            if let Value::Bottom(d) = &cb {
                return Value::Bottom(d.clone());
            }

            let op_fn = |x: f64, y: f64| match op {
                CmpOp::Eq => x == y,
                CmpOp::Ne => x != y,
                _ => unreachable!(),
            };
            match (&ca, &cb) {
                (Value::Atom(AtomKind::Int(x), _, _), Value::Atom(AtomKind::Int(y), _, _)) => {
                    return Value::Atom(
                        AtomKind::Tag(
                            if op_fn(x.to_f64().unwrap_or(0.0), y.to_f64().unwrap_or(0.0)) {
                                "true".into()
                            } else {
                                "false".into()
                            },
                        ),
                        res_e,
                        None,
                    );
                }
                (Value::Atom(AtomKind::Float(x), _, _), Value::Atom(AtomKind::Float(y), _, _)) => {
                    return Value::Atom(
                        AtomKind::Tag(if op_fn(*x, *y) {
                            "true".into()
                        } else {
                            "false".into()
                        }),
                        res_e,
                        None,
                    );
                }
                (Value::Atom(AtomKind::Int(x), _, _), Value::Atom(AtomKind::Float(y), _, _)) => {
                    return Value::Atom(
                        AtomKind::Tag(if op_fn(x.to_f64().unwrap_or(0.0), *y) {
                            "true".into()
                        } else {
                            "false".into()
                        }),
                        res_e,
                        None,
                    );
                }
                (Value::Atom(AtomKind::Float(x), _, _), Value::Atom(AtomKind::Int(y), _, _)) => {
                    return Value::Atom(
                        AtomKind::Tag(if op_fn(*x, y.to_f64().unwrap_or(0.0)) {
                            "true".into()
                        } else {
                            "false".into()
                        }),
                        res_e,
                        None,
                    );
                }
                _ => {}
            }
            return Value::Atom(
                AtomKind::Tag(if matches!(op, CmpOp::Eq) {
                    if ca == cb {
                        "true".into()
                    } else {
                        "false".into()
                    }
                } else if ca != cb {
                    "true".into()
                } else {
                    "false".into()
                }),
                res_e,
                None,
            );
        }

        // ── Set family (`<` `<=` `>` `>=`): clean boolean, NON-absorbing ──
        // SYNTAX_06 §4.2: ⊥ = empty set (⊆ everything), ⊤ = universe (⊇ everything).
        let res_e = va.effect().union(vb.effect());
        let ca = va.collapse().clone();
        let cb = vb.collapse().clone();

        let is_bot = |v: &Value| {
            matches!(v, Value::Bottom(_)) || matches!(v, Value::Atom(AtomKind::Bottom, _, _))
        };
        let is_top = |v: &Value| v.is_top() || matches!(v, Value::Atom(AtomKind::Top, _, _));
        // If Atom(Top)/Atom(Bottom) still appear here, they are dual-spelling
        // leftovers (complement path etc.); treated as extremes — not expanded.
        let bool_tag = |b: bool| {
            Value::Atom(
                AtomKind::Tag(if b { "true".into() } else { "false".into() }),
                res_e,
                None,
            )
        };
        // Lte truth table at extremes (handover §修法). Finite×finite falls through.
        let lte_extreme = |x: &Value, y: &Value| -> Option<bool> {
            let xb = is_bot(x);
            let xt = is_top(x);
            let yb = is_bot(y);
            let yt = is_top(y);
            if !(xb || xt || yb || yt) {
                return None;
            }
            // x = ⊥ → true; y = ⊤ → true; y = ⊥ → (x = ⊥); x = ⊤ → (y = ⊤)
            if xb {
                return Some(true);
            }
            if yt {
                return Some(true);
            }
            if yb {
                return Some(xb);
            }
            if xt {
                return Some(yt);
            }
            None
        };
        // Strict subset: ⊥ < y ⟺ y ≠ ⊥; x < ⊤ ⟺ x ≠ ⊤; never ⊤ < · or · < ⊥.
        let lt_extreme = |x: &Value, y: &Value| -> Option<bool> {
            let xb = is_bot(x);
            let xt = is_top(x);
            let yb = is_bot(y);
            let yt = is_top(y);
            if !(xb || xt || yb || yt) {
                return None;
            }
            if xb {
                return Some(!yb);
            }
            if yt {
                return Some(!xt);
            }
            if xt {
                return Some(false);
            }
            if yb {
                return Some(false);
            }
            None
        };

        match op {
            CmpOp::Lte => {
                if let Some(b) = lte_extreme(&ca, &cb) {
                    return bool_tag(b);
                }
            }
            CmpOp::Gte => {
                // x >= y ≡ y <= x
                if let Some(b) = lte_extreme(&cb, &ca) {
                    return bool_tag(b);
                }
            }
            CmpOp::Lt => {
                if let Some(b) = lt_extreme(&ca, &cb) {
                    return bool_tag(b);
                }
            }
            CmpOp::Gt => {
                if let Some(b) = lt_extreme(&cb, &ca) {
                    return bool_tag(b);
                }
            }
            CmpOp::Eq | CmpOp::Ne => unreachable!("atomic family handled above"),
        }

        // SYNTAX_06 §4 #13 (W3): order × #blur follows `=` two-stage law.
        // Before meet reduction (unify must not consume the horizon).
        // Same CAID → reflexive (<=/>= #true, </> #false); else absorb
        // left-priority (never #false / #conflict).
        match (&ca, &cb) {
            (Value::Blur(ba), Value::Blur(bb)) if ba.blur_caid() == bb.blur_caid() => {
                return bool_tag(match op {
                    CmpOp::Lte | CmpOp::Gte => true,
                    CmpOp::Lt | CmpOp::Gt => false,
                    CmpOp::Eq | CmpOp::Ne => unreachable!(),
                });
            }
            (Value::Blur(bd), _) => {
                let mut bd = bd.clone();
                bd.effect = bd.effect.union(cb.effect());
                return Value::Blur(bd);
            }
            (_, Value::Blur(bd)) => {
                let mut bd = bd.clone();
                bd.effect = bd.effect.union(ca.effect());
                return Value::Blur(bd);
            }
            _ => {}
        }

        // Poset tags with declared ranks (SYNTAX_10) — lattice order by rank.
        // Kept before atom-subset so #h1 < #h2 stays the real ≤.
        if let (Value::Atom(ak, _, rx), Value::Atom(bk, _, ry)) = (&ca, &cb) {
            if matches!(ak, AtomKind::Tag(_) | AtomKind::TagStart | AtomKind::TagEnd)
                && matches!(bk, AtomKind::Tag(_) | AtomKind::TagStart | AtomKind::TagEnd)
            {
                if let (Some(rx_val), Some(ry_val)) = (rx, ry) {
                    let x = *rx_val as f64;
                    let y = *ry_val as f64;
                    return bool_tag(match op {
                        CmpOp::Lt => x < y,
                        CmpOp::Gt => x > y,
                        CmpOp::Lte => x <= y,
                        CmpOp::Gte => x >= y,
                        CmpOp::Eq | CmpOp::Ne => unreachable!(),
                    });
                }
            }
        }

        // W2: atom order = subset, not magnitude. Distinct singletons never
        // contain each other → clean #false. Numbers by value (3 ≡ 3.0).
        if matches!((&ca, &cb), (Value::Atom(..), Value::Atom(..))) {
            let same = self.subset_lte(&ca, &cb, ctx);
            return bool_tag(match op {
                CmpOp::Lte | CmpOp::Gte => same,
                CmpOp::Lt | CmpOp::Gt => false,
                CmpOp::Eq | CmpOp::Ne => unreachable!(),
            });
        }

        let is_tc = |v: &Value| matches!(v, Value::Combo(c) if is_type_constraint_combo(c));

        // Type-marker ≤ via subtype table (pin @int <= @num).
        if let (Value::Combo(ac), Value::Combo(bc)) = (&ca, &cb) {
            if is_type_constraint_combo(ac) && is_type_constraint_combo(bc) {
                if let (Some(na), Some(nb)) =
                    (get_type_constraint_name(ac), get_type_constraint_name(bc))
                {
                    let ta = TypeConstraint::from_name(&na);
                    let tb = TypeConstraint::from_name(&nb);
                    let ab = self.check_subtype_relation(&ta, &tb) || na == nb;
                    let ba = self.check_subtype_relation(&tb, &ta) || na == nb;
                    return bool_tag(match op {
                        CmpOp::Lte => ab,
                        CmpOp::Gte => ba,
                        CmpOp::Lt => ab && !ba,
                        CmpOp::Gt => ba && !ab,
                        CmpOp::Eq | CmpOp::Ne => unreachable!(),
                    });
                }
            }
        }

        // Atom × type-constraint (or reverse): meet reduction.
        if (matches!(&ca, Value::Atom(..)) && is_tc(&cb))
            || (is_tc(&ca) && matches!(&cb, Value::Atom(..)))
        {
            let ab = self.subset_lte(&ca, &cb, ctx);
            let ba = self.subset_lte(&cb, &ca, ctx);
            return bool_tag(match op {
                CmpOp::Lte => ab,
                CmpOp::Gte => ba,
                CmpOp::Lt => ab && !ba,
                CmpOp::Gt => ba && !ab,
                CmpOp::Eq | CmpOp::Ne => unreachable!(),
            });
        }

        // W3: non-atom order via the same meet reduction (combo / union /
        // mixed). Bidirectional meets independent; proper = lte ∧ ¬gte.
        let ab = self.subset_lte(&ca, &cb, ctx);
        let ba = self.subset_lte(&cb, &ca, ctx);
        bool_tag(match op {
            CmpOp::Lte => ab,
            CmpOp::Gte => ba,
            CmpOp::Lt => ab && !ba,
            CmpOp::Gt => ba && !ab,
            CmpOp::Eq | CmpOp::Ne => unreachable!(),
        })
    }
}
