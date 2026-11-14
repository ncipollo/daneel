use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use serde_json::json;
use tempfile::TempDir;

use super::*;
use crate::feature::events::{self, EventKind};
use crate::feature::provider::{BoxFuture, ChatRequest, ProviderError};
use crate::feature::script_provider::{ScriptProvider, advance, call, finish};
use crate::feature::strategy::{PhaseConfig, PhaseSettings};
use crate::feature::tools::safety::FixedValidator;

/// A strategy and tactic of one terminal phase that may write.
fn one_phase() -> PhaseList {
    PhaseList {
        phases: vec![PhaseConfig {
            name: "Deliver".into(),
            prompt: "Answer the task.".into(),
            settings: PhaseSettings {
                can_write_working_directory: true,
                terminal: true,
            },
        }],
    }
}

fn chat(provider: &Arc<ScriptProvider>, dir: &Path) -> Chat {
    Chat::new(provider.clone(), dir.to_path_buf()).with_phases(one_phase(), one_phase())
}

/// The first step answers the task-transform call; the rest drive the run.
fn script(steps: Vec<Vec<crate::feature::script_provider::Call>>) -> Arc<ScriptProvider> {
    let mut all = vec![vec![]];
    all.extend(steps);
    Arc::new(ScriptProvider::new(all))
}

fn session(root: &TempDir) -> Session {
    Session::create(root.path(), SystemTime::now(), Path::new("."), "m").unwrap()
}

#[tokio::test]
async fn a_message_is_delivered_through_the_strategy() {
    let provider = script(vec![finish("the answer")]);
    let mut chat = chat(&provider, Path::new("."));
    let reply = chat.send("what is it?", events::discard()).await.unwrap();
    assert_eq!(reply, "the answer");
    let prompts = provider.prompts();
    assert!(prompts[0].contains("what is it?"), "task transform first");
    assert!(prompts[1].contains("PHASE 1 of 1: Deliver"));
    assert!(prompts[1].contains("what is it?"));
}

#[tokio::test]
async fn the_default_five_phase_strategy_runs_end_to_end() {
    let steps = vec![
        advance(),
        advance(),
        advance(),
        advance(),
        finish("all done"),
    ];
    let provider = script(steps);
    let mut chat = Chat::new(provider.clone(), Path::new(".").to_path_buf());
    assert_eq!(
        chat.send("go", events::discard()).await.unwrap(),
        "all done"
    );
    assert!(provider.prompts()[5].contains("PHASE 5 of 5: Deliver"));
}

#[tokio::test]
async fn history_reaches_the_next_run() {
    let provider = script(vec![finish("first answer"), vec![], finish("second")]);
    let mut chat = chat(&provider, Path::new("."));
    chat.send("question one", events::discard()).await.unwrap();
    chat.send("question two", events::discard()).await.unwrap();
    let prompts = provider.prompts();
    assert!(prompts[3].contains("Earlier you were asked: question one"));
    assert!(prompts[3].contains("You answered: first answer"));
}

#[tokio::test]
async fn turns_and_the_task_are_recorded_in_the_session() {
    let root = TempDir::new().unwrap();
    let session = session(&root);
    let dir = session.dir().to_path_buf();
    let provider = script(vec![finish("hello back")]);
    let mut chat = chat(&provider, Path::new(".")).with_session(session);
    chat.send("hi", events::discard()).await.unwrap();
    let messages = std::fs::read_to_string(dir.join("messages.jsonl")).unwrap();
    assert_eq!(messages.lines().count(), 2);
    assert!(messages.contains(r#""role":"assistant","content":"hello back""#));
    let tasks = std::fs::read_to_string(dir.join("tasks.jsonl")).unwrap();
    assert_eq!(tasks.lines().count(), 1);
}

struct Failing;

impl Provider for Failing {
    fn chat<'a>(
        &'a self,
        _request: ChatRequest<'a>,
    ) -> BoxFuture<'a, Result<String, ProviderError>> {
        Box::pin(async { Err(ProviderError::Provider("kaboom".into())) })
    }
}

#[tokio::test]
async fn provider_failures_keep_their_message_and_are_recorded() {
    let root = TempDir::new().unwrap();
    let session = session(&root);
    let log = session.dir().join("messages.jsonl");
    let mut chat = Chat::new(Arc::new(Failing), Path::new(".").to_path_buf()).with_session(session);
    let err = chat.send("hi", events::discard()).await.unwrap_err();
    assert_eq!(err.to_string(), "provider error: kaboom");
    let text = std::fs::read_to_string(log).unwrap();
    assert!(text.contains(r#""role":"error","content":"provider error: kaboom""#));
}

#[tokio::test]
async fn a_run_that_never_delivers_is_an_error_not_an_empty_reply() {
    let provider = script(vec![vec![]]);
    let mut chat = chat(&provider, Path::new("."));
    let err = chat.send("hi", events::discard()).await.unwrap_err();
    assert!(err.to_string().contains("without delivering"), "{err}");
}

#[tokio::test]
async fn progress_and_tool_events_reach_the_sink() {
    let (sink, seen) = events::collector();
    let provider = script(vec![finish("done")]);
    chat(&provider, Path::new("."))
        .send("hi", sink)
        .await
        .unwrap();
    let seen = seen.lock().unwrap();
    assert!(matches!(&seen[0], EventKind::Task { description, .. } if description == "hi"));
    assert!(
        seen.iter()
            .any(|k| matches!(k, EventKind::Phase { name, .. } if name == "Deliver"))
    );
    assert!(
        seen.iter()
            .any(|k| matches!(k, EventKind::ToolCall { name, .. } if name == "complete_task"))
    );
}

#[tokio::test]
async fn messages_in_and_out_are_logged() {
    let logs = crate::feature::log_capture::capture();
    let provider = script(vec![finish("the reply")]);
    chat(&provider, Path::new("."))
        .send("hello there", events::discard())
        .await
        .unwrap();
    let text = logs.text();
    assert!(text.contains("direction=\"out\"") && text.contains("hello there"));
    assert!(text.contains("direction=\"in\"") && text.contains("the reply"));
}

#[tokio::test]
async fn failures_are_logged() {
    let logs = crate::feature::log_capture::capture();
    let mut chat = Chat::new(Arc::new(Failing), Path::new(".").to_path_buf());
    let _ = chat.send("hi", events::discard()).await;
    assert!(logs.text().contains("kaboom"));
}

#[tokio::test]
async fn a_blocked_write_inside_a_phase_never_touches_the_disk() {
    let dir = TempDir::new().unwrap();
    let write = call("write_file", json!({ "path": "x.txt", "content": "hi" }));
    let provider = script(vec![vec![write, finish("done").remove(0)]]);
    let validator = Arc::new(FixedValidator::unsafe_because("nope"));
    let mut chat = chat(&provider, dir.path()).with_safety(validator);
    assert_eq!(chat.send("go", events::discard()).await.unwrap(), "done");
    assert!(!dir.path().join("x.txt").exists());
}

#[tokio::test]
async fn custom_tools_are_available_inside_phases() {
    let (tools, work) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let body =
        r#"function parameters() { return {}; } function run() { return shell("touch made"); }"#;
    std::fs::write(tools.path().join("maker.js"), body).unwrap();
    let provider = script(vec![vec![call("maker", json!({})), finish("ok").remove(0)]]);
    let mut chat = chat(&provider, work.path()).with_tools_dir(tools.path().to_path_buf());
    chat.send("go", events::discard()).await.unwrap();
    assert!(work.path().join("made").exists());
}

#[test]
fn completion_events_cover_success_and_failure() {
    let ok = completion_events(&Ok("hi".to_string()));
    assert_eq!(ok.len(), 2);
    let err = completion_events(&Err(ProviderError::Provider("x".into())));
    assert!(matches!(err[0].kind, EventKind::Error { .. }));
}
