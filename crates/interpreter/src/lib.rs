pub mod universe; pub use universe::Universe;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use indexmap::IndexMap;
use nlang_parser::ast::{Path, PathAnchor, AtomKind, Expr, ExprKind};
use nlang_parser::tier::{Tier, classify_tier};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use std::path::PathBuf;
pub mod value;
pub mod bn_serial;
pub mod lattice_sketch;
pub mod storage;
pub mod complement;
pub mod builtins;
pub mod unify;
pub mod eval;
pub mod type_constraint;
pub mod dispatch;
pub mod observation;
pub mod genesis;
pub mod ladd;
pub mod oml;
pub mod authority;
pub use crate::value::{Value, ComboVal, EffectTag, Privilege, ContentHash, CaidVersion, MasaRef, BottomDetail, BottomCause, CommitMeta, Commit, CommitKind, RefineInfo, Holonomy, Identity, AuthorityInfo, BlurDetail, BlurCause, HorizonParams, ObservationStrategy, normalize_union, primary_bottom_from_culled};
pub use crate::storage::{ObjectStore, StoreReadError, value_address_matches};
pub use crate::dispatch::{MorphismDispatchResult, MorphismDispatchResult as DispatchResult};
pub use crate::observation::{ObservationState, handle_resource_exhausted};
use crate::builtins::create_default_builtins;
use crate::type_constraint::{
    TypeConstraint, get_type_constraint_name, is_type_constraint_combo, is_user_field_type_combo,
};
use anyhow::Result;
use sha2::Digest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceExhausted { FuelExhausted, Timeout, StackOverflow }

#[derive(Debug, Clone)]
pub struct EvalContext {
    // F2 (Stage 3-fix): Arc so EvalContext::clone (sub_context on every thunk
    // force) bumps a refcount instead of deep-copying the universe — the
    // O(depth x N_fields x |root|) amplifier in self-referential observation.
    // Reads deref transparently; the engine never mutates root mid-eval.
    pub root: Arc<ComboVal>,
    /// Stage 4: lazily computed CAID of `root`, shared through sub_context
    /// clones. Sound because the engine never mutates root mid-observation
    /// (tests that mutate via Arc::make_mut do so before any force).
    pub root_caid_cache: Option<ContentHash>,
    pub scopes: Vec<ComboVal>,
    pub staged: Option<ComboVal>,
    pub computing: HashSet<String>,
    pub call_history: HashMap<String, Vec<ContentHash>>,
    /// L2-17: thunk content-hashes currently being forced in this observation.
    /// Re-entering the same thunk → ⊥ #divergent (must fire before stack/fuel),
    /// unless `lexical_forcing` is non-empty (sibling bare-name soft re-entry
    /// → Top; SPEC_04 §2.1 completion — mutual/self sibling pins stay `_`).
    /// Survives `sub_context` clones (unlike the legacy `computing` clear).
    pub in_flight: HashSet<ContentHash>,
    /// Bare names currently being forced out of a scope-frame field (lexical
    /// chain). Soft re-entry uses cycle_reentry (static → caused Top /
    /// transform → #divergent).
    pub lexical_forcing: HashSet<String>,
    /// SPEC_12 §1.1: true if any hop on the current force chain is a
    /// non-pure-reference expression (arithmetic, apply, …). Pure-ref
    /// re-entry → caused Top; tainted re-entry → ⊥ #divergent.
    pub chain_transform_taint: bool,
    /// Coordinates / bare names on the current force stack (cycle members).
    pub cycle_chain: Vec<String>,
    pub in_math_op: bool,
    /// forward_spread acceptance repair: evolve-phase marker — pending
    /// spread sources that resolve Top during evolve are re-queued (the
    /// binding may simply not exist YET); only observation-time force
    /// consumes Top as a true no-op (never-defined / open hole).
    pub in_evolve: bool,
    /// Fixed-point fence (union_absorption): while true, union normalize
    /// stays pure dedupe — absorption's internal meets must not re-enter
    /// the absorbing normalizer (single-layer; compare raw branches).
    pub union_absorb_fence: bool,
    pub context_value: Option<Value>,
    pub fuel: u64,
    pub timeout_deadline: Option<u64>,
    pub depth: u32,
    /// Stage 5 (§5a): dependency collector for Route B per-coordinate
    /// invalidation. When Some, coordinate reads are recorded here. None
    /// means dependency tracking is disabled (no memo miss in progress or
    /// staged context). Nested thunk forces install a fresh collector and
    /// merge deps back into the outer one.
    pub dep_collector: Option<HashSet<String>>,
    /// Stage 5 acceptance fix: memo participation is restricted to
    /// observation contexts. Engine-internal contexts (eval_context: unify
    /// merges, formatting) run against the pristine system root with no
    /// collector — letting them insert produces deps-less "permanent"
    /// entries computed against the wrong root.
    pub memo_enabled: bool,
    pub horizon_salt: ContentHash,
    pub strategy: ObservationStrategy,
    pub max_branches: usize,
    pub max_unification_depth: usize,
    pub max_pattern_nodes: usize,
    pub max_lifting_depth: usize,
    pub refine_map_active: bool,
    pub had_nondistrib_event: bool,
    pub disc_routing_visited: std::collections::HashSet<String>,
    pub disc_routing_hops: u32,
    /// SPEC_08 §6 capability lattice (selective_discharge). Cloned via
    /// sub_context — no special inheritance.
    pub privilege: crate::value::Privilege,
}

impl EvalContext {
    pub fn new(root: ComboVal) -> Self {
        let mut hasher = sha2::Sha256::new();
        hasher.update(b"default");
        let salt = ContentHash::v1(hasher.finalize().to_vec());
        Self { 
            root: Arc::new(root), root_caid_cache: None, scopes: Vec::new(), staged: None, computing: HashSet::new(), 
            call_history: HashMap::new(), in_flight: HashSet::new(), lexical_forcing: HashSet::new(),
            chain_transform_taint: false, cycle_chain: Vec::new(),
            in_math_op: false, in_evolve: false, union_absorb_fence: false,
            context_value: None, 
            fuel: 10000, timeout_deadline: None, depth: 0, dep_collector: None, memo_enabled: true, 
            horizon_salt: salt, strategy: ObservationStrategy::Blur,
            max_branches: 64, max_unification_depth: 256, max_pattern_nodes: 1024, max_lifting_depth: 32,
            refine_map_active: false,
            had_nondistrib_event: false,
            disc_routing_visited: std::collections::HashSet::new(),
            disc_routing_hops: 0,
            privilege: crate::value::Privilege::NONE,
        }
    }
    /// Root CAID for memo keys — computed once per root version, then cached
    /// (the cache rides along sub_context clones). Avoids the per-force
    /// deep-clone + full re-hash of the universe.
    pub fn root_caid(&mut self) -> ContentHash {
        if let Some(ref h) = self.root_caid_cache { return h.clone(); }
        let h = Value::Combo((*self.root).clone()).content_hash();
        self.root_caid_cache = Some(h.clone());
        h
    }

    pub fn with_fuel(mut self, fuel: u64) -> Self { self.fuel = fuel; self }
    pub fn with_strategy(mut self, strategy: ObservationStrategy) -> Self { self.strategy = strategy; self }
    pub fn check_resources(&mut self, cost: u64) -> Result<(), ResourceExhausted> { 
        if self.fuel < cost { return Err(ResourceExhausted::FuelExhausted); }
        // G3 R3: depth/stack gate is observation-budget exhaustion, not a
        // detected cycle. Report FuelExhausted so Blur %cause / Strict ⊥
        // share the #fuel_exhausted tag (L2-21/22; #divergent reserved for
        // L2-17 in_flight / coordinate self-ref). ResourceExhausted::StackOverflow
        // remains for explicit callers if ever needed.
        if self.depth > self.max_unification_depth as u32 {
            return Err(ResourceExhausted::FuelExhausted);
        }
        if let Some(deadline) = self.timeout_deadline {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            if now > deadline { return Err(ResourceExhausted::Timeout); }
        }
        self.fuel -= cost;
        Ok(())
    }
}


/// Bare path coordinate for L2-17 path-shaped thunks (`s.v` → `"s.v"`).
fn path_coord_of(expr: &Expr) -> Option<String> {
    match &expr.kind {
        ExprKind::Path(p) if p.anchor == PathAnchor::Bare && !p.segments.is_empty() => Some(
            p.segments
                .iter()
                .map(|s| s.trim().to_string())
                .collect::<Vec<_>>()
                .join("."),
        ),
        _ => None,
    }
}

/// SPEC_12 §1.1: pure-reference hop = path only (any anchor, segments only).
/// Everything else (arith, apply, pipe, literal construction, …) taints —
/// except lattice join/meet/diff, which are structural superposition of
/// branches, not a transform of either coordinate (taint_scope field-join
/// face: forcing `p.v | 9` must not reclassify pure-cycle `p.v` as
/// `#divergent`; caused_top ruling C needs the diagnostic branch to stand).
fn expr_is_pure_ref(expr: &Expr) -> bool {
    matches!(&expr.kind, ExprKind::Path(_))
}

/// Lattice structural ops: do not set `chain_transform_taint` when forced.
fn expr_is_lattice_structural(expr: &Expr) -> bool {
    matches!(
        &expr.kind,
        ExprKind::Join(_, _) | ExprKind::Meet(_, _) | ExprKind::Diff(_, _)
    )
}

/// Re-entry on a force chain: pure static cycle → caused Top; any transform
/// hop → ⊥ #divergent. `reentered` is the coordinate whose re-entry fired —
/// it belongs to the loop even when the chain missed it (acceptance repair:
/// mutual cycle minted members ["a"] only, misreading 互指 as 自指 — the
/// ruling makes loop SHAPE readable from the member list).
fn cycle_reentry(ctx: &EvalContext, reentered: Option<&str>) -> Value {
    if ctx.chain_transform_taint {
        BottomCause::Divergent.into()
    } else {
        let mut members = ctx.cycle_chain.clone();
        if let Some(name) = reentered {
            members.push(name.to_string());
        }
        crate::value::static_cycle_top(members)
    }
}

/// SPEC_08 §4.1: `.%effect` answer — pure tag atom (tag body without `#`).
/// SPEC_08 §4.1: `.%effect` answer — pure tag atom(s). Multi-tag sets become
/// a normalize_union of tag atoms (display order = alphabetical via §2.4.1).
fn effect_tag_atom(e: EffectTag) -> Value {
    if e.is_pure() {
        return Value::Atom(
            AtomKind::Tag("pure".to_string()),
            EffectTag::Pure,
            None,
        );
    }
    let mut atoms = Vec::new();
    // Alphabetical: io, nondet, state (, cached) — same as Display.
    if e.contains(EffectTag::IO) {
        atoms.push(Value::Atom(
            AtomKind::Tag("io".to_string()),
            EffectTag::Pure,
            None,
        ));
    }
    if e.contains(EffectTag::NonDet) {
        atoms.push(Value::Atom(
            AtomKind::Tag("nondet".to_string()),
            EffectTag::Pure,
            None,
        ));
    }
    if e.contains(EffectTag::State) {
        atoms.push(Value::Atom(
            AtomKind::Tag("state".to_string()),
            EffectTag::Pure,
            None,
        ));
    }
    if e.contains(EffectTag::Cached) {
        atoms.push(Value::Atom(
            AtomKind::Tag("cached".to_string()),
            EffectTag::Pure,
            None,
        ));
    }
    match atoms.len() {
        0 => Value::Atom(
            AtomKind::Tag("pure".to_string()),
            EffectTag::Pure,
            None,
        ),
        1 => atoms.into_iter().next().unwrap(),
        _ => crate::value::normalize_union(atoms),
    }
}

pub type BuiltinFn = dyn Fn(Value, &Ouroboros, &mut EvalContext) -> Value + Send + Sync;

#[derive(Clone)]
pub enum Peer {
    Local(Arc<ObjectStore>),
    Remote(String), // TCP address
}

/// Force-memo key (Stage 5): (expr CAID, frame CAID, context CAID | #open).
/// root_caid removed — invalidation is now per-coordinate (Route B, deps).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForceMemoKey {
    pub expr_caid: ContentHash,
    pub frame_caid: ContentHash,
    pub context_caid: Option<ContentHash>,
}

/// Force-memo entry (Stage 5): cached value + coordinate dependencies.
#[derive(Debug, Clone)]
pub struct MemoEntry {
    pub value: Value,
    /// Coordinates (top-level root field names) read during evaluation.
    /// Empty set = C₀ (path-free, $-free — permanent tier).
    pub deps: HashSet<String>,
}

/// REAL_03 §6.6 integrity incident (peer-fetch / store read). Minimal log —
/// not a general diagnostics framework. Printed by the CLI after evaluation.
#[derive(Debug, Clone)]
pub enum IntegrityKind {
    Mismatch,
    Undecodable,
}

#[derive(Debug, Clone)]
pub struct IntegrityIncident {
    pub requested: String,
    /// Peer name, `tcp://…`, `local`, `shadow-scan`, etc.
    pub source: String,
    pub kind: IntegrityKind,
}

pub struct Ouroboros {
    pub store: ObjectStore,
    pub base_dir: Option<PathBuf>,
    pub unify_memo: RwLock<HashMap<(ContentHash, ContentHash), Value>>,
    pub force_memo: RwLock<HashMap<ForceMemoKey, MemoEntry>>,
    /// Reverse index: coord → memo keys that read this coord.
    pub force_memo_rev: RwLock<HashMap<String, HashSet<ForceMemoKey>>>,
    pub builtin_registry: HashMap<String, Arc<BuiltinFn>>,
    pub peers: RwLock<HashMap<String, Peer>>,
    pub identity: crate::value::Identity,
    pub refine_map: RwLock<HashMap<String, Vec<String>>>,
    pub gbb_registry: RwLock<HashMap<String, crate::ladd::GBB>>,
    pub architect_registry: RwLock<std::collections::HashSet<String>>,
    /// SPEC_08 §6 capability lattice. Default NONE; set only via trusted
    /// channel (`set_privilege` / CLI `--privileged`/`--grant`). Never from
    /// in-program n/ code.
    pub privilege: crate::value::Privilege,
    /// Accumulated integrity incidents (條款四). Library is silent; CLI prints.
    pub integrity_log: RwLock<Vec<IntegrityIncident>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp { Eq, Ne, Lt, Gt, Lte, Gte }

impl Ouroboros {
    /// Stage 5 (§5b): invalidate all memo entries that depend on any of the
    /// given coordinates. `"*"` entries are cleared on every call.
    pub fn invalidate_coords(&self, coords: &[String]) {
        if let Ok(mut rev) = self.force_memo_rev.write() {
            if let Ok(mut memo) = self.force_memo.write() {
                for coord in coords {
                    if let Some(keys) = rev.remove(coord) {
                        for k in keys { memo.remove(&k); }
                    }
                }
                if let Some(wildcard_keys) = rev.remove("*") {
                    for k in wildcard_keys { memo.remove(&k); }
                }
            }
        }
    }

    /// Stage 5 (§5b): full clear for coarse events (commit/load/refine).
    pub fn clear_force_memo(&self) {
        if let Ok(mut memo) = self.force_memo.write() { memo.clear(); }
        if let Ok(mut rev) = self.force_memo_rev.write() { rev.clear(); }
    }

    /// Stage 5 (§5a): record a coordinate dependency in the active collector.
    fn record_dep(&self, ctx: &mut EvalContext, coord: &str) {
        if let Some(ref mut deps) = ctx.dep_collector {
            deps.insert(coord.to_string());
        }
    }

    pub fn new_in_memory() -> Self {
        use ring::rand::SecureRandom;
        let mut bytes = [0u8; 8];
        ring::rand::SystemRandom::new().fill(&mut bytes).unwrap();
        let dir = std::env::temp_dir().join(format!("nlang-test-{}", hex::encode(bytes)));
        let store = ObjectStore::init(&dir).unwrap();
        let builtins = create_default_builtins();
        let identity = crate::value::Identity::new_random();
        // No self-appointment into architect_registry (universe_determinism).
        // Empty registry → bootstrap_exempt; provision via load_architects only.
        Self {
            store,
            base_dir: None,
            unify_memo: RwLock::new(HashMap::new()),
            force_memo: RwLock::new(HashMap::new()),
            force_memo_rev: RwLock::new(HashMap::new()),
            builtin_registry: builtins,
            peers: RwLock::new(HashMap::new()),
            identity,
            refine_map: RwLock::new(HashMap::new()),
            gbb_registry: RwLock::new(HashMap::new()),
            architect_registry: RwLock::new(std::collections::HashSet::new()),
            privilege: crate::value::Privilege::NONE,
            integrity_log: RwLock::new(Vec::new()),
        }
    }

    pub fn init(base_dir: &std::path::Path) -> Result<Self> {
        let store = ObjectStore::init(base_dir)?;
        let builtins = create_default_builtins();
        let identity = crate::value::Identity::new_random();
        // Assertion layer only: load provisioned whitelist from .oo/architects.json.
        // Never mint a random local key into the registry (that was a self-signed
        // authority theatre — SPEC_13 §4.1.2 / ORDER_01 trust root).
        let architects = store
            .load_architects(base_dir)
            .unwrap_or_else(|_| std::collections::HashSet::new());
        let oo = Self {
            store,
            base_dir: Some(base_dir.to_path_buf()),
            unify_memo: RwLock::new(HashMap::new()),
            force_memo: RwLock::new(HashMap::new()),
            force_memo_rev: RwLock::new(HashMap::new()),
            builtin_registry: builtins,
            peers: RwLock::new(HashMap::new()),
            identity,
            refine_map: RwLock::new(HashMap::new()),
            gbb_registry: RwLock::new(HashMap::new()),
            architect_registry: RwLock::new(architects),
            privilege: crate::value::Privilege::NONE,
            integrity_log: RwLock::new(Vec::new()),
        };
        Ok(oo)
    }

    /// Record a REAL_03 §6.6 integrity incident (never silently drop a verdict).
    pub fn record_integrity(&self, requested: &ContentHash, source: &str, kind: IntegrityKind) {
        if let Ok(mut log) = self.integrity_log.write() {
            log.push(IntegrityIncident {
                requested: requested.to_string(),
                source: source.to_string(),
                kind,
            });
        }
    }

    /// Drain incidents for CLI display (stderr). Clears the log.
    pub fn take_integrity_incidents(&self) -> Vec<IntegrityIncident> {
        self.integrity_log
            .write()
            .map(|mut log| std::mem::take(&mut *log))
            .unwrap_or_default()
    }

    /// Trusted-channel only. No in-program n/ path may call this.
    pub fn set_privilege(&mut self, privilege: crate::value::Privilege) {
        self.privilege = privilege;
    }

    /// Back-compat shim: `true` → full grant, `false` → NONE.
    pub fn set_privileged(&mut self, privileged: bool) {
        self.privilege = if privileged {
            crate::value::Privilege::all()
        } else {
            crate::value::Privilege::NONE
        };
    }

    /// Union a grant into the horizon capability (CLI `--grant` accumulation).
    pub fn grant_privilege(&mut self, grant: crate::value::Privilege) {
        self.privilege = self.privilege.union(grant);
    }
    
    fn is_list(&self, v: &Value, ctx: &mut EvalContext) -> bool {
        let fv = self.force(v.clone(), ctx);
        match fv.collapse() {
            Value::Combo(cv) => cv.get_field("%kind").map(|k| {
                let ks = self.force(k.clone(), ctx).to_string_plain();
                ks.trim_start_matches('#') == "list"
            }).unwrap_or(false),
            _ => false
        }
    }

    pub fn root_with_system(&self) -> ComboVal {
        let mut fields = IndexMap::new();
        let add_morph = Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str("math.add".to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::Pure, vec![]));
        fields.insert("/add".to_string(), add_morph.clone());
        
        let mut math_builtins = IndexMap::new();
        math_builtins.insert("/add".to_string(), add_morph);
        let math_morphisms = vec![
            ("/sub",    "math.sub"),
            ("/mul",    "math.mul"),
            ("/div",    "math.div"),
            ("/rem",    "math.rem"),
            ("/abs",    "math.abs"),
            ("/bits",   "math.bits"),
            ("/pow",    "math.pow"),
            ("/sqrt",   "math.sqrt"),
            ("/bitAnd", "math.bitAnd"),
            ("/bitOr",  "math.bitOr"),
            ("/bitXor", "math.bitXor"),
            ("/bitNot", "math.bitNot"),
            ("/shl",    "math.shl"),
            ("/shr",    "math.shr"),
            ("/exp",    "math.exp"),
            ("/ln",     "math.ln"),
            ("/sin",    "math.sin"),
            ("/cos",    "math.cos"),
            ("/eml",    "math.eml"),
            // Phase 19 (previously missing from module)
            ("/min",    "math.min"),
            ("/max",    "math.max"),
            ("/floor",  "math.floor"),
            ("/ceil",   "math.ceil"),
            ("/round",  "math.round"),
            ("/clamp",  "math.clamp"),
            // Phase 27
            ("/gcd",    "math.gcd"),
            ("/lcm",    "math.lcm"),
            ("/sign",   "math.sign"),
            ("/log2",      "math.log2"),
            ("/log10",     "math.log10"),
            // Phase 35
            ("/factorial", "math.factorial"),
            ("/choose",    "math.choose"),
            ("/is_prime",  "math.is_prime"),
            ("/pow_mod",   "math.pow_mod"),
            // Phase 45
            ("/atan2",    "math.atan2"),
            ("/hypot",    "math.hypot"),
            ("/sinh",     "math.sinh"),
            ("/cosh",     "math.cosh"),
            ("/tanh",     "math.tanh"),
            ("/trunc",    "math.trunc"),
            ("/fract",    "math.fract"),
            ("/to_float", "math.to_float"),
            // Order wave W1 (SPEC_09 §3): numeric order predicates.
            ("/lt",  "math.lt"),
            ("/lte",  "math.lte"),
            ("/gt",  "math.gt"),
            ("/gte", "math.gte"),
        ];
        for (n, b) in math_morphisms { math_builtins.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::Pure, vec![]))); }
        math_builtins.insert("/random".to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str("math.random".to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::NonDet, vec![])));
        math_builtins.insert("one".to_string(), Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None));
        fields.insert("~%Math".to_string(), Value::Combo(ComboVal::new(math_builtins, true, IndexMap::new(), EffectTag::Pure, vec![])));

        let mut cond_fields = IndexMap::new();
        let cond_morphisms = vec![("/if", "cond.if"), ("/cond", "cond.cond"), ("/match", "cond.match")];
        for (n, b) in cond_morphisms { cond_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::Pure, vec![]))); }
        fields.insert("~%Cond".to_string(), Value::Combo(ComboVal::new(cond_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));

        let mut list_fields = IndexMap::new();
        let list_morphisms = vec![
            ("/map",       "list.map"),
            ("/filter",    "list.filter"),
            ("/fold",      "list.fold"),
            ("/len",       "list.len"),
            ("/concat",    "list.concat"),
            ("/at",        "list.at"),
            ("/sort",      "list.sort"),
            ("/reverse",   "list.reverse"),
            ("/slice",     "list.slice"),
            ("/zip",       "list.zip"),
            // Phase 17
            ("/flat_map",  "list.flat_map"),
            // Phase 18
            ("/any",       "list.any"),
            ("/all",       "list.all"),
            ("/find",      "list.find"),
            ("/head",      "list.head"),
            ("/tail",      "list.tail"),
            ("/take",      "list.take"),
            ("/drop",      "list.drop"),
            // Phase 19
            ("/count",     "list.count"),
            ("/zip_with",  "list.zip_with"),
            // Phase 22
            ("/partition", "list.partition"),
            ("/flatten",   "list.flatten"),
            ("/sum",       "list.sum"),
            ("/min_by",    "list.min_by"),
            ("/max_by",    "list.max_by"),
            // Phase 25
            ("/unique",    "list.unique"),
            ("/range",     "list.range"),
            ("/reduce",    "list.reduce"),
            // Phase 28
            ("/group_by",  "list.group_by"),
            ("/chunk",     "list.chunk"),
            ("/window",      "list.window"),
            // Phase 35
            ("/enumerate",   "list.enumerate"),
            ("/sort_by",     "list.sort_by"),
            ("/dedup",       "list.dedup"),
            ("/intersperse", "list.intersperse"),
            // Phase 45
            ("/scan",        "list.scan"),
            ("/take_while",  "list.take_while"),
            ("/drop_while",  "list.drop_while"),
            ("/product",     "list.product"),
            ("/transpose",   "list.transpose"),
        ];
        for (n, b) in list_morphisms { list_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::Pure, vec![]))); }
        fields.insert("~%List".to_string(), Value::Combo(ComboVal::new(list_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
        
        let mut string_fields = IndexMap::new();
        let string_morphisms = vec![
            ("/concat",      "str.concat"),
            ("/split",       "str.split"),
            ("/join",        "str.join"),
            ("/trim",        "str.trim"),
            ("/len",         "str.len"),
            ("/replace",     "str.replace"),
            ("/to_lower",    "str.to_lower"),
            ("/to_upper",    "str.to_upper"),
            ("/starts_with", "str.starts_with"),
            ("/ends_with",   "str.ends_with"),
            ("/contains",    "str.contains"),
            // Phase 19
            ("/parse_int",   "str.parse_int"),
            ("/from_int",    "str.from_int"),
            ("/repeat",      "str.repeat"),
            // Phase 21
            ("/format",      "str.format"),
            // Phase 25
            ("/char_at",     "str.char_at"),
            ("/chars",       "str.chars"),
            // Phase 27
            ("/index_of",    "str.index_of"),
            ("/pad_left",    "str.pad_left"),
            ("/pad_right",   "str.pad_right"),
            ("/trim_start",  "str.trim_start"),
            ("/trim_end",    "str.trim_end"),
            // Phase 32
            ("/reverse",     "str.reverse"),
            ("/count",       "str.count"),
            ("/slice",       "str.slice"),
            ("/is_empty",    "str.is_empty"),
            ("/parse_float", "str.parse_float"),
            ("/lines",       "str.lines"),
            // Phase 45
            ("/encode_uri",   "str.encode_uri"),
            ("/decode_uri",   "str.decode_uri"),
            ("/levenshtein",  "str.levenshtein"),
            ("/word_count",   "str.word_count"),
            ("/title_case",   "str.title_case"),
        ];
        for (n, b) in string_morphisms { string_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::Pure, vec![]))); }
        fields.insert("~%String".to_string(), Value::Combo(ComboVal::new(string_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
        
        let mut time_fields = IndexMap::new();
        time_fields.insert("/now".to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str("time.now".to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::IO, vec![])));
        let time_morphisms = vec![
            ("/format", "time.format"),
            ("/diff",   "time.diff"),
            ("/add_ms", "time.add_ms"),
            // Phase 45
            ("/parse",     "time.parse"),
            ("/to_iso8601","time.to_iso8601"),
            ("/add_days",  "time.add_days"),
            ("/add_hours", "time.add_hours"),
            ("/weekday",   "time.weekday"),
        ];
        for (n, b) in time_morphisms { time_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::Pure, vec![]))); }
        fields.insert("~%Time".to_string(), Value::Combo(ComboVal::new(time_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
        
        let mut bytes_fields = IndexMap::new();
        let bytes_morphisms = vec![
            ("/from_str",      "bytes.from_str"),
            ("/to_str",        "bytes.to_str"),
            ("/len",           "bytes.len"),
            ("/at",            "bytes.at"),
            ("/concat",        "bytes.concat"),
            ("/slice",         "bytes.slice"),
            ("/to_hex",        "bytes.to_hex"),
            ("/from_hex",      "bytes.from_hex"),
            // Phase 32
            ("/sha256",        "bytes.sha256"),
            ("/base64_encode", "bytes.base64_encode"),
            ("/base64_decode", "bytes.base64_decode"),
            ("/hmac_sha256",   "bytes.hmac_sha256"),
        ];
        for (n, b) in bytes_morphisms {
            bytes_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(),  Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])));
        }
        fields.insert("~%Bytes".to_string(), Value::Combo(ComboVal::new(bytes_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
        
        let mut regex_fields = IndexMap::new();
        let regex_morphisms = vec![
            ("/match",   "regex.match"),
            ("/find",    "regex.find"),
            ("/replace", "regex.replace"),
            ("/split",   "regex.split"),
        ];
        for (n, b) in regex_morphisms {
            regex_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(),  Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])));
        }
        fields.insert("~%Regex".to_string(), Value::Combo(ComboVal::new(regex_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
        
        let mut json_fields = IndexMap::new();
        let json_morphisms = vec![
            ("/parse",     "json.parse"),
            ("/stringify", "json.stringify"),
            ("/get",       "json.get"),
            ("/keys",      "json.keys"),
        ];
        for (n, b) in json_morphisms {
            json_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(),  Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])));
        }
        fields.insert("~%Json".to_string(), Value::Combo(ComboVal::new(json_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
        
        let mut io_fields = IndexMap::new();
        let io_morphisms = vec![
            ("/read_file",   "io.read_file"),
            ("/write_file",  "io.write_file"),
            ("/exists",      "io.exists"),
            ("/append_file", "io.append_file"),
        ];
        for (n, b) in io_morphisms {
            io_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(),  Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::IO, vec![])));
        }
        fields.insert("~%Io".to_string(), Value::Combo(ComboVal::new(io_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
        
        let mut env_fields = IndexMap::new();
        let env_morphisms = vec![
            ("/get",  "env.get"),
            ("/args", "env.args"),
            ("/cwd",  "env.cwd"),
        ];
        for (n, b) in env_morphisms {
            env_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(),  Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::IO, vec![])));
        }
        fields.insert("~%Env".to_string(), Value::Combo(ComboVal::new(env_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));

        let mut process_fields = IndexMap::new();
        let process_morphisms = vec![
            ("/exit", "process.exit"),
            ("/pid",  "process.pid"),
        ];
        for (n, b) in process_morphisms {
            process_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(),  Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::IO, vec![])));
        }
        fields.insert("~%Process".to_string(), Value::Combo(ComboVal::new(process_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
        
        let mut path_fields = IndexMap::new();
        let path_morphisms = vec![
            ("/join",        "path.join"),
            ("/dirname",     "path.dirname"),
            ("/basename",    "path.basename"),
            ("/extension",   "path.extension"),
            ("/is_absolute", "path.is_absolute"),
        ];
        for (n, b) in path_morphisms {
            path_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(),  Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])));
        }
        fields.insert("~%Path".to_string(), Value::Combo(ComboVal::new(path_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));

        // ~%Query module
        let mut query_fields = IndexMap::new();
        let qmorph = |name: &str, id: &str, eff: EffectTag| -> Value {
            let mut f = IndexMap::new();
            f.insert("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
            f.insert("%builtin".to_string(), Value::Atom(AtomKind::Str(id.to_string()), EffectTag::Pure, None));
            f.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("logic".to_string()), EffectTag::Pure, None));
            Value::Combo(ComboVal::new(f, true, IndexMap::new(), eff, vec![]))
        };
        query_fields.insert("/select".to_string(),     qmorph("/select",     "query.select",     EffectTag::Pure));
        query_fields.insert("/where".to_string(),      qmorph("/where",      "query.where",      EffectTag::IO));
        query_fields.insert("/pluck".to_string(),      qmorph("/pluck",      "query.pluck",      EffectTag::Pure));
        query_fields.insert("/deep_merge".to_string(), qmorph("/deep_merge", "query.deep_merge", EffectTag::Pure));
        let query_module = Value::Combo(ComboVal::new(query_fields, true, IndexMap::new(), EffectTag::Pure, vec![]));
        fields.insert("~%Query".to_string(), query_module);

        // ~%Diff module
        let mut diff_fields = IndexMap::new();
        let dmorph = |name: &str, id: &str, eff: EffectTag| -> Value {
            let mut f = IndexMap::new();
            f.insert("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
            f.insert("%builtin".to_string(), Value::Atom(AtomKind::Str(id.to_string()), EffectTag::Pure, None));
            f.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("logic".to_string()), EffectTag::Pure, None));
            Value::Combo(ComboVal::new(f, true, IndexMap::new(), eff, vec![]))
        };
        diff_fields.insert("/diff".to_string(),          dmorph("/diff",          "diff.diff",          EffectTag::Pure));
        diff_fields.insert("/patch".to_string(),         dmorph("/patch",         "diff.patch",         EffectTag::Pure));
        diff_fields.insert("/is_compatible".to_string(), dmorph("/is_compatible", "diff.is_compatible", EffectTag::Pure));
        let diff_module = Value::Combo(ComboVal::new(diff_fields, true, IndexMap::new(), EffectTag::Pure, vec![]));
        fields.insert("~%Diff".to_string(), diff_module);

        // ~%Set module
        let mut set_fields = IndexMap::new();
        let smorph = |name: &str, id: &str, eff: EffectTag| -> Value {
            let mut f = IndexMap::new();
            f.insert("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
            f.insert("%builtin".to_string(), Value::Atom(AtomKind::Str(id.to_string()), EffectTag::Pure, None));
            f.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("logic".to_string()), EffectTag::Pure, None));
            Value::Combo(ComboVal::new(f, true, IndexMap::new(), eff, vec![]))
        };
        set_fields.insert("/from_list".to_string(),    smorph("/from_list",    "set.from_list",    EffectTag::Pure));
        set_fields.insert("/union".to_string(),         smorph("/union",         "set.union",        EffectTag::Pure));
        set_fields.insert("/intersection".to_string(),  smorph("/intersection",  "set.intersection", EffectTag::Pure));
        set_fields.insert("/difference".to_string(),    smorph("/difference",    "set.difference",   EffectTag::Pure));
        set_fields.insert("/is_subset".to_string(),     smorph("/is_subset",     "set.is_subset",    EffectTag::Pure));
        set_fields.insert("/is_superset".to_string(),   smorph("/is_superset",   "set.is_superset",  EffectTag::Pure));
        set_fields.insert("/is_disjoint".to_string(),   smorph("/is_disjoint",   "set.is_disjoint",  EffectTag::Pure));
        set_fields.insert("/contains".to_string(),      smorph("/contains",      "set.contains",     EffectTag::Pure));
        let set_module = Value::Combo(ComboVal::new(set_fields, true, IndexMap::new(), EffectTag::Pure, vec![]));
        fields.insert("~%Set".to_string(), set_module);

        // ~%Stat module
        let mut stat_fields = IndexMap::new();
        stat_fields.insert("/mean".to_string(),        smorph("/mean",        "stat.mean",        EffectTag::Pure));
        stat_fields.insert("/variance".to_string(),    smorph("/variance",    "stat.variance",    EffectTag::Pure));
        stat_fields.insert("/std_dev".to_string(),     smorph("/std_dev",     "stat.std_dev",     EffectTag::Pure));
        stat_fields.insert("/median".to_string(),      smorph("/median",      "stat.median",      EffectTag::Pure));
        stat_fields.insert("/percentile".to_string(),  smorph("/percentile",  "stat.percentile",  EffectTag::Pure));
        stat_fields.insert("/histogram".to_string(),   smorph("/histogram",   "stat.histogram",   EffectTag::Pure));
        let stat_module = Value::Combo(ComboVal::new(stat_fields, true, IndexMap::new(), EffectTag::Pure, vec![]));
        fields.insert("~%Stat".to_string(), stat_module);

        // ~%Csv module
        let mut csv_fields = IndexMap::new();
        csv_fields.insert("/parse".to_string(),              smorph("/parse",              "csv.parse",              EffectTag::Pure));
        csv_fields.insert("/parse_with_headers".to_string(), smorph("/parse_with_headers", "csv.parse_with_headers", EffectTag::Pure));
        csv_fields.insert("/stringify".to_string(),          smorph("/stringify",          "csv.stringify",          EffectTag::Pure));
        csv_fields.insert("/read_csv".to_string(),           smorph("/read_csv",           "csv.read_csv",           EffectTag::IO));
        let csv_module = Value::Combo(ComboVal::new(csv_fields, true, IndexMap::new(), EffectTag::Pure, vec![]));
        fields.insert("~%Csv".to_string(), csv_module);

        // ~%Url module
        let mut url_fields = IndexMap::new();
        url_fields.insert("/parse".to_string(),        smorph("/parse",        "url.parse",        EffectTag::Pure));
        url_fields.insert("/encode".to_string(),       smorph("/encode",       "url.encode",       EffectTag::Pure));
        url_fields.insert("/decode".to_string(),       smorph("/decode",       "url.decode",       EffectTag::Pure));
        url_fields.insert("/join".to_string(),         smorph("/join",         "url.join",         EffectTag::Pure));
        url_fields.insert("/query_params".to_string(), smorph("/query_params", "url.query_params", EffectTag::Pure));
        let url_module = Value::Combo(ComboVal::new(url_fields, true, IndexMap::new(), EffectTag::Pure, vec![]));
        fields.insert("~%Url".to_string(), url_module);

        // ~%Toml module
        let mut toml_fields = IndexMap::new();
        toml_fields.insert("/parse".to_string(),     smorph("/parse",     "toml.parse",     EffectTag::Pure));
        toml_fields.insert("/stringify".to_string(), smorph("/stringify", "toml.stringify", EffectTag::Pure));
        let toml_module = Value::Combo(ComboVal::new(toml_fields, true, IndexMap::new(), EffectTag::Pure, vec![]));
        fields.insert("~%Toml".to_string(), toml_module);

        let mut disc_fields = IndexMap::new();
        let disc_morphisms = vec![("/connect", "disc.connect"), ("/fetch", "disc.fetch"), ("/identify", "disc.identify"), ("/identify_and_store", "engine.save"), ("/advertise", "disc.advertise"), ("/find", "disc.find")];
        for (n, b) in disc_morphisms { disc_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::IO, vec![]))); }
        fields.insert("~%Discovery".to_string(), Value::Combo(ComboVal::new(disc_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));

        // ~%Effect./runPure — SPEC_08 §4.3 / §6 privileged discharge.
        let mut effect_fields = IndexMap::new();
        effect_fields.insert(
            "/runPure".to_string(),
            Value::Combo(ComboVal::new(
                IndexMap::from_iter(vec![
                    (
                        "%morphism".to_string(),
                        Value::Atom(
                            AtomKind::Tag("true".to_string()),
                            EffectTag::Pure,
                            None,
                        ),
                    ),
                    (
                        "%builtin".to_string(),
                        Value::Atom(
                            AtomKind::Str("effect.run_pure".to_string()),
                            EffectTag::Pure,
                            None,
                        ),
                    ),
                ]),
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );
        fields.insert(
            "~%Effect".to_string(),
            Value::Combo(ComboVal::new(
                effect_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        let mut refl_fields = IndexMap::new();
        let refl_morphisms = vec![
            ("/keys",         "refl.keys"),
            ("/has",          "refl.has"),
            ("/is_cocoon",    "refl.is_cocoon"),
            ("/type_of",      "refl.type_of"),
            ("/is_blur",      "refl.is_blur"),
            ("/is_bottom",    "refl.is_bottom"),
            ("/is_some",      "refl.is_some"),
            ("/is_none",      "refl.is_none"),
            ("/is_ok",        "refl.is_ok"),
            ("/is_err",       "refl.is_err"),
            ("/to_str",       "refl.to_str"),
            ("/bottom_cause", "refl.bottom_cause"),
            ("/get",          "refl.get"),
            ("/set",          "refl.set"),
            ("/delete",       "refl.delete"),
            ("/values",       "refl.values"),
            ("/entries",      "refl.entries"),
        ];
        for (n, b) in refl_morphisms { refl_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::Pure, vec![]))); }
        fields.insert("~%Reflection".to_string(), Value::Combo(ComboVal::new(refl_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
        
        let mut complex_fields = IndexMap::new();
        let complex_morphisms = vec![("/conj", "complex.conj"), ("/phase", "complex.phase"), ("/real", "complex.real"), ("/imag", "complex.imag")];
        for (n, b) in complex_morphisms { complex_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::Pure, vec![]))); }
        fields.insert("~%Complex".to_string(), Value::Combo(ComboVal::new(complex_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));

        // @option: @Some { %val: _ } | #none  (SPEC_09 §2.7)
        let mut option_fields = IndexMap::new();
        option_fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("type".to_string()), EffectTag::Pure, None));
        option_fields.insert("%name".to_string(), Value::Atom(AtomKind::Str("option".to_string()), EffectTag::Pure, None));
        option_fields.insert(
            "%some".to_string(),
            Value::Combo(ComboVal::new(
                IndexMap::from_iter(vec![("%val".to_string(), Value::Top)]),
                false, IndexMap::new(), EffectTag::Pure, vec![],
            )),
        );
        option_fields.insert(
            "%none".to_string(),
            Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
        );
        option_fields.insert(
            "%fmap".to_string(),
            Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(), Value::Atom(AtomKind::Str("option.map".to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])),
        );
        let opt_morphisms = vec![
            ("/and_then",  "option.and_then"),
            ("/or",        "option.or"),
            ("/unwrap_or", "option.unwrap_or"),
            ("/filter",    "option.filter"),
            ("/expect",    "option.expect"),
            ("/zip",       "option.zip"),
            ("/flatten",   "option.flatten"),
        ];
        for (n, b) in opt_morphisms {
            option_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])));
        }
        fields.insert(
            "@option".to_string(),
            Value::Combo(ComboVal::new(option_fields, true, IndexMap::new(), EffectTag::Pure, vec![])),
        );

        // @result: @Ok { %val: _ } | @Err { %cause: _ }  (SPEC_09 §2.8)
        let mut result_fields = IndexMap::new();
        result_fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("type".to_string()), EffectTag::Pure, None));
        result_fields.insert("%name".to_string(), Value::Atom(AtomKind::Str("result".to_string()), EffectTag::Pure, None));
        result_fields.insert(
            "%ok".to_string(),
            Value::Combo(ComboVal::new(
                IndexMap::from_iter(vec![("%val".to_string(), Value::Top)]),
                false, IndexMap::new(), EffectTag::Pure, vec![],
            )),
        );
        result_fields.insert(
            "%err".to_string(),
            Value::Combo(ComboVal::new(
                IndexMap::from_iter(vec![("%cause".to_string(), Value::Top)]),
                false, IndexMap::new(), EffectTag::Pure, vec![],
            )),
        );
        result_fields.insert(
            "%fmap".to_string(),
            Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(), Value::Atom(AtomKind::Str("result.map".to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])),
        );
        result_fields.insert(
            "%map_err".to_string(),
            Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(), Value::Atom(AtomKind::Str("result.map_err".to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])),
        );
        let res_morphisms = vec![
            ("/and_then", "result.and_then"),
            ("/unwrap",   "result.unwrap"),
            ("/expect",   "result.expect"),
            ("/and",      "result.and"),
            ("/or",       "result.or"),
            ("/flatten",  "result.flatten"),
        ];
        for (n, b) in res_morphisms {
            result_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])));
        }
        fields.insert(
            "@result".to_string(),
            Value::Combo(ComboVal::new(result_fields, true, IndexMap::new(), EffectTag::Pure, vec![])),
        );

        // @list: Combo with %kind: #list  (SPEC_09 §2.x)
        let mut list_type_fields = IndexMap::new();
        list_type_fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("type".to_string()), EffectTag::Pure, None));
        list_type_fields.insert("%name".to_string(), Value::Atom(AtomKind::Str("list".to_string()), EffectTag::Pure, None));
        list_type_fields.insert(
            "%fmap".to_string(),
            Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(), Value::Atom(AtomKind::Str("list.map".to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), EffectTag::Pure, vec![])),
        );
        fields.insert(
            "@list".to_string(),
            Value::Combo(ComboVal::new(list_type_fields, true, IndexMap::new(), EffectTag::Pure, vec![])),
        );

        // ~%Engine: observe, save, /%differential.{1,2,3}
        fn engine_morph(name: &str, builtin: &str, effect: EffectTag) -> Value {
            Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(), Value::Atom(AtomKind::Str(builtin.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), effect, vec![]))
        }
        let mut engine_fields = IndexMap::new();
        engine_fields.insert("/observe".to_string(), engine_morph("/observe", "engine.observe", EffectTag::IO));
        engine_fields.insert("/save".to_string(), engine_morph("/save", "engine.save", EffectTag::IO));
        for i in 1u8..=3 {
            engine_fields.insert(format!("/%differential.{}", i), engine_morph(&format!("/%differential.{}", i), "engine.differential", EffectTag::Pure));
        }
        engine_fields.insert("/project_down".to_string(), engine_morph("/project_down", "engine.project_down", EffectTag::State));
        engine_fields.insert("/project_up".to_string(), engine_morph("/project_up", "engine.project_up", EffectTag::State));
        engine_fields.insert("/set_strategy".to_string(), engine_morph("/set_strategy", "engine.set_strategy", EffectTag::State));
        engine_fields.insert("/check_oml".to_string(), engine_morph("/check_oml", "engine.check_oml", EffectTag::Pure));
        engine_fields.insert("/equivalence_map".to_string(), engine_morph("/equivalence_map", "engine.equivalence_map", EffectTag::State));
        engine_fields.insert("/resolve".to_string(),         engine_morph("/resolve",         "engine.resolve",         EffectTag::State));
        let mut state_inner = IndexMap::new();
        state_inner.insert("differential".to_string(), Value::Atom(AtomKind::Tag("d1_converging".to_string()), EffectTag::Pure, None));
        // G-config: ~%Engine.state.strategy was a dead display (always #blur,
        // never tracked ctx overrides). Normative strategy home = ~%Config;
        // runtime override = /set_strategy. Removed the lying field.
        engine_fields.insert("state".to_string(), Value::Combo(ComboVal::new(state_inner, false, IndexMap::new(), EffectTag::Pure, vec![])));
        fields.insert("~%Engine".to_string(), Value::Combo(ComboVal::new(engine_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));

        // ~%Official: signing morphisms only. `architects` is NOT minted into
        // the universe root (universe_determinism / ORDER_01: trust root is a
        // governance object, not a per-process random self-appointment).
        // Whitelist lives in the assertion layer (`.oo/architects.json`).
        // Observing ~%Official.architects → #missing_key is the honest answer.
        let mut official_fields = IndexMap::new();
        fn official_morph(builtin: &str, effect: EffectTag) -> Value {
            Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(), Value::Atom(AtomKind::Str(builtin.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), effect, vec![]))
        }
        official_fields.insert("/sign_refine".to_string(), official_morph("engine.sign_refine", EffectTag::IO));
        // /add_architect retired (store_boundary: language surface must not
        // own the refine trust root; REAL_01 §7.2 out-of-band provisioning).
        fields.insert("~%Official".to_string(), Value::Combo(ComboVal::new(official_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));

        // ~%Config: genesis defaults (SPEC_08 §3.1 / SPEC_09 §6).
        // Bare field names on the data axis — path-observable as ~%Config.fuel.
        // No %-meta fallback (category error: % is node metadata, not config).
        let mut config_fields = IndexMap::new();
        config_fields.insert("fuel".to_string(), Value::Atom(AtomKind::Int(10000i64.into()), EffectTag::Pure, None));
        config_fields.insert("max_branches".to_string(), Value::Atom(AtomKind::Int(64i64.into()), EffectTag::Pure, None));
        config_fields.insert("max_unification_depth".to_string(), Value::Atom(AtomKind::Int(256i64.into()), EffectTag::Pure, None));
        config_fields.insert("max_lifting_depth".to_string(), Value::Atom(AtomKind::Int(32i64.into()), EffectTag::Pure, None));
        config_fields.insert("max_pattern_nodes".to_string(), Value::Atom(AtomKind::Int(1024i64.into()), EffectTag::Pure, None));
        config_fields.insert("timeout".to_string(), Value::Atom(AtomKind::Int(1000i64.into()), EffectTag::Pure, None));
        config_fields.insert("strategy".to_string(), Value::Atom(AtomKind::Tag("blur".to_string()), EffectTag::Pure, None));
        fields.insert(
            "~%Config".to_string(),
            Value::Combo(ComboVal::new(config_fields, true, IndexMap::new(), EffectTag::Pure, vec![])),
        );

        ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![])
    }

    pub fn eval_context(&self) -> EvalContext {
        let sys_root = self.root_with_system();
        let mut ctx = EvalContext::new(sys_root.clone());
        ctx.memo_enabled = false; // engine-internal: wrong root for memo (see field doc)
        ctx.privilege = self.privilege;
        // Initial horizon from ~%Config (bare names). Runtime override of
        // strategy is /set_strategy (mutates live ctx, not the genesis node).
        if let Some(Value::Combo(ref cfg)) = sys_root.get_field("~%Config").cloned() {
            if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("fuel").cloned() {
                if let Some(f) = n.to_u64() { ctx.fuel = f; }
            }
            if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("max_branches").cloned() {
                if let Some(v) = n.to_u64() { ctx.max_branches = v as usize; }
            }
            if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("max_unification_depth").cloned() {
                if let Some(v) = n.to_u64() { ctx.max_unification_depth = v as usize; }
            }
            if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("max_lifting_depth").cloned() {
                if let Some(v) = n.to_u64() { ctx.max_lifting_depth = v as usize; }
            }
            if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("max_pattern_nodes").cloned() {
                if let Some(v) = n.to_u64() { ctx.max_pattern_nodes = v as usize; }
            }
            if let Some(Value::Atom(AtomKind::Tag(s), _, _)) = cfg.get_field("strategy").cloned() {
                ctx.strategy = match s.trim_start_matches('#') {
                    "strict" => ObservationStrategy::Strict,
                    "approximate" => ObservationStrategy::Approximate,
                    _ => ObservationStrategy::Blur,
                };
            }
            if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("timeout").cloned() {
                if let Some(timeout_ms) = n.to_u64() {
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    ctx.timeout_deadline = Some(now_ms + timeout_ms);
                }
            }
        }
        ctx
    }

    pub fn apply_morphism(&self, f: Value, arg: Value, ctx: &mut EvalContext) -> Value {
        let f = self.force(f, ctx); if let Value::Bottom(_) = f { return f; } if f.is_top() { return Value::Top; }
        // G3 R1: Blur is not a callable / not a dispatchable arg — absorb
        // (pass through) rather than falling into the non-combo Conflict arm.
        if let Value::Blur(_) = &f { return f; }
        let arg = self.force(arg, ctx); if let Value::Bottom(_) = arg { return arg; }
        if let Value::Blur(_) = &arg { return arg; }
        // bind additivity (SPEC_07 §4, ENGINE_SYNC #18): a superposed argument
        // evolves branchwise — f(A|B) = f(A) | f(B); ⊥ branches prune (| identity)
        if let Value::Union(branches) = arg {
            let mut out = Vec::new();
            for b in branches {
                let res = self.apply_morphism(f.clone(), b, ctx);
                if !matches!(res, Value::Bottom(_)) && !matches!(res, Value::Atom(nlang_parser::ast::AtomKind::Bottom, _, _)) {
                    out.push(res);
                }
            }
            return self.normalize_union_absorbing(out, ctx);
        }
        if !f.is_morphism() { if arg.is_morphism() { return self.apply_morphism(arg, f, ctx); } }
        match f {
            Value::Combo(ref c) => {
                if let Some(inner) = c.get_field("%val") { return self.apply_morphism(inner.clone(), arg, ctx); }
                
                let is_arg_pack = match &arg { 
                    Value::Combo(ac) => ac.contains_key("%arg") || (ac.contains_key("0") && !ac.contains_key("%kind")), 
                    _ => false 
                };
                // Positional (numeric) keys from the morphism, then from the
                // argument. Named non-`%` fields of the argument are also
                // lifted to top-level so builtins that read SPEC_08 §3.5
                // named parameters (check_oml a/b, set_strategy strategy, …)
                // can see them. Conflict rule: argument wins (same as
                // arg-pack positional overwrite).
                let mut nf = IndexMap::new(); 
                for (k, v) in c.fields() {
                    if k.parse::<usize>().is_ok() {
                        nf.insert(k.clone(), v.clone());
                    }
                }
                if is_arg_pack { 
                    if let Value::Combo(ref ac) = arg { 
                        for (k, v) in &ac.fields() {
                            if k.parse::<usize>().is_ok() {
                                nf.insert(k.clone(), v.clone());
                            }
                        } 
                    } 
                } else { 
                    let mut max_idx = -1i32; 
                    for k in nf.keys() {
                        if let Ok(idx) = k.parse::<i32>() {
                            if idx > max_idx {
                                max_idx = idx;
                            }
                        }
                    } 
                    nf.insert((max_idx + 1).to_string(), arg.clone()); 
                }
                // Named public fields from the argument (not %, not digit keys).
                // Force each so builtins that pattern-match Atom/Tag without an
                // internal force (e.g. set_strategy) see solid values. Positional
                // keys keep Stage-2 Thunk laziness.
                if let Value::Combo(ref ac) = arg {
                    for (k, v) in &ac.fields() {
                        if k.starts_with('%') {
                            continue;
                        }
                        if k.parse::<usize>().is_ok() {
                            continue;
                        }
                        let forced = self.force(v.clone(), ctx);
                        nf.insert(k.clone(), forced);
                    }
                }
                let unified_arg = Value::Combo(ComboVal::new(nf, true, IndexMap::new(), arg.effect(), vec![]));
                
                if let Some(Value::Combo(rules_source)) = c.get_field("%rules") {
                    let dispatch_result = self.dispatch_morphism(rules_source, &arg, ctx);
                    return dispatch_result.to_value(f.effect());
                }

                // Pattern-key dispatch table (`{ @{ 4.. }: "A", ... }`): non-meta
                // non-numeric keys + `%morphism`, no `%rules`/`%builtin`.
                // Numeric keys alone are curry slots (partial apply of builtins) —
                // must NOT be treated as patterns (would steal math.add partials).
                if c.get_field("%morphism").is_some()
                    && c.get_field("%rules").is_none()
                    && c.get_field("%builtin").is_none()
                {
                    let has_pattern_fields = c.all_fields_iter().any(|(k, _)| {
                        !k.starts_with('%') && k.parse::<usize>().is_err()
                    });
                    if has_pattern_fields {
                        let dispatch_result = self.dispatch_morphism(c, &arg, ctx);
                        return dispatch_result.to_value(f.effect());
                    }
                }
                
                if let Some(Value::Atom(AtomKind::Str(builtin_id), _, _)) = c.get_field("%builtin") { 
                    if let Some(func) = self.builtin_registry.get(builtin_id) { 
                        let res = func(unified_arg.clone(), self, ctx); 
                        if let Value::Top = res { 
                            let mut partial_fields = c.fields().clone(); 
                            if let Value::Combo(ref ac) = unified_arg { 
                                for (k, v) in &ac.fields() { partial_fields.insert(k.clone(), v.clone()); } 
                            } 
                            partial_fields.insert("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None));
                            partial_fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("logic".to_string()), EffectTag::Pure, None));
                            return Value::Combo(ComboVal::new(partial_fields, true, IndexMap::new(), f.effect(), vec![])); 
                        } 
                        return res; 
                    } 
                }
                
                let ks = arg.collapse().to_string_plain();
                if let Some(v) = c.get_field(&ks).or_else(|| c.get_field("it")).or_else(|| c.get_field("_")) {
                    return v.clone();
                }
                BottomCause::Conflict.into()
            }
            _ => BottomCause::Conflict.into(),
        }
    }

    /// Evaluate an expression and force the result recursively — the
    /// **observation** view. This mirrors `universe.observe`'s solidification
    /// of the return value (GUIDE_03 §11.5): eval returns thunks for
    /// unevaluated fields, force_recursive solidifies them.
    ///
    /// Call this from test harnesses, REPL, and any site that consumes the
    /// *value* of an expression (rather than its structure). `oo.eval` is the
    /// pre-observation API; `oo.eval_observed` is the observation API. The
    /// distinction is now part of the engine's public surface.
    pub fn eval_observed(&self, expr: &Expr, ctx: &mut EvalContext) -> Value {
        let v = self.eval(expr, ctx);
        self.force_recursive(v, ctx)
    }

    pub fn force(&self, val: Value, ctx: &mut EvalContext) -> Value {
        match val {
            Value::Thunk { expr, closure, context, effect } => {
                // Stage 4 (§4b): force-level memo with tier strategy.
                // Stage 5 (§5-pre): staged gate — evolve-time forces read
                // staged, which the key does not include.
                let effective_context: Option<Value> = context.as_ref().map(|b| (**b).clone()).or(ctx.context_value.clone());
                let tier: Option<Tier> = classify_tier(&expr).0.into();
                let should_memo = matches!(tier, Some(Tier::C) | Some(Tier::M));
                let staged_ok = ctx.staged.is_none() && ctx.memo_enabled;
                let memo_key = if should_memo && staged_ok {
                    let expr_caid = {
                        let mut h = sha2::Sha256::new();
                        h.update(expr.to_nlang(0).as_bytes());
                        ContentHash::v1(h.finalize().to_vec())
                    };
                    let frame_caid = {
                        let cv = Value::Combo(ComboVal::new(
                            closure.iter().enumerate().flat_map(|(i, cv)| {
                                vec![(i.to_string(), Value::Combo(cv.clone()))]
                            }).collect(),
                            true, IndexMap::new(), EffectTag::Pure, vec![]));
                        cv.content_hash()
                    };
                    let context_caid = effective_context.as_ref().map(|v| v.content_hash());
                    Some(ForceMemoKey { expr_caid, frame_caid, context_caid })
                } else { None };

                if let Some(ref k) = memo_key {
                    if let Ok(memo) = self.force_memo.read() {
                        if let Some(entry) = memo.get(k) {
                            // Stage 5 acceptance fix: a HIT must float the
                            // entry's deps into the active outer collector —
                            // an outer entry built over this hit embeds this
                            // value, so it inherits these invalidation edges
                            // (probe p1_hit_path_must_float_transitive_deps).
                            if let Some(ref mut c) = ctx.dep_collector {
                                c.extend(entry.deps.iter().cloned());
                            }
                            let mut res = entry.value.clone();
                            res = match res {
                                Value::Atom(kind, old_e, r) => {
                                    Value::Atom(kind, old_e.union(effect), r)
                                }
                                Value::Combo(mut cv) => {
                                    cv.effect = cv.effect.union(effect);
                                    Value::Combo(cv)
                                }
                                Value::Bottom(_)
                                | Value::Blur(_)
                                | Value::Top
                                | Value::TopCaused { .. } => res,
                                other if !other.effect().contains_all(effect) => {
                                    let e = effect.union(other.effect());
                                    Value::Combo(ComboVal::new(
                                        IndexMap::from_iter(vec![("%val".to_string(), other)]),
                                        true,
                                        IndexMap::new(),
                                        e,
                                        vec![],
                                    ))
                                }
                                other => other.with_effect(effect),
                            };
                            return res;
                        }
                    }
                }

                // L2-17 / forward-ref: same-thunk re-entry → ⊥ #divergent
                // (before stack/fuel). Identity = content_hash of the thunk
                // (expr+frame+context).
                //
                // Path-shaped thunks (`out: mid`): do NOT mark the *target*
                // path (`mid`) into `computing` — that collides with
                // force_coord of the binding that *lives* at mid (false
                // #divergent on bare reference chains). Cycle detection for
                // path self-loops keys on the *holder* coordinate already
                // placed in `computing` by force_coord (see below), or on
                // in_flight content-hash when solidifying a re-fetched Thunk.
                let thunk_id = Value::Thunk {
                    expr: expr.clone(),
                    closure: closure.clone(),
                    context: context.clone(),
                    effect,
                }
                .content_hash();
                if ctx.in_flight.contains(&thunk_id) {
                    // SPEC_12 §1.1: pure-ref cycle → caused Top; transform → ⊥.
                    // The re-entered thunk's own expr coordinate is a loop
                    // member (repair: chain alone missed the mutual partner).
                    let rc = path_coord_of(&expr);
                    return cycle_reentry(ctx, rc.as_deref());
                }
                // Holder re-entry: force_coord("s.v") put "s.v" in computing;
                // a path-shaped thunk whose expr is that same path is the
                // self-loop case. A *reference* thunk at `out` with expr `mid`
                // only sees "out" in computing, not "mid" — no false hit.
                let path_coord = path_coord_of(&expr);
                if let Some(ref pc) = path_coord {
                    if ctx.computing.contains(pc) {
                        return cycle_reentry(ctx, Some(pc));
                    }
                }

                // Stage 5 (§5a): nested dep_collector propagation.
                // Install fresh collector in call_ctx; after eval, merge
                // inner deps into outer collector (inner result embeds
                // inner deps — they must float up).
                let had_inner_collector = ctx.dep_collector.is_some();
                let inner_collector = if should_memo && staged_ok {
                    Some(HashSet::new())
                } else { None };

                let mut call_ctx = self.sub_context(ctx);
                // SPEC_04 §2.1 completion: when forcing a field found on the
                // scope chain (`lexical_forcing` non-empty), keep ambient
                // frames so chained siblings recurse at any depth. Definition
                // frames from the thunk still push innermost (rev-search).
                // Outside lexical force, preserve historical replace semantics
                // (capture isolation; cycle_test / L2-17 ambient-free).
                if ctx.lexical_forcing.is_empty() {
                    call_ctx.scopes = closure;
                } else {
                    call_ctx.scopes = ctx.scopes.clone();
                    for frame in &closure {
                        call_ctx.scopes.push(frame.clone());
                    }
                }
                // SPEC_12 §1.1: non-pure-ref hop taints the whole force chain.
                // Lattice join/meet/diff are structural (see
                // `expr_is_lattice_structural`) — not transform hops.
                if !expr_is_pure_ref(&expr) && !expr_is_lattice_structural(&expr) {
                    call_ctx.chain_transform_taint = true;
                }
                call_ctx.context_value = effective_context;
                call_ctx.dep_collector = inner_collector;
                // in_flight rides sub_context clone for nested observation.
                call_ctx.in_flight.insert(thunk_id.clone());
                let res = self.eval(&expr, &mut call_ctx);
                // Solidify residual Thunks under the same in_flight set:
                // - Path-shaped: re-fetched Thunk of the same expr hits
                //   content-hash re-entry (path self-loop). Do not mark path
                //   targets into computing (forward-ref fix).
                // - Lens / bare field (e.g. `$.k`): navigate leaves the field
                //   Thunk unforced (GUIDE_03 §11.4); one more force peels to
                //   the value so lattice unify of sealed siblings (`k:1` from
                //   different holders) meets on the atom, not frame-tagged
                //   Thunk identity.
                let mut res = res;
                let mut peel = 0u32;
                while matches!(res, Value::Thunk { .. }) && peel < 32 {
                    res = self.force(res, &mut call_ctx);
                    peel += 1;
                }
                call_ctx.in_flight.remove(&thunk_id);
                ctx.fuel = call_ctx.fuel;
                ctx.in_flight = call_ctx.in_flight;
                ctx.computing = call_ctx.computing;
                ctx.lexical_forcing = call_ctx.lexical_forcing;
                // Taint scoping (SPEC_12 §1.1 Q2/Q4; taint_scope arc):
                // chain_transform_taint is *chain-local*. Downward inheritance
                // via sub_context clone is correct (a real transform hop
                // taints its own force chain; re-entry inside that chain
                // still sees the flag). Upward write-back was the bug —
                // "once transform, always transform" globalized taint to the
                // whole observation ctx, so forcing an unrelated non-pure
                // sibling (even literal `9`) permanently poisoned later
                // static-cycle re-entries → false #divergent → silent cull.
                // Do NOT write chain_transform_taint back.
                // cycle_chain: healthy paths push/pop balanced; write-back
                // is identity on balanced exit. Keep (minimal diff); members
                // list for %members still filled at re-entry sites.
                ctx.cycle_chain = call_ctx.cycle_chain;
                let inner_deps = call_ctx.dep_collector.take();

                // Do NOT wrap Bottom/Blur/Top in a pure-wrapper for effect
                // escalation — that shell traps `.%cause` navigation (meta
                // segment treated as on-shell, peel skipped, open miss Top).
                let res = match res {
                    Value::Atom(kind, old_e, r) => Value::Atom(kind, old_e.union(effect), r),
                    Value::Combo(mut cv) => {
                        cv.effect = cv.effect.union(effect);
                        Value::Combo(cv)
                    }
                    Value::Bottom(_)
                    | Value::Blur(_)
                    | Value::Top
                    | Value::TopCaused { .. } => res,
                    other if !other.effect().contains_all(effect) => {
                        let e = effect.union(other.effect());
                        Value::Combo(ComboVal::new(
                            IndexMap::from_iter(vec![("%val".to_string(), other)]),
                            true,
                            IndexMap::new(),
                            e,
                            vec![],
                        ))
                    }
                    other => other.with_effect(effect),
                };

                let deps = inner_deps.unwrap_or_default();

                // Merge inner deps into outer collector.
                if let Some(ref mut outer) = ctx.dep_collector {
                    for d in &deps { outer.insert(d.clone()); }
                }

                // Stage 5 (§5b): insert into force_memo with deps + reverse index.
                // L2-17: #divergent may be memoized (handover); other Bottoms stay out.
                if let Some(ref k) = memo_key {
                    let memoizable = match &res {
                        Value::Bottom(d) if matches!(d.cause, BottomCause::Divergent) => true,
                        Value::Bottom(_) => false,
                        // Memoize only when NonDet is absent (nondet must not
                        // be cached across observations).
                        _ => !res.contains_blur() && !res.effect().contains(EffectTag::NonDet),
                    };
                    if memoizable {
                        if let Ok(mut memo) = self.force_memo.write() {
                            const FORCE_MEMO_CAP: usize = 100_000;
                            if memo.len() >= FORCE_MEMO_CAP { memo.clear(); if let Ok(mut rev) = self.force_memo_rev.write() { rev.clear(); } }
                            if let Ok(mut rev) = self.force_memo_rev.write() {
                                for dep in &deps {
                                    rev.entry(dep.clone()).or_default().insert(k.clone());
                                }
                            }
                            memo.insert(k.clone(), MemoEntry { value: res.clone(), deps });
                        }
                    }
                }
                res
            }
            Value::Blur(_) => val,
            Value::Ref(path) => {
                // Stage 3 (§3a): dereference — resolve path against ctx.root
                // at observation time. fuel charged here (force = observation
                // primitive, GUIDE_03 §11.4).
                // Stage 5 (§5a): deref is a universal root read — any evolve
                // invalidates (conservative over-approximation, correct for
                // self-referential chains).
                self.record_dep(ctx, "*");
                let cost = if path.segments.is_empty() { 32 } else { 1 + path.segments.len() as u64 };
                if let Err(e) = ctx.check_resources(cost) {
                    return handle_resource_exhausted(e, ctx.strategy, &ctx.horizon_salt, ctx.fuel, None, EffectTag::Pure);
                }
                self.resolve_path_internal(&path, ctx)
            }
            // forward_spread: private force_coord skips the computing gate and
            // only calls force — expand pending spreads here too so bits/pipe
            // on `~d: {...~c, z:3}` see merged fields.
            Value::Combo(c) if !c.pending_spreads.is_empty() => {
                self.expand_combo_pending(c, ctx)
            }
            _ => val,
        }
    }

    pub fn force_recursive(&self, val: Value, ctx: &mut EvalContext) -> Value {
        // Stage 3 (§3c): solidification must participate in depth accounting —
        // a self-referential Ref chain (v: <<_.>>) recurses through here, not
        // through eval, so without this guard the Rust stack dies before the
        // fuel horizon ever engages. Depth exhaustion is the same semantic
        // truncation as fuel: the horizon, not an error.
        ctx.depth += 1;
        if let Err(e) = ctx.check_resources(0) {
            ctx.depth -= 1;
            return handle_resource_exhausted(e, ctx.strategy, &ctx.horizon_salt, ctx.fuel, None, EffectTag::Pure);
        }
        // F1 (§3-fix): when force_recursive deref's a Ref, the resolved value
        // becomes the $ binding for all thunks forced in the subtree (SYNTAX_07
        // §2.4: "binding occurs at observation time"). Scope via save/restore
        // so sibling observations are not polluted.
        let is_ref = matches!(&val, Value::Ref(_));
        let val = self.force(val, ctx);
        let old_ctx_val = if is_ref {
            let old = ctx.context_value.take();
            ctx.context_value = Some(val.clone());
            old
        } else { None };
        let res = match val {
            // Stage 2: force may return a Thunk if the underlying eval hit a
            // navigate_segments that returned an unforced field thunk (GUIDE_03
            // §11.4 — path-directed observe forces only path nodes, not the
            // final field). force_recursive must keep forcing until non-Thunk.
            Value::Thunk { .. } => self.force_recursive(val, ctx),
            Value::Combo(c) => {
                // forward_spread: expand deferred sources before solidifying fields.
                let expanded = self.expand_combo_pending(c, ctx);
                match expanded {
                    Value::Combo(c) => {
                        let mut new_c = ComboVal::default();
                        new_c.closed = c.closed;
                        new_c.effect = c.effect;
                        new_c.relations = c.relations.clone();
                        // forward_spread acceptance repair: re-queued pending
                        // sources (evolve-phase Top) must survive the rebuild.
                        new_c.pending_spreads = c.pending_spreads.clone();
                        for (k, v) in c.all_fields_iter() {
                            new_c.insert_field(&k, self.force_recursive(v, ctx));
                        }
                        for (k, v) in c.local.iter() {
                            new_c
                                .local
                                .insert(k.clone(), self.force_recursive(v.clone(), ctx));
                        }
                        Value::Combo(new_c)
                    }
                    other => other,
                }
            }
            // T2 (union_cull): force each branch, then cull ⊥ (SPEC_08
            // §3.2.2 #5). Observation exit of field `|` must not leak
            // `⊥ | 5`. All-⊥ → primary member ⊥ verbatim.
            Value::Union(branches) => {
                let mut survivors: Vec<Value> = Vec::new();
                let mut culled: Vec<BottomDetail> = Vec::new();
                for b in branches {
                    let forced = self.force_recursive(b, ctx);
                    match forced {
                        Value::Bottom(d) => culled.push(*d),
                        other => survivors.push(other),
                    }
                }
                if survivors.is_empty() {
                    primary_bottom_from_culled(culled)
                } else {
                    // SPEC_01 §2.4.2: absorb after solidify+cull.
                    self.normalize_union_absorbing(survivors, ctx)
                }
            }
            _ => val
        };
        if is_ref { ctx.context_value = old_ctx_val; }
        ctx.depth -= 1;
        res
    }

    pub fn resolve_path(&self, path: &Path, ctx: &mut EvalContext) -> Value {
        let name_raw = if !path.segments.is_empty() { &path.segments[0] } else { "" };
        let name = name_raw.trim();
        
        if path.anchor == PathAnchor::Bare && path.segments.len() == 1 {
            if name == "#_|_" { return Value::Atom(AtomKind::TagStart, EffectTag::Pure, None); }
            if name == "#_" { return Value::Atom(AtomKind::TagEnd, EffectTag::Pure, None); }
            if name == "_" { return Value::Top; }
            // Dual of Top normalization: literal `_|_` is Value::Bottom
            // (Conflict), not Atom(AtomKind::Bottom) — SYNTAX_06 §4.1 absorption.
            if name == "_|_" {
                return crate::value::BottomCause::Conflict.into();
            }
            
            // E4: only *builtin* `@name` short-circuits to a type_constraint
            // marker. User `@Name` falls through the normal lookup chain
            // (scopes → staged → root; force + record_dep). Not-found keeps
            // the Unknown marker pass-through (e4_undefined_typeref_passthrough).
            // Builtins are reserved and not shadowable.
            if TypeConstraint::is_type_constraint_path(name) {
                let type_name = name.trim_start_matches('@');
                if TypeConstraint::is_builtin_type_name(type_name) {
                    return TypeConstraint::marker_value(type_name);
                }
            }

            // G6: bare single-segment Ref is returned unforced so observe can
            // treat Ref-mediated paths as structural view (SYNTAX_07 §4 #6).
            // Judgment sites (math/cmp/force_recursive) still force Refs.
            //
            // Scope-frame hits use force_lexical_name: ambient frames stay on
            // the chain (any hop depth) with soft re-entry → Top for mutual
            // sibling pins (not #divergent).
            let return_or_force = |oo: &Self, n: &str, val: Value, ctx: &mut EvalContext, use_coord: bool| -> Value {
                if matches!(&val, Value::Ref(_)) {
                    return val;
                }
                if use_coord {
                    oo.force_coord(n, val, ctx)
                } else {
                    oo.force(val, ctx)
                }
            };

            // Iterate by index so we can call force_lexical without holding
            // a borrow on ctx.scopes.
            for i in (0..ctx.scopes.len()).rev() {
                let scope = ctx.scopes[i].clone();
                // Scope frames: lexical force (parameter rebinding is not a
                // coordinate cycle — force_coord here false-triggers HOFs).
                if let Some(val) = scope.get_field(name).cloned() {
                    return self.force_lexical_name(name, val, ctx, false);
                }
                if let Some(val) = scope.local_fields().get(name).cloned() {
                    return self.force_lexical_name(name, val, ctx, false);
                }
                let prefixes = vec!["/", "@", "~", "~%"];
                for p in prefixes {
                    let alt_name = if name.starts_with(p) { name.trim_start_matches(p).to_string() } else { format!("{}{}", p, name) };
                    if let Some(val) = scope.get_field(&alt_name).cloned() {
                        return self.force_lexical_name(&alt_name, val, ctx, false);
                    }
                    if let Some(val) = scope.get_local_field(&alt_name).cloned() {
                        return self.force_lexical_name(&alt_name, val, ctx, false);
                    }
                }
            }
            // SPEC_09 §6: ~%Config binding = effective (genesis ∧ overrides),
            // never the staged fragment alone (display + path reads).
            if name == "~%Config" {
                if let Some(eff) =
                    crate::universe::effective_config(&ctx.root, ctx.staged.as_ref())
                {
                    self.record_dep(ctx, "~%Config");
                    return Value::Combo(eff);
                }
            }
            if let Some(ref s) = ctx.staged {
                // Public staged fields use force_coord (self/mutual cycles).
                // Local (~private) fields: plain force — re-entry during HOF
                // application is not a lattice cycle (list.fold + ~f).
                if let Some(val) = s.get_field(name) {
                    return return_or_force(self, name, val.clone(), ctx, true);
                }
                if let Some(val) = s.get_local_field(name) {
                    return return_or_force(self, name, val.clone(), ctx, false);
                }
                let prefixes = vec!["/", "@", "~", "~%"];
                for p in prefixes {
                    let alt_name = if name.starts_with(p) { name.trim_start_matches(p).to_string() } else { format!("{}{}", p, name) };
                    if let Some(val) = s.get_field(&alt_name) {
                        return return_or_force(self, &alt_name, val.clone(), ctx, true);
                    }
                    if let Some(val) = s.get_local_field(&alt_name) {
                        return return_or_force(self, &alt_name, val.clone(), ctx, false);
                    }
                }
            }
            if let Some(val) = ctx.root.get_field(name) {
                let v = val.clone();
                self.record_dep(ctx, name);
                return return_or_force(self, name, v, ctx, true);
            }
            if let Some(val) = ctx.root.get_local_field(name) {
                let v = val.clone();
                self.record_dep(ctx, name);
                return return_or_force(self, name, v, ctx, false);
            }
            let prefixes = vec!["/", "@", "~", "~%"];
            for p in prefixes {
                let alt_name = if name.starts_with(p) { name.trim_start_matches(p).to_string() } else { format!("{}{}", p, name) };
                if let Some(val) = ctx.root.get_field(&alt_name) {
                    return return_or_force(self, &alt_name, val.clone(), ctx, true);
                }
                if let Some(val) = ctx.root.get_local_field(&alt_name) {
                    return return_or_force(self, &alt_name, val.clone(), ctx, false);
                }
            }

            // Non-builtin @Name not found → Unknown marker (same shape as
            // the old always-marker path; validate = unconditional pass).
            if TypeConstraint::is_type_constraint_path(name) {
                return TypeConstraint::marker_value(name.trim_start_matches('@'));
            }
        }
        self.resolve_path_internal(path, ctx)
    }

    fn resolve_path_internal(&self, path: &Path, ctx: &mut EvalContext) -> Value {
        let start_val: Value = match path.anchor {
            PathAnchor::Root => Value::Combo((*ctx.root).clone()),
            PathAnchor::Bare => {
                let name = if !path.segments.is_empty() { path.segments[0].trim() } else { "" };
                let mut found = None;
                for scope in ctx.scopes.iter().rev() {
                    if let Some(val) = scope.get_field(name) { found = Some(val.clone()); break; }
                    if let Some(val) = scope.get_local_field(name) { found = Some(val.clone()); break; }
                    let prefixes = vec!["/", "@", "~", "~%"];
                    for p in prefixes { let alt_name = if name.starts_with(p) { name.trim_start_matches(p).to_string() } else { format!("{}{}", p, name) }; if let Some(val) = scope.get_field(&alt_name) { found = Some(val.clone()); break; } if let Some(val) = scope.get_local_field(&alt_name) { found = Some(val.clone()); break; } }
                    if found.is_some() { break; }
                }
                // SPEC_09 §6: never bind staged Config fragment as ~%Config;
                // multi-segment reads (~%Config.timeout) need genesis ∧ override.
                if found.is_none() && name == "~%Config" {
                    if let Some(eff) =
                        crate::universe::effective_config(&ctx.root, ctx.staged.as_ref())
                    {
                        found = Some(Value::Combo(eff));
                        self.record_dep(ctx, "~%Config");
                    }
                }
                if found.is_none() { if let Some(ref s) = ctx.staged { if let Some(val) = s.get_field(name).or_else(|| s.get_local_field(name)) { found = Some(val.clone()); self.record_dep(ctx, name); } else { let prefixes = vec!["/", "@", "~", "~%"]; for p in prefixes { let alt_name = if name.starts_with(p) { name.trim_start_matches(p).to_string() } else { format!("{}{}", p, name) }; if let Some(val) = s.get_field(&alt_name).or_else(|| s.get_local_field(&alt_name)) { found = Some(val.clone()); self.record_dep(ctx, &alt_name); break; } } } } }
                if found.is_none() { if let Some(val) = ctx.root.get_field(name).or_else(|| ctx.root.get_local_field(name)) { found = Some(val.clone()); self.record_dep(ctx, name); } else { let prefixes = vec!["/", "@", "~", "~%"]; for p in prefixes { let alt_name = if name.starts_with(p) { name.trim_start_matches(p).to_string() } else { format!("{}{}", p, name) }; if let Some(val) = ctx.root.get_field(&alt_name).or_else(|| ctx.root.get_local_field(&alt_name)) { found = Some(val.clone()); self.record_dep(ctx, &alt_name); break; } } } }
                match found { Some(v) => {
                    let is_ref = matches!(&v, Value::Ref(_));
                    // force_coord only for public root/staged coordinates;
                    // private/local re-entry during HOF is not a cycle.
                    let forced = if name.starts_with('~') {
                        self.force(v, ctx)
                    } else {
                        self.force_coord(name, v, ctx)
                    };
                    if path.segments.len() > 1 {
                        // F1 (§3-fix): when resolving through a Ref, the deref'd
                        // value becomes the $ binding for thunks forced in the
                        // remaining path traversal (SYNTAX_07 §2.4: "binding
                        // occurs at observation time"). Scoped to the deref
                        // subtree — saved/restored so sibling observations are
                        // not polluted.
                        let old_ctx_val = if is_ref {
                            let old = ctx.context_value.take();
                            ctx.context_value = Some(forced.clone());
                            old
                        } else { None };
                        let res = self.navigate_segments(forced, &path.segments[1..], ctx, name);
                        if is_ref { ctx.context_value = old_ctx_val; }
                        return res;
                    }
                    // Single-segment: no navigation subtree — return deref'd
                    // value directly, no context scoping needed. But if this
                    // Ref observation feeds into force_recursive, the recursion
                    // needs the deref context (handled in force_recursive F1).
                    forced
                } None => Value::Top }
            }
            // F4b + caret Q1: parent-anchor ascent on the container chain.
            // Encoding (parser, unchanged): Parent(0) = `^.`, Parent(1) = `^^.`,
            // … — `n` is "extra carets past the first". hops = n+1 levels up
            // from the *current* container (scopes.last()). Chain outermost is
            // the root universe (beyond scopes[0]); hop past root →
            // #out_of_horizon. Strict coordinate access after landing (Q2):
            // missing key at that level is open `_`, no further ancestor walk.
            PathAnchor::Parent(count) => {
                // Parent(0)=^ → 1 hop to parent (NOT the current frame).
                let hops = count as usize + 1;
                let len = ctx.scopes.len();
                if hops < len {
                    // Still inside sealed frames: scopes[len-1] is current,
                    // scopes[len-1-hops] is the designated ancestor.
                    Value::Combo(ctx.scopes[len - 1 - hops].clone())
                } else if hops == len {
                    // Exactly through all frames → root universe.
                    Value::Combo((*ctx.root).clone())
                } else {
                    // Past root.
                    return BottomCause::OutOfHorizon.into();
                }
            }
            PathAnchor::Current => { if let Some(top) = ctx.scopes.last() { Value::Combo(top.clone()) } else { Value::Combo((*ctx.root).clone()) } }
        };
        if !path.segments.is_empty() && !matches!(path.anchor, PathAnchor::Bare) {
            self.navigate_segments(start_val, &path.segments, ctx, "")
        } else {
            start_val
        }
    }

    /// Shallow-force public + local fields and test for embedded type markers.
    /// Used by derived `%super` for user field-types whose `@T` fields are
    /// still Stage-2 Thunks at observation (not yet solid markers).
    fn combo_embeds_type_marker_shallow(
        &self,
        c: &ComboVal,
        ctx: &mut EvalContext,
    ) -> bool {
        let check = |v: Value, oo: &Self, ctx: &mut EvalContext| -> bool {
            let mut v = v;
            let mut n = 0u32;
            while matches!(&v, Value::Thunk { .. }) && n < 8 {
                v = oo.force(v, ctx);
                n += 1;
            }
            match v {
                Value::Combo(ref inner) if is_type_constraint_combo(inner) => true,
                Value::Combo(ref inner) => oo.combo_embeds_type_marker_shallow(inner, ctx),
                _ => false,
            }
        };
        for (_, v) in c.all_fields_iter() {
            if check(v, self, ctx) {
                return true;
            }
        }
        for v in c.local.values() {
            if check(v.clone(), self, ctx) {
                return true;
            }
        }
        false
    }

    fn navigate_segments(&self, start: Value, segments: &[String], ctx: &mut EvalContext, path_prefix: &str) -> Value {
        let mut val = start;
        let mut accumulated_effect = val.effect();
        let mut path_so_far = path_prefix.to_string();
        for seg in segments {
            if let Err(e) = ctx.check_resources(2) { 
                return handle_resource_exhausted(e, ctx.strategy, &ctx.horizon_salt, ctx.fuel, None, accumulated_effect);
            }
            let seg = seg.trim();
            let mut current = self.force(val, ctx);
            accumulated_effect = accumulated_effect.union(current.effect());
            // forward_spread: expand deferred sources before field ops so
            // ⊥/blur absorb and field merge are visible to this segment.
            if let Value::Combo(c) = current {
                if !c.pending_spreads.is_empty() {
                    current = self.expand_combo_pending(c, ctx);
                    accumulated_effect = accumulated_effect.union(current.effect());
                } else {
                    current = Value::Combo(c);
                }
            }
            while let Value::Combo(ref c) = current {
                if c.is_pure_wrapper() {
                    // Meta / present-key access hits the pure-wrapper shell
                    // (C2 atom spread `{%val: v}` must answer `.%val`). Peel
                    // only when the next segment is not on the shell and is
                    // non-meta — value-context collapse for data descent.
                    let on_shell = c.get_field(seg).is_some()
                        || seg.starts_with('%')
                        || seg == "%id"
                        || seg == "%rank";
                    if on_shell {
                        break;
                    }
                    if let Some(inner) = c.get_field("%val") {
                        current = self.force(inner.clone(), ctx);
                        accumulated_effect = accumulated_effect.union(current.effect());
                    } else { break; }
                } else if crate::value::is_structural_view(c) {
                    // G6 acceptance repair: the structural-view mark is a
                    // display filter, transparent to navigation — <<…>>
                    // bindings navigate exactly like the underlying node
                    // (SYNTAX_07 §4 #7: post-`>>` field access collapses).
                    if let Some(inner) = crate::value::structural_node(c) {
                        current = self.force(inner.clone(), ctx);
                        accumulated_effect = accumulated_effect.union(current.effect());
                    } else { break; }
                } else { break; }
            }
            if seg == "%id" {
                // %id is an OBSERVATION of content identity — hash the
                // solidified content, not the lazy plumbing. Sealed frames in
                // Thunk closures are evaluation machinery: hashing them split
                // %id across nesting depths for identical spellings
                // (`a1 = {k:5,d:k+1}` vs `b1.q2` same literal → different
                // %id), a content lie. force_recursive is the same
                // solidification the observe exit uses; fuel-guarded.
                let solid = self.force_recursive(current.clone(), ctx);
                return Value::Atom(AtomKind::Str(solid.content_hash_with_salt(&ctx.horizon_salt).to_string()), EffectTag::Pure, None).with_effect(accumulated_effect);
            }
            if seg == "%rank" { if let Value::Atom(_, _, Some(r)) = current { return Value::Atom(AtomKind::Int(BigInt::from(r)), EffectTag::Pure, None).with_effect(accumulated_effect); } }
            // SPEC_08 §4.1 元欄觀測 (effect_meta): `.%effect` → effect tag
            // atom. Pure carrier (no re-taint via accumulated_effect — the
            // answer *is* the tag; `#io  ;; %effect: #io` would lie).
            // ⊥ / #blur handled below on their whitelist arms (F1 / absorb);
            // Combo path: field lookup first (spoof), then this lens before
            // closed-miss. Non-combo values answer here after force.
            if seg == "%effect"
                && !matches!(
                    &current,
                    Value::Bottom(_) | Value::Blur(_) | Value::Combo(_) | Value::Union(_)
                )
            {
                return effect_tag_atom(current.effect());
            }

            if let Value::Bottom(ref d) = current {
                if seg == "%cause" {
                    return d.as_cause_combo().with_effect(accumulated_effect);
                }
                // %type alias retired (cocoon_shape 2026-07-19): non-meta on
                // ⊥ passes the bottom through (F1 compositionality).
                // F1: ⊥ navigation is compositional (x.a.b ≡ (x.a).b) — same
                // shape as the Blur continue repair. Non-meta segments pass
                // the bottom through so a later meta segment still answers.
                val = current;
                continue;
            }
            // SPEC_12 §1.1 / SPEC_01 §2.4.2: caused Top provenance —
            // meta-only readability (#static_cycle / #no_coordinate).
            if let Value::TopCaused {
                ref cause,
                ref members,
            } = current
            {
                if seg == "%cause" {
                    return crate::value::caused_top_cause_combo(cause, members)
                        .with_effect(accumulated_effect);
                }
                // Non-meta on caused Top: consumption evaporates cause → bare Top.
                val = Value::Top;
                continue;
            }
            // G3 R4 + Blur boundary #4/#5: #blur meta whitelist and
            // coordinate-context absorption (SPEC_08 §3.2.2).
            // Whitelist = %cause / %caid only (%type fossil retired).
            if let Value::Blur(bd) = current {
                if seg == "%cause" {
                    return Value::Atom(
                        AtomKind::Tag(bd.cause.as_str().to_string()),
                        EffectTag::Pure,
                        None,
                    )
                    .with_effect(accumulated_effect);
                }
                if seg == "%caid" {
                    // Snapshot identity string (totally decidable via ==).
                    return Value::Atom(
                        AtomKind::Str(bd.blur_caid().to_string()),
                        EffectTag::Pure,
                        None,
                    )
                    .with_effect(accumulated_effect);
                }
                // #5 non-meta nav on #blur: pass the horizon out unchanged
                // (never mint #invalid_path — nothing is known behind a
                // horizon). Navigation stays compositional (x.a.b ≡ (x.a).b):
                // remaining segments continue on the blur so a later meta
                // segment (%cause/%caid) still answers honestly.
                let mut bd = bd;
                bd.effect = bd.effect.union(accumulated_effect);
                val = Value::Blur(bd);
                continue;
            }

            match current {
                Value::Combo(c) => {
                    // SPEC_04 §3.1 #3/#5: dotted descent into a private (`~`)
                    // segment is external locating → ⊥ #private_access_violation.
                    // System axis `~%…` is exempt (not the local axis).
                    if seg.starts_with('~') && !seg.starts_with("~%") {
                        return BottomCause::PrivateAccessViolation.into();
                    }
                    let found = c
                        .get_field(seg)
                        .or_else(|| c.get_field(&format!("/{}", seg)))
                        .or_else(|| c.get_field(&format!("@{}", seg)))
                        .cloned();
                    if !path_so_far.is_empty() {
                        path_so_far = format!("{}.{}", path_so_far, seg);
                    } else {
                        path_so_far = seg.to_string();
                    }
                    // SPEC_08 §4.1: explicit `%effect` field (SYNTAX_08) wins;
                    // else engine tag lens — *before* closed-miss so cocoon
                    // `.%effect` is #pure shield, not #missing_key.
                    // type_super R1: `.%super` is DERIVED hierarchy reflection
                    // (SPEC_09 §2.1 tree) — intercept before closed-miss /
                    // open-miss so type markers answer the parent marker.
                    let target = match found {
                        Some(v) => v,
                        None if seg == "%effect" => {
                            return effect_tag_atom(c.effect);
                        }
                        None if seg == "%super" => {
                            if is_type_constraint_combo(&c) {
                                match get_type_constraint_name(&c)
                                    .as_deref()
                                    .and_then(TypeConstraint::super_parent)
                                {
                                    // @any (⊤): no super — honest open-miss.
                                    None => Value::Top,
                                    Some(parent) => TypeConstraint::marker_value(parent)
                                        .with_effect(accumulated_effect),
                                }
                            } else if is_user_field_type_combo(&c)
                                || self.combo_embeds_type_marker_shallow(&c, ctx)
                            {
                                // User field-structure type → @combo.
                                // Fields may still be Thunks of `@T` at
                                // observation — shallow-force to see markers.
                                TypeConstraint::marker_value("combo")
                                    .with_effect(accumulated_effect)
                            } else {
                                // Non-type value: ordinary open-miss.
                                crate::value::no_coordinate_top()
                            }
                        }
                        None if c.closed && !seg.starts_with('%') => {
                            // SPEC_03 §1.2 #1 / §1.3: Cocoon eigenstate —
                            // undefined coordinate → ⊥ #missing_key.
                            // %-meta is another axis (F-series open; pins).
                            return Value::Bottom(Box::new(crate::value::BottomDetail {
                                cause: BottomCause::MissingKey,
                                path: Some(path_so_far.clone()),
                                message: Some(format!(
                                    "Key '{}' missing in closed Cocoon",
                                    seg
                                )),
                                expected: None,
                                found: None,
                                involved: vec![],
                                ..Default::default()
                            }))
                            .with_effect(accumulated_effect);
                        }
                        // Open-combo missing key: caused Top #no_coordinate
                        // (ruling C — diagnostic member, not bare lattice Top).
                        None => crate::value::no_coordinate_top(),
                    };
                    // Leave the field unforced (Stage 2: open terms stay Thunk
                    // until observation). Cycle detection for the full path is
                    // handled in `force` when the thunk's expr is that path.
                    val = target;
                }
                // G4: path nav over Union = per-branch projection (SPEC_07
                // 平等演化). Drop ⊥ survivors; keep Top (open-miss); then
                // normalize_union. Single-segment recursion — remaining
                // segments continue on the aggregated result (no double
                // projection of the full tail).
                // T1 (union_cull): projected field may be a Stage-2 Thunk
                // that forces to ⊥ — shallow-force before the Bottom match
                // (do NOT force_recursive the whole branch; nested combo
                // fields stay lazy). Collect full BottomDetail; all-⊥ →
                // primary member ⊥ verbatim (REAL_04 §4 supplement).
                Value::Union(branches) => {
                    let mut survivors: Vec<Value> = Vec::new();
                    let mut culled: Vec<BottomDetail> = Vec::new();
                    let mut branch_effect = accumulated_effect;
                    for b in branches {
                        let mut projected =
                            self.navigate_segments(b, &[seg.to_string()], ctx, "");
                        // Shallow force peel — expose thunk-⊥ to cull.
                        let mut peel = 0u32;
                        while matches!(&projected, Value::Thunk { .. }) && peel < 32 {
                            projected = self.force(projected, ctx);
                            peel += 1;
                        }
                        match projected {
                            Value::Bottom(d) => {
                                // Drop — compatible-survivor rule; keep
                                // full detail for all-⊥ primary pick.
                                branch_effect = branch_effect.union(Value::Bottom(d.clone()).effect());
                                culled.push(*d);
                            }
                            other => {
                                branch_effect = branch_effect.union(other.effect());
                                survivors.push(other);
                            }
                        }
                    }
                    if survivors.is_empty() {
                        // F4c + T3: primary-rank member ⊥ out verbatim.
                        return primary_bottom_from_culled(culled)
                            .with_effect(branch_effect);
                    }
                    // SPEC_08 §4.1: union distributes `.%effect` per branch;
                    // answers are pure tag atoms — do not re-taint with the
                    // path's accumulated IO/nondet (would print
                    // `#io ;; %effect: #io | #pure ;; %effect: #io`).
                    if seg == "%effect" {
                        return self.normalize_union_absorbing(survivors, ctx);
                    }
                    if !path_so_far.is_empty() {
                        path_so_far = format!("{}.{}", path_so_far, seg);
                    } else {
                        path_so_far = seg.to_string();
                    }
                    val = self.normalize_union_absorbing(survivors, ctx);
                    accumulated_effect = branch_effect;
                }
                // F3+F4a: atom/Top/other non-navigable → open miss with
                // cause #no_coordinate (ruling C). Never mint abolished
                // #invalid_path. Compositional further segments continue.
                _ => {
                    if !path_so_far.is_empty() {
                        path_so_far = format!("{}.{}", path_so_far, seg);
                    } else {
                        path_so_far = seg.to_string();
                    }
                    val = crate::value::no_coordinate_top();
                }
            }
        }
        val.with_effect(accumulated_effect)
    }

    fn inject_path(&self, target: &mut ComboVal, segments: &[String], val: Value) -> std::result::Result<(), BottomCause> {
        if segments.is_empty() { return Ok(()); }
        let key = segments[0].trim();
        if segments.len() == 1 { target.insert_field(key, val); } 
        else {
            let mut sub = match target.get_field(key) { Some(Value::Combo(c)) => c.clone(), _ => ComboVal::new(IndexMap::new(), false, IndexMap::new(), EffectTag::Pure, vec![]) };
            self.inject_path(&mut sub, &segments[1..], val)?;
            target.insert_field(key, Value::Combo(sub));
        }
        Ok(())
    }

    const MAX_REFINE_HOPS: usize = 16;

    pub fn follow_refine(&self, caid: &ContentHash) -> Result<ContentHash, BottomCause> {
        let mut current = caid.to_string();
        let mut visited = std::collections::HashSet::new();
        for _ in 0..Self::MAX_REFINE_HOPS {
            if !visited.insert(current.clone()) {
                return Err(BottomCause::Divergent);
            }
            let map = self.refine_map.read().map_err(|_| BottomCause::Conflict)?;
            match map.get(&current) {
                None => break,
                Some(targets) if targets.is_empty() => break,
                Some(targets) => { current = targets[0].clone(); }
            }
        }
        // F4 abolition: InvalidPath minting stopped. Malformed refine-map
        // targets are a store integrity conflict, not a path-syntax error.
        ContentHash::parse(&current).map_err(|_| BottomCause::Conflict)
    }

    pub fn get_live_value(&self, caid: &ContentHash) -> Result<Value> {
        // Split honest messages: Divergent = cycle; Conflict = parse/store
        // integrity; other causes pass their tag through.
        let resolved = self.follow_refine(caid).map_err(|cause| match cause {
            BottomCause::Divergent => anyhow::anyhow!("Refinement cycle detected"),
            BottomCause::Conflict => {
                anyhow::anyhow!("Invalid refinement target (store integrity conflict)")
            }
            other => anyhow::anyhow!("Refinement follow failed: {}", other.as_tag()),
        })?;
        self.store.get_value(&resolved)
    }

    pub fn sub_context(&self, ctx: &EvalContext) -> EvalContext {
        // F2 (§3-fix, open): ctx.clone() deep-copies root (ComboVal). Each
        // force(thunk) creates a sub_context; in a self-referential Ref chain
        // (observe v with v:<<_.>>), force_recursive iterates root's N fields,
        // each thunk-force cloning root again — O(depth × N × |root|) memory.
        // The probe test uses a minimal-root build_universe_with(dir, true)
        // and passes. With stdlib-heavy root (build_universe_with(dir, false)),
        // memory exhaustion outruns the fuel horizon. Fix: share root via
        // Arc<ComboVal> so sub_context clones are cheap; requires making
        // root access patterns Arc-compatible (tests mutate root directly).
        // L2-17: do NOT clear `in_flight` or `computing` — nested force/eval
        // must see the parent observation's cycle set (re-entry → #divergent).
        // `call_history` remains a per-subsession reset.
        let mut sub = ctx.clone(); sub.call_history.clear(); sub
    }

    /// Force a binding under a coordinate key. Re-entering the same coordinate
    /// in this observation = cycle → ⊥ #divergent (L2-17). Covers path cycles
    /// (`s.v` → `s.v`) where fresh thunk instances may not share a content-hash.
    fn force_coord(&self, coord: &str, val: Value, ctx: &mut EvalContext) -> Value {
        // Private / system locals: not lattice cycle coordinates.
        if coord.starts_with('~') {
            return self.force(val, ctx);
        }
        // Gate re-entry for Thunk / Ref, and for Combo still holding deferred
        // spreads (forward_spread): expanding `...al` that aliases back to
        // this coord must hit #divergent, not re-enter expand forever.
        let needs_gate = matches!(val, Value::Thunk { .. } | Value::Ref(_))
            || matches!(&val, Value::Combo(c) if !c.pending_spreads.is_empty());
        if needs_gate {
            if ctx.computing.contains(coord) {
                return cycle_reentry(ctx, Some(coord));
            }
            ctx.computing.insert(coord.to_string());
            ctx.cycle_chain.push(coord.to_string());
            let res = self.force(val, ctx);
            // Expand deferred spreads WHILE still under this coord so
            // alias-detour re-entry (`...al` → `a`) sees computing and
            // chain_transform_taint (forward_spread C4).
            let res = match res {
                Value::Combo(c) if !c.pending_spreads.is_empty() => {
                    self.expand_combo_pending(c, ctx)
                }
                other => other,
            };
            if ctx.cycle_chain.last().map(|s| s == coord).unwrap_or(false) {
                ctx.cycle_chain.pop();
            }
            ctx.computing.remove(coord);
            res
        } else {
            self.force(val, ctx)
        }
    }

    /// SPEC_04 §2.1: force a value found on a scope-frame field. Marks the
    /// bare name in `lexical_forcing` so force keeps ambient frames
    /// (unbounded sibling-chain depth). SPEC_12 §1.1: re-entry is
    /// cycle_reentry (static → caused Top / transform → #divergent).
    fn force_lexical_name(&self, name: &str, val: Value, ctx: &mut EvalContext, use_coord: bool) -> Value {
        if matches!(&val, Value::Ref(_)) {
            return val;
        }
        if ctx.lexical_forcing.contains(name) {
            return cycle_reentry(ctx, Some(name));
        }
        let track = matches!(&val, Value::Thunk { .. });
        if track {
            ctx.lexical_forcing.insert(name.to_string());
            ctx.cycle_chain.push(name.to_string());
        }
        let res = if use_coord {
            self.force_coord(name, val, ctx)
        } else {
            self.force(val, ctx)
        };
        if track {
            if ctx.cycle_chain.last().map(|s| s == name).unwrap_or(false) {
                ctx.cycle_chain.pop();
            }
            ctx.lexical_forcing.remove(name);
        }
        res
    }

    /// Fetch a value from a remote peer and verify its address (REAL_03 §6.6).
    ///
    /// - `Ok(val)` — decoded and address matches the requested CAID
    /// - `Err(CaidMismatch)` — peer returned bytes that do not authenticate
    ///   (mismatch or undecodable); incident recorded
    /// - `Err(Conflict)` — connection failure or empty response (absence)
    pub fn remote_fetch(&self, addr: &str, hash: &ContentHash) -> Result<Value, BottomCause> {
        use std::io::{Read, Write};
        use std::net::TcpStream;
        use std::time::Duration;

        let mut stream = TcpStream::connect_timeout(
            &addr.parse().map_err(|_| BottomCause::Conflict)?,
            Duration::from_secs(5),
        )
        .map_err(|_| BottomCause::Conflict)?;
        let _ = stream.write_all(hash.to_string().as_bytes());
        let _ = stream.write_all(b"\n");
        let _ = stream.flush();

        let mut buffer = Vec::new();
        let _ = stream.read_to_end(&mut buffer);
        if buffer.is_empty() {
            // Peer sent nothing — absence, not a lie.
            return Err(BottomCause::Conflict);
        }
        let source = format!("tcp://{addr}");
        let val: Value = match serde_json::from_slice(&buffer) {
            Ok(v) => v,
            Err(_) => {
                self.record_integrity(hash, &source, IntegrityKind::Undecodable);
                return Err(BottomCause::CaidMismatch);
            }
        };
        let recomputed = val.content_hash();
        if !crate::storage::value_address_matches(hash, &recomputed) {
            self.record_integrity(hash, &source, IntegrityKind::Mismatch);
            return Err(BottomCause::CaidMismatch);
        }
        Ok(val)
    }

    /// History newest-first: (hash, meta, kind). Kind is required so privileged
    /// commits (`CommitKind::Pin`) are auditable from `oo log` without living
    /// inside values (SPEC_08 §6.2).
    pub fn log(&self) -> Result<Vec<(ContentHash, CommitMeta, CommitKind)>> {
        let current_dir = std::env::current_dir()?;
        if let Some(head) = self.store.get_head(&current_dir)? {
            let mut history = Vec::new();
            let mut curr = Some(head);
            while let Some(h) = curr {
                let commit = self.store.get_commit(&h)?;
                history.push((h, commit.meta.clone(), commit.kind));
                curr = commit.parent;
            }
            return Ok(history);
        }
        Ok(Vec::new())
    }
    pub fn tropical_weight(&self, val: &Value) -> u64 {
        val.tropical_weight()
    }

}
