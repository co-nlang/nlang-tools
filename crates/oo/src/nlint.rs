// nlint.rs — Tier 1 linter for n/ (pure syntax / pure graph theory)
//
// Implements docs/linter_tier1_handover.md:
//   D1: three static rules (R1 rerun-safe, R2 tier C/M/Q/U, R3 sealed x keyed = static ⊥)
//   D2: context graph + clique number ω(G) + K4/K5 witnesses
//   D3: CLI + JSON schema (tier1-v1)
//
// Discipline: parser AST only — no eval, no type inference, no q/ω data.
// Firewall: K4/K5 are "candidate sites" only; no obstruction claims at Tier 1.

use nlang_parser::ast::{Expr, ExprKind, Field, FieldKey, Path, PathAnchor, Program, UnaryOp, StringPart};
use std::collections::{BTreeSet, HashSet, HashMap};
use std::path::{Path as FsPath, PathBuf};
use std::fs;

// =====================================================================
// §0  Types
// =====================================================================

/// Right-value form of a pipe (handover §2 first classification).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RhsForm {
    /// combo/cocoon literal
    Transformer,
    /// `->` literal or `/`-prefixed path
    Morphism,
    /// atom literal (#tag, int, str, …)
    Atom,
    /// everything else — do not classify, do not guess
    Unknown,
}

/// Refinement-arrow tier (019 §2; handover §2 R2).
/// Only meaningful for Transformer-form RHS; other forms get no R1/R2/R3 diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier {
    /// $-free transformer (or atom form) — idempotent nucleus
    C,
    /// positive fragment — monotone + shrinking; idempotence not guaranteed
    M,
    /// full language — monotonicity generally broken
    Q,
    /// RHS form Unknown — no classification
    U,
}

impl Tier {
    fn as_str(&self) -> &'static str {
        match self { Tier::C => "C", Tier::M => "M", Tier::Q => "Q", Tier::U => "U" }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub rule: String,          // "R1" | "R2" | "R3" | "SPEC15-*"
    pub severity: Severity,
    pub loc: Loc,
    pub tier: Option<Tier>,    // Some for R1/R2 (transformer forms); None otherwise
    pub demotion_reason: Option<String>,
    pub msg: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity { Info, Warn, Error }

#[derive(Debug, Clone)]
pub struct Loc {
    pub file: String,
    pub span: (usize, usize),
}

// =====================================================================
// §1  RHS form classification
// =====================================================================

/// Classify a pipe RHS into one of four forms (handover §2).
pub fn classify_rhs(rhs: &Expr) -> RhsForm {
    match &rhs.kind {
        // combo/cocoon literal → transformer
        ExprKind::Combo { .. } => RhsForm::Transformer,
        // morphism literal `param -> body`
        ExprKind::Morphism { .. } => RhsForm::Morphism,
        // `/`-prefixed path: `/f`, `/foo.bar`
        ExprKind::Path(p) if is_morphism_path(p) => RhsForm::Morphism,
        // atom literal
        ExprKind::Atom(_) => RhsForm::Atom,
        // everything else
        _ => RhsForm::Unknown,
    }
}

/// A path is a morphism path iff anchor is Bare and the first segment starts with `/`.
/// (Parser keeps the `/` prefix inside the segment string — see parser/src/lib.rs:346-349.)
fn is_morphism_path(p: &Path) -> bool {
    matches!(p.anchor, PathAnchor::Bare)
        && p.segments.first().map(|s| s.starts_with('/')).unwrap_or(false)
}

// =====================================================================
// §2  R1 — free-$ scan + rerun-safe marking
// =====================================================================

/// Does `expr` contain a free `$`?
///
/// Scan boundaries (handover §2 R1): `$` rebinds at evolution boundaries (P1),
/// so the scan does NOT descend into:
///   (a) the RHS subtree of a nested Pipe;
///   (b) the body of a Morphism literal (`->`).
/// Interpolation `${...}` DOES descend (P5: interpolation builds no scope).
pub fn has_free_dollar(expr: &Expr) -> bool {
    has_free_dollar_inner(expr, false)
}

/// `in_interp` tracks whether we are inside a `${...}` (P5: still descends,
/// but a `$` reached there is free w.r.t. the enclosing pipe).
fn has_free_dollar_inner(expr: &Expr, in_interp: bool) -> bool {
    match &expr.kind {
        ExprKind::Context => true,
        // (a) nested Pipe: scan LHS freely, but do NOT descend into RHS subtree
        ExprKind::Pipe(lhs, _rhs) => has_free_dollar_inner(lhs, in_interp),
        // (b) Morphism literal: `$` in body is bound to the morphism's own param,
        //     not free for the enclosing context. Do not descend into body.
        //     Param is a pattern expr; a `$` there would be structural, scan it.
        ExprKind::Morphism { param, body: _ } => has_free_dollar_inner(param, in_interp),
        // Interpolation: P5 — descends, `$` is free w.r.t. enclosing pipe.
        ExprKind::Interpolated(parts) => {
            for part in parts {
                if let StringPart::Interpolated(e) = part {
                    if has_free_dollar_inner(e, true) { return true; }
                }
            }
            false
        }
        // Combo: scan each field's value. A `$` in a field value is free
        // (the combo literal itself is not an evolution boundary).
        ExprKind::Combo { fields, .. } => {
            for f in fields { if has_free_dollar_field(f, in_interp) { return true; } }
            false
        }
        // Recurse into all other composite forms.
        ExprKind::Apply(f, a) => has_free_dollar_inner(f, in_interp) || has_free_dollar_inner(a, in_interp),
        ExprKind::Meet(a, b) | ExprKind::Join(a, b) | ExprKind::Diff(a, b)
        | ExprKind::Add(a, b) | ExprKind::Sub(a, b) | ExprKind::Mul(a, b)
        | ExprKind::Div(a, b) | ExprKind::Rem(a, b)
        | ExprKind::Eq(a, b) | ExprKind::Ne(a, b) | ExprKind::Lt(a, b)
        | ExprKind::Gt(a, b) | ExprKind::Lte(a, b) | ExprKind::Gte(a, b)
        | ExprKind::LatticeEq(a, b) | ExprKind::Probe(a, b)
        | ExprKind::TypeAnnotation(a, b) | ExprKind::Lens(a, b) => {
            has_free_dollar_inner(a, in_interp) || has_free_dollar_inner(b, in_interp)
        }
        ExprKind::Ternary { cond, then_branch, else_branch } => {
            has_free_dollar_inner(cond, in_interp)
                || has_free_dollar_inner(then_branch, in_interp)
                || has_free_dollar_inner(else_branch, in_interp)
        }
        ExprKind::Unary { expr: e, .. }
        | ExprKind::AnonSet(e) | ExprKind::Spread(e) | ExprKind::Structural(e)
        | ExprKind::Complement(e) => has_free_dollar_inner(e, in_interp),
        ExprKind::List(items) | ExprKind::Tuple(items) => {
            items.iter().any(|i| has_free_dollar_inner(i, in_interp))
        }
        ExprKind::Range { start, end, step } => {
            has_free_dollar_inner(start, in_interp)
                || has_free_dollar_inner(end, in_interp)
                || step.as_ref().map_or(false, |s| has_free_dollar_inner(s, in_interp))
        }
        // Path / Atom / Poset / Context already handled: no free $ beyond Context itself.
        ExprKind::Path(_) | ExprKind::Atom(_) | ExprKind::Poset(_) => false,
    }
}

fn has_free_dollar_field(field: &Field, in_interp: bool) -> bool {
    // field key: a `$` could appear in a Pattern key; scan it.
    let key_has = match &field.key {
        FieldKey::Pattern(e) => has_free_dollar_inner(e, in_interp),
        _ => false,
    };
    key_has || has_free_dollar_inner(&field.value, in_interp)
}

// =====================================================================
// §3  R2 — tier classification (positive-fragment whitelist)
// =====================================================================

/// Classify a transformer RHS into a tier with optional demotion reason.
/// Precondition: `rhs` is a combo/cocoon literal (Transformer form).
pub fn classify_tier(rhs: &Expr) -> (Tier, Option<String>) {
    // R1: $-free → C
    if !has_free_dollar(rhs) {
        return (Tier::C, None);
    }
    // Has $: walk the whitelist. First non-whitelist node → Q with demotion reason.
    match first_non_whitelist(rhs) {
        Some((desc, span)) => (Tier::Q, Some(format!("{} at span {}..{}", desc, span.0, span.1))),
        None => (Tier::M, None),
    }
}

/// Whitelist (handover §2 R2 M row):
///   `$` (Context), Path, Lens, `&` (Meet), `|` (Join), atom literals,
///   combo/list/tuple construction, spread.
/// Anything else → not in positive fragment.
///
/// Note: Diff `\`, Complement `!`, Ternary, Apply, Morphism, arithmetic,
/// comparisons, interpolation, AnonSet, Range, Structural, TypeAnnotation,
/// Poset, Probe, LatticeEq are all outside the whitelist.
fn first_non_whitelist(expr: &Expr) -> Option<(&'static str, (usize, usize))> {
    match &expr.kind {
        // Whitelisted leaves / constructs:
        ExprKind::Context => None,
        ExprKind::Path(_) => None,
        ExprKind::Atom(_) => None,
        ExprKind::Combo { fields, .. } => {
            for f in fields {
                if let Some(r) = first_non_whitelist_field(f) { return Some(r); }
            }
            None
        }
        ExprKind::List(items) | ExprKind::Tuple(items) => {
            for i in items { if let Some(r) = first_non_whitelist(i) { return Some(r); } }
            None
        }
        ExprKind::Spread(e) => first_non_whitelist(e),
        ExprKind::Meet(a, b) | ExprKind::Join(a, b) => {
            first_non_whitelist(a).or_else(|| first_non_whitelist(b))
        }
        // Whitelist Lens (path/lens projection)
        ExprKind::Lens(a, b) => first_non_whitelist(a).or_else(|| first_non_whitelist(b)),

        // Non-whitelist nodes — report the first one:
        ExprKind::Apply(_, _) => Some(("Apply (morphism application)", (expr.span.start, expr.span.end))),
        ExprKind::Morphism { .. } => Some(("Morphism literal `->`", (expr.span.start, expr.span.end))),
        ExprKind::Ternary { .. } => Some(("Ternary `? :`", (expr.span.start, expr.span.end))),
        ExprKind::Diff(_, _) => Some(("Diff `\\` (not in M whitelist: only & and |)", (expr.span.start, expr.span.end))),
        ExprKind::Complement(_) => Some(("Complement `!` (anti-monotone)", (expr.span.start, expr.span.end))),
        ExprKind::Unary { op, .. } => Some((match op { UnaryOp::Not => "Unary `!`", UnaryOp::Neg => "Unary `-` (arithmetic)" }, (expr.span.start, expr.span.end))),
        ExprKind::Add(_, _) => Some(("Add `+` (arithmetic)", (expr.span.start, expr.span.end))),
        ExprKind::Sub(_, _) => Some(("Sub `-` (arithmetic)", (expr.span.start, expr.span.end))),
        ExprKind::Mul(_, _) => Some(("Mul `*` (arithmetic)", (expr.span.start, expr.span.end))),
        ExprKind::Div(_, _) => Some(("Div `/` (arithmetic)", (expr.span.start, expr.span.end))),
        ExprKind::Rem(_, _) => Some(("Rem `%` (arithmetic)", (expr.span.start, expr.span.end))),
        ExprKind::Eq(_, _) => Some(("Eq `==` (booleanizing comparison)", (expr.span.start, expr.span.end))),
        ExprKind::Ne(_, _) => Some(("Ne `!=` (booleanizing comparison)", (expr.span.start, expr.span.end))),
        ExprKind::Lt(_, _) => Some(("Lt `<` (booleanizing comparison)", (expr.span.start, expr.span.end))),
        ExprKind::Gt(_, _) => Some(("Gt `>` (booleanizing comparison)", (expr.span.start, expr.span.end))),
        ExprKind::Lte(_, _) => Some(("Lte `<=` (booleanizing comparison)", (expr.span.start, expr.span.end))),
        ExprKind::Gte(_, _) => Some(("Gte `>=` (booleanizing comparison)", (expr.span.start, expr.span.end))),
        ExprKind::LatticeEq(_, _) => Some(("LatticeEq `=` (lattice-family boolean track)", (expr.span.start, expr.span.end))),
        ExprKind::Probe(_, _) => Some(("Probe `<=>` (direction probe)", (expr.span.start, expr.span.end))),
        ExprKind::TypeAnnotation(_, _) => Some(("TypeAnnotation `@`", (expr.span.start, expr.span.end))),
        ExprKind::Interpolated(_) => Some(("Interpolated string (string building — non-monotone)", (expr.span.start, expr.span.end))),
        ExprKind::AnonSet(_) => Some(("AnonSet `@{}`", (expr.span.start, expr.span.end))),
        ExprKind::Range { .. } => Some(("Range `..`", (expr.span.start, expr.span.end))),
        ExprKind::Structural(_) => Some(("Structural `<<>>`", (expr.span.start, expr.span.end))),
        ExprKind::Poset(_) => Some(("Poset literal `#{}`", (expr.span.start, expr.span.end))),
        // Pipe should not appear inside a transformer RHS at this level
        // (it would be a nested pipe; still non-whitelist if it does appear outside
        // the scan-boundary rule — but the whitelist walk is independent of $-scan;
        // a nested pipe in a transformer makes the transformer non-monotone).
        ExprKind::Pipe(_, _) => Some(("Pipe `|>` (nested)", (expr.span.start, expr.span.end))),
    }
}

fn first_non_whitelist_field(field: &Field) -> Option<(&'static str, (usize, usize))> {
    if let FieldKey::Pattern(e) = &field.key {
        if let Some(r) = first_non_whitelist(e) { return Some(r); }
    }
    first_non_whitelist(&field.value)
}

// =====================================================================
// §4  R3 — sealed LHS × keyed transformer = static ⊥
// =====================================================================

/// R3 (handover §2 R3; 018 §5.2; SYNTAX_12 §4 #7):
///   LHS is a tuple-literal or cocoon-literal, RHS is transformer-form,
///   and the transformer's key-set ⊄ LHS literal's key-set
///   ⟹ diagnose static `_|_ #missing_key`.
///
/// LHS non-literal → pass through (no inference, no guessing).
/// Transformer with spread → key-set uncertain → do not fire (honest approximation).
pub fn check_r3(lhs: &Expr, rhs: &Expr) -> Option<(String, String)> {
    // Only transformer-form RHS.
    if !matches!(classify_rhs(rhs), RhsForm::Transformer) { return None; }
    // LHS must be a *sealed* literal: tuple or cocoon (closed combo).
    let lhs_keys = sealed_literal_keys(lhs)?;
    // Transformer key-set (explicit field keys only; spread → None → bail).
    let rhs_keys = transformer_keys(rhs)?;
    // Fire when rhs_keys ⊄ lhs_keys.
    let extra: Vec<_> = rhs_keys.iter().filter(|k| !lhs_keys.contains(*k)).collect();
    if extra.is_empty() {
        None
    } else {
        let msg = format!(
            "static `_|_ #missing_key`: sealed LHS does not contain key(s) [{}]; \
             use morphism evolution or `{{ ...t }}` explicit unbox (SYNTAX_12 §4 #7)",
            extra.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        );
        let demotion = format!("R3 sealed-keyed: transformer adds [{}] to sealed LHS", extra.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
        Some((msg, demotion))
    }
}

/// Keys of a sealed literal: tuple (positional "0","1",…) or cocoon (closed combo).
/// Returns None if `lhs` is not a sealed literal (combo open, list, path, atom, …).
fn sealed_literal_keys(lhs: &Expr) -> Option<HashSet<String>> {
    match &lhs.kind {
        ExprKind::Tuple(items) => {
            let keys: HashSet<String> = (0..items.len()).map(|i| i.to_string()).collect();
            Some(keys)
        }
        ExprKind::Combo { fields, closed: true, .. } => {
            // Cocoon: sealed. Keys = explicit named/path keys.
            Some(combo_field_keys(fields))
        }
        // Open combo, list, anon_set, atom, path, … → not sealed
        _ => None,
    }
}

/// Explicit field keys of a combo (no spread). Returns None if a spread is present
/// (key-set uncertain). Caller decides whether to bail.
fn transformer_keys(rhs: &Expr) -> Option<HashSet<String>> {
    match &rhs.kind {
        ExprKind::Combo { fields, .. } => {
            let mut keys = HashSet::new();
            let mut has_spread = false;
            for f in fields {
                match &f.key {
                    FieldKey::Named { name, .. } => { keys.insert(name.clone()); }
                    FieldKey::Quoted(s) => { keys.insert(s.clone()); }
                    FieldKey::Path(p) => { keys.insert(p.to_key()); }
                    FieldKey::Pattern(_) => {
                        // pattern key — not a plain key; treat as uncertain
                        has_spread = true;
                    }
                }
                // detect spread-as-field (`...x` is parsed as FieldKey::Quoted("...") with a Spread value)
                if let FieldKey::Quoted(s) = &f.key {
                    if s == "..." { has_spread = true; }
                }
                if let ExprKind::Spread(_) = &f.value.kind {
                    has_spread = true;
                }
            }
            if has_spread { None } else { Some(keys) }
        }
        _ => None,
    }
}

/// Combo field keys (for cocoon LHS — pattern keys map to their string form).
fn combo_field_keys(fields: &[Field]) -> HashSet<String> {
    let mut keys = HashSet::new();
    for f in fields {
        match &f.key {
            FieldKey::Named { name, .. } => { keys.insert(name.clone()); }
            FieldKey::Quoted(s) => {
                if s != "..." { keys.insert(s.clone()); }
            }
            FieldKey::Path(p) => { keys.insert(p.to_key()); }
            FieldKey::Pattern(e) => { keys.insert(e.to_nlang(0)); }
        }
    }
    keys
}

// =====================================================================
// §5  D2 — context graph + ω(G) + K4/K5 witnesses
// =====================================================================

#[derive(Debug, Clone)]
pub struct ContextNode {
    /// root-relative coordinate path if statically derivable, else local key
    pub coord_path: String,
    pub approx_local_key: bool,
    pub file: String,
    pub span: (usize, usize),
    /// the coordinates (full root-relative or local-key strings) owned by this context
    pub own_coords: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub struct GraphReport {
    pub contexts: Vec<ContextNode>,
    pub coordinates: BTreeSet<String>,
    /// incidence: context index → set of coordinate indices
    pub incidence: Vec<BTreeSet<usize>>,
    /// coordinate index → set of context indices
    pub coord_to_contexts: Vec<BTreeSet<usize>>,
    pub edges: usize,
    pub omega: usize,
    pub k4_witnesses: Vec<CliqueWitness>,
    pub k5_witnesses: Vec<CliqueWitness>,
    pub components: usize,
    pub degree_dist: HashMap<usize, usize>,
    pub approximations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CliqueWitness {
    pub contexts: Vec<usize>,        // context indices
    pub shared_coords: Vec<String>,  // coordinates shared by all members
}

pub fn build_context_graph(program: &Program, file: &str) -> GraphReport {
    let mut contexts: Vec<ContextNode> = Vec::new();
    let mut coord_set: BTreeSet<String> = Vec::new().into_iter().collect();

    // Walk the program; for each combo/cocoon literal occurrence, register a context
    // and record its coordinates (root-relative where derivable).
    for field in &program.fields {
        let prefix = field_key_prefix_str(&field.key);
        collect_contexts(&field.value, &prefix, file, &mut contexts, &mut coord_set);
    }

    // Build incidence.
    let coord_index: HashMap<String, usize> = coord_set.iter().cloned().enumerate().map(|(i, c)| (c, i)).collect();
    let n_coords = coord_set.len();
    let mut incidence: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); contexts.len()];
    let mut coord_to_contexts: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); n_coords];
    for (ci, ctx) in contexts.iter().enumerate() {
        // A context's coordinates = its own field keys, joined with its coord_path prefix.
        // We already collected them as full root-relative strings in collect_contexts.
        for c in &ctx.own_coords {
            if let Some(&k) = coord_index.get(c) {
                incidence[ci].insert(k);
                coord_to_contexts[k].insert(ci);
            }
        }
    }

    // Context graph G: edge between contexts sharing ≥1 coordinate.
    let n_ctx = contexts.len();
    let mut adj: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); n_ctx];
    let mut edges = 0usize;
    for k in 0..n_coords {
        let cs: Vec<usize> = coord_to_contexts[k].iter().copied().collect();
        for i in 0..cs.len() {
            for j in (i+1)..cs.len() {
                let a = cs[i]; let b = cs[j];
                if adj[a].insert(b) { edges += 1; }
                adj[b].insert(a);
            }
        }
    }

    // ω(G) via Bron–Kerbosch with pivot.
    let max_cliques = bron_kerbosch(&adj);

    // All 4-cliques and 5-cliques (including non-maximal ones contained in larger cliques).
    // We enumerate 4- and 5-subsets of each maximal clique of size ≥ 4.
    let mut k4: HashSet<BTreeSet<usize>> = HashSet::new();
    let mut k5: HashSet<BTreeSet<usize>> = HashSet::new();
    for mc in &max_cliques {
        if mc.len() >= 4 {
            for combo in combinations(mc, 4) { k4.insert(combo); }
        }
        if mc.len() >= 5 {
            for combo in combinations(mc, 5) { k5.insert(combo); }
        }
    }
    // Also: a 5-clique contains 4-cliques, but those are already covered above
    // (each 5-subset of a ≥5 maximal clique yields its own 4-subsets via the ≥4 branch).
    // To be safe, also expand 4-subsets out of every 5-clique we just recorded:
    let k5_owned: Vec<BTreeSet<usize>> = k5.iter().cloned().collect();
    for mc in &k5_owned {
        for combo in combinations(&mc.iter().copied().collect::<Vec<_>>(), 4) { k4.insert(combo); }
    }

    let k4_witnesses: Vec<CliqueWitness> = k4.into_iter().map(|set| witness_of(&set, &contexts, &coord_index)).collect();
    let k5_witnesses: Vec<CliqueWitness> = k5.into_iter().map(|set| witness_of(&set, &contexts, &coord_index)).collect();

    let omega = max_cliques.iter().map(|c| c.len()).max().unwrap_or(0);

    // Connected components (on context graph).
    let components = count_components(&adj);

    // Degree distribution.
    let mut degree_dist: HashMap<usize, usize> = HashMap::new();
    for a in &adj {
        *degree_dist.entry(a.len()).or_insert(0) += 1;
    }

    // Tier 1 always uses local-key identity for coordinates (handover §3
    // "同名即同座標是一個已知的過近似"). The approximation flag is always set
    // when there is at least one context.
    let approximations = if contexts.is_empty() {
        vec![]
    } else {
        vec!["local-key-identity".to_string()]
    };

    GraphReport {
        contexts,
        coordinates: coord_set,
        incidence,
        coord_to_contexts,
        edges,
        omega,
        k4_witnesses,
        k5_witnesses,
        components,
        degree_dist,
        approximations,
    }
}

fn witness_of(set: &BTreeSet<usize>, contexts: &[ContextNode], coord_index: &HashMap<String, usize>) -> CliqueWitness {
    let ctxs: Vec<usize> = set.iter().copied().collect();
    // shared coords = intersection of each member's coord set.
    let mut shared: BTreeSet<String> = BTreeSet::new();
    if let Some(&first) = ctxs.first() {
        // recompute first's coords
        let first_coords: BTreeSet<String> = contexts[first].own_coords.iter().cloned().collect();
        shared = first_coords;
        for &ci in ctxs.iter().skip(1) {
            let cs: BTreeSet<String> = contexts[ci].own_coords.iter().cloned().collect();
            shared = shared.intersection(&cs).cloned().collect();
        }
    }
    let _ = coord_index; // (kept for future use)
    CliqueWitness {
        contexts: ctxs,
        shared_coords: shared.into_iter().collect(),
    }
}

/// Recursive collector: walk the AST and register each combo/cocoon literal as a context.
///
/// Coordinate = the field key's canonical string *within* the combo (local key).
/// (handover §3: "座標 ＝ 欄位鍵的正準字串"; "同名即同座標是一個已知的過近似" —
/// i.e. coordinates are local key names, not root-relative paths. The root-relative
/// path of the context itself is recorded separately in `coord_path` for inspection.)
///
/// `prefix` tracks the root-relative location of the *context* (for human-readable
/// reporting), but coordinates are always local keys.
fn collect_contexts(
    expr: &Expr,
    prefix: &Option<String>,
    file: &str,
    contexts: &mut Vec<ContextNode>,
    coord_set: &mut BTreeSet<String>,
) {
    match &expr.kind {
        ExprKind::Combo { fields, .. } => {
            // Register this combo as a context.
            // Coordinates = local field keys (the over-approximation handover §3 flags).
            let own_coords: BTreeSet<String> = fields.iter().map(|f| field_key_local_str(&f.key)).filter(|s| s != "...").collect();
            for c in &own_coords { coord_set.insert(c.clone()); }
            let ctx = ContextNode {
                coord_path: prefix.clone().unwrap_or_else(|| "<local>".to_string()),
                approx_local_key: true, // Tier 1 always uses local-key identity
                file: file.to_string(),
                span: (expr.span.start, expr.span.end),
                own_coords,
            };
            contexts.push(ctx);

            // Recurse into fields with extended prefix (for nested context locations).
            for f in fields {
                let child_prefix = child_prefix(prefix, &f.key);
                collect_contexts(&f.value, &child_prefix, file, contexts, coord_set);
                if let FieldKey::Pattern(e) = &f.key {
                    collect_contexts(e, prefix, file, contexts, coord_set);
                }
            }
        }
        // Non-combo composite forms: recurse with prefix=None (can't extend path through them).
        ExprKind::List(items) | ExprKind::Tuple(items) => {
            for i in items { collect_contexts(i, &None, file, contexts, coord_set); }
        }
        ExprKind::Apply(f, a) => {
            collect_contexts(f, &None, file, contexts, coord_set);
            collect_contexts(a, &None, file, contexts, coord_set);
        }
        ExprKind::Pipe(lhs, rhs) => {
            collect_contexts(lhs, &None, file, contexts, coord_set);
            collect_contexts(rhs, &None, file, contexts, coord_set);
        }
        ExprKind::Morphism { param, body } => {
            collect_contexts(param, &None, file, contexts, coord_set);
            collect_contexts(body, &None, file, contexts, coord_set);
        }
        ExprKind::Meet(a, b) | ExprKind::Join(a, b) | ExprKind::Diff(a, b)
        | ExprKind::Add(a, b) | ExprKind::Sub(a, b) | ExprKind::Mul(a, b)
        | ExprKind::Div(a, b) | ExprKind::Rem(a, b)
        | ExprKind::Eq(a, b) | ExprKind::Ne(a, b) | ExprKind::Lt(a, b)
        | ExprKind::Gt(a, b) | ExprKind::Lte(a, b) | ExprKind::Gte(a, b)
        | ExprKind::LatticeEq(a, b) | ExprKind::Probe(a, b)
        | ExprKind::TypeAnnotation(a, b) | ExprKind::Lens(a, b) => {
            collect_contexts(a, &None, file, contexts, coord_set);
            collect_contexts(b, &None, file, contexts, coord_set);
        }
        ExprKind::Ternary { cond, then_branch, else_branch } => {
            collect_contexts(cond, &None, file, contexts, coord_set);
            collect_contexts(then_branch, &None, file, contexts, coord_set);
            collect_contexts(else_branch, &None, file, contexts, coord_set);
        }
        ExprKind::Unary { expr: e, .. }
        | ExprKind::AnonSet(e) | ExprKind::Spread(e) | ExprKind::Structural(e)
        | ExprKind::Complement(e) => {
            collect_contexts(e, &None, file, contexts, coord_set);
        }
        ExprKind::Range { start, end, step } => {
            collect_contexts(start, &None, file, contexts, coord_set);
            collect_contexts(end, &None, file, contexts, coord_set);
            if let Some(s) = step { collect_contexts(s, &None, file, contexts, coord_set); }
        }
        ExprKind::Interpolated(parts) => {
            for p in parts {
                if let StringPart::Interpolated(e) = p {
                    collect_contexts(e, &None, file, contexts, coord_set);
                }
            }
        }
        // Leaves: no contexts.
        ExprKind::Path(_) | ExprKind::Atom(_) | ExprKind::Poset(_) | ExprKind::Context => {}
    }
}

fn field_key_prefix_str(key: &FieldKey) -> Option<String> {
    match key {
        FieldKey::Named { name, .. } => Some(name.clone()),
        FieldKey::Quoted(s) => if s == "..." { None } else { Some(s.clone()) },
        FieldKey::Path(p) => Some(p.to_key()),
        FieldKey::Pattern(e) => Some(e.to_nlang(0)),
    }
}

fn field_key_local_str(key: &FieldKey) -> String {
    match key {
        FieldKey::Named { name, .. } => name.clone(),
        FieldKey::Quoted(s) => s.clone(),
        FieldKey::Path(p) => p.to_key(),
        FieldKey::Pattern(e) => e.to_nlang(0),
    }
}

fn child_prefix(parent: &Option<String>, key: &FieldKey) -> Option<String> {
    let k = field_key_prefix_str(key)?;
    match parent {
        Some(p) => Some(format!("{}.{}", p, k)),
        None => Some(k),
    }
}

// Bron–Kerbosch with pivot — returns all maximal cliques.
fn bron_kerbosch(adj: &[BTreeSet<usize>]) -> Vec<Vec<usize>> {
    let n = adj.len();
    let mut r: BTreeSet<usize> = BTreeSet::new();
    let mut p: BTreeSet<usize> = (0..n).collect();
    let mut x: BTreeSet<usize> = BTreeSet::new();
    let mut out: Vec<Vec<usize>> = Vec::new();
    bron_kerbosch_inner(adj, &mut r, &mut p, &mut x, &mut out);
    out
}

fn bron_kerbosch_inner(
    adj: &[BTreeSet<usize>],
    r: &mut BTreeSet<usize>,
    p: &mut BTreeSet<usize>,
    x: &mut BTreeSet<usize>,
    out: &mut Vec<Vec<usize>>,
) {
    if p.is_empty() && x.is_empty() {
        out.push(r.iter().copied().collect());
        return;
    }
    // pivot = any node in p ∪ x
    let pivot = p.iter().chain(x.iter()).next().copied();
    if let Some(u) = pivot {
        let neighbors: BTreeSet<usize> = adj[u].iter().copied().collect();
        // branch on p \ N(u)
        let candidates: Vec<usize> = p.difference(&neighbors).copied().collect();
        for v in candidates {
            let nv: BTreeSet<usize> = adj[v].iter().copied().collect();
            let mut new_r = r.clone(); new_r.insert(v);
            let mut new_p: BTreeSet<usize> = p.intersection(&nv).copied().collect();
            let mut new_x: BTreeSet<usize> = x.intersection(&nv).copied().collect();
            bron_kerbosch_inner(adj, &mut new_r, &mut new_p, &mut new_x, out);
            p.remove(&v);
            x.insert(v);
        }
    }
}

fn combinations(items: &[usize], k: usize) -> Vec<BTreeSet<usize>> {
    let n = items.len();
    if k > n { return vec![]; }
    let mut out = Vec::new();
    let mut idx: Vec<usize> = (0..k).collect();
    loop {
        let set: BTreeSet<usize> = idx.iter().map(|&i| items[i]).collect();
        out.push(set);
        // increment
        let mut i = k as isize - 1;
        while i >= 0 {
            let ii = i as usize;
            if idx[ii] < n - k + ii {
                idx[ii] += 1;
                for j in (ii+1)..k { idx[j] = idx[j-1] + 1; }
                break;
            }
            i -= 1;
        }
        if i < 0 { break; }
    }
    out
}

fn count_components(adj: &[BTreeSet<usize>]) -> usize {
    let n = adj.len();
    let mut visited = vec![false; n];
    let mut comps = 0;
    for s in 0..n {
        if !visited[s] {
            comps += 1;
            let mut stack = vec![s];
            while let Some(u) = stack.pop() {
                if visited[u] { continue; }
                visited[u] = true;
                for &v in adj[u].iter() {
                    if !visited[v] { stack.push(v); }
                }
            }
        }
    }
    comps
}

// =====================================================================
// §6  Per-file analysis: walk all pipes, emit diagnostics + graph
// =====================================================================

#[derive(Debug, Clone)]
pub struct FileReport {
    pub file: String,
    pub diagnostics: Vec<Diagnostic>,
    pub graph: GraphReport,
    pub parse_error: Option<String>,
}

pub fn analyze_file(path: &FsPath) -> FileReport {
    let file = path.to_string_lossy().to_string();
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return FileReport { file, diagnostics: vec![], graph: empty_graph(), parse_error: Some(format!("read error: {}", e)) },
    };
    let program = match nlang_parser::parse_program(&content) {
        Ok(p) => p,
        Err(e) => return FileReport { file, diagnostics: vec![], graph: empty_graph(), parse_error: Some(format!("parse error: {}", e)) },
    };
    let mut diags = Vec::new();
    walk_pipes(&program, &file, &mut diags);
    // SPEC_15 checks (existing static_analyzer) — keep, prefix rule names.
    {
        let mut analyzer = crate::static_analyzer::StaticAnalyzer::new();
        let violations = analyzer.analyze(&program.fields);
        for v in violations {
            diags.push(Diagnostic {
                rule: format!("SPEC15-{:?}", v_kind(&v)),
                severity: Severity::Warn,
                loc: Loc { file: file.clone(), span: (v.line(), v.line()) },
                tier: None,
                demotion_reason: None,
                msg: v.message(),
            });
        }
    }
    let graph = build_context_graph(&program, &file);
    FileReport { file, diagnostics: diags, graph, parse_error: None }
}

fn v_kind(v: &crate::static_analyzer::StaticViolation) -> &'static str {
    use crate::static_analyzer::StaticViolation::*;
    match v {
        RandomnessInjection { .. } => "RandomnessInjection",
        EnvironmentDependency { .. } => "EnvironmentDependency",
        PrivateAccessViolation { .. } => "PrivateAccessViolation",
        PotentialInfiniteRecursion { .. } => "PotentialInfiniteRecursion",
        TypeConflict { .. } => "TypeConflict",
    }
}

fn empty_graph() -> GraphReport {
    GraphReport {
        contexts: vec![], coordinates: BTreeSet::new(), incidence: vec![], coord_to_contexts: vec![],
        edges: 0, omega: 0, k4_witnesses: vec![], k5_witnesses: vec![],
        components: 0, degree_dist: HashMap::new(), approximations: vec![],
    }
}

fn walk_pipes(program: &Program, file: &str, diags: &mut Vec<Diagnostic>) {
    for field in &program.fields {
        walk_pipes_expr(&field.value, file, diags);
        if let FieldKey::Pattern(e) = &field.key {
            walk_pipes_expr(e, file, diags);
        }
    }
}

fn walk_pipes_expr(expr: &Expr, file: &str, diags: &mut Vec<Diagnostic>) {
    match &expr.kind {
        ExprKind::Pipe(lhs, rhs) => {
            // Visit this pipe.
            visit_pipe(lhs, rhs, file, diags);
            // Recurse into LHS (it may contain nested pipes).
            walk_pipes_expr(lhs, file, diags);
            // Recurse into RHS (nested pipes inside RHS).
            walk_pipes_expr(rhs, file, diags);
        }
        ExprKind::Combo { fields, .. } => {
            for f in fields {
                walk_pipes_expr(&f.value, file, diags);
                if let FieldKey::Pattern(e) = &f.key { walk_pipes_expr(e, file, diags); }
            }
        }
        ExprKind::Apply(f, a) => { walk_pipes_expr(f, file, diags); walk_pipes_expr(a, file, diags); }
        ExprKind::Morphism { param, body } => { walk_pipes_expr(param, file, diags); walk_pipes_expr(body, file, diags); }
        ExprKind::Meet(a, b) | ExprKind::Join(a, b) | ExprKind::Diff(a, b)
        | ExprKind::Add(a, b) | ExprKind::Sub(a, b) | ExprKind::Mul(a, b)
        | ExprKind::Div(a, b) | ExprKind::Rem(a, b)
        | ExprKind::Eq(a, b) | ExprKind::Ne(a, b) | ExprKind::Lt(a, b)
        | ExprKind::Gt(a, b) | ExprKind::Lte(a, b) | ExprKind::Gte(a, b)
        | ExprKind::LatticeEq(a, b) | ExprKind::Probe(a, b)
        | ExprKind::TypeAnnotation(a, b) | ExprKind::Lens(a, b) => {
            walk_pipes_expr(a, file, diags); walk_pipes_expr(b, file, diags);
        }
        ExprKind::Ternary { cond, then_branch, else_branch } => {
            walk_pipes_expr(cond, file, diags);
            walk_pipes_expr(then_branch, file, diags);
            walk_pipes_expr(else_branch, file, diags);
        }
        ExprKind::Unary { expr: e, .. }
        | ExprKind::AnonSet(e) | ExprKind::Spread(e) | ExprKind::Structural(e)
        | ExprKind::Complement(e) => walk_pipes_expr(e, file, diags),
        ExprKind::List(items) | ExprKind::Tuple(items) => {
            for i in items { walk_pipes_expr(i, file, diags); }
        }
        ExprKind::Range { start, end, step } => {
            walk_pipes_expr(start, file, diags); walk_pipes_expr(end, file, diags);
            if let Some(s) = step { walk_pipes_expr(s, file, diags); }
        }
        ExprKind::Interpolated(parts) => {
            for p in parts { if let StringPart::Interpolated(e) = p { walk_pipes_expr(e, file, diags); } }
        }
        ExprKind::Path(_) | ExprKind::Atom(_) | ExprKind::Poset(_) | ExprKind::Context => {}
    }
}

fn visit_pipe(lhs: &Expr, rhs: &Expr, file: &str, diags: &mut Vec<Diagnostic>) {
    let form = classify_rhs(rhs);
    let loc = Loc { file: file.to_string(), span: (rhs.span.start, rhs.span.end) };

    // R3 (sealed × keyed) — applies to transformer-form RHS only.
    if matches!(form, RhsForm::Transformer) {
        if let Some((msg, demotion)) = check_r3(lhs, rhs) {
            diags.push(Diagnostic {
                rule: "R3".to_string(),
                severity: Severity::Error,
                loc: loc.clone(),
                tier: None,
                demotion_reason: Some(demotion),
                msg,
            });
        }
    }

    // Atom form: constant refinement — Tier C by 019 Prop 2 (acceptance fix:
    // handover §2 R1 named transformers only, but 019 §2 explicitly includes
    // the atomic form in Tier C).
    if matches!(form, RhsForm::Atom) {
        diags.push(Diagnostic {
            rule: "R1".to_string(),
            severity: Severity::Info,
            loc: loc.clone(),
            tier: Some(Tier::C),
            demotion_reason: None,
            msg: "rerun-safe: atomic collapse is a constant refinement (idempotent nucleus)".to_string(),
        });
        diags.push(Diagnostic {
            rule: "R2".to_string(),
            severity: Severity::Info,
            loc: loc.clone(),
            tier: Some(Tier::C),
            demotion_reason: None,
            msg: "tier C: atomic collapse (constant refinement)".to_string(),
        });
        return;
    }

    // Unknown form: emit tier U so downstream schedulers (GUIDE_03 §11.3) see
    // the pipe at all; conservative handling = same as Q. Morphism form stays
    // silent by design (free Kleisli arrow — refinement tiers do not apply).
    if matches!(form, RhsForm::Unknown) {
        diags.push(Diagnostic {
            rule: "R2".to_string(),
            severity: Severity::Info,
            loc: loc.clone(),
            tier: Some(Tier::U),
            demotion_reason: None,
            msg: "tier U: RHS form not statically classifiable (treat as Q for scheduling)".to_string(),
        });
        return;
    }

    // R1 + R2 — only for transformer-form RHS.
    if matches!(form, RhsForm::Transformer) {
        let (tier, demotion) = classify_tier(rhs);
        // R1: rerun-safe (Tier C) — info-level.
        if tier == Tier::C {
            diags.push(Diagnostic {
                rule: "R1".to_string(),
                severity: Severity::Info,
                loc: loc.clone(),
                tier: Some(Tier::C),
                demotion_reason: None,
                msg: "rerun-safe: transformer RHS is $-free (idempotent nucleus)".to_string(),
            });
        }
        // R2: tier classification — info for M, warn for Q.
        let sev = match tier { Tier::C | Tier::M => Severity::Info, Tier::Q => Severity::Warn, Tier::U => Severity::Info };
        diags.push(Diagnostic {
            rule: "R2".to_string(),
            severity: sev,
            loc: loc.clone(),
            tier: Some(tier),
            demotion_reason: demotion.clone(),
            msg: match tier {
                Tier::C => "tier C: $-free constant refinement".to_string(),
                Tier::M => "tier M: positive fragment (monotone + shrinking; idempotence not guaranteed)".to_string(),
                Tier::Q => format!("tier Q: outside positive fragment — {}", demotion.as_deref().unwrap_or("")),
                Tier::U => "tier U: RHS form unknown".to_string(),
            },
        });
    }
}

// =====================================================================
// §7  Output: JSON (tier1-v1) + human-readable
// =====================================================================

pub fn report_to_json(reports: &[FileReport]) -> serde_json::Value {
    let mut diagnostics = serde_json::json!([]);
    let mut all_diagnostics = Vec::new();
    let mut agg_contexts = 0usize;
    let mut agg_coordinates = 0usize;
    let mut agg_edges = 0usize;
    let mut agg_omega = 0usize;
    let mut agg_k4 = 0usize;
    let mut agg_k5 = 0usize;
    let mut agg_components = 0usize;
    let mut all_approx: Vec<String> = Vec::new();

    for r in reports {
        for d in &r.diagnostics {
            all_diagnostics.push(serde_json::json!({
                "rule": d.rule,
                "severity": match d.severity { Severity::Info => "info", Severity::Warn => "warn", Severity::Error => "error" },
                "loc": { "file": d.loc.file, "span": [d.loc.span.0, d.loc.span.1] },
                "tier": d.tier.map(|t| t.as_str()),
                "demotion_reason": d.demotion_reason,
                "msg": d.msg,
            }));
        }
        agg_contexts += r.graph.contexts.len();
        agg_coordinates += r.graph.coordinates.len();
        agg_edges += r.graph.edges;
        agg_omega = agg_omega.max(r.graph.omega);
        agg_k4 += r.graph.k4_witnesses.len();
        agg_k5 += r.graph.k5_witnesses.len();
        agg_components += r.graph.components;
        all_approx.extend(r.graph.approximations.clone());
    }
    all_approx.sort(); all_approx.dedup();

    diagnostics = serde_json::Value::Array(all_diagnostics);

    serde_json::json!({
        "version": "tier1-v1",
        "diagnostics": diagnostics,
        "graph": {
            "contexts": agg_contexts,
            "coordinates": agg_coordinates,
            "edges": agg_edges,
            "omega": agg_omega,
            "k4_witnesses": agg_k4,
            "k5_witnesses": agg_k5,
            "components": agg_components,
            "approximations": all_approx,
        },
        // Per-file detail (for inspection; Tier 2 will read aggregate fields).
        "files": reports.iter().map(|r| serde_json::json!({
            "file": r.file,
            "parse_error": r.parse_error,
            "graph": {
                "contexts": r.graph.contexts.len(),
                "coordinates": r.graph.coordinates.len(),
                "edges": r.graph.edges,
                "omega": r.graph.omega,
                "context_nodes": r.graph.contexts.iter().enumerate().map(|(i, c)| serde_json::json!({
                    "index": i, "coord_path": c.coord_path, "span": [c.span.0, c.span.1],
                    "approx_local_key": c.approx_local_key,
                })).collect::<Vec<_>>(),
                "k4_witnesses": r.graph.k4_witnesses.iter().map(|w| serde_json::json!({
                    "contexts": w.contexts, "shared_coords": w.shared_coords,
                })).collect::<Vec<_>>(),
                "k5_witnesses": r.graph.k5_witnesses.iter().map(|w| serde_json::json!({
                    "contexts": w.contexts, "shared_coords": w.shared_coords,
                })).collect::<Vec<_>>(),
                "components": r.graph.components,
                "degree_dist": r.graph.degree_dist,
                "approximations": r.graph.approximations,
            }
        })).collect::<Vec<_>>(),
        "footer": "graph facts only; no obstruction claims at Tier 1",
    })
}

pub fn report_to_human(reports: &[FileReport]) -> String {
    let mut s = String::new();
    let mut total_diags = 0;
    let mut total_errors = 0;
    let mut total_warns = 0;
    let mut total_infos = 0;
    let mut agg_omega = 0;
    let mut agg_k4 = 0;
    let mut agg_k5 = 0;

    for r in reports {
        if let Some(e) = &r.parse_error {
            s.push_str(&format!("[{}] PARSE-SKIP: {}\n", r.file, e));
            continue;
        }
        if r.diagnostics.is_empty() && r.graph.contexts.is_empty() {
            continue;
        }
        s.push_str(&format!("=== {} ===\n", r.file));
        for d in &r.diagnostics {
            total_diags += 1;
            let sev = match d.severity { Severity::Info => { total_infos += 1; "INFO" } Severity::Warn => { total_warns += 1; "WARN" } Severity::Error => { total_errors += 1; "ERROR" } };
            let tier = d.tier.map(|t| format!("[tier {}]", t.as_str())).unwrap_or_default();
            s.push_str(&format!("  {} {} {} ({}..{}): {}\n", sev, d.rule, tier, d.loc.span.0, d.loc.span.1, d.msg));
            if let Some(reason) = &d.demotion_reason {
                s.push_str(&format!("      demotion: {}\n", reason));
            }
        }
        let g = &r.graph;
        agg_omega = agg_omega.max(g.omega);
        agg_k4 += g.k4_witnesses.len();
        agg_k5 += g.k5_witnesses.len();
        s.push_str(&format!("  graph: {} contexts, {} coords, {} edges, ω(G)={}, K4={}, K5={}, components={}\n",
            g.contexts.len(), g.coordinates.len(), g.edges, g.omega,
            g.k4_witnesses.len(), g.k5_witnesses.len(), g.components));
        let fmt_witness = |w: &CliqueWitness| -> String {
            let members: Vec<String> = w.contexts.iter().map(|&i| {
                let c = &g.contexts[i];
                format!("{}@{}..{}", c.coord_path, c.span.0, c.span.1)
            }).collect();
            format!("    • {{{}}} share [{}]\n", members.join(", "), w.shared_coords.join(", "))
        };
        if !g.k4_witnesses.is_empty() {
            s.push_str("  K4 candidate sites (no obstruction claims at Tier 1):\n");
            for w in &g.k4_witnesses { s.push_str(&fmt_witness(w)); }
        }
        if !g.k5_witnesses.is_empty() {
            s.push_str("  K5 candidate sites (no obstruction claims at Tier 1):\n");
            for w in &g.k5_witnesses { s.push_str(&fmt_witness(w)); }
        }
        if !g.approximations.is_empty() {
            s.push_str(&format!("  approximations: {}\n", g.approximations.join(", ")));
        }
        s.push('\n');
    }
    s.push_str("--- summary ---\n");
    s.push_str(&format!("diagnostics: {} ({} error, {} warn, {} info)\n", total_diags, total_errors, total_warns, total_infos));
    s.push_str(&format!("max ω(G) across files: {} | K4 sites: {} | K5 sites: {}\n", agg_omega, agg_k4, agg_k5));
    s.push_str("graph facts only; no obstruction claims at Tier 1\n");
    s
}

pub fn has_r3_error(reports: &[FileReport]) -> bool {
    reports.iter().any(|r| r.diagnostics.iter().any(|d| d.rule == "R3" && d.severity == Severity::Error))
}

pub fn has_any_diagnostic(reports: &[FileReport]) -> bool {
    reports.iter().any(|r| !r.diagnostics.is_empty())
}

// =====================================================================
// §8  CLI entry
// =====================================================================

pub fn run_cli(path: &FsPath, json: bool) -> i32 {
    let mut files: Vec<PathBuf> = Vec::new();
    if path.is_dir() {
        collect_n_files(path, &mut files);
    } else {
        files.push(path.to_path_buf());
    }
    let mut reports: Vec<FileReport> = Vec::new();
    for f in &files {
        reports.push(analyze_file(f));
    }
    if json {
        let j = report_to_json(&reports);
        println!("{}", serde_json::to_string_pretty(&j).unwrap_or_else(|_| "{}".to_string()));
    } else {
        print!("{}", report_to_human(&reports));
    }
    if has_r3_error(&reports) { 2 } else if has_any_diagnostic(&reports) { 1 } else { 0 }
}

fn collect_n_files(dir: &FsPath, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_n_files(&p, out);
            } else if p.extension().and_then(|s| s.to_str()) == Some("n") {
                out.push(p);
            }
        }
    }
}
