//! Tasks: the typed unit of work the strategy and tactic machines act on.

pub mod parse;
pub mod transform;

use serde::{Deserialize, Serialize};

pub const GENERIC_TYPE: &str = "general";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    /// An unbounded hint such as "query" or "code creation", not a closed set.
    pub task_type: String,
    /// What we are going to do.
    pub description: String,
}

impl Task {
    pub fn new(task_type: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            task_type: task_type.into(),
            description: description.into(),
        }
    }

    /// The fallback that wraps a raw user message unchanged.
    pub fn generic(message: &str) -> Self {
        Self::new(GENERIC_TYPE, message.trim())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_wraps_the_trimmed_message() {
        let task = Task::generic("  fix the bug \n");
        assert_eq!(task, Task::new("general", "fix the bug"));
    }

    #[test]
    fn serde_round_trips() {
        let task = Task::new("code creation", "write a parser");
        let json = serde_json::to_string(&task).unwrap();
        assert_eq!(
            json,
            r#"{"task_type":"code creation","description":"write a parser"}"#
        );
        assert_eq!(serde_json::from_str::<Task>(&json).unwrap(), task);
    }
}
