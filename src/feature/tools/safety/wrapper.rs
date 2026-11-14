use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use crate::feature::provider::BoxFuture;
use crate::feature::tools::AgentTool;
use crate::feature::tools::safety::Validator;
use crate::feature::tools::safety::prompt::review_prompt;

/// Runs a tool only after the validator calls it safe. Every other outcome
/// (unsafe, validator down, unreadable reply) blocks the call, and the reason
/// goes back to the model as the tool's error.
pub struct SafetyWrapper {
    inner: Arc<dyn AgentTool>,
    validator: Arc<dyn Validator>,
    working_dir: PathBuf,
}

impl SafetyWrapper {
    pub fn new(
        inner: Arc<dyn AgentTool>,
        validator: Arc<dyn Validator>,
        working_dir: PathBuf,
    ) -> Self {
        Self {
            inner,
            validator,
            working_dir,
        }
    }

    async fn check(&self, args: &Value) -> Result<(), String> {
        let tool = self.inner.name();
        let request = review_prompt(tool, args, &self.working_dir);
        match self.validator.review(&request).await {
            Ok(verdict) if verdict.safe => {
                tracing::info!(tool, "safety review passed");
                Ok(())
            }
            Ok(verdict) => {
                let findings = verdict.findings.join("; ");
                tracing::warn!(
                    tool,
                    findings = findings.as_str(),
                    "safety review blocked a tool call"
                );
                Err(blocked(&findings))
            }
            Err(reason) => {
                tracing::warn!(
                    tool,
                    reason = reason.as_str(),
                    "safety review unavailable, blocking"
                );
                Err(blocked(&format!(
                    "the reviewer could not be reached ({reason})"
                )))
            }
        }
    }
}

fn blocked(why: &str) -> String {
    let why = if why.is_empty() {
        "no reason given"
    } else {
        why
    };
    format!("blocked by safety review: {why}. Choose a safer approach.")
}

impl AgentTool for SafetyWrapper {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn schema(&self) -> Value {
        self.inner.schema()
    }

    fn call<'a>(&'a self, args: Value) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            self.check(&args).await?;
            self.inner.call(args).await
        })
    }
}
