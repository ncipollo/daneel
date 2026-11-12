use std::collections::BTreeMap;
use std::sync::Arc;

use rig::completion::ToolDefinition;
use serde_json::Value;

use crate::feature::tools::agent_tool::AgentTool;

/// Holds the available tools and is the one place tool calls are dispatched.
#[derive(Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, Arc<dyn AgentTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a tool, replacing any existing tool with the same name.
    pub fn register(&mut self, tool: impl AgentTool + 'static) {
        self.tools.insert(tool.name().to_string(), Arc::new(tool));
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.tools.keys().map(String::as_str)
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|tool| definition_of(tool.as_ref()))
            .collect()
    }

    /// Runs a tool by name. Failures come back as text for the model, never as a panic.
    pub async fn dispatch(&self, name: &str, args: Value) -> String {
        match self.tools.get(name) {
            None => format!("error: unknown tool '{name}'"),
            Some(tool) => tool
                .call(args)
                .await
                .unwrap_or_else(|message| format!("error: {message}")),
        }
    }
}

fn definition_of(tool: &dyn AgentTool) -> ToolDefinition {
    ToolDefinition {
        name: tool.name().to_string(),
        description: tool.description().to_string(),
        parameters: tool.schema(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::feature::provider::BoxFuture;

    struct Stub(&'static str, Result<&'static str, &'static str>);

    impl AgentTool for Stub {
        fn name(&self) -> &str {
            self.0
        }
        fn description(&self) -> &str {
            "stub"
        }
        fn schema(&self) -> Value {
            json!({ "type": "object" })
        }
        fn call<'a>(&'a self, _args: Value) -> BoxFuture<'a, Result<String, String>> {
            let result = self.1.map(str::to_string).map_err(str::to_string);
            Box::pin(async move { result })
        }
    }

    #[tokio::test]
    async fn dispatch_runs_the_named_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(Stub("a", Ok("done")));
        assert_eq!(registry.dispatch("a", json!({})).await, "done");
    }

    #[tokio::test]
    async fn tool_errors_become_output() {
        let mut registry = ToolRegistry::new();
        registry.register(Stub("a", Err("boom")));
        assert_eq!(registry.dispatch("a", json!({})).await, "error: boom");
    }

    #[tokio::test]
    async fn unknown_tool_becomes_output() {
        let registry = ToolRegistry::new();
        let out = registry.dispatch("nope", json!({})).await;
        assert_eq!(out, "error: unknown tool 'nope'");
    }

    #[tokio::test]
    async fn registering_twice_replaces_the_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(Stub("a", Ok("old")));
        registry.register(Stub("a", Ok("new")));
        assert_eq!(registry.dispatch("a", json!({})).await, "new");
        assert_eq!(registry.names().count(), 1);
    }

    #[test]
    fn definitions_describe_each_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(Stub("a", Ok("")));
        let defs = registry.definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "a");
        assert_eq!(defs[0].description, "stub");
    }
}
