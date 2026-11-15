//! The `strategies` topic: strategy and tactic phase files.

const EXAMPLE: &str = r#"[[phases]]
name = "Plan"
prompt = """
Write a short numbered plan."""
settings = { can_write_working_directory = false, terminal = false }

[[phases]]
name = "Deliver"
prompt = """
Summarize what you did."""
settings = { can_write_working_directory = false, terminal = true }"#;

pub fn render() -> String {
    format!(
        "STRATEGY AND TACTIC\n\
         The strategy is the ordered list of phases every task goes through.\n\
         The tactic is the list used for each subtask a phase creates.\n\
         \n\
         \x20 ~/.daneel/strategies/default.toml\n\
         \x20 ~/.daneel/strategies/tactics/default.toml\n\
         \n\
         daneel writes the defaults (Triage, Discover, Plan, Execute, Validate,\n\
         Deliver; tactic Plan, Execute, Validate, Deliver) at startup only if\n\
         the files are missing, so edit them freely. daneel never overwrites\n\
         a file at startup. To reset them to the built-in defaults, run\n\
         daneel --regenerate-strategy (asks first; discards your edits).\n\
         \n\
         PHASE FORMAT\n\
         Each [[phases]] table has:\n\
         \x20 name      shown in progress output\n\
         \x20 prompt    the instructions the model gets for this phase\n\
         \x20 settings  can_write_working_directory (default false): when\n\
         \x20           false the phase may only read; terminal (default\n\
         \x20           false): marks the last phase; produces (default empty):\n\
         \x20           artifact file names the phase must save with\n\
         \x20           write_artifact before advance_phase is accepted\n\
         \n\
         ARTIFACTS\n\
         write_artifact saves markdown to <session>/artifacts/<phase>/<name> and\n\
         works in every phase. Each prompt lists this phase's folder and the files\n\
         earlier phases saved, which read_file can open.\n\
         \n\
         RULES\n\
         At least one phase. Only the last may be terminal. The model moves\n\
         on by calling advance_phase, jumps ahead to a later named phase with\n\
         skip_to_phase (Triage uses it to go straight to Deliver for a task\n\
         that needs no work), goes back with retry_phase, and ends\n\
         the last phase with complete_task. A broken file stops daneel with\n\
         a message naming it.\n\
         \n\
         EXAMPLE\n\
         {EXAMPLE}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::strategy::PhaseList;

    #[test]
    fn the_example_is_a_valid_phase_list() {
        let list: PhaseList = toml::from_str(EXAMPLE).unwrap();
        assert!(list.validate().is_ok());
    }
}
