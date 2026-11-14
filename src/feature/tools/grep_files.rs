use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::feature::provider::BoxFuture;
use crate::feature::tools::agent_tool::AgentTool;
use crate::feature::tools::args::{resolve_path, string_arg};
use crate::feature::tools::output::{MAX_OUTPUT_CHARS, truncate};
use crate::feature::tools::search::{MAX_RESULTS, files_under, glob_matches, relative};

const MAX_LINE_CHARS: usize = 200;

pub struct GrepFiles {
    working_dir: PathBuf,
}

impl GrepFiles {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

impl AgentTool for GrepFiles {
    fn name(&self) -> &str {
        "grep_files"
    }

    fn description(&self) -> &str {
        "Search file contents for some text (case-insensitive, not a regex). Returns 'file:line: text'."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "Text to look for" },
                "path": { "type": "string", "description": "Folder to search; defaults to the project folder" },
                "glob": { "type": "string", "description": "Only search files whose name matches, e.g. '*.rs'" }
            },
            "required": ["text"]
        })
    }

    fn call<'a>(&'a self, args: Value) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let text = string_arg(&args, "text")?;
            let given = args.get("path").and_then(Value::as_str).unwrap_or(".");
            let glob = args.get("glob").and_then(Value::as_str).map(str::to_string);
            let root = resolve_path(&self.working_dir, given);
            let hits = tokio::task::spawn_blocking(move || search(&root, &text, glob.as_deref()))
                .await
                .map_err(|e| e.to_string())?;
            if hits.is_empty() {
                return Ok("no matches".to_string());
            }
            Ok(truncate(hits.join("\n"), MAX_OUTPUT_CHARS))
        })
    }
}

fn search(root: &Path, text: &str, glob: Option<&str>) -> Vec<String> {
    let needle = text.to_lowercase();
    let mut hits = Vec::new();
    for file in files_under(root) {
        let name = file.file_name().unwrap_or_default().to_string_lossy();
        if glob.is_some_and(|pattern| !glob_matches(pattern, &name)) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };
        for (number, line) in content.lines().enumerate() {
            if line.to_lowercase().contains(&needle) {
                let shown: String = line.trim().chars().take(MAX_LINE_CHARS).collect();
                hits.push(format!("{}:{}: {shown}", relative(root, &file), number + 1));
            }
        }
        if hits.len() >= MAX_RESULTS {
            hits.truncate(MAX_RESULTS);
            break;
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn setup() -> (TempDir, GrepFiles) {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn main() {}\nlet Needle = 1;\n").unwrap();
        std::fs::write(dir.path().join("b.txt"), "needle here\n").unwrap();
        std::fs::write(dir.path().join("bin"), [0xff, 0xfe, 0x00]).unwrap();
        let tool = GrepFiles::new(dir.path().to_path_buf());
        (dir, tool)
    }

    #[tokio::test]
    async fn finds_lines_with_file_and_number() {
        let (_dir, tool) = setup();
        let out = tool.call(json!({ "text": "needle" })).await.unwrap();
        assert_eq!(out, "a.rs:2: let Needle = 1;\nb.txt:1: needle here");
    }

    #[tokio::test]
    async fn the_glob_limits_which_files_are_searched() {
        let (_dir, tool) = setup();
        let out = tool
            .call(json!({ "text": "needle", "glob": "*.txt" }))
            .await
            .unwrap();
        assert_eq!(out, "b.txt:1: needle here");
        let none = tool.call(json!({ "text": "absent" })).await.unwrap();
        assert_eq!(none, "no matches");
    }
}
