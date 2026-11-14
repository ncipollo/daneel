//! Strategy and tactic config: ordered lists of phases, loaded from
//! `~/.daneel/strategies/` and `~/.daneel/strategies/tactics/`.

pub mod defaults;
pub mod load;
pub mod machine;
pub mod paths;

use serde::Deserialize;

/// One phase of a strategy or tactic. Both file kinds share this shape.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PhaseConfig {
    pub name: String,
    pub prompt: String,
    #[serde(default)]
    pub settings: PhaseSettings,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct PhaseSettings {
    /// When false, artifacts may only be written to the session folder.
    pub can_write_working_directory: bool,
    /// When true, this is the last phase.
    pub terminal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PhaseList {
    pub phases: Vec<PhaseConfig>,
}

impl PhaseList {
    /// A list needs at least one phase, and only the last may be terminal.
    pub fn validate(&self) -> Result<(), String> {
        let Some((last, rest)) = self.phases.split_last() else {
            return Err("no phases defined".to_string());
        };
        match rest.iter().find(|phase| phase.settings.terminal) {
            Some(phase) => Err(format!(
                "phase '{}' is marked terminal but is not the last phase",
                phase.name
            )),
            None if last.name.trim().is_empty() => Err("a phase has an empty name".to_string()),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phase(name: &str, terminal: bool) -> PhaseConfig {
        PhaseConfig {
            name: name.into(),
            prompt: "p".into(),
            settings: PhaseSettings {
                can_write_working_directory: false,
                terminal,
            },
        }
    }

    #[test]
    fn empty_list_is_invalid() {
        assert!(PhaseList { phases: vec![] }.validate().is_err());
    }

    #[test]
    fn terminal_must_be_last() {
        let list = PhaseList {
            phases: vec![phase("a", true), phase("b", false)],
        };
        assert!(list.validate().unwrap_err().contains("'a'"));
    }

    #[test]
    fn terminal_last_is_valid() {
        let list = PhaseList {
            phases: vec![phase("a", false), phase("b", true)],
        };
        assert!(list.validate().is_ok());
    }

    #[test]
    fn settings_default_to_restricted() {
        let list: PhaseList = toml::from_str("[[phases]]\nname = \"x\"\nprompt = \"y\"\n").unwrap();
        assert_eq!(list.phases[0].settings, PhaseSettings::default());
    }
}
