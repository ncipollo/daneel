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
         daneel writes the defaults (Discover, Plan, Execute, Validate,\n\
         Deliver; tactic Plan, Execute) at startup only if the files are\n\
         missing, so edit them freely. Delete one to get the default back.\n\
         \n\
         PHASE FORMAT\n\
         Each [[phases]] table has:\n\
         \x20 name      shown in progress output\n\
         \x20 prompt    the instructions the model gets for this phase\n\
         \x20 settings  can_write_working_directory (default false): when\n\
         \x20           false the phase may only read; terminal (default\n\
         \x20           false): marks the last phase\n\
         \n\
         RULES\n\
         At least one phase. Only the last may be terminal. The model moves\n\
         on by calling advance_phase, goes back with retry_phase, and ends\n\
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
