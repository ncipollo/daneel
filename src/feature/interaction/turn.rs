//! One phase of model work: the prompt is sent, and if the model neither signals
//! nor creates subtasks it is nudged a couple of times before giving up.

use std::path::Path;

use crate::feature::interaction::register_interaction_tools;
use crate::feature::interaction::state::{InteractionState, SharedState, Signal};
use crate::feature::provider::{ChatMessage, ChatRequest, Provider, ProviderError, Role};
use crate::feature::task::Task;
use crate::feature::tools::{into_rig, phase_registry};

const INSTRUCTIONS: &str =
    "You are daneel, a careful assistant. Do exactly what the phase asks, using tools.";
const NUDGE: &str = "You did not call a tool, so nothing moved. Follow the HOW TO RESPOND lines above and call the right tool now.";
pub const MAX_NUDGES: usize = 2;
const BRIEF_CHARS: usize = 300;

/// What a phase run produced. `signal` and `created` are both empty when the
/// model never acted.
pub struct TurnOutcome {
    pub reply: String,
    pub signal: Option<Signal>,
    pub created: Vec<Task>,
}

pub struct PhaseTurn<'a> {
    pub provider: &'a dyn Provider,
    pub working_dir: &'a Path,
    pub allow_write: bool,
}

impl PhaseTurn<'_> {
    pub async fn run(
        &self,
        state: InteractionState,
        prompt: &str,
    ) -> Result<TurnOutcome, ProviderError> {
        let shared = state.shared();
        let mut history = Vec::new();
        let mut next = prompt.to_string();
        let mut reply = String::new();
        for _ in 0..=MAX_NUDGES {
            reply = self.ask(&shared, &history, &next).await?;
            if acted(&shared) {
                break;
            }
            history.push(message(Role::User, &next));
            history.push(message(Role::Assistant, &reply));
            next = NUDGE.to_string();
        }
        Ok(outcome(&shared, reply))
    }

    async fn ask(
        &self,
        shared: &SharedState,
        history: &[ChatMessage],
        prompt: &str,
    ) -> Result<String, ProviderError> {
        let mut registry = phase_registry(self.working_dir, self.allow_write);
        register_interaction_tools(&mut registry, shared);
        let mut request = ChatRequest::new(INSTRUCTIONS, prompt);
        request.history = history;
        request.tools = into_rig(registry);
        self.provider.chat(request).await
    }
}

/// A one-line version of `reply` for the progress notes in later prompts.
pub fn brief(reply: &str) -> String {
    let flat = reply.split_whitespace().collect::<Vec<_>>().join(" ");
    match flat.char_indices().nth(BRIEF_CHARS) {
        Some((end, _)) => format!("{}...", &flat[..end]),
        None => flat,
    }
}

fn acted(shared: &SharedState) -> bool {
    let state = shared.lock().unwrap_or_else(|e| e.into_inner());
    state.has_acted()
}

fn outcome(shared: &SharedState, reply: String) -> TurnOutcome {
    let mut state = shared.lock().unwrap_or_else(|e| e.into_inner());
    TurnOutcome {
        reply,
        signal: state.take_signal(),
        created: state.take_created(),
    }
}

fn message(role: Role, content: &str) -> ChatMessage {
    ChatMessage {
        role,
        content: content.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::feature::script_provider::{ScriptProvider, call};

    fn state() -> InteractionState {
        InteractionState::new(vec!["Plan".into(), "Deliver".into()], 0, false)
            .with_subtasks(true, 0)
    }

    async fn run(provider: &ScriptProvider) -> TurnOutcome {
        let turn = PhaseTurn {
            provider,
            working_dir: Path::new("."),
            allow_write: false,
        };
        turn.run(state(), "do it").await.unwrap()
    }

    #[tokio::test]
    async fn a_signal_ends_the_turn() {
        let provider = ScriptProvider::new(vec![vec![call("advance_phase", json!({}))]]);
        let out = run(&provider).await;
        assert_eq!(out.signal, Some(Signal::Advance));
        assert_eq!(provider.prompts().len(), 1);
    }

    #[tokio::test]
    async fn created_subtasks_end_the_turn() {
        let step = vec![call("create_subtask", json!({ "description": "a" }))];
        let out = run(&ScriptProvider::new(vec![step])).await;
        assert_eq!(out.created, vec![Task::new("general", "a")]);
        assert_eq!(out.signal, None);
    }

    #[tokio::test]
    async fn an_idle_model_is_nudged_then_given_up_on() {
        let provider = ScriptProvider::new(vec![]);
        let out = run(&provider).await;
        assert_eq!(out.signal, None);
        assert_eq!(provider.prompts().len(), MAX_NUDGES + 1);
        assert!(provider.prompts()[1].contains("did not call a tool"));
    }

    #[tokio::test]
    async fn a_nudge_can_recover() {
        let provider = ScriptProvider::new(vec![vec![], vec![call("advance_phase", json!({}))]]);
        assert_eq!(run(&provider).await.signal, Some(Signal::Advance));
    }

    #[test]
    fn brief_flattens_and_truncates() {
        assert_eq!(brief(" a \n b "), "a b");
        let long = "x".repeat(400);
        assert_eq!(brief(&long).len(), BRIEF_CHARS + 3);
    }
}
