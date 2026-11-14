//! The strategy machine: carries a task through the strategy phases, running
//! every subtask the model creates through the tactic machine, and insists on
//! delivering a result at the end.

use std::path::Path;

use thiserror::Error;

use crate::feature::interaction::{InteractionState, PhaseTurn, PhaseView, Signal};
use crate::feature::interaction::{brief, wrap_prompt};
use crate::feature::provider::{Provider, ProviderError};
use crate::feature::session::Session;
use crate::feature::strategy::{PhaseConfig, PhaseList};
use crate::feature::tactic::{self, TacticContext, TacticError};
use crate::feature::task::Task;

/// Model turns allowed in one phase before it is declared stuck.
pub const MAX_ROUNDS: usize = 10;
pub const MAX_RETRIES: usize = 5;

#[derive(Debug, Error)]
pub enum StrategyError {
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error("invalid strategy: {0}")]
    Invalid(String),
    #[error("the model never moved phase '{0}' forward")]
    Stalled(String),
    #[error("the run ended without delivering a result to the user")]
    NotDelivered,
    #[error("went back too many times")]
    TooManyRetries,
    #[error("a subtask failed: {0}")]
    Tactic(#[from] TacticError),
}

pub struct StrategyContext<'a> {
    pub provider: &'a dyn Provider,
    pub working_dir: &'a Path,
    pub session: Option<&'a Session>,
    pub tactic: &'a PhaseList,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrategyOutcome {
    /// What was delivered to the user.
    pub delivered: String,
}

/// What a phase decided once its subtasks were done.
enum Step {
    Next,
    Back(usize),
    Delivered(String),
}

struct Run<'a> {
    ctx: &'a StrategyContext<'a>,
    strategy: &'a PhaseList,
    task: &'a Task,
    notes: Vec<String>,
    subtasks_run: usize,
}

/// Runs `task` through `strategy`; the final phase must deliver.
pub async fn run_strategy(
    ctx: &StrategyContext<'_>,
    strategy: &PhaseList,
    task: &Task,
) -> Result<StrategyOutcome, StrategyError> {
    strategy.validate().map_err(StrategyError::Invalid)?;
    record_task(ctx.session, task);
    let mut run = Run {
        ctx,
        strategy,
        task,
        notes: Vec::new(),
        subtasks_run: 0,
    };
    let (mut index, mut retries) = (0, 0);
    loop {
        match run.phase(index).await? {
            Step::Next => index += 1,
            Step::Delivered(delivered) => return Ok(StrategyOutcome { delivered }),
            Step::Back(target) => {
                retries += 1;
                if retries > MAX_RETRIES {
                    return Err(StrategyError::TooManyRetries);
                }
                index = target;
            }
        }
    }
}

impl Run<'_> {
    async fn phase(&mut self, index: usize) -> Result<Step, StrategyError> {
        let phase = &self.strategy.phases[index];
        let terminal = index + 1 == self.strategy.phases.len();
        for _ in 0..MAX_ROUNDS {
            let out = self.turn(index, terminal).await?;
            self.save_output(phase, index, &out.reply);
            self.notes
                .push(format!("{}: {}", phase.name, brief(&out.reply)));
            let ran = !out.created.is_empty();
            self.run_subtasks(out.created).await?;
            match out.signal {
                Some(Signal::Advance) => {
                    tracing::info!(from = phase.name.as_str(), "strategy phase transition");
                    return Ok(Step::Next);
                }
                Some(Signal::Complete { summary }) => {
                    tracing::info!(summary = summary.as_str(), "strategy delivered");
                    return Ok(Step::Delivered(summary));
                }
                Some(Signal::Retry {
                    phase: target,
                    reason,
                }) => {
                    tracing::info!(
                        from = phase.name.as_str(),
                        to = target.as_str(),
                        "strategy phase transition (retry)"
                    );
                    self.notes.push(format!("went back to {target}: {reason}"));
                    return Ok(Step::Back(self.position(&target)));
                }
                None if ran => continue,
                None if terminal => return Err(StrategyError::NotDelivered),
                None => return Err(StrategyError::Stalled(phase.name.clone())),
            }
        }
        Err(StrategyError::Stalled(phase.name.clone()))
    }

    async fn turn(
        &self,
        index: usize,
        terminal: bool,
    ) -> Result<crate::feature::interaction::TurnOutcome, StrategyError> {
        let phase = &self.strategy.phases[index];
        let names = self
            .strategy
            .phases
            .iter()
            .map(|p| p.name.clone())
            .collect();
        let state = InteractionState::new(names, index, terminal).with_subtasks(!terminal, 0);
        let view = PhaseView {
            task_type: &self.task.task_type,
            task_description: &self.task.description,
            phase: &phase.name,
            number: index + 1,
            total: self.strategy.phases.len(),
            terminal,
            can_create_subtasks: !terminal,
            notes: &self.notes,
            prompt: &phase.prompt,
        };
        let turn = PhaseTurn {
            provider: self.ctx.provider,
            working_dir: self.ctx.working_dir,
            allow_write: phase.settings.can_write_working_directory,
        };
        Ok(turn.run(state, &wrap_prompt(&view)).await?)
    }

    async fn run_subtasks(&mut self, subtasks: Vec<Task>) -> Result<(), StrategyError> {
        for subtask in subtasks {
            self.subtasks_run += 1;
            record_task(self.ctx.session, &subtask);
            let label = format!("subtask-{}", self.subtasks_run);
            let tactic_ctx = TacticContext {
                provider: self.ctx.provider,
                working_dir: self.ctx.working_dir,
                session: self.ctx.session,
                label: &label,
            };
            let outcome = tactic::run_tactic(&tactic_ctx, self.ctx.tactic, &subtask).await?;
            let note = format!(
                "subtask '{}' done: {}",
                subtask.description,
                brief(&outcome.summary)
            );
            self.notes.push(note);
        }
        Ok(())
    }

    fn position(&self, name: &str) -> usize {
        let phases = &self.strategy.phases;
        phases.iter().position(|p| p.name == name).unwrap_or(0)
    }

    fn save_output(&self, phase: &PhaseConfig, index: usize, reply: &str) {
        if let Some(session) = self.ctx.session {
            let name = format!("strategy-{}.txt", index + 1);
            let _ = session.write_phase_output(&phase.name.to_lowercase(), &name, reply);
        }
    }
}

fn record_task(session: Option<&Session>, task: &Task) {
    if let Some(session) = session {
        let _ = session.append_task(&format!("{}: {}", task.task_type, task.description));
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use serde_json::json;

    use super::*;
    use crate::feature::script_provider::{Call, ScriptProvider, call};
    use crate::feature::strategy::PhaseSettings;

    fn list(names: [&str; 2]) -> PhaseList {
        let phase = |name: &str, terminal: bool| PhaseConfig {
            name: name.into(),
            prompt: format!("{name} prompt"),
            settings: PhaseSettings {
                can_write_working_directory: false,
                terminal,
            },
        };
        PhaseList {
            phases: vec![phase(names[0], false), phase(names[1], true)],
        }
    }

    fn create(description: &str) -> Call {
        call("create_subtask", json!({ "description": description }))
    }

    fn advance() -> Vec<Call> {
        vec![call("advance_phase", json!({}))]
    }

    fn finish(summary: &str) -> Vec<Call> {
        vec![call("complete_task", json!({ "summary": summary }))]
    }

    async fn run(
        provider: &ScriptProvider,
        session: Option<&Session>,
    ) -> Result<StrategyOutcome, StrategyError> {
        let (strategy, tactic) = (list(["Discover", "Deliver"]), list(["Do", "Wrap"]));
        let ctx = StrategyContext {
            provider,
            working_dir: Path::new("."),
            session,
            tactic: &tactic,
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
    async fn advancing_with_open_subtasks_is_refused() {
        let mut first = vec![create("look")];
        first.extend(advance());
        let steps = vec![first, advance(), finish("ok"), advance(), finish("done")];
        let provider = ScriptProvider::new(steps);
        assert_eq!(run(&provider, None).await.unwrap().delivered, "done");
        assert_eq!(provider.prompts().len(), 5);
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
        let ctx = StrategyContext {
            provider: &provider,
            working_dir: Path::new("."),
            session: None,
            tactic: &tactic,
        };
        let empty = PhaseList { phases: vec![] };
        let err = run_strategy(&ctx, &empty, &Task::new("t", "d"))
            .await
            .unwrap_err();
        assert!(matches!(err, StrategyError::Invalid(_)));
    }
}
