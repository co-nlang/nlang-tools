extern crate pest;
#[macro_use]
extern crate pest_derive;

use pest::Parser;
use std::error::Error;
use std::fmt;

#[derive(Parser)]
#[grammar = "n.pest"]
pub struct NParser;

pub mod ast;
pub mod tier;
use crate::ast::{
    AtomKind, Expr, ExprKind, Field, FieldKey, Path, PathAnchor, Prefix, Program, RelOp, Relation,
    Span, StringPart, UnaryOp,
};

/// A parser crash-fence tripped before pest is allowed to recurse on the native
/// stack.  This is an implementation incapacity, not the evaluator's
/// operator-configurable depth policy, so it carries `#stack_overflow`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParserNestingLimitExceeded;

impl fmt::Display for ParserNestingLimitExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("#stack_overflow")
    }
}

impl Error for ParserNestingLimitExceeded {}

/// Guaranteed parser nesting depth. The parser thread is sized to retain a
/// substantial native-stack margin at this boundary in debug builds.
pub const PARSER_NESTING_LIMIT: usize = 256;

/// Maximum AST height allowed to leave the parser. This protects recursive AST
/// consumers (formatter, canonicalizer, evaluator adapters, and drop glue)
/// from a deep tree built by otherwise flat source such as a long `+` chain.
/// It is intentionally an implementation fence, not an operator knob. 4,096
/// admits the existing 4,000-term conformance stress vectors while retaining
/// nearly a twofold margin below the measured ~7,900-level native-stack cliff.
pub const PARSER_AST_DEPTH_LIMIT: usize = 4096;

/// Lets front ends render the parser fence as the language Bottom rather than
/// treating it as an ordinary syntax diagnostic.
pub fn is_parser_nesting_limit_error(error: &(dyn Error + 'static)) -> bool {
    error.downcast_ref::<ParserNestingLimitExceeded>().is_some()
}

pub fn parse_field(pair: pest::iterators::Pair<Rule>) -> Result<Field, Box<dyn Error>> {
    let span = Span::new(pair.as_span().start(), pair.as_span().end());
    let mut inner = pair.into_inner();
    let key_pair = inner.next().ok_or("Empty field")?;

    if key_pair.as_rule() == Rule::spread_expr {
        let expr = parse_expr(key_pair)?;
        return Ok(Field {
            key: FieldKey::Quoted("...".to_string()),
            value: expr,
            span,
        });
    }

    let key = parse_field_key(key_pair)?;
    let value_pair = inner.next().ok_or("Field missing value")?;
    let value = parse_expr(value_pair)?;
    Ok(Field { key, value, span })
}

fn parse_order_chain(pair: pest::iterators::Pair<Rule>) -> Result<Vec<Relation>, Box<dyn Error>> {
    let span = Span::new(pair.as_span().start(), pair.as_span().end());
    let mut inner = pair.into_inner();
    let mut relations = Vec::new();

    let mut left_atom = parse_atom(
        inner
            .next()
            .ok_or("Empty order chain")?
            .into_inner()
            .next()
            .ok_or("Empty poset node")?,
    )?;

    while let Some(op_pair) = inner.next() {
        let op = match op_pair.as_str() {
            "<" => RelOp::Lt,
            ">" => RelOp::Gt,
            "<=" => RelOp::Lte,
            ">=" => RelOp::Gte,
            "=" => RelOp::Eq,
            _ => return Err(format!("Unsupported order operator: {}", op_pair.as_str()).into()),
        };
        let right_atom = parse_atom(
            inner
                .next()
                .ok_or("Order chain missing right operand")?
                .into_inner()
                .next()
                .ok_or("Empty poset node")?,
        )?;
        relations.push(Relation {
            left: left_atom.clone(),
            op,
            right: right_atom.clone(),
            span,
        });
        left_atom = right_atom;
    }
    Ok(relations)
}

fn parse_field_key(pair: pest::iterators::Pair<Rule>) -> Result<FieldKey, Box<dyn Error>> {
    let _key_span = Span::new(pair.as_span().start(), pair.as_span().end());
    let inner = pair.into_inner().next().ok_or("Empty field key")?;
    match inner.as_rule() {
        Rule::named_key => {
            let s = inner.as_str();
            if s.starts_with("~%") {
                Ok(FieldKey::Named {
                    prefix: Some(Prefix::System),
                    name: s[2..].to_string(),
                })
            } else if s.starts_with('~') {
                Ok(FieldKey::Named {
                    prefix: Some(Prefix::Private),
                    name: s[1..].to_string(),
                })
            } else if s.starts_with('/') {
                Ok(FieldKey::Named {
                    prefix: Some(Prefix::Logic),
                    name: s[1..].to_string(),
                })
            } else if s.starts_with('@') {
                Ok(FieldKey::Named {
                    prefix: Some(Prefix::Type),
                    name: s[1..].to_string(),
                })
            } else if s.starts_with('%') {
                Ok(FieldKey::Named {
                    prefix: Some(Prefix::Meta),
                    name: s[1..].to_string(),
                })
            } else {
                Ok(FieldKey::Named {
                    prefix: None,
                    name: s.to_string(),
                })
            }
        }
        Rule::quoted_key => Ok(FieldKey::Quoted(
            inner.as_str()[1..inner.as_str().len() - 1].to_string(),
        )),
        Rule::tag => Ok(FieldKey::Pattern(Expr::new(
            ExprKind::Atom(AtomKind::Tag(inner.as_str()[1..].to_string())),
            Span::new(inner.as_span().start(), inner.as_span().end()),
        ))),
        // field_root_path = `_.…` only (parent `^` banned on definition keys).
        Rule::path | Rule::anchored_path | Rule::field_root_path => {
            Ok(FieldKey::Path(parse_path(inner)?))
        }
        Rule::anon_set => {
            let expr = parse_expr(inner.into_inner().next().ok_or("Empty anon_set")?)?;
            Ok(FieldKey::Pattern(expr))
        }
        _ => Err(format!("Unsupported field key rule: {:?}", inner.as_rule()).into()),
    }
}

fn parse_expr(pair: pest::iterators::Pair<Rule>) -> Result<Expr, Box<dyn Error>> {
    let span = Span::new(pair.as_span().start(), pair.as_span().end());
    match pair.as_rule() {
        Rule::expr => parse_expr(pair.into_inner().next().ok_or("Empty expr")?),

        // 1. Positional: Ternary (1 or 3 Rule sub-items)
        Rule::ternary_expr => {
            let inner: Vec<_> = pair.into_inner().collect();
            if inner.len() == 3 {
                let cond = parse_expr(inner[0].clone())?;
                let then_branch = parse_expr(inner[1].clone())?;
                let else_branch = parse_expr(inner[2].clone())?;
                Ok(Expr::new(
                    ExprKind::Ternary {
                        cond: Box::new(cond),
                        then_branch: Box::new(then_branch),
                        else_branch: Box::new(else_branch),
                    },
                    span,
                ))
            } else {
                parse_expr(inner[0].clone())
            }
        }

        // 2. Binary Chained (expr op expr op ...)
        Rule::morphism_expr => {
            let inner: Vec<_> = pair.into_inner().collect();
            if inner.len() == 1 {
                return parse_expr(inner[0].clone());
            }
            let mut i = inner.len() - 1;
            let mut res = parse_expr(inner[i].clone())?;
            while i > 0 {
                let left = parse_expr(inner[i - 2].clone())?;
                // G2-M: multi-param sugar `x y -> body` ≡ nested curry
                res = fold_multiparam(left, res, span);
                i -= 2;
            }
            Ok(res)
        }
        Rule::pipe_expr
        | Rule::join_expr
        | Rule::meet_expr
        | Rule::cmp_expr
        | Rule::add_expr
        | Rule::mul_expr
        | Rule::infix_expr
        | Rule::type_ann_expr => {
            let inner: Vec<_> = pair.into_inner().collect();
            if inner.len() == 1 {
                return parse_expr(inner[0].clone());
            }

            let mut left = parse_expr(inner[0].clone())?;
            let mut i = 1;
            while i < inner.len() {
                let op_pair = &inner[i];
                let right = parse_expr(inner[i + 1].clone())?;
                // G2-M: multi-param sugar folds to nested morphisms (outer span).
                if op_pair.as_rule() == Rule::morphism_op {
                    left = fold_multiparam(left, right, span);
                    i += 2;
                    continue;
                }
                let kind = match op_pair.as_rule() {
                    Rule::pipe_op => ExprKind::Pipe(Box::new(left), Box::new(right)),
                    Rule::meet_op => ExprKind::Meet(Box::new(left), Box::new(right)),
                    Rule::type_ann_op => ExprKind::TypeAnnotation(Box::new(left), Box::new(right)),
                    Rule::join_op => {
                        if op_pair.as_str() == "|" {
                            ExprKind::Join(Box::new(left), Box::new(right))
                        } else {
                            ExprKind::Diff(Box::new(left), Box::new(right))
                        }
                    }
                    Rule::cmp_op => match op_pair.as_str() {
                        "==" => ExprKind::Eq(Box::new(left), Box::new(right)),
                        "!=" => ExprKind::Ne(Box::new(left), Box::new(right)),
                        "<=>" => ExprKind::Probe(Box::new(left), Box::new(right)),
                        "=" => ExprKind::LatticeEq(Box::new(left), Box::new(right)),
                        "<" => ExprKind::Lt(Box::new(left), Box::new(right)),
                        ">" => ExprKind::Gt(Box::new(left), Box::new(right)),
                        "<=" => ExprKind::Lte(Box::new(left), Box::new(right)),
                        ">=" => ExprKind::Gte(Box::new(left), Box::new(right)),
                        _ => unreachable!(),
                    },
                    Rule::logic_infix => {
                        let op_span = Span::new(op_pair.as_span().start(), op_pair.as_span().end());
                        let f = Expr::new(
                            ExprKind::Path(Path {
                                anchor: PathAnchor::Bare,
                                segments: vec![op_pair.as_str().to_string()],
                                span: op_span,
                            }),
                            op_span,
                        );
                        ExprKind::Apply(
                            Box::new(Expr::new(
                                ExprKind::Apply(Box::new(f), Box::new(left)),
                                span,
                            )),
                            Box::new(right),
                        )
                    }
                    Rule::add_op => {
                        if op_pair.as_str() == "+" {
                            ExprKind::Add(Box::new(left), Box::new(right))
                        } else {
                            ExprKind::Sub(Box::new(left), Box::new(right))
                        }
                    }
                    Rule::mul_op => match op_pair.as_str() {
                        "*" => ExprKind::Mul(Box::new(left), Box::new(right)),
                        "/" => ExprKind::Div(Box::new(left), Box::new(right)),
                        "%" => ExprKind::Rem(Box::new(left), Box::new(right)),
                        _ => unreachable!(),
                    },
                    _ => {
                        return Err(format!(
                            "Unexpected operator: {} ({:?})",
                            op_pair.as_str(),
                            op_pair.as_rule()
                        )
                        .into())
                    }
                };
                left = Expr::new(kind, span);
                i += 2;
            }
            Ok(left)
        }

        // 3. Sequential: Apply (expr expr expr ...)
        Rule::apply_expr => {
            let inner: Vec<_> = pair.into_inner().collect();
            if inner.len() == 1 {
                return parse_expr(inner[0].clone());
            }
            let mut left = parse_expr(inner[0].clone())?;
            for item in inner.into_iter().skip(1) {
                let right = parse_expr(item)?;
                left = Expr::new(ExprKind::Apply(Box::new(left), Box::new(right)), span);
            }
            Ok(left)
        }

        // 4. Positional: Unary. `unary_op*` is deliberately iterative in the
        // grammar, and this fold is iterative too: moving a long `!` run from
        // pest recursion into a recursive AST fold would only relocate the
        // native-stack failure.
        Rule::unary_expr => {
            let mut ops = Vec::new();
            let mut operand = None;
            for inner in pair.into_inner() {
                if inner.as_rule() == Rule::unary_op {
                    let op = match inner.as_str() {
                        "!" => UnaryOp::Not,
                        "-" => UnaryOp::Neg,
                        _ => return Err("Unknown unary op".into()),
                    };
                    ops.push((op, inner.as_span()));
                } else {
                    operand = Some(inner);
                }
            }
            let mut expr = parse_expr(operand.ok_or("Unary op missing operand")?)?;
            for (op, op_span) in ops.into_iter().rev() {
                let unary_span = Span::new(op_span.start(), expr.span.end);
                expr = Expr::new(
                    ExprKind::Unary {
                        op,
                        expr: Box::new(expr),
                    },
                    unary_span,
                );
            }
            Ok(expr)
        }

        // 5. Positional: Spread (... expr) — the "..." literal produces no pest
        // pair, so detect it on the rule's own span (the old first.as_str()
        // check never fired and silently dropped the dots)
        Rule::spread_expr => {
            let is_spread = pair.as_str().trim_start().starts_with("...");
            let inner_pair = pair.into_inner().next().ok_or("Empty spread")?;
            let expr = parse_expr(inner_pair)?;
            if is_spread {
                Ok(Expr::new(ExprKind::Spread(Box::new(expr)), span))
            } else {
                Ok(expr)
            }
        }

        // 6. Postfix: Lens (expr.key or expr[key])
        Rule::postfix_expr => {
            let mut inner = pair.into_inner();
            let mut left = parse_expr(inner.next().ok_or("Empty postfix")?)?;
            for op_pair in inner {
                let op_inner = op_pair.into_inner().next().ok_or("Empty postfix_op")?;
                match op_inner.as_rule() {
                    Rule::named_key => {
                        let key = Expr::new(
                            ExprKind::Atom(AtomKind::Str(op_inner.as_str().to_string())),
                            Span::new(op_inner.as_span().start(), op_inner.as_span().end()),
                        );
                        left = Expr::new(ExprKind::Lens(Box::new(left), Box::new(key)), span);
                    }
                    Rule::expr => {
                        let key = parse_expr(op_inner)?;
                        left = Expr::new(ExprKind::Lens(Box::new(left), Box::new(key)), span);
                    }
                    _ => return Err("Unsupported postfix op".into()),
                }
            }
            Ok(left)
        }

        Rule::complex_lit => Ok(Expr::new(
            ExprKind::Atom(parse_complex(pair.as_str())?),
            span,
        )),
        Rule::primary => parse_expr(pair.into_inner().next().ok_or("Empty primary")?),
        Rule::int_lit => Ok(Expr::new(
            ExprKind::Atom(AtomKind::Int(pair.as_str().parse::<num_bigint::BigInt>()?)),
            span,
        )),
        Rule::float_lit => Ok(Expr::new(
            ExprKind::Atom(AtomKind::Float(pair.as_str().parse()?)),
            span,
        )),
        Rule::str_lit => Ok(Expr::new(
            ExprKind::Atom(AtomKind::Str(
                pair.as_str()[1..pair.as_str().len() - 1].to_string(),
            )),
            span,
        )),
        Rule::tag => Ok(Expr::new(
            ExprKind::Atom(AtomKind::Tag(pair.as_str()[1..].to_string())),
            span,
        )),
        Rule::path | Rule::anchored_path => Ok(Expr::new(ExprKind::Path(parse_path(pair)?), span)),
        Rule::combo | Rule::cocoon => {
            let is_closed = pair.as_rule() == Rule::cocoon;
            let mut fields = Vec::new();
            for item in pair.into_inner() {
                if item.as_rule() == Rule::field {
                    fields.push(parse_field(item)?);
                }
            }
            Ok(Expr::new(
                ExprKind::Combo {
                    fields,
                    relations: Vec::new(),
                    closed: is_closed,
                },
                span,
            ))
        }
        Rule::poset_lit => {
            let mut relations = Vec::new();
            for item in pair.into_inner() {
                if item.as_rule() == Rule::order_chain {
                    relations.extend(parse_order_chain(item)?);
                }
            }
            Ok(Expr::new(ExprKind::Poset(relations), span))
        }
        // `paren_expr` has already parsed the shared opening paren and first
        // expression.  With no `tuple_tail` it is grouping (the identity);
        // with one it is a positional tuple including that first expression.
        Rule::paren_expr => {
            let mut inner = pair.into_inner();
            let first = parse_expr(inner.next().ok_or("Empty paren expression")?)?;
            match inner.next() {
                None => Ok(first),
                Some(tail) if tail.as_rule() == Rule::tuple_tail => {
                    let mut items = vec![first];
                    for e in tail.into_inner() {
                        if e.as_rule() == Rule::expr {
                            items.push(parse_expr(e)?);
                        }
                    }
                    Ok(Expr::new(ExprKind::Tuple(items), span))
                }
                Some(other) => Err(format!(
                    "Unexpected parenthesized expression tail: {:?}",
                    other.as_rule()
                )
                .into()),
            }
        }
        Rule::list => {
            let mut items = Vec::new();
            for e in pair.into_inner() {
                if e.as_rule() == Rule::expr {
                    items.push(parse_expr(e)?);
                }
            }
            Ok(Expr::new(ExprKind::List(items), span))
        }
        Rule::context => Ok(Expr::new(ExprKind::Context, span)),
        Rule::structural => Ok(Expr::new(
            ExprKind::Structural(Box::new(parse_expr(
                pair.into_inner().next().ok_or("Empty structural")?,
            )?)),
            span,
        )),
        // range was accepted by the grammar but never built into an AST node
        // (silent failure: "Unexpected rule: range") — SYNTAX_04 §4.5
        Rule::range => {
            let mut start: Option<Expr> = None;
            let mut end: Option<Expr> = None;
            let mut step: Option<Expr> = None;
            for part in pair.into_inner() {
                match part.as_rule() {
                    Rule::range_start => {
                        let b = part.into_inner().next().ok_or("Empty range_start")?;
                        start = Some(parse_range_bound(b)?);
                    }
                    Rule::range_end => {
                        let b = part.into_inner().next().ok_or("Empty range_end")?;
                        end = Some(parse_range_bound(b)?);
                    }
                    Rule::range_step => {
                        let b = part.into_inner().next().ok_or("Empty range_step")?;
                        step = Some(parse_range_bound(b)?);
                    }
                    _ => {}
                }
            }
            // Omitted bounds default to ORDER anchors (SPEC_02 §3): `#_|_`
            // (TagStart, start) / `#_` (TagEnd, end) — not information Top.
            let start =
                start.unwrap_or_else(|| Expr::new(ExprKind::Atom(AtomKind::TagStart), span));
            let end = end.unwrap_or_else(|| Expr::new(ExprKind::Atom(AtomKind::TagEnd), span));
            Ok(Expr::new(
                ExprKind::Range {
                    start: Box::new(start),
                    end: Box::new(end),
                    step: step.map(Box::new),
                },
                span,
            ))
        }
        Rule::anon_set => {
            let mut inner_it = pair.into_inner();
            // Empty `@{ }` / `@{}` (if grammar allows) ≡ Bottom per SYNTAX_02/04.
            match inner_it.next() {
                Some(inner) => {
                    let expr = parse_expr(inner)?;
                    Ok(Expr::new(ExprKind::AnonSet(Box::new(expr)), span))
                }
                None => Ok(Expr::new(
                    ExprKind::AnonSet(Box::new(Expr::new(ExprKind::Atom(AtomKind::Bottom), span))),
                    span,
                )),
            }
        }
        Rule::atom => parse_atom(pair.into_inner().next().ok_or("Empty atom")?)
            .map(|ak| Expr::new(ExprKind::Atom(ak), span)),
        Rule::bottom => Ok(Expr::new(ExprKind::Atom(AtomKind::Bottom), span)),
        Rule::top => Ok(Expr::new(ExprKind::Atom(AtomKind::Top), span)),
        Rule::unit => Ok(Expr::new(ExprKind::Atom(AtomKind::Unit), span)),
        Rule::tag_start => Ok(Expr::new(ExprKind::Atom(AtomKind::TagStart), span)),
        Rule::tag_end => Ok(Expr::new(ExprKind::Atom(AtomKind::TagEnd), span)),
        Rule::interp_str => {
            let mut parts = Vec::new();
            for p in pair.into_inner() {
                if p.as_rule() == Rule::interp_part {
                    if let Some(inner) = p.into_inner().next() {
                        match inner.as_rule() {
                            Rule::interp_literal => {
                                parts.push(StringPart::Literal(inner.as_str().to_string()))
                            }
                            Rule::interp_expr => {
                                let expr = parse_expr(
                                    inner.into_inner().next().ok_or("Empty interp_expr")?,
                                )?;
                                parts.push(StringPart::Interpolated(Box::new(expr)));
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(Expr::new(ExprKind::Interpolated(parts), span))
        }
        _ => Err(format!("Unexpected rule in parse_expr: {:?}", pair.as_rule()).into()),
    }
}

fn parse_atom(pair: pest::iterators::Pair<Rule>) -> Result<AtomKind, Box<dyn Error>> {
    match pair.as_rule() {
        Rule::int_lit => Ok(AtomKind::Int(pair.as_str().parse::<num_bigint::BigInt>()?)),
        Rule::float_lit => Ok(AtomKind::Float(pair.as_str().parse()?)),
        Rule::complex_lit => parse_complex(pair.as_str()),
        Rule::str_lit => Ok(AtomKind::Str(
            pair.as_str()[1..pair.as_str().len() - 1].to_string(),
        )),
        Rule::tag => Ok(AtomKind::Tag(pair.as_str()[1..].to_string())),
        Rule::top => Ok(AtomKind::Top),
        Rule::bottom => Ok(AtomKind::Bottom),
        Rule::unit => Ok(AtomKind::Unit),
        Rule::tag_start => Ok(AtomKind::TagStart),
        Rule::tag_end => Ok(AtomKind::TagEnd),
        Rule::regex_lit => Ok(AtomKind::Regex(
            pair.as_str()[2..pair.as_str().len() - 1].to_string(),
        )),
        Rule::path_lit => Ok(AtomKind::PathLit(
            pair.as_str()[2..pair.as_str().len() - 1].to_string(),
        )),
        Rule::bytes_lit => Ok(AtomKind::Bytes(
            pair.as_str()[2..pair.as_str().len() - 1]
                .as_bytes()
                .to_vec(),
        )),
        Rule::uri_lit => Ok(AtomKind::Uri(
            pair.as_str()[2..pair.as_str().len() - 1].to_string(),
        )),
        Rule::time_lit => Ok(AtomKind::Time(
            pair.as_str()[2..pair.as_str().len() - 1].to_string(),
        )),
        Rule::multiline_str => Ok(AtomKind::MultilineStr(
            pair.as_str()[3..pair.as_str().len() - 3].to_string(),
        )),
        _ => Err(format!("Unexpected atom rule: {:?}", pair.as_rule()).into()),
    }
}

fn parse_complex(s: &str) -> Result<AtomKind, Box<dyn Error>> {
    let s = s.trim();
    if s == "i" {
        return Ok(AtomKind::Complex(0.0, 1.0));
    }
    if s == "-i" {
        return Ok(AtomKind::Complex(0.0, -1.0));
    }

    if s.ends_with('i') {
        let without_i = &s[..s.len() - 1];
        if without_i.contains('+') || (without_i.contains('-') && !without_i.starts_with('-')) {
            let parts: Vec<&str> = if without_i.contains('+') {
                without_i.split('+').collect()
            } else {
                let pos = without_i.find('-').unwrap_or(0);
                if pos == 0 {
                    let second_minus = without_i[1..].find('-');
                    if let Some(sm) = second_minus {
                        vec![&without_i[..sm + 1], &without_i[sm + 1..]]
                    } else {
                        vec![without_i]
                    }
                } else {
                    vec![&without_i[..pos], &without_i[pos..]]
                }
            };

            if parts.len() == 2 {
                let real = parts[0].trim().parse::<f64>()?;
                let imag = parts[1].trim().parse::<f64>()?;
                return Ok(AtomKind::Complex(real, imag));
            }
        }

        let imag = without_i.parse::<f64>()?;
        return Ok(AtomKind::Complex(0.0, imag));
    }

    Err(format!("Invalid complex literal: {}", s).into())
}

fn parse_range_bound(pair: pest::iterators::Pair<Rule>) -> Result<Expr, Box<dyn Error>> {
    let span = Span::new(pair.as_span().start(), pair.as_span().end());
    match pair.as_rule() {
        Rule::range_bound => {
            let inner = pair.into_inner().next().ok_or("Empty range_bound")?;
            parse_range_bound(inner)
        }
        Rule::atom => {
            let ak = parse_atom(pair.into_inner().next().ok_or("Empty atom")?)?;
            Ok(Expr::new(ExprKind::Atom(ak), span))
        }
        Rule::path | Rule::anchored_path => Ok(Expr::new(ExprKind::Path(parse_path(pair)?), span)),
        // atom alternatives may surface directly
        Rule::int_lit
        | Rule::float_lit
        | Rule::complex_lit
        | Rule::str_lit
        | Rule::tag
        | Rule::top
        | Rule::bottom
        | Rule::unit
        | Rule::tag_start
        | Rule::tag_end
        | Rule::regex_lit
        | Rule::path_lit
        | Rule::bytes_lit
        | Rule::uri_lit
        | Rule::time_lit
        | Rule::multiline_str => {
            let ak = parse_atom(pair)?;
            Ok(Expr::new(ExprKind::Atom(ak), span))
        }
        _ => Err(format!("Unexpected range_bound rule: {:?}", pair.as_rule()).into()),
    }
}

/// G2-M (SYNTAX_11 auto-curry): `x y -> body` ≡ `x -> (y -> body)`.
/// Fold ONLY when `param` is an Apply chain whose every leaf is a bare
/// single-segment Path; other param shapes (Tuple, pattern, anchored) keep
/// the un-folded Morphism (tuple params = G5, out of scope).
fn fold_multiparam(param: Expr, body: Expr, span: Span) -> Expr {
    match collect_bare_path_apply_chain(&param) {
        Some(params) if params.len() >= 2 => {
            let mut res = body;
            for p in params.into_iter().rev() {
                res = Expr::new(
                    ExprKind::Morphism {
                        param: Box::new(p),
                        body: Box::new(res),
                    },
                    span,
                );
            }
            res
        }
        _ => Expr::new(
            ExprKind::Morphism {
                param: Box::new(param),
                body: Box::new(body),
            },
            span,
        ),
    }
}

/// Left-to-right leaves of a juxtaposition Apply chain, if every leaf is a
/// bare single-segment Path. Returns None on any other shape (strict gate).
fn collect_bare_path_apply_chain(expr: &Expr) -> Option<Vec<Expr>> {
    match &expr.kind {
        ExprKind::Path(p) if p.anchor == PathAnchor::Bare && p.segments.len() == 1 => {
            Some(vec![expr.clone()])
        }
        ExprKind::Apply(f, a) => {
            let mut left = collect_bare_path_apply_chain(f)?;
            let right = collect_bare_path_apply_chain(a)?;
            left.extend(right);
            Some(left)
        }
        _ => None,
    }
}

fn parse_path(pair: pest::iterators::Pair<Rule>) -> Result<Path, Box<dyn Error>> {
    let span = Span::new(pair.as_span().start(), pair.as_span().end());
    let mut segments = Vec::new();
    let mut anchor = PathAnchor::Bare;
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::anchor_root => anchor = PathAnchor::Root,
            Rule::anchor_parent => {
                anchor =
                    PathAnchor::Parent((p.as_str().matches('^').count() as u32).saturating_sub(1))
            }
            Rule::path_segments => {
                for seg_pair in p.into_inner() {
                    segments.push(seg_pair.as_str().trim().to_string());
                }
            }
            _ => {}
        }
    }
    Ok(Path {
        anchor,
        segments,
        span,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScanMode {
    Code,
    Comment,
    Quoted,
    MultilineQuoted,
    Interpolated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Delimiter {
    Paren,
    Bracket,
    Brace,
    Cocoon,
    Structural,
    Interpolation,
}

fn push_delimiter(delimiters: &mut Vec<Delimiter>, delimiter: Delimiter) -> bool {
    delimiters.push(delimiter);
    delimiters.len() > PARSER_NESTING_LIMIT
}

/// Returns true before pest sees a syntactically nested input that could cross
/// its native-stack cliff.  It is intentionally a small lexical scan rather
/// than a second parser: delimiters inside comments and string bodies do not
/// count, while interpolation bodies do return to normal n/ syntax.
fn exceeds_parser_nesting_limit(input: &str) -> bool {
    let bytes = input.as_bytes();
    let mut delimiters = Vec::with_capacity(PARSER_NESTING_LIMIT + 1);
    let mut mode = ScanMode::Code;
    let mut i = 0;

    while i < bytes.len() {
        match mode {
            ScanMode::Comment => {
                if matches!(bytes[i], b'\n' | b'\r') {
                    mode = ScanMode::Code;
                }
                i += 1;
            }
            ScanMode::Quoted => {
                if bytes[i] == b'"' {
                    mode = ScanMode::Code;
                }
                i += 1;
            }
            ScanMode::MultilineQuoted => {
                if bytes[i..].starts_with(b"\\\"\"\"") {
                    i += 4;
                } else if bytes[i..].starts_with(b"\"\"\"") {
                    mode = ScanMode::Code;
                    i += 3;
                } else {
                    i += 1;
                }
            }
            ScanMode::Interpolated => {
                if bytes[i] == b'`' {
                    mode = ScanMode::Code;
                    i += 1;
                } else if bytes[i..].starts_with(b"${") {
                    if push_delimiter(&mut delimiters, Delimiter::Interpolation) {
                        return true;
                    }
                    mode = ScanMode::Code;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            ScanMode::Code => {
                if bytes[i..].starts_with(b";;") {
                    mode = ScanMode::Comment;
                    i += 2;
                } else if bytes[i..].starts_with(b"\"\"\"") {
                    mode = ScanMode::MultilineQuoted;
                    i += 3;
                } else if bytes[i] == b'"' {
                    mode = ScanMode::Quoted;
                    i += 1;
                } else if bytes[i] == b'`' {
                    mode = ScanMode::Interpolated;
                    i += 1;
                } else if bytes[i..].starts_with(b"{{") {
                    if push_delimiter(&mut delimiters, Delimiter::Cocoon) {
                        return true;
                    }
                    i += 2;
                } else if bytes[i..].starts_with(b"<<") {
                    if push_delimiter(&mut delimiters, Delimiter::Structural) {
                        return true;
                    }
                    i += 2;
                } else {
                    match bytes[i] {
                        b'(' => {
                            if push_delimiter(&mut delimiters, Delimiter::Paren) {
                                return true;
                            }
                            i += 1;
                        }
                        b'[' => {
                            if push_delimiter(&mut delimiters, Delimiter::Bracket) {
                                return true;
                            }
                            i += 1;
                        }
                        b'{' => {
                            if push_delimiter(&mut delimiters, Delimiter::Brace) {
                                return true;
                            }
                            i += 1;
                        }
                        b')' => {
                            if matches!(delimiters.last(), Some(Delimiter::Paren)) {
                                delimiters.pop();
                            }
                            i += 1;
                        }
                        b']' => {
                            if matches!(delimiters.last(), Some(Delimiter::Bracket)) {
                                delimiters.pop();
                            }
                            i += 1;
                        }
                        b'}' if bytes[i..].starts_with(b"}}")
                            && matches!(delimiters.last(), Some(Delimiter::Cocoon)) =>
                        {
                            delimiters.pop();
                            i += 2;
                        }
                        b'}' => {
                            if matches!(
                                delimiters.last(),
                                Some(Delimiter::Brace | Delimiter::Interpolation)
                            ) {
                                let closed_interpolation =
                                    matches!(delimiters.pop(), Some(Delimiter::Interpolation));
                                if closed_interpolation {
                                    mode = ScanMode::Interpolated;
                                }
                            }
                            i += 1;
                        }
                        b'>' if bytes[i..].starts_with(b">>")
                            && matches!(delimiters.last(), Some(Delimiter::Structural)) =>
                        {
                            delimiters.pop();
                            i += 2;
                        }
                        _ => i += 1,
                    }
                }
            }
        }
    }
    false
}

fn parser_nesting_gate(input: &str) -> Result<(), Box<dyn Error>> {
    if exceeds_parser_nesting_limit(input) {
        Err(Box::new(ParserNestingLimitExceeded))
    } else {
        Ok(())
    }
}

/// Walk an expression with an explicit worklist. Recursive AST walkers are
/// precisely what this fence protects, so this check must never recurse.
fn expr_exceeds_ast_depth(root: &Expr) -> bool {
    let mut pending = vec![(root, 1usize)];

    while let Some((expr, depth)) = pending.pop() {
        if depth > PARSER_AST_DEPTH_LIMIT {
            return true;
        }
        let child_depth = depth + 1;
        match &expr.kind {
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
                pending.push((a, child_depth));
                pending.push((b, child_depth));
            }
            ExprKind::Morphism { param, body } => {
                pending.push((param, child_depth));
                pending.push((body, child_depth));
            }
            ExprKind::Combo { fields, .. } => {
                for field in fields {
                    pending.push((&field.value, child_depth));
                    if let FieldKey::Pattern(expr) = &field.key {
                        pending.push((expr, child_depth));
                    }
                }
            }
            ExprKind::Ternary {
                cond,
                then_branch,
                else_branch,
            } => {
                pending.push((cond, child_depth));
                pending.push((then_branch, child_depth));
                pending.push((else_branch, child_depth));
            }
            ExprKind::Unary { expr, .. }
            | ExprKind::Complement(expr)
            | ExprKind::AnonSet(expr)
            | ExprKind::Spread(expr)
            | ExprKind::Structural(expr) => pending.push((expr, child_depth)),
            ExprKind::List(items) | ExprKind::Tuple(items) => {
                pending.extend(items.iter().map(|item| (item, child_depth)));
            }
            ExprKind::Interpolated(parts) => {
                for part in parts {
                    if let StringPart::Interpolated(expr) = part {
                        pending.push((expr, child_depth));
                    }
                }
            }
            ExprKind::Range { start, end, step } => {
                pending.push((start, child_depth));
                pending.push((end, child_depth));
                if let Some(step) = step {
                    pending.push((step, child_depth));
                }
            }
            ExprKind::Atom(_) | ExprKind::Path(_) | ExprKind::Poset(_) | ExprKind::Context => {}
        }
    }
    false
}

fn program_exceeds_ast_depth(program: &Program) -> bool {
    program.fields.iter().any(|field| {
        expr_exceeds_ast_depth(&field.value)
            || matches!(&field.key, FieldKey::Pattern(expr) if expr_exceeds_ast_depth(expr))
    })
}

/// Drain a rejected AST without asking Rust's generated recursive drop glue to
/// follow its deepest child chain. Each node is first replaced by an atom, then
/// its owned child expressions are moved onto an explicit worklist.
fn drop_exprs_iteratively(mut pending: Vec<Expr>) {
    while let Some(mut expr) = pending.pop() {
        let kind = std::mem::replace(&mut expr.kind, ExprKind::Atom(AtomKind::Unit));
        match kind {
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
                pending.push(*a);
                pending.push(*b);
            }
            ExprKind::Morphism { param, body } => {
                pending.push(*param);
                pending.push(*body);
            }
            ExprKind::Combo { fields, .. } => {
                for field in fields {
                    let Field { key, value, .. } = field;
                    if let FieldKey::Pattern(expr) = key {
                        pending.push(expr);
                    }
                    pending.push(value);
                }
            }
            ExprKind::Ternary {
                cond,
                then_branch,
                else_branch,
            } => {
                pending.push(*cond);
                pending.push(*then_branch);
                pending.push(*else_branch);
            }
            ExprKind::Unary { expr, .. }
            | ExprKind::Complement(expr)
            | ExprKind::AnonSet(expr)
            | ExprKind::Spread(expr)
            | ExprKind::Structural(expr) => pending.push(*expr),
            ExprKind::List(items) | ExprKind::Tuple(items) => pending.extend(items),
            ExprKind::Interpolated(parts) => {
                for part in parts {
                    if let StringPart::Interpolated(expr) = part {
                        pending.push(*expr);
                    }
                }
            }
            ExprKind::Range { start, end, step } => {
                pending.push(*start);
                pending.push(*end);
                if let Some(step) = step {
                    pending.push(*step);
                }
            }
            ExprKind::Atom(_) | ExprKind::Path(_) | ExprKind::Poset(_) | ExprKind::Context => {}
        }
    }
}

fn drop_program_iteratively(mut program: Program) {
    let fields = std::mem::take(&mut program.fields);
    let mut pending = Vec::with_capacity(fields.len());
    for field in fields {
        let Field { key, value, .. } = field;
        if let FieldKey::Pattern(expr) = key {
            pending.push(expr);
        }
        pending.push(value);
    }
    drop_exprs_iteratively(pending);
}

fn parser_ast_gate_expr(expr: Expr) -> Result<Expr, Box<dyn Error>> {
    if expr_exceeds_ast_depth(&expr) {
        drop_exprs_iteratively(vec![expr]);
        Err(Box::new(ParserNestingLimitExceeded))
    } else {
        Ok(expr)
    }
}

fn parser_ast_gate_program(program: Program) -> Result<Program, Box<dyn Error>> {
    if program_exceeds_ast_depth(&program) {
        drop_program_iteratively(program);
        Err(Box::new(ParserNestingLimitExceeded))
    } else {
        Ok(program)
    }
}

// The 16-level precedence chain makes recursive descent stack-hungry on deeply
// nested programs. 512 MiB reserves about four times the debug requirement for
// the promised 256-level worst form; Linux commits that reservation lazily.
const PARSER_STACK_BYTES: usize = 512 * 1024 * 1024;

fn with_parser_stack<T: Send>(
    f: impl FnOnce() -> T + Send,
) -> Result<T, ParserNestingLimitExceeded> {
    with_parser_stack_using(
        std::thread::Builder::new().stack_size(PARSER_STACK_BYTES),
        f,
    )
}

/// Keep thread creation fallible: a process under a tight address-space limit
/// must report a parser incapacity, not turn an allocation failure into panic.
fn with_parser_stack_using<T: Send>(
    builder: std::thread::Builder,
    f: impl FnOnce() -> T + Send,
) -> Result<T, ParserNestingLimitExceeded> {
    std::thread::scope(|s| {
        let parser = builder
            .spawn_scoped(s, f)
            .map_err(|_| ParserNestingLimitExceeded)?;
        parser.join().map_err(|_| ParserNestingLimitExceeded)
    })
}

pub fn parse_expr_only(input: &str) -> Result<Expr, Box<dyn Error>> {
    parser_nesting_gate(input)?;
    let expr = with_parser_stack(|| {
        // expr_toplevel = SOI ~ expr ~ EOI — rejects trailing junk that the bare
        // `expr` rule would silently leave unparsed (e.g. `a <=> b <=> c`,
        // `x: leftover`). Silent partial parse is the same bug class as
        // grammar-accept / AST-deform.
        let mut pairs = NParser::parse(Rule::expr_toplevel, input).map_err(|e| e.to_string())?;
        let top = pairs
            .next()
            .ok_or_else(|| "empty expr_toplevel".to_string())?;
        let inner = top
            .into_inner()
            .next()
            .ok_or_else(|| "expr_toplevel missing expr".to_string())?;
        parse_expr(inner).map_err(|e| e.to_string())
    })
    .map_err(|error| -> Box<dyn Error> { Box::new(error) })?
    .map_err(|e: String| -> Box<dyn Error> { e.into() })?;
    parser_ast_gate_expr(expr)
}

pub fn parse_program(input: &str) -> Result<Program, Box<dyn Error>> {
    parser_nesting_gate(input)?;
    let program = with_parser_stack(|| {
        let mut pairs = NParser::parse(Rule::program, input).map_err(|e| e.to_string())?;
        let mut fields = Vec::new();
        if let Some(p) = pairs.next() {
            for f in p.into_inner() {
                if f.as_rule() == Rule::field {
                    fields.push(parse_field(f).map_err(|e| e.to_string())?);
                }
            }
        }
        Ok(Program { fields })
    })
    .map_err(|error| -> Box<dyn Error> { Box::new(error) })?
    .map_err(|e: String| -> Box<dyn Error> { e.into() })?;
    parser_ast_gate_program(program)
}

#[cfg(test)]
mod nesting_gate_tests {
    use super::*;

    #[test]
    fn ignores_delimiters_inside_comments_and_string_bodies() {
        let delimiters = "(".repeat(PARSER_NESTING_LIMIT + 1);

        assert!(parse_expr_only(&format!("\"{delimiters}\"")).is_ok());
        assert!(parse_expr_only(&format!("\"\"\"{delimiters}\"\"\"")).is_ok());
        assert!(parse_expr_only(&format!("`{delimiters}`")).is_ok());
        assert!(parse_program(&format!("a: 1 ;; {delimiters}\n")).is_ok());
    }

    #[test]
    fn rejects_real_nesting_before_pest_recurses() {
        // Cocoon chains are the measured worst form for parser stack use. The
        // exact ceiling remains parseable; the next layer never reaches pest.
        let mut at_limit = "1".to_string();
        for _ in 0..PARSER_NESTING_LIMIT {
            at_limit = format!("{{{{a: {at_limit}}}}}");
        }
        assert!(parse_expr_only(&at_limit).is_ok());

        let over_limit = format!("{{{{a: {at_limit}}}}}");
        let error = parse_expr_only(&over_limit).expect_err("deep input must hit the parser fence");
        assert!(is_parser_nesting_limit_error(error.as_ref()));
    }

    #[test]
    fn ast_depth_fence_keeps_the_normal_drop_path_inside_its_margin() {
        // A left-associated binary chain with N terms has AST height N. Build
        // exactly the permitted height, then let Rust drop it normally: this
        // pins the drop-glue side of the post-parse fence, not only formatting.
        let at_limit = format!("1{}", "+1".repeat(PARSER_AST_DEPTH_LIMIT - 1));
        let expr = parse_expr_only(&at_limit).expect("AST at the fence must parse");
        drop(expr);

        // The next node is built, measured and drained by the explicit
        // worklist before the public error is returned.
        let over_limit = format!("1{}", "+1".repeat(PARSER_AST_DEPTH_LIMIT));
        let error = parse_expr_only(&over_limit).expect_err("deep AST must hit the fence");
        assert!(is_parser_nesting_limit_error(error.as_ref()));
    }

    #[test]
    fn iterative_unary_rule_keeps_whitespace_between_prefixes() {
        assert!(parse_expr_only("! !#true").is_ok());
    }

    #[test]
    fn parser_thread_spawn_failure_is_a_typed_fence_error() {
        let error =
            with_parser_stack_using(std::thread::Builder::new().stack_size(usize::MAX), || ())
                .expect_err("an impossible stack reservation must make spawn fail cleanly");
        assert_eq!(error, ParserNestingLimitExceeded);
    }
}
