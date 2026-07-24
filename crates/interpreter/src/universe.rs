use crate::value::{Value, ComboVal, ContentHash, BottomCause, CommitKind, RefineInfo, Commit, default_cache_id, AuthorityInfo, EffectTag};
use crate::Ouroboros;
use crate::EvalContext;
use nlang_parser::ast::{Path, PathAnchor, Field, FieldKey, Prefix, Expr, ExprKind, AtomKind};
use indexmap::IndexMap;
use anyhow::Result;

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
    // Non-negative integer knobs.
    match v {
        Value::Atom(AtomKind::Int(n), _, _) if *n >= 0i64.into() => Ok(()),
        _ => Err(BottomCause::InvalidConfig),
    }
}

/// Genesis ∧ staged overrides — effective closed config (display + resolve).
pub(crate) fn effective_config(root: &ComboVal, staged: Option<&ComboVal>) -> Option<ComboVal> {
    let mut base = match root.get_field("~%Config") {
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
        FieldKey::Quoted(name) if name.trim().starts_with("~%") => true,
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

/// True only for the literal open-world hole `_` (not e.g. `a + 1` → Top).
fn is_literal_top(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Atom(AtomKind::Top) => true,
        ExprKind::Path(p) => {
            p.anchor == PathAnchor::Bare
                && p.segments.len() == 1
                && p.segments[0].trim() == "_"
        }
        _ => false,
    }
}

pub struct Universe { pub head: Option<ContentHash>, pub root: ComboVal, pub staged: ComboVal, pub is_dirty: bool }
impl Universe {
    pub fn new(head: Option<ContentHash>, root: ComboVal) -> Self { Self { head, root, staged: ComboVal::default(), is_dirty: false } }
    pub fn load(engine: &Ouroboros, base_dir: &std::path::Path) -> Result<Self> {
        engine.clear_force_memo();
        let head = engine.store.get_head(base_dir)?; match head { Some(h) => { let commit = engine.store.get_commit(&h)?; let root_val = engine.store.get_value(&commit.root)?; if let Value::Combo(root) = root_val { Ok(Self::new(Some(h), root)) } else { Err(anyhow::anyhow!("Invalid root")) } } None => Ok(Self::new(None, engine.root_with_system())), } }
    
    pub fn evolve(&mut self, engine: &Ouroboros, field: &Field) -> std::result::Result<(), BottomCause> {
        // SPEC_09 ownership: user LHS on `~%` is illegal (except root
        // ~%Config.<bare> horizon family). Loud at evolve boundary — same
        // family as G2-S Evolution Conflict (CLI exit 1).
        if is_system_axis_lhs_forbidden(&field.key) {
            return Err(BottomCause::SystemReserved);
        }

        let mut ctx = EvalContext::new(self.root.clone());
        ctx.staged = Some(self.staged.clone());
        ctx.horizon_salt = engine.store.get_horizon_salt();
        ctx.privileged = engine.privileged;
        // forward_spread acceptance repair: cocoon literals force_recursive
        // at construction (GUIDE_03 §11.5) — during evolve a forward source
        // is open-miss Top only because it is not defined YET; mark the
        // phase so expansion re-queues instead of consuming (cocoon face:
        // {{...later, b: 1}} with later defined below).
        ctx.in_evolve = true;

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
                effect: EffectTag::Pure,
            };
        }
        let val_effect = val.effect();

        let mut rf = IndexMap::new();
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
                    let p = match prefix { Some(Prefix::Logic) => "/", Some(Prefix::Type) => "@", Some(Prefix::Meta) => "%", Some(Prefix::System) => "~%", _ => "" };
                    let stored = format!("{}{}", p, trimmed);
                    // Stage 5 acceptance fix: dependency recording uses stored
                    // (prefixed) names, so invalidate by BOTH forms.
                    evolved_coords.push(stored);
                    evolved_coords.push(trimmed);
                }
            }
            FieldKey::Quoted(name) => { evolved_coords.push(name.trim().to_string()); }
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
                    return Err(BottomCause::InvalidConfig);
                }
                // Type after eval — `fuel: 40 + 10` is lawful 50.
                validate_config_knob_value(&bare, &val)?;
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
                let incoming = Value::Combo(ComboVal::new(rf, false, rl, val_effect, vec![]));
                if !evolved_coords.is_empty() {
                    engine.invalidate_coords(&evolved_coords);
                }
                // Merge open partials only (no root Config in the meet).
                let res = engine.unify(Value::Combo(self.staged.clone()), incoming);
                return match res {
                    Value::Combo(m) => {
                        self.staged = m;
                        self.is_dirty = true;
                        Ok(())
                    }
                    Value::Bottom(d) => Err(d.cause),
                    _ => Err(BottomCause::Conflict),
                };
            }
            _ => { self.is_dirty = true; return Ok(()); }
        };

        // G2-S: root coordinates evolve monotonically. If the incoming value
        // conflicts with an existing ROOT binding at any written coordinate,
        // fail at the evolve boundary (loud Evolution Conflict) instead of
        // poisoning the whole universe at observe-entry unify(root, staged).
        // Staged×staged conflicts stay on the existing unify path below.
        for c in &evolved_coords {
            if let Some(root_val) = self.root.get_field(c).cloned()
                .or_else(|| self.root.get_local_field(c).cloned())
            {
                if let Value::Bottom(d) = engine.unify(root_val, val.clone()) {
                    return Err(d.cause);
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
                    let p = match prefix { Some(Prefix::Logic) => "/", Some(Prefix::Type) => "@", Some(Prefix::Meta) => "%", Some(Prefix::System) => "~%", _ => "" };
                    let stored = format!("{}{}", p, trimmed);
                    rf.insert(stored, val);
                }
            }
            FieldKey::Quoted(name) => { rf.insert(name.trim().to_string(), val); }
            FieldKey::Path(p) if p.segments.len() == 1 && p.anchor == PathAnchor::Bare => {
                rf.insert(p.segments[0].trim().to_string(), val);
            }
            _ => unreachable!("coords already filtered non-writable keys"),
        };

        let incoming = Value::Combo(ComboVal::new(rf, false, rl, val_effect, vec![]));
        // Stage 5 (§5b): invalidate memo entries that depend on the evolved
        // coordinates. Called before the merge succeeds so entries reading
        // staged values are cleared.
        if !evolved_coords.is_empty() {
            engine.invalidate_coords(&evolved_coords);
        }
        let res = engine.unify(Value::Combo(self.staged.clone()), incoming);
        match res {
            Value::Combo(m) => { self.staged = m; self.is_dirty = true; Ok(()) }
            Value::Bottom(d) => Err(d.cause),
            _ => Err(BottomCause::Conflict)
        }
    }

    pub fn save_staged(&self, _engine: &Ouroboros, base_dir: &std::path::Path) -> Result<()> {
        let staged_path = base_dir.join(".oo").join("staged");
        if !staged_path.parent().unwrap().exists() { std::fs::create_dir_all(staged_path.parent().unwrap())?; }
        let json = serde_json::to_string(&self.staged)?;
        std::fs::write(staged_path, json)?;
        Ok(())
    }

    pub fn load_staged(&mut self, base_dir: &std::path::Path) -> Result<()> {
        let staged_path = base_dir.join(".oo").join("staged");
        if staged_path.exists() {
            let json = std::fs::read_to_string(staged_path)?;
            self.staged = serde_json::from_str(&json)?;
            self.is_dirty = true;
        }
        Ok(())
    }

    pub fn commit(&mut self, engine: &Ouroboros, base_dir: &std::path::Path, meta: crate::value::CommitMeta) -> Result<ContentHash> {
        engine.clear_force_memo();
        let res = engine.unify(Value::Combo(self.root.clone()), Value::Combo(self.staged.clone()));
        match res { Value::Combo(new_root) => { 
            let root_hash = engine.store.put_value(&Value::Combo(new_root.clone()))?; 
            let commit = crate::value::Commit::new(self.head.clone(), root_hash, meta); 
            let commit_hash = engine.store.put_commit(&commit)?; 
            engine.store.set_head(base_dir, &commit_hash)?; 
            self.root = new_root; 
            self.staged = ComboVal::default(); 
            self.head = Some(commit_hash.clone()); 
            self.is_dirty = false; 
            let staged_path = base_dir.join(".oo").join("staged");
            if staged_path.exists() { let _ = std::fs::remove_file(staged_path); }
            Ok(commit_hash) 
        } _ => Err(anyhow::anyhow!("Commit failed")), }
    }
    pub fn observe(&self, engine: &Ouroboros, path: &Path) -> Value {
        // Overlay staged ~%Config field overrides onto root before unify so
        // lattice meet never conflicts genesis fuel with user override.
        // SPEC_09 §6 display: binding is the EFFECTIVE config (genesis ∧
        // overrides, all seven knobs) — not the staged fragment alone.
        let mut root_for_obs = self.root.clone();
        let mut staged_for_obs = self.staged.clone();
        if staged_for_obs.get_field("~%Config").is_some() {
            if let Some(eff) = effective_config(&self.root, Some(&self.staged)) {
                root_for_obs.insert_field("~%Config", Value::Combo(eff));
            }
            // Strip Config from staged so unify does not re-meet overrides.
            staged_for_obs.insert_field("~%Config", Value::Top);
        }
        let current =
            engine.unify(Value::Combo(root_for_obs), Value::Combo(staged_for_obs));
        if let Value::Combo(r) = current {
            let mut ctx = EvalContext::new(r.clone());
            ctx.privileged = engine.privileged;
            // Apply ~%Config horizon params from the observation root
            // (includes staged overrides — SPEC_08 §3.1).
            if let Some(Value::Combo(ref cfg)) = r.get_field("~%Config").cloned() {
                use num_traits::ToPrimitive;
                if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("fuel").cloned() {
                    if let Some(f) = n.to_u64() {
                        ctx.fuel = f;
                    }
                }
                if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("max_branches").cloned() {
                    if let Some(v) = n.to_u64() {
                        ctx.max_branches = v as usize;
                    }
                }
                if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("max_unification_depth").cloned() {
                    if let Some(v) = n.to_u64() {
                        ctx.max_unification_depth = v as usize;
                    }
                }
                if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("max_lifting_depth").cloned() {
                    if let Some(v) = n.to_u64() {
                        ctx.max_lifting_depth = v as usize;
                    }
                }
                if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("max_pattern_nodes").cloned() {
                    if let Some(v) = n.to_u64() {
                        ctx.max_pattern_nodes = v as usize;
                    }
                }
                if let Some(Value::Atom(AtomKind::Tag(s), _, _)) = cfg.get_field("strategy").cloned() {
                    use crate::value::ObservationStrategy;
                    ctx.strategy = match s.trim_start_matches('#') {
                        "strict" => ObservationStrategy::Strict,
                        "approximate" => ObservationStrategy::Approximate,
                        _ => ObservationStrategy::Blur,
                    };
                }
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
                        crate::value::strip_local_axis(crate::value::unwrap_structural_view(
                            forced,
                        ))
                    }
                    _ => crate::value::project_value_context(forced),
                }
            }
        } else { BottomCause::Conflict.into() }
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
        for src in &source_caids {
            for tgt in &target_caids {
                if let (Ok(src_val), Ok(tgt_val)) = (engine.store.get_value(src), engine.store.get_value(tgt)) {
                    let meet = engine.unify(tgt_val.clone(), src_val.clone());
                    if meet.content_hash() != tgt_val.content_hash() {
                        return Err(anyhow::anyhow!("new ⋢ old: refinement fails geometric monotonicity"));
                    }
                }
            }
        }

        // Step 1b: authority verification
        let payload = crate::authority::compute_refine_payload(&source_caids, &target_caids);
        let architect_reg = engine.architect_registry.read().map_err(|e| anyhow::anyhow!("{:?}", e))?;
        // Epoch judgment: exempt only in genesis state (no HEAD) or before any architect registered
        let bootstrap_exempt = self.head.is_none() || architect_reg.is_empty();
        match crate::authority::verify_refine_authority(authority.as_ref(), &payload, &architect_reg, bootstrap_exempt) {
            crate::authority::AuthVerifyResult::Valid | crate::authority::AuthVerifyResult::Exempt => {}
            crate::authority::AuthVerifyResult::Invalid(reason) => {
                return Err(anyhow::anyhow!("authority verification failed: {}", reason));
            }
        }

        // Step 1c: Shadow scan — identify historical commits that directly reference source CAIDs
        const SHADOW_SCAN_DEPTH: usize = 16;
        let mut shadow_affected: Vec<ContentHash> = Vec::new();
        {
            let mut current = self.head.clone();
            let mut depth = 0;
            while let Some(ref ch) = current.clone() {
                if depth >= SHADOW_SCAN_DEPTH { break; }
                depth += 1;
                let commit = match engine.store.get_commit(ch) {
                    Ok(c) => c,
                    Err(_) => break,
                };
                let root_val = match engine.store.get_value(&commit.root) {
                    Ok(v) => v,
                    Err(_) => { current = commit.parent; continue; }
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
                current = commit.parent;
            }
        }

        // Step 1d: cycle detection — reject if source→target would close a refine cycle
        {
            let map = engine.refine_map.read().map_err(|e| anyhow::anyhow!("{:?}", e))?;
            for src in &source_caids {
                let src_str = src.to_string();
                for tgt in &target_caids {
                    if src == tgt { continue; }
                    let mut stack = vec![tgt.to_string()];
                    let mut seen = std::collections::HashSet::new();
                    while let Some(current) = stack.pop() {
                        if current == src_str {
                            return Err(anyhow::anyhow!(
                                "refine cycle detected: {} → {} would create a cycle",
                                src_str, tgt
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
            parent: self.head.clone(),
            root: current_root_hash,
            meta,
            kind: CommitKind::Refine,
            refine_info: Some(RefineInfo {
                source_caids: source_caids.clone(),
                target_caids: target_caids.clone(),
                authority,
                shadow_affected,
            }),
            cache_id: crate::value::default_cache_id(),
        };
        let commit_hash = engine.store.put_commit(&commit)?;
        engine.store.set_head(base_dir, &commit_hash)?;
        self.head = Some(commit_hash.clone());

        // Step 3: update RefineMap
        let mut map = engine.refine_map.write().map_err(|e| anyhow::anyhow!("{:?}", e))?;
        for src in &source_caids {
            let targets: Vec<String> = target_caids.iter().map(|t| t.to_string()).collect();
            map.entry(src.to_string()).or_default().extend(targets);
        }

        Ok(commit_hash)
    }
}
