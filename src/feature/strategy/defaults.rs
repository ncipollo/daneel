use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::feature::config;
use crate::feature::strategy::PhaseList;
use crate::feature::strategy::paths;

const DEFAULT_STRATEGY: &str = include_str!("defaults/strategy.toml");
const DEFAULT_TACTIC: &str = include_str!("defaults/tactic.toml");

/// The embedded default strategy, used when no file has been written yet.
pub fn builtin_strategy() -> PhaseList {
    toml::from_str(DEFAULT_STRATEGY).expect("the embedded strategy is valid")
}

/// The embedded default tactic.
pub fn builtin_tactic() -> PhaseList {
    toml::from_str(DEFAULT_TACTIC).expect("the embedded tactic is valid")
}

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

/// `regenerate` for the real `~/.daneel` directory.
pub fn regenerate_default_dir() -> Result<Vec<PathBuf>, String> {
    let dir = config::default_dir().map_err(|e| e.to_string())?;
    regenerate(&dir).map_err(|e| format!("could not write default strategies: {e}"))
}

/// Overwrites the default strategy and tactic files with the built-in ones,
/// returning the paths written.
pub fn regenerate(config_dir: &Path) -> io::Result<Vec<PathBuf>> {
    let name = paths::DEFAULT_NAME;
    let files = [
        (paths::strategy_file(config_dir, name), DEFAULT_STRATEGY),
        (paths::tactic_file(config_dir, name), DEFAULT_TACTIC),
    ];
    for (path, contents) in &files {
        write_file(path, contents)?;
    }
    Ok(files.into_iter().map(|(path, _)| path).collect())
}

fn write_if_missing(path: &Path, contents: &str) -> io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    write_file(path, contents)
}

fn write_file(path: &Path, contents: &str) -> io::Result<()> {
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
    fn the_embedded_defaults_parse_and_validate() {
        assert!(builtin_strategy().validate().is_ok());
        assert!(builtin_tactic().validate().is_ok());
        assert_eq!(builtin_strategy().phases.len(), 6);
    }

    #[test]
    fn writes_valid_defaults_when_missing() {
        let dir = TempDir::new().unwrap();
        ensure_defaults(dir.path()).unwrap();
        let strategy = load_strategy(dir.path(), "default").unwrap();
        let tactic = load_tactic(dir.path(), "default").unwrap();
        assert_eq!(
            names(&strategy),
            [
                "Triage", "Discover", "Plan", "Execute", "Validate", "Deliver"
            ]
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
    fn regenerate_replaces_edited_files() {
        let dir = TempDir::new().unwrap();
        ensure_defaults(dir.path()).unwrap();
        let path = paths::strategy_file(dir.path(), "default");
        fs::write(&path, "# mine").unwrap();
        let written = regenerate(dir.path()).unwrap();
        assert_eq!(written.len(), 2);
        assert!(load_strategy(dir.path(), "default").is_ok());
    }

    #[test]
    fn regenerate_creates_missing_folders() {
        let dir = TempDir::new().unwrap();
        regenerate(dir.path()).unwrap();
        assert!(load_tactic(dir.path(), "default").is_ok());
    }

    #[test]
    fn discover_plan_and_validate_declare_their_deliverables() {
        let produced: Vec<_> = builtin_strategy()
            .phases
            .iter()
            .map(|p| (p.name.clone(), p.settings.produces.clone()))
            .filter(|(_, produces)| !produces.is_empty())
            .collect();
        let expect = |n: &str, f: &str| (n.to_string(), vec![f.to_string()]);
        assert_eq!(
            produced,
            [
                expect("Discover", "findings.md"),
                expect("Plan", "plan.md"),
                expect("Validate", "validation.md")
            ]
        );
    }

    #[test]
    fn triage_names_the_skip_argument() {
        let strategy = builtin_strategy();
        let prompt = &strategy.phases[0].prompt;
        assert!(prompt.contains(r#"skip_to_phase {"phase": "Deliver""#));
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
