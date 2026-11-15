use serde_json::{Value, json};

use crate::feature::provider::BoxFuture;
use crate::feature::session::Session;
use crate::feature::tools::agent_tool::AgentTool;
use crate::feature::tools::args::string_arg;

/// Saves a deliverable in the session folder for later phases. Works in every
/// phase, including read-only ones, because it never touches the working directory.
pub struct WriteArtifact {
    session: Session,
    phase: String,
}

impl WriteArtifact {
    pub fn new(session: Session, phase: impl Into<String>) -> Self {
        Self {
            session,
            phase: phase.into(),
        }
    }
}

impl AgentTool for WriteArtifact {
    fn name(&self) -> &str {
        "write_artifact"
    }

    fn description(&self) -> &str {
        "Save a markdown deliverable for this phase so later phases can read it. Does not change the working directory."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "File name such as findings.md, no folders" },
                "content": { "type": "string", "description": "The full contents of the file" }
            },
            "required": ["name", "content"]
        })
    }

    fn call<'a>(&'a self, args: Value) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let name = string_arg(&args, "name")?;
            let content = string_arg(&args, "content")?;
            let path = self
                .session
                .write_artifact(&self.phase, &name, &content)
                .map_err(|e| format!("could not save {name}: {e}"))?;
            Ok(format!("saved {}", path.display()))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::UNIX_EPOCH;

    use tempfile::TempDir;

    use super::*;

    fn tool(root: &Path) -> WriteArtifact {
        let session = Session::create(root, UNIX_EPOCH, Path::new("/w"), "m").unwrap();
        WriteArtifact::new(session, "Plan")
    }

    #[tokio::test]
    async fn saves_into_the_phase_folder() {
        let root = TempDir::new().unwrap();
        let out = tool(root.path())
            .call(json!({ "name": "plan.md", "content": "1. go" }))
            .await
            .unwrap();
        assert!(out.starts_with("saved ") && out.ends_with("artifacts/plan/plan.md"));
    }

    #[tokio::test]
    async fn bad_names_come_back_as_errors() {
        let root = TempDir::new().unwrap();
        let err = tool(root.path())
            .call(json!({ "name": "../x", "content": "." }))
            .await
            .unwrap_err();
        assert!(err.contains("plain file name"));
    }
}
