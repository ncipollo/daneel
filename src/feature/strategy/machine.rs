//! The strategy machine: carries a task through the strategy phases, running
//! every subtask the model creates through the tactic machine, and insists on
//! delivering a result at the end.

use std::path::PathBuf;

use thiserror::Error;

use crate::feature::events::{Event, EventKind};
use crate::feature::interaction::{
    ArtifactInfo, ArtifactScope, Environment, InteractionState, PhaseTurn, PhaseView, Signal,
};
use crate::feature::interaction::{brief, wrap_prompt};
use crate::feature::provider::ProviderError;
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

/// Provider failures keep their own error (so messages stay short and familiar);
/// every other machine failure becomes a `Run` error.
impl From<StrategyError> for ProviderError {
    fn from(error: StrategyError) -> Self {
        match error {
            StrategyError::Provider(inner)
            | StrategyError::Tactic(TacticError::Provider(inner)) => inner,
            other => ProviderError::Run(other.to_string()),
        }
    }
}

pub struct StrategyContext<'a> {
    pub env: &'a Environment<'a>,
    pub tactic: &'a PhaseList,
    /// What came up earlier in the conversation; it starts every phase's notes.
    pub background: &'a [String],
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
    Skip(usize),
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
    record_task(ctx.env.session, task);
    (ctx.env.sink)(Event::new(EventKind::Task {
        task_type: task.task_type.clone(),
        description: task.description.clone(),
    }));
    let mut run = Run {
        ctx,
        strategy,
        task,
        notes: ctx.background.to_vec(),
        subtasks_run: 0,
    };
    let (mut index, mut retries) = (0, 0);
    loop {
        match run.phase(index).await? {
            Step::Next => index += 1,
            Step::Skip(target) => index = target,
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
        self.announce(phase, index);
        for _ in 0..MAX_ROUNDS {
            let out = self.turn(index, terminal).await?;
            self.save_output(phase, index, &out.reply);
            self.notes
                .push(format!("{}: {}", phase.name, brief(&out.reply)));
            let ran = !out.created.is_empty();
            self.run_subtasks(out.created).await?;
            match out.signal {
                Some(Signal::Advance) => {
                    self.transition(phase, "advance", self.next_name(index), "");
                    tracing::info!(from = phase.name.as_str(), "strategy phase transition");
                    return Ok(Step::Next);
                }
                Some(Signal::Complete { summary }) => {
                    self.transition(phase, "complete", "", "");
                    tracing::info!(summary = summary.as_str(), "strategy delivered");
                    return Ok(Step::Delivered(summary));
                }
                Some(Signal::Skip {
                    phase: target,
                    reason,
                }) => {
                    tracing::info!(
                        from = phase.name.as_str(),
                        to = target.as_str(),
                        "strategy phase transition (skip)"
                    );
                    self.transition(phase, "skip", &target, &reason);
                    self.notes.push(format!("skipped to {target}: {reason}"));
                    return Ok(Step::Skip(self.position(&target)));
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
                    self.transition(phase, "retry", &target, &reason);
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
        let artifacts = self.artifact_info(phase);
        let required = required_paths(artifacts.as_ref());
        let state = InteractionState::new(names, index, terminal)
            .with_subtasks(!terminal, 0)
            .with_required(required);
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
            artifacts: artifacts.as_ref(),
        };
        let turn = PhaseTurn {
            env: self.ctx.env,
            allow_write: phase.settings.can_write_working_directory,
            artifacts: self.ctx.env.session.map(|session| ArtifactScope {
                session,
                phase: &phase.name,
            }),
        };
        Ok(turn.run(state, &wrap_prompt(&view)).await?)
    }

    async fn run_subtasks(&mut self, subtasks: Vec<Task>) -> Result<(), StrategyError> {
        for subtask in subtasks {
            self.subtasks_run += 1;
            record_task(self.ctx.env.session, &subtask);
            let label = format!("subtask-{}", self.subtasks_run);
            (self.ctx.env.sink)(Event::new(EventKind::SubtaskStart {
                label: label.clone(),
                task_type: subtask.task_type.clone(),
                description: subtask.description.clone(),
            }));
            let tactic_ctx = TacticContext {
                env: self.ctx.env,
                label: &label,
            };
            let outcome = tactic::run_tactic(&tactic_ctx, self.ctx.tactic, &subtask).await?;
            (self.ctx.env.sink)(Event::new(EventKind::SubtaskEnd {
                label: label.clone(),
                summary: outcome.summary.clone(),
            }));
            let note = format!(
                "subtask '{}' done: {}",
                subtask.description,
                brief(&outcome.summary)
            );
            self.notes.push(note);
        }
        Ok(())
    }

    fn artifact_info(&self, phase: &PhaseConfig) -> Option<ArtifactInfo> {
        let session = self.ctx.env.session?;
        Some(ArtifactInfo {
            dir: session.artifact_dir(&phase.name),
            produces: phase.settings.produces.clone(),
            earlier: session.list_artifacts(),
        })
    }

    fn next_name(&self, index: usize) -> &str {
        let phases = &self.strategy.phases;
        phases.get(index + 1).map_or("", |p| p.name.as_str())
    }

    fn transition(&self, from: &PhaseConfig, kind: &str, to: &str, reason: &str) {
        (self.ctx.env.sink)(Event::new(EventKind::Transition {
            scope: "strategy".to_string(),
            from: from.name.clone(),
            to: to.to_string(),
            kind: kind.to_string(),
            reason: reason.to_string(),
        }));
    }

    fn announce(&self, phase: &PhaseConfig, index: usize) {
        (self.ctx.env.sink)(Event::new(EventKind::Phase {
            scope: "strategy".to_string(),
            name: phase.name.clone(),
            number: index + 1,
            total: self.strategy.phases.len(),
        }));
    }

    fn position(&self, name: &str) -> usize {
        let phases = &self.strategy.phases;
        phases.iter().position(|p| p.name == name).unwrap_or(0)
    }

    fn save_output(&self, phase: &PhaseConfig, index: usize, reply: &str) {
        if let Some(session) = self.ctx.env.session {
            let name = format!("strategy-{}.txt", index + 1);
            let _ = session.write_phase_output(&phase.name.to_lowercase(), &name, reply);
        }
    }
}

fn required_paths(info: Option<&ArtifactInfo>) -> Vec<PathBuf> {
    let Some(info) = info else {
        return Vec::new();
    };
    info.produces
        .iter()
        .map(|name| info.dir.join(name))
        .collect()
}

fn record_task(session: Option<&Session>, task: &Task) {
    if let Some(session) = session {
        let _ = session.append_task(&format!("{}: {}", task.task_type, task.description));
    }
}

#[cfg(test)]
mod tests;
