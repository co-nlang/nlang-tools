pub mod universe; pub use universe::Universe;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use indexmap::IndexMap;
use nlang_parser::ast::{Path, PathAnchor, AtomKind, Expr};
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
pub use crate::value::{Value, ComboVal, EffectTag, ContentHash, CaidVersion, MasaRef, BottomDetail, BottomCause, CommitMeta, Commit, CommitKind, RefineInfo, Holonomy, Identity, AuthorityInfo, BlurDetail, BlurCause, HorizonParams, ObservationStrategy};
pub use crate::storage::ObjectStore;
pub use crate::dispatch::{MorphismDispatchResult, MorphismDispatchResult as DispatchResult};
pub use crate::observation::{ObservationState, handle_resource_exhausted};
use crate::builtins::create_default_builtins;
use crate::type_constraint::TypeConstraint;
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
    pub in_math_op: bool,
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
}

impl EvalContext {
    pub fn new(root: ComboVal) -> Self {
        let mut hasher = sha2::Sha256::new();
        hasher.update(b"default");
        let salt = ContentHash::v1(hasher.finalize().to_vec());
        Self { 
            root: Arc::new(root), root_caid_cache: None, scopes: Vec::new(), staged: None, computing: HashSet::new(), 
            call_history: HashMap::new(), in_math_op: false, context_value: None, 
            fuel: 10000, timeout_deadline: None, depth: 0, dep_collector: None, memo_enabled: true, 
            horizon_salt: salt, strategy: ObservationStrategy::Blur,
            max_branches: 64, max_unification_depth: 256, max_pattern_nodes: 1024, max_lifting_depth: 32,
            refine_map_active: false,
            had_nondistrib_event: false,
            disc_routing_visited: std::collections::HashSet::new(),
            disc_routing_hops: 0,
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
        if self.depth > self.max_unification_depth as u32 { return Err(ResourceExhausted::StackOverflow); }
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
        let local_pk_hex = hex::encode(&identity.public_key);
        let mut architects = std::collections::HashSet::new();
        architects.insert(local_pk_hex);
        Self { store, base_dir: None, unify_memo: RwLock::new(HashMap::new()), force_memo: RwLock::new(HashMap::new()), force_memo_rev: RwLock::new(HashMap::new()), builtin_registry: builtins, peers: RwLock::new(HashMap::new()), identity, refine_map: RwLock::new(HashMap::new()), gbb_registry: RwLock::new(HashMap::new()), architect_registry: RwLock::new(architects) }
    }

    pub fn init(base_dir: &std::path::Path) -> Result<Self> {
        let store = ObjectStore::init(base_dir)?;
        let builtins = create_default_builtins();
        let identity = crate::value::Identity::new_random();
        let local_pk_hex = hex::encode(&identity.public_key);
        let mut architects = std::collections::HashSet::new();
        architects.insert(local_pk_hex);
        if let Ok(persisted) = store.load_architects(base_dir) {
            architects.extend(persisted);
        }
        let mut oo = Self { store, base_dir: Some(base_dir.to_path_buf()), unify_memo: RwLock::new(HashMap::new()), force_memo: RwLock::new(HashMap::new()), force_memo_rev: RwLock::new(HashMap::new()), builtin_registry: builtins, peers: RwLock::new(HashMap::new()), identity, refine_map: RwLock::new(HashMap::new()), gbb_registry: RwLock::new(HashMap::new()), architect_registry: RwLock::new(architects) };
        Ok(oo)
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
        state_inner.insert("strategy".to_string(), Value::Atom(AtomKind::Tag("blur".to_string()), EffectTag::Pure, None));
        engine_fields.insert("state".to_string(), Value::Combo(ComboVal::new(state_inner, false, IndexMap::new(), EffectTag::Pure, vec![])));
        fields.insert("~%Engine".to_string(), Value::Combo(ComboVal::new(engine_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));

        // ~%Official：建築師白名單
        let local_pk_hex = hex::encode(&self.identity.public_key);
        let mut official_fields = IndexMap::new();
        official_fields.insert("architects".to_string(), Value::Atom(AtomKind::Str(local_pk_hex), EffectTag::Pure, None));
        fn official_morph(builtin: &str, effect: EffectTag) -> Value {
            Value::Combo(ComboVal::new(IndexMap::from_iter(vec![
                ("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)),
                ("%builtin".to_string(), Value::Atom(AtomKind::Str(builtin.to_string()), EffectTag::Pure, None)),
            ]), true, IndexMap::new(), effect, vec![]))
        }
        official_fields.insert("/sign_refine".to_string(), official_morph("engine.sign_refine", EffectTag::IO));
        official_fields.insert("/add_architect".to_string(), official_morph("engine.add_architect", EffectTag::IO));
        fields.insert("~%Official".to_string(), Value::Combo(ComboVal::new(official_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));

        // ~%Config: genesis defaults (SPEC_09 §6)
        let mut config_fields = IndexMap::new();
        config_fields.insert("%fuel".to_string(), Value::Atom(AtomKind::Int(10000i64.into()), EffectTag::Pure, None));
        config_fields.insert("%max_branches".to_string(), Value::Atom(AtomKind::Int(64i64.into()), EffectTag::Pure, None));
        config_fields.insert("%max_depth".to_string(), Value::Atom(AtomKind::Int(256i64.into()), EffectTag::Pure, None));
        config_fields.insert("%max_pattern_nodes".to_string(), Value::Atom(AtomKind::Int(1024i64.into()), EffectTag::Pure, None));
        config_fields.insert("%timeout".to_string(), Value::Atom(AtomKind::Int(1000i64.into()), EffectTag::Pure, None));
        config_fields.insert("%strategy".to_string(), Value::Atom(AtomKind::Tag("blur".to_string()), EffectTag::Pure, None));
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
        if let Some(Value::Combo(ref cfg)) = sys_root.get_field("~%Config").cloned() {
            if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("%fuel").cloned() {
                if let Some(f) = n.to_u64() { ctx.fuel = f; }
            }
            if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("%max_branches").cloned() {
                if let Some(v) = n.to_u64() { ctx.max_branches = v as usize; }
            }
            if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("%max_depth").cloned() {
                if let Some(v) = n.to_u64() { ctx.max_unification_depth = v as usize; }
            }
            if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("%max_pattern_nodes").cloned() {
                if let Some(v) = n.to_u64() { ctx.max_pattern_nodes = v as usize; }
            }
            if let Some(Value::Atom(AtomKind::Tag(s), _, _)) = cfg.get_field("%strategy").cloned() {
                ctx.strategy = match s.trim_start_matches('#') {
                    "strict" => ObservationStrategy::Strict,
                    "approximate" => ObservationStrategy::Approximate,
                    _ => ObservationStrategy::Blur,
                };
            }
        if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("%timeout").cloned() {
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
        let f = self.force(f, ctx); if let Value::Bottom(_) = f { return f; } if let Value::Top = f { return Value::Top; }
        let arg = self.force(arg, ctx); if let Value::Bottom(_) = arg { return arg; }
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
            return match out.len() {
                0 => BottomCause::Conflict.into(),
                1 => out.into_iter().next().unwrap(),
                _ => Value::Union(out),
            };
        }
        if !f.is_morphism() { if arg.is_morphism() { return self.apply_morphism(arg, f, ctx); } }
        match f {
            Value::Combo(ref c) => {
                if let Some(inner) = c.get_field("%val") { return self.apply_morphism(inner.clone(), arg, ctx); }
                
                let is_arg_pack = match &arg { 
                    Value::Combo(ac) => ac.contains_key("%arg") || (ac.contains_key("0") && !ac.contains_key("%kind")), 
                    _ => false 
                };
                let mut nf = IndexMap::new(); 
                for (k, v) in c.fields() { if k.parse::<usize>().is_ok() { nf.insert(k.clone(), v.clone()); } }
                if is_arg_pack { 
                    if let Value::Combo(ref ac) = arg { 
                        for (k, v) in &ac.fields() { if k.parse::<usize>().is_ok() { nf.insert(k.clone(), v.clone()); } } 
                    } 
                } else { 
                    let mut max_idx = -1i32; 
                    for k in nf.keys() { if let Ok(idx) = k.parse::<i32>() { if idx > max_idx { max_idx = idx; } } } 
                    nf.insert((max_idx + 1).to_string(), arg.clone()); 
                }
                let unified_arg = Value::Combo(ComboVal::new(nf, true, IndexMap::new(), arg.effect(), vec![]));
                
                if let Some(Value::Combo(rules_source)) = c.get_field("%rules") {
                    let dispatch_result = self.dispatch_morphism(rules_source, &arg, ctx);
                    return dispatch_result.to_value(f.effect());
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
                let effective_context: Option<Value> = context.map(|b| *b).or(ctx.context_value.clone());
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
                                Value::Atom(kind, old_e, r) if old_e < effect => Value::Atom(kind, effect, r),
                                Value::Combo(mut cv) if cv.effect < effect => { cv.effect = effect; Value::Combo(cv) },
                                _ if res.effect() < effect => Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%val".to_string(), res)]), true, IndexMap::new(), effect, vec![])),
                                _ => res,
                            };
                            return res;
                        }
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
                call_ctx.scopes = closure;
                call_ctx.context_value = effective_context;
                call_ctx.dep_collector = inner_collector;
                let res = self.eval(&expr, &mut call_ctx);
                ctx.fuel = call_ctx.fuel;
                let inner_deps = call_ctx.dep_collector.take();

                let res = match res {
                    Value::Atom(kind, old_e, r) if old_e < effect => Value::Atom(kind, effect, r),
                    Value::Combo(mut cv) if cv.effect < effect => { cv.effect = effect; Value::Combo(cv) },
                    _ if res.effect() < effect => Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%val".to_string(), res)]), true, IndexMap::new(), effect, vec![])),
                    _ => res,
                };

                let deps = inner_deps.unwrap_or_default();

                // Merge inner deps into outer collector.
                if let Some(ref mut outer) = ctx.dep_collector {
                    for d in &deps { outer.insert(d.clone()); }
                }

                // Stage 5 (§5b): insert into force_memo with deps + reverse index.
                if let Some(ref k) = memo_key {
                    if !matches!(res, Value::Bottom(_)) && !res.contains_blur() && res.effect() < EffectTag::NonDet {
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
                let mut new_c = ComboVal::default();
                new_c.closed = c.closed;
                new_c.effect = c.effect;
                for (k, v) in c.all_fields_iter() { new_c.insert_field(&k, self.force_recursive(v, ctx)); }
                // also force local fields (all_fields_iter skips them)
                for (k, v) in c.local.iter() { new_c.local.insert(k.clone(), self.force_recursive(v.clone(), ctx)); }
                Value::Combo(new_c)
            }
            Value::Union(branches) => Value::Union(branches.into_iter().map(|b| self.force_recursive(b, ctx)).collect()),
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
            
            if TypeConstraint::is_type_constraint_path(name) {
                let type_name = name.trim_start_matches('@');
                return Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        ("%kind".to_string(), Value::Atom(AtomKind::Tag("type_constraint".to_string()), EffectTag::Pure, None)),
                        ("%type".to_string(), Value::Atom(AtomKind::Str(type_name.to_string()), EffectTag::Pure, None)),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![]
                ));
            }
            
            for scope in ctx.scopes.iter().rev() {
                if let Some(val) = scope.get_field(name) { return self.force(val.clone(), ctx); }
                if let Some(val) = scope.local_fields().get(name) { return self.force(val.clone(), ctx); }
                let prefixes = vec!["/", "@", "~", "~%"];
                for p in prefixes {
                    let alt_name = if name.starts_with(p) { name.trim_start_matches(p).to_string() } else { format!("{}{}", p, name) };
                    if let Some(val) = scope.get_field(&alt_name) { return self.force(val.clone(), ctx); }
                    if let Some(val) = scope.get_local_field(&alt_name) { return self.force(val.clone(), ctx); }
                }
            }
            if let Some(ref s) = ctx.staged { if let Some(val) = s.get_field(name).or_else(|| s.get_local_field(name)) { return self.force(val.clone(), ctx); } let prefixes = vec!["/", "@", "~", "~%"]; for p in prefixes { let alt_name = if name.starts_with(p) { name.trim_start_matches(p).to_string() } else { format!("{}{}", p, name) }; if let Some(val) = s.get_field(&alt_name).or_else(|| s.get_local_field(&alt_name)) { return self.force(val.clone(), ctx); } } }
            if let Some(val) = ctx.root.get_field(name).or_else(|| ctx.root.get_local_field(name)) { let v = val.clone(); self.record_dep(ctx, name); return self.force(v, ctx); }
            let prefixes = vec!["/", "@", "~", "~%"];
            for p in prefixes { let alt_name = if name.starts_with(p) { name.trim_start_matches(p).to_string() } else { format!("{}{}", p, name) }; if let Some(val) = ctx.root.get_field(&alt_name).or_else(|| ctx.root.get_local_field(&alt_name)) { return self.force(val.clone(), ctx); } }
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
                if found.is_none() { if let Some(ref s) = ctx.staged { if let Some(val) = s.get_field(name).or_else(|| s.get_local_field(name)) { found = Some(val.clone()); self.record_dep(ctx, name); } else { let prefixes = vec!["/", "@", "~", "~%"]; for p in prefixes { let alt_name = if name.starts_with(p) { name.trim_start_matches(p).to_string() } else { format!("{}{}", p, name) }; if let Some(val) = s.get_field(&alt_name).or_else(|| s.get_local_field(&alt_name)) { found = Some(val.clone()); self.record_dep(ctx, &alt_name); break; } } } } }
                if found.is_none() { if let Some(val) = ctx.root.get_field(name).or_else(|| ctx.root.get_local_field(name)) { found = Some(val.clone()); self.record_dep(ctx, name); } else { let prefixes = vec!["/", "@", "~", "~%"]; for p in prefixes { let alt_name = if name.starts_with(p) { name.trim_start_matches(p).to_string() } else { format!("{}{}", p, name) }; if let Some(val) = ctx.root.get_field(&alt_name).or_else(|| ctx.root.get_local_field(&alt_name)) { found = Some(val.clone()); self.record_dep(ctx, &alt_name); break; } } } }
                match found { Some(v) => {
                    let is_ref = matches!(&v, Value::Ref(_));
                    let forced = self.force(v, ctx);
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
                        let res = self.navigate_segments(forced, &path.segments[1..], ctx);
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
            PathAnchor::Parent(count) => { let len = ctx.scopes.len(); if len > count as usize { Value::Combo(ctx.scopes[len - 1 - (count as usize)].clone()) } else { return BottomCause::InvalidPath.into(); } }
            PathAnchor::Current => { if let Some(top) = ctx.scopes.last() { Value::Combo(top.clone()) } else { Value::Combo((*ctx.root).clone()) } }
        };
        if !path.segments.is_empty() && !matches!(path.anchor, PathAnchor::Bare) { self.navigate_segments(start_val, &path.segments, ctx) } else { start_val }
    }

    fn navigate_segments(&self, start: Value, segments: &[String], ctx: &mut EvalContext) -> Value {
        let mut val = start;
        let mut accumulated_effect = val.effect();
        for seg in segments {
            if let Err(e) = ctx.check_resources(2) { 
                return handle_resource_exhausted(e, ctx.strategy, &ctx.horizon_salt, ctx.fuel, None, accumulated_effect);
            }
            let seg = seg.trim();
            let mut current = self.force(val, ctx);
            accumulated_effect = accumulated_effect.max(current.effect());
            while let Value::Combo(ref c) = current {
                if c.is_pure_wrapper() {
                    if let Some(inner) = c.get_field("%val") {
                        current = self.force(inner.clone(), ctx);
                        accumulated_effect = accumulated_effect.max(current.effect());
                    } else { break; }
                } else { break; }
            }
            if seg == "%id" { return Value::Atom(AtomKind::Str(current.content_hash_with_salt(&ctx.horizon_salt).to_string()), EffectTag::Pure, None).with_effect(accumulated_effect); }
            if seg == "%rank" { if let Value::Atom(_, _, Some(r)) = current { return Value::Atom(AtomKind::Int(BigInt::from(r)), EffectTag::Pure, None).with_effect(accumulated_effect); } }
            
            if let Value::Bottom(ref d) = current {
                if seg == "%cause" { return d.as_cause_combo().with_effect(accumulated_effect); }
                if seg == "%type" {
                    let type_tag = match d.cause {
                        BottomCause::Conflict => "conflict",
                        BottomCause::MissingKey => "missing_key",
                        BottomCause::FuelExhausted => "fuel_exhausted",
                        BottomCause::Timeout => "timeout",
                        BottomCause::Divergent => "divergent",
                        BottomCause::InvalidPath => "invalid_path",
                        BottomCause::PrivateAccessViolation => "private_access_violation",
                        BottomCause::NumericalError => "numerical_error",
                        BottomCause::ArithmeticOnAnchor => "arithmetic_on_anchor",
                        BottomCause::H1Split => "h1_split",
                        BottomCause::H2Split => "h2_split",
                        BottomCause::SemanticEclipse => "semantic_eclipse",
                        BottomCause::NoContext => "no_context",
                    };
                    return Value::Atom(AtomKind::Tag(type_tag.to_string()), EffectTag::Pure, None).with_effect(accumulated_effect);
                }
                return current;
            }

            match current { 
                Value::Combo(ref c) => { 
                    let target = c.get_field(seg).or_else(|| c.get_field(&format!("/{}", seg))).or_else(|| c.get_field(&format!("@{}", seg))).cloned().unwrap_or(Value::Top); 
                    val = target; 
                } 
                _ => { return BottomCause::InvalidPath.into() } 
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
        ContentHash::parse(&current).map_err(|_| BottomCause::InvalidPath)
    }

    pub fn get_live_value(&self, caid: &ContentHash) -> Result<Value> {
        let resolved = self.follow_refine(caid).map_err(|_| anyhow::anyhow!("Refinement cycle detected"))?;
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
        let mut sub = ctx.clone(); sub.computing.clear(); sub.call_history.clear(); sub
    }

    pub fn remote_fetch(&self, addr: &str, hash: &ContentHash) -> Result<Value, BottomCause> {
        use std::io::{Read, Write};
        use std::net::TcpStream;
        use std::time::Duration;

        let mut stream = TcpStream::connect_timeout(&addr.parse().map_err(|_| BottomCause::Conflict)?, Duration::from_secs(5)).map_err(|_| BottomCause::Conflict)?;
        let _ = stream.write_all(hash.to_string().as_bytes());
        let _ = stream.write_all(b"\n");
        let _ = stream.flush();

        let mut buffer = Vec::new();
        let _ = stream.read_to_end(&mut buffer);
        let val: Value = serde_json::from_slice(&buffer).map_err(|_| BottomCause::Conflict)?;
        Ok(val)
    }

    pub fn log(&self) -> Result<Vec<(ContentHash, CommitMeta)>> {
        let current_dir = std::env::current_dir()?;
        if let Some(head) = self.store.get_head(&current_dir)? { let mut history = Vec::new(); let mut curr = Some(head); while let Some(h) = curr { let commit = self.store.get_commit(&h)?; history.push((h, commit.meta.clone())); curr = commit.parent; } return Ok(history); }
        Ok(Vec::new())
    }
    pub fn tropical_weight(&self, val: &Value) -> u64 {
        val.tropical_weight()
    }

}
