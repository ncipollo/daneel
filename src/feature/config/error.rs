use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not determine home directory")]
    NoHomeDir,
    #[error("could not read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not parse {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("{path}: environment variable '{var}' not set")]
    UnsetEnvVar { var: String, path: PathBuf },
    #[error("both {json} and {toml} exist; remove one")]
    AmbiguousFiles { json: PathBuf, toml: PathBuf },
}
