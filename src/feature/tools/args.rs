use std::path::{Component, Path, PathBuf};

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

/// Like `resolve_path`, but refuses anything that lands outside `working_dir`,
/// whether by an absolute path, `..`, or a symlink. Used by the writing tools.
pub fn resolve_inside(working_dir: &Path, path: &str) -> Result<PathBuf, String> {
    let root = resolve_real(working_dir);
    let target = resolve_real(&resolve_path(working_dir, path));
    if target.starts_with(&root) {
        return Ok(target);
    }
    Err(format!(
        "refusing to touch {}: it is outside the working directory {}. Use a path inside it",
        target.display(),
        root.display()
    ))
}

/// Cleans `.` and `..` and follows symlinks through the deepest part that exists.
fn resolve_real(path: &Path) -> PathBuf {
    let clean = clean(path);
    let mut missing = Vec::new();
    let mut base = clean.as_path();
    while !base.exists() {
        let (Some(name), Some(parent)) = (base.file_name(), base.parent()) else {
            return clean;
        };
        missing.push(name.to_os_string());
        base = parent;
    }
    let mut real = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    real.extend(missing.iter().rev());
    real
}

fn clean(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use serde_json::json;

    #[test]
    fn resolve_inside_allows_new_and_existing_paths_in_the_working_dir() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        assert_eq!(
            resolve_inside(dir.path(), "a/b.txt").unwrap(),
            root.join("a/b.txt")
        );
        let absolute = dir.path().join("c.txt");
        assert_eq!(
            resolve_inside(dir.path(), absolute.to_str().unwrap()).unwrap(),
            root.join("c.txt")
        );
        assert_eq!(
            resolve_inside(dir.path(), "a/../d.txt").unwrap(),
            root.join("d.txt")
        );
    }

    #[test]
    fn resolve_inside_refuses_paths_that_escape() {
        let dir = TempDir::new().unwrap();
        let sibling = TempDir::new().unwrap();
        let other = sibling.path().join("x.txt");
        for path in [
            "../x.txt",
            "a/../../x.txt",
            "/etc/x",
            other.to_str().unwrap(),
        ] {
            let err = resolve_inside(dir.path(), path).unwrap_err();
            assert!(
                err.contains("outside the working directory"),
                "{path}: {err}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn resolve_inside_refuses_a_symlink_that_points_out() {
        let dir = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
        assert!(resolve_inside(dir.path(), "link/x.txt").is_err());
    }

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
