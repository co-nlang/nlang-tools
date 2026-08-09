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
    // W4‴: implementation stack ceiling is incapacity — always ⊥
    // `#stack_overflow`, never `#blur` (a blur claims an addressable snapshot;
    // an aborted stack has none). Strategy is ignored for this cause.
    if matches!(cause, crate::ResourceExhausted::StackOverflow) {
        return Value::Bottom(Box::new(BottomDetail {
            cause: BottomCause::StackOverflow,
            path: None,
            message: Some(
                "Implementation recursion limit exceeded (native stack ceiling)"
                    .to_string(),
            ),
            expected: None,
            found: partial_result,
            involved: vec![],
            ..Default::default()
        }));
    }
    match strategy {
        ObservationStrategy::Strict => {
            let cause_name = match cause {
                crate::ResourceExhausted::FuelExhausted => BottomCause::FuelExhausted,
                crate::ResourceExhausted::Timeout => BottomCause::Timeout,
                crate::ResourceExhausted::StackOverflow => BottomCause::StackOverflow,
                crate::ResourceExhausted::DepthExceeded => BottomCause::MaxDepthExceeded,
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
                // Unreachable: StackOverflow handled above.
                crate::ResourceExhausted::StackOverflow => BlurCause::StackOverflow,
                crate::ResourceExhausted::DepthExceeded => BlurCause::MaxDepthExceeded,
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
