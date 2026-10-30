use crate::value::{BottomCause, BottomDetail, ComboVal, EffectTag, Value};
use crate::{BuiltinFn, EvalContext, Ouroboros};
use indexmap::IndexMap;
use nlang_parser::ast::AtomKind;
use std::collections::HashMap;
use std::sync::Arc;

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
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
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
        if !current_row.is_empty() {
            rows.push(current_row);
        }
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
    m.insert(
        "%kind".to_string(),
        Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
    );
    for (i, v) in items.iter().enumerate() {
        m.insert(i.to_string(), v.clone());
    }
    Value::Combo(ComboVal::new(
        m,
        false,
        IndexMap::new(),
        EffectTag::Pure,
        vec![],
    ))
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
    } else {
        vec![]
    }
}

fn csv_text(rows: &Value, oo: &Ouroboros, ctx: &mut EvalContext) -> Value {
    let Value::Combo(ref rc) = rows else {
        return BottomCause::Conflict.into();
    };
    if !is_list(rc) {
        return BottomCause::Conflict.into();
    }
    let rows = extract_list_items(rows, oo, ctx);
    let cell = |v: &Value| match v {
        Value::Atom(AtomKind::Str(s), _, _) => Some(escape_csv_field(s)),
        Value::Atom(..) => Some(escape_csv_field(&v.to_string_plain())),
        _ => None,
    };
    let mut lines: Vec<String> = Vec::new();
    let header: Option<Vec<String>> = match rows.first() {
        // The keys in the order their content gives, as JSON and TOML
        // write them (REAL_03 §6.7): two `=` records write one header.
        Some(Value::Combo(c)) if !is_list(c) => {
            let mut h: Vec<String> = c.data.keys().filter(|k| !k.starts_with('%')).cloned().collect();
            h.sort();
            Some(h)
        }
        _ => None,
    };
    if let Some(h) = &header {
        lines.push(h.iter().map(|k| escape_csv_field(k)).collect::<Vec<_>>().join(","));
    }
    for row in &rows {
        let Value::Combo(c) = row else {
            return BottomCause::Conflict.into();
        };
        let cells: Option<Vec<String>> = match &header {
            Some(h) => {
                if is_list(c) || c.data.keys().any(|k| !k.starts_with('%') && !h.contains(k)) {
                    return BottomCause::Conflict.into();
                }
                h.iter().map(|k| c.data.get(k).map_or(Some(String::new()), &cell)).collect()
            }
            None => {
                if !is_list(c) {
                    return BottomCause::Conflict.into();
                }
                extract_list_items(row, oo, ctx).iter().map(&cell).collect()
            }
        };
        match cells {
            Some(cells) => lines.push(cells.join(",")),
            None => return BottomCause::Conflict.into(),
        }
    }
    Value::Atom(AtomKind::Str(lines.join("\n")), EffectTag::Pure, None)
}

fn is_list(c: &ComboVal) -> bool {
    matches!(c.get_field("%kind"), Some(Value::Atom(AtomKind::Tag(t), _, _)) if t.trim_start_matches('#') == "list")
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
    m.insert(
        "csv.parse".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = oo.force(arg, ctx);
            let s = match extract_str_arg(&v, oo, ctx) {
                Some(s) => s,
                None => return BottomCause::Conflict.into(),
            };
            let rows = parse_csv(&s);
            let row_vals: Vec<Value> = rows
                .into_iter()
                .map(|row| build_list(row.into_iter().map(|f| str_atom(f)).collect()))
                .collect();
            build_list(row_vals)
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "csv.parse_with_headers".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = oo.force(arg, ctx);
            let s = match extract_str_arg(&v, oo, ctx) {
                Some(s) => s,
                None => return BottomCause::Conflict.into(),
            };
            let mut rows = parse_csv(&s);
            if rows.is_empty() {
                return build_list(vec![]);
            }
            let headers = rows.remove(0);
            let record_vals: Vec<Value> = rows
                .into_iter()
                .map(|row| {
                    let mut fields = IndexMap::new();
                    for (i, header) in headers.iter().enumerate() {
                        let val = row.get(i).cloned().unwrap_or_default();
                        fields.insert(header.clone(), str_atom(val));
                    }
                    Value::Combo(ComboVal::new(
                        fields,
                        false,
                        IndexMap::new(),
                        EffectTag::Pure,
                        vec![],
                    ))
                })
                .collect();
            build_list(record_vals)
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "csv.stringify".to_string(),
        // {0: rows} → CSV text. A row is a list of cells, or a record (D124):
        // the first record's keys, in order, are the header line and every
        // record is written in that order — a key it lacks is an empty cell,
        // a key the header lacks is a conflict. A cell is the value it reads
        // as (Q-113): a string as itself, another atom as it prints; a cell
        // that is a structure is a conflict.
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let rows = match &arg {
                Value::Combo(c) if !is_list(c) => c.get_field("0").cloned().unwrap_or(Value::Top),
                _ => arg.clone(),
            };
            super::written_text(oo, rows, ctx, &mut |rows, ctx| csv_text(rows, oo, ctx))
        }) as Arc<BuiltinFn>,
    );

    m.insert(
        "csv.read_csv".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            let v = oo.force(arg, ctx);
            let path = match extract_str_arg(&v, oo, ctx) {
                Some(s) => s,
                None => return BottomCause::Conflict.into(),
            };
            if crate::builtins::fs_guard::crosses_store_boundary(&path) {
                return crate::builtins::fs_guard::store_boundary_refusal(&path);
            }
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let rows = parse_csv(&content);
                    let row_vals: Vec<Value> = rows
                        .into_iter()
                        .map(|row| build_list(row.into_iter().map(|f| str_atom(f)).collect()))
                        .collect();
                    let inner = build_list(row_vals);
                    if let Value::Combo(c) = inner {
                        let rebuilt = ComboVal::new(
                            c.fields(),
                            false,
                            IndexMap::new(),
                            EffectTag::IO,
                            vec![],
                        );
                        Value::Combo(rebuilt)
                    } else {
                        inner
                    }
                }
                Err(e) => Value::Bottom(Box::new(BottomDetail {
                    cause: BottomCause::Conflict,
                    message: Some(format!("csv.read_csv: {}", e)),
                    ..Default::default()
                })),
            }
        }) as Arc<BuiltinFn>,
    );
}
