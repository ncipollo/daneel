use serde_json::{Value, json};

use crate::feature::interaction::state::{InteractionState, SharedState};
use crate::feature::provider::BoxFuture;
use crate::feature::task::Task;
use crate::feature::tools::args::string_arg;
use crate::feature::tools::{AgentTool, ToolRegistry};

const TASK_TYPE_HINT: &str = "Short kind of work, such as 'query' or 'code creation'";

/// Adds the four interaction tools to `registry`, sharing one `state`.
pub fn register_interaction_tools(registry: &mut ToolRegistry, state: &SharedState) {
    registry.register(Interaction::advance(state.clone()));
    registry.register(Interaction::retry(state.clone()));
    registry.register(Interaction::complete(state.clone()));
    registry.register(Interaction::subtask(state.clone()));
}

type Action = fn(&mut InteractionState, &Value) -> Result<String, String>;

/// One tool: a name, a schema, and the state change it performs.
struct Interaction {
    name: &'static str,
    description: &'static str,
    schema: Value,
    action: Action,
    state: SharedState,
}

impl Interaction {
    fn advance(state: SharedState) -> Self {
        Self {
            name: "advance_phase",
            description: "Finish this phase and move to the next one.",
            schema: json!({ "type": "object", "properties": {} }),
            action: |s, _| s.advance().map(|()| "ok: moving to the next phase".into()),
            state,
        }
    }

    fn retry(state: SharedState) -> Self {
        Self {
            name: "retry_phase",
            description: "Go back to an earlier phase, for example when validation failed.",
            schema: json!({
                "type": "object",
                "properties": {
                    "phase": { "type": "string", "description": "Earlier phase to return to" },
                    "reason": { "type": "string", "description": "What went wrong" }
                },
                "required": ["phase"]
            }),
            action: |s, args| {
                let phase = string_arg(args, "phase")?;
                let reason = string_arg(args, "reason").unwrap_or_default();
                s.retry(&phase, &reason)
                    .map(|()| format!("ok: going back to {phase}"))
            },
            state,
        }
    }

    fn complete(state: SharedState) -> Self {
        Self {
            name: "complete_task",
            description: "Finish the task and deliver the result. Only in the last phase.",
            schema: json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string", "description": "The result to deliver" }
                },
                "required": ["summary"]
            }),
            action: |s, args| {
                let summary = string_arg(args, "summary")?;
                s.complete(&summary).map(|()| "ok: task complete".into())
            },
            state,
        }
    }

    fn subtask(state: SharedState) -> Self {
        Self {
            name: "create_subtask",
            description: "Add one smaller piece of work to do before moving on.",
            schema: json!({
                "type": "object",
                "properties": {
                    "task_type": { "type": "string", "description": TASK_TYPE_HINT },
                    "description": { "type": "string", "description": "What to do" }
                },
                "required": ["description"]
            }),
            action: |s, args| {
                let kind = string_arg(args, "task_type").unwrap_or_else(|_| "general".into());
                let description = string_arg(args, "description")?;
                s.create_subtask(Task::new(kind, description))
                    .map(|()| "ok: subtask created".into())
            },
            state,
        }
    }
}

impl AgentTool for Interaction {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        self.description
    }

    fn schema(&self) -> Value {
        self.schema.clone()
    }

    fn call<'a>(&'a self, args: Value) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "interaction state is unavailable")?;
            (self.action)(&mut state, &args)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::interaction::state::Signal;

    fn setup(current: usize, terminal: bool) -> (ToolRegistry, SharedState) {
        let phases = ["Plan", "Execute", "Deliver"].map(String::from).to_vec();
        let state = InteractionState::new(phases, current, terminal)
            .with_subtasks(!terminal, 0)
            .shared();
        let mut registry = ToolRegistry::new();
        register_interaction_tools(&mut registry, &state);
        (registry, state)
    }

    #[test]
    fn registers_the_four_tools() {
        let (registry, _) = setup(0, false);
        let names: Vec<_> = registry.names().collect();
        assert_eq!(
            names,
            [
                "advance_phase",
                "complete_task",
                "create_subtask",
                "retry_phase"
            ]
        );
    }

    #[tokio::test]
    async fn advance_records_the_signal() {
        let (registry, state) = setup(0, false);
        let out = registry.dispatch("advance_phase", json!({})).await;
        assert!(out.starts_with("ok"));
        assert_eq!(state.lock().unwrap().take_signal(), Some(Signal::Advance));
    }

    #[tokio::test]
    async fn invalid_transitions_come_back_as_errors() {
        let (registry, _) = setup(0, false);
        let out = registry
            .dispatch("complete_task", json!({ "summary": "x" }))
            .await;
        assert!(out.starts_with("error:") && out.contains("advance_phase"));
        let out = registry
            .dispatch("retry_phase", json!({ "phase": "Deliver" }))
            .await;
        assert!(out.starts_with("error:"));
    }

    #[tokio::test]
    async fn complete_and_retry_record_their_arguments() {
        let (registry, state) = setup(2, true);
        registry
            .dispatch("retry_phase", json!({ "phase": "Plan" }))
            .await;
        let signal = state.lock().unwrap().take_signal();
        assert_eq!(
            signal,
            Some(Signal::Retry {
                phase: "Plan".into(),
                reason: String::new()
            })
        );
        registry
            .dispatch("complete_task", json!({ "summary": "done" }))
            .await;
        let signal = state.lock().unwrap().take_signal();
        assert_eq!(
            signal,
            Some(Signal::Complete {
                summary: "done".into()
            })
        );
    }

    #[tokio::test]
    async fn create_subtask_defaults_the_type_and_requires_a_description() {
        let (registry, state) = setup(0, false);
        registry
            .dispatch("create_subtask", json!({ "description": "look" }))
            .await;
        let out = registry.dispatch("create_subtask", json!({})).await;
        assert!(out.contains("description"));
        let created = state.lock().unwrap().take_created();
        assert_eq!(created, vec![Task::new("general", "look")]);
    }
}
