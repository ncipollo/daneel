//! How the model drives the strategy and tactic machines: a small set of tools
//! that record its intent, plus the prompt wrapper that explains them.

pub mod protocol;
pub mod state;
pub mod tools;
pub mod turn;

pub use protocol::{PhaseView, wrap_prompt};
pub use state::{InteractionState, SharedState, Signal};
pub use tools::register_interaction_tools;
pub use turn::{PhaseTurn, TurnOutcome, brief};
