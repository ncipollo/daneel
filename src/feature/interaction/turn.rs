//! One phase of model work: the prompt is sent, and if the model neither signals
//! nor creates subtasks it is nudged a couple of times before giving up.

use std::sync::{Arc, Mutex};

use crate::feature::events::{Event, EventKind, EventSink};
use crate::feature::interaction::environment::Environment;
use crate::feature::interaction::register_interaction_tools;
use crate::feature::interaction::state::{InteractionState, SharedState, Signal};
use crate::feature::provider::{ChatMessage, ChatRequest, ProviderError, Role};
use crate::feature::session::Session;
use crate::feature::task::Task;
use crate::feature::tools::write_artifact::WriteArtifact;
use crate::feature::tools::{assemble, into_rig_observed};

const INSTRUCTIONS: &str =
    "You are daneel, a careful assistant. Do exactly what the phase asks, using tools.";
const NUDGE: &str = "You did not call a tool, so nothing moved. Follow the HOW TO RESPOND lines above and call the right tool now.";
const NUDGE_AFTER_WORK: &str = "Your tool calls were done and their results are kept, so do not redo the work. The phase only moves when you call advance_phase (or complete_task in the last phase). If the work is finished, call it now.";
pub const MAX_NUDGES: usize = 2;
const BRIEF_CHARS: usize = 300;

/// What a phase run produced. `signal` and `created` are both empty when the
/// model never acted.
pub struct TurnOutcome {
    pub reply: String,
    pub signal: Option<Signal>,
    pub created: Vec<Task>,
}

/// Where a phase saves artifacts: the session and the phase name.
pub struct ArtifactScope<'a> {
    pub session: &'a Session,
    pub phase: &'a str,
}

pub struct PhaseTurn<'a> {
    pub env: &'a Environment<'a>,
    pub allow_write: bool,
    pub artifacts: Option<ArtifactScope<'a>>,
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
        let worked = Arc::new(Mutex::new(false));
        for _ in 0..=MAX_NUDGES {
            reply = self.ask(&shared, &history, &next, &worked).await?;
            if acted(&shared) {
                break;
            }
            history.push(message(Role::User, &next));
            history.push(message(Role::Assistant, &reply));
            next = nudge(&worked).to_string();
        }
        Ok(outcome(&shared, reply))
    }

    async fn ask(
        &self,
        shared: &SharedState,
        history: &[ChatMessage],
        prompt: &str,
        worked: &Arc<Mutex<bool>>,
    ) -> Result<String, ProviderError> {
        let env = self.env;
        let mut registry = assemble(env.working_dir, env.tools, self.allow_write);
        register_interaction_tools(&mut registry, shared);
        if let Some(scope) = &self.artifacts {
            registry.register(WriteArtifact::new(scope.session.clone(), scope.phase));
        }
        let mut request = ChatRequest::new(INSTRUCTIONS, prompt);
        request.history = history;
        request.tools = into_rig_observed(registry, watch_tools(&env.sink, worked));
        tracing::debug!(direction = "out", content = prompt, "model message");
        let reply = env.provider.chat(request).await?;
        tracing::debug!(direction = "in", content = reply.as_str(), "model message");
        Ok(reply)
    }
}

/// Passes events on, noting when the model called any tool.
fn watch_tools(inner: &EventSink, worked: &Arc<Mutex<bool>>) -> EventSink {
    let (inner, worked) = (inner.clone(), worked.clone());
    Arc::new(move |event: Event| {
        if matches!(event.kind, EventKind::ToolCall { .. }) {
            *worked.lock().unwrap_or_else(|e| e.into_inner()) = true;
        }
        inner(event);
    })
}

fn nudge(worked: &Arc<Mutex<bool>>) -> &'static str {
    if *worked.lock().unwrap_or_else(|e| e.into_inner()) {
        NUDGE_AFTER_WORK
    } else {
        NUDGE
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
    use std::path::Path;

    use serde_json::json;

    use super::*;
    use crate::feature::script_provider::{ScriptProvider, call};
    use crate::feature::tools::ToolSetup;

    fn state() -> InteractionState {
        InteractionState::new(vec!["Plan".into(), "Deliver".into()], 0, false)
            .with_subtasks(true, 0)
    }

    async fn run(provider: &ScriptProvider) -> TurnOutcome {
        let setup = ToolSetup::default();
        let env = Environment::new(provider, Path::new("."), &setup);
        let turn = PhaseTurn {
            env: &env,
            allow_write: false,
            artifacts: None,
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

    #[tokio::test]
    async fn a_nudge_after_work_says_not_to_redo_it() {
        let step = vec![call("list_files", json!({ "path": "." }))];
        let provider = ScriptProvider::new(vec![step]);
        let out = run(&provider).await;
        assert_eq!(out.signal, None);
        assert!(provider.prompts()[1].contains("do not redo the work"));
    }

    #[test]
    fn brief_flattens_and_truncates() {
        assert_eq!(brief(" a \n b "), "a b");
        let long = "x".repeat(400);
        assert_eq!(brief(&long).len(), BRIEF_CHARS + 3);
    }
}
