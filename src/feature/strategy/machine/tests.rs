use std::path::Path;
use std::time::SystemTime;

use serde_json::json;

use super::*;
use crate::feature::script_provider::{Call, ScriptProvider, call};
use crate::feature::strategy::PhaseSettings;
use crate::feature::tools::ToolSetup;

pub(super) fn list(names: [&str; 2]) -> PhaseList {
    let phase = |name: &str, terminal: bool| PhaseConfig {
        name: name.into(),
        prompt: format!("{name} prompt"),
        settings: PhaseSettings {
            can_write_working_directory: false,
            terminal,
            produces: Vec::new(),
        },
    };
    PhaseList {
        phases: vec![phase(names[0], false), phase(names[1], true)],
    }
}

fn create(description: &str) -> Call {
    call("create_subtask", json!({ "description": description }))
}

pub(super) fn advance() -> Vec<Call> {
    vec![call("advance_phase", json!({}))]
}

pub(super) fn finish(summary: &str) -> Vec<Call> {
    vec![call("complete_task", json!({ "summary": summary }))]
}

async fn run(
    provider: &ScriptProvider,
    session: Option<&Session>,
) -> Result<StrategyOutcome, StrategyError> {
    let setup = ToolSetup::default();
    let env = Environment::new(provider, Path::new("."), &setup);
    run_in(&env.with_session(session), &[]).await
}

async fn run_in(
    env: &Environment<'_>,
    background: &[String],
) -> Result<StrategyOutcome, StrategyError> {
    let (strategy, tactic) = (list(["Discover", "Deliver"]), list(["Do", "Wrap"]));
    let ctx = StrategyContext {
        env,
        tactic: &tactic,
        background,
    };
    run_strategy(&ctx, &strategy, &Task::new("query", "answer it")).await
}

#[tokio::test]
async fn subtasks_run_through_the_tactic_before_the_phase_advances() {
    let steps = vec![
        vec![create("look"), create("count")],
        advance(),
        finish("found it"),
        advance(),
        finish("counted"),
        advance(),
        finish("the answer"),
    ];
    let provider = ScriptProvider::new(steps);
    let outcome = run(&provider, None).await.unwrap();
    assert_eq!(outcome.delivered, "the answer");
    let prompts = provider.prompts();
    assert_eq!(prompts.len(), 7);
    assert!(prompts[1].contains("PHASE 1 of 2: Do"));
    assert!(prompts[5].contains("subtask 'look' done: found it"));
    assert!(prompts[6].contains("PHASE 2 of 2: Deliver"));
}

#[tokio::test]
async fn phase_transitions_and_delivery_are_logged() {
    let logs = crate::feature::log_capture::capture();
    let provider = ScriptProvider::new(vec![advance(), finish("the answer")]);
    run(&provider, None).await.unwrap();
    let text = logs.text();
    assert!(text.contains("strategy phase transition"));
    assert!(text.contains("strategy delivered") && text.contains("the answer"));
}

#[tokio::test]
async fn the_task_and_every_phase_are_announced() {
    let (sink, seen) = crate::feature::events::collector();
    let provider = ScriptProvider::new(vec![advance(), finish("d")]);
    let setup = ToolSetup::default();
    let env = Environment::new(&provider, Path::new("."), &setup).with_sink(sink);
    run_in(&env, &[]).await.unwrap();
    let seen = seen.lock().unwrap();
    assert!(matches!(&seen[0], EventKind::Task { description, .. } if description == "answer it"));
    let names: Vec<_> = seen
        .iter()
        .filter_map(|k| match k {
            EventKind::Phase { scope, name, .. } if scope == "strategy" => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(names, ["Discover", "Deliver"]);
}

#[tokio::test]
async fn background_reaches_the_phase_prompts() {
    let provider = ScriptProvider::new(vec![advance(), finish("d")]);
    let setup = ToolSetup::default();
    let env = Environment::new(&provider, Path::new("."), &setup);
    let background = vec!["Earlier you were asked: fix it".to_string()];
    run_in(&env, &background).await.unwrap();
    assert!(provider.prompts()[0].contains("- Earlier you were asked: fix it"));
}

#[tokio::test]
async fn advancing_with_open_subtasks_is_refused() {
    let mut first = vec![create("look")];
    first.extend(advance());
    let steps = vec![first, advance(), finish("ok"), advance(), finish("done")];
    let provider = ScriptProvider::new(steps);
    assert_eq!(run(&provider, None).await.unwrap().delivered, "done");
    assert_eq!(provider.prompts().len(), 5);
}

fn skip(phase: &str) -> Vec<Call> {
    vec![call(
        "skip_to_phase",
        json!({ "phase": phase, "reason": "trivial" }),
    )]
}

async fn run_three(provider: &ScriptProvider) -> Result<StrategyOutcome, StrategyError> {
    let setup = ToolSetup::default();
    let env = Environment::new(provider, Path::new("."), &setup);
    let (mut strategy, tactic) = (list(["Triage", "Deliver"]), list(["Do", "Wrap"]));
    let middle = list(["Discover", "x"]).phases.remove(0);
    strategy.phases.insert(1, middle);
    let ctx = StrategyContext {
        env: &env,
        tactic: &tactic,
        background: &[],
    };
    run_strategy(&ctx, &strategy, &Task::new("query", "hi")).await
}

#[tokio::test]
async fn skipping_jumps_to_the_named_phase_and_leaves_a_note() {
    let provider = ScriptProvider::new(vec![skip("Deliver"), finish("hello")]);
    let outcome = run_three(&provider).await.unwrap();
    assert_eq!(outcome.delivered, "hello");
    let prompts = provider.prompts();
    assert_eq!(prompts.len(), 2);
    assert!(prompts[1].contains("PHASE 3 of 3: Deliver"));
    assert!(prompts[1].contains("skipped to Deliver: trivial"));
}

#[tokio::test]
async fn skipping_to_an_unknown_or_earlier_phase_is_refused() {
    let mut first = skip("Nowhere");
    first.extend(advance());
    let provider = ScriptProvider::new(vec![first, skip("Deliver"), finish("ok")]);
    assert_eq!(run_three(&provider).await.unwrap().delivered, "ok");
}

#[tokio::test]
async fn skipping_still_requires_delivery() {
    let provider = ScriptProvider::new(vec![skip("Deliver")]);
    let err = run_three(&provider).await.unwrap_err();
    assert!(matches!(err, StrategyError::NotDelivered));
}

#[tokio::test]
async fn ending_without_delivery_is_a_failure() {
    let provider = ScriptProvider::new(vec![advance()]);
    let err = run(&provider, None).await.unwrap_err();
    assert!(matches!(err, StrategyError::NotDelivered));
}

#[tokio::test]
async fn a_stuck_phase_is_reported_by_name() {
    let err = run(&ScriptProvider::new(vec![]), None).await.unwrap_err();
    assert!(matches!(err, StrategyError::Stalled(phase) if phase == "Discover"));
}

#[tokio::test]
async fn a_failed_subtask_fails_the_run() {
    let provider = ScriptProvider::new(vec![vec![create("look")]]);
    let err = run(&provider, None).await.unwrap_err();
    assert!(matches!(
        err,
        StrategyError::Tactic(TacticError::Stalled(_))
    ));
}

#[tokio::test]
async fn tasks_and_outputs_are_recorded_in_the_session() {
    let root = tempfile::TempDir::new().unwrap();
    let session = Session::create(root.path(), SystemTime::now(), Path::new("."), "m").unwrap();
    let steps = vec![
        vec![create("look")],
        advance(),
        finish("ok"),
        advance(),
        finish("d"),
    ];
    run(&ScriptProvider::new(steps), Some(&session))
        .await
        .unwrap();
    let tasks = std::fs::read_to_string(session.dir().join("tasks.jsonl")).unwrap();
    assert_eq!(tasks.lines().count(), 2);
    assert!(
        session
            .dir()
            .join("phases/discover/strategy-1.txt")
            .exists()
    );
}

#[tokio::test]
async fn an_invalid_strategy_is_rejected() {
    let tactic = list(["Do", "Wrap"]);
    let provider = ScriptProvider::new(vec![]);
    let setup = ToolSetup::default();
    let env = Environment::new(&provider, Path::new("."), &setup);
    let ctx = StrategyContext {
        env: &env,
        tactic: &tactic,
        background: &[],
    };
    let empty = PhaseList { phases: vec![] };
    let err = run_strategy(&ctx, &empty, &Task::new("t", "d"))
        .await
        .unwrap_err();
    assert!(matches!(err, StrategyError::Invalid(_)));
}

#[tokio::test]
async fn the_trace_follows_a_subtask_from_start_to_end() {
    let root = tempfile::TempDir::new().unwrap();
    let session = Session::create(root.path(), SystemTime::now(), Path::new("."), "m").unwrap();
    let steps = vec![
        vec![create("look")],
        advance(),
        finish("found it"),
        advance(),
        finish("the answer"),
    ];
    let provider = ScriptProvider::new(steps);
    let setup = ToolSetup::default();
    let sink = crate::feature::trace::tee(session.clone(), crate::feature::events::discard());
    let env = Environment::new(&provider, Path::new("."), &setup)
        .with_session(Some(&session))
        .with_sink(sink);
    run_in(&env, &[]).await.unwrap();
    let text = std::fs::read_to_string(session.dir().join("trace.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let shape: Vec<(String, String, String)> = rows
        .iter()
        .map(|r| {
            let field = |k: &str| r[k].as_str().unwrap_or("").to_string();
            (
                r["event"]["type"].as_str().unwrap_or("").to_string(),
                field("scope"),
                field("phase"),
            )
        })
        .collect();
    let row = |t: &str, s: &str, p: &str| (t.to_string(), s.to_string(), p.to_string());
    assert_eq!(shape[0], row("task", "", ""));
    assert_eq!(shape[1], row("phase", "strategy", "Discover"));
    let at = |r: &(String, String, String)| shape.iter().position(|s| s == r).unwrap();
    let start = at(&row("subtask_start", "strategy", "Discover"));
    assert!(start < at(&row("phase", "subtask 1", "Do")));
    assert!(start < at(&row("subtask_end", "subtask 1", "Wrap")));
    assert!(shape.contains(&row("transition", "subtask 1", "Do")));
    assert!(shape.contains(&row("subtask_end", "subtask 1", "Wrap")));
    assert!(shape.contains(&row("transition", "strategy", "Discover")));
}
