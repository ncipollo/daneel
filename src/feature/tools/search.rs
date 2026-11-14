//! What the read-only search tools share: a recursive walk that skips build and
//! VCS folders, and simple `*`/`?` name matching.

use std::path::{Path, PathBuf};

use walkdir::{DirEntry, WalkDir};

pub const MAX_RESULTS: usize = 200;
const SKIPPED_DIRS: [&str; 3] = [".git", "target", "node_modules"];

/// Every file under `root`, in a stable order.
pub fn files_under(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| !skipped(entry))
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(DirEntry::into_path)
        .collect()
}

fn skipped(entry: &DirEntry) -> bool {
    entry.depth() > 0
        && entry.file_type().is_dir()
        && SKIPPED_DIRS.iter().any(|name| entry.file_name() == *name)
}

/// Case-insensitive match of `name` against `pattern`, where `*` is any run
/// of characters and `?` is one character.
pub fn glob_matches(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.to_lowercase().chars().collect();
    let name: Vec<char> = name.to_lowercase().chars().collect();
    matches_from(&pattern, &name)
}

fn matches_from(pattern: &[char], name: &[char]) -> bool {
    match pattern.split_first() {
        None => name.is_empty(),
        Some(('*', rest)) => (0..=name.len()).any(|skip| matches_from(rest, &name[skip..])),
        Some((&c, rest)) => match name.split_first() {
            Some((&n, tail)) => (c == '?' || c == n) && matches_from(rest, tail),
            None => false,
        },
    }
}

/// `path` relative to `base`, for compact results.
pub fn relative(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs_support_star_and_question_mark() {
        assert!(glob_matches("*.rs", "Main.RS"));
        assert!(glob_matches("a?c", "abc"));
        assert!(!glob_matches("*.rs", "main.toml"));
        assert!(glob_matches("*", ""));
    }

    #[test]
    fn walks_skip_vcs_and_build_folders() {
        let dir = tempfile::TempDir::new().unwrap();
        for sub in [".git", "target", "src"] {
            std::fs::create_dir(dir.path().join(sub)).unwrap();
            std::fs::write(dir.path().join(sub).join("f.txt"), "x").unwrap();
        }
        let found = files_under(dir.path());
        assert_eq!(found, [dir.path().join("src/f.txt")]);
    }
}
