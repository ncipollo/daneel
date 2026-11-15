//! One phase of model work: the prompt is sent, and if the model neither signals
//! nor creates subtasks it is nudged a couple of times before giving up.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

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
const NUDGE_AFTER_WORK: &str = "Your tool calls were done and their results are kept, so do not redo the work. Writing text does not run anything: a file only exists once write_file or write_artifact succeeded. If something is still missing, call the tool for it now. If the work is finished, call advance_phase (or complete_task in the last phase) now.";
pub const MAX_NUDGES: usize = 2;
/// Hard cap on model calls in one phase, however much progress they make.
const MAX_ASKS: usize = 12;
/// Lets tool calls that ran alongside the signal finish before the model call is cut off.
const SIGNAL_GRACE: Duration = Duration::from_millis(1500);
const SIGNAL_POLL: Duration = Duration::from_millis(100);
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
        let calls = Arc::new(AtomicUsize::new(0));
        let mut idle = 0;
        for _ in 0..MAX_ASKS {
            let before = calls.load(Ordering::SeqCst);
            reply = self.ask(&shared, &history, &next, &calls).await?;
            if acted(&shared) {
                break;
            }
            idle = if calls.load(Ordering::SeqCst) > before {
                0
            } else {
                idle + 1
            };
            if idle > MAX_NUDGES {
                break;
            }
            history.push(message(Role::User, &next));
            history.push(message(Role::Assistant, &reply));
            next = nudge(&calls).to_string();
        }
        Ok(outcome(&shared, reply))
    }

    async fn ask(
        &self,
        shared: &SharedState,
        history: &[ChatMessage],
        prompt: &str,
        calls: &Arc<AtomicUsize>,
    ) -> Result<String, ProviderError> {
        let env = self.env;
        let mut registry = assemble(env.working_dir, env.tools, self.allow_write);
        register_interaction_tools(&mut registry, shared);
        if let Some(scope) = &self.artifacts {
            registry.register(WriteArtifact::new(scope.session.clone(), scope.phase));
        }
        let mut request = ChatRequest::new(INSTRUCTIONS, prompt);
        request.history = history;
        request.tools = into_rig_observed(registry, watch_tools(&env.sink, calls));
        tracing::debug!(direction = "out", content = prompt, "model message");
        let reply = tokio::select! {
            biased;
            reply = env.provider.chat(request) => reply?,
            () = signalled(shared) => String::new(),
        };
        tracing::debug!(direction = "in", content = reply.as_str(), "model message");
        Ok(reply)
    }
}

/// Resolves shortly after the model signals a phase change. The model keeps
/// calling tools after the signal otherwise, which stalls the phase for minutes.
async fn signalled(shared: &SharedState) {
    while !has_signal(shared) {
        tokio::time::sleep(SIGNAL_POLL).await;
    }
    tokio::time::sleep(SIGNAL_GRACE).await;
}

fn has_signal(shared: &SharedState) -> bool {
    let state = shared.lock().unwrap_or_else(|e| e.into_inner());
    state.has_signal()
}

/// Passes events on, counting the model's tool calls.
fn watch_tools(inner: &EventSink, calls: &Arc<AtomicUsize>) -> EventSink {
    let (inner, calls) = (inner.clone(), calls.clone());
    Arc::new(move |event: Event| {
        if matches!(event.kind, EventKind::ToolCall { .. }) {
            calls.fetch_add(1, Ordering::SeqCst);
        }
        inner(event);
    })
}

fn nudge(calls: &Arc<AtomicUsize>) -> &'static str {
    if calls.load(Ordering::SeqCst) > 0 {
        NUDGE_AFTER_WORK
    } else {
        NUDGE
    }
}

/// A one-line version of `reply` for the progress notes in later prompts.
pub fn brief(reply: &str) -> String {
    let flat = reply.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.is_empty() {
        return "done".to_string();
    }
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
    use crate::feature::provider::{BoxFuture, Provider};
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

    struct Hang(SharedState);

    impl Provider for Hang {
        fn chat<'a>(&'a self, _: ChatRequest<'a>) -> BoxFuture<'a, Result<String, ProviderError>> {
            Box::pin(async move {
                self.0.lock().unwrap().advance().unwrap();
                std::future::pending().await
            })
        }
    }

    #[tokio::test]
    async fn a_model_that_keeps_going_after_signalling_is_cut_off() {
        let state = state().shared();
        let hang = Hang(state.clone());
        let setup = ToolSetup::default();
        let env = Environment::new(&hang, Path::new("."), &setup);
        let turn = PhaseTurn {
            env: &env,
            allow_write: false,
            artifacts: None,
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let done = turn.ask(&state, &[], "do it", &calls);
        let reply = tokio::time::timeout(Duration::from_secs(10), done).await;
        assert_eq!(reply.unwrap().unwrap(), "");
        assert_eq!(state.lock().unwrap().take_signal(), Some(Signal::Advance));
    }

    #[tokio::test]
    async fn rounds_that_use_tools_do_not_spend_the_nudge_budget() {
        let work = || vec![call("list_files", json!({ "path": "." }))];
        let mut steps: Vec<_> = (0..MAX_NUDGES + 2).map(|_| work()).collect();
        steps.push(vec![call("advance_phase", json!({}))]);
        let provider = ScriptProvider::new(steps);
        assert_eq!(run(&provider).await.signal, Some(Signal::Advance));
    }

    #[test]
    fn brief_flattens_and_truncates() {
        assert_eq!(brief(" a \n b "), "a b");
        assert_eq!(brief("  "), "done");
        let long = "x".repeat(400);
        assert_eq!(brief(&long).len(), BRIEF_CHARS + 3);
    }
}
