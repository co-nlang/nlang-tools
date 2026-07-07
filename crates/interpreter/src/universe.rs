use crate::value::{Value, ComboVal, ContentHash, BottomCause, CommitKind, RefineInfo, Commit, default_cache_id, AuthorityInfo};
use crate::Ouroboros;
use crate::EvalContext;
use nlang_parser::ast::{Path, PathAnchor, Field, FieldKey, Prefix};
use indexmap::IndexMap;
use anyhow::Result;

pub struct Universe { pub head: Option<ContentHash>, pub root: ComboVal, pub staged: ComboVal, pub is_dirty: bool }
impl Universe {
    pub fn new(head: Option<ContentHash>, root: ComboVal) -> Self { Self { head, root, staged: ComboVal::default(), is_dirty: false } }
    pub fn load(engine: &Ouroboros, base_dir: &std::path::Path) -> Result<Self> { let head = engine.store.get_head(base_dir)?; match head { Some(h) => { let commit = engine.store.get_commit(&h)?; let root_val = engine.store.get_value(&commit.root)?; if let Value::Combo(root) = root_val { Ok(Self::new(Some(h), root)) } else { Err(anyhow::anyhow!("Invalid root")) } } None => Ok(Self::new(None, engine.root_with_system())), } }
    
    pub fn evolve(&mut self, engine: &Ouroboros, field: &Field) -> std::result::Result<(), BottomCause> {
        let mut ctx = EvalContext::new(self.root.clone());
        ctx.staged = Some(self.staged.clone());
        ctx.horizon_salt = engine.store.get_horizon_salt();
        // Stage 2 (§3.4): do NOT force_recursive at evolve time. Open terms are
        // stored as thunks (P3: open terms may be stored; the binding is supplied
        // at observation). Solidification moves to observe.
        let val = engine.eval(&field.value, &mut ctx);
        let val_effect = val.effect();

        let mut rf = IndexMap::new();
        let mut rl = IndexMap::new();

        match &field.key {
            FieldKey::Named { name, prefix } => {
                let is_p = matches!(prefix, Some(Prefix::Private) | Some(Prefix::Local));
                let trimmed = name.trim().to_string();
                if is_p {
                    rl.insert(trimmed, val);
                } else {
                    let p = match prefix { Some(Prefix::Logic) => "/", Some(Prefix::Type) => "@", Some(Prefix::Meta) => "%", Some(Prefix::System) => "~%", _ => "" };
                    rf.insert(format!("{}{}", p, trimmed), val);
                }
            }
            FieldKey::Quoted(name) => { rf.insert(name.trim().to_string(), val); }
            FieldKey::Path(p) if p.segments.len() == 1 && p.anchor == PathAnchor::Bare => { rf.insert(p.segments[0].trim().to_string(), val); }
            _ => { self.is_dirty = true; return Ok(()); }
        };

        let incoming = Value::Combo(ComboVal::new(rf, false, rl, val_effect, vec![]));
        let res = engine.unify(Value::Combo(self.staged.clone()), incoming);
        match res {
            Value::Combo(m) => { self.staged = m; self.is_dirty = true; Ok(()) }
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
        let current = engine.unify(Value::Combo(self.root.clone()), Value::Combo(self.staged.clone()));
        if let Value::Combo(r) = current {
            let mut ctx = EvalContext::new(r);
            ctx.refine_map_active = true;
            // Stage 2 (§3.4): force_recursive on the *return value* — solidification
            // moved from evolve to observe (GUIDE_03 §11.5). REPL observes return
            // values, so interactive experience is unchanged; path-directed observe
            // (navigate_segments) forces only the path (§11.4).
            let res = engine.resolve_path(path, &mut ctx);
            engine.force_recursive(res, &mut ctx)
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
