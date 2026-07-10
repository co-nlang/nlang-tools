use std::sync::Arc;
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::{Ouroboros, EvalContext, BuiltinFn};
use crate::value::{Value, EffectTag, BottomCause, BottomDetail, ComboVal};
use nlang_parser::ast::AtomKind;

fn parse_csv(input: &str) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut current_field = String::new();
    let mut chars = input.chars().peekable();
    let mut in_quotes = false;

    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                if in_quotes {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        current_field.push('"');
                    } else {
                        in_quotes = false;
                    }
                } else {
                    in_quotes = true;
                }
            }
            ',' if !in_quotes => {
                current_row.push(current_field.clone());
                current_field.clear();
            }
            '\r' if !in_quotes => {
                if chars.peek() == Some(&'\n') { chars.next(); }
                current_row.push(current_field.clone());
                current_field.clear();
                rows.push(current_row.clone());
                current_row.clear();
            }
            '\n' if !in_quotes => {
                current_row.push(current_field.clone());
                current_field.clear();
                rows.push(current_row.clone());
                current_row.clear();
            }
            _ => current_field.push(ch),
        }
    }
    if !current_field.is_empty() || !current_row.is_empty() {
        current_row.push(current_field);
        if !current_row.is_empty() { rows.push(current_row); }
    }
    rows
}

fn escape_csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn str_atom(s: impl Into<String>) -> Value {
    Value::Atom(AtomKind::Str(s.into()), EffectTag::Pure, None)
}

fn build_list(items: Vec<Value>) -> Value {
    let mut m = IndexMap::new();
    m.insert("%kind".to_string(), Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None));
    for (i, v) in items.iter().enumerate() { m.insert(i.to_string(), v.clone()); }
    Value::Combo(ComboVal::new(m, false, IndexMap::new(), EffectTag::Pure, vec![]))
}

fn extract_list_items(v: &Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Vec<Value> {
    if let Value::Combo(c) = v {
        let mut out = Vec::new();
        for i in 0u32.. {
            match c.get_field(&i.to_string()) {
                Some(v) => out.push(oo.force(v.clone(), ctx)),
                None => break,
            }
        }
        out
    } else { vec![] }
}

fn extract_str_arg(v: &Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Option<String> {
    match v {
        Value::Atom(AtomKind::Str(s), _, _) => Some(s.clone()),
        Value::Combo(c) => match c.get_field("0") {
            Some(v) => match oo.force(v.clone(), ctx) {
                Value::Atom(AtomKind::Str(s), _, _) => Some(s),
                _ => None,
            },
            None => None,
        },
        _ => None,
    }
}

pub fn register_csv_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert("csv.parse".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = oo.force(arg, ctx);
        let s = match extract_str_arg(&v, oo, ctx) {
            Some(s) => s,
            None => return BottomCause::Conflict.into(),
        };
        let rows = parse_csv(&s);
        let row_vals: Vec<Value> = rows.into_iter()
            .map(|row| build_list(row.into_iter().map(|f| str_atom(f)).collect()))
            .collect();
        build_list(row_vals)
    }) as Arc<BuiltinFn>);

    m.insert("csv.parse_with_headers".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = oo.force(arg, ctx);
        let s = match extract_str_arg(&v, oo, ctx) {
            Some(s) => s,
            None => return BottomCause::Conflict.into(),
        };
        let mut rows = parse_csv(&s);
        if rows.is_empty() { return build_list(vec![]); }
        let headers = rows.remove(0);
        let record_vals: Vec<Value> = rows.into_iter().map(|row| {
            let mut fields = IndexMap::new();
            for (i, header) in headers.iter().enumerate() {
                let val = row.get(i).cloned().unwrap_or_default();
                fields.insert(header.clone(), str_atom(val));
            }
            Value::Combo(ComboVal::new(fields, false, IndexMap::new(), EffectTag::Pure, vec![]))
        }).collect();
        build_list(record_vals)
    }) as Arc<BuiltinFn>);

    m.insert("csv.stringify".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = oo.force(arg, ctx);
        let outer = match &v {
            Value::Combo(_) => v.clone(),
            _ => return BottomCause::Conflict.into(),
        };
        let rows = extract_list_items(&outer, oo, ctx);
        let mut lines: Vec<String> = Vec::new();
        for row in rows {
            let fields = extract_list_items(&row, oo, ctx);
            let escaped: Vec<String> = fields.iter().map(|v| {
                match v {
                    Value::Atom(AtomKind::Str(s), _, _) => escape_csv_field(s),
                    _ => escape_csv_field(&format!("{:?}", v)),
                }
            }).collect();
            lines.push(escaped.join(","));
        }
        Value::Atom(AtomKind::Str(lines.join("\n")), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);

    m.insert("csv.read_csv".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let v = oo.force(arg, ctx);
        let path = match extract_str_arg(&v, oo, ctx) {
            Some(s) => s,
            None => return BottomCause::Conflict.into(),
        };
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let rows = parse_csv(&content);
                let row_vals: Vec<Value> = rows.into_iter()
                    .map(|row| build_list(row.into_iter().map(|f| str_atom(f)).collect()))
                    .collect();
                let inner = build_list(row_vals);
                if let Value::Combo(c) = inner {
                    let rebuilt = ComboVal::new(c.fields(), false, IndexMap::new(), EffectTag::IO, vec![]);
                    Value::Combo(rebuilt)
                } else { inner }
            }
            Err(e) => Value::Bottom(Box::new(BottomDetail {
                cause: BottomCause::Conflict,
                message: Some(format!("csv.read_csv: {}", e)),
                ..Default::default()
            })),
        }
    }) as Arc<BuiltinFn>);
}
