//! Loads the `~/.daneel` config directory: `settings` and `models`, each as JSON or TOML,
//! with `$ENV_VAR` resolution for any string value.

pub mod env;
pub mod error;
pub mod format;
pub mod models;
pub mod settings;

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

pub use error::ConfigError;
pub use models::{ModelEntry, ModelsConfig, ProviderConfig};
pub use settings::Settings;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Config {
    pub settings: Settings,
    pub models: ModelsConfig,
}

pub fn default_dir() -> Result<PathBuf, ConfigError> {
    dirs::home_dir()
        .map(|home| home.join(".daneel"))
        .ok_or(ConfigError::NoHomeDir)
}

pub fn load_default() -> Result<Config, ConfigError> {
    load(&default_dir()?)
}

/// Loads both config files from `dir`. A missing file falls back to defaults.
pub fn load(dir: &Path) -> Result<Config, ConfigError> {
    Ok(Config {
        settings: load_file(dir, "settings")?,
        models: load_file(dir, "models")?,
    })
}

fn load_file<T: DeserializeOwned + Default>(dir: &Path, stem: &str) -> Result<T, ConfigError> {
    let Some((path, format)) = format::find(dir, stem)? else {
        return Ok(T::default());
    };
    let raw = format::parse(&path, format)?;
    let lookup = |name: &str| std::env::var(name).ok();
    let resolved = env::resolve_tree(raw, &lookup).map_err(|var| ConfigError::UnsetEnvVar {
        var,
        path: path.clone(),
    })?;
    serde_json::from_value(resolved).map_err(|e| ConfigError::Parse {
        path,
        message: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(dir: &TempDir, name: &str, contents: &str) {
        fs::write(dir.path().join(name), contents).unwrap();
    }

    #[test]
    fn empty_dir_yields_defaults() {
        let dir = TempDir::new().unwrap();
        assert_eq!(load(dir.path()).unwrap(), Config::default());
    }

    #[test]
    fn missing_dir_yields_defaults() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("nope");
        assert_eq!(load(&missing).unwrap(), Config::default());
    }

    #[test]
    fn json_and_toml_load_equal_settings() {
        let json_dir = TempDir::new().unwrap();
        write(
            &json_dir,
            "settings.json",
            r#"{"theme":"dark","defaultModel":"qwen"}"#,
        );
        let toml_dir = TempDir::new().unwrap();
        write(
            &toml_dir,
            "settings.toml",
            "theme = \"dark\"\ndefaultModel = \"qwen\"\n",
        );
        assert_eq!(
            load(json_dir.path()).unwrap().settings,
            load(toml_dir.path()).unwrap().settings
        );
    }

    #[test]
    fn malformed_file_names_the_path() {
        let dir = TempDir::new().unwrap();
        write(&dir, "settings.json", "{ not json");
        let err = load(dir.path()).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
        assert!(err.to_string().contains("settings.json"));
    }

    #[test]
    fn unset_env_var_is_an_error() {
        let dir = TempDir::new().unwrap();
        write(
            &dir,
            "models.toml",
            "[providers.p]\napiKey = \"$DANEEL_TEST_DEFINITELY_UNSET_XYZ\"\n",
        );
        let err = load(dir.path()).unwrap_err();
        assert!(
            matches!(err, ConfigError::UnsetEnvVar { ref var, .. } if var == "DANEEL_TEST_DEFINITELY_UNSET_XYZ")
        );
    }

    #[test]
    fn env_var_resolves_in_models_toml() {
        let dir = TempDir::new().unwrap();
        write(&dir, "models.toml", "[providers.p]\nbaseUrl = \"$HOME\"\n");
        let expected = std::env::var("HOME").unwrap();
        let config = load(dir.path()).unwrap();
        assert_eq!(
            config.models.providers["p"].base_url.as_deref(),
            Some(expected.as_str())
        );
    }
}
