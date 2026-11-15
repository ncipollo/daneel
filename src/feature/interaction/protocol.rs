use std::path::{Path, PathBuf};

/// Where this phase saves deliverables and what earlier phases left behind.
pub struct ArtifactInfo {
    pub dir: PathBuf,
    pub produces: Vec<String>,
    pub earlier: Vec<PathBuf>,
}

/// Everything the wrapper needs to tell the model where it is.
pub struct PhaseView<'a> {
    pub task_type: &'a str,
    pub task_description: &'a str,
    pub phase: &'a str,
    pub working_dir: &'a Path,
    pub number: usize,
    pub total: usize,
    pub terminal: bool,
    pub can_create_subtasks: bool,
    pub notes: &'a [String],
    pub prompt: &'a str,
    pub artifacts: Option<&'a ArtifactInfo>,
}

/// Wraps a phase's configured prompt with a standard header and footer. The
/// footer repeats the protocol because small models forget it.
pub fn wrap_prompt(view: &PhaseView) -> String {
    let mut text = format!(
        "TASK ({}): {}\nPHASE {} of {}: {}\nWORKING DIRECTORY (the project lives here): {}\n",
        view.task_type,
        view.task_description,
        view.number,
        view.total,
        view.phase,
        view.working_dir.display()
    );
    if !view.notes.is_empty() {
        text.push_str("\nPROGRESS SO FAR:\n");
        view.notes
            .iter()
            .for_each(|note| text.push_str(&format!("- {note}\n")));
    }
    text.push_str(&format!("\n{}\n\n", view.prompt.trim()));
    if let Some(info) = view.artifacts {
        text.push_str(&artifacts_block(info));
    }
    text.push_str(&footer(view));
    text
}

fn artifacts_block(info: &ArtifactInfo) -> String {
    let mut text = format!(
        "ARTIFACTS: save deliverables with write_artifact (name, content). This phase's files go in {}.\n",
        info.dir.display()
    );
    if !info.produces.is_empty() {
        let line = format!(
            "You must save {} before advance_phase.\n",
            info.produces.join(", ")
        );
        text.push_str(&line);
    }
    if !info.earlier.is_empty() {
        text.push_str("Earlier phases saved these files; read them with read_file:\n");
        info.earlier
            .iter()
            .for_each(|p| text.push_str(&format!("- {}\n", p.display())));
    }
    text.push('\n');
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
    if !view.terminal {
        lines.push(
            "To skip phases you do not need, call skip_to_phase with the name of a later phase, for example {\"phase\": \"Deliver\"}."
                .to_string(),
        );
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
            working_dir: Path::new("/work"),
            number,
            total: 3,
            terminal,
            can_create_subtasks: !terminal,
            notes,
            prompt: "  Think first.  ",
            artifacts: None,
        }
    }

    #[test]
    fn header_names_the_task_and_phase() {
        let text = wrap_prompt(&view(1, false, &[]));
        assert!(text.starts_with("TASK (query): find the bug\nPHASE 1 of 3: Plan"));
        assert!(text.contains("\nThink first.\n"));
        assert!(text.contains("WORKING DIRECTORY (the project lives here): /work\n"));
    }

    #[test]
    fn footer_matches_the_phase() {
        let middle = wrap_prompt(&view(2, false, &[]));
        assert!(middle.contains("advance_phase") && middle.contains("create_subtask"));
        assert!(middle.contains("retry_phase"));
        assert!(middle.contains("skip_to_phase"));
        let first = wrap_prompt(&view(1, false, &[]));
        assert!(!first.contains("retry_phase"));
        let last = wrap_prompt(&view(3, true, &[]));
        assert!(last.contains("complete_task") && !last.contains("create_subtask"));
        assert!(!last.contains("skip_to_phase"));
    }

    #[test]
    fn artifacts_block_names_folder_requirements_and_earlier_files() {
        let info = ArtifactInfo {
            dir: PathBuf::from("/s/artifacts/plan"),
            produces: vec!["plan.md".into()],
            earlier: vec![PathBuf::from("/s/artifacts/discover/findings.md")],
        };
        let mut v = view(2, false, &[]);
        v.artifacts = Some(&info);
        let text = wrap_prompt(&v);
        assert!(text.contains("go in /s/artifacts/plan"));
        assert!(text.contains("You must save plan.md before advance_phase"));
        assert!(text.contains("- /s/artifacts/discover/findings.md"));
        assert!(!wrap_prompt(&view(2, false, &[])).contains("ARTIFACTS"));
    }

    #[test]
    fn progress_notes_are_listed() {
        let notes = vec!["subtask A done".to_string()];
        assert!(wrap_prompt(&view(2, false, &notes)).contains("- subtask A done"));
    }
}
