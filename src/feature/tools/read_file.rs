use std::path::PathBuf;

use serde_json::{Value, json};

use crate::feature::provider::BoxFuture;
use crate::feature::tools::agent_tool::AgentTool;
use crate::feature::tools::args::{resolve_path, string_arg};
use crate::feature::tools::output::{MAX_OUTPUT_CHARS, truncate};

pub struct ReadFile {
    working_dir: PathBuf,
}

impl ReadFile {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

impl AgentTool for ReadFile {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read a text file and return its contents."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "File to read" } },
            "required": ["path"]
        })
    }

    fn call<'a>(&'a self, args: Value) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let path = resolve_path(&self.working_dir, &string_arg(&args, "path")?);
            let text = tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| format!("could not read {}: {e}", path.display()))?;
            Ok(truncate(text, MAX_OUTPUT_CHARS))
        })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[tokio::test]
    async fn reads_a_file() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hello").unwrap();
        let tool = ReadFile::new(dir.path().to_path_buf());
        assert_eq!(
            tool.call(json!({ "path": "a.txt" })).await.unwrap(),
            "hello"
        );
    }

    #[tokio::test]
    async fn missing_file_is_an_error_naming_the_path() {
        let dir = TempDir::new().unwrap();
        let tool = ReadFile::new(dir.path().to_path_buf());
        let err = tool.call(json!({ "path": "nope.txt" })).await.unwrap_err();
        assert!(err.contains("nope.txt"));
    }

    #[tokio::test]
    async fn large_files_are_truncated() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("big"), "x".repeat(MAX_OUTPUT_CHARS + 5)).unwrap();
        let tool = ReadFile::new(dir.path().to_path_buf());
        let out = tool.call(json!({ "path": "big" })).await.unwrap();
        assert!(out.contains("output truncated: 5 more characters"));
    }
}
