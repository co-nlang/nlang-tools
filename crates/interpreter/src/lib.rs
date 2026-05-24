pub mod universe; pub use universe::Universe;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use indexmap::IndexMap;
use nlang_parser::ast::{Path, PathAnchor, AtomKind};
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
    pub root: ComboVal,
    pub scopes: Vec<ComboVal>,
    pub staged: Option<ComboVal>,
    pub computing: HashSet<String>,
    pub call_history: HashMap<String, Vec<ContentHash>>,
    pub in_math_op: bool,
    pub context_value: Option<Value>,
    pub fuel: u64,
    pub timeout_deadline: Option<u64>,
    pub depth: u32,
    pub horizon_salt: ContentHash,
    pub strategy: ObservationStrategy,
    pub max_branches: usize,
    pub max_unification_depth: usize,
    pub max_pattern_nodes: usize,
    pub max_lifting_depth: usize,
    pub refine_map_active: bool,
    pub had_nondistrib_event: bool,
}

impl EvalContext {
    pub fn new(root: ComboVal) -> Self {
        let mut hasher = sha2::Sha256::new();
        hasher.update(b"default");
        let salt = ContentHash::v1(hasher.finalize().to_vec());
        Self { 
            root, scopes: Vec::new(), staged: None, computing: HashSet::new(), 
            call_history: HashMap::new(), in_math_op: false, context_value: None, 
            fuel: 10000, timeout_deadline: None, depth: 0, 
            horizon_salt: salt, strategy: ObservationStrategy::Blur,
            max_branches: 64, max_unification_depth: 256, max_pattern_nodes: 1024, max_lifting_depth: 32,
            refine_map_active: false,
            had_nondistrib_event: false,
        }
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

pub struct Ouroboros {
    pub store: ObjectStore,
    pub base_dir: Option<PathBuf>,
    pub unify_memo: RwLock<HashMap<(ContentHash, ContentHash), Value>>,
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
        Self { store, base_dir: None, unify_memo: RwLock::new(HashMap::new()), builtin_registry: builtins, peers: RwLock::new(HashMap::new()), identity, refine_map: RwLock::new(HashMap::new()), gbb_registry: RwLock::new(HashMap::new()), architect_registry: RwLock::new(architects) }
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
        let mut oo = Self { store, base_dir: Some(base_dir.to_path_buf()), unify_memo: RwLock::new(HashMap::new()), builtin_registry: builtins, peers: RwLock::new(HashMap::new()), identity, refine_map: RwLock::new(HashMap::new()), gbb_registry: RwLock::new(HashMap::new()), architect_registry: RwLock::new(architects) };
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
        ];
        for (n, b) in string_morphisms { string_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::Pure, vec![]))); }
        fields.insert("~%String".to_string(), Value::Combo(ComboVal::new(string_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
        
        let mut time_fields = IndexMap::new();
        time_fields.insert("/now".to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str("time.now".to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::IO, vec![])));
        let time_morphisms = vec![
            ("/format", "time.format"),
            ("/diff",   "time.diff"),
            ("/add_ms", "time.add_ms"),
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

    pub fn force(&self, val: Value, ctx: &mut EvalContext) -> Value {
        match val {
            Value::Thunk { expr, closure, effect } => {
                let mut call_ctx = self.sub_context(ctx); call_ctx.scopes = closure;
                let res = self.eval(&expr, &mut call_ctx); ctx.fuel = call_ctx.fuel;
                match res {
                    Value::Atom(kind, old_e, r) if old_e < effect => Value::Atom(kind, effect, r),
                    Value::Combo(mut cv) if cv.effect < effect => { cv.effect = effect; Value::Combo(cv) },
                    _ if res.effect() < effect => Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%val".to_string(), res)]), true, IndexMap::new(), effect, vec![])),
                    _ => res,
                }
            }
            Value::Blur(_) => val,
            _ => val,
        }
    }

    pub fn force_recursive(&self, val: Value, ctx: &mut EvalContext) -> Value {
        let val = self.force(val, ctx);
        match val {
            Value::Combo(c) => { 
                let mut new_c = ComboVal::default();
                new_c.closed = c.closed;
                new_c.effect = c.effect;
                for (k, v) in c.all_fields_iter() { new_c.insert_field(&k, self.force_recursive(v, ctx)); }
                Value::Combo(new_c)
            }
            Value::Union(branches) => Value::Union(branches.into_iter().map(|b| self.force_recursive(b, ctx)).collect()),
            _ => val
        }
    }

    pub fn resolve_path(&self, path: &Path, ctx: &mut EvalContext) -> Value {
        let name_raw = if !path.segments.is_empty() { &path.segments[0] } else { "" };
        let name = name_raw.trim();
        
        if path.anchor == PathAnchor::Bare && path.segments.len() == 1 {
            if name == "#_|_" { return Value::Atom(AtomKind::TagStart, EffectTag::Pure, None); }
            if name == "#_" { return Value::Atom(AtomKind::TagEnd, EffectTag::Pure, None); }
            if name == "_" { return Value::Atom(AtomKind::Top, EffectTag::Pure, None); }
            if name == "_|_" { return Value::Atom(AtomKind::Bottom, EffectTag::Pure, None); }
            
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
            if let Some(val) = ctx.root.get_field(name).or_else(|| ctx.root.get_local_field(name)) { return self.force(val.clone(), ctx); }
            let prefixes = vec!["/", "@", "~", "~%"];
            for p in prefixes { let alt_name = if name.starts_with(p) { name.trim_start_matches(p).to_string() } else { format!("{}{}", p, name) }; if let Some(val) = ctx.root.get_field(&alt_name).or_else(|| ctx.root.get_local_field(&alt_name)) { return self.force(val.clone(), ctx); } }
        }
        self.resolve_path_internal(path, ctx)
    }

    fn resolve_path_internal(&self, path: &Path, ctx: &mut EvalContext) -> Value {
        let start_val: Value = match path.anchor {
            PathAnchor::Root => Value::Combo(ctx.root.clone()),
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
                if found.is_none() { if let Some(ref s) = ctx.staged { if let Some(val) = s.get_field(name).or_else(|| s.get_local_field(name)) { found = Some(val.clone()); } else { let prefixes = vec!["/", "@", "~", "~%"]; for p in prefixes { let alt_name = if name.starts_with(p) { name.trim_start_matches(p).to_string() } else { format!("{}{}", p, name) }; if let Some(val) = s.get_field(&alt_name).or_else(|| s.get_local_field(&alt_name)) { found = Some(val.clone()); break; } } } } }
                if found.is_none() { if let Some(val) = ctx.root.get_field(name).or_else(|| ctx.root.get_local_field(name)) { found = Some(val.clone()); } else { let prefixes = vec!["/", "@", "~", "~%"]; for p in prefixes { let alt_name = if name.starts_with(p) { name.trim_start_matches(p).to_string() } else { format!("{}{}", p, name) }; if let Some(val) = ctx.root.get_field(&alt_name).or_else(|| ctx.root.get_local_field(&alt_name)) { found = Some(val.clone()); break; } } } }
                match found { Some(v) => { let forced = self.force(v, ctx); if path.segments.len() > 1 { return self.navigate_segments(forced, &path.segments[1..], ctx); } forced } None => Value::Top }
            }
            PathAnchor::Parent(count) => { let len = ctx.scopes.len(); if len > count as usize { Value::Combo(ctx.scopes[len - 1 - (count as usize)].clone()) } else { return BottomCause::InvalidPath.into(); } }
            PathAnchor::Current => { if let Some(top) = ctx.scopes.last() { Value::Combo(top.clone()) } else { Value::Combo(ctx.root.clone()) } }
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
                if let Some(inner) = c.get_field("%val") {
                    current = self.force(inner.clone(), ctx);
                    accumulated_effect = accumulated_effect.max(current.effect());
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
