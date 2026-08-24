use indexmap::IndexMap;
use nlang_parser::ast::{AtomKind, Expr, Path, PathAnchor, Span};
use ring::{
    rand,
    signature::{self, KeyPair as _},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, RwLock};

/// Effect label **set** (SPEC_08 §4.1 join-semilattice). Composition is
/// set-union, not a total-order max — `io`/`nondet`/`state` are incomparable
/// siblings (effect_union arc 1, 2026-07-23).
///
/// Bit layout: bit0=IO, bit1=NonDet, bit2=State, bit3=Cached; `0` = Pure.
/// No `PartialOrd`/`Ord` — forbids silent scalar collapse via `.max()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EffectTag(u8);

impl EffectTag {
    // Legacy names (not SCREAMING_SNAKE) keep ~480 call sites stable.
    #[allow(non_upper_case_globals)]
    pub const Pure: EffectTag = EffectTag(0);
    #[allow(non_upper_case_globals)]
    pub const IO: EffectTag = EffectTag(0b0001);
    #[allow(non_upper_case_globals)]
    pub const NonDet: EffectTag = EffectTag(0b0010);
    #[allow(non_upper_case_globals)]
    pub const State: EffectTag = EffectTag(0b0100);
    /// Reserved (§4.2.4); no producer in arc 1.
    #[allow(non_upper_case_globals)]
    pub const Cached: EffectTag = EffectTag(0b1000);

    pub fn union(self, o: EffectTag) -> EffectTag {
        EffectTag(self.0 | o.0)
    }
    /// True if `o` is a non-empty subset of `self`.
    pub fn contains(self, o: EffectTag) -> bool {
        o.0 != 0 && (self.0 & o.0) == o.0
    }
    /// True if every bit of `o` is set in `self` (Pure is always covered).
    pub fn contains_all(self, o: EffectTag) -> bool {
        (self.0 & o.0) == o.0
    }
    pub fn is_pure(self) -> bool {
        self.0 == 0
    }
    /// Active contagion tags (SPEC_08 §4.2.4 / §4.3): io | nondet | state.
    /// `#cached` and `#pure` are not active.
    pub fn has_active(self) -> bool {
        self.contains(EffectTag::IO)
            || self.contains(EffectTag::NonDet)
            || self.contains(EffectTag::State)
    }
    /// Mask to active contagion bits only (`#cached` stripped). Used by
    /// selective discharge coverage (SPEC_08 §6.2).
    pub fn active_part(self) -> EffectTag {
        EffectTag(self.0 & (Self::IO.0 | Self::NonDet.0 | Self::State.0))
    }
    /// All three active tags (for bare `effect_override` grant).
    pub fn all_active() -> EffectTag {
        Self::IO.union(Self::NonDet).union(Self::State)
    }
    /// Raw bits, for persisting a tag SET across processes.
    ///
    /// ACCEPTOR REPAIR (privileged_effect_audit): `.oo/effect_pending` has to
    /// carry *which* tags were discharged, not merely that something was, or
    /// commit cannot check that the capability re-presented is the one the act
    /// required. Measured on the delivered build: a discharge of `io` was
    /// authorised at commit by `--grant effect_override:nondet`.
    pub fn to_bits(self) -> u8 {
        self.0
    }
    /// Inverse of [`to_bits`], masked to active tags — a persisted set is only
    /// ever an `active_part()`, and unknown bits must not become capabilities.
    pub fn from_bits(bits: u8) -> EffectTag {
        EffectTag(bits & (Self::IO.0 | Self::NonDet.0 | Self::State.0))
    }
    /// Thunk CAID serial byte: single-tag legacy ordinals unchanged
    /// (Pure=0, State=1, IO=2, NonDet=3); multi-tag / Cached use high bit.
    pub fn to_serial_byte(self) -> u8 {
        match self.0 {
            0 => 0,      // Pure
            0b0100 => 1, // State (legacy)
            0b0001 => 2, // IO (legacy)
            0b0010 => 3, // NonDet (legacy)
            bits => 0x80 | bits,
        }
    }
    /// Tag names present in the set, alphabetical (io, nondet, pure, state).
    fn tag_names(self) -> Vec<&'static str> {
        if self.0 == 0 {
            return vec!["pure"];
        }
        let mut names = Vec::new();
        // Alphabetical emission: io, nondet, state (, cached)
        if self.0 & Self::IO.0 != 0 {
            names.push("io");
        }
        if self.0 & Self::NonDet.0 != 0 {
            names.push("nondet");
        }
        if self.0 & Self::State.0 != 0 {
            names.push("state");
        }
        if self.0 & Self::Cached.0 != 0 {
            names.push("cached");
        }
        names
    }
}

impl std::ops::BitOr for EffectTag {
    type Output = EffectTag;
    fn bitor(self, rhs: EffectTag) -> EffectTag {
        self.union(rhs)
    }
}

/// Canonical value spelling for an effect set.  Kept beside `EffectTag` so
/// CAS projection can write `%effect` without depending on the evaluator.
pub fn effect_tag_value(e: EffectTag) -> Value {
    if e.is_pure() {
        return Value::Atom(AtomKind::Tag("pure".to_string()), EffectTag::Pure, None);
    }
    let mut atoms = Vec::new();
    for (tag, member) in [
        ("io", EffectTag::IO),
        ("nondet", EffectTag::NonDet),
        ("state", EffectTag::State),
        ("cached", EffectTag::Cached),
    ] {
        if e.contains(member) {
            atoms.push(Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::Pure, None));
        }
    }
    if atoms.len() == 1 {
        atoms.pop().unwrap()
    } else {
        Value::Union(atoms)
    }
}

impl fmt::Display for EffectTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = self.tag_names();
        let mut first = true;
        for n in names {
            if !first {
                write!(f, " | ")?;
            }
            write!(f, "#{n}")?;
            first = false;
        }
        Ok(())
    }
}

/// SPEC_08 §6.2 privileged capability lattice (selective_discharge).
/// Horizon-only, not stored in values / CAID. Trusted-channel set only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Privilege {
    /// `#effect_override`: `None` = op not authorized (even pure args refused);
    /// `Some(tags)` = may discharge exactly those active tags (all-or-nothing).
    pub effect_override: Option<EffectTag>,
    /// `#pin` — re-present at commit when `.oo/pin_pending` is set.
    pub pin: bool,
    /// Retired as a CLI grant (SPEC_08 §6.2 2026-07-26); field kept absent
    /// from live grants. Do not re-enable without a consumer.
    pub commit: bool,
    /// `#rollback` history op.
    pub rollback: bool,
    /// `#squash` history op.
    pub squash: bool,
    /// Local store GC (`oo gc`) — irreversible byte deletion (local_gc arc).
    pub gc: bool,
    /// Remote `~%Discovery./connect` with `tcp://` (connect_consent).
    /// Local ObjectStore form needs no grant.
    pub connect: bool,
    /// `oo migrate` — advance container layout declarations (O73 ③).
    pub migrate: bool,
}

impl Privilege {
    pub const NONE: Privilege = Privilege {
        effect_override: None,
        pin: false,
        commit: false,
        rollback: false,
        squash: false,
        gc: false,
        connect: false,
        migrate: false,
    };

    /// Full grant (CLI `--privileged` back-compat). Does **not** set `commit`
    /// (retired spelling). Includes `connect` (full §6 grant surface).
    pub fn all() -> Privilege {
        Privilege {
            effect_override: Some(EffectTag::all_active()),
            pin: true,
            commit: false,
            rollback: true,
            squash: true,
            gc: true,
            connect: true,
            migrate: true,
        }
    }

    pub fn union(self, other: Privilege) -> Privilege {
        let effect_override = match (self.effect_override, other.effect_override) {
            (None, None) => None,
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (Some(a), Some(b)) => Some(a.union(b)),
        };
        Privilege {
            effect_override,
            pin: self.pin || other.pin,
            commit: self.commit || other.commit,
            rollback: self.rollback || other.rollback,
            squash: self.squash || other.squash,
            gc: self.gc || other.gc,
            connect: self.connect || other.connect,
            migrate: self.migrate || other.migrate,
        }
    }

    /// Q2: `C ⊇ E.active` — only active bits participate (`#cached` ignored).
    pub fn may_discharge(self, e: EffectTag) -> bool {
        match self.effect_override {
            None => false,
            Some(c) => c.contains_all(e.active_part()),
        }
    }
}

pub fn default_cache_id() -> Arc<RwLock<Option<ContentHash>>> {
    Arc::new(RwLock::new(None))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    Top,
    /// Caused Top (SPEC_01 §2.4.2 ruling C / SPEC_12 §1.1): lattice-identical
    /// to bare Top (solution set = everything) with observation-only
    /// provenance. Display / unify / PartialEq treat as bare Top;
    /// consumption evaporates the cause (operations yield plain Top).
    /// Diagnostic member under absorption — never absorbs / never absorbed.
    /// `cause`: `"static_cycle"` | `"no_coordinate"` (TAG_REGISTRY tags).
    TopCaused {
        /// Provenance tag body (no leading `#`).
        cause: String,
        /// Loop member coordinates for static_cycle (self = len 1, mutual = 2, …).
        /// Empty for open-miss `#no_coordinate`.
        members: Vec<String>,
    },
    Atom(AtomKind, EffectTag, Option<i64>),
    Combo(ComboVal),
    Union(Vec<Value>),
    Code(Box<Expr>),
    Thunk {
        expr: Box<Expr>,
        /// Scope frames shared by refcount (D1: every level of nesting doubles
        /// the universe). Was `Vec<ComboVal>` by value — seal_defining_scope
        /// deep-cloned the whole frame into every field thunk → 2^depth.
        /// Arc keeps seal's effect; cost falls to ~n² (frame structure still
        /// cloned once per level; Arc-ing Value::Combo is out of scope).
        closure: Vec<std::sync::Arc<ComboVal>>,
        #[serde(default)]
        context: Option<Box<Value>>,
        effect: EffectTag,
    },
    Ref(Path),
    Bottom(Box<BottomDetail>),
    Blur(BlurDetail),
    /// Closed interval set [start, end] (optionally stepped). Symbolic lattice
    /// value — observation neither materializes nor collapses it (SPEC_02 §3).
    Range {
        start: Box<Value>,
        end: Box<Value>,
        step: Option<Box<Value>>,
    },
}

/// Mint a caused Top for a pure-reference (static) cycle.
pub fn static_cycle_top(members: Vec<String>) -> Value {
    let mut m = members;
    m.sort();
    m.dedup();
    Value::TopCaused {
        cause: "static_cycle".to_string(),
        members: m,
    }
}

/// Mint a caused Top for open-miss navigation (undefined coordinate on an
/// open combo). Display stays `_`; `.%cause` → `#no_coordinate`.
pub fn no_coordinate_top() -> Value {
    Value::TopCaused {
        cause: "no_coordinate".to_string(),
        members: Vec::new(),
    }
}

/// `%cause` carrier for caused Top — closed cocoon, G6-peelable to tag.
/// REAL_04 §1 (2026-07-19): core = %val only; no fossil %type.
/// Anti-peel data pad `_`→Top is engine scaffolding — stripped at display
/// (to_nlang) so it never appears in user-visible projections.
pub fn static_cycle_cause_combo(members: &[String]) -> Value {
    caused_top_cause_combo("static_cycle", members)
}

/// Generic `%cause` carrier for any caused-Top tag.
pub fn caused_top_cause_combo(cause: &str, members: &[String]) -> Value {
    let mut fields = IndexMap::new();
    fields.insert(
        "%val".to_string(),
        Value::Atom(AtomKind::Tag(cause.to_string()), EffectTag::Pure, None),
    );
    // Anti-peel data pad (is_pure_wrapper requires empty data).
    fields.insert("_".to_string(), Value::Top);
    if !members.is_empty() {
        let mut mf = IndexMap::new();
        for (i, name) in members.iter().enumerate() {
            mf.insert(
                i.to_string(),
                Value::Atom(AtomKind::Str(name.clone()), EffectTag::Pure, None),
            );
        }
        mf.insert(
            "%kind".to_string(),
            Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
        );
        fields.insert(
            "%members".to_string(),
            Value::Combo(ComboVal::new(
                mf,
                false,
                IndexMap::new(),
                EffectTag::Pure,
                vec![],
            )),
        );
    }
    Value::Combo(ComboVal::new(
        fields,
        true,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
}

/// True if this value is a diagnostic member (blur or caused Top) — SPEC_01
/// §2.4.2 ruling C exemption.
pub fn is_diagnostic_member(v: &Value) -> bool {
    matches!(v, Value::Blur(_) | Value::TopCaused { .. })
}

/// True if any diagnostic member occurs within `max_depth` of `v`.
/// Bounded walk — no force, no thunk expand (normalization must not open
/// a new fuel horizon).
pub fn contains_diagnostic(v: &Value, max_depth: u32) -> bool {
    contains_diagnostic_inner(v, max_depth, 0)
}

fn contains_diagnostic_inner(v: &Value, max_depth: u32, depth: u32) -> bool {
    if depth > max_depth {
        return false;
    }
    match v {
        Value::Blur(_) | Value::TopCaused { .. } => true,
        Value::Combo(c) => {
            c.all_fields_iter()
                .any(|(_, fv)| contains_diagnostic_inner(&fv, max_depth, depth + 1))
                || c.local
                    .values()
                    .any(|fv| contains_diagnostic_inner(fv, max_depth, depth + 1))
        }
        Value::Union(bs) => bs
            .iter()
            .any(|b| contains_diagnostic_inner(b, max_depth, depth + 1)),
        Value::Range { start, end, step } => {
            contains_diagnostic_inner(start, max_depth, depth + 1)
                || contains_diagnostic_inner(end, max_depth, depth + 1)
                || step
                    .as_ref()
                    .map(|s| contains_diagnostic_inner(s, max_depth, depth + 1))
                    .unwrap_or(false)
        }
        // Thunk / Code / Ref / Atom / Top / Bottom: no nested diagnostic visible
        // without force — treat as non-diagnostic for absorption qualification.
        _ => false,
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // Lattice: caused Top ≡ bare Top (guardrail; cycle_test + `= _`).
            (Value::Top, Value::Top)
            | (Value::Top, Value::TopCaused { .. })
            | (Value::TopCaused { .. }, Value::Top)
            | (Value::TopCaused { .. }, Value::TopCaused { .. }) => true,
            (Value::Atom(a1, e1, r1), Value::Atom(a2, e2, r2)) => a1 == a2 && e1 == e2 && r1 == r2,
            (Value::Combo(c1), Value::Combo(c2)) => c1 == c2,
            // Union branches are a SET (SPEC_01: `|` commutative + idempotent;
            // G1 #11 集合觀). Branch order is display/encounter order only —
            // equality is multiset comparison, so `(1|2) = (2|1)` holds
            // without a build-time sort.
            (Value::Union(u1), Value::Union(u2)) => {
                u1.len() == u2.len() && {
                    let mut used = vec![false; u2.len()];
                    u1.iter().all(|a| {
                        u2.iter().enumerate().any(|(i, b)| {
                            if !used[i] && a == b {
                                used[i] = true;
                                true
                            } else {
                                false
                            }
                        })
                    })
                }
            }
            // G1 #13: Code/Thunk equality is span-blind (value property, not
            // source property). Spelling still differs (`q` vs `w`). Shared
            // with normalize_union so cmp and dedupe stay one relation.
            (Value::Code(c1), Value::Code(c2)) => c1.without_spans() == c2.without_spans(),
            (
                Value::Thunk {
                    expr: ex1,
                    closure: cl1,
                    context: c1,
                    effect: ef1,
                },
                Value::Thunk {
                    expr: ex2,
                    closure: cl2,
                    context: c2,
                    effect: ef2,
                },
            ) => ex1.without_spans() == ex2.without_spans() && cl1 == cl2 && c1 == c2 && ef1 == ef2,
            (Value::Bottom(b1), Value::Bottom(b2)) => b1 == b2,
            (Value::Blur(b1), Value::Blur(b2)) => b1 == b2,
            (
                Value::Range {
                    start: s1,
                    end: e1,
                    step: st1,
                },
                Value::Range {
                    start: s2,
                    end: e2,
                    step: st2,
                },
            ) => s1 == s2 && e1 == e2 && st1 == st2,
            (Value::Ref(p1), Value::Ref(p2)) => p1 == p2,
            _ => false,
        }
    }
}

/// G6: `%structural: #true` mark on a structural-view wrapper (`<<…>>`
/// non-path). Payload lives in `%node` (not `%val`) so lattice `collapse()`
/// does not erase the mark during evolve unify.
pub fn is_structural_view(cv: &ComboVal) -> bool {
    match cv.get_field("%structural") {
        Some(Value::Atom(AtomKind::Tag(t), _, _)) => t.trim_start_matches('#') == "true",
        _ => false,
    }
}

/// Payload of a structural-view wrapper (`%node`), if present.
pub fn structural_node(cv: &ComboVal) -> Option<&Value> {
    if is_structural_view(cv) {
        cv.get_field("%node")
    } else {
        None
    }
}

/// Unwrap structural-view mark to the full node; otherwise return `v` unchanged.
pub fn unwrap_structural_view(v: Value) -> Value {
    match v {
        Value::Combo(c) if is_structural_view(&c) => {
            c.get_field("%node").cloned().unwrap_or(Value::Combo(c))
        }
        other => other,
    }
}

/// SPEC_04 §3.1 #4: display projection strips the local (`~`) axis at every
/// depth. System axis (`~%…`) lives on `system`, not `local`, and is kept.
/// Does **not** alter CAID / `=` / content identity (those use the raw value).
///
/// Move-based (not `all_fields_iter` owned clones): a horizon-deep Ref chain
/// would OOM if each layer re-cloned the whole tree (stage3 stdlib probe).
pub fn strip_local_axis(v: Value) -> Value {
    match v {
        Value::Combo(mut c) => {
            if is_structural_view(&c) {
                let inner = c.get_field("%node").cloned().unwrap_or(Value::Combo(c));
                return strip_local_axis(inner);
            }
            c.local.clear();
            for (_, fv) in c.data.iter_mut() {
                let taken = std::mem::replace(fv, Value::Top);
                *fv = strip_local_axis(taken);
            }
            for (_, fv) in c.rules.iter_mut() {
                let taken = std::mem::replace(fv, Value::Top);
                *fv = strip_local_axis(taken);
            }
            for (_, fv) in c.types.iter_mut() {
                let taken = std::mem::replace(fv, Value::Top);
                *fv = strip_local_axis(taken);
            }
            for (_, fv) in c.meta.iter_mut() {
                let taken = std::mem::replace(fv, Value::Top);
                *fv = strip_local_axis(taken);
            }
            for (_, fv) in c.system.iter_mut() {
                let taken = std::mem::replace(fv, Value::Top);
                *fv = strip_local_axis(taken);
            }
            Value::Combo(c)
        }
        Value::Union(branches) => normalize_union(branches.into_iter().map(strip_local_axis)),
        other => other,
    }
}

/// Engine anti-peel scaffolding: data key `_` bound to Top (printed as `_`).
/// Not user-visible (REAL_04 cocoon_shape law 3). User fields named `_` with
/// any non-Top value still display.
fn is_engine_scaffold_field(key: &str, val: &Value) -> bool {
    key == "_" && matches!(val, Value::Top | Value::TopCaused { .. })
}

/// SYNTAX_03 §104 #6 inverse: a stored name is a plain identifier
/// (`named_key` = ident | numeric_key).
fn is_plain_ident(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    let mut cs = name.chars();
    matches!(cs.next(), Some(c) if c.is_alphabetic() || c == '_')
        && cs.all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

fn quote_nlang_multiline(s: &str) -> String {
    format!("\"\"\"{}\"\"\"", s.replace("\"\"\"", "\\\"\"\""))
}

fn quote_nlang_string(s: &str) -> String {
    if s.contains('"') || s.contains('\n') || s.contains('\r') {
        quote_nlang_multiline(s)
    } else {
        format!("\"{}\"", s)
    }
}

/// Print a field key. `prefix` is the axis sigil (`""`, `"@"`, `"/"`, …);
/// `name` is the stored map key (no sigil). Non-plain names are quoted so
/// the printed form parses back as the same coordinate (Q3).
fn quote_nlang_field_key(prefix: &str, name: &str) -> String {
    if is_plain_ident(name) {
        format!("{prefix}{name}")
    } else if name.contains('"') || name.contains('\n') || name.contains('\r') {
        format!("{prefix}{}", quote_nlang_multiline(name))
    } else {
        format!("{prefix}\"{name}\"")
    }
}

/// G6: collapsed-observation projection (SYNTAX_06 §4 #6 value-context).
/// Peels hybrid/pure-wrapper `%val` for display; recurses into combo fields
/// and list elements. Structural-view markers unwrap to the full node without
/// peeling its hybrid shape. Strips local axis (#4). Does **not** alter `to_nlang`.
pub fn project_value_context(v: Value) -> Value {
    match v {
        // Display: caused Top looks like bare `_` (provenance meta-only).
        Value::TopCaused { .. } => Value::Top,
        Value::Combo(c) => {
            // Structural view: full node, no hybrid peel (still strip local).
            if is_structural_view(&c) {
                let inner = c.get_field("%node").cloned().unwrap_or(Value::Combo(c));
                return strip_local_axis(inner);
            }
            // Hybrid or pure wrapper: value context reads %val.
            if let Some(inner) = c.get_field("%val").cloned() {
                return project_value_context(inner);
            }
            // Plain combo / list: project each public field; drop local axis.
            // Per-axis copy: insert_field would re-route data `@t` to types (Q1).
            let mut new_c = ComboVal::default();
            new_c.closed = c.closed;
            new_c.effect = c.effect;
            new_c.relations = c.relations.clone();
            new_c.masa_ref = c.masa_ref.clone();
            for (k, fv) in c.data {
                new_c.data.insert(k, project_value_context(fv));
            }
            for (k, fv) in c.types {
                new_c.types.insert(k, project_value_context(fv));
            }
            for (k, fv) in c.rules {
                new_c.rules.insert(k, project_value_context(fv));
            }
            for (k, fv) in c.meta {
                new_c.meta.insert(k, project_value_context(fv));
            }
            for (k, fv) in c.system {
                new_c.system.insert(k, project_value_context(fv));
            }
            Value::Combo(new_c)
        }
        Value::Union(branches) => normalize_union(branches.into_iter().map(project_value_context)),
        other => other,
    }
}

/// SPEC_04 §2.1 / §3.1 #1/#2/#3.3: inject this combo as a defining scope
/// frame into every field thunk (and nested values) so bare names — public
/// *and* private — resolve via the scope chain (sibling + ancestor lifting
/// + morphism capture when the morphism thunk is forced under this frame).
///
/// Frame is a pre-inject clone (no self-referential closure edges). Chained
/// sibling depth is **not** limited here: `force_lexical_name` keeps the
/// ambient frame on the chain when a bare name is forced *out of* a scope
/// frame (any hop). Twin-literal / `%id` tripwires stay green when public
/// spelling matches (equal pre-inject frames).
pub fn seal_defining_scope(c: &mut ComboVal) {
    // One deep clone of this level's structure, then Arc-shared into every
    // field thunk (D1). Pre-D1 each push cloned the whole frame again.
    let frame = std::sync::Arc::new(c.clone());
    fn inject(v: &mut Value, frame: &std::sync::Arc<ComboVal>) {
        match v {
            Value::Thunk { closure, .. } => {
                closure.push(std::sync::Arc::clone(frame));
            }
            Value::Combo(inner) => {
                for (_, fv) in inner.data.iter_mut() {
                    inject(fv, frame);
                }
                for (_, fv) in inner.local.iter_mut() {
                    inject(fv, frame);
                }
                for (_, fv) in inner.rules.iter_mut() {
                    inject(fv, frame);
                }
                for (_, fv) in inner.types.iter_mut() {
                    inject(fv, frame);
                }
                for (_, fv) in inner.meta.iter_mut() {
                    inject(fv, frame);
                }
                for (_, fv) in inner.system.iter_mut() {
                    inject(fv, frame);
                }
                // forward_spread: pending spread thunks need the holder frame too.
                for ps in inner.pending_spreads.iter_mut() {
                    inject(ps, frame);
                }
            }
            Value::Union(branches) => {
                for b in branches.iter_mut() {
                    inject(b, frame);
                }
            }
            _ => {}
        }
    }
    for (_, fv) in c.data.iter_mut() {
        inject(fv, &frame);
    }
    for (_, fv) in c.local.iter_mut() {
        inject(fv, &frame);
    }
    for (_, fv) in c.rules.iter_mut() {
        inject(fv, &frame);
    }
    for (_, fv) in c.types.iter_mut() {
        inject(fv, &frame);
    }
    for (_, fv) in c.meta.iter_mut() {
        inject(fv, &frame);
    }
    for (_, fv) in c.system.iter_mut() {
        inject(fv, &frame);
    }
}

// ── SPEC_01 §2.4.1 canonical display order (display layer only) ──

/// Type-family rank for union display (lower first).
/// numbers → strings → tag atoms → structured → #blur → Top → Bottom.
fn display_family_rank(v: &Value) -> u8 {
    match v {
        Value::Atom(AtomKind::Int(_), _, _)
        | Value::Atom(AtomKind::Float(_), _, _)
        | Value::Atom(AtomKind::Complex(_, _), _, _) => 0,
        Value::Atom(AtomKind::Str(_), _, _) => 1,
        Value::Atom(AtomKind::Tag(_), _, _)
        | Value::Atom(AtomKind::TagStart, _, _)
        | Value::Atom(AtomKind::TagEnd, _, _) => 2,
        Value::Blur(_) => 4,
        Value::Top | Value::TopCaused { .. } => 5,
        Value::Bottom(_) => 6,
        // Range, Combo, Thunk, Ref, Code, Bytes, … — structured family
        _ => 3,
    }
}

/// Numeric key: (magnitude as f64, subrank) with int before float on equal value.
fn display_numeric_key(v: &Value) -> Option<(f64, u8)> {
    match v {
        Value::Atom(AtomKind::Int(i), _, _) => {
            use num_traits::ToPrimitive;
            Some((i.to_f64().unwrap_or(f64::NAN), 0))
        }
        Value::Atom(AtomKind::Float(f), _, _) => Some((*f, 1)),
        Value::Atom(AtomKind::Complex(r, i), _, _) => {
            // Lexicographic on (re, im) via packed comparison of re first.
            Some((*r, 2 + if *i >= 0.0 { 0 } else { 1 }))
        }
        _ => None,
    }
}

fn display_string_key(v: &Value) -> Option<&str> {
    match v {
        Value::Atom(AtomKind::Str(s), _, _) => Some(s.as_str()),
        _ => None,
    }
}

fn display_tag_key(v: &Value) -> Option<String> {
    match v {
        Value::Atom(AtomKind::Tag(t), _, _) => Some(t.clone()),
        Value::Atom(AtomKind::TagStart, _, _) => Some("_|_".to_string()),
        Value::Atom(AtomKind::TagEnd, _, _) => Some("_".to_string()),
        _ => None,
    }
}

/// Compare two values for SPEC_01 §2.4.1 display order.
/// Does **not** use CAID/digest (blur CAID is salt-bearing).
fn display_order_cmp(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let ra = display_family_rank(a);
    let rb = display_family_rank(b);
    match ra.cmp(&rb) {
        Ordering::Equal => {}
        o => return o,
    }
    match ra {
        0 => {
            // Numbers: ascending; same magnitude prefers int over float.
            let ka = display_numeric_key(a);
            let kb = display_numeric_key(b);
            match (ka, kb) {
                (Some((va, sa)), Some((vb, sb))) => {
                    // Total order with NaN last within numbers.
                    let o = match (va.is_nan(), vb.is_nan()) {
                        (true, true) => Ordering::Equal,
                        (true, false) => Ordering::Greater,
                        (false, true) => Ordering::Less,
                        (false, false) => va.partial_cmp(&vb).unwrap_or(Ordering::Equal),
                    };
                    o.then_with(|| sa.cmp(&sb))
                }
                _ => a.to_nlang(0).cmp(&b.to_nlang(0)),
            }
        }
        1 => {
            let sa = display_string_key(a).unwrap_or("");
            let sb = display_string_key(b).unwrap_or("");
            sa.cmp(sb)
        }
        2 => {
            let ta = display_tag_key(a).unwrap_or_default();
            let tb = display_tag_key(b).unwrap_or_default();
            ta.cmp(&tb)
        }
        // structured (3): canonical display string lex (no salt).
        3 => a.to_nlang(0).cmp(&b.to_nlang(0)),
        // #blur (4): SPEC_01 §2.4.1 — cause + strategy + CHS digest.
        // NEVER fuel_remaining (a reading) or salt (O42). Full key tie → Equal
        // so stable sort keeps encounter order.
        4 => match (a, b) {
            (Value::Blur(ba), Value::Blur(bb)) => ba
                .cause
                .as_str()
                .cmp(bb.cause.as_str())
                .then_with(|| ba.horizon.strategy_byte().cmp(&bb.horizon.strategy_byte()))
                .then_with(|| ba.blur_caid().digest.cmp(&bb.blur_caid().digest)),
            _ => Ordering::Equal,
        },
        // Top (5) / Bottom (6): all equal within family (stable keeps order).
        _ => Ordering::Equal,
    }
}

/// SPEC_01 §2.4.1: order union branches for display only.
/// Stable: equal keys keep encounter order. Never mutates the value vector.
pub fn canonical_display_order(branches: &[Value]) -> Vec<&Value> {
    let mut refs: Vec<&Value> = branches.iter().collect();
    refs.sort_by(|a, b| display_order_cmp(a, b));
    refs
}

/// All-⊥ union collapse (REAL_04 §4 + 2026-07-17 engineering supplement):
/// pick the primary-rank member (`primary_rank` lower = more primary) and
/// pass that `_|_` out **verbatim** (message/path/involved preserved).
/// Ties keep encounter-order leftmost (`min_by_key` first-min). Empty
/// culled list → tag-only `#conflict` defensive mint.
pub fn primary_bottom_from_culled(culled: impl IntoIterator<Item = BottomDetail>) -> Value {
    let detail = culled
        .into_iter()
        .min_by_key(|d| d.cause.primary_rank())
        .unwrap_or(BottomDetail {
            cause: BottomCause::Conflict,
            path: None,
            message: None,
            expected: None,
            found: None,
            involved: vec![],
            obstruction_degree: None,
            holonomy: None,
        });
    Value::Bottom(Box::new(detail))
}

/// SPEC_01 join idempotence: flatten nested Unions, drop structural
/// duplicates (PartialEq, first occurrence kept), collapse to a single
/// value when one survivor remains. Does not re-sort (eval `|` order /
/// tropical-weight order of callers preserved among first-seen survivors).
/// Range coalescing is intentionally out of scope.
pub fn normalize_union(branches: impl IntoIterator<Item = Value>) -> Value {
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
    let mut unique: Vec<Value> = Vec::new();
    for b in flat {
        if !unique.iter().any(|u| u == &b) {
            unique.push(b);
        }
    }
    match unique.len() {
        0 => Value::Bottom(Box::new(BottomDetail {
            cause: BottomCause::Conflict,
            path: None,
            message: Some("empty union after normalize".to_string()),
            expected: None,
            found: None,
            involved: vec![],
            obstruction_degree: None,
            holonomy: None,
        })),
        1 => unique.into_iter().next().unwrap(),
        _ => Value::Union(unique),
    }
}

#[derive(Debug, Serialize, Deserialize)]
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
    /// SPEC_03 §3.1 timing (forward_spread 2026-07-19): spread sources held
    /// as Thunks until observation convergence — not expanded at construction.
    /// Empty after expand. Skipped in serde (re-expand from source on load is
    /// not required for in-session evolve/observe; store solidifies via force).
    #[serde(skip, default)]
    pub pending_spreads: Vec<Value>,
    #[serde(skip, default = "default_cache_id")]
    pub cache_id: Arc<RwLock<Option<ContentHash>>>,
    /// M1: memo for `cycle_frame_digest` (force/in_flight frame identity).
    /// Distinct from `cache_id` (store content_hash). Shared via Arc of the
    /// sealed frame so equal content hashes once across thunks.
    #[serde(skip, default = "default_cache_id")]
    pub cycle_frame_id: Arc<RwLock<Option<ContentHash>>>,
    #[serde(skip, default)]
    pub legacy_fields: IndexMap<String, Value>,
    #[serde(skip, default)]
    pub legacy_local: IndexMap<String, Value>,
}

/// Clone must **not** share `cache_id` or `cycle_frame_id`. content_hash /
/// cycle_frame_digest memoize there; a cloned ComboVal is often mutated
/// (Config partials, seal frames) and a shared cache makes unify's CAID
/// early-out treat the post-mutation value as equal to the pre-mutation one
/// — dropping fields (D1: `~%Config.strategy` lost when staged after `fuel`).
impl Clone for ComboVal {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            types: self.types.clone(),
            rules: self.rules.clone(),
            meta: self.meta.clone(),
            system: self.system.clone(),
            local: self.local.clone(),
            closed: self.closed,
            effect: self.effect,
            relations: self.relations.clone(),
            masa_ref: self.masa_ref.clone(),
            pending_spreads: self.pending_spreads.clone(),
            cache_id: default_cache_id(),
            cycle_frame_id: default_cache_id(),
            legacy_fields: self.legacy_fields.clone(),
            legacy_local: self.legacy_local.clone(),
        }
    }
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
            pending_spreads: Vec::new(),
            cache_id: default_cache_id(),
            cycle_frame_id: default_cache_id(),
            legacy_fields: IndexMap::new(),
            legacy_local: IndexMap::new(),
        }
    }
}

fn mark_combo_spans_unknown(combo: &mut ComboVal) {
    for value in combo.data.values_mut() {
        value.mark_spans_unknown();
    }
    for value in combo.types.values_mut() {
        value.mark_spans_unknown();
    }
    for value in combo.rules.values_mut() {
        value.mark_spans_unknown();
    }
    for value in combo.meta.values_mut() {
        value.mark_spans_unknown();
    }
    for value in combo.system.values_mut() {
        value.mark_spans_unknown();
    }
    for value in combo.local.values_mut() {
        value.mark_spans_unknown();
    }
    for value in &mut combo.pending_spreads {
        value.mark_spans_unknown();
    }
}

impl ComboVal {
    pub fn new(
        fields: IndexMap<String, Value>,
        closed: bool,
        local_fields: IndexMap<String, Value>,
        effect: EffectTag,
        relations: Vec<ValRelation>,
    ) -> Self {
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

    pub fn is_pure_wrapper(&self) -> bool {
        self.meta.contains_key("val")
            && self.data.is_empty()
            && self.types.is_empty()
            && self.rules.is_empty()
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
        for (k, v) in &self.data {
            all.insert(k.clone(), v.clone());
        }
        for (k, v) in &self.rules {
            all.insert(format!("/{}", k), v.clone());
        }
        for (k, v) in &self.types {
            all.insert(format!("@{}", k), v.clone());
        }
        for (k, v) in &self.meta {
            all.insert(format!("%{}", k), v.clone());
        }
        for (k, v) in &self.system {
            all.insert(format!("~%{}", k), v.clone());
        }
        all
    }

    pub fn fields_iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.data
            .iter()
            .chain(self.rules.iter().map(|(k, v)| (k, v)))
    }

    pub fn all_fields_iter(&self) -> impl Iterator<Item = (String, Value)> + '_ {
        self.data
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .chain(
                self.rules
                    .iter()
                    .map(|(k, v)| (format!("/{}", k), v.clone())),
            )
            .chain(
                self.types
                    .iter()
                    .map(|(k, v)| (format!("@{}", k), v.clone())),
            )
            .chain(
                self.meta
                    .iter()
                    .map(|(k, v)| (format!("%{}", k), v.clone())),
            )
            .chain(
                self.system
                    .iter()
                    .map(|(k, v)| (format!("~%{}", k), v.clone())),
            )
    }

    pub fn field_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        for k in self.data.keys() {
            keys.push(k.clone());
        }
        for k in self.rules.keys() {
            keys.push(format!("/{}", k));
        }
        for k in self.types.keys() {
            keys.push(format!("@{}", k));
        }
        for k in self.meta.keys() {
            keys.push(format!("%{}", k));
        }
        for k in self.system.keys() {
            keys.push(format!("~%{}", k));
        }
        keys
    }

    pub fn local_fields(&self) -> IndexMap<String, Value> {
        let mut all = IndexMap::new();
        for (k, v) in &self.local {
            all.insert(format!("~{}", k), v.clone());
        }
        all
    }

    pub fn local_keys(&self) -> Vec<String> {
        self.local.keys().map(|k| format!("~{}", k)).collect()
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.get_field(key).is_some()
    }

    pub fn remove_field(&mut self, key: &str) {
        let key_trimmed = key.trim();
        if key_trimmed.starts_with("~%") {
            self.system.shift_remove(&key_trimmed[2..]);
        } else if key_trimmed.starts_with('/') {
            self.rules.shift_remove(&key_trimmed[1..]);
        } else if key_trimmed.starts_with('@') {
            self.types.shift_remove(&key_trimmed[1..]);
        } else if key_trimmed.starts_with('%') {
            self.meta.shift_remove(&key_trimmed[1..]);
        } else if key_trimmed.starts_with('~') {
            self.local.shift_remove(&key_trimmed[1..]);
        } else {
            self.data.shift_remove(key_trimmed);
        }
    }

    /// Names this combo (and every nested combo) projects via `%builtin`.
    /// The dispatch gate (O68 Q3.B) consults this set, not the process registry.
    pub fn collect_projected_builtins(&self) -> HashSet<String> {
        let mut out = HashSet::new();
        collect_projected_builtins_in_combo(self, &mut out);
        out
    }

    /// True when every axis is empty. Format 1/2 load the standard layer as
    /// this shape: self-contained, library inlined on the user root.
    pub fn is_blank(&self) -> bool {
        self.data.is_empty()
            && self.types.is_empty()
            && self.rules.is_empty()
            && self.meta.is_empty()
            && self.system.is_empty()
            && self.local.is_empty()
    }

    /// `%builtin` names under this combo's library axes (`system` / `rules`)
    /// only. Format 1/2 credentials live here; user `data` is not a table
    /// (Q-035 repair 1). Nested combos under those axes are walked in full.
    pub fn collect_projected_builtins_from_library_axes(&self) -> HashSet<String> {
        let mut out = HashSet::new();
        for map in [&self.system, &self.rules] {
            for v in map.values() {
                collect_projected_builtins_in_value(v, &mut out);
            }
        }
        out
    }

    pub fn bits(&self) -> u64 {
        let mut b = 64u64;
        for (k, v) in &self.data {
            b += (k.len() as u64) * 8 + v.bits();
        }
        for (k, v) in &self.rules {
            b += ((k.len() + 1) as u64) * 8 + v.bits();
        }
        for (k, v) in &self.types {
            b += ((k.len() + 1) as u64) * 8 + v.bits();
        }
        for (k, v) in &self.meta {
            b += ((k.len() + 1) as u64) * 8 + v.bits();
        }
        for (k, v) in &self.system {
            b += ((k.len() + 2) as u64) * 8 + v.bits();
        }
        for (k, v) in &self.local {
            b += ((k.len() + 1) as u64) * 8 + v.bits();
        }
        b
    }
}

impl PartialEq for ComboVal {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
            && self.types == other.types
            && self.rules == other.rules
            && self.meta == other.meta
            && self.system == other.system
            && self.local == other.local
            && self.closed == other.closed
            && self.effect == other.effect
            && self.relations == other.relations
            && self.pending_spreads == other.pending_spreads
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelOp {
    Lt,
    Gt,
    Lte,
    Gte,
    Eq,
}

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
    pub fn new(
        cause: BottomCause,
        path: Option<String>,
        message: Option<String>,
        expected: Option<Value>,
        found: Option<Value>,
        involved: Vec<ContentHash>,
    ) -> Self {
        BottomDetail {
            cause,
            path,
            message,
            expected,
            found,
            involved,
            obstruction_degree: None,
            holonomy: None,
        }
    }

    pub fn bits(&self) -> u64 {
        let mut b = 128u64;
        if let Some(ref p) = self.path {
            b += (p.len() as u64) * 8;
        }
        if let Some(ref m) = self.message {
            b += (m.len() as u64) * 8;
        }
        b += (self.involved.len() as u64) * 256;
        if self.obstruction_degree.is_some() {
            b += 64;
        }
        if self.holonomy.is_some() {
            b += 64;
        }
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
            BottomCause::PeerTimeout => "#peer_timeout",
            BottomCause::Divergent => "#divergent",
            BottomCause::InvalidPath => "#invalid_path",
            BottomCause::PrivateAccessViolation => "#private_access_violation",
            BottomCause::NumericalError => "#numerical_error",
            BottomCause::ArithmeticOnAnchor => "#arithmetic_on_anchor",
            BottomCause::H1Split => "#h1_split",
            BottomCause::H2Split => "#h2_split",
            BottomCause::SemanticEclipse => "#semantic_eclipse",
            BottomCause::NoContext => "#no_context",
            BottomCause::OutOfHorizon => "#out_of_horizon",
            BottomCause::SystemReserved => "#system_reserved",
            BottomCause::InvalidConfig => "#invalid_config",
            BottomCause::EffectViolation => "#effect_violation",
            BottomCause::PrivilegedRequired => "#privileged_required",
            BottomCause::StoreBoundary => "#store_boundary",
            BottomCause::CaidMismatch => "#caid_mismatch",
            BottomCause::PeerNotImplemented => "#peer_not_implemented",
            BottomCause::PeerUnknownStatus => "#peer_unknown_status",
            BottomCause::PeerRefused => "#peer_refused",
            BottomCause::RoutingBudgetExceeded => "#routing_budget_exceeded",
            BottomCause::MaxDepthExceeded => "#max_depth_exceeded",
            BottomCause::StackOverflow => "#stack_overflow",
            BottomCause::ObjectUndecodable => "#object_undecodable",
            BottomCause::StandardRootUnavailable => "#standard_root_unavailable",
            BottomCause::NoStandardRoot => "#no_standard_root",
            BottomCause::UnprojectedBuiltin => "#unprojected_builtin",
            BottomCause::UnprovidedBuiltin => "#unprovided_builtin",
        };
        // F2 (REAL_04 §1 / SYNTAX_08 §4 #3): %cause is a Cocoon whose duality
        // core is %val = the cause tag. Direct observation collapses via G6
        // value-context projection; <<path>> keeps the full chain.
        // Fossil %type twin removed (cocoon_shape arc 2026-07-19).
        fields.insert(
            "%val".to_string(),
            Value::Atom(
                AtomKind::Tag(type_tag[1..].to_string()),
                EffectTag::Pure,
                None,
            ),
        );
        // Non-empty data axis so lattice unify does not treat this as a pure
        // wrapper and peel to the bare tag during evolve field-merge (which
        // would erase the cocoon before `m.%val` can navigate). Collapsed
        // observation still peels %val (project_value_context). Engine
        // scaffolding: stripped at display (to_nlang) — never user-visible.
        fields.insert("_".to_string(), Value::Top);
        if let Some(ref p) = self.path {
            fields.insert(
                "%path".to_string(),
                Value::Atom(AtomKind::Str(p.clone()), EffectTag::Pure, None),
            );
        }
        if let Some(ref m) = self.message {
            fields.insert(
                "%message".to_string(),
                Value::Atom(AtomKind::Str(m.clone()), EffectTag::Pure, None),
            );
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
                involved_fields.insert(
                    i.to_string(),
                    Value::Atom(AtomKind::Str(h.to_string()), EffectTag::Pure, None),
                );
            }
            involved_fields.insert(
                "%kind".to_string(),
                Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
            );
            fields.insert(
                "%involved".to_string(),
                Value::Combo(ComboVal::new(
                    involved_fields,
                    false,
                    IndexMap::new(),
                    EffectTag::Pure,
                    vec![],
                )),
            );
        }

        // Phase NEW: cocycle format (SPEC_06 §1.3.2)
        if let Some(degree) = self.obstruction_degree {
            fields.insert(
                "%degree".to_string(),
                Value::Atom(AtomKind::Int(BigInt::from(degree)), EffectTag::Pure, None),
            );
            let obs_tag = match degree {
                1 => "h1_phase",
                2 => "h2_sign",
                3 => "h3_gerbe",
                4 => "h4_sybil",
                _ => "unknown",
            };
            fields.insert(
                "%obstruction".to_string(),
                Value::Atom(AtomKind::Tag(obs_tag.to_string()), EffectTag::Pure, None),
            );

            // %cocycle: build from involved
            if !self.involved.is_empty() {
                let mut cyc = IndexMap::new();
                cyc.insert(
                    "%kind".to_string(),
                    Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
                );
                for (i, h) in self.involved.iter().enumerate() {
                    cyc.insert(
                        i.to_string(),
                        Value::Atom(AtomKind::Str(h.to_string()), EffectTag::Pure, None),
                    );
                }
                // H²: pad to 4 positions (spec requires 4-cycle)
                if degree == 2 && self.involved.len() == 2 {
                    cyc.insert(
                        "2".to_string(),
                        Value::Atom(AtomKind::Tag("_".to_string()), EffectTag::Pure, None),
                    );
                    cyc.insert(
                        "3".to_string(),
                        Value::Atom(AtomKind::Tag("_".to_string()), EffectTag::Pure, None),
                    );
                }
                fields.insert(
                    "%cocycle".to_string(),
                    Value::Combo(ComboVal::new(
                        cyc,
                        false,
                        IndexMap::new(),
                        EffectTag::Pure,
                        vec![],
                    )),
                );
            }

            // %holonomy
            if let Some(ref h) = self.holonomy {
                let hv = match h {
                    Holonomy::Phase(theta) => {
                        Value::Atom(AtomKind::Float(*theta), EffectTag::Pure, None)
                    }
                    Holonomy::NegI => {
                        Value::Atom(AtomKind::Tag("neg_I".to_string()), EffectTag::Pure, None)
                    }
                };
                fields.insert("%holonomy".to_string(), hv);
            }

            // %branches: H²
            if degree == 2 {
                fields.insert(
                    "%branches".to_string(),
                    Value::Atom(AtomKind::Int(2u8.into()), EffectTag::Pure, None),
                );
            }
        }

        Value::Combo(ComboVal::new(
            fields,
            true,
            IndexMap::new(),
            EffectTag::Pure,
            vec![],
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
/// Append-only (fmt v2 freeze). New causes go at the tail only.
/// `InvalidPath` is retained for stored-universe decode; minting has stopped
/// (F4 abolition, 2026-07-14).
pub enum BottomCause {
    #[default]
    Conflict,
    MissingKey,
    FuelExhausted,
    Timeout,
    Divergent,
    InvalidPath,
    PrivateAccessViolation,
    NumericalError,
    ArithmeticOnAnchor,
    H1Split,
    H2Split,
    SemanticEclipse,
    NoContext,
    /// `^` parent-anchor overflow (TAG_REGISTRY §1 #out_of_horizon).
    OutOfHorizon,
    /// User LHS write to engine-minted `~%` system axis (SPEC_09 ownership;
    /// TAG_REGISTRY §1.4 #system_reserved). Append-only tail.
    SystemReserved,
    /// Root `~%Config.<bare>` name/type violation (SPEC_09 §6 closed knob
    /// family; TAG_REGISTRY #invalid_config). Evolve-boundary named error —
    /// never a node-level ⊥. Append-only tail (fmt discipline).
    InvalidConfig,
    /// Declared `%effect: #pure` contradicted by active contagion
    /// (SPEC_08 §4.3; TAG_REGISTRY #effect_violation). Append-only tail.
    EffectViolation,
    /// Privileged op invoked without horizon privilege (SPEC_08 §6.1.2;
    /// TAG_REGISTRY #privileged_required). Append-only tail.
    PrivilegedRequired,
    /// Filesystem access from the language layer to a path inside the engine
    /// store (`.oo`). Unconditional — no capability unlocks it.
    /// SPEC_08 §6.3; TAG_REGISTRY #store_boundary. Append-only tail.
    StoreBoundary,
    /// Content address does not match requested CAID (REAL_03 §6.6 peer/store
    /// read path). Append-only tail. Distinct from absence (`#conflict` /
    /// `#missing_key` at the language surface for plain miss).
    CaidMismatch,
    /// A peer accepted the connection and did not answer within the read
    /// deadline (REAL_02 §3.2). Append-only tail.
    ///
    /// ACCEPTOR REPAIR (oodp_packet_format): the delivery reused `Timeout`,
    /// which pre-exists for `ResourceExhausted::Timeout` — a COMPUTATION that
    /// outran `%timeout` (`observation.rs:63`). TAG_REGISTRY gives `#timeout`
    /// the remedy 「請優化性能、減少嵌套,或放寬時間限制」, which is not merely
    /// unhelpful for a silent peer, it points the reader at their own code.
    /// An arc whose entire thesis is that four situations must be separable
    /// cannot ship a fifth that is not.
    PeerTimeout,
    /// Peer answered `#not_implemented` (wire_says_why). Not an integrity incident.
    PeerNotImplemented,
    /// Peer answered a `%status` this client does not recognise (forward compat).
    PeerUnknownStatus,
    /// Peer answered `#conflict` without a substantiated integrity reason
    /// (older peer, or non-integrity conflict). Not an integrity incident.
    PeerRefused,
    /// disc.find hop budget exhausted (TAG_REGISTRY §2.7.1). Append-only tail.
    /// Replaces minting `#semantic_eclipse` for a spent routing budget —
    /// that tag named an attack; the remedy for a far peer is budget/topology,
    /// not eclipse defence. `#semantic_eclipse` remains readable for stored
    /// universes (same retention as `#invalid_path`).
    RoutingBudgetExceeded,
    /// Unification / observation depth budget exhausted (TAG_REGISTRY §2.7.2).
    /// Append-only tail. Distinct from `#fuel_exhausted` — different knob.
    MaxDepthExceeded,
    /// Implementation recursion ceiling (W4‴). Append-only tail.
    /// Incapacity — the native stack cannot go further — not the operator's
    /// policy (`#max_depth_exceeded`). Never minted as `#blur`.
    StackOverflow,
    /// Object present but cannot be deserialized (REAL_03 §6.6;
    /// TAG_REGISTRY #object_undecodable). Append-only tail. Distinct from
    /// absence (`#missing_key`) and from a decoded-but-lying object
    /// (`#caid_mismatch`).
    ObjectUndecodable,
    /// Object is held, but its format-3 standard root is not shipped by
    /// this engine (REAL_03 §6.8). Append-only tail. Distinct from absence.
    StandardRootUnavailable,
    /// Eval context never installed a standard root (O68 Q3.B / Q-035 S2).
    /// Distinct from an installed root that does not project the name, and
    /// from `#standard_root_unavailable` (a held object names a digest this
    /// engine does not ship). Append-only tail.
    NoStandardRoot,
    /// This universe's standard root does not project the `%builtin` name
    /// (O68 Q3.B / Q-035 S1). Append-only tail.
    UnprojectedBuiltin,
    /// The standard root projects the `%builtin` name, but this engine's
    /// registry cannot provide it (O68 Q3.B / Q-035 S3, the six dead names).
    /// Append-only tail.
    UnprovidedBuiltin,
}

impl BottomCause {
    pub fn as_tag(&self) -> &str {
        match self {
            BottomCause::Conflict => "conflict",
            BottomCause::MissingKey => "missing_key",
            BottomCause::FuelExhausted => "fuel_exhausted",
            BottomCause::Timeout => "timeout",
            BottomCause::PeerTimeout => "peer_timeout",
            BottomCause::Divergent => "divergent",
            BottomCause::InvalidPath => "invalid_path",
            BottomCause::PrivateAccessViolation => "private_access_violation",
            BottomCause::NumericalError => "numerical_error",
            BottomCause::ArithmeticOnAnchor => "arithmetic_on_anchor",
            BottomCause::H1Split => "h1_split",
            BottomCause::H2Split => "h2_split",
            BottomCause::SemanticEclipse => "semantic_eclipse",
            BottomCause::NoContext => "no_context",
            BottomCause::OutOfHorizon => "out_of_horizon",
            BottomCause::SystemReserved => "system_reserved",
            BottomCause::InvalidConfig => "invalid_config",
            BottomCause::EffectViolation => "effect_violation",
            BottomCause::PrivilegedRequired => "privileged_required",
            BottomCause::StoreBoundary => "store_boundary",
            BottomCause::CaidMismatch => "caid_mismatch",
            BottomCause::PeerNotImplemented => "peer_not_implemented",
            BottomCause::PeerUnknownStatus => "peer_unknown_status",
            BottomCause::PeerRefused => "peer_refused",
            BottomCause::RoutingBudgetExceeded => "routing_budget_exceeded",
            BottomCause::MaxDepthExceeded => "max_depth_exceeded",
            BottomCause::StackOverflow => "stack_overflow",
            BottomCause::ObjectUndecodable => "object_undecodable",
            BottomCause::StandardRootUnavailable => "standard_root_unavailable",
            BottomCause::NoStandardRoot => "no_standard_root",
            BottomCause::UnprojectedBuiltin => "unprojected_builtin",
            BottomCause::UnprovidedBuiltin => "unprovided_builtin",
        }
    }

    /// REAL_04 §4 primary-cause priority for multi-branch collapse
    /// (lower rank = more primary). Used when union navigation culls all
    /// branches to ⊥.
    pub fn primary_rank(self) -> u8 {
        match self {
            BottomCause::Divergent => 0,
            BottomCause::PrivateAccessViolation
            | BottomCause::SystemReserved
            | BottomCause::InvalidConfig
            | BottomCause::EffectViolation
            | BottomCause::PrivilegedRequired
            | BottomCause::StoreBoundary
            | BottomCause::CaidMismatch
            | BottomCause::ObjectUndecodable
            | BottomCause::StandardRootUnavailable
            | BottomCause::NoStandardRoot
            | BottomCause::UnprojectedBuiltin
            | BottomCause::UnprovidedBuiltin => 1,
            BottomCause::Conflict
            | BottomCause::H1Split
            | BottomCause::H2Split
            | BottomCause::SemanticEclipse
            | BottomCause::NumericalError
            | BottomCause::ArithmeticOnAnchor
            | BottomCause::NoContext => 2,
            BottomCause::FuelExhausted
            | BottomCause::Timeout
            | BottomCause::PeerTimeout
            | BottomCause::PeerNotImplemented
            | BottomCause::PeerUnknownStatus
            | BottomCause::PeerRefused
            | BottomCause::OutOfHorizon
            | BottomCause::RoutingBudgetExceeded
            | BottomCause::MaxDepthExceeded
            | BottomCause::StackOverflow => 3,
            BottomCause::MissingKey | BottomCause::InvalidPath => 4,
        }
    }
}

impl From<BottomCause> for Value {
    fn from(cause: BottomCause) -> Self {
        Value::Bottom(Box::new(BottomDetail {
            cause,
            path: None,
            message: None,
            expected: None,
            found: None,
            involved: vec![],
            obstruction_degree: None,
            holonomy: None,
        }))
    }
}

/// Recover the value that was applied to a morphism (caid_of_the_argument).
///
/// `apply_morphism` sets `%arg` **only** when it wraps a non-pack argument into
/// a positional pack. Whole-argument builtins (identify, save, advertise, …)
/// must read slot `0` in that case and the pack itself otherwise — never
/// unconditional `get_field("0")`, which silently drops tuples and slot-0 combos.
fn collect_projected_builtins_in_combo(c: &ComboVal, out: &mut HashSet<String>) {
    if let Some(Value::Atom(AtomKind::Str(id), _, _)) = c.meta.get("builtin") {
        out.insert(id.clone());
    }
    for map in [&c.data, &c.types, &c.rules, &c.meta, &c.system, &c.local] {
        for v in map.values() {
            collect_projected_builtins_in_value(v, out);
        }
    }
}

fn collect_projected_builtins_in_value(v: &Value, out: &mut HashSet<String>) {
    match v {
        Value::Combo(c) => collect_projected_builtins_in_combo(c, out),
        Value::Union(vs) => {
            for x in vs {
                collect_projected_builtins_in_value(x, out);
            }
        }
        Value::Range { start, end, step } => {
            collect_projected_builtins_in_value(start, out);
            collect_projected_builtins_in_value(end, out);
            if let Some(s) = step {
                collect_projected_builtins_in_value(s, out);
            }
        }
        _ => {}
    }
}

impl Value {
    /// True if this value, or any nested combo, carries `meta.builtin`.
    /// Display-time query for O68 Q4.C; does not force thunks and does not
    /// rewrite the value.
    pub fn holds_meta_builtin(&self) -> bool {
        match self {
            Value::Combo(c) => {
                c.meta.contains_key("builtin")
                    || c.data.values().any(Value::holds_meta_builtin)
                    || c.types.values().any(Value::holds_meta_builtin)
                    || c.rules.values().any(Value::holds_meta_builtin)
                    || c.meta.values().any(Value::holds_meta_builtin)
                    || c.system.values().any(Value::holds_meta_builtin)
                    || c.local.values().any(Value::holds_meta_builtin)
            }
            Value::Union(vs) => vs.iter().any(Value::holds_meta_builtin),
            Value::Range { start, end, step } => {
                start.holds_meta_builtin()
                    || end.holds_meta_builtin()
                    || step.as_ref().is_some_and(|s| s.holds_meta_builtin())
            }
            Value::Blur(b) => b
                .partial_body
                .as_ref()
                .is_some_and(|body| body.holds_meta_builtin()),
            _ => false,
        }
    }
}

pub fn whole_argument(arg: Value) -> Value {
    match &arg {
        Value::Combo(c) if c.contains_key("%arg") => c.get_field("0").cloned().unwrap_or(arg),
        _ => arg,
    }
}

// ── ObservationStrategy (moved from observation.rs to break circular dep) ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObservationStrategy {
    Blur,
    Strict,
    Approximate,
}

impl Default for ObservationStrategy {
    fn default() -> Self {
        ObservationStrategy::Blur
    }
}

// ── Blur types (Phase 9: first-class #blur) ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BlurCause {
    FuelExhausted,
    Timeout,
    StackOverflow,
    MathSingularity(String),
    /// Depth budget exhausted (TAG_REGISTRY §2.7.2). Append-only tail.
    /// Enters blur CAID via `as_bytes` (bn_serial path). StackOverflow is
    /// retained for CAID table / stored decode but is no longer minted by
    /// `handle_resource_exhausted`.
    MaxDepthExceeded,
}

impl BlurCause {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            BlurCause::FuelExhausted => b"fuel_exhausted",
            BlurCause::Timeout => b"timeout",
            BlurCause::StackOverflow => b"stack_overflow",
            BlurCause::MathSingularity(s) => s.as_bytes(),
            BlurCause::MaxDepthExceeded => b"max_depth_exceeded",
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            BlurCause::FuelExhausted => "fuel_exhausted",
            BlurCause::Timeout => "timeout",
            BlurCause::StackOverflow => "stack_overflow",
            BlurCause::MathSingularity(s) => s.as_str(),
            BlurCause::MaxDepthExceeded => "max_depth_exceeded",
        }
    }
}

/// Horizon budgets that enter blur identity (REAL_03 §7.3 CHS / O42).
///
/// `fuel` is the **ceiling** (allowed budget), not remaining. `fuel_remaining`
/// is runtime provenance only — never hashed (R-2). No salt (R-1). No timeout
/// (spec forbids non-discrete horizon params in CAID).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HorizonParams {
    /// Budget ceiling (`%fuel` 上限). Identity.
    pub fuel: u64,
    /// Runtime remaining — NOT in CAID.
    pub fuel_remaining: u64,
    pub strategy: ObservationStrategy,
    pub max_branches: u64,
    pub max_unification_depth: u64,
    pub max_lifting_depth: u64,
    pub max_pattern_nodes: u64,
}

impl HorizonParams {
    pub fn strategy_byte(&self) -> u8 {
        match self.strategy {
            ObservationStrategy::Blur => 0,
            ObservationStrategy::Strict => 1,
            ObservationStrategy::Approximate => 2,
        }
    }

    /// Identity-relevant bytes (no fuel_remaining).
    pub fn encode_chs(&self, hasher: &mut Sha256) {
        hasher.update(b":fuel=");
        hasher.update(&self.fuel.to_le_bytes());
        hasher.update(b":strategy=");
        hasher.update(&[self.strategy_byte()]);
        hasher.update(b":max_branches=");
        hasher.update(&self.max_branches.to_le_bytes());
        hasher.update(b":max_unification_depth=");
        hasher.update(&self.max_unification_depth.to_le_bytes());
        hasher.update(b":max_lifting_depth=");
        hasher.update(&self.max_lifting_depth.to_le_bytes());
        hasher.update(b":max_pattern_nodes=");
        hasher.update(&self.max_pattern_nodes.to_le_bytes());
    }
}

/// One horizon hit: cause + six CHS params + node_content as its CAID.
///
/// O42 repair (11.5): `node_content` enters identity via content-addressing.
/// The body is **not** inlined into staged JSON — only its CAID is. The body
/// is held ephemerally (`partial_body`) until `save_staged` writes it to CAS
/// (11.6.1 (i)); reload recovers it with `store.get_value` when needed (O45).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HorizonRecord {
    pub cause: BlurCause,
    pub horizon: HorizonParams,
    /// CAID of node_content, or none.
    pub partial: Option<ContentHash>,
    /// Ephemeral body for CAS write on evolve. Never serialized.
    #[serde(skip)]
    pub partial_body: Option<Box<Value>>,
}

impl HorizonRecord {
    /// Canonical key for set ordering / dedupe (R-6).
    pub fn chs_digest(&self) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(b"rec:");
        hasher.update(self.cause.as_bytes());
        self.horizon.encode_chs(&mut hasher);
        hasher.update(b":partial=");
        match &self.partial {
            None => hasher.update(b"none"),
            // node_content participates as its content-address (11.5).
            Some(h) => hasher.update(&h.digest),
        }
        hasher.finalize().to_vec()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlurDetail {
    /// Primary record (REAL_04 §4 projection for `%cause` / display).
    pub cause: BlurCause,
    pub horizon: HorizonParams,
    /// CAID of node_content (not the tree itself).
    pub partial: Option<ContentHash>,
    /// Ephemeral body until CAS write on evolve (11.6.1).
    #[serde(skip)]
    pub partial_body: Option<Box<Value>>,
    pub effect: EffectTag,
    /// Co-horizon records from merges (O46). The full set is
    /// `{primary} ∪ co_horizons`, canonically ordered for CHS.
    #[serde(default)]
    pub co_horizons: Vec<HorizonRecord>,
}

impl BlurDetail {
    pub fn from_single(
        cause: BlurCause,
        horizon: HorizonParams,
        partial: Option<Value>,
        effect: EffectTag,
    ) -> Self {
        let (partial, partial_body) = match partial {
            None => (None, None),
            Some(v) => {
                let h = v.content_hash();
                (Some(h), Some(Box::new(v)))
            }
        };
        Self {
            cause,
            horizon,
            partial,
            partial_body,
            effect,
            co_horizons: Vec::new(),
        }
    }

    pub fn primary_record(&self) -> HorizonRecord {
        HorizonRecord {
            cause: self.cause.clone(),
            horizon: self.horizon.clone(),
            partial: self.partial.clone(),
            partial_body: self.partial_body.clone(),
        }
    }

    /// All records as a canonically ordered set (R-6).
    pub fn record_set(&self) -> Vec<HorizonRecord> {
        let mut recs = Vec::with_capacity(1 + self.co_horizons.len());
        recs.push(self.primary_record());
        recs.extend(self.co_horizons.iter().cloned());
        recs.sort_by(|a, b| a.chs_digest().cmp(&b.chs_digest()));
        recs.dedup_by(|a, b| a.chs_digest() == b.chs_digest());
        recs
    }

    /// REAL_03 §7.3 CHS envelope over the record set (R-3/R-4/R-5).
    /// Single function used by `%caid`, bn_serial 0xFD, and recursive hash.
    pub fn blur_caid(&self) -> ContentHash {
        let mut hasher = Sha256::new();
        hasher.update(b"blur:chs:v1:");
        for rec in self.record_set() {
            hasher.update(&rec.chs_digest());
        }
        ContentHash::v1(hasher.finalize().to_vec())
    }

    /// Merge two blurs as a set of records (O46). Not meet, not ordered tuple.
    pub fn merge_set(a: &BlurDetail, b: &BlurDetail) -> BlurDetail {
        let mut recs = a.record_set();
        recs.extend(b.record_set());
        recs.sort_by(|x, y| x.chs_digest().cmp(&y.chs_digest()));
        recs.dedup_by(|x, y| x.chs_digest() == y.chs_digest());
        // Primary = REAL_04-ish: prefer lower cause-name for stable projection
        // among equal rank; fuel exhaustion ranks with timeout class.
        let primary_idx = recs
            .iter()
            .enumerate()
            .min_by(|(_, r1), (_, r2)| {
                blur_cause_primary_key(&r1.cause).cmp(&blur_cause_primary_key(&r2.cause))
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        let primary = recs.remove(primary_idx);
        let effect = a.effect.union(b.effect);
        BlurDetail {
            cause: primary.cause,
            horizon: primary.horizon,
            partial: primary.partial,
            partial_body: primary.partial_body,
            effect,
            co_horizons: recs,
        }
    }

    /// Write any held partial bodies into the object store (11.6.1 (i)).
    pub fn persist_partials(&self, store: &crate::storage::ObjectStore) -> anyhow::Result<()> {
        if let Some(body) = &self.partial_body {
            store.put_value(body)?;
        }
        for rec in &self.co_horizons {
            if let Some(body) = &rec.partial_body {
                store.put_value(body)?;
            }
        }
        Ok(())
    }
}

impl Value {
    /// Clone a value into the typed format-2 CAS projection: all and only AST
    /// source coordinates become `Span::unknown()`, which AST serde omits.
    /// User Combo keys are ordinary `String`s and are never inspected here.
    pub fn for_cas_storage(&self) -> Self {
        let mut projected = self.clone();
        projected.mark_spans_unknown();
        projected.materialize_effect_fields();
        projected
    }

    /// Encoding 1--3 projection, before O61 made propagated effects durable.
    /// The container format selects this path; using the current projection
    /// while retaining an old encoding declaration would make the declaration
    /// lie and would move addresses in an existing store.
    pub(crate) fn for_legacy_cas_storage(&self) -> Self {
        let mut projected = self.clone();
        projected.mark_spans_unknown();
        projected
    }

    /// The propagated effect is runtime state, but durable identity records it
    /// as the ordinary hashed `%effect` meta field.  An absent field is the
    /// canonical spelling of `#pure`; every other tag, including `#cached`,
    /// is written down.
    fn materialize_effect_fields(&mut self) {
        match self {
            Value::Combo(combo) => {
                for value in combo
                    .data
                    .values_mut()
                    .chain(combo.types.values_mut())
                    .chain(combo.rules.values_mut())
                    .chain(combo.meta.values_mut())
                    .chain(combo.system.values_mut())
                    .chain(combo.local.values_mut())
                    .chain(combo.pending_spreads.iter_mut())
                {
                    value.materialize_effect_fields();
                }
                if combo.effect.is_pure() {
                    if matches!(combo.meta.get("effect"), Some(Value::Atom(AtomKind::Tag(tag), _, _)) if tag == "pure") {
                        combo.meta.shift_remove("effect");
                    }
                } else {
                    combo.meta.insert("effect".to_string(), effect_tag_value(combo.effect));
                }
            }
            Value::Union(values) => {
                for value in values {
                    value.materialize_effect_fields();
                }
            }
            Value::Range { start, end, step } => {
                start.materialize_effect_fields();
                end.materialize_effect_fields();
                if let Some(step) = step {
                    step.materialize_effect_fields();
                }
            }
            Value::Blur(blur) => {
                if let Some(body) = blur.partial_body.as_mut() {
                    body.materialize_effect_fields();
                }
                for horizon in &mut blur.co_horizons {
                    if let Some(body) = horizon.partial_body.as_mut() {
                        body.materialize_effect_fields();
                    }
                }
            }
            Value::Thunk { closure, context, .. } => {
                for frame in closure {
                    let mut value = Value::Combo(std::sync::Arc::make_mut(frame).clone());
                    value.materialize_effect_fields();
                    let Value::Combo(combo) = value else { unreachable!() };
                    *std::sync::Arc::make_mut(frame) = combo;
                }
                if let Some(context) = context {
                    context.materialize_effect_fields();
                }
            }
            _ => {}
        }
    }

    fn mark_spans_unknown(&mut self) {
        match self {
            Value::Combo(combo) => mark_combo_spans_unknown(combo),
            Value::Union(items) => {
                for item in items {
                    item.mark_spans_unknown();
                }
            }
            Value::Code(expr) => expr.mark_spans_unknown(),
            Value::Thunk {
                expr,
                closure,
                context,
                ..
            } => {
                expr.mark_spans_unknown();
                for frame in closure {
                    mark_combo_spans_unknown(std::sync::Arc::make_mut(frame));
                }
                if let Some(context) = context {
                    context.mark_spans_unknown();
                }
            }
            Value::Ref(path) => path.span = Span::unknown(),
            Value::Bottom(detail) => {
                if let Some(expected) = &mut detail.expected {
                    expected.mark_spans_unknown();
                }
                if let Some(found) = &mut detail.found {
                    found.mark_spans_unknown();
                }
            }
            Value::Range { start, end, step } => {
                start.mark_spans_unknown();
                end.mark_spans_unknown();
                if let Some(step) = step {
                    step.mark_spans_unknown();
                }
            }
            Value::Top | Value::TopCaused { .. } | Value::Atom(..) | Value::Blur(_) => {}
        }
    }

    /// Walk a value tree and CAS-write every blur partial body (evolve path).
    pub fn persist_blur_partials(&self, store: &crate::storage::ObjectStore) -> anyhow::Result<()> {
        match self {
            Value::Blur(bd) => bd.persist_partials(store)?,
            Value::Combo(c) => {
                for (_, v) in c.all_fields_iter() {
                    v.persist_blur_partials(store)?;
                }
                for v in c.local.values() {
                    v.persist_blur_partials(store)?;
                }
                for v in &c.pending_spreads {
                    v.persist_blur_partials(store)?;
                }
            }
            Value::Union(branches) => {
                for b in branches {
                    b.persist_blur_partials(store)?;
                }
            }
            Value::Range { start, end, step } => {
                start.persist_blur_partials(store)?;
                end.persist_blur_partials(store)?;
                if let Some(s) = step {
                    s.persist_blur_partials(store)?;
                }
            }
            Value::Thunk { context, .. } => {
                if let Some(c) = context {
                    c.persist_blur_partials(store)?;
                }
            }
            Value::Bottom(d) => {
                if let Some(v) = &d.found {
                    v.persist_blur_partials(store)?;
                }
                if let Some(v) = &d.expected {
                    v.persist_blur_partials(store)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn blur_cause_primary_key(c: &BlurCause) -> (u8, &str) {
    // Lower first — align roughly with BottomCause ranks; single string tiebreak.
    let rank = match c {
        BlurCause::StackOverflow => 0,
        BlurCause::FuelExhausted | BlurCause::Timeout | BlurCause::MaxDepthExceeded => 1,
        BlurCause::MathSingularity(_) => 2,
    };
    (rank, c.as_str())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HashAlgorithm {
    Sha256,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CaidVersion {
    V1,
    V2,
}

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
            CaidVersion::V2 => write!(
                f,
                "hash:{}:v2:{}:{}:{}",
                algo, self.masa_ref, self.lattice_sketch, digest_hex
            ),
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
        fn parse_sha256_digest(hex_digest: &str) -> anyhow::Result<Vec<u8>> {
            let digest = hex::decode(hex_digest)?;
            if digest.len() != 32 {
                anyhow::bail!("Invalid sha256 CAID digest length: expected 32 bytes, got {}", digest.len());
            }
            Ok(digest)
        }
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
                digest: parse_sha256_digest(parts[3])?,
            }),
            "v2" => {
                if parts.len() < 6 {
                    return Err(anyhow::anyhow!(
                        "Invalid v2 CAID: needs 6 colon-delimited parts"
                    ));
                }
                let masa_ref = if parts[3] == "_" {
                    MasaRef::Top
                } else {
                    MasaRef::Digest(hex::decode(parts[3])?)
                };
                Ok(ContentHash {
                    algorithm: HashAlgorithm::Sha256,
                    version: CaidVersion::V2,
                    masa_ref,
                    lattice_sketch: parts[4].to_string(),
                    digest: parse_sha256_digest(parts[5])?,
                })
            }
            _ => Err(anyhow::anyhow!("Unknown CAID version: {}", parts[2])),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct CommitMeta {
    pub author: Option<String>,
    pub timestamp: u64,
    pub message: Option<String>,
    /// Heads abandoned by `#rollback` and recorded on the *next* commit
    /// (ruling R1, history_ops). Never stored in values. `None` for ordinary
    /// commits — serde skip keeps old objects bit-compatible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abandoned: Option<Vec<String>>,
    /// SPEC_08 §6.2 `#privileged_effect`: this commit fixed privileged-
    /// discharged content into history. Statement about the *commit*, not
    /// every coordinate. `None` omitted from serde/Debug so ordinary commit
    /// digests stay bit-stable (`Commit::content_hash` formats meta via Debug).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privileged_effect: Option<bool>,
}

impl Default for CommitMeta {
    fn default() -> Self {
        Self {
            author: None,
            timestamp: 0,
            message: None,
            abandoned: None,
            privileged_effect: None,
        }
    }
}

/// Custom Debug so `Commit::content_hash` (which formats meta via `Debug`)
/// stays bit-stable when optional audit fields are absent. Adding a field
/// under derive(Debug) would change every historical commit digest.
impl fmt::Debug for CommitMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ds = f.debug_struct("CommitMeta");
        ds.field("author", &self.author)
            .field("timestamp", &self.timestamp)
            .field("message", &self.message);
        if self.abandoned.is_some() {
            ds.field("abandoned", &self.abandoned);
        }
        if self.privileged_effect.is_some() {
            ds.field("privileged_effect", &self.privileged_effect);
        }
        ds.finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitKind {
    Refine,
    /// SPEC_08 §6.2 privileged overwrite — audit on the commit, never in the value.
    Pin,
    /// SPEC_08 §6.2 privileged history compression — the fact of removal.
    Squash,
    #[serde(other)]
    Standard,
}
impl Default for CommitKind {
    fn default() -> Self {
        Self::Standard
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthorityInfo {
    pub signer_pubkey_hex: String,
    pub signature_hex: String,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefineInfo {
    pub source_caids: Vec<ContentHash>,
    pub target_caids: Vec<ContentHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority: Option<AuthorityInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shadow_affected: Vec<ContentHash>,
    /// Whether refine authority was cryptographically verified against a
    /// non-empty architect registry (`"verified"`) or proceeded under
    /// bootstrap exemption (`"unverified"`). Recorded so history is not a
    /// silent lying audit surface (universe_determinism ruling C).
    /// Not hashed into the commit CAID beyond source/target digests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority_status: Option<String>,
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
            meta: CommitMeta::default(),
            kind: CommitKind::Standard,
            refine_info: None,
            cache_id: default_cache_id(),
        }
    }
}

impl PartialEq for Commit {
    fn eq(&self, other: &Self) -> bool {
        self.parent == other.parent
            && self.root == other.root
            && self.meta == other.meta
            && self.kind == other.kind
            && self.refine_info == other.refine_info
    }
}

impl Commit {
    pub fn new(parent: Option<ContentHash>, root: ContentHash, meta: CommitMeta) -> Self {
        Self {
            parent,
            root,
            meta,
            kind: CommitKind::Standard,
            refine_info: None,
            cache_id: default_cache_id(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Identity {
    pub public_key: Vec<u8>,
    /// PKCS#8 DER bytes of the private key (the only form written to disk).
    pub private_key: Vec<u8>,
}

impl Identity {
    /// Ephemeral key for in-memory engines and tests. Never written to disk.
    pub fn new_random() -> Self {
        let rng = rand::SystemRandom::new();
        let pkcs8_bytes = signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let key_pair = signature::Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref()).unwrap();
        Self {
            public_key: key_pair.public_key().as_ref().to_vec(),
            private_key: pkcs8_bytes.as_ref().to_vec(),
        }
    }

    /// Resolve the operator identity path: `OO_IDENTITY` (must be absolute) or
    /// `~/.oo/identity`. A secret must not live inside a shareable `.oo/` workspace.
    pub fn resolve_path() -> anyhow::Result<std::path::PathBuf> {
        if let Ok(p) = std::env::var("OO_IDENTITY") {
            let path = std::path::PathBuf::from(p);
            if !path.is_absolute() {
                anyhow::bail!(
                    "OO_IDENTITY must be an absolute path (got {})",
                    path.display()
                );
            }
            return Ok(path);
        }
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| anyhow::anyhow!("cannot resolve home directory for ~/.oo/identity"))?;
        Ok(std::path::PathBuf::from(home).join(".oo").join("identity"))
    }

    /// Load PKCS#8 from `path`. On parse failure the file is left untouched.
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)
            .map_err(|e| anyhow::anyhow!("identity file {}: read failed: {}", path.display(), e))?;
        Self::from_pkcs8(&bytes).map_err(|e| {
            anyhow::anyhow!(
                "identity file {}: not a valid PKCS#8 Ed25519 key ({}); file left unchanged",
                path.display(),
                e
            )
        })
    }

    pub fn from_pkcs8(bytes: &[u8]) -> anyhow::Result<Self> {
        let key_pair =
            signature::Ed25519KeyPair::from_pkcs8(bytes).map_err(|e| anyhow::anyhow!("{:?}", e))?;
        Ok(Self {
            public_key: key_pair.public_key().as_ref().to_vec(),
            private_key: bytes.to_vec(),
        })
    }

    /// Write PKCS#8 to `path`, **claiming it atomically**. Never replaces an
    /// existing file: `AlreadyExists` is the signal that somebody else won.
    ///
    /// ACCEPTOR REPAIR (identity_persistence). Measured on the delivered
    /// build, three rounds of eight concurrent first-mints:
    ///
    ///   8 processes → 3 distinct keys minted, and 2 of the 8 printed a
    ///   public key that is NOT the one left in the file.
    ///
    /// The operator is shown key X, declares X out of band, and the engine
    /// signs with Y forever after — the declaration silently never takes
    /// effect and surfaces much later as "signer not in architect_registry",
    /// pointing at the wrong thing. That is R6's defect (a printed key that
    /// is not the signing key) wearing a race condition.
    ///
    /// Two causes, both removed here:
    ///   * `tmp + rename` replaces unconditionally, so every racer's rename
    ///     succeeded and the last one won while each returned its own key;
    ///   * the tmp path was a fixed, predictable name that no racer owned,
    ///     and — being neither `.oo` nor the identity path — the language
    ///     layer could read the private key out of it (measured: `#none`,
    ///     i.e. permitted, merely absent).
    ///
    /// `create_new` + `mode(0o600)` claims the path in one syscall and the
    /// file is 0600 from creation, with no window at the umask default.
    /// Replacement is not needed by anything: D2 forbids overwriting a key.
    fn create_new_at(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            // Mode is set only when WE create the directory. Chmod-ing a
            // directory the operator already made is outside this arc's
            // mandate — measured on the delivered build: an existing 0750
            // `~/.oo` became 0700, and a deliberately read-only 0500 parent
            // became writable, on every mint.
            let pre_existing = parent.exists();
            std::fs::create_dir_all(parent)?;
            #[cfg(unix)]
            if !pre_existing {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
            }
        }
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(path)?;
        use std::io::Write;
        f.write_all(&self.private_key)?;
        f.sync_all()?;
        Ok(())
    }

    /// Load the existing operator identity, or mint one and claim the path.
    ///
    /// Under a concurrent first mint exactly one process wins the
    /// `create_new`; every loser loads the winner's file, so the key a
    /// process reports is always the key on disk.
    pub fn load_or_mint(path: &std::path::Path) -> anyhow::Result<Self> {
        if path.exists() {
            return Self::load(path);
        }
        let id = Self::new_random();
        match id.create_new_at(path) {
            Ok(()) => Ok(id),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Self::load_after_race(path),
            Err(e) => Err(anyhow::anyhow!(
                "identity file {}: could not be created: {}",
                path.display(),
                e
            )),
        }
    }

    /// Load a file the winner of a `create_new` race has claimed.
    ///
    /// `create_new` claims the path before the bytes are written, so a loser
    /// arriving inside that window would read an empty file and report it as
    /// a corrupt key. Not observed in 5 rounds of 8 — a loser generates an
    /// Ed25519 keypair between the two, which is far longer than the winner's
    /// write — but "not observed" is not "cannot happen", and this arc exists
    /// because a check that cannot fail is not a check.
    ///
    /// Bounded, and only on the cold-start path. Failure stays D2-honest: the
    /// file is never overwritten, so the worst case is a loud transient error
    /// and a re-run, never a wrong key.
    fn load_after_race(path: &std::path::Path) -> anyhow::Result<Self> {
        let mut last = None;
        for attempt in 0..100 {
            match Self::load(path) {
                Ok(id) => return Ok(id),
                Err(e) => {
                    last = Some(e);
                    if attempt < 99 {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
            }
        }
        Err(last.unwrap_or_else(|| {
            anyhow::anyhow!("identity file {}: unreadable after race", path.display())
        }))
    }

    pub fn public_key_hex(&self) -> String {
        hex::encode(&self.public_key)
    }

    /// Home for node keys: `OO_NODE_HOME` (absolute, replaces `~/.oo`) or
    /// `~/.oo`. Relative `OO_NODE_HOME` is refused (same rule as `OO_IDENTITY`).
    pub fn resolve_node_home() -> anyhow::Result<std::path::PathBuf> {
        if let Ok(p) = std::env::var("OO_NODE_HOME") {
            let path = std::path::PathBuf::from(p);
            if !path.is_absolute() {
                anyhow::bail!(
                    "OO_NODE_HOME must be an absolute path (got {})",
                    path.display()
                );
            }
            return Ok(path);
        }
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| anyhow::anyhow!("cannot resolve home directory for ~/.oo/nodes"))?;
        Ok(std::path::PathBuf::from(home).join(".oo"))
    }

    /// Key path for this workspace: `{node_home}/nodes/<sha256 hex of abs path>`.
    ///
    /// The workspace path is part of the node identity (node_identity arc Q1):
    /// the engine cannot distinguish a moved workspace from a copied one, so a
    /// path change *is* a new node — no detection, no heuristic. Secrets still
    /// live outside `.oo/` (REAL_01 §7.5 / v0.2.46).
    pub fn node_key_path(workspace: &std::path::Path) -> anyhow::Result<std::path::PathBuf> {
        let abs = if workspace.is_absolute() {
            workspace.to_path_buf()
        } else {
            std::env::current_dir()?.join(workspace)
        };
        // canonicalize when possible so symlinks/relative segments collapse;
        // fall back to the absolute join if the path does not exist yet.
        let abs = abs.canonicalize().unwrap_or(abs);
        let mut hasher = Sha256::new();
        hasher.update(abs.to_string_lossy().as_bytes());
        let digest = hex::encode(hasher.finalize());
        Ok(Self::resolve_node_home()?.join("nodes").join(digest))
    }

    /// `node_id` = CAID of the node's **public key** (closes REAL_02 §4.1 gap:
    /// 「節點 ID = CAID 的內容指紋前 160 bit」— the CAID addresses this key).
    /// DHT address is the leading 160 bits of this digest; unused until L2.
    pub fn node_id_caid(&self) -> ContentHash {
        Value::Atom(
            AtomKind::Bytes(self.public_key.clone()),
            EffectTag::Pure,
            None,
        )
        .content_hash()
    }
}

pub const TROPICAL_INFINITY: u64 = u64::MAX;

impl Value {
    pub fn bits(&self) -> u64 {
        match self {
            Value::Top | Value::TopCaused { .. } => 0,
            Value::Atom(kind, _, _) => match kind {
                AtomKind::Int(i) => i.bits() as u64,
                AtomKind::Float(_) => 64,
                AtomKind::Complex(_, _) => 128,
                AtomKind::Str(s) | AtomKind::MultilineStr(s) => (s.len() as u64) * 8,
                AtomKind::Tag(t) => (t.len() as u64) * 8 + 64,
                AtomKind::TagStart | AtomKind::TagEnd => 32,
                AtomKind::Top => 0,
                AtomKind::Bottom => 128,
                AtomKind::Bytes(b) => (b.len() as u64) * 8,
                _ => 128,
            },
            Value::Combo(c) => c.bits(),
            Value::Union(branches) => branches.iter().map(|b| b.bits()).sum(),
            Value::Code(_) | Value::Thunk { .. } => 256,
            Value::Bottom(d) => d.bits(),
            Value::Ref(_) => 64,
            // Partial is a CAID only — fixed cost (body lives in CAS).
            Value::Blur(_) => 128 + 256,
            Value::Range { start, end, step } => {
                start.bits() + end.bits() + step.as_ref().map(|s| s.bits()).unwrap_or(0)
            }
        }
    }

    pub fn tropical_weight(&self) -> u64 {
        match self {
            Value::Top | Value::TopCaused { .. } => 0,
            Value::Bottom(_) => TROPICAL_INFINITY,
            Value::Atom(_, _, _) => 1,
            Value::Thunk { .. } | Value::Code(_) | Value::Ref(_) | Value::Range { .. } => 1,
            Value::Union(branches) => branches
                .iter()
                .map(|b| b.tropical_weight())
                .min()
                .unwrap_or(TROPICAL_INFINITY),
            Value::Combo(c) => c
                .all_fields_iter()
                .map(|(_, v)| v.tropical_weight())
                .fold(0u64, |acc, w| acc.saturating_add(w)),
            Value::Blur(_) => 64,
        }
    }

    pub fn is_top(&self) -> bool {
        matches!(self, Value::Top | Value::TopCaused { .. })
    }

    /// Drop static-cycle provenance — lattice/ops consume bare Top only.
    pub fn bare_top_if_caused(self) -> Value {
        match self {
            Value::TopCaused { .. } => Value::Top,
            other => other,
        }
    }

    /// True if the value embeds any Blur (fuel-horizon partial) — such values
    /// are observation-relative and must not enter horizon-blind caches
    /// (GUIDE_03 §2A.1: cache keys lack horizon params, so only exact results
    /// may be memoized). Thunks count as exact: unevaluated, not partial.
    pub fn contains_blur(&self) -> bool {
        match self {
            Value::Blur(_) => true,
            Value::Combo(cv) => cv
                .data
                .values()
                .chain(cv.types.values())
                .chain(cv.rules.values())
                .chain(cv.meta.values())
                .chain(cv.system.values())
                .chain(cv.local.values())
                .any(|v| v.contains_blur()),
            Value::Union(branches) => branches.iter().any(|b| b.contains_blur()),
            _ => false,
        }
    }
    pub fn with_effect(self, e: EffectTag) -> Self {
        match self {
            Value::Atom(ak, old_e, r) => Value::Atom(ak, old_e.union(e), r),
            Value::Combo(mut cv) => {
                cv.effect = cv.effect.union(e);
                Value::Combo(cv)
            }
            Value::Union(branches) => {
                Value::Union(branches.into_iter().map(|b| b.with_effect(e)).collect())
            }
            Value::Thunk {
                expr,
                closure,
                context,
                effect,
            } => Value::Thunk {
                expr,
                closure,
                context,
                effect: effect.union(e),
            },
            _ => self,
        }
    }

    /// SPEC_08 §4.2.4 observation projection: any active tag
    /// (`#io`/`#nondet`/`#state`) collapses to a single `#cached`. Pure and
    /// already-cached are unchanged. Multi-active sets → single Cached.
    pub fn solidify_active_effect(e: EffectTag) -> EffectTag {
        if e.has_active() {
            EffectTag::Cached
        } else {
            e
        }
    }

    /// Recursively solidify active effects on a store-fetched value (observation
    /// projection only — does not write back to the store).
    pub fn solidify_effects(self) -> Self {
        match self {
            Value::Atom(k, e, r) => Value::Atom(k, Self::solidify_active_effect(e), r),
            Value::Combo(mut c) => {
                let was_active = c.effect.has_active();
                c.effect = Self::solidify_active_effect(c.effect);
                for map in [
                    &mut c.data,
                    &mut c.types,
                    &mut c.rules,
                    &mut c.meta,
                    &mut c.system,
                    &mut c.local,
                ] {
                    for (_, v) in map.iter_mut() {
                        *v = std::mem::replace(v, Value::Top).solidify_effects();
                    }
                }
                // O61's `%effect` field is the durable identity spelling.
                // Once decoded for observation, ComboVal.effect is again the
                // runtime source of truth and the field must not masquerade as
                // an explicit user override (which navigation would re-taint
                // with the carrier's effect). The `.%effect` lens below then
                // returns one pure `#cached` tag as §4.2.4 requires.
                if was_active || c.effect.contains(EffectTag::Cached) {
                    c.meta.shift_remove("effect");
                }
                for v in c.pending_spreads.iter_mut() {
                    *v = std::mem::replace(v, Value::Top).solidify_effects();
                }
                Value::Combo(c)
            }
            Value::Union(branches) => {
                Value::Union(branches.into_iter().map(|b| b.solidify_effects()).collect())
            }
            Value::Blur(mut bd) => {
                bd.effect = Self::solidify_active_effect(bd.effect);
                // O42 repair: partial body is content-addressed; solidify the
                // ephemeral body when still held, never rewrite the CAID.
                if let Some(p) = bd.partial_body.take() {
                    bd.partial_body = Some(Box::new((*p).solidify_effects()));
                }
                Value::Blur(bd)
            }
            Value::Range { start, end, step } => Value::Range {
                start: Box::new((*start).solidify_effects()),
                end: Box::new((*end).solidify_effects()),
                step: step.map(|s| Box::new((*s).solidify_effects())),
            },
            Value::Thunk {
                expr,
                closure,
                context,
                effect,
            } => Value::Thunk {
                expr,
                closure,
                context: context.map(|c| Box::new((*c).solidify_effects())),
                effect: Self::solidify_active_effect(effect),
            },
            // Top / TopCaused / Bottom / Code / Ref: no active effect surface.
            other => other,
        }
    }

    /// SPEC_08 §4.3 runPure discharge: active tags → Pure (observation
    /// projection). `#cached` is not active and is left unchanged.
    pub fn purify_active_effect(e: EffectTag) -> EffectTag {
        if e.has_active() {
            EffectTag::Pure
        } else {
            e
        }
    }

    /// Recursively strip active effects to Pure (privileged runPure result).
    pub fn purify_effects(self) -> Self {
        match self {
            Value::Atom(k, e, r) => Value::Atom(k, Self::purify_active_effect(e), r),
            Value::Combo(mut c) => {
                c.effect = Self::purify_active_effect(c.effect);
                for map in [
                    &mut c.data,
                    &mut c.types,
                    &mut c.rules,
                    &mut c.meta,
                    &mut c.system,
                    &mut c.local,
                ] {
                    for (_, v) in map.iter_mut() {
                        *v = std::mem::replace(v, Value::Top).purify_effects();
                    }
                }
                for v in c.pending_spreads.iter_mut() {
                    *v = std::mem::replace(v, Value::Top).purify_effects();
                }
                Value::Combo(c)
            }
            Value::Union(branches) => {
                Value::Union(branches.into_iter().map(|b| b.purify_effects()).collect())
            }
            Value::Blur(mut bd) => {
                bd.effect = Self::purify_active_effect(bd.effect);
                if let Some(p) = bd.partial_body.take() {
                    bd.partial_body = Some(Box::new((*p).purify_effects()));
                }
                Value::Blur(bd)
            }
            Value::Range { start, end, step } => Value::Range {
                start: Box::new((*start).purify_effects()),
                end: Box::new((*end).purify_effects()),
                step: step.map(|s| Box::new((*s).purify_effects())),
            },
            Value::Thunk {
                expr,
                closure,
                context,
                effect,
            } => Value::Thunk {
                expr,
                closure,
                context: context.map(|c| Box::new((*c).purify_effects())),
                effect: Self::purify_active_effect(effect),
            },
            other => other,
        }
    }

    pub fn is_morphism(&self) -> bool {
        if let Value::Combo(ref c) = self {
            if c.contains_key("%morphism") || c.contains_key("%rules") || c.contains_key("%builtin")
            {
                return true;
            }
        }
        match self.collapse() {
            Value::Combo(c) => {
                c.contains_key("%morphism")
                    || c.contains_key("%rules")
                    || c.contains_key("%builtin")
            }
            _ => false,
        }
    }
    pub fn effect(&self) -> EffectTag {
        match self {
            Value::Combo(c) => c.effect,
            Value::Atom(_, e, None) => *e,
            Value::Atom(_, e, Some(_)) => *e,
            Value::Thunk { effect, .. } => *effect,
            Value::Union(b) => b
                .iter()
                .fold(EffectTag::Pure, |acc, v| acc.union(v.effect())),
            Value::Blur(bd) => bd.effect,
            Value::Range { start, end, step } => {
                let mut e = start.effect().union(end.effect());
                if let Some(s) = step {
                    e = e.union(s.effect());
                }
                e
            }
            _ => EffectTag::Pure,
        }
    }
    pub fn collapse(&self) -> &Value {
        match self {
            Value::Combo(c) if c.is_pure_wrapper() => {
                c.get_field("%val").map(|v| v.collapse()).unwrap_or(self)
            }
            _ => self,
        }
    }

    pub fn collapse_with_effect(&self) -> (Value, EffectTag) {
        match self {
            Value::Combo(c) => {
                if c.is_pure_wrapper() {
                    if let Some(v) = c.get_field("%val") {
                        let (inner, inner_e) = v.collapse_with_effect();
                        (inner, inner_e.union(c.effect))
                    } else {
                        (self.clone(), c.effect)
                    }
                } else {
                    (self.clone(), c.effect)
                }
            }
            Value::Atom(_, e, _) => (self.clone(), *e),
            Value::Thunk { effect, .. } => (self.clone(), *effect),
            Value::Union(branches) => {
                let u = branches
                    .iter()
                    .fold(EffectTag::Pure, |acc, b| acc.union(b.effect()));
                (self.clone(), u)
            }
            Value::Blur(bd) => (self.clone(), bd.effect),
            _ => (self.clone(), EffectTag::Pure),
        }
    }

    pub fn to_string_plain(&self) -> String {
        match self {
            Value::Atom(kind, _, _) => match kind {
                AtomKind::Int(i) => i.to_string(),
                AtomKind::Float(f) => f.to_string(),
                AtomKind::Complex(r, i) => {
                    if *i >= 0.0 {
                        format!("{}+{}i", r, i)
                    } else {
                        format!("{}-{}i", r, i.abs())
                    }
                }
                AtomKind::Str(s) => s.clone(),
                AtomKind::MultilineStr(s) => s.clone(),
                AtomKind::Tag(t) => format!("#{}", t),
                AtomKind::TagStart => "#_|_".to_string(),
                AtomKind::TagEnd => "#_".to_string(),
                AtomKind::Top => "_".to_string(),
                AtomKind::Bottom => "_|_".to_string(),
                AtomKind::Bytes(b) => format!("b\"{:?}\"", b),
                _ => format!("{:?}", kind),
            },
            Value::Top | Value::TopCaused { .. } => "_".to_string(),
            // Align with to_nlang: `#<tag>` not Debug variant name.
            Value::Bottom(d) => format!("_|_ (%cause: #{})", d.cause.as_tag()),
            Value::Combo(c) => {
                if c.is_pure_wrapper() {
                    if let Some(v) = c.get_field("%val") {
                        return v.to_string_plain();
                    }
                }
                "{...}".to_string()
            }
            Value::Union(_) => "(...|...)".to_string(),
            Value::Blur(bd) => format!("#blur({})", bd.cause.as_str()),
            Value::Range { start, end, step } => {
                let mut s = format!("{}..{}", start.to_string_plain(), end.to_string_plain());
                if let Some(st) = step {
                    s.push_str(&format!("..{}", st.to_string_plain()));
                }
                s
            }
            // Align with to_nlang — no Debug fallthrough (print_what_can_be_read).
            Value::Thunk { expr, .. } => expr.to_nlang(0),
            Value::Ref(path) => format!("<<{}>>", path),
            Value::Code(expr) => expr.to_nlang(0),
        }
    }

    pub fn to_nlang(&self, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        match self {
            Value::Top | Value::TopCaused { .. } => "_".to_string(),
            Value::Atom(kind, effect, rank) => {
                let mut s = match kind {
                    AtomKind::Int(i) => i.to_string(),
                    AtomKind::Float(f) => f.to_string(),
                    AtomKind::Complex(r, i) => {
                        if *i >= 0.0 {
                            format!("{}+{}i", r, i)
                        } else {
                            format!("{}-{}i", r, i.abs())
                        }
                    }
                    AtomKind::Str(s) => quote_nlang_string(s),
                    AtomKind::MultilineStr(s) => quote_nlang_multiline(s),
                    AtomKind::Tag(t) => format!("#{}", t),
                    AtomKind::TagStart => "#_|_".to_string(),
                    AtomKind::TagEnd => "#_".to_string(),
                    AtomKind::Bytes(b) => format!("b\"{:?}\"", b),
                    _ => format!("{:?}", kind),
                };
                if let Some(r) = rank {
                    s.push_str(&format!("  ;; %rank: {}", r));
                }
                if !effect.is_pure() {
                    s.push_str(&format!("  ;; %effect: {}", effect));
                }
                s
            }
            Value::Combo(c) => {
                let is_list = c
                    .get_field("%kind")
                    .map(|k| k.to_string_plain() == "#list")
                    .unwrap_or(false);
                if is_list {
                    let mut items = Vec::new();
                    let mut i = 0;
                    while let Some(v) = c.get_field(&i.to_string()) {
                        items.push(v.to_nlang(indent + 1));
                        i += 1;
                    }
                    return format!("[{}]", items.join(", "));
                }

                let mut s = if c.closed {
                    "{{".to_string()
                } else {
                    "{".to_string()
                };
                // Empty closed combo must re-parse (identity_persistence R8).
                // Was `{{ }` (one brace short) — invalid n/ source.
                if c.data.is_empty()
                    && c.types.is_empty()
                    && c.rules.is_empty()
                    && c.meta.is_empty()
                    && c.system.is_empty()
                    && c.local.is_empty()
                {
                    return if c.closed {
                        "{{ }}".to_string()
                    } else {
                        "{}".to_string()
                    };
                }
                s.push('\n');
                // Per-axis spelling, then sort displayed keys so `%val`
                // still precedes `name` (same order as the old flattened
                // `fields()` sort). Data `@t` quotes; type-axis `t` stays `@t`.
                let mut rows: Vec<(String, Value)> = Vec::new();
                let push = |rows: &mut Vec<(String, Value)>, prefix: &str, k: &str, v: &Value| {
                    rows.push((quote_nlang_field_key(prefix, k), v.clone()));
                };
                for (k, v) in &c.data {
                    push(&mut rows, "", k, v);
                }
                for (k, v) in &c.types {
                    push(&mut rows, "@", k, v);
                }
                for (k, v) in &c.rules {
                    push(&mut rows, "/", k, v);
                }
                for (k, v) in &c.meta {
                    push(&mut rows, "%", k, v);
                }
                for (k, v) in &c.system {
                    push(&mut rows, "~%", k, v);
                }
                for (k, v) in &c.local {
                    push(&mut rows, "~", k, v);
                }
                rows.sort_by(|a, b| a.0.cmp(&b.0));
                for (disp, v) in rows {
                    if is_engine_scaffold_field(&disp, &v) {
                        continue;
                    }
                    s.push_str(&format!("{}  {}: {}\n", pad, disp, v.to_nlang(indent + 1)));
                }
                s.push_str(&format!("{}}}", pad));
                if c.closed {
                    s.push('}');
                }
                // SPEC_08 §4.1 / effect_union: non-pure combo carries the
                // set-rendered diagnostic tail (same order as `.%effect`).
                if !c.effect.is_pure() {
                    s.push_str(&format!("  ;; %effect: {}", c.effect));
                }
                s
            }
            // SPEC_01 §2.4.1: display spelling is a function of the value —
            // sort branches for print only; internal vector stays put.
            Value::Union(branches) => {
                let ordered = canonical_display_order(branches);
                let parts: Vec<String> = ordered.iter().map(|b| b.to_nlang(indent)).collect();
                parts.join(" | ")
            }
            // L2-17: Blur-precedent cause tag on the display axis (bn_serial
            // identity axis untouched — Bottom hashes by cause discriminant).
            Value::Bottom(d) => {
                let mut s = format!("_|_ (%cause: #{})", d.cause.as_tag());
                if let Some(ref m) = d.message {
                    s.push_str(&format!("  ;; {}", m));
                }
                s
            }
            Value::Blur(bd) => {
                let caid = bd.blur_caid().to_string();
                format!(
                    "#blur {{ %cause: #{}, %caid: \"{}\" }}",
                    bd.cause.as_str(),
                    caid
                )
            }
            // Canonical print: `a..b` / `a..b..s`, no spaces (range_eval probes).
            Value::Range { start, end, step } => {
                let mut s = format!("{}..{}", start.to_nlang(0), end.to_nlang(0));
                if let Some(st) = step {
                    s.push_str(&format!("..{}", st.to_nlang(0)));
                }
                s
            }
            // print_what_can_be_read (W8'-a): never fall through to Debug.
            // Unobserved thunk prints its source expression (not its answer).
            Value::Thunk { expr, effect, .. } => {
                let mut s = expr.to_nlang(indent);
                if !effect.is_pure() {
                    s.push_str(&format!("  ;; %effect: {}", effect));
                }
                s
            }
            // Structural ref: `<<path>>` is the only spelling that round-trips
            // as a held reference (bare `_.a` would re-evaluate).
            Value::Ref(path) => format!("<<{}>>", path),
            // Quoted code prints its expression. Round-trip as *code* is an
            // acknowledged gap (re-read evaluates rather than quotes).
            Value::Code(expr) => expr.to_nlang(indent),
        }
    }

    pub fn content_hash_with_salt(&self, salt: &ContentHash) -> ContentHash {
        let mut hasher = Sha256::new();
        if !self.effect().is_pure() {
            hasher.update(b"HORIZON_SALT_V1");
            hasher.update(&salt.digest);
        }
        self.hash_recursive_with_salt(&mut hasher, salt);
        ContentHash::v1(hasher.finalize().to_vec())
    }

    pub fn content_hash(&self) -> ContentHash {
        // ComboVal has its own memoized path (cache_id).
        if let Value::Combo(c) = self {
            return c.content_hash();
        }
        let digest = crate::bn_serial::content_digest(self);
        let sketch = crate::lattice_sketch::compute_sketch_v2(self);
        ContentHash {
            algorithm: HashAlgorithm::Sha256,
            version: CaidVersion::V2,
            masa_ref: MasaRef::Top,
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
            if let (
                Some(Value::Atom(AtomKind::Str(pk_hex), _, None)),
                Some(Value::Atom(AtomKind::Str(sig_hex), _, None)),
                Some(target),
            ) = (
                c.get_field("%pubkey"),
                c.get_field("%signature"),
                c.get_field("%target"),
            ) {
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
            // Caused Top hashes as bare Top (CAID / lattice identity).
            Value::Top | Value::TopCaused { .. } => hasher.update([0x00]),
            Value::Atom(kind, effect, rank) => {
                hasher.update([0x01]);
                hasher.update([effect.to_serial_byte()]);
                match kind {
                    AtomKind::Int(i) => {
                        hasher.update([0x01]);
                        let (sign, bytes) = i.to_bytes_be();
                        hasher.update(&[if sign == num_bigint::Sign::Minus {
                            1
                        } else {
                            0
                        }]);
                        hasher.update(&bytes);
                    }
                    AtomKind::Float(f) => {
                        hasher.update([0x07]);
                        hasher.update(f.to_bits().to_le_bytes());
                    }
                    AtomKind::Complex(r, i) => {
                        hasher.update([0x08]);
                        hasher.update(r.to_bits().to_le_bytes());
                        hasher.update(i.to_bits().to_le_bytes());
                    }
                    AtomKind::Str(s) => {
                        hasher.update([0x02]);
                        hasher.update(s.as_bytes());
                    }
                    AtomKind::Tag(t) => {
                        hasher.update([0x03]);
                        hasher.update(t.as_bytes());
                    }
                    AtomKind::TagStart => {
                        hasher.update([0x04]);
                    }
                    AtomKind::TagEnd => {
                        hasher.update([0x05]);
                    }
                    AtomKind::Bytes(b) => {
                        hasher.update([0x09]);
                        hasher.update(b);
                    }
                    _ => {
                        hasher.update([0x06]);
                        hasher.update(format!("{:?}", kind).as_bytes());
                    }
                }
                if let Some(r) = rank {
                    hasher.update(r.to_le_bytes());
                }
            }
            Value::Combo(c) => {
                hasher.update([0x02]);
                hasher.update([if c.closed { 1 } else { 0 }]);
                hasher.update([c.effect.to_serial_byte()]);
                // Same flattened spelling as `fields()`, except a data key
                // that looks like an axis sigil (`@t`) must not hash as the
                // type-axis key `t` (Q1). Ordinary data keys keep the old
                // bytes so existing `%id` pins do not move.
                let mut entries: Vec<(String, &Value)> = Vec::new();
                for (k, v) in &c.data {
                    let hk = if k.starts_with("~%")
                        || k.starts_with('~')
                        || k.starts_with('@')
                        || k.starts_with('/')
                        || k.starts_with('%')
                    {
                        format!("\0data:{k}")
                    } else {
                        k.clone()
                    };
                    entries.push((hk, v));
                }
                for (k, v) in &c.rules {
                    entries.push((format!("/{k}"), v));
                }
                for (k, v) in &c.types {
                    entries.push((format!("@{k}"), v));
                }
                for (k, v) in &c.meta {
                    entries.push((format!("%{k}"), v));
                }
                for (k, v) in &c.system {
                    entries.push((format!("~%{k}"), v));
                }
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                for (k, v) in entries {
                    hasher.update(k.as_bytes());
                    v.hash_recursive_with_salt(hasher, salt);
                }
                let local = c.local_fields();
                let mut lkeys: Vec<_> = local.keys().collect();
                lkeys.sort();
                for k in lkeys {
                    hasher.update(b"local:");
                    hasher.update(k.as_bytes());
                    local.get(k).unwrap().hash_recursive_with_salt(hasher, salt);
                }
            }
            Value::Union(branches) => {
                hasher.update([0x03]);
                let mut digests: Vec<_> = branches
                    .iter()
                    .map(|b| {
                        let mut h = Sha256::new();
                        b.hash_recursive_with_salt(&mut h, salt);
                        h.finalize().to_vec()
                    })
                    .collect();
                digests.sort();
                for d in digests {
                    hasher.update(&d);
                }
            }
            Value::Bottom(d) => {
                hasher.update([0x04]);
                hasher.update([d.cause as u8]);
            }
            Value::Blur(bd) => {
                // O42 R-5: same CHS as blur_caid / bn_serial.
                hasher.update([0xFD]);
                hasher.update(&bd.blur_caid().digest);
            }
            Value::Thunk {
                expr,
                closure,
                context,
                effect,
            } => {
                // GUIDE_03 §11.3 memo key: (expr CAID, frame CAID, context CAID | #open).
                // Must be deterministic and evaluation-independent.
                hasher.update([0x05]);
                // expr: canonical serialization (to_nlang) rather than Debug format,
                // so structurally-equivalent exprs hash identically regardless of
                // internal span/field-order differences that canonicalize() resolves.
                hasher.update(expr.to_nlang(0).as_bytes());
                // frame (closure scopes): hash each ComboVal in the scope stack.
                hasher.update(b"|frame:");
                for cv in closure.iter() {
                    let cv_hash = cv.content_hash();
                    hasher.update(&cv_hash.digest);
                }
                // context: None = open term (#open); Some(v) = v's content hash.
                match context {
                    None => hasher.update(b"|#open"),
                    Some(v) => {
                        hasher.update(b"|ctx:");
                        let ch = v.content_hash();
                        hasher.update(&ch.digest);
                    }
                }
                hasher.update(&[effect.to_serial_byte()]);
            }
            Value::Code(expr) => {
                // Span-free (O42 M4). See bn_serial Code arm — to_nlang is
                // too stack-heavy for deep left-associated chains.
                hasher.update([0x06]);
                hasher.update(format!("{:?}", expr.without_spans()).as_bytes());
            }
            Value::Ref(path) => {
                hasher.update([0x07]);
                match path.anchor {
                    PathAnchor::Bare => hasher.update([0x00]),
                    PathAnchor::Root => hasher.update([0x01]),
                    PathAnchor::Parent(n) => {
                        hasher.update([0x02]);
                        hasher.update(&n.to_le_bytes());
                    }
                    PathAnchor::Current => hasher.update([0x03]),
                    PathAnchor::Address { algo, digest } => {
                        hasher.update([0x04]);
                        hasher.update([match algo {
                            nlang_parser::ast::AddressAlgo::Sha256 => 0u8,
                        }]);
                        hasher.update(&digest);
                    }
                }
                // length-delimited: <<a.bc>> and <<ab.c>> are different geometry
                // and must not collide (CAID equality drives lazy-unify early-out)
                for seg in &path.segments {
                    hasher.update((seg.len() as u64).to_le_bytes());
                    hasher.update(seg.as_bytes());
                }
            }
            Value::Range { start, end, step } => {
                hasher.update([0x08]);
                start.hash_recursive_with_salt(hasher, salt);
                end.hash_recursive_with_salt(hasher, salt);
                match step {
                    None => hasher.update([0x00]),
                    Some(s) => {
                        hasher.update([0x01]);
                        s.hash_recursive_with_salt(hasher, salt);
                    }
                }
            }
        }
    }
}

impl ComboVal {
    /// In-place content hash with `cache_id` memo. Do not clone first —
    /// Clone resets the cache (see `impl Clone for ComboVal`).
    pub fn content_hash(&self) -> ContentHash {
        if let Ok(guard) = self.cache_id.read() {
            if let Some(ref h) = *guard {
                return h.clone();
            }
        }
        let digest = crate::bn_serial::content_digest_combo(self);
        // Sketch walks the value tree; a temporary wrap clones fields only.
        let sketch = crate::lattice_sketch::compute_sketch_v2(&Value::Combo(Self {
            data: self.data.clone(),
            types: self.types.clone(),
            rules: self.rules.clone(),
            meta: self.meta.clone(),
            system: self.system.clone(),
            local: self.local.clone(),
            closed: self.closed,
            effect: self.effect,
            relations: self.relations.clone(),
            masa_ref: self.masa_ref.clone(),
            pending_spreads: self.pending_spreads.clone(),
            cache_id: default_cache_id(),
            cycle_frame_id: default_cache_id(),
            legacy_fields: self.legacy_fields.clone(),
            legacy_local: self.legacy_local.clone(),
        }));
        let h = ContentHash {
            algorithm: HashAlgorithm::Sha256,
            version: CaidVersion::V2,
            masa_ref: self.masa_ref.clone(),
            lattice_sketch: sketch,
            digest: digest.to_vec(),
        };
        if let Ok(mut guard) = self.cache_id.write() {
            *guard = Some(h.clone());
        }
        h
    }

    /// Content identity of a sealed scope frame for force/in_flight keys (M1).
    ///
    /// Nested `Arc` frames in thunk closures contribute only their digests
    /// (Merkle), memoized on `cycle_frame_id` so each unique Arc is hashed
    /// once. Distinct from `content_hash` / store CAID (which still inlines
    /// frames for bit-stable durable encoding of unforced thunks).
    pub fn cycle_frame_digest(&self) -> ContentHash {
        if let Ok(guard) = self.cycle_frame_id.read() {
            if let Some(ref h) = *guard {
                return h.clone();
            }
        }
        let mut hasher = Sha256::new();
        hasher.update(b"cycle_frame:v1:");
        hasher.update([self.closed as u8]);
        hasher.update([self.effect.to_serial_byte()]);
        // Stable family order matching bn_serial axis priority.
        for (prio, map) in [
            (1u8, &self.system),
            (2, &self.meta),
            (3, &self.types),
            (4, &self.rules),
            (5, &self.data),
            (6, &self.local),
        ] {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for k in keys {
                hasher.update([prio]);
                hasher.update(k.as_bytes());
                hash_value_for_cycle_frame(map.get(k).unwrap(), &mut hasher);
            }
        }
        for (prio, map) in [(5u8, &self.legacy_fields), (6, &self.legacy_local)] {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for k in keys {
                hasher.update([prio]);
                hasher.update(k.as_bytes());
                hash_value_for_cycle_frame(map.get(k).unwrap(), &mut hasher);
            }
        }
        for r in &self.relations {
            hasher.update(b"rel:");
            hasher.update(r.left.as_bytes());
            hasher.update([match r.op {
                RelOp::Lt => 0,
                RelOp::Gt => 1,
                RelOp::Lte => 2,
                RelOp::Gte => 3,
                RelOp::Eq => 4,
            }]);
            hasher.update(r.right.as_bytes());
        }
        let h = ContentHash::v1(hasher.finalize().to_vec());
        if let Ok(mut guard) = self.cycle_frame_id.write() {
            *guard = Some(h.clone());
        }
        h
    }
}

/// Structural walk for `cycle_frame_digest`: Arc frames Merkle-hash; atoms
/// and other leaves use their ordinary content digest.
fn hash_value_for_cycle_frame(v: &Value, hasher: &mut Sha256) {
    match v {
        Value::Thunk {
            expr,
            closure,
            context,
            effect,
        } => {
            hasher.update(b"thunk:");
            hasher.update(expr.to_nlang(0).as_bytes());
            hasher.update(&(closure.len() as u64).to_le_bytes());
            for cv in closure.iter() {
                hasher.update(&cv.cycle_frame_digest().digest);
            }
            match context {
                None => hasher.update(b"#open"),
                Some(c) => {
                    hasher.update(b"ctx:");
                    hash_value_for_cycle_frame(c, hasher);
                }
            }
            hasher.update([effect.to_serial_byte()]);
        }
        Value::Combo(cv) => {
            hasher.update(b"combo:");
            hasher.update(&cv.cycle_frame_digest().digest);
        }
        Value::Union(items) => {
            hasher.update(b"union:");
            hasher.update(&(items.len() as u64).to_le_bytes());
            for it in items {
                hash_value_for_cycle_frame(it, hasher);
            }
        }
        Value::Range { start, end, step } => {
            hasher.update(b"range:");
            hash_value_for_cycle_frame(start, hasher);
            hash_value_for_cycle_frame(end, hasher);
            match step {
                None => hasher.update([0]),
                Some(s) => {
                    hasher.update([1]);
                    hash_value_for_cycle_frame(s, hasher);
                }
            }
        }
        // Atoms / Top / Bottom / Blur / Code / Ref: store content digest.
        other => {
            hasher.update(b"leaf:");
            hasher.update(&other.content_hash().digest);
        }
    }
}

impl Commit {
    pub fn content_hash(&self) -> ContentHash {
        let mut buf = Vec::new();
        if let Some(p) = &self.parent {
            buf.extend_from_slice(&p.digest);
        }
        buf.extend_from_slice(&self.root.digest);
        // Tag bytes: Standard=0, Refine=1, Pin=2, Squash=3. New kinds only
        // append so existing commit digests stay bit-stable.
        buf.push(match self.kind {
            CommitKind::Standard => 0,
            CommitKind::Refine => 1,
            CommitKind::Pin => 2,
            CommitKind::Squash => 3,
        });
        if let Some(ref ri) = self.refine_info {
            for src in &ri.source_caids {
                buf.extend_from_slice(&src.digest);
            }
            for tgt in &ri.target_caids {
                buf.extend_from_slice(&tgt.digest);
            }
        }
        let meta_bytes = format!("{:?}", self.meta);
        crate::bn_serial::encode_unsigned_leb128(meta_bytes.len() as u64, &mut buf);
        buf.extend_from_slice(meta_bytes.as_bytes());
        let digest = Sha256::digest(&buf).to_vec();
        ContentHash::v1(digest)
    }
}
