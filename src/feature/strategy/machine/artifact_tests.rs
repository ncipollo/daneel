use std::path::Path;
use std::time::SystemTime;

use serde_json::json;

use super::tests::{advance, finish, list};
use super::*;
use crate::feature::script_provider::{ScriptProvider, call};
use crate::feature::tools::ToolSetup;

fn artifact_session(root: &Path) -> Session {
    Session::create(root, SystemTime::now(), Path::new("."), "m").unwrap()
}

#[tokio::test]
async fn a_phase_must_save_its_deliverable_before_advancing() {
    let root = tempfile::TempDir::new().unwrap();
    let session = artifact_session(root.path());
    let mut strategy = list(["Discover", "Deliver"]);
    strategy.phases[0].settings.produces = vec!["findings.md".into()];
    let save = call(
        "write_artifact",
        json!({ "name": "findings.md", "content": "facts" }),
    );
    let provider = ScriptProvider::new(vec![
        advance(),
        vec![save, call("advance_phase", json!({}))],
        finish("done"),
    ]);
    let setup = ToolSetup::default();
    let env = Environment::new(&provider, Path::new("."), &setup).with_session(Some(&session));
    let tactic = list(["Do", "Wrap"]);
    let ctx = StrategyContext {
        env: &env,
        tactic: &tactic,
        background: &[],
    };
    let task = Task::new("query", "q");
    let outcome = run_strategy(&ctx, &strategy, &task).await.unwrap();
    assert_eq!(outcome.delivered, "done");
    let prompts = provider.prompts();
    assert!(prompts[0].contains("You must save findings.md"));
    let last = prompts.last().unwrap();
    assert!(last.contains("artifacts/task-1/discover/findings.md"));
}

#[tokio::test]
async fn a_second_task_in_a_session_keeps_its_own_artifacts() {
    let root = tempfile::TempDir::new().unwrap();
    let session = artifact_session(root.path());
    let mut strategy = list(["Discover", "Deliver"]);
    strategy.phases[0].settings.produces = vec!["findings.md".into()];
    let task_steps = |facts: &str, done: &str| {
        let save = call(
            "write_artifact",
            json!({ "name": "findings.md", "content": facts }),
        );
        vec![
            advance(),
            vec![save, call("advance_phase", json!({}))],
            finish(done),
        ]
    };
    let steps = [task_steps("one", "first"), task_steps("two", "second")].concat();
    let provider = ScriptProvider::new(steps);
    let setup = ToolSetup::default();
    let env = Environment::new(&provider, Path::new("."), &setup).with_session(Some(&session));
    let tactic = list(["Do", "Wrap"]);
    let ctx = StrategyContext {
        env: &env,
        tactic: &tactic,
        background: &[],
    };
    let task = Task::new("query", "q");
    run_strategy(&ctx, &strategy, &task).await.unwrap();
    let second = run_strategy(&ctx, &strategy, &task).await.unwrap();
    assert_eq!(second.delivered, "second");
    let read = |n: u8| {
        session
            .dir()
            .join(format!("artifacts/task-{n}/discover/findings.md"))
    };
    assert_eq!(std::fs::read_to_string(read(1)).unwrap(), "one");
    assert_eq!(std::fs::read_to_string(read(2)).unwrap(), "two");
}
