use std::fs;
use std::io;
use std::path::Path;

use crate::feature::config;
use crate::feature::strategy::paths;

const DEFAULT_STRATEGY: &str = include_str!("defaults/strategy.toml");
const DEFAULT_TACTIC: &str = include_str!("defaults/tactic.toml");

/// `ensure_defaults` for the real `~/.daneel` directory.
pub fn ensure_default_dir() -> Result<(), String> {
    let dir = config::default_dir().map_err(|e| e.to_string())?;
    ensure_defaults(&dir).map_err(|e| format!("could not write default strategies: {e}"))
}

/// Writes the default strategy and tactic files when they are missing.
/// Existing files are never touched, so user edits survive.
pub fn ensure_defaults(config_dir: &Path) -> io::Result<()> {
    let name = paths::DEFAULT_NAME;
    write_if_missing(&paths::strategy_file(config_dir, name), DEFAULT_STRATEGY)?;
    write_if_missing(&paths::tactic_file(config_dir, name), DEFAULT_TACTIC)
}

fn write_if_missing(path: &Path, contents: &str) -> io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::feature::strategy::load::{load_strategy, load_tactic};

    fn names(list: &crate::feature::strategy::PhaseList) -> Vec<&str> {
        list.phases.iter().map(|p| p.name.as_str()).collect()
    }

    #[test]
    fn writes_valid_defaults_when_missing() {
        let dir = TempDir::new().unwrap();
        ensure_defaults(dir.path()).unwrap();
        let strategy = load_strategy(dir.path(), "default").unwrap();
        let tactic = load_tactic(dir.path(), "default").unwrap();
        assert_eq!(
            names(&strategy),
            ["Discover", "Plan", "Execute", "Validate", "Deliver"]
        );
        assert_eq!(names(&tactic), ["Plan", "Execute", "Validate", "Deliver"]);
    }

    #[test]
    fn deliver_is_terminal_and_only_execute_writes() {
        let dir = TempDir::new().unwrap();
        ensure_defaults(dir.path()).unwrap();
        for list in [
            load_strategy(dir.path(), "default").unwrap(),
            load_tactic(dir.path(), "default").unwrap(),
        ] {
            for phase in &list.phases {
                assert_eq!(phase.settings.terminal, phase.name == "Deliver");
                assert_eq!(
                    phase.settings.can_write_working_directory,
                    phase.name == "Execute"
                );
            }
        }
    }

    #[test]
    fn existing_files_are_not_overwritten() {
        let dir = TempDir::new().unwrap();
        ensure_defaults(dir.path()).unwrap();
        let path = paths::strategy_file(dir.path(), "default");
        fs::write(&path, "# mine").unwrap();
        ensure_defaults(dir.path()).unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), "# mine");
    }
}
