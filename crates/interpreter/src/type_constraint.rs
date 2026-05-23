use crate::value::{Value, BottomCause, BottomDetail};
use nlang_parser::ast::AtomKind;
use num_traits::ToPrimitive;

pub enum TypeConstraint {
    Any,
    Num,
    Complex,
    Float,
    Int,
    Str,
    Bool,
    List,
    Combo,
    Morphism,
    Unknown(String),
}

impl TypeConstraint {
    pub fn from_name(name: &str) -> Self {
        match name.trim_start_matches('@').trim_start_matches('#') {
            "any" => TypeConstraint::Any,
            "num" => TypeConstraint::Num,
            "complex" => TypeConstraint::Complex,
            "float" => TypeConstraint::Float,
            "int" => TypeConstraint::Int,
            "str" => TypeConstraint::Str,
            "bool" => TypeConstraint::Bool,
            "list" => TypeConstraint::List,
            "combo" => TypeConstraint::Combo,
            "morphism" => TypeConstraint::Morphism,
            other => TypeConstraint::Unknown(other.to_string()),
        }
    }

    pub fn is_type_constraint_path(path_name: &str) -> bool {
        path_name.trim().starts_with('@')
    }

    pub fn validate_value(&self, value: &Value) -> ValidationResult {
        match self {
            TypeConstraint::Any => ValidationResult::Pass,
            TypeConstraint::Num => match value {
                Value::Atom(AtomKind::Int(_), _, _) | Value::Atom(AtomKind::Float(_), _, _) | Value::Atom(AtomKind::Complex(_, _), _, _) => ValidationResult::Pass,
                _ => ValidationResult::Fail("Value is not a number".to_string()),
            },
            TypeConstraint::Complex => match value {
                Value::Atom(AtomKind::Complex(_, _), _, _) => ValidationResult::Pass,
                Value::Atom(AtomKind::Float(_), _, _) => ValidationResult::Pass,
                Value::Atom(AtomKind::Int(_), _, _) => ValidationResult::Pass,
                _ => ValidationResult::Fail("Value is not a complex number".to_string()),
            },
            TypeConstraint::Float => match value {
                Value::Atom(AtomKind::Float(_), _, _) => ValidationResult::Pass,
                Value::Atom(AtomKind::Int(_), _, _) => ValidationResult::PassWithProjection,
                _ => ValidationResult::Fail("Value is not a float".to_string()),
            },
            TypeConstraint::Int => match value {
                Value::Atom(AtomKind::Int(_), _, _) => ValidationResult::Pass,
                _ => ValidationResult::Fail("Value is not an integer".to_string()),
            },
            TypeConstraint::Str => match value {
                Value::Atom(AtomKind::Str(_), _, _) | Value::Atom(AtomKind::MultilineStr(_), _, _) => ValidationResult::Pass,
                _ => ValidationResult::Fail("Value is not a string".to_string()),
            },
            TypeConstraint::Bool => match value {
                Value::Atom(AtomKind::Tag(t), _, _) => {
                    let tag = t.trim_start_matches('#');
                    if tag == "true" || tag == "false" {
                        ValidationResult::Pass
                    } else {
                        ValidationResult::Fail(format!("Tag {} is not a boolean", tag))
                    }
                }
                _ => ValidationResult::Fail("Value is not a boolean".to_string()),
            },
            TypeConstraint::List => match value {
                Value::Combo(cv) => {
                    if cv.get_field("%kind").map(|k| {
                        k.to_string_plain().trim_start_matches('#') == "list"
                    }).unwrap_or(false) {
                        ValidationResult::Pass
                    } else {
                        ValidationResult::Fail("Combo is not a list".to_string())
                    }
                }
                _ => ValidationResult::Fail("Value is not a list".to_string()),
            },
            TypeConstraint::Combo => match value {
                Value::Combo(_) => ValidationResult::Pass,
                _ => ValidationResult::Fail("Value is not a combo".to_string()),
            },
            TypeConstraint::Morphism => match value {
                Value::Combo(cv) => {
                    if cv.contains_key("%morphism") {
                        ValidationResult::Pass
                    } else {
                        ValidationResult::Fail("Combo is not a morphism".to_string())
                    }
                }
                _ => ValidationResult::Fail("Value is not a morphism".to_string()),
            },
            TypeConstraint::Unknown(name) => ValidationResult::Unknown(name.clone()),
        }
    }
}

pub enum ValidationResult {
    Pass,
    PassWithProjection,
    Fail(String),
    Unknown(String),
}

pub fn type_constraint_meet(value: Value, type_name: &str) -> Value {
    let constraint = TypeConstraint::from_name(type_name);
    let result = constraint.validate_value(&value);
    
    match result {
        ValidationResult::Pass => value,
        ValidationResult::PassWithProjection => {
            match &constraint {
                TypeConstraint::Float => match value {
                    Value::Atom(AtomKind::Int(i), e, _) => Value::Atom(AtomKind::Float(i.to_f64().unwrap_or(0.0)), e, None),
                    _ => value,
                },
                _ => value,
            }
        }
        ValidationResult::Fail(msg) => Value::Bottom(Box::new(BottomDetail {
            cause: BottomCause::Conflict,
            path: None,
            message: Some(msg),
            expected: None,
            found: Some(value),
            involved: vec![],
         ..Default::default() })),
        ValidationResult::Unknown(_) => value,
    }
}

pub fn is_type_constraint_combo(cv: &crate::value::ComboVal) -> bool {
    cv.get_field("%kind").map(|k| {
        k.to_string_plain().trim_start_matches('#') == "type_constraint"
    }).unwrap_or(false)
}

pub fn get_type_constraint_name(cv: &crate::value::ComboVal) -> Option<String> {
    cv.get_field("%type").and_then(|t| {
        match t {
            crate::value::Value::Atom(AtomKind::Str(name), _, _) => Some(name.clone()),
            _ => None,
        }
    })
}