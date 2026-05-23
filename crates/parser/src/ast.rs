use serde::{Deserialize, Serialize};
use std::fmt;
use num_bigint::BigInt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub key: FieldKey,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldKey {
    Named { prefix: Option<Prefix>, name: String },
    Quoted(String),
    Pattern(Expr),
    Path(Path),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Prefix { Data, Private, Logic, Type, Meta, System, Local }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expr {
    pub kind: ExprKind,
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
    Morphism { param: Box<Expr>, body: Box<Expr> },
    Combo { fields: Vec<Field>, relations: Vec<Relation>, closed: bool },
    Meet(Box<Expr>, Box<Expr>),
    Join(Box<Expr>, Box<Expr>),
    Diff(Box<Expr>, Box<Expr>),
    Complement(Box<Expr>),
    Ternary { cond: Box<Expr>, then_branch: Box<Expr>, else_branch: Box<Expr> },
    Add(Box<Expr>, Box<Expr>), Sub(Box<Expr>, Box<Expr>), Mul(Box<Expr>, Box<Expr>), Div(Box<Expr>, Box<Expr>), Rem(Box<Expr>, Box<Expr>),
    Eq(Box<Expr>, Box<Expr>), Ne(Box<Expr>, Box<Expr>), Lt(Box<Expr>, Box<Expr>), Gt(Box<Expr>, Box<Expr>), Lte(Box<Expr>, Box<Expr>), Gte(Box<Expr>, Box<Expr>),
    TypeAnnotation(Box<Expr>, Box<Expr>),
    Unary { op: UnaryOp, expr: Box<Expr> },
    List(Vec<Expr>),
    Lens(Box<Expr>, Box<Expr>),
    AnonSet(Box<Expr>),
    Interpolated(Vec<StringPart>),
    Range { start: Box<Expr>, end: Box<Expr>, step: Option<Box<Expr>> },
    Context,
    Spread(Box<Expr>),
    Structural(Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelOp { Lt, Gt, Lte, Gte }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    pub left: AtomKind,
    pub op: RelOp,
    pub right: AtomKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnaryOp { Not, Neg }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AtomKind {
    Int(BigInt), Float(f64), Complex(f64, f64), Str(String), MultilineStr(String), Tag(String), TagStart, TagEnd, Regex(String),
    Top, Bottom, Unit, PathLit(String), Bytes(Vec<u8>), Uri(String), Time(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Path {
    pub anchor: PathAnchor,
    pub segments: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PathAnchor { Root, Current, Parent(u32), Bare }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StringPart { Literal(String), Interpolated(Box<Expr>) }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Span { pub start: usize, pub end: usize }

impl Program {
    pub fn canonicalize(&mut self) {
        for field in &mut self.fields { field.canonicalize(); }
        self.fields.sort_by_key(|f| f.key.to_string_canonical());
    }
    pub fn to_nlang(&self) -> String {
        let mut s = String::new();
        for field in &self.fields { s.push_str(&field.to_nlang(0)); s.push('\n'); }
        s
    }
}

impl Field {
    pub fn canonicalize(&mut self) { self.value.canonicalize(); }
    pub fn to_nlang(&self, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        format!("{}{}: {}", pad, self.key.to_string_canonical(), self.value.to_nlang(indent))
    }
}

impl Expr {
    pub fn canonicalize(&mut self) {
        match &mut self.kind {
            ExprKind::Apply(f, a) | ExprKind::Pipe(f, a) | ExprKind::Meet(f, a) | ExprKind::Join(f, a) | ExprKind::Diff(f, a) |
            ExprKind::Add(f, a) | ExprKind::Sub(f, a) | ExprKind::Mul(f, a) | ExprKind::Div(f, a) | ExprKind::Rem(f, a) |
            ExprKind::Eq(f, a) | ExprKind::Ne(f, a) | ExprKind::Lt(f, a) | ExprKind::Gt(f, a) | ExprKind::Lte(f, a) | ExprKind::Gte(f, a) |
            ExprKind::TypeAnnotation(f, a) | ExprKind::Lens(f, a) => { f.canonicalize(); a.canonicalize(); }
            ExprKind::Morphism { param, body } => { param.canonicalize(); body.canonicalize(); }
            ExprKind::Combo { fields, relations, .. } => {
                for f in fields.iter_mut() { f.canonicalize(); }
                fields.sort_by_key(|f| f.key.to_string_canonical());
                relations.sort_by_key(|r| format!("{:?}{:?}{:?}", r.left, r.op, r.right));
            }
            ExprKind::Ternary { cond, then_branch, else_branch } => { cond.canonicalize(); then_branch.canonicalize(); else_branch.canonicalize(); }
            ExprKind::Unary { expr, .. } | ExprKind::AnonSet(expr) | ExprKind::Spread(expr) | ExprKind::Structural(expr) | ExprKind::Complement(expr) => { expr.canonicalize(); }
            ExprKind::List(items) => { for i in items { i.canonicalize(); } }
            ExprKind::Interpolated(parts) => { for part in parts { if let StringPart::Interpolated(e) = part { e.canonicalize(); } } }
            ExprKind::Range { start, end, step } => { start.canonicalize(); end.canonicalize(); if let Some(s) = step { s.canonicalize(); } }
            _ => {}
        }
    }
    pub fn to_nlang(&self, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        match &self.kind {
            ExprKind::Atom(kind) => kind.to_string_canonical(),
            ExprKind::Path(path) => format!("{}", path),
            ExprKind::Apply(f, a) => {
                let fs = f.to_nlang(indent);
                let mut as_ = a.to_nlang(indent);
                if !matches!(a.kind, ExprKind::Atom(_) | ExprKind::Path(_) | ExprKind::Combo{..} | ExprKind::List(_) | ExprKind::Lens(..)) { as_ = format!("({})", as_); }
                format!("{} {}", fs, as_)
            }
            ExprKind::Pipe(l, r) => format!("{} |> {}", l.to_nlang(indent), r.to_nlang(indent)),
            ExprKind::Morphism { param, body } => format!("{} -> {}", param.to_nlang(indent), body.to_nlang(indent)),
            ExprKind::Combo { fields, relations, closed } => {
                if fields.is_empty() && relations.is_empty() { return if *closed { "{{}}" } else { "{}" }.to_string(); }
                let mut s = if *closed { "{{\n" } else { "{\n" }.to_string();
                for f in fields { s.push_str(&f.to_nlang(indent + 1)); s.push('\n'); }
                for r in relations {
                    let ls = r.left.to_string_canonical();
                    let rs = r.right.to_string_canonical();
                    let os = match r.op { RelOp::Lt => "<", RelOp::Gt => ">", RelOp::Lte => "<=", RelOp::Gte => ">=" };
                    s.push_str(&format!("  {}{} {} {}\n", pad, ls, os, rs));
                }
                s.push_str(&format!("{}}}", pad)); if *closed { s.push('}'); }
                s
            }
            ExprKind::Meet(a, b) => format!("({} & {})", a.to_nlang(indent), b.to_nlang(indent)),
            ExprKind::Join(a, b) => format!("({} | {})", a.to_nlang(indent), b.to_nlang(indent)),
            ExprKind::Diff(a, b) => format!("({} \\ {})", a.to_nlang(indent), b.to_nlang(indent)),
            ExprKind::Complement(e) => format!("!{}", e.to_nlang(indent)),
            ExprKind::Ternary { cond, then_branch, else_branch } => format!("({} ? {} : {})", cond.to_nlang(indent), then_branch.to_nlang(indent), else_branch.to_nlang(indent)),
            ExprKind::Add(a, b) => format!("({} + {})", a.to_nlang(indent), b.to_nlang(indent)),
            ExprKind::Sub(a, b) => format!("({} - {})", a.to_nlang(indent), b.to_nlang(indent)),
            ExprKind::Mul(a, b) => format!("({} * {})", a.to_nlang(indent), b.to_nlang(indent)),
            ExprKind::Div(a, b) => format!("({} / {})", a.to_nlang(indent), b.to_nlang(indent)),
            ExprKind::Rem(a, b) => format!("({} % {})", a.to_nlang(indent), b.to_nlang(indent)),
            ExprKind::Eq(a, b) => format!("({} == {})", a.to_nlang(indent), b.to_nlang(indent)),
            ExprKind::Ne(a, b) => format!("({} != {})", a.to_nlang(indent), b.to_nlang(indent)),
            ExprKind::Lt(a, b) => format!("({} < {})", a.to_nlang(indent), b.to_nlang(indent)),
            ExprKind::Gt(a, b) => format!("({} > {})", a.to_nlang(indent), b.to_nlang(indent)),
            ExprKind::Lte(a, b) => format!("({} <= {})", a.to_nlang(indent), b.to_nlang(indent)),
            ExprKind::Gte(a, b) => format!("({} >= {})", a.to_nlang(indent), b.to_nlang(indent)),
            ExprKind::TypeAnnotation(v, t) => format!("{}: {}", t.to_nlang(indent), v.to_nlang(indent)),
            ExprKind::Unary { op, expr } => { let s = match op { UnaryOp::Not => "!", UnaryOp::Neg => "-" }; format!("{}{}", s, expr.to_nlang(indent)) }
            ExprKind::List(items) => { let parts: Vec<_> = items.iter().map(|i| i.to_nlang(indent)).collect(); format!("[{}]", parts.join(", ")) }
            ExprKind::Lens(obj, key) => {
                let mut os = obj.to_nlang(indent);
                if matches!(obj.kind, ExprKind::Apply(..) | ExprKind::Add(..) | ExprKind::Sub(..) | ExprKind::Mul(..) | ExprKind::Div(..) | ExprKind::Rem(..) | ExprKind::Eq(..) | ExprKind::Ne(..)) { os = format!("({})", os); }
                if let ExprKind::Atom(AtomKind::Str(ref s)) = key.kind {
                    if !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '%' || c == '/' || c == '@' || c == '~' || c == '^') {
                        format!("{}.{}", os, s)
                    } else {
                        format!("{}[{}]", os, key.to_nlang(indent))
                    }
                } else {
                    format!("{}.{}", os, key.to_nlang(indent))
                }
            }
            ExprKind::AnonSet(e) => format!("#{{ {} }}", e.to_nlang(indent)),
            ExprKind::Interpolated(parts) => {
                let mut s = "`".to_string();
                for part in parts { match part { StringPart::Literal(l) => s.push_str(l), StringPart::Interpolated(e) => s.push_str(&format!("${{{}}}", e.to_nlang(indent))) } }
                s.push('`'); s
            }
            ExprKind::Range { start, end, step } => { let mut res = format!("{}..{}", start.to_nlang(indent), end.to_nlang(indent)); if let Some(s) = step { res.push_str(&format!("..{}", s.to_nlang(indent))); } res }
            ExprKind::Context => "$".to_string(),
            ExprKind::Spread(e) => format!("...{}", e.to_nlang(indent)),
            ExprKind::Structural(e) => format!("<{}>", e.to_nlang(indent)),
        }
    }
}

impl AtomKind {
    pub fn to_string_canonical(&self) -> String {
        match self {
            AtomKind::Int(i) => i.to_string(), AtomKind::Float(f) => f.to_string(),
            AtomKind::Complex(r, i) => {
                if *i >= 0.0 { format!("{}+{}i", r, i) }
                else { format!("{}-{}i", r, i.abs()) }
            },
            AtomKind::Str(s) => format!("\"{}\"", s), AtomKind::MultilineStr(s) => format!("\"\"\"{}\"\"\"", s),
            AtomKind::Tag(t) => format!("#{}", t), AtomKind::TagStart => "#_|_".to_string(), AtomKind::TagEnd => "#_".to_string(),
            AtomKind::Top => "_".to_string(), AtomKind::Bottom => "_|_".to_string(), AtomKind::Unit => "()".to_string(),
            AtomKind::Regex(s) => format!("r\"{}\"", s), AtomKind::PathLit(s) => format!("p\"{}\"", s),
            AtomKind::Bytes(b) => format!("b\"{:?}\"", b), AtomKind::Uri(s) => format!("u\"{}\"", s),
            AtomKind::Time(s) => format!("t\"{}\"", s),
        }
    }
}

impl FieldKey {
    pub fn to_string_canonical(&self) -> String {
        match self {
            FieldKey::Named { prefix, name } => {
                let p = match prefix { Some(Prefix::Logic) => "/", Some(Prefix::Type) => "@", Some(Prefix::Meta) => "%", Some(Prefix::System) => "~%", Some(Prefix::Private) => "~", Some(Prefix::Local) => "^", _ => "" };
                format!("{}{}", p, name)
            }
            FieldKey::Quoted(s) => format!("\"{}\"", s),
            FieldKey::Pattern(e) => e.to_nlang(0),
            FieldKey::Path(p) => format!("{}", p),
        }
    }
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self { Self { start, end } }
    pub fn merge(a: Span, b: Span) -> Self { Self { start: a.start.min(b.start), end: a.end.max(b.end) } }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.anchor { PathAnchor::Root => write!(f, "_.")?, PathAnchor::Current => write!(f, "^.")?, PathAnchor::Parent(n) => write!(f, "{}", "^".repeat(n as usize + 1))?, PathAnchor::Bare => {} }
        write!(f, "{}", self.segments.join("."))
    }
}

impl Path { pub fn to_key(&self) -> String { format!("{}", self) } }
