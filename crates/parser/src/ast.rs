use num_bigint::BigInt;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub key: FieldKey,
    pub value: Expr,
    /// Source offsets are deliberately absent from format-2 CAS objects.
    /// A deserialized `unknown()` span is a sentinel, never the believable
    /// `{0, 0}` fabricated by `Span::default()`.
    #[serde(default = "Span::unknown", skip_serializing_if = "Span::is_unknown")]
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldKey {
    Named {
        prefix: Option<Prefix>,
        name: String,
    },
    Quoted(String),
    Pattern(Expr),
    Path(Path),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Prefix {
    Data,
    Private,
    Logic,
    Type,
    Meta,
    System,
    Local,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expr {
    pub kind: ExprKind,
    /// See [`Field::span`]: persisted values can have no source coordinate.
    #[serde(default = "Span::unknown", skip_serializing_if = "Span::is_unknown")]
    pub span: Span,
}

impl Expr {
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExprKind {
    Atom(AtomKind),
    Path(Path),
    Apply(Box<Expr>, Box<Expr>),
    Pipe(Box<Expr>, Box<Expr>),
    Morphism {
        param: Box<Expr>,
        body: Box<Expr>,
    },
    Combo {
        fields: Vec<Field>,
        relations: Vec<Relation>,
        closed: bool,
    },
    Meet(Box<Expr>, Box<Expr>),
    Join(Box<Expr>, Box<Expr>),
    Diff(Box<Expr>, Box<Expr>),
    Complement(Box<Expr>),
    Ternary {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Rem(Box<Expr>, Box<Expr>),
    Eq(Box<Expr>, Box<Expr>),
    Ne(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Lte(Box<Expr>, Box<Expr>),
    Gte(Box<Expr>, Box<Expr>),
    /// Lattice-family equality `=` (non-collapsing; distinct from atomic `==`)
    LatticeEq(Box<Expr>, Box<Expr>),
    /// Direction probe `<=>` (returns an order tag, never a boolean; SYNTAX_10)
    Probe(Box<Expr>, Box<Expr>),
    TypeAnnotation(Box<Expr>, Box<Expr>),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    List(Vec<Expr>),
    /// Fixed-arity positional tuple `(a, b)` / 1-tuple `(a,)` — a "numeric cocoon" (SYNTAX_04)
    Tuple(Vec<Expr>),
    /// Poset literal `#{ #a <= #b < #c }` — order chains live only here (SYNTAX_10)
    Poset(Vec<Relation>),
    Lens(Box<Expr>, Box<Expr>),
    AnonSet(Box<Expr>),
    Interpolated(Vec<StringPart>),
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        step: Option<Box<Expr>>,
    },
    Context,
    Spread(Box<Expr>),
    Structural(Box<Expr>),
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
pub struct Relation {
    pub left: AtomKind,
    pub op: RelOp,
    pub right: AtomKind,
    #[serde(default = "Span::unknown", skip_serializing_if = "Span::is_unknown")]
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnaryOp {
    Not,
    Neg,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AtomKind {
    Int(BigInt),
    Float(f64),
    Complex(f64, f64),
    Str(String),
    MultilineStr(String),
    Tag(String),
    TagStart,
    TagEnd,
    Regex(String),
    Top,
    Bottom,
    Unit,
    PathLit(String),
    Bytes(Vec<u8>),
    Uri(String),
    Time(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Path {
    pub anchor: PathAnchor,
    pub segments: Vec<String>,
    #[serde(default = "Span::unknown", skip_serializing_if = "Span::is_unknown")]
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PathAnchor {
    Root,
    Current,
    Parent(u32),
    Bare,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StringPart {
    Literal(String),
    Interpolated(Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Program {
    pub fn canonicalize(&mut self) {
        for field in &mut self.fields {
            field.canonicalize();
        }
        self.fields.sort_by_key(|f| f.key.to_string_canonical());
    }
    pub fn to_nlang(&self) -> String {
        let mut s = String::new();
        for field in &self.fields {
            s.push_str(&field.to_nlang(0));
            s.push('\n');
        }
        s
    }
    /// Zero all spans (golden / roundtrip equality ignores source positions).
    pub fn strip_spans(&mut self) {
        for f in &mut self.fields {
            f.strip_spans();
        }
    }
    pub fn without_spans(&self) -> Self {
        let mut p = self.clone();
        p.strip_spans();
        p
    }
    /// Structural fingerprint for golden-AST snapshots (span-free).
    pub fn shape(&self) -> String {
        let parts: Vec<_> = self.fields.iter().map(|f| f.shape()).collect();
        format!("Program[{}]", parts.join("; "))
    }
}

impl Field {
    pub fn canonicalize(&mut self) {
        self.value.canonicalize();
    }
    pub fn to_nlang(&self, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        format!(
            "{}{}: {}",
            pad,
            self.key.to_string_canonical(),
            self.value.to_nlang(indent)
        )
    }
    pub fn strip_spans(&mut self) {
        self.span = Span::default();
        self.key.strip_spans();
        self.value.strip_spans();
    }
    /// Mark source positions absent for the format-2 CAS projection. This is
    /// distinct from `strip_spans()`: zero is a plausible source coordinate.
    pub fn mark_spans_unknown(&mut self) {
        self.span = Span::unknown();
        self.key.mark_spans_unknown();
        self.value.mark_spans_unknown();
    }
    pub fn shape(&self) -> String {
        format!("{}:{}", self.key.shape(), self.value.shape())
    }
}

impl FieldKey {
    pub fn strip_spans(&mut self) {
        match self {
            FieldKey::Pattern(e) => e.strip_spans(),
            FieldKey::Path(p) => p.strip_spans(),
            _ => {}
        }
    }
    pub fn mark_spans_unknown(&mut self) {
        match self {
            FieldKey::Pattern(e) => e.mark_spans_unknown(),
            FieldKey::Path(p) => p.mark_spans_unknown(),
            _ => {}
        }
    }
    pub fn shape(&self) -> String {
        match self {
            FieldKey::Named { prefix, name } => {
                let p = match prefix {
                    Some(Prefix::Logic) => "/",
                    Some(Prefix::Type) => "@",
                    Some(Prefix::Meta) => "%",
                    Some(Prefix::System) => "~%",
                    Some(Prefix::Private) => "~",
                    Some(Prefix::Local) => "^",
                    Some(Prefix::Data) | None => "",
                };
                format!("Named({}{})", p, name)
            }
            FieldKey::Quoted(s) => format!("Quoted({s})"),
            FieldKey::Pattern(e) => format!("Pattern({})", e.shape()),
            FieldKey::Path(p) => format!("PathKey({})", p.shape()),
        }
    }
}

impl Expr {
    pub fn canonicalize(&mut self) {
        match &mut self.kind {
            ExprKind::Apply(f, a)
            | ExprKind::Pipe(f, a)
            | ExprKind::Meet(f, a)
            | ExprKind::Join(f, a)
            | ExprKind::Diff(f, a)
            | ExprKind::Add(f, a)
            | ExprKind::Sub(f, a)
            | ExprKind::Mul(f, a)
            | ExprKind::Div(f, a)
            | ExprKind::Rem(f, a)
            | ExprKind::Eq(f, a)
            | ExprKind::Ne(f, a)
            | ExprKind::Lt(f, a)
            | ExprKind::Gt(f, a)
            | ExprKind::Lte(f, a)
            | ExprKind::Gte(f, a)
            | ExprKind::TypeAnnotation(f, a)
            | ExprKind::Lens(f, a)
            | ExprKind::LatticeEq(f, a)
            | ExprKind::Probe(f, a) => {
                f.canonicalize();
                a.canonicalize();
            }
            ExprKind::Morphism { param, body } => {
                param.canonicalize();
                body.canonicalize();
            }
            ExprKind::Combo {
                fields, relations, ..
            } => {
                for f in fields.iter_mut() {
                    f.canonicalize();
                }
                fields.sort_by_key(|f| f.key.to_string_canonical());
                relations.sort_by_key(|r| format!("{:?}{:?}{:?}", r.left, r.op, r.right));
            }
            ExprKind::Ternary {
                cond,
                then_branch,
                else_branch,
            } => {
                cond.canonicalize();
                then_branch.canonicalize();
                else_branch.canonicalize();
            }
            ExprKind::Unary { expr, .. }
            | ExprKind::AnonSet(expr)
            | ExprKind::Spread(expr)
            | ExprKind::Structural(expr)
            | ExprKind::Complement(expr) => {
                expr.canonicalize();
            }
            ExprKind::List(items) | ExprKind::Tuple(items) => {
                for i in items {
                    i.canonicalize();
                }
            }
            ExprKind::Poset(relations) => {
                relations.sort_by_key(|r| format!("{:?}{:?}{:?}", r.left, r.op, r.right));
            }
            ExprKind::Interpolated(parts) => {
                for part in parts {
                    if let StringPart::Interpolated(e) = part {
                        e.canonicalize();
                    }
                }
            }
            ExprKind::Range { start, end, step } => {
                start.canonicalize();
                end.canonicalize();
                if let Some(s) = step {
                    s.canonicalize();
                }
            }
            _ => {}
        }
    }
    pub fn strip_spans(&mut self) {
        self.span = Span::default();
        match &mut self.kind {
            ExprKind::Atom(_) | ExprKind::Context => {}
            ExprKind::Path(p) => p.strip_spans(),
            ExprKind::Apply(a, b)
            | ExprKind::Pipe(a, b)
            | ExprKind::Meet(a, b)
            | ExprKind::Join(a, b)
            | ExprKind::Diff(a, b)
            | ExprKind::Add(a, b)
            | ExprKind::Sub(a, b)
            | ExprKind::Mul(a, b)
            | ExprKind::Div(a, b)
            | ExprKind::Rem(a, b)
            | ExprKind::Eq(a, b)
            | ExprKind::Ne(a, b)
            | ExprKind::Lt(a, b)
            | ExprKind::Gt(a, b)
            | ExprKind::Lte(a, b)
            | ExprKind::Gte(a, b)
            | ExprKind::TypeAnnotation(a, b)
            | ExprKind::Lens(a, b)
            | ExprKind::LatticeEq(a, b)
            | ExprKind::Probe(a, b) => {
                a.strip_spans();
                b.strip_spans();
            }
            ExprKind::Morphism { param, body } => {
                param.strip_spans();
                body.strip_spans();
            }
            ExprKind::Combo {
                fields, relations, ..
            } => {
                for f in fields {
                    f.strip_spans();
                }
                for r in relations {
                    r.strip_spans();
                }
            }
            ExprKind::Ternary {
                cond,
                then_branch,
                else_branch,
            } => {
                cond.strip_spans();
                then_branch.strip_spans();
                else_branch.strip_spans();
            }
            ExprKind::Unary { expr, .. }
            | ExprKind::AnonSet(expr)
            | ExprKind::Spread(expr)
            | ExprKind::Structural(expr)
            | ExprKind::Complement(expr) => expr.strip_spans(),
            ExprKind::List(items) | ExprKind::Tuple(items) => {
                for i in items {
                    i.strip_spans();
                }
            }
            ExprKind::Poset(relations) => {
                for r in relations {
                    r.strip_spans();
                }
            }
            ExprKind::Interpolated(parts) => {
                for part in parts {
                    if let StringPart::Interpolated(e) = part {
                        e.strip_spans();
                    }
                }
            }
            ExprKind::Range { start, end, step } => {
                start.strip_spans();
                end.strip_spans();
                if let Some(s) = step {
                    s.strip_spans();
                }
            }
        }
    }
    /// Mark every source coordinate absent for a typed persistence projection.
    /// Serialization then omits these fields via `Span::is_unknown`; no JSON
    /// field-name or object-shape inference is involved.
    pub fn mark_spans_unknown(&mut self) {
        self.span = Span::unknown();
        match &mut self.kind {
            ExprKind::Atom(_) | ExprKind::Context => {}
            ExprKind::Path(p) => p.mark_spans_unknown(),
            ExprKind::Apply(a, b)
            | ExprKind::Pipe(a, b)
            | ExprKind::Meet(a, b)
            | ExprKind::Join(a, b)
            | ExprKind::Diff(a, b)
            | ExprKind::Add(a, b)
            | ExprKind::Sub(a, b)
            | ExprKind::Mul(a, b)
            | ExprKind::Div(a, b)
            | ExprKind::Rem(a, b)
            | ExprKind::Eq(a, b)
            | ExprKind::Ne(a, b)
            | ExprKind::Lt(a, b)
            | ExprKind::Gt(a, b)
            | ExprKind::Lte(a, b)
            | ExprKind::Gte(a, b)
            | ExprKind::TypeAnnotation(a, b)
            | ExprKind::Lens(a, b)
            | ExprKind::LatticeEq(a, b)
            | ExprKind::Probe(a, b) => {
                a.mark_spans_unknown();
                b.mark_spans_unknown();
            }
            ExprKind::Morphism { param, body } => {
                param.mark_spans_unknown();
                body.mark_spans_unknown();
            }
            ExprKind::Combo {
                fields, relations, ..
            } => {
                for f in fields {
                    f.mark_spans_unknown();
                }
                for r in relations {
                    r.mark_spans_unknown();
                }
            }
            ExprKind::Ternary {
                cond,
                then_branch,
                else_branch,
            } => {
                cond.mark_spans_unknown();
                then_branch.mark_spans_unknown();
                else_branch.mark_spans_unknown();
            }
            ExprKind::Unary { expr, .. }
            | ExprKind::AnonSet(expr)
            | ExprKind::Spread(expr)
            | ExprKind::Structural(expr)
            | ExprKind::Complement(expr) => expr.mark_spans_unknown(),
            ExprKind::List(items) | ExprKind::Tuple(items) => {
                for i in items {
                    i.mark_spans_unknown();
                }
            }
            ExprKind::Poset(relations) => {
                for r in relations {
                    r.mark_spans_unknown();
                }
            }
            ExprKind::Interpolated(parts) => {
                for part in parts {
                    if let StringPart::Interpolated(e) = part {
                        e.mark_spans_unknown();
                    }
                }
            }
            ExprKind::Range { start, end, step } => {
                start.mark_spans_unknown();
                end.mark_spans_unknown();
                if let Some(s) = step {
                    s.mark_spans_unknown();
                }
            }
        }
    }
    pub fn without_spans(&self) -> Self {
        let mut e = self.clone();
        e.strip_spans();
        e
    }
    /// Compact structural fingerprint (span-free) for golden-AST tests.
    pub fn shape(&self) -> String {
        match &self.kind {
            ExprKind::Atom(k) => format!("Atom({})", k.shape()),
            ExprKind::Path(p) => format!("Path({})", p.shape()),
            ExprKind::Apply(f, a) => format!("Apply({}, {})", f.shape(), a.shape()),
            ExprKind::Pipe(l, r) => format!("Pipe({}, {})", l.shape(), r.shape()),
            ExprKind::Morphism { param, body } => {
                format!("Morphism({}, {})", param.shape(), body.shape())
            }
            ExprKind::Combo {
                fields,
                relations,
                closed,
            } => {
                let fs: Vec<_> = fields.iter().map(|f| f.shape()).collect();
                let rs: Vec<_> = relations.iter().map(|r| r.shape()).collect();
                format!(
                    "Combo(closed={closed}, [{}], [{}])",
                    fs.join(", "),
                    rs.join(", ")
                )
            }
            ExprKind::Meet(a, b) => format!("Meet({}, {})", a.shape(), b.shape()),
            ExprKind::Join(a, b) => format!("Join({}, {})", a.shape(), b.shape()),
            ExprKind::Diff(a, b) => format!("Diff({}, {})", a.shape(), b.shape()),
            ExprKind::Complement(e) => format!("Complement({})", e.shape()),
            ExprKind::Ternary {
                cond,
                then_branch,
                else_branch,
            } => {
                format!(
                    "Ternary({}, {}, {})",
                    cond.shape(),
                    then_branch.shape(),
                    else_branch.shape()
                )
            }
            ExprKind::Add(a, b) => format!("Add({}, {})", a.shape(), b.shape()),
            ExprKind::Sub(a, b) => format!("Sub({}, {})", a.shape(), b.shape()),
            ExprKind::Mul(a, b) => format!("Mul({}, {})", a.shape(), b.shape()),
            ExprKind::Div(a, b) => format!("Div({}, {})", a.shape(), b.shape()),
            ExprKind::Rem(a, b) => format!("Rem({}, {})", a.shape(), b.shape()),
            ExprKind::Eq(a, b) => format!("Eq({}, {})", a.shape(), b.shape()),
            ExprKind::Ne(a, b) => format!("Ne({}, {})", a.shape(), b.shape()),
            ExprKind::Lt(a, b) => format!("Lt({}, {})", a.shape(), b.shape()),
            ExprKind::Gt(a, b) => format!("Gt({}, {})", a.shape(), b.shape()),
            ExprKind::Lte(a, b) => format!("Lte({}, {})", a.shape(), b.shape()),
            ExprKind::Gte(a, b) => format!("Gte({}, {})", a.shape(), b.shape()),
            ExprKind::LatticeEq(a, b) => format!("LatticeEq({}, {})", a.shape(), b.shape()),
            ExprKind::Probe(a, b) => format!("Probe({}, {})", a.shape(), b.shape()),
            ExprKind::TypeAnnotation(v, t) => {
                format!("TypeAnnotation({}, {})", v.shape(), t.shape())
            }
            ExprKind::Unary { op, expr } => format!("Unary({op:?}, {})", expr.shape()),
            ExprKind::List(items) => {
                let parts: Vec<_> = items.iter().map(|i| i.shape()).collect();
                format!("List([{}])", parts.join(", "))
            }
            ExprKind::Tuple(items) => {
                let parts: Vec<_> = items.iter().map(|i| i.shape()).collect();
                format!("Tuple([{}])", parts.join(", "))
            }
            ExprKind::Poset(relations) => {
                let parts: Vec<_> = relations.iter().map(|r| r.shape()).collect();
                format!("Poset([{}])", parts.join(", "))
            }
            ExprKind::Lens(obj, key) => format!("Lens({}, {})", obj.shape(), key.shape()),
            ExprKind::AnonSet(e) => format!("AnonSet({})", e.shape()),
            ExprKind::Interpolated(parts) => {
                let ps: Vec<_> = parts
                    .iter()
                    .map(|p| match p {
                        StringPart::Literal(l) => format!("Lit({l})"),
                        StringPart::Interpolated(e) => format!("Interp({})", e.shape()),
                    })
                    .collect();
                format!("Interpolated([{}])", ps.join(", "))
            }
            ExprKind::Range { start, end, step } => match step {
                Some(s) => format!("Range({}, {}, {})", start.shape(), end.shape(), s.shape()),
                None => format!("Range({}, {})", start.shape(), end.shape()),
            },
            ExprKind::Context => "Context".to_string(),
            ExprKind::Spread(e) => format!("Spread({})", e.shape()),
            ExprKind::Structural(e) => format!("Structural({})", e.shape()),
        }
    }
    /// Binding strength: higher = tighter. Matches SPEC_14 / n.pest levels.
    pub fn precedence(&self) -> u8 {
        match &self.kind {
            ExprKind::Morphism { .. } => 1,
            ExprKind::Ternary { .. } => 2,
            ExprKind::Pipe(..) => 3,
            ExprKind::Join(..) | ExprKind::Diff(..) => 4,
            ExprKind::Eq(..)
            | ExprKind::Ne(..)
            | ExprKind::Lt(..)
            | ExprKind::Gt(..)
            | ExprKind::Lte(..)
            | ExprKind::Gte(..)
            | ExprKind::LatticeEq(..)
            | ExprKind::Probe(..) => 5,
            ExprKind::Meet(..) => 6,
            ExprKind::Add(..) | ExprKind::Sub(..) => 7,
            ExprKind::Mul(..) | ExprKind::Div(..) | ExprKind::Rem(..) => 8,
            // infix logic `/f` sits between mul and apply; apply is juxtaposition
            ExprKind::Apply(..) => 9,
            ExprKind::TypeAnnotation(..) => 10,
            ExprKind::Unary { .. } | ExprKind::Complement(_) | ExprKind::Spread(_) => 11,
            ExprKind::Lens(..) => 12,
            // atoms / containers / poset / range / structural / context
            _ => 13,
        }
    }

    pub fn to_nlang(&self, indent: usize) -> String {
        self.to_nlang_prec(indent, 0)
    }

    /// Print with parent context precedence for correct parenthesization.
    pub fn to_nlang_prec(&self, indent: usize, parent_prec: u8) -> String {
        let own = self.precedence();
        let pad = "  ".repeat(indent);
        let raw = match &self.kind {
            ExprKind::Atom(kind) => kind.to_string_canonical(),
            ExprKind::Path(path) => format!("{}", path),
            ExprKind::Apply(f, a) => {
                // juxtaposition: parenthesize either side when looser than apply
                let fs = f.to_nlang_prec(indent, own);
                let mut as_ = a.to_nlang_prec(indent, own + 1); // right side: tighter demand
                                                                // Leading `-` → binary Sub (`f -1`); leading `/ident` → logic_infix
                                                                // (`f /g` incomplete). Force grouping (SYNTAX_02 §4.3 / SYNTAX_09 §4.9).
                if as_.starts_with('-') || as_.starts_with('/') {
                    as_ = format!("({as_})");
                }
                format!("{fs} {as_}")
            }
            ExprKind::Pipe(l, r) => {
                format!(
                    "{} |> {}",
                    l.to_nlang_prec(indent, own),
                    r.to_nlang_prec(indent, own + 1)
                )
            }
            ExprKind::Morphism { param, body } => {
                format!(
                    "{} -> {}",
                    param.to_nlang_prec(indent, own + 1),
                    body.to_nlang_prec(indent, own)
                )
            }
            ExprKind::Combo {
                fields,
                relations,
                closed,
            } => {
                if fields.is_empty() && relations.is_empty() {
                    return if *closed { "{{}}" } else { "{}" }.to_string();
                }
                let mut s = if *closed { "{{\n" } else { "{\n" }.to_string();
                for f in fields {
                    s.push_str(&f.to_nlang(indent + 1));
                    s.push('\n');
                }
                for r in relations {
                    let ls = r.left.to_string_canonical();
                    let rs = r.right.to_string_canonical();
                    let os = match r.op {
                        RelOp::Lt => "<",
                        RelOp::Gt => ">",
                        RelOp::Lte => "<=",
                        RelOp::Gte => ">=",
                        RelOp::Eq => "=",
                    };
                    s.push_str(&format!("  {pad}{ls} {os} {rs}\n"));
                }
                s.push_str(&format!("{pad}}}"));
                if *closed {
                    s.push('}');
                }
                s
            }
            ExprKind::Meet(a, b) => format!(
                "{} & {}",
                a.to_nlang_prec(indent, own),
                b.to_nlang_prec(indent, own + 1)
            ),
            ExprKind::Join(a, b) => format!(
                "{} | {}",
                a.to_nlang_prec(indent, own),
                b.to_nlang_prec(indent, own + 1)
            ),
            ExprKind::Diff(a, b) => format!(
                "{} \\ {}",
                a.to_nlang_prec(indent, own),
                b.to_nlang_prec(indent, own + 1)
            ),
            ExprKind::Complement(e) => {
                format!("!{}", e.to_nlang_prec(indent, own))
            }
            // Non-associative: bare `a ? b : c ? d : e` is illegal (SYNTAX_12 §4.1).
            // All three children demand tighter prec so nested ternaries keep parens.
            ExprKind::Ternary {
                cond,
                then_branch,
                else_branch,
            } => format!(
                "{} ? {} : {}",
                cond.to_nlang_prec(indent, own + 1),
                then_branch.to_nlang_prec(indent, own + 1),
                else_branch.to_nlang_prec(indent, own + 1)
            ),
            ExprKind::Add(a, b) => format!(
                "{} + {}",
                a.to_nlang_prec(indent, own),
                b.to_nlang_prec(indent, own + 1)
            ),
            ExprKind::Sub(a, b) => format!(
                "{} - {}",
                a.to_nlang_prec(indent, own),
                b.to_nlang_prec(indent, own + 1)
            ),
            ExprKind::Mul(a, b) => format!(
                "{} * {}",
                a.to_nlang_prec(indent, own),
                b.to_nlang_prec(indent, own + 1)
            ),
            ExprKind::Div(a, b) => format!(
                "{} / {}",
                a.to_nlang_prec(indent, own),
                b.to_nlang_prec(indent, own + 1)
            ),
            ExprKind::Rem(a, b) => format!(
                "{} % {}",
                a.to_nlang_prec(indent, own),
                b.to_nlang_prec(indent, own + 1)
            ),
            // cmp_op is non-associative in the grammar (at most one); both sides
            // demand tighter prec so nested cmp always parenthesizes.
            ExprKind::Eq(a, b) => format!(
                "{} == {}",
                a.to_nlang_prec(indent, own + 1),
                b.to_nlang_prec(indent, own + 1)
            ),
            ExprKind::Ne(a, b) => format!(
                "{} != {}",
                a.to_nlang_prec(indent, own + 1),
                b.to_nlang_prec(indent, own + 1)
            ),
            ExprKind::Lt(a, b) => format!(
                "{} < {}",
                a.to_nlang_prec(indent, own + 1),
                b.to_nlang_prec(indent, own + 1)
            ),
            ExprKind::Gt(a, b) => format!(
                "{} > {}",
                a.to_nlang_prec(indent, own + 1),
                b.to_nlang_prec(indent, own + 1)
            ),
            ExprKind::Lte(a, b) => format!(
                "{} <= {}",
                a.to_nlang_prec(indent, own + 1),
                b.to_nlang_prec(indent, own + 1)
            ),
            ExprKind::Gte(a, b) => format!(
                "{} >= {}",
                a.to_nlang_prec(indent, own + 1),
                b.to_nlang_prec(indent, own + 1)
            ),
            ExprKind::LatticeEq(a, b) => format!(
                "{} = {}",
                a.to_nlang_prec(indent, own + 1),
                b.to_nlang_prec(indent, own + 1)
            ),
            ExprKind::Probe(a, b) => format!(
                "{} <=> {}",
                a.to_nlang_prec(indent, own + 1),
                b.to_nlang_prec(indent, own + 1)
            ),
            // type_ann_op = "@" (not ":"); colon is field assignment only.
            ExprKind::TypeAnnotation(v, t) => format!(
                "{} @ {}",
                v.to_nlang_prec(indent, own + 1),
                t.to_nlang_prec(indent, own + 1)
            ),
            ExprKind::Unary { op, expr } => {
                let s = match op {
                    UnaryOp::Not => "!",
                    UnaryOp::Neg => "-",
                };
                format!("{s}{}", expr.to_nlang_prec(indent, own))
            }
            ExprKind::List(items) => {
                let parts: Vec<_> = items.iter().map(|i| i.to_nlang_prec(indent, 0)).collect();
                format!("[{}]", parts.join(", "))
            }
            ExprKind::Tuple(items) => {
                let parts: Vec<_> = items.iter().map(|i| i.to_nlang_prec(indent, 0)).collect();
                if parts.len() == 1 {
                    format!("({},)", parts[0])
                } else {
                    format!("({})", parts.join(", "))
                }
            }
            ExprKind::Poset(relations) => {
                let parts: Vec<_> = relations
                    .iter()
                    .map(|r| {
                        let os = match r.op {
                            RelOp::Lt => "<",
                            RelOp::Gt => ">",
                            RelOp::Lte => "<=",
                            RelOp::Gte => ">=",
                            RelOp::Eq => "=",
                        };
                        format!(
                            "{} {} {}",
                            r.left.to_string_canonical(),
                            os,
                            r.right.to_string_canonical()
                        )
                    })
                    .collect();
                format!("#{{ {} }}", parts.join(", "))
            }
            // Always use subscript form so Lens ≉ Path roundtrips (dot form
            // re-parses as a multi-segment Path).
            ExprKind::Lens(obj, key) => {
                let os = obj.to_nlang_prec(indent, own);
                format!("{os}[{}]", key.to_nlang_prec(indent, 0))
            }
            ExprKind::AnonSet(e) => format!("@{{ {} }}", e.to_nlang_prec(indent, 0)),
            ExprKind::Interpolated(parts) => {
                let mut s = "`".to_string();
                for part in parts {
                    match part {
                        StringPart::Literal(l) => s.push_str(l),
                        StringPart::Interpolated(e) => {
                            s.push_str(&format!("${{{}}}", e.to_nlang_prec(indent, 0)))
                        }
                    }
                }
                s.push('`');
                s
            }
            ExprKind::Range { start, end, step } => {
                let mut res = format!(
                    "{}..{}",
                    start.to_nlang_prec(indent, 0),
                    end.to_nlang_prec(indent, 0)
                );
                if let Some(s) = step {
                    res.push_str(&format!("..{}", s.to_nlang_prec(indent, 0)));
                }
                res
            }
            ExprKind::Context => "$".to_string(),
            ExprKind::Spread(e) => format!("...{}", e.to_nlang_prec(indent, own)),
            ExprKind::Structural(e) => format!("<<{}>>", e.to_nlang_prec(indent, 0)),
        };
        // Parenthesize when this node binds looser than what the parent expects.
        if own < parent_prec {
            format!("({raw})")
        } else {
            raw
        }
    }
}

impl Relation {
    pub fn strip_spans(&mut self) {
        self.span = Span::default();
    }
    pub fn mark_spans_unknown(&mut self) {
        self.span = Span::unknown();
    }
    pub fn shape(&self) -> String {
        let os = match self.op {
            RelOp::Lt => "<",
            RelOp::Gt => ">",
            RelOp::Lte => "<=",
            RelOp::Gte => ">=",
            RelOp::Eq => "=",
        };
        format!("{}{}{}", self.left.shape(), os, self.right.shape())
    }
}

impl Path {
    pub fn strip_spans(&mut self) {
        self.span = Span::default();
    }
    pub fn mark_spans_unknown(&mut self) {
        self.span = Span::unknown();
    }
    pub fn shape(&self) -> String {
        format!("{:?}:{}", self.anchor, self.segments.join("."))
    }
}

impl AtomKind {
    pub fn shape(&self) -> String {
        match self {
            AtomKind::Int(i) => format!("Int({i})"),
            AtomKind::Float(f) => format!("Float({f})"),
            AtomKind::Complex(r, i) => format!("Complex({r},{i})"),
            AtomKind::Str(s) => format!("Str({s})"),
            AtomKind::MultilineStr(s) => format!("MultilineStr({s})"),
            AtomKind::Tag(t) => format!("Tag({t})"),
            AtomKind::TagStart => "TagStart".into(),
            AtomKind::TagEnd => "TagEnd".into(),
            AtomKind::Regex(s) => format!("Regex({s})"),
            AtomKind::Top => "Top".into(),
            AtomKind::Bottom => "Bottom".into(),
            AtomKind::Unit => "Unit".into(),
            AtomKind::PathLit(s) => format!("PathLit({s})"),
            AtomKind::Bytes(b) => format!("Bytes({b:?})"),
            AtomKind::Uri(s) => format!("Uri({s})"),
            AtomKind::Time(s) => format!("Time({s})"),
        }
    }
    pub fn to_string_canonical(&self) -> String {
        match self {
            AtomKind::Int(i) => i.to_string(),
            // Always keep a decimal so Float roundtrips as float (not Int).
            AtomKind::Float(f) => {
                let s = f.to_string();
                if s.contains('.') || s.contains('e') || s.contains('E') {
                    s
                } else {
                    format!("{s}.0")
                }
            }
            AtomKind::Complex(r, i) => {
                // Prefer pure-imag forms that re-parse as a single complex atom.
                if *r == 0.0 {
                    if *i == 1.0 {
                        return "i".into();
                    }
                    if *i == -1.0 {
                        return "-i".into();
                    }
                    if *i >= 0.0 {
                        return format!("{i}i");
                    }
                    return format!("{i}i"); // already signed, e.g. -3i
                }
                if *i >= 0.0 {
                    format!("{r}+{i}i")
                } else {
                    format!("{r}{i}i")
                } // i already signed
            }
            AtomKind::Str(s) => format!("\"{}\"", s),
            AtomKind::MultilineStr(s) => format!("\"\"\"{}\"\"\"", s),
            AtomKind::Tag(t) => format!("#{}", t),
            AtomKind::TagStart => "#_|_".to_string(),
            AtomKind::TagEnd => "#_".to_string(),
            AtomKind::Top => "_".to_string(),
            AtomKind::Bottom => "_|_".to_string(),
            AtomKind::Unit => "()".to_string(),
            AtomKind::Regex(s) => format!("r\"{}\"", s),
            AtomKind::PathLit(s) => format!("p\"{}\"", s),
            // Raw payload bytes as latin-1-ish chars (parser stores as_bytes of content).
            AtomKind::Bytes(b) => {
                let s: String = b.iter().map(|&c| c as char).collect();
                format!("b\"{s}\"")
            }
            AtomKind::Uri(s) => format!("u\"{}\"", s),
            AtomKind::Time(s) => format!("t\"{}\"", s),
        }
    }
}

impl FieldKey {
    pub fn to_string_canonical(&self) -> String {
        match self {
            FieldKey::Named { prefix, name } => {
                let p = match prefix {
                    Some(Prefix::Logic) => "/",
                    Some(Prefix::Type) => "@",
                    Some(Prefix::Meta) => "%",
                    Some(Prefix::System) => "~%",
                    Some(Prefix::Private) => "~",
                    Some(Prefix::Local) => "^",
                    _ => "",
                };
                format!("{}{}", p, name)
            }
            FieldKey::Quoted(s) => {
                if s.contains('"') || s.contains('\n') || s.contains('\r') {
                    format!("\"\"\"{}\"\"\"", s.replace("\"\"\"", "\\\"\"\""))
                } else {
                    format!("\"{}\"", s)
                }
            }
            FieldKey::Pattern(e) => e.to_nlang(0),
            FieldKey::Path(p) => format!("{}", p),
        }
    }
}

impl Span {
    /// A persisted AST was intentionally detached from its source text.
    ///
    /// `usize::MAX` cannot be a source offset in a real input on supported
    /// hosts, so it cannot be mistaken for the first byte of a source file.
    /// This keeps format-2's omitted spans distinct from `Span::default()`,
    /// which is still used by synthetic ASTs with a genuine zero coordinate.
    pub fn unknown() -> Self {
        Self {
            start: usize::MAX,
            end: usize::MAX,
        }
    }

    pub fn is_unknown(&self) -> bool {
        self.start == usize::MAX && self.end == usize::MAX
    }

    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
    pub fn merge(a: Span, b: Span) -> Self {
        Self {
            start: a.start.min(b.start),
            end: a.end.max(b.end),
        }
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Grammar: anchor_root = "_." ; anchor_parent = "^"+ ~ "."
        // Parent(0) ⇔ `^.`, Parent(1) ⇔ `^^.`, …
        match self.anchor {
            PathAnchor::Root => write!(f, "_.")?,
            PathAnchor::Current => write!(f, "^.")?,
            PathAnchor::Parent(n) => write!(f, "{}.", "^".repeat(n as usize + 1))?,
            PathAnchor::Bare => {}
        }
        write!(f, "{}", self.segments.join("."))
    }
}

impl Path {
    pub fn to_key(&self) -> String {
        format!("{}", self)
    }
}
