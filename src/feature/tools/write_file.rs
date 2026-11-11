use std::path::PathBuf;

use serde_json::{Value, json};

use crate::feature::provider::BoxFuture;
use crate::feature::tools::agent_tool::AgentTool;
use crate::feature::tools::args::{resolve_path, string_arg};

pub struct WriteFile {
    working_dir: PathBuf,
}

impl WriteFile {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

impl AgentTool for WriteFile {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write text to a file, creating it (and parent folders) or overwriting it."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File to write" },
                "content": { "type": "string", "description": "Full new contents" }
            },
            "required": ["path", "content"]
        })
    }

    fn call<'a>(&'a self, args: Value) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let path = resolve_path(&self.working_dir, &string_arg(&args, "path")?);
            let content = string_arg(&args, "content")?;
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
            }
            tokio::fs::write(&path, &content)
                .await
                .map_err(|e| format!("could not write {}: {e}", path.display()))?;
            Ok(format!(
                "wrote {} bytes to {}",
                content.len(),
                path.display()
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[tokio::test]
    async fn writes_and_overwrites() {
        let dir = TempDir::new().unwrap();
        let tool = WriteFile::new(dir.path().to_path_buf());
        tool.call(json!({ "path": "a.txt", "content": "one" }))
            .await
            .unwrap();
        tool.call(json!({ "path": "a.txt", "content": "two" }))
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "two"
        );
    }

    #[tokio::test]
    async fn creates_nested_directories() {
        let dir = TempDir::new().unwrap();
        let tool = WriteFile::new(dir.path().to_path_buf());
        tool.call(json!({ "path": "x/y/z.txt", "content": "hi" }))
            .await
            .unwrap();
        assert!(dir.path().join("x/y/z.txt").is_file());
    }

    #[tokio::test]
    async fn missing_content_is_an_error() {
        let dir = TempDir::new().unwrap();
        let tool = WriteFile::new(dir.path().to_path_buf());
        let err = tool.call(json!({ "path": "a.txt" })).await.unwrap_err();
        assert!(err.contains("content"));
    }
}
