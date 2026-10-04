pub use crate::value::ObservationStrategy;
use crate::value::{
    BlurCause, BlurDetail, BottomCause, BottomDetail, ContentHash, EffectTag, Value,
};
use nlang_parser::ast::AtomKind;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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

/// Whether a resource-exhaustion mint needs `node_content` at all.
///
/// Only Blur strategy records partial (as its CAID). StackOverflow never
/// does — cloning a deep remaining AST just to discard it re-opens the
/// native-stack bomb on long chains (measured: 6000+ terms abort).
pub fn needs_partial_body(
    cause: &crate::ResourceExhausted,
    strategy: ObservationStrategy,
) -> bool {
    !matches!(cause, crate::ResourceExhausted::StackOverflow)
        && matches!(strategy, ObservationStrategy::Blur)
}

/// Mint a resource-exhaustion horizon value.
///
/// O42: blur identity takes budgets from `ctx` (CHS six params + partial CAID),
/// never salt or fuel_remaining-as-identity. Callers must not build a deep
/// partial when [`needs_partial_body`] is false.
pub fn handle_resource_exhausted(
    cause: crate::ResourceExhausted,
    strategy: ObservationStrategy,
    ctx: &crate::EvalContext,
    partial_result: Option<Value>,
    effect: EffectTag,
) -> Value {
    // A horizon suspension is part of this answer when an answer is in
    // progress. No active answer: injection and commit stay unmarked.
    note_horizon();
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
            found: None,
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
                found: None,
                involved: vec![],
                ..Default::default()
            }))
        }
        ObservationStrategy::Blur => {
            let blur_cause = match cause {
                crate::ResourceExhausted::FuelExhausted => BlurCause::FuelExhausted,
                crate::ResourceExhausted::Timeout => BlurCause::Timeout,
                crate::ResourceExhausted::StackOverflow => BlurCause::StackOverflow,
                crate::ResourceExhausted::DepthExceeded => BlurCause::MaxDepthExceeded,
            };
            // 11.5: partial enters identity as its CAID; body held for CAS write.
            Value::Blur(BlurDetail::from_single(
                blur_cause,
                ctx.horizon_params(),
                partial_result,
                effect,
            ))
        }
        ObservationStrategy::Approximate => {
            Value::Atom(AtomKind::Tag("approximate".to_string()), effect, None)
        }
    }
}

/// What one one-shot answer did, judged by the engine while it ran.
///
/// `reduced_thunk` is set only when a thunk body is actually evaluated.
/// A memo hit does not set it, and neither does spending the entry fuel.
/// `touched_horizon` is set when that answer suspends (fuel, depth,
/// timeout, the stack ceiling, or the approximate tag).
pub struct AnswerTrace {
    reduced_thunk: AtomicBool,
    touched_horizon: AtomicBool,
}

impl AnswerTrace {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            reduced_thunk: AtomicBool::new(false),
            touched_horizon: AtomicBool::new(false),
        })
    }

    pub fn reduced_thunk(&self) -> bool {
        self.reduced_thunk.load(Ordering::Relaxed)
    }

    pub fn touched_horizon(&self) -> bool {
        self.touched_horizon.load(Ordering::Relaxed)
    }
}

thread_local! {
    static ACTIVE_ANSWER: RefCell<Option<Arc<AnswerTrace>>> = const { RefCell::new(None) };
}

/// Restores the previous answer, including on panic.
pub struct AnswerTraceGuard {
    prev: Option<Arc<AnswerTrace>>,
}

impl Drop for AnswerTraceGuard {
    fn drop(&mut self) {
        let prev = self.prev.take();
        ACTIVE_ANSWER.with(|slot| {
            *slot.borrow_mut() = prev;
        });
    }
}

/// Marks thunk reduction and horizon suspension until dropped.
pub fn push_answer_trace(trace: Arc<AnswerTrace>) -> AnswerTraceGuard {
    let prev = ACTIVE_ANSWER.with(|slot| slot.borrow_mut().replace(trace));
    AnswerTraceGuard { prev }
}

pub fn note_thunk_reduced() {
    ACTIVE_ANSWER.with(|slot| {
        if let Some(trace) = slot.borrow().as_ref() {
            trace.reduced_thunk.store(true, Ordering::Relaxed);
        }
    });
}

pub fn note_horizon() {
    ACTIVE_ANSWER.with(|slot| {
        if let Some(trace) = slot.borrow().as_ref() {
            trace.touched_horizon.store(true, Ordering::Relaxed);
        }
    });
}
