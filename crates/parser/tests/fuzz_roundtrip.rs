//! Lightweight expression fuzzer: generate AST → to_nlang → re-parse → AST eq.
//!
//! No extra crate deps — xorshift64 RNG, fixed seed, deterministic CI.
//! Depth-bounded generators stay inside the printer's faithful subset
//! (no Bytes, no bare Unary::Neg on non-atoms the grammar rejects, etc.).

use nlang_parser::ast::{
    AtomKind, Expr, ExprKind, Path, PathAnchor, RelOp, Relation, Span, StringPart, UnaryOp,
};
use nlang_parser::parse_expr_only;
use num_bigint::BigInt;

// ---------------------------------------------------------------------------
// RNG
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1) // never zero
    }
    fn next_u64(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn gen_range(&mut self, lo: usize, hi: usize) -> usize {
        assert!(hi > lo);
        lo + (self.next_u64() as usize % (hi - lo))
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.gen_range(0, xs.len())]
    }
    fn chance(&mut self, num: u32, den: u32) -> bool {
        (self.next_u64() % den as u64) < num as u64
    }
}

fn sp() -> Span {
    Span::default()
}

fn atom(k: AtomKind) -> Expr {
    Expr::new(ExprKind::Atom(k), sp())
}

fn path(segs: &[&str]) -> Expr {
    Expr::new(
        ExprKind::Path(Path {
            anchor: PathAnchor::Bare,
            segments: segs.iter().map(|s| s.to_string()).collect(),
            span: sp(),
        }),
        sp(),
    )
}

fn path_anchored(anchor: PathAnchor, segs: &[&str]) -> Expr {
    Expr::new(
        ExprKind::Path(Path {
            anchor,
            segments: segs.iter().map(|s| s.to_string()).collect(),
            span: sp(),
        }),
        sp(),
    )
}

fn bin(kind: impl Fn(Box<Expr>, Box<Expr>) -> ExprKind, a: Expr, b: Expr) -> Expr {
    Expr::new(kind(Box::new(a), Box::new(b)), sp())
}

// ---------------------------------------------------------------------------
// Generator (depth-bounded)
// ---------------------------------------------------------------------------

fn gen_atom(rng: &mut Rng) -> AtomKind {
    match rng.gen_range(0, 12) {
        0 => AtomKind::Int(BigInt::from(rng.gen_range(0, 100) as i64 - 20)),
        1 => AtomKind::Float((rng.gen_range(1, 50) as f64) / 2.0), // always *.0 or *.5
        2 => AtomKind::Complex(0.0, 1.0),
        3 => AtomKind::Complex(0.0, -1.0),
        4 => AtomKind::Complex(rng.gen_range(0, 5) as f64, rng.gen_range(1, 5) as f64),
        5 => AtomKind::Str(
            rng.pick(&["a", "hi", "x1", "kebab-ok"])
                .to_string(),
        ),
        6 => AtomKind::Tag(rng.pick(&["ok", "draft", "a", "b"]).to_string()),
        7 => AtomKind::Top,
        8 => AtomKind::Bottom,
        9 => AtomKind::Unit,
        10 => AtomKind::TagStart,
        _ => AtomKind::TagEnd,
    }
}

fn gen_ident(rng: &mut Rng) -> String {
    // Avoid pure `i` / leading pure-numeric; include i-prefix idents (the bug class).
    rng.pick(&[
        "a", "b", "x", "xs", "io", "input", "i2", "i-1", "max-retry", "f", "g", "val",
    ])
    .to_string()
}

fn gen_expr(rng: &mut Rng, depth: usize) -> Expr {
    if depth == 0 {
        return if rng.chance(1, 2) {
            atom(gen_atom(rng))
        } else {
            path(&[&gen_ident(rng)])
        };
    }

    match rng.gen_range(0, 22) {
        0 => atom(gen_atom(rng)),
        1 => path(&[&gen_ident(rng)]),
        2 => path(&[&gen_ident(rng), &gen_ident(rng)]),
        3 => path_anchored(PathAnchor::Root, &[&gen_ident(rng)]),
        4 => path_anchored(PathAnchor::Parent(0), &[&gen_ident(rng)]),
        5 => path_anchored(PathAnchor::Parent(1), &[&gen_ident(rng)]),
        6 => bin(ExprKind::Add, gen_expr(rng, depth - 1), gen_expr(rng, depth - 1)),
        7 => bin(ExprKind::Sub, gen_expr(rng, depth - 1), gen_expr(rng, depth - 1)),
        8 => bin(ExprKind::Mul, gen_expr(rng, depth - 1), gen_expr(rng, depth - 1)),
        9 => bin(ExprKind::Meet, gen_expr(rng, depth - 1), gen_expr(rng, depth - 1)),
        10 => bin(ExprKind::Join, gen_expr(rng, depth - 1), gen_expr(rng, depth - 1)),
        11 => bin(ExprKind::Eq, gen_expr(rng, depth - 1), gen_expr(rng, depth - 1)),
        12 => bin(ExprKind::Lt, gen_expr(rng, depth - 1), gen_expr(rng, depth - 1)),
        13 => bin(ExprKind::LatticeEq, gen_expr(rng, depth - 1), gen_expr(rng, depth - 1)),
        14 => bin(ExprKind::Pipe, gen_expr(rng, depth - 1), gen_expr(rng, depth - 1)),
        15 => {
            let n = rng.gen_range(0, 4);
            let items: Vec<_> = (0..n).map(|_| gen_expr(rng, depth - 1)).collect();
            // inject spread sometimes
            let mut items = items;
            if n > 0 && rng.chance(1, 3) {
                items[0] = Expr::new(
                    ExprKind::Spread(Box::new(path(&[&gen_ident(rng)]))),
                    sp(),
                );
            }
            Expr::new(ExprKind::List(items), sp())
        }
        16 => {
            let n = rng.gen_range(1, 3); // 1- or 2-tuple
            let items: Vec<_> = (0..n).map(|_| gen_expr(rng, depth - 1)).collect();
            Expr::new(ExprKind::Tuple(items), sp())
        }
        17 => Expr::new(
            ExprKind::Unary {
                op: UnaryOp::Not,
                expr: Box::new(gen_expr(rng, depth - 1)),
            },
            sp(),
        ),
        18 => Expr::new(
            ExprKind::Structural(Box::new(gen_expr(rng, depth - 1))),
            sp(),
        ),
        19 => Expr::new(ExprKind::Context, sp()),
        20 => Expr::new(
            ExprKind::Morphism {
                param: Box::new(path(&[&gen_ident(rng)])),
                body: Box::new(gen_expr(rng, depth - 1)),
            },
            sp(),
        ),
        _ => {
            // poset of two tags
            let left = AtomKind::Tag(rng.pick(&["a", "b", "c"]).to_string());
            let right = AtomKind::Tag(rng.pick(&["a", "b", "c", "d"]).to_string());
            let op = *rng.pick(&[RelOp::Lt, RelOp::Lte, RelOp::Eq]);
            Expr::new(
                ExprKind::Poset(vec![Relation {
                    left,
                    op,
                    right,
                    span: sp(),
                }]),
                sp(),
            )
        }
    }
}

/// Normalize semantic aliases so roundtrip compares the observable structure.
fn normalize(e: &mut Expr) {
    e.strip_spans();
    match &mut e.kind {
        // Complement ≡ Unary(Not) — only Unary is produced by the grammar
        ExprKind::Complement(inner) => {
            let mut inner = *inner.clone();
            normalize(&mut inner);
            e.kind = ExprKind::Unary {
                op: UnaryOp::Not,
                expr: Box::new(inner),
            };
        }
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
        | ExprKind::LatticeEq(a, b)
        | ExprKind::Probe(a, b)
        | ExprKind::TypeAnnotation(a, b)
        | ExprKind::Lens(a, b) => {
            normalize(a);
            normalize(b);
        }
        ExprKind::Morphism { param, body } => {
            normalize(param);
            normalize(body);
        }
        ExprKind::Ternary {
            cond,
            then_branch,
            else_branch,
        } => {
            normalize(cond);
            normalize(then_branch);
            normalize(else_branch);
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::AnonSet(expr)
        | ExprKind::Spread(expr)
        | ExprKind::Structural(expr) => normalize(expr),
        ExprKind::List(items) | ExprKind::Tuple(items) => {
            for i in items {
                normalize(i);
            }
        }
        ExprKind::Combo { fields, .. } => {
            for f in fields {
                normalize(&mut f.value);
            }
        }
        ExprKind::Interpolated(parts) => {
            for p in parts {
                if let StringPart::Interpolated(e) = p {
                    normalize(e);
                }
            }
        }
        ExprKind::Range { start, end, step } => {
            normalize(start);
            normalize(end);
            if let Some(s) = step {
                normalize(s);
            }
        }
        _ => {}
    }
}

fn roundtrip_check(e: &Expr, seed_note: &str) {
    let mut expected = e.clone();
    normalize(&mut expected);
    let printed = expected.to_nlang(0);
    let again = match parse_expr_only(&printed) {
        Ok(x) => x,
        Err(err) => panic!(
            "[{seed_note}] re-parse failed\n  shape:   {}\n  printed: {printed:?}\n  err:     {err}",
            e.shape()
        ),
    };
    let mut b = again;
    normalize(&mut b);
    assert_eq!(
        expected, b,
        "[{seed_note}] AST mismatch\n  printed: {printed:?}\n  shape1:  {}\n  shape2:  {}",
        expected.shape(),
        b.shape()
    );
}

#[test]
fn fuzz_expr_roundtrip_seeded() {
    const N: usize = 800;
    const DEPTH: usize = 3;
    let mut rng = Rng::new(0xc01a97f15);
    let mut failures = 0usize;
    let mut first_err = String::new();

    for i in 0..N {
        let e = gen_expr(&mut rng, DEPTH);
        let mut expected = e.clone();
        normalize(&mut expected);
        let printed = expected.to_nlang(0);
        match parse_expr_only(&printed) {
            Ok(again) => {
                let mut b = again;
                normalize(&mut b);
                if expected != b {
                    failures += 1;
                    if first_err.is_empty() {
                        first_err = format!(
                            "i={i} shape1={} shape2={} printed={printed:?}",
                            expected.shape(),
                            b.shape()
                        );
                    }
                }
            }
            Err(err) => {
                failures += 1;
                if first_err.is_empty() {
                    first_err = format!(
                        "i={i} reparse err={err} shape={} printed={printed:?}",
                        e.shape()
                    );
                }
            }
        }
    }
    assert_eq!(
        failures, 0,
        "{failures}/{N} fuzz roundtrip failures; first: {first_err}"
    );
}

#[test]
fn fuzz_known_bug_class_i_prefix_idents() {
    // Explicitly hammer the complex_lit / ident boundary.
    for src in ["io", "it", "i2", "i-1", "i-foo", "input", "index"] {
        let e = parse_expr_only(src).unwrap_or_else(|e| panic!("{src}: {e}"));
        assert!(
            matches!(e.kind, ExprKind::Path(_)),
            "{src} must be Path, got {:?}",
            e.kind
        );
        roundtrip_check(&e, src);
    }
}

#[test]
fn fuzz_spread_in_lists() {
    for src in ["[...xs]", "[...xs, 1]", "[1, ...ys, 2]", "[...a, ...b]"] {
        let e = parse_expr_only(src).unwrap_or_else(|e| panic!("{src}: {e}"));
        // at least one Spread node somewhere
        fn has_spread(e: &Expr) -> bool {
            match &e.kind {
                ExprKind::Spread(_) => true,
                ExprKind::List(items) | ExprKind::Tuple(items) => items.iter().any(has_spread),
                ExprKind::Apply(a, b)
                | ExprKind::Pipe(a, b)
                | ExprKind::Meet(a, b)
                | ExprKind::Join(a, b)
                | ExprKind::Add(a, b)
                | ExprKind::Sub(a, b) => has_spread(a) || has_spread(b),
                _ => false,
            }
        }
        assert!(has_spread(&e), "{src} missing Spread: {}", e.shape());
        roundtrip_check(&e, src);
    }
}

#[test]
fn fuzz_interp_roundtrip() {
    let e = Expr::new(
        ExprKind::Interpolated(vec![
            StringPart::Literal("hi ".into()),
            StringPart::Interpolated(Box::new(path(&["x"]))),
        ]),
        sp(),
    );
    roundtrip_check(&e, "interp");
}

#[test]
fn fuzz_unary_not() {
    let e = Expr::new(
        ExprKind::Unary {
            op: UnaryOp::Not,
            expr: Box::new(path(&["x"])),
        },
        sp(),
    );
    // !x may parse as Complement not Unary — either is ok if roundtrip stable
    let printed = e.to_nlang(0);
    let again = parse_expr_only(&printed).unwrap();
    // printer of Complement and Unary(Not) both print `!…`
    let p2 = again.to_nlang(0);
    let thrice = parse_expr_only(&p2).unwrap();
    assert_eq!(again.without_spans(), thrice.without_spans());
}
