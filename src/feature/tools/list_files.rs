use std::path::PathBuf;

use serde_json::{Value, json};

use crate::feature::provider::BoxFuture;
use crate::feature::tools::agent_tool::AgentTool;
use crate::feature::tools::args::resolve_path;
use crate::feature::tools::output::{MAX_OUTPUT_CHARS, truncate};

pub struct ListFiles {
    working_dir: PathBuf,
}

impl ListFiles {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

impl AgentTool for ListFiles {
    fn name(&self) -> &str {
        "list_files"
    }

    fn description(&self) -> &str {
        "List the files and folders directly inside a folder. Folders end with '/'."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Folder to list; defaults to the project folder" }
            }
        })
    }

    fn call<'a>(&'a self, args: Value) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let given = args.get("path").and_then(Value::as_str).unwrap_or(".");
            let path = resolve_path(&self.working_dir, given);
            let mut entries = tokio::fs::read_dir(&path)
                .await
                .map_err(|e| format!("could not list {}: {e}", path.display()))?;
            let mut names = Vec::new();
            while let Ok(Some(entry)) = entries.next_entry().await {
                let is_dir = entry.file_type().await.is_ok_and(|t| t.is_dir());
                let name = entry.file_name().to_string_lossy().into_owned();
                names.push(if is_dir { format!("{name}/") } else { name });
            }
            names.sort();
            Ok(truncate(names.join("\n"), MAX_OUTPUT_CHARS))
        })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[tokio::test]
    async fn lists_files_and_marks_folders() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("b.txt"), "").unwrap();
        std::fs::create_dir(dir.path().join("a")).unwrap();
        let tool = ListFiles::new(dir.path().to_path_buf());
        assert_eq!(tool.call(json!({})).await.unwrap(), "a/\nb.txt");
    }

    #[tokio::test]
    async fn a_missing_folder_is_an_error_naming_the_path() {
        let dir = TempDir::new().unwrap();
        let tool = ListFiles::new(dir.path().to_path_buf());
        let err = tool.call(json!({ "path": "nope" })).await.unwrap_err();
        assert!(err.contains("nope"));
    }
}
