use std::path::Path;

use crate::feature::interaction::{InteractionState, PhaseTurn, PhaseView, Signal};
use crate::feature::interaction::{brief, wrap_prompt};
use crate::feature::provider::Provider;
use crate::feature::session::Session;
use crate::feature::strategy::{PhaseConfig, PhaseList};
use crate::feature::tactic::{MAX_RETRIES, TacticError};
use crate::feature::task::Task;

pub struct TacticContext<'a> {
    pub provider: &'a dyn Provider,
    pub working_dir: &'a Path,
    pub session: Option<&'a Session>,
    /// Names this run's phase outputs in the session, e.g. `subtask-1`.
    pub label: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TacticOutcome {
    pub summary: String,
}

/// Runs `subtask` through `phases`. Validation can send it back to an earlier
/// phase; it only finishes when the terminal phase delivers.
pub async fn run_tactic(
    ctx: &TacticContext<'_>,
    phases: &PhaseList,
    subtask: &Task,
) -> Result<TacticOutcome, TacticError> {
    phases.validate().map_err(TacticError::Invalid)?;
    let names: Vec<String> = phases.phases.iter().map(|p| p.name.clone()).collect();
    let (mut index, mut retries) = (0, 0);
    let mut notes = Vec::new();
    loop {
        let phase = &phases.phases[index];
        let terminal = index + 1 == phases.phases.len();
        let state = InteractionState::new(names.clone(), index, terminal);
        let view = view_of(subtask, phase, (index, names.len()), terminal, &notes);
        let turn = PhaseTurn {
            provider: ctx.provider,
            working_dir: ctx.working_dir,
            allow_write: phase.settings.can_write_working_directory,
        };
        let out = turn.run(state, &wrap_prompt(&view)).await?;
        save_output(ctx, phase, index, &out.reply);
        notes.push(format!("{}: {}", phase.name, brief(&out.reply)));
        match out.signal {
            None => return Err(TacticError::Stalled(phase.name.clone())),
            Some(Signal::Advance) => {
                tracing::info!(
                    label = ctx.label,
                    from = phase.name.as_str(),
                    to = names[index + 1].as_str(),
                    "tactic phase transition"
                );
                index += 1;
            }
            Some(Signal::Complete { summary }) => {
                tracing::info!(
                    label = ctx.label,
                    summary = summary.as_str(),
                    "tactic complete"
                );
                return Ok(TacticOutcome { summary });
            }
            Some(Signal::Retry {
                phase: target,
                reason,
            }) => {
                retries += 1;
                if retries > MAX_RETRIES {
                    return Err(TacticError::TooManyRetries);
                }
                notes.push(format!(
                    "went back to {target} from {}: {reason}",
                    phase.name
                ));
                index = names.iter().position(|n| *n == target).unwrap_or(0);
                tracing::info!(
                    label = ctx.label,
                    from = phase.name.as_str(),
                    to = target.as_str(),
                    retries,
                    "tactic phase transition (retry)"
                );
            }
        }
    }
}

fn view_of<'a>(
    subtask: &'a Task,
    phase: &'a PhaseConfig,
    (index, total): (usize, usize),
    terminal: bool,
    notes: &'a [String],
) -> PhaseView<'a> {
    PhaseView {
        task_type: &subtask.task_type,
        task_description: &subtask.description,
        phase: &phase.name,
        number: index + 1,
        total,
        terminal,
        can_create_subtasks: false,
        notes,
        prompt: &phase.prompt,
    }
}

/// Keeps what each phase said in the session folder; failures are not fatal.
fn save_output(ctx: &TacticContext<'_>, phase: &PhaseConfig, index: usize, reply: &str) {
    if let Some(session) = ctx.session {
        let name = format!("{}-{}.txt", ctx.label, index + 1);
        let _ = session.write_phase_output(&phase.name.to_lowercase(), &name, reply);
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use serde_json::json;

    use super::*;
    use crate::feature::script_provider::{ScriptProvider, call};
    use crate::feature::strategy::PhaseSettings;

    fn phases() -> PhaseList {
        let phase = |name: &str, write: bool| PhaseConfig {
            name: name.into(),
            prompt: format!("{name} prompt"),
            settings: PhaseSettings {
                can_write_working_directory: write,
                terminal: name == "Deliver",
            },
        };
        PhaseList {
            phases: vec![
                phase("Plan", false),
                phase("Execute", true),
                phase("Validate", false),
                phase("Deliver", false),
            ],
        }
    }

    fn advance() -> Vec<crate::feature::script_provider::Call> {
        vec![call("advance_phase", json!({}))]
    }

    fn finish(summary: &str) -> Vec<crate::feature::script_provider::Call> {
        vec![call("complete_task", json!({ "summary": summary }))]
    }

    async fn run(provider: &ScriptProvider) -> Result<TacticOutcome, TacticError> {
        let ctx = TacticContext {
            provider,
            working_dir: Path::new("."),
            session: None,
            label: "subtask-1",
        };
        run_tactic(&ctx, &phases(), &Task::new("code", "write it")).await
    }

    #[tokio::test]
    async fn a_subtask_runs_every_phase_in_order() {
        let provider = ScriptProvider::new(vec![advance(), advance(), advance(), finish("done")]);
        let outcome = run(&provider).await.unwrap();
        assert_eq!(outcome.summary, "done");
        let prompts = provider.prompts();
        assert_eq!(prompts.len(), 4);
        assert!(prompts[0].contains("PHASE 1 of 4: Plan"));
        assert!(prompts[3].contains("PHASE 4 of 4: Deliver"));
        assert!(prompts[1].contains("- Plan: scripted reply"));
    }

    #[tokio::test]
    async fn phase_transitions_are_logged() {
        let logs = crate::feature::log_capture::capture();
        let provider = ScriptProvider::new(vec![advance(), advance(), advance(), finish("done")]);
        run(&provider).await.unwrap();
        let text = logs.text();
        assert!(text.contains("tactic phase transition"));
        assert!(text.contains("from=\"Plan\"") && text.contains("to=\"Execute\""));
        assert!(text.contains("tactic complete"));
    }

    #[tokio::test]
    async fn failed_validation_loops_back_to_execute() {
        let retry = vec![call(
            "retry_phase",
            json!({ "phase": "Execute", "reason": "tests fail" }),
        )];
        let steps = vec![
            advance(),
            advance(),
            retry,
            advance(),
            advance(),
            finish("ok"),
        ];
        let provider = ScriptProvider::new(steps);
        assert_eq!(run(&provider).await.unwrap().summary, "ok");
        let prompts = provider.prompts();
        assert!(prompts[3].contains("PHASE 2 of 4: Execute"));
        assert!(prompts[3].contains("went back to Execute from Validate: tests fail"));
    }

    #[tokio::test]
    async fn delivering_early_is_refused() {
        let early = vec![call("complete_task", json!({ "summary": "x" }))];
        let provider = ScriptProvider::new(vec![early]);
        let err = run(&provider).await.unwrap_err();
        assert!(matches!(err, TacticError::Stalled(phase) if phase == "Plan"));
    }

    #[tokio::test]
    async fn endless_retries_are_cut_off() {
        let retry = || vec![call("retry_phase", json!({ "phase": "Plan" }))];
        let mut script = vec![advance(), advance()];
        for _ in 0..=MAX_RETRIES {
            script.extend([retry(), advance(), advance()]);
        }
        let err = run(&ScriptProvider::new(script)).await.unwrap_err();
        assert!(matches!(err, TacticError::TooManyRetries));
    }

    #[tokio::test]
    async fn only_the_execute_phase_gets_write_tools() {
        let provider = ScriptProvider::new(vec![advance(), advance(), advance(), finish("x")]);
        run(&provider).await.unwrap();
        let has_write = |names: &[String]| names.iter().any(|n| n == "write_file");
        let tools = provider.tool_names();
        let writes: Vec<bool> = tools.iter().map(|names| has_write(names)).collect();
        assert_eq!(writes, [false, true, false, false]);
    }

    #[tokio::test]
    async fn phase_output_is_saved_in_the_session() {
        let root = tempfile::TempDir::new().unwrap();
        let session = Session::create(root.path(), SystemTime::now(), Path::new("."), "m").unwrap();
        let provider = ScriptProvider::new(vec![advance(), advance(), advance(), finish("x")]);
        let ctx = TacticContext {
            provider: &provider,
            working_dir: Path::new("."),
            session: Some(&session),
            label: "subtask-1",
        };
        run_tactic(&ctx, &phases(), &Task::new("code", "go"))
            .await
            .unwrap();
        let saved = session.dir().join("phases/plan/subtask-1-1.txt");
        assert_eq!(std::fs::read_to_string(saved).unwrap(), "scripted reply");
    }

    #[tokio::test]
    async fn the_default_tactic_is_runnable() {
        let list: PhaseList =
            toml::from_str(include_str!("../strategy/defaults/tactic.toml")).unwrap();
        let provider = ScriptProvider::new(vec![advance(), advance(), advance(), finish("x")]);
        let ctx = TacticContext {
            provider: &provider,
            working_dir: Path::new("."),
            session: None,
            label: "s",
        };
        run_tactic(&ctx, &list, &Task::new("t", "d")).await.unwrap();
    }
}
