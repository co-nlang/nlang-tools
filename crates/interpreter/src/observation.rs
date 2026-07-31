pub use crate::value::ObservationStrategy;
use crate::value::{
    BlurCause, BlurDetail, BottomCause, BottomDetail, ContentHash, EffectTag, HorizonParams, Value,
};
use nlang_parser::ast::AtomKind;
use serde::{Deserialize, Serialize};

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
        matches!(
            self,
            ObservationState::Converged | ObservationState::Conflict
        )
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
    fuel_remaining: u64,
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
                ..Default::default()
            }))
        }
        ObservationStrategy::Blur => {
            let blur_cause = match cause {
                crate::ResourceExhausted::FuelExhausted => BlurCause::FuelExhausted,
                crate::ResourceExhausted::Timeout => BlurCause::Timeout,
                crate::ResourceExhausted::StackOverflow => BlurCause::StackOverflow,
            };
            Value::Blur(BlurDetail {
                cause: blur_cause,
                horizon: HorizonParams {
                    fuel_remaining,
                    strategy,
                    salt: horizon_salt.clone(),
                },
                partial: partial_result.map(Box::new),
                effect,
            })
        }
        ObservationStrategy::Approximate => {
            Value::Atom(AtomKind::Tag("approximate".to_string()), effect, None)
        }
    }
}
