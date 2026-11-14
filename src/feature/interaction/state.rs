use std::sync::{Arc, Mutex};

use crate::feature::task::Task;

pub type SharedState = Arc<Mutex<InteractionState>>;

/// What the model asked the machine to do. The machine reads it after the turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    Advance,
    Retry { phase: String, reason: String },
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
        }
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
        self.signal = Some(Signal::Retry {
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
        self.signal = Some(Signal::Complete {
            summary: summary.trim().to_string(),
        });
        Ok(())
    }

    pub fn create_subtask(&mut self, task: Task) -> Result<(), String> {
        if !self.can_create_subtasks {
            return Err("subtasks cannot be created in this phase".to_string());
        }
        if task.description.trim().is_empty() {
            return Err("a subtask needs a description".to_string());
        }
        self.created.push(task);
        Ok(())
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

    fn ensure_unsignalled(&self) -> Result<(), String> {
        match &self.signal {
            Some(signal) => Err(format!("already requested {signal:?}; stop and wait")),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn state(current: usize, terminal: bool) -> InteractionState {
        let phases = ["Plan", "Execute", "Deliver"].map(String::from).to_vec();
        InteractionState::new(phases, current, terminal)
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
        assert!(s.advance().unwrap_err().contains("already requested"));
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
