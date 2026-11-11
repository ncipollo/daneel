use std::path::PathBuf;
use std::process::Output;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::process::Command;

use crate::feature::provider::BoxFuture;
use crate::feature::tools::agent_tool::AgentTool;
use crate::feature::tools::args::string_arg;
use crate::feature::tools::output::{MAX_OUTPUT_CHARS, truncate};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

pub struct InvokeShell {
    working_dir: PathBuf,
    timeout: Duration,
}

impl InvokeShell {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            working_dir,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    async fn run(&self, command: &str) -> Result<Output, String> {
        let child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.working_dir)
            .kill_on_drop(true)
            .output();
        match tokio::time::timeout(self.timeout, child).await {
            Ok(result) => result.map_err(|e| format!("could not run command: {e}")),
            Err(_) => Err(format!(
                "command timed out after {}s",
                self.timeout.as_secs()
            )),
        }
    }
}

fn format_output(output: &Output) -> String {
    let code = output
        .status
        .code()
        .map_or("terminated by signal".to_string(), |c| c.to_string());
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    format!(
        "exit code: {code}\nstdout:\n{}\nstderr:\n{}",
        truncate(stdout, MAX_OUTPUT_CHARS),
        truncate(stderr, MAX_OUTPUT_CHARS)
    )
}

impl AgentTool for InvokeShell {
    fn name(&self) -> &str {
        "invoke_shell"
    }

    fn description(&self) -> &str {
        "Run a shell command in the working directory. Returns exit code, stdout and stderr."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "command": { "type": "string", "description": "Command to run" } },
            "required": ["command"]
        })
    }

    fn call<'a>(&'a self, args: Value) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let command = string_arg(&args, "command")?;
            self.run(&command)
                .await
                .map(|output| format_output(&output))
        })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    async fn run(dir: &TempDir, command: &str) -> Result<String, String> {
        InvokeShell::new(dir.path().to_path_buf())
            .call(json!({ "command": command }))
            .await
    }

    #[tokio::test]
    async fn captures_stdout_and_exit_code() {
        let dir = TempDir::new().unwrap();
        let out = run(&dir, "echo hi").await.unwrap();
        assert!(out.starts_with("exit code: 0"));
        assert!(out.contains("stdout:\nhi"));
    }

    #[tokio::test]
    async fn captures_stderr_and_nonzero_exit() {
        let dir = TempDir::new().unwrap();
        let out = run(&dir, "echo oops >&2; exit 3").await.unwrap();
        assert!(out.starts_with("exit code: 3"));
        assert!(out.contains("stderr:\noops"));
    }

    #[tokio::test]
    async fn runs_in_the_working_directory() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("marker.txt"), "").unwrap();
        let out = run(&dir, "ls").await.unwrap();
        assert!(out.contains("marker.txt"));
    }

    #[tokio::test]
    async fn large_output_is_truncated() {
        let dir = TempDir::new().unwrap();
        let command = format!("head -c {} /dev/zero | tr '\\0' x", MAX_OUTPUT_CHARS + 10);
        let out = run(&dir, &command).await.unwrap();
        assert!(out.contains("output truncated: 10 more characters"));
    }

    #[tokio::test]
    async fn slow_commands_time_out() {
        let dir = TempDir::new().unwrap();
        let tool =
            InvokeShell::new(dir.path().to_path_buf()).with_timeout(Duration::from_millis(100));
        let err = tool
            .call(json!({ "command": "sleep 5" }))
            .await
            .unwrap_err();
        assert!(err.contains("timed out"));
    }
}
