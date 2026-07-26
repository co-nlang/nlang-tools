use nlang_parser::ast::{Expr, AtomKind, Path, PathAnchor};
use indexmap::IndexMap;
use sha2::{Sha256, Digest};
use std::fmt;
use serde::{Serialize, Deserialize};
use ring::{signature::{self, KeyPair as _}, rand};
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
    /// Thunk CAID serial byte: single-tag legacy ordinals unchanged
    /// (Pure=0, State=1, IO=2, NonDet=3); multi-tag / Cached use high bit.
    pub fn to_serial_byte(self) -> u8 {
        match self.0 {
            0 => 0,          // Pure
            0b0100 => 1,     // State (legacy)
            0b0001 => 2,     // IO (legacy)
            0b0010 => 3,     // NonDet (legacy)
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
    /// Declared but inert slots (no consumer yet).
    pub pin: bool,
    pub commit: bool,
    pub rollback: bool,
    pub squash: bool,
}

impl Privilege {
    pub const NONE: Privilege = Privilege {
        effect_override: None,
        pin: false,
        commit: false,
        rollback: false,
        squash: false,
    };

    /// Full grant (CLI `--privileged` back-compat).
    pub fn all() -> Privilege {
        Privilege {
            effect_override: Some(EffectTag::all_active()),
            pin: true,
            commit: true,
            rollback: true,
            squash: true,
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
    /// `cause`: `"static_cycle"` | `"no_coordinate"` (ERROR_CODES tags).
    TopCaused {
        /// Provenance tag body (no leading `#`).
        cause: String,
        /// Loop member coordinates for static_cycle (self = len 1, mutual = 2, …).
        /// Empty for open-miss `#no_coordinate`.
        members: Vec<String>,
    },
    Atom(AtomKind, EffectTag, Option<i64>), Combo(ComboVal), Union(Vec<Value>), Code(Box<Expr>),
    Thunk {
        expr: Box<Expr>,
        closure: Vec<ComboVal>,
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
            Value::Combo(ComboVal::new(mf, false, IndexMap::new(), EffectTag::Pure, vec![])),
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
            (Value::Thunk { expr: ex1, closure: cl1, context: c1, effect: ef1 },
             Value::Thunk { expr: ex2, closure: cl2, context: c2, effect: ef2 }) =>
                ex1.without_spans() == ex2.without_spans()
                    && cl1 == cl2
                    && c1 == c2
                    && ef1 == ef2,
            (Value::Bottom(b1), Value::Bottom(b2)) => b1 == b2,
            (Value::Blur(b1), Value::Blur(b2)) => b1 == b2,
            (
                Value::Range { start: s1, end: e1, step: st1 },
                Value::Range { start: s2, end: e2, step: st2 },
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
        Some(Value::Atom(AtomKind::Tag(t), _, _)) => {
            t.trim_start_matches('#') == "true"
        }
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
        Value::Combo(c) if is_structural_view(&c) => c
            .get_field("%node")
            .cloned()
            .unwrap_or(Value::Combo(c)),
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
                let inner = c
                    .get_field("%node")
                    .cloned()
                    .unwrap_or(Value::Combo(c));
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
        Value::Union(branches) => {
            normalize_union(branches.into_iter().map(strip_local_axis))
        }
        other => other,
    }
}

/// Engine anti-peel scaffolding: data key `_` bound to Top (printed as `_`).
/// Not user-visible (REAL_04 cocoon_shape law 3). User fields named `_` with
/// any non-Top value still display.
fn is_engine_scaffold_field(key: &str, val: &Value) -> bool {
    key == "_" && matches!(val, Value::Top | Value::TopCaused { .. })
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
                let inner = c
                    .get_field("%node")
                    .cloned()
                    .unwrap_or(Value::Combo(c));
                return strip_local_axis(inner);
            }
            // Hybrid or pure wrapper: value context reads %val.
            if let Some(inner) = c.get_field("%val").cloned() {
                return project_value_context(inner);
            }
            // Plain combo / list: project each public field; drop local axis.
            let mut new_c = ComboVal::default();
            new_c.closed = c.closed;
            new_c.effect = c.effect;
            new_c.relations = c.relations.clone();
            new_c.masa_ref = c.masa_ref.clone();
            for (k, fv) in c.all_fields_iter() {
                new_c.insert_field(&k, project_value_context(fv));
            }
            Value::Combo(new_c)
        }
        Value::Union(branches) => {
            normalize_union(branches.into_iter().map(project_value_context))
        }
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
    let frame = c.clone();
    fn inject(v: &mut Value, frame: &ComboVal) {
        match v {
            Value::Thunk { closure, .. } => {
                closure.push(frame.clone());
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
        // #blur (4): SPEC_01 §2.4.1 #5 amended — key is
        // (cause name lex, fuel_remaining asc, strategy). NEVER %caid/salt
        // (display string embeds salted caid → cross-process flip).
        // Full key tie → Equal so stable sort keeps encounter order.
        4 => match (a, b) {
            (Value::Blur(ba), Value::Blur(bb)) => {
                let strat = |s: ObservationStrategy| -> u8 {
                    match s {
                        ObservationStrategy::Blur => 0,
                        ObservationStrategy::Strict => 1,
                        ObservationStrategy::Approximate => 2,
                    }
                };
                ba.cause
                    .as_str()
                    .cmp(bb.cause.as_str())
                    .then_with(|| ba.horizon.fuel_remaining.cmp(&bb.horizon.fuel_remaining))
                    .then_with(|| strat(ba.horizon.strategy).cmp(&strat(bb.horizon.strategy)))
            }
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
pub fn primary_bottom_from_culled(
    culled: impl IntoIterator<Item = BottomDetail>,
) -> Value {
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
    /// SPEC_03 §3.1 timing (forward_spread 2026-07-19): spread sources held
    /// as Thunks until observation convergence — not expanded at construction.
    /// Empty after expand. Skipped in serde (re-expand from source on load is
    /// not required for in-session evolve/observe; store solidifies via force).
    #[serde(skip, default)]
    pub pending_spreads: Vec<Value>,
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
            pending_spreads: Vec::new(),
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
            && self.pending_spreads == other.pending_spreads
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelOp { Lt, Gt, Lte, Gte, Eq }

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
            BottomCause::SemanticEclipse => "#semantic_eclipse",
            BottomCause::NoContext => "#no_context",
            BottomCause::OutOfHorizon => "#out_of_horizon",
            BottomCause::SystemReserved => "#system_reserved",
            BottomCause::InvalidConfig => "#invalid_config",
            BottomCause::EffectViolation => "#effect_violation",
            BottomCause::PrivilegedRequired => "#privileged_required",
            BottomCause::StoreBoundary => "#store_boundary",
            BottomCause::CaidMismatch => "#caid_mismatch",
        };
        // F2 (REAL_04 §1 / SYNTAX_08 §4 #3): %cause is a Cocoon whose duality
        // core is %val = the cause tag. Direct observation collapses via G6
        // value-context projection; <<path>> keeps the full chain.
        // Fossil %type twin removed (cocoon_shape arc 2026-07-19).
        fields.insert(
            "%val".to_string(),
            Value::Atom(AtomKind::Tag(type_tag[1..].to_string()), EffectTag::Pure, None),
        );
        // Non-empty data axis so lattice unify does not treat this as a pure
        // wrapper and peel to the bare tag during evolve field-merge (which
        // would erase the cocoon before `m.%val` can navigate). Collapsed
        // observation still peels %val (project_value_context). Engine
        // scaffolding: stripped at display (to_nlang) — never user-visible.
        fields.insert(
            "_".to_string(),
            Value::Top,
        );
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
    /// `^` parent-anchor overflow (ERROR_CODES §1 #out_of_horizon).
    OutOfHorizon,
    /// User LHS write to engine-minted `~%` system axis (SPEC_09 ownership;
    /// ERROR_CODES §1.4 #system_reserved). Append-only tail.
    SystemReserved,
    /// Root `~%Config.<bare>` name/type violation (SPEC_09 §6 closed knob
    /// family; ERROR_CODES #invalid_config). Evolve-boundary named error —
    /// never a node-level ⊥. Append-only tail (fmt discipline).
    InvalidConfig,
    /// Declared `%effect: #pure` contradicted by active contagion
    /// (SPEC_08 §4.3; ERROR_CODES #effect_violation). Append-only tail.
    EffectViolation,
    /// Privileged op invoked without horizon privilege (SPEC_08 §6.1.2;
    /// ERROR_CODES #privileged_required). Append-only tail.
    PrivilegedRequired,
    /// Filesystem access from the language layer to a path inside the engine
    /// store (`.oo`). Unconditional — no capability unlocks it.
    /// SPEC_08 §6.3; ERROR_CODES #store_boundary. Append-only tail.
    StoreBoundary,
    /// Content address does not match requested CAID (REAL_03 §6.6 peer/store
    /// read path). Append-only tail. Distinct from absence (`#conflict` /
    /// `#missing_key` at the language surface for plain miss).
    CaidMismatch,
}

impl BottomCause {
    pub fn as_tag(&self) -> &str {
        match self {
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
            BottomCause::OutOfHorizon => "out_of_horizon",
            BottomCause::SystemReserved => "system_reserved",
            BottomCause::InvalidConfig => "invalid_config",
            BottomCause::EffectViolation => "effect_violation",
            BottomCause::PrivilegedRequired => "privileged_required",
            BottomCause::StoreBoundary => "store_boundary",
            BottomCause::CaidMismatch => "caid_mismatch",
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
            | BottomCause::CaidMismatch => 1,
            BottomCause::Conflict
            | BottomCause::H1Split
            | BottomCause::H2Split
            | BottomCause::SemanticEclipse
            | BottomCause::NumericalError
            | BottomCause::ArithmeticOnAnchor
            | BottomCause::NoContext => 2,
            BottomCause::FuelExhausted
            | BottomCause::Timeout
            | BottomCause::OutOfHorizon => 3,
            BottomCause::MissingKey | BottomCause::InvalidPath => 4,
        }
    }
}

impl From<BottomCause> for Value {
    fn from(cause: BottomCause) -> Self {
        Value::Bottom(Box::new(BottomDetail { cause, path: None, message: None, expected: None, found: None, involved: vec![], obstruction_degree: None, holonomy: None }))
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
}

impl BlurCause {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            BlurCause::FuelExhausted => b"fuel_exhausted",
            BlurCause::Timeout => b"timeout",
            BlurCause::StackOverflow => b"stack_overflow",
            BlurCause::MathSingularity(s) => s.as_bytes(),
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            BlurCause::FuelExhausted => "fuel_exhausted",
            BlurCause::Timeout => "timeout",
            BlurCause::StackOverflow => "stack_overflow",
            BlurCause::MathSingularity(s) => s.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HorizonParams {
    pub fuel_remaining: u64,
    pub strategy: ObservationStrategy,
    pub salt: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlurDetail {
    pub cause: BlurCause,
    pub horizon: HorizonParams,
    pub partial: Option<Box<Value>>,
    pub effect: EffectTag,
}

impl BlurDetail {
    pub fn blur_caid(&self) -> ContentHash {
        let mut hasher = Sha256::new();
        hasher.update(b"blur:");
        hasher.update(self.cause.as_bytes());
        hasher.update(b":fuel=");
        hasher.update(&self.horizon.fuel_remaining.to_le_bytes());
        hasher.update(b":strategy=");
        let strat_byte: u8 = match self.horizon.strategy {
            ObservationStrategy::Blur => 0,
            ObservationStrategy::Strict => 1,
            ObservationStrategy::Approximate => 2,
        };
        hasher.update(&[strat_byte]);
        hasher.update(b":salt=");
        hasher.update(&self.horizon.salt.digest);
        ContentHash::v1(hasher.finalize().to_vec())
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
}

impl Default for CommitMeta {
    fn default() -> Self {
        Self {
            author: None,
            timestamp: 0,
            message: None,
            abandoned: None,
        }
    }
}

/// Custom Debug so `Commit::content_hash` (which formats meta via `Debug`)
/// stays bit-stable for commits with no abandonment record. Adding a field
/// under derive(Debug) would change every historical commit digest.
impl fmt::Debug for CommitMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.abandoned.is_none() {
            f.debug_struct("CommitMeta")
                .field("author", &self.author)
                .field("timestamp", &self.timestamp)
                .field("message", &self.message)
                .finish()
        } else {
            f.debug_struct("CommitMeta")
                .field("author", &self.author)
                .field("timestamp", &self.timestamp)
                .field("message", &self.message)
                .field("abandoned", &self.abandoned)
                .finish()
        }
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
impl Default for CommitKind { fn default() -> Self { Self::Standard } }

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
            Value::Blur(bd) => {
                128 + bd.partial.as_ref().map(|p| p.bits()).unwrap_or(0)
            },
            Value::Range { start, end, step } => {
                start.bits()
                    + end.bits()
                    + step.as_ref().map(|s| s.bits()).unwrap_or(0)
            }
        }
    }

    pub fn tropical_weight(&self) -> u64 {
        match self {
            Value::Top | Value::TopCaused { .. } => 0,
            Value::Bottom(_) => TROPICAL_INFINITY,
            Value::Atom(_, _, _) => 1,
            Value::Thunk { .. } | Value::Code(_) | Value::Ref(_) | Value::Range { .. } => 1,
            Value::Union(branches) => branches.iter().map(|b| b.tropical_weight()).min().unwrap_or(TROPICAL_INFINITY),
            Value::Combo(c) => c.all_fields_iter().map(|(_, v)| v.tropical_weight()).fold(0u64, |acc, w| acc.saturating_add(w)),
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
            Value::Combo(cv) => {
                cv.data.values().chain(cv.types.values()).chain(cv.rules.values())
                    .chain(cv.meta.values()).chain(cv.system.values()).chain(cv.local.values())
                    .any(|v| v.contains_blur())
            }
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
                if let Some(p) = bd.partial.take() {
                    bd.partial = Some(Box::new((*p).solidify_effects()));
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
                if let Some(p) = bd.partial.take() {
                    bd.partial = Some(Box::new((*p).purify_effects()));
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
            _ => EffectTag::Pure 
        }
    }
    pub fn collapse(&self) -> &Value { match self { Value::Combo(c) if c.is_pure_wrapper() => c.get_field("%val").map(|v| v.collapse()).unwrap_or(self), _ => self } }
    
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
                    if *i >= 0.0 { format!("{}+{}i", r, i) }
                    else { format!("{}-{}i", r, i.abs()) }
                },
                AtomKind::Str(s) => s.clone(),
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
            Value::Combo(c) => { if c.is_pure_wrapper() { if let Some(v) = c.get_field("%val") { return v.to_string_plain(); } } "{...}".to_string() }
            Value::Union(_) => "(...|...)".to_string(),
            Value::Blur(bd) => format!("#blur({})", bd.cause.as_str()),
            Value::Range { start, end, step } => {
                let mut s = format!("{}..{}", start.to_string_plain(), end.to_string_plain());
                if let Some(st) = step {
                    s.push_str(&format!("..{}", st.to_string_plain()));
                }
                s
            }
            _ => format!("{:?}", self),
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
                        if *i >= 0.0 { format!("{}+{}i", r, i) }
                        else { format!("{}-{}i", r, i.abs()) }
                    },
                    AtomKind::Str(s) => format!("\"{}\"", s),
                    AtomKind::Tag(t) => format!("#{}", t),
                    AtomKind::TagStart => "#_|_".to_string(),
                    AtomKind::TagEnd => "#_".to_string(),
                    AtomKind::Bytes(b) => format!("b\"{:?}\"", b),
                    _ => format!("{:?}", kind),
                };
                if let Some(r) = rank { s.push_str(&format!("  ;; %rank: {}", r)); }
                if !effect.is_pure() {
                    s.push_str(&format!("  ;; %effect: {}", effect));
                }
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
                for k in keys {
                    let v = fields.get(k).unwrap();
                    // Engine anti-peel scaffolding (`_: _` / `_`→Top) is not
                    // user-visible (REAL_04 cocoon_shape law 3). User data
                    // fields named `_` with a non-Top value still print.
                    if is_engine_scaffold_field(k, v) {
                        continue;
                    }
                    s.push_str(&format!("{}  {}: {}\n", pad, k, v.to_nlang(indent + 1)));
                }
                let local = c.local_fields();
                let mut lkeys: Vec<_> = local.keys().collect(); lkeys.sort();
                for k in lkeys {
                    let v = local.get(k).unwrap();
                    if is_engine_scaffold_field(k, v) {
                        continue;
                    }
                    s.push_str(&format!("{}  {}: {}\n", pad, k, v.to_nlang(indent + 1)));
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
                format!("#blur {{ %cause: #{}, %caid: \"{}\" }}", bd.cause.as_str(), caid)
            }
            // Canonical print: `a..b` / `a..b..s`, no spaces (range_eval probes).
            Value::Range { start, end, step } => {
                let mut s = format!("{}..{}", start.to_nlang(0), end.to_nlang(0));
                if let Some(st) = step {
                    s.push_str(&format!("..{}", st.to_nlang(0)));
                }
                s
            }
            _ => format!("{:?}", self),
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
        let _bn_bytes = crate::bn_serial::serialize_bn(self);
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
            // Caused Top hashes as bare Top (CAID / lattice identity).
            Value::Top | Value::TopCaused { .. } => hasher.update([0x00]),
            Value::Atom(kind, effect, rank) => {
                hasher.update([0x01]);
                hasher.update([effect.to_serial_byte()]);
                match kind {
                    AtomKind::Int(i) => { hasher.update([0x01]); let (sign, bytes) = i.to_bytes_be(); hasher.update(&[if sign == num_bigint::Sign::Minus { 1 } else { 0 }]); hasher.update(&bytes); }
                    AtomKind::Float(f) => { hasher.update([0x07]); hasher.update(f.to_bits().to_le_bytes()); }
                    AtomKind::Complex(r, i) => { hasher.update([0x08]); hasher.update(r.to_bits().to_le_bytes()); hasher.update(i.to_bits().to_le_bytes()); }
                    AtomKind::Str(s) => { hasher.update([0x02]); hasher.update(s.as_bytes()); }
                    AtomKind::Tag(t) => { hasher.update([0x03]); hasher.update(t.as_bytes()); }
                    AtomKind::TagStart => { hasher.update([0x04]); }
                    AtomKind::TagEnd => { hasher.update([0x05]); }
                    AtomKind::Bytes(b) => { hasher.update([0x09]); hasher.update(b); }
                    _ => { hasher.update([0x06]); hasher.update(format!("{:?}", kind).as_bytes()); }
                }
                if let Some(r) = rank { hasher.update(r.to_le_bytes()); }
            }
            Value::Combo(c) => {
                hasher.update([0x02]);
                hasher.update([if c.closed { 1 } else { 0 }]);
                hasher.update([c.effect.to_serial_byte()]);
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
            Value::Blur(bd) => {
                hasher.update([0xFD]);
                hasher.update(bd.cause.as_bytes());
                hasher.update(&bd.horizon.fuel_remaining.to_le_bytes());
                hasher.update(&bd.horizon.salt.digest);
            }
            Value::Thunk { expr, closure, context, effect } => {
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
                    let cv_hash = Value::Combo(cv.clone()).content_hash();
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
            Value::Code(expr) => { hasher.update([0x06]); hasher.update(format!("{:?}", expr).as_bytes()); }
            Value::Ref(path) => {
                hasher.update([0x07]);
                match path.anchor {
                    PathAnchor::Bare => hasher.update([0x00]),
                    PathAnchor::Root => hasher.update([0x01]),
                    PathAnchor::Parent(n) => { hasher.update([0x02]); hasher.update(&n.to_le_bytes()); }
                    PathAnchor::Current => hasher.update([0x03]),
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
        // Tag bytes: Standard=0, Refine=1, Pin=2, Squash=3. New kinds only
        // append so existing commit digests stay bit-stable.
        buf.push(match self.kind {
            CommitKind::Standard => 0,
            CommitKind::Refine => 1,
            CommitKind::Pin => 2,
            CommitKind::Squash => 3,
        });
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
