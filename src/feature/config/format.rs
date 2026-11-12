use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::feature::config::error::ConfigError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Toml,
}

/// Finds `<stem>.json` or `<stem>.toml` in `dir`. Having both is ambiguous.
pub fn find(dir: &Path, stem: &str) -> Result<Option<(PathBuf, Format)>, ConfigError> {
    let json = dir.join(format!("{stem}.json"));
    let toml = dir.join(format!("{stem}.toml"));
    match (json.is_file(), toml.is_file()) {
        (true, true) => Err(ConfigError::AmbiguousFiles { json, toml }),
        (true, false) => Ok(Some((json, Format::Json))),
        (false, true) => Ok(Some((toml, Format::Toml))),
        (false, false) => Ok(None),
    }
}

pub fn parse(path: &Path, format: Format) -> Result<Value, ConfigError> {
    let text = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_text(&text, format).map_err(|message| ConfigError::Parse {
        path: path.to_path_buf(),
        message,
    })
}

fn parse_text(text: &str, format: Format) -> Result<Value, String> {
    match format {
        Format::Json => serde_json::from_str(text).map_err(|e| e.to_string()),
        Format::Toml => {
            let value: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
            serde_json::to_value(value).map_err(|e| e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn find_returns_none_when_neither_exists() {
        let dir = TempDir::new().unwrap();
        assert!(find(dir.path(), "settings").unwrap().is_none());
    }

    #[test]
    fn find_picks_format_by_extension() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("settings.toml"), "").unwrap();
        let (path, format) = find(dir.path(), "settings").unwrap().unwrap();
        assert_eq!(format, Format::Toml);
        assert_eq!(path, dir.path().join("settings.toml"));
    }

    #[test]
    fn find_rejects_both_formats() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("settings.json"), "{}").unwrap();
        fs::write(dir.path().join("settings.toml"), "").unwrap();
        let result = find(dir.path(), "settings");
        assert!(matches!(result, Err(ConfigError::AmbiguousFiles { .. })));
    }

    #[test]
    fn json_and_toml_parse_to_the_same_value() {
        let json = parse_text(r#"{"theme": "dark"}"#, Format::Json).unwrap();
        let toml = parse_text(r#"theme = "dark""#, Format::Toml).unwrap();
        assert_eq!(json, toml);
    }

    #[test]
    fn malformed_text_is_an_error() {
        assert!(parse_text("{ not json", Format::Json).is_err());
    }
}
