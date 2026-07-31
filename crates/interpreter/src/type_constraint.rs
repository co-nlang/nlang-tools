use crate::value::{BottomCause, BottomDetail, Value};
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
    Option,
    Result,
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
            "option" => TypeConstraint::Option,
            "result" => TypeConstraint::Result,
            other => TypeConstraint::Unknown(other.to_string()),
        }
    }

    pub fn is_type_constraint_path(path_name: &str) -> bool {
        path_name.trim().starts_with('@')
    }

    /// Builtin reserved set (`from_name` ≠ Unknown). User `@Name` defs must
    /// not shadow these (E4 / `e4_builtin_reserved_not_shadowable`).
    pub fn is_builtin_type_name(name: &str) -> bool {
        !matches!(Self::from_name(name), TypeConstraint::Unknown(_))
    }

    /// Opaque `{{%kind: #type, %name: "…"}}` constraint marker used for
    /// builtins and the Unknown not-found fallback.
    /// SPEC_03 §4 / kind_tag B3 + type_super R1 (2026-07-22): role tag is
    /// canonical `#type`; name payload is `%name` (fossil `%type` retired —
    /// unifies with stdlib type-node spelling).
    pub fn marker_value(type_name: &str) -> Value {
        use crate::value::{ComboVal, EffectTag};
        use indexmap::IndexMap;
        Value::Combo(ComboVal::new(
            IndexMap::from_iter(vec![
                (
                    "%kind".to_string(),
                    Value::Atom(AtomKind::Tag("type".to_string()), EffectTag::Pure, None),
                ),
                (
                    "%name".to_string(),
                    Value::Atom(AtomKind::Str(type_name.to_string()), EffectTag::Pure, None),
                ),
            ]),
            true,
            IndexMap::new(),
            EffectTag::Pure,
            vec![],
        ))
    }

    /// SPEC_09 §2.1 hierarchy tree — immediate parent type name (no `@`).
    /// `None` = lattice top (`@any`) has no super (honest open-miss).
    /// Fixed-width ints → `int`; unknown / user names → `combo`.
    pub fn super_parent(type_name: &str) -> Option<&'static str> {
        let n = type_name.trim_start_matches('@');
        match n {
            "any" => None,
            "unit" | "bool" | "str" | "list" | "combo" | "morphism" | "type" | "caid" | "num"
            | "option" | "result" => Some("any"),
            "complex" | "int" => Some("num"),
            // §2.1 tree (not §2.3 non-immediate table): float under complex.
            "float" => Some("complex"),
            "record" => Some("combo"),
            n if is_fixed_width_int_name(n) => Some("int"),
            // User / unknown type names → field-structure family.
            _ => Some("combo"),
        }
    }

    pub fn validate_value(&self, value: &Value) -> ValidationResult {
        // E1: @T & Range = Range (refinement) iff non-anchor bounds pass @T.
        // Anchors (TagStart/TagEnd) always pass. Bounds are never rewritten
        // (PassWithProjection still returns the original Range in meet).
        if let Value::Range { start, end, step } = value {
            return self.validate_range_bounds(start, end, step.as_deref());
        }
        match self {
            TypeConstraint::Any => ValidationResult::Pass,
            TypeConstraint::Num => match value {
                Value::Atom(AtomKind::Int(_), _, _)
                | Value::Atom(AtomKind::Float(_), _, _)
                | Value::Atom(AtomKind::Complex(_, _), _, _) => ValidationResult::Pass,
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
                Value::Atom(AtomKind::Str(_), _, _)
                | Value::Atom(AtomKind::MultilineStr(_), _, _) => ValidationResult::Pass,
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
                    if cv
                        .get_field("%kind")
                        .map(|k| k.to_string_plain().trim_start_matches('#') == "list")
                        .unwrap_or(false)
                    {
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
            TypeConstraint::Option => match value {
                Value::Atom(AtomKind::Tag(t), _, _) if t == "none" => ValidationResult::Pass,
                Value::Combo(cv) if cv.get_field("%val").is_some() => ValidationResult::Pass,
                Value::Top => ValidationResult::Pass,
                _ => ValidationResult::Fail(
                    "Value is not @option (expected #none or Combo with %val)".to_string(),
                ),
            },
            TypeConstraint::Result => match value {
                Value::Combo(cv) if cv.get_field("%val").is_some() => ValidationResult::Pass,
                Value::Combo(cv) if cv.get_field("%cause").is_some() => ValidationResult::Pass,
                Value::Top => ValidationResult::Pass,
                _ => ValidationResult::Fail(
                    "Value is not @result (expected Combo with %val or %cause)".to_string(),
                ),
            },
            TypeConstraint::Unknown(name) => ValidationResult::Unknown(name.clone()),
        }
    }

    /// Validate Range bounds under this constraint. Order anchors always pass;
    /// non-anchor bounds must each validate. Result is Pass if all ok (including
    /// when bounds would project under @float — projection is NOT applied to bounds).
    fn validate_range_bounds(
        &self,
        start: &Value,
        end: &Value,
        step: Option<&Value>,
    ) -> ValidationResult {
        let is_order_anchor = |v: &Value| {
            matches!(
                v,
                Value::Atom(AtomKind::TagStart, _, _) | Value::Atom(AtomKind::TagEnd, _, _)
            )
        };
        let mut any_projection = false;
        for bound in [start, end].into_iter().chain(step.into_iter()) {
            if is_order_anchor(bound) {
                continue;
            }
            match self.validate_value(bound) {
                // Recursion: bound is atom → hits atom arms (not Range).
                ValidationResult::Pass => {}
                ValidationResult::PassWithProjection => {
                    any_projection = true;
                }
                ValidationResult::Fail(m) => return ValidationResult::Fail(m),
                ValidationResult::Unknown(u) => return ValidationResult::Unknown(u),
            }
        }
        if any_projection {
            ValidationResult::PassWithProjection
        } else {
            ValidationResult::Pass
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
            // Range bounds must never be rewritten (fmt v2 freeze / E1).
            if matches!(value, Value::Range { .. }) {
                return value;
            }
            match &constraint {
                TypeConstraint::Float => match value {
                    Value::Atom(AtomKind::Int(i), e, _) => {
                        Value::Atom(AtomKind::Float(i.to_f64().unwrap_or(0.0)), e, None)
                    }
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
            ..Default::default()
        })),
        ValidationResult::Unknown(_) => value,
    }
}

/// True iff this combo is a **constraint marker** (builtin / Unknown `@Name`
/// refine payload). Markers are the closed two-field cocoon
/// `{{%kind: #type, %name: "…"}}` minted by [`TypeConstraint::marker_value`].
/// Rich stdlib type nodes (`@option`/`@result`/`@list` on root) also carry
/// `%kind`+`%name` but have additional members — they are not markers.
/// Path resolution short-circuits builtins to markers, so meet sites see
/// markers; this predicate keys on the closed marker shape (no extra public
/// data fields beyond the reflection pair).
pub fn is_type_constraint_combo(cv: &crate::value::ComboVal) -> bool {
    let kind_is_type = cv
        .get_field("%kind")
        .map(|k| k.to_string_plain().trim_start_matches('#') == "type")
        .unwrap_or(false);
    if !kind_is_type || get_type_constraint_name(cv).is_none() {
        return false;
    }
    // Marker = closed cocoon whose only public payload is %kind + %name.
    // (stdlib type nodes add %fmap / %some / … and must not take the meet arm.)
    cv.closed && marker_field_count(cv) <= 2
}

fn marker_field_count(cv: &crate::value::ComboVal) -> usize {
    cv.fields().len()
}

/// Type name from the marker / type-node `%name` payload (R1).
pub fn get_type_constraint_name(cv: &crate::value::ComboVal) -> Option<String> {
    cv.get_field("%name").and_then(|t| match t {
        crate::value::Value::Atom(AtomKind::Str(name), _, _) => Some(name.clone()),
        _ => None,
    })
}

/// Fixed-width integer type names (`u8`..`u256`, `i8`..`i256`) under `@int`.
fn is_fixed_width_int_name(n: &str) -> bool {
    let rest = if let Some(r) = n.strip_prefix('u') {
        r
    } else if let Some(r) = n.strip_prefix('i') {
        r
    } else {
        return false;
    };
    matches!(rest, "8" | "16" | "32" | "64" | "128" | "256")
}

/// User field-structure type: a non-marker combo that embeds type markers
/// (e.g. `@Box: { value: @int }`). Its hierarchy parent is `@combo`.
pub fn is_user_field_type_combo(cv: &crate::value::ComboVal) -> bool {
    if is_type_constraint_combo(cv) {
        return false;
    }
    embeds_type_marker(cv)
}

fn embeds_type_marker(cv: &crate::value::ComboVal) -> bool {
    for (_, v) in cv.all_fields_iter() {
        match v {
            crate::value::Value::Combo(ref inner) if is_type_constraint_combo(inner) => {
                return true;
            }
            crate::value::Value::Combo(ref inner) if embeds_type_marker(inner) => return true,
            _ => {}
        }
    }
    for v in cv.local.values() {
        match v {
            crate::value::Value::Combo(ref inner) if is_type_constraint_combo(inner) => {
                return true;
            }
            crate::value::Value::Combo(ref inner) if embeds_type_marker(inner) => return true,
            _ => {}
        }
    }
    false
}
