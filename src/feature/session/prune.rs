use std::fs;
use std::io;
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::feature::session::metadata::SessionMetadata;

/// Removes sessions under `root` created more than `retention` before `now`.
/// Directories without readable metadata are left alone. Returns how many were removed.
pub fn prune(root: &Path, now: SystemTime, retention: Duration) -> io::Result<usize> {
    if !root.is_dir() {
        return Ok(0);
    }
    let mut removed = 0;
    for entry in fs::read_dir(root)?.flatten() {
        let path = entry.path();
        let expired = SessionMetadata::read(&path)
            .is_ok_and(|metadata| metadata.is_older_than(now, retention));
        if expired && fs::remove_dir_all(&path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::UNIX_EPOCH;

    use tempfile::TempDir;

    use super::*;

    fn session(root: &Path, name: &str, created_at_secs: u64) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        SessionMetadata {
            id: name.into(),
            created_at_secs,
            working_dir: PathBuf::from("/w"),
            model: "m".into(),
        }
        .write(&dir)
        .unwrap();
    }

    #[test]
    fn removes_only_expired_sessions() {
        let root = TempDir::new().unwrap();
        session(root.path(), "old", 100);
        session(root.path(), "new", 950);
        let now = UNIX_EPOCH + Duration::from_secs(1000);
        let removed = prune(root.path(), now, Duration::from_secs(200)).unwrap();
        assert_eq!(removed, 1);
        assert!(!root.path().join("old").exists());
        assert!(root.path().join("new").exists());
    }

    #[test]
    fn keeps_directories_without_metadata() {
        let root = TempDir::new().unwrap();
        fs::create_dir(root.path().join("stray")).unwrap();
        let now = UNIX_EPOCH + Duration::from_secs(1_000_000);
        assert_eq!(prune(root.path(), now, Duration::ZERO).unwrap(), 0);
        assert!(root.path().join("stray").exists());
    }

    #[test]
    fn missing_root_is_fine() {
        let root = TempDir::new().unwrap();
        let missing = root.path().join("nope");
        assert_eq!(
            prune(&missing, SystemTime::now(), Duration::ZERO).unwrap(),
            0
        );
    }
}
