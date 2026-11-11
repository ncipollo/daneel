use serde_json::Value;

use crate::feature::provider::BoxFuture;

/// A tool the agent can call. `call` returns `Err` for failures the model should
/// see and recover from; the registry turns that into tool output.
pub trait AgentTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> Value;
    fn call<'a>(&'a self, args: Value) -> BoxFuture<'a, Result<String, String>>;
}
