use std::path::{Path, PathBuf};

use serde_json::Value;

pub fn string_arg(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("missing or non-string argument '{key}'"))
}

/// Resolves `path` against `working_dir`; an absolute `path` wins.
pub fn resolve_path(working_dir: &Path, path: &str) -> PathBuf {
    working_dir.join(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn string_arg_reads_a_string() {
        assert_eq!(string_arg(&json!({ "a": "b" }), "a").unwrap(), "b");
    }

    #[test]
    fn string_arg_rejects_missing_and_non_string() {
        assert!(string_arg(&json!({}), "a").is_err());
        assert!(string_arg(&json!({ "a": 3 }), "a").is_err());
    }

    #[test]
    fn relative_paths_join_and_absolute_paths_win() {
        let base = Path::new("/work");
        assert_eq!(resolve_path(base, "a/b"), Path::new("/work/a/b"));
        assert_eq!(resolve_path(base, "/etc/x"), Path::new("/etc/x"));
    }
}
