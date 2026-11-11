use std::sync::Arc;

use rig::completion::ToolDefinition;
use rig::tool::{ToolDyn, ToolError};
use rig::wasm_compat::WasmBoxedFuture;
use serde_json::Value;

use crate::feature::tools::registry::ToolRegistry;

struct RigTool {
    definition: ToolDefinition,
    registry: Arc<ToolRegistry>,
}

impl ToolDyn for RigTool {
    fn name(&self) -> String {
        self.definition.name.clone()
    }

    fn definition<'a>(&'a self, _prompt: String) -> WasmBoxedFuture<'a, ToolDefinition> {
        Box::pin(async move { self.definition.clone() })
    }

    fn call<'a>(&'a self, args: String) -> WasmBoxedFuture<'a, Result<String, ToolError>> {
        Box::pin(async move {
            let output = match serde_json::from_str::<Value>(&args) {
                Ok(value) => self.registry.dispatch(&self.definition.name, value).await,
                Err(e) => format!("error: invalid arguments: {e}"),
            };
            Ok(output)
        })
    }
}

/// Exposes every tool in `registry` to rig. Calls still route through `dispatch`.
pub fn into_rig(registry: ToolRegistry) -> Vec<Box<dyn ToolDyn>> {
    let registry = Arc::new(registry);
    registry
        .definitions()
        .into_iter()
        .map(|definition| {
            Box::new(RigTool {
                definition,
                registry: Arc::clone(&registry),
            }) as Box<dyn ToolDyn>
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::provider::BoxFuture;
    use crate::feature::tools::agent_tool::AgentTool;

    struct Upper;

    impl AgentTool for Upper {
        fn name(&self) -> &str {
            "upper"
        }
        fn description(&self) -> &str {
            "uppercases text"
        }
        fn schema(&self) -> Value {
            serde_json::json!({ "type": "object" })
        }
        fn call<'a>(&'a self, args: Value) -> BoxFuture<'a, Result<String, String>> {
            Box::pin(async move { Ok(args["text"].as_str().unwrap_or("").to_uppercase()) })
        }
    }

    fn tools() -> Vec<Box<dyn ToolDyn>> {
        let mut registry = ToolRegistry::new();
        registry.register(Upper);
        into_rig(registry)
    }

    #[tokio::test]
    async fn definition_describes_the_tool() {
        let tools = tools();
        let definition = tools[0].definition(String::new()).await;
        assert_eq!(tools[0].name(), "upper");
        assert_eq!(definition.description, "uppercases text");
    }

    #[tokio::test]
    async fn call_routes_through_the_registry() {
        let tools = tools();
        let out = tools[0].call(r#"{"text":"hi"}"#.to_string()).await.unwrap();
        assert_eq!(out, "HI");
    }

    #[tokio::test]
    async fn invalid_json_becomes_output_not_an_error() {
        let tools = tools();
        let out = tools[0].call("{ nope".to_string()).await.unwrap();
        assert!(out.starts_with("error: invalid arguments"));
    }
}
