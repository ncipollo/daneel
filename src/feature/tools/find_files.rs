use std::path::PathBuf;

use serde_json::{Value, json};

use crate::feature::provider::BoxFuture;
use crate::feature::tools::agent_tool::AgentTool;
use crate::feature::tools::args::{resolve_path, string_arg};
use crate::feature::tools::output::{MAX_OUTPUT_CHARS, truncate};
use crate::feature::tools::search::{MAX_RESULTS, files_under, glob_matches, relative};

pub struct FindFiles {
    working_dir: PathBuf,
}

impl FindFiles {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

impl AgentTool for FindFiles {
    fn name(&self) -> &str {
        "find_files"
    }

    fn description(&self) -> &str {
        "Find files by name anywhere under a folder. Use * and ? in the pattern, e.g. '*.rs' or 'config*'."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "File name pattern, e.g. '*.toml'" },
                "path": { "type": "string", "description": "Folder to search; defaults to the project folder" }
            },
            "required": ["pattern"]
        })
    }

    fn call<'a>(&'a self, args: Value) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let pattern = string_arg(&args, "pattern")?;
            let given = args.get("path").and_then(Value::as_str).unwrap_or(".");
            let root = resolve_path(&self.working_dir, given);
            let found: Vec<String> = tokio::task::spawn_blocking(move || {
                files_under(&root)
                    .iter()
                    .filter(|file| {
                        let name = file.file_name().unwrap_or_default().to_string_lossy();
                        glob_matches(&pattern, &name)
                    })
                    .take(MAX_RESULTS)
                    .map(|file| relative(&root, file))
                    .collect()
            })
            .await
            .map_err(|e| e.to_string())?;
            if found.is_empty() {
                return Ok("no files found".to_string());
            }
            Ok(truncate(found.join("\n"), MAX_OUTPUT_CHARS))
        })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[tokio::test]
    async fn finds_files_by_name_pattern() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "").unwrap();
        let tool = FindFiles::new(dir.path().to_path_buf());
        let out = tool.call(json!({ "pattern": "*.rs" })).await.unwrap();
        assert_eq!(out, "src/main.rs");
        let none = tool.call(json!({ "pattern": "*.go" })).await.unwrap();
        assert_eq!(none, "no files found");
    }
}
