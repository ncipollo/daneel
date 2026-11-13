use std::path::Path;

use crate::feature::config::ConfigError;
use crate::feature::config::format::{self, Format};
use crate::feature::strategy::PhaseList;
use crate::feature::strategy::paths;

pub fn load_strategy(config_dir: &Path, name: &str) -> Result<PhaseList, ConfigError> {
    load_file(&paths::strategy_file(config_dir, name))
}

pub fn load_tactic(config_dir: &Path, name: &str) -> Result<PhaseList, ConfigError> {
    load_file(&paths::tactic_file(config_dir, name))
}

pub fn load_file(path: &Path) -> Result<PhaseList, ConfigError> {
    let parse_error = |message: String| ConfigError::Parse {
        path: path.to_path_buf(),
        message,
    };
    let value = format::parse(path, Format::Toml)?;
    let list: PhaseList = serde_json::from_value(value).map_err(|e| parse_error(e.to_string()))?;
    list.validate().map_err(parse_error)?;
    Ok(list)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn write(dir: &TempDir, text: &str) -> std::path::PathBuf {
        let path = dir.path().join("s.toml");
        fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn loads_a_valid_file() {
        let dir = TempDir::new().unwrap();
        let path = write(
            &dir,
            "[[phases]]\nname = \"A\"\nprompt = \"p\"\nsettings = { terminal = true }\n",
        );
        let list = load_file(&path).unwrap();
        assert!(list.phases[0].settings.terminal);
    }

    #[test]
    fn invalid_files_name_the_path() {
        let dir = TempDir::new().unwrap();
        for text in [
            "phases = []",
            "not toml [",
            "[[phases]]\nname = \"A\"\n",
            "[[phases]]\nname = \"A\"\nprompt = \"p\"\nsettings = { terminal = true }\n[[phases]]\nname = \"B\"\nprompt = \"p\"\n",
        ] {
            let err = load_file(&write(&dir, text)).unwrap_err();
            assert!(err.to_string().contains("s.toml"), "{text}: {err}");
        }
    }

    #[test]
    fn missing_file_is_an_io_error() {
        let dir = TempDir::new().unwrap();
        let err = load_strategy(dir.path(), "nope").unwrap_err();
        assert!(matches!(err, ConfigError::Io { .. }));
    }
}
