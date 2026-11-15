use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::feature::task::Task;

pub type SharedState = Arc<Mutex<InteractionState>>;

/// What the model asked the machine to do. The machine reads it after the turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    Advance,
    Retry { phase: String, reason: String },
    Skip { phase: String, reason: String },
    Complete { summary: String },
}

/// The rules for one phase, plus what the model has requested so far.
/// Invalid requests come back as messages the model can act on.
#[derive(Debug)]
pub struct InteractionState {
    phases: Vec<String>,
    current: usize,
    terminal: bool,
    can_create_subtasks: bool,
    open_subtasks: usize,
    signal: Option<Signal>,
    created: Vec<Task>,
    required: Vec<PathBuf>,
}

impl InteractionState {
    pub fn new(phases: Vec<String>, current: usize, terminal: bool) -> Self {
        Self {
            phases,
            current,
            terminal,
            can_create_subtasks: false,
            open_subtasks: 0,
            signal: None,
            created: Vec::new(),
            required: Vec::new(),
        }
    }

    /// Files that must exist before the phase may advance.
    pub fn with_required(mut self, required: Vec<PathBuf>) -> Self {
        self.required = required;
        self
    }

    pub fn with_subtasks(mut self, can_create: bool, open: usize) -> Self {
        self.can_create_subtasks = can_create;
        self.open_subtasks = open;
        self
    }

    pub fn shared(self) -> SharedState {
        Arc::new(Mutex::new(self))
    }

    pub fn advance(&mut self) -> Result<(), String> {
        self.ensure_unsignalled()?;
        if self.terminal {
            return Err("this is the last phase; call complete_task instead".to_string());
        }
        self.ensure_no_open_subtasks("advance")?;
        ensure_deliverables(&self.required)?;
        tracing::info!(phase = self.phase_name(), "advance requested");
        self.signal = Some(Signal::Advance);
        Ok(())
    }

    pub fn retry(&mut self, phase: &str, reason: &str) -> Result<(), String> {
        self.ensure_unsignalled()?;
        let earlier = &self.phases[..self.current];
        let found = earlier.iter().find(|name| name.eq_ignore_ascii_case(phase));
        let Some(name) = found else {
            return Err(format!(
                "cannot go back to '{phase}'; earlier phases are: {}",
                earlier.join(", ")
            ));
        };
        tracing::info!(
            phase = self.phase_name(),
            target = name.as_str(),
            reason,
            "retry requested"
        );
        self.signal = Some(Signal::Retry {
            phase: name.clone(),
            reason: reason.trim().to_string(),
        });
        Ok(())
    }

    pub fn skip(&mut self, phase: &str, reason: &str) -> Result<(), String> {
        self.ensure_unsignalled()?;
        if self.terminal {
            return Err("this is the last phase; call complete_task instead".to_string());
        }
        self.ensure_no_open_subtasks("skip")?;
        let later = &self.phases[self.current + 1..];
        let found = later.iter().find(|name| name.eq_ignore_ascii_case(phase));
        let Some(name) = found else {
            return Err(format!(
                "cannot skip to '{phase}'; later phases are: {}",
                later.join(", ")
            ));
        };
        tracing::info!(
            phase = self.phase_name(),
            target = name.as_str(),
            reason,
            "skip requested"
        );
        self.signal = Some(Signal::Skip {
            phase: name.clone(),
            reason: reason.trim().to_string(),
        });
        Ok(())
    }

    pub fn complete(&mut self, summary: &str) -> Result<(), String> {
        self.ensure_unsignalled()?;
        if !self.terminal {
            return Err("this is not the last phase; call advance_phase instead".to_string());
        }
        self.ensure_no_open_subtasks("complete")?;
        tracing::info!(phase = self.phase_name(), summary, "complete requested");
        self.signal = Some(Signal::Complete {
            summary: summary.trim().to_string(),
        });
        Ok(())
    }

    pub fn create_subtask(&mut self, task: Task) -> Result<(), String> {
        self.ensure_unsignalled()?;
        if !self.can_create_subtasks {
            return Err("subtasks cannot be created in this phase".to_string());
        }
        if task.description.trim().is_empty() {
            return Err("a subtask needs a description".to_string());
        }
        tracing::info!(
            phase = self.phase_name(),
            task_type = task.task_type.as_str(),
            description = task.description.as_str(),
            "subtask created"
        );
        self.created.push(task);
        Ok(())
    }

    pub fn has_signal(&self) -> bool {
        self.signal.is_some()
    }

    /// True once the model has signalled or created a subtask.
    pub fn has_acted(&self) -> bool {
        self.signal.is_some() || !self.created.is_empty()
    }

    pub fn take_signal(&mut self) -> Option<Signal> {
        self.signal.take()
    }

    pub fn take_created(&mut self) -> Vec<Task> {
        std::mem::take(&mut self.created)
    }

    fn phase_name(&self) -> &str {
        self.phases.get(self.current).map_or("?", String::as_str)
    }

    fn ensure_unsignalled(&self) -> Result<(), String> {
        match &self.signal {
            Some(_) => Err(
                "a phase change is already pending. Do not call any more tools; just stop now."
                    .to_string(),
            ),
            None => Ok(()),
        }
    }

    fn ensure_no_open_subtasks(&self, action: &str) -> Result<(), String> {
        match self.open_subtasks + self.created.len() {
            0 => Ok(()),
            n => Err(format!("cannot {action}: {n} subtasks are still open")),
        }
    }
}

fn ensure_deliverables(required: &[PathBuf]) -> Result<(), String> {
    let missing: Vec<String> = required
        .iter()
        .filter(|path| !path.exists())
        .filter_map(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .collect();
    match missing.is_empty() {
        true => Ok(()),
        false => Err(format!(
            "cannot advance yet: save {} with write_artifact first",
            missing.join(", ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(current: usize, terminal: bool) -> InteractionState {
        let phases = ["Plan", "Execute", "Deliver"].map(String::from).to_vec();
        InteractionState::new(phases, current, terminal)
    }

    #[test]
    fn interactions_are_logged() {
        let logs = crate::feature::log_capture::capture();
        let mut s = state(1, false).with_subtasks(true, 0);
        s.create_subtask(Task::new("query", "look around")).unwrap();
        s.take_created();
        s.retry("plan", "bad plan").unwrap();
        let text = logs.text();
        assert!(text.contains("subtask created") && text.contains("look around"));
        assert!(text.contains("retry requested") && text.contains("bad plan"));
        assert!(text.contains("phase=\"Execute\""));
    }

    #[test]
    fn advance_records_a_signal() {
        let mut s = state(0, false);
        s.advance().unwrap();
        assert_eq!(s.take_signal(), Some(Signal::Advance));
        assert_eq!(s.take_signal(), None);
    }

    #[test]
    fn advance_is_refused_in_the_last_phase_and_with_open_subtasks() {
        assert!(
            state(2, true)
                .advance()
                .unwrap_err()
                .contains("complete_task")
        );
        let mut s = state(0, false).with_subtasks(true, 2);
        assert!(s.advance().unwrap_err().contains("2 subtasks"));
    }

    #[test]
    fn a_second_request_is_refused() {
        let mut s = state(0, false);
        s.advance().unwrap();
        assert!(s.advance().unwrap_err().contains("already pending"));
    }

    #[test]
    fn subtasks_are_refused_once_a_phase_change_is_pending() {
        let mut s = state(0, false).with_subtasks(true, 0);
        s.advance().unwrap();
        let err = s.create_subtask(Task::new("general", "late")).unwrap_err();
        assert!(err.contains("already pending"));
        assert!(s.take_created().is_empty());
    }

    #[test]
    fn retry_targets_earlier_phases_only() {
        let mut s = state(2, true);
        s.retry("execute", "tests failed").unwrap();
        let expected = Signal::Retry {
            phase: "Execute".into(),
            reason: "tests failed".into(),
        };
        assert_eq!(s.take_signal(), Some(expected));
        let err = state(1, false).retry("Deliver", "").unwrap_err();
        assert!(err.contains("earlier phases are: Plan"));
    }

    #[test]
    fn skip_targets_later_phases_only() {
        let mut s = state(0, false);
        s.skip("deliver", "just a greeting").unwrap();
        let expected = Signal::Skip {
            phase: "Deliver".into(),
            reason: "just a greeting".into(),
        };
        assert_eq!(s.take_signal(), Some(expected));
        let err = state(1, false).skip("Plan", "").unwrap_err();
        assert!(err.contains("later phases are: Deliver"));
        assert!(
            state(2, true)
                .skip("Plan", "")
                .unwrap_err()
                .contains("complete_task")
        );
        assert!(
            state(0, false)
                .with_subtasks(true, 1)
                .skip("Deliver", "")
                .is_err()
        );
    }

    #[test]
    fn advance_waits_for_required_deliverables() {
        let dir = tempfile::TempDir::new().unwrap();
        let plan = dir.path().join("plan.md");
        let mut s = state(0, false).with_required(vec![plan.clone()]);
        let err = s.advance().unwrap_err();
        assert!(err.contains("plan.md") && err.contains("write_artifact"));
        std::fs::write(&plan, "1. go").unwrap();
        s.advance().unwrap();
        assert_eq!(s.take_signal(), Some(Signal::Advance));
    }

    #[test]
    fn complete_needs_the_last_phase() {
        assert!(
            state(0, false)
                .complete("x")
                .unwrap_err()
                .contains("advance_phase")
        );
        let mut s = state(2, true);
        s.complete("  done ").unwrap();
        assert_eq!(
            s.take_signal(),
            Some(Signal::Complete {
                summary: "done".into()
            })
        );
    }

    #[test]
    fn subtasks_are_only_created_where_allowed() {
        let mut s = state(0, false);
        assert!(s.create_subtask(Task::new("t", "d")).is_err());
        let mut s = state(0, false).with_subtasks(true, 0);
        assert!(s.create_subtask(Task::new("t", " ")).is_err());
        s.create_subtask(Task::new("t", "d")).unwrap();
        assert_eq!(s.take_created(), vec![Task::new("t", "d")]);
        assert!(s.take_created().is_empty());
    }
}
