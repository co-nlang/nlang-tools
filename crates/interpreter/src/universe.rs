use crate::value::{
    default_cache_id, AuthorityInfo, BottomCause, ComboVal, Commit, CommitKind, ContentHash,
    EffectTag, RefineInfo, Value,
};
use crate::EvalContext;
use crate::Ouroboros;
use anyhow::Result;
use indexmap::IndexMap;
use nlang_parser::ast::{AtomKind, Expr, ExprKind, Field, FieldKey, Path, PathAnchor, Prefix};

/// Top-level `...X` is the same operator Combo construction already knows:
/// a quoted key `"..."` whose value is the source (usually `ExprKind::Spread`).
/// `Universe::evolve` must flatten it into staged rather than store a
/// coordinate literally named `"..."` (Q-036 / O72).
fn is_spread_field(field: &Field) -> bool {
    matches!(&field.key, FieldKey::Quoted(name) if name == "...")
}

/// Coordinate names a field key will occupy in staged (prefixed + bare).
fn field_coords(key: &FieldKey) -> Vec<String> {
    match key {
        FieldKey::Named { name, prefix } => {
            let is_p = matches!(prefix, Some(Prefix::Private) | Some(Prefix::Local));
            let trimmed = name.trim().to_string();
            if is_p {
                vec![trimmed]
            } else {
                let p = match prefix {
                    Some(Prefix::Logic) => "/",
                    Some(Prefix::Type) => "@",
                    Some(Prefix::Meta) => "%",
                    Some(Prefix::System) => "~%",
                    _ => "",
                };
                let stored = format!("{}{}", p, trimmed);
                if stored == trimmed {
                    vec![stored]
                } else {
                    vec![stored, trimmed]
                }
            }
        }
        FieldKey::Quoted(name) => vec![name.trim().to_string()],
        FieldKey::Path(p) if p.segments.len() == 1 && p.anchor == PathAnchor::Bare => {
            vec![p.segments[0].trim().to_string()]
        }
        FieldKey::Path(p) if p.anchor == PathAnchor::Bare && p.segments.len() == 2 => {
            // ~%Config.fuel form — root module + bare field.
            vec![
                p.segments[0].trim().to_string(),
                format!("{}.{}", p.segments[0].trim(), p.segments[1].trim()),
            ]
        }
        _ => vec![],
    }
}

/// Root `~%Config.<bare>` horizon-parameter family (SPEC_08 §3.1) — write exempt.
fn is_root_config_field_write(key: &FieldKey) -> bool {
    match key {
        FieldKey::Path(p)
            if p.anchor == PathAnchor::Bare
                && p.segments.len() == 2
                && p.segments[0].trim() == "~%Config" =>
        {
            let field = p.segments[1].trim();
            !field.is_empty() && !field.contains('%') && !field.starts_with('~')
        }
        _ => false,
    }
}

/// SPEC_09 §6 closed knob table — non-negative Int knobs.
const CONFIG_INT_KNOBS: &[&str] = &[
    "fuel",
    "timeout",
    "max_branches",
    "max_unification_depth",
    "max_lifting_depth",
    "max_pattern_nodes",
];

fn is_known_config_knob(name: &str) -> bool {
    CONFIG_INT_KNOBS.contains(&name) || name == "strategy"
}

/// Knobs that may take `#_` (order supremum / unbound). Criterion (O41):
/// lifting the bound must leave another bound on every path the knob governs.
const CONFIG_UNLIMITED_OK: &[&str] = &["timeout", "max_branches", "max_pattern_nodes"];

/// Validate a root `~%Config.<bare>` RHS after evaluation (SPEC_09 §6).
/// Unknown name / wrong type / ⊥ / Top → `InvalidConfig` (evolve-loud;
/// never stored as a node-level ⊥).
fn validate_config_knob_value(name: &str, val: &Value) -> Result<(), BottomCause> {
    if !is_known_config_knob(name) {
        return Err(BottomCause::InvalidConfig);
    }
    // Collapse pure-wrappers / force residues — validation is on the value.
    let v = val.collapse();
    if matches!(v, Value::Top | Value::TopCaused { .. } | Value::Bottom(_)) {
        return Err(BottomCause::InvalidConfig);
    }
    if name == "strategy" {
        return match v {
            Value::Atom(AtomKind::Tag(s), _, _) => match s.trim_start_matches('#') {
                "blur" | "strict" | "approximate" => Ok(()),
                _ => Err(BottomCause::InvalidConfig),
            },
            _ => Err(BottomCause::InvalidConfig),
        };
    }
    // O41: `#_` is AtomKind::TagEnd (order supremum, SPEC_01 §2.6), not
    // Tag("_") and not lattice Top `_` (AtomKind::Top) — the WARNING table
    // is exactly this distinction. Allowed only where lifting leaves another
    // bound on every path the knob governs.
    if matches!(v, Value::Atom(AtomKind::TagEnd, _, _)) {
        return if CONFIG_UNLIMITED_OK.contains(&name) {
            Ok(())
        } else {
            Err(BottomCause::InvalidConfig)
        };
    }
    // Non-negative integer knobs.
    match v {
        Value::Atom(AtomKind::Int(n), _, _) if *n >= 0i64.into() => Ok(()),
        _ => Err(BottomCause::InvalidConfig),
    }
}

/// True when staged holds anything other than horizon knobs (O37/W4‴).
/// A stage of only `~%Config` is session state, not a commit payload.
pub fn staged_has_committable_content(staged: &ComboVal) -> bool {
    let mut s = staged.clone();
    s.remove_field("~%Config");
    !s.data.is_empty()
        || !s.types.is_empty()
        || !s.rules.is_empty()
        || !s.meta.is_empty()
        || !s.system.is_empty()
        || !s.local.is_empty()
}

/// Genesis ∧ staged overrides — effective closed config (display + resolve).
pub(crate) fn effective_config(
    root: &ComboVal,
    standard_root: &ComboVal,
    staged: Option<&ComboVal>,
) -> Option<ComboVal> {
    let mut base = match root
        .get_field("~%Config")
        .or_else(|| standard_root.get_field("~%Config"))
    {
        Some(Value::Combo(c)) => c.clone(),
        _ => return None,
    };
    if let Some(s) = staged {
        if let Some(Value::Combo(over)) = s.get_field("~%Config") {
            for (k, v) in over.fields() {
                base.insert_field(&k, v);
            }
        }
    }
    base.closed = true;
    Some(base)
}

fn standard_for_root(engine: &Ouroboros, root: &ContentHash) -> Result<ComboVal> {
    match engine.store.root_standard_digest(root)? {
        Some(digest) => engine
            .standard_roots
            .get(&digest)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("root names unavailable standard root {digest}")),
        // Formats 1/2 were self-contained; keeping the standard layer empty
        // preserves that shape rather than adding today's library to history.
        // Dispatch still has a credential: EvalContext::with_standard_root
        // treats a blank layer as "use the user root's ~% / rules axes".
        None => Ok(ComboVal::default()),
    }
}

/// User LHS write to engine-minted `~%` axis (ownership; not Config.bare).
fn is_system_axis_lhs_forbidden(key: &FieldKey) -> bool {
    if is_root_config_field_write(key) {
        return false;
    }
    match key {
        FieldKey::Named {
            prefix: Some(Prefix::System),
            ..
        } => true,
        FieldKey::Path(p)
            if p.anchor == PathAnchor::Bare
                && !p.segments.is_empty()
                && p.segments[0].trim().starts_with("~%") =>
        {
            true
        }
        _ => false,
    }
}

/// After evolve-time eval, a `#pure` derived answer is not what injection
/// stores (D46 (i) / S3). Eval still runs so evolve-boundary reports stay
/// loud (Config types, `#divergent`, resource horizons). The recipe is
/// what goes into staged; the answer is discarded.
fn recipe_instead_of_pure_answer(
    engine: &Ouroboros,
    expr: &Expr,
    val: Value,
    ctx: &crate::EvalContext,
) -> Value {
    // S3 as measured: `top: 1 + 2` must not land as `3`. Arithmetic is the
    // evolve-time solidification. Other expr kinds keep their eval result
    // (live Refs, `%effect` reads, applications that already ran).
    if !matches!(
        expr.kind,
        ExprKind::Add(_, _)
            | ExprKind::Sub(_, _)
            | ExprKind::Mul(_, _)
            | ExprKind::Div(_, _)
            | ExprKind::Rem(_, _)
            | ExprKind::Eq(_, _)
            | ExprKind::Ne(_, _)
            | ExprKind::Lt(_, _)
            | ExprKind::Gt(_, _)
            | ExprKind::Lte(_, _)
            | ExprKind::Gte(_, _)
    ) {
        return val;
    }
    match &val {
        Value::Bottom(_) | Value::Blur(_) | Value::Thunk { .. } => val,
        Value::Top | Value::TopCaused { .. } | Value::Ref(_) => val,
        _ if val.effect().is_pure() => Value::Thunk {
            expr: Box::new(expr.clone()),
            closure: ctx.scopes.clone(),
            context: ctx.context_value.clone().map(Box::new),
            effect: engine.predict_effect(expr, ctx),
        },
        _ => val,
    }
}

fn refresh_combo_thunks(engine: &Ouroboros, ctx: &crate::EvalContext, c: &mut ComboVal) {
    for v in c
        .data
        .values_mut()
        .chain(c.types.values_mut())
        .chain(c.rules.values_mut())
        .chain(c.meta.values_mut())
        .chain(c.system.values_mut())
        .chain(c.local.values_mut())
    {
        refresh_thunk_effects(engine, ctx, v);
    }
    for v in &mut c.pending_spreads {
        refresh_thunk_effects(engine, ctx, v);
    }
}

fn refresh_thunk_effects(engine: &Ouroboros, ctx: &crate::EvalContext, val: &mut Value) {
    match val {
        Value::Thunk { expr, effect, .. } => {
            *effect = engine.predict_effect(expr, ctx);
        }
        Value::Combo(c) => {
            for v in c
                .data
                .values_mut()
                .chain(c.types.values_mut())
                .chain(c.rules.values_mut())
                .chain(c.meta.values_mut())
                .chain(c.system.values_mut())
                .chain(c.local.values_mut())
            {
                refresh_thunk_effects(engine, ctx, v);
            }
            for v in &mut c.pending_spreads {
                refresh_thunk_effects(engine, ctx, v);
            }
        }
        Value::Union(branches) => {
            for b in branches {
                refresh_thunk_effects(engine, ctx, b);
            }
        }
        Value::Range { start, end, step } => {
            refresh_thunk_effects(engine, ctx, start);
            refresh_thunk_effects(engine, ctx, end);
            if let Some(s) = step {
                refresh_thunk_effects(engine, ctx, s);
            }
        }
        _ => {}
    }
}

/// True only for the literal open-world hole `_` (not e.g. `a + 1` → Top).
fn is_literal_top(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Atom(AtomKind::Top) => true,
        ExprKind::Path(p) => {
            p.anchor == PathAnchor::Bare && p.segments.len() == 1 && p.segments[0].trim() == "_"
        }
        _ => false,
    }
}

pub struct Universe {
    pub head: Option<ContentHash>,
    pub root: ComboVal,
    /// The table named by the loaded root.  New universes keep it separate
    /// from user content from their first write onward.
    pub standard_root: ComboVal,
    pub staged: ComboVal,
    pub is_dirty: bool,
    /// Request flag for this evolve session (`oo evolve --pin`). Capability
    /// alone must not set this — two-step like runPure.
    pub pin_mode: bool,
    /// Staged under pin; next commit is `CommitKind::Pin` with replace-merge.
    /// Persisted beside staged so evolve/commit stay separate CLI processes.
    pub pin_pending: bool,
    /// ACCEPTANCE REPAIR: exactly which coordinates were written under `--pin`.
    /// Replace-merge at commit applies to THESE ONLY; every other staged
    /// coordinate still meets the root normally. Without this, a pin anywhere
    /// in a staging session silently gave replace semantics to every ordinary
    /// write in the same commit — a privileged operation changing the meaning
    /// of unprivileged ones (work order §3 C.3: pin acts only on its own
    /// fields).
    pub pin_coords: std::collections::BTreeSet<String>,
    /// Intent: the active effect tags a `runPure` in the staged content
    /// actually DISCHARGED. Persisted as `.oo/effect_pending` (assertion
    /// layer, same home as `pin_pending`). `None` = no discharge.
    /// **Not authorization** — commit must re-present a capability that
    /// COVERS these tags (SPEC_08 §6.2 授權時點 / 意圖≠授權).
    ///
    /// ACCEPTOR REPAIR: was a `bool`, which cannot express what the commit
    /// gate has to check.
    pub effect_pending: Option<crate::value::EffectTag>,
    /// Incoming combo(s) of this CLI process, not the fold. Written as one
    /// immutable injection at `save_staged`. Empty unless an evolve succeeded.
    session_delta: ComboVal,
    session_has_delta: bool,
    /// D49: the on-disk injection set jointly meets to ⊥. The files stay;
    /// this is the leaf the fold reported. Not a ComboVal — ⊥ is not a combo.
    pub workset_bottom: Option<crate::value::BottomDetail>,
}
impl Universe {
    pub fn new(head: Option<ContentHash>, root: ComboVal) -> Self {
        Self::new_with_standard(head, root, ComboVal::default())
    }

    pub fn new_with_standard(
        head: Option<ContentHash>,
        root: ComboVal,
        standard_root: ComboVal,
    ) -> Self {
        Self {
            head,
            root,
            standard_root,
            staged: ComboVal::default(),
            is_dirty: false,
            pin_mode: false,
            pin_pending: false,
            pin_coords: std::collections::BTreeSet::new(),
            effect_pending: None,
            session_delta: ComboVal::default(),
            session_has_delta: false,
            workset_bottom: None,
        }
    }

    /// Commit body under `#pin`: pinned coordinates OVERWRITE the root; all
    /// other staged coordinates take the ordinary lattice meet. Returns `None`
    /// if the ordinary part conflicts (that path must still fail loudly —
    /// privilege was granted for the pinned coordinates, not for the rest).
    fn pin_commit_merge(
        engine: &Ouroboros,
        root: &ComboVal,
        staged: &ComboVal,
        pin_coords: &std::collections::BTreeSet<String>,
    ) -> Option<ComboVal> {
        // 1. ordinary part = staged minus the pinned coordinates
        let mut ordinary = staged.clone();
        for c in pin_coords {
            ordinary.remove_field(c);
            ordinary.local.shift_remove(c.as_str());
        }
        let met = match engine.unify(Value::Combo(root.clone()), Value::Combo(ordinary)) {
            Value::Combo(m) => m,
            _ => return None,
        };
        // 2. pinned coordinates overwrite on top
        let mut out = met;
        for c in pin_coords {
            if let Some(v) = staged.get_field(c).cloned() {
                out.insert_field(c, v);
            } else if let Some(v) = staged.get_local_field(c).cloned() {
                out.local.insert(c.clone(), v);
            }
        }
        out.effect = out.effect.union(staged.effect);
        Some(out)
    }

    /// Overlay `incoming` fields onto `base` by replace (not lattice meet).
    /// Used by `#pin` so incompatible rebinding lands rather than ⊥.
    fn replace_merge(base: &ComboVal, incoming: &ComboVal) -> ComboVal {
        let mut out = base.clone();
        for (k, v) in incoming.all_fields_iter() {
            out.insert_field(&k, v);
        }
        for (k, v) in &incoming.local {
            out.local.insert(k.clone(), v.clone());
        }
        // Carry effect union conservatively (does not taint CAID of values).
        out.effect = out.effect.union(incoming.effect);
        out
    }
    pub fn load(engine: &Ouroboros, base_dir: &std::path::Path) -> Result<Self> {
        engine.clear_force_memo();
        let head = engine.store.get_head(base_dir)?;
        match head {
            Some(h) => {
                let commit = engine.store.get_commit(&h)?;
                let root = engine
                    .store
                    .get_root(&commit.root, &engine.standard_roots)?;
                let standard_root = standard_for_root(engine, &commit.root)?;
                Ok(Self::new_with_standard(Some(h), root, standard_root))
            }
            None => Ok(Self::new_with_standard(
                None,
                ComboVal::default(),
                engine.root_with_system(),
            )),
        }
    }

    fn note_session_incoming(&mut self, engine: &Ouroboros, incoming: &ComboVal) {
        if !self.session_has_delta {
            self.session_delta = incoming.clone();
            self.session_has_delta = true;
            return;
        }
        if self.pin_mode && engine.privilege.pin {
            self.session_delta = Self::replace_merge(&self.session_delta, incoming);
            return;
        }
        if let Value::Combo(m) = engine.unify(
            Value::Combo(self.session_delta.clone()),
            Value::Combo(incoming.clone()),
        ) {
            self.session_delta = m;
        }
    }

    pub fn evolve(
        &mut self,
        engine: &Ouroboros,
        field: &Field,
    ) -> std::result::Result<(), crate::value::BottomDetail> {
        if let Some(d) = &self.workset_bottom {
            return Err(d.clone());
        }
        // SPEC_09 ownership: user LHS on `~%` is illegal (except root
        // ~%Config.<bare> horizon family). Loud at evolve boundary — same
        // family as G2-S Evolution Conflict (CLI exit 1).
        // BottomDetail (not bare BottomCause) so conflict coordinates survive
        // the boundary (where_the_conflict_is / W3'-a).
        if is_system_axis_lhs_forbidden(&field.key) {
            return Err(crate::value::BottomDetail {
                cause: BottomCause::SystemReserved,
                ..Default::default()
            });
        }

        let mut ctx =
            EvalContext::new(self.root.clone()).with_standard_root(self.standard_root.clone());
        ctx.staged = Some(self.staged.clone());
        // O42: no clock salt — blur identity is CHS (budgets + partial).
        // EvalContext keeps a fixed disc tie-break salt from ::new only.
        ctx.privilege = engine.privilege;
        // User-staged horizon knobs govern evolve-time evaluation too. A field
        // may complete a pipe, application, merge, or cocoon force before the
        // final observe call; leaving fuel out here made that real work free at
        // the only boundary which can still represent it. The fixed horizon
        // parameters flow into any resulting blur's CHS just as they do at
        // observe (REAL_01 §9 / meter_reads_two).
        if let Some(Value::Combo(ref staged_cfg)) = self.staged.get_field("~%Config") {
            if let Some(eff) = effective_config(&self.root, &self.standard_root, Some(&self.staged))
            {
                let apply_timeout = staged_cfg.get_field("timeout").is_some();
                ctx.apply_horizon_config(&eff, true, apply_timeout);
            }
        }
        // forward_spread acceptance repair: cocoon literals force_recursive
        // at construction (GUIDE_03 §11.5) — during evolve a forward source
        // is open-miss Top only because it is not defined YET; mark the
        // phase so expansion re-queues instead of consuming (cocoon face:
        // {{...later, b: 1}} with later defined below).
        ctx.in_evolve = true;

        if is_spread_field(field) {
            return self.evolve_spread(engine, field, ctx);
        }

        // Coordinate(s) this field will occupy — marked in-flight during eval
        // so self-ref (`a: a + 1`) is ⊥ #divergent, not open-miss Top (L2-17).
        let coords = field_coords(&field.key);
        for c in &coords {
            ctx.computing.insert(c.clone());
        }
        let mut val = engine.eval(&field.value, &mut ctx);
        for c in &coords {
            ctx.computing.remove(c);
        }

        // Forward / mutual refs evaluate to open-miss Top under sequential
        // evolve (ctx.root is the *committed* root; staged siblings are not
        // visible yet). Reify as a Thunk so later observation can force once
        // both bindings exist — and so mutual cycles hit force_coord /
        // in_flight. Ruling C: open-miss is now caused Top `#no_coordinate`
        // (diagnostic), not bare Top — both faces are forward-miss signals
        // here. Static-cycle TopCaused stays concrete (real answer). Literal
        // `_` stays bare Top (open-world hole). Stage 3 live-Ref results are
        // concrete (not Top) and pass through unchanged.
        let is_forward_open_miss = matches!(val, Value::Top)
            || matches!(
                &val,
                Value::TopCaused { cause, .. } if cause == "no_coordinate"
            );
        if is_forward_open_miss && !is_literal_top(&field.value) {
            val = Value::Thunk {
                expr: Box::new(field.value.clone()),
                closure: ctx.scopes.clone(),
                context: ctx.context_value.clone().map(Box::new),
                effect: engine.predict_effect(&field.value, &ctx),
            };
        } else if !is_root_config_field_write(&field.key) {
            val = recipe_instead_of_pure_answer(engine, &field.value, val, &ctx);
        }
        let val_effect = val.effect();

        let mut rf = IndexMap::new();
        let mut quoted_data: Option<(String, Value)> = None;
        let mut rl = IndexMap::new();
        // Stage 5 (§5b): collect field keys for per-coordinate invalidation.
        let mut evolved_coords: Vec<String> = Vec::new();

        // Resolve write coordinates first (needed for G2-S root check before
        // val is moved into the incoming combo).
        match &field.key {
            FieldKey::Named { name, prefix } => {
                let is_p = matches!(prefix, Some(Prefix::Private) | Some(Prefix::Local));
                let trimmed = name.trim().to_string();
                if is_p {
                    evolved_coords.push(trimmed);
                } else {
                    let p = match prefix {
                        Some(Prefix::Logic) => "/",
                        Some(Prefix::Type) => "@",
                        Some(Prefix::Meta) => "%",
                        Some(Prefix::System) => "~%",
                        _ => "",
                    };
                    let stored = format!("{}{}", p, trimmed);
                    // Stage 5 acceptance fix: dependency recording uses stored
                    // (prefixed) names, so invalidate by BOTH forms.
                    evolved_coords.push(stored);
                    evolved_coords.push(trimmed);
                }
            }
            FieldKey::Quoted(name) => {
                evolved_coords.push(name.trim().to_string());
            }
            FieldKey::Path(p) if p.segments.len() == 1 && p.anchor == PathAnchor::Bare => {
                evolved_coords.push(p.segments[0].trim().to_string());
            }
            // Root ~%Config.<bare>: stage an OPEN partial override. Lattice
            // meet cannot overwrite fuel 10000 with 50; observe overlays
            // staged Config fields onto the genesis module before unify.
            // SPEC_09 §6: closed knob family — name + evaluated type loud
            // at evolve (`InvalidConfig`); never silent accept.
            FieldKey::Path(p) if is_root_config_field_write(&field.key) => {
                let bare = p.segments[1].trim().to_string();
                // Name membership before type check (unknown never stages).
                if !is_known_config_knob(&bare) {
                    return Err(crate::value::BottomDetail {
                        cause: BottomCause::InvalidConfig,
                        ..Default::default()
                    });
                }
                // Type after eval — `fuel: 40 + 10` is lawful 50.
                validate_config_knob_value(&bare, &val).map_err(|c| {
                    crate::value::BottomDetail {
                        cause: c,
                        ..Default::default()
                    }
                })?;
                evolved_coords.push("~%Config".to_string());
                evolved_coords.push(format!("~%Config.{}", bare));
                let mut partial = match self.staged.get_field("~%Config").cloned() {
                    Some(Value::Combo(c)) => c,
                    _ => ComboVal::new(
                        IndexMap::new(),
                        false,
                        IndexMap::new(),
                        EffectTag::Pure,
                        vec![],
                    ),
                };
                partial.closed = false;
                partial.insert_field(&bare, val);
                rf.insert("~%Config".to_string(), Value::Combo(partial));
                let incoming_combo = ComboVal::new(rf, false, rl, val_effect, vec![]);
                if !evolved_coords.is_empty() {
                    engine.invalidate_coords(&evolved_coords);
                }
                // Merge open partials only (no root Config in the meet).
                let res = engine.unify(
                    Value::Combo(self.staged.clone()),
                    Value::Combo(incoming_combo.clone()),
                );
                return match res {
                    Value::Combo(m) => {
                        self.note_session_incoming(engine, &incoming_combo);
                        self.staged = m;
                        self.is_dirty = true;
                        self.restamp_thunk_effects(engine);
                        Ok(())
                    }
                    Value::Bottom(d) => Err(*d),
                    _ => Err(crate::value::BottomDetail {
                        cause: BottomCause::Conflict,
                        ..Default::default()
                    }),
                };
            }
            _ => {
                self.is_dirty = true;
                return Ok(());
            }
        };

        // G2-S: root coordinates evolve monotonically. If the incoming value
        // conflicts with an existing ROOT binding at any written coordinate,
        // fail at the evolve boundary (loud Evolution Conflict) instead of
        // poisoning the whole universe at observe-entry unify(root, staged).
        // Staged×staged conflicts stay on the existing unify path below.
        // `#pin` (SPEC_08 §6.2): privileged exception — skip the monotone check
        // when both request (`pin_mode`) and capability are present. Capability
        // alone never reaches here with pin_mode (CLI two-step gate).
        if !(self.pin_mode && engine.privilege.pin) {
            for c in &evolved_coords {
                if let Some(root_val) = self
                    .root
                    .get_field(c)
                    .cloned()
                    .or_else(|| self.root.get_local_field(c).cloned())
                {
                    if let Value::Bottom(mut d) = engine.unify(root_val, val.clone()) {
                        // unify ran on the *field value*, so `path` is relative
                        // to that coordinate. Absolute-ise for the operator
                        // (where_the_conflict_is R1/R3): `app` + `db.opts…`
                        // → `app.db.opts…`. Do not use f.key again at the CLI.
                        let leaf = d.path.take().filter(|s| !s.is_empty());
                        d.path = Some(match leaf {
                            Some(p) if p == *c || p.starts_with(&format!("{c}.")) => p,
                            Some(p) => format!("{c}.{p}"),
                            None => c.clone(),
                        });
                        return Err(*d);
                    }
                }
            }
        }

        match &field.key {
            FieldKey::Named { name, prefix } => {
                let is_p = matches!(prefix, Some(Prefix::Private) | Some(Prefix::Local));
                let trimmed = name.trim().to_string();
                if is_p {
                    rl.insert(trimmed, val);
                } else {
                    let p = match prefix {
                        Some(Prefix::Logic) => "/",
                        Some(Prefix::Type) => "@",
                        Some(Prefix::Meta) => "%",
                        Some(Prefix::System) => "~%",
                        _ => "",
                    };
                    let stored = format!("{}{}", p, trimmed);
                    rf.insert(stored, val);
                }
            }
            FieldKey::Quoted(name) => {
                quoted_data = Some((name.trim().to_string(), val));
            }
            FieldKey::Path(p) if p.segments.len() == 1 && p.anchor == PathAnchor::Bare => {
                rf.insert(p.segments[0].trim().to_string(), val);
            }
            _ => unreachable!("coords already filtered non-writable keys"),
        };

        let mut incoming = ComboVal::new(rf, false, rl, val_effect, vec![]);
        if let Some((k, v)) = quoted_data {
            incoming.data.insert(k, v);
        }
        // Stage 5 (§5b): invalidate memo entries that depend on the evolved
        // coordinates. Called before the merge succeeds so entries reading
        // staged values are cleared.
        if !evolved_coords.is_empty() {
            engine.invalidate_coords(&evolved_coords);
        }
        // `#pin`: overwrite into staged (replace), not lattice meet — meet of
        // staged-vs-incoming would re-⊥ an earlier incompatible pin, and
        // staged-vs-root conflict is handled only at commit (also replace).
        // Capture discharge intent from this field's evaluation: the tags a
        // runPure actually overrode (Pure = nothing happened).
        let discharged = engine.take_privileged_discharge();
        if !discharged.is_pure() {
            self.effect_pending = Some(
                self.effect_pending
                    .unwrap_or(crate::value::EffectTag::Pure)
                    .union(discharged),
            );
        }
        if self.pin_mode && engine.privilege.pin {
            self.note_session_incoming(engine, &incoming);
            self.staged = Self::replace_merge(&self.staged, &incoming);
            self.is_dirty = true;
            self.pin_pending = true;
            // ACCEPTANCE REPAIR: remember WHICH coordinates were pinned, so the
            // commit overwrites only these and every other staged coordinate
            // still meets normally.
            for c in &evolved_coords {
                self.pin_coords.insert(c.clone());
            }
            self.restamp_thunk_effects(engine);
            return Ok(());
        }
        let res = engine.unify(
            Value::Combo(self.staged.clone()),
            Value::Combo(incoming.clone()),
        );
        match res {
            Value::Combo(m) => {
                self.note_session_incoming(engine, &incoming);
                self.staged = m;
                self.is_dirty = true;
                self.restamp_thunk_effects(engine);
                Ok(())
            }
            Value::Bottom(d) => Err(*d),
            _ => Err(crate::value::BottomDetail {
                cause: BottomCause::Conflict,
                ..Default::default()
            }),
        }
    }

    /// Flatten a top-level `...X` into staged with the same laws Combo
    /// construction already applies (intersect on collision, private
    /// exclusion, Top no-op, ⊥ collapse). Nested spread is untouched.
    fn evolve_spread(
        &mut self,
        engine: &Ouroboros,
        field: &Field,
        mut ctx: EvalContext,
    ) -> std::result::Result<(), crate::value::BottomDetail> {
        let te = engine.predict_effect(&field.value, &mut ctx);
        let mut holder = ComboVal::default();
        holder.pending_spreads.push(Value::Thunk {
            expr: Box::new(field.value.clone()),
            closure: ctx.scopes.clone(),
            context: ctx.context_value.clone().map(Box::new),
            effect: te,
        });
        match engine.expand_combo_pending(holder, &mut ctx) {
            Value::Combo(incoming) => {
                let evolved_coords: Vec<String> = incoming.field_keys();
                if !evolved_coords.is_empty() {
                    engine.invalidate_coords(&evolved_coords);
                }
                let discharged = engine.take_privileged_discharge();
                if !discharged.is_pure() {
                    self.effect_pending = Some(
                        self.effect_pending
                            .unwrap_or(crate::value::EffectTag::Pure)
                            .union(discharged),
                    );
                }
                if self.pin_mode && engine.privilege.pin {
                    self.note_session_incoming(engine, &incoming);
                    self.staged = Self::replace_merge(&self.staged, &incoming);
                    self.is_dirty = true;
                    self.pin_pending = true;
                    for c in &evolved_coords {
                        self.pin_coords.insert(c.clone());
                    }
                    self.restamp_thunk_effects(engine);
                    return Ok(());
                }
                // Same field lattice as Combo construction / expand_combo_pending:
                // a colliding key is the meet (⊥ when they disagree). Must not
                // unify the whole staged combo — that path promotes a field
                // clash to Evolution Conflict and drops every other member.
                self.note_session_incoming(engine, &incoming);
                for (k, v) in incoming.fields() {
                    if let Some(existing) = self.staged.get_field(&k).cloned() {
                        let merged = engine.unify_internal(existing, v, &mut ctx);
                        self.staged.insert_field(&k, merged);
                    } else {
                        self.staged.insert_field(&k, v);
                    }
                }
                for (k, v) in incoming.local_fields() {
                    let bare = k.trim().trim_start_matches('~').to_string();
                    if let Some(existing) = self.staged.local.get(&bare).cloned() {
                        let merged = engine.unify_internal(existing, v, &mut ctx);
                        self.staged.local.insert(bare, merged);
                    } else {
                        self.staged.local.insert(bare, v);
                    }
                }
                if !self.staged.closed {
                    self.staged.effect = self.staged.effect.union(incoming.effect);
                }
                self.staged.pending_spreads.extend(incoming.pending_spreads);
                self.is_dirty = true;
                self.restamp_thunk_effects(engine);
                Ok(())
            }
            Value::Bottom(d) => Err(*d),
            // Nested blur-spread absorbs the target combo. Staged is a
            // ComboVal face, not a Blur; absorbing the universe would be a
            // new lookup/overlay rule. Collision, not overwrite.
            Value::Blur(_) => Err(crate::value::BottomDetail {
                cause: BottomCause::Conflict,
                ..Default::default()
            }),
            _ => {
                self.is_dirty = true;
                Ok(())
            }
        }
    }

    fn restamp_thunk_effects(&mut self, engine: &Ouroboros) {
        let mut ctx = crate::EvalContext::new(self.root.clone())
            .with_standard_root(self.standard_root.clone());
        ctx.staged = Some(self.staged.clone());
        refresh_combo_thunks(engine, &ctx, &mut self.staged);
        if self.session_has_delta {
            let mut next = ComboVal::default();
            next.effect = self.session_delta.effect;
            for (k, _) in self.session_delta.all_fields_iter() {
                if let Some(v) = self.staged.get_field(&k).cloned() {
                    next.insert_field(&k, v);
                } else if let Some(v) = self.staged.get_local_field(&k).cloned() {
                    next.local.insert(k, v);
                }
            }
            self.session_delta = next;
        }
    }

    fn unlink_legacy_staged(base_dir: &std::path::Path) {
        let staged_path = base_dir.join(".oo").join("staged");
        if staged_path.exists() {
            let _ = std::fs::remove_file(staged_path);
        }
    }

    pub fn save_staged(&mut self, engine: &Ouroboros, base_dir: &std::path::Path) -> Result<()> {
        // O42 11.6.1 (i): write blur partial bodies into CAS before the
        // injection drops them (partial is CAID-only on disk). Uncommitted
        // bodies are unreachable after the next commit of a different root
        // and are local-GC fodder — same class as auto-cleared adverts.
        Value::Combo(self.staged.clone()).persist_blur_partials(&engine.store)?;
        if self.session_has_delta {
            Value::Combo(self.session_delta.clone()).persist_blur_partials(&engine.store)?;
            crate::injections::write(base_dir, &self.session_delta)?;
            self.session_delta = ComboVal::default();
            self.session_has_delta = false;
        }
        Self::unlink_legacy_staged(base_dir);
        // ○ lives beside the working set, not in CAS. Unchanged this arc
        // (D48 split: Q-014b owns identity and order). Identical bodies do
        // not mint a new ○ (D47).
        crate::savepoint::record(base_dir, &self.staged)?;
        // Pin audit intent lives beside staged, never inside values (CAID).
        // ACCEPTANCE REPAIR: the file now carries the pinned COORDINATES, not
        // a bare flag — the commit must know which coordinates the privilege
        // covers, or it applies replace semantics to everything staged.
        let pin_path = base_dir.join(".oo").join("pin_pending");
        if self.pin_pending {
            let coords: Vec<&String> = self.pin_coords.iter().collect();
            crate::storage::atomic_write(&pin_path, serde_json::to_string(&coords)?)?;
        } else if pin_path.exists() {
            let _ = std::fs::remove_file(pin_path);
        }
        // Effect-discharge intent (SPEC_08 §6.2). Same strength as pin_pending:
        // intent only — commit must re-present the capability. Not writable
        // from the language layer (store boundary).
        let effect_path = base_dir.join(".oo").join("effect_pending");
        if let Some(tags) = self.effect_pending {
            // The TAG SET, not a bare marker: commit must be able to check
            // that the capability re-presented covers what was discharged.
            crate::storage::atomic_write(&effect_path, tags.to_bits().to_string().as_bytes())?;
        } else if effect_path.exists() {
            let _ = std::fs::remove_file(effect_path);
        }
        Ok(())
    }

    pub fn load_staged(&mut self, engine: &Ouroboros, base_dir: &std::path::Path) -> Result<()> {
        self.workset_bottom = None;
        self.session_delta = ComboVal::default();
        self.session_has_delta = false;
        let pin_path = base_dir.join(".oo").join("pin_pending");
        self.pin_pending = pin_path.exists();
        if self.pin_pending {
            // ACCEPTANCE REPAIR: restore the pinned coordinate set. An
            // unreadable/legacy file means "pinned, coordinates unknown" — the
            // safe reading is the EMPTY set (no coordinate gets replace
            // semantics), never "all of them".
            self.pin_coords = std::fs::read_to_string(&pin_path)
                .ok()
                .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
                .map(|v| v.into_iter().collect())
                .unwrap_or_default();
        }
        let injections = crate::injections::load_all(base_dir)?;
        if !injections.is_empty() {
            if self.pin_pending {
                // Pin is replace, not meet (SPEC_08 §6.2). Q-016 owns the
                // order of concurrent pins; sequential pins of one coordinate
                // must still land, or the pin sidecar itself is never rewritten.
                let mut acc = ComboVal::default();
                for c in injections {
                    acc = Self::replace_merge(&acc, &c);
                }
                self.staged = acc;
                self.restamp_thunk_effects(engine);
                self.is_dirty = true;
            } else {
                match crate::injections::fold(engine, injections) {
                    Ok(combo) => {
                        self.staged = combo;
                        self.restamp_thunk_effects(engine);
                        self.is_dirty = staged_has_committable_content(&self.staged)
                            || self.staged.get_field("~%Config").is_some();
                    }
                    Err(d) => {
                        self.workset_bottom = Some(d);
                        self.staged = ComboVal::default();
                        self.is_dirty = true;
                    }
                }
            }
        } else {
            let staged_path = base_dir.join(".oo").join("staged");
            if staged_path.exists() {
                let json = std::fs::read_to_string(staged_path)?;
                // O42 repair: blur carries partial as CAID only — staged stays
                // shallow; default serde recursion limit is correct again.
                self.staged = if crate::store_codec::is_framed(&json) {
                    crate::store_codec::decode_staged(&json)?
                } else {
                    serde_json::from_str(&json)?
                };
                self.is_dirty = true;
            }
        }
        self.effect_pending = std::fs::read_to_string(base_dir.join(".oo").join("effect_pending"))
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok())
            .map(crate::value::EffectTag::from_bits)
            .filter(|t| !t.is_pure());
        Ok(())
    }

    /// Outcome of a successful commit. `config_not_committed` is true when a
    /// staged `~%Config` was retained as session state (O37) and did not enter
    /// the committed root — the CLI must say so (never silent drop).
    pub fn commit(
        &mut self,
        engine: &Ouroboros,
        base_dir: &std::path::Path,
        meta: crate::value::CommitMeta,
    ) -> Result<(ContentHash, bool)> {
        engine.clear_force_memo();
        if let Some(d) = &self.workset_bottom {
            let coord = d.path.as_deref().filter(|s| !s.is_empty()).unwrap_or("");
            return Err(anyhow::anyhow!(
                "Evolution Conflict: {}{}",
                match d.cause {
                    BottomCause::Conflict => "#conflict",
                    _ => "bottom",
                },
                if coord.is_empty() {
                    String::new()
                } else {
                    format!(" at {coord}")
                }
            ));
        }
        // O37: horizon parameters do not enter history. Strip `~%Config` from
        // the commit meet so ordinary writes beside a knob still land; keep
        // the override in staging as session state after the commit.
        let retained_config = self.staged.get_field("~%Config").cloned();
        let mut staged_for_commit = self.staged.clone();
        if retained_config.is_some() {
            staged_for_commit.remove_field("~%Config");
        }
        let (new_root, kind) = if self.pin_pending {
            // ACCEPTANCE REPAIR: replace ONLY the pinned coordinates; everything
            // else staged still meets the root. Replacing the whole staged combo
            // let a pin on one coordinate silently give overwrite semantics to
            // ordinary writes sharing the commit (measured: a committed `y: 5`
            // was widened to `@int` by an unprivileged write, because `x` had
            // been pinned in the same session). Audit marker is CommitKind only
            // — the value structure is identical to a normal write of the same
            // payload, so its content hash is unchanged (§6.2).
            match Self::pin_commit_merge(engine, &self.root, &staged_for_commit, &self.pin_coords) {
                Some(m) => (m, CommitKind::Pin),
                None => return Err(anyhow::anyhow!("Commit failed")),
            }
        } else {
            match engine.unify(
                Value::Combo(self.root.clone()),
                Value::Combo(staged_for_commit),
            ) {
                Value::Combo(m) => (m, CommitKind::Standard),
                _ => return Err(anyhow::anyhow!("Commit failed")),
            }
        };
        // O35/O51: commit, not evolve, is the solidification boundary. The
        // staged workset remains lazy; history receives its observation.
        let mut commit_ctx = crate::EvalContext::new(new_root.clone())
            .with_standard_root(self.standard_root.clone());
        commit_ctx.memo_enabled = false;
        commit_ctx.preserve_refs = true;
        let parent_root = Value::Combo(self.root.clone());
        let new_root = match engine.project_for_commit(
            Value::Combo(new_root),
            Some(&parent_root),
            &mut commit_ctx,
        ) {
            Value::Combo(root) => root,
            _ => return Err(anyhow::anyhow!("Commit observation did not produce a root")),
        };
        // O63: the container encoding selects the root-address rule.  An
        // encoding-3 store keeps the standard table it already names; a new
        // encoding-4 store begins with this engine's current table.
        let standard = if engine.store.encoding_version() < 4 {
            self.standard_root.clone()
        } else {
            engine.root_with_system()
        };
        let root_hash = engine.store.put_root(&new_root, &standard)?;
        // R1: next commit after a rollback records the abandoned head(s) in
        // meta — never in values. Consumed from `.oo/abandoned` and cleared.
        let mut meta = meta;
        if meta.abandoned.is_none() {
            if let Some(abs) = Self::load_abandoned_file(base_dir) {
                if !abs.is_empty() {
                    meta.abandoned = Some(abs);
                }
            }
        }
        // SPEC_08 §6.2 `#privileged_effect`: mark only when a discharge fact
        // was staged (effect_pending), never merely because a grant was present.
        if self.effect_pending.is_some() {
            meta.privileged_effect = Some(true);
        }
        let mut commit = crate::value::Commit::new(None, root_hash, meta);
        commit.kind = kind;
        let commit_hash = engine.store.put_commit(&commit)?;
        engine.store.set_head(base_dir, &commit_hash)?;
        // D52/D55: put_commit → set_head → mint. Covering parent is the
        // workset tip just submitted (S2 / G5). Ancestor is HEAD's
        // commit-circle at mint time (`self.head` is still the previous
        // HEAD) — log/squash/gc walk that annotation, not `parents:`.
        let ancestor = self.head.as_ref().and_then(|h| {
            crate::savepoint::circle_id_for_commit(base_dir, &hex::encode(&h.digest))
                .ok()
                .flatten()
        });
        crate::savepoint::record_commit(base_dir, &commit_hash, None, ancestor.as_deref())?;
        self.root = new_root;
        self.standard_root = standard;
        // Workset injections are consumed. Config is session-scoped (O37):
        // write one Config-only injection back so a "clear the directory"
        // cannot drop the horizon (recon Q18, candidate 3).
        crate::injections::clear(base_dir)?;
        Self::unlink_legacy_staged(base_dir);
        self.session_delta = ComboVal::default();
        self.session_has_delta = false;
        if let Some(cfg) = retained_config {
            let mut restaged = ComboVal::default();
            restaged.insert_field("~%Config", cfg);
            self.staged = restaged.clone();
            self.is_dirty = true;
            self.session_delta = restaged;
            self.session_has_delta = true;
            self.save_staged(engine, base_dir)?;
        } else {
            self.staged = ComboVal::default();
            self.is_dirty = false;
        }
        let config_not_committed = self.staged.get_field("~%Config").is_some();
        self.head = Some(commit_hash.clone());
        self.pin_pending = false;
        self.pin_coords.clear();
        self.effect_pending = None;
        let pin_path = base_dir.join(".oo").join("pin_pending");
        if pin_path.exists() {
            let _ = std::fs::remove_file(pin_path);
        }
        let effect_path = base_dir.join(".oo").join("effect_pending");
        if effect_path.exists() {
            let _ = std::fs::remove_file(effect_path);
        }
        Self::clear_abandoned_file(base_dir);
        Ok((commit_hash, config_not_committed))
    }

    fn abandoned_path(base_dir: &std::path::Path) -> std::path::PathBuf {
        base_dir.join(".oo").join("abandoned")
    }

    fn load_abandoned_file(base_dir: &std::path::Path) -> Option<Vec<String>> {
        let p = Self::abandoned_path(base_dir);
        if !p.exists() {
            return None;
        }
        let s = std::fs::read_to_string(p).ok()?;
        let lines: Vec<String> = s
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();
        if lines.is_empty() {
            None
        } else {
            Some(lines)
        }
    }

    fn clear_abandoned_file(base_dir: &std::path::Path) {
        let p = Self::abandoned_path(base_dir);
        if p.exists() {
            let _ = std::fs::remove_file(p);
        }
    }

    fn append_abandoned_file(base_dir: &std::path::Path, caid: &ContentHash) -> Result<()> {
        let oo = base_dir.join(".oo");
        if !oo.exists() {
            std::fs::create_dir_all(&oo)?;
        }
        let p = Self::abandoned_path(base_dir);
        let mut existing = Self::load_abandoned_file(base_dir).unwrap_or_default();
        let s = caid.to_string();
        if !existing.contains(&s) {
            existing.push(s);
        }
        crate::storage::atomic_write(&p, existing.join("\n") + "\n")?;
        Ok(())
    }

    /// `#rollback` (SPEC_08 §6.2): move HEAD to `target`, reload root.
    /// Does not create a commit. Records the abandoned former HEAD for the
    /// next commit's meta (R1). Objects stay in the store.
    pub fn rollback(
        &mut self,
        engine: &Ouroboros,
        base_dir: &std::path::Path,
        target: &ContentHash,
    ) -> Result<()> {
        if self.is_dirty {
            return Err(anyhow::anyhow!(
                "dirty worktree: commit or discard staged changes before rollback"
            ));
        }
        // Target must exist as a commit object (any historical commit).
        let target_commit = engine.store.get_commit(target)?;
        let new_root = engine
            .store
            .get_root(&target_commit.root, &engine.standard_roots)?;
        let standard_root = standard_for_root(engine, &target_commit.root)?;
        // Record the head we leave behind (if any and different from target).
        if let Some(ref old) = self.head {
            if old != target {
                Self::append_abandoned_file(base_dir, old)?;
            }
        }
        engine.store.set_head(base_dir, target)?;
        self.head = Some(target.clone());
        self.root = new_root;
        self.standard_root = standard_root;
        self.staged = ComboVal::default();
        self.is_dirty = false;
        self.pin_pending = false;
        self.pin_coords.clear();
        engine.clear_force_memo();
        Ok(())
    }

    /// `#squash` (SPEC_08 §6.2): compress commits strictly after `base` up to
    /// HEAD into one commit with `parent = base`, `root = HEAD.root`, kind
    /// Squash. Drops parent-chain reachability of the range; abandoned edges
    /// on intermediate commits leave with them (R2: the Squash marker carries
    /// the fact of removal). Does not delete store objects.
    /// How many commits sit between `base` (exclusive) and HEAD (inclusive).
    /// ACCEPTANCE REPAIR: lets the squash audit message state what it removed,
    /// so the machine-set kind marker stays distinguishable from the message.
    pub fn commits_after(
        &self,
        engine: &Ouroboros,
        base_dir: &std::path::Path,
        base: &ContentHash,
    ) -> Result<usize> {
        let mut n = 0usize;
        let mut curr = self.head.clone();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        while let Some(h) = curr {
            if &h == base {
                return Ok(n);
            }
            let d = hex::encode(&h.digest);
            if !seen.insert(d.clone()) {
                break;
            }
            n += 1;
            let commit = engine.store.get_commit(&h)?;
            curr = crate::savepoint::previous_commit(base_dir, &commit, &d)?;
        }
        Err(anyhow::anyhow!("squash base is not an ancestor of HEAD"))
    }

    pub fn squash(
        &mut self,
        engine: &Ouroboros,
        base_dir: &std::path::Path,
        base: &ContentHash,
        meta: crate::value::CommitMeta,
    ) -> Result<ContentHash> {
        if self.is_dirty {
            return Err(anyhow::anyhow!(
                "dirty worktree: commit or discard staged changes before squash"
            ));
        }
        let head = self
            .head
            .clone()
            .ok_or_else(|| anyhow::anyhow!("no HEAD to squash"))?;
        if &head == base {
            return Err(anyhow::anyhow!(
                "squash range empty: HEAD is already the base"
            ));
        }
        // Verify base is an ancestor of HEAD (D50 DAG, dual walk).
        let head_commit = engine.store.get_commit(&head)?;
        if !crate::savepoint::commit_is_ancestor(base_dir, &engine.store, &head, base)? {
            return Err(anyhow::anyhow!("squash base is not an ancestor of HEAD"));
        }
        // Confirm base object exists.
        let _ = engine.store.get_commit(base)?;
        // New commit: parent=base, root unchanged from HEAD, kind Squash.
        // Intentionally does NOT copy abandoned meta from intermediates —
        // those edges leave with the range (R2: Squash marker is the fact).
        let mut commit = crate::value::Commit::new(None, head_commit.root.clone(), meta);
        commit.kind = CommitKind::Squash;
        let commit_hash = engine.store.put_commit(&commit)?;
        engine.store.set_head(base_dir, &commit_hash)?;
        // Covering: unique tip (HEAD's commit-circle). Ancestor: the base,
        // so log/squash skip the compressed range without opening a hole.
        let ancestor =
            crate::savepoint::circle_id_for_commit(base_dir, &hex::encode(&base.digest))?;
        crate::savepoint::record_commit(base_dir, &commit_hash, None, ancestor.as_deref())?;
        // Root value is the same as before; reload for consistency.
        self.root = engine
            .store
            .get_root(&head_commit.root, &engine.standard_roots)?;
        self.standard_root = standard_for_root(engine, &head_commit.root)?;
        self.head = Some(commit_hash.clone());
        self.staged = ComboVal::default();
        self.is_dirty = false;
        // Pending abandonment file may point into the compressed range;
        // drop it — squash made those edges unreachable and marks itself.
        Self::clear_abandoned_file(base_dir);
        engine.clear_force_memo();
        Ok(commit_hash)
    }
    pub fn observe(&self, engine: &Ouroboros, path: &Path) -> Value {
        // Overlay staged ~%Config field overrides onto root before unify so
        // lattice meet never conflicts genesis fuel with user override.
        // SPEC_09 §6 display: binding is the EFFECTIVE config (genesis ∧
        // overrides, all seven knobs) — not the staged fragment alone.
        let mut root_for_obs = self.root.clone();
        let mut staged_for_obs = self.staged.clone();
        if staged_for_obs.get_field("~%Config").is_some() {
            if let Some(eff) = effective_config(&self.root, &self.standard_root, Some(&self.staged))
            {
                root_for_obs.insert_field("~%Config", Value::Combo(eff));
            }
            // Strip Config from staged so unify does not re-meet overrides.
            staged_for_obs.insert_field("~%Config", Value::Top);
        }
        let current = engine.unify(Value::Combo(root_for_obs), Value::Combo(staged_for_obs));
        if let Value::Combo(r) = current {
            let mut ctx =
                EvalContext::new(r.clone()).with_standard_root(self.standard_root.clone());
            ctx.privilege = engine.privilege;
            // Apply ~%Config horizon params from the observation root
            // (includes staged overrides — SPEC_08 §3.1). Timeout only when
            // the user stages a finite value: genesis is `timeout: #_`
            // (unbound), so ordinary observations carry no wall-clock limit.
            if let Some(Value::Combo(ref cfg)) = r.get_field("~%Config").cloned() {
                let apply_timeout = match self.staged.get_field("~%Config") {
                    Some(Value::Combo(sc)) => sc.get_field("timeout").is_some(),
                    _ => false,
                };
                ctx.apply_horizon_config(cfg, true, apply_timeout);
            }
            ctx.refine_map_active = true;
            // Stage 2 (§3.4): force_recursive on the *return value* — solidification
            // moved from evolve to observe (GUIDE_03 §11.5). REPL observes return
            // values, so interactive experience is unchanged; path-directed observe
            // (navigate_segments) forces only the path (§11.4).
            let res = engine.resolve_path(path, &mut ctx);
            // G6 / SYNTAX_07 §4 #6: Ref-mediated observation = structural view
            // (full hybrid node). Non-path `<<…>>` carries a `%structural`
            // mark (payload in `%node`, not `%val`, so lattice collapse does
            // not erase it). Collapsed observation peels `%val` at the
            // projection layer — never inside to_nlang.
            let structural = matches!(&res, Value::Ref(_))
                || matches!(
                    &res,
                    Value::Combo(c) if crate::value::is_structural_view(c)
                );
            let forced = engine.force_recursive(res, &mut ctx);
            // Display projection: G6 value-context peel + SPEC_04 §3.1 #4
            // private-axis strip (collapsed and structural).
            if structural {
                crate::value::strip_local_axis(crate::value::unwrap_structural_view(forced))
            } else {
                match &forced {
                    Value::Combo(c) if crate::value::is_structural_view(c) => {
                        crate::value::strip_local_axis(crate::value::unwrap_structural_view(forced))
                    }
                    _ => crate::value::project_value_context(forced),
                }
            }
        } else {
            BottomCause::Conflict.into()
        }
    }

    /// Create a #refine Commit with geometric monotonicity verification.
    pub fn refine(
        &mut self,
        engine: &Ouroboros,
        base_dir: &std::path::Path,
        source_caids: Vec<ContentHash>,
        target_caids: Vec<ContentHash>,
        authority: Option<AuthorityInfo>,
        meta: crate::value::CommitMeta,
    ) -> Result<ContentHash> {
        engine.clear_force_memo();
        // Step 1: verify geometric monotonicity (new & old = new)
        //
        // ACCEPTANCE REPAIR (cas_integrity arc). This was `if let (Ok, Ok)`,
        // so ANY failure to load either operand silently skipped the check.
        // Skipping on ABSENCE is spec'd — REAL_03 §9.1 opaque mode covers a
        // CAID the engine cannot compute. Skipping on CORRUPTION is not:
        // §9.1 is about CAIDs we cannot evaluate, not about bytes that lie.
        //
        // Demonstrated before the repair, as a paired discriminator:
        //   untampered, monotonicity-violating direction
        //     → "new ⋢ old: refinement fails geometric monotonicity"
        //   SAME direction after editing the target object's bytes in place
        //     → passes step 1 entirely, stops only at authority
        // Tampering bought a skip of the geometric check. Detection existed
        // as of this arc; this call site was discarding it.
        for src in &source_caids {
            for tgt in &target_caids {
                let load = |h: &ContentHash| -> Result<Option<Value>> {
                    match engine.store.get_value(h) {
                        Ok(v) => Ok(Some(v)),
                        Err(e) => match e.downcast_ref::<crate::storage::StoreReadError>() {
                            // Not held locally — opaque, REAL_03 §9.1.
                            Some(crate::storage::StoreReadError::NotFound { .. }) | None => {
                                Ok(None)
                            }
                            Some(crate::storage::StoreReadError::StandardRootUnavailable {
                                ..
                            }) => Err(anyhow::anyhow!("refine operand cannot be opened: {}", e)),
                            // Present and lying, or present and undecodable:
                            // the check cannot be performed, and pretending it
                            // passed is the fail-open this arc exists to close.
                            Some(other) => Err(anyhow::anyhow!(
                                "refine operand cannot be verified: {}",
                                other
                            )),
                        },
                    }
                };
                if let (Some(src_val), Some(tgt_val)) = (load(src)?, load(tgt)?) {
                    let meet = engine.unify(tgt_val.clone(), src_val.clone());
                    if meet.content_hash() != tgt_val.content_hash() {
                        return Err(anyhow::anyhow!(
                            "new ⋢ old: refinement fails geometric monotonicity"
                        ));
                    }
                }
            }
        }

        // Step 1b: authority verification
        let payload = crate::authority::compute_refine_payload(&source_caids, &target_caids);
        let architect_reg = engine
            .architect_registry
            .read()
            .map_err(|e| anyhow::anyhow!("{:?}", e))?;
        // Epoch judgment: exempt only in genesis state (no HEAD) or before any architect registered
        let bootstrap_exempt = self.head.is_none() || architect_reg.is_empty();
        let authority_status = match crate::authority::verify_refine_authority(
            authority.as_ref(),
            &payload,
            &architect_reg,
            bootstrap_exempt,
        ) {
            crate::authority::AuthVerifyResult::Valid => Some("verified".to_string()),
            crate::authority::AuthVerifyResult::Exempt => Some("unverified".to_string()),
            crate::authority::AuthVerifyResult::Invalid(reason) => {
                return Err(anyhow::anyhow!("authority verification failed: {}", reason));
            }
        };

        // Step 1c: Shadow scan — identify historical commits that directly reference source CAIDs
        // REAL_03 §6.6: do not silently truncate on corruption (v0.2.43 refine
        // precedent). NotFound/opaque → current behaviour; CaidMismatch /
        // ObjectUndecodable → record incident, stop scan, flag incomplete.
        //
        // ACCEPTANCE REPAIR (peer-fetch arc). The delivery recorded the failure
        // with its true kind and then recorded a SECOND incident, for the same
        // address, whose kind was hard-coded `Mismatch`. Measured:
        //
        //   integrity #undecodable: requested <X> source=shadow-scan
        //   integrity #mismatch:    requested <X> source=shadow-scan-truncated
        //
        // One object, one address, two contradictory verdicts — and the second
        // one false. §6.6 條款三 exists so the three outcomes stay separable;
        // an audit line that asserts the wrong one is worse than a missing one.
        // Now a single incident carries the true kind and says it truncated.
        const SHADOW_SCAN_DEPTH: usize = 16;
        let mut shadow_affected: Vec<ContentHash> = Vec::new();
        {
            let mut current = self.head.clone();
            let mut depth = 0;
            while let Some(ref ch) = current.clone() {
                if depth >= SHADOW_SCAN_DEPTH {
                    break;
                }
                depth += 1;
                let commit = match engine.store.get_commit(ch) {
                    Ok(c) => c,
                    Err(e) => match e.downcast_ref::<crate::storage::StoreReadError>() {
                        Some(crate::storage::StoreReadError::NotFound { .. }) | None => break,
                        Some(crate::storage::StoreReadError::StandardRootUnavailable {
                            ..
                        }) => {
                            return Err(anyhow::anyhow!(
                                "refine shadow scan cannot open commit {ch}: {}",
                                e
                            ));
                        }
                        Some(other) => {
                            let kind = match other {
                                crate::storage::StoreReadError::CaidMismatch { .. } => {
                                    crate::IntegrityKind::Mismatch
                                }
                                crate::storage::StoreReadError::ObjectUndecodable { .. } => {
                                    crate::IntegrityKind::Undecodable
                                }
                                crate::storage::StoreReadError::NotFound { .. } => unreachable!(),
                                crate::storage::StoreReadError::StandardRootUnavailable {
                                    ..
                                } => {
                                    unreachable!("handled by the abort arm above")
                                }
                            };
                            engine.record_integrity(ch, "shadow-scan-truncated", kind);
                            break;
                        }
                    },
                };
                let root_val = match engine.store.get_value(&commit.root) {
                    Ok(v) => v,
                    Err(e) => match e.downcast_ref::<crate::storage::StoreReadError>() {
                        Some(crate::storage::StoreReadError::NotFound { .. }) | None => {
                            let d = hex::encode(&ch.digest);
                            current = crate::savepoint::previous_commit(base_dir, &commit, &d)?;
                            continue;
                        }
                        Some(crate::storage::StoreReadError::StandardRootUnavailable {
                            ..
                        }) => {
                            return Err(anyhow::anyhow!(
                                "refine shadow scan cannot open root of commit {ch}: {}",
                                e
                            ));
                        }
                        Some(other) => {
                            let kind = match other {
                                crate::storage::StoreReadError::CaidMismatch { .. } => {
                                    crate::IntegrityKind::Mismatch
                                }
                                crate::storage::StoreReadError::ObjectUndecodable { .. } => {
                                    crate::IntegrityKind::Undecodable
                                }
                                crate::storage::StoreReadError::NotFound { .. } => unreachable!(),
                                crate::storage::StoreReadError::StandardRootUnavailable {
                                    ..
                                } => {
                                    unreachable!("handled by the abort arm above")
                                }
                            };
                            engine.record_integrity(
                                &commit.root,
                                &format!("shadow-scan-truncated (root of commit {ch})"),
                                kind,
                            );
                            break;
                        }
                    },
                };
                if let Value::Combo(ref cv) = root_val {
                    'field_scan: for (_, fv) in cv.all_fields_iter() {
                        let fh = fv.content_hash();
                        for src in &source_caids {
                            if &fh == src {
                                shadow_affected.push(ch.clone());
                                break 'field_scan;
                            }
                        }
                    }
                }
                let d = hex::encode(&ch.digest);
                current = crate::savepoint::previous_commit(base_dir, &commit, &d)?;
            }
        }

        // Step 1d: cycle detection — reject if source→target would close a refine cycle
        {
            let map = engine
                .refine_map
                .read()
                .map_err(|e| anyhow::anyhow!("{:?}", e))?;
            for src in &source_caids {
                let src_str = src.to_string();
                for tgt in &target_caids {
                    if src == tgt {
                        continue;
                    }
                    let mut stack = vec![tgt.to_string()];
                    let mut seen = std::collections::HashSet::new();
                    while let Some(current) = stack.pop() {
                        if current == src_str {
                            return Err(anyhow::anyhow!(
                                "refine cycle detected: {} → {} would create a cycle",
                                src_str,
                                tgt
                            ));
                        }
                        if seen.insert(current.clone()) {
                            if let Some(nexts) = map.get(&current) {
                                stack.extend(nexts.iter().cloned());
                            }
                        }
                    }
                }
            }
        }

        // Step 2: build Refine Commit
        let current_root_hash = match &self.head {
            Some(h) => engine.store.get_commit(h)?.root.clone(),
            None => engine.store.put_value(&Value::Combo(self.root.clone()))?,
        };
        let commit = Commit {
            parent: None,
            root: current_root_hash,
            meta,
            kind: CommitKind::Refine,
            refine_info: Some(RefineInfo {
                source_caids: source_caids.clone(),
                target_caids: target_caids.clone(),
                authority,
                shadow_affected,
                authority_status,
            }),
            cache_id: crate::value::default_cache_id(),
        };
        let commit_hash = engine.store.put_commit(&commit)?;
        engine.store.set_head(base_dir, &commit_hash)?;
        let ancestor = self.head.as_ref().and_then(|h| {
            crate::savepoint::circle_id_for_commit(base_dir, &hex::encode(&h.digest))
                .ok()
                .flatten()
        });
        crate::savepoint::record_commit(base_dir, &commit_hash, None, ancestor.as_deref())?;
        self.head = Some(commit_hash.clone());

        // Step 3: update RefineMap
        let mut map = engine
            .refine_map
            .write()
            .map_err(|e| anyhow::anyhow!("{:?}", e))?;
        for src in &source_caids {
            let targets: Vec<String> = target_caids.iter().map(|t| t.to_string()).collect();
            map.entry(src.to_string()).or_default().extend(targets);
        }

        Ok(commit_hash)
    }
}
