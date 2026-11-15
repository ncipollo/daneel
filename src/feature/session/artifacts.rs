//! Deliverables the model writes for later phases, kept apart from the raw phase
//! replies: `artifacts/<phase>/<name>` inside the session folder.

use std::fs;
use std::io;
use std::path::PathBuf;

use crate::feature::session::Session;

const ARTIFACTS_DIR: &str = "artifacts";

impl Session {
    /// The folder `phase` saves its deliverables in (lowercased phase name).
    pub fn artifact_dir(&self, phase: &str) -> PathBuf {
        self.dir().join(ARTIFACTS_DIR).join(phase.to_lowercase())
    }

    /// Saves `contents` as `artifacts/<phase>/<name>`, returning the path.
    /// `name` must be a plain file name.
    pub fn write_artifact(&self, phase: &str, name: &str, contents: &str) -> io::Result<PathBuf> {
        if !is_plain_name(name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the name must be a plain file name such as plan.md, without folders",
            ));
        }
        let dir = self.artifact_dir(phase);
        fs::create_dir_all(&dir)?;
        let path = dir.join(name);
        fs::write(&path, contents)?;
        Ok(path)
    }

    /// Every artifact written so far, in phase then name order.
    pub fn list_artifacts(&self) -> Vec<PathBuf> {
        let root = self.dir().join(ARTIFACTS_DIR);
        let mut found = Vec::new();
        for phase in sorted_entries(&root) {
            found.extend(sorted_entries(&phase).into_iter().filter(|p| p.is_file()));
        }
        found
    }
}

fn is_plain_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\'])
}

fn sorted_entries(dir: &std::path::Path) -> Vec<PathBuf> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<PathBuf> = read.flatten().map(|e| e.path()).collect();
    entries.sort();
    entries
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::UNIX_EPOCH;

    use tempfile::TempDir;

    use super::*;

    fn session(root: &Path) -> Session {
        Session::create(root, UNIX_EPOCH, Path::new("/work"), "m").unwrap()
    }

    #[test]
    fn artifacts_land_in_a_folder_per_phase() {
        let root = TempDir::new().unwrap();
        let session = session(root.path());
        let path = session
            .write_artifact("Plan", "plan.md", "1. do it")
            .unwrap();
        assert_eq!(path, session.dir().join("artifacts/plan/plan.md"));
        assert_eq!(fs::read_to_string(path).unwrap(), "1. do it");
    }

    #[test]
    fn names_with_folders_are_rejected() {
        let root = TempDir::new().unwrap();
        let session = session(root.path());
        for name in ["", "../x", "a/b", "a\\b", ".hidden", ".."] {
            assert!(session.write_artifact("Plan", name, "x").is_err(), "{name}");
        }
    }

    #[test]
    fn listing_is_ordered_and_ignores_raw_outputs() {
        let root = TempDir::new().unwrap();
        let session = session(root.path());
        session.write_artifact("Plan", "plan.md", "p").unwrap();
        session
            .write_artifact("Discover", "findings.md", "f")
            .unwrap();
        session
            .write_phase_output("plan", "strategy-3.txt", "raw")
            .unwrap();
        let names: Vec<_> = session
            .list_artifacts()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["findings.md", "plan.md"]);
    }

    #[test]
    fn listing_is_empty_before_any_artifact() {
        let root = TempDir::new().unwrap();
        assert!(session(root.path()).list_artifacts().is_empty());
    }
}
