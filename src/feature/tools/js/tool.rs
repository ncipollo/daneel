use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use tokio::runtime::Handle;

use crate::feature::provider::BoxFuture;
use crate::feature::tools::AgentTool;
use crate::feature::tools::js::engine::{self, ShellAccess};
use crate::feature::tools::js::shape::check_shape;

/// A tool defined by a script. The script runs on a blocking thread because
/// boa is synchronous, and its result must match the shape it declared.
pub struct JsTool {
    pub(super) name: String,
    pub(super) description: String,
    pub(super) schema: Value,
    pub(super) output: Option<Value>,
    pub(super) source: Arc<str>,
    pub(super) working_dir: PathBuf,
    pub(super) shell: Arc<dyn AgentTool>,
}

impl JsTool {
    async fn execute(&self, args: Value) -> Result<Value, String> {
        let source = Arc::clone(&self.source);
        let dir = self.working_dir.clone();
        let shell = ShellAccess {
            tool: Arc::clone(&self.shell),
            handle: Handle::current(),
        };
        tokio::task::spawn_blocking(move || engine::run(&source, &args, &dir, shell))
            .await
            .map_err(|e| format!("the script crashed: {e}"))?
    }
}

impl AgentTool for JsTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn schema(&self) -> Value {
        self.schema.clone()
    }

    fn call<'a>(&'a self, args: Value) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let value = self.execute(args).await?;
            if let Some(shape) = &self.output {
                check_shape(&value, shape).map_err(|e| format!("output did not match: {e}"))?;
            }
            Ok(match value {
                Value::String(text) => text,
                other => other.to_string(),
            })
        })
    }
}
