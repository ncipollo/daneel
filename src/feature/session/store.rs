use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::feature::session::metadata::SessionMetadata;

const MESSAGES_FILE: &str = "messages.jsonl";
const TASKS_FILE: &str = "tasks.jsonl";
const PHASES_DIR: &str = "phases";

/// One session directory. Messages and tasks are appended as JSON lines, so a
/// crash can at worst lose the line being written.
#[derive(Debug, Clone)]
pub struct Session {
    dir: PathBuf,
    metadata: SessionMetadata,
}

#[derive(Serialize)]
struct MessageLine<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct TaskLine<'a> {
    task: &'a str,
}

impl Session {
    pub fn create(
        root: &Path,
        now: SystemTime,
        working_dir: &Path,
        model: &str,
    ) -> io::Result<Self> {
        fs::create_dir_all(root)?;
        let since_epoch = now.duration_since(UNIX_EPOCH).unwrap_or_default();
        let (id, dir) = claim_dir(root, since_epoch.as_millis())?;
        let metadata = SessionMetadata {
            id,
            created_at_secs: since_epoch.as_secs(),
            working_dir: working_dir.to_path_buf(),
            model: model.to_string(),
        };
        metadata.write(&dir)?;
        Ok(Self { dir, metadata })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn metadata(&self) -> &SessionMetadata {
        &self.metadata
    }

    pub fn append_message(&self, role: &str, content: &str) -> io::Result<()> {
        append_line(
            &self.dir.join(MESSAGES_FILE),
            &MessageLine { role, content },
        )
    }

    pub fn append_task(&self, task: &str) -> io::Result<()> {
        append_line(&self.dir.join(TASKS_FILE), &TaskLine { task })
    }

    /// Saves a strategy/tactic phase output as `phases/<phase>/<name>`.
    pub fn write_phase_output(&self, phase: &str, name: &str, contents: &str) -> io::Result<()> {
        let dir = self.dir.join(PHASES_DIR).join(phase);
        fs::create_dir_all(&dir)?;
        fs::write(dir.join(name), contents)
    }
}

/// Creates a fresh `<millis>-<n>` directory; `create_dir` fails on collision so ids stay unique.
fn claim_dir(root: &Path, millis: u128) -> io::Result<(String, PathBuf)> {
    for n in 0u32.. {
        let id = format!("{millis}-{n}");
        let dir = root.join(&id);
        match fs::create_dir(&dir) {
            Ok(()) => return Ok((id, dir)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    unreachable!("the counter does not run out")
}

fn append_line(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let mut line = serde_json::to_vec(value)?;
    line.push(b'\n');
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(&line)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tempfile::TempDir;

    use super::*;

    fn create(root: &Path) -> Session {
        let now = UNIX_EPOCH + Duration::from_secs(42);
        Session::create(root, now, Path::new("/work"), "qwen").unwrap()
    }

    #[test]
    fn create_writes_metadata_with_timestamp() {
        let root = TempDir::new().unwrap();
        let session = create(root.path());
        let stored = SessionMetadata::read(session.dir()).unwrap();
        assert_eq!(stored.created_at_secs, 42);
        assert_eq!(
            (stored.model.as_str(), stored.working_dir.clone()),
            ("qwen", PathBuf::from("/work"))
        );
        assert_eq!(&stored, session.metadata());
    }

    #[test]
    fn sessions_created_in_the_same_instant_get_distinct_dirs() {
        let root = TempDir::new().unwrap();
        assert_ne!(create(root.path()).dir(), create(root.path()).dir());
    }

    #[test]
    fn messages_append_as_json_lines() {
        let root = TempDir::new().unwrap();
        let session = create(root.path());
        session.append_message("user", "hi").unwrap();
        session.append_message("assistant", "yo").unwrap();
        let text = fs::read_to_string(session.dir().join("messages.jsonl")).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], r#"{"role":"user","content":"hi"}"#);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn tasks_and_phase_outputs_are_stored_as_artifacts() {
        let root = TempDir::new().unwrap();
        let session = create(root.path());
        session.append_task("fix the bug").unwrap();
        session
            .write_phase_output("strategy", "plan.md", "# plan")
            .unwrap();
        let tasks = fs::read_to_string(session.dir().join("tasks.jsonl")).unwrap();
        assert_eq!(tasks.trim(), r#"{"task":"fix the bug"}"#);
        let plan = fs::read_to_string(session.dir().join("phases/strategy/plan.md")).unwrap();
        assert_eq!(plan, "# plan");
    }
}
