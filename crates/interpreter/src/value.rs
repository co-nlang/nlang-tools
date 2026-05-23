use nlang_parser::ast::{Expr, AtomKind};
use indexmap::IndexMap;
use sha2::{Sha256, Digest};
use std::fmt;
use serde::{Serialize, Deserialize};
use ring::{signature::{self, KeyPair as _}, rand};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EffectTag { Pure = 0, State = 1, IO = 2, NonDet = 3 }

impl fmt::Display for EffectTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self { EffectTag::Pure => write!(f, "#pure"), EffectTag::State => write!(f, "#state"), EffectTag::IO => write!(f, "#io"), EffectTag::NonDet => write!(f, "#nondet") }
    }
}

pub fn default_cache_id() -> Arc<RwLock<Option<ContentHash>>> {
    Arc::new(RwLock::new(None))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    Top, Atom(AtomKind, EffectTag, Option<i64>), Combo(ComboVal), Union(Vec<Value>), Code(Box<Expr>),
    Thunk { expr: Box<Expr>, closure: Vec<ComboVal>, effect: EffectTag },
    Bottom(Box<BottomDetail>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Top, Value::Top) => true,
            (Value::Atom(a1, e1, r1), Value::Atom(a2, e2, r2)) => a1 == a2 && e1 == e2 && r1 == r2,
            (Value::Combo(c1), Value::Combo(c2)) => c1 == c2,
            (Value::Union(u1), Value::Union(u2)) => u1 == u2,
            (Value::Code(c1), Value::Code(c2)) => c1 == c2,
            (Value::Thunk { expr: ex1, closure: cl1, effect: ef1 }, Value::Thunk { expr: ex2, closure: cl2, effect: ef2 }) => ex1 == ex2 && cl1 == cl2 && ef1 == ef2,
            (Value::Bottom(b1), Value::Bottom(b2)) => b1 == b2,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComboVal {
    pub data: IndexMap<String, Value>,
    pub types: IndexMap<String, Value>,
    pub rules: IndexMap<String, Value>,
    pub meta: IndexMap<String, Value>,
    pub system: IndexMap<String, Value>,
    pub local: IndexMap<String, Value>,
    pub closed: bool,
    pub effect: EffectTag,
    pub relations: Vec<ValRelation>,
    pub masa_ref: MasaRef,
    #[serde(skip, default = "default_cache_id")]
    pub cache_id: Arc<RwLock<Option<ContentHash>>>,
    #[serde(skip, default)]
    pub legacy_fields: IndexMap<String, Value>,
    #[serde(skip, default)]
    pub legacy_local: IndexMap<String, Value>,
}

impl Default for ComboVal {
    fn default() -> Self {
        Self {
            data: IndexMap::new(),
            types: IndexMap::new(),
            rules: IndexMap::new(),
            meta: IndexMap::new(),
            system: IndexMap::new(),
            local: IndexMap::new(),
            closed: false,
            effect: EffectTag::Pure,
            relations: vec![],
            masa_ref: MasaRef::Top,
            cache_id: default_cache_id(),
            legacy_fields: IndexMap::new(),
            legacy_local: IndexMap::new(),
        }
    }
}

impl ComboVal {
    pub fn new(fields: IndexMap<String, Value>, closed: bool, local_fields: IndexMap<String, Value>, effect: EffectTag, relations: Vec<ValRelation>) -> Self {
        let mut cv = Self::default();
        cv.closed = closed;
        cv.effect = effect;
        cv.relations = relations;
        for (k, v) in fields {
            cv.insert_field(&k, v);
        }
        for (k, v) in local_fields {
            let key_stripped = k.trim().trim_start_matches('~');
            cv.local.insert(key_stripped.to_string(), v);
        }
        cv
    }

    pub fn insert_field(&mut self, key: &str, value: Value) {
        let key_trimmed = key.trim();
        if key_trimmed.starts_with("~%") {
            let name = key_trimmed[2..].to_string();
            self.system.insert(name, value);
        } else if key_trimmed.starts_with("/") {
            let name = key_trimmed[1..].to_string();
            self.rules.insert(name, value);
        } else if key_trimmed.starts_with("@") {
            let name = key_trimmed[1..].to_string();
            self.types.insert(name, value);
        } else if key_trimmed.starts_with("%") {
            let name = key_trimmed[1..].to_string();
            self.meta.insert(name, value);
        } else if key_trimmed.starts_with("~") {
            let name = key_trimmed[1..].to_string();
            self.local.insert(name, value);
        } else {
            self.data.insert(key_trimmed.to_string(), value);
        }
    }

    pub fn get_field(&self, key: &str) -> Option<&Value> {
        let key_trimmed = key.trim();
        if key_trimmed.starts_with("~%") {
            let name = &key_trimmed[2..];
            self.system.get(name)
        } else if key_trimmed.starts_with("/") {
            let name = &key_trimmed[1..];
            self.rules.get(name)
        } else if key_trimmed.starts_with("@") {
            let name = &key_trimmed[1..];
            self.types.get(name)
        } else if key_trimmed.starts_with("%") {
            let name = &key_trimmed[1..];
            self.meta.get(name)
        } else if key_trimmed.starts_with("~") {
            let name = &key_trimmed[1..];
            self.local.get(name)
        } else {
            self.data.get(key_trimmed)
        }
    }

    pub fn get_local_field(&self, name: &str) -> Option<&Value> {
        let name_trimmed = name.trim().trim_start_matches('~');
        self.local.get(name_trimmed)
    }

    pub fn get_field_mut(&mut self, key: &str) -> Option<&mut Value> {
        let key_trimmed = key.trim();
        if key_trimmed.starts_with("~%") {
            let name = &key_trimmed[2..];
            self.system.get_mut(name)
        } else if key_trimmed.starts_with("/") {
            let name = &key_trimmed[1..];
            self.rules.get_mut(name)
        } else if key_trimmed.starts_with("@") {
            let name = &key_trimmed[1..];
            self.types.get_mut(name)
        } else if key_trimmed.starts_with("%") {
            let name = &key_trimmed[1..];
            self.meta.get_mut(name)
        } else if key_trimmed.starts_with("~") {
            let name = &key_trimmed[1..];
            self.local.get_mut(name)
        } else {
            self.data.get_mut(key_trimmed)
        }
    }

    pub fn fields(&self) -> IndexMap<String, Value> {
        let mut all = IndexMap::new();
        for (k, v) in &self.data { all.insert(k.clone(), v.clone()); }
        for (k, v) in &self.rules { all.insert(format!("/{}", k), v.clone()); }
        for (k, v) in &self.types { all.insert(format!("@{}", k), v.clone()); }
        for (k, v) in &self.meta { all.insert(format!("%{}", k), v.clone()); }
        for (k, v) in &self.system { all.insert(format!("~%{}", k), v.clone()); }
        all
    }

    pub fn fields_iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.data.iter()
            .chain(self.rules.iter().map(|(k, v)| (k, v)))
    }

    pub fn all_fields_iter(&self) -> impl Iterator<Item = (String, Value)> + '_ {
        self.data.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .chain(self.rules.iter().map(|(k, v)| (format!("/{}", k), v.clone())))
            .chain(self.types.iter().map(|(k, v)| (format!("@{}", k), v.clone())))
            .chain(self.meta.iter().map(|(k, v)| (format!("%{}", k), v.clone())))
            .chain(self.system.iter().map(|(k, v)| (format!("~%{}", k), v.clone())))
    }

    pub fn field_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        for k in self.data.keys() { keys.push(k.clone()); }
        for k in self.rules.keys() { keys.push(format!("/{}", k)); }
        for k in self.types.keys() { keys.push(format!("@{}", k)); }
        for k in self.meta.keys() { keys.push(format!("%{}", k)); }
        for k in self.system.keys() { keys.push(format!("~%{}", k)); }
        keys
    }

    pub fn local_fields(&self) -> IndexMap<String, Value> {
        let mut all = IndexMap::new();
        for (k, v) in &self.local { all.insert(format!("~{}", k), v.clone()); }
        all
    }

    pub fn local_keys(&self) -> Vec<String> {
        self.local.keys().map(|k| format!("~{}", k)).collect()
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.get_field(key).is_some()
    }

    pub fn bits(&self) -> u64 {
        let mut b = 64u64;
        for (k, v) in &self.data { b += (k.len() as u64) * 8 + v.bits(); }
        for (k, v) in &self.rules { b += ((k.len() + 1) as u64) * 8 + v.bits(); }
        for (k, v) in &self.types { b += ((k.len() + 1) as u64) * 8 + v.bits(); }
        for (k, v) in &self.meta { b += ((k.len() + 1) as u64) * 8 + v.bits(); }
        for (k, v) in &self.system { b += ((k.len() + 2) as u64) * 8 + v.bits(); }
        for (k, v) in &self.local { b += ((k.len() + 1) as u64) * 8 + v.bits(); }
        b
    }
}

impl PartialEq for ComboVal {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data && self.types == other.types && self.rules == other.rules 
            && self.meta == other.meta && self.system == other.system && self.local == other.local
            && self.closed == other.closed && self.effect == other.effect && self.relations == other.relations
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelOp { Lt, Gt, Lte, Gte }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValRelation {
    pub left: String,
    pub op: RelOp,
    pub right: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Holonomy {
    Phase(f64),
    NegI,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BottomDetail {
    pub cause: BottomCause, 
    pub path: Option<String>, 
    pub message: Option<String>,
    pub expected: Option<Value>,
    pub found: Option<Value>,
    pub involved: Vec<ContentHash>,
    pub obstruction_degree: Option<u8>,
    pub holonomy: Option<Holonomy>,
}

impl BottomDetail {
    /// Construct a BottomDetail with default None for obstruction fields
    pub fn new(cause: BottomCause, path: Option<String>, message: Option<String>,
               expected: Option<Value>, found: Option<Value>, involved: Vec<ContentHash>) -> Self {
        BottomDetail { cause, path, message, expected, found, involved, obstruction_degree: None, holonomy: None }
    }

    pub fn bits(&self) -> u64 {
        let mut b = 128u64;
        if let Some(ref p) = self.path { b += (p.len() as u64) * 8; }
        if let Some(ref m) = self.message { b += (m.len() as u64) * 8; }
        b += (self.involved.len() as u64) * 256;
        if self.obstruction_degree.is_some() { b += 64; }
        if self.holonomy.is_some() { b += 64; }
        b
    }

    pub fn as_cause_combo(&self) -> Value {
        use num_bigint::BigInt;
        let mut fields = IndexMap::new();
        let type_tag = match self.cause {
            BottomCause::Conflict => "#conflict",
            BottomCause::MissingKey => "#missing_key",
            BottomCause::FuelExhausted => "#fuel_exhausted",
            BottomCause::Timeout => "#timeout",
            BottomCause::Divergent => "#divergent",
            BottomCause::InvalidPath => "#invalid_path",
            BottomCause::PrivateAccessViolation => "#private_access_violation",
            BottomCause::NumericalError => "#numerical_error",
            BottomCause::ArithmeticOnAnchor => "#arithmetic_on_anchor",
            BottomCause::H1Split => "#h1_split",
            BottomCause::H2Split => "#h2_split",
        };
        fields.insert("%type".to_string(), Value::Atom(AtomKind::Tag(type_tag[1..].to_string()), EffectTag::Pure, None));
        if let Some(ref p) = self.path {
            fields.insert("%path".to_string(), Value::Atom(AtomKind::Str(p.clone()), EffectTag::Pure, None));
        }
        if let Some(ref m) = self.message {
            fields.insert("%message".to_string(), Value::Atom(AtomKind::Str(m.clone()), EffectTag::Pure, None));
        }
        if let Some(ref e) = self.expected {
            fields.insert("%expected".to_string(), e.clone());
        }
        if let Some(ref f) = self.found {
            fields.insert("%found".to_string(), f.clone());
        }
        if !self.involved.is_empty() {
            let mut involved_fields = IndexMap::new();
            for (i, h) in self.involved.iter().enumerate() {
                involved_fields.insert(i.to_string(), Value::Atom(AtomKind::Str(h.to_string()), EffectTag::Pure, None));
            }
            involved_fields.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
            fields.insert("%involved".to_string(), Value::Combo(ComboVal::new(involved_fields, false, IndexMap::new(), EffectTag::Pure, vec![])));
        }

        // Phase NEW: cocycle format (SPEC_06 §1.3.2)
        if let Some(degree) = self.obstruction_degree {
            fields.insert("%degree".to_string(), Value::Atom(AtomKind::Int(BigInt::from(degree)), EffectTag::Pure, None));
            let obs_tag = match degree { 1 => "h1_phase", 2 => "h2_sign", 3 => "h3_gerbe", 4 => "h4_sybil", _ => "unknown" };
            fields.insert("%obstruction".to_string(), Value::Atom(AtomKind::Tag(obs_tag.to_string()), EffectTag::Pure, None));

            // %cocycle: build from involved
            if !self.involved.is_empty() {
                let mut cyc = IndexMap::new();
                cyc.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
                for (i, h) in self.involved.iter().enumerate() {
                    cyc.insert(i.to_string(), Value::Atom(AtomKind::Str(h.to_string()), EffectTag::Pure, None));
                }
                // H²: pad to 4 positions (spec requires 4-cycle)
                if degree == 2 && self.involved.len() == 2 {
                    cyc.insert("2".to_string(), Value::Atom(AtomKind::Tag("_".to_string()), EffectTag::Pure, None));
                    cyc.insert("3".to_string(), Value::Atom(AtomKind::Tag("_".to_string()), EffectTag::Pure, None));
                }
                fields.insert("%cocycle".to_string(), Value::Combo(ComboVal::new(cyc, false, IndexMap::new(), EffectTag::Pure, vec![])));
            }

            // %holonomy
            if let Some(ref h) = self.holonomy {
                let hv = match h {
                    Holonomy::Phase(theta) => Value::Atom(AtomKind::Float(*theta), EffectTag::Pure, None),
                    Holonomy::NegI => Value::Atom(AtomKind::Tag("neg_I".to_string()), EffectTag::Pure, None),
                };
                fields.insert("%holonomy".to_string(), hv);
            }

            // %branches: H²
            if degree == 2 {
                fields.insert("%branches".to_string(), Value::Atom(AtomKind::Int(2u8.into()), EffectTag::Pure, None));
            }
        }

        Value::Combo(ComboVal::new(fields, true, IndexMap::new(), EffectTag::Pure, vec![]))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum BottomCause { #[default] Conflict, MissingKey, FuelExhausted, Timeout, Divergent, InvalidPath, PrivateAccessViolation, NumericalError, ArithmeticOnAnchor, H1Split, H2Split }

impl From<BottomCause> for Value {
    fn from(cause: BottomCause) -> Self {
        Value::Bottom(Box::new(BottomDetail { cause, path: None, message: None, expected: None, found: None, involved: vec![], obstruction_degree: None, holonomy: None }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HashAlgorithm { Sha256 }

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CaidVersion { V1, V2 }

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MasaRef {
    Top,
    Digest(Vec<u8>),
}

impl fmt::Display for MasaRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MasaRef::Top => write!(f, "_"),
            MasaRef::Digest(d) => write!(f, "{}", hex::encode(d)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentHash {
    pub algorithm: HashAlgorithm,
    pub version: CaidVersion,
    pub masa_ref: MasaRef,
    pub lattice_sketch: String,
    pub digest: Vec<u8>,
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let algo = "sha256";
        let digest_hex = hex::encode(&self.digest);
        match self.version {
            CaidVersion::V1 => write!(f, "hash:{}:v1:{}", algo, digest_hex),
            CaidVersion::V2 => write!(f, "hash:{}:v2:{}:{}:{}", algo, self.masa_ref, self.lattice_sketch, digest_hex),
        }
    }
}

impl ContentHash {
    /// Convenience constructor for internal hashing (v1 format, Top MASA)
    pub fn v1(digest: Vec<u8>) -> Self {
        ContentHash {
            algorithm: HashAlgorithm::Sha256,
            version: CaidVersion::V1,
            masa_ref: MasaRef::Top,
            lattice_sketch: String::new(),
            digest,
        }
    }

    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() < 4 || parts[0] != "hash" || parts[1] != "sha256" {
            return Err(anyhow::anyhow!("Invalid CAID format"));
        }
        match parts[2] {
            "v1" => Ok(ContentHash {
                algorithm: HashAlgorithm::Sha256,
                version: CaidVersion::V1,
                masa_ref: MasaRef::Top,
                lattice_sketch: String::new(),
                digest: hex::decode(parts[3])?,
            }),
            "v2" => {
                if parts.len() < 6 {
                    return Err(anyhow::anyhow!("Invalid v2 CAID: needs 6 colon-delimited parts"));
                }
                let masa_ref = if parts[3] == "_" { MasaRef::Top } else { MasaRef::Digest(hex::decode(parts[3])?) };
                Ok(ContentHash {
                    algorithm: HashAlgorithm::Sha256,
                    version: CaidVersion::V2,
                    masa_ref,
                    lattice_sketch: parts[4].to_string(),
                    digest: hex::decode(parts[5])?,
                })
            }
            _ => Err(anyhow::anyhow!("Unknown CAID version: {}", parts[2])),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommitMeta { pub author: Option<String>, pub timestamp: u64, pub message: Option<String> }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitKind { Refine, #[serde(other)] Standard }
impl Default for CommitKind { fn default() -> Self { Self::Standard } }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefineInfo {
    pub source_caids: Vec<ContentHash>,
    pub target_caids: Vec<ContentHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority_signer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub parent: Option<ContentHash>,
    pub root: ContentHash,
    pub meta: CommitMeta,
    #[serde(default)]
    pub kind: CommitKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refine_info: Option<RefineInfo>,
    #[serde(skip, default = "default_cache_id")]
    pub cache_id: Arc<RwLock<Option<ContentHash>>>,
}

impl Default for Commit {
    fn default() -> Self {
        Self {
            parent: None,
            root: ContentHash::v1(vec![0; 32]),
            meta: CommitMeta { author: None, timestamp: 0, message: None },
            kind: CommitKind::Standard,
            refine_info: None,
            cache_id: default_cache_id(),
        }
    }
}

impl PartialEq for Commit {
    fn eq(&self, other: &Self) -> bool {
        self.parent == other.parent && self.root == other.root && self.meta == other.meta
            && self.kind == other.kind && self.refine_info == other.refine_info
    }
}

impl Commit {
    pub fn new(parent: Option<ContentHash>, root: ContentHash, meta: CommitMeta) -> Self {
        Self { parent, root, meta, kind: CommitKind::Standard, refine_info: None, cache_id: default_cache_id() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Identity { pub public_key: Vec<u8>, pub private_key: Vec<u8> }

impl Identity {
    pub fn new_random() -> Self {
        let rng = rand::SystemRandom::new();
        let pkcs8_bytes = signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let key_pair = signature::Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref()).unwrap();
        Self { public_key: key_pair.public_key().as_ref().to_vec(), private_key: pkcs8_bytes.as_ref().to_vec() }
    }
}

pub const TROPICAL_INFINITY: u64 = u64::MAX;

impl Value {
    pub fn bits(&self) -> u64 {
        match self {
            Value::Top => 0,
            Value::Atom(kind, _, _) => match kind {
                AtomKind::Int(i) => i.bits() as u64,
                AtomKind::Float(_) => 64,
                AtomKind::Complex(_, _) => 128,
                AtomKind::Str(s) | AtomKind::MultilineStr(s) => (s.len() as u64) * 8,
                AtomKind::Tag(t) => (t.len() as u64) * 8 + 64,
                AtomKind::TagStart | AtomKind::TagEnd => 32,
                AtomKind::Top => 0,
                AtomKind::Bottom => 128,
                _ => 128,
            },
            Value::Combo(c) => c.bits(),
            Value::Union(branches) => branches.iter().map(|b| b.bits()).sum(),
            Value::Code(_) | Value::Thunk { .. } => 256,
            Value::Bottom(d) => d.bits(),
        }
    }

    pub fn tropical_weight(&self) -> u64 {
        match self {
            Value::Top => 0,
            Value::Bottom(_) => TROPICAL_INFINITY,
            Value::Atom(_, _, _) => 1,
            Value::Thunk { .. } | Value::Code(_) => 1,
            Value::Union(branches) => branches.iter().map(|b| b.tropical_weight()).min().unwrap_or(TROPICAL_INFINITY),
            Value::Combo(c) => c.all_fields_iter().map(|(_, v)| v.tropical_weight()).fold(0u64, |acc, w| acc.saturating_add(w)),
        }
    }

    pub fn is_top(&self) -> bool { matches!(self, Value::Top) }
    pub fn with_effect(self, e: EffectTag) -> Self {
        match self {
            Value::Atom(ak, old_e, r) => Value::Atom(ak, old_e.max(e), r),
            Value::Combo(mut cv) => { cv.effect = cv.effect.max(e); Value::Combo(cv) },
            Value::Union(branches) => Value::Union(branches.into_iter().map(|b| b.with_effect(e)).collect()),
            Value::Thunk { expr, closure, effect } => Value::Thunk { expr, closure, effect: effect.max(e) },
            _ => self
        }
    }

    pub fn is_morphism(&self) -> bool {
        if let Value::Combo(ref c) = self {
            if c.contains_key("%morphism") || c.contains_key("%rules") || c.contains_key("%builtin") {
                return true;
            }
        }
        match self.collapse() {
            Value::Combo(c) => c.contains_key("%morphism") || c.contains_key("%rules") || c.contains_key("%builtin"),
            _ => false,
        }
    }
    pub fn effect(&self) -> EffectTag {
        match self { 
            Value::Combo(c) => c.effect, 
            Value::Atom(_, e, None) => *e,
            Value::Thunk { effect, .. } => *effect, 
            Value::Union(b) => b.iter().map(|v| v.effect()).max().unwrap_or(EffectTag::Pure), 
            _ => EffectTag::Pure 
        }
    }
    pub fn collapse(&self) -> &Value { match self { Value::Combo(c) => c.get_field("%val").map(|v| v.collapse()).unwrap_or(self), _ => self } }
    
    pub fn collapse_with_effect(&self) -> (Value, EffectTag) {
        match self {
            Value::Combo(c) => {
                if let Some(v) = c.get_field("%val") {
                    let (inner, inner_e) = v.collapse_with_effect();
                    (inner, inner_e.max(c.effect))
                } else {
                    (self.clone(), c.effect)
                }
            }
            Value::Atom(_, e, _) => (self.clone(), *e),
            Value::Thunk { effect, .. } => (self.clone(), *effect),
            Value::Union(branches) => {
                let max_e = branches.iter().map(|b| b.effect()).max().unwrap_or(EffectTag::Pure);
                (self.clone(), max_e)
            }
            _ => (self.clone(), EffectTag::Pure),
        }
    }
    
    pub fn to_string_plain(&self) -> String {
        match self {
            Value::Atom(kind, _, _) => match kind { 
                AtomKind::Int(i) => i.to_string(), 
                AtomKind::Float(f) => f.to_string(),
                AtomKind::Complex(r, i) => {
                    if *i >= 0.0 { format!("{}+{}i", r, i) }
                    else { format!("{}-{}i", r, i.abs()) }
                },
                AtomKind::Str(s) => s.clone(), 
                AtomKind::Tag(t) => format!("#{}", t), 
                AtomKind::TagStart => "#_|_".to_string(),
                AtomKind::TagEnd => "#_".to_string(),
                AtomKind::Top => "_".to_string(),
                AtomKind::Bottom => "_|_".to_string(),
                _ => format!("{:?}", kind) 
            },
            Value::Top => "_".to_string(),
            Value::Bottom(d) => format!("_|_ (%cause: {:?})", d.cause),
            Value::Combo(c) => { if let Some(v) = c.get_field("%val") { return v.to_string_plain(); } "{...}".to_string() }
            Value::Union(_) => "(...|...)".to_string(),
            _ => format!("{:?}", self),
        }
    }

    pub fn to_nlang(&self, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        match self {
            Value::Top => "_".to_string(),
            Value::Atom(kind, effect, rank) => {
                let mut s = match kind {
                    AtomKind::Int(i) => i.to_string(),
                    AtomKind::Float(f) => f.to_string(),
                    AtomKind::Complex(r, i) => {
                        if *i >= 0.0 { format!("{}+{}i", r, i) }
                        else { format!("{}-{}i", r, i.abs()) }
                    },
                    AtomKind::Str(s) => format!("\"{}\"", s),
                    AtomKind::Tag(t) => format!("#{}", t),
                    AtomKind::TagStart => "#_|_".to_string(),
                    AtomKind::TagEnd => "#_".to_string(),
                    _ => format!("{:?}", kind),
                };
                if let Some(r) = rank { s.push_str(&format!("  ;; %rank: {}", r)); }
                if *effect > EffectTag::Pure { s.push_str(&format!("  ;; %effect: {}", effect)); }
                s
            },
            Value::Combo(c) => {
                let is_list = c.get_field("%kind").map(|k| k.to_string_plain() == "#list").unwrap_or(false);
                if is_list {
                    let mut items = Vec::new();
                    let mut i = 0;
                    while let Some(v) = c.get_field(&i.to_string()) { items.push(v.to_nlang(indent + 1)); i += 1; }
                    return format!("[{}]", items.join(", "));
                }

                let mut s = if c.closed { "{{".to_string() } else { "{".to_string() };
                if c.data.is_empty() && c.types.is_empty() && c.rules.is_empty() && c.meta.is_empty() && c.system.is_empty() && c.local.is_empty() { return format!("{} }}", s); }
                s.push('\n');
                let fields = c.fields();
                let mut keys: Vec<_> = fields.keys().collect(); keys.sort();
                for k in keys { let v = fields.get(k).unwrap(); s.push_str(&format!("{}  {}: {}\n", pad, k, v.to_nlang(indent + 1))); }
                let local = c.local_fields();
                let mut lkeys: Vec<_> = local.keys().collect(); lkeys.sort();
                for k in lkeys { let v = local.get(k).unwrap(); s.push_str(&format!("{}  {}: {}\n", pad, k, v.to_nlang(indent + 1))); }
                s.push_str(&format!("{}}}", pad)); if c.closed { s.push('}'); }
                s
            }
            Value::Union(branches) => { let parts: Vec<String> = branches.iter().map(|b| b.to_nlang(indent)).collect(); parts.join(" | ") }
            Value::Bottom(d) => { let mut s = "_|_".to_string(); if let Some(ref m) = d.message { s.push_str(&format!("  ;; {}", m)); } s }
            _ => format!("{:?}", self),
        }
    }

    pub fn content_hash_with_salt(&self, salt: &ContentHash) -> ContentHash {
        let mut hasher = Sha256::new(); 
        if self.effect() > EffectTag::Pure { hasher.update(b"HORIZON_SALT_V1"); hasher.update(&salt.digest); }
        self.hash_recursive_with_salt(&mut hasher, salt);
        ContentHash::v1(hasher.finalize().to_vec())
    }
    
    pub fn content_hash(&self) -> ContentHash {
        let bn_bytes = crate::bn_serial::serialize_bn(self);
        let digest = crate::bn_serial::content_digest(self);
        let sketch = crate::lattice_sketch::compute_sketch_v2(self);
        let masa_ref = match self {
            Value::Combo(c) => c.masa_ref.clone(),
            _ => MasaRef::Top,
        };
        ContentHash {
            algorithm: HashAlgorithm::Sha256,
            version: CaidVersion::V2,
            masa_ref,
            lattice_sketch: sketch,
            digest: digest.to_vec(),
        }
    }

    /// v1 format for genesis commit only
    pub fn content_hash_v1(&self) -> ContentHash {
        let digest = crate::bn_serial::content_digest(self);
        ContentHash {
            algorithm: HashAlgorithm::Sha256,
            version: CaidVersion::V1,
            masa_ref: MasaRef::Top,
            lattice_sketch: String::new(),
            digest: digest.to_vec(),
        }
    }

    pub fn verify_signature(&self) -> bool {
        if let Value::Combo(c) = self {
            if let (Some(Value::Atom(AtomKind::Str(pk_hex), _, None)), Some(Value::Atom(AtomKind::Str(sig_hex), _, None)), Some(target)) = 
                (c.get_field("%pubkey"), c.get_field("%signature"), c.get_field("%target")) {
                if let (Ok(pk_bytes), Ok(sig_bytes)) = (hex::decode(pk_hex), hex::decode(sig_hex)) {
                    let vk = signature::UnparsedPublicKey::new(&signature::ED25519, pk_bytes);
                    let msg = target.content_hash().to_string();
                    return vk.verify(msg.as_bytes(), &sig_bytes).is_ok();
                }
            }
        }
        false
    }

    fn hash_recursive_with_salt(&self, hasher: &mut Sha256, salt: &ContentHash) {
        match self {
            Value::Top => hasher.update([0x00]),
            Value::Atom(kind, effect, rank) => {
                hasher.update([0x01]);
                hasher.update([*effect as u8]);
                match kind {
                    AtomKind::Int(i) => { hasher.update([0x01]); let (sign, bytes) = i.to_bytes_be(); hasher.update(&[if sign == num_bigint::Sign::Minus { 1 } else { 0 }]); hasher.update(&bytes); }
                    AtomKind::Float(f) => { hasher.update([0x07]); hasher.update(f.to_bits().to_le_bytes()); }
                    AtomKind::Complex(r, i) => { hasher.update([0x08]); hasher.update(r.to_bits().to_le_bytes()); hasher.update(i.to_bits().to_le_bytes()); }
                    AtomKind::Str(s) => { hasher.update([0x02]); hasher.update(s.as_bytes()); }
                    AtomKind::Tag(t) => { hasher.update([0x03]); hasher.update(t.as_bytes()); }
                    AtomKind::TagStart => { hasher.update([0x04]); }
                    AtomKind::TagEnd => { hasher.update([0x05]); }
                    _ => { hasher.update([0x06]); hasher.update(format!("{:?}", kind).as_bytes()); }
                }
                if let Some(r) = rank { hasher.update(r.to_le_bytes()); }
            }
            Value::Combo(c) => {
                hasher.update([0x02]);
                hasher.update([if c.closed { 1 } else { 0 }]);
                hasher.update([c.effect as u8]);
                let fields = c.fields();
                let mut keys: Vec<_> = fields.keys().collect(); keys.sort();
                for k in keys {
                    hasher.update(k.as_bytes());
                    fields.get(k).unwrap().hash_recursive_with_salt(hasher, salt);
                }
                let local = c.local_fields();
                let mut lkeys: Vec<_> = local.keys().collect(); lkeys.sort();
                for k in lkeys {
                    hasher.update(b"local:");
                    hasher.update(k.as_bytes());
                    local.get(k).unwrap().hash_recursive_with_salt(hasher, salt);
                }
            }
            Value::Union(branches) => {
                hasher.update([0x03]);
                let mut digests: Vec<_> = branches.iter().map(|b| {
                    let mut h = Sha256::new();
                    b.hash_recursive_with_salt(&mut h, salt);
                    h.finalize().to_vec()
                }).collect();
                digests.sort();
                for d in digests { hasher.update(&d); }
            }
            Value::Bottom(d) => { hasher.update([0x04]); hasher.update([d.cause as u8]); }
            Value::Thunk { expr, .. } => { hasher.update([0x05]); hasher.update(format!("{:?}", expr).as_bytes()); }
            Value::Code(expr) => { hasher.update([0x06]); hasher.update(format!("{:?}", expr).as_bytes()); }
        }
    }
}

impl ComboVal {
    pub fn content_hash(&self) -> ContentHash {
        let v = Value::Combo(self.clone());
        v.content_hash()
    }
}

impl Commit {
    pub fn content_hash(&self) -> ContentHash {
        let mut buf = Vec::new();
        if let Some(p) = &self.parent {
            buf.extend_from_slice(&p.digest);
        }
        buf.extend_from_slice(&self.root.digest);
        buf.push(match self.kind { CommitKind::Standard => 0, CommitKind::Refine => 1 });
        if let Some(ref ri) = self.refine_info {
            for src in &ri.source_caids { buf.extend_from_slice(&src.digest); }
            for tgt in &ri.target_caids { buf.extend_from_slice(&tgt.digest); }
        }
        let meta_bytes = format!("{:?}", self.meta);
        crate::bn_serial::encode_unsigned_leb128(meta_bytes.len() as u64, &mut buf);
        buf.extend_from_slice(meta_bytes.as_bytes());
        let digest = Sha256::digest(&buf).to_vec();
        ContentHash::v1(digest)
    }
}
