// tier.rs — pipe RHS tier classification (moved from oo/nlint.rs)
//
// Used by:
//   - nlint Tier 1 (R1 rerun-safe, R2 tier C/M/Q/U)
//   - Stage 4 observation memo (force-level tier-based memo strategy)
//
// Based on docs/discussion/019 §2 + GUIDE_03 §11.3

use crate::ast::{Expr, ExprKind, Field, FieldKey, Path, PathAnchor, UnaryOp, StringPart};

/// Right-value form of a pipe.
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

/// Refinement-arrow tier (019 §2).
/// Only meaningful for Transformer-form RHS.
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
    pub fn as_str(&self) -> &'static str {
        match self { Tier::C => "C", Tier::M => "M", Tier::Q => "Q", Tier::U => "U" }
    }
}

// =====================================================================
// §1  RHS form classification
// =====================================================================

/// Classify a pipe RHS into one of four forms.
pub fn classify_rhs(rhs: &Expr) -> RhsForm {
    match &rhs.kind {
        ExprKind::Combo { .. } => RhsForm::Transformer,
        ExprKind::Morphism { .. } => RhsForm::Morphism,
        ExprKind::Path(p) if is_morphism_path(p) => RhsForm::Morphism,
        ExprKind::Atom(_) => RhsForm::Atom,
        _ => RhsForm::Unknown,
    }
}

/// A path is a morphism path iff anchor is Bare and the first segment starts with `/`.
pub fn is_morphism_path(p: &Path) -> bool {
    matches!(p.anchor, PathAnchor::Bare)
        && p.segments.first().map(|s| s.starts_with('/')).unwrap_or(false)
}

// =====================================================================
// §2  R1 — free-$ scan
// =====================================================================

/// Does `expr` contain a free `$`?
///
/// Scan boundaries: `$` rebinds at evolution boundaries (P1),
/// so the scan does NOT descend into:
///   (a) the RHS subtree of a nested Pipe;
///   (b) the body of a Morphism literal (`->`).
/// Interpolation `${...}` DOES descend (P5: interpolation builds no scope).
pub fn has_free_dollar(expr: &Expr) -> bool {
    has_free_dollar_inner(expr, false)
}

fn has_free_dollar_inner(expr: &Expr, in_interp: bool) -> bool {
    match &expr.kind {
        ExprKind::Context => true,
        ExprKind::Pipe(lhs, _rhs) => has_free_dollar_inner(lhs, in_interp),
        ExprKind::Morphism { param, body: _ } => has_free_dollar_inner(param, in_interp),
        ExprKind::Interpolated(parts) => {
            for part in parts {
                if let StringPart::Interpolated(e) = part {
                    if has_free_dollar_inner(e, true) { return true; }
                }
            }
            false
        }
        ExprKind::Combo { fields, .. } => {
            for f in fields { if has_free_dollar_field(f, in_interp) { return true; } }
            false
        }
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
        ExprKind::Path(_) | ExprKind::Atom(_) | ExprKind::Poset(_) => false,
    }
}

fn has_free_dollar_field(field: &Field, in_interp: bool) -> bool {
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
    if !has_free_dollar(rhs) {
        return (Tier::C, None);
    }
    match first_non_whitelist(rhs) {
        Some((desc, span)) => (Tier::Q, Some(format!("{} at span {}..{}", desc, span.0, span.1))),
        None => (Tier::M, None),
    }
}

/// Whitelist for M-tier positive fragment:
///   `$` (Context), Path, Lens, `&` (Meet), `|` (Join), atom literals,
///   combo/list/tuple construction, spread.
fn first_non_whitelist(expr: &Expr) -> Option<(&'static str, (usize, usize))> {
    match &expr.kind {
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
        ExprKind::Lens(a, b) => first_non_whitelist(a).or_else(|| first_non_whitelist(b)),
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
        ExprKind::Pipe(_, _) => Some(("Pipe `|>` (nested)", (expr.span.start, expr.span.end))),
    }
}

fn first_non_whitelist_field(field: &Field) -> Option<(&'static str, (usize, usize))> {
    if let FieldKey::Pattern(e) = &field.key {
        if let Some(r) = first_non_whitelist(e) { return Some(r); }
    }
    first_non_whitelist(&field.value)
}
