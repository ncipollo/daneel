use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::feature::config::SafeToolsSettings;
use crate::feature::provider::BoxFuture;
use crate::feature::tools::safety::Validator;
use crate::feature::tools::safety::verdict::{Verdict, parse_verdict};

/// Asks an external agent CLI (such as `claude -p`) for a verdict. The command
/// is run directly, not through a shell, with the request on stdin.
pub struct ProcessValidator {
    command: String,
    args: Vec<String>,
    timeout: Duration,
}

impl ProcessValidator {
    pub fn new(command: String, args: Vec<String>, timeout: Duration) -> Self {
        Self {
            command,
            args,
            timeout,
        }
    }

    pub fn from_settings(settings: &SafeToolsSettings) -> Self {
        let timeout = Duration::from_secs(settings.timeout_secs);
        Self::new(settings.command.clone(), settings.args.clone(), timeout)
    }

    async fn ask(&self, request: &str) -> Result<String, String> {
        let mut child = Command::new(&self.command)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("could not start validator '{}': {e}", self.command))?;
        if let Some(mut stdin) = child.stdin.take() {
            // A validator that ignores stdin closes the pipe early; that is not an error.
            let _ = stdin.write_all(request.as_bytes()).await;
        }
        let output = child
            .wait_with_output()
            .await
            .map_err(|e| format!("validator failed: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "validator exited with {}: {}",
                output.status,
                stderr.trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

impl Validator for ProcessValidator {
    fn review<'a>(&'a self, request: &'a str) -> BoxFuture<'a, Result<Verdict, String>> {
        Box::pin(async move {
            let reply = tokio::time::timeout(self.timeout, self.ask(request))
                .await
                .map_err(|_| format!("validator timed out after {}s", self.timeout.as_secs()))??;
            parse_verdict(&reply)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(script: &str, timeout: Duration) -> ProcessValidator {
        let args = vec!["-c".to_string(), script.to_string()];
        ProcessValidator::new("sh".to_string(), args, timeout)
    }

    async fn review(script: &str) -> Result<Verdict, String> {
        sh(script, Duration::from_secs(10))
            .review("check this")
            .await
    }

    #[tokio::test]
    async fn a_safe_verdict_comes_back() {
        let verdict = review(r#"cat >/dev/null; echo '{"safe":true,"findings":[]}'"#).await;
        assert!(verdict.unwrap().safe);
    }

    #[tokio::test]
    async fn the_request_is_sent_on_stdin() {
        let script = r#"if grep -q 'check this'; then echo '{"safe":false,"findings":["saw it"]}'; else echo '{"safe":true}'; fi"#;
        let verdict = review(script).await.unwrap();
        assert!(!verdict.safe);
        assert_eq!(verdict.findings, ["saw it"]);
    }

    #[tokio::test]
    async fn garbage_output_is_an_error() {
        assert!(review("echo nope").await.is_err());
    }

    #[tokio::test]
    async fn a_failing_validator_is_an_error() {
        let err = review("echo boom >&2; exit 3").await.unwrap_err();
        assert!(err.contains("exited") && err.contains("boom"));
    }

    #[tokio::test]
    async fn a_missing_binary_is_an_error() {
        let validator = ProcessValidator::new(
            "no-such-validator-xyz".into(),
            vec![],
            Duration::from_secs(5),
        );
        let err = validator.review("x").await.unwrap_err();
        assert!(err.contains("could not start validator"));
    }

    #[tokio::test]
    async fn a_slow_validator_times_out() {
        let validator = sh("sleep 5", Duration::from_millis(100));
        let err = validator.review("x").await.unwrap_err();
        assert!(err.contains("timed out"));
    }
}
