/// Everything the wrapper needs to tell the model where it is.
pub struct PhaseView<'a> {
    pub task_type: &'a str,
    pub task_description: &'a str,
    pub phase: &'a str,
    pub number: usize,
    pub total: usize,
    pub terminal: bool,
    pub can_create_subtasks: bool,
    pub notes: &'a [String],
    pub prompt: &'a str,
}

/// Wraps a phase's configured prompt with a standard header and footer. The
/// footer repeats the protocol because small models forget it.
pub fn wrap_prompt(view: &PhaseView) -> String {
    let mut text = format!(
        "TASK ({}): {}\nPHASE {} of {}: {}\n",
        view.task_type, view.task_description, view.number, view.total, view.phase
    );
    if !view.notes.is_empty() {
        text.push_str("\nPROGRESS SO FAR:\n");
        view.notes
            .iter()
            .for_each(|note| text.push_str(&format!("- {note}\n")));
    }
    text.push_str(&format!("\n{}\n\n", view.prompt.trim()));
    text.push_str(&footer(view));
    text
}

fn footer(view: &PhaseView) -> String {
    let finish = if view.terminal {
        "When you have the final answer, call complete_task with the result."
    } else {
        "When this phase is done, call advance_phase."
    };
    let mut lines = vec![
        "HOW TO RESPOND: use tools to act. Move on only by calling a tool, never by writing text."
            .to_string(),
        finish.to_string(),
    ];
    if view.can_create_subtasks {
        lines.push("To split off a smaller piece of work, call create_subtask.".to_string());
    }
    if view.number > 1 {
        lines.push(
            "If an earlier phase must be redone, call retry_phase with its name.".to_string(),
        );
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view<'a>(number: usize, terminal: bool, notes: &'a [String]) -> PhaseView<'a> {
        PhaseView {
            task_type: "query",
            task_description: "find the bug",
            phase: "Plan",
            number,
            total: 3,
            terminal,
            can_create_subtasks: !terminal,
            notes,
            prompt: "  Think first.  ",
        }
    }

    #[test]
    fn header_names_the_task_and_phase() {
        let text = wrap_prompt(&view(1, false, &[]));
        assert!(text.starts_with("TASK (query): find the bug\nPHASE 1 of 3: Plan"));
        assert!(text.contains("\nThink first.\n"));
    }

    #[test]
    fn footer_matches_the_phase() {
        let middle = wrap_prompt(&view(2, false, &[]));
        assert!(middle.contains("advance_phase") && middle.contains("create_subtask"));
        assert!(middle.contains("retry_phase"));
        let first = wrap_prompt(&view(1, false, &[]));
        assert!(!first.contains("retry_phase"));
        let last = wrap_prompt(&view(3, true, &[]));
        assert!(last.contains("complete_task") && !last.contains("create_subtask"));
    }

    #[test]
    fn progress_notes_are_listed() {
        let notes = vec!["subtask A done".to_string()];
        assert!(wrap_prompt(&view(2, false, &notes)).contains("- subtask A done"));
    }
}
