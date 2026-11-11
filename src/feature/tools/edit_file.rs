use std::path::PathBuf;

use serde_json::{Value, json};

use crate::feature::provider::BoxFuture;
use crate::feature::tools::agent_tool::AgentTool;
use crate::feature::tools::args::{resolve_path, string_arg};

pub struct EditFile {
    working_dir: PathBuf,
}

impl EditFile {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

/// Replaces `old` with `new` in `text`, requiring exactly one match.
fn replace_unique(text: &str, old: &str, new: &str, path: &str) -> Result<String, String> {
    if old.is_empty() {
        return Err("old_text must not be empty".to_string());
    }
    match text.matches(old).count() {
        0 => Err(format!("old_text not found in {path}")),
        1 => Ok(text.replacen(old, new, 1)),
        n => Err(format!(
            "old_text matches {n} times in {path}; include more surrounding text to make it unique"
        )),
    }
}

impl AgentTool for EditFile {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Replace one piece of text in a file. old_text must match exactly once."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File to edit" },
                "old_text": { "type": "string", "description": "Exact text to replace" },
                "new_text": { "type": "string", "description": "Replacement text" }
            },
            "required": ["path", "old_text", "new_text"]
        })
    }

    fn call<'a>(&'a self, args: Value) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let name = string_arg(&args, "path")?;
            let old = string_arg(&args, "old_text")?;
            let new = string_arg(&args, "new_text")?;
            let path = resolve_path(&self.working_dir, &name);
            let text = tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| format!("could not read {}: {e}", path.display()))?;
            let edited = replace_unique(&text, &old, &new, &name)?;
            tokio::fs::write(&path, edited)
                .await
                .map_err(|e| format!("could not write {}: {e}", path.display()))?;
            Ok(format!("edited {name}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    async fn edit(dir: &TempDir, old: &str, new: &str) -> Result<String, String> {
        let tool = EditFile::new(dir.path().to_path_buf());
        tool.call(json!({ "path": "a.txt", "old_text": old, "new_text": new }))
            .await
    }

    fn file(dir: &TempDir, contents: &str) {
        std::fs::write(dir.path().join("a.txt"), contents).unwrap();
    }

    #[tokio::test]
    async fn replaces_a_unique_match() {
        let dir = TempDir::new().unwrap();
        file(&dir, "one two three");
        edit(&dir, "two", "2").await.unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "one 2 three"
        );
    }

    #[tokio::test]
    async fn no_match_is_an_error_and_leaves_the_file() {
        let dir = TempDir::new().unwrap();
        file(&dir, "one");
        let err = edit(&dir, "zzz", "x").await.unwrap_err();
        assert!(err.contains("not found"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "one"
        );
    }

    #[tokio::test]
    async fn multiple_matches_are_an_error_with_the_count() {
        let dir = TempDir::new().unwrap();
        file(&dir, "a a a");
        let err = edit(&dir, "a", "b").await.unwrap_err();
        assert!(err.contains("matches 3 times"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "a a a"
        );
    }

    #[tokio::test]
    async fn empty_old_text_is_rejected() {
        let dir = TempDir::new().unwrap();
        file(&dir, "abc");
        assert!(edit(&dir, "", "x").await.is_err());
    }
}
