//! The tactic machine: the tight inner loop that carries one subtask through the
//! tactic phases (Plan, Execute, Validate, Deliver by default).

pub mod runner;

use thiserror::Error;

use crate::feature::provider::ProviderError;

pub use runner::{TacticContext, TacticOutcome, run_tactic};

pub const MAX_RETRIES: usize = 5;

#[derive(Debug, Error)]
pub enum TacticError {
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error("invalid tactic: {0}")]
    Invalid(String),
    #[error("the model never moved phase '{0}' forward")]
    Stalled(String),
    #[error("validation kept failing after {MAX_RETRIES} retries")]
    TooManyRetries,
}
