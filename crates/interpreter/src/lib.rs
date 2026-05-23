pub mod universe; pub use universe::Universe;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use indexmap::IndexMap;
use nlang_parser::ast::{Path, PathAnchor, AtomKind};
use num_bigint::BigInt;
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
pub use crate::value::{Value, ComboVal, EffectTag, ContentHash, CaidVersion, MasaRef, BottomDetail, BottomCause, CommitMeta, Commit};
pub use crate::storage::ObjectStore;
pub use crate::dispatch::{MorphismDispatchResult, MorphismDispatchResult as DispatchResult};
pub use crate::observation::{ObservationState, ObservationStrategy, handle_resource_exhausted};
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
            horizon_salt: salt, strategy: ObservationStrategy::Blur 
        }
    }
    pub fn with_fuel(mut self, fuel: u64) -> Self { self.fuel = fuel; self }
    pub fn with_strategy(mut self, strategy: ObservationStrategy) -> Self { self.strategy = strategy; self }
    pub fn check_resources(&mut self, cost: u64) -> Result<(), ResourceExhausted> { 
        if self.fuel < cost { Err(ResourceExhausted::FuelExhausted) } 
        else if self.depth > 100 { Err(ResourceExhausted::StackOverflow) } 
        else { self.fuel -= cost; Ok(()) } 
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
    pub unify_memo: RwLock<HashMap<(ContentHash, ContentHash), Value>>,
    pub builtin_registry: HashMap<String, Arc<BuiltinFn>>,
    pub peers: RwLock<HashMap<String, Peer>>,
    pub identity: crate::value::Identity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp { Eq, Ne, Lt, Gt, Lte, Gte }

impl Ouroboros {
    pub fn new_in_memory() -> Self {
        use ring::rand::SecureRandom;
        let mut bytes = [0u8; 8];
        ring::rand::SystemRandom::new().fill(&mut bytes).unwrap();
        let dir = std::env::temp_dir().join(format!("nlang-test-{}", hex::encode(bytes)));
        Self::init(&dir).unwrap()
    }

    pub fn init(base_dir: &std::path::Path) -> Result<Self> {
        let store = ObjectStore::init(base_dir)?;
        Ok(Self { store, unify_memo: RwLock::new(HashMap::new()), builtin_registry: create_default_builtins(), peers: RwLock::new(HashMap::new()), identity: crate::value::Identity::new_random() })
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
        let math_morphisms = vec![("/sub", "math.sub"), ("/mul", "math.mul"), ("/div", "math.div"), ("/rem", "math.rem"), ("/abs", "math.abs"), ("/bits", "math.bits"), ("/pow", "math.pow"), ("/sqrt", "math.sqrt"), ("/bitAnd", "math.bitAnd"), ("/bitOr", "math.bitOr"), ("/bitXor", "math.bitXor"), ("/bitNot", "math.bitNot"), ("/shl", "math.shl"), ("/shr", "math.shr")];
        for (n, b) in math_morphisms { math_builtins.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::Pure, vec![]))); }
        math_builtins.insert("/random".to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str("math.random".to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::NonDet, vec![])));
        fields.insert("~%Math".to_string(), Value::Combo(ComboVal::new(math_builtins, true, IndexMap::new(), EffectTag::Pure, vec![])));

        let mut cond_fields = IndexMap::new();
        let cond_morphisms = vec![("/if", "cond.if"), ("/cond", "cond.cond"), ("/match", "cond.match")];
        for (n, b) in cond_morphisms { cond_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::Pure, vec![]))); }
        fields.insert("~%Cond".to_string(), Value::Combo(ComboVal::new(cond_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));

        let mut list_fields = IndexMap::new();
        let list_morphisms = vec![("/map", "list.map"), ("/filter", "list.filter"), ("/fold", "list.fold"), ("/len", "list.len"), ("/concat", "list.concat"), ("/at", "list.at"), ("/sort", "list.sort"), ("/reverse", "list.reverse"), ("/slice", "list.slice"), ("/zip", "list.zip")];
        for (n, b) in list_morphisms { list_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::Pure, vec![]))); }
        fields.insert("~%List".to_string(), Value::Combo(ComboVal::new(list_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
        
        let mut string_fields = IndexMap::new();
        let string_morphisms = vec![
            ("/concat", "str.concat"), ("/split", "str.split"), ("/join", "str.join"), ("/trim", "str.trim"), ("/len", "str.len"),
            ("/replace", "str.replace"), ("/to_lower", "str.to_lower"), ("/to_upper", "str.to_upper"), 
            ("/starts_with", "str.starts_with"), ("/ends_with", "str.ends_with"), ("/contains", "str.contains")
        ];
        for (n, b) in string_morphisms { string_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::Pure, vec![]))); }
        fields.insert("~%String".to_string(), Value::Combo(ComboVal::new(string_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
        
        let mut time_fields = IndexMap::new();
        time_fields.insert("/now".to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str("time.now".to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::IO, vec![])));
        fields.insert("~%Time".to_string(), Value::Combo(ComboVal::new(time_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
        
        let mut disc_fields = IndexMap::new();
        let disc_morphisms = vec![("/connect", "disc.connect"), ("/fetch", "disc.fetch"), ("/identify", "disc.identify"), ("/identify_and_store", "engine.save")];
        for (n, b) in disc_morphisms { disc_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::IO, vec![]))); }
        fields.insert("~%Discovery".to_string(), Value::Combo(ComboVal::new(disc_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
        
let mut refl_fields = IndexMap::new();
        let refl_morphisms = vec![("/keys", "refl.keys"), ("/has", "refl.has"), ("/is_cocoon", "refl.is_cocoon"), ("/type_of", "refl.type_of")];
        for (n, b) in refl_morphisms { refl_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::Pure, vec![]))); }
        fields.insert("~%Reflection".to_string(), Value::Combo(ComboVal::new(refl_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
        
        let mut complex_fields = IndexMap::new();
        let complex_morphisms = vec![("/conj", "complex.conj"), ("/phase", "complex.phase"), ("/real", "complex.real"), ("/imag", "complex.imag")];
        for (n, b) in complex_morphisms { complex_fields.insert(n.to_string(), Value::Combo(ComboVal::new(IndexMap::from_iter(vec![("%morphism".to_string(), Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None)), ("%builtin".to_string(), Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None))]), true, IndexMap::new(), EffectTag::Pure, vec![]))); }
        fields.insert("~%Complex".to_string(), Value::Combo(ComboVal::new(complex_fields, true, IndexMap::new(), EffectTag::Pure, vec![])));
        
        ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![])
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
                return handle_resource_exhausted(e, ctx.strategy, &ctx.horizon_salt, None, accumulated_effect);
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
