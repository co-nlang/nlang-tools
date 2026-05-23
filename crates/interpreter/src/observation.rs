use crate::value::{Value, BottomCause, BottomDetail, EffectTag, ContentHash};
use nlang_parser::ast::AtomKind;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObservationState {
    Lazy,
    Incomplete,
    Converged,
    Conflict,
    Blur(ContentHash),
}

impl Default for ObservationState {
    fn default() -> Self {
        ObservationState::Lazy
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObservationStrategy {
    Blur,
    Strict,
    Approximate,
}

impl Default for ObservationStrategy {
    fn default() -> Self {
        ObservationStrategy::Blur
    }
}

impl ObservationState {
    pub fn has_caid(&self) -> bool {
        match self {
            ObservationState::Lazy => false,
            ObservationState::Incomplete => false,
            ObservationState::Converged => true,
            ObservationState::Conflict => true,
            ObservationState::Blur(_) => true,
        }
    }
    
    pub fn is_terminal(&self) -> bool {
        matches!(self, ObservationState::Converged | ObservationState::Conflict)
    }
    
    pub fn is_transient(&self) -> bool {
        matches!(self, ObservationState::Incomplete)
    }
    
    pub fn to_tag(&self) -> String {
        match self {
            ObservationState::Lazy => "lazy".to_string(),
            ObservationState::Incomplete => "incomplete".to_string(),
            ObservationState::Converged => "converged".to_string(),
            ObservationState::Conflict => "conflict".to_string(),
            ObservationState::Blur(_) => "blur".to_string(),
        }
    }
}

pub fn handle_resource_exhausted(
    cause: crate::ResourceExhausted,
    strategy: ObservationStrategy,
    horizon_salt: &ContentHash,
    partial_result: Option<Value>,
    effect: EffectTag,
) -> Value {
    match strategy {
        ObservationStrategy::Strict => {
            let cause_name = match cause {
                crate::ResourceExhausted::FuelExhausted => BottomCause::FuelExhausted,
                crate::ResourceExhausted::Timeout => BottomCause::Timeout,
                crate::ResourceExhausted::StackOverflow => BottomCause::Divergent,
            };
            Value::Bottom(Box::new(BottomDetail {
                cause: cause_name,
                path: None,
                message: Some("Resource exhausted in strict mode".to_string()),
                expected: None,
                found: partial_result,
                involved: vec![],
            }))
        }
        ObservationStrategy::Blur => {
            let blur_hash = compute_blur_caid(&cause, horizon_salt);
            Value::Combo(crate::value::ComboVal::new(
                indexmap::IndexMap::from_iter(vec![
                    ("%kind".to_string(), Value::Atom(AtomKind::Tag("blur".to_string()), EffectTag::Pure, None)),
                    ("%state".to_string(), Value::Atom(AtomKind::Str(blur_hash.to_string()), effect, None)),
                    ("%partial".to_string(), partial_result.unwrap_or(Value::Atom(AtomKind::Tag("incomplete".to_string()), effect, None))),
                ]),
                true,
                indexmap::IndexMap::new(),
                effect,
                vec![]
            ))
        }
        ObservationStrategy::Approximate => {
            Value::Atom(AtomKind::Tag("approximate".to_string()), effect, None)
        }
    }
}

fn compute_blur_caid(cause: &crate::ResourceExhausted, salt: &ContentHash) -> ContentHash {
    use sha2::{Sha256, Digest};
    
    let mut hasher = Sha256::new();
    hasher.update(salt.digest.as_slice());
    let cause_bytes: &[u8] = match cause {
        crate::ResourceExhausted::FuelExhausted => b"fuel_exhausted",
        crate::ResourceExhausted::Timeout => b"timeout",
        crate::ResourceExhausted::StackOverflow => b"stack_overflow",
    };
    hasher.update(cause_bytes);
    
    ContentHash {
        algorithm: crate::value::HashAlgorithm::Sha256,
        digest: hasher.finalize().to_vec(),
    }
}