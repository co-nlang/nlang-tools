use crate::value::{ComboVal, EffectTag, Value};
use crate::{BuiltinFn, EvalContext, Ouroboros};
use indexmap::IndexMap;
use nlang_parser::ast::AtomKind;
use num_bigint::BigInt;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;

pub fn register_regex_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    // regex.match: {0: pattern, 1: str} → #true | #false  (Top if invalid pattern)
    m.insert(
        "regex.match".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vp), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
                    let fp = oo.force(vp.clone(), ctx);
                    let fs = oo.force(vs.clone(), ctx);
                    if let (
                        Value::Atom(AtomKind::Str(pattern), _, _),
                        Value::Atom(AtomKind::Str(s), _, _),
                    ) = (fp.collapse(), fs.collapse())
                    {
                        let tag = match Regex::new(pattern.as_str()) {
                            Ok(re) => {
                                if re.is_match(s.as_str()) {
                                    "true"
                                } else {
                                    "false"
                                }
                            }
                            Err(_) => return Value::Top,
                        };
                        return Value::Atom(AtomKind::Tag(tag.to_string()), EffectTag::Pure, None);
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // regex.find: {0: pattern, 1: str} → {match: Str, start: Int, end: Int} | #none
    m.insert(
        "regex.find".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vp), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
                    let fp = oo.force(vp.clone(), ctx);
                    let fs = oo.force(vs.clone(), ctx);
                    if let (
                        Value::Atom(AtomKind::Str(pattern), _, _),
                        Value::Atom(AtomKind::Str(s), _, _),
                    ) = (fp.collapse(), fs.collapse())
                    {
                        let re = match Regex::new(pattern.as_str()) {
                            Ok(r) => r,
                            Err(_) => return Value::Top,
                        };
                        return match re.find(s.as_str()) {
                            None => Value::Atom(
                                AtomKind::Tag("none".to_string()),
                                EffectTag::Pure,
                                None,
                            ),
                            Some(mat) => {
                                let matched = mat.as_str().to_string();
                                let char_start = s[..mat.start()].chars().count();
                                let char_end = char_start + matched.chars().count();
                                let mut res = IndexMap::new();
                                res.insert(
                                    "match".to_string(),
                                    Value::Atom(AtomKind::Str(matched), EffectTag::Pure, None),
                                );
                                res.insert(
                                    "start".to_string(),
                                    Value::Atom(
                                        AtomKind::Int(BigInt::from(char_start)),
                                        EffectTag::Pure,
                                        None,
                                    ),
                                );
                                res.insert(
                                    "end".to_string(),
                                    Value::Atom(
                                        AtomKind::Int(BigInt::from(char_end)),
                                        EffectTag::Pure,
                                        None,
                                    ),
                                );
                                Value::Combo(ComboVal::new(
                                    res,
                                    false,
                                    IndexMap::new(),
                                    EffectTag::Pure,
                                    vec![],
                                ))
                            }
                        };
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // regex.replace: {0: pattern, 1: replacement, 2: str} → Str (replace all)
    m.insert(
        "regex.replace".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vp), Some(vr), Some(vs)) =
                    (c.get_field("0"), c.get_field("1"), c.get_field("2"))
                {
                    let fp = oo.force(vp.clone(), ctx);
                    let fr = oo.force(vr.clone(), ctx);
                    let fs = oo.force(vs.clone(), ctx);
                    if let (
                        Value::Atom(AtomKind::Str(pattern), _, _),
                        Value::Atom(AtomKind::Str(replacement), _, _),
                        Value::Atom(AtomKind::Str(s), _, _),
                    ) = (fp.collapse(), fr.collapse(), fs.collapse())
                    {
                        return match Regex::new(pattern.as_str()) {
                            Err(_) => Value::Top,
                            Ok(re) => {
                                let result =
                                    re.replace_all(s.as_str(), replacement.as_str()).to_string();
                                Value::Atom(AtomKind::Str(result), EffectTag::Pure, None)
                            }
                        };
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );

    // regex.split: {0: pattern, 1: str} → list of Str
    m.insert(
        "regex.split".to_string(),
        Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
            if let Value::Combo(ref c) = arg {
                if let (Some(vp), Some(vs)) = (c.get_field("0"), c.get_field("1")) {
                    let fp = oo.force(vp.clone(), ctx);
                    let fs = oo.force(vs.clone(), ctx);
                    if let (
                        Value::Atom(AtomKind::Str(pattern), _, _),
                        Value::Atom(AtomKind::Str(s), _, _),
                    ) = (fp.collapse(), fs.collapse())
                    {
                        let re = match Regex::new(pattern.as_str()) {
                            Ok(r) => r,
                            Err(_) => return Value::Top,
                        };
                        let mut res = IndexMap::new();
                        for (i, part) in re.split(s.as_str()).enumerate() {
                            res.insert(
                                i.to_string(),
                                Value::Atom(AtomKind::Str(part.to_string()), EffectTag::Pure, None),
                            );
                        }
                        res.insert(
                            "%kind".to_string(),
                            Value::Atom(AtomKind::Tag("list".to_string()), EffectTag::Pure, None),
                        );
                        return Value::Combo(ComboVal::new(
                            res,
                            false,
                            IndexMap::new(),
                            EffectTag::Pure,
                            vec![],
                        ));
                    }
                }
            }
            Value::Top
        }) as Arc<BuiltinFn>,
    );
}
