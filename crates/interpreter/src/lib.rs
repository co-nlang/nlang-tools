pub mod universe;
use indexmap::IndexMap;
use nlang_parser::ast::{AddressAlgo, AtomKind, Expr, ExprKind, Path, PathAnchor};
use nlang_parser::tier::{classify_tier, Tier};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, RwLock,
};
pub use universe::Universe;
pub mod authority;
pub mod bn_serial;
pub mod builtins;
pub mod complement;
pub mod discovery_config;
pub mod dispatch;
pub mod eval;
pub mod gc;
pub mod genesis;
pub mod ladd;
pub mod lattice_sketch;
pub mod observation;
pub mod oml;
pub mod oodp;
pub mod peers;
pub mod routing;
pub mod scratch;
pub mod storage;
pub mod type_constraint;
pub mod unify;
pub mod value;
use crate::builtins::create_default_builtins;
pub use crate::dispatch::{MorphismDispatchResult, MorphismDispatchResult as DispatchResult};
pub use crate::observation::{handle_resource_exhausted, ObservationState};
pub use crate::scratch::ScratchDir;
pub use crate::storage::{value_address_matches, ObjectStore, StoreReadError};
use crate::type_constraint::{
    get_type_constraint_name, is_type_constraint_combo, is_user_field_type_combo, TypeConstraint,
};
pub use crate::value::{
    normalize_union, primary_bottom_from_culled, AuthorityInfo, BlurCause, BlurDetail, BottomCause,
    BottomDetail, CaidVersion, ComboVal, Commit, CommitKind, CommitMeta, ContentHash, EffectTag,
    Holonomy, HorizonParams, HorizonRecord, Identity, MasaRef, ObservationStrategy, Privilege,
    RefineInfo, Value,
};
use anyhow::Result;
use sha2::Digest;

/// REAL_01 §9.1's standardized Metered Billing Unit schedule. These are
/// semantic operations, deliberately separate from implementation visits: two
/// engines may use different data structures yet must put a #blur horizon at
/// the same semantic point.
///
/// The interpreter has no language-level FFI or spectral-calibration dispatch
/// yet, so those prescribed rows are retained here for the future binding but
/// intentionally have no current call site.
#[allow(dead_code)]
pub(crate) mod mbu {
    pub const SUBSPACE_EXPANSION: u64 = 1;
    pub const OPERATOR_APPLICATION: u64 = 10;
    pub const SPECTRAL_CALIBRATION: u64 = 25;
    pub const ORTHOGONAL_MERGE: u64 = 5;
    pub const LIFTING_BASE: u64 = 5;
    pub const FFI_BASE: u64 = 50;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceExhausted {
    FuelExhausted,
    Timeout,
    /// Implementation recursion ceiling — incapacity, not policy
    /// (`max_unification_depth`). Always becomes ⊥ `#stack_overflow`, never
    /// `#blur` (W4‴ / a_limit_you_cannot_choose).
    StackOverflow,
    /// Unification / observation depth budget exhausted (ERROR_CODES §2.7.2).
    /// Distinct from FuelExhausted — different operator knob.
    DepthExceeded,
}

/// Implementation-owned recursion ceiling (W4‴).
///
/// Measured 2026-08-09 on the 64 MiB CLI thread (`main.rs`): policy depth
/// 488 still exits cleanly; 499+ dumps core (`stack overflow, aborting`).
/// Frame size ≈ 64 MiB / ~490 ≈ 134 KB/layer. This constant is a **safety
/// margin under the measured ~488** so no operator-chosen
/// `max_unification_depth` can outrun the native stack. Not a policy knob.
pub const HARD_RECURSION_LIMIT: u32 = 400;

#[derive(Debug, Clone)]
pub struct EvalContext {
    // F2 (Stage 3-fix): Arc so EvalContext::clone (sub_context on every thunk
    // force) bumps a refcount instead of deep-copying the universe — the
    // O(depth x N_fields x |root|) amplifier in self-referential observation.
    // Reads deref transparently; the engine never mutates root mid-eval.
    pub root: Arc<ComboVal>,
    /// Engine-supplied standard table.  It is deliberately a lookup layer,
    /// not fields hydrated into `root`: user coordinates therefore win.
    pub standard_root: Arc<ComboVal>,
    /// Stage 4: lazily computed CAID of `root`, shared through sub_context
    /// clones. Sound because the engine never mutates root mid-observation
    /// (tests that mutate via Arc::make_mut do so before any force).
    pub root_caid_cache: Option<ContentHash>,
    /// Lexical scope frames, Arc-shared (D1 nesting). See Thunk.closure.
    pub scopes: Vec<std::sync::Arc<ComboVal>>,
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
    /// Initial fuel budget (config ceiling). Identity for blur CHS; `fuel` is remaining.
    pub fuel_budget: u64,
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
    /// Commit solidification stores quotes as quotes. Unfolding `<<_.>>`
    /// into the combo that contains it writes a self-nested JSON object
    /// this engine cannot read back (`#object_undecodable`). Observation
    /// still dereferences; only the durable projection keeps the Ref.
    pub preserve_refs: bool,
}

impl EvalContext {
    pub fn new(root: ComboVal) -> Self {
        let mut hasher = sha2::Sha256::new();
        hasher.update(b"default");
        let salt = ContentHash::v1(hasher.finalize().to_vec());
        Self {
            root: Arc::new(root),
            standard_root: Arc::new(ComboVal::default()),
            root_caid_cache: None,
            scopes: Vec::new(),
            staged: None,
            computing: HashSet::new(),
            call_history: HashMap::new(),
            in_flight: HashSet::new(),
            lexical_forcing: HashSet::new(),
            chain_transform_taint: false,
            cycle_chain: Vec::new(),
            in_math_op: false,
            in_evolve: false,
            union_absorb_fence: false,
            context_value: None,
            fuel: 10000,
            fuel_budget: 10000,
            timeout_deadline: None,
            depth: 0,
            dep_collector: None,
            memo_enabled: true,
            // Fixed salt for disc tie-break only — NOT blur identity (O42).
            horizon_salt: salt,
            strategy: ObservationStrategy::Blur,
            max_branches: 64,
            max_unification_depth: 256,
            max_pattern_nodes: 1024,
            max_lifting_depth: 32,
            refine_map_active: false,
            had_nondistrib_event: false,
            disc_routing_visited: std::collections::HashSet::new(),
            disc_routing_hops: 0,
            privilege: crate::value::Privilege::NONE,
            preserve_refs: false,
        }
    }

    pub fn with_standard_root(mut self, standard_root: ComboVal) -> Self {
        self.standard_root = Arc::new(standard_root);
        self
    }
    /// Root CAID for memo keys — computed once per root version, then cached
    /// (the cache rides along sub_context clones). Avoids the per-force
    /// deep-clone + full re-hash of the universe.
    pub fn root_caid(&mut self) -> ContentHash {
        if let Some(ref h) = self.root_caid_cache {
            return h.clone();
        }
        let h = Value::Combo((*self.root).clone()).content_hash();
        self.root_caid_cache = Some(h.clone());
        h
    }

    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = fuel;
        self.fuel_budget = fuel;
        self
    }

    /// Snapshot of horizon budgets for blur minting (O42 CHS).
    pub fn horizon_params(&self) -> crate::value::HorizonParams {
        crate::value::HorizonParams {
            fuel: self.fuel_budget,
            fuel_remaining: self.fuel,
            strategy: self.strategy,
            max_branches: self.max_branches as u64,
            max_unification_depth: self.max_unification_depth as u64,
            max_lifting_depth: self.max_lifting_depth as u64,
            max_pattern_nodes: self.max_pattern_nodes as u64,
        }
    }
    pub fn with_strategy(mut self, strategy: ObservationStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Apply `~%Config` horizon knobs (SPEC_08 §3.1 / SPEC_09 §6 closed family).
    ///
    /// * `include_fuel_strategy` — observe / `eval_context` apply fuel & strategy.
    ///   Evolve skips them: fuel already governs force-at-observe, and applying
    ///   it on evolve mints `#blur` under evolve's per-call random salt (moves
    ///   fuel-side CAIDs).
    /// * `apply_timeout` — only when the user staged a finite `timeout` override.
    ///   Genesis carries `timeout: #_` (unbound); a finite override alone may
    ///   arm a wall-clock deadline for the observation.
    pub fn apply_horizon_config(
        &mut self,
        cfg: &crate::value::ComboVal,
        include_fuel_strategy: bool,
        apply_timeout: bool,
    ) {
        use num_traits::ToPrimitive;
        if include_fuel_strategy {
            if let Some(Value::Atom(AtomKind::Int(n), _, _)) = cfg.get_field("fuel").cloned() {
                if let Some(f) = n.to_u64() {
                    self.fuel = f;
                    self.fuel_budget = f;
                }
            }
            if let Some(Value::Atom(AtomKind::Tag(s), _, _)) = cfg.get_field("strategy").cloned() {
                self.strategy = match s.trim_start_matches('#') {
                    "strict" => ObservationStrategy::Strict,
                    "approximate" => ObservationStrategy::Approximate,
                    _ => ObservationStrategy::Blur,
                };
            }
        }
        if let Some(v) = cfg.get_field("max_branches").cloned() {
            match v {
                // O41: `#_` = TagEnd (order supremum).
                Value::Atom(AtomKind::TagEnd, _, _) => {
                    self.max_branches = usize::MAX;
                }
                Value::Atom(AtomKind::Int(n), _, _) => {
                    if let Some(n) = n.to_u64() {
                        self.max_branches = n as usize;
                    }
                }
                _ => {}
            }
        }
        if let Some(Value::Atom(AtomKind::Int(n), _, _)) =
            cfg.get_field("max_unification_depth").cloned()
        {
            if let Some(v) = n.to_u64() {
                self.max_unification_depth = v as usize;
            }
        }
        if let Some(Value::Atom(AtomKind::Int(n), _, _)) =
            cfg.get_field("max_lifting_depth").cloned()
        {
            if let Some(v) = n.to_u64() {
                self.max_lifting_depth = v as usize;
            }
        }
        if let Some(v) = cfg.get_field("max_pattern_nodes").cloned() {
            match v {
                Value::Atom(AtomKind::TagEnd, _, _) => {
                    self.max_pattern_nodes = usize::MAX;
                }
                Value::Atom(AtomKind::Int(n), _, _) => {
                    if let Some(n) = n.to_u64() {
                        self.max_pattern_nodes = n as usize;
                    }
                }
                _ => {}
            }
        }
        if apply_timeout {
            match cfg.get_field("timeout").cloned() {
                // O41: `#_` (TagEnd) = no deadline.
                Some(Value::Atom(AtomKind::TagEnd, _, _)) => {
                    self.timeout_deadline = None;
                }
                Some(Value::Atom(AtomKind::Int(n), _, _)) => {
                    if let Some(timeout_ms) = n.to_u64() {
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        self.timeout_deadline = Some(now_ms + timeout_ms);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn check_resources(&mut self, cost: u64) -> Result<(), ResourceExhausted> {
        // G3 R3 / ERROR_CODES §2.7.2: depth gate is observation-budget
        // exhaustion, not a cycle and not fuel. Report DepthExceeded so
        // Blur %cause / Strict ⊥ share #max_depth_exceeded (L2-21/22;
        // #divergent reserved for L2-17 in_flight / coordinate self-ref;
        // #fuel_exhausted reserved for actual fuel).
        if self.depth > self.max_unification_depth as u32 {
            return Err(ResourceExhausted::DepthExceeded);
        }
        // W4‴: implementation ceiling — incapacity, not policy. Always below
        // the native-stack cliff (~490 frames / 64 MiB). Operator-chosen
        // max_unification_depth must not be able to dump core.
        if self.depth > HARD_RECURSION_LIMIT {
            return Err(ResourceExhausted::StackOverflow);
        }
        if let Some(deadline) = self.timeout_deadline {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            if now > deadline {
                return Err(ResourceExhausted::Timeout);
            }
        }
        // Structural policy limits have priority when multiple horizons are
        // crossed at the same evaluation point.  In particular, a depth-
        //bounded program must not be reported as fuel exhaustion merely
        // because the semantic meter was also depleted on the way there.
        if self.fuel < cost {
            return Err(ResourceExhausted::FuelExhausted);
        }
        self.fuel -= cost;
        Ok(())
    }
}

/// The semantic subspace-expansion bill for proving a combo already solid.
/// `None` means that a thunk/ref/pending spread needs the ordinary forcing
/// path. The fast path still walks dynamic value structure, so letting it
/// return for free would make a deep, already-solid cocoon evade the meter.
///
/// Count semantic containers and their projected members, not recursive-frame
/// visits: a depth-n cocoon bills O(n), even if this implementation revisits
/// frames while settling it.
fn solid_combo_expansion_cost(c: &ComboVal) -> Option<u64> {
    if !c.pending_spreads.is_empty() {
        return None;
    }
    let mut cost = mbu::SUBSPACE_EXPANSION;
    let mut pending: Vec<Value> = c
        .all_fields_iter()
        .map(|(_, value)| value)
        .chain(c.local.values().cloned())
        .collect();
    while let Some(value) = pending.pop() {
        cost = cost.saturating_add(mbu::SUBSPACE_EXPANSION);
        match value {
            Value::Thunk { .. } | Value::Ref(_) => return None,
            Value::Combo(inner) => {
                if !inner.pending_spreads.is_empty() {
                    return None;
                }
                pending.extend(inner.all_fields_iter().map(|(_, child)| child));
                pending.extend(inner.local.values().cloned());
            }
            Value::Union(branches) => pending.extend(branches),
            Value::Range { start, end, step } => {
                pending.push(*start);
                pending.push(*end);
                if let Some(step) = step {
                    pending.push(*step);
                }
            }
            _ => {}
        }
    }
    Some(cost)
}

/// D1/M1: digest of sealed frame **content** for force-memo / in_flight keys.
///
/// Each `Arc<ComboVal>` contributes its memoized `cycle_frame_digest` (Merkle
/// over nested Arc frames). Equal content ⇒ equal key across allocations
/// (SPEC_01 §2.4.1: not a memory address). Cost is once per unique Arc.
fn frames_content_digest(closure: &[std::sync::Arc<ComboVal>]) -> ContentHash {
    let mut h = sha2::Sha256::new();
    h.update(b"frames_content:v1:");
    for cv in closure {
        h.update(&cv.cycle_frame_digest().digest);
    }
    ContentHash::v1(h.finalize().to_vec())
}

/// Cycle / memo identity for a thunk under force. Frame component is content
/// (M1), not `Arc::as_ptr` — pointer keys missed re-entry of equal frames in
/// separate allocations (SPEC_01 §2.4.1).
fn thunk_cycle_id(
    expr: &Expr,
    closure: &[std::sync::Arc<ComboVal>],
    context: &Option<Box<Value>>,
    effect: EffectTag,
) -> ContentHash {
    let mut h = sha2::Sha256::new();
    h.update(b"thunk_cycle:v1:");
    h.update(expr.to_nlang(0).as_bytes());
    h.update(&frames_content_digest(closure).digest);
    match context {
        None => h.update(b"|#open"),
        Some(v) => {
            h.update(b"|ctx:");
            h.update(&v.content_hash().digest);
        }
    }
    h.update(&[effect.to_serial_byte()]);
    ContentHash::v1(h.finalize().to_vec())
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
        return Value::Atom(AtomKind::Tag("pure".to_string()), EffectTag::Pure, None);
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
        0 => Value::Atom(AtomKind::Tag("pure".to_string()), EffectTag::Pure, None),
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

/// Hard cap on **automatic** remote fetch sources (automatic_admission arc).
/// Derived from the measured ~5 s silent-source budget (3 × ≈15 s), not from
/// discovery's 8 or Kademlia's K=20. Manual `connect` remotes and local stores
/// do not consume these slots.
pub const AUTOMATIC_REMOTE_CAP: usize = 3;

/// One process-local automatic remote source, tied to an exact signed
/// advertisement. Not durable; reconstructed from eligible peer-directory
/// records on engine init.
#[derive(Debug, Clone)]
pub struct AutomaticRemote {
    /// `host:port` for [`Peer::Remote`] / `remote_fetch` (no `tcp://` prefix).
    pub addr: String,
    /// Exact signed advertisement identity that justified admission.
    pub ad_source: String,
}

/// How this receiver learned a verified signed advertisement.
///
/// Receiver-local observation only — not trust, correctness, ranking, or
/// address safety. Bound to the exact signed advertisement identity
/// (`ad_source`), not merely to `node_id` or `%hops`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationProvenance {
    /// Accepted a valid `#advertise` on a connection whose peer host was observed here.
    Direct,
    /// Learned the signed advertisement through a `#discover` response (relay assertions).
    Relayed,
    /// Absent, legacy, or cleared (e.g. copied workspace / owner mismatch).
    Unknown,
}

impl ObservationProvenance {
    /// Durable spelling (`direct` / `relayed` / `unknown`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Relayed => "relayed",
            Self::Unknown => "unknown",
        }
    }

    /// Parse a durable spelling; unknown tokens and absence map to [`Unknown`].
    pub fn parse(s: &str) -> Self {
        match s {
            "direct" => Self::Direct,
            "relayed" => Self::Relayed,
            _ => Self::Unknown,
        }
    }

    /// Merge rank: higher rank wins for the same exact advertisement.
    /// Direct > Relayed > Unknown. Equal rank allows same-class last-wins.
    fn rank(self) -> u8 {
        match self {
            Self::Direct => 2,
            Self::Relayed => 1,
            Self::Unknown => 0,
        }
    }
}

/// An accepted OODP `#advertise` record (wire advertise / discover index).
/// Written on verify; searched by `#discover`. Still **not** a fetch source
/// (discover_index §5 — consent arc later).
#[derive(Debug, Clone)]
pub struct PeerAdvert {
    pub node_id: String,
    pub public_key_hex: String,
    pub services: Vec<String>,
    /// Observed host + claimed listen port (`host:port`).
    pub addr: String,
    /// Host observed on the connection that carried `#advertise` (unsigned).
    pub observed_host: String,
    pub listen_port: u16,
    pub capacity: i64,
    /// Signed lattice/relay bound (`0..=15`). Never modified after accept.
    pub ttl: i64,
    /// Signed origin timestamp (unix seconds).
    pub ts: i64,
    /// Hops at which this record arrived (`0` for a direct `#advertise`).
    pub hops: i64,
    /// Verbatim n/ source of `%ad` as received (signature included). Relay emits
    /// this byte-for-byte — not a re-serialisation (discover_index §3.3).
    pub ad_source: String,
    pub received_at: std::time::SystemTime,
    pub received_at_unparseable: bool,
    /// Derived affiliation operator public key (64 hex), if a claim verified.
    /// **Never persisted** — rebuilt from `ad_source` (affiliation_claim / #3c-a).
    pub verified_operator_key: Option<String>,
    /// Receiver-local how-this-ad-was-learned (not on the wire / not signed).
    pub provenance: ObservationProvenance,
    /// Receiver-local monotonic admission order (seat_order / REAL_02 §4.2.6.3).
    /// Durable optional key; `0` means absent/legacy. Totals arrival order when
    /// `received_at` only has one-second resolution.
    pub admission_seq: u64,
    pub admission_seq_unparseable: bool,
}

/// Force-memo key (Stage 5): (expr CAID, frame CAID, context CAID | #open).
/// root_caid removed — invalidation is now per-coordinate (Route B, deps).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForceMemoKey {
    pub expr_caid: ContentHash,
    pub frame_caid: ContentHash,
    pub context_caid: Option<ContentHash>,
    /// Horizon strategy + fuel ceiling must partition the memo: evolve fills
    /// entries under default Blur/10k fuel; observe may re-force the same
    /// expr under `#strict` / low fuel. Without these, a hit returns a
    /// `#blur` under a strategy that should have minted ⊥ (D1 regression
    /// via denser force activity during evolve).
    pub strategy: ObservationStrategy,
    pub fuel_budget: u64,
}

/// Force-memo entry (Stage 5): cached value + coordinate dependencies.
#[derive(Debug, Clone)]
pub struct MemoEntry {
    pub value: Value,
    /// Semantic MBU consumed to produce `value`.  A cache is an implementation
    /// optimisation, not a second billing schedule: a hit must debit the same
    /// semantic work as a miss so cache warmth cannot move a blur horizon.
    pub mbu_cost: u64,
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

/// Standard-library roots this engine can resolve by content digest.
///
/// This is deliberately a table, rather than a current-root comparison: a
/// later engine adds a historical root as data and leaves root decoding alone.
#[derive(Debug, Clone, Default)]
pub struct StandardRootSet {
    by_digest: IndexMap<String, ComboVal>,
}

impl StandardRootSet {
    pub fn from_roots(roots: impl IntoIterator<Item = ComboVal>) -> Self {
        let mut by_digest = IndexMap::new();
        for root in roots {
            let digest = hex::encode(Value::Combo(root.clone()).content_hash().digest);
            by_digest.insert(digest, root);
        }
        Self { by_digest }
    }

    pub fn get(&self, digest: &str) -> Option<&ComboVal> {
        self.by_digest.get(digest)
    }

    pub fn contains(&self, digest: &str) -> bool {
        self.by_digest.contains_key(digest)
    }

    pub fn digests(&self) -> impl Iterator<Item = &str> {
        self.by_digest.keys().map(String::as_str)
    }
}

pub struct Ouroboros {
    pub store: ObjectStore,
    /// All standard-library roots this binary ships, indexed by their CAID
    /// digest. Historical compatibility is data in this table.
    pub standard_roots: StandardRootSet,
    pub base_dir: Option<PathBuf>,
    /// Owns the temp tree for [`Self::new_in_memory`]. Drop removes its `.oo/`.
    _ephemeral_root: Option<tempfile::TempDir>,
    pub unify_memo: RwLock<HashMap<(ContentHash, ContentHash), Value>>,
    pub force_memo: RwLock<HashMap<ForceMemoKey, MemoEntry>>,
    /// Reverse index: coord → memo keys that read this coord.
    pub force_memo_rev: RwLock<HashMap<String, HashSet<ForceMemoKey>>>,
    /// Monotonic count of successfully served force-memo entries. This is
    /// diagnostic state only: it never participates in fuel or CAID.
    force_memo_hit_count: AtomicU64,
    pub builtin_registry: HashMap<String, Arc<BuiltinFn>>,
    pub peers: RwLock<HashMap<String, Peer>>,
    /// Automatic remote fetch sources (admission class). Keyed by `node_id`,
    /// insertion-ordered (incumbent-first). Cap: [`AUTOMATIC_REMOTE_CAP`].
    /// Separate from manual [`Self::peers`]; does not persist.
    pub automatic_remotes: RwLock<IndexMap<String, AutomaticRemote>>,
    /// Wire peer directory (accepted `#advertise` records). Keyed by `node_id`.
    /// Shared with the Kademlia index (routing) — one store, two views.
    pub peer_adverts: RwLock<HashMap<String, PeerAdvert>>,
    /// Kademlia bucket index over `peer_adverts` (kademlia_table arc).
    pub routing: RwLock<crate::routing::RoutingIndex>,
    /// Durable peer-directory file state (line count for 2× compaction).
    pub peer_dir_state: RwLock<crate::peers::PeerDirectoryState>,
    /// Load report from `.oo/peers/directory` at init (serve prints once).
    pub peers_load_report: Option<crate::peers::LoadReport>,
    /// Lazy operator identity. `None` until a signature is needed (or
    /// `oo identity`). In-memory engines pre-fill an ephemeral key and never
    /// touch the operator path.
    identity_cell: RwLock<Option<crate::value::Identity>>,
    /// Lazy **node** identity (keypair for OODP `%from` / `%source`). Independent
    /// of the operator key (who authorises vs which machine answered). Minted
    /// only on first network use / `oo node id` — never on plain run/evolve/commit.
    node_identity_cell: RwLock<Option<crate::value::Identity>>,
    /// When true, `identity()` loads/mints at `OO_IDENTITY` or `~/.oo/identity`.
    /// When false (`new_in_memory`), only the ephemeral key is used.
    identity_persist: bool,
    pub refine_map: RwLock<HashMap<String, Vec<String>>>,
    pub gbb_registry: RwLock<HashMap<String, crate::ladd::GBB>>,
    pub architect_registry: RwLock<std::collections::HashSet<String>>,
    /// Affiliation trust roots from `.oo/discovery.n` (discovery_trust).
    /// Consumed by automatic admission as consent roots only.
    pub affiliation_roots: std::collections::BTreeSet<String>,
    /// SPEC_08 §6 capability lattice. Default NONE; set only via trusted
    /// channel (`set_privilege` / CLI `--privileged`/`--grant`). Never from
    /// in-program n/ code.
    pub privilege: crate::value::Privilege,
    /// Accumulated integrity incidents (條款四). Library is silent; CLI prints.
    pub integrity_log: RwLock<Vec<IntegrityIncident>>,
    /// Union of the effect tags `runPure` has actually DISCHARGED in this
    /// process. Read by `Universe::evolve` into `effect_pending` (SPEC_08
    /// §6.2). Not authorization — only "these tags were overridden".
    ///
    /// ACCEPTOR REPAIR: was an `AtomicBool`. A bool cannot answer the question
    /// commit has to ask — *is the capability now being re-presented the one
    /// this act required?* Measured on the delivered build, a discharge of
    /// `io` was authorised at commit by `--grant effect_override:nondet`.
    pub privileged_discharge_tags: std::sync::atomic::AtomicU8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Lte,
    Gte,
}

impl Ouroboros {
    /// Stage 5 (§5b): invalidate all memo entries that depend on any of the
    /// given coordinates. `"*"` entries are cleared on every call.
    pub fn invalidate_coords(&self, coords: &[String]) {
        if let Ok(mut rev) = self.force_memo_rev.write() {
            if let Ok(mut memo) = self.force_memo.write() {
                for coord in coords {
                    if let Some(keys) = rev.remove(coord) {
                        for k in keys {
                            memo.remove(&k);
                        }
                    }
                }
                if let Some(wildcard_keys) = rev.remove("*") {
                    for k in wildcard_keys {
                        memo.remove(&k);
                    }
                }
            }
        }
    }

    /// Stage 5 (§5b): full clear for coarse events (commit/load/refine).
    pub fn clear_force_memo(&self) {
        if let Ok(mut memo) = self.force_memo.write() {
            memo.clear();
        }
        if let Ok(mut rev) = self.force_memo_rev.write() {
            rev.clear();
        }
    }

    /// Number of force-memo entries that have been successfully served by
    /// this engine instance. The counter is intentionally not reset by
    /// invalidation: callers compare before/after snapshots to verify a hit
    /// without making cache warmth observable through the fuel horizon.
    pub fn force_memo_hit_count(&self) -> u64 {
        self.force_memo_hit_count.load(Ordering::Relaxed)
    }

    /// Stage 5 (§5a): record a coordinate dependency in the active collector.
    fn record_dep(&self, ctx: &mut EvalContext, coord: &str) {
        if let Some(ref mut deps) = ctx.dep_collector {
            deps.insert(coord.to_string());
        }
    }

    pub fn new_in_memory() -> Self {
        let ephemeral = crate::scratch::ephemeral_store_root().expect("ephemeral store root");
        let store = ObjectStore::init(ephemeral.path()).unwrap();
        let builtins = create_default_builtins();
        // Ephemeral identity only — never read/write the operator path.
        let identity = crate::value::Identity::new_random();
        // No self-appointment into architect_registry (universe_determinism).
        let mut engine = Self {
            store,
            standard_roots: StandardRootSet::default(),
            base_dir: None,
            _ephemeral_root: Some(ephemeral),
            unify_memo: RwLock::new(HashMap::new()),
            force_memo: RwLock::new(HashMap::new()),
            force_memo_rev: RwLock::new(HashMap::new()),
            force_memo_hit_count: AtomicU64::new(0),
            builtin_registry: builtins,
            peers: RwLock::new(HashMap::new()),
            automatic_remotes: RwLock::new(IndexMap::new()),
            peer_adverts: RwLock::new(HashMap::new()),
            routing: RwLock::new(crate::routing::RoutingIndex::new([0u8; 20])),
            peer_dir_state: RwLock::new(crate::peers::PeerDirectoryState::default()),
            peers_load_report: None,
            identity_cell: RwLock::new(Some(identity)),
            node_identity_cell: RwLock::new(None),
            identity_persist: false,
            refine_map: RwLock::new(HashMap::new()),
            gbb_registry: RwLock::new(HashMap::new()),
            architect_registry: RwLock::new(std::collections::HashSet::new()),
            affiliation_roots: std::collections::BTreeSet::new(),
            privilege: crate::value::Privilege::NONE,
            integrity_log: RwLock::new(Vec::new()),
            privileged_discharge_tags: std::sync::atomic::AtomicU8::new(0),
        };
        engine.standard_roots = engine.shipped_standard_roots();
        engine
    }

    pub fn init(base_dir: &std::path::Path) -> Result<Self> {
        let store = ObjectStore::init(base_dir)?;
        let builtins = create_default_builtins();
        // Lazy identity: do not mint on init (P5 — ordinary work must not
        // create ~/.oo/identity). Loaded on first signature / `oo identity`.
        // Assertion layer: load provisioned whitelist from .oo/architects.json.
        let architects = store
            .load_architects(base_dir)
            .unwrap_or_else(|_| std::collections::HashSet::new());
        // discovery_trust: load affiliation roots loudly (not fail-soft).
        let discovery = crate::discovery_config::DiscoveryConfig::load(base_dir)?;
        // Durable peer directory (advert_persistence): load signed records;
        // restore observations only when the file's owner matches this node.
        // Do not mint a node key here (node_identity P5).
        let this_node_id = match crate::value::Identity::node_key_path(base_dir) {
            Ok(path) if path.exists() => crate::value::Identity::load(&path)
                .ok()
                .map(|id| id.node_id_caid().to_string()),
            _ => None,
        };
        let (peer_adverts, routing, peer_dir_state, load_report) =
            crate::peers::load(base_dir, this_node_id.as_deref());
        // Prefill node identity when we already loaded it (no second mint).
        let node_identity_cell = match crate::value::Identity::node_key_path(base_dir) {
            Ok(path) if path.exists() => match crate::value::Identity::load(&path) {
                Ok(id) => RwLock::new(Some(id)),
                Err(_) => RwLock::new(None),
            },
            _ => RwLock::new(None),
        };
        let oo = Self {
            store,
            standard_roots: StandardRootSet::default(),
            base_dir: Some(base_dir.to_path_buf()),
            _ephemeral_root: None,
            unify_memo: RwLock::new(HashMap::new()),
            force_memo: RwLock::new(HashMap::new()),
            force_memo_rev: RwLock::new(HashMap::new()),
            force_memo_hit_count: AtomicU64::new(0),
            builtin_registry: builtins,
            peers: RwLock::new(HashMap::new()),
            automatic_remotes: RwLock::new(IndexMap::new()),
            peer_adverts: RwLock::new(peer_adverts),
            routing: RwLock::new(routing),
            peer_dir_state: RwLock::new(peer_dir_state),
            peers_load_report: if load_report.log_line.is_some() {
                Some(load_report)
            } else {
                None
            },
            identity_cell: RwLock::new(None),
            node_identity_cell,
            identity_persist: true,
            refine_map: RwLock::new(HashMap::new()),
            gbb_registry: RwLock::new(HashMap::new()),
            architect_registry: RwLock::new(architects),
            affiliation_roots: discovery.affiliation_roots,
            privilege: crate::value::Privilege::NONE,
            integrity_log: RwLock::new(Vec::new()),
            privileged_discharge_tags: std::sync::atomic::AtomicU8::new(0),
        };
        // ACCEPTANCE REPAIR (advert_persistence): the loader takes stored
        // records on trust because it runs before the engine exists. Now that
        // it does, check the signatures — a signed record that nobody checks
        // is an assertion wearing a signature.
        let mut oo = oo;
        oo.standard_roots = oo.shipped_standard_roots();
        let unverifiable = crate::peers::verify_loaded(&oo);
        if let Some(ref mut r) = oo.peers_load_report {
            r.records = r.records.saturating_sub(unverifiable);
            r.unverifiable = unverifiable;
            r.log_line = Some(format!(
                "OODP Peers: loaded {} records, skipped {} damaged, {} unverifiable",
                r.records, r.skipped, unverifiable
            ));
        }
        // Reconstruct automatic remote sources from eligible durable records
        // (no dial — first contact remains the fetch scan).
        oo.reconstruct_automatic_remotes();
        Ok(oo)
    }

    /// Note that a privileged discharge actually occurred, and which active
    /// tags it overrode. Callers must pass `active_part()`; a Pure argument is
    /// ignored, because `runPure` over an already-pure value overrides nothing
    /// and marking it would assert an intervention that never happened.
    pub fn note_privileged_discharge(&self, tags: crate::value::EffectTag) {
        let bits = tags.active_part().to_bits();
        if bits == 0 {
            return;
        }
        self.privileged_discharge_tags
            .fetch_or(bits, std::sync::atomic::Ordering::SeqCst);
    }

    /// Consume the accumulated discharge tags (`Pure` if none since last take).
    pub fn take_privileged_discharge(&self) -> crate::value::EffectTag {
        crate::value::EffectTag::from_bits(
            self.privileged_discharge_tags
                .swap(0, std::sync::atomic::Ordering::SeqCst),
        )
    }

    /// Operator identity for signing. Lazy-loads/mints at the operator path
    /// when `identity_persist`; otherwise returns the ephemeral in-memory key.
    ///
    /// **Only consumer of the private key in the engine after identity_persistence
    /// is `oo refine --sign` (via `authority::sign_refine`).** Language surface
    /// must not obtain it.
    pub fn identity(&self) -> Result<crate::value::Identity> {
        {
            let guard = self
                .identity_cell
                .read()
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if let Some(ref id) = *guard {
                return Ok(id.clone());
            }
        }
        if !self.identity_persist {
            // Should have been pre-filled; fall back to ephemeral.
            let id = crate::value::Identity::new_random();
            if let Ok(mut w) = self.identity_cell.write() {
                *w = Some(id.clone());
            }
            return Ok(id);
        }
        let path = crate::value::Identity::resolve_path()?;
        let id = crate::value::Identity::load_or_mint(&path)?;
        if let Ok(mut w) = self.identity_cell.write() {
            *w = Some(id.clone());
        }
        Ok(id)
    }

    /// Node keypair for this workspace. Lazy: first network use / `oo node id`.
    ///
    /// Independent of [`Self::identity`] (operator key). Path:
    /// `{OO_NODE_HOME|~/.oo}/nodes/<digest of workspace absolute path>`.
    pub fn node_identity(&self) -> Result<crate::value::Identity> {
        {
            let guard = self
                .node_identity_cell
                .read()
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if let Some(ref id) = *guard {
                return Ok(id.clone());
            }
        }
        let id = if !self.identity_persist || self.base_dir.is_none() {
            // In-memory / non-persisting engines: ephemeral node key, never on disk.
            crate::value::Identity::new_random()
        } else {
            let ws = self.base_dir.as_ref().unwrap();
            let path = crate::value::Identity::node_key_path(ws)?;
            crate::value::Identity::load_or_mint(&path)?
        };
        if let Ok(mut w) = self.node_identity_cell.write() {
            *w = Some(id.clone());
        }
        Ok(id)
    }

    /// Wire / DHT node id: CAID of the node public key (REAL_02 §4.1).
    pub fn node_id(&self) -> Result<ContentHash> {
        Ok(self.node_identity()?.node_id_caid())
    }

    /// Node id if already loaded or present on disk — **never mints**.
    /// Used by seat rebuild / verify paths that must not create identity.
    pub fn node_id_if_present(&self) -> Option<String> {
        if let Ok(cell) = self.node_identity_cell.read() {
            if let Some(ref id) = *cell {
                return Some(id.node_id_caid().to_string());
            }
        }
        if !self.identity_persist {
            return None;
        }
        let base = self.base_dir.as_ref()?;
        let path = crate::value::Identity::node_key_path(base).ok()?;
        if !path.exists() {
            return None;
        }
        crate::value::Identity::load(&path)
            .ok()
            .map(|id| id.node_id_caid().to_string())
    }

    /// Record an accepted OODP advertisement and update the Kademlia index.
    /// Appends to `.oo/peers/directory` when this engine has a workspace.
    /// Returns log lines (routing insert / full-drop + peers append/compact).
    ///
    /// For the **same** exact signed advertisement (`ad_source`), provenance
    /// merge precedence is Direct > Relayed > Unknown. A lower-ranked arrival
    /// does not replace the live record and is not durable-appended. A
    /// **different** signed advertisement for the same `node_id` replaces
    /// under the existing last-wins policy and keeps its own provenance.
    pub fn record_peer_advert(&self, mut advert: PeerAdvert) -> Vec<String> {
        let node_id = advert.node_id.clone();
        let pk_hex = advert.public_key_hex.clone();

        // Exact-ad provenance merge before insert / durable append.
        let accept = if let Ok(dir) = self.peer_adverts.read() {
            match dir.get(&node_id) {
                Some(existing) if existing.ad_source == advert.ad_source => {
                    advert.provenance.rank() >= existing.provenance.rank()
                }
                _ => true,
            }
        } else {
            true
        };
        if !accept {
            return Vec::new();
        }

        // Total arrival order for seat rebuild (additive durable field).
        if advert.admission_seq == 0 {
            if let Ok(mut st) = self.peer_dir_state.write() {
                advert.admission_seq = st.alloc_admission_seq();
            }
        }

        if let Ok(mut dir) = self.peer_adverts.write() {
            dir.insert(node_id.clone(), advert.clone());
        }
        // Ensure routing self_id matches this node before insert.
        let mut logs = Vec::new();
        if let Ok(self_caid) = self.node_id() {
            let sid = crate::routing::routing_id_from_caid(&self_caid);
            if let Ok(mut rt) = self.routing.write() {
                if rt.self_id_hex.is_empty() || rt.self_id_hex == hex::encode([0u8; 20]) {
                    *rt = crate::routing::RoutingIndex::new(sid);
                } else if rt.self_id() != sid {
                    // Should not happen mid-process; keep existing.
                }
                if let Some(rid) = crate::routing::routing_id_from_pubkey_hex(&pk_hex) {
                    logs = rt.insert(&node_id, rid);
                }
            }
            // Durable append (only for workspace engines).
            if let Some(ref base) = self.base_dir {
                let owner = self_caid.to_string();
                if let (Ok(live), Ok(mut st)) =
                    (self.peer_adverts.read(), self.peer_dir_state.write())
                {
                    let peer_logs = crate::peers::append(base, &owner, &advert, &live, &mut st);
                    logs.extend(peer_logs);
                }
            }
        }
        // Admission is process-local and never dials (lazy until fetch).
        self.consider_automatic_admission(&advert);
        logs
    }

    /// Whether this exact advertisement is eligible for automatic remote
    /// admission under the current roots and clock. Does not dial.
    pub fn eligible_for_automatic_admission(&self, advert: &PeerAdvert) -> bool {
        if advert.provenance != ObservationProvenance::Direct {
            return false;
        }
        if advert.observed_host.is_empty() || advert.listen_port == 0 {
            return false;
        }
        // Signature / identity / literal / TTL / freshness ladder.
        if crate::oodp::verify_stored_ad(self, &advert.ad_source).is_err() {
            return false;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let Some(op) = crate::oodp::verified_operator_of_ad_source(
            self,
            &advert.ad_source,
            &advert.node_id,
            now,
        ) else {
            return false;
        };
        self.affiliation_roots.contains(&op)
    }

    fn automatic_source_addr(advert: &PeerAdvert) -> String {
        if !advert.addr.is_empty() {
            advert.addr.clone()
        } else {
            format!("{}:{}", advert.observed_host, advert.listen_port)
        }
    }

    /// Insert, refresh, or drop the automatic source for `advert`'s `node_id`.
    /// Cap is automatic-only; overflow never evicts incumbents. No network I/O.
    pub fn consider_automatic_admission(&self, advert: &PeerAdvert) {
        let node_id = advert.node_id.clone();
        if !self.eligible_for_automatic_admission(advert) {
            if let Ok(mut auto) = self.automatic_remotes.write() {
                auto.shift_remove(&node_id);
            }
            return;
        }
        let entry = AutomaticRemote {
            addr: Self::automatic_source_addr(advert),
            ad_source: advert.ad_source.clone(),
        };
        let Ok(mut auto) = self.automatic_remotes.write() else {
            return;
        };
        if auto.contains_key(&node_id) {
            // Same node, still eligible: refresh address / exact-ad identity.
            auto.insert(node_id, entry);
            return;
        }
        if auto.len() >= AUTOMATIC_REMOTE_CAP {
            // Incumbent-first: free slots only; no eviction by capacity/freshness.
            return;
        }
        auto.insert(node_id, entry);
    }

    /// Rebuild automatic remotes from the live peer directory after load.
    /// Incumbent order is total arrival order (`admission_seq` / §4.2.6.3),
    /// not raw `node_id`. Does not dial.
    pub fn reconstruct_automatic_remotes(&self) {
        if let Ok(mut auto) = self.automatic_remotes.write() {
            auto.clear();
        }
        let mut sorted: Vec<PeerAdvert> = if let Ok(dir) = self.peer_adverts.read() {
            dir.values().cloned().collect()
        } else {
            return;
        };
        // Never mint: ordinary eval/run must not create node identity.
        let self_id = self.node_id_if_present();
        sorted.sort_by(|a, b| crate::peers::cmp_admission_order(a, b, self_id.as_deref()));
        for adv in &sorted {
            self.consider_automatic_admission(adv);
        }
    }

    /// Drop automatic sources that no longer match an eligible exact ad.
    /// Called before unnamed fetch scans. Does not backfill free slots.
    pub fn revalidate_automatic_remotes(&self) {
        let snapshot: Vec<(String, AutomaticRemote)> =
            if let Ok(auto) = self.automatic_remotes.read() {
                auto.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
            } else {
                return;
            };
        for (node_id, auto) in snapshot {
            let still = if let Ok(dir) = self.peer_adverts.read() {
                match dir.get(&node_id) {
                    Some(adv)
                        if adv.ad_source == auto.ad_source
                            && self.eligible_for_automatic_admission(adv) =>
                    {
                        true
                    }
                    _ => false,
                }
            } else {
                false
            };
            if !still {
                if let Ok(mut map) = self.automatic_remotes.write() {
                    map.shift_remove(&node_id);
                }
            }
        }
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
            Value::Combo(cv) => cv
                .get_field("%kind")
                .map(|k| {
                    let ks = self.force(k.clone(), ctx).to_string_plain();
                    ks.trim_start_matches('#') == "list"
                })
                .unwrap_or(false),
            _ => false,
        }
    }

    /// Ruling B: `/ %differential.1/.2/.3` are not plain identifiers.
    /// Rename in-place on a clone of the v0.22 root so the historical
    /// spelling stays loadable.
    fn rename_engine_differential_keys(root: &mut ComboVal) {
        let Some(Value::Combo(eng)) = root.system.get_mut("Engine") else {
            return;
        };
        for i in 1u8..=3 {
            let old = format!("%differential.{i}");
            if let Some(v) = eng.rules.shift_remove(&old) {
                eng.rules.insert(format!("differential_{i}"), v);
            }
        }
    }

    /// O64 / O65 / O66 on a clone of the quoted-names root. The v0.22
    /// builder and the quoted-names spelling stay loadable as history.
    fn apply_shelf_rulings(root: &mut ComboVal) {
        // O64: the module for type `str` is `~%Str`.
        if let Some(v) = root.system.shift_remove("String") {
            root.system.insert("Str".to_string(), v);
        }
        // O65: `/add` is not a top-level rule; it lives in `~%Math`.
        root.rules.shift_remove("add");
        // O66: do not synthesise an empty `~%Official` shell.
        root.system.shift_remove("Official");
    }

    /// The v0.22 standard-library root. Keep this builder after a future
    /// library change so that `shipped_standard_roots` can retain it as data.
    fn v0_22_standard_root() -> ComboVal {
        let mut fields = IndexMap::new();
        let add_morph = Value::Combo(ComboVal::new(
            IndexMap::from_iter(vec![
                (
                    "%morphism".to_string(),
                    Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                ),
                (
                    "%builtin".to_string(),
                    Value::Atom(AtomKind::Str("math.add".to_string()), EffectTag::Pure, None),
                ),
            ]),
            true,
            IndexMap::new(),
            EffectTag::Pure,
            vec![],
        ));
        fields.insert("/add".to_string(), add_morph.clone());

        let mut math_builtins = IndexMap::new();
        math_builtins.insert("/add".to_string(), add_morph);
        let math_morphisms = vec![
            ("/sub", "math.sub"),
            ("/mul", "math.mul"),
            ("/div", "math.div"),
            ("/rem", "math.rem"),
            ("/abs", "math.abs"),
            ("/bits", "math.bits"),
            ("/pow", "math.pow"),
            ("/sqrt", "math.sqrt"),
            ("/bitAnd", "math.bitAnd"),
            ("/bitOr", "math.bitOr"),
            ("/bitXor", "math.bitXor"),
            ("/bitNot", "math.bitNot"),
            ("/shl", "math.shl"),
            ("/shr", "math.shr"),
            ("/exp", "math.exp"),
            ("/ln", "math.ln"),
            ("/sin", "math.sin"),
            ("/cos", "math.cos"),
            ("/eml", "math.eml"),
            // Phase 19 (previously missing from module)
            ("/min", "math.min"),
            ("/max", "math.max"),
            ("/floor", "math.floor"),
            ("/ceil", "math.ceil"),
            ("/round", "math.round"),
            ("/clamp", "math.clamp"),
            // Phase 27
            ("/gcd", "math.gcd"),
            ("/lcm", "math.lcm"),
            ("/sign", "math.sign"),
            ("/log2", "math.log2"),
            ("/log10", "math.log10"),
            // Phase 35
            ("/factorial", "math.factorial"),
            ("/choose", "math.choose"),
            ("/is_prime", "math.is_prime"),
            ("/pow_mod", "math.pow_mod"),
            // Phase 45
            ("/atan2", "math.atan2"),
            ("/hypot", "math.hypot"),
            ("/sinh", "math.sinh"),
            ("/cosh", "math.cosh"),
            ("/tanh", "math.tanh"),
            ("/trunc", "math.trunc"),
            ("/fract", "math.fract"),
            ("/to_float", "math.to_float"),
            // Order wave W1 (SPEC_09 §3): numeric order predicates.
            ("/lt", "math.lt"),
            ("/lte", "math.lte"),
            ("/gt", "math.gt"),
            ("/gte", "math.gte"),
        ];
        for (n, b) in math_morphisms {
            math_builtins.insert(
                n.to_string(),
                Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        (
                            "%morphism".to_string(),
                            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                        ),
                        (
                            "%builtin".to_string(),
                            Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None),
                        ),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                )),
            );
        }
        math_builtins.insert(
            "/random".to_string(),
            Value::Combo(ComboVal::new(
                IndexMap::from_iter(vec![
                    (
                        "%morphism".to_string(),
                        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                    ),
                    (
                        "%builtin".to_string(),
                        Value::Atom(
                            AtomKind::Str("math.random".to_string()),
                            EffectTag::Pure,
                            None,
                        ),
                    ),
                ]),
                true,
                IndexMap::new(),
                EffectTag::NonDet,
                vec![],
            )),
        );
        math_builtins.insert(
            "one".to_string(),
            Value::Atom(AtomKind::Int(1.into()), EffectTag::Pure, None),
        );
        fields.insert(
            "~%Math".to_string(),
            Value::Combo(ComboVal::new(
                math_builtins,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        let mut cond_fields = IndexMap::new();
        let cond_morphisms = vec![
            ("/if", "cond.if"),
            ("/cond", "cond.cond"),
            ("/match", "cond.match"),
        ];
        for (n, b) in cond_morphisms {
            cond_fields.insert(
                n.to_string(),
                Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        (
                            "%morphism".to_string(),
                            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                        ),
                        (
                            "%builtin".to_string(),
                            Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None),
                        ),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                )),
            );
        }
        fields.insert(
            "~%Cond".to_string(),
            Value::Combo(ComboVal::new(
                cond_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        let mut list_fields = IndexMap::new();
        let list_morphisms = vec![
            ("/map", "list.map"),
            ("/filter", "list.filter"),
            ("/fold", "list.fold"),
            ("/len", "list.len"),
            ("/concat", "list.concat"),
            ("/at", "list.at"),
            ("/sort", "list.sort"),
            ("/reverse", "list.reverse"),
            ("/slice", "list.slice"),
            ("/zip", "list.zip"),
            // Phase 17
            ("/flat_map", "list.flat_map"),
            // Phase 18
            ("/any", "list.any"),
            ("/all", "list.all"),
            ("/find", "list.find"),
            ("/head", "list.head"),
            ("/tail", "list.tail"),
            ("/take", "list.take"),
            ("/drop", "list.drop"),
            // Phase 19
            ("/count", "list.count"),
            ("/zip_with", "list.zip_with"),
            // Phase 22
            ("/partition", "list.partition"),
            ("/flatten", "list.flatten"),
            ("/sum", "list.sum"),
            ("/min_by", "list.min_by"),
            ("/max_by", "list.max_by"),
            // Phase 25
            ("/unique", "list.unique"),
            ("/range", "list.range"),
            ("/reduce", "list.reduce"),
            // Phase 28
            ("/group_by", "list.group_by"),
            ("/chunk", "list.chunk"),
            ("/window", "list.window"),
            // Phase 35
            ("/enumerate", "list.enumerate"),
            ("/sort_by", "list.sort_by"),
            ("/dedup", "list.dedup"),
            ("/intersperse", "list.intersperse"),
            // Phase 45
            ("/scan", "list.scan"),
            ("/take_while", "list.take_while"),
            ("/drop_while", "list.drop_while"),
            ("/product", "list.product"),
            ("/transpose", "list.transpose"),
        ];
        for (n, b) in list_morphisms {
            list_fields.insert(
                n.to_string(),
                Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        (
                            "%morphism".to_string(),
                            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                        ),
                        (
                            "%builtin".to_string(),
                            Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None),
                        ),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                )),
            );
        }
        fields.insert(
            "~%List".to_string(),
            Value::Combo(ComboVal::new(
                list_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        let mut string_fields = IndexMap::new();
        let string_morphisms = vec![
            ("/concat", "str.concat"),
            ("/split", "str.split"),
            ("/join", "str.join"),
            ("/trim", "str.trim"),
            ("/len", "str.len"),
            ("/replace", "str.replace"),
            ("/to_lower", "str.to_lower"),
            ("/to_upper", "str.to_upper"),
            ("/starts_with", "str.starts_with"),
            ("/ends_with", "str.ends_with"),
            ("/contains", "str.contains"),
            // Phase 19
            ("/parse_int", "str.parse_int"),
            ("/from_int", "str.from_int"),
            ("/repeat", "str.repeat"),
            // Phase 21
            ("/format", "str.format"),
            // Phase 25
            ("/char_at", "str.char_at"),
            ("/chars", "str.chars"),
            // Phase 27
            ("/index_of", "str.index_of"),
            ("/pad_left", "str.pad_left"),
            ("/pad_right", "str.pad_right"),
            ("/trim_start", "str.trim_start"),
            ("/trim_end", "str.trim_end"),
            // Phase 32
            ("/reverse", "str.reverse"),
            ("/count", "str.count"),
            ("/slice", "str.slice"),
            ("/is_empty", "str.is_empty"),
            ("/parse_float", "str.parse_float"),
            ("/lines", "str.lines"),
            // Phase 45
            ("/encode_uri", "str.encode_uri"),
            ("/decode_uri", "str.decode_uri"),
            ("/levenshtein", "str.levenshtein"),
            ("/word_count", "str.word_count"),
            ("/title_case", "str.title_case"),
        ];
        for (n, b) in string_morphisms {
            string_fields.insert(
                n.to_string(),
                Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        (
                            "%morphism".to_string(),
                            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                        ),
                        (
                            "%builtin".to_string(),
                            Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None),
                        ),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                )),
            );
        }
        fields.insert(
            "~%String".to_string(),
            Value::Combo(ComboVal::new(
                string_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        let mut time_fields = IndexMap::new();
        time_fields.insert(
            "/now".to_string(),
            Value::Combo(ComboVal::new(
                IndexMap::from_iter(vec![
                    (
                        "%morphism".to_string(),
                        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                    ),
                    (
                        "%builtin".to_string(),
                        Value::Atom(AtomKind::Str("time.now".to_string()), EffectTag::Pure, None),
                    ),
                ]),
                true,
                IndexMap::new(),
                EffectTag::IO,
                vec![],
            )),
        );
        let time_morphisms = vec![
            ("/format", "time.format"),
            ("/diff", "time.diff"),
            ("/add_ms", "time.add_ms"),
            // Phase 45
            ("/parse", "time.parse"),
            ("/to_iso8601", "time.to_iso8601"),
            ("/add_days", "time.add_days"),
            ("/add_hours", "time.add_hours"),
            ("/weekday", "time.weekday"),
        ];
        for (n, b) in time_morphisms {
            time_fields.insert(
                n.to_string(),
                Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        (
                            "%morphism".to_string(),
                            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                        ),
                        (
                            "%builtin".to_string(),
                            Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None),
                        ),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                )),
            );
        }
        fields.insert(
            "~%Time".to_string(),
            Value::Combo(ComboVal::new(
                time_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        let mut bytes_fields = IndexMap::new();
        let bytes_morphisms = vec![
            ("/from_str", "bytes.from_str"),
            ("/to_str", "bytes.to_str"),
            ("/len", "bytes.len"),
            ("/at", "bytes.at"),
            ("/concat", "bytes.concat"),
            ("/slice", "bytes.slice"),
            ("/to_hex", "bytes.to_hex"),
            ("/from_hex", "bytes.from_hex"),
            // Phase 32
            ("/sha256", "bytes.sha256"),
            ("/base64_encode", "bytes.base64_encode"),
            ("/base64_decode", "bytes.base64_decode"),
            ("/hmac_sha256", "bytes.hmac_sha256"),
        ];
        for (n, b) in bytes_morphisms {
            bytes_fields.insert(
                n.to_string(),
                Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        (
                            "%morphism".to_string(),
                            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                        ),
                        (
                            "%builtin".to_string(),
                            Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None),
                        ),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                )),
            );
        }
        fields.insert(
            "~%Bytes".to_string(),
            Value::Combo(ComboVal::new(
                bytes_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        let mut regex_fields = IndexMap::new();
        let regex_morphisms = vec![
            ("/match", "regex.match"),
            ("/find", "regex.find"),
            ("/replace", "regex.replace"),
            ("/split", "regex.split"),
        ];
        for (n, b) in regex_morphisms {
            regex_fields.insert(
                n.to_string(),
                Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        (
                            "%morphism".to_string(),
                            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                        ),
                        (
                            "%builtin".to_string(),
                            Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None),
                        ),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                )),
            );
        }
        fields.insert(
            "~%Regex".to_string(),
            Value::Combo(ComboVal::new(
                regex_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        let mut json_fields = IndexMap::new();
        let json_morphisms = vec![
            ("/parse", "json.parse"),
            ("/stringify", "json.stringify"),
            ("/get", "json.get"),
            ("/keys", "json.keys"),
        ];
        for (n, b) in json_morphisms {
            json_fields.insert(
                n.to_string(),
                Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        (
                            "%morphism".to_string(),
                            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                        ),
                        (
                            "%builtin".to_string(),
                            Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None),
                        ),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                )),
            );
        }
        fields.insert(
            "~%Json".to_string(),
            Value::Combo(ComboVal::new(
                json_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        let mut io_fields = IndexMap::new();
        let io_morphisms = vec![
            ("/read_file", "io.read_file"),
            ("/write_file", "io.write_file"),
            ("/exists", "io.exists"),
            ("/append_file", "io.append_file"),
        ];
        for (n, b) in io_morphisms {
            io_fields.insert(
                n.to_string(),
                Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        (
                            "%morphism".to_string(),
                            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                        ),
                        (
                            "%builtin".to_string(),
                            Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None),
                        ),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::IO,
                    vec![],
                )),
            );
        }
        fields.insert(
            "~%Io".to_string(),
            Value::Combo(ComboVal::new(
                io_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        let mut env_fields = IndexMap::new();
        let env_morphisms = vec![
            ("/get", "env.get"),
            ("/args", "env.args"),
            ("/cwd", "env.cwd"),
        ];
        for (n, b) in env_morphisms {
            env_fields.insert(
                n.to_string(),
                Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        (
                            "%morphism".to_string(),
                            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                        ),
                        (
                            "%builtin".to_string(),
                            Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None),
                        ),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::IO,
                    vec![],
                )),
            );
        }
        fields.insert(
            "~%Env".to_string(),
            Value::Combo(ComboVal::new(
                env_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        let mut process_fields = IndexMap::new();
        let process_morphisms = vec![("/exit", "process.exit"), ("/pid", "process.pid")];
        for (n, b) in process_morphisms {
            process_fields.insert(
                n.to_string(),
                Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        (
                            "%morphism".to_string(),
                            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                        ),
                        (
                            "%builtin".to_string(),
                            Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None),
                        ),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::IO,
                    vec![],
                )),
            );
        }
        fields.insert(
            "~%Process".to_string(),
            Value::Combo(ComboVal::new(
                process_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        let mut path_fields = IndexMap::new();
        let path_morphisms = vec![
            ("/join", "path.join"),
            ("/dirname", "path.dirname"),
            ("/basename", "path.basename"),
            ("/extension", "path.extension"),
            ("/is_absolute", "path.is_absolute"),
        ];
        for (n, b) in path_morphisms {
            path_fields.insert(
                n.to_string(),
                Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        (
                            "%morphism".to_string(),
                            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                        ),
                        (
                            "%builtin".to_string(),
                            Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None),
                        ),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                )),
            );
        }
        fields.insert(
            "~%Path".to_string(),
            Value::Combo(ComboVal::new(
                path_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        // ~%Query module
        let mut query_fields = IndexMap::new();
        let qmorph = |name: &str, id: &str, eff: EffectTag| -> Value {
            let mut f = IndexMap::new();
            f.insert(
                "%morphism".to_string(),
                Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
            );
            f.insert(
                "%builtin".to_string(),
                Value::Atom(AtomKind::Str(id.to_string()), EffectTag::Pure, None),
            );
            f.insert(
                "%kind".to_string(),
                Value::Atom(AtomKind::Tag("logic".to_string()), EffectTag::Pure, None),
            );
            Value::Combo(ComboVal::new(f, true, IndexMap::new(), eff, vec![]))
        };
        query_fields.insert(
            "/select".to_string(),
            qmorph("/select", "query.select", EffectTag::Pure),
        );
        query_fields.insert(
            "/where".to_string(),
            qmorph("/where", "query.where", EffectTag::IO),
        );
        query_fields.insert(
            "/pluck".to_string(),
            qmorph("/pluck", "query.pluck", EffectTag::Pure),
        );
        query_fields.insert(
            "/deep_merge".to_string(),
            qmorph("/deep_merge", "query.deep_merge", EffectTag::Pure),
        );
        let query_module = Value::Combo(ComboVal::new(
            query_fields,
            true,
            IndexMap::new(),
            EffectTag::Pure,
            vec![],
        ));
        fields.insert("~%Query".to_string(), query_module);

        // ~%Diff module
        let mut diff_fields = IndexMap::new();
        let dmorph = |name: &str, id: &str, eff: EffectTag| -> Value {
            let mut f = IndexMap::new();
            f.insert(
                "%morphism".to_string(),
                Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
            );
            f.insert(
                "%builtin".to_string(),
                Value::Atom(AtomKind::Str(id.to_string()), EffectTag::Pure, None),
            );
            f.insert(
                "%kind".to_string(),
                Value::Atom(AtomKind::Tag("logic".to_string()), EffectTag::Pure, None),
            );
            Value::Combo(ComboVal::new(f, true, IndexMap::new(), eff, vec![]))
        };
        diff_fields.insert(
            "/diff".to_string(),
            dmorph("/diff", "diff.diff", EffectTag::Pure),
        );
        diff_fields.insert(
            "/patch".to_string(),
            dmorph("/patch", "diff.patch", EffectTag::Pure),
        );
        diff_fields.insert(
            "/is_compatible".to_string(),
            dmorph("/is_compatible", "diff.is_compatible", EffectTag::Pure),
        );
        let diff_module = Value::Combo(ComboVal::new(
            diff_fields,
            true,
            IndexMap::new(),
            EffectTag::Pure,
            vec![],
        ));
        fields.insert("~%Diff".to_string(), diff_module);

        // ~%Set module
        let mut set_fields = IndexMap::new();
        let smorph = |name: &str, id: &str, eff: EffectTag| -> Value {
            let mut f = IndexMap::new();
            f.insert(
                "%morphism".to_string(),
                Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
            );
            f.insert(
                "%builtin".to_string(),
                Value::Atom(AtomKind::Str(id.to_string()), EffectTag::Pure, None),
            );
            f.insert(
                "%kind".to_string(),
                Value::Atom(AtomKind::Tag("logic".to_string()), EffectTag::Pure, None),
            );
            Value::Combo(ComboVal::new(f, true, IndexMap::new(), eff, vec![]))
        };
        set_fields.insert(
            "/from_list".to_string(),
            smorph("/from_list", "set.from_list", EffectTag::Pure),
        );
        set_fields.insert(
            "/union".to_string(),
            smorph("/union", "set.union", EffectTag::Pure),
        );
        set_fields.insert(
            "/intersection".to_string(),
            smorph("/intersection", "set.intersection", EffectTag::Pure),
        );
        set_fields.insert(
            "/difference".to_string(),
            smorph("/difference", "set.difference", EffectTag::Pure),
        );
        set_fields.insert(
            "/is_subset".to_string(),
            smorph("/is_subset", "set.is_subset", EffectTag::Pure),
        );
        set_fields.insert(
            "/is_superset".to_string(),
            smorph("/is_superset", "set.is_superset", EffectTag::Pure),
        );
        set_fields.insert(
            "/is_disjoint".to_string(),
            smorph("/is_disjoint", "set.is_disjoint", EffectTag::Pure),
        );
        set_fields.insert(
            "/contains".to_string(),
            smorph("/contains", "set.contains", EffectTag::Pure),
        );
        let set_module = Value::Combo(ComboVal::new(
            set_fields,
            true,
            IndexMap::new(),
            EffectTag::Pure,
            vec![],
        ));
        fields.insert("~%Set".to_string(), set_module);

        // ~%Stat module
        let mut stat_fields = IndexMap::new();
        stat_fields.insert(
            "/mean".to_string(),
            smorph("/mean", "stat.mean", EffectTag::Pure),
        );
        stat_fields.insert(
            "/variance".to_string(),
            smorph("/variance", "stat.variance", EffectTag::Pure),
        );
        stat_fields.insert(
            "/std_dev".to_string(),
            smorph("/std_dev", "stat.std_dev", EffectTag::Pure),
        );
        stat_fields.insert(
            "/median".to_string(),
            smorph("/median", "stat.median", EffectTag::Pure),
        );
        stat_fields.insert(
            "/percentile".to_string(),
            smorph("/percentile", "stat.percentile", EffectTag::Pure),
        );
        stat_fields.insert(
            "/histogram".to_string(),
            smorph("/histogram", "stat.histogram", EffectTag::Pure),
        );
        let stat_module = Value::Combo(ComboVal::new(
            stat_fields,
            true,
            IndexMap::new(),
            EffectTag::Pure,
            vec![],
        ));
        fields.insert("~%Stat".to_string(), stat_module);

        // ~%Csv module
        let mut csv_fields = IndexMap::new();
        csv_fields.insert(
            "/parse".to_string(),
            smorph("/parse", "csv.parse", EffectTag::Pure),
        );
        csv_fields.insert(
            "/parse_with_headers".to_string(),
            smorph(
                "/parse_with_headers",
                "csv.parse_with_headers",
                EffectTag::Pure,
            ),
        );
        csv_fields.insert(
            "/stringify".to_string(),
            smorph("/stringify", "csv.stringify", EffectTag::Pure),
        );
        csv_fields.insert(
            "/read_csv".to_string(),
            smorph("/read_csv", "csv.read_csv", EffectTag::IO),
        );
        let csv_module = Value::Combo(ComboVal::new(
            csv_fields,
            true,
            IndexMap::new(),
            EffectTag::Pure,
            vec![],
        ));
        fields.insert("~%Csv".to_string(), csv_module);

        // ~%Url module
        let mut url_fields = IndexMap::new();
        url_fields.insert(
            "/parse".to_string(),
            smorph("/parse", "url.parse", EffectTag::Pure),
        );
        url_fields.insert(
            "/encode".to_string(),
            smorph("/encode", "url.encode", EffectTag::Pure),
        );
        url_fields.insert(
            "/decode".to_string(),
            smorph("/decode", "url.decode", EffectTag::Pure),
        );
        url_fields.insert(
            "/join".to_string(),
            smorph("/join", "url.join", EffectTag::Pure),
        );
        url_fields.insert(
            "/query_params".to_string(),
            smorph("/query_params", "url.query_params", EffectTag::Pure),
        );
        let url_module = Value::Combo(ComboVal::new(
            url_fields,
            true,
            IndexMap::new(),
            EffectTag::Pure,
            vec![],
        ));
        fields.insert("~%Url".to_string(), url_module);

        // ~%Toml module
        let mut toml_fields = IndexMap::new();
        toml_fields.insert(
            "/parse".to_string(),
            smorph("/parse", "toml.parse", EffectTag::Pure),
        );
        toml_fields.insert(
            "/stringify".to_string(),
            smorph("/stringify", "toml.stringify", EffectTag::Pure),
        );
        let toml_module = Value::Combo(ComboVal::new(
            toml_fields,
            true,
            IndexMap::new(),
            EffectTag::Pure,
            vec![],
        ));
        fields.insert("~%Toml".to_string(), toml_module);

        let mut disc_fields = IndexMap::new();
        let disc_morphisms = vec![
            ("/connect", "disc.connect"),
            ("/fetch", "disc.fetch"),
            ("/identify", "disc.identify"),
            ("/identify_and_store", "engine.save"),
            ("/advertise", "disc.advertise"),
            ("/find", "disc.find"),
        ];
        for (n, b) in disc_morphisms {
            disc_fields.insert(
                n.to_string(),
                Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        (
                            "%morphism".to_string(),
                            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                        ),
                        (
                            "%builtin".to_string(),
                            Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None),
                        ),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::IO,
                    vec![],
                )),
            );
        }
        fields.insert(
            "~%Discovery".to_string(),
            Value::Combo(ComboVal::new(
                disc_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        // ~%Effect./runPure — SPEC_08 §4.3 / §6 privileged discharge.
        let mut effect_fields = IndexMap::new();
        effect_fields.insert(
            "/runPure".to_string(),
            Value::Combo(ComboVal::new(
                IndexMap::from_iter(vec![
                    (
                        "%morphism".to_string(),
                        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
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
            ("/keys", "refl.keys"),
            ("/has", "refl.has"),
            ("/is_cocoon", "refl.is_cocoon"),
            ("/type_of", "refl.type_of"),
            ("/is_blur", "refl.is_blur"),
            ("/is_bottom", "refl.is_bottom"),
            ("/is_some", "refl.is_some"),
            ("/is_none", "refl.is_none"),
            ("/is_ok", "refl.is_ok"),
            ("/is_err", "refl.is_err"),
            ("/to_str", "refl.to_str"),
            ("/bottom_cause", "refl.bottom_cause"),
            ("/get", "refl.get"),
            ("/set", "refl.set"),
            ("/delete", "refl.delete"),
            ("/values", "refl.values"),
            ("/entries", "refl.entries"),
        ];
        for (n, b) in refl_morphisms {
            refl_fields.insert(
                n.to_string(),
                Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        (
                            "%morphism".to_string(),
                            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                        ),
                        (
                            "%builtin".to_string(),
                            Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None),
                        ),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                )),
            );
        }
        fields.insert(
            "~%Reflection".to_string(),
            Value::Combo(ComboVal::new(
                refl_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        let mut complex_fields = IndexMap::new();
        let complex_morphisms = vec![
            ("/conj", "complex.conj"),
            ("/phase", "complex.phase"),
            ("/real", "complex.real"),
            ("/imag", "complex.imag"),
        ];
        for (n, b) in complex_morphisms {
            complex_fields.insert(
                n.to_string(),
                Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        (
                            "%morphism".to_string(),
                            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                        ),
                        (
                            "%builtin".to_string(),
                            Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None),
                        ),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                )),
            );
        }
        fields.insert(
            "~%Complex".to_string(),
            Value::Combo(ComboVal::new(
                complex_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        // @option: @Some { %val: _ } | #none  (SPEC_09 §2.7)
        let mut option_fields = IndexMap::new();
        option_fields.insert(
            "%kind".to_string(),
            Value::Atom(AtomKind::Tag("type".to_string()), EffectTag::Pure, None),
        );
        option_fields.insert(
            "%name".to_string(),
            Value::Atom(AtomKind::Str("option".to_string()), EffectTag::Pure, None),
        );
        option_fields.insert(
            "%some".to_string(),
            Value::Combo(ComboVal::new(
                IndexMap::from_iter(vec![("%val".to_string(), Value::Top)]),
                false,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );
        option_fields.insert(
            "%none".to_string(),
            Value::Atom(AtomKind::Tag("none".to_string()), EffectTag::Pure, None),
        );
        option_fields.insert(
            "%fmap".to_string(),
            Value::Combo(ComboVal::new(
                IndexMap::from_iter(vec![
                    (
                        "%morphism".to_string(),
                        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                    ),
                    (
                        "%builtin".to_string(),
                        Value::Atom(
                            AtomKind::Str("option.map".to_string()),
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
        let opt_morphisms = vec![
            ("/and_then", "option.and_then"),
            ("/or", "option.or"),
            ("/unwrap_or", "option.unwrap_or"),
            ("/filter", "option.filter"),
            ("/expect", "option.expect"),
            ("/zip", "option.zip"),
            ("/flatten", "option.flatten"),
        ];
        for (n, b) in opt_morphisms {
            option_fields.insert(
                n.to_string(),
                Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        (
                            "%morphism".to_string(),
                            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                        ),
                        (
                            "%builtin".to_string(),
                            Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None),
                        ),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                )),
            );
        }
        fields.insert(
            "@option".to_string(),
            Value::Combo(ComboVal::new(
                option_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        // @result: @Ok { %val: _ } | @Err { %cause: _ }  (SPEC_09 §2.8)
        let mut result_fields = IndexMap::new();
        result_fields.insert(
            "%kind".to_string(),
            Value::Atom(AtomKind::Tag("type".to_string()), EffectTag::Pure, None),
        );
        result_fields.insert(
            "%name".to_string(),
            Value::Atom(AtomKind::Str("result".to_string()), EffectTag::Pure, None),
        );
        result_fields.insert(
            "%ok".to_string(),
            Value::Combo(ComboVal::new(
                IndexMap::from_iter(vec![("%val".to_string(), Value::Top)]),
                false,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );
        result_fields.insert(
            "%err".to_string(),
            Value::Combo(ComboVal::new(
                IndexMap::from_iter(vec![("%cause".to_string(), Value::Top)]),
                false,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );
        result_fields.insert(
            "%fmap".to_string(),
            Value::Combo(ComboVal::new(
                IndexMap::from_iter(vec![
                    (
                        "%morphism".to_string(),
                        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                    ),
                    (
                        "%builtin".to_string(),
                        Value::Atom(
                            AtomKind::Str("result.map".to_string()),
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
        result_fields.insert(
            "%map_err".to_string(),
            Value::Combo(ComboVal::new(
                IndexMap::from_iter(vec![
                    (
                        "%morphism".to_string(),
                        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                    ),
                    (
                        "%builtin".to_string(),
                        Value::Atom(
                            AtomKind::Str("result.map_err".to_string()),
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
        let res_morphisms = vec![
            ("/and_then", "result.and_then"),
            ("/unwrap", "result.unwrap"),
            ("/expect", "result.expect"),
            ("/and", "result.and"),
            ("/or", "result.or"),
            ("/flatten", "result.flatten"),
        ];
        for (n, b) in res_morphisms {
            result_fields.insert(
                n.to_string(),
                Value::Combo(ComboVal::new(
                    IndexMap::from_iter(vec![
                        (
                            "%morphism".to_string(),
                            Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                        ),
                        (
                            "%builtin".to_string(),
                            Value::Atom(AtomKind::Str(b.to_string()), EffectTag::Pure, None),
                        ),
                    ]),
                    true,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                )),
            );
        }
        fields.insert(
            "@result".to_string(),
            Value::Combo(ComboVal::new(
                result_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        // @list: Combo with %kind: #list  (SPEC_09 §2.x)
        let mut list_type_fields = IndexMap::new();
        list_type_fields.insert(
            "%kind".to_string(),
            Value::Atom(AtomKind::Tag("type".to_string()), EffectTag::Pure, None),
        );
        list_type_fields.insert(
            "%name".to_string(),
            Value::Atom(AtomKind::Str("list".to_string()), EffectTag::Pure, None),
        );
        list_type_fields.insert(
            "%fmap".to_string(),
            Value::Combo(ComboVal::new(
                IndexMap::from_iter(vec![
                    (
                        "%morphism".to_string(),
                        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                    ),
                    (
                        "%builtin".to_string(),
                        Value::Atom(AtomKind::Str("list.map".to_string()), EffectTag::Pure, None),
                    ),
                ]),
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );
        fields.insert(
            "@list".to_string(),
            Value::Combo(ComboVal::new(
                list_type_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        // ~%Engine: observe, save, /%differential.{1,2,3}
        // (historical spelling; current root renames these — see
        // rename_engine_differential_keys)
        fn engine_morph(name: &str, builtin: &str, effect: EffectTag) -> Value {
            Value::Combo(ComboVal::new(
                IndexMap::from_iter(vec![
                    (
                        "%morphism".to_string(),
                        Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::Pure, None),
                    ),
                    (
                        "%builtin".to_string(),
                        Value::Atom(AtomKind::Str(builtin.to_string()), EffectTag::Pure, None),
                    ),
                ]),
                true,
                IndexMap::new(),
                effect,
                vec![],
            ))
        }
        let mut engine_fields = IndexMap::new();
        engine_fields.insert(
            "/observe".to_string(),
            engine_morph("/observe", "engine.observe", EffectTag::IO),
        );
        engine_fields.insert(
            "/save".to_string(),
            engine_morph("/save", "engine.save", EffectTag::IO),
        );
        for i in 1u8..=3 {
            engine_fields.insert(
                format!("/%differential.{}", i),
                engine_morph(
                    &format!("/%differential.{}", i),
                    "engine.differential",
                    EffectTag::Pure,
                ),
            );
        }
        engine_fields.insert(
            "/project_down".to_string(),
            engine_morph("/project_down", "engine.project_down", EffectTag::State),
        );
        engine_fields.insert(
            "/project_up".to_string(),
            engine_morph("/project_up", "engine.project_up", EffectTag::State),
        );
        engine_fields.insert(
            "/set_strategy".to_string(),
            engine_morph("/set_strategy", "engine.set_strategy", EffectTag::State),
        );
        engine_fields.insert(
            "/check_oml".to_string(),
            engine_morph("/check_oml", "engine.check_oml", EffectTag::Pure),
        );
        engine_fields.insert(
            "/equivalence_map".to_string(),
            engine_morph(
                "/equivalence_map",
                "engine.equivalence_map",
                EffectTag::State,
            ),
        );
        engine_fields.insert(
            "/resolve".to_string(),
            engine_morph("/resolve", "engine.resolve", EffectTag::State),
        );
        let mut state_inner = IndexMap::new();
        state_inner.insert(
            "differential".to_string(),
            Value::Atom(
                AtomKind::Tag("d1_converging".to_string()),
                EffectTag::Pure,
                None,
            ),
        );
        // G-config: ~%Engine.state.strategy was a dead display (always #blur,
        // never tracked ctx overrides). Normative strategy home = ~%Config;
        // runtime override = /set_strategy. Removed the lying field.
        engine_fields.insert(
            "state".to_string(),
            Value::Combo(ComboVal::new(
                state_inner,
                false,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );
        fields.insert(
            "~%Engine".to_string(),
            Value::Combo(ComboVal::new(
                engine_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        // ~%Official: empty closed combo (honest cold-start). No architects
        // field (universe_determinism), no /sign_refine (identity_persistence —
        // language must not own the private key; only `oo refine --sign` signs).
        // /add_architect already retired (store_boundary). Observing
        // ~%Official.architects → #missing_key.
        fields.insert(
            "~%Official".to_string(),
            Value::Combo(ComboVal::new(
                IndexMap::new(),
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        // ~%Config: genesis defaults (SPEC_08 §3.1 / SPEC_09 §6).
        // Bare field names on the data axis — path-observable as ~%Config.fuel.
        // No %-meta fallback (category error: % is node metadata, not config).
        let mut config_fields = IndexMap::new();
        config_fields.insert(
            "fuel".to_string(),
            Value::Atom(AtomKind::Int(10000i64.into()), EffectTag::Pure, None),
        );
        config_fields.insert(
            "max_branches".to_string(),
            Value::Atom(AtomKind::Int(64i64.into()), EffectTag::Pure, None),
        );
        config_fields.insert(
            "max_unification_depth".to_string(),
            Value::Atom(AtomKind::Int(256i64.into()), EffectTag::Pure, None),
        );
        config_fields.insert(
            "max_lifting_depth".to_string(),
            Value::Atom(AtomKind::Int(32i64.into()), EffectTag::Pure, None),
        );
        config_fields.insert(
            "max_pattern_nodes".to_string(),
            Value::Atom(AtomKind::Int(1024i64.into()), EffectTag::Pure, None),
        );
        // O41: genesis timeout is `#_` (TagEnd / order supremum — unbound).
        // Wall-clock only when the operator stages a finite non-negative Int.
        config_fields.insert(
            "timeout".to_string(),
            Value::Atom(AtomKind::TagEnd, EffectTag::Pure, None),
        );
        config_fields.insert(
            "strategy".to_string(),
            Value::Atom(AtomKind::Tag("blur".to_string()), EffectTag::Pure, None),
        );
        fields.insert(
            "~%Config".to_string(),
            Value::Combo(ComboVal::new(
                config_fields,
                true,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );

        ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![])
    }

    /// The standard root used for new universes written by this engine.
    pub fn root_with_system(&self) -> ComboVal {
        Self::as_shipped(Self::current_standard_root())
    }

    /// Quoted-names era library: v0.22 plus the three arity-overload keys
    /// renamed to plain identifiers (ruling B). Historical row `229be911…`.
    fn quoted_names_standard_root() -> ComboVal {
        let mut root = Self::v0_22_standard_root();
        Self::rename_engine_differential_keys(&mut root);
        root
    }

    /// Current library: quoted-names root plus O64/O65/O66.
    fn current_standard_root() -> ComboVal {
        let mut root = Self::quoted_names_standard_root();
        Self::apply_shelf_rulings(&mut root);
        root
    }

    /// Projection a given engine actually wrote: `for_cas_storage` sits
    /// between the builder and the bytes on disk (v0.26.0+). Historical
    /// rows must be this form, not the builder return value.
    fn as_shipped(root: ComboVal) -> ComboVal {
        match Value::Combo(root).for_cas_storage() {
            Value::Combo(root) => root,
            _ => unreachable!(),
        }
    }

    /// The table of roots this binary ships. Each row is a form some
    /// released `root_with_system()` actually returned.
    fn shipped_standard_roots(&self) -> StandardRootSet {
        StandardRootSet::from_roots([
            // This engine (shelf rulings, then CAS projection).
            self.root_with_system(),
            // Quoted-names arc: ruling B rename, then CAS projection
            // (`229be911…`).
            Self::as_shipped(Self::quoted_names_standard_root()),
            // v0.26.0 ..= v0.26.1: `for_cas_storage(v0_22_standard_root())`.
            Self::as_shipped(Self::v0_22_standard_root()),
            // Before v0.26.0, `root_with_system()` returned the builder
            // without `for_cas_storage` (Q-032 C3: `65f52e2d…`).
            Self::v0_22_standard_root(),
        ])
    }

    pub fn supports_standard_root(&self, digest: &str) -> bool {
        self.standard_roots.contains(digest)
    }

    pub fn eval_context(&self) -> EvalContext {
        let sys_root = self.root_with_system();
        let mut ctx = EvalContext::new(ComboVal::default()).with_standard_root(sys_root.clone());
        ctx.memo_enabled = false; // engine-internal: wrong root for memo (see field doc)
        ctx.privilege = self.privilege;
        // Initial horizon from ~%Config (bare names). Runtime override of
        // strategy is /set_strategy (mutates live ctx, not the genesis node).
        if let Some(Value::Combo(ref cfg)) = sys_root.get_field("~%Config").cloned() {
            // Engine-internal: apply the closed table including timeout.
            ctx.apply_horizon_config(cfg, true, true);
        }
        ctx
    }

    pub fn apply_morphism(&self, f: Value, arg: Value, ctx: &mut EvalContext) -> Value {
        let f = self.force(f, ctx);
        if let Value::Bottom(_) = f {
            return f;
        }
        if f.is_top() {
            return Value::Top;
        }
        // G3 R1: Blur is not a callable / not a dispatchable arg — absorb
        // (pass through) rather than falling into the non-combo Conflict arm.
        if let Value::Blur(_) = &f {
            return f;
        }
        let arg = self.force(arg, ctx);
        if let Value::Bottom(_) = arg {
            return arg;
        }
        if let Value::Blur(_) = &arg {
            return arg;
        }
        // bind additivity (SPEC_07 §4, ENGINE_SYNC #18): a superposed argument
        // evolves branchwise — f(A|B) = f(A) | f(B); ⊥ branches prune (| identity)
        if let Value::Union(branches) = arg {
            let mut out = Vec::new();
            for b in branches {
                let res = self.apply_morphism(f.clone(), b, ctx);
                if !matches!(res, Value::Bottom(_))
                    && !matches!(res, Value::Atom(nlang_parser::ast::AtomKind::Bottom, _, _))
                {
                    out.push(res);
                }
            }
            return self.normalize_union_absorbing(out, ctx);
        }
        if !f.is_morphism() {
            if arg.is_morphism() {
                return self.apply_morphism(arg, f, ctx);
            }
        }
        match f {
            Value::Combo(ref c) => {
                if let Some(inner) = c.get_field("%val") {
                    return self.apply_morphism(inner.clone(), arg, ctx);
                }

                let is_arg_pack = match &arg {
                    Value::Combo(ac) => {
                        ac.contains_key("%arg")
                            || (ac.contains_key("0") && !ac.contains_key("%kind"))
                    }
                    _ => false,
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
                    // caid_of_the_argument: mark wraps only. Pack-shaped args
                    // (tuples, {{0:…}}) take the branch above and carry no marker.
                    nf.insert(
                        "%arg".to_string(),
                        Value::Atom(
                            nlang_parser::ast::AtomKind::Tag("true".to_string()),
                            crate::value::EffectTag::Pure,
                            None,
                        ),
                    );
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
                let unified_arg = Value::Combo(ComboVal::new(
                    nf,
                    true,
                    IndexMap::new(),
                    arg.effect(),
                    vec![],
                ));

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
                    let has_pattern_fields = c
                        .all_fields_iter()
                        .any(|(k, _)| !k.starts_with('%') && k.parse::<usize>().is_err());
                    if has_pattern_fields {
                        let dispatch_result = self.dispatch_morphism(c, &arg, ctx);
                        return dispatch_result.to_value(f.effect());
                    }
                }

                if let Some(Value::Atom(AtomKind::Str(builtin_id), _, _)) = c.get_field("%builtin")
                {
                    if let Some(func) = self.builtin_registry.get(builtin_id) {
                        let res = func(unified_arg.clone(), self, ctx);
                        if let Value::Top = res {
                            let mut partial_fields = c.fields().clone();
                            if let Value::Combo(ref ac) = unified_arg {
                                for (k, v) in &ac.fields() {
                                    // Calling-convention marker must not become
                                    // part of a user-visible partial morphism.
                                    if k == "%arg" {
                                        continue;
                                    }
                                    partial_fields.insert(k.clone(), v.clone());
                                }
                            }
                            partial_fields.insert(
                                "%morphism".to_string(),
                                Value::Atom(
                                    AtomKind::Tag("true".to_string()),
                                    EffectTag::Pure,
                                    None,
                                ),
                            );
                            partial_fields.insert(
                                "%kind".to_string(),
                                Value::Atom(
                                    AtomKind::Tag("logic".to_string()),
                                    EffectTag::Pure,
                                    None,
                                ),
                            );
                            return Value::Combo(ComboVal::new(
                                partial_fields,
                                true,
                                IndexMap::new(),
                                f.effect(),
                                vec![],
                            ));
                        }
                        return res;
                    }
                }

                let ks = arg.collapse().to_string_plain();
                if let Some(v) = c
                    .get_field(&ks)
                    .or_else(|| c.get_field("it"))
                    .or_else(|| c.get_field("_"))
                {
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
            Value::Thunk {
                expr,
                closure,
                context,
                effect,
            } => {
                let fuel_before = ctx.fuel;
                // Stage 4 (§4b): force-level memo with tier strategy.
                // Stage 5 (§5-pre): staged gate — evolve-time forces read
                // staged, which the key does not include.
                let effective_context: Option<Value> = context
                    .as_ref()
                    .map(|b| (**b).clone())
                    .or(ctx.context_value.clone());
                let tier: Option<Tier> = classify_tier(&expr).0.into();
                let should_memo = matches!(tier, Some(Tier::C) | Some(Tier::M));
                let staged_ok = ctx.staged.is_none() && ctx.memo_enabled;
                let memo_key = if should_memo && staged_ok {
                    let expr_caid = {
                        let mut h = sha2::Sha256::new();
                        h.update(expr.to_nlang(0).as_bytes());
                        ContentHash::v1(h.finalize().to_vec())
                    };
                    // Frame identity for memo: content digests of Arc frames
                    // (see thunk_cycle_id / frames_content_digest).
                    let frame_caid = frames_content_digest(&closure);
                    let context_caid = effective_context.as_ref().map(|v| v.content_hash());
                    Some(ForceMemoKey {
                        expr_caid,
                        frame_caid,
                        context_caid,
                        strategy: ctx.strategy,
                        fuel_budget: ctx.fuel_budget,
                    })
                } else {
                    None
                };

                if let Some(ref k) = memo_key {
                    let hit = self
                        .force_memo
                        .read()
                        .ok()
                        .and_then(|memo| memo.get(k).cloned());
                    if let Some(entry) = hit {
                        if let Err(e) = ctx.check_resources(entry.mbu_cost) {
                            let partial =
                                if crate::observation::needs_partial_body(&e, ctx.strategy) {
                                    Some(Value::Code(Box::new(
                                        expr.as_ref().clone().without_spans(),
                                    )))
                                } else {
                                    None
                                };
                            return handle_resource_exhausted(
                                e,
                                ctx.strategy,
                                &*ctx,
                                partial,
                                effect,
                            );
                        }
                        // §10 repair: memo reuse is observable for testing,
                        // but its semantic MBU bill remains identical to a
                        // miss, so cache warmth cannot affect #blur CAIDs.
                        self.force_memo_hit_count
                            .fetch_add(1, Ordering::Relaxed);
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

                // L2-17 / forward-ref: same-thunk re-entry → ⊥ #divergent
                // (before stack/fuel). Cycle key uses content digests of
                // sealed frames (M1), memoized per Arc — not Arc::as_ptr
                // (SPEC_01 §2.4.1) and not full inline content_hash of the
                // frame tree (2^depth, the cost D1 eliminates).
                //
                // Path-shaped thunks (`out: mid`): do NOT mark the *target*
                // path (`mid`) into `computing` — that collides with
                // force_coord of the binding that *lives* at mid (false
                // #divergent on bare reference chains). Cycle detection for
                // path self-loops keys on the *holder* coordinate already
                // placed in `computing` by force_coord (see below), or on
                // in_flight when solidifying a re-fetched Thunk.
                let thunk_id = thunk_cycle_id(&expr, &closure, &context, effect);
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

                // A thunk force opens one deferred subspace even when its
                // expression is an atom. This is semantic observation work,
                // not AST traversal; include it in the stored memo bill.
                if let Err(e) = ctx.check_resources(mbu::SUBSPACE_EXPANSION) {
                    let partial = if crate::observation::needs_partial_body(&e, ctx.strategy) {
                        Some(Value::Code(Box::new(expr.as_ref().clone().without_spans())))
                    } else {
                        None
                    };
                    return handle_resource_exhausted(e, ctx.strategy, &*ctx, partial, effect);
                }

                // Stage 5 (§5a): nested dep_collector propagation.
                // Install fresh collector in call_ctx; after eval, merge
                // inner deps into outer collector (inner result embeds
                // inner deps — they must float up).
                let had_inner_collector = ctx.dep_collector.is_some();
                let inner_collector = if should_memo && staged_ok {
                    Some(HashSet::new())
                } else {
                    None
                };

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
                let mbu_cost = fuel_before.saturating_sub(ctx.fuel);

                // Do NOT wrap Bottom/Blur/Top in a pure-wrapper for effect
                // escalation — that shell traps `.%cause` navigation (meta
                // segment treated as on-shell, peel skipped, open miss Top).
                let res = match res {
                    Value::Atom(kind, old_e, r) => Value::Atom(kind, old_e.union(effect), r),
                    Value::Combo(mut cv) => {
                        cv.effect = cv.effect.union(effect);
                        Value::Combo(cv)
                    }
                    Value::Bottom(_) | Value::Blur(_) | Value::Top | Value::TopCaused { .. } => res,
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
                    for d in &deps {
                        outer.insert(d.clone());
                    }
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
                            if memo.len() >= FORCE_MEMO_CAP {
                                memo.clear();
                                if let Ok(mut rev) = self.force_memo_rev.write() {
                                    rev.clear();
                                }
                            }
                            if let Ok(mut rev) = self.force_memo_rev.write() {
                                for dep in &deps {
                                    rev.entry(dep.clone()).or_default().insert(k.clone());
                                }
                            }
                            memo.insert(
                                k.clone(),
                                MemoEntry {
                                    value: res.clone(),
                                    mbu_cost,
                                    deps,
                                },
                            );
                        }
                    }
                }
                res
            }
            Value::Blur(_) => val,
            Value::Ref(_) if ctx.preserve_refs => val,
            Value::Ref(path) => {
                // Stage 3 (§3a): dereference — resolve path against ctx.root
                // at observation time. fuel charged here (force = observation
                // primitive, GUIDE_03 §11.4).
                // Stage 5 (§5a): deref is a universal root read — any evolve
                // invalidates (conservative over-approximation, correct for
                // self-referential chains).
                self.record_dep(ctx, "*");
                let cost = mbu::SUBSPACE_EXPANSION;
                if let Err(e) = ctx.check_resources(cost) {
                    return handle_resource_exhausted(
                        e,
                        ctx.strategy,
                        &*ctx,
                        None,
                        EffectTag::Pure,
                    );
                }
                self.resolve_path_internal(&path, ctx)
            }
            // forward_spread: private force_coord skips the computing gate and
            // only calls force — expand pending spreads here too so bits/pipe
            // on `~d: {...~c, z:3}` see merged fields.
            Value::Combo(c) if !c.pending_spreads.is_empty() => self.expand_combo_pending(c, ctx),
            _ => val,
        }
    }

    pub fn force_recursive(&self, val: Value, ctx: &mut EvalContext) -> Value {
        // Already-solid combos (cocoon eval forced fields after seal): avoid
        // recursively re-forcing them, but still bill the semantic subspaces
        // inspected to establish that fact.
        if let Value::Combo(ref c) = val {
            if let Some(cost) = solid_combo_expansion_cost(c) {
                if let Err(e) = ctx.check_resources(cost) {
                    let partial = if crate::observation::needs_partial_body(&e, ctx.strategy) {
                        Some(val.clone())
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
                return val;
            }
        }
        // Thunk peel without stacking force_recursive depth: force alone runs
        // eval (cocoon post-seal solidify charges depth once). Nested
        // force_recursive(Thunk) used to add a second/third unit per level.
        if matches!(val, Value::Thunk { .. }) {
            let mut v = val;
            let mut peel = 0u32;
            while matches!(v, Value::Thunk { .. }) && peel < 64 {
                if let Err(e) = ctx.check_resources(mbu::SUBSPACE_EXPANSION) {
                    let partial = if crate::observation::needs_partial_body(&e, ctx.strategy) {
                        Some(v.clone())
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
                v = self.force(v, ctx);
                peel += 1;
            }
            return self.force_recursive(v, ctx);
        }
        // Stage 3 (§3c): solidification must participate in depth accounting —
        // a self-referential Ref chain (v: <<_.>>) recurses through here, not
        // through eval, so without this guard the Rust stack dies before the
        // fuel horizon ever engages. Depth exhaustion is the same semantic
        // truncation as fuel: the horizon, not an error.
        ctx.depth += 1;
        if let Err(e) = ctx.check_resources(mbu::SUBSPACE_EXPANSION) {
            ctx.depth -= 1;
            // O42 R-3: node_content of the force site for Blur only.
            let partial = if crate::observation::needs_partial_body(&e, ctx.strategy) {
                Some(val.clone())
            } else {
                None
            };
            return handle_resource_exhausted(e, ctx.strategy, &*ctx, partial, EffectTag::Pure);
        }
        // A quote of the combo that contains it must stay a quote in the
        // durable projection (`preserve_refs`). Unfolding it writes a
        // self-nested object that serde cannot read back.
        if matches!(&val, Value::Ref(_)) && ctx.preserve_refs {
            return val;
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
        } else {
            None
        };
        let res = match val {
            // Residual thunk after force (path field leave): peel without
            // double-counting depth (Thunk arm above).
            Value::Thunk { .. } => self.force_recursive(val, ctx),
            Value::Combo(c) => {
                // May have become solid under force (thunk → cocoon eval).
                if let Some(cost) = solid_combo_expansion_cost(&c) {
                    match ctx.check_resources(cost) {
                        Ok(()) => Value::Combo(c),
                        Err(e) => {
                            let partial =
                                if crate::observation::needs_partial_body(&e, ctx.strategy) {
                                    Some(Value::Combo(c))
                                } else {
                                    None
                                };
                            handle_resource_exhausted(
                                e,
                                ctx.strategy,
                                &*ctx,
                                partial,
                                EffectTag::Pure,
                            )
                        }
                    }
                } else {
                    // forward_spread: expand deferred sources before solidifying fields.
                    let expanded = self.expand_combo_pending(c, ctx);
                    match expanded {
                        Value::Combo(mut c) => {
                            let mut new_c = ComboVal::default();
                            new_c.closed = c.closed;
                            new_c.effect = c.effect;
                            new_c.relations = std::mem::take(&mut c.relations);
                            // forward_spread acceptance repair: re-queued pending
                            // sources (evolve-phase Top) must survive the rebuild.
                            new_c.pending_spreads = std::mem::take(&mut c.pending_spreads);
                            let closed = c.closed;
                            // Copy per axis. Flattening through insert_field
                            // re-routes a data key named `@t` onto the type
                            // axis (Q1).
                            for (k, v) in std::mem::take(&mut c.data) {
                                let forced = self.force_recursive(v, ctx);
                                if !closed {
                                    new_c.effect = new_c.effect.union(forced.effect());
                                }
                                new_c.data.insert(k, forced);
                            }
                            for (k, v) in std::mem::take(&mut c.types) {
                                let forced = self.force_recursive(v, ctx);
                                if !closed {
                                    new_c.effect = new_c.effect.union(forced.effect());
                                }
                                new_c.types.insert(k, forced);
                            }
                            for (k, v) in std::mem::take(&mut c.rules) {
                                let forced = self.force_recursive(v, ctx);
                                if !closed {
                                    new_c.effect = new_c.effect.union(forced.effect());
                                }
                                new_c.rules.insert(k, forced);
                            }
                            for (k, v) in std::mem::take(&mut c.meta) {
                                let forced = self.force_recursive(v, ctx);
                                if !closed {
                                    new_c.effect = new_c.effect.union(forced.effect());
                                }
                                new_c.meta.insert(k, forced);
                            }
                            for (k, v) in std::mem::take(&mut c.system) {
                                let forced = self.force_recursive(v, ctx);
                                if !closed {
                                    new_c.effect = new_c.effect.union(forced.effect());
                                }
                                new_c.system.insert(k, forced);
                            }
                            for (k, v) in std::mem::take(&mut c.local) {
                                let forced = self.force_recursive(v, ctx);
                                if !closed {
                                    new_c.effect = new_c.effect.union(forced.effect());
                                }
                                new_c.local.insert(k, forced);
                            }
                            Value::Combo(new_c)
                        }
                        other => other,
                    }
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
            _ => val,
        };
        if is_ref {
            ctx.context_value = old_ctx_val;
        }
        ctx.depth -= 1;
        res
    }

    pub fn resolve_path(&self, path: &Path, ctx: &mut EvalContext) -> Value {
        let name_raw = if !path.segments.is_empty() {
            &path.segments[0]
        } else {
            ""
        };
        let name = name_raw.trim();

        if path.anchor == PathAnchor::Bare && path.segments.len() == 1 {
            if name == "#_|_" {
                return Value::Atom(AtomKind::TagStart, EffectTag::Pure, None);
            }
            if name == "#_" {
                return Value::Atom(AtomKind::TagEnd, EffectTag::Pure, None);
            }
            if name == "_" {
                return Value::Top;
            }
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
            let return_or_force =
                |oo: &Self, n: &str, val: Value, ctx: &mut EvalContext, use_coord: bool| -> Value {
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
                    let alt_name = if name.starts_with(p) {
                        name.trim_start_matches(p).to_string()
                    } else {
                        format!("{}{}", p, name)
                    };
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
                if let Some(eff) = crate::universe::effective_config(
                    &ctx.root,
                    &ctx.standard_root,
                    ctx.staged.as_ref(),
                )
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
                    let alt_name = if name.starts_with(p) {
                        name.trim_start_matches(p).to_string()
                    } else {
                        format!("{}{}", p, name)
                    };
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
                let alt_name = if name.starts_with(p) {
                    name.trim_start_matches(p).to_string()
                } else {
                    format!("{}{}", p, name)
                };
                if let Some(val) = ctx.root.get_field(&alt_name) {
                    return return_or_force(self, &alt_name, val.clone(), ctx, true);
                }
                if let Some(val) = ctx.root.get_local_field(&alt_name) {
                    return return_or_force(self, &alt_name, val.clone(), ctx, false);
                }
            }

            // Standard names are a final lookup layer.  They are not
            // hydrated into the user root, so a user definition above wins.
            if let Some(val) = ctx.standard_root.get_field(name) {
                let v = val.clone();
                self.record_dep(ctx, name);
                return return_or_force(self, name, v, ctx, true);
            }
            if let Some(val) = ctx.standard_root.get_local_field(name) {
                let v = val.clone();
                self.record_dep(ctx, name);
                return return_or_force(self, name, v, ctx, false);
            }
            for p in ["/", "@", "~", "~%"] {
                let alt_name = if name.starts_with(p) {
                    name.trim_start_matches(p).to_string()
                } else {
                    format!("{}{}", p, name)
                };
                if let Some(val) = ctx.standard_root.get_field(&alt_name) {
                    return return_or_force(self, &alt_name, val.clone(), ctx, true);
                }
                if let Some(val) = ctx.standard_root.get_local_field(&alt_name) {
                    return return_or_force(self, &alt_name, val.clone(), ctx, false);
                }
            }

            // Non-builtin @Name not found → Unknown marker (same shape as
            // the old always-marker path; validate = unconditional pass).
            if TypeConstraint::is_type_constraint_path(name) {
                return TypeConstraint::marker_value(name.trim_start_matches('@'));
            }
            // Closed world on `~%` only: the engine minted every name on
            // this axis, so an absent one is `#missing_key`, not `_`.
            if name.starts_with("~%") {
                return BottomCause::MissingKey.into();
            }
        }
        self.resolve_path_internal(path, ctx)
    }

    fn resolve_path_internal(&self, path: &Path, ctx: &mut EvalContext) -> Value {
        let start_val: Value = match path.anchor {
            PathAnchor::Root => Value::Combo((*ctx.root).clone()),
            PathAnchor::Bare => {
                let name = if !path.segments.is_empty() {
                    path.segments[0].trim()
                } else {
                    ""
                };
                let mut found = None;
                for scope in ctx.scopes.iter().rev() {
                    if let Some(val) = scope.get_field(name) {
                        found = Some(val.clone());
                        break;
                    }
                    if let Some(val) = scope.get_local_field(name) {
                        found = Some(val.clone());
                        break;
                    }
                    let prefixes = vec!["/", "@", "~", "~%"];
                    for p in prefixes {
                        let alt_name = if name.starts_with(p) {
                            name.trim_start_matches(p).to_string()
                        } else {
                            format!("{}{}", p, name)
                        };
                        if let Some(val) = scope.get_field(&alt_name) {
                            found = Some(val.clone());
                            break;
                        }
                        if let Some(val) = scope.get_local_field(&alt_name) {
                            found = Some(val.clone());
                            break;
                        }
                    }
                    if found.is_some() {
                        break;
                    }
                }
                // SPEC_09 §6: never bind staged Config fragment as ~%Config;
                // multi-segment reads (~%Config.timeout) need genesis ∧ override.
                if found.is_none() && name == "~%Config" {
                    if let Some(eff) =
                        crate::universe::effective_config(
                            &ctx.root,
                            &ctx.standard_root,
                            ctx.staged.as_ref(),
                        )
                    {
                        found = Some(Value::Combo(eff));
                        self.record_dep(ctx, "~%Config");
                    }
                }
                if found.is_none() {
                    if let Some(ref s) = ctx.staged {
                        if let Some(val) = s.get_field(name).or_else(|| s.get_local_field(name)) {
                            found = Some(val.clone());
                            self.record_dep(ctx, name);
                        } else {
                            let prefixes = vec!["/", "@", "~", "~%"];
                            for p in prefixes {
                                let alt_name = if name.starts_with(p) {
                                    name.trim_start_matches(p).to_string()
                                } else {
                                    format!("{}{}", p, name)
                                };
                                if let Some(val) = s
                                    .get_field(&alt_name)
                                    .or_else(|| s.get_local_field(&alt_name))
                                {
                                    found = Some(val.clone());
                                    self.record_dep(ctx, &alt_name);
                                    break;
                                }
                            }
                        }
                    }
                }
                if found.is_none() {
                    if let Some(val) = ctx
                        .root
                        .get_field(name)
                        .or_else(|| ctx.root.get_local_field(name))
                    {
                        found = Some(val.clone());
                        self.record_dep(ctx, name);
                    } else {
                        let prefixes = vec!["/", "@", "~", "~%"];
                        for p in prefixes {
                            let alt_name = if name.starts_with(p) {
                                name.trim_start_matches(p).to_string()
                            } else {
                                format!("{}{}", p, name)
                            };
                            if let Some(val) = ctx
                                .root
                                .get_field(&alt_name)
                                .or_else(|| ctx.root.get_local_field(&alt_name))
                            {
                                found = Some(val.clone());
                                self.record_dep(ctx, &alt_name);
                                break;
                            }
                        }
                    }
                }
                // Standard names are a final lookup layer here too: the
                // bare-name path above consults ctx.root first, and a
                // projection of the same name must mean the same thing by it
                // (O58 work order §2.1 -- the direction must not change).
                if found.is_none() {
                    if let Some(val) = ctx
                        .standard_root
                        .get_field(name)
                        .or_else(|| ctx.standard_root.get_local_field(name))
                    {
                        found = Some(val.clone());
                        self.record_dep(ctx, name);
                    } else {
                        for p in ["/", "@", "~", "~%"] {
                            let alt_name = if name.starts_with(p) {
                                name.trim_start_matches(p).to_string()
                            } else {
                                format!("{}{}", p, name)
                            };
                            if let Some(val) = ctx
                                .standard_root
                                .get_field(&alt_name)
                                .or_else(|| ctx.standard_root.get_local_field(&alt_name))
                            {
                                found = Some(val.clone());
                                self.record_dep(ctx, &alt_name);
                                break;
                            }
                        }
                    }
                }
                match found {
                    None if name.starts_with("~%") => {
                        return BottomCause::MissingKey.into();
                    }
                    Some(v) => {
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
                            } else {
                                None
                            };
                            let res =
                                self.navigate_segments(forced, &path.segments[1..], ctx, name);
                            if is_ref {
                                ctx.context_value = old_ctx_val;
                            }
                            return res;
                        }
                        // Single-segment: no navigation subtree — return deref'd
                        // value directly, no context scoping needed. But if this
                        // Ref observation feeds into force_recursive, the recursion
                        // needs the deref context (handled in force_recursive F1).
                        forced
                    }
                    None => Value::Top,
                }
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
                    Value::Combo((*ctx.scopes[len - 1 - hops]).clone())
                } else if hops == len {
                    // Exactly through all frames → root universe.
                    Value::Combo((*ctx.root).clone())
                } else {
                    // Past root.
                    return BottomCause::OutOfHorizon.into();
                }
            }
            PathAnchor::Current => {
                if let Some(top) = ctx.scopes.last() {
                    Value::Combo((**top).clone())
                } else {
                    Value::Combo((*ctx.root).clone())
                }
            }
            // Local store only (this arc). Missing address is a named
            // refusal, never silent `_` (§4.2). The act of resolving is
            // pure (§4.3 / O70 ④): do not tag IO onto whatever came back.
            PathAnchor::Address {
                algo: AddressAlgo::Sha256,
                digest,
            } => {
                let hex = hex::encode(digest);
                let addr_label = format!("sha256:{hex}");
                let hash = crate::value::ContentHash::v1(digest.to_vec());
                match self.store.get_value(&hash) {
                    Ok(v) => v,
                    Err(_) => Value::Bottom(Box::new(crate::value::BottomDetail {
                        cause: BottomCause::MissingKey,
                        path: Some(addr_label.clone()),
                        message: Some(addr_label),
                        expected: None,
                        found: None,
                        involved: vec![],
                        ..Default::default()
                    })),
                }
            }
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
    fn combo_embeds_type_marker_shallow(&self, c: &ComboVal, ctx: &mut EvalContext) -> bool {
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

    fn navigate_segments(
        &self,
        start: Value,
        segments: &[String],
        ctx: &mut EvalContext,
        path_prefix: &str,
    ) -> Value {
        let mut val = start;
        let mut accumulated_effect = val.effect();
        let mut path_so_far = path_prefix.to_string();
        for seg in segments {
            if let Err(e) = ctx.check_resources(mbu::SUBSPACE_EXPANSION) {
                return handle_resource_exhausted(e, ctx.strategy, &*ctx, None, accumulated_effect);
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
                    } else {
                        break;
                    }
                } else if crate::value::is_structural_view(c) {
                    // G6 acceptance repair: the structural-view mark is a
                    // display filter, transparent to navigation — <<…>>
                    // bindings navigate exactly like the underlying node
                    // (SYNTAX_07 §4 #7: post-`>>` field access collapses).
                    if let Some(inner) = crate::value::structural_node(c) {
                        current = self.force(inner.clone(), ctx);
                        accumulated_effect = accumulated_effect.union(current.effect());
                    } else {
                        break;
                    }
                } else {
                    break;
                }
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
                return Value::Atom(
                    AtomKind::Str(solid.content_hash_with_salt(&ctx.horizon_salt).to_string()),
                    EffectTag::Pure,
                    None,
                )
                .with_effect(accumulated_effect);
            }
            if seg == "%rank" {
                if let Value::Atom(_, _, Some(r)) = current {
                    return Value::Atom(AtomKind::Int(BigInt::from(r)), EffectTag::Pure, None)
                        .with_effect(accumulated_effect);
                }
            }
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
                                message: Some(format!("Key '{}' missing in closed Cocoon", seg)),
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
                        let mut projected = self.navigate_segments(b, &[seg.to_string()], ctx, "");
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
                                branch_effect =
                                    branch_effect.union(Value::Bottom(d.clone()).effect());
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
                        return primary_bottom_from_culled(culled).with_effect(branch_effect);
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

    fn inject_path(
        &self,
        target: &mut ComboVal,
        segments: &[String],
        val: Value,
    ) -> std::result::Result<(), BottomCause> {
        if segments.is_empty() {
            return Ok(());
        }
        let key = segments[0].trim();
        if segments.len() == 1 {
            target.insert_field(key, val);
        } else {
            let mut sub = match target.get_field(key) {
                Some(Value::Combo(c)) => c.clone(),
                _ => ComboVal::new(
                    IndexMap::new(),
                    false,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                ),
            };
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
                Some(targets) => {
                    current = targets[0].clone();
                }
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
        let mut sub = ctx.clone();
        sub.call_history.clear();
        sub
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
    fn force_lexical_name(
        &self,
        name: &str,
        val: Value,
        ctx: &mut EvalContext,
        use_coord: bool,
    ) -> Value {
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

    /// Fetch a value from a remote peer via OODP (REAL_02 §3.2) and re-verify
    /// its address (REAL_03 §6.6). Peer `%status` is a claim, never trust.
    ///
    /// - `Ok(val)` — `#success` and address matches the requested CAID
    /// - `Err(MissingKey)` — peer `#not_found` (absence, not conflict)
    /// - `Err(CaidMismatch)` — peer `#conflict`, bad envelope, or address fail
    /// - `Err(Timeout)` — read/connect deadline (distinct from all three)
    /// - `Err(Conflict)` — connection refused / empty body / other transport
    pub fn remote_fetch(&self, addr: &str, hash: &ContentHash) -> Result<Value, BottomCause> {
        crate::oodp::remote_fetch_oodp(self, addr, hash)
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
