//! Debug logging: complete traces go to `~/.daneel/logs/daneel.log`, never to
//! stdout (that belongs to the TUI and `-p` output). `RUST_LOG` filters them.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tracing_subscriber::EnvFilter;

use crate::feature::config;

const LOG_DIR: &str = "logs";
const LOG_FILE: &str = "daneel.log";
/// Daneel's own events in full; dependencies only when they matter.
const DEFAULT_FILTER: &str = "info,daneel=debug";

pub fn log_path(config_dir: &Path) -> PathBuf {
    config_dir.join(LOG_DIR).join(LOG_FILE)
}

/// `init_in` for the real `~/.daneel` directory.
pub fn init_default() -> Result<(), String> {
    let dir = config::default_dir().map_err(|e| e.to_string())?;
    init_in(&dir).map_err(|e| format!("could not start logging: {e}"))
}

/// Installs the global subscriber, appending to the log file under `config_dir`.
pub fn init_in(config_dir: &Path) -> io::Result<()> {
    let path = log_path(config_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    tracing::subscriber::set_global_default(subscriber(file))
        .map_err(|e| io::Error::other(e.to_string()))
}

fn subscriber(file: File) -> impl tracing::Subscriber + Send + Sync {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(Mutex::new(file))
        .with_ansi(false)
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_path_lives_under_logs() {
        let path = log_path(Path::new("/home/u/.daneel"));
        assert_eq!(path, Path::new("/home/u/.daneel/logs/daneel.log"));
    }

    #[test]
    fn the_subscriber_writes_events_to_its_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("t.log");
        let file = File::create(&path).unwrap();
        tracing::subscriber::with_default(subscriber(file), || {
            tracing::info!(content = "hello", "model message");
        });
        let text = fs::read_to_string(path).unwrap();
        assert!(text.contains("model message") && text.contains("hello"));
        assert!(!text.contains('\u{1b}'), "no ansi colour codes in files");
    }
}
