//! Safe-tools mode: before a write or shell call runs, a separate agent process
//! reviews it and can veto it. Anything but a clear "safe" verdict blocks the call.

pub mod process;
pub mod prompt;
pub mod verdict;
pub mod wrapper;

use std::path::Path;
use std::sync::Arc;

use crate::feature::config::Settings;
use crate::feature::provider::BoxFuture;
use crate::feature::tools::ToolRegistry;

pub use process::ProcessValidator;
pub use verdict::{Verdict, parse_verdict};
pub use wrapper::SafetyWrapper;

/// The tools that change things and so need a second opinion.
pub const GUARDED: [&str; 3] = ["write_file", "edit_file", "invoke_shell"];

/// Something that can judge a proposed tool call.
pub trait Validator: Send + Sync {
    fn review<'a>(&'a self, request: &'a str) -> BoxFuture<'a, Result<Verdict, String>>;
}

/// The validator configured in `settings`, if safe-tools mode is on.
pub fn validator_from(settings: &Settings) -> Option<Arc<dyn Validator>> {
    let safe = settings.safe_tools.as_ref().filter(|safe| safe.enabled)?;
    Some(Arc::new(ProcessValidator::from_settings(safe)))
}

/// Wraps the write and shell tools in `registry` so each call is reviewed first.
pub fn guard(registry: &mut ToolRegistry, validator: &Arc<dyn Validator>, working_dir: &Path) {
    registry.map_tools(|tool| {
        if !GUARDED.contains(&tool.name()) {
            return tool;
        }
        let wrapper = SafetyWrapper::new(tool, Arc::clone(validator), working_dir.to_path_buf());
        Arc::new(wrapper)
    });
}

/// Test validator that returns a fixed answer and remembers what it was asked.
#[cfg(test)]
pub struct FixedValidator {
    pub answer: Result<Verdict, String>,
    pub requests: std::sync::Mutex<Vec<String>>,
}

#[cfg(test)]
impl FixedValidator {
    pub fn new(answer: Result<Verdict, String>) -> Self {
        Self {
            answer,
            requests: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn safe() -> Self {
        Self::new(Ok(Verdict {
            safe: true,
            findings: Vec::new(),
        }))
    }

    pub fn unsafe_because(finding: &str) -> Self {
        Self::new(Ok(Verdict {
            safe: false,
            findings: vec![finding.to_string()],
        }))
    }
}

#[cfg(test)]
impl Validator for FixedValidator {
    fn review<'a>(&'a self, request: &'a str) -> BoxFuture<'a, Result<Verdict, String>> {
        self.requests.lock().unwrap().push(request.to_string());
        let answer = self.answer.clone();
        Box::pin(async move { answer })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;
    use crate::feature::config::SafeToolsSettings;
    use crate::feature::tools::builtin_registry;

    fn guarded(validator: FixedValidator, dir: &Path) -> (ToolRegistry, Arc<FixedValidator>) {
        let validator = Arc::new(validator);
        let mut registry = builtin_registry(dir);
        let as_dyn: Arc<dyn Validator> = validator.clone();
        guard(&mut registry, &as_dyn, dir);
        (registry, validator)
    }

    #[tokio::test]
    async fn an_unsafe_verdict_blocks_the_write() {
        let dir = TempDir::new().unwrap();
        let (registry, _) = guarded(
            FixedValidator::unsafe_because("overwrites secrets"),
            dir.path(),
        );
        let args = json!({ "path": "a.txt", "content": "x" });
        let out = registry.dispatch("write_file", args).await;
        assert!(out.starts_with("error: blocked by safety review: overwrites secrets"));
        assert!(!dir.path().join("a.txt").exists());
    }

    #[tokio::test]
    async fn a_safe_verdict_lets_the_call_through_and_sees_it() {
        let dir = TempDir::new().unwrap();
        let (registry, validator) = guarded(FixedValidator::safe(), dir.path());
        let args = json!({ "path": "a.txt", "content": "x" });
        let out = registry.dispatch("write_file", args).await;
        assert!(!out.starts_with("error"), "{out}");
        assert!(dir.path().join("a.txt").exists());
        let requests = validator.requests.lock().unwrap();
        assert!(requests[0].contains("Tool: write_file") && requests[0].contains("a.txt"));
    }

    #[tokio::test]
    async fn a_dead_validator_blocks_everything_guarded() {
        let dir = TempDir::new().unwrap();
        let (registry, _) = guarded(FixedValidator::new(Err("down".into())), dir.path());
        let out = registry
            .dispatch("invoke_shell", json!({ "command": "touch f" }))
            .await;
        assert!(out.contains("blocked by safety review") && out.contains("down"));
        assert!(!dir.path().join("f").exists());
    }

    #[tokio::test]
    async fn reading_is_never_reviewed() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("r.txt"), "hello").unwrap();
        let (registry, validator) = guarded(FixedValidator::unsafe_because("no"), dir.path());
        let out = registry
            .dispatch("read_file", json!({ "path": "r.txt" }))
            .await;
        assert_eq!(out, "hello");
        assert!(validator.requests.lock().unwrap().is_empty());
    }

    #[test]
    fn the_validator_is_built_only_when_enabled() {
        let mut settings = Settings::default();
        assert!(validator_from(&settings).is_none());
        settings.safe_tools = Some(SafeToolsSettings::default());
        assert!(validator_from(&settings).is_none());
        settings.safe_tools = Some(SafeToolsSettings {
            enabled: true,
            ..SafeToolsSettings::default()
        });
        assert!(validator_from(&settings).is_some());
    }
}
