extern crate pest;
#[macro_use]
extern crate pest_derive;

use std::error::Error;
use pest::Parser;

#[derive(Parser)]
#[grammar = "n.pest"]
pub struct NParser;

pub mod ast;
pub mod tier;
use crate::ast::{Expr, ExprKind, Field, FieldKey, AtomKind, Path, PathAnchor, Span, Prefix, UnaryOp, StringPart, Relation, RelOp, Program};

pub fn parse_field(pair: pest::iterators::Pair<Rule>) -> Result<Field, Box<dyn Error>> {
    let span = Span::new(pair.as_span().start(), pair.as_span().end());
    let mut inner = pair.into_inner();
    let key_pair = inner.next().ok_or("Empty field")?;
    
    if key_pair.as_rule() == Rule::spread_expr {
        let expr = parse_expr(key_pair)?;
        return Ok(Field { key: FieldKey::Quoted("...".to_string()), value: expr, span });
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

    let mut left_atom = parse_atom(inner.next().ok_or("Empty order chain")?.into_inner().next().ok_or("Empty poset node")?)?;

    while let Some(op_pair) = inner.next() {
        let op = match op_pair.as_str() {
            "<" => RelOp::Lt,
            ">" => RelOp::Gt,
            "<=" => RelOp::Lte,
            ">=" => RelOp::Gte,
            "=" => RelOp::Eq,
            _ => return Err(format!("Unsupported order operator: {}", op_pair.as_str()).into()),
        };
        let right_atom = parse_atom(inner.next().ok_or("Order chain missing right operand")?.into_inner().next().ok_or("Empty poset node")?)?;
        relations.push(Relation { left: left_atom.clone(), op, right: right_atom.clone(), span });
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
            if s.starts_with("~%") { Ok(FieldKey::Named { prefix: Some(Prefix::System), name: s[2..].to_string() }) }
            else if s.starts_with('~') { Ok(FieldKey::Named { prefix: Some(Prefix::Private), name: s[1..].to_string() }) }
            else if s.starts_with('/') { Ok(FieldKey::Named { prefix: Some(Prefix::Logic), name: s[1..].to_string() }) }
            else if s.starts_with('@') { Ok(FieldKey::Named { prefix: Some(Prefix::Type), name: s[1..].to_string() }) }
            else if s.starts_with('%') { Ok(FieldKey::Named { prefix: Some(Prefix::Meta), name: s[1..].to_string() }) }
            else { Ok(FieldKey::Named { prefix: None, name: s.to_string() }) }
        }
        Rule::quoted_key => Ok(FieldKey::Quoted(inner.as_str()[1..inner.as_str().len()-1].to_string())),
        Rule::tag => Ok(FieldKey::Pattern(Expr::new(ExprKind::Atom(AtomKind::Tag(inner.as_str()[1..].to_string())), Span::new(inner.as_span().start(), inner.as_span().end())))),
        Rule::path | Rule::anchored_path => Ok(FieldKey::Path(parse_path(inner)?)),
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
                Ok(Expr::new(ExprKind::Ternary { cond: Box::new(cond), then_branch: Box::new(then_branch), else_branch: Box::new(else_branch) }, span))
            } else {
                parse_expr(inner[0].clone())
            }
        }

        // 2. Binary Chained (expr op expr op ...)
        Rule::morphism_expr => {
            let inner: Vec<_> = pair.into_inner().collect();
            if inner.len() == 1 { return parse_expr(inner[0].clone()); }
            let mut i = inner.len() - 1;
            let mut res = parse_expr(inner[i].clone())?;
            while i > 0 {
                let left = parse_expr(inner[i-2].clone())?;
                res = Expr::new(ExprKind::Morphism { param: Box::new(left), body: Box::new(res) }, span);
                i -= 2;
            }
            Ok(res)
        }
        Rule::pipe_expr | Rule::join_expr | Rule::meet_expr |
        Rule::cmp_expr | Rule::add_expr | Rule::mul_expr | Rule::infix_expr | Rule::type_ann_expr => {
            let inner: Vec<_> = pair.into_inner().collect();
            if inner.len() == 1 { return parse_expr(inner[0].clone()); }
            
            let mut left = parse_expr(inner[0].clone())?;
            let mut i = 1;
            while i < inner.len() {
                let op_pair = &inner[i];
                let right = parse_expr(inner[i+1].clone())?;
                let kind = match op_pair.as_rule() {
                    Rule::morphism_op => ExprKind::Morphism { param: Box::new(left), body: Box::new(right) },
                    Rule::pipe_op => ExprKind::Pipe(Box::new(left), Box::new(right)),
                    Rule::meet_op => ExprKind::Meet(Box::new(left), Box::new(right)),
                    Rule::type_ann_op => ExprKind::TypeAnnotation(Box::new(left), Box::new(right)),
                    Rule::join_op => if op_pair.as_str() == "|" { ExprKind::Join(Box::new(left), Box::new(right)) } else { ExprKind::Diff(Box::new(left), Box::new(right)) },
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
                        let f = Expr::new(ExprKind::Path(Path { anchor: PathAnchor::Bare, segments: vec![op_pair.as_str().to_string()], span: op_span }), op_span);
                        ExprKind::Apply(Box::new(Expr::new(ExprKind::Apply(Box::new(f), Box::new(left)), span)), Box::new(right))
                    }
                    Rule::add_op => if op_pair.as_str() == "+" { ExprKind::Add(Box::new(left), Box::new(right)) } else { ExprKind::Sub(Box::new(left), Box::new(right)) },
                    Rule::mul_op => match op_pair.as_str() { "*" => ExprKind::Mul(Box::new(left), Box::new(right)), "/" => ExprKind::Div(Box::new(left), Box::new(right)), "%" => ExprKind::Rem(Box::new(left), Box::new(right)), _ => unreachable!() },
                    _ => return Err(format!("Unexpected operator: {} ({:?})", op_pair.as_str(), op_pair.as_rule()).into()),
                };
                left = Expr::new(kind, span);
                i += 2;
            }
            Ok(left)
        }

        // 3. Sequential: Apply (expr expr expr ...)
        Rule::apply_expr => {
            let inner: Vec<_> = pair.into_inner().collect();
            if inner.len() == 1 { return parse_expr(inner[0].clone()); }
            let mut left = parse_expr(inner[0].clone())?;
            for item in inner.into_iter().skip(1) {
                let right = parse_expr(item)?;
                left = Expr::new(ExprKind::Apply(Box::new(left), Box::new(right)), span);
            }
            Ok(left)
        }

        // 4. Positional: Unary (op expr)
        Rule::unary_expr => {
            let mut inner = pair.into_inner();
            let first = inner.next().ok_or("Empty unary")?;
            if first.as_rule() == Rule::unary_op {
                let op = match first.as_str() { "!" => UnaryOp::Not, "-" => UnaryOp::Neg, _ => return Err("Unknown unary op".into()) };
                let expr = parse_expr(inner.next().ok_or("Unary op missing expr")?)?;
                Ok(Expr::new(ExprKind::Unary { op, expr: Box::new(expr) }, span))
            } else {
                parse_expr(first)
            }
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
                        let key = Expr::new(ExprKind::Atom(AtomKind::Str(op_inner.as_str().to_string())), Span::new(op_inner.as_span().start(), op_inner.as_span().end()));
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

        Rule::complex_lit => Ok(Expr::new(ExprKind::Atom(parse_complex(pair.as_str())?), span)),
        Rule::primary => parse_expr(pair.into_inner().next().ok_or("Empty primary")?),
        Rule::int_lit => Ok(Expr::new(ExprKind::Atom(AtomKind::Int(pair.as_str().parse::<num_bigint::BigInt>()?)), span)),
        Rule::float_lit => Ok(Expr::new(ExprKind::Atom(AtomKind::Float(pair.as_str().parse()?)), span)),
        Rule::str_lit => Ok(Expr::new(ExprKind::Atom(AtomKind::Str(pair.as_str()[1..pair.as_str().len()-1].to_string())), span)),
        Rule::tag => Ok(Expr::new(ExprKind::Atom(AtomKind::Tag(pair.as_str()[1..].to_string())), span)),
        Rule::path | Rule::anchored_path => Ok(Expr::new(ExprKind::Path(parse_path(pair)?), span)),
        Rule::combo | Rule::cocoon => {
            let is_closed = pair.as_rule() == Rule::cocoon;
            let mut fields = Vec::new();
            for item in pair.into_inner() {
                if item.as_rule() == Rule::field { fields.push(parse_field(item)?); }
            }
            Ok(Expr::new(ExprKind::Combo { fields, relations: Vec::new(), closed: is_closed }, span))
        }
        Rule::poset_lit => {
            let mut relations = Vec::new();
            for item in pair.into_inner() {
                if item.as_rule() == Rule::order_chain { relations.extend(parse_order_chain(item)?); }
            }
            Ok(Expr::new(ExprKind::Poset(relations), span))
        }
        Rule::tuple => {
            let mut items = Vec::new();
            for e in pair.into_inner() { if e.as_rule() == Rule::expr { items.push(parse_expr(e)?); } }
            Ok(Expr::new(ExprKind::Tuple(items), span))
        }
        Rule::list => {
            let mut items = Vec::new();
            for e in pair.into_inner() { if e.as_rule() == Rule::expr { items.push(parse_expr(e)?); } }
            Ok(Expr::new(ExprKind::List(items), span))
        }
        Rule::context => Ok(Expr::new(ExprKind::Context, span)),
        Rule::structural => Ok(Expr::new(ExprKind::Structural(Box::new(parse_expr(pair.into_inner().next().ok_or("Empty structural")?)?)), span)),
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
            // Grammar allows open bounds; represent missing sides as Top (`_`).
            let start = start.unwrap_or_else(|| Expr::new(ExprKind::Atom(AtomKind::Top), span));
            let end = end.unwrap_or_else(|| Expr::new(ExprKind::Atom(AtomKind::Top), span));
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
        Rule::atom => parse_atom(pair.into_inner().next().ok_or("Empty atom")?).map(|ak| Expr::new(ExprKind::Atom(ak), span)),
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
                            Rule::interp_literal => parts.push(StringPart::Literal(inner.as_str().to_string())),
                            Rule::interp_expr => {
                                let expr = parse_expr(inner.into_inner().next().ok_or("Empty interp_expr")?)?;
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
        Rule::str_lit => Ok(AtomKind::Str(pair.as_str()[1..pair.as_str().len()-1].to_string())),
        Rule::tag => Ok(AtomKind::Tag(pair.as_str()[1..].to_string())),
        Rule::top => Ok(AtomKind::Top),
        Rule::bottom => Ok(AtomKind::Bottom),
        Rule::unit => Ok(AtomKind::Unit),
        Rule::tag_start => Ok(AtomKind::TagStart),
        Rule::tag_end => Ok(AtomKind::TagEnd),
        Rule::regex_lit => Ok(AtomKind::Regex(pair.as_str()[2..pair.as_str().len()-1].to_string())),
        Rule::path_lit => Ok(AtomKind::PathLit(pair.as_str()[2..pair.as_str().len()-1].to_string())),
        Rule::bytes_lit => Ok(AtomKind::Bytes(pair.as_str()[2..pair.as_str().len()-1].as_bytes().to_vec())),
        Rule::uri_lit => Ok(AtomKind::Uri(pair.as_str()[2..pair.as_str().len()-1].to_string())),
        Rule::time_lit => Ok(AtomKind::Time(pair.as_str()[2..pair.as_str().len()-1].to_string())),
        Rule::multiline_str => Ok(AtomKind::MultilineStr(pair.as_str()[3..pair.as_str().len()-3].to_string())),
        _ => Err(format!("Unexpected atom rule: {:?}", pair.as_rule()).into()),
    }
}

fn parse_complex(s: &str) -> Result<AtomKind, Box<dyn Error>> {
    let s = s.trim();
    if s == "i" { return Ok(AtomKind::Complex(0.0, 1.0)); }
    if s == "-i" { return Ok(AtomKind::Complex(0.0, -1.0)); }
    
    if s.ends_with('i') {
        let without_i = &s[..s.len()-1];
        if without_i.contains('+') || (without_i.contains('-') && !without_i.starts_with('-')) {
            let parts: Vec<&str> = if without_i.contains('+') {
                without_i.split('+').collect()
            } else {
                let pos = without_i.find('-').unwrap_or(0);
                if pos == 0 {
                    let second_minus = without_i[1..].find('-');
                    if let Some(sm) = second_minus {
                        vec![&without_i[..sm+1], &without_i[sm+1..]]
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
        Rule::int_lit | Rule::float_lit | Rule::complex_lit | Rule::str_lit | Rule::tag
        | Rule::top | Rule::bottom | Rule::unit | Rule::tag_start | Rule::tag_end
        | Rule::regex_lit | Rule::path_lit | Rule::bytes_lit | Rule::uri_lit | Rule::time_lit
        | Rule::multiline_str => {
            let ak = parse_atom(pair)?;
            Ok(Expr::new(ExprKind::Atom(ak), span))
        }
        _ => Err(format!("Unexpected range_bound rule: {:?}", pair.as_rule()).into()),
    }
}

fn parse_path(pair: pest::iterators::Pair<Rule>) -> Result<Path, Box<dyn Error>> {
    let span = Span::new(pair.as_span().start(), pair.as_span().end());
    let mut segments = Vec::new();
    let mut anchor = PathAnchor::Bare;
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::anchor_root => anchor = PathAnchor::Root,
            Rule::anchor_parent => anchor = PathAnchor::Parent((p.as_str().matches('^').count() as u32).saturating_sub(1)),
            Rule::path_segments => {
                for seg_pair in p.into_inner() {
                    segments.push(seg_pair.as_str().trim().to_string());
                }
            }
            _ => {}
        }
    }
    Ok(Path { anchor, segments, span })
}

// The 16-level precedence chain makes recursive descent stack-hungry on deeply
// nested programs (debug-build test threads default to 2 MiB) — run parse entry
// points on a dedicated thread with generous stack.
const PARSER_STACK_BYTES: usize = 64 * 1024 * 1024;

fn with_parser_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(PARSER_STACK_BYTES)
            .spawn_scoped(s, f)
            .expect("failed to spawn parser thread")
            .join()
            .expect("parser thread panicked")
    })
}

pub fn parse_expr_only(input: &str) -> Result<Expr, Box<dyn Error>> {
    with_parser_stack(|| {
        // expr_toplevel = SOI ~ expr ~ EOI — rejects trailing junk that the bare
        // `expr` rule would silently leave unparsed (e.g. `a <=> b <=> c`,
        // `x: leftover`). Silent partial parse is the same bug class as
        // grammar-accept / AST-deform.
        let mut pairs = NParser::parse(Rule::expr_toplevel, input).map_err(|e| e.to_string())?;
        let top = pairs.next().ok_or_else(|| "empty expr_toplevel".to_string())?;
        let inner = top
            .into_inner()
            .next()
            .ok_or_else(|| "expr_toplevel missing expr".to_string())?;
        parse_expr(inner).map_err(|e| e.to_string())
    })
    .map_err(|e: String| e.into())
}

pub fn parse_program(input: &str) -> Result<Program, Box<dyn Error>> {
    with_parser_stack(|| {
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
    .map_err(|e: String| e.into())
}
