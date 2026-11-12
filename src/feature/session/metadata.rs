use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "metadata.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub id: String,
    pub created_at_secs: u64,
    pub working_dir: PathBuf,
    pub model: String,
}

impl SessionMetadata {
    pub fn created_at(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(self.created_at_secs)
    }

    pub fn is_older_than(&self, now: SystemTime, age: Duration) -> bool {
        now.duration_since(self.created_at())
            .is_ok_and(|elapsed| elapsed > age)
    }

    /// Writes via a temp file and rename so a crash never leaves a torn file.
    pub fn write(&self, dir: &Path) -> io::Result<()> {
        let tmp = dir.join(format!("{FILE_NAME}.tmp"));
        fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        fs::rename(tmp, dir.join(FILE_NAME))
    }

    pub fn read(dir: &Path) -> io::Result<Self> {
        Ok(serde_json::from_slice(&fs::read(dir.join(FILE_NAME))?)?)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    pub fn sample(created_at_secs: u64) -> SessionMetadata {
        SessionMetadata {
            id: "s".into(),
            created_at_secs,
            working_dir: PathBuf::from("/w"),
            model: "m".into(),
        }
    }

    #[test]
    fn write_then_read_round_trips() {
        let dir = TempDir::new().unwrap();
        sample(5).write(dir.path()).unwrap();
        assert_eq!(SessionMetadata::read(dir.path()).unwrap(), sample(5));
        assert!(!dir.path().join("metadata.json.tmp").exists());
    }

    #[test]
    fn age_check_uses_the_creation_timestamp() {
        let now = UNIX_EPOCH + Duration::from_secs(1000);
        let week = Duration::from_secs(100);
        assert!(sample(800).is_older_than(now, week));
        assert!(!sample(950).is_older_than(now, week));
    }
}
